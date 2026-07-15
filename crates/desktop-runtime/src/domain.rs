use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    canonical_hash, canonical_hash_without_field, sha256_bytes, CanonicalHashError, ContractId,
    RelativeWorkspacePath, Sha256Digest, UnixMillis,
};

pub const HARD_MAX_CHANGED_FILES: u32 = 20;
pub const HARD_MAX_CHANGED_BYTES: u64 = 1024 * 1024;
const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

const CANDIDATE_SCHEMA: &str = "sapphirus.candidate-action.v1";
const PATCH_SCHEMA: &str = "sapphirus.patch-set.v1";
const APPROVAL_SCHEMA: &str = "sapphirus.approval-decision.v1";
const SPEC_SCHEMA: &str = "sapphirus.approved-execution-spec.v1";
const CONSUMPTION_SCHEMA: &str = "sapphirus.spec-consumption.v1";

#[derive(Debug, Error)]
pub enum DomainValidationError {
    #[error("contract schema version is not supported")]
    UnsupportedSchema,
    #[error("the object does not use windows-local delivery")]
    DeliveryModelMismatch,
    #[error("the object contains an invalid time range")]
    InvalidTimeRange,
    #[error("the patch contains no file operations")]
    EmptyPatch,
    #[error("the patch exceeds the file limit")]
    FileLimitExceeded,
    #[error("the patch exceeds the byte limit")]
    ByteLimitExceeded,
    #[error("a patch path appears more than once or has a Windows case alias")]
    DuplicatePath,
    #[error("the patch contains a binary marker or an incorrect postimage hash")]
    InvalidPostimage,
    #[error("the candidate collections are not in canonical order")]
    NonCanonicalOrder,
    #[error("candidate preimages and declared writes do not match")]
    PreimageMismatch,
    #[error("the candidate permits a process or network effect")]
    ForbiddenEffect,
    #[error("the candidate does not provide file-tracked rollback")]
    InvalidRollbackClass,
    #[error("the candidate limits exceed the desktop hard limit")]
    InvalidLimits,
    #[error("the candidate contains an invalid content-addressed reference")]
    InvalidContentReference,
    #[error("a security-sensitive contract hash does not match its content")]
    HashMismatch,
    #[error("approval does not bind the candidate")]
    ApprovalMismatch,
    #[error("spec consumption does not bind the immutable spec")]
    ConsumptionMismatch,
    #[error(transparent)]
    CanonicalHash(#[from] CanonicalHashError),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryModel {
    WindowsLocal,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuthorityRef {
    pub authority_kind: String,
    pub authority_id: ContractId,
    pub installation_id: ContractId,
    pub local_store_id: ContractId,
    pub authority_epoch: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputKind {
    ExternalResource,
    Package,
    PathPreimage,
    Policy,
    Toolchain,
    WorkspaceManifest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MutableInputBinding {
    pub input_kind: InputKind,
    pub input_id: String,
    pub content_hash: Sha256Digest,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclaredWriteOperation {
    Create,
    Delete,
    Modify,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeclaredWrite {
    pub path_pattern: RelativeWorkspacePath,
    pub operation: DeclaredWriteOperation,
    pub preimage_hash: Option<Sha256Digest>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionLimits {
    pub timeout_seconds: u32,
    pub max_output_bytes: u64,
    pub max_changed_files: u32,
    pub max_changed_bytes: u64,
    pub max_process_count: u32,
}

impl ExecutionLimits {
    #[must_use]
    pub const fn governed_patch_defaults() -> Self {
        Self {
            timeout_seconds: 0,
            max_output_bytes: 0,
            max_changed_files: HARD_MAX_CHANGED_FILES,
            max_changed_bytes: HARD_MAX_CHANGED_BYTES,
            max_process_count: 0,
        }
    }

    fn validate_patch(self) -> Result<(), DomainValidationError> {
        if self.timeout_seconds != 0
            || self.max_output_bytes != 0
            || self.max_process_count != 0
            || self.max_changed_files == 0
            || self.max_changed_files > HARD_MAX_CHANGED_FILES
            || self.max_changed_bytes == 0
            || self.max_changed_bytes > HARD_MAX_CHANGED_BYTES
        {
            return Err(DomainValidationError::InvalidLimits);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RollbackClass {
    FileTracked,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceTarget {
    pub target_kind: String,
    pub workspace_capability_id: ContractId,
    pub grant_epoch: u64,
    pub root_identity_hash: Sha256Digest,
    pub filesystem_capability_hash: Sha256Digest,
    pub base_checkpoint_id: ContractId,
    pub workspace_manifest_hash: Sha256Digest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativePatchEngineAudience {
    pub audience_kind: String,
    pub installation_id: ContractId,
    pub host_build_id: String,
    pub host_binary_sha256: Sha256Digest,
    pub patch_engine_profile_hash: Sha256Digest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalPathPreimage {
    pub relative_path: RelativeWorkspacePath,
    pub exists: bool,
    pub file_identity_hash: Option<Sha256Digest>,
    pub content_hash: Option<Sha256Digest>,
    pub metadata_hash: Option<Sha256Digest>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
// This host-domain fragment is flattened into the sealed candidate.
#[serde(rename_all = "camelCase")]
pub struct CandidateCommon {
    pub candidate_id: ContractId,
    pub project_id: ContractId,
    pub run_id: ContractId,
    pub proposal_id: ContractId,
    pub proposal_hash: Sha256Digest,
    pub authority_ref: AuthorityRef,
    pub owner_scope_ref: ContractId,
    pub policy_context_hash: Sha256Digest,
    pub mutable_inputs: Vec<MutableInputBinding>,
    pub declared_writes: Vec<DeclaredWrite>,
    pub limits: ExecutionLimits,
    pub rollback_class: RollbackClass,
    #[serde(serialize_with = "crate::ids::serialize_utc_instant")]
    pub created_at: UnixMillis,
    #[serde(serialize_with = "crate::ids::serialize_utc_instant")]
    pub expires_at: UnixMillis,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsPatchCandidateDraft {
    pub schema_version: String,
    #[serde(flatten)]
    pub common: CandidateCommon,
    pub delivery_model: DeliveryModel,
    pub action_kind: String,
    pub workspace_target: WorkspaceTarget,
    pub executor_audience: NativePatchEngineAudience,
    pub patch_ref: String,
    pub patch_hash: Sha256Digest,
    pub preimages: Vec<LocalPathPreimage>,
}

impl WindowsPatchCandidateDraft {
    /// Validates the candidate's schema, authority bindings, governed limits,
    /// content reference, and canonical input collections.
    ///
    /// # Errors
    ///
    /// Returns [`DomainValidationError`] when any candidate invariant is
    /// violated or a bound collection is not canonical.
    pub fn validate(&self) -> Result<(), DomainValidationError> {
        if self.schema_version != CANDIDATE_SCHEMA || self.action_kind != "patch_apply" {
            return Err(DomainValidationError::UnsupportedSchema);
        }
        if self.delivery_model != DeliveryModel::WindowsLocal {
            return Err(DomainValidationError::DeliveryModelMismatch);
        }
        if self.common.expires_at <= self.common.created_at {
            return Err(DomainValidationError::InvalidTimeRange);
        }
        if self.common.authority_ref.authority_kind != "desktop_local_store"
            || self.common.authority_ref.authority_epoch == 0
            || self.common.authority_ref.authority_epoch > MAX_SAFE_JSON_INTEGER
            || self.workspace_target.target_kind != "local_folder_capability"
            || self.workspace_target.grant_epoch == 0
            || self.workspace_target.grant_epoch > MAX_SAFE_JSON_INTEGER
            || self.executor_audience.audience_kind != "native_patch_engine"
            || self.executor_audience.host_build_id.is_empty()
            || self.executor_audience.host_build_id.len() > 128
            || self.executor_audience.installation_id != self.common.authority_ref.installation_id
        {
            return Err(DomainValidationError::ForbiddenEffect);
        }
        self.common.limits.validate_patch()?;
        if self.common.rollback_class != RollbackClass::FileTracked {
            return Err(DomainValidationError::InvalidRollbackClass);
        }
        if !valid_cas_reference(&self.patch_ref, self.patch_hash) {
            return Err(DomainValidationError::InvalidContentReference);
        }
        validate_candidate_collections(
            &self.common.mutable_inputs,
            &self.common.declared_writes,
            &self.preimages,
        )
    }

    /// Validates this draft and seals it with its canonical candidate hash.
    ///
    /// # Errors
    ///
    /// Returns [`DomainValidationError`] when validation or canonical hashing
    /// fails.
    pub fn seal(self) -> Result<WindowsPatchCandidate, DomainValidationError> {
        self.validate()?;
        let candidate_hash = canonical_hash("candidate-action", 1, &self)?;
        Ok(WindowsPatchCandidate {
            draft: self,
            candidate_hash,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsPatchCandidate {
    #[serde(flatten)]
    pub draft: WindowsPatchCandidateDraft,
    pub candidate_hash: Sha256Digest,
}

impl WindowsPatchCandidate {
    /// Revalidates the sealed draft and its canonical candidate hash.
    ///
    /// # Errors
    ///
    /// Returns [`DomainValidationError`] when the draft is invalid, canonical
    /// hashing fails, or the stored hash does not match the draft.
    pub fn verify(&self) -> Result<(), DomainValidationError> {
        self.draft.validate()?;
        let actual = canonical_hash("candidate-action", 1, &self.draft)?;
        if actual != self.candidate_hash {
            return Err(DomainValidationError::HashMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PatchSet {
    pub schema_version: String,
    pub operations: Vec<PatchOperation>,
}

impl PatchSet {
    #[must_use]
    pub fn new(operations: Vec<PatchOperation>) -> Self {
        Self {
            schema_version: PATCH_SCHEMA.to_owned(),
            operations,
        }
    }

    /// Validates the patch schema, governed limits, unique paths, and
    /// postimage hashes.
    ///
    /// # Errors
    ///
    /// Returns [`DomainValidationError`] when the schema is unsupported, the
    /// patch is empty, a hard limit is exceeded, a path is aliased, or a
    /// postimage is invalid.
    pub fn validate(&self) -> Result<(), DomainValidationError> {
        if self.schema_version != PATCH_SCHEMA {
            return Err(DomainValidationError::UnsupportedSchema);
        }
        if self.operations.is_empty() {
            return Err(DomainValidationError::EmptyPatch);
        }
        if self.operations.len() > HARD_MAX_CHANGED_FILES as usize {
            return Err(DomainValidationError::FileLimitExceeded);
        }

        let mut paths = BTreeSet::new();
        let mut total_bytes = 0_u64;
        for operation in &self.operations {
            if !paths.insert(operation.relative_path().case_folded()) {
                return Err(DomainValidationError::DuplicatePath);
            }
            if let Some(content) = operation.content() {
                if content.contains('\0')
                    || Some(sha256_bytes(content.as_bytes())) != operation.postimage_hash()
                {
                    return Err(DomainValidationError::InvalidPostimage);
                }
                total_bytes = total_bytes
                    .checked_add(content.len() as u64)
                    .ok_or(DomainValidationError::ByteLimitExceeded)?;
            }
        }
        if total_bytes > HARD_MAX_CHANGED_BYTES {
            return Err(DomainValidationError::ByteLimitExceeded);
        }
        Ok(())
    }

    /// Computes the canonical hash of a valid patch set.
    ///
    /// # Errors
    ///
    /// Returns [`DomainValidationError`] when patch validation or canonical
    /// hashing fails.
    pub fn content_hash(&self) -> Result<Sha256Digest, DomainValidationError> {
        self.validate()?;
        Ok(canonical_hash("patch-set", 1, self)?)
    }

    #[must_use]
    pub fn changed_bytes(&self) -> u64 {
        self.operations
            .iter()
            .filter_map(PatchOperation::content)
            .map(|content| content.len() as u64)
            .fold(0_u64, u64::saturating_add)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum PatchOperation {
    Create {
        #[serde(rename = "relativePath")]
        relative_path: RelativeWorkspacePath,
        content: String,
        #[serde(rename = "postimageHash")]
        postimage_hash: Sha256Digest,
    },
    Replace {
        #[serde(rename = "relativePath")]
        relative_path: RelativeWorkspacePath,
        #[serde(rename = "preimageHash")]
        preimage_hash: Sha256Digest,
        content: String,
        #[serde(rename = "postimageHash")]
        postimage_hash: Sha256Digest,
    },
    Delete {
        #[serde(rename = "relativePath")]
        relative_path: RelativeWorkspacePath,
        #[serde(rename = "preimageHash")]
        preimage_hash: Sha256Digest,
    },
}

impl PatchOperation {
    #[must_use]
    pub fn create(relative_path: RelativeWorkspacePath, content: String) -> Self {
        let postimage_hash = sha256_bytes(content.as_bytes());
        Self::Create {
            relative_path,
            content,
            postimage_hash,
        }
    }

    #[must_use]
    pub fn replace(
        relative_path: RelativeWorkspacePath,
        preimage_hash: Sha256Digest,
        content: String,
    ) -> Self {
        let postimage_hash = sha256_bytes(content.as_bytes());
        Self::Replace {
            relative_path,
            preimage_hash,
            content,
            postimage_hash,
        }
    }

    #[must_use]
    pub const fn delete(relative_path: RelativeWorkspacePath, preimage_hash: Sha256Digest) -> Self {
        Self::Delete {
            relative_path,
            preimage_hash,
        }
    }

    #[must_use]
    pub const fn relative_path(&self) -> &RelativeWorkspacePath {
        match self {
            Self::Create { relative_path, .. }
            | Self::Replace { relative_path, .. }
            | Self::Delete { relative_path, .. } => relative_path,
        }
    }

    #[must_use]
    pub fn content(&self) -> Option<&str> {
        match self {
            Self::Create { content, .. } | Self::Replace { content, .. } => Some(content),
            Self::Delete { .. } => None,
        }
    }

    #[must_use]
    pub fn preimage_hash(&self) -> Option<Sha256Digest> {
        match self {
            Self::Create { .. } => None,
            Self::Replace { preimage_hash, .. } | Self::Delete { preimage_hash, .. } => {
                Some(*preimage_hash)
            }
        }
    }

    #[must_use]
    pub const fn postimage_hash(&self) -> Option<Sha256Digest> {
        match self {
            Self::Create { postimage_hash, .. } | Self::Replace { postimage_hash, .. } => {
                Some(*postimage_hash)
            }
            Self::Delete { .. } => None,
        }
    }

    #[must_use]
    pub const fn declared_operation(&self) -> DeclaredWriteOperation {
        match self {
            Self::Create { .. } => DeclaredWriteOperation::Create,
            Self::Replace { .. } => DeclaredWriteOperation::Modify,
            Self::Delete { .. } => DeclaredWriteOperation::Delete,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalOutcome {
    Approved,
    Rejected,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
// This shape is flattened into `ApprovalDecision`; see `CandidateCommon`.
#[serde(rename_all = "camelCase")]
pub struct ApprovalDecisionDraft {
    pub schema_version: String,
    pub approval_id: ContractId,
    pub candidate_id: ContractId,
    pub candidate_hash: Sha256Digest,
    pub displayed_diff_hash: Sha256Digest,
    pub outcome: ApprovalOutcome,
    #[serde(serialize_with = "crate::ids::serialize_utc_instant")]
    pub decided_at: UnixMillis,
}

impl ApprovalDecisionDraft {
    #[must_use]
    pub fn approved(
        approval_id: ContractId,
        candidate: &WindowsPatchCandidate,
        displayed_diff_hash: Sha256Digest,
        decided_at: UnixMillis,
    ) -> Self {
        Self {
            schema_version: APPROVAL_SCHEMA.to_owned(),
            approval_id,
            candidate_id: candidate.draft.common.candidate_id.clone(),
            candidate_hash: candidate.candidate_hash,
            displayed_diff_hash,
            outcome: ApprovalOutcome::Approved,
            decided_at,
        }
    }

    /// Validates the approval schema and seals the decision with its canonical
    /// hash.
    ///
    /// # Errors
    ///
    /// Returns [`DomainValidationError`] when the schema is unsupported or
    /// canonical hashing fails.
    pub fn seal(self) -> Result<ApprovalDecision, DomainValidationError> {
        if self.schema_version != APPROVAL_SCHEMA {
            return Err(DomainValidationError::UnsupportedSchema);
        }
        let approval_decision_hash = canonical_hash("approval-decision", 1, &self)?;
        Ok(ApprovalDecision {
            draft: self,
            approval_decision_hash,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalDecision {
    #[serde(flatten)]
    pub draft: ApprovalDecisionDraft,
    pub approval_decision_hash: Sha256Digest,
}

impl ApprovalDecision {
    /// Verifies this decision is an intact approval for `candidate`.
    ///
    /// # Errors
    ///
    /// Returns [`DomainValidationError`] when canonical hashing fails, the
    /// decision hash is invalid, or the decision does not approve the supplied
    /// candidate.
    pub fn verify_for(
        &self,
        candidate: &WindowsPatchCandidate,
    ) -> Result<(), DomainValidationError> {
        let actual = canonical_hash("approval-decision", 1, &self.draft)?;
        if self.draft.schema_version != APPROVAL_SCHEMA || actual != self.approval_decision_hash {
            return Err(DomainValidationError::HashMismatch);
        }
        if self.draft.outcome != ApprovalOutcome::Approved
            || self.draft.candidate_id != candidate.draft.common.candidate_id
            || self.draft.candidate_hash != candidate.candidate_hash
        {
            return Err(DomainValidationError::ApprovalMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovedExecutionSpecDraft {
    pub schema_version: String,
    pub spec_id: ContractId,
    pub delivery_model: DeliveryModel,
    pub authority_ref: AuthorityRef,
    pub owner_scope_ref: ContractId,
    pub project_id: ContractId,
    pub run_id: ContractId,
    pub proposal_id: ContractId,
    pub proposal_hash: Sha256Digest,
    pub candidate_id: ContractId,
    pub candidate_hash: Sha256Digest,
    pub approval_id: ContractId,
    pub approval_decision_hash: Sha256Digest,
    pub policy_version: String,
    pub policy_hash: Sha256Digest,
    pub workspace_target_hash: Sha256Digest,
    pub mutable_input_set_hash: Sha256Digest,
    pub executor_audience: NativePatchEngineAudience,
    #[serde(serialize_with = "crate::ids::serialize_utc_instant")]
    pub issued_at: UnixMillis,
    #[serde(serialize_with = "crate::ids::serialize_utc_instant")]
    pub expires_at: UnixMillis,
    pub single_use_nonce_hash: Sha256Digest,
}

impl ApprovedExecutionSpecDraft {
    /// Validates and seals this Windows-local execution specification.
    ///
    /// # Errors
    ///
    /// Returns [`DomainValidationError`] when the schema, delivery model, or
    /// validity interval is invalid, or when canonical hashing fails.
    pub fn seal(self) -> Result<ApprovedExecutionSpec, DomainValidationError> {
        if self.schema_version != SPEC_SCHEMA || self.delivery_model != DeliveryModel::WindowsLocal
        {
            return Err(DomainValidationError::UnsupportedSchema);
        }
        if self.expires_at <= self.issued_at {
            return Err(DomainValidationError::InvalidTimeRange);
        }
        let spec_hash = canonical_hash("approved-execution-spec", 1, &self)?;
        Ok(ApprovedExecutionSpec {
            draft: self,
            spec_hash,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovedExecutionSpec {
    #[serde(flatten)]
    pub draft: ApprovedExecutionSpecDraft,
    pub spec_hash: Sha256Digest,
}

impl ApprovedExecutionSpec {
    /// Verifies this execution specification's invariants and canonical hash.
    ///
    /// # Errors
    ///
    /// Returns [`DomainValidationError`] when the sealed draft is unsupported,
    /// its validity interval is invalid, canonical hashing fails, or its hash
    /// does not match.
    pub fn verify(&self) -> Result<(), DomainValidationError> {
        if self.draft.schema_version != SPEC_SCHEMA
            || self.draft.delivery_model != DeliveryModel::WindowsLocal
            || self.draft.expires_at <= self.draft.issued_at
        {
            return Err(DomainValidationError::UnsupportedSchema);
        }
        let actual = canonical_hash("approved-execution-spec", 1, &self.draft)?;
        if actual != self.spec_hash {
            return Err(DomainValidationError::HashMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
// This host-domain fragment is flattened into `SpecConsumptionRecord`.
#[serde(rename_all = "camelCase")]
pub struct SpecConsumptionRecordDraft {
    pub schema_version: String,
    pub consumption_id: ContractId,
    pub delivery_model: DeliveryModel,
    pub authority_ref: AuthorityRef,
    pub spec_id: ContractId,
    pub spec_hash: Sha256Digest,
    pub candidate_hash: Sha256Digest,
    pub single_use_nonce_hash: Sha256Digest,
    pub executor_audience_hash: Sha256Digest,
    pub execution_id: ContractId,
    pub attempt_number: u32,
    #[serde(serialize_with = "crate::ids::serialize_utc_instant")]
    pub consumed_at: UnixMillis,
}

impl SpecConsumptionRecordDraft {
    /// Validates and seals a first-attempt Windows-local consumption record.
    ///
    /// # Errors
    ///
    /// Returns [`DomainValidationError`] when the record does not satisfy the
    /// consumption invariants or canonical hashing fails.
    pub fn seal(self) -> Result<SpecConsumptionRecord, DomainValidationError> {
        if self.schema_version != CONSUMPTION_SCHEMA
            || self.delivery_model != DeliveryModel::WindowsLocal
            || self.attempt_number != 1
        {
            return Err(DomainValidationError::ConsumptionMismatch);
        }
        let consumption_hash = canonical_hash("spec-consumption", 1, &self)?;
        Ok(SpecConsumptionRecord {
            draft: self,
            consumption_hash,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecConsumptionRecord {
    #[serde(flatten)]
    pub draft: SpecConsumptionRecordDraft,
    pub consumption_hash: Sha256Digest,
}

impl SpecConsumptionRecord {
    /// Verifies this consumption record's invariants and canonical hash.
    ///
    /// # Errors
    ///
    /// Returns [`DomainValidationError`] when the record does not satisfy the
    /// consumption invariants, canonical hashing fails, or its hash does not
    /// match.
    pub fn verify(&self) -> Result<(), DomainValidationError> {
        if self.draft.schema_version != CONSUMPTION_SCHEMA
            || self.draft.delivery_model != DeliveryModel::WindowsLocal
            || self.draft.attempt_number != 1
        {
            return Err(DomainValidationError::ConsumptionMismatch);
        }
        let actual = canonical_hash_without_field("spec-consumption", 1, self, "consumptionHash")?;
        if actual != self.consumption_hash {
            return Err(DomainValidationError::HashMismatch);
        }
        Ok(())
    }
}

fn validate_candidate_collections(
    inputs: &[MutableInputBinding],
    writes: &[DeclaredWrite],
    preimages: &[LocalPathPreimage],
) -> Result<(), DomainValidationError> {
    if writes.is_empty() || writes.len() > HARD_MAX_CHANGED_FILES as usize {
        return Err(DomainValidationError::InvalidLimits);
    }
    if inputs.is_empty()
        || inputs.len() > 64
        || inputs
            .iter()
            .any(|input| input.input_id.is_empty() || input.input_id.len() > 256)
    {
        return Err(DomainValidationError::InvalidLimits);
    }
    let mut case_folded_paths = BTreeSet::new();
    if writes
        .iter()
        .any(|write| !case_folded_paths.insert(write.path_pattern.case_folded()))
    {
        return Err(DomainValidationError::DuplicatePath);
    }
    if !inputs
        .windows(2)
        .all(|pair| input_is_before(&pair[0], &pair[1]))
        || !writes
            .windows(2)
            .all(|pair| path_is_before(&pair[0].path_pattern, &pair[1].path_pattern))
        || !preimages
            .windows(2)
            .all(|pair| path_is_before(&pair[0].relative_path, &pair[1].relative_path))
    {
        return Err(DomainValidationError::NonCanonicalOrder);
    }
    if writes.len() != preimages.len() {
        return Err(DomainValidationError::PreimageMismatch);
    }
    for (write, preimage) in writes.iter().zip(preimages) {
        let expected_exists = write.operation != DeclaredWriteOperation::Create;
        if write.path_pattern != preimage.relative_path
            || preimage.exists != expected_exists
            || write.preimage_hash != preimage.content_hash
            || (preimage.exists && preimage.content_hash.is_none())
            || (preimage.exists
                && (preimage.file_identity_hash.is_none() || preimage.metadata_hash.is_none()))
            || (!preimage.exists
                && (preimage.file_identity_hash.is_some()
                    || preimage.content_hash.is_some()
                    || preimage.metadata_hash.is_some()))
        {
            return Err(DomainValidationError::PreimageMismatch);
        }
    }
    Ok(())
}

fn input_is_before(left: &MutableInputBinding, right: &MutableInputBinding) -> bool {
    left.input_kind < right.input_kind
        || (left.input_kind == right.input_kind
            && left
                .input_id
                .encode_utf16()
                .cmp(right.input_id.encode_utf16())
                .is_lt())
}

fn path_is_before(left: &RelativeWorkspacePath, right: &RelativeWorkspacePath) -> bool {
    left.canonical_cmp(right).is_lt()
}

fn valid_cas_reference(reference: &str, digest: Sha256Digest) -> bool {
    reference == format!("cas://sha256/{}", digest.hex_value())
}

#[cfg(test)]
mod tests {
    use super::{
        ApprovedExecutionSpecDraft, AuthorityRef, CandidateCommon, DeclaredWrite,
        DeclaredWriteOperation, DeliveryModel, ExecutionLimits, InputKind, LocalPathPreimage,
        MutableInputBinding, NativePatchEngineAudience, PatchOperation, PatchSet, RollbackClass,
        SpecConsumptionRecordDraft, WindowsPatchCandidateDraft, WorkspaceTarget,
    };
    use crate::{
        canonical_hash, canonical_hash_without_field, generated_contracts, sha256_bytes,
        ContractId, RelativeWorkspacePath, Sha256Digest, UnixMillis,
    };

    fn id(value: &str) -> Result<ContractId, Box<dyn std::error::Error>> {
        // Domain IDs are intentionally generic, while the wire schema binds
        // persisted contract IDs to a canonical ULID-shaped suffix. Keep this
        // generated-shape fixture valid for both boundaries.
        Ok(ContractId::new(format!(
            "{value}_01J00000000000000000000000"
        ))?)
    }

    fn path(value: &str) -> Result<RelativeWorkspacePath, Box<dyn std::error::Error>> {
        Ok(RelativeWorkspacePath::new(value)?)
    }

    #[test]
    fn patch_hash_binds_content_and_order() -> Result<(), Box<dyn std::error::Error>> {
        let first = PatchSet::new(vec![PatchOperation::create(
            path("src/a.txt")?,
            "first".to_owned(),
        )]);
        let second = PatchSet::new(vec![PatchOperation::create(
            path("src/a.txt")?,
            "second".to_owned(),
        )]);
        assert_ne!(first.content_hash()?, second.content_hash()?);
        Ok(())
    }

    #[test]
    fn patch_rejects_case_aliases() -> Result<(), Box<dyn std::error::Error>> {
        let patch = PatchSet::new(vec![
            PatchOperation::create(path("src/App.tsx")?, "one".to_owned()),
            PatchOperation::create(path("src/app.tsx")?, "two".to_owned()),
        ]);
        assert!(patch.validate().is_err());
        Ok(())
    }

    #[test]
    fn replace_constructor_binds_postimage() -> Result<(), Box<dyn std::error::Error>> {
        let operation =
            PatchOperation::replace(path("README.md")?, sha256_bytes(b"old"), "new".to_owned());
        assert_eq!(operation.postimage_hash(), Some(sha256_bytes(b"new")));
        Ok(())
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one conformance fixture keeps the candidate, spec, and consumption bindings visible"
    )]
    fn authority_aggregates_match_generated_wire_shapes_and_hashes(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let relative_path = path("README.md")?;
        let preimage_hash = sha256_bytes(b"before");
        let patch = PatchSet::new(vec![PatchOperation::replace(
            relative_path.clone(),
            preimage_hash,
            "after".to_owned(),
        )]);
        let patch_hash = patch.content_hash()?;
        let authority_ref = AuthorityRef {
            authority_kind: "desktop_local_store".to_owned(),
            authority_id: id("authority_1")?,
            installation_id: id("install_1")?,
            local_store_id: id("store_1")?,
            authority_epoch: 1,
        };
        let mutable_inputs = vec![MutableInputBinding {
            input_kind: InputKind::PathPreimage,
            input_id: relative_path.to_string(),
            content_hash: preimage_hash,
        }];
        let workspace_target = WorkspaceTarget {
            target_kind: "local_folder_capability".to_owned(),
            workspace_capability_id: id("workspace_1")?,
            grant_epoch: 1,
            root_identity_hash: sha256_bytes(b"root"),
            filesystem_capability_hash: sha256_bytes(b"filesystem-capability"),
            base_checkpoint_id: id("checkpoint_0")?,
            workspace_manifest_hash: sha256_bytes(b"workspace-manifest"),
        };
        let executor_audience = NativePatchEngineAudience {
            audience_kind: "native_patch_engine".to_owned(),
            installation_id: authority_ref.installation_id.clone(),
            host_build_id: "desktop-test".to_owned(),
            host_binary_sha256: sha256_bytes(b"host-binary"),
            patch_engine_profile_hash: sha256_bytes(b"patch-engine-profile"),
        };
        let candidate = WindowsPatchCandidateDraft {
            schema_version: "sapphirus.candidate-action.v1".to_owned(),
            common: CandidateCommon {
                candidate_id: id("candidate_1")?,
                project_id: id("project_1")?,
                run_id: id("run_1")?,
                proposal_id: id("proposal_1")?,
                proposal_hash: sha256_bytes(b"proposal"),
                authority_ref: authority_ref.clone(),
                owner_scope_ref: id("owner_1")?,
                policy_context_hash: sha256_bytes(b"policy-context"),
                mutable_inputs: mutable_inputs.clone(),
                declared_writes: vec![DeclaredWrite {
                    path_pattern: relative_path.clone(),
                    operation: DeclaredWriteOperation::Modify,
                    preimage_hash: Some(preimage_hash),
                }],
                limits: ExecutionLimits::governed_patch_defaults(),
                rollback_class: RollbackClass::FileTracked,
                created_at: UnixMillis(1_000),
                expires_at: UnixMillis(901_000),
            },
            delivery_model: DeliveryModel::WindowsLocal,
            action_kind: "patch_apply".to_owned(),
            workspace_target: workspace_target.clone(),
            executor_audience: executor_audience.clone(),
            patch_ref: format!("cas://sha256/{}", patch_hash.hex_value()),
            patch_hash,
            preimages: vec![LocalPathPreimage {
                relative_path,
                exists: true,
                file_identity_hash: Some(sha256_bytes(b"file-identity")),
                content_hash: Some(preimage_hash),
                metadata_hash: Some(sha256_bytes(b"metadata")),
            }],
        }
        .seal()?;

        let candidate_value = serde_json::to_value(&candidate)?;
        assert!(candidate_value.get("networkIntent").is_none());
        assert_eq!(candidate_value["createdAt"], "1970-01-01T00:00:01.000Z");
        let generated_candidate: generated_contracts::CandidateAction =
            serde_json::from_value(candidate_value)?;
        assert_wire_hash(
            &generated_candidate,
            "candidateHash",
            "candidate-action",
            candidate.candidate_hash,
        )?;

        let spec = ApprovedExecutionSpecDraft {
            schema_version: "sapphirus.approved-execution-spec.v1".to_owned(),
            spec_id: id("spec_1")?,
            delivery_model: DeliveryModel::WindowsLocal,
            authority_ref: authority_ref.clone(),
            owner_scope_ref: candidate.draft.common.owner_scope_ref.clone(),
            project_id: candidate.draft.common.project_id.clone(),
            run_id: candidate.draft.common.run_id.clone(),
            proposal_id: candidate.draft.common.proposal_id.clone(),
            proposal_hash: candidate.draft.common.proposal_hash,
            candidate_id: candidate.draft.common.candidate_id.clone(),
            candidate_hash: candidate.candidate_hash,
            approval_id: id("approval_1")?,
            approval_decision_hash: sha256_bytes(b"approval-decision"),
            policy_version: "desktop-policy-1".to_owned(),
            policy_hash: sha256_bytes(b"policy"),
            workspace_target_hash: canonical_hash("workspace-target", 1, &workspace_target)?,
            mutable_input_set_hash: canonical_hash("mutable-input-set", 1, &mutable_inputs)?,
            executor_audience: executor_audience.clone(),
            issued_at: UnixMillis(2_000),
            expires_at: UnixMillis(602_000),
            single_use_nonce_hash: sha256_bytes(b"single-use-nonce"),
        }
        .seal()?;
        let generated_spec: generated_contracts::ApprovedExecutionSpec =
            serde_json::from_value(serde_json::to_value(&spec)?)?;
        assert_wire_hash(
            &generated_spec,
            "specHash",
            "approved-execution-spec",
            spec.spec_hash,
        )?;

        let consumption = SpecConsumptionRecordDraft {
            schema_version: "sapphirus.spec-consumption.v1".to_owned(),
            consumption_id: id("consumption_1")?,
            delivery_model: DeliveryModel::WindowsLocal,
            authority_ref,
            spec_id: spec.draft.spec_id.clone(),
            spec_hash: spec.spec_hash,
            candidate_hash: candidate.candidate_hash,
            single_use_nonce_hash: spec.draft.single_use_nonce_hash,
            executor_audience_hash: canonical_hash("executor-audience", 1, &executor_audience)?,
            execution_id: id("execution_1")?,
            attempt_number: 1,
            consumed_at: UnixMillis(3_000),
        }
        .seal()?;
        let generated_consumption: generated_contracts::SpecConsumptionRecord =
            serde_json::from_value(serde_json::to_value(&consumption)?)?;
        assert_wire_hash(
            &generated_consumption,
            "consumptionHash",
            "spec-consumption",
            consumption.consumption_hash,
        )?;
        Ok(())
    }

    fn assert_wire_hash<T>(
        wire_value: &T,
        hash_field: &str,
        purpose: &str,
        expected: Sha256Digest,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        T: serde::Serialize,
    {
        let actual = canonical_hash_without_field(purpose, 1, wire_value, hash_field)?;
        assert_eq!(actual, expected);
        Ok(())
    }
}
