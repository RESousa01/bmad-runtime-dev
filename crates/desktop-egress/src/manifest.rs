use std::collections::HashSet;

use desktop_runtime::{
    canonical_hash, sha256_bytes, ContractId, RelativeWorkspacePath, Sha256Digest, UnixMillis,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const MANIFEST_SCHEMA: &str = "sapphirus.context-egress-manifest.v1";
const MAX_MANIFEST_LIFETIME_MS: u64 = 15 * 60 * 1_000;
const HARD_MAX_CONTEXT_ITEMS: u32 = 128;
const HARD_MAX_CONTEXT_BYTES: u64 = 4 * 1024 * 1024;
const HARD_MAX_TOKEN_ESTIMATE: u64 = 1_000_000;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextClassification {
    Public,
    Internal,
    Confidential,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RetentionMode {
    TransientNoStore,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EgressLimits {
    pub maximum_context_items: u32,
    pub maximum_context_bytes: u64,
    pub maximum_token_estimate: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedactionRecord {
    pub kind: String,
    pub occurrence_count: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretFinding {
    pub client_item_id: ContractId,
    pub kind: String,
    pub occurrence_count: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextExclusion {
    pub relative_label: RelativeWorkspacePath,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreparedContextItemDraft {
    pub client_item_id: ContractId,
    pub relative_label: RelativeWorkspacePath,
    pub semantic_role: String,
    pub language: Option<String>,
    pub original_content_hash: Sha256Digest,
    pub outbound_content_hash: Sha256Digest,
    pub original_byte_count: u64,
    pub outbound_byte_count: u64,
    pub token_estimate: u64,
    pub classification: ContextClassification,
    pub redactions: Vec<RedactionRecord>,
    pub outbound_content: String,
}

/// Context prepared by the crate-owned scanner. Its private attestation makes
/// original-source metadata immutable to safe downstream code.
///
/// ```compile_fail
/// # use desktop_egress::PreparedContextItem;
/// fn forge(item: PreparedContextItem) -> PreparedContextItem {
///     PreparedContextItem { original_byte_count: 0, ..item }
/// }
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedContextItem {
    pub client_item_id: ContractId,
    pub relative_label: RelativeWorkspacePath,
    pub semantic_role: String,
    pub language: Option<String>,
    pub original_content_hash: Sha256Digest,
    pub outbound_content_hash: Sha256Digest,
    pub original_byte_count: u64,
    pub outbound_byte_count: u64,
    pub token_estimate: u64,
    pub classification: ContextClassification,
    pub redactions: Vec<RedactionRecord>,
    pub outbound_content: String,
    preparation_attestation: Sha256Digest,
}

impl PreparedContextItem {
    pub(crate) fn seal(draft: PreparedContextItemDraft) -> Result<Self, EgressError> {
        let preparation_attestation = canonical_hash("prepared-context-item", 1, &draft)
            .map_err(|_| EgressError::CanonicalHash)?;
        Ok(Self {
            client_item_id: draft.client_item_id,
            relative_label: draft.relative_label,
            semantic_role: draft.semantic_role,
            language: draft.language,
            original_content_hash: draft.original_content_hash,
            outbound_content_hash: draft.outbound_content_hash,
            original_byte_count: draft.original_byte_count,
            outbound_byte_count: draft.outbound_byte_count,
            token_estimate: draft.token_estimate,
            classification: draft.classification,
            redactions: draft.redactions,
            outbound_content: draft.outbound_content,
            preparation_attestation,
        })
    }

    fn attestation_draft(&self) -> PreparedContextItemDraft {
        PreparedContextItemDraft {
            client_item_id: self.client_item_id.clone(),
            relative_label: self.relative_label.clone(),
            semantic_role: self.semantic_role.clone(),
            language: self.language.clone(),
            original_content_hash: self.original_content_hash,
            outbound_content_hash: self.outbound_content_hash,
            original_byte_count: self.original_byte_count,
            outbound_byte_count: self.outbound_byte_count,
            token_estimate: self.token_estimate,
            classification: self.classification,
            redactions: self.redactions.clone(),
            outbound_content: self.outbound_content.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextEgressManifestDraft {
    pub schema_version: String,
    pub tenant_ref: ContractId,
    pub project_ref: ContractId,
    pub run_ref: ContractId,
    pub purpose: String,
    pub model_role: String,
    pub canonical_output_schema_id: ContractId,
    pub canonical_output_schema_hash: Sha256Digest,
    pub provider_profile_hash: Sha256Digest,
    pub model_profile_hash: Sha256Digest,
    pub deployment_hash: Sha256Digest,
    pub policy_hash: Sha256Digest,
    pub region: String,
    pub retention_mode: RetentionMode,
    pub created_at: UnixMillis,
    pub expires_at: UnixMillis,
    pub limits: EgressLimits,
    pub items: Vec<PreparedContextItem>,
    pub exclusions: Vec<ContextExclusion>,
    pub secret_findings: Vec<SecretFinding>,
    pub total_outbound_bytes: u64,
    pub total_token_estimate: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextEgressManifest {
    #[serde(flatten)]
    pub draft: ContextEgressManifestDraft,
    pub manifest_hash: Sha256Digest,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextReviewItem {
    pub client_item_id: ContractId,
    pub relative_label: RelativeWorkspacePath,
    pub semantic_role: String,
    pub language: Option<String>,
    pub outbound_content_hash: Sha256Digest,
    pub outbound_byte_count: u64,
    pub token_estimate: u64,
    pub classification: ContextClassification,
    pub redactions: Vec<RedactionRecord>,
    pub outbound_content: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextReviewProjection {
    pub manifest_hash: Sha256Digest,
    pub purpose: String,
    pub model_role: String,
    pub provider_profile_hash: Sha256Digest,
    pub model_profile_hash: Sha256Digest,
    pub deployment_hash: Sha256Digest,
    pub region: String,
    pub retention_mode: RetentionMode,
    pub expires_at: UnixMillis,
    pub items: Vec<ContextReviewItem>,
    pub exclusions: Vec<ContextExclusion>,
    pub secret_findings: Vec<SecretFinding>,
    pub total_outbound_bytes: u64,
    pub total_token_estimate: u64,
    pub redaction_limitation: String,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum EgressError {
    #[error("context bytes, counts, or hashes drifted from the manifest")]
    ContextDrift,
    #[error("the context manifest integrity hash is invalid")]
    ManifestIntegrity,
    #[error("the context manifest lifetime is invalid")]
    InvalidLifetime,
    #[error("the context manifest shape is invalid")]
    InvalidManifest,
    #[error("a context item identifier is duplicated")]
    DuplicateContextItem,
    #[error("the context exceeds its approved budget")]
    ContextBudgetExceeded,
    #[error("the context label is denied by egress policy")]
    DeniedContextLabel,
    #[error("the model invocation binding is invalid")]
    InvalidInvocationBinding,
    #[error("the consent decision integrity hash is invalid")]
    DecisionIntegrity,
    #[error("the consent decision does not match the exact invocation binding")]
    DecisionBindingMismatch,
    #[error("the consent decision identifier already exists")]
    DecisionAlreadyExists,
    #[error("the consent decision is unknown")]
    DecisionUnknown,
    #[error("the consent decision has expired")]
    DecisionExpired,
    #[error("the consent decision was already consumed")]
    DecisionAlreadyConsumed,
    #[error("the consent decision was cancelled")]
    DecisionCancelled,
    #[error("canonical hashing failed")]
    CanonicalHash,
}

impl ContextEgressManifestDraft {
    /// Validates and seals the exact outbound context.
    ///
    /// # Errors
    ///
    /// Returns [`EgressError`] when manifest shape, lifetime, budgets, item
    /// identity, or exact outbound bytes are invalid.
    pub fn seal(self) -> Result<ContextEgressManifest, EgressError> {
        validate_manifest(&self)?;
        let manifest_hash = canonical_hash("context-egress-manifest", 1, &self)
            .map_err(|_| EgressError::CanonicalHash)?;
        Ok(ContextEgressManifest {
            draft: self,
            manifest_hash,
        })
    }
}

impl ContextEgressManifest {
    /// Revalidates manifest semantics and its canonical integrity hash.
    ///
    /// # Errors
    ///
    /// Returns [`EgressError`] when semantics, exact outbound bytes, or the
    /// integrity hash no longer match.
    pub fn verify(&self) -> Result<(), EgressError> {
        validate_manifest(&self.draft)?;
        let actual = canonical_hash("context-egress-manifest", 1, &self.draft)
            .map_err(|_| EgressError::CanonicalHash)?;
        if actual != self.manifest_hash {
            return Err(EgressError::ManifestIntegrity);
        }
        Ok(())
    }

    #[must_use]
    pub fn review_projection(&self) -> ContextReviewProjection {
        ContextReviewProjection {
            manifest_hash: self.manifest_hash,
            purpose: self.draft.purpose.clone(),
            model_role: self.draft.model_role.clone(),
            provider_profile_hash: self.draft.provider_profile_hash,
            model_profile_hash: self.draft.model_profile_hash,
            deployment_hash: self.draft.deployment_hash,
            region: self.draft.region.clone(),
            retention_mode: self.draft.retention_mode,
            expires_at: self.draft.expires_at,
            items: self
                .draft
                .items
                .iter()
                .map(|item| ContextReviewItem {
                    client_item_id: item.client_item_id.clone(),
                    relative_label: item.relative_label.clone(),
                    semantic_role: item.semantic_role.clone(),
                    language: item.language.clone(),
                    outbound_content_hash: item.outbound_content_hash,
                    outbound_byte_count: item.outbound_byte_count,
                    token_estimate: item.token_estimate,
                    classification: item.classification,
                    redactions: item.redactions.clone(),
                    outbound_content: item.outbound_content.clone(),
                })
                .collect(),
            exclusions: self.draft.exclusions.clone(),
            secret_findings: self.draft.secret_findings.clone(),
            total_outbound_bytes: self.draft.total_outbound_bytes,
            total_token_estimate: self.draft.total_token_estimate,
            redaction_limitation:
                "Redaction reduces risk but cannot prove that every secret was detected.".to_owned(),
        }
    }
}

fn validate_manifest(draft: &ContextEgressManifestDraft) -> Result<(), EgressError> {
    if draft.schema_version != MANIFEST_SCHEMA
        || !is_safe_label(&draft.purpose, 128)
        || !is_safe_label(&draft.model_role, 128)
        || !is_safe_region(&draft.region)
        || draft.items.is_empty()
    {
        return Err(EgressError::InvalidManifest);
    }

    let lifetime = draft
        .expires_at
        .0
        .checked_sub(draft.created_at.0)
        .ok_or(EgressError::InvalidLifetime)?;
    if lifetime == 0 || lifetime > MAX_MANIFEST_LIFETIME_MS {
        return Err(EgressError::InvalidLifetime);
    }
    validate_limits(&draft.limits)?;
    if draft.items.len() > draft.limits.maximum_context_items as usize {
        return Err(EgressError::ContextBudgetExceeded);
    }

    let mut identifiers = HashSet::with_capacity(draft.items.len());
    let mut total_bytes = 0_u64;
    let mut total_tokens = 0_u64;
    for item in &draft.items {
        if !identifiers.insert(&item.client_item_id) {
            return Err(EgressError::DuplicateContextItem);
        }
        validate_item(item)?;
        total_bytes = total_bytes
            .checked_add(item.outbound_byte_count)
            .ok_or(EgressError::ContextBudgetExceeded)?;
        total_tokens = total_tokens
            .checked_add(item.token_estimate)
            .ok_or(EgressError::ContextBudgetExceeded)?;
    }

    if total_bytes != draft.total_outbound_bytes || total_tokens != draft.total_token_estimate {
        return Err(EgressError::ContextDrift);
    }
    if total_bytes > draft.limits.maximum_context_bytes
        || total_tokens > draft.limits.maximum_token_estimate
    {
        return Err(EgressError::ContextBudgetExceeded);
    }
    if draft
        .exclusions
        .iter()
        .any(|entry| !is_safe_label(&entry.reason, 128))
        || draft.secret_findings.iter().any(|finding| {
            !identifiers.contains(&finding.client_item_id)
                || !is_safe_label(&finding.kind, 64)
                || finding.occurrence_count == 0
        })
    {
        return Err(EgressError::InvalidManifest);
    }
    Ok(())
}

fn validate_limits(limits: &EgressLimits) -> Result<(), EgressError> {
    if limits.maximum_context_items == 0
        || limits.maximum_context_items > HARD_MAX_CONTEXT_ITEMS
        || limits.maximum_context_bytes == 0
        || limits.maximum_context_bytes > HARD_MAX_CONTEXT_BYTES
        || limits.maximum_token_estimate == 0
        || limits.maximum_token_estimate > HARD_MAX_TOKEN_ESTIMATE
    {
        return Err(EgressError::InvalidManifest);
    }
    Ok(())
}

fn validate_item(item: &PreparedContextItem) -> Result<(), EgressError> {
    if !is_safe_label(&item.semantic_role, 128)
        || item
            .language
            .as_deref()
            .is_some_and(|language| !is_safe_label(language, 64))
        || item
            .redactions
            .iter()
            .any(|redaction| !is_safe_label(&redaction.kind, 64) || redaction.occurrence_count == 0)
    {
        return Err(EgressError::InvalidManifest);
    }
    let content_bytes = u64::try_from(item.outbound_content.len())
        .map_err(|_| EgressError::ContextBudgetExceeded)?;
    if content_bytes != item.outbound_byte_count
        || sha256_bytes(item.outbound_content.as_bytes()) != item.outbound_content_hash
    {
        return Err(EgressError::ContextDrift);
    }
    let actual_attestation = canonical_hash("prepared-context-item", 1, &item.attestation_draft())
        .map_err(|_| EgressError::CanonicalHash)?;
    if actual_attestation != item.preparation_attestation {
        return Err(EgressError::ContextDrift);
    }
    Ok(())
}

fn is_safe_label(value: &str, maximum_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum_bytes
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn is_safe_region(value: &str) -> bool {
    (3..=64).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_lowercase())
}
