use core::fmt;
use std::time::Duration;

use async_trait::async_trait;
use url::Url;
use zeroize::Zeroizing;

use crate::{
    AuthorizedModelRequest, CloudAccess, CloudError, CloudSession, IdentityBroker, RawModelOutput,
};
use desktop_runtime::UnixMillis;

const MODEL_ACCESS_PATH: &str = "desktop/v1/model-access/calls";
const MAX_REQUEST_BYTES: usize = 4 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MIN_HTTP_TIMEOUT: Duration = Duration::from_secs(1);
const MAX_HTTP_TIMEOUT: Duration = Duration::from_mins(2);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupportApiOrigin {
    endpoint: Url,
}

impl SupportApiOrigin {
    /// Parses one immutable support-plane HTTPS origin.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::InvalidSupportOrigin`] for non-HTTPS URLs or an
    /// origin containing credentials, a path, query, or fragment.
    pub fn new(value: impl AsRef<str>) -> Result<Self, CloudError> {
        let origin = Url::parse(value.as_ref()).map_err(|_| CloudError::InvalidSupportOrigin)?;
        if origin.scheme() != "https"
            || origin.host_str().is_none()
            || !origin.username().is_empty()
            || origin.password().is_some()
            || origin.path() != "/"
            || origin.query().is_some()
            || origin.fragment().is_some()
        {
            return Err(CloudError::InvalidSupportOrigin);
        }
        let endpoint = origin
            .join(MODEL_ACCESS_PATH)
            .map_err(|_| CloudError::InvalidSupportOrigin)?;
        Ok(Self { endpoint })
    }

    #[must_use]
    pub fn endpoint(&self) -> &Url {
        &self.endpoint
    }
}

pub struct OutboundHttpRequest {
    url: Url,
    body: Vec<u8>,
    bearer: Zeroizing<String>,
    idempotency_key: String,
}

impl OutboundHttpRequest {
    #[must_use]
    pub fn url(&self) -> &Url {
        &self.url
    }

    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    pub fn with_bearer<T>(&self, operation: impl FnOnce(&str) -> T) -> T {
        operation(self.bearer.as_str())
    }

    #[must_use]
    pub fn idempotency_key(&self) -> &str {
        &self.idempotency_key
    }
}

impl fmt::Debug for OutboundHttpRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OutboundHttpRequest")
            .field("url", &self.url)
            .field("body", &"[REDACTED]")
            .field("body_bytes", &self.body.len())
            .field("bearer", &"[REDACTED]")
            .field("idempotency_key", &self.idempotency_key)
            .finish()
    }
}

#[derive(Clone)]
pub struct HttpResponse {
    status: u16,
    content_type: Option<String>,
    content_length: Option<u64>,
    body: Vec<u8>,
}

impl HttpResponse {
    #[must_use]
    pub fn new(
        status: u16,
        content_type: Option<String>,
        content_length: Option<u64>,
        body: Vec<u8>,
    ) -> Self {
        Self {
            status,
            content_type,
            content_length,
            body,
        }
    }
}

impl fmt::Debug for HttpResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpResponse")
            .field("status", &self.status)
            .field("content_type", &self.content_type)
            .field("content_length", &self.content_length)
            .field("body", &"[REDACTED]")
            .field("body_bytes", &self.body.len())
            .finish()
    }
}

#[async_trait]
pub trait HttpExecutor: Send + Sync {
    /// Executes one already-authorized request against its immutable URL.
    ///
    /// # Errors
    ///
    /// Returns only stable, sanitized cloud errors.
    async fn execute(&self, request: OutboundHttpRequest) -> Result<HttpResponse, CloudError>;
}

#[derive(Debug)]
pub struct SupportApiTransport<E> {
    origin: SupportApiOrigin,
    executor: E,
}

impl<E> SupportApiTransport<E>
where
    E: HttpExecutor,
{
    #[must_use]
    pub const fn new(origin: SupportApiOrigin, executor: E) -> Self {
        Self { origin, executor }
    }

    /// Sends one sealed request with one currently valid cloud access grant.
    ///
    /// # Errors
    ///
    /// Returns a stable [`CloudError`] for stale sessions, request drift,
    /// transport failure, or malformed and oversized untrusted responses.
    pub async fn send<B>(
        &self,
        session: &CloudSession<B>,
        access: &CloudAccess,
        request: &AuthorizedModelRequest,
        now: UnixMillis,
    ) -> Result<RawModelOutput, CloudError>
    where
        B: IdentityBroker,
    {
        if !session.is_current(access) {
            return Err(CloudError::SessionInvalidated);
        }
        if !session.is_current_at(access, now) {
            return Err(CloudError::ReauthenticationRequired);
        }
        request.verify()?;
        let body = serde_json::to_vec(request).map_err(|_| CloudError::TransportFailed)?;
        if body.len() > MAX_REQUEST_BYTES {
            return Err(CloudError::TransportFailed);
        }
        let bearer = access.with_bearer(|token| Zeroizing::new(token.to_owned()));
        if !session.is_current(access) {
            return Err(CloudError::SessionInvalidated);
        }
        let response = self
            .executor
            .execute(OutboundHttpRequest {
                url: self.origin.endpoint.clone(),
                body,
                bearer,
                idempotency_key: request.request_id.to_string(),
            })
            .await?;
        if !session.is_current(access) {
            return Err(CloudError::SessionInvalidated);
        }
        validate_response(&response)
    }
}

#[derive(Debug)]
pub struct ReqwestHttpExecutor {
    client: reqwest::Client,
}

impl ReqwestHttpExecutor {
    /// Creates a no-redirect, no-proxy, rustls HTTPS executor.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::TransportFailed`] for invalid timeout bounds or
    /// client construction failure.
    pub fn new(connect_timeout: Duration, operation_timeout: Duration) -> Result<Self, CloudError> {
        if !(MIN_HTTP_TIMEOUT..=MAX_HTTP_TIMEOUT).contains(&connect_timeout)
            || !(MIN_HTTP_TIMEOUT..=MAX_HTTP_TIMEOUT).contains(&operation_timeout)
            || connect_timeout > operation_timeout
        {
            return Err(CloudError::TransportFailed);
        }
        let client = reqwest::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(connect_timeout)
            .timeout(operation_timeout)
            .user_agent("sapphirus-desktop/0.1")
            .build()
            .map_err(|_| CloudError::TransportFailed)?;
        Ok(Self { client })
    }
}

#[async_trait]
impl HttpExecutor for ReqwestHttpExecutor {
    async fn execute(&self, request: OutboundHttpRequest) -> Result<HttpResponse, CloudError> {
        let OutboundHttpRequest {
            url,
            body,
            bearer,
            idempotency_key,
        } = request;
        let mut response = self
            .client
            .post(url)
            .bearer_auth(bearer.as_str())
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(reqwest::header::ACCEPT, "application/json")
            .header("Idempotency-Key", idempotency_key)
            .body(body)
            .send()
            .await
            .map_err(|_| CloudError::TransportFailed)?;
        let status = response.status().as_u16();
        let content_length = response.content_length();
        if content_length.is_some_and(|length| {
            usize::try_from(length).map_or(true, |length| length > MAX_RESPONSE_BYTES)
        }) {
            return Err(CloudError::TransportFailed);
        }
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let mut response_body = Vec::with_capacity(
            content_length
                .and_then(|length| usize::try_from(length).ok())
                .unwrap_or_default(),
        );
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| CloudError::TransportFailed)?
        {
            let next_length = response_body
                .len()
                .checked_add(chunk.len())
                .ok_or(CloudError::TransportFailed)?;
            if next_length > MAX_RESPONSE_BYTES {
                return Err(CloudError::TransportFailed);
            }
            response_body.extend_from_slice(&chunk);
        }
        Ok(HttpResponse::new(
            status,
            content_type,
            content_length,
            response_body,
        ))
    }
}

impl SupportApiTransport<ReqwestHttpExecutor> {
    /// Creates the production fixed-origin HTTPS composition.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::TransportFailed`] when the bounded executor cannot
    /// be constructed.
    pub fn production(
        origin: SupportApiOrigin,
        connect_timeout: Duration,
        operation_timeout: Duration,
    ) -> Result<Self, CloudError> {
        Ok(Self::new(
            origin,
            ReqwestHttpExecutor::new(connect_timeout, operation_timeout)?,
        ))
    }
}

fn validate_response(response: &HttpResponse) -> Result<RawModelOutput, CloudError> {
    let actual_length =
        u64::try_from(response.body.len()).map_err(|_| CloudError::TransportFailed)?;
    if response.status != 200
        || response
            .content_length
            .is_some_and(|declared_length| declared_length != actual_length)
        || response.body.len() > MAX_RESPONSE_BYTES
        || !response
            .content_type
            .as_deref()
            .is_some_and(is_json_content_type)
    {
        return Err(CloudError::TransportFailed);
    }
    serde_json::from_slice(&response.body).map_err(|_| CloudError::InvalidModelOutput)
}

fn is_json_content_type(value: &str) -> bool {
    value
        .split_once(';')
        .map_or(value, |(media_type, _)| media_type)
        .trim()
        .eq_ignore_ascii_case("application/json")
}
