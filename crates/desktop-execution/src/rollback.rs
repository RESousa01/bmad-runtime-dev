use std::collections::BTreeMap;

use desktop_runtime::{
    canonical_hash, canonical_hash_without_field, sha256_bytes, ContractId, DomainValidationError,
    PatchOperation, PatchSet, RelativeWorkspacePath, Sha256Digest, UnixMillis,
};
use serde::{Deserialize, Serialize};

use crate::{
    CheckpointFileState, EffectJournal, ExecutionError, FileObservation, JournalState,
    LocalCheckpoint, LocalExecutionResult, RecoveryDisposition, RecoveryPlan, WorkspaceFileIo,
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

    if matches!(
        journal.state,
        JournalState::Prepared
            | JournalState::CheckpointDurable
            | JournalState::PreconditionsVerified
    ) && journal
        .operations
        .iter()
        .all(|operation| operation.state == crate::JournalOperationState::Pending)
    {
        return Ok(RecoveryPlan {
            journal_id: journal.journal_id.clone(),
            disposition: RecoveryDisposition::NoEffect,
            reason: "journal contains no started file operation",
        });
    }
    if matches!(
        journal.state,
        JournalState::Completed | JournalState::ResultRecorded
    ) {
        return Ok(RecoveryPlan {
            journal_id: journal.journal_id.clone(),
            disposition: RecoveryDisposition::Complete,
            reason: "host result is already durably recorded",
        });
    }

    let checkpoint_entries: BTreeMap<_, _> = checkpoint
        .entries
        .iter()
        .map(|entry| (entry.relative_path.case_folded(), entry))
        .collect();
    let mut all_preimages = true;
    let mut all_postimages = true;
    for operation in &journal.operations {
        let checkpoint_entry = checkpoint_entries
            .get(&operation.relative_path.case_folded())
            .ok_or(ExecutionError::IntegrityFailure)?;
        let current = workspace
            .read_file(&operation.relative_path, None)
            .map_err(|_| ExecutionError::WorkspaceFailure)?;
        let current_exists = current.is_some();
        let current_hash = current.as_deref().map(sha256_bytes);
        all_preimages &= current_exists == checkpoint_entry.before.exists()
            && current_hash == checkpoint_entry.before.content_hash();
        all_postimages &= current_exists == operation.postimage_hash.is_some()
            && current_hash == operation.postimage_hash;
    }

    let (disposition, reason) = if all_postimages {
        (
            RecoveryDisposition::Complete,
            "all declared postimages are present and can be verified",
        )
    } else if all_preimages {
        (
            RecoveryDisposition::NoEffect,
            "all checkpoint preimages remain present",
        )
    } else if checkpoint.entries.len() == journal.operations.len() {
        (
            RecoveryDisposition::RestoreCheckpoint,
            "journal is partial but every declared path has checkpoint coverage",
        )
    } else {
        (
            RecoveryDisposition::ManualReview,
            "observed files cannot be reconciled from complete checkpoint coverage",
        )
    };
    Ok(RecoveryPlan {
        journal_id: journal.journal_id.clone(),
        disposition,
        reason,
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
