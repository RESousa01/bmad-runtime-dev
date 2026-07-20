//! Desktop-host composition for reviewed recovery authority.

use std::collections::{HashMap, VecDeque};

use desktop_execution::{
    plan_recovery, restore_checkpoint, EffectJournal, ExecutionError, JournalState,
    LocalCheckpoint, RecoveryDisposition, RecoveryOperation, RecoveryPlan,
};
use desktop_ipc::deserialize_strict;
use desktop_runtime::{
    canonical_hash, sha256_bytes, ContractId, LocalError, ProjectionEventKind,
    RecoveryApprovalChoice, Sha256Digest, UnixMillis,
};
use desktop_store::{EffectJournalRow, EvidenceAppend, LocalStore};
use serde::Serialize;
use ulid::Ulid;

use crate::edits::{current_workspace_target_hash, GovernedWorkspaceIo};
use crate::state::{
    conflict_error, not_found_error, recovery_error, HostState, ReadyAuthorityGuard,
    RendererSessionGuard,
};
use crate::wire::{
    ChangesRecoveryDecisionWire, ChangesRecoveryPreparedWire, HostCommandData,
    RecoveryManualReviewReasonWire, RecoveryOperationSummaryWire,
};

const MAX_PENDING_RECOVERIES: usize = 32;
const RECOVERY_REVIEW_WINDOW_MS: u64 = 2 * 60 * 1000;

pub(crate) struct PendingRecovery {
    approval_id: ContractId,
    installation_id: ContractId,
    renderer_session_id: ContractId,
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    journal_workspace_grant_epoch: u64,
    journal_id: ContractId,
    execution_id: ContractId,
    checkpoint_id: ContractId,
    journal_snapshot_hash: Sha256Digest,
    checkpoint_payload_hash: Sha256Digest,
    plan: RecoveryPlan,
    displayed_recovery_hash: Sha256Digest,
    expires_at: UnixMillis,
}

#[derive(Default)]
pub(crate) struct PendingRecoveries {
    values: HashMap<ContractId, PendingRecovery>,
    order: VecDeque<ContractId>,
}

impl PendingRecoveries {
    pub(crate) fn insert(&mut self, pending: PendingRecovery) {
        let replaced = self.values.iter().find_map(|(approval_id, retained)| {
            (retained.journal_id == pending.journal_id).then(|| approval_id.clone())
        });
        if let Some(replaced) = replaced {
            self.order.retain(|approval_id| approval_id != &replaced);
            self.values.remove(&replaced);
        }
        while self.order.len() >= MAX_PENDING_RECOVERIES {
            if let Some(evicted) = self.order.pop_front() {
                self.values.remove(&evicted);
            }
        }
        let approval_id = pending.approval_id.clone();
        self.order.push_back(approval_id.clone());
        self.values.insert(approval_id, pending);
    }

    pub(crate) fn take(&mut self, approval_id: &ContractId) -> Option<PendingRecovery> {
        self.order.retain(|retained| retained != approval_id);
        self.values.remove(approval_id)
    }

    pub(crate) fn invalidate_all(&mut self) {
        self.values.clear();
        self.order.clear();
    }
}

struct AuthenticatedRecovery {
    journal: EffectJournal,
    checkpoint: LocalCheckpoint,
    journal_workspace_grant_epoch: u64,
    journal_snapshot_hash: Sha256Digest,
    checkpoint_payload_hash: Sha256Digest,
}

struct ReauthenticatedRecovery<'a> {
    commit: crate::state::ReadyWorkspaceCommitGuard<'a>,
    authenticated: AuthenticatedRecovery,
    plan: RecoveryPlan,
    root_identity_hash: Sha256Digest,
    workspace_target_hash: Sha256Digest,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DisplayedRecoveryBinding<'a> {
    schema_version: &'static str,
    journal_id: &'a ContractId,
    execution_id: &'a ContractId,
    plan_hash: Sha256Digest,
    operations: &'a [RecoveryOperationSummaryWire],
    expires_at: UnixMillis,
}

#[expect(
    clippy::too_many_lines,
    reason = "the closed preparation flow keeps authentication, classification, and authority minting visible together"
)]
pub(crate) fn prepare_recovery(
    state: &HostState,
    renderer: &RendererSessionGuard<'_>,
    workspace_id: &ContractId,
    workspace_grant_epoch: u64,
    journal_id: &ContractId,
    accepted_at: UnixMillis,
) -> Result<HostCommandData, LocalError> {
    let commit = state.ready_workspace_commit()?;
    let scope = state
        .workspace
        .authorize_scope(workspace_id.as_str(), workspace_grant_epoch)
        .map_err(|error| crate::commands::map_workspace_error(&error))?;
    let binding = scope.authority_binding();
    let projection = scope.projection();
    if binding.workspace_id != workspace_id.as_str()
        || binding.grant_epoch != workspace_grant_epoch
        || projection.permissions != desktop_workspace::WorkspacePermissions::GovernedEdits
    {
        return Err(conflict_error(
            "Recovery authority changed; reopen the exact workspace and review recovery again.",
        ));
    }
    let root_identity_hash =
        Sha256Digest::parse(&binding.root_identity_hash).map_err(|_| recovery_error())?;
    drop(scope);

    let store = state.local_store(commit.authority())?;
    let authenticated = match load_authenticated_recovery(store, workspace_id, journal_id) {
        Ok(authenticated) => authenticated,
        Err(LoadRecoveryError::Unavailable(error)) => return Err(error),
        Err(LoadRecoveryError::Unsafe(row)) => {
            return persist_unsafe_manual_review(store, &row, accepted_at)
        }
    };
    let workspace_target_hash = current_workspace_target_hash(
        store,
        workspace_id,
        authenticated.journal_workspace_grant_epoch,
        root_identity_hash,
    )?;
    if !constant_time_digest_eq(
        &workspace_target_hash,
        &authenticated.checkpoint.workspace_target_hash,
    ) {
        return Err(conflict_error(
            "Recovery belongs to a different workspace target.",
        ));
    }
    let live_binding = state
        .workspace
        .authority_binding(workspace_id.as_str())
        .map_err(|error| crate::commands::map_workspace_error(&error))?;
    let io = GovernedWorkspaceIo::new(
        &state.workspace,
        workspace_id,
        workspace_grant_epoch,
        live_binding.governed_edit_epoch,
        root_identity_hash,
        workspace_target_hash,
    );
    let plan = match plan_recovery(&io, &authenticated.journal, &authenticated.checkpoint) {
        Ok(plan) => plan,
        Err(
            ExecutionError::IntegrityFailure
            | ExecutionError::AuthorizationMismatch
            | ExecutionError::InvalidDomain(_),
        ) => return persist_manual_review(store, authenticated, accepted_at),
        Err(_) => return Err(conflict_error(
            "Recovery is not available from the current workspace observation; review it again.",
        )),
    };

    match plan.disposition {
        RecoveryDisposition::NoEffect => {
            let journal_id = authenticated.journal.journal_id.clone();
            let execution_id = authenticated.journal.execution_id.clone();
            transition_journal(
                store,
                authenticated.journal,
                JournalState::Recovered,
                "execution.recovery-satisfied",
                "recovery already satisfied",
                accepted_at,
            )?;
            Ok(HostCommandData::ChangesRecoveryPrepared(
                ChangesRecoveryPreparedWire::AlreadyRecovered {
                    journal_id,
                    execution_id,
                },
            ))
        }
        RecoveryDisposition::ManualReview => {
            persist_manual_review(store, authenticated, accepted_at)
        }
        RecoveryDisposition::Complete | RecoveryDisposition::RestoreCheckpoint => {
            let operations = plan
                .operations
                .iter()
                .map(operation_summary)
                .collect::<Vec<_>>();
            let expires_at = UnixMillis(accepted_at.0.saturating_add(RECOVERY_REVIEW_WINDOW_MS));
            let displayed_recovery_hash = displayed_recovery_hash(&plan, &operations, expires_at)?;
            let approval_id = new_id("recovery_approval")?;
            let installation_id = state
                .local_identity(commit.authority())
                .map_err(|_| recovery_error())?
                .installation_id()
                .clone();
            let response = ChangesRecoveryPreparedWire::ReviewRequired {
                recovery_approval_id: approval_id.clone(),
                displayed_recovery_hash,
                journal_id: plan.journal_id.clone(),
                execution_id: plan.execution_id.clone(),
                operations,
                expires_at,
            };
            state.insert_pending_recovery(PendingRecovery {
                approval_id,
                installation_id,
                renderer_session_id: renderer.session_id().clone(),
                workspace_id: workspace_id.clone(),
                workspace_grant_epoch,
                journal_workspace_grant_epoch: authenticated.journal_workspace_grant_epoch,
                journal_id: plan.journal_id.clone(),
                execution_id: plan.execution_id.clone(),
                checkpoint_id: plan.checkpoint_id.clone(),
                journal_snapshot_hash: authenticated.journal_snapshot_hash,
                checkpoint_payload_hash: authenticated.checkpoint_payload_hash,
                plan,
                displayed_recovery_hash,
                expires_at,
            });
            Ok(HostCommandData::ChangesRecoveryPrepared(response))
        }
    }
}

pub(crate) fn decide_recovery(
    state: &HostState,
    renderer: &RendererSessionGuard<'_>,
    approval_id: &ContractId,
    displayed_recovery_hash: Sha256Digest,
    choice: RecoveryApprovalChoice,
    accepted_at: UnixMillis,
) -> Result<HostCommandData, LocalError> {
    let Some(pending) = state.take_pending_recovery(approval_id) else {
        return Err(not_found_error(
            "The recovery review is no longer available; prepare it again.",
        ));
    };
    if pending.approval_id != *approval_id
        || pending.renderer_session_id != *renderer.session_id()
        || accepted_at > pending.expires_at
        || !constant_time_digest_eq(&displayed_recovery_hash, &pending.displayed_recovery_hash)
    {
        return Err(conflict_error(
            "The recovery review expired or no longer matches; prepare it again.",
        ));
    }

    let reauthenticated = reauthenticate_pending_recovery(state, renderer, &pending)?;
    match choice {
        RecoveryApprovalChoice::Cancel => Ok(HostCommandData::ChangesRecoveryDecision(
            ChangesRecoveryDecisionWire {
                recovery_approval_id: approval_id.clone(),
                disposition: "cancelled".to_owned(),
                journal_id: pending.journal_id,
                execution_id: pending.execution_id,
                restored_files: 0,
            },
        )),
        RecoveryApprovalChoice::Restore => {
            restore_pending_recovery(state, approval_id, pending, reauthenticated, accepted_at)
        }
    }
}

fn reauthenticate_pending_recovery<'a>(
    state: &'a HostState,
    renderer: &RendererSessionGuard<'_>,
    pending: &PendingRecovery,
) -> Result<ReauthenticatedRecovery<'a>, LocalError> {
    let commit = state.ready_workspace_commit()?;
    validate_pending_authority(state, commit.authority(), renderer, pending)?;
    let store = state.local_store(commit.authority())?;
    let Ok(authenticated) =
        load_authenticated_recovery(store, &pending.workspace_id, &pending.journal_id)
    else {
        return Err(conflict_error(
            "Recovery authority changed after review; prepare it again.",
        ));
    };
    if authenticated.journal.execution_id != pending.execution_id
        || authenticated.journal.checkpoint_id != pending.checkpoint_id
        || authenticated.journal_workspace_grant_epoch != pending.journal_workspace_grant_epoch
        || !constant_time_digest_eq(
            &authenticated.journal_snapshot_hash,
            &pending.journal_snapshot_hash,
        )
        || !constant_time_digest_eq(
            &authenticated.checkpoint_payload_hash,
            &pending.checkpoint_payload_hash,
        )
    {
        return Err(conflict_error(
            "Recovery authority changed after review; prepare it again.",
        ));
    }

    let (root_identity_hash, workspace_target_hash) = live_workspace_target_binding(
        state,
        store,
        &pending.workspace_id,
        pending.workspace_grant_epoch,
        authenticated.journal_workspace_grant_epoch,
    )?;
    if !constant_time_digest_eq(
        &workspace_target_hash,
        &authenticated.checkpoint.workspace_target_hash,
    ) {
        return Err(conflict_error(
            "Recovery belongs to a different workspace target.",
        ));
    }
    let live_binding = state
        .workspace
        .authority_binding(pending.workspace_id.as_str())
        .map_err(|error| crate::commands::map_workspace_error(&error))?;
    let io = GovernedWorkspaceIo::new(
        &state.workspace,
        &pending.workspace_id,
        pending.workspace_grant_epoch,
        live_binding.governed_edit_epoch,
        root_identity_hash,
        workspace_target_hash,
    );
    let fresh_plan = plan_recovery(&io, &authenticated.journal, &authenticated.checkpoint)
        .map_err(|_| {
            conflict_error("The workspace changed after review; prepare recovery again.")
        })?;
    if !matches!(
        fresh_plan.disposition,
        RecoveryDisposition::Complete | RecoveryDisposition::RestoreCheckpoint
    ) || !constant_time_digest_eq(&fresh_plan.plan_hash, &pending.plan.plan_hash)
    {
        return Err(conflict_error(
            "The workspace changed after review; prepare recovery again.",
        ));
    }
    let fresh_operations = fresh_plan
        .operations
        .iter()
        .map(operation_summary)
        .collect::<Vec<_>>();
    let fresh_displayed_hash =
        displayed_recovery_hash(&fresh_plan, &fresh_operations, pending.expires_at)?;
    if !constant_time_digest_eq(&fresh_displayed_hash, &pending.displayed_recovery_hash) {
        return Err(conflict_error(
            "The recovery review no longer matches; prepare it again.",
        ));
    }
    Ok(ReauthenticatedRecovery {
        commit,
        authenticated,
        plan: fresh_plan,
        root_identity_hash,
        workspace_target_hash,
    })
}

fn restore_pending_recovery(
    state: &HostState,
    approval_id: &ContractId,
    pending: PendingRecovery,
    reauthenticated: ReauthenticatedRecovery<'_>,
    accepted_at: UnixMillis,
) -> Result<HostCommandData, LocalError> {
    let ReauthenticatedRecovery {
        commit,
        authenticated,
        plan,
        root_identity_hash,
        workspace_target_hash,
    } = reauthenticated;
    let store = state.local_store(commit.authority())?;
    let live_binding = state
        .workspace
        .authority_binding(pending.workspace_id.as_str())
        .map_err(|error| crate::commands::map_workspace_error(&error))?;
    let io = GovernedWorkspaceIo::new(
        &state.workspace,
        &pending.workspace_id,
        pending.workspace_grant_epoch,
        live_binding.governed_edit_epoch,
        root_identity_hash,
        workspace_target_hash,
    );
    let restoring = transition_journal(
        store,
        authenticated.journal,
        JournalState::Restoring,
        "execution.recovery-restore-started",
        "reviewed recovery restore started",
        accepted_at,
    )?;
    let Ok(restored) = restore_checkpoint(&io, &plan) else {
        mark_restore_manual_review(store, restoring, accepted_at);
        return Err(recovery_error());
    };
    if transition_journal(
        store,
        restoring.clone(),
        JournalState::Recovered,
        "execution.recovery-restore-completed",
        "reviewed recovery restore completed",
        accepted_at,
    )
    .is_err()
    {
        mark_restore_manual_review(store, restoring, accepted_at);
        return Err(recovery_error());
    }

    state.record_event(ProjectionEventKind::ExecutionStateChanged {
        execution_id: pending.execution_id.clone(),
        state: "recovered".to_owned(),
    });
    state.record_event(ProjectionEventKind::CheckpointChanged {
        checkpoint_id: pending.checkpoint_id,
        rollback_available: false,
    });
    Ok(HostCommandData::ChangesRecoveryDecision(
        ChangesRecoveryDecisionWire {
            recovery_approval_id: approval_id.clone(),
            disposition: "recovered".to_owned(),
            journal_id: pending.journal_id,
            execution_id: pending.execution_id,
            restored_files: u32::try_from(restored.restored_count).unwrap_or(u32::MAX),
        },
    ))
}

fn validate_pending_authority(
    state: &HostState,
    authority: &ReadyAuthorityGuard<'_>,
    renderer: &RendererSessionGuard<'_>,
    pending: &PendingRecovery,
) -> Result<(), LocalError> {
    let identity = state
        .local_identity(authority)
        .map_err(|_| recovery_error())?;
    if identity.installation_id() != &pending.installation_id
        || renderer.session_id() != &pending.renderer_session_id
    {
        return Err(conflict_error(
            "Recovery authority changed; prepare recovery again.",
        ));
    }
    let scope = state
        .workspace
        .authorize_scope(pending.workspace_id.as_str(), pending.workspace_grant_epoch)
        .map_err(|error| crate::commands::map_workspace_error(&error))?;
    if scope.projection().permissions != desktop_workspace::WorkspacePermissions::GovernedEdits {
        return Err(conflict_error(
            "Recovery authority changed; prepare recovery again.",
        ));
    }
    Ok(())
}

fn live_workspace_target_binding(
    state: &HostState,
    store: &LocalStore,
    workspace_id: &ContractId,
    workspace_grant_epoch: u64,
    journal_workspace_grant_epoch: u64,
) -> Result<(Sha256Digest, Sha256Digest), LocalError> {
    let binding = state
        .workspace
        .authority_binding(workspace_id.as_str())
        .map_err(|error| crate::commands::map_workspace_error(&error))?;
    if binding.workspace_id != workspace_id.as_str() || binding.grant_epoch != workspace_grant_epoch
    {
        return Err(conflict_error(
            "Recovery authority changed; prepare recovery again.",
        ));
    }
    let root_identity_hash =
        Sha256Digest::parse(&binding.root_identity_hash).map_err(|_| recovery_error())?;
    let workspace_target_hash = current_workspace_target_hash(
        store,
        workspace_id,
        journal_workspace_grant_epoch,
        root_identity_hash,
    )?;
    Ok((root_identity_hash, workspace_target_hash))
}

fn displayed_recovery_hash(
    plan: &RecoveryPlan,
    operations: &[RecoveryOperationSummaryWire],
    expires_at: UnixMillis,
) -> Result<Sha256Digest, LocalError> {
    canonical_hash(
        "displayed-recovery-review",
        1,
        &DisplayedRecoveryBinding {
            schema_version: "sapphirus.displayed-recovery-review.v1",
            journal_id: &plan.journal_id,
            execution_id: &plan.execution_id,
            plan_hash: plan.plan_hash,
            operations,
            expires_at,
        },
    )
    .map_err(|_| recovery_error())
}

fn constant_time_digest_eq(left: &Sha256Digest, right: &Sha256Digest) -> bool {
    left.as_bytes()
        .iter()
        .zip(right.as_bytes())
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

fn mark_restore_manual_review(
    store: &LocalStore,
    restoring: EffectJournal,
    accepted_at: UnixMillis,
) {
    let _ = transition_journal(
        store,
        restoring,
        JournalState::ManualReview,
        "execution.recovery-restore-failed",
        "reviewed recovery restore requires manual review",
        accepted_at,
    );
}

enum LoadRecoveryError {
    Unavailable(LocalError),
    Unsafe(Box<EffectJournalRow>),
}

fn load_authenticated_recovery(
    store: &LocalStore,
    workspace_id: &ContractId,
    journal_id: &ContractId,
) -> Result<AuthenticatedRecovery, LoadRecoveryError> {
    let row = store
        .load_effect_journal(journal_id.as_str())
        .map_err(|_| LoadRecoveryError::Unavailable(recovery_error()))?
        .ok_or_else(|| {
            LoadRecoveryError::Unavailable(not_found_error(
                "Recovery is no longer available for this journal.",
            ))
        })?;
    if row.workspace_id != workspace_id.as_str() {
        return Err(LoadRecoveryError::Unavailable(conflict_error(
            "Recovery belongs to a different workspace authority.",
        )));
    }
    if row.state != "recovery_required" {
        return Err(LoadRecoveryError::Unavailable(conflict_error(
            "Recovery is no longer available for this journal.",
        )));
    }

    let mut journal: EffectJournal = deserialize_strict(row.journal_json.as_bytes())
        .map_err(|_| LoadRecoveryError::Unsafe(Box::new(row.clone())))?;
    if journal.verify_plan().is_err()
        || journal.journal_id.as_str() != row.journal_id
        || journal.execution_id.as_str() != row.execution_id
        || journal.checkpoint_id.as_str() != row.checkpoint_id
        || journal.candidate_hash.to_string() != row.candidate_hash
        || journal.spec_hash.to_string() != row.spec_hash
        || journal.consumption_hash.to_string() != row.consumption_hash
    {
        return Err(LoadRecoveryError::Unsafe(Box::new(row)));
    }
    journal.state = JournalState::RecoveryRequired;

    let (checkpoint_row, checkpoint_bytes) = store
        .load_execution_checkpoint(&row.checkpoint_id)
        .map_err(|_| LoadRecoveryError::Unavailable(recovery_error()))?
        .ok_or_else(|| LoadRecoveryError::Unsafe(Box::new(row.clone())))?;
    let checkpoint: LocalCheckpoint = deserialize_strict(&checkpoint_bytes)
        .map_err(|_| LoadRecoveryError::Unsafe(Box::new(row.clone())))?;
    if checkpoint.verify().is_err()
        || checkpoint.checkpoint_id.as_str() != checkpoint_row.checkpoint_id
        || checkpoint.workspace_target_hash.to_string() != checkpoint_row.workspace_target_hash
        || checkpoint.candidate_hash.to_string() != checkpoint_row.candidate_hash
        || checkpoint.manifest_hash.to_string() != checkpoint_row.manifest_hash
        || checkpoint.entries.len() != checkpoint_row.entry_count as usize
        || checkpoint.checkpoint_id != journal.checkpoint_id
        || checkpoint.candidate_hash != journal.candidate_hash
        || checkpoint.workspace_target_hash != journal.workspace_target_hash
    {
        return Err(LoadRecoveryError::Unsafe(Box::new(row)));
    }
    let checkpoint_payload_hash = Sha256Digest::parse(&checkpoint_row.payload.content_hash)
        .map_err(|_| LoadRecoveryError::Unsafe(Box::new(row.clone())))?;
    Ok(AuthenticatedRecovery {
        journal_workspace_grant_epoch: row.workspace_grant_epoch,
        journal_snapshot_hash: sha256_bytes(row.journal_json.as_bytes()),
        checkpoint_payload_hash,
        journal,
        checkpoint,
    })
}

fn operation_summary(operation: &RecoveryOperation) -> RecoveryOperationSummaryWire {
    let (label, explanation) = match &operation.restore_to {
        desktop_execution::CheckpointFileState::Absent => (
            "delete",
            "Remove a partial file created by the interrupted change.",
        ),
        desktop_execution::CheckpointFileState::Utf8 { .. }
            if operation.expected_current_exists =>
        {
            (
                "replace",
                "Restore the file content saved before the interrupted change.",
            )
        }
        desktop_execution::CheckpointFileState::Utf8 { .. } => (
            "create",
            "Recreate the file from the saved pre-change checkpoint.",
        ),
    };
    RecoveryOperationSummaryWire {
        relative_path: operation.relative_path.clone(),
        operation: label.to_owned(),
        explanation: explanation.to_owned(),
    }
}

fn persist_unsafe_manual_review(
    store: &LocalStore,
    row: &EffectJournalRow,
    _accepted_at: UnixMillis,
) -> Result<HostCommandData, LocalError> {
    let journal_id = ContractId::new(row.journal_id.clone()).map_err(|_| recovery_error())?;
    let execution_id = ContractId::new(row.execution_id.clone()).map_err(|_| recovery_error())?;
    let evidence = recovery_evidence(
        &execution_id,
        "execution.recovery-manual-review",
        "unsafe durable recovery structure",
    );
    store
        .update_effect_journal(
            journal_id.as_str(),
            "manual_review",
            &row.journal_json,
            Some(&evidence),
        )
        .map_err(|_| recovery_error())?;
    Ok(HostCommandData::ChangesRecoveryPrepared(
        ChangesRecoveryPreparedWire::ManualReview {
            journal_id,
            execution_id,
            reason_code: RecoveryManualReviewReasonWire::CheckpointIncompleteOrInconsistent,
        },
    ))
}

fn persist_manual_review(
    store: &LocalStore,
    authenticated: AuthenticatedRecovery,
    accepted_at: UnixMillis,
) -> Result<HostCommandData, LocalError> {
    let journal_id = authenticated.journal.journal_id.clone();
    let execution_id = authenticated.journal.execution_id.clone();
    transition_journal(
        store,
        authenticated.journal,
        JournalState::ManualReview,
        "execution.recovery-manual-review",
        "manual recovery review required",
        accepted_at,
    )?;
    Ok(HostCommandData::ChangesRecoveryPrepared(
        ChangesRecoveryPreparedWire::ManualReview {
            journal_id,
            execution_id,
            reason_code: RecoveryManualReviewReasonWire::CheckpointIncompleteOrInconsistent,
        },
    ))
}

fn transition_journal(
    store: &LocalStore,
    mut journal: EffectJournal,
    next: JournalState,
    event_type: &str,
    evidence_reason: &str,
    accepted_at: UnixMillis,
) -> Result<EffectJournal, LocalError> {
    journal.state = next;
    journal.updated_at = accepted_at;
    let journal_json = serde_json::to_string(&journal).map_err(|_| recovery_error())?;
    let evidence = recovery_evidence(&journal.execution_id, event_type, evidence_reason);
    store
        .update_effect_journal(
            journal.journal_id.as_str(),
            journal_state_label(next),
            &journal_json,
            Some(&evidence),
        )
        .map_err(|_| recovery_error())?;
    Ok(journal)
}

fn recovery_evidence(execution_id: &ContractId, event_type: &str, reason: &str) -> EvidenceAppend {
    EvidenceAppend {
        stream_id: format!("execution:{execution_id}"),
        event_type: event_type.to_owned(),
        payload_hash: sha256_bytes(reason.as_bytes()).to_string(),
        payload_ref: None,
        correlation_id: "reviewed_recovery".to_owned(),
        causation_id: None,
        redaction_level: "metadata".to_owned(),
        retention_class: "evidence".to_owned(),
    }
}

const fn journal_state_label(state: JournalState) -> &'static str {
    match state {
        JournalState::Restoring => "restoring",
        JournalState::Recovered => "recovered",
        JournalState::ManualReview => "manual_review",
        _ => "recovery_required",
    }
}

fn new_id(prefix: &str) -> Result<ContractId, LocalError> {
    ContractId::new(format!("{prefix}_{}", Ulid::new())).map_err(|_| recovery_error())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use desktop_execution::{
        CheckpointEntry, CheckpointFileState, EffectJournal, JournalOperation,
        JournalOperationState, JournalState, LocalCheckpoint, ResultFileOperation,
    };
    use desktop_runtime::{
        canonical_hash, sha256_bytes, ApprovalChoice, ContractId, LocalCommand, ProposedFileChange,
        RelativeWorkspacePath, UnixMillis, WorkspaceTarget,
    };
    use desktop_store::{
        EffectJournalUpsert, EvidenceAppend, ExecutionCheckpointAppend, LocalStore,
    };
    use serde::Serialize;

    use super::{decide_recovery, prepare_recovery, PendingRecoveries, PendingRecovery};
    use crate::state::HostState;
    use crate::wire::{ChangesRecoveryPreparedWire, HostCommandData};
    use desktop_runtime::RecoveryApprovalChoice;

    struct RecoveryFixture {
        store_dir: tempfile::TempDir,
        _workspace: tempfile::TempDir,
        state: HostState,
        workspace_id: ContractId,
        grant_epoch: u64,
        root: std::path::PathBuf,
    }

    fn fixture() -> Result<RecoveryFixture, Box<dyn std::error::Error>> {
        let store = tempfile::tempdir()?;
        let workspace = tempfile::tempdir()?;
        let state = HostState::initialize(Some(store.path().join("authority")))
            .map_err(|error| error.safe_message)?;
        let authority = state
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        let projection = state
            .workspace
            .grant("project_recovery", workspace.path())?;
        let binding = state
            .workspace
            .authority_binding(&projection.workspace_id)?;
        state
            .persist_workspace(
                &authority,
                projection.clone(),
                workspace.path(),
                &binding.root_identity_hash,
                &ContractId::new("request_recovery_grant")?,
            )
            .map_err(|error| error.safe_message)?;
        let enabled = state
            .workspace
            .enable_governed_edits(&projection.workspace_id)?;
        state
            .persist_workspace_update(
                &authority,
                &enabled,
                "workspace.edits_enabled",
                &ContractId::new("request_recovery_enable")?,
            )
            .map_err(|error| error.safe_message)?;
        drop(authority);
        state
            .bind_renderer("main")
            .map_err(|error| error.safe_message)?;
        Ok(RecoveryFixture {
            store_dir: store,
            root: workspace.path().to_path_buf(),
            _workspace: workspace,
            workspace_id: ContractId::new(enabled.workspace_id)?,
            grant_epoch: enabled.grant_epoch,
            state,
        })
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct CheckpointDraft<'a> {
        schema_version: &'static str,
        checkpoint_id: &'a ContractId,
        workspace_target_hash: desktop_runtime::Sha256Digest,
        candidate_hash: desktop_runtime::Sha256Digest,
        entries: &'a [CheckpointEntry],
        created_at: UnixMillis,
    }

    fn seed_recovery(
        fixture: &RecoveryFixture,
        before: &str,
        current: &str,
        incomplete_checkpoint: bool,
    ) -> Result<ContractId, Box<dyn std::error::Error>> {
        seed_recovery_in_state(fixture, before, current, incomplete_checkpoint, true)
    }

    fn seed_interrupted_recovery(
        fixture: &RecoveryFixture,
        before: &str,
        current: &str,
    ) -> Result<ContractId, Box<dyn std::error::Error>> {
        seed_recovery_in_state(fixture, before, current, false, false)
    }

    fn seed_recovery_in_state(
        fixture: &RecoveryFixture,
        before: &str,
        current: &str,
        incomplete_checkpoint: bool,
        quarantine: bool,
    ) -> Result<ContractId, Box<dyn std::error::Error>> {
        let workspace_target_hash = fixture_workspace_target_hash(fixture)?;
        seed_recovery_with_target(
            fixture,
            before,
            current,
            incomplete_checkpoint,
            workspace_target_hash,
            quarantine,
            fixture.grant_epoch,
        )
    }

    fn fixture_workspace_target_hash(
        fixture: &RecoveryFixture,
    ) -> Result<desktop_runtime::Sha256Digest, Box<dyn std::error::Error>> {
        let binding = fixture
            .state
            .workspace
            .authority_binding(fixture.workspace_id.as_str())?;
        let root_identity_hash = desktop_runtime::Sha256Digest::parse(&binding.root_identity_hash)?;
        let workspace_id = fixture.workspace_id.as_str();
        let target = WorkspaceTarget {
            target_kind: "local_folder_capability".to_owned(),
            workspace_capability_id: fixture.workspace_id.clone(),
            grant_epoch: fixture.grant_epoch,
            root_identity_hash,
            filesystem_capability_hash: sha256_bytes(
                format!(
                    "sapphirus:filesystem-capability:v1\n{}\n{}",
                    root_identity_hash.hex_value(),
                    fixture.grant_epoch
                )
                .as_bytes(),
            ),
            base_checkpoint_id: ContractId::new("checkpoint_genesis")?,
            workspace_manifest_hash: sha256_bytes(
                format!(
                    "sapphirus:workspace-manifest:v1\n{workspace_id}\n{}\n{}",
                    fixture.grant_epoch,
                    root_identity_hash.hex_value()
                )
                .as_bytes(),
            ),
        };
        Ok(canonical_hash("workspace-target", 1, &target)?)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the native fixture keeps its authenticated checkpoint and journal seed visibly paired"
    )]
    fn seed_recovery_with_target(
        fixture: &RecoveryFixture,
        before: &str,
        current: &str,
        incomplete_checkpoint: bool,
        workspace_target_hash: desktop_runtime::Sha256Digest,
        quarantine: bool,
        stored_grant_epoch: u64,
    ) -> Result<ContractId, Box<dyn std::error::Error>> {
        let path = RelativeWorkspacePath::new("main.rs")?;
        fs::write(fixture.root.join(path.as_str()), current)?;
        let checkpoint_id = ContractId::new("checkpoint_recovery")?;
        let journal_id = ContractId::new("journal_recovery")?;
        let execution_id = ContractId::new("execution_recovery")?;
        let candidate_hash = sha256_bytes(b"recovery candidate");
        let entries = vec![CheckpointEntry {
            relative_path: path.clone(),
            before: CheckpointFileState::Utf8 {
                content: before.to_owned(),
                content_hash: sha256_bytes(before.as_bytes()),
            },
        }];
        let created_at = UnixMillis(100);
        let manifest_hash = canonical_hash(
            "local-checkpoint",
            1,
            &CheckpointDraft {
                schema_version: "sapphirus.local-checkpoint.v1",
                checkpoint_id: &checkpoint_id,
                workspace_target_hash,
                candidate_hash,
                entries: &entries,
                created_at,
            },
        )?;
        let checkpoint = LocalCheckpoint {
            schema_version: "sapphirus.local-checkpoint.v1".to_owned(),
            checkpoint_id: checkpoint_id.clone(),
            workspace_target_hash,
            candidate_hash,
            entries,
            created_at,
            manifest_hash,
        };
        checkpoint.verify()?;

        let patch_hash = sha256_bytes(b"recovery patch");
        let mut operations = vec![JournalOperation {
            ordinal: 1,
            relative_path: path,
            operation: ResultFileOperation::Modified,
            preimage_hash: Some(sha256_bytes(before.as_bytes())),
            postimage_hash: Some(sha256_bytes(b"completed postimage")),
            state: JournalOperationState::Applying,
        }];
        if incomplete_checkpoint {
            operations.push(JournalOperation {
                ordinal: 2,
                relative_path: RelativeWorkspacePath::new("other.rs")?,
                operation: ResultFileOperation::Created,
                preimage_hash: None,
                postimage_hash: Some(sha256_bytes(b"other postimage")),
                state: JournalOperationState::Applying,
            });
        }
        let mut journal = EffectJournal {
            schema_version: "sapphirus.local-effect-journal.v1".to_owned(),
            journal_id: journal_id.clone(),
            execution_id: execution_id.clone(),
            candidate_hash,
            spec_hash: sha256_bytes(b"recovery spec"),
            consumption_hash: sha256_bytes(b"recovery consumption"),
            checkpoint_id: checkpoint_id.clone(),
            workspace_target_hash,
            patch_ref: format!("cas://sha256/{}", patch_hash.hex_value()),
            patch_hash,
            state: JournalState::Prepared,
            operations,
            created_at,
            updated_at: UnixMillis(200),
        };
        journal.verify_plan()?;

        let authority = fixture
            .state
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        let store = fixture
            .state
            .local_store(&authority)
            .map_err(|error| error.safe_message)?;
        persist_seed(store, fixture, stored_grant_epoch, &checkpoint, &journal)?;
        for state in ["checkpoint_durable", "preconditions_verified", "applying"] {
            store.update_effect_journal(
                journal_id.as_str(),
                state,
                &serde_json::to_string(&journal)?,
                None,
            )?;
        }
        if !quarantine {
            return Ok(journal_id);
        }
        journal.state = JournalState::RecoveryRequired;
        journal.updated_at = UnixMillis(300);
        store.update_effect_journal(
            journal_id.as_str(),
            "recovery_required",
            &serde_json::to_string(&journal)?,
            None,
        )?;
        Ok(journal_id)
    }

    fn persist_seed(
        store: &LocalStore,
        fixture: &RecoveryFixture,
        stored_grant_epoch: u64,
        checkpoint: &LocalCheckpoint,
        journal: &EffectJournal,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let checkpoint_json = serde_json::to_vec(checkpoint)?;
        store.persist_execution_checkpoint(&ExecutionCheckpointAppend {
            checkpoint_id: checkpoint.checkpoint_id.to_string(),
            workspace_target_hash: checkpoint.workspace_target_hash.to_string(),
            candidate_hash: checkpoint.candidate_hash.to_string(),
            manifest_hash: checkpoint.manifest_hash.to_string(),
            entry_count: u32::try_from(checkpoint.entries.len())?,
            checkpoint_json,
        })?;
        store.create_effect_journal(
            &EffectJournalUpsert {
                journal_id: journal.journal_id.to_string(),
                execution_id: journal.execution_id.to_string(),
                checkpoint_id: journal.checkpoint_id.to_string(),
                candidate_hash: journal.candidate_hash.to_string(),
                spec_hash: journal.spec_hash.to_string(),
                consumption_hash: journal.consumption_hash.to_string(),
                workspace_id: fixture.workspace_id.to_string(),
                workspace_grant_epoch: stored_grant_epoch,
                state: "prepared".to_owned(),
                journal_json: serde_json::to_string(journal)?,
            },
            &EvidenceAppend {
                stream_id: format!("execution:{}", journal.execution_id),
                event_type: "execution.journal-created".to_owned(),
                payload_hash: sha256_bytes(b"seed recovery journal").to_string(),
                payload_ref: None,
                correlation_id: "test_recovery_seed".to_owned(),
                causation_id: None,
                redaction_level: "metadata".to_owned(),
                retention_class: "evidence".to_owned(),
            },
        )?;
        Ok(())
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "the end-to-end restart fixture intentionally proves one continuous durable lifecycle"
    )]
    fn restart_recovery_requires_fresh_review_restores_once_and_keeps_failed_restore_blocking(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let initial = fixture()?;
        let original_epoch = initial.grant_epoch;
        let journal_id = seed_interrupted_recovery(&initial, "before\n", "partial\n")?;
        assert_eq!(fs::read(initial.root.join("main.rs"))?, b"partial\n");

        let RecoveryFixture {
            store_dir,
            _workspace: workspace,
            state,
            workspace_id,
            root,
            ..
        } = initial;
        let authority_root = store_dir.path().join("authority");
        drop(state);

        let restarted = HostState::initialize(Some(authority_root.clone()))
            .map_err(|error| error.safe_message)?;
        assert_eq!(fs::read(root.join("main.rs"))?, b"partial\n");
        let authority = restarted
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        let store = restarted
            .local_store(&authority)
            .map_err(|error| error.safe_message)?;
        assert_eq!(
            store
                .load_effect_journal(journal_id.as_str())?
                .ok_or("journal unavailable after restart")?
                .state,
            "recovery_required"
        );
        assert_eq!(store.list_open_effect_journals()?.len(), 1);
        assert!(crate::update::ensure_update_is_safe(&restarted).is_err());
        let enabled = restarted
            .workspace
            .enable_governed_edits(workspace_id.as_str())?;
        // ADR-0002: re-enabling edits advances only the governed-edit epoch.
        assert_eq!(enabled.grant_epoch, original_epoch);
        assert!(enabled.governed_edit_epoch > 1);
        restarted
            .persist_workspace_update(
                &authority,
                &enabled,
                "workspace.edits_enabled",
                &ContractId::new("request_restart_recovery_enable_1")?,
            )
            .map_err(|error| error.safe_message)?;
        drop(authority);
        restarted
            .bind_renderer("main")
            .map_err(|error| error.safe_message)?;
        let renderer = restarted
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;
        let first_review = prepare_recovery(
            &restarted,
            &renderer,
            &workspace_id,
            enabled.grant_epoch,
            &journal_id,
            UnixMillis(1_000),
        )
        .map_err(|error| error.safe_message)?;
        let HostCommandData::ChangesRecoveryPrepared(ChangesRecoveryPreparedWire::ReviewRequired {
            recovery_approval_id: old_approval_id,
            displayed_recovery_hash: old_displayed_hash,
            operations,
            ..
        }) = first_review
        else {
            return Err("expected a safe recovery review".into());
        };
        assert_eq!(operations.len(), 1);
        assert_eq!(operations[0].relative_path.as_str(), "main.rs");
        assert_eq!(operations[0].operation, "replace");
        let projected = serde_json::to_string(&operations)?;
        assert!(!projected.contains(root.to_string_lossy().as_ref()));
        assert!(!projected.contains("sha256:"));
        drop(renderer);
        drop(restarted);

        let reopened = HostState::initialize(Some(authority_root.clone()))
            .map_err(|error| error.safe_message)?;
        reopened
            .bind_renderer("main")
            .map_err(|error| error.safe_message)?;
        let renderer = reopened
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;
        assert!(decide_recovery(
            &reopened,
            &renderer,
            &old_approval_id,
            old_displayed_hash,
            RecoveryApprovalChoice::Restore,
            UnixMillis(1_001),
        )
        .is_err());
        drop(renderer);

        let authority = reopened
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        let enabled_again = reopened
            .workspace
            .enable_governed_edits(workspace_id.as_str())?;
        // ADR-0002: repeated enablement advances only the governed-edit
        // epoch; the binding epoch the renderer holds stays stable.
        assert_eq!(enabled_again.grant_epoch, enabled.grant_epoch);
        assert!(enabled_again.governed_edit_epoch > enabled.governed_edit_epoch);
        reopened
            .persist_workspace_update(
                &authority,
                &enabled_again,
                "workspace.edits_enabled",
                &ContractId::new("request_restart_recovery_enable_2")?,
            )
            .map_err(|error| error.safe_message)?;
        drop(authority);
        let renderer = reopened
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;
        let review = prepare_recovery(
            &reopened,
            &renderer,
            &workspace_id,
            enabled_again.grant_epoch,
            &journal_id,
            UnixMillis(1_002),
        )
        .map_err(|error| error.safe_message)?;
        let HostCommandData::ChangesRecoveryPrepared(ChangesRecoveryPreparedWire::ReviewRequired {
            recovery_approval_id,
            displayed_recovery_hash,
            ..
        }) = review
        else {
            return Err("expected a fresh recovery review".into());
        };
        let restored = decide_recovery(
            &reopened,
            &renderer,
            &recovery_approval_id,
            displayed_recovery_hash,
            RecoveryApprovalChoice::Restore,
            UnixMillis(1_003),
        )
        .map_err(|error| error.safe_message)?;
        let HostCommandData::ChangesRecoveryDecision(restored) = restored else {
            return Err("expected a recovery decision".into());
        };
        assert_eq!(restored.disposition, "recovered");
        assert_eq!(restored.restored_files, 1);
        assert_eq!(fs::read(root.join("main.rs"))?, b"before\n");
        let authority = reopened
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        let store = reopened
            .local_store(&authority)
            .map_err(|error| error.safe_message)?;
        assert_eq!(
            store
                .load_effect_journal(journal_id.as_str())?
                .ok_or("recovered journal unavailable")?
                .state,
            "recovered"
        );
        assert!(store.list_open_effect_journals()?.is_empty());
        assert!(crate::update::ensure_update_is_safe(&reopened).is_ok());
        drop(authority);
        drop(renderer);
        drop(reopened);
        drop(workspace);
        drop(store_dir);

        let failed = fixture()?;
        let failed_journal = seed_recovery(&failed, "before\n", "partial\n", false)?;
        let authority = failed
            .state
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        let store = failed
            .state
            .local_store(&authority)
            .map_err(|error| error.safe_message)?;
        let row = store
            .load_effect_journal(failed_journal.as_str())?
            .ok_or("failed journal unavailable")?;
        store.update_effect_journal(
            failed_journal.as_str(),
            "restoring",
            &row.journal_json,
            None,
        )?;
        drop(authority);
        let RecoveryFixture {
            store_dir: failed_store,
            _workspace: failed_workspace,
            state: failed_state,
            ..
        } = failed;
        let failed_authority_root = failed_store.path().join("authority");
        drop(failed_state);
        let failed_reopened = HostState::initialize(Some(failed_authority_root))
            .map_err(|error| error.safe_message)?;
        let authority = failed_reopened
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        let store = failed_reopened
            .local_store(&authority)
            .map_err(|error| error.safe_message)?;
        assert_eq!(
            store
                .load_effect_journal(failed_journal.as_str())?
                .ok_or("manual-review journal unavailable")?
                .state,
            "manual_review"
        );
        assert_eq!(store.list_open_effect_journals()?.len(), 1);
        assert!(crate::update::ensure_update_is_safe(&failed_reopened).is_err());
        drop(failed_workspace);
        Ok(())
    }

    #[test]
    fn recovery_prepare_requires_ready_exact_governed_workspace_authority(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let store = tempfile::tempdir()?;
        let state = HostState::initialize(Some(store.path().join("authority")))
            .map_err(|error| error.safe_message)?;
        state
            .bind_renderer("main")
            .map_err(|error| error.safe_message)?;
        let renderer = state
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;

        let result = prepare_recovery(
            &state,
            &renderer,
            &ContractId::new("workspace_missing")?,
            1,
            &ContractId::new("journal_missing")?,
            UnixMillis(1_000),
        );

        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn consistently_misbound_workspace_target_fails_before_recovery_mutation(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let journal_id = seed_recovery_with_target(
            &fixture,
            "before\n",
            "partial\n",
            false,
            sha256_bytes(b"misbound workspace target"),
            true,
            fixture.grant_epoch,
        )?;
        let renderer = fixture
            .state
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;

        assert!(prepare_recovery(
            &fixture.state,
            &renderer,
            &fixture.workspace_id,
            fixture.grant_epoch,
            &journal_id,
            UnixMillis(1_000),
        )
        .is_err());
        assert_eq!(
            fs::read_to_string(fixture.root.join("main.rs"))?,
            "partial\n"
        );
        let authority = fixture
            .state
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        assert_eq!(
            fixture
                .state
                .local_store(&authority)
                .map_err(|error| error.safe_message)?
                .load_effect_journal(journal_id.as_str())?
                .ok_or("journal unavailable")?
                .state,
            "recovery_required"
        );
        Ok(())
    }

    #[test]
    fn recovery_rejects_historical_epoch_substitution_and_current_request_drift(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let target_hash = fixture_workspace_target_hash(&fixture)?;
        // ADR-0002: the binding epoch no longer moves on edit enablement,
        // so a substituted historical epoch is any value that differs from
        // the live binding epoch.
        let substituted_epoch = fixture.grant_epoch.saturating_add(1);
        assert_ne!(substituted_epoch, fixture.grant_epoch);
        let journal_id = seed_recovery_with_target(
            &fixture,
            "before\n",
            "partial\n",
            false,
            target_hash,
            true,
            substituted_epoch,
        )?;
        let renderer = fixture
            .state
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;
        assert!(prepare_recovery(
            &fixture.state,
            &renderer,
            &fixture.workspace_id,
            fixture.grant_epoch,
            &journal_id,
            UnixMillis(1_000),
        )
        .is_err());
        assert_eq!(fs::read(fixture.root.join("main.rs"))?, b"partial\n");

        assert!(prepare_recovery(
            &fixture.state,
            &renderer,
            &fixture.workspace_id,
            fixture.grant_epoch.saturating_add(1),
            &journal_id,
            UnixMillis(1_001),
        )
        .is_err());
        assert!(prepare_recovery(
            &fixture.state,
            &renderer,
            &ContractId::new("workspace_other")?,
            fixture.grant_epoch,
            &journal_id,
            UnixMillis(1_002),
        )
        .is_err());
        assert_eq!(fs::read(fixture.root.join("main.rs"))?, b"partial\n");
        let authority = fixture
            .state
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        assert_eq!(
            fixture
                .state
                .local_store(&authority)
                .map_err(|error| error.safe_message)?
                .load_effect_journal(journal_id.as_str())?
                .ok_or("journal unavailable")?
                .state,
            "recovery_required"
        );
        Ok(())
    }

    #[test]
    fn recovery_prepare_is_observation_only_and_returns_a_bounded_review(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let journal_id = seed_recovery(&fixture, "before\n", "partial\n", false)?;
        let bytes_before = fs::read(fixture.root.join("main.rs"))?;
        let renderer = fixture
            .state
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;

        let prepared = prepare_recovery(
            &fixture.state,
            &renderer,
            &fixture.workspace_id,
            fixture.grant_epoch,
            &journal_id,
            UnixMillis(1_000),
        )
        .map_err(|error| error.safe_message)?;

        let HostCommandData::ChangesRecoveryPrepared(ChangesRecoveryPreparedWire::ReviewRequired {
            operations,
            ..
        }) = prepared
        else {
            return Err("expected a recovery review".into());
        };
        assert_eq!(operations.len(), 1);
        assert_eq!(operations[0].relative_path.as_str(), "main.rs");
        assert_eq!(operations[0].operation, "replace");
        assert_eq!(fs::read(fixture.root.join("main.rs"))?, bytes_before);
        let authority = fixture
            .state
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        let row = fixture
            .state
            .local_store(&authority)
            .map_err(|error| error.safe_message)?
            .load_effect_journal(journal_id.as_str())?
            .ok_or("journal unavailable")?;
        assert_eq!(row.state, "recovery_required");
        Ok(())
    }

    #[test]
    fn recovery_cancel_consumes_the_review_without_changing_durable_state(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let journal_id = seed_recovery(&fixture, "before\n", "partial\n", false)?;
        let renderer = fixture
            .state
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;
        let prepared = prepare_recovery(
            &fixture.state,
            &renderer,
            &fixture.workspace_id,
            fixture.grant_epoch,
            &journal_id,
            UnixMillis(1_000),
        )
        .map_err(|error| error.safe_message)?;
        let HostCommandData::ChangesRecoveryPrepared(ChangesRecoveryPreparedWire::ReviewRequired {
            recovery_approval_id,
            displayed_recovery_hash,
            ..
        }) = prepared
        else {
            return Err("expected a recovery review".into());
        };

        decide_recovery(
            &fixture.state,
            &renderer,
            &recovery_approval_id,
            displayed_recovery_hash,
            RecoveryApprovalChoice::Cancel,
            UnixMillis(1_001),
        )
        .map_err(|error| error.safe_message)?;
        let duplicate = decide_recovery(
            &fixture.state,
            &renderer,
            &recovery_approval_id,
            displayed_recovery_hash,
            RecoveryApprovalChoice::Cancel,
            UnixMillis(1_002),
        );
        assert!(duplicate.is_err());
        let authority = fixture
            .state
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        let row = fixture
            .state
            .local_store(&authority)
            .map_err(|error| error.safe_message)?
            .load_effect_journal(journal_id.as_str())?
            .ok_or("journal unavailable")?;
        assert_eq!(row.state, "recovery_required");
        assert_eq!(
            fs::read_to_string(fixture.root.join("main.rs"))?,
            "partial\n"
        );
        Ok(())
    }

    #[test]
    fn cancel_reauthentication_rejects_journal_and_state_drift(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let journal_fixture = fixture()?;
        let journal_id = seed_recovery(&journal_fixture, "before\n", "partial\n", false)?;
        let renderer = journal_fixture
            .state
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;
        let review = prepare_recovery(
            &journal_fixture.state,
            &renderer,
            &journal_fixture.workspace_id,
            journal_fixture.grant_epoch,
            &journal_id,
            UnixMillis(1_000),
        )
        .map_err(|error| error.safe_message)?;
        let (approval_id, displayed_hash) = recovery_approval(review)?;
        let authority = journal_fixture
            .state
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        let store = journal_fixture
            .state
            .local_store(&authority)
            .map_err(|error| error.safe_message)?;
        let row = store
            .load_effect_journal(journal_id.as_str())?
            .ok_or("journal unavailable")?;
        let mut drifted: EffectJournal = serde_json::from_str(&row.journal_json)?;
        drifted.updated_at = UnixMillis(drifted.updated_at.0 + 1);
        store.update_effect_journal(
            journal_id.as_str(),
            "recovery_required",
            &serde_json::to_string(&drifted)?,
            None,
        )?;
        drop(authority);
        assert!(decide_recovery(
            &journal_fixture.state,
            &renderer,
            &approval_id,
            displayed_hash,
            RecoveryApprovalChoice::Cancel,
            UnixMillis(1_001),
        )
        .is_err());

        let state_fixture = fixture()?;
        let state_journal = seed_recovery(&state_fixture, "before\n", "partial\n", false)?;
        let state_renderer = state_fixture
            .state
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;
        let state_review = prepare_recovery(
            &state_fixture.state,
            &state_renderer,
            &state_fixture.workspace_id,
            state_fixture.grant_epoch,
            &state_journal,
            UnixMillis(2_000),
        )
        .map_err(|error| error.safe_message)?;
        let (state_approval_id, state_displayed_hash) = recovery_approval(state_review)?;
        let authority = state_fixture
            .state
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        let store = state_fixture
            .state
            .local_store(&authority)
            .map_err(|error| error.safe_message)?;
        let row = store
            .load_effect_journal(state_journal.as_str())?
            .ok_or("journal unavailable")?;
        store.update_effect_journal(
            state_journal.as_str(),
            "manual_review",
            &row.journal_json,
            None,
        )?;
        drop(authority);
        assert!(decide_recovery(
            &state_fixture.state,
            &state_renderer,
            &state_approval_id,
            state_displayed_hash,
            RecoveryApprovalChoice::Cancel,
            UnixMillis(2_001),
        )
        .is_err());
        Ok(())
    }

    #[test]
    fn cancel_reauthentication_rejects_checkpoint_binding_and_plan_drift(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let checkpoint_fixture = fixture()?;
        let checkpoint_journal =
            seed_recovery(&checkpoint_fixture, "before\n", "partial\n", false)?;
        let checkpoint_renderer = checkpoint_fixture
            .state
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;
        let checkpoint_review = prepare_recovery(
            &checkpoint_fixture.state,
            &checkpoint_renderer,
            &checkpoint_fixture.workspace_id,
            checkpoint_fixture.grant_epoch,
            &checkpoint_journal,
            UnixMillis(1_000),
        )
        .map_err(|error| error.safe_message)?;
        let (checkpoint_approval_id, checkpoint_displayed_hash) =
            recovery_approval(checkpoint_review)?;
        let authority = checkpoint_fixture
            .state
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        let store = checkpoint_fixture
            .state
            .local_store(&authority)
            .map_err(|error| error.safe_message)?;
        let (checkpoint_row, _) = store
            .load_execution_checkpoint("checkpoint_recovery")?
            .ok_or("checkpoint unavailable")?;
        let storage_preimage = format!(
            "sapphirus:cas-storage:1\n{}\n{}\n{}",
            checkpoint_row.payload.kind,
            checkpoint_row.payload.schema_version,
            checkpoint_row.payload.content_hash
        );
        let storage_digest = sha256_bytes(storage_preimage.as_bytes()).hex_value();
        let checkpoint_payload_path = checkpoint_fixture
            .store_dir
            .path()
            .join("authority")
            .join("cas")
            .join(&storage_digest[..2])
            .join(storage_digest);
        fs::remove_file(checkpoint_payload_path)?;
        drop(authority);
        assert!(decide_recovery(
            &checkpoint_fixture.state,
            &checkpoint_renderer,
            &checkpoint_approval_id,
            checkpoint_displayed_hash,
            RecoveryApprovalChoice::Cancel,
            UnixMillis(1_001),
        )
        .is_err());

        let plan_fixture = fixture()?;
        let plan_journal = seed_recovery(&plan_fixture, "before\n", "partial\n", false)?;
        let plan_renderer = plan_fixture
            .state
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;
        let plan_review = prepare_recovery(
            &plan_fixture.state,
            &plan_renderer,
            &plan_fixture.workspace_id,
            plan_fixture.grant_epoch,
            &plan_journal,
            UnixMillis(2_000),
        )
        .map_err(|error| error.safe_message)?;
        let (plan_approval_id, plan_displayed_hash) = recovery_approval(plan_review)?;
        fs::write(plan_fixture.root.join("main.rs"), "plan drift\n")?;
        assert!(decide_recovery(
            &plan_fixture.state,
            &plan_renderer,
            &plan_approval_id,
            plan_displayed_hash,
            RecoveryApprovalChoice::Cancel,
            UnixMillis(2_001),
        )
        .is_err());
        assert_eq!(
            fs::read_to_string(plan_fixture.root.join("main.rs"))?,
            "plan drift\n"
        );
        Ok(())
    }

    #[test]
    fn recovery_restore_is_single_use_and_restores_the_checkpoint(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let journal_id = seed_recovery(&fixture, "before\n", "partial\n", false)?;
        let renderer = fixture
            .state
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;
        let prepared = prepare_recovery(
            &fixture.state,
            &renderer,
            &fixture.workspace_id,
            fixture.grant_epoch,
            &journal_id,
            UnixMillis(1_000),
        )
        .map_err(|error| error.safe_message)?;
        let HostCommandData::ChangesRecoveryPrepared(ChangesRecoveryPreparedWire::ReviewRequired {
            recovery_approval_id,
            displayed_recovery_hash,
            ..
        }) = prepared
        else {
            return Err("expected a recovery review".into());
        };

        let restored = decide_recovery(
            &fixture.state,
            &renderer,
            &recovery_approval_id,
            displayed_recovery_hash,
            RecoveryApprovalChoice::Restore,
            UnixMillis(1_001),
        )
        .map_err(|error| error.safe_message)?;
        let HostCommandData::ChangesRecoveryDecision(restored) = restored else {
            return Err("expected a recovery decision".into());
        };
        assert_eq!(restored.disposition, "recovered");
        assert_eq!(restored.restored_files, 1);
        assert_eq!(
            fs::read_to_string(fixture.root.join("main.rs"))?,
            "before\n"
        );
        let duplicate = decide_recovery(
            &fixture.state,
            &renderer,
            &recovery_approval_id,
            displayed_recovery_hash,
            RecoveryApprovalChoice::Restore,
            UnixMillis(1_002),
        );
        assert!(duplicate.is_err());
        let authority = fixture
            .state
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        let row = fixture
            .state
            .local_store(&authority)
            .map_err(|error| error.safe_message)?
            .load_effect_journal(journal_id.as_str())?
            .ok_or("journal unavailable")?;
        assert_eq!(row.state, "recovered");
        Ok(())
    }

    #[test]
    fn complete_recovery_requires_review_and_restores_the_exact_checkpoint(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let journal_id = seed_recovery(&fixture, "before\n", "completed postimage", false)?;
        let renderer = fixture
            .state
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;

        let prepared = prepare_recovery(
            &fixture.state,
            &renderer,
            &fixture.workspace_id,
            fixture.grant_epoch,
            &journal_id,
            UnixMillis(1_000),
        )
        .map_err(|error| error.safe_message)?;
        let HostCommandData::ChangesRecoveryPrepared(ChangesRecoveryPreparedWire::ReviewRequired {
            recovery_approval_id,
            displayed_recovery_hash,
            operations,
            ..
        }) = prepared
        else {
            return Err("complete recovery must require review".into());
        };
        assert_eq!(operations.len(), 1);
        assert_eq!(operations[0].operation, "replace");
        let authority = fixture
            .state
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        let store = fixture
            .state
            .local_store(&authority)
            .map_err(|error| error.safe_message)?;
        assert_eq!(
            store
                .load_effect_journal(journal_id.as_str())?
                .ok_or("journal unavailable")?
                .state,
            "recovery_required"
        );
        drop(authority);

        decide_recovery(
            &fixture.state,
            &renderer,
            &recovery_approval_id,
            displayed_recovery_hash,
            RecoveryApprovalChoice::Restore,
            UnixMillis(1_001),
        )
        .map_err(|error| error.safe_message)?;

        assert_eq!(
            fs::read_to_string(fixture.root.join("main.rs"))?,
            "before\n"
        );
        let authority = fixture
            .state
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        assert_eq!(
            fixture
                .state
                .local_store(&authority)
                .map_err(|error| error.safe_message)?
                .load_effect_journal(journal_id.as_str())?
                .ok_or("journal unavailable")?
                .state,
            "recovered"
        );
        Ok(())
    }

    #[test]
    fn no_effect_recovery_closes_without_a_review_or_workspace_mutation(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let journal_id = seed_recovery(&fixture, "before\n", "before\n", false)?;
        let renderer = fixture
            .state
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;

        let prepared = prepare_recovery(
            &fixture.state,
            &renderer,
            &fixture.workspace_id,
            fixture.grant_epoch,
            &journal_id,
            UnixMillis(1_000),
        )
        .map_err(|error| error.safe_message)?;

        assert!(matches!(
            prepared,
            HostCommandData::ChangesRecoveryPrepared(
                ChangesRecoveryPreparedWire::AlreadyRecovered { .. }
            )
        ));
        assert_eq!(
            fs::read_to_string(fixture.root.join("main.rs"))?,
            "before\n"
        );
        let authority = fixture
            .state
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        let row = fixture
            .state
            .local_store(&authority)
            .map_err(|error| error.safe_message)?
            .load_effect_journal(journal_id.as_str())?
            .ok_or("journal unavailable")?;
        assert_eq!(row.state, "recovered");
        Ok(())
    }

    #[test]
    fn incomplete_checkpoint_coverage_becomes_safe_terminal_manual_review(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let journal_id = seed_recovery(&fixture, "before\n", "partial\n", true)?;
        let renderer = fixture
            .state
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;

        let prepared = prepare_recovery(
            &fixture.state,
            &renderer,
            &fixture.workspace_id,
            fixture.grant_epoch,
            &journal_id,
            UnixMillis(1_000),
        )
        .map_err(|error| error.safe_message)?;

        let HostCommandData::ChangesRecoveryPrepared(ChangesRecoveryPreparedWire::ManualReview {
            reason_code,
            ..
        }) = prepared
        else {
            return Err("expected terminal manual review".into());
        };
        assert_eq!(
            reason_code,
            crate::wire::RecoveryManualReviewReasonWire::CheckpointIncompleteOrInconsistent,
        );
        let authority = fixture
            .state
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        let row = fixture
            .state
            .local_store(&authority)
            .map_err(|error| error.safe_message)?
            .load_effect_journal(journal_id.as_str())?
            .ok_or("journal unavailable")?;
        assert_eq!(row.state, "manual_review");
        Ok(())
    }

    #[test]
    fn expiry_and_workspace_drift_consume_review_before_any_restore(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let expired_fixture = fixture()?;
        let expired_journal = seed_recovery(&expired_fixture, "before\n", "partial\n", false)?;
        let expired_renderer = expired_fixture
            .state
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;
        let expired = prepare_recovery(
            &expired_fixture.state,
            &expired_renderer,
            &expired_fixture.workspace_id,
            expired_fixture.grant_epoch,
            &expired_journal,
            UnixMillis(1_000),
        )
        .map_err(|error| error.safe_message)?;
        let HostCommandData::ChangesRecoveryPrepared(ChangesRecoveryPreparedWire::ReviewRequired {
            recovery_approval_id,
            displayed_recovery_hash,
            expires_at,
            ..
        }) = expired
        else {
            return Err("expected a recovery review".into());
        };
        assert!(decide_recovery(
            &expired_fixture.state,
            &expired_renderer,
            &recovery_approval_id,
            displayed_recovery_hash,
            RecoveryApprovalChoice::Restore,
            UnixMillis(expires_at.0 + 1),
        )
        .is_err());
        assert_eq!(
            fs::read_to_string(expired_fixture.root.join("main.rs"))?,
            "partial\n"
        );

        let drift_fixture = fixture()?;
        let drift_journal = seed_recovery(&drift_fixture, "before\n", "partial\n", false)?;
        let drift_renderer = drift_fixture
            .state
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;
        let drifted = prepare_recovery(
            &drift_fixture.state,
            &drift_renderer,
            &drift_fixture.workspace_id,
            drift_fixture.grant_epoch,
            &drift_journal,
            UnixMillis(2_000),
        )
        .map_err(|error| error.safe_message)?;
        let HostCommandData::ChangesRecoveryPrepared(ChangesRecoveryPreparedWire::ReviewRequired {
            recovery_approval_id,
            displayed_recovery_hash,
            ..
        }) = drifted
        else {
            return Err("expected a recovery review".into());
        };
        fs::write(drift_fixture.root.join("main.rs"), "external drift\n")?;
        assert!(decide_recovery(
            &drift_fixture.state,
            &drift_renderer,
            &recovery_approval_id,
            displayed_recovery_hash,
            RecoveryApprovalChoice::Restore,
            UnixMillis(2_001),
        )
        .is_err());
        assert_eq!(
            fs::read_to_string(drift_fixture.root.join("main.rs"))?,
            "external drift\n"
        );
        let authority = drift_fixture
            .state
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        let row = drift_fixture
            .state
            .local_store(&authority)
            .map_err(|error| error.safe_message)?
            .load_effect_journal(drift_journal.as_str())?
            .ok_or("journal unavailable")?;
        assert_eq!(row.state, "recovery_required");
        Ok(())
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one matrix test proves every host event that must invalidate recovery authority"
    )]
    fn renderer_grant_workspace_recovery_and_restart_events_invalidate_reviews(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let rebound = fixture()?;
        let rebound_journal = seed_recovery(&rebound, "before\n", "partial\n", false)?;
        let rebound_renderer = rebound
            .state
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;
        let rebound_review = prepare_recovery(
            &rebound.state,
            &rebound_renderer,
            &rebound.workspace_id,
            rebound.grant_epoch,
            &rebound_journal,
            UnixMillis(1_000),
        )
        .map_err(|error| error.safe_message)?;
        let rebound_id = recovery_approval_id(rebound_review)?;
        drop(rebound_renderer);
        rebound
            .state
            .bind_renderer("main")
            .map_err(|error| error.safe_message)?;
        assert!(rebound.state.take_pending_recovery(&rebound_id).is_none());

        let changed_grant = fixture()?;
        let changed_journal = seed_recovery(&changed_grant, "before\n", "partial\n", false)?;
        let changed_renderer = changed_grant
            .state
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;
        let changed_review = prepare_recovery(
            &changed_grant.state,
            &changed_renderer,
            &changed_grant.workspace_id,
            changed_grant.grant_epoch,
            &changed_journal,
            UnixMillis(1_000),
        )
        .map_err(|error| error.safe_message)?;
        let changed_id = recovery_approval_id(changed_review)?;
        drop(changed_renderer);
        let authority = changed_grant
            .state
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        let enabled = changed_grant
            .state
            .workspace
            .enable_governed_edits(changed_grant.workspace_id.as_str())?;
        changed_grant
            .state
            .persist_workspace_update(
                &authority,
                &enabled,
                "workspace.edits_enabled",
                &ContractId::new("request_recovery_reenable")?,
            )
            .map_err(|error| error.safe_message)?;
        drop(authority);
        assert!(changed_grant
            .state
            .take_pending_recovery(&changed_id)
            .is_none());

        let revoked = fixture()?;
        let revoked_journal = seed_recovery(&revoked, "before\n", "partial\n", false)?;
        let revoked_renderer = revoked
            .state
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;
        let revoked_review = prepare_recovery(
            &revoked.state,
            &revoked_renderer,
            &revoked.workspace_id,
            revoked.grant_epoch,
            &revoked_journal,
            UnixMillis(1_000),
        )
        .map_err(|error| error.safe_message)?;
        let revoked_id = recovery_approval_id(revoked_review)?;
        drop(revoked_renderer);
        let authority = revoked
            .state
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        revoked
            .state
            .persist_revocation(
                &authority,
                &revoked.workspace_id,
                &ContractId::new("request_recovery_revoke")?,
            )
            .map_err(|error| error.safe_message)?;
        drop(authority);
        assert!(revoked.state.take_pending_recovery(&revoked_id).is_none());

        let switched = fixture()?;
        let switched_journal = seed_recovery(&switched, "before\n", "partial\n", false)?;
        let switched_renderer = switched
            .state
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;
        let switched_review = prepare_recovery(
            &switched.state,
            &switched_renderer,
            &switched.workspace_id,
            switched.grant_epoch,
            &switched_journal,
            UnixMillis(1_000),
        )
        .map_err(|error| error.safe_message)?;
        let switched_id = recovery_approval_id(switched_review)?;
        drop(switched_renderer);
        let second_workspace = tempfile::tempdir()?;
        let authority = switched
            .state
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        let projection = switched
            .state
            .workspace
            .grant("project_recovery_second", second_workspace.path())?;
        let binding = switched
            .state
            .workspace
            .authority_binding(&projection.workspace_id)?;
        switched
            .state
            .persist_workspace(
                &authority,
                projection,
                second_workspace.path(),
                &binding.root_identity_hash,
                &ContractId::new("request_recovery_switch")?,
            )
            .map_err(|error| error.safe_message)?;
        drop(authority);
        assert!(switched.state.take_pending_recovery(&switched_id).is_none());

        let recovery_entry = fixture()?;
        let recovery_journal = seed_recovery(&recovery_entry, "before\n", "partial\n", false)?;
        let recovery_renderer = recovery_entry
            .state
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;
        let recovery_review = prepare_recovery(
            &recovery_entry.state,
            &recovery_renderer,
            &recovery_entry.workspace_id,
            recovery_entry.grant_epoch,
            &recovery_journal,
            UnixMillis(1_000),
        )
        .map_err(|error| error.safe_message)?;
        let recovery_id = recovery_approval_id(recovery_review)?;
        drop(recovery_renderer);
        recovery_entry.state.enter_recovery();
        assert!(recovery_entry
            .state
            .take_pending_recovery(&recovery_id)
            .is_none());

        let restarted = fixture()?;
        let restarted_journal = seed_recovery(&restarted, "before\n", "partial\n", false)?;
        let restarted_renderer = restarted
            .state
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;
        let restarted_review = prepare_recovery(
            &restarted.state,
            &restarted_renderer,
            &restarted.workspace_id,
            restarted.grant_epoch,
            &restarted_journal,
            UnixMillis(1_000),
        )
        .map_err(|error| error.safe_message)?;
        let restarted_id = recovery_approval_id(restarted_review)?;
        drop(restarted_renderer);
        let RecoveryFixture {
            store_dir: retained_store,
            _workspace,
            state,
            ..
        } = restarted;
        let authority_root = retained_store.path().join("authority");
        drop(state);
        let reopened =
            HostState::initialize(Some(authority_root)).map_err(|error| error.safe_message)?;
        assert!(reopened.take_pending_recovery(&restarted_id).is_none());
        Ok(())
    }

    #[test]
    fn recovery_and_ordinary_approval_ids_are_cross_domain_isolated(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let ordinary_fixture = fixture()?;
        fs::write(ordinary_fixture.root.join("main.rs"), "proposal preimage\n")?;
        let ordinary = crate::edits::execute_changes_command(
            &ordinary_fixture.state,
            &ContractId::new("request_ordinary_review")?,
            UnixMillis(900),
            LocalCommand::ProposeChanges {
                workspace_id: ordinary_fixture.workspace_id.clone(),
                workspace_grant_epoch: ordinary_fixture.grant_epoch,
                changes: vec![ProposedFileChange::SetContent {
                    relative_path: RelativeWorkspacePath::new("main.rs")?,
                    content: "ordinary proposal\n".to_owned(),
                }],
            },
        )
        .map_err(|error| error.safe_message)?;
        let HostCommandData::ChangesReview(ordinary) = ordinary else {
            return Err("expected an ordinary review".into());
        };
        let renderer = ordinary_fixture
            .state
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;
        assert!(decide_recovery(
            &ordinary_fixture.state,
            &renderer,
            &ordinary.approval_id,
            ordinary.displayed_diff_hash,
            RecoveryApprovalChoice::Cancel,
            UnixMillis(901),
        )
        .is_err());
        drop(renderer);

        let recovery_fixture = fixture()?;
        let journal_id = seed_recovery(&recovery_fixture, "before\n", "partial\n", false)?;
        let renderer = recovery_fixture
            .state
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;
        let review = prepare_recovery(
            &recovery_fixture.state,
            &renderer,
            &recovery_fixture.workspace_id,
            recovery_fixture.grant_epoch,
            &journal_id,
            UnixMillis(1_000),
        )
        .map_err(|error| error.safe_message)?;
        let HostCommandData::ChangesRecoveryPrepared(ChangesRecoveryPreparedWire::ReviewRequired {
            recovery_approval_id,
            displayed_recovery_hash,
            ..
        }) = review
        else {
            return Err("expected a recovery review".into());
        };
        let ordinary_decision = crate::edits::execute_changes_command(
            &recovery_fixture.state,
            &ContractId::new("request_wrong_approval_domain")?,
            UnixMillis(1_001),
            LocalCommand::DecideApproval {
                approval_id: recovery_approval_id.clone(),
                candidate_hash: sha256_bytes(b"not a recovery candidate"),
                displayed_diff_hash: displayed_recovery_hash,
                choice: ApprovalChoice::Discard,
            },
        );
        assert!(ordinary_decision.is_err());
        decide_recovery(
            &recovery_fixture.state,
            &renderer,
            &recovery_approval_id,
            displayed_recovery_hash,
            RecoveryApprovalChoice::Cancel,
            UnixMillis(1_002),
        )
        .map_err(|error| error.safe_message)?;
        Ok(())
    }

    #[test]
    fn restore_failure_after_durable_restoring_is_terminal_manual_review(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let journal_id = seed_recovery(&fixture, "before\n", "partial\n", false)?;
        let renderer = fixture
            .state
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;
        let review = prepare_recovery(
            &fixture.state,
            &renderer,
            &fixture.workspace_id,
            fixture.grant_epoch,
            &journal_id,
            UnixMillis(1_000),
        )
        .map_err(|error| error.safe_message)?;
        let HostCommandData::ChangesRecoveryPrepared(ChangesRecoveryPreparedWire::ReviewRequired {
            recovery_approval_id,
            displayed_recovery_hash,
            ..
        }) = review
        else {
            return Err("expected a recovery review".into());
        };
        let target = fixture.root.join("main.rs");
        let original_permissions = fs::metadata(&target)?.permissions();
        let mut readonly_permissions = original_permissions.clone();
        readonly_permissions.set_readonly(true);
        fs::set_permissions(&target, readonly_permissions)?;

        let decision = decide_recovery(
            &fixture.state,
            &renderer,
            &recovery_approval_id,
            displayed_recovery_hash,
            RecoveryApprovalChoice::Restore,
            UnixMillis(1_001),
        );
        fs::set_permissions(&target, original_permissions)?;
        assert!(decision.is_err_and(|error| {
            error.code == desktop_runtime::LocalErrorCode::RecoveryRequired
        }));
        let authority = fixture
            .state
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        let row = fixture
            .state
            .local_store(&authority)
            .map_err(|error| error.safe_message)?
            .load_effect_journal(journal_id.as_str())?
            .ok_or("journal unavailable")?;
        assert_eq!(row.state, "manual_review");
        assert!(decide_recovery(
            &fixture.state,
            &renderer,
            &recovery_approval_id,
            displayed_recovery_hash,
            RecoveryApprovalChoice::Restore,
            UnixMillis(1_002),
        )
        .is_err());
        Ok(())
    }

    #[test]
    fn repeated_prepare_replaces_the_old_review_for_the_same_journal(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let journal_id = seed_recovery(&fixture, "before\n", "partial\n", false)?;
        let renderer = fixture
            .state
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;
        let first = prepare_recovery(
            &fixture.state,
            &renderer,
            &fixture.workspace_id,
            fixture.grant_epoch,
            &journal_id,
            UnixMillis(1_000),
        )
        .map_err(|error| error.safe_message)?;
        let first_id = recovery_approval_id(first)?;
        let second = prepare_recovery(
            &fixture.state,
            &renderer,
            &fixture.workspace_id,
            fixture.grant_epoch,
            &journal_id,
            UnixMillis(1_001),
        )
        .map_err(|error| error.safe_message)?;
        let second_id = recovery_approval_id(second)?;
        assert_ne!(first_id, second_id);
        assert!(fixture.state.take_pending_recovery(&first_id).is_none());
        assert!(fixture.state.take_pending_recovery(&second_id).is_some());
        Ok(())
    }

    #[test]
    fn recovery_wire_contains_only_bounded_relative_summaries(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let journal_id = seed_recovery(&fixture, "private checkpoint bytes\n", "partial\n", false)?;
        let renderer = fixture
            .state
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;
        let review = prepare_recovery(
            &fixture.state,
            &renderer,
            &fixture.workspace_id,
            fixture.grant_epoch,
            &journal_id,
            UnixMillis(1_000),
        )
        .map_err(|error| error.safe_message)?;
        let serialized = serde_json::to_string(&review)?;
        assert!(serialized.contains("main.rs"));
        assert!(serialized.contains("replace"));
        assert!(!serialized.contains(&fixture.root.display().to_string()));
        for private in [
            "private checkpoint bytes",
            "recovery candidate",
            "recovery workspace target",
            "recovery spec",
            "recovery consumption",
            "fileIdentityHash",
            "checkpointId",
            "planHash",
            "contentHash",
        ] {
            assert!(!serialized.contains(private), "leaked {private}");
        }
        Ok(())
    }

    #[test]
    fn durable_journal_drift_after_prepare_fails_before_workspace_mutation(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let journal_id = seed_recovery(&fixture, "before\n", "partial\n", false)?;
        let renderer = fixture
            .state
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;
        let review = prepare_recovery(
            &fixture.state,
            &renderer,
            &fixture.workspace_id,
            fixture.grant_epoch,
            &journal_id,
            UnixMillis(1_000),
        )
        .map_err(|error| error.safe_message)?;
        let HostCommandData::ChangesRecoveryPrepared(ChangesRecoveryPreparedWire::ReviewRequired {
            recovery_approval_id,
            displayed_recovery_hash,
            ..
        }) = review
        else {
            return Err("expected a recovery review".into());
        };
        let authority = fixture
            .state
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        let store = fixture
            .state
            .local_store(&authority)
            .map_err(|error| error.safe_message)?;
        let row = store
            .load_effect_journal(journal_id.as_str())?
            .ok_or("journal unavailable")?;
        let mut drifted: EffectJournal = serde_json::from_str(&row.journal_json)?;
        drifted.operations[0].state = JournalOperationState::Applied;
        store.update_effect_journal(
            journal_id.as_str(),
            "recovery_required",
            &serde_json::to_string(&drifted)?,
            None,
        )?;
        drop(authority);

        assert!(decide_recovery(
            &fixture.state,
            &renderer,
            &recovery_approval_id,
            displayed_recovery_hash,
            RecoveryApprovalChoice::Restore,
            UnixMillis(1_001),
        )
        .is_err());
        assert_eq!(
            fs::read_to_string(fixture.root.join("main.rs"))?,
            "partial\n"
        );
        let authority = fixture
            .state
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        let row = fixture
            .state
            .local_store(&authority)
            .map_err(|error| error.safe_message)?
            .load_effect_journal(journal_id.as_str())?
            .ok_or("journal unavailable")?;
        assert_eq!(row.state, "recovery_required");
        Ok(())
    }

    #[test]
    fn pending_recovery_collection_evicts_oldest_at_its_fixed_bound(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let journal_id = seed_recovery(&fixture, "before\n", "partial\n", false)?;
        let renderer = fixture
            .state
            .renderer_session_authority("main")
            .ok_or("renderer authority unavailable")?;
        let review = prepare_recovery(
            &fixture.state,
            &renderer,
            &fixture.workspace_id,
            fixture.grant_epoch,
            &journal_id,
            UnixMillis(1_000),
        )
        .map_err(|error| error.safe_message)?;
        let template_id = recovery_approval_id(review)?;
        let template = fixture
            .state
            .take_pending_recovery(&template_id)
            .ok_or("pending recovery unavailable")?;
        let mut collection = PendingRecoveries::default();
        for index in 0..=super::MAX_PENDING_RECOVERIES {
            let approval_id = ContractId::new(format!("recovery_approval_bound_{index}"))?;
            collection.insert(PendingRecovery {
                approval_id,
                installation_id: template.installation_id.clone(),
                renderer_session_id: template.renderer_session_id.clone(),
                workspace_id: template.workspace_id.clone(),
                workspace_grant_epoch: template.workspace_grant_epoch,
                journal_workspace_grant_epoch: template.journal_workspace_grant_epoch,
                journal_id: ContractId::new(format!("journal_bound_{index}"))?,
                execution_id: ContractId::new(format!("execution_bound_{index}"))?,
                checkpoint_id: ContractId::new(format!("checkpoint_bound_{index}"))?,
                journal_snapshot_hash: template.journal_snapshot_hash,
                checkpoint_payload_hash: template.checkpoint_payload_hash,
                plan: template.plan.clone(),
                displayed_recovery_hash: template.displayed_recovery_hash,
                expires_at: template.expires_at,
            });
        }
        assert!(collection
            .take(&ContractId::new("recovery_approval_bound_0")?)
            .is_none());
        assert!(collection
            .take(&ContractId::new(format!(
                "recovery_approval_bound_{}",
                super::MAX_PENDING_RECOVERIES
            ))?)
            .is_some());
        Ok(())
    }

    fn recovery_approval_id(
        data: HostCommandData,
    ) -> Result<ContractId, Box<dyn std::error::Error>> {
        let HostCommandData::ChangesRecoveryPrepared(ChangesRecoveryPreparedWire::ReviewRequired {
            recovery_approval_id,
            ..
        }) = data
        else {
            return Err("expected a recovery review".into());
        };
        Ok(recovery_approval_id)
    }

    fn recovery_approval(
        data: HostCommandData,
    ) -> Result<(ContractId, desktop_runtime::Sha256Digest), Box<dyn std::error::Error>> {
        let HostCommandData::ChangesRecoveryPrepared(ChangesRecoveryPreparedWire::ReviewRequired {
            recovery_approval_id,
            displayed_recovery_hash,
            ..
        }) = data
        else {
            return Err("expected a recovery review".into());
        };
        Ok((recovery_approval_id, displayed_recovery_hash))
    }
}
