//! Strict renderer-to-host transport validation.
//!
//! The renderer can request only the catalog represented by [`LocalCommand`].
//! Validation binds a request to a Rust-issued renderer session and to the
//! installation before a domain command is created.

mod bmad;
mod bmad_completion;
mod bmad_help;
mod bmad_model_access;
mod bmad_run;
mod envelope;
mod gate;

pub use envelope::{
    CommandEnvelopeValidator, IpcReply, IpcValidationContext, IpcValidationError,
    ProjectionEventEnvelope, ValidatedCommandEnvelope, MAX_COMMAND_BYTES,
};
pub use gate::{Admission, AdmissionPolicy, RequestGate};

use serde::de::DeserializeOwned;

/// Deserializes trusted-shape local records while preserving the same
/// duplicate-key rejection used by renderer command envelopes.
///
/// # Errors
///
/// Returns [`IpcValidationError::InvalidJson`] when the input is malformed,
/// contains duplicate object keys, or cannot be converted to `T`.
pub fn deserialize_strict<T>(bytes: &[u8]) -> Result<T, IpcValidationError>
where
    T: DeserializeOwned,
{
    desktop_runtime::deserialize_strict_json(bytes).map_err(|_| IpcValidationError::InvalidJson)
}
/// One bounded retention-manifest row for offboarding (ADR-0004).
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetentionCategoryProjection {
    pub category: String,
    pub count: u64,
}

/// The offboarding retention manifest: category counts and byte totals
/// only — never paths, identifiers, or content.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetentionManifestProjection {
    pub schema_version: String,
    pub categories: Vec<RetentionCategoryProjection>,
    pub retained_bytes: u64,
}

/// The terminal offboarding acknowledgement.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OffboardingErasedProjection {
    pub schema_version: String,
    pub status: String,
    pub restart_required: bool,
}

pub use bmad::{
    project_bmad_library, project_bmad_library_with_activations, project_bmad_persona_perspective,
    BmadAgentMenuProjection, BmadAgentMenuTargetProjection, BmadAgentProjection,
    BmadBuilderPackageKind, BmadBuilderPackageProjection, BmadHelpActionProjection,
    BmadInstalledSkillProjection, BmadLibrarySnapshotProjection, BmadLibrarySourceKind,
    BmadLibrarySourceProjection, BmadPersonaPerspectiveProjection, BmadProjectionAvailability,
    BmadProjectionBlockerCode, BmadProjectionError, MAX_BMAD_LIBRARY_PROJECTION_BYTES,
};
pub use bmad_completion::{
    decode_retained_bmad_help_completion, project_completed_bmad_help_run,
    BmadHelpCompletedRecommendationProjection, BmadHelpEvidenceClassProjection,
    BmadHelpNoRecommendationReasonProjection, BmadHelpReceiptStatusProjection,
    BmadHelpReceiptSummaryInput, BmadHelpReceiptSummaryProjection, BmadHelpRetentionProjection,
    BmadHelpRunCompletedProjection, MAX_BMAD_HELP_COMPLETED_PROJECTION_BYTES,
};
pub use bmad_help::{
    project_bmad_help_recommendation, BmadHelpConfidenceProjection, BmadHelpProjectionError,
    BmadHelpRecommendationProjection, MAX_BMAD_HELP_RECOMMENDATION_BYTES,
};
pub use bmad_model_access::{
    project_bmad_help_approved, project_bmad_help_approved_lifecycle, project_bmad_help_cancelled,
    project_bmad_help_review, project_bmad_help_terminal, project_model_auth_status,
    BmadHelpApprovalInput, BmadHelpApprovedLifecycleInput, BmadHelpApprovedLifecycleProjection,
    BmadHelpApprovedProjection, BmadHelpCancellationInput, BmadHelpCancelledProjection,
    BmadHelpContextClassificationProjection, BmadHelpModelAccessProjectionError,
    BmadHelpReviewExclusionInput, BmadHelpReviewInput, BmadHelpReviewItemInput,
    BmadHelpReviewProjection, BmadHelpReviewRedactionInput, BmadHelpSecretFindingInput,
    BmadHelpTerminalInput, BmadHelpTerminalProjection, BmadHelpTerminalReasonProjection,
    ModelAuthModeProjection, ModelAuthStatusInput, ModelAuthStatusKindProjection,
    ModelAuthStatusProjection, MAX_BMAD_HELP_REVIEW_PROJECTION_BYTES,
};
pub use bmad_run::{
    decode_retained_bmad_help_run, project_created_bmad_help_run, BmadHelpRunCreatedProjection,
    MAX_BMAD_HELP_RUN_PROJECTION_BYTES,
};
