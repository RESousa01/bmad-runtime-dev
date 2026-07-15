use thiserror::Error;

/// Stable, renderer-safe Method failure codes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MethodErrorCode {
    MethodBindingStale,
    MethodResourceDrift,
    MethodModelBindingDrift,
    ContextDecisionAlreadyConsumed,
    MethodStateConflict,
    MethodResultInvalid,
    MethodStoreRecoveryRequired,
}

impl MethodErrorCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MethodBindingStale => "method_binding_stale",
            Self::MethodResourceDrift => "method_resource_drift",
            Self::MethodModelBindingDrift => "method_model_binding_drift",
            Self::ContextDecisionAlreadyConsumed => "context_decision_already_consumed",
            Self::MethodStateConflict => "method_state_conflict",
            Self::MethodResultInvalid => "method_result_invalid",
            Self::MethodStoreRecoveryRequired => "method_store_recovery_required",
        }
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("BMAD Method operation failed: {code_text}")]
pub struct MethodError {
    code: MethodErrorCode,
    code_text: &'static str,
}

impl MethodError {
    #[must_use]
    pub const fn new(code: MethodErrorCode) -> Self {
        Self {
            code,
            code_text: code.as_str(),
        }
    }

    #[must_use]
    pub const fn code(&self) -> MethodErrorCode {
        self.code
    }
}

impl From<crate::CanonicalHashError> for MethodError {
    fn from(_: crate::CanonicalHashError) -> Self {
        Self::new(MethodErrorCode::MethodResultInvalid)
    }
}
