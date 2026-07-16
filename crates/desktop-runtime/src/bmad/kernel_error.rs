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
    #[error("the adoption ledger does not match the sealed expectation")]
    AdoptionLedgerMismatch,
    #[error("the installed BMAD Help invocation does not match its sealed identity chain")]
    SealedHelpMismatch,
    #[error("the compiled BMAD Help binding does not match its sealed source and catalog")]
    SealedHelpBindingMismatch,
    #[error("the verified BMAD Help proposal or its canonical lineage is invalid")]
    HelpProposalInvalid,
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
    #[error("the BMAD help catalog is invalid")]
    HelpCatalogInvalid,
    #[error("a BMAD menu code is ambiguous in its module scope")]
    MenuCodeAmbiguous,
    #[error("a BMAD agent menu target is invalid")]
    AgentMenuTargetInvalid,
    #[error("the BMAD help advisor has insufficient catalog evidence")]
    HelpEvidenceInsufficient,
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
