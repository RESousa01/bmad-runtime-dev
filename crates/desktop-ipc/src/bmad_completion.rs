use desktop_runtime::{
    canonical_hash_without_field, BmadHelpEvidenceClass, BmadHelpNoRecommendationReason,
    BmadMethodHelpRecommendation, ContractId, UnixMillis,
};
use serde::{Deserialize, Serialize};

use crate::bmad_help::BmadHelpProjectionError;

pub const MAX_BMAD_HELP_COMPLETED_PROJECTION_BYTES: usize = 16 * 1024;
const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_DISPLAY_NAME_BYTES: usize = 256;
const MAX_IDENTIFIER_BYTES: usize = 256;
const MAX_RATIONALE_BYTES: usize = 4_096;
const MAX_RECEIPT_INPUT_BYTES: u64 = 4 * 1024 * 1024;
const MAX_RECEIPT_OUTPUT_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum BmadHelpCompletedRunKindProjection {
    BmadHelp,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum BmadHelpCompletedLifecycleProjection {
    Completed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadHelpReceiptStatusProjection {
    Succeeded,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadHelpRetentionProjection {
    TransientNoStore,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadHelpEvidenceClassProjection {
    Authoritative,
    UserAsserted,
    Heuristic,
    Contextual,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadHelpNoRecommendationReasonProjection {
    CatalogEvidenceAbsent,
    CompletionEvidenceAmbiguous,
    DependencyUnavailable,
}

/// Renderer-safe fields copied from an already verified model-access receipt.
///
/// Hashes, provider identity, deployment identity, consent, binding material,
/// and receipt proof are deliberately absent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadHelpReceiptSummaryInput {
    pub receipt_id: ContractId,
    pub status: BmadHelpReceiptStatusProjection,
    pub retention_mode: BmadHelpRetentionProjection,
    pub region: String,
    pub input_bytes: u64,
    pub output_bytes: u64,
    pub started_at: UnixMillis,
    pub completed_at: UnixMillis,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BmadHelpReceiptSummaryProjection {
    schema_version: String,
    receipt_id: ContractId,
    status: BmadHelpReceiptStatusProjection,
    retention_mode: BmadHelpRetentionProjection,
    region: String,
    input_bytes: u64,
    output_bytes: u64,
    started_at: UnixMillis,
    completed_at: UnixMillis,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "recommendationKind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum BmadHelpCompletedRecommendationProjection {
    RecommendedCapability {
        display_name: String,
        module_code: String,
        skill_name: String,
        action: Option<String>,
        evidence_class: BmadHelpEvidenceClassProjection,
        guidance_required: bool,
        rationale_summary: String,
        created_at: UnixMillis,
    },
    NoRecommendation {
        reason_code: BmadHelpNoRecommendationReasonProjection,
        created_at: UnixMillis,
    },
}

/// Strict renderer-safe completed BMAD Help projection.
///
/// This type contains display data only. It is neither model authority nor a
/// substitute for the authenticated canonical Method records retained by the
/// native host.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BmadHelpRunCompletedProjection {
    schema_version: String,
    run_kind: BmadHelpCompletedRunKindProjection,
    lifecycle: BmadHelpCompletedLifecycleProjection,
    workspace_id: ContractId,
    run_id: ContractId,
    session_id: ContractId,
    runnable: bool,
    completion_claimed: bool,
    recommendation: BmadHelpCompletedRecommendationProjection,
    receipt: BmadHelpReceiptSummaryProjection,
}

/// Builds one bounded completed projection from canonical Method output and an
/// already verified metadata-only receipt summary.
///
/// The optional display name must be present only for the recommended branch;
/// all capability identity fields are derived from the canonical
/// recommendation rather than accepted from a renderer or model response.
///
/// # Errors
///
/// Returns [`BmadHelpProjectionError::Unavailable`] for noncanonical Method
/// data, identity drift, unsafe display text, absolute-path disclosure,
/// invalid receipt metadata, impossible chronology, or an oversized result.
pub fn project_completed_bmad_help_run(
    recommendation: &BmadMethodHelpRecommendation,
    recommended_display_name: Option<&str>,
    workspace_id: ContractId,
    run_id: ContractId,
    session_id: ContractId,
    receipt: &BmadHelpReceiptSummaryInput,
) -> Result<BmadHelpRunCompletedProjection, BmadHelpProjectionError> {
    if !valid_canonical_recommendation(recommendation) {
        return Err(BmadHelpProjectionError::Unavailable);
    }

    let recommendation = match recommendation {
        BmadMethodHelpRecommendation::RecommendedCapability {
            session_id: recommendation_session_id,
            capability_key,
            evidence_class,
            evidence_refs,
            guidance_required,
            rationale_summary,
            created_at,
            ..
        } => {
            let display_name = recommended_display_name
                .filter(|value| valid_display_text(value, MAX_DISPLAY_NAME_BYTES))
                .ok_or(BmadHelpProjectionError::Unavailable)?;
            let evidence_class = project_evidence_class(*evidence_class)
                .ok_or(BmadHelpProjectionError::Unavailable)?;
            if recommendation_session_id != &session_id
                || evidence_refs.is_empty()
                || !valid_identifier(&capability_key.module_code)
                || !valid_identifier(&capability_key.skill_name)
                || capability_key
                    .normalized_action
                    .as_deref()
                    .is_some_and(|action| !valid_identifier(action))
                || !valid_display_text(rationale_summary, MAX_RATIONALE_BYTES)
                || !valid_timestamp(*created_at)
            {
                return Err(BmadHelpProjectionError::Unavailable);
            }
            BmadHelpCompletedRecommendationProjection::RecommendedCapability {
                display_name: display_name.to_owned(),
                module_code: capability_key.module_code.clone(),
                skill_name: capability_key.skill_name.clone(),
                action: capability_key.normalized_action.clone(),
                evidence_class,
                guidance_required: *guidance_required,
                rationale_summary: rationale_summary.clone(),
                created_at: *created_at,
            }
        }
        BmadMethodHelpRecommendation::NoRecommendation {
            session_id: recommendation_session_id,
            evidence_class,
            reason_code,
            created_at,
            ..
        } => {
            if recommended_display_name.is_some()
                || recommendation_session_id != &session_id
                || *evidence_class != BmadHelpEvidenceClass::Unknown
                || !valid_timestamp(*created_at)
            {
                return Err(BmadHelpProjectionError::Unavailable);
            }
            BmadHelpCompletedRecommendationProjection::NoRecommendation {
                reason_code: project_no_recommendation_reason(*reason_code),
                created_at: *created_at,
            }
        }
    };

    let projection = BmadHelpRunCompletedProjection {
        schema_version: "bmad-help-completed.v1".to_owned(),
        run_kind: BmadHelpCompletedRunKindProjection::BmadHelp,
        lifecycle: BmadHelpCompletedLifecycleProjection::Completed,
        workspace_id,
        run_id,
        session_id,
        runnable: false,
        completion_claimed: true,
        recommendation,
        receipt: BmadHelpReceiptSummaryProjection {
            schema_version: "bmad-model-receipt-summary.v1".to_owned(),
            receipt_id: receipt.receipt_id.clone(),
            status: receipt.status,
            retention_mode: receipt.retention_mode,
            region: receipt.region.clone(),
            input_bytes: receipt.input_bytes,
            output_bytes: receipt.output_bytes,
            started_at: receipt.started_at,
            completed_at: receipt.completed_at,
        },
    };
    if !projection.valid(
        &projection.workspace_id,
        &projection.run_id,
        &projection.session_id,
    ) || serde_json::to_vec(&projection)
        .map_err(|_| BmadHelpProjectionError::Unavailable)?
        .len()
        > MAX_BMAD_HELP_COMPLETED_PROJECTION_BYTES
    {
        return Err(BmadHelpProjectionError::Unavailable);
    }
    Ok(projection)
}

/// Strictly decodes authenticated retained completed-projection bytes.
///
/// Retained bytes remain display data, not authority. The decoder rejects
/// duplicate/unknown fields and binds workspace, run, and session identifiers
/// to the authenticated record selected by the caller.
///
/// # Errors
///
/// Returns [`BmadHelpProjectionError::Unavailable`] for invalid or oversized
/// JSON, closed-literal drift, unsafe display text, invalid receipt metadata,
/// impossible chronology, or expected-identity substitution.
pub fn decode_retained_bmad_help_completion(
    bytes: &[u8],
    expected_workspace_id: &ContractId,
    expected_run_id: &ContractId,
    expected_session_id: &ContractId,
) -> Result<BmadHelpRunCompletedProjection, BmadHelpProjectionError> {
    if bytes.is_empty() || bytes.len() > MAX_BMAD_HELP_COMPLETED_PROJECTION_BYTES {
        return Err(BmadHelpProjectionError::Unavailable);
    }
    let projection: BmadHelpRunCompletedProjection =
        crate::deserialize_strict(bytes).map_err(|_| BmadHelpProjectionError::Unavailable)?;
    if !projection.valid(expected_workspace_id, expected_run_id, expected_session_id) {
        return Err(BmadHelpProjectionError::Unavailable);
    }
    Ok(projection)
}

impl BmadHelpRunCompletedProjection {
    fn valid(
        &self,
        expected_workspace_id: &ContractId,
        expected_run_id: &ContractId,
        expected_session_id: &ContractId,
    ) -> bool {
        self.schema_version == "bmad-help-completed.v1"
            && self.run_kind == BmadHelpCompletedRunKindProjection::BmadHelp
            && self.lifecycle == BmadHelpCompletedLifecycleProjection::Completed
            && self.workspace_id == *expected_workspace_id
            && self.run_id == *expected_run_id
            && self.session_id == *expected_session_id
            && !self.runnable
            && self.completion_claimed
            && self.receipt.valid()
            && self.recommendation.valid(self.receipt.completed_at)
    }
}

impl BmadHelpReceiptSummaryProjection {
    fn valid(&self) -> bool {
        self.schema_version == "bmad-model-receipt-summary.v1"
            && self.status == BmadHelpReceiptStatusProjection::Succeeded
            && self.retention_mode == BmadHelpRetentionProjection::TransientNoStore
            && valid_region(&self.region)
            && (1..=MAX_RECEIPT_INPUT_BYTES).contains(&self.input_bytes)
            && (1..=MAX_RECEIPT_OUTPUT_BYTES).contains(&self.output_bytes)
            && valid_timestamp(self.started_at)
            && valid_timestamp(self.completed_at)
            && self.started_at <= self.completed_at
    }
}

impl BmadHelpCompletedRecommendationProjection {
    fn valid(&self, receipt_completed_at: UnixMillis) -> bool {
        match self {
            Self::RecommendedCapability {
                display_name,
                module_code,
                skill_name,
                action,
                rationale_summary,
                created_at,
                ..
            } => {
                valid_display_text(display_name, MAX_DISPLAY_NAME_BYTES)
                    && valid_identifier(module_code)
                    && valid_identifier(skill_name)
                    && action.as_deref().is_none_or(valid_identifier)
                    && valid_display_text(rationale_summary, MAX_RATIONALE_BYTES)
                    && valid_timestamp(*created_at)
                    && *created_at >= receipt_completed_at
            }
            Self::NoRecommendation { created_at, .. } => {
                valid_timestamp(*created_at) && *created_at >= receipt_completed_at
            }
        }
    }
}

fn valid_canonical_recommendation(value: &BmadMethodHelpRecommendation) -> bool {
    serde_json::to_value(value)
        .ok()
        .and_then(|shape| {
            canonical_hash_without_field(
                "bmad-method-help-recommendation",
                1,
                &shape,
                "recommendationHash",
            )
            .ok()
        })
        .is_some_and(|expected| expected == value.recommendation_hash())
}

const fn project_evidence_class(
    value: BmadHelpEvidenceClass,
) -> Option<BmadHelpEvidenceClassProjection> {
    match value {
        BmadHelpEvidenceClass::Authoritative => {
            Some(BmadHelpEvidenceClassProjection::Authoritative)
        }
        BmadHelpEvidenceClass::UserAsserted => Some(BmadHelpEvidenceClassProjection::UserAsserted),
        BmadHelpEvidenceClass::Heuristic => Some(BmadHelpEvidenceClassProjection::Heuristic),
        BmadHelpEvidenceClass::Contextual => Some(BmadHelpEvidenceClassProjection::Contextual),
        BmadHelpEvidenceClass::Unknown => None,
    }
}

const fn project_no_recommendation_reason(
    value: BmadHelpNoRecommendationReason,
) -> BmadHelpNoRecommendationReasonProjection {
    match value {
        BmadHelpNoRecommendationReason::CatalogEvidenceAbsent => {
            BmadHelpNoRecommendationReasonProjection::CatalogEvidenceAbsent
        }
        BmadHelpNoRecommendationReason::CompletionEvidenceAmbiguous => {
            BmadHelpNoRecommendationReasonProjection::CompletionEvidenceAmbiguous
        }
        BmadHelpNoRecommendationReason::DependencyUnavailable => {
            BmadHelpNoRecommendationReasonProjection::DependencyUnavailable
        }
    }
}

fn valid_region(value: &str) -> bool {
    (3..=64).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_lowercase())
}

fn valid_timestamp(value: UnixMillis) -> bool {
    (1..=MAX_SAFE_JSON_INTEGER).contains(&value.0)
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn valid_display_text(value: &str, max_bytes: usize) -> bool {
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
        && !contains_absolute_path(value)
}

fn contains_absolute_path(value: &str) -> bool {
    if value.to_ascii_lowercase().contains("file://") {
        return true;
    }
    value.split_whitespace().any(|word| {
        let token = word.trim_start_matches(|character: char| {
            matches!(character, '(' | '[' | '{' | '<' | '"' | '\'')
        });
        let bytes = token.as_bytes();
        token.starts_with('/')
            || token.starts_with("\\\\")
            || (bytes.len() >= 3
                && bytes[0].is_ascii_alphabetic()
                && bytes[1] == b':'
                && matches!(bytes[2], b'\\' | b'/'))
    })
}
