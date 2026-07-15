use desktop_runtime::{BmadHelpRecommendation, BmadLoadedPackage, ContractId};
use serde::Serialize;

use crate::bmad_help::{
    project_bmad_help_recommendation, BmadHelpProjectionError, BmadHelpRecommendationProjection,
    MAX_BMAD_HELP_RECOMMENDATION_BYTES,
};

pub const MAX_BMAD_HELP_RUN_PROJECTION_BYTES: usize = MAX_BMAD_HELP_RECOMMENDATION_BYTES + 1_024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum BmadRunKindProjection {
    BmadHelp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum BmadRunLifecycleProjection {
    CreatedUnbound,
}

/// Exact renderer-safe result of creating an inert, unbound Help run.
///
/// The host cannot project a runnable or completed state through this type.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadHelpRunCreatedProjection {
    schema_version: &'static str,
    run_kind: BmadRunKindProjection,
    lifecycle: BmadRunLifecycleProjection,
    workspace_id: ContractId,
    run_id: ContractId,
    session_id: ContractId,
    runnable: bool,
    completion_claimed: bool,
    recommendation: BmadHelpRecommendationProjection,
}

/// Projects a newly persisted Help session without adding execution authority.
///
/// # Errors
///
/// Returns [`BmadHelpProjectionError::Unavailable`] if the recommendation is
/// unsafe, inconsistent, claims completion, or the complete response exceeds
/// the closed renderer projection limit.
pub fn project_created_bmad_help_run(
    package: &BmadLoadedPackage,
    recommendation: &BmadHelpRecommendation,
    workspace_id: ContractId,
    run_id: ContractId,
    session_id: ContractId,
) -> Result<BmadHelpRunCreatedProjection, BmadHelpProjectionError> {
    let projection = BmadHelpRunCreatedProjection {
        schema_version: "bmad-help-run.v1",
        run_kind: BmadRunKindProjection::BmadHelp,
        lifecycle: BmadRunLifecycleProjection::CreatedUnbound,
        workspace_id,
        run_id,
        session_id,
        runnable: false,
        completion_claimed: false,
        recommendation: project_bmad_help_recommendation(package, recommendation)?,
    };
    if serde_json::to_vec(&projection)
        .map_err(|_| BmadHelpProjectionError::Unavailable)?
        .len()
        > MAX_BMAD_HELP_RUN_PROJECTION_BYTES
    {
        return Err(BmadHelpProjectionError::Unavailable);
    }
    Ok(projection)
}
