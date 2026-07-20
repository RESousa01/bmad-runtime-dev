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

mod bmad;
mod command;
mod domain;
mod edits;
mod error;
mod hash;
mod ids;
mod strict_json;

// Typify emits crate/module-level inner attributes. Loading its output as a
// real module keeps those tool-owned attributes valid; a nested `include!`
// would reject them before the generated shapes could be compiled.
#[allow(dead_code, clippy::all, clippy::pedantic, clippy::unwrap_used)]
#[path = "../../../packages/contracts/generated/rust/contracts.rs"]
#[doc(hidden)]
pub mod generated_contracts;

pub use bmad::{
    validate_bmad_help_proposal_schema, BmadAgentMenuRecord, BmadAgentRecord, BmadAgentRoster,
    BmadAgentRosterBuilder, BmadAgentSource, BmadArtifactClassification, BmadArtifactEvidence,
    BmadArtifactEvidenceKind, BmadArtifactReference, BmadBuilderDraftFile, BmadBuilderDraftKind,
    BmadCandidateChange, BmadCanonicalAdvanceResult, BmadCanonicalHelpRecords, BmadCapabilityKey,
    BmadCapabilityOutput, BmadCapabilityRun, BmadCapabilityRunError, BmadCapabilityRunParams,
    BmadCatalog, BmadCatalogAvailability, BmadCatalogBuilder, BmadClosureCapabilityId,
    BmadCompiledHelpInvocation, BmadConfigGraph, BmadConfigGraphKind, BmadConfigLayer,
    BmadConfigResolution, BmadConfigResolver, BmadConfigWarning, BmadContentReference,
    BmadDocumentArtifact, BmadDocumentSection, BmadEntrypointKind, BmadGovernedChangeSet,
    BmadHelpAction, BmadHelpActionKey, BmadHelpAdvisor, BmadHelpBindingCompiler,
    BmadHelpCatalogSource, BmadHelpConfidence, BmadHelpEvidenceClass, BmadHelpEvidenceToken,
    BmadHelpIntent, BmadHelpMaterializer, BmadHelpNoRecommendationReason, BmadHelpRecommendation,
    BmadHelpRecordIds, BmadHelpSourceRef, BmadHostInputReplacement, BmadInactiveBuilderDraft,
    BmadInstalledSkillRecord, BmadKernelError, BmadKernelErrorCode, BmadLoadedMethodPackage,
    BmadLoadedPackage, BmadLoadedSkill, BmadLocationClass, BmadMenuTargetKind,
    BmadMethodHelpRecommendation, BmadPackageLoader, BmadQualifiedHelpSource, BmadResolvedConfig,
    BmadReviewedPromptReference, BmadSealedHelpInvocation, BmadSourceEntry, BmadSourceKind,
    BmadSourceSnapshot, BmadTrustedHelpModelProfile, BmadTrustedHelpModelProfileData,
    BmadUnavailableDependency, BmadUtcInstant, BmadVerifiedHelpProposal, BuilderActionName,
    BuilderAnalysisContextDecision, BuilderAnalysisDecisionConsumption,
    BuilderAnalysisDecisionInvalidation, BuilderAnalysisDecisionInvalidationReason,
    BuilderAnalysisKind, BuilderAnalysisModelBinding, BuilderAnalysisRun, BuilderAuthoringAction,
    BuilderAuthoringService, BuilderCapabilityFact, BuilderDeterministicFinding, BuilderDraft,
    BuilderDraftRecord, BuilderDraftRepository, BuilderDraftRevision, BuilderDraftScope,
    BuilderDraftState, BuilderError, BuilderErrorCode, BuilderFindingSeverity, BuilderKind,
    BuilderLensVerdict, BuilderModelAnalysisDecisionInput, BuilderModelFinding, BuilderModelLens,
    BuilderModelLensResult, BuilderModelLensesNotPerformedReason, BuilderPersistenceEvent,
    BuilderProposedFile, BuilderProposedFileSet, BuilderRendererProjection, BuilderServiceError,
    BuilderValidationProfile, CreateInertBmadHelpSession, CreateMethodSession,
    InertBmadHelpSession, InertBmadHelpSessionCoordinator, InertBmadHelpSessionError,
    InertBmadHelpSessionPreparationError, MethodAdvanceDisposition, MethodAdvanceReceipt,
    MethodAdvanceRequest, MethodAdvanceResult, MethodAgentBinding, MethodAgentBindingData,
    MethodArtifactExpectation, MethodArtifactProvenance, MethodCanonicalAdvanceResultData,
    MethodCheckpoint, MethodContextDecision, MethodError, MethodErrorCode, MethodEvidenceClass,
    MethodExactBinding, MethodExecutionProfile, MethodExecutionProfileData, MethodInvocationModes,
    MethodModelBinding, MethodModelBindingData, MethodModelPort, MethodPersistenceEvent,
    MethodRendererProjection, MethodResourcePolicy, MethodRuntimeRequirement, MethodServiceError,
    MethodSession, MethodSessionRepository, MethodSessionScope, MethodSessionService, MethodState,
    MethodStepTable, MethodVerifiedAdvanceResult, MethodVerifiedResultBindingData,
    BMAD_CAPABILITY_RUN_SCHEMA, BMAD_DOCUMENT_ARTIFACT_SCHEMA, BMAD_GOVERNED_CHANGE_SET_SCHEMA,
    BMAD_INACTIVE_BUILDER_DRAFT_SCHEMA,
};
pub use command::{
    ApprovalChoice, BmadLibraryProjectionScope, BmadProjectionInvalidationScope, CommandReceipt,
    DensityPreference, LocalCommand, LocalRuntimeCommandBus, ProjectionCursor, ProjectionEvent,
    ProjectionEventKind, ProjectionScope, ProjectionSnapshot, RecoveryApprovalChoice,
    RendererProjection, ThemePreference,
};
pub use domain::{
    ApprovalDecision, ApprovalDecisionDraft, ApprovalOutcome, ApprovedExecutionSpec,
    ApprovedExecutionSpecDraft, AuthorityRef, CandidateCommon, DeclaredWrite,
    DeclaredWriteOperation, DeliveryModel, DesktopLocalIdentity, DomainValidationError,
    ExecutionLimits, InputKind, LocalPathPreimage, MutableInputBinding, NativePatchEngineAudience,
    PatchOperation, PatchSet, RollbackClass, SpecConsumptionRecord, SpecConsumptionRecordDraft,
    WindowsPatchCandidate, WindowsPatchCandidateDraft, WorkspaceTarget, HARD_MAX_CHANGED_BYTES,
    HARD_MAX_CHANGED_FILES,
};
pub use edits::{
    build_changes_candidate, build_changes_review, ChangesProposalBinding, ChangesProposalKind,
    ChangesReviewProjection, EditsError, FileChangeReview, ObservedPreimage,
    PreparedChangesProposal, ProposedFileChange, CHANGES_REVIEW_SCHEMA,
};
pub use error::{LocalError, LocalErrorCode, LocalResult};
pub use hash::{
    canonical_hash, canonical_hash_without_field, canonical_json_bytes, legacy_canonical_hash,
    legacy_canonical_hash_without_field, sha256_bytes, CanonicalHashError, Sha256Digest,
};
pub use ids::{ContractId, IdentifierError, RelativeWorkspacePath, UnixMillis};
pub use strict_json::{deserialize_strict_json, StrictJsonError};
