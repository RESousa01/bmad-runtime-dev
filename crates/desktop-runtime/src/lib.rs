//! Authority-owned domain types shared by the Windows desktop host crates.
//!
//! This crate deliberately contains no Tauri, filesystem, database, network, or
//! process APIs. It defines the values which may cross those ports and the
//! deterministic hashes that bind security-sensitive decisions.
//!
//! Sealed authority aggregates are internal host-domain values and intentionally
//! do not implement `Deserialize`. The generated-shape tests in this crate prove
//! serialization and hash compatibility only; they are not schema validation.
//! The composition root must run strict JSON, schema, and semantic validation
//! before explicitly mapping an untrusted contract into these types.

mod command;
mod domain;
mod error;
mod hash;
mod ids;

pub use command::{
    ApprovalChoice, CommandReceipt, LocalCommand, LocalRuntimeCommandBus, ProjectionCursor,
    ProjectionEvent, ProjectionEventKind, ProjectionScope, ProjectionSnapshot, RendererProjection,
};
pub use domain::{
    ApprovalDecision, ApprovalDecisionDraft, ApprovalOutcome, ApprovedExecutionSpec,
    ApprovedExecutionSpecDraft, AuthorityRef, CandidateCommon, DeclaredWrite,
    DeclaredWriteOperation, DeliveryModel, DomainValidationError, ExecutionLimits, InputKind,
    LocalPathPreimage, MutableInputBinding, NativePatchEngineAudience, PatchOperation, PatchSet,
    RollbackClass, SpecConsumptionRecord, SpecConsumptionRecordDraft, WindowsPatchCandidate,
    WindowsPatchCandidateDraft, WorkspaceTarget, HARD_MAX_CHANGED_BYTES, HARD_MAX_CHANGED_FILES,
};
pub use error::{LocalError, LocalErrorCode, LocalResult};
pub use hash::{
    canonical_hash, canonical_hash_without_field, canonical_json_bytes, sha256_bytes,
    CanonicalHashError, Sha256Digest,
};
pub use ids::{ContractId, IdentifierError, RelativeWorkspacePath, UnixMillis};
