//! Pure local policy evaluation, immutable spec issuance, and one-time consume.
//!
//! This crate has no file, process, network, or database implementation. The
//! storage adapter must implement [`ConsumptionLedger`] as one atomic
//! insert-if-absent operation over [`ConsumptionKey`].

use desktop_runtime::{
    canonical_hash, sha256_bytes, ApprovalDecision, ApprovedExecutionSpec,
    ApprovedExecutionSpecDraft, AuthorityRef, ContractId, DeclaredWriteOperation,
    DomainValidationError, NativePatchEngineAudience, PatchOperation, PatchSet, Sha256Digest,
    SpecConsumptionRecord, SpecConsumptionRecordDraft, UnixMillis, WindowsPatchCandidate,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const POLICY_SCHEMA: &str = "sapphirus.desktop-patch-policy.v1";
const MIN_NONCE_BYTES: usize = 16;
const MAX_NONCE_BYTES: usize = 64;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
// This host-internal projection is flattened into `PatchPolicy`. `seal` binds
// its content but does not verify a service signature; the cloud/policy adapter
// must perform that validation before mapping a signed policy into this type.
#[serde(rename_all = "camelCase")]
pub struct PatchPolicyBody {
    pub schema_version: String,
    pub policy_version: String,
    pub policy_context_hash: Sha256Digest,
    pub authority_ref: AuthorityRef,
    pub installation_id: ContractId,
    pub max_changed_files: u32,
    pub max_changed_bytes: u64,
    pub max_spec_lifetime_ms: u64,
    pub expires_at: UnixMillis,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchPolicy {
    #[serde(flatten)]
    pub body: PatchPolicyBody,
    pub policy_hash: Sha256Digest,
}

impl PatchPolicyBody {
    /// Validates and seals this policy body with its canonical content hash.
    ///
    /// # Errors
    ///
    /// Returns [`AirlockError::PolicyDenied`] when a policy limit or authority
    /// binding is invalid, or [`AirlockError::InvalidDomain`] when canonical
    /// hashing fails.
    pub fn seal(self) -> Result<PatchPolicy, AirlockError> {
        validate_policy_body(&self)?;
        let policy_hash = canonical_hash("desktop-patch-policy", 1, &self)
            .map_err(DomainValidationError::from)?;
        Ok(PatchPolicy {
            body: self,
            policy_hash,
        })
    }
}

impl PatchPolicy {
    /// Verifies the policy body and its canonical content hash.
    ///
    /// # Errors
    ///
    /// Returns an [`AirlockError`] when the policy body is invalid, cannot be
    /// hashed canonically, or does not match its bound hash.
    pub fn verify(&self) -> Result<(), AirlockError> {
        validate_policy_body(&self.body)?;
        let actual = canonical_hash("desktop-patch-policy", 1, &self.body)
            .map_err(DomainValidationError::from)?;
        if actual != self.policy_hash {
            return Err(AirlockError::PolicyIntegrity);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ConsumptionKey {
    pub spec_hash: Sha256Digest,
    pub single_use_nonce_hash: Sha256Digest,
    pub executor_audience_hash: Sha256Digest,
}

#[derive(Debug, Error)]
#[error("the consumption store could not complete its atomic operation")]
pub struct ConsumptionStoreError;

/// Durable implementations must enforce a unique key and commit the record in
/// the same transaction as the successful compare-and-swap.
pub trait ConsumptionLedger: Send + Sync {
    /// Atomically records a previously unseen consumption key and record.
    ///
    /// # Errors
    ///
    /// Returns [`ConsumptionStoreError`] when the durable store cannot complete
    /// the atomic insert-if-absent operation.
    fn insert_if_absent(
        &self,
        key: &ConsumptionKey,
        record: &SpecConsumptionRecord,
    ) -> Result<bool, ConsumptionStoreError>;
}

#[derive(Debug, Error)]
pub enum AirlockError {
    #[error("candidate was denied by desktop patch policy")]
    PolicyDenied,
    #[error("policy content or signature binding is invalid")]
    PolicyIntegrity,
    #[error("candidate, approval, patch, or spec binding is inconsistent")]
    BindingMismatch,
    #[error("the authority object has expired or is not yet valid")]
    Expired,
    #[error("the single-use nonce does not meet requirements")]
    InvalidNonce,
    #[error("the approved spec has already been consumed")]
    AlreadyConsumed,
    #[error("the consumption store is unavailable")]
    StoreUnavailable,
    #[error(transparent)]
    InvalidDomain(#[from] DomainValidationError),
}

#[derive(Clone, Debug)]
pub struct IssueSpecInput<'a> {
    pub candidate: &'a WindowsPatchCandidate,
    pub patch: &'a PatchSet,
    pub approval: &'a ApprovalDecision,
    pub policy: &'a PatchPolicy,
    pub spec_id: ContractId,
    pub issued_at: UnixMillis,
    pub expires_at: UnixMillis,
    pub single_use_nonce: &'a [u8],
}

#[derive(Clone, Debug)]
pub struct ConsumeSpecInput<'a> {
    pub spec: &'a ApprovedExecutionSpec,
    pub candidate: &'a WindowsPatchCandidate,
    pub patch: &'a PatchSet,
    pub policy: &'a PatchPolicy,
    pub current_audience: &'a NativePatchEngineAudience,
    pub single_use_nonce: &'a [u8],
    pub consumption_id: ContractId,
    pub execution_id: ContractId,
    pub consumed_at: UnixMillis,
}

pub struct PatchAirlock;

impl PatchAirlock {
    /// Validates the exact approval inputs and issues a single-use execution spec.
    ///
    /// # Errors
    ///
    /// Returns an [`AirlockError`] when any candidate, patch, approval, policy,
    /// lifetime, nonce, or authority binding is invalid.
    pub fn issue(input: IssueSpecInput<'_>) -> Result<ApprovedExecutionSpec, AirlockError> {
        validate_exact_patch(input.candidate, input.patch)?;
        input.approval.verify_for(input.candidate)?;
        input.policy.verify()?;
        validate_policy_binding(input.candidate, input.policy, input.issued_at)?;
        validate_nonce(input.single_use_nonce)?;

        let expected_diff_hash = canonical_hash("displayed-diff", 1, input.patch)
            .map_err(DomainValidationError::from)?;
        if input.approval.draft.displayed_diff_hash != expected_diff_hash
            || input.approval.draft.decided_at < input.candidate.draft.common.created_at
            || input.approval.draft.decided_at > input.issued_at
        {
            return Err(AirlockError::BindingMismatch);
        }

        let latest_expiry = input
            .issued_at
            .0
            .checked_add(input.policy.body.max_spec_lifetime_ms)
            .ok_or(AirlockError::Expired)?;
        if input.expires_at <= input.issued_at
            || input.expires_at > input.candidate.draft.common.expires_at
            || input.expires_at > input.policy.body.expires_at
            || input.expires_at.0 > latest_expiry
        {
            return Err(AirlockError::Expired);
        }

        ApprovedExecutionSpecDraft {
            schema_version: "sapphirus.approved-execution-spec.v1".to_owned(),
            spec_id: input.spec_id,
            delivery_model: input.candidate.draft.delivery_model,
            authority_ref: input.candidate.draft.common.authority_ref.clone(),
            owner_scope_ref: input.candidate.draft.common.owner_scope_ref.clone(),
            project_id: input.candidate.draft.common.project_id.clone(),
            run_id: input.candidate.draft.common.run_id.clone(),
            proposal_id: input.candidate.draft.common.proposal_id.clone(),
            proposal_hash: input.candidate.draft.common.proposal_hash,
            candidate_id: input.candidate.draft.common.candidate_id.clone(),
            candidate_hash: input.candidate.candidate_hash,
            approval_id: input.approval.draft.approval_id.clone(),
            approval_decision_hash: input.approval.approval_decision_hash,
            policy_version: input.policy.body.policy_version.clone(),
            policy_hash: input.policy.policy_hash,
            workspace_target_hash: canonical_hash(
                "workspace-target",
                1,
                &input.candidate.draft.workspace_target,
            )
            .map_err(DomainValidationError::from)?,
            mutable_input_set_hash: canonical_hash(
                "mutable-input-set",
                1,
                &input.candidate.draft.common.mutable_inputs,
            )
            .map_err(DomainValidationError::from)?,
            executor_audience: input.candidate.draft.executor_audience.clone(),
            issued_at: input.issued_at,
            expires_at: input.expires_at,
            single_use_nonce_hash: sha256_bytes(input.single_use_nonce),
        }
        .seal()
        .map_err(AirlockError::from)
    }

    /// Validates and atomically consumes a single-use execution spec.
    ///
    /// # Errors
    ///
    /// Returns an [`AirlockError`] when any bound input is invalid, the spec is
    /// expired or already consumed, or the consumption store is unavailable.
    pub fn consume<L>(
        ledger: &L,
        input: ConsumeSpecInput<'_>,
    ) -> Result<SpecConsumptionRecord, AirlockError>
    where
        L: ConsumptionLedger,
    {
        input.spec.verify()?;
        validate_exact_patch(input.candidate, input.patch)?;
        input.policy.verify()?;
        validate_policy_binding(input.candidate, input.policy, input.consumed_at)?;
        validate_nonce(input.single_use_nonce)?;
        validate_spec_binding(&input)?;

        let audience_hash = canonical_hash("executor-audience", 1, input.current_audience)
            .map_err(DomainValidationError::from)?;
        let nonce_hash = sha256_bytes(input.single_use_nonce);
        let record = SpecConsumptionRecordDraft {
            schema_version: "sapphirus.spec-consumption.v1".to_owned(),
            consumption_id: input.consumption_id,
            delivery_model: input.spec.draft.delivery_model,
            authority_ref: input.spec.draft.authority_ref.clone(),
            spec_id: input.spec.draft.spec_id.clone(),
            spec_hash: input.spec.spec_hash,
            candidate_hash: input.spec.draft.candidate_hash,
            single_use_nonce_hash: nonce_hash,
            executor_audience_hash: audience_hash,
            execution_id: input.execution_id,
            attempt_number: 1,
            consumed_at: input.consumed_at,
        }
        .seal()?;
        let key = ConsumptionKey {
            spec_hash: input.spec.spec_hash,
            single_use_nonce_hash: nonce_hash,
            executor_audience_hash: audience_hash,
        };
        let inserted = ledger
            .insert_if_absent(&key, &record)
            .map_err(|_| AirlockError::StoreUnavailable)?;
        if !inserted {
            return Err(AirlockError::AlreadyConsumed);
        }
        Ok(record)
    }
}

fn validate_policy_body(body: &PatchPolicyBody) -> Result<(), AirlockError> {
    if body.schema_version != POLICY_SCHEMA
        || body.policy_version.is_empty()
        || body.policy_version.len() > 128
        || body.max_changed_files == 0
        || body.max_changed_files > desktop_runtime::HARD_MAX_CHANGED_FILES
        || body.max_changed_bytes == 0
        || body.max_changed_bytes > desktop_runtime::HARD_MAX_CHANGED_BYTES
        || body.max_spec_lifetime_ms == 0
        || body.max_spec_lifetime_ms > 15 * 60 * 1000
        || body.installation_id != body.authority_ref.installation_id
    {
        return Err(AirlockError::PolicyDenied);
    }
    Ok(())
}

fn validate_policy_binding(
    candidate: &WindowsPatchCandidate,
    policy: &PatchPolicy,
    now: UnixMillis,
) -> Result<(), AirlockError> {
    candidate.verify()?;
    let common = &candidate.draft.common;
    if now < common.created_at || now > common.expires_at || now > policy.body.expires_at {
        return Err(AirlockError::Expired);
    }
    if common.policy_context_hash != policy.body.policy_context_hash
        || common.authority_ref != policy.body.authority_ref
        || candidate.draft.executor_audience.installation_id != policy.body.installation_id
        || common.limits.max_changed_files > policy.body.max_changed_files
        || common.limits.max_changed_bytes > policy.body.max_changed_bytes
    {
        return Err(AirlockError::PolicyDenied);
    }
    Ok(())
}

fn validate_exact_patch(
    candidate: &WindowsPatchCandidate,
    patch: &PatchSet,
) -> Result<(), AirlockError> {
    candidate.verify()?;
    patch.validate()?;
    if patch.content_hash()? != candidate.draft.patch_hash
        || patch.operations.len() != candidate.draft.common.declared_writes.len()
        || patch.operations.len() != candidate.draft.preimages.len()
        || patch.operations.len() > candidate.draft.common.limits.max_changed_files as usize
        || patch.changed_bytes() > candidate.draft.common.limits.max_changed_bytes
    {
        return Err(AirlockError::BindingMismatch);
    }

    for ((operation, declared), preimage) in patch
        .operations
        .iter()
        .zip(&candidate.draft.common.declared_writes)
        .zip(&candidate.draft.preimages)
    {
        let expected_operation = match operation {
            PatchOperation::Create { .. } => DeclaredWriteOperation::Create,
            PatchOperation::Replace { .. } => DeclaredWriteOperation::Modify,
            PatchOperation::Delete { .. } => DeclaredWriteOperation::Delete,
        };
        if operation.relative_path() != &declared.path_pattern
            || operation.relative_path() != &preimage.relative_path
            || expected_operation != declared.operation
            || operation.preimage_hash() != declared.preimage_hash
            || operation.preimage_hash() != preimage.content_hash
        {
            return Err(AirlockError::BindingMismatch);
        }
    }
    Ok(())
}

fn validate_spec_binding(input: &ConsumeSpecInput<'_>) -> Result<(), AirlockError> {
    let spec = &input.spec.draft;
    let candidate = input.candidate;
    let audience_hash = canonical_hash("executor-audience", 1, input.current_audience)
        .map_err(DomainValidationError::from)?;
    let spec_audience_hash = canonical_hash("executor-audience", 1, &spec.executor_audience)
        .map_err(DomainValidationError::from)?;
    let workspace_target_hash =
        canonical_hash("workspace-target", 1, &candidate.draft.workspace_target)
            .map_err(DomainValidationError::from)?;
    let input_set_hash = canonical_hash(
        "mutable-input-set",
        1,
        &candidate.draft.common.mutable_inputs,
    )
    .map_err(DomainValidationError::from)?;

    if input.consumed_at < spec.issued_at
        || input.consumed_at > spec.expires_at
        || spec.candidate_id != candidate.draft.common.candidate_id
        || spec.candidate_hash != candidate.candidate_hash
        || spec.authority_ref != candidate.draft.common.authority_ref
        || spec.owner_scope_ref != candidate.draft.common.owner_scope_ref
        || spec.project_id != candidate.draft.common.project_id
        || spec.run_id != candidate.draft.common.run_id
        || spec.proposal_id != candidate.draft.common.proposal_id
        || spec.proposal_hash != candidate.draft.common.proposal_hash
        || spec.policy_hash != input.policy.policy_hash
        || spec.policy_version != input.policy.body.policy_version
        || spec.workspace_target_hash != workspace_target_hash
        || spec.mutable_input_set_hash != input_set_hash
        || spec.single_use_nonce_hash != sha256_bytes(input.single_use_nonce)
        || audience_hash != spec_audience_hash
        || input.current_audience != &spec.executor_audience
    {
        return Err(AirlockError::BindingMismatch);
    }
    Ok(())
}

fn validate_nonce(nonce: &[u8]) -> Result<(), AirlockError> {
    if !(MIN_NONCE_BYTES..=MAX_NONCE_BYTES).contains(&nonce.len())
        || nonce.iter().all(|byte| *byte == 0)
    {
        return Err(AirlockError::InvalidNonce);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::Mutex;

    use desktop_runtime::{
        canonical_hash, sha256_bytes, ApprovalDecisionDraft, AuthorityRef, CandidateCommon,
        ContractId, DeclaredWrite, DeclaredWriteOperation, DeliveryModel, ExecutionLimits,
        LocalPathPreimage, MutableInputBinding, NativePatchEngineAudience, PatchOperation,
        PatchSet, RelativeWorkspacePath, RollbackClass, UnixMillis, WindowsPatchCandidateDraft,
        WorkspaceTarget,
    };

    use super::{
        AirlockError, ConsumeSpecInput, ConsumptionKey, ConsumptionLedger, ConsumptionStoreError,
        IssueSpecInput, PatchAirlock, PatchPolicy, PatchPolicyBody,
    };

    #[derive(Default)]
    struct MemoryLedger {
        keys: Mutex<HashSet<ConsumptionKey>>,
    }

    impl ConsumptionLedger for MemoryLedger {
        fn insert_if_absent(
            &self,
            key: &ConsumptionKey,
            _record: &desktop_runtime::SpecConsumptionRecord,
        ) -> Result<bool, ConsumptionStoreError> {
            let mut keys = self.keys.lock().map_err(|_| ConsumptionStoreError)?;
            Ok(keys.insert(key.clone()))
        }
    }

    fn id(value: &str) -> Result<ContractId, Box<dyn std::error::Error>> {
        Ok(ContractId::new(value)?)
    }

    type AirlockFixture = (
        desktop_runtime::WindowsPatchCandidate,
        PatchSet,
        desktop_runtime::ApprovalDecision,
        PatchPolicy,
        NativePatchEngineAudience,
    );

    fn fixture() -> Result<AirlockFixture, Box<dyn std::error::Error>> {
        let relative_path = RelativeWorkspacePath::new("src/App.tsx")?;
        let old_hash = sha256_bytes(b"old");
        let patch = PatchSet::new(vec![PatchOperation::replace(
            relative_path.clone(),
            old_hash,
            "new".to_owned(),
        )]);
        let patch_hash = patch.content_hash()?;
        let authority = AuthorityRef {
            authority_kind: "desktop_local_store".to_owned(),
            authority_id: id("authority_1")?,
            installation_id: id("install_1")?,
            local_store_id: id("store_1")?,
            authority_epoch: 1,
        };
        let policy_context_hash = sha256_bytes(b"policy-context");
        let audience = NativePatchEngineAudience {
            audience_kind: "native_patch_engine".to_owned(),
            installation_id: id("install_1")?,
            host_build_id: "desktop-test".to_owned(),
            host_binary_sha256: sha256_bytes(b"binary"),
            patch_engine_profile_hash: sha256_bytes(b"profile"),
        };
        let candidate = WindowsPatchCandidateDraft {
            schema_version: "sapphirus.candidate-action.v1".to_owned(),
            common: CandidateCommon {
                candidate_id: id("candidate_1")?,
                project_id: id("project_1")?,
                run_id: id("run_1")?,
                proposal_id: id("proposal_1")?,
                proposal_hash: sha256_bytes(b"proposal"),
                authority_ref: authority.clone(),
                owner_scope_ref: id("owner_1")?,
                policy_context_hash,
                mutable_inputs: vec![MutableInputBinding {
                    input_kind: desktop_runtime::InputKind::WorkspaceManifest,
                    input_id: "manifest_1".to_owned(),
                    content_hash: sha256_bytes(b"manifest"),
                }],
                declared_writes: vec![DeclaredWrite {
                    path_pattern: relative_path.clone(),
                    operation: DeclaredWriteOperation::Modify,
                    preimage_hash: Some(old_hash),
                }],
                limits: ExecutionLimits::governed_patch_defaults(),
                rollback_class: RollbackClass::FileTracked,
                created_at: UnixMillis(1_000),
                expires_at: UnixMillis(20_000),
            },
            delivery_model: DeliveryModel::WindowsLocal,
            action_kind: "patch_apply".to_owned(),
            workspace_target: WorkspaceTarget {
                target_kind: "local_folder_capability".to_owned(),
                workspace_capability_id: id("workspace_1")?,
                grant_epoch: 1,
                root_identity_hash: sha256_bytes(b"root"),
                filesystem_capability_hash: sha256_bytes(b"filesystem"),
                base_checkpoint_id: id("checkpoint_0")?,
                workspace_manifest_hash: sha256_bytes(b"manifest"),
            },
            executor_audience: audience.clone(),
            patch_ref: format!("cas://sha256/{}", patch_hash.hex_value()),
            patch_hash,
            preimages: vec![LocalPathPreimage {
                relative_path,
                exists: true,
                file_identity_hash: Some(sha256_bytes(b"file-id")),
                content_hash: Some(old_hash),
                metadata_hash: Some(sha256_bytes(b"metadata")),
            }],
        }
        .seal()?;
        let diff_hash = canonical_hash("displayed-diff", 1, &patch)?;
        let approval = ApprovalDecisionDraft::approved(
            id("approval_1")?,
            &candidate,
            diff_hash,
            UnixMillis(2_000),
        )
        .seal()?;
        let policy = PatchPolicyBody {
            schema_version: "sapphirus.desktop-patch-policy.v1".to_owned(),
            policy_version: "policy-1".to_owned(),
            policy_context_hash,
            authority_ref: authority,
            installation_id: id("install_1")?,
            max_changed_files: 20,
            max_changed_bytes: 1024 * 1024,
            max_spec_lifetime_ms: 300_000,
            expires_at: UnixMillis(20_000),
        }
        .seal()?;
        Ok((candidate, patch, approval, policy, audience))
    }

    #[test]
    fn spec_is_immutable_and_consumption_is_single_use() -> Result<(), Box<dyn std::error::Error>> {
        let (candidate, patch, approval, policy, audience) = fixture()?;
        let nonce = b"0123456789abcdef0123456789abcdef";
        let spec = PatchAirlock::issue(IssueSpecInput {
            candidate: &candidate,
            patch: &patch,
            approval: &approval,
            policy: &policy,
            spec_id: id("spec_1")?,
            issued_at: UnixMillis(3_000),
            expires_at: UnixMillis(10_000),
            single_use_nonce: nonce,
        })?;
        let original_spec_hash = spec.spec_hash;
        let ledger = MemoryLedger::default();
        let first = PatchAirlock::consume(
            &ledger,
            ConsumeSpecInput {
                spec: &spec,
                candidate: &candidate,
                patch: &patch,
                policy: &policy,
                current_audience: &audience,
                single_use_nonce: nonce,
                consumption_id: id("consumption_1")?,
                execution_id: id("execution_1")?,
                consumed_at: UnixMillis(4_000),
            },
        )?;
        assert_eq!(first.draft.spec_hash, original_spec_hash);
        assert_eq!(spec.spec_hash, original_spec_hash);

        let replay = PatchAirlock::consume(
            &ledger,
            ConsumeSpecInput {
                spec: &spec,
                candidate: &candidate,
                patch: &patch,
                policy: &policy,
                current_audience: &audience,
                single_use_nonce: nonce,
                consumption_id: id("consumption_2")?,
                execution_id: id("execution_2")?,
                consumed_at: UnixMillis(4_001),
            },
        );
        assert!(matches!(replay, Err(AirlockError::AlreadyConsumed)));
        Ok(())
    }

    #[test]
    fn changed_nonce_cannot_consume_the_spec() -> Result<(), Box<dyn std::error::Error>> {
        let (candidate, patch, approval, policy, audience) = fixture()?;
        let spec = PatchAirlock::issue(IssueSpecInput {
            candidate: &candidate,
            patch: &patch,
            approval: &approval,
            policy: &policy,
            spec_id: id("spec_1")?,
            issued_at: UnixMillis(3_000),
            expires_at: UnixMillis(10_000),
            single_use_nonce: b"0123456789abcdef",
        })?;
        let result = PatchAirlock::consume(
            &MemoryLedger::default(),
            ConsumeSpecInput {
                spec: &spec,
                candidate: &candidate,
                patch: &patch,
                policy: &policy,
                current_audience: &audience,
                single_use_nonce: b"fedcba9876543210",
                consumption_id: id("consumption_1")?,
                execution_id: id("execution_1")?,
                consumed_at: UnixMillis(4_000),
            },
        );
        assert!(matches!(result, Err(AirlockError::BindingMismatch)));
        Ok(())
    }
}
