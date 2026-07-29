mod action_plan;
pub mod architecture;
mod artifacts;
mod binding;
mod builder;
mod builder_ports;
pub mod capability;
mod catalog;
mod completion;
mod config;
mod customization;
mod review;
mod failure;
mod context_projection;
mod dependencies;
mod error;
mod help;
mod help_binding;
mod help_materialization;
mod help_run;
mod help_session;
mod invocation;
mod kernel_error;
pub mod memlog;
mod memory;
mod method;
mod model_port;
mod orchestrator;
mod package;
mod ports;
mod resources;
mod service;
mod session;
mod source;
mod state_machine;
pub mod toml_subset;
pub mod workflow;

pub use action_plan::{
    build_plan, required_authority_class, validate_plan, BmadActionPlanError, BmadActionPlanRequest,
};
pub use architecture::{
    BmadArchitectureDecision, BmadArchitectureError, BmadArchitectureSession, BmadArchitectureStep,
    BmadArchitectureStepRecord, BMAD_ARCHITECTURE_SESSION_SCHEMA,
};
pub use artifacts::{
    artifact_lineage_view, reproduction_hash, BmadArtifactIntent, BmadArtifactIntentKind,
    BmadArtifactLineageView, BmadArtifactTransitionError,
};
pub use binding::{
    BmadCapabilityKey, MethodAgentBinding, MethodAgentBindingData, MethodArtifactExpectation,
    MethodContextDecision, MethodEvidenceClass, MethodExactBinding, MethodExecutionProfile,
    MethodExecutionProfileData, MethodInvocationModes, MethodModelBinding, MethodModelBindingData,
    MethodResourcePolicy, MethodRuntimeRequirement,
};
pub use builder::{
    BmadUtcInstant, BuilderActionName, BuilderAnalysisContextDecision,
    BuilderAnalysisDecisionConsumption, BuilderAnalysisDecisionInvalidation,
    BuilderAnalysisDecisionInvalidationReason, BuilderAnalysisKind, BuilderAnalysisModelBinding,
    BuilderAnalysisRun, BuilderAuthoringAction, BuilderCapabilityFact, BuilderDeterministicFinding,
    BuilderDraft, BuilderDraftRecord, BuilderDraftRevision, BuilderDraftScope, BuilderDraftState,
    BuilderError, BuilderErrorCode, BuilderFindingSeverity, BuilderKind, BuilderLensVerdict,
    BuilderModelAnalysisDecisionInput, BuilderModelFinding, BuilderModelLens,
    BuilderModelLensResult, BuilderModelLensesNotPerformedReason, BuilderPersistenceEvent,
    BuilderProposedFile, BuilderProposedFileSet, BuilderRendererProjection,
    BuilderValidationProfile,
};
pub use builder_ports::{BuilderAuthoringService, BuilderDraftRepository, BuilderServiceError};
pub use capability::{
    BmadBuilderDraftFile, BmadBuilderDraftKind, BmadCandidateChange, BmadCapabilityOutput,
    BmadCapabilityRun, BmadCapabilityRunError, BmadCapabilityRunParams, BmadClosureCapabilityId,
    BmadDocumentArtifact, BmadDocumentLogEntry, BmadDocumentSection, BmadGovernedChangeSet,
    BmadInactiveBuilderDraft, BMAD_CAPABILITY_RUN_SCHEMA, BMAD_DOCUMENT_ARTIFACT_SCHEMA,
    BMAD_GOVERNED_CHANGE_SET_SCHEMA, BMAD_INACTIVE_BUILDER_DRAFT_SCHEMA,
};
pub use catalog::{
    BmadAgentMenuRecord, BmadAgentRecord, BmadAgentRoster, BmadAgentRosterBuilder, BmadAgentSource,
    BmadCatalog, BmadCatalogAvailability, BmadCatalogBuilder, BmadHelpAction, BmadHelpActionKey,
    BmadHelpCatalogSource, BmadInstalledSkillRecord, BmadMenuTargetKind,
    BmadReviewedPromptReference, BmadUnavailableDependency,
};
pub use completion::{BmadCompletionError, BmadCompletionIntent};
pub use config::{
    BmadConfigGraph, BmadConfigGraphKind, BmadConfigLayer, BmadConfigResolution,
    BmadConfigResolver, BmadConfigWarning, BmadResolvedConfig,
};
pub use customization::{
    BmadCustomizationAvailability, BmadCustomizationConflict, BmadCustomizationDeclaration,
    BmadCustomizationDeclarationKind, BmadCustomizationError, BmadCustomizationLayer,
    BmadCustomizationLayerKind, BmadCustomizationLayerRecord, BmadCustomizationMode,
    BmadCustomizationRequest, BmadCustomizationResolution, BmadCustomizationResolver,
    BmadCustomizationStatus, BmadCustomizationTextOverlay,
};
pub use review::BmadReviewError;
pub use failure::BmadFailureError;
pub use context_projection::BmadContextProjectionError;
pub use dependencies::BmadDependencyError;
pub use error::{MethodError, MethodErrorCode};
pub use help::{
    BmadArtifactEvidence, BmadArtifactEvidenceKind, BmadHelpAdvisor, BmadHelpConfidence,
    BmadHelpIntent, BmadHelpRecommendation, BmadHelpSourceRef,
};
pub use help_binding::{
    BmadCompiledHelpInvocation, BmadHelpBindingCompiler, BmadTrustedHelpModelProfile,
    BmadTrustedHelpModelProfileData,
};
pub use help_materialization::{
    validate_bmad_help_proposal_schema, BmadArtifactClassification, BmadArtifactReference,
    BmadCanonicalAdvanceResult, BmadCanonicalHelpRecords, BmadContentReference,
    BmadHelpEvidenceClass, BmadHelpEvidenceToken, BmadHelpMaterializer,
    BmadHelpNoRecommendationReason, BmadHelpRecordIds, BmadMethodHelpRecommendation,
    BmadVerifiedHelpProposal,
};
pub use help_run::{
    BmadHostInputReplacement, BmadLoadedMethodPackage, BmadQualifiedHelpSource,
    BmadSealedHelpInvocation,
};
pub use help_session::{
    CreateInertBmadHelpSession, InertBmadHelpSession, InertBmadHelpSessionCoordinator,
    InertBmadHelpSessionError, InertBmadHelpSessionPreparationError,
};
pub use invocation::{
    is_headless, BmadInvocationError, BmadInvocationRequest, BmadInvocationResolver,
    BmadInvocationSuppliedValue,
};
pub use kernel_error::{BmadKernelError, BmadKernelErrorCode};
pub use memlog::{
    labelled_body_part, Memlog, MemlogEntry, MemlogEntryKind, MemlogError, MEMLOG_FILE_NAME,
};
pub use memory::{
    authoritative_entries, compact, compute_resume_state, entries_for_scope, insert_entry,
    is_authoritative_origin, project_for_export, supersede_entry, BmadMemoryError,
    BmadMemoryProjectedEntry, BmadResumeState, MAX_MEMORY_ENTRIES, MAX_SOURCE_ENTRY_IDS,
};
pub use method::{
    CreateMethodSession, MethodAdvanceDisposition, MethodAdvanceReceipt, MethodAdvanceRequest,
    MethodAdvanceResult, MethodArtifactProvenance, MethodCanonicalAdvanceResultData,
    MethodCheckpoint, MethodPersistenceEvent, MethodRendererProjection, MethodSession,
    MethodSessionScope, MethodState, MethodStepTable, MethodVerifiedAdvanceResult,
    MethodVerifiedResultBindingData,
};
pub use model_port::{
    accept_model_response, record_repair, record_retry, BmadClosedOutputSchema, BmadModelCallUsage,
    BmadModelPortError,
};
pub use orchestrator::{plan_source_steps, BmadOrchestratorError, BmadSourceStep};
pub use package::{
    BmadEntrypointKind, BmadLoadedPackage, BmadLoadedSkill, BmadLocationClass, BmadPackageLoader,
    BmadSourceEntry, BmadSourceKind, BmadSourceSnapshot,
};
pub use ports::MethodSessionRepository;
pub use resources::{
    BmadPackageRoot, BmadResolvedResource, BmadResource, BmadResourceClosure,
    BmadResourceEdgeDisposition, BmadResourceEdgeDispositionKind, BmadResourceInput,
    BmadResourceProvenance, BmadResourceReferenceKind, BmadResourceRequirement,
    BmadResourceResolver, BmadResourceResolverError, BmadResourceSet,
};
pub use service::{MethodServiceError, MethodSessionService};
pub use session::{BmadSessionIntent, BmadSessionTransitionError};
pub use source::{BmadSourcePackage, BmadSourceService};
pub use state_machine::{
    apply_command, is_terminal, project, BmadSessionCommand, BmadSessionCommandError,
    BmadSessionEvent, BmadSessionProjection,
};
pub use toml_subset::{parse_toml_subset, TomlSubsetError};
pub use workflow::{BmadWorkflowGraph, BmadWorkflowPhase, BmadWorkflowState, BmadWorkflowStep};

/// Host-facing BMAD intent boundary.
///
/// Implementations orchestrate persistence and authority outside the domain.
/// Values crossing this port are generated contracts; renderer requests and
/// provider clients are intentionally excluded.
pub trait BmadRuntimePort {
    /// Applies one validated session intent.
    ///
    /// # Errors
    ///
    /// Returns a transition error when the current generated state rejects the
    /// intent.
    fn apply_session_intent(
        &self,
        session: &mut crate::generated_contracts::BmadSession,
        intent: BmadSessionIntent,
    ) -> Result<(), BmadSessionTransitionError>;

    /// Applies one validated artifact intent.
    ///
    /// # Errors
    ///
    /// Returns a transition error when the current generated state rejects the
    /// intent.
    fn apply_artifact_intent(
        &self,
        artifact: &mut crate::generated_contracts::BmadArtifact,
        intent: BmadArtifactIntent,
    ) -> Result<(), BmadArtifactTransitionError>;

    /// Plans inert completion intents for host authorization.
    ///
    /// # Errors
    ///
    /// Returns a completion error until session and artifact preconditions are
    /// satisfied.
    fn plan_completion(
        &self,
        session: &crate::generated_contracts::BmadSession,
        artifacts: &[crate::generated_contracts::BmadArtifact],
    ) -> Result<Vec<BmadCompletionIntent>, BmadCompletionError>;
}

/// Stateless implementation of the generated-contract intent boundary.
///
/// Keeping this value zero-sized prevents the facade from accumulating
/// cross-domain state or authority. Persistence and effects remain host ports.
#[derive(Clone, Copy, Debug, Default)]
pub struct BmadDomainRuntime;

impl BmadRuntimePort for BmadDomainRuntime {
    fn apply_session_intent(
        &self,
        session: &mut crate::generated_contracts::BmadSession,
        intent: BmadSessionIntent,
    ) -> Result<(), BmadSessionTransitionError> {
        session::apply_session_intent(session, intent)
    }

    fn apply_artifact_intent(
        &self,
        artifact: &mut crate::generated_contracts::BmadArtifact,
        intent: BmadArtifactIntent,
    ) -> Result<(), BmadArtifactTransitionError> {
        artifacts::apply_artifact_intent(artifact, intent)
    }

    fn plan_completion(
        &self,
        session: &crate::generated_contracts::BmadSession,
        artifacts: &[crate::generated_contracts::BmadArtifact],
    ) -> Result<Vec<BmadCompletionIntent>, BmadCompletionError> {
        completion::plan_completion(session, artifacts)
    }
}
