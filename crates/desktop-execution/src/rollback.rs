use std::collections::BTreeMap;

use desktop_runtime::{
    canonical_hash, canonical_hash_without_field, sha256_bytes, ContractId, DomainValidationError,
    PatchOperation, PatchSet, RelativeWorkspacePath, Sha256Digest, UnixMillis,
};
use serde::{Deserialize, Serialize};

use crate::model::RecoveryPathObservation;

use crate::{
    CheckpointFileState, EffectJournal, ExecutionError, FileObservation, LocalCheckpoint,
    LocalExecutionResult, RecoveryDisposition, RecoveryOperation, RecoveryPlan, RecoveryReason,
    RecoveryRestoreResult, WorkspaceFileIo,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RollbackConflict {
    pub relative_path: RelativeWorkspacePath,
    pub expected_exists: bool,
    pub expected_content_hash: Option<Sha256Digest>,
    pub current_exists: bool,
    pub current_content_hash: Option<Sha256Digest>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RollbackPlan {
    pub schema_version: String,
    pub rollback_plan_id: ContractId,
    pub source_execution_id: ContractId,
    pub target_checkpoint_id: ContractId,
    pub workspace_target_hash: Sha256Digest,
    pub operations: Vec<PatchOperation>,
    pub conflicts: Vec<RollbackConflict>,
    pub created_at: UnixMillis,
    pub rollback_plan_hash: Sha256Digest,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RollbackPlanDraft<'a> {
    schema_version: &'static str,
    rollback_plan_id: &'a ContractId,
    source_execution_id: &'a ContractId,
    target_checkpoint_id: &'a ContractId,
    workspace_target_hash: Sha256Digest,
    operations: &'a [PatchOperation],
    conflicts: &'a [RollbackConflict],
    created_at: UnixMillis,
}

struct RollbackChanges {
    operations: Vec<PatchOperation>,
    conflicts: Vec<RollbackConflict>,
}

impl RollbackPlan {
    /// Verifies the rollback plan's canonical content hash.
    ///
    /// # Errors
    ///
    /// Returns [`ExecutionError`] when the plan schema or bound hash is invalid.
    pub fn verify(&self) -> Result<(), ExecutionError> {
        if self.schema_version != "sapphirus.local-rollback-plan.v1" {
            return Err(ExecutionError::IntegrityFailure);
        }
        let actual =
            canonical_hash_without_field("local-rollback-plan", 1, self, "rollbackPlanHash")
                .map_err(DomainValidationError::from)?;
        if actual != self.rollback_plan_hash {
            return Err(ExecutionError::IntegrityFailure);
        }
        Ok(())
    }

    /// Converts a conflict-free rollback plan into a governed patch set.
    ///
    /// # Errors
    ///
    /// Returns [`ExecutionError`] when plan verification fails, a conflict is
    /// present, or the generated patch is invalid.
    pub fn to_patch_set(&self) -> Result<Option<PatchSet>, ExecutionError> {
        self.verify()?;
        if !self.conflicts.is_empty() {
            return Err(ExecutionError::PreconditionFailed);
        }
        if self.operations.is_empty() {
            return Ok(None);
        }
        let patch = PatchSet::new(self.operations.clone());
        patch.validate()?;
        Ok(Some(patch))
    }
}

/// Builds a fresh rollback plan from a verified checkpoint and execution result.
///
/// # Errors
///
/// Returns [`ExecutionError`] when authority bindings, checkpoint or result
/// integrity, workspace observations, or canonical hashing fail.
pub fn plan_rollback<W>(
    workspace: &W,
    rollback_plan_id: ContractId,
    checkpoint: &LocalCheckpoint,
    result: &LocalExecutionResult,
    created_at: UnixMillis,
) -> Result<RollbackPlan, ExecutionError>
where
    W: WorkspaceFileIo,
{
    checkpoint.verify()?;
    result.verify()?;
    let current_target_hash = workspace
        .workspace_target_hash()
        .map_err(|_| ExecutionError::WorkspaceFailure)?;
    if current_target_hash != checkpoint.workspace_target_hash
        || result.checkpoint_id != checkpoint.checkpoint_id
        || result.candidate_hash != checkpoint.candidate_hash
    {
        return Err(ExecutionError::AuthorizationMismatch);
    }

    let expected = observation_map(&result.files)?;
    let RollbackChanges {
        operations,
        conflicts,
    } = build_rollback_changes(workspace, checkpoint, &expected)?;
    if expected.len() != checkpoint.entries.len() {
        return Err(ExecutionError::IntegrityFailure);
    }

    let rollback_plan_hash = canonical_hash(
        "local-rollback-plan",
        1,
        &RollbackPlanDraft {
            schema_version: "sapphirus.local-rollback-plan.v1",
            rollback_plan_id: &rollback_plan_id,
            source_execution_id: &result.execution_id,
            target_checkpoint_id: &checkpoint.checkpoint_id,
            workspace_target_hash: checkpoint.workspace_target_hash,
            operations: &operations,
            conflicts: &conflicts,
            created_at,
        },
    )
    .map_err(DomainValidationError::from)?;

    Ok(RollbackPlan {
        schema_version: "sapphirus.local-rollback-plan.v1".to_owned(),
        rollback_plan_id,
        source_execution_id: result.execution_id.clone(),
        target_checkpoint_id: checkpoint.checkpoint_id.clone(),
        workspace_target_hash: checkpoint.workspace_target_hash,
        operations,
        conflicts,
        created_at,
        rollback_plan_hash,
    })
}

fn build_rollback_changes<W>(
    workspace: &W,
    checkpoint: &LocalCheckpoint,
    expected: &BTreeMap<String, &FileObservation>,
) -> Result<RollbackChanges, ExecutionError>
where
    W: WorkspaceFileIo,
{
    let mut operations = Vec::new();
    let mut conflicts = Vec::new();
    for entry in &checkpoint.entries {
        let expected_after = expected
            .get(&entry.relative_path.case_folded())
            .ok_or(ExecutionError::IntegrityFailure)?;
        let current_bytes = workspace
            .read_file(&entry.relative_path, None)
            .map_err(|_| ExecutionError::WorkspaceFailure)?;
        let current_exists = current_bytes.is_some();
        let current_hash = current_bytes.as_deref().map(sha256_bytes);
        let drifted =
            current_exists != expected_after.exists || current_hash != expected_after.content_hash;
        if drifted {
            conflicts.push(RollbackConflict {
                relative_path: entry.relative_path.clone(),
                expected_exists: expected_after.exists,
                expected_content_hash: expected_after.content_hash,
                current_exists,
                current_content_hash: current_hash,
            });
        }

        match (&entry.before, current_bytes) {
            (CheckpointFileState::Absent, Some(_)) => {
                if let Some(preimage_hash) = current_hash {
                    operations.push(PatchOperation::delete(
                        entry.relative_path.clone(),
                        preimage_hash,
                    ));
                }
            }
            (
                CheckpointFileState::Utf8 {
                    content,
                    content_hash: _,
                },
                None,
            ) => operations.push(PatchOperation::create(
                entry.relative_path.clone(),
                content.clone(),
            )),
            (
                CheckpointFileState::Utf8 {
                    content,
                    content_hash,
                },
                Some(_),
            ) if current_hash != Some(*content_hash) => {
                if let Some(preimage_hash) = current_hash {
                    operations.push(PatchOperation::replace(
                        entry.relative_path.clone(),
                        preimage_hash,
                        content.clone(),
                    ));
                }
            }
            (CheckpointFileState::Absent, None) | (CheckpointFileState::Utf8 { .. }, Some(_)) => {}
        }
    }
    Ok(RollbackChanges {
        operations,
        conflicts,
    })
}

/// Classifies an interrupted effect journal using current workspace observations.
///
/// # Errors
///
/// Returns [`ExecutionError`] when checkpoint or journal integrity, authority
/// bindings, or workspace observations cannot be verified.
pub fn plan_recovery<W>(
    workspace: &W,
    journal: &EffectJournal,
    checkpoint: &LocalCheckpoint,
) -> Result<RecoveryPlan, ExecutionError>
where
    W: WorkspaceFileIo,
{
    checkpoint.verify()?;
    journal.verify_plan()?;
    if journal.checkpoint_id != checkpoint.checkpoint_id
        || journal.candidate_hash != checkpoint.candidate_hash
        || journal.workspace_target_hash != checkpoint.workspace_target_hash
        || workspace
            .workspace_target_hash()
            .map_err(|_| ExecutionError::WorkspaceFailure)?
            != checkpoint.workspace_target_hash
    {
        return Err(ExecutionError::AuthorizationMismatch);
    }

    let checkpoint_entries: BTreeMap<_, _> = checkpoint
        .entries
        .iter()
        .map(|entry| (entry.relative_path.case_folded(), entry))
        .collect();
    if !verify_recovery_checkpoint_coverage(journal, checkpoint, &checkpoint_entries)? {
        return seal_recovery_plan(
            journal,
            RecoveryDisposition::ManualReview,
            Vec::new(),
            Vec::new(),
            RecoveryReason::IncompleteCheckpointCoverage,
        );
    }

    let mut all_preimages = true;
    let mut all_postimages = true;
    let mut recovery_observations = Vec::with_capacity(journal.operations.len());
    let mut recovery_operations = Vec::new();
    for operation in &journal.operations {
        let checkpoint_entry = checkpoint_entries
            .get(&operation.relative_path.case_folded())
            .ok_or(ExecutionError::IntegrityFailure)?;
        let current = observe_recovery_file(workspace, &operation.relative_path)?;
        let current_exists = current.content.is_some();
        let current_hash = current.content.as_deref().map(sha256_bytes);
        all_preimages &= current_exists == checkpoint_entry.before.exists()
            && current_hash == checkpoint_entry.before.content_hash();
        all_postimages &= current_exists == operation.postimage_hash.is_some()
            && current_hash == operation.postimage_hash;

        recovery_observations.push(RecoveryPathObservation {
            relative_path: checkpoint_entry.relative_path.clone(),
            current_exists,
            current_content_hash: current_hash,
            current_file_identity_hash: current.file_identity_hash,
            restore_to: checkpoint_entry.before.clone(),
        });

        if current_exists != checkpoint_entry.before.exists()
            || current_hash != checkpoint_entry.before.content_hash()
        {
            recovery_operations.push(RecoveryOperation {
                relative_path: checkpoint_entry.relative_path.clone(),
                expected_current_exists: current_exists,
                expected_current_content_hash: current_hash,
                expected_current_file_identity_hash: current.file_identity_hash,
                restore_to: checkpoint_entry.before.clone(),
            });
        }
    }
    recovery_observations
        .sort_by(|left, right| left.relative_path.canonical_cmp(&right.relative_path));
    recovery_operations
        .sort_by(|left, right| left.relative_path.canonical_cmp(&right.relative_path));

    if all_preimages {
        seal_recovery_plan(
            journal,
            RecoveryDisposition::NoEffect,
            recovery_observations,
            Vec::new(),
            RecoveryReason::NoEffectObserved,
        )
    } else if all_postimages {
        seal_recovery_plan(
            journal,
            RecoveryDisposition::Complete,
            recovery_observations,
            recovery_operations,
            RecoveryReason::PostimagesVerified,
        )
    } else {
        seal_recovery_plan(
            journal,
            RecoveryDisposition::RestoreCheckpoint,
            recovery_observations,
            recovery_operations,
            RecoveryReason::CompleteCheckpointCoverage,
        )
    }
}

fn verify_recovery_checkpoint_coverage(
    journal: &EffectJournal,
    checkpoint: &LocalCheckpoint,
    checkpoint_entries: &BTreeMap<String, &crate::CheckpointEntry>,
) -> Result<bool, ExecutionError> {
    let same_cardinality = checkpoint.entries.len() == journal.operations.len();
    let complete = same_cardinality
        && journal.operations.iter().all(|operation| {
            checkpoint_entries.contains_key(&operation.relative_path.case_folded())
        });
    for operation in &journal.operations {
        let Some(checkpoint_entry) = checkpoint_entries.get(&operation.relative_path.case_folded())
        else {
            if same_cardinality {
                return Err(ExecutionError::IntegrityFailure);
            }
            continue;
        };
        if operation.relative_path != checkpoint_entry.relative_path {
            return Err(ExecutionError::IntegrityFailure);
        }
        let valid_kind = matches!(
            (operation.operation, &checkpoint_entry.before),
            (
                crate::ResultFileOperation::Created,
                CheckpointFileState::Absent
            ) | (
                crate::ResultFileOperation::Modified | crate::ResultFileOperation::Deleted,
                CheckpointFileState::Utf8 { .. }
            )
        );
        if operation.preimage_hash != checkpoint_entry.before.content_hash() || !valid_kind {
            return Err(ExecutionError::IntegrityFailure);
        }
    }
    Ok(complete)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryPlanBinding<'a> {
    schema_version: &'static str,
    journal_id: &'a ContractId,
    execution_id: &'a ContractId,
    checkpoint_id: &'a ContractId,
    workspace_target_hash: Sha256Digest,
    disposition: RecoveryDisposition,
    observations: Vec<RecoveryObservationBinding<'a>>,
    operations: Vec<RecoveryOperationBinding<'a>>,
    reason: RecoveryReason,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryObservationBinding<'a> {
    relative_path: &'a RelativeWorkspacePath,
    current_exists: bool,
    current_content_hash: Option<Sha256Digest>,
    current_file_identity_hash: Option<Sha256Digest>,
    restore_exists: bool,
    restore_content_hash: Option<Sha256Digest>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryOperationBinding<'a> {
    relative_path: &'a RelativeWorkspacePath,
    expected_current_exists: bool,
    expected_current_content_hash: Option<Sha256Digest>,
    expected_current_file_identity_hash: Option<Sha256Digest>,
    restore_exists: bool,
    restore_content_hash: Option<Sha256Digest>,
}

fn recovery_plan_hash(plan: &RecoveryPlan) -> Result<Sha256Digest, ExecutionError> {
    let observations = plan
        .observations
        .iter()
        .map(|observation| RecoveryObservationBinding {
            relative_path: &observation.relative_path,
            current_exists: observation.current_exists,
            current_content_hash: observation.current_content_hash,
            current_file_identity_hash: observation.current_file_identity_hash,
            restore_exists: observation.restore_to.exists(),
            restore_content_hash: observation.restore_to.content_hash(),
        })
        .collect();
    let operations = plan
        .operations
        .iter()
        .map(|operation| RecoveryOperationBinding {
            relative_path: &operation.relative_path,
            expected_current_exists: operation.expected_current_exists,
            expected_current_content_hash: operation.expected_current_content_hash,
            expected_current_file_identity_hash: operation.expected_current_file_identity_hash,
            restore_exists: operation.restore_to.exists(),
            restore_content_hash: operation.restore_to.content_hash(),
        })
        .collect();
    canonical_hash(
        "local-recovery-plan",
        1,
        &RecoveryPlanBinding {
            schema_version: "sapphirus.local-recovery-plan.v1",
            journal_id: &plan.journal_id,
            execution_id: &plan.execution_id,
            checkpoint_id: &plan.checkpoint_id,
            workspace_target_hash: plan.workspace_target_hash,
            disposition: plan.disposition,
            observations,
            operations,
            reason: plan.reason,
        },
    )
    .map_err(DomainValidationError::from)
    .map_err(ExecutionError::from)
}

fn seal_recovery_plan(
    journal: &EffectJournal,
    disposition: RecoveryDisposition,
    observations: Vec<RecoveryPathObservation>,
    operations: Vec<RecoveryOperation>,
    reason: RecoveryReason,
) -> Result<RecoveryPlan, ExecutionError> {
    let mut plan = RecoveryPlan {
        journal_id: journal.journal_id.clone(),
        execution_id: journal.execution_id.clone(),
        checkpoint_id: journal.checkpoint_id.clone(),
        workspace_target_hash: journal.workspace_target_hash,
        disposition,
        observations,
        operations,
        plan_hash: sha256_bytes(b"unsealed-recovery-plan"),
        reason,
    };
    plan.plan_hash = recovery_plan_hash(&plan)?;
    Ok(plan)
}

struct RecoveryObservation {
    content: Option<Vec<u8>>,
    file_identity_hash: Option<Sha256Digest>,
}

fn observe_recovery_file<W: WorkspaceFileIo + ?Sized>(
    workspace: &W,
    path: &RelativeWorkspacePath,
) -> Result<RecoveryObservation, ExecutionError> {
    let observation = workspace
        .observe_recovery_file(path)
        .map_err(|_| ExecutionError::WorkspaceFailure)?;
    if observation.content.is_some() != observation.file_identity_hash.is_some() {
        return Err(ExecutionError::WorkspaceFailure);
    }
    CheckpointFileState::from_bytes(observation.content.clone())?;
    Ok(RecoveryObservation {
        content: observation.content,
        file_identity_hash: observation.file_identity_hash,
    })
}

/// Restores a deterministic recovery plan inside one identity-bound workspace
/// transaction.
///
/// # Errors
///
/// Returns [`ExecutionError`] when the plan, workspace binding, observations,
/// durable effects, or restored postconditions cannot be verified.
pub fn restore_checkpoint<W: WorkspaceFileIo>(
    workspace: &W,
    plan: &RecoveryPlan,
) -> Result<RecoveryRestoreResult, ExecutionError> {
    verify_recovery_plan(plan)?;
    if !matches!(
        plan.disposition,
        RecoveryDisposition::Complete | RecoveryDisposition::RestoreCheckpoint
    ) {
        return Err(ExecutionError::AuthorizationMismatch);
    }

    let mut scoped_result = None;
    let transaction_result = {
        let mut transaction = |scoped_workspace: &dyn WorkspaceFileIo| {
            scoped_result = Some(restore_checkpoint_scoped(scoped_workspace, plan));
            Ok(())
        };
        workspace.with_recovery_transaction(&mut transaction)
    };
    match (transaction_result, scoped_result) {
        (Ok(()), Some(result)) => result,
        (Err(_), Some(Ok(_))) => Err(ExecutionError::RecoveryRequired),
        (Err(_), Some(Err(error))) => Err(error),
        (_, None) => Err(ExecutionError::WorkspaceFailure),
    }
}

fn verify_recovery_plan(plan: &RecoveryPlan) -> Result<(), ExecutionError> {
    if plan.observations.is_empty()
        || plan.observations.windows(2).any(|pair| {
            !pair[0]
                .relative_path
                .canonical_cmp(&pair[1].relative_path)
                .is_lt()
        })
        || plan.observations.iter().any(|observation| {
            observation.current_exists != observation.current_content_hash.is_some()
                || observation.current_exists != observation.current_file_identity_hash.is_some()
                || !checkpoint_state_is_valid(&observation.restore_to)
        })
        || plan.operations.is_empty()
        || plan.operations.windows(2).any(|pair| {
            !pair[0]
                .relative_path
                .canonical_cmp(&pair[1].relative_path)
                .is_lt()
        })
        || plan.operations.iter().any(|operation| {
            operation.expected_current_exists != operation.expected_current_content_hash.is_some()
                || operation.expected_current_exists
                    != operation.expected_current_file_identity_hash.is_some()
                || !checkpoint_state_is_valid(&operation.restore_to)
                || !plan.observations.iter().any(|observation| {
                    observation.relative_path == operation.relative_path
                        && observation.current_exists == operation.expected_current_exists
                        && observation.current_content_hash
                            == operation.expected_current_content_hash
                        && observation.current_file_identity_hash
                            == operation.expected_current_file_identity_hash
                        && observation.restore_to == operation.restore_to
                })
        })
        || recovery_plan_hash(plan)? != plan.plan_hash
    {
        return Err(ExecutionError::IntegrityFailure);
    }
    Ok(())
}

fn checkpoint_state_is_valid(state: &CheckpointFileState) -> bool {
    !matches!(
        state,
        CheckpointFileState::Utf8 {
            content,
            content_hash
        } if content.contains('\0') || sha256_bytes(content.as_bytes()) != *content_hash
    )
}

fn restore_checkpoint_scoped(
    workspace: &dyn WorkspaceFileIo,
    plan: &RecoveryPlan,
) -> Result<RecoveryRestoreResult, ExecutionError> {
    if workspace
        .workspace_target_hash()
        .map_err(|_| ExecutionError::WorkspaceFailure)?
        != plan.workspace_target_hash
    {
        return Err(ExecutionError::AuthorizationMismatch);
    }

    let mut reobserved_paths = Vec::with_capacity(plan.observations.len());
    for planned in &plan.observations {
        let observation = observe_recovery_file(workspace, &planned.relative_path)?;
        reobserved_paths.push(RecoveryPathObservation {
            relative_path: planned.relative_path.clone(),
            current_exists: observation.content.is_some(),
            current_content_hash: observation.content.as_deref().map(sha256_bytes),
            current_file_identity_hash: observation.file_identity_hash,
            restore_to: planned.restore_to.clone(),
        });
    }
    let reobserved = RecoveryPlan {
        journal_id: plan.journal_id.clone(),
        execution_id: plan.execution_id.clone(),
        checkpoint_id: plan.checkpoint_id.clone(),
        workspace_target_hash: plan.workspace_target_hash,
        disposition: plan.disposition,
        observations: reobserved_paths,
        operations: plan.operations.clone(),
        plan_hash: plan.plan_hash,
        reason: plan.reason,
    };
    if recovery_plan_hash(&reobserved)? != plan.plan_hash {
        return Err(ExecutionError::PreconditionFailed);
    }

    let mut effect_started = false;
    for operation in &plan.operations {
        effect_started = true;
        let effect = match &operation.restore_to {
            CheckpointFileState::Absent => workspace.delete_durable(
                &operation.relative_path,
                operation
                    .expected_current_content_hash
                    .ok_or(ExecutionError::IntegrityFailure)?,
                operation
                    .expected_current_file_identity_hash
                    .ok_or(ExecutionError::IntegrityFailure)?,
            ),
            CheckpointFileState::Utf8 { content, .. } if operation.expected_current_exists => {
                workspace.replace_utf8_durable(
                    &operation.relative_path,
                    operation
                        .expected_current_content_hash
                        .ok_or(ExecutionError::IntegrityFailure)?,
                    operation
                        .expected_current_file_identity_hash
                        .ok_or(ExecutionError::IntegrityFailure)?,
                    content,
                )
            }
            CheckpointFileState::Utf8 { content, .. } => {
                workspace.create_utf8_durable(&operation.relative_path, content)
            }
        };
        if effect.is_err() {
            return Err(ExecutionError::RecoveryRequired);
        }
    }

    for planned in &plan.observations {
        let observed = observe_recovery_file(workspace, &planned.relative_path)
            .map_err(|_| ExecutionError::RecoveryRequired)?;
        let matches_checkpoint = match &planned.restore_to {
            CheckpointFileState::Absent => observed.content.is_none(),
            CheckpointFileState::Utf8 {
                content,
                content_hash,
            } => {
                observed.content.as_deref() == Some(content.as_bytes())
                    && observed.content.as_deref().map(sha256_bytes) == Some(*content_hash)
            }
        };
        if !matches_checkpoint {
            return Err(if effect_started {
                ExecutionError::RecoveryRequired
            } else {
                ExecutionError::PreconditionFailed
            });
        }
    }

    Ok(RecoveryRestoreResult {
        journal_id: plan.journal_id.clone(),
        restored_count: plan.operations.len(),
    })
}

fn observation_map(
    files: &[FileObservation],
) -> Result<BTreeMap<String, &FileObservation>, ExecutionError> {
    let mut map = BTreeMap::new();
    for file in files {
        if map.insert(file.relative_path.case_folded(), file).is_some() {
            return Err(ExecutionError::IntegrityFailure);
        }
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    use desktop_runtime::{sha256_bytes, ContractId, RelativeWorkspacePath, UnixMillis};

    use super::{plan_recovery, restore_checkpoint};
    use crate::{
        CheckpointEntry, CheckpointFileState, EffectJournal, ExecutionError, JournalOperation,
        JournalOperationState, JournalState, LocalCheckpoint, RecoveryDisposition, RecoveryReason,
        ResultFileOperation, WorkspaceFileIo, WorkspaceFileObservation, WorkspaceIoError,
    };

    fn must<T, E>(result: Result<T, E>) -> T {
        match result {
            Ok(value) => value,
            Err(_) => std::process::abort(),
        }
    }

    #[derive(Clone)]
    struct TestFile {
        bytes: Vec<u8>,
        identity: desktop_runtime::Sha256Digest,
    }

    #[allow(clippy::struct_excessive_bools)]
    struct RecoveryWorkspace {
        target_hash: Mutex<desktop_runtime::Sha256Digest>,
        files: Mutex<BTreeMap<String, TestFile>>,
        observations: Mutex<Vec<String>>,
        mutations: Mutex<Vec<String>>,
        fail_reads: bool,
        fail_mutation: Option<usize>,
        corrupt_postcondition: bool,
        substitute_identity_on_transaction: bool,
        fail_transaction_after_callback: bool,
    }

    impl RecoveryWorkspace {
        fn new(
            target_hash: desktop_runtime::Sha256Digest,
            files: impl IntoIterator<Item = (&'static str, &'static [u8])>,
        ) -> Self {
            Self {
                target_hash: Mutex::new(target_hash),
                files: Mutex::new(
                    files
                        .into_iter()
                        .map(|(path, bytes)| {
                            (
                                path.to_owned(),
                                TestFile {
                                    bytes: bytes.to_vec(),
                                    identity: identity(path, 1),
                                },
                            )
                        })
                        .collect(),
                ),
                observations: Mutex::new(Vec::new()),
                mutations: Mutex::new(Vec::new()),
                fail_reads: false,
                fail_mutation: None,
                corrupt_postcondition: false,
                substitute_identity_on_transaction: false,
                fail_transaction_after_callback: false,
            }
        }

        fn with_read_failure(mut self) -> Self {
            self.fail_reads = true;
            self
        }

        fn with_mutation_failure(mut self, operation: usize) -> Self {
            self.fail_mutation = Some(operation);
            self
        }

        fn with_postcondition_corruption(mut self) -> Self {
            self.corrupt_postcondition = true;
            self
        }

        fn with_identity_substitution(mut self) -> Self {
            self.substitute_identity_on_transaction = true;
            self
        }

        fn with_post_scope_failure(mut self) -> Self {
            self.fail_transaction_after_callback = true;
            self
        }

        fn with_file_identity(self, path: &str, generation: u8) -> Self {
            {
                let mut files = must(self.files.lock());
                let Some(file) = files.get_mut(path) else {
                    std::process::abort()
                };
                file.identity = identity(path, generation);
            }
            self
        }

        fn mutation_started(&self, path: &RelativeWorkspacePath) -> Result<(), WorkspaceIoError> {
            let mut mutations = self
                .mutations
                .lock()
                .map_err(|_| WorkspaceIoError::Unavailable)?;
            mutations.push(path.as_str().to_owned());
            if self.fail_mutation == Some(mutations.len()) {
                return Err(WorkspaceIoError::Unavailable);
            }
            Ok(())
        }

        fn set_target_hash(&self, hash: desktop_runtime::Sha256Digest) {
            *must(self.target_hash.lock()) = hash;
        }

        fn mutations(&self) -> Vec<String> {
            must(self.mutations.lock()).clone()
        }

        fn observation_count(&self) -> usize {
            must(self.observations.lock()).len()
        }
    }

    impl WorkspaceFileIo for RecoveryWorkspace {
        fn workspace_target_hash(&self) -> Result<desktop_runtime::Sha256Digest, WorkspaceIoError> {
            self.target_hash
                .lock()
                .map(|hash| *hash)
                .map_err(|_| WorkspaceIoError::Unavailable)
        }

        fn read_file(
            &self,
            path: &RelativeWorkspacePath,
            expected_file_identity_hash: Option<desktop_runtime::Sha256Digest>,
        ) -> Result<Option<Vec<u8>>, WorkspaceIoError> {
            let observation = self.observe_recovery_file(path)?;
            if expected_file_identity_hash.is_some()
                && expected_file_identity_hash != observation.file_identity_hash
            {
                return Err(WorkspaceIoError::CapabilityRevoked);
            }
            Ok(observation.content)
        }

        fn observe_recovery_file(
            &self,
            path: &RelativeWorkspacePath,
        ) -> Result<WorkspaceFileObservation, WorkspaceIoError> {
            self.observations
                .lock()
                .map_err(|_| WorkspaceIoError::Unavailable)?
                .push(path.as_str().to_owned());
            if self.fail_reads {
                return Err(WorkspaceIoError::Unavailable);
            }
            let corrupt = self.corrupt_postcondition
                && self
                    .mutations
                    .lock()
                    .map_err(|_| WorkspaceIoError::Unavailable)?
                    .len()
                    >= 3
                && path.as_str() == "a-modified.txt";
            let files = self
                .files
                .lock()
                .map_err(|_| WorkspaceIoError::Unavailable)?;
            Ok(files.get(path.as_str()).map_or(
                WorkspaceFileObservation {
                    content: None,
                    file_identity_hash: None,
                },
                |file| WorkspaceFileObservation {
                    content: Some(if corrupt {
                        b"corrupted after restore".to_vec()
                    } else {
                        file.bytes.clone()
                    }),
                    file_identity_hash: Some(file.identity),
                },
            ))
        }

        fn with_recovery_transaction(
            &self,
            transaction: &mut dyn FnMut(&dyn WorkspaceFileIo) -> Result<(), WorkspaceIoError>,
        ) -> Result<(), WorkspaceIoError> {
            if self.substitute_identity_on_transaction {
                let mut files = self
                    .files
                    .lock()
                    .map_err(|_| WorkspaceIoError::Unavailable)?;
                if let Some(file) = files.get_mut("a-modified.txt") {
                    file.identity = identity("a-modified.txt", 2);
                }
            }
            transaction(self)?;
            if self.fail_transaction_after_callback {
                return Err(WorkspaceIoError::Unavailable);
            }
            Ok(())
        }

        fn create_utf8_durable(
            &self,
            path: &RelativeWorkspacePath,
            content: &str,
        ) -> Result<(), WorkspaceIoError> {
            self.mutation_started(path)?;
            let mut files = self
                .files
                .lock()
                .map_err(|_| WorkspaceIoError::Unavailable)?;
            if files.contains_key(path.as_str()) {
                return Err(WorkspaceIoError::AlreadyExists);
            }
            files.insert(
                path.as_str().to_owned(),
                TestFile {
                    bytes: content.as_bytes().to_vec(),
                    identity: identity(path.as_str(), 3),
                },
            );
            Ok(())
        }

        fn replace_utf8_durable(
            &self,
            path: &RelativeWorkspacePath,
            expected_content_hash: desktop_runtime::Sha256Digest,
            expected_file_identity_hash: desktop_runtime::Sha256Digest,
            content: &str,
        ) -> Result<(), WorkspaceIoError> {
            self.mutation_started(path)?;
            let mut files = self
                .files
                .lock()
                .map_err(|_| WorkspaceIoError::Unavailable)?;
            let file = files
                .get_mut(path.as_str())
                .ok_or(WorkspaceIoError::NotFound)?;
            if sha256_bytes(&file.bytes) != expected_content_hash
                || file.identity != expected_file_identity_hash
            {
                return Err(WorkspaceIoError::CapabilityRevoked);
            }
            file.bytes = content.as_bytes().to_vec();
            file.identity = identity(path.as_str(), 3);
            Ok(())
        }

        fn delete_durable(
            &self,
            path: &RelativeWorkspacePath,
            expected_content_hash: desktop_runtime::Sha256Digest,
            expected_file_identity_hash: desktop_runtime::Sha256Digest,
        ) -> Result<(), WorkspaceIoError> {
            self.mutation_started(path)?;
            let mut files = self
                .files
                .lock()
                .map_err(|_| WorkspaceIoError::Unavailable)?;
            let file = files.get(path.as_str()).ok_or(WorkspaceIoError::NotFound)?;
            if sha256_bytes(&file.bytes) != expected_content_hash
                || file.identity != expected_file_identity_hash
            {
                return Err(WorkspaceIoError::CapabilityRevoked);
            }
            files.remove(path.as_str());
            Ok(())
        }
    }

    fn identity(path: &str, generation: u8) -> desktop_runtime::Sha256Digest {
        sha256_bytes(format!("{path}:{generation}").as_bytes())
    }

    fn id(value: &str) -> ContractId {
        must(ContractId::new(value))
    }

    fn path(value: &str) -> RelativeWorkspacePath {
        must(RelativeWorkspacePath::new(value))
    }

    fn make_checkpoint(
        target_hash: desktop_runtime::Sha256Digest,
        entries: Vec<CheckpointEntry>,
    ) -> LocalCheckpoint {
        must(LocalCheckpoint::seal(
            id("checkpoint_recovery"),
            target_hash,
            sha256_bytes(b"candidate"),
            entries,
            UnixMillis(1_000),
        ))
    }

    fn complete_checkpoint(target_hash: desktop_runtime::Sha256Digest) -> LocalCheckpoint {
        make_checkpoint(
            target_hash,
            vec![
                CheckpointEntry {
                    relative_path: path("a-modified.txt"),
                    before: must(CheckpointFileState::from_bytes(Some(
                        b"before modified".to_vec(),
                    ))),
                },
                CheckpointEntry {
                    relative_path: path("m-deleted.txt"),
                    before: must(CheckpointFileState::from_bytes(Some(
                        b"before deleted".to_vec(),
                    ))),
                },
                CheckpointEntry {
                    relative_path: path("z-created.txt"),
                    before: CheckpointFileState::Absent,
                },
            ],
        )
    }

    fn journal(checkpoint: &LocalCheckpoint, operation_paths: &[&str]) -> EffectJournal {
        let mut operations = Vec::new();
        for (index, relative_path) in operation_paths.iter().enumerate() {
            let (operation, preimage_hash, postimage_hash) = match *relative_path {
                "a-modified.txt" => (
                    ResultFileOperation::Modified,
                    Some(sha256_bytes(b"before modified")),
                    Some(sha256_bytes(b"after modified")),
                ),
                "m-deleted.txt" => (
                    ResultFileOperation::Deleted,
                    Some(sha256_bytes(b"before deleted")),
                    None,
                ),
                "z-created.txt" => (
                    ResultFileOperation::Created,
                    None,
                    Some(sha256_bytes(b"created content")),
                ),
                _ => std::process::abort(),
            };
            operations.push(JournalOperation {
                ordinal: must(u16::try_from(index + 1)),
                relative_path: path(relative_path),
                operation,
                preimage_hash,
                postimage_hash,
                state: JournalOperationState::Applied,
            });
        }
        EffectJournal {
            schema_version: "sapphirus.local-effect-journal.v1".to_owned(),
            journal_id: id("journal_recovery"),
            execution_id: id("execution_recovery"),
            candidate_hash: checkpoint.candidate_hash,
            spec_hash: sha256_bytes(b"spec"),
            consumption_hash: sha256_bytes(b"consumption"),
            checkpoint_id: checkpoint.checkpoint_id.clone(),
            workspace_target_hash: checkpoint.workspace_target_hash,
            patch_ref: format!("cas://sha256/{}", sha256_bytes(b"patch").hex_value()),
            patch_hash: sha256_bytes(b"patch"),
            state: JournalState::RecoveryRequired,
            operations,
            created_at: UnixMillis(2_000),
            updated_at: UnixMillis(3_000),
        }
    }

    fn preimage_workspace(target_hash: desktop_runtime::Sha256Digest) -> RecoveryWorkspace {
        RecoveryWorkspace::new(
            target_hash,
            [
                ("a-modified.txt", b"before modified".as_slice()),
                ("m-deleted.txt", b"before deleted".as_slice()),
            ],
        )
    }

    fn postimage_workspace(target_hash: desktop_runtime::Sha256Digest) -> RecoveryWorkspace {
        RecoveryWorkspace::new(
            target_hash,
            [
                ("a-modified.txt", b"after modified".as_slice()),
                ("z-created.txt", b"created content".as_slice()),
            ],
        )
    }

    fn partial_workspace(target_hash: desktop_runtime::Sha256Digest) -> RecoveryWorkspace {
        RecoveryWorkspace::new(
            target_hash,
            [
                ("a-modified.txt", b"partial modified".as_slice()),
                ("z-created.txt", b"created content".as_slice()),
            ],
        )
    }

    fn partial_workspace_with_unchanged_path(
        target_hash: desktop_runtime::Sha256Digest,
    ) -> RecoveryWorkspace {
        RecoveryWorkspace::new(
            target_hash,
            [
                ("a-modified.txt", b"partial modified".as_slice()),
                ("m-deleted.txt", b"before deleted".as_slice()),
                ("z-created.txt", b"created content".as_slice()),
            ],
        )
    }

    #[test]
    fn recovery_planning_classifies_verified_workspace_states() {
        let target_hash = sha256_bytes(b"workspace");
        let checkpoint = complete_checkpoint(target_hash);
        let journal = journal(
            &checkpoint,
            &["z-created.txt", "m-deleted.txt", "a-modified.txt"],
        );

        let no_effect = must(plan_recovery(
            &preimage_workspace(target_hash),
            &journal,
            &checkpoint,
        ));
        assert_eq!(no_effect.disposition, RecoveryDisposition::NoEffect);
        assert_eq!(no_effect.reason, RecoveryReason::NoEffectObserved);
        assert!(no_effect.operations.is_empty());

        let complete = must(plan_recovery(
            &postimage_workspace(target_hash),
            &journal,
            &checkpoint,
        ));
        assert_eq!(complete.disposition, RecoveryDisposition::Complete);
        assert_eq!(complete.reason, RecoveryReason::PostimagesVerified);
        assert_eq!(
            complete
                .operations
                .iter()
                .map(|operation| operation.relative_path.as_str())
                .collect::<Vec<_>>(),
            ["a-modified.txt", "m-deleted.txt", "z-created.txt"]
        );

        let restore = must(plan_recovery(
            &partial_workspace(target_hash),
            &journal,
            &checkpoint,
        ));
        assert_eq!(restore.disposition, RecoveryDisposition::RestoreCheckpoint);
        assert_eq!(restore.reason, RecoveryReason::CompleteCheckpointCoverage);
        assert_eq!(
            restore
                .operations
                .iter()
                .map(|operation| operation.relative_path.as_str())
                .collect::<Vec<_>>(),
            ["a-modified.txt", "m-deleted.txt", "z-created.txt"]
        );
        assert!(restore.operations.iter().all(|operation| checkpoint
            .entries
            .iter()
            .any(|entry| entry.relative_path == operation.relative_path)));
    }

    #[test]
    fn recovery_planning_is_canonical_across_journal_ordering() {
        let target_hash = sha256_bytes(b"workspace");
        let checkpoint = complete_checkpoint(target_hash);
        let forward = journal(
            &checkpoint,
            &["a-modified.txt", "m-deleted.txt", "z-created.txt"],
        );
        let reverse = journal(
            &checkpoint,
            &["z-created.txt", "m-deleted.txt", "a-modified.txt"],
        );

        let first = must(plan_recovery(
            &partial_workspace(target_hash),
            &forward,
            &checkpoint,
        ));
        let second = must(plan_recovery(
            &partial_workspace(target_hash),
            &reverse,
            &checkpoint,
        ));
        assert_eq!(first.plan_hash, second.plan_hash);
        assert_eq!(first.operations, second.operations);
    }

    #[test]
    fn recovery_no_effect_hash_binds_every_file_identity() {
        let target_hash = sha256_bytes(b"workspace");
        let checkpoint = complete_checkpoint(target_hash);
        let journal = journal(
            &checkpoint,
            &["z-created.txt", "m-deleted.txt", "a-modified.txt"],
        );
        let baseline = must(plan_recovery(
            &preimage_workspace(target_hash),
            &journal,
            &checkpoint,
        ));
        let identity_changed = must(plan_recovery(
            &preimage_workspace(target_hash).with_file_identity("a-modified.txt", 2),
            &journal,
            &checkpoint,
        ));

        assert_eq!(baseline.disposition, RecoveryDisposition::NoEffect);
        assert!(baseline.operations.is_empty());
        assert_ne!(baseline.plan_hash, identity_changed.plan_hash);
    }

    #[test]
    fn recovery_complete_hash_binds_every_file_identity() {
        let target_hash = sha256_bytes(b"workspace");
        let checkpoint = complete_checkpoint(target_hash);
        let journal = journal(
            &checkpoint,
            &["m-deleted.txt", "a-modified.txt", "z-created.txt"],
        );
        let baseline = must(plan_recovery(
            &postimage_workspace(target_hash),
            &journal,
            &checkpoint,
        ));
        let identity_changed = must(plan_recovery(
            &postimage_workspace(target_hash).with_file_identity("z-created.txt", 2),
            &journal,
            &checkpoint,
        ));

        assert_eq!(baseline.disposition, RecoveryDisposition::Complete);
        assert_eq!(baseline.operations.len(), 3);
        assert_ne!(baseline.plan_hash, identity_changed.plan_hash);
    }

    #[test]
    fn recovery_restore_hash_binds_unchanged_identity_in_canonical_order() {
        let target_hash = sha256_bytes(b"workspace");
        let checkpoint = complete_checkpoint(target_hash);
        let journal = journal(
            &checkpoint,
            &["z-created.txt", "a-modified.txt", "m-deleted.txt"],
        );
        let baseline = must(plan_recovery(
            &partial_workspace_with_unchanged_path(target_hash),
            &journal,
            &checkpoint,
        ));
        let identity_changed = must(plan_recovery(
            &partial_workspace_with_unchanged_path(target_hash)
                .with_file_identity("m-deleted.txt", 2),
            &journal,
            &checkpoint,
        ));

        assert_eq!(baseline.disposition, RecoveryDisposition::RestoreCheckpoint);
        assert_eq!(
            baseline
                .operations
                .iter()
                .map(|operation| operation.relative_path.as_str())
                .collect::<Vec<_>>(),
            ["a-modified.txt", "z-created.txt"]
        );
        assert_eq!(baseline.operations, identity_changed.operations);
        assert_ne!(baseline.plan_hash, identity_changed.plan_hash);
        assert_eq!(
            baseline
                .observations
                .iter()
                .map(|observation| observation.relative_path.as_str())
                .collect::<Vec<_>>(),
            ["a-modified.txt", "m-deleted.txt", "z-created.txt"]
        );
    }

    #[test]
    fn recovery_planning_fails_closed_for_incomplete_or_tampered_inputs() {
        let target_hash = sha256_bytes(b"workspace");
        let checkpoint = complete_checkpoint(target_hash);
        let journal = journal(
            &checkpoint,
            &["a-modified.txt", "m-deleted.txt", "z-created.txt"],
        );
        let incomplete = make_checkpoint(target_hash, checkpoint.entries[..2].to_vec());
        let manual = must(plan_recovery(
            &partial_workspace(target_hash),
            &journal,
            &incomplete,
        ));
        assert_eq!(manual.disposition, RecoveryDisposition::ManualReview);
        assert_eq!(manual.reason, RecoveryReason::IncompleteCheckpointCoverage);
        assert!(manual.operations.is_empty());

        let mut mismatched = journal.clone();
        mismatched.checkpoint_id = id("other_checkpoint");
        assert!(matches!(
            plan_recovery(&partial_workspace(target_hash), &mismatched, &checkpoint),
            Err(ExecutionError::AuthorizationMismatch)
        ));

        let mut bad_hash = journal.clone();
        bad_hash.operations[0].preimage_hash = Some(sha256_bytes(b"tampered"));
        assert!(matches!(
            plan_recovery(&partial_workspace(target_hash), &bad_hash, &checkpoint),
            Err(ExecutionError::IntegrityFailure)
        ));

        let mut tampered_checkpoint = checkpoint.clone();
        let CheckpointFileState::Utf8 { content_hash, .. } =
            &mut tampered_checkpoint.entries[0].before
        else {
            std::process::abort()
        };
        *content_hash = sha256_bytes(b"tampered");
        assert!(matches!(
            plan_recovery(
                &partial_workspace(target_hash),
                &journal,
                &tampered_checkpoint
            ),
            Err(ExecutionError::IntegrityFailure)
        ));

        assert!(matches!(
            plan_recovery(
                &partial_workspace(sha256_bytes(b"other workspace")),
                &journal,
                &checkpoint
            ),
            Err(ExecutionError::AuthorizationMismatch)
        ));
        assert!(matches!(
            plan_recovery(
                &RecoveryWorkspace::new(target_hash, [("a-modified.txt", &[0xff_u8][..])]),
                &journal,
                &checkpoint
            ),
            Err(ExecutionError::UnsupportedContent)
        ));
        assert!(matches!(
            plan_recovery(
                &partial_workspace(target_hash).with_read_failure(),
                &journal,
                &checkpoint
            ),
            Err(ExecutionError::WorkspaceFailure)
        ));
    }

    #[test]
    fn recovery_rejects_case_alias_before_workspace_observation() {
        let target_hash = sha256_bytes(b"workspace");
        let checkpoint = complete_checkpoint(target_hash);
        let mut aliased = journal(
            &checkpoint,
            &["a-modified.txt", "m-deleted.txt", "z-created.txt"],
        );
        aliased.operations[0].relative_path = path("A-modified.txt");
        let workspace = partial_workspace(target_hash);

        assert!(matches!(
            plan_recovery(&workspace, &aliased, &checkpoint),
            Err(ExecutionError::IntegrityFailure)
        ));
        assert_eq!(workspace.observation_count(), 0);
        assert!(workspace.mutations().is_empty());
    }

    #[test]
    fn recovery_rejects_duplicate_folded_checkpoint_paths_before_observation() {
        let target_hash = sha256_bytes(b"workspace");
        let original = complete_checkpoint(target_hash);
        let mut upper = original.entries[0].clone();
        upper.relative_path = path("Foo.txt");
        let mut lower = original.entries[1].clone();
        lower.relative_path = path("foo.txt");
        let duplicate = make_checkpoint(target_hash, vec![upper, lower]);
        let journal = journal(
            &duplicate,
            &["a-modified.txt", "m-deleted.txt", "z-created.txt"],
        );
        let workspace = partial_workspace(target_hash);

        assert!(matches!(
            plan_recovery(&workspace, &journal, &duplicate),
            Err(ExecutionError::IntegrityFailure)
        ));
        assert_eq!(workspace.observation_count(), 0);
        assert!(workspace.mutations().is_empty());
    }

    #[test]
    fn recovery_rejects_duplicate_folded_journal_paths_before_observation() {
        let target_hash = sha256_bytes(b"workspace");
        let checkpoint = complete_checkpoint(target_hash);
        let mut duplicate = journal(
            &checkpoint,
            &["a-modified.txt", "m-deleted.txt", "z-created.txt"],
        );
        duplicate.operations[2].relative_path = path("A-modified.txt");
        let workspace = partial_workspace(target_hash);

        assert!(matches!(
            plan_recovery(&workspace, &duplicate, &checkpoint),
            Err(ExecutionError::IntegrityFailure)
        ));
        assert_eq!(workspace.observation_count(), 0);
        assert!(workspace.mutations().is_empty());
    }

    #[test]
    fn recovery_rejects_exact_path_mismatch_before_workspace_observation() {
        let target_hash = sha256_bytes(b"workspace");
        let checkpoint = complete_checkpoint(target_hash);
        let mut mismatched = journal(
            &checkpoint,
            &["a-modified.txt", "m-deleted.txt", "z-created.txt"],
        );
        mismatched.operations[0].relative_path = path("other-modified.txt");
        let workspace = partial_workspace(target_hash);

        assert!(matches!(
            plan_recovery(&workspace, &mismatched, &checkpoint),
            Err(ExecutionError::IntegrityFailure)
        ));
        assert_eq!(workspace.observation_count(), 0);
        assert!(workspace.mutations().is_empty());
    }

    #[test]
    fn restore_checkpoint_restores_create_replace_and_delete_in_canonical_order() {
        let target_hash = sha256_bytes(b"workspace");
        let checkpoint = complete_checkpoint(target_hash);
        let journal = journal(
            &checkpoint,
            &["z-created.txt", "a-modified.txt", "m-deleted.txt"],
        );
        let workspace = partial_workspace(target_hash);
        let plan = must(plan_recovery(&workspace, &journal, &checkpoint));

        let result = must(restore_checkpoint(&workspace, &plan));
        assert_eq!(result.journal_id, journal.journal_id);
        assert_eq!(result.restored_count, 3);
        assert_eq!(
            workspace.mutations(),
            ["a-modified.txt", "m-deleted.txt", "z-created.txt"]
        );
        assert_eq!(
            must(workspace.read_file(&path("a-modified.txt"), None)),
            Some(b"before modified".to_vec())
        );
        assert_eq!(
            must(workspace.read_file(&path("m-deleted.txt"), None)),
            Some(b"before deleted".to_vec())
        );
        assert_eq!(
            must(workspace.read_file(&path("z-created.txt"), None)),
            None
        );
    }

    #[test]
    fn restore_checkpoint_rejects_target_or_identity_drift_before_mutation() {
        let target_hash = sha256_bytes(b"workspace");
        let checkpoint = complete_checkpoint(target_hash);
        let journal = journal(
            &checkpoint,
            &["a-modified.txt", "m-deleted.txt", "z-created.txt"],
        );

        let target_drift = partial_workspace(target_hash);
        let target_plan = must(plan_recovery(&target_drift, &journal, &checkpoint));
        target_drift.set_target_hash(sha256_bytes(b"other workspace"));
        assert!(matches!(
            restore_checkpoint(&target_drift, &target_plan),
            Err(ExecutionError::AuthorizationMismatch)
        ));
        assert!(target_drift.mutations().is_empty());

        let identity_drift = partial_workspace(target_hash).with_identity_substitution();
        let identity_plan = must(plan_recovery(&identity_drift, &journal, &checkpoint));
        assert!(matches!(
            restore_checkpoint(&identity_drift, &identity_plan),
            Err(ExecutionError::PreconditionFailed)
        ));
        assert!(identity_drift.mutations().is_empty());
    }

    #[test]
    fn restore_checkpoint_reports_ambiguous_failures_after_effects_start() {
        let target_hash = sha256_bytes(b"workspace");
        let checkpoint = complete_checkpoint(target_hash);
        let journal = journal(
            &checkpoint,
            &["a-modified.txt", "m-deleted.txt", "z-created.txt"],
        );

        let mid_restore = partial_workspace(target_hash).with_mutation_failure(2);
        let mid_plan = must(plan_recovery(&mid_restore, &journal, &checkpoint));
        assert!(matches!(
            restore_checkpoint(&mid_restore, &mid_plan),
            Err(ExecutionError::RecoveryRequired)
        ));

        let corrupt = partial_workspace(target_hash).with_postcondition_corruption();
        let corrupt_plan = must(plan_recovery(&corrupt, &journal, &checkpoint));
        assert!(matches!(
            restore_checkpoint(&corrupt, &corrupt_plan),
            Err(ExecutionError::RecoveryRequired)
        ));

        let post_scope_failure = partial_workspace(target_hash).with_post_scope_failure();
        let post_scope_plan = must(plan_recovery(&post_scope_failure, &journal, &checkpoint));
        assert!(matches!(
            restore_checkpoint(&post_scope_failure, &post_scope_plan),
            Err(ExecutionError::RecoveryRequired)
        ));
    }
}
