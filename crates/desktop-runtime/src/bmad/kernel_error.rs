use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum BmadKernelErrorCode {
    #[error("BMAD source path is invalid")]
    SourcePathInvalid,
    #[error("BMAD source snapshot exceeds its bounded-read limits")]
    SourceLimitExceeded,
    #[error("BMAD source snapshot contains an aliased path")]
    SourceAliasConflict,
    #[error("the normalized BMAD package descriptor is missing")]
    DescriptorMissing,
    #[error("the normalized BMAD package descriptor is invalid")]
    DescriptorInvalid,
    #[error("the normalized BMAD package descriptor hash does not match")]
    DescriptorHashMismatch,
    #[error("the semantic source ledger does not match the sealed expectation")]
    SemanticLedgerMismatch,
    #[error("the observed final BMAD inventory does not match the descriptor")]
    FinalInventoryMismatch,
    #[error("a managed BMAD resource does not match the descriptor")]
    ManagedResourceMismatch,
    #[error("the BMAD config graph is invalid")]
    ConfigGraphInvalid,
    #[error("the BMAD config graph cannot be merged safely")]
    ConfigMergeConflict,
    #[error("BMAD config attempted to grant host policy or authority")]
    ConfigPolicyForbidden,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{code}")]
pub struct BmadKernelError {
    code: BmadKernelErrorCode,
}

impl BmadKernelError {
    #[must_use]
    pub const fn new(code: BmadKernelErrorCode) -> Self {
        Self { code }
    }

    #[must_use]
    pub const fn code(&self) -> BmadKernelErrorCode {
        self.code
    }
}

impl From<BmadKernelErrorCode> for BmadKernelError {
    fn from(code: BmadKernelErrorCode) -> Self {
        Self::new(code)
    }
}
