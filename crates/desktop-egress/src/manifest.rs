use std::collections::HashSet;

use desktop_runtime::{
    canonical_hash, sha256_bytes, CanonicalHashError, ContractId, RelativeWorkspacePath,
    Sha256Digest, UnixMillis,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextClassification {
    Workspace,
    UserProvided,
    Generated,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RetentionMode {
    TransientNoStore,
    Persistent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RedactionRecord {
    pub kind: String,
    pub count: u64,
}

pub type SecretFinding = RedactionRecord;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PreparedContextItem {
    pub item_id: ContractId,
    pub relative_label: RelativeWorkspacePath,
    pub classification: ContextClassification,
    pub original_content: String,
    pub outbound_content: String,
    pub original_byte_count: u64,
    pub outbound_byte_count: u64,
    pub original_content_hash: Sha256Digest,
    pub outbound_content_hash: Sha256Digest,
    pub token_count: u64,
    pub redactions: Vec<RedactionRecord>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EgressLimits {
    pub maximum_items: u64,
    pub maximum_context_bytes: u64,
    pub maximum_tokens: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContextEgressManifestDraft {
    pub schema_version: u32,
    pub created_at: UnixMillis,
    pub expires_at: UnixMillis,
    pub items: Vec<PreparedContextItem>,
    pub total_byte_count: u64,
    pub total_token_count: u64,
    pub limits: EgressLimits,
    pub retention_mode: RetentionMode,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContextEgressManifest {
    pub draft: ContextEgressManifestDraft,
    pub manifest_hash: Sha256Digest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContextReviewProjection {
    pub manifest_hash: Sha256Digest,
    pub items: Vec<ContextReviewItem>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContextReviewItem {
    pub item_id: ContractId,
    pub relative_label: RelativeWorkspacePath,
    pub outbound_content: String,
}

#[derive(Debug, Error)]
pub enum EgressError {
    #[error("the manifest schema version is unsupported")]
    UnsupportedSchemaVersion,
    #[error("the manifest must contain at least one item")]
    EmptyItems,
    #[error("manifest item identifiers must be unique")]
    DuplicateItemId,
    #[error("manifest creation must precede expiry")]
    InvalidTimeRange,
    #[error("context bytes, counts, or hashes drifted")]
    ContextDrift,
    #[error("manifest limits must all be positive")]
    InvalidLimits,
    #[error("only transient no-store retention is supported")]
    UnsupportedRetention,
    #[error("sealed manifest integrity check failed")]
    ManifestIntegrity,
    #[error("canonical manifest hashing failed: {0}")]
    Hash(#[from] CanonicalHashError),
}

impl PartialEq for EgressError {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (
                Self::UnsupportedSchemaVersion,
                Self::UnsupportedSchemaVersion
            ) | (Self::EmptyItems, Self::EmptyItems)
                | (Self::DuplicateItemId, Self::DuplicateItemId)
                | (Self::InvalidTimeRange, Self::InvalidTimeRange)
                | (Self::ContextDrift, Self::ContextDrift)
                | (Self::InvalidLimits, Self::InvalidLimits)
                | (Self::UnsupportedRetention, Self::UnsupportedRetention)
                | (Self::ManifestIntegrity, Self::ManifestIntegrity)
        )
    }
}

impl Eq for EgressError {}

impl ContextEgressManifestDraft {
    /// Validates and seals the complete outbound context draft.
    ///
    /// # Errors
    ///
    /// Returns an [`EgressError`] when the draft violates the manifest policy
    /// or cannot be canonically hashed.
    pub fn seal(self) -> Result<ContextEgressManifest, EgressError> {
        validate_manifest(&self)?;
        let manifest_hash = canonical_hash("context-egress-manifest", SCHEMA_VERSION, &self)?;
        Ok(ContextEgressManifest {
            draft: self,
            manifest_hash,
        })
    }
}

impl ContextEgressManifest {
    /// Revalidates the draft and verifies its integrity hash.
    ///
    /// # Errors
    ///
    /// Returns an [`EgressError`] when the draft is invalid or its sealed hash
    /// no longer matches.
    pub fn verify(&self) -> Result<(), EgressError> {
        validate_manifest(&self.draft)?;
        let actual = canonical_hash("context-egress-manifest", SCHEMA_VERSION, &self.draft)?;
        if actual != self.manifest_hash {
            return Err(EgressError::ManifestIntegrity);
        }
        Ok(())
    }

    #[must_use]
    pub fn review_projection(&self) -> ContextReviewProjection {
        ContextReviewProjection {
            manifest_hash: self.manifest_hash,
            items: self
                .draft
                .items
                .iter()
                .map(|item| ContextReviewItem {
                    item_id: item.item_id.clone(),
                    relative_label: item.relative_label.clone(),
                    outbound_content: item.outbound_content.clone(),
                })
                .collect(),
        }
    }
}

fn validate_manifest(draft: &ContextEgressManifestDraft) -> Result<(), EgressError> {
    if draft.schema_version != SCHEMA_VERSION {
        return Err(EgressError::UnsupportedSchemaVersion);
    }
    if draft.items.is_empty() {
        return Err(EgressError::EmptyItems);
    }
    if draft.created_at >= draft.expires_at {
        return Err(EgressError::InvalidTimeRange);
    }
    if draft.limits.maximum_items == 0
        || draft.limits.maximum_context_bytes == 0
        || draft.limits.maximum_tokens == 0
    {
        return Err(EgressError::InvalidLimits);
    }
    if draft.retention_mode != RetentionMode::TransientNoStore {
        return Err(EgressError::UnsupportedRetention);
    }
    if draft.items.len() as u64 > draft.limits.maximum_items {
        return Err(EgressError::InvalidLimits);
    }

    let mut item_ids = HashSet::with_capacity(draft.items.len());
    let mut total_bytes = 0_u64;
    let mut total_tokens = 0_u64;
    for item in &draft.items {
        if !item_ids.insert(item.item_id.clone()) {
            return Err(EgressError::DuplicateItemId);
        }
        if item.original_byte_count != item.original_content.len() as u64
            || item.outbound_byte_count != item.outbound_content.len() as u64
            || item.original_content_hash != sha256_bytes(item.original_content.as_bytes())
            || item.outbound_content_hash != sha256_bytes(item.outbound_content.as_bytes())
        {
            return Err(EgressError::ContextDrift);
        }
        total_bytes = total_bytes
            .checked_add(item.outbound_byte_count)
            .ok_or(EgressError::ContextDrift)?;
        total_tokens = total_tokens
            .checked_add(item.token_count)
            .ok_or(EgressError::ContextDrift)?;
    }
    if draft.total_byte_count != total_bytes
        || draft.total_token_count != total_tokens
        || total_bytes > draft.limits.maximum_context_bytes
        || total_tokens > draft.limits.maximum_tokens
    {
        return Err(EgressError::ContextDrift);
    }
    Ok(())
}
