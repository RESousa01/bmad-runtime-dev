use serde::{Deserialize, Serialize};

pub type LocalResult<T> = Result<T, LocalError>;

/// Stable, renderer-safe error categories. Detailed causes stay in native logs.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalErrorCode {
    InvalidRequest,
    Unauthorized,
    Conflict,
    NotFound,
    ResourceLimit,
    Expired,
    IntegrityFailure,
    RecoveryRequired,
    TemporarilyUnavailable,
    BmadProjectionUnavailable,
    BmadProjectionGap,
    RendererSessionExpired,
    IdentityUnavailable,
    AuthenticationRequired,
    ReauthenticationRequired,
    TenantMismatch,
    EntitlementUnavailable,
    FeatureDisabled,
    ContextRejected,
    ContextDrift,
    ConsentRequired,
    ConsentExpired,
    ConsentBindingMismatch,
    ConsentAlreadyConsumed,
    SupportPlaneOffline,
    TransportFailed,
    ResponseBindingMismatch,
    InvalidModelOutput,
    ReceiptInvalid,
    Internal,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalError {
    pub code: LocalErrorCode,
    pub safe_message: String,
    pub retryable: bool,
    pub correlation_id: Option<ContractId>,
}

impl LocalError {
    #[must_use]
    pub fn new(code: LocalErrorCode, safe_message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code,
            safe_message: safe_message.into(),
            retryable,
            correlation_id: None,
        }
    }

    #[must_use]
    pub fn with_correlation_id(mut self, correlation_id: ContractId) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }
}

use crate::ContractId;
