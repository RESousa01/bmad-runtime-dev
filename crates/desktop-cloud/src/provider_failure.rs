//! Why a direct-provider call failed, in terms the user can act on.
//!
//! Every non-2xx provider response used to collapse into a single
//! [`CloudError::TransportFailed`], which the renderer reports as *"Connect a
//! model provider in Settings"* with `retryable: false`. On a 429 that is two
//! lies at once: the provider *is* connected, and retrying is exactly what
//! would work. On a spent quota the opposite lie is worse, because a caller
//! that believed a 429 was retryable would loop against an account that cannot
//! succeed until someone pays for it.
//!
//! # What may cross this boundary
//!
//! A provider error body is attacker-influenced and, worse, routinely echoes
//! the request — which for this app is reviewed workspace content. So
//! [`ProviderFailure`] holds **no strings from the wire at all**: a fieldless
//! kind, an HTTP status, a clamped retry delay, and the stable
//! [`CloudError`] the legacy signature already returned. Its `Debug` and
//! `Display` are therefore secret-safe by construction rather than by review.
//!
//! Bodies are still *read* — a 429 that means "no quota left" and a 429 that
//! means "slow down" are different statuses only in the body — but they are
//! read to be **matched against a fixed allowlist of provider error codes**,
//! never to be forwarded. Matching is by whole-string equality, because
//! substring matching over provider-controlled text is how `sk-` came to match
//! `risk-assessment`.

use std::time::Duration;

use desktop_runtime::deserialize_strict_json;
use serde_json::Value;
use thiserror::Error;

use crate::model::MAX_OUTPUT_BYTES;
use crate::transport::is_json_content_type;
use crate::CloudError;

/// The one ceiling on a provider response body.
///
/// Both the declared-length check and the streaming accumulator read *this*
/// constant. They used to be pinned separately — a `MAX_PROVIDER_RESPONSE_BYTES`
/// alias for the `Content-Length` check and a bare `MAX_OUTPUT_BYTES` inline in
/// the read loop — so lowering the header ceiling would have left a chunked
/// response bounded by the old, larger number. Chunked is what these providers
/// actually send, so the loop is the arm that matters.
pub(crate) const MAX_PROVIDER_RESPONSE_BYTES: usize = MAX_OUTPUT_BYTES;

/// The `Content-Length` form of the same ceiling.
pub(crate) const MAX_PROVIDER_RESPONSE_BYTES_DECLARED: u64 = MAX_PROVIDER_RESPONSE_BYTES as u64;

/// How much of a *rejected* response is read so its error code can be matched.
///
/// Small on purpose. Nothing here is kept, and an error body large enough to
/// need more than this is not one whose `error.code` is going to be legible.
pub(crate) const MAX_ERROR_BODY_BYTES: usize = 16 * 1024;

/// The longest provider error code that will be compared at all.
///
/// A code is a short enum-like token. Refusing to even compare a longer one
/// keeps a hostile body from turning classification into a long memcmp, and
/// makes the allowlist the only thing that can ever match.
const MAX_ERROR_CODE_BYTES: usize = 128;

/// The ceiling on a `Retry-After` this crate will repeat to a caller.
///
/// The value is provider-controlled advice, so it needs a bound: without one a
/// broken or hostile provider could park a retry for hours, and a caller that
/// honoured it would look hung. Five minutes is longer than any real rate-limit
/// window these providers advertise and short enough that a user waiting it out
/// is a wait, not an outage.
pub const MAX_RETRY_AFTER: Duration = Duration::from_mins(5);

/// Provider error codes that turn a 429 into "you are out of quota".
///
/// This is the one refinement that *removes* retryability, and it is the whole
/// reason error bodies are read: `OpenAI` reports an exhausted balance as a 429,
/// identical in status to an ordinary rate limit.
const QUOTA_ERROR_CODES: [&str; 5] = [
    "insufficient_quota",
    "billing_hard_limit_reached",
    "billing_not_active",
    "quota_exceeded",
    "exceeded_quota",
];

/// Provider error codes that turn a generic 4xx into "your request is too big".
///
/// These arrive as **400**, not 413, which is why status alone cannot tell a
/// user to shorten the conversation.
const TOO_LARGE_ERROR_CODES: [&str; 4] = [
    "context_length_exceeded",
    "string_above_max_length",
    "request_too_large",
    "max_tokens_exceeded",
];

/// Provider error codes that turn a generic 4xx into "the content filter said no".
const CONTENT_FILTER_ERROR_CODES: [&str; 3] = [
    "content_filter",
    "content_policy_violation",
    "ResponsibleAIPolicyViolation",
];

/// Provider error codes that turn a generic 4xx into "this key is wrong".
const CREDENTIAL_ERROR_CODES: [&str; 4] = [
    "invalid_api_key",
    "invalid_authentication",
    "authentication_error",
    "PermissionDenied",
];

/// What went wrong, at the granularity that changes what the user should do.
///
/// Deliberately fieldless: a kind may never carry a byte of provider text.
/// Deliberately *not* `#[non_exhaustive]`: every mapping site should stop
/// compiling when a class is added, rather than quietly routing a new class
/// into a catch-all that says the wrong thing. That catch-all is the failure
/// this module exists to remove.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProviderFailureKind {
    /// The credential was rejected. Fix the key; retrying cannot help.
    CredentialRejected,
    /// The credential is valid but not entitled to this model, region, or
    /// deployment. A different key or a different model, not a retry.
    PermissionDenied,
    /// The provider has no such model or deployment. Choose another.
    ModelUnavailable,
    /// The provider refused the request as malformed or unacceptable.
    RequestRejected,
    /// The request exceeded what the model accepts. Reduce the context.
    RequestTooLarge,
    /// The provider's content filter refused the request. Rephrase it.
    ContentFiltered,
    /// The account has no quota or credit left. Neither a retry nor a new key.
    QuotaExhausted,
    /// The key is being rate limited. Wait, then retry — see
    /// [`ProviderFailure::retry_after`].
    RateLimited,
    /// The provider failed on its own side. Retry later.
    ProviderUnavailable,
    /// The provider could not be reached at all: DNS, TLS, or connect.
    NetworkUnreachable,
    /// The provider did not answer within the operation timeout.
    Timeout,
    /// A reply arrived that is not a usable provider response envelope — wrong
    /// media type, an unfollowed redirect, or a truncated read.
    MalformedResponse,
    /// A well-formed provider response whose payload is not what the sealed
    /// schema requires.
    UnusableModelOutput,
    /// The request never left this machine. Not the provider's fault and not
    /// the user's; nothing about the key or the network will change it.
    HostRequestInvalid,
    /// No model provider is configured at all.
    ///
    /// The *only* class for which "Connect a model provider in Settings" is a
    /// true sentence. It was previously the message for every failure in this
    /// enum, which is how a rate limit on a working key came to tell the user
    /// they had no provider. Nothing produces this kind inside this crate —
    /// the composition layer that knows a provider was never selected does.
    ProviderNotConfigured,
}

impl ProviderFailureKind {
    /// Whether sending the identical request again could plausibly succeed.
    ///
    /// The asymmetry matters. A false `true` on a rejected credential is a
    /// retry loop against a provider that will never say yes; a false `false`
    /// on a rate limit tells the user to give up on a call that would have
    /// worked in ten seconds. Everything here that is `true` is `true` because
    /// the *provider's own state* is expected to change, not because a retry
    /// might get lucky.
    #[must_use]
    pub const fn retryable(self) -> bool {
        match self {
            Self::RateLimited
            | Self::ProviderUnavailable
            | Self::NetworkUnreachable
            | Self::Timeout
            | Self::MalformedResponse
            | Self::UnusableModelOutput => true,
            Self::CredentialRejected
            | Self::PermissionDenied
            | Self::ModelUnavailable
            | Self::RequestRejected
            | Self::RequestTooLarge
            | Self::ContentFiltered
            | Self::QuotaExhausted
            | Self::HostRequestInvalid
            | Self::ProviderNotConfigured => false,
        }
    }

    /// The sentence shown to the user.
    ///
    /// `&'static str` is the point. There is no formatting seam here through
    /// which a provider body could reach a screen.
    #[must_use]
    pub const fn user_message(self) -> &'static str {
        match self {
            Self::CredentialRejected => {
                "The provider rejected this API key. Open Settings and enter the key again."
            }
            Self::PermissionDenied => {
                "This API key is not allowed to use the selected model. Choose another model, or use a key that has access."
            }
            Self::ModelUnavailable => {
                "The provider has no model by that name. Choose a different model in Settings."
            }
            Self::RequestRejected => {
                "The provider refused this request. Try again with a shorter message and fewer attached files."
            }
            Self::RequestTooLarge => {
                "This request is larger than the selected model accepts. Remove some attached files, or start a new conversation."
            }
            Self::ContentFiltered => {
                "The provider's content filter refused this request. Rephrase the message and send it again."
            }
            Self::QuotaExhausted => {
                "This provider account has no quota left. Add credit or raise the limit with the provider — sending again will not help."
            }
            Self::RateLimited => {
                "The provider is rate limiting this key. Wait a moment, then send the message again."
            }
            Self::ProviderUnavailable => {
                "The provider is having trouble on its side. Send the message again shortly."
            }
            Self::NetworkUnreachable => {
                "The provider could not be reached. Check the network connection and send the message again."
            }
            Self::Timeout => {
                "The provider did not answer in time. Send the message again, or shorten it."
            }
            Self::MalformedResponse => {
                "The provider's reply could not be read. Send the message again."
            }
            Self::UnusableModelOutput => {
                "The model's reply did not match the shape this app requires. Send the message again."
            }
            Self::HostRequestInvalid => {
                "This request could not be prepared for the provider and was not sent."
            }
            Self::ProviderNotConfigured => {
                "Connect a model provider in Settings before sending a message."
            }
        }
    }

    /// A stable token for logs and telemetry.
    ///
    /// Deliberately not derived from `Debug`: a rename of a variant should not
    /// silently rewrite whatever a support engineer is grepping for.
    #[must_use]
    pub const fn diagnostic_code(self) -> &'static str {
        match self {
            Self::CredentialRejected => "provider.credential_rejected",
            Self::PermissionDenied => "provider.permission_denied",
            Self::ModelUnavailable => "provider.model_unavailable",
            Self::RequestRejected => "provider.request_rejected",
            Self::RequestTooLarge => "provider.request_too_large",
            Self::ContentFiltered => "provider.content_filtered",
            Self::QuotaExhausted => "provider.quota_exhausted",
            Self::RateLimited => "provider.rate_limited",
            Self::ProviderUnavailable => "provider.unavailable",
            Self::NetworkUnreachable => "provider.network_unreachable",
            Self::Timeout => "provider.timeout",
            Self::MalformedResponse => "provider.malformed_response",
            Self::UnusableModelOutput => "provider.unusable_model_output",
            Self::HostRequestInvalid => "host.request_invalid",
            Self::ProviderNotConfigured => "host.provider_not_configured",
        }
    }
}

/// One classified direct-provider failure.
///
/// Every field is either host-authored or a number off the wire. Nothing here
/// is provider text, so `Debug` and `Display` are safe to log and safe to show.
/// `cloud_error` is carried rather than derived so the legacy
/// [`CloudError`]-returning signature keeps answering exactly what it answered
/// before this classification existed — the BMAD help and capability lanes
/// distinguish those variants, and this module is not entitled to change what
/// they see.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{}", .kind.user_message())]
pub struct ProviderFailure {
    kind: ProviderFailureKind,
    status: Option<u16>,
    retry_after: Option<Duration>,
    cloud_error: CloudError,
}

impl ProviderFailure {
    /// Reports a failure that this crate cannot observe for itself.
    ///
    /// The composition layer above knows things the transport does not — that
    /// no provider was ever selected, that the configured mode is offline —
    /// and those need to reach the user through the same classification as
    /// everything else, or they arrive back at a generic message.
    ///
    /// `cloud_error` is the variant the caller's existing lane already reads;
    /// it is carried through untouched.
    #[must_use]
    pub const fn reported(kind: ProviderFailureKind, cloud_error: CloudError) -> Self {
        Self::local(kind, cloud_error)
    }

    /// A failure that never reached the provider.
    pub(crate) const fn local(kind: ProviderFailureKind, cloud_error: CloudError) -> Self {
        Self {
            kind,
            status: None,
            retry_after: None,
            cloud_error,
        }
    }

    #[must_use]
    pub const fn kind(&self) -> ProviderFailureKind {
        self.kind
    }

    /// The HTTP status, when one was received. A number, never a body.
    #[must_use]
    pub const fn status(&self) -> Option<u16> {
        self.status
    }

    /// The provider's own advice on when to try again, clamped to
    /// [`MAX_RETRY_AFTER`], and only ever present on a retryable kind.
    #[must_use]
    pub const fn retry_after(&self) -> Option<Duration> {
        self.retry_after
    }

    #[must_use]
    pub const fn retryable(&self) -> bool {
        self.kind.retryable()
    }

    #[must_use]
    pub const fn user_message(&self) -> &'static str {
        self.kind.user_message()
    }

    #[must_use]
    pub const fn diagnostic_code(&self) -> &'static str {
        self.kind.diagnostic_code()
    }

    /// Lets a fixed allowlist of provider error codes sharpen the class.
    ///
    /// Refinement may only ever move *within* what the status already allows —
    /// see `error_body_refinement_never_increases_retryability`, which walks
    /// every entry of every allowlist. A body that could make a failure more
    /// retryable would be a way to drive a retry loop from the wire.
    #[must_use]
    pub(crate) fn refined_by_error_body(self, body: &[u8]) -> Self {
        let Some(code) = provider_error_code(body) else {
            return self;
        };
        let code = code.as_str();
        let refined = match self.kind {
            ProviderFailureKind::RateLimited if QUOTA_ERROR_CODES.contains(&code) => {
                ProviderFailureKind::QuotaExhausted
            }
            ProviderFailureKind::RequestRejected if TOO_LARGE_ERROR_CODES.contains(&code) => {
                ProviderFailureKind::RequestTooLarge
            }
            ProviderFailureKind::RequestRejected if CONTENT_FILTER_ERROR_CODES.contains(&code) => {
                ProviderFailureKind::ContentFiltered
            }
            ProviderFailureKind::RequestRejected if CREDENTIAL_ERROR_CODES.contains(&code) => {
                ProviderFailureKind::CredentialRejected
            }
            _ => return self,
        };
        Self {
            kind: refined,
            status: self.status,
            retry_after: refined.retryable().then_some(self.retry_after).flatten(),
            cloud_error: self.cloud_error,
        }
    }

    /// Whether reading a bounded slice of the error body could change the
    /// answer. Keeps the common 5xx path from paying for a read it will ignore.
    pub(crate) const fn wants_error_body(&self) -> bool {
        matches!(
            self.kind,
            ProviderFailureKind::RateLimited | ProviderFailureKind::RequestRejected
        )
    }
}

impl From<ProviderFailure> for CloudError {
    fn from(failure: ProviderFailure) -> Self {
        failure.cloud_error
    }
}

/// The parts of a provider response that decide whether its body is worth
/// reading. Owned, so classification is a pure function of copied scalars and
/// two header strings that never outlive it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ProviderResponseHead {
    pub(crate) status: u16,
    pub(crate) content_type: Option<String>,
    pub(crate) content_length: Option<u64>,
    pub(crate) retry_after: Option<String>,
    pub(crate) retry_after_ms: Option<String>,
}

/// Proof that a response head was classified and accepted.
///
/// Only [`ProviderResponseHead::accept`] can mint one, and the body reader
/// demands one. Deleting the classification call therefore fails to compile
/// rather than silently letting a 429 body be parsed as a model reply — which
/// is the shape of "a guard that reads like protection and isn't".
#[derive(Debug)]
pub(crate) struct AcceptedResponseHead {
    pub(crate) declared_length: Option<u64>,
}

impl ProviderResponseHead {
    pub(crate) fn from_response(response: &reqwest::Response) -> Self {
        let header = |name: &str| {
            response
                .headers()
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned)
        };
        Self {
            status: response.status().as_u16(),
            content_type: header("content-type"),
            content_length: response.content_length(),
            retry_after: header("retry-after"),
            retry_after_ms: header("retry-after-ms"),
        }
    }

    /// Classifies the head, admitting only a 2xx JSON response of a declared
    /// size this process will read.
    ///
    /// # Errors
    ///
    /// Returns the classified [`ProviderFailure`]. For a status whose meaning
    /// depends on the body, the caller refines it with
    /// [`ProviderFailure::refined_by_error_body`].
    pub(crate) fn accept(&self) -> Result<AcceptedResponseHead, ProviderFailure> {
        if !(200..300).contains(&self.status) {
            return Err(self.status_failure());
        }
        if self
            .content_length
            .is_some_and(|length| length > MAX_PROVIDER_RESPONSE_BYTES_DECLARED)
            || !self
                .content_type
                .as_deref()
                .is_some_and(is_json_content_type)
        {
            return Err(ProviderFailure {
                kind: ProviderFailureKind::MalformedResponse,
                status: Some(self.status),
                retry_after: None,
                cloud_error: CloudError::TransportFailed,
            });
        }
        Ok(AcceptedResponseHead {
            declared_length: self.content_length,
        })
    }

    fn status_failure(&self) -> ProviderFailure {
        let kind = status_kind(self.status);
        ProviderFailure {
            kind,
            status: Some(self.status),
            retry_after: kind
                .retryable()
                .then(|| {
                    parse_retry_after(self.retry_after.as_deref(), self.retry_after_ms.as_deref())
                })
                .flatten(),
            // Unchanged from before this module existed: every non-2xx was a
            // `TransportFailed`, and the lanes that read `CloudError` still see
            // exactly that. Only the classification alongside it is new.
            cloud_error: CloudError::TransportFailed,
        }
    }
}

/// The status-only classification.
///
/// Specific codes first, then the family fallbacks. An unrecognised 4xx is a
/// client error, so it is *not* retryable — resending the identical bytes to a
/// provider that already refused them is the loop this module exists to
/// prevent. An unrecognised 1xx or 3xx is not an answer at all: with redirects
/// disabled a 302 means something other than the provider replied, which is a
/// captive portal far more often than it is a real move.
const fn status_kind(status: u16) -> ProviderFailureKind {
    match status {
        401 => ProviderFailureKind::CredentialRejected,
        402 => ProviderFailureKind::QuotaExhausted,
        403 => ProviderFailureKind::PermissionDenied,
        404 => ProviderFailureKind::ModelUnavailable,
        408 => ProviderFailureKind::Timeout,
        413 => ProviderFailureKind::RequestTooLarge,
        429 => ProviderFailureKind::RateLimited,
        400..=499 => ProviderFailureKind::RequestRejected,
        500..=599 => ProviderFailureKind::ProviderUnavailable,
        _ => ProviderFailureKind::MalformedResponse,
    }
}

/// Classifies a transport-level failure — nothing was received to inspect.
pub(crate) fn classify_transport_error(error: &reqwest::Error) -> ProviderFailure {
    let kind = if error.is_timeout() {
        ProviderFailureKind::Timeout
    } else if error.is_connect() {
        ProviderFailureKind::NetworkUnreachable
    } else if error.is_builder() || error.is_request() {
        // The host could not form the request. Telling the user to check their
        // network for this would send them looking in the wrong place.
        ProviderFailureKind::HostRequestInvalid
    } else if error.is_body() || error.is_decode() {
        ProviderFailureKind::MalformedResponse
    } else {
        ProviderFailureKind::NetworkUnreachable
    };
    ProviderFailure::local(kind, CloudError::TransportFailed)
}

/// Reads `error.code`, falling back to `error.type`, from a provider error body.
///
/// The returned string is compared against an allowlist by the only caller and
/// then dropped. It is never stored in a [`ProviderFailure`] and never rendered.
fn provider_error_code(body: &[u8]) -> Option<String> {
    let value: Value = deserialize_strict_json(body).ok()?;
    let error = value.get("error")?;
    error
        .get("code")
        .and_then(Value::as_str)
        .or_else(|| error.get("type").and_then(Value::as_str))
        .filter(|code| code.len() <= MAX_ERROR_CODE_BYTES)
        .map(str::to_owned)
}

/// Parses `Retry-After` / `Retry-After-Ms` as a bounded delay.
///
/// Only the delta form is honoured. The HTTP-date form is deliberately ignored:
/// it is worth exactly as much as the agreement between the provider's clock
/// and this machine's, which on a laptop that has been asleep is nothing, and
/// neither provider sends it. `parse::<u64>` also disposes of negatives, `NaN`,
/// and `"1e9"` without a special case.
fn parse_retry_after(seconds: Option<&str>, millis: Option<&str>) -> Option<Duration> {
    let from_millis = millis
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_millis);
    let from_seconds = seconds
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_secs);
    Some(from_millis.or(from_seconds)?.min(MAX_RETRY_AFTER))
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "fixture construction failures should stop these unit tests at their source"
    )]

    use super::{
        classify_transport_error, parse_retry_after, provider_error_code, ProviderFailure,
        ProviderFailureKind, ProviderResponseHead, CONTENT_FILTER_ERROR_CODES,
        CREDENTIAL_ERROR_CODES, MAX_ERROR_CODE_BYTES, MAX_RETRY_AFTER, QUOTA_ERROR_CODES,
        TOO_LARGE_ERROR_CODES,
    };
    use crate::CloudError;
    use std::time::Duration;

    /// A synthetic provider response head. No socket, no network, no clock.
    fn head(status: u16) -> ProviderResponseHead {
        ProviderResponseHead {
            status,
            content_type: Some("application/json".to_owned()),
            content_length: Some(2),
            retry_after: None,
            retry_after_ms: None,
        }
    }

    fn failure(status: u16) -> ProviderFailure {
        head(status).accept().expect_err("a non-2xx is a failure")
    }

    /// The defect this module exists for: one class per thing the user should
    /// do, and a `retryable` flag that is true only when trying again could
    /// work. Every row is a status a real provider returns.
    #[test]
    fn every_provider_status_class_carries_its_own_action_and_retry_flag() {
        for (status, expected_kind, expected_retryable) in [
            (401, ProviderFailureKind::CredentialRejected, false),
            (402, ProviderFailureKind::QuotaExhausted, false),
            (403, ProviderFailureKind::PermissionDenied, false),
            (404, ProviderFailureKind::ModelUnavailable, false),
            (408, ProviderFailureKind::Timeout, true),
            (413, ProviderFailureKind::RequestTooLarge, false),
            (422, ProviderFailureKind::RequestRejected, false),
            (429, ProviderFailureKind::RateLimited, true),
            (500, ProviderFailureKind::ProviderUnavailable, true),
            (502, ProviderFailureKind::ProviderUnavailable, true),
            (503, ProviderFailureKind::ProviderUnavailable, true),
            (504, ProviderFailureKind::ProviderUnavailable, true),
            (302, ProviderFailureKind::MalformedResponse, true),
        ] {
            let failure = failure(status);
            assert_eq!(
                failure.kind(),
                expected_kind,
                "status {status} was classified as {:?}",
                failure.kind()
            );
            assert_eq!(
                failure.retryable(),
                expected_retryable,
                "status {status} ({expected_kind:?}) had the wrong retry flag"
            );
            assert_eq!(failure.status(), Some(status));
            // The legacy contract is unchanged: every one of these is still a
            // `TransportFailed` to the lanes that read `CloudError`.
            assert_eq!(CloudError::from(failure), CloudError::TransportFailed);
        }
    }

    /// The two classes that are ruinous to get wrong, stated on their own so a
    /// change to either fails a test that names it.
    #[test]
    fn a_rejected_key_is_never_retryable_and_a_rate_limit_always_is() {
        assert!(
            !failure(401).retryable(),
            "retrying a rejected key loops forever against a provider that will never accept it"
        );
        assert!(
            failure(429).retryable(),
            "a rate limit is the one failure where sending the same request again is the fix"
        );
        assert_ne!(
            failure(401).user_message(),
            failure(429).user_message(),
            "the whole defect was these two reading identically to the user"
        );
    }

    /// A 200 that is not a JSON provider envelope is not a success.
    #[test]
    fn a_two_hundred_is_admitted_only_as_bounded_json() {
        assert!(head(200).accept().is_ok());
        for (label, bad) in [
            (
                "html from a captive portal",
                ProviderResponseHead {
                    content_type: Some("text/html".to_owned()),
                    ..head(200)
                },
            ),
            (
                "a media type that merely starts with the right prefix",
                ProviderResponseHead {
                    content_type: Some("application/json-seq".to_owned()),
                    ..head(200)
                },
            ),
            (
                "no media type at all",
                ProviderResponseHead {
                    content_type: None,
                    ..head(200)
                },
            ),
            (
                "a declared length past the ceiling",
                ProviderResponseHead {
                    content_length: Some(super::MAX_PROVIDER_RESPONSE_BYTES_DECLARED + 1),
                    ..head(200)
                },
            ),
        ] {
            let rejected = bad.accept().expect_err(label);
            assert_eq!(
                rejected.kind(),
                ProviderFailureKind::MalformedResponse,
                "{label} should be a malformed response"
            );
        }
        // Charset parameters are ordinary, and exactly at the ceiling is inside it.
        assert!(ProviderResponseHead {
            content_type: Some("application/json; charset=utf-8".to_owned()),
            content_length: Some(super::MAX_PROVIDER_RESPONSE_BYTES_DECLARED),
            ..head(200)
        }
        .accept()
        .is_ok());
    }

    /// The distinction that cannot be made from the status: a 429 that means
    /// "slow down" and a 429 that means "your balance is zero".
    #[test]
    fn an_exhausted_quota_is_separated_from_an_ordinary_rate_limit() {
        let rate_limited = ProviderResponseHead {
            retry_after: Some("20".to_owned()),
            ..head(429)
        }
        .accept()
        .expect_err("429");
        assert_eq!(rate_limited.kind(), ProviderFailureKind::RateLimited);
        assert!(rate_limited.retryable());
        assert_eq!(rate_limited.retry_after(), Some(Duration::from_secs(20)));

        let quota = rate_limited.clone().refined_by_error_body(
            br#"{"error":{"message":"You exceeded your current quota","type":"insufficient_quota","code":"insufficient_quota"}}"#,
        );
        assert_eq!(quota.kind(), ProviderFailureKind::QuotaExhausted);
        assert!(
            !quota.retryable(),
            "retrying an exhausted balance is the loop this refinement exists to stop"
        );
        assert_eq!(
            quota.retry_after(),
            None,
            "a wait cannot be advertised for a failure no wait resolves"
        );

        // A 429 with an unrelated code stays a rate limit.
        assert_eq!(
            rate_limited
                .refined_by_error_body(br#"{"error":{"code":"rate_limit_exceeded"}}"#)
                .kind(),
            ProviderFailureKind::RateLimited
        );
    }

    /// The other body-only distinction: `OpenAI` reports an over-long prompt as a
    /// **400**, so status alone would tell the user their request was malformed
    /// when the fix is to detach a file.
    #[test]
    fn an_over_long_prompt_is_reported_as_too_large_not_as_malformed() {
        let rejected = failure(400);
        assert_eq!(rejected.kind(), ProviderFailureKind::RequestRejected);

        let too_large = rejected.clone().refined_by_error_body(
            br#"{"error":{"message":"maximum context length","code":"context_length_exceeded"}}"#,
        );
        assert_eq!(too_large.kind(), ProviderFailureKind::RequestTooLarge);
        assert!(too_large.user_message().contains("attached files"));

        let filtered = rejected
            .clone()
            .refined_by_error_body(br#"{"error":{"code":"content_filter"}}"#);
        assert_eq!(filtered.kind(), ProviderFailureKind::ContentFiltered);

        // `error.type` is the fallback when `code` is null, which is what Azure
        // sends.
        let by_type =
            rejected.refined_by_error_body(br#"{"error":{"code":null,"type":"invalid_api_key"}}"#);
        assert_eq!(by_type.kind(), ProviderFailureKind::CredentialRejected);
    }

    /// The safety property that makes reading an attacker-influenced body
    /// acceptable at all: a body may sharpen a classification, never soften it.
    /// Walked over every allowlist entry rather than asserted for one case, so
    /// a future refinement that increases retryability fails here.
    #[test]
    fn error_body_refinement_never_increases_retryability() {
        let all_codes = QUOTA_ERROR_CODES
            .iter()
            .chain(TOO_LARGE_ERROR_CODES.iter())
            .chain(CONTENT_FILTER_ERROR_CODES.iter())
            .chain(CREDENTIAL_ERROR_CODES.iter());
        for code in all_codes {
            for status in [400, 401, 403, 404, 413, 422, 429, 500, 503] {
                let before = failure(status);
                let body = format!(r#"{{"error":{{"code":"{code}"}}}}"#);
                let after = before.clone().refined_by_error_body(body.as_bytes());
                assert!(
                    !after.retryable() || before.retryable(),
                    "code {code} on status {status} turned a non-retryable failure into a retryable one"
                );
                assert_eq!(
                    after.status(),
                    before.status(),
                    "refinement must not rewrite the observed status"
                );
            }
        }
    }

    /// A provider error body routinely echoes the request. For this app the
    /// request *is* reviewed workspace content, so none of it may survive into
    /// anything the user or a log can see.
    ///
    /// Both construction paths through refinement are walked. An earlier
    /// revision only covered the "no allowlist match" path, and a mutation that
    /// attached the body to the *refined* failure slipped past it — the one
    /// place a future "just add the provider's message, it's useful for
    /// support" change would naturally land.
    #[test]
    fn a_provider_error_body_cannot_carry_request_content_into_user_visible_text() {
        let secret = concat!("sk-", "proj-abcdefghijklmnopqrstuvwxyz0123456789");
        let workspace_line = "the acquisition closes on Tuesday";
        let prose =
            format!("Invalid value for 'input': {workspace_line} / Authorization: Bearer {secret}");
        let bodies = [
            // Nothing in the allowlist matches, so the class is unchanged.
            format!(
                r#"{{"error":{{"message":"{prose}","code":"{secret}","type":"invalid_request_error","param":"{workspace_line}"}}}}"#
            ),
            // The code *does* match, so the refined failure is rebuilt — with
            // the same prose sitting beside it in the body.
            format!(r#"{{"error":{{"message":"{prose}","code":"insufficient_quota"}}}}"#),
            format!(r#"{{"error":{{"message":"{prose}","code":"context_length_exceeded"}}}}"#),
        ];

        for body in &bodies {
            for base in [failure(400), failure(429), failure(401), failure(500)] {
                let refined = base.refined_by_error_body(body.as_bytes());
                for rendered in [
                    format!("{refined:?}"),
                    refined.to_string(),
                    refined.user_message().to_owned(),
                    refined.diagnostic_code().to_owned(),
                ] {
                    assert!(
                        !rendered.contains(secret),
                        "a credential from the provider body reached user-visible text: {rendered}"
                    );
                    assert!(
                        !rendered.contains(workspace_line),
                        "workspace content echoed by the provider reached user-visible text: {rendered}"
                    );
                    assert!(
                        !rendered.contains("Invalid value"),
                        "provider prose reached user-visible text: {rendered}"
                    );
                }
            }
        }
    }

    /// An error code long enough to be a payload rather than a token is not
    /// even compared. Substring matching over provider text is how `sk-` came
    /// to match `risk-assessment`; whole-string equality plus a length bound is
    /// the answer to both.
    #[test]
    fn only_a_short_whole_string_error_code_is_ever_matched() {
        let padded = format!(
            "{}{}",
            "x".repeat(MAX_ERROR_CODE_BYTES),
            "insufficient_quota"
        );
        let body = format!(r#"{{"error":{{"code":"{padded}"}}}}"#);
        assert_eq!(
            provider_error_code(body.as_bytes()),
            None,
            "an over-long code must not be compared at all"
        );
        assert_eq!(
            failure(429).refined_by_error_body(body.as_bytes()).kind(),
            ProviderFailureKind::RateLimited
        );

        // A code that merely *contains* an allowlist entry must not match it.
        for near_miss in [
            "not_insufficient_quota",
            "insufficient_quota_warning",
            "INSUFFICIENT_QUOTA",
        ] {
            let body = format!(r#"{{"error":{{"code":"{near_miss}"}}}}"#);
            assert_eq!(
                failure(429).refined_by_error_body(body.as_bytes()).kind(),
                ProviderFailureKind::RateLimited,
                "{near_miss} must not be read as an exhausted quota"
            );
        }

        // And a body that is not a provider error envelope refines nothing.
        for inert in [
            b"not json at all".as_slice(),
            br#"{"error":"a string, not an object"}"#,
            br#"{"code":"insufficient_quota"}"#,
            br#"{"error":{"code":"insufficient_quota"},"error":{"code":"x"}}"#,
            b"",
        ] {
            assert_eq!(
                failure(429).refined_by_error_body(inert).kind(),
                ProviderFailureKind::RateLimited
            );
        }
    }

    #[test]
    fn retry_after_is_parsed_as_a_delta_and_clamped() {
        assert_eq!(
            parse_retry_after(Some("20"), None),
            Some(Duration::from_secs(20))
        );
        assert_eq!(
            parse_retry_after(None, Some("1500")),
            Some(Duration::from_millis(1_500))
        );
        assert_eq!(
            parse_retry_after(Some("20"), Some("1500")),
            Some(Duration::from_millis(1_500)),
            "the millisecond header is the more precise of the two"
        );
        assert_eq!(
            parse_retry_after(Some(" 7 "), None),
            Some(Duration::from_secs(7))
        );
        assert_eq!(parse_retry_after(Some("0"), None), Some(Duration::ZERO));
        assert_eq!(
            parse_retry_after(Some("86400"), None),
            Some(MAX_RETRY_AFTER),
            "a provider must not be able to park a retry beyond the ceiling"
        );
        assert_eq!(
            parse_retry_after(Some("99999999999999999999999"), None),
            None,
            "a value that does not fit a u64 is advice, not arithmetic"
        );
        for ignored in [
            "-5",
            "Wed, 21 Oct 2015 07:28:00 GMT",
            "soon",
            "1.5",
            "1e9",
            "",
        ] {
            assert_eq!(
                parse_retry_after(Some(ignored), None),
                None,
                "{ignored} must not become a delay"
            );
        }
    }

    /// A retry delay is attached only where waiting is the advice.
    #[test]
    fn retry_after_is_offered_only_on_a_failure_that_waiting_resolves() {
        let with_header = |status| {
            ProviderResponseHead {
                retry_after: Some("30".to_owned()),
                ..head(status)
            }
            .accept()
            .expect_err("a non-2xx is a failure")
        };
        assert_eq!(
            with_header(429).retry_after(),
            Some(Duration::from_secs(30))
        );
        assert_eq!(
            with_header(503).retry_after(),
            Some(Duration::from_secs(30))
        );
        assert_eq!(
            with_header(401).retry_after(),
            None,
            "a wait must never be advertised for a key that will never be accepted"
        );
        assert_eq!(with_header(403).retry_after(), None);
    }

    /// Every kind must answer every question, and the answers must be distinct
    /// enough to be worth having separate kinds for.
    #[test]
    fn every_kind_has_its_own_message_and_diagnostic_code() {
        let all = [
            ProviderFailureKind::CredentialRejected,
            ProviderFailureKind::PermissionDenied,
            ProviderFailureKind::ModelUnavailable,
            ProviderFailureKind::RequestRejected,
            ProviderFailureKind::RequestTooLarge,
            ProviderFailureKind::ContentFiltered,
            ProviderFailureKind::QuotaExhausted,
            ProviderFailureKind::RateLimited,
            ProviderFailureKind::ProviderUnavailable,
            ProviderFailureKind::NetworkUnreachable,
            ProviderFailureKind::Timeout,
            ProviderFailureKind::MalformedResponse,
            ProviderFailureKind::UnusableModelOutput,
            ProviderFailureKind::HostRequestInvalid,
            ProviderFailureKind::ProviderNotConfigured,
        ];
        let messages: std::collections::BTreeSet<_> =
            all.iter().map(|kind| kind.user_message()).collect();
        let codes: std::collections::BTreeSet<_> =
            all.iter().map(|kind| kind.diagnostic_code()).collect();
        assert_eq!(messages.len(), all.len(), "two kinds share a message");
        assert_eq!(codes.len(), all.len(), "two kinds share a diagnostic code");
        for kind in all {
            assert!(!kind.user_message().is_empty());
            assert_eq!(
                kind.user_message().contains("Connect a model provider"),
                kind == ProviderFailureKind::ProviderNotConfigured,
                "{kind:?} must not tell a user with a working key that they have none —                  that sentence belongs to exactly one class, the one where it is true"
            );
        }
        assert!(
            !ProviderFailureKind::ProviderNotConfigured.retryable(),
            "there is nothing to retry until a provider exists"
        );
    }

    /// A failure that never reached the provider carries no status and no wait.
    #[test]
    fn a_local_failure_reports_no_provider_facts() {
        let local = ProviderFailure::local(
            ProviderFailureKind::HostRequestInvalid,
            CloudError::ConsentBindingMismatch,
        );
        assert_eq!(local.status(), None);
        assert_eq!(local.retry_after(), None);
        assert!(!local.retryable());
        assert_eq!(
            CloudError::from(local),
            CloudError::ConsentBindingMismatch,
            "a pre-flight failure must keep the exact cloud error its lane already reads"
        );
    }

    /// The transport-error classifier is reached only with a real
    /// `reqwest::Error`, which cannot be constructed here; a connect failure to
    /// a reserved-for-documentation address is the closest synthetic stand-in
    /// that needs no live provider.
    #[tokio::test]
    async fn an_unreachable_provider_is_a_network_failure_not_a_missing_key() {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_millis(200))
            .timeout(Duration::from_millis(400))
            .build()
            .expect("client");
        let error = client
            .post("http://127.0.0.1:1/responses")
            .send()
            .await
            .expect_err("nothing listens on port 1");
        let failure = classify_transport_error(&error);
        assert!(
            matches!(
                failure.kind(),
                ProviderFailureKind::NetworkUnreachable | ProviderFailureKind::Timeout
            ),
            "an unreachable provider was classified as {:?}",
            failure.kind()
        );
        assert!(
            failure.retryable(),
            "a network failure is the ordinary transient case"
        );
        assert_eq!(failure.status(), None);
        assert_eq!(CloudError::from(failure), CloudError::TransportFailed);
    }
}
