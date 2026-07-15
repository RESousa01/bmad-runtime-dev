use std::collections::HashSet;

use desktop_runtime::{sha256_bytes, ContractId, UnixMillis};
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::{BrokerToken, CloudError};

const PROTOCOL_VERSION: &str = "sapphirus.auth-broker.v1";
const MAX_MESSAGE_BYTES: usize = 64 * 1024;
const MAX_SCOPES: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BrokerOperation {
    AcquireToken,
    SignOut,
}

#[derive(Debug)]
pub enum BrokerOutcome {
    Token(BrokerToken),
    SignedOut,
}

#[derive(Clone, Debug)]
pub struct BrokerProtocol {
    client_id: String,
    tenant_id: String,
    authority: String,
    scopes: Vec<String>,
}

impl BrokerProtocol {
    /// Creates a protocol configuration from sealed host identity settings.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::IdentityUnavailable`] when identifiers or scopes
    /// do not match the helper's strict allowlisted syntax.
    pub fn new<S, I>(
        client_id: impl Into<String>,
        tenant_id: impl Into<String>,
        scopes: I,
    ) -> Result<Self, CloudError>
    where
        S: Into<String>,
        I: IntoIterator<Item = S>,
    {
        let client_id = client_id.into();
        let tenant_id = tenant_id.into();
        let scopes: Vec<String> = scopes.into_iter().map(Into::into).collect();
        if !is_guid(&client_id)
            || !is_guid(&tenant_id)
            || scopes.is_empty()
            || scopes.len() > MAX_SCOPES
            || scopes.iter().any(|scope| !is_allowed_scope(scope))
        {
            return Err(CloudError::IdentityUnavailable);
        }
        let authority = format!("https://login.microsoftonline.com/{tenant_id}/v2.0");
        Ok(Self {
            client_id,
            tenant_id,
            authority,
            scopes,
        })
    }

    #[must_use]
    pub fn tenant_ref(&self) -> ContractId {
        opaque_ref("tenant", &self.tenant_id)
    }

    /// Creates a correlated interactive-token exchange.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::IdentityUnavailable`] for a malformed request
    /// identifier, account identifier, or zero parent window handle.
    pub fn acquire_exchange(
        &self,
        request_id: ContractId,
        parent_window_handle: u64,
        account_id: Option<&str>,
        allow_system_browser_fallback: bool,
    ) -> Result<BrokerExchange, CloudError> {
        if parent_window_handle == 0 || !valid_request_id(&request_id) {
            return Err(CloudError::IdentityUnavailable);
        }
        validate_account(account_id)?;
        self.exchange(
            request_id,
            BrokerOperation::AcquireToken,
            account_id,
            Some(format!("0x{parent_window_handle:x}")),
            allow_system_browser_fallback,
        )
    }

    /// Creates a correlated broker-cache cleanup exchange.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::IdentityUnavailable`] for a malformed request or
    /// account identifier.
    pub fn sign_out_exchange(
        &self,
        request_id: ContractId,
        account_id: Option<&str>,
    ) -> Result<BrokerExchange, CloudError> {
        if !valid_request_id(&request_id) {
            return Err(CloudError::IdentityUnavailable);
        }
        validate_account(account_id)?;
        self.exchange(
            request_id,
            BrokerOperation::SignOut,
            account_id,
            None,
            false,
        )
    }

    fn exchange(
        &self,
        request_id: ContractId,
        operation: BrokerOperation,
        account_id: Option<&str>,
        parent_window_handle: Option<String>,
        allow_system_browser_fallback: bool,
    ) -> Result<BrokerExchange, CloudError> {
        let request = BrokerRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: request_id.as_str(),
            operation: match operation {
                BrokerOperation::AcquireToken => "acquire_token",
                BrokerOperation::SignOut => "sign_out",
            },
            client_id: &self.client_id,
            authority: &self.authority,
            scopes: &self.scopes,
            account_id,
            parent_window_handle,
            allow_system_browser_fallback,
        };
        let payload = serde_json::to_vec(&request).map_err(|_| CloudError::IdentityUnavailable)?;
        if payload.len() > MAX_MESSAGE_BYTES {
            return Err(CloudError::IdentityUnavailable);
        }
        Ok(BrokerExchange {
            request_id,
            operation,
            expected_tenant_id: self.tenant_id.clone(),
            expected_account_id: account_id.map(str::to_owned),
            payload,
        })
    }
}

#[derive(Debug)]
pub struct BrokerExchange {
    request_id: ContractId,
    operation: BrokerOperation,
    expected_tenant_id: String,
    expected_account_id: Option<String>,
    payload: Vec<u8>,
}

impl BrokerExchange {
    #[must_use]
    pub fn request_payload(&self) -> &[u8] {
        &self.payload
    }

    /// Encodes the request using the helper's bounded big-endian frame.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::IdentityUnavailable`] if the payload length
    /// cannot be represented by the protocol.
    pub fn request_frame(&self) -> Result<Vec<u8>, CloudError> {
        let length = u32::try_from(self.payload.len())
            .map_err(|_| CloudError::IdentityUnavailable)?
            .to_be_bytes();
        let mut frame = Vec::with_capacity(self.payload.len() + length.len());
        frame.extend_from_slice(&length);
        frame.extend_from_slice(&self.payload);
        Ok(frame)
    }

    /// Validates one complete helper response and returns a secret-safe result.
    ///
    /// # Errors
    ///
    /// Returns a stable [`CloudError`] for malformed frames, substitutions,
    /// broker failures, or expired token material.
    pub fn accept_response(
        &self,
        frame: &[u8],
        now: UnixMillis,
    ) -> Result<BrokerOutcome, CloudError> {
        let payload = decode_frame(frame)?;
        let response = deserialize_unique_response(payload)?;
        if response.protocol_version != PROTOCOL_VERSION
            || response.request_id != self.request_id.as_str()
        {
            return Err(CloudError::IdentityUnavailable);
        }
        if !response.success {
            if response.access_token.is_some()
                || response.expires_on.is_some()
                || response.account_id.is_some()
                || response.tenant_id.is_some()
            {
                return Err(CloudError::IdentityUnavailable);
            }
            return Err(map_broker_failure(response.error_code.as_deref()));
        }
        if response.error_code.is_some() || response.retryable {
            return Err(CloudError::IdentityUnavailable);
        }
        match self.operation {
            BrokerOperation::AcquireToken => self.accept_token(response, now),
            BrokerOperation::SignOut => {
                if response.access_token.is_some()
                    || response.expires_on.is_some()
                    || response.account_id.is_some()
                    || response.tenant_id.is_some()
                {
                    return Err(CloudError::IdentityUnavailable);
                }
                Ok(BrokerOutcome::SignedOut)
            }
        }
    }

    fn accept_token(
        &self,
        response: BrokerResponse,
        now: UnixMillis,
    ) -> Result<BrokerOutcome, CloudError> {
        let access_token = response
            .access_token
            .ok_or(CloudError::IdentityUnavailable)?;
        let expires_at = parse_expiry(
            response
                .expires_on
                .as_deref()
                .ok_or(CloudError::IdentityUnavailable)?,
        )?;
        let account_id = response
            .account_id
            .as_deref()
            .ok_or(CloudError::IdentityUnavailable)?;
        let tenant_id = response
            .tenant_id
            .as_deref()
            .ok_or(CloudError::IdentityUnavailable)?;
        if !tenant_id.eq_ignore_ascii_case(&self.expected_tenant_id) {
            return Err(CloudError::TenantMismatch);
        }
        if self
            .expected_account_id
            .as_deref()
            .is_some_and(|expected| expected != account_id)
        {
            return Err(CloudError::ReauthenticationRequired);
        }
        validate_account(Some(account_id))?;
        if expires_at <= now {
            return Err(CloudError::ReauthenticationRequired);
        }
        let token = BrokerToken::new(
            access_token,
            opaque_ref("tenant", tenant_id),
            opaque_ref("account", account_id),
            expires_at,
        )?;
        Ok(BrokerOutcome::Token(token))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BrokerRequest<'a> {
    protocol_version: &'static str,
    request_id: &'a str,
    operation: &'static str,
    client_id: &'a str,
    authority: &'a str,
    scopes: &'a [String],
    account_id: Option<&'a str>,
    parent_window_handle: Option<String>,
    allow_system_browser_fallback: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BrokerResponse {
    protocol_version: String,
    request_id: String,
    success: bool,
    error_code: Option<String>,
    retryable: bool,
    access_token: Option<String>,
    expires_on: Option<String>,
    account_id: Option<String>,
    tenant_id: Option<String>,
}

struct UniqueObject(Map<String, Value>);

impl<'de> Deserialize<'de> for UniqueObject {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct UniqueObjectVisitor;

        impl<'de> Visitor<'de> for UniqueObjectVisitor {
            type Value = UniqueObject;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a JSON object with unique property names")
            }

            fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut keys = HashSet::new();
                let mut values = Map::new();
                while let Some(key) = access.next_key::<String>()? {
                    if !keys.insert(key.clone()) {
                        return Err(serde::de::Error::custom("duplicate object property"));
                    }
                    let value = access.next_value::<Value>()?;
                    values.insert(key, value);
                }
                Ok(UniqueObject(values))
            }
        }

        deserializer.deserialize_map(UniqueObjectVisitor)
    }
}

fn deserialize_unique_response(payload: &[u8]) -> Result<BrokerResponse, CloudError> {
    let mut deserializer = serde_json::Deserializer::from_slice(payload);
    let value = UniqueObject::deserialize(&mut deserializer)
        .map_err(|_| CloudError::IdentityUnavailable)?;
    deserializer
        .end()
        .map_err(|_| CloudError::IdentityUnavailable)?;
    serde_json::from_value(Value::Object(value.0)).map_err(|_| CloudError::IdentityUnavailable)
}

fn decode_frame(frame: &[u8]) -> Result<&[u8], CloudError> {
    let (length_bytes, payload) = frame
        .split_at_checked(size_of::<u32>())
        .ok_or(CloudError::IdentityUnavailable)?;
    let length = usize::try_from(u32::from_be_bytes(
        length_bytes
            .try_into()
            .map_err(|_| CloudError::IdentityUnavailable)?,
    ))
    .map_err(|_| CloudError::IdentityUnavailable)?;
    if length == 0 || length > MAX_MESSAGE_BYTES || payload.len() != length {
        return Err(CloudError::IdentityUnavailable);
    }
    Ok(payload)
}

fn parse_expiry(value: &str) -> Result<UnixMillis, CloudError> {
    let timestamp = OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|_| CloudError::IdentityUnavailable)?
        .unix_timestamp_nanos();
    let millis = timestamp
        .checked_div(1_000_000)
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(CloudError::IdentityUnavailable)?;
    Ok(UnixMillis(millis))
}

fn opaque_ref(prefix: &str, value: &str) -> ContractId {
    ContractId::new(format!(
        "{prefix}_{}",
        sha256_bytes(value.as_bytes()).hex_value()
    ))
    .unwrap_or_else(|_| unreachable!("fixed prefix and SHA-256 hex always form a contract id"))
}

fn map_broker_failure(error_code: Option<&str>) -> CloudError {
    match error_code {
        Some("reauthentication_required") => CloudError::ReauthenticationRequired,
        Some("access_denied" | "authentication_cancelled") => CloudError::AuthenticationRequired,
        _ => CloudError::IdentityUnavailable,
    }
}

fn valid_request_id(value: &ContractId) -> bool {
    value.as_str().len() >= 8
}

fn validate_account(value: Option<&str>) -> Result<(), CloudError> {
    if value.is_some_and(|account| {
        !(3..=256).contains(&account.len())
            || account.bytes().any(|byte| {
                !byte.is_ascii_alphanumeric() && !matches!(byte, b'-' | b'_' | b'.' | b':')
            })
    }) {
        return Err(CloudError::IdentityUnavailable);
    }
    Ok(())
}

fn is_allowed_scope(scope: &str) -> bool {
    (3..=256).contains(&scope.len())
        && scope.starts_with("api://")
        && !scope.bytes().any(|byte| byte.is_ascii_whitespace())
}

fn is_guid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
        && value.bytes().any(|byte| byte != b'0' && byte != b'-')
}
