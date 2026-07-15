//! Strict renderer-to-host transport validation.
//!
//! The renderer can request only the catalog represented by [`LocalCommand`].
//! Validation binds a request to a Rust-issued renderer session and to the
//! installation before a domain command is created.

mod bmad;
mod bmad_help;
mod bmad_run;
mod envelope;
mod gate;
mod unique_json;

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
    let unique_json::UniqueJson(value) =
        serde_json::from_slice(bytes).map_err(|_| IpcValidationError::InvalidJson)?;
    serde_json::from_value(value).map_err(|_| IpcValidationError::InvalidJson)
}
pub use bmad::{
    project_bmad_library, BmadAgentMenuProjection, BmadAgentMenuTargetProjection,
    BmadAgentProjection, BmadHelpActionProjection, BmadInstalledSkillProjection,
    BmadLibrarySnapshotProjection, BmadLibrarySourceKind, BmadLibrarySourceProjection,
    BmadProjectionAvailability, BmadProjectionBlockerCode, BmadProjectionError,
    MAX_BMAD_LIBRARY_PROJECTION_BYTES,
};
pub use bmad_help::{
    project_bmad_help_recommendation, BmadHelpConfidenceProjection, BmadHelpProjectionError,
    BmadHelpRecommendationProjection, MAX_BMAD_HELP_RECOMMENDATION_BYTES,
};
pub use bmad_run::{
    decode_retained_bmad_help_run, project_created_bmad_help_run, BmadHelpRunCreatedProjection,
    MAX_BMAD_HELP_RUN_PROJECTION_BYTES,
};
