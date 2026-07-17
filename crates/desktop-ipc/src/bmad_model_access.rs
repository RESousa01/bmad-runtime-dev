use std::collections::HashSet;

use desktop_runtime::{ContractId, RelativeWorkspacePath, Sha256Digest, UnixMillis};
use serde::Serialize;
use thiserror::Error;

use crate::bmad_completion::BmadHelpRetentionProjection;

pub const MAX_BMAD_HELP_REVIEW_PROJECTION_BYTES: usize = 96 * 1024;
const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_DESTINATION_BYTES: usize = 256;
const MAX_DISCLOSURE_BYTES: usize = 4_096;
const MAX_PURPOSE_BYTES: usize = 128;
const MAX_REGION_BYTES: usize = 64;
const MAX_REVIEW_ITEMS: usize = 16;
const MAX_EXCLUSIONS: usize = 32;
const MAX_SECRET_FINDINGS: usize = 64;
const MAX_REDACTIONS_PER_ITEM: usize = 32;
const MAX_SEMANTIC_ROLE_BYTES: usize = 128;
const MAX_LANGUAGE_BYTES: usize = 64;
const MAX_REASON_BYTES: usize = 1_024;
const MAX_KIND_BYTES: usize = 128;
const MAX_OUTBOUND_CONTEXT_BYTES: u64 = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelAuthStatusKindProjection {
    Unavailable,
    DevelopmentReady,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelAuthModeProjection {
    Offline,
    DeterministicDevelopment,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelAuthStatusInput {
    pub status: ModelAuthStatusKindProjection,
    pub mode: ModelAuthModeProjection,
    pub auth_epoch: u64,
    pub development_only: bool,
    pub destination_label: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelAuthStatusProjection {
    status: ModelAuthStatusKindProjection,
    mode: ModelAuthModeProjection,
    auth_epoch: u64,
    development_only: bool,
    destination_label: String,
    sign_in_available: bool,
    sign_out_available: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadHelpContextClassificationProjection {
    Public,
    Internal,
    Confidential,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadHelpReviewRedactionInput {
    pub kind: String,
    pub occurrence_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadHelpReviewItemInput {
    pub relative_label: RelativeWorkspacePath,
    pub semantic_role: String,
    pub language: Option<String>,
    pub outbound_byte_count: u64,
    pub token_estimate: u64,
    pub classification: BmadHelpContextClassificationProjection,
    pub redactions: Vec<BmadHelpReviewRedactionInput>,
    pub outbound_content: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadHelpReviewExclusionInput {
    pub relative_label: RelativeWorkspacePath,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadHelpSecretFindingInput {
    pub relative_label: RelativeWorkspacePath,
    pub kind: String,
    pub occurrence_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadHelpReviewInput {
    pub workspace_id: ContractId,
    pub workspace_grant_epoch: u64,
    pub run_id: ContractId,
    pub session_id: ContractId,
    pub destination_label: String,
    pub development_only: bool,
    pub consent_disclosure: String,
    pub manifest_hash: Sha256Digest,
    pub purpose: String,
    pub region: String,
    pub retention_mode: BmadHelpRetentionProjection,
    pub expires_at: UnixMillis,
    pub items: Vec<BmadHelpReviewItemInput>,
    pub exclusions: Vec<BmadHelpReviewExclusionInput>,
    pub secret_findings: Vec<BmadHelpSecretFindingInput>,
    pub total_outbound_bytes: u64,
    pub total_token_estimate: u64,
    pub redaction_limitation: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct BmadHelpReviewRedactionProjection {
    kind: String,
    occurrence_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct BmadHelpReviewItemProjection {
    relative_label: RelativeWorkspacePath,
    semantic_role: String,
    language: Option<String>,
    outbound_byte_count: u64,
    token_estimate: u64,
    classification: BmadHelpContextClassificationProjection,
    redactions: Vec<BmadHelpReviewRedactionProjection>,
    outbound_content: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct BmadHelpReviewExclusionProjection {
    relative_label: RelativeWorkspacePath,
    reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct BmadHelpSecretFindingProjection {
    relative_label: RelativeWorkspacePath,
    kind: String,
    occurrence_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadHelpReviewProjection {
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    run_id: ContractId,
    session_id: ContractId,
    destination_label: String,
    development_only: bool,
    consent_disclosure: String,
    manifest_hash: Sha256Digest,
    purpose: String,
    region: String,
    retention_mode: BmadHelpRetentionProjection,
    expires_at: UnixMillis,
    items: Vec<BmadHelpReviewItemProjection>,
    exclusions: Vec<BmadHelpReviewExclusionProjection>,
    secret_findings: Vec<BmadHelpSecretFindingProjection>,
    total_outbound_bytes: u64,
    total_token_estimate: u64,
    redaction_limitation: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadHelpApprovalInput {
    pub manifest_hash: Sha256Digest,
    pub decision_id: ContractId,
    pub expires_at: UnixMillis,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadHelpApprovedProjection {
    manifest_hash: Sha256Digest,
    decision_id: ContractId,
    expires_at: UnixMillis,
    send_eligible: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadHelpApprovedLifecycleInput {
    pub review: BmadHelpReviewProjection,
    pub approval: BmadHelpApprovedProjection,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadHelpApprovedLifecycleProjection {
    review: BmadHelpReviewProjection,
    approval: BmadHelpApprovedProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadHelpCancellationInput {
    pub manifest_hash: Sha256Digest,
    pub decision_id: ContractId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadHelpCancelledProjection {
    manifest_hash: Sha256Digest,
    decision_id: ContractId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadHelpTerminalReasonProjection {
    Cancelled,
    ConsentExpired,
    ConsentConsumed,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadHelpTerminalInput {
    pub workspace_id: ContractId,
    pub reason: BmadHelpTerminalReasonProjection,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadHelpTerminalProjection {
    workspace_id: ContractId,
    reason: BmadHelpTerminalReasonProjection,
    resumable: bool,
    send_eligible: bool,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum BmadHelpModelAccessProjectionError {
    #[error("the model-access projection is unavailable")]
    Unavailable,
}

/// Builds the closed authentication-status projection for the current host
/// composition. D2-D never advertises an interactive sign-in capability.
///
/// # Errors
///
/// Returns [`BmadHelpModelAccessProjectionError::Unavailable`] for inconsistent
/// build-mode claims, invalid epochs, or unsafe destination disclosure.
pub fn project_model_auth_status(
    input: ModelAuthStatusInput,
) -> Result<ModelAuthStatusProjection, BmadHelpModelAccessProjectionError> {
    let mode_is_consistent = matches!(
        (input.status, input.mode, input.development_only),
        (
            ModelAuthStatusKindProjection::Unavailable,
            ModelAuthModeProjection::Offline,
            false
        ) | (
            ModelAuthStatusKindProjection::DevelopmentReady,
            ModelAuthModeProjection::DeterministicDevelopment,
            true
        )
    );
    if !mode_is_consistent
        || !valid_safe_integer(input.auth_epoch)
        || !valid_display_text(&input.destination_label, MAX_DESTINATION_BYTES)
    {
        return Err(BmadHelpModelAccessProjectionError::Unavailable);
    }
    Ok(ModelAuthStatusProjection {
        status: input.status,
        mode: input.mode,
        auth_epoch: input.auth_epoch,
        development_only: input.development_only,
        destination_label: input.destination_label,
        sign_in_available: false,
        sign_out_available: true,
    })
}

/// Closes the larger egress review down to exact inert bytes and renderer-safe
/// disclosure. Provider/model/deployment/schema/content hashes and authority
/// objects are intentionally unrepresentable in the output type.
///
/// # Errors
///
/// Returns [`BmadHelpModelAccessProjectionError::Unavailable`] for invalid
/// bounds, count drift, unsafe disclosure, duplicate labels, or oversized JSON.
#[expect(
    clippy::too_many_lines,
    reason = "the exact context projection keeps all bounded item and disclosure validation visible at the IPC boundary"
)]
pub fn project_bmad_help_review(
    input: BmadHelpReviewInput,
) -> Result<BmadHelpReviewProjection, BmadHelpModelAccessProjectionError> {
    if !valid_safe_integer(input.workspace_grant_epoch)
        || !valid_safe_integer(input.expires_at.0)
        || !valid_display_text(&input.destination_label, MAX_DESTINATION_BYTES)
        || !valid_display_text(&input.consent_disclosure, MAX_DISCLOSURE_BYTES)
        || !valid_identifier(&input.purpose, MAX_PURPOSE_BYTES)
        || !valid_region(&input.region)
        || !valid_display_text(&input.redaction_limitation, MAX_DISCLOSURE_BYTES)
        || input.items.is_empty()
        || input.items.len() > MAX_REVIEW_ITEMS
        || input.exclusions.len() > MAX_EXCLUSIONS
        || input.secret_findings.len() > MAX_SECRET_FINDINGS
        || input.total_outbound_bytes == 0
        || input.total_outbound_bytes > MAX_OUTBOUND_CONTEXT_BYTES
        || !valid_safe_integer(input.total_token_estimate)
    {
        return Err(BmadHelpModelAccessProjectionError::Unavailable);
    }

    let mut labels = HashSet::new();
    let mut total_bytes = 0_u64;
    let mut total_tokens = 0_u64;
    let mut items = Vec::with_capacity(input.items.len());
    for item in input.items {
        if !labels.insert(item.relative_label.case_folded())
            || !valid_identifier(&item.semantic_role, MAX_SEMANTIC_ROLE_BYTES)
            || item
                .language
                .as_deref()
                .is_some_and(|language| !valid_identifier(language, MAX_LANGUAGE_BYTES))
            || item.outbound_byte_count == 0
            || item.outbound_byte_count != item.outbound_content.len() as u64
            || !valid_safe_integer(item.token_estimate)
            || !valid_outbound_content(&item.outbound_content)
            || item.redactions.len() > MAX_REDACTIONS_PER_ITEM
        {
            return Err(BmadHelpModelAccessProjectionError::Unavailable);
        }
        let mut redactions = Vec::with_capacity(item.redactions.len());
        for redaction in item.redactions {
            if redaction.occurrence_count == 0 || !valid_identifier(&redaction.kind, MAX_KIND_BYTES)
            {
                return Err(BmadHelpModelAccessProjectionError::Unavailable);
            }
            redactions.push(BmadHelpReviewRedactionProjection {
                kind: redaction.kind,
                occurrence_count: redaction.occurrence_count,
            });
        }
        total_bytes = total_bytes
            .checked_add(item.outbound_byte_count)
            .ok_or(BmadHelpModelAccessProjectionError::Unavailable)?;
        total_tokens = total_tokens
            .checked_add(item.token_estimate)
            .ok_or(BmadHelpModelAccessProjectionError::Unavailable)?;
        items.push(BmadHelpReviewItemProjection {
            relative_label: item.relative_label,
            semantic_role: item.semantic_role,
            language: item.language,
            outbound_byte_count: item.outbound_byte_count,
            token_estimate: item.token_estimate,
            classification: item.classification,
            redactions,
            outbound_content: item.outbound_content,
        });
    }
    if total_bytes != input.total_outbound_bytes || total_tokens != input.total_token_estimate {
        return Err(BmadHelpModelAccessProjectionError::Unavailable);
    }

    let mut exclusions = Vec::with_capacity(input.exclusions.len());
    for exclusion in input.exclusions {
        if !valid_display_text(&exclusion.reason, MAX_REASON_BYTES) {
            return Err(BmadHelpModelAccessProjectionError::Unavailable);
        }
        exclusions.push(BmadHelpReviewExclusionProjection {
            relative_label: exclusion.relative_label,
            reason: exclusion.reason,
        });
    }

    let mut secret_findings = Vec::with_capacity(input.secret_findings.len());
    for finding in input.secret_findings {
        if !labels.contains(&finding.relative_label.case_folded())
            || finding.occurrence_count == 0
            || !valid_identifier(&finding.kind, MAX_KIND_BYTES)
        {
            return Err(BmadHelpModelAccessProjectionError::Unavailable);
        }
        secret_findings.push(BmadHelpSecretFindingProjection {
            relative_label: finding.relative_label,
            kind: finding.kind,
            occurrence_count: finding.occurrence_count,
        });
    }

    let projection = BmadHelpReviewProjection {
        workspace_id: input.workspace_id,
        workspace_grant_epoch: input.workspace_grant_epoch,
        run_id: input.run_id,
        session_id: input.session_id,
        destination_label: input.destination_label,
        development_only: input.development_only,
        consent_disclosure: input.consent_disclosure,
        manifest_hash: input.manifest_hash,
        purpose: input.purpose,
        region: input.region,
        retention_mode: input.retention_mode,
        expires_at: input.expires_at,
        items,
        exclusions,
        secret_findings,
        total_outbound_bytes: input.total_outbound_bytes,
        total_token_estimate: input.total_token_estimate,
        redaction_limitation: input.redaction_limitation,
    };
    let encoded = serde_json::to_vec(&projection)
        .map_err(|_| BmadHelpModelAccessProjectionError::Unavailable)?;
    if encoded.len() > MAX_BMAD_HELP_REVIEW_PROJECTION_BYTES {
        return Err(BmadHelpModelAccessProjectionError::Unavailable);
    }
    Ok(projection)
}

/// Projects the exact displayed manifest decision with a fixed send-eligible
/// outcome. No invocation or model authority is represented.
///
/// # Errors
///
/// Returns [`BmadHelpModelAccessProjectionError::Unavailable`] for an invalid
/// expiry timestamp.
pub fn project_bmad_help_approved(
    input: BmadHelpApprovalInput,
) -> Result<BmadHelpApprovedProjection, BmadHelpModelAccessProjectionError> {
    if !valid_safe_integer(input.expires_at.0) {
        return Err(BmadHelpModelAccessProjectionError::Unavailable);
    }
    Ok(BmadHelpApprovedProjection {
        manifest_hash: input.manifest_hash,
        decision_id: input.decision_id,
        expires_at: input.expires_at,
        send_eligible: true,
    })
}

#[must_use]
pub fn project_bmad_help_approved_lifecycle(
    input: BmadHelpApprovedLifecycleInput,
) -> BmadHelpApprovedLifecycleProjection {
    BmadHelpApprovedLifecycleProjection {
        review: input.review,
        approval: input.approval,
    }
}

#[must_use]
pub fn project_bmad_help_cancelled(
    input: BmadHelpCancellationInput,
) -> BmadHelpCancelledProjection {
    BmadHelpCancelledProjection {
        manifest_hash: input.manifest_hash,
        decision_id: input.decision_id,
    }
}

#[must_use]
pub fn project_bmad_help_terminal(input: BmadHelpTerminalInput) -> BmadHelpTerminalProjection {
    BmadHelpTerminalProjection {
        workspace_id: input.workspace_id,
        reason: input.reason,
        resumable: false,
        send_eligible: false,
    }
}

const fn valid_safe_integer(value: u64) -> bool {
    value > 0 && value <= MAX_SAFE_JSON_INTEGER
}

fn valid_identifier(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn valid_region(value: &str) -> bool {
    (3..=MAX_REGION_BYTES).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn valid_display_text(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= max_bytes
        && !contains_unsafe_character(value, false)
        && !contains_absolute_path(value)
}

fn valid_outbound_content(value: &str) -> bool {
    !value.is_empty()
        && value.len() as u64 <= MAX_OUTBOUND_CONTEXT_BYTES
        && !contains_unsafe_character(value, true)
        && !contains_absolute_path(value)
}

fn contains_unsafe_character(value: &str, allow_layout_controls: bool) -> bool {
    value.chars().any(|character| {
        let denied_control = if allow_layout_controls {
            character.is_control() && !matches!(character, '\n' | '\r' | '\t')
        } else {
            character.is_control()
        };
        denied_control
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
