use desktop_runtime::{BmadHelpRecommendation, BmadLoadedPackage, ContractId};
use serde::{Deserialize, Serialize};

use crate::bmad::{
    BmadLibrarySourceKind, BmadLibrarySourceProjection, BmadProjectionAvailability,
    BmadProjectionBlockerCode,
};
use crate::bmad_help::{
    project_bmad_help_recommendation, BmadHelpConfidenceProjection, BmadHelpProjectionError,
    BmadHelpRecommendationProjection, MAX_BMAD_HELP_RECOMMENDATION_BYTES,
};

pub const MAX_BMAD_HELP_RUN_PROJECTION_BYTES: usize = MAX_BMAD_HELP_RECOMMENDATION_BYTES + 1_024;
const MAX_IDENTIFIER_BYTES: usize = 256;
const MAX_LABEL_BYTES: usize = 256;
const MAX_REASON_BYTES: usize = 4_096;
const MAX_EXPECTED_ARTIFACTS: usize = 16;

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

#[derive(Clone, Copy, Debug, Deserialize)]
enum RetainedBmadHelpRunSchema {
    #[serde(rename = "bmad-help-run.v1")]
    V1,
}

#[derive(Clone, Copy, Debug, Deserialize)]
enum RetainedBmadHelpRecommendationSchema {
    #[serde(rename = "bmad-help-recommendation.v1")]
    V1,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RetainedBmadRunKind {
    BmadHelp,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RetainedBmadRunLifecycle {
    CreatedUnbound,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RetainedBmadLibrarySourceKind {
    SealedFoundation,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RetainedBmadHelpConfidence {
    Authoritative,
    UserAsserted,
    Heuristic,
    Contextual,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum RetainedBmadProjectionAvailability {
    Available,
    CapabilityDisabled,
    DependencyUnavailable,
    OrphanSkill,
    NetworkUnavailable,
    SourcePromptUnavailable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum RetainedBmadProjectionBlockerCode {
    #[serde(rename = "bmad_capability_disabled")]
    CapabilityDisabled,
    #[serde(rename = "bmad_dependency_unavailable")]
    DependencyUnavailable,
    #[serde(rename = "bmad_help_catalog_orphan")]
    HelpCatalogOrphan,
    #[serde(rename = "bmad_network_reference_unavailable")]
    NetworkReferenceUnavailable,
    #[serde(rename = "bmad_source_prompt_unavailable")]
    SourcePromptUnavailable,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RetainedBmadLibrarySourceProjection {
    source_kind: RetainedBmadLibrarySourceKind,
    package_name: String,
    package_version: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RetainedBmadHelpRecommendationProjection {
    schema_version: RetainedBmadHelpRecommendationSchema,
    display_name: String,
    module_code: String,
    skill_name: String,
    action: Option<String>,
    source: RetainedBmadLibrarySourceProjection,
    confidence: RetainedBmadHelpConfidence,
    reason: String,
    required_guidance: bool,
    expected_artifacts: Vec<String>,
    availability: RetainedBmadProjectionAvailability,
    blocker_codes: Vec<RetainedBmadProjectionBlockerCode>,
    completion_claimed: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RetainedBmadHelpRunCreatedProjection {
    schema_version: RetainedBmadHelpRunSchema,
    run_kind: RetainedBmadRunKind,
    lifecycle: RetainedBmadRunLifecycle,
    workspace_id: ContractId,
    run_id: ContractId,
    session_id: ContractId,
    runnable: bool,
    completion_claimed: bool,
    recommendation: RetainedBmadHelpRecommendationProjection,
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

/// Strictly decodes an authenticated retained Help-run projection.
///
/// Retained bytes remain data, not authority. The decoder accepts only the
/// closed v1 inert shape, rejects duplicate and unknown fields, and binds the
/// decoded identifiers to the authenticated store record selected by the
/// caller.
///
/// # Errors
///
/// Returns [`BmadHelpProjectionError::Unavailable`] if the bytes exceed the
/// projection limit, are not strict JSON, contain a non-v1 or non-inert value,
/// fail display-safety validation, or do not match every expected identity.
pub fn decode_retained_bmad_help_run(
    bytes: &[u8],
    expected_workspace_id: &ContractId,
    expected_run_id: &ContractId,
    expected_session_id: &ContractId,
) -> Result<BmadHelpRunCreatedProjection, BmadHelpProjectionError> {
    if bytes.len() > MAX_BMAD_HELP_RUN_PROJECTION_BYTES {
        return Err(BmadHelpProjectionError::Unavailable);
    }
    let retained: RetainedBmadHelpRunCreatedProjection =
        crate::deserialize_strict(bytes).map_err(|_| BmadHelpProjectionError::Unavailable)?;
    if retained.workspace_id != *expected_workspace_id
        || retained.run_id != *expected_run_id
        || retained.session_id != *expected_session_id
        || retained.runnable
        || retained.completion_claimed
        || !valid_retained_recommendation(&retained.recommendation)
    {
        return Err(BmadHelpProjectionError::Unavailable);
    }

    Ok(BmadHelpRunCreatedProjection {
        schema_version: restore_run_schema(retained.schema_version),
        run_kind: restore_run_kind(retained.run_kind),
        lifecycle: restore_run_lifecycle(retained.lifecycle),
        workspace_id: retained.workspace_id,
        run_id: retained.run_id,
        session_id: retained.session_id,
        runnable: false,
        completion_claimed: false,
        recommendation: restore_recommendation(retained.recommendation),
    })
}

fn valid_retained_recommendation(value: &RetainedBmadHelpRecommendationProjection) -> bool {
    !value.completion_claimed
        && value.expected_artifacts.len() <= MAX_EXPECTED_ARTIFACTS
        && valid_nonempty_text(&value.display_name, MAX_LABEL_BYTES)
        && valid_identifier(&value.module_code)
        && valid_identifier(&value.skill_name)
        && value.action.as_deref().is_none_or(valid_identifier)
        && valid_nonempty_text(&value.source.package_name, MAX_IDENTIFIER_BYTES)
        && valid_nonempty_text(&value.source.package_version, MAX_IDENTIFIER_BYTES)
        && valid_nonempty_text(&value.reason, MAX_REASON_BYTES)
        && value
            .expected_artifacts
            .iter()
            .all(|artifact| valid_nonempty_text(artifact, MAX_LABEL_BYTES))
        && valid_retained_blockers(value.availability, &value.blocker_codes)
}

fn valid_retained_blockers(
    availability: RetainedBmadProjectionAvailability,
    blocker_codes: &[RetainedBmadProjectionBlockerCode],
) -> bool {
    match availability {
        RetainedBmadProjectionAvailability::Available => blocker_codes.is_empty(),
        RetainedBmadProjectionAvailability::CapabilityDisabled => {
            blocker_codes == [RetainedBmadProjectionBlockerCode::CapabilityDisabled]
        }
        RetainedBmadProjectionAvailability::DependencyUnavailable => {
            blocker_codes == [RetainedBmadProjectionBlockerCode::DependencyUnavailable]
        }
        RetainedBmadProjectionAvailability::OrphanSkill => {
            blocker_codes == [RetainedBmadProjectionBlockerCode::HelpCatalogOrphan]
        }
        RetainedBmadProjectionAvailability::NetworkUnavailable => {
            blocker_codes == [RetainedBmadProjectionBlockerCode::NetworkReferenceUnavailable]
        }
        RetainedBmadProjectionAvailability::SourcePromptUnavailable => {
            blocker_codes == [RetainedBmadProjectionBlockerCode::SourcePromptUnavailable]
        }
    }
}

fn restore_recommendation(
    retained: RetainedBmadHelpRecommendationProjection,
) -> BmadHelpRecommendationProjection {
    BmadHelpRecommendationProjection {
        schema_version: restore_recommendation_schema(retained.schema_version),
        display_name: retained.display_name,
        module_code: retained.module_code,
        skill_name: retained.skill_name,
        action: retained.action,
        source: BmadLibrarySourceProjection {
            source_kind: restore_source_kind(retained.source.source_kind),
            package_name: retained.source.package_name,
            package_version: retained.source.package_version,
        },
        confidence: match retained.confidence {
            RetainedBmadHelpConfidence::Authoritative => {
                BmadHelpConfidenceProjection::Authoritative
            }
            RetainedBmadHelpConfidence::UserAsserted => BmadHelpConfidenceProjection::UserAsserted,
            RetainedBmadHelpConfidence::Heuristic => BmadHelpConfidenceProjection::Heuristic,
            RetainedBmadHelpConfidence::Contextual => BmadHelpConfidenceProjection::Contextual,
            RetainedBmadHelpConfidence::Unknown => BmadHelpConfidenceProjection::Unknown,
        },
        reason: retained.reason,
        required_guidance: retained.required_guidance,
        expected_artifacts: retained.expected_artifacts,
        availability: restore_availability(retained.availability),
        blocker_codes: retained
            .blocker_codes
            .into_iter()
            .map(restore_blocker_code)
            .collect(),
        completion_claimed: false,
    }
}

const fn restore_run_schema(value: RetainedBmadHelpRunSchema) -> &'static str {
    match value {
        RetainedBmadHelpRunSchema::V1 => "bmad-help-run.v1",
    }
}

const fn restore_recommendation_schema(
    value: RetainedBmadHelpRecommendationSchema,
) -> &'static str {
    match value {
        RetainedBmadHelpRecommendationSchema::V1 => "bmad-help-recommendation.v1",
    }
}

const fn restore_run_kind(value: RetainedBmadRunKind) -> BmadRunKindProjection {
    match value {
        RetainedBmadRunKind::BmadHelp => BmadRunKindProjection::BmadHelp,
    }
}

const fn restore_run_lifecycle(value: RetainedBmadRunLifecycle) -> BmadRunLifecycleProjection {
    match value {
        RetainedBmadRunLifecycle::CreatedUnbound => BmadRunLifecycleProjection::CreatedUnbound,
    }
}

const fn restore_source_kind(value: RetainedBmadLibrarySourceKind) -> BmadLibrarySourceKind {
    match value {
        RetainedBmadLibrarySourceKind::SealedFoundation => BmadLibrarySourceKind::SealedFoundation,
    }
}

const fn restore_availability(
    value: RetainedBmadProjectionAvailability,
) -> BmadProjectionAvailability {
    match value {
        RetainedBmadProjectionAvailability::Available => BmadProjectionAvailability::Available,
        RetainedBmadProjectionAvailability::CapabilityDisabled => {
            BmadProjectionAvailability::CapabilityDisabled
        }
        RetainedBmadProjectionAvailability::DependencyUnavailable => {
            BmadProjectionAvailability::DependencyUnavailable
        }
        RetainedBmadProjectionAvailability::OrphanSkill => BmadProjectionAvailability::OrphanSkill,
        RetainedBmadProjectionAvailability::NetworkUnavailable => {
            BmadProjectionAvailability::NetworkUnavailable
        }
        RetainedBmadProjectionAvailability::SourcePromptUnavailable => {
            BmadProjectionAvailability::SourcePromptUnavailable
        }
    }
}

const fn restore_blocker_code(
    value: RetainedBmadProjectionBlockerCode,
) -> BmadProjectionBlockerCode {
    match value {
        RetainedBmadProjectionBlockerCode::CapabilityDisabled => {
            BmadProjectionBlockerCode::BmadCapabilityDisabled
        }
        RetainedBmadProjectionBlockerCode::DependencyUnavailable => {
            BmadProjectionBlockerCode::BmadDependencyUnavailable
        }
        RetainedBmadProjectionBlockerCode::HelpCatalogOrphan => {
            BmadProjectionBlockerCode::BmadHelpCatalogOrphan
        }
        RetainedBmadProjectionBlockerCode::NetworkReferenceUnavailable => {
            BmadProjectionBlockerCode::BmadNetworkReferenceUnavailable
        }
        RetainedBmadProjectionBlockerCode::SourcePromptUnavailable => {
            BmadProjectionBlockerCode::BmadSourcePromptUnavailable
        }
    }
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn valid_nonempty_text(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= max_bytes
        && !value.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '\u{061c}'
                        | '\u{200e}'
                        | '\u{200f}'
                        | '\u{202a}'..='\u{202e}'
                        | '\u{2066}'..='\u{2069}'
                )
        })
}
