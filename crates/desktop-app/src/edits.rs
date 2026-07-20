//! Governed local edits composition: proposal preparation, approval binding,
//! durable single-use spec consumption, checkpointed patch execution, undo
//! planning, and boot-time journal reconciliation.
//!
//! This module is the only place where the pure edits domain, the governed
//! workspace file broker, the durable execution store, and the D3 patch
//! engine meet. The renderer supplies untrusted proposed content; every
//! authority fact is observed or minted here.

use std::collections::{HashMap, VecDeque};

use desktop_execution::{
    plan_rollback, EffectJournal, ExecutionError, ExecutionRequest, ExecutionStore, JournalState,
    LocalCheckpoint, LocalExecutionResult, PatchExecutor, WorkspaceFileIo,
    WorkspaceFileObservation, WorkspaceIoError,
};
use desktop_runtime::{
    build_changes_candidate, build_changes_review, canonical_hash, sha256_bytes, ApprovalChoice,
    ApprovalDecisionDraft, ApprovedExecutionSpecDraft, ChangesProposalBinding, ChangesProposalKind,
    ChangesReviewProjection, ContractId, DeliveryModel, EditsError, InputKind, LocalError,
    LocalErrorCode, MutableInputBinding, NativePatchEngineAudience, ObservedPreimage,
    PreparedChangesProposal, ProjectionEventKind, ProposedFileChange, RelativeWorkspacePath,
    Sha256Digest, SpecConsumptionRecordDraft, UnixMillis, WorkspaceTarget,
};
use desktop_store::{
    EffectJournalRow, EffectJournalUpsert, EvidenceAppend, ExecutionCheckpointAppend,
    ExecutionResultAppend, ExecutionResultRow, LocalStore, StoreError,
};
use desktop_workspace::{
    GovernedRecoveryTransaction, PreimageObservation, WorkspaceBroker, WorkspaceError,
};
use ulid::Ulid;

use crate::state::{
    conflict_error, invalid_request, not_found_error, now, recovery_error, HostState,
    ReadyAuthorityGuard,
};
use crate::wire::{
    AppliedFileWire, ChangesDecisionWire, ChangesExecutionWire, ChangesHistoryEntryWire,
    ChangesHistoryWire, ChangesReviewWire, HostCommandData, OpenJournalWire,
    RecoveryAvailabilityWire, UndoConflictWire,
};

const MAX_PENDING_PROPOSALS: usize = 32;
const PROPOSAL_REVIEW_WINDOW_MS: u64 = 10 * 60 * 1000;
const SPEC_VALIDITY_WINDOW_MS: u64 = 2 * 60 * 1000;
const GENESIS_CHECKPOINT_ID: &str = "checkpoint_genesis";
const PATCH_ENGINE_PROFILE: &str = "sapphirus:governed-utf8-patch:v1";
const LOCAL_EDITS_POLICY: &str = "sapphirus:local-edits-policy:v1";

/// One reviewed proposal awaiting a single approval decision. Pending
/// proposals are deliberately in-memory only: a restart discards them and a
/// decision can never be replayed against restored state.
pub(crate) struct PendingChangesProposal {
    pub prepared: PreparedChangesProposal,
    pub review: ChangesReviewProjection,
    pub displayed_diff_hash: Sha256Digest,
    pub workspace_id: String,
    pub workspace_grant_epoch: u64,
    /// The exact D3 edit-authority version the proposal was reviewed under
    /// (ADR-0002). Any edit-authority change invalidates the proposal.
    pub workspace_governed_edit_epoch: u64,
}

#[derive(Default)]
pub(crate) struct PendingProposals {
    values: HashMap<String, PendingChangesProposal>,
    order: VecDeque<String>,
}

impl PendingProposals {
    pub fn insert(&mut self, approval_id: String, proposal: PendingChangesProposal) {
        while self.order.len() >= MAX_PENDING_PROPOSALS {
            if let Some(evicted) = self.order.pop_front() {
                self.values.remove(&evicted);
            }
        }
        self.order.push_back(approval_id.clone());
        self.values.insert(approval_id, proposal);
    }

    pub fn take(&mut self, approval_id: &str) -> Option<PendingChangesProposal> {
        self.order.retain(|id| id != approval_id);
        self.values.remove(approval_id)
    }
}

/// Bridges the D3 engine's file port onto the governed workspace broker.
/// Each call revalidates the grant at the pinned epoch; the target hash is
/// revalidated against live authority facts while the exact reviewed target
/// hash remains pinned for execution and rollback.
pub(crate) struct GovernedWorkspaceIo<'a> {
    broker: &'a WorkspaceBroker,
    workspace_id: String,
    grant_epoch: u64,
    governed_edit_epoch: u64,
    expected_root_identity_hash: Sha256Digest,
    workspace_target_hash: Sha256Digest,
}

impl GovernedWorkspaceIo<'_> {
    #[allow(
        dead_code,
        reason = "the recovery-only constructor is consumed by the Task 4 command boundary"
    )]
    pub(crate) fn new<'a>(
        broker: &'a WorkspaceBroker,
        workspace_id: &ContractId,
        grant_epoch: u64,
        governed_edit_epoch: u64,
        expected_root_identity_hash: Sha256Digest,
        workspace_target_hash: Sha256Digest,
    ) -> GovernedWorkspaceIo<'a> {
        GovernedWorkspaceIo {
            broker,
            workspace_id: workspace_id.to_string(),
            grant_epoch,
            governed_edit_epoch,
            expected_root_identity_hash,
            workspace_target_hash,
        }
    }

    fn current_target_hash(&self) -> Result<Sha256Digest, WorkspaceIoError> {
        let binding = self
            .broker
            .authority_binding(&self.workspace_id)
            .map_err(map_broker_error)?;
        if binding.grant_epoch != self.grant_epoch
            || binding.governed_edit_epoch != self.governed_edit_epoch
        {
            return Err(WorkspaceIoError::CapabilityRevoked);
        }
        let root_identity_hash = Sha256Digest::parse(&binding.root_identity_hash)
            .map_err(|_| WorkspaceIoError::Unavailable)?;
        if root_identity_hash != self.expected_root_identity_hash {
            return Err(WorkspaceIoError::CapabilityRevoked);
        }
        Ok(self.workspace_target_hash)
    }
}

impl WorkspaceFileIo for GovernedWorkspaceIo<'_> {
    fn workspace_target_hash(&self) -> Result<Sha256Digest, WorkspaceIoError> {
        self.current_target_hash()
    }

    fn read_file(
        &self,
        path: &RelativeWorkspacePath,
        expected_file_identity_hash: Option<Sha256Digest>,
    ) -> Result<Option<Vec<u8>>, WorkspaceIoError> {
        let expected = expected_file_identity_hash.map(|digest| digest.to_string());
        self.broker
            .read_effect_file(
                &self.workspace_id,
                self.grant_epoch,
                self.governed_edit_epoch,
                path.as_str(),
                expected.as_deref(),
            )
            .map_err(map_broker_error)
    }

    fn observe_recovery_file(
        &self,
        path: &RelativeWorkspacePath,
    ) -> Result<WorkspaceFileObservation, WorkspaceIoError> {
        let observation = self
            .broker
            .observe_preimage(
                &self.workspace_id,
                self.grant_epoch,
                self.governed_edit_epoch,
                path.as_str(),
            )
            .map_err(map_broker_error)?;
        map_recovery_observation(path, observation)
    }

    fn with_recovery_transaction(
        &self,
        transaction: &mut dyn FnMut(&dyn WorkspaceFileIo) -> Result<(), WorkspaceIoError>,
    ) -> Result<(), WorkspaceIoError> {
        let workspace_target_hash = self.current_target_hash()?;
        self.broker
            .with_governed_recovery(
                &self.workspace_id,
                self.grant_epoch,
                self.governed_edit_epoch,
                |broker_transaction| {
                    let scoped = ScopedRecoveryWorkspaceIo {
                        transaction: broker_transaction,
                        workspace_target_hash,
                    };
                    transaction(&scoped)
                },
            )
            .map_err(map_broker_error)?
    }

    fn create_utf8_durable(
        &self,
        path: &RelativeWorkspacePath,
        content: &str,
    ) -> Result<(), WorkspaceIoError> {
        self.broker
            .create_utf8_durable(
                &self.workspace_id,
                self.grant_epoch,
                self.governed_edit_epoch,
                path.as_str(),
                content,
            )
            .map_err(map_broker_error)
    }

    fn replace_utf8_durable(
        &self,
        path: &RelativeWorkspacePath,
        expected_content_hash: Sha256Digest,
        expected_file_identity_hash: Sha256Digest,
        content: &str,
    ) -> Result<(), WorkspaceIoError> {
        self.broker
            .replace_utf8_durable(
                &self.workspace_id,
                self.grant_epoch,
                self.governed_edit_epoch,
                path.as_str(),
                &expected_content_hash.to_string(),
                &expected_file_identity_hash.to_string(),
                content,
            )
            .map_err(map_broker_error)
    }

    fn delete_durable(
        &self,
        path: &RelativeWorkspacePath,
        expected_content_hash: Sha256Digest,
        expected_file_identity_hash: Sha256Digest,
    ) -> Result<(), WorkspaceIoError> {
        self.broker
            .delete_durable(
                &self.workspace_id,
                self.grant_epoch,
                self.governed_edit_epoch,
                path.as_str(),
                &expected_content_hash.to_string(),
                &expected_file_identity_hash.to_string(),
            )
            .map_err(map_broker_error)
    }
}

struct ScopedRecoveryWorkspaceIo<'transaction, 'scope> {
    transaction: &'transaction GovernedRecoveryTransaction<'scope>,
    workspace_target_hash: Sha256Digest,
}

impl WorkspaceFileIo for ScopedRecoveryWorkspaceIo<'_, '_> {
    fn with_recovery_transaction(
        &self,
        _transaction: &mut dyn FnMut(&dyn WorkspaceFileIo) -> Result<(), WorkspaceIoError>,
    ) -> Result<(), WorkspaceIoError> {
        Err(WorkspaceIoError::CapabilityRevoked)
    }

    fn workspace_target_hash(&self) -> Result<Sha256Digest, WorkspaceIoError> {
        Ok(self.workspace_target_hash)
    }

    fn read_file(
        &self,
        path: &RelativeWorkspacePath,
        expected_file_identity_hash: Option<Sha256Digest>,
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
        let observation = self
            .transaction
            .observe_preimage(path.as_str())
            .map_err(map_broker_error)?;
        map_recovery_observation(path, observation)
    }

    fn create_utf8_durable(
        &self,
        path: &RelativeWorkspacePath,
        content: &str,
    ) -> Result<(), WorkspaceIoError> {
        self.transaction
            .create_utf8_durable(path.as_str(), content)
            .map_err(map_broker_error)
    }

    fn replace_utf8_durable(
        &self,
        path: &RelativeWorkspacePath,
        expected_content_hash: Sha256Digest,
        expected_file_identity_hash: Sha256Digest,
        content: &str,
    ) -> Result<(), WorkspaceIoError> {
        self.transaction
            .replace_utf8_durable(
                path.as_str(),
                &expected_content_hash.to_string(),
                &expected_file_identity_hash.to_string(),
                content,
            )
            .map_err(map_broker_error)
    }

    fn delete_durable(
        &self,
        path: &RelativeWorkspacePath,
        expected_content_hash: Sha256Digest,
        expected_file_identity_hash: Sha256Digest,
    ) -> Result<(), WorkspaceIoError> {
        self.transaction
            .delete_durable(
                path.as_str(),
                &expected_content_hash.to_string(),
                &expected_file_identity_hash.to_string(),
            )
            .map_err(map_broker_error)
    }
}

fn map_recovery_observation(
    expected_path: &RelativeWorkspacePath,
    observation: PreimageObservation,
) -> Result<WorkspaceFileObservation, WorkspaceIoError> {
    if observation.relative_path != expected_path.as_str()
        || observation.exists != observation.content.is_some()
        || observation.exists != observation.file_identity_hash.is_some()
    {
        return Err(WorkspaceIoError::Unavailable);
    }
    let file_identity_hash = observation
        .file_identity_hash
        .as_deref()
        .map(Sha256Digest::parse)
        .transpose()
        .map_err(|_| WorkspaceIoError::Unavailable)?;
    Ok(WorkspaceFileObservation {
        content: observation.content.map(String::into_bytes),
        file_identity_hash,
    })
}

fn map_broker_error(error: WorkspaceError) -> WorkspaceIoError {
    match error {
        WorkspaceError::AlreadyExists => WorkspaceIoError::AlreadyExists,
        WorkspaceError::UnsupportedText => WorkspaceIoError::UnsupportedContent,
        WorkspaceError::StalePreimage
        | WorkspaceError::GrantUnavailable
        | WorkspaceError::RootIdentityChanged
        | WorkspaceError::EditsNotEnabled => WorkspaceIoError::CapabilityRevoked,
        WorkspaceError::Io(io_error) if io_error.kind() == std::io::ErrorKind::NotFound => {
            WorkspaceIoError::NotFound
        }
        _ => WorkspaceIoError::Unavailable,
    }
}

/// Bridges the D3 engine's durability port onto the encrypted local store.
struct DurableExecutionStore<'a> {
    store: &'a LocalStore,
    workspace_id: String,
    workspace_grant_epoch: u64,
    correlation_id: String,
}

impl DurableExecutionStore<'_> {
    fn evidence(&self, journal: &EffectJournal, event_type: &str, payload: &str) -> EvidenceAppend {
        EvidenceAppend {
            stream_id: format!("execution:{}", journal.execution_id.as_str()),
            event_type: event_type.to_owned(),
            payload_hash: sha256_bytes(payload.as_bytes()).to_string(),
            payload_ref: None,
            correlation_id: self.correlation_id.clone(),
            causation_id: None,
            redaction_level: "metadata".to_owned(),
            retention_class: "evidence".to_owned(),
        }
    }
}

impl ExecutionStore for DurableExecutionStore<'_> {
    fn persist_checkpoint(
        &self,
        checkpoint: &LocalCheckpoint,
    ) -> Result<(), desktop_execution::JournalStoreError> {
        let checkpoint_json =
            serde_json::to_vec(checkpoint).map_err(|_| desktop_execution::JournalStoreError)?;
        self.store
            .persist_execution_checkpoint(&ExecutionCheckpointAppend {
                checkpoint_id: checkpoint.checkpoint_id.as_str().to_owned(),
                workspace_target_hash: checkpoint.workspace_target_hash.to_string(),
                candidate_hash: checkpoint.candidate_hash.to_string(),
                manifest_hash: checkpoint.manifest_hash.to_string(),
                entry_count: u32::try_from(checkpoint.entries.len())
                    .map_err(|_| desktop_execution::JournalStoreError)?,
                checkpoint_json,
            })
            .map_err(|_| desktop_execution::JournalStoreError)
    }

    fn create_journal(
        &self,
        journal: &EffectJournal,
    ) -> Result<(), desktop_execution::JournalStoreError> {
        let journal_json =
            serde_json::to_string(journal).map_err(|_| desktop_execution::JournalStoreError)?;
        let event = self.evidence(journal, "execution.journal-created", &journal_json);
        self.store
            .create_effect_journal(
                &EffectJournalUpsert {
                    journal_id: journal.journal_id.as_str().to_owned(),
                    execution_id: journal.execution_id.as_str().to_owned(),
                    checkpoint_id: journal.checkpoint_id.as_str().to_owned(),
                    candidate_hash: journal.candidate_hash.to_string(),
                    spec_hash: journal.spec_hash.to_string(),
                    consumption_hash: journal.consumption_hash.to_string(),
                    workspace_id: self.workspace_id.clone(),
                    workspace_grant_epoch: self.workspace_grant_epoch,
                    state: journal_state_label(journal.state).to_owned(),
                    journal_json,
                },
                &event,
            )
            .map_err(|_| desktop_execution::JournalStoreError)
    }

    fn update_journal(
        &self,
        journal: &EffectJournal,
    ) -> Result<(), desktop_execution::JournalStoreError> {
        let journal_json =
            serde_json::to_string(journal).map_err(|_| desktop_execution::JournalStoreError)?;
        let event = matches!(
            journal.state,
            JournalState::RecoveryRequired
                | JournalState::Restoring
                | JournalState::Recovered
                | JournalState::ManualReview
        )
        .then(|| self.evidence(journal, "execution.recovery-state", &journal_json));
        self.store
            .update_effect_journal(
                journal.journal_id.as_str(),
                journal_state_label(journal.state),
                &journal_json,
                event.as_ref(),
            )
            .map_err(|_| desktop_execution::JournalStoreError)
    }

    fn record_result(
        &self,
        result: &LocalExecutionResult,
        journal: &EffectJournal,
    ) -> Result<(), desktop_execution::JournalStoreError> {
        let result_json =
            serde_json::to_string(result).map_err(|_| desktop_execution::JournalStoreError)?;
        let journal_json =
            serde_json::to_string(journal).map_err(|_| desktop_execution::JournalStoreError)?;
        let event = self.evidence(journal, "execution.result-recorded", &result_json);
        self.store
            .record_execution_result(
                &ExecutionResultAppend {
                    execution_id: result.execution_id.as_str().to_owned(),
                    journal_id: result.journal_id.as_str().to_owned(),
                    checkpoint_id: result.checkpoint_id.as_str().to_owned(),
                    candidate_hash: result.candidate_hash.to_string(),
                    spec_hash: result.spec_hash.to_string(),
                    consumption_hash: result.consumption_hash.to_string(),
                    result_hash: result.result_hash.to_string(),
                    result_json,
                    file_count: u32::try_from(result.files.len())
                        .map_err(|_| desktop_execution::JournalStoreError)?,
                    journal_json,
                },
                &event,
            )
            .map_err(|_| desktop_execution::JournalStoreError)
    }
}

const fn journal_state_label(state: JournalState) -> &'static str {
    match state {
        JournalState::Prepared => "prepared",
        JournalState::CheckpointDurable => "checkpoint_durable",
        JournalState::PreconditionsVerified => "preconditions_verified",
        JournalState::Applying => "applying",
        JournalState::EffectsApplied => "effects_applied",
        JournalState::PostimagesVerified => "postimages_verified",
        JournalState::ResultRecorded => "result_recorded",
        JournalState::Completed => "completed",
        JournalState::RecoveryRequired => "recovery_required",
        JournalState::Restoring => "restoring",
        JournalState::Recovered => "recovered",
        JournalState::ManualReview => "manual_review",
    }
}

/// Derives the deterministic workspace target bound into candidates, specs,
/// and checkpoints for one grant epoch of one selected workspace.
fn workspace_target(
    workspace_id: &str,
    grant_epoch: u64,
    root_identity_hash: Sha256Digest,
    base_checkpoint_id: ContractId,
) -> Result<WorkspaceTarget, LocalError> {
    let workspace_capability_id =
        ContractId::new(workspace_id).map_err(|_| invalid_request("Unknown local workspace."))?;
    let filesystem_capability_hash = sha256_bytes(
        format!(
            "sapphirus:filesystem-capability:v1\n{}\n{grant_epoch}",
            root_identity_hash.hex_value()
        )
        .as_bytes(),
    );
    let workspace_manifest_hash = sha256_bytes(
        format!(
            "sapphirus:workspace-manifest:v1\n{workspace_id}\n{grant_epoch}\n{}",
            root_identity_hash.hex_value()
        )
        .as_bytes(),
    );
    Ok(WorkspaceTarget {
        target_kind: "local_folder_capability".to_owned(),
        workspace_capability_id,
        grant_epoch,
        root_identity_hash,
        filesystem_capability_hash,
        base_checkpoint_id,
        workspace_manifest_hash,
    })
}

fn latest_completed_checkpoint(
    store: &LocalStore,
    scope: &WorkspaceEditsScope,
) -> Result<ContractId, LocalError> {
    let checkpoint = store
        .latest_completed_checkpoint(scope.workspace_id.as_str(), scope.grant_epoch)
        .map_err(|_| recovery_error())?
        .unwrap_or_else(|| GENESIS_CHECKPOINT_ID.to_owned());
    ContractId::new(checkpoint).map_err(|_| recovery_error())
}

pub(crate) fn current_workspace_target_hash(
    store: &LocalStore,
    workspace_id: &ContractId,
    grant_epoch: u64,
    root_identity_hash: Sha256Digest,
) -> Result<Sha256Digest, LocalError> {
    let scope = WorkspaceEditsScope {
        workspace_id: workspace_id.clone(),
        grant_epoch,
        // Hash-derivation scope only: it never reaches governed IO, and a
        // zero edit epoch fails closed everywhere governed IO validates it.
        governed_edit_epoch: 0,
        root_identity_hash,
    };
    let target = workspace_target(
        workspace_id.as_str(),
        grant_epoch,
        root_identity_hash,
        latest_completed_checkpoint(store, &scope)?,
    )?;
    canonical_hash("workspace-target", 1, &target).map_err(|_| recovery_error())
}

fn executor_audience(installation_id: &ContractId) -> NativePatchEngineAudience {
    let host_build_id = format!("sapphirus-desktop/{}", env!("CARGO_PKG_VERSION"));
    NativePatchEngineAudience {
        audience_kind: "native_patch_engine".to_owned(),
        installation_id: installation_id.clone(),
        // Authenticode signing is a later gate; until then the audience binds
        // the reviewed build identifier rather than a signed binary digest.
        host_binary_sha256: sha256_bytes(host_build_id.as_bytes()),
        patch_engine_profile_hash: sha256_bytes(PATCH_ENGINE_PROFILE.as_bytes()),
        host_build_id,
    }
}

fn new_id(prefix: &str) -> Result<ContractId, LocalError> {
    ContractId::new(format!("{prefix}_{}", Ulid::new())).map_err(|_| recovery_error())
}

fn observed_preimage(observation: PreimageObservation) -> Result<ObservedPreimage, LocalError> {
    let parse = |value: Option<String>| -> Result<Option<Sha256Digest>, LocalError> {
        value
            .map(|hash| {
                Sha256Digest::parse(&hash)
                    .map_err(|_| conflict_error("The workspace observation is inconsistent."))
            })
            .transpose()
    };
    Ok(ObservedPreimage {
        relative_path: RelativeWorkspacePath::new(observation.relative_path)
            .map_err(|_| invalid_request("The requested workspace path is invalid."))?,
        exists: observation.exists,
        content: observation.content,
        content_hash: parse(observation.content_hash)?,
        file_identity_hash: parse(observation.file_identity_hash)?,
        metadata_hash: parse(observation.metadata_hash)?,
    })
}

fn map_edits_error(error: &EditsError) -> LocalError {
    match error {
        EditsError::EmptyProposal => invalid_request("Propose at least one file change."),
        EditsError::TooManyChanges => {
            invalid_request("The proposal exceeds the governed changed-file limit.")
        }
        EditsError::DuplicatePath => {
            invalid_request("The proposal names the same file more than once.")
        }
        EditsError::NoOpChange => invalid_request(
            "The proposed content matches the current file; there is nothing to apply.",
        ),
        EditsError::MissingTarget => {
            conflict_error("A file in the proposal no longer exists; refresh and revise.")
        }
        EditsError::PreimageMismatch => {
            conflict_error("The workspace changed while preparing the proposal; retry.")
        }
        EditsError::InvalidDomain(_) | EditsError::Hash(_) => {
            invalid_request("The proposal violates a governed edit invariant.")
        }
    }
}

fn map_execution_error(error: &ExecutionError) -> LocalError {
    match error {
        ExecutionError::AuthorizationMismatch => {
            conflict_error("The approval no longer authorizes this exact change; review it again.")
        }
        ExecutionError::PreconditionFailed => conflict_error(
            "The workspace changed after review; the change was not applied. Review it again.",
        ),
        ExecutionError::UnsupportedContent => {
            invalid_request("A file in the change is not supported bounded UTF-8 text.")
        }
        ExecutionError::WorkspaceFailure => conflict_error(
            "The workspace rejected the governed effect; no partial change was kept.",
        ),
        ExecutionError::RecoveryRequired => LocalError::new(
            LocalErrorCode::Conflict,
            "The change may be partially applied and was journaled for recovery review.",
            false,
        ),
        ExecutionError::StoreFailure
        | ExecutionError::IntegrityFailure
        | ExecutionError::InvalidDomain(_) => recovery_error(),
    }
}

struct WorkspaceEditsScope {
    workspace_id: ContractId,
    grant_epoch: u64,
    governed_edit_epoch: u64,
    root_identity_hash: Sha256Digest,
}

fn authorize_edits_scope(
    state: &HostState,
    workspace_id: &ContractId,
    expected_grant_epoch: u64,
) -> Result<WorkspaceEditsScope, LocalError> {
    let scope = state
        .workspace
        .authorize_scope(workspace_id.as_str(), expected_grant_epoch)
        .map_err(|error| crate::commands::map_workspace_error(&error))?;
    let binding = scope.authority_binding();
    drop(scope);
    let root_identity_hash =
        Sha256Digest::parse(&binding.root_identity_hash).map_err(|_| recovery_error())?;
    Ok(WorkspaceEditsScope {
        workspace_id: workspace_id.clone(),
        grant_epoch: binding.grant_epoch,
        governed_edit_epoch: binding.governed_edit_epoch,
        root_identity_hash,
    })
}

fn changes_proposal_binding(
    state: &HostState,
    authority: &ReadyAuthorityGuard<'_>,
    scope: &WorkspaceEditsScope,
    accepted_at: UnixMillis,
) -> Result<ChangesProposalBinding, LocalError> {
    let identity = state
        .local_identity(authority)
        .map_err(|_| recovery_error())?;
    let authority_ref = identity.authority_ref().map_err(|_| recovery_error())?;
    let owner_scope_ref = identity.owner_scope_ref().clone();
    let store = state.local_store(authority)?;
    let target = workspace_target(
        scope.workspace_id.as_str(),
        scope.grant_epoch,
        scope.root_identity_hash,
        latest_completed_checkpoint(store, scope)?,
    )?;
    let mutable_inputs = vec![MutableInputBinding {
        input_kind: InputKind::WorkspaceManifest,
        input_id: scope.workspace_id.as_str().to_owned(),
        content_hash: target.workspace_manifest_hash,
    }];
    Ok(ChangesProposalBinding {
        proposal_id: new_id("proposal")?,
        candidate_id: new_id("candidate")?,
        project_id: new_id("project")?,
        run_id: new_id("run")?,
        owner_scope_ref,
        executor_audience: executor_audience(&authority_ref.installation_id),
        authority_ref,
        policy_context_hash: sha256_bytes(LOCAL_EDITS_POLICY.as_bytes()),
        workspace_target: target,
        mutable_inputs,
        created_at: accepted_at,
        expires_at: UnixMillis(accepted_at.0.saturating_add(PROPOSAL_REVIEW_WINDOW_MS)),
    })
}

fn prepare_review(
    state: &HostState,
    authority: &ReadyAuthorityGuard<'_>,
    scope: &WorkspaceEditsScope,
    changes: &[ProposedFileChange],
    kind: ChangesProposalKind,
    source_execution_id: Option<ContractId>,
    accepted_at: UnixMillis,
) -> Result<HostCommandData, LocalError> {
    let mut preimages = Vec::with_capacity(changes.len());
    for change in changes {
        let observation = state
            .workspace
            .observe_preimage(
                scope.workspace_id.as_str(),
                scope.grant_epoch,
                scope.governed_edit_epoch,
                change.relative_path().as_str(),
            )
            .map_err(|error| crate::commands::map_workspace_error(&error))?;
        preimages.push(observed_preimage(observation)?);
    }

    let binding = changes_proposal_binding(state, authority, scope, accepted_at)?;
    let prepared = build_changes_candidate(&binding, changes, &preimages)
        .map_err(|error| map_edits_error(&error))?;
    let review = build_changes_review(
        &prepared,
        &preimages,
        scope.workspace_id.as_str(),
        scope.grant_epoch,
        kind,
        source_execution_id,
    )
    .map_err(|error| map_edits_error(&error))?;
    let displayed_diff_hash = review.displayed_diff_hash().map_err(|_| recovery_error())?;

    let approval_id = new_id("approval")?;
    state.insert_pending_proposal(
        approval_id.as_str().to_owned(),
        PendingChangesProposal {
            prepared,
            review: review.clone(),
            displayed_diff_hash,
            workspace_id: scope.workspace_id.as_str().to_owned(),
            workspace_grant_epoch: scope.grant_epoch,
            workspace_governed_edit_epoch: scope.governed_edit_epoch,
        },
    );
    Ok(HostCommandData::ChangesReview(ChangesReviewWire {
        approval_id,
        displayed_diff_hash,
        review,
    }))
}

/// Executes one governed-changes command variant. Every other variant is a
/// caller error and fails closed.
pub(crate) fn execute_changes_command(
    state: &HostState,
    request_id: &ContractId,
    accepted_at: UnixMillis,
    command: desktop_runtime::LocalCommand,
) -> Result<HostCommandData, LocalError> {
    use desktop_runtime::LocalCommand;

    match command {
        LocalCommand::EnableWorkspaceEdits {
            workspace_id,
            workspace_grant_epoch,
        } => enable_workspace_edits(state, request_id, &workspace_id, workspace_grant_epoch),
        LocalCommand::ProposeChanges {
            workspace_id,
            workspace_grant_epoch,
            changes,
        } => propose_changes(
            state,
            &workspace_id,
            workspace_grant_epoch,
            &changes,
            accepted_at,
        ),
        LocalCommand::DecideApproval {
            approval_id,
            candidate_hash,
            displayed_diff_hash,
            choice,
        } => decide_approval(
            state,
            request_id,
            &approval_id,
            candidate_hash,
            displayed_diff_hash,
            choice,
            accepted_at,
        ),
        LocalCommand::RequestRollback { execution_id } => {
            request_rollback(state, &execution_id, accepted_at)
        }
        LocalCommand::ChangesHistory {
            workspace_id,
            workspace_grant_epoch,
        } => changes_history(state, &workspace_id, workspace_grant_epoch),
        _ => Err(invalid_request(
            "The command is not a governed-changes command.",
        )),
    }
}

/// Enables governed edits for one workspace at an exact reviewed epoch. The
/// broker advances the grant epoch, which invalidates every prior proposal
/// and authority binding, and the updated projection is durably persisted.
fn enable_workspace_edits(
    state: &HostState,
    request_id: &ContractId,
    workspace_id: &ContractId,
    expected_grant_epoch: u64,
) -> Result<HostCommandData, LocalError> {
    let authority = state.ready_workspace_commit()?;
    let current = state
        .workspace
        .authorize_scope(workspace_id.as_str(), expected_grant_epoch)
        .map_err(|error| crate::commands::map_workspace_error(&error))?;
    drop(current);
    let projection = state
        .workspace
        .enable_governed_edits(workspace_id.as_str())
        .map_err(|error| crate::commands::map_workspace_error(&error))?;
    if let Err(error) = state.persist_workspace_update(
        authority.authority(),
        &projection,
        "workspace.edits_enabled",
        request_id,
    ) {
        if error.code == LocalErrorCode::RecoveryRequired {
            authority.enter_recovery();
        } else {
            drop(authority);
        }
        return Err(error);
    }
    state.record_event(ProjectionEventKind::WorkspaceChanged {
        workspace_id: workspace_id.clone(),
    });
    Ok(HostCommandData::WorkspaceEditsEnabled(projection))
}

fn propose_changes(
    state: &HostState,
    workspace_id: &ContractId,
    workspace_grant_epoch: u64,
    changes: &[ProposedFileChange],
    accepted_at: UnixMillis,
) -> Result<HostCommandData, LocalError> {
    let authority = state.ready_authority()?;
    let scope = authorize_edits_scope(state, workspace_id, workspace_grant_epoch)?;
    prepare_review(
        state,
        &authority,
        &scope,
        changes,
        ChangesProposalKind::Edit,
        None,
        accepted_at,
    )
}

fn decide_approval(
    state: &HostState,
    request_id: &ContractId,
    approval_id: &ContractId,
    candidate_hash: Sha256Digest,
    displayed_diff_hash: Sha256Digest,
    choice: ApprovalChoice,
    accepted_at: UnixMillis,
) -> Result<HostCommandData, LocalError> {
    let commit = state.ready_workspace_commit()?;
    // A pending proposal is single-use: any decision, failed apply, or drift
    // consumes it and a fresh review is required.
    let Some(pending) = state.take_pending_proposal(approval_id.as_str()) else {
        return Err(not_found_error(
            "The reviewed proposal is no longer available; review the changes again.",
        ));
    };
    if pending.prepared.candidate.candidate_hash != candidate_hash
        || pending.displayed_diff_hash != displayed_diff_hash
    {
        return Err(conflict_error(
            "The decision does not match the reviewed changes; review them again.",
        ));
    }
    if accepted_at > pending.review.expires_at {
        return Err(conflict_error(
            "The review expired before a decision was made; review the changes again.",
        ));
    }

    match choice {
        ApprovalChoice::Discard => Ok(HostCommandData::ChangesDecision(ChangesDecisionWire {
            approval_id: approval_id.clone(),
            disposition: "discarded".to_owned(),
            execution: None,
        })),
        ApprovalChoice::Revise => Ok(HostCommandData::ChangesDecision(ChangesDecisionWire {
            approval_id: approval_id.clone(),
            disposition: "revise_requested".to_owned(),
            execution: None,
        })),
        ApprovalChoice::Apply => {
            let execution = apply_pending(
                state,
                commit.authority(),
                request_id,
                approval_id,
                &pending,
                accepted_at,
            )?;
            Ok(HostCommandData::ChangesDecision(ChangesDecisionWire {
                approval_id: approval_id.clone(),
                disposition: "applied".to_owned(),
                execution: Some(execution),
            }))
        }
    }
}

/// Seals the approval decision, the single-use execution spec, and its
/// consumption record for one pending proposal.
fn issue_single_use_authorization(
    state: &HostState,
    authority: &ReadyAuthorityGuard<'_>,
    approval_id: &ContractId,
    pending: &PendingChangesProposal,
    accepted_at: UnixMillis,
) -> Result<
    (
        desktop_runtime::ApprovedExecutionSpec,
        desktop_runtime::SpecConsumptionRecord,
        ContractId,
    ),
    LocalError,
> {
    let identity = state
        .local_identity(authority)
        .map_err(|_| recovery_error())?;
    let authority_ref = identity.authority_ref().map_err(|_| recovery_error())?;
    let candidate = &pending.prepared.candidate;

    let approval = ApprovalDecisionDraft::approved(
        approval_id.clone(),
        candidate,
        pending.displayed_diff_hash,
        accepted_at,
    )
    .seal()
    .map_err(|_| recovery_error())?;

    let nonce_hash = sha256_bytes(
        format!(
            "sapphirus:spec-nonce:{}:{}:{}",
            Ulid::new(),
            Ulid::new(),
            accepted_at.0
        )
        .as_bytes(),
    );
    let workspace_target_hash =
        canonical_hash("workspace-target", 1, &candidate.draft.workspace_target)
            .map_err(|_| recovery_error())?;
    let mutable_input_set_hash = canonical_hash(
        "mutable-input-set",
        1,
        &candidate.draft.common.mutable_inputs,
    )
    .map_err(|_| recovery_error())?;
    let spec = ApprovedExecutionSpecDraft {
        schema_version: "sapphirus.approved-execution-spec.v1".to_owned(),
        spec_id: new_id("spec")?,
        delivery_model: DeliveryModel::WindowsLocal,
        authority_ref: authority_ref.clone(),
        owner_scope_ref: candidate.draft.common.owner_scope_ref.clone(),
        project_id: candidate.draft.common.project_id.clone(),
        run_id: candidate.draft.common.run_id.clone(),
        proposal_id: candidate.draft.common.proposal_id.clone(),
        proposal_hash: candidate.draft.common.proposal_hash,
        candidate_id: candidate.draft.common.candidate_id.clone(),
        candidate_hash: candidate.candidate_hash,
        approval_id: approval.draft.approval_id.clone(),
        approval_decision_hash: approval.approval_decision_hash,
        policy_version: LOCAL_EDITS_POLICY.to_owned(),
        policy_hash: candidate.draft.common.policy_context_hash,
        workspace_target_hash,
        mutable_input_set_hash,
        executor_audience: candidate.draft.executor_audience.clone(),
        issued_at: accepted_at,
        expires_at: UnixMillis(accepted_at.0.saturating_add(SPEC_VALIDITY_WINDOW_MS)),
        single_use_nonce_hash: nonce_hash,
    }
    .seal()
    .map_err(|_| recovery_error())?;

    let executor_audience_hash =
        canonical_hash("executor-audience", 1, &spec.draft.executor_audience)
            .map_err(|_| recovery_error())?;
    let execution_id = new_id("execution")?;
    let consumption = SpecConsumptionRecordDraft {
        schema_version: "sapphirus.spec-consumption.v1".to_owned(),
        consumption_id: new_id("consumption")?,
        delivery_model: DeliveryModel::WindowsLocal,
        authority_ref,
        spec_id: spec.draft.spec_id.clone(),
        spec_hash: spec.spec_hash,
        candidate_hash: candidate.candidate_hash,
        single_use_nonce_hash: nonce_hash,
        executor_audience_hash,
        execution_id: execution_id.clone(),
        attempt_number: 1,
        consumed_at: accepted_at,
    }
    .seal()
    .map_err(|_| recovery_error())?;
    Ok((spec, consumption, execution_id))
}

fn apply_pending(
    state: &HostState,
    authority: &ReadyAuthorityGuard<'_>,
    request_id: &ContractId,
    approval_id: &ContractId,
    pending: &PendingChangesProposal,
    accepted_at: UnixMillis,
) -> Result<ChangesExecutionWire, LocalError> {
    let workspace_id =
        ContractId::new(pending.workspace_id.clone()).map_err(|_| recovery_error())?;
    let scope = authorize_edits_scope(state, &workspace_id, pending.workspace_grant_epoch)?;
    if scope.governed_edit_epoch != pending.workspace_governed_edit_epoch {
        return Err(conflict_error(
            "Edit authority changed after review; propose the changes again.",
        ));
    }
    let (spec, consumption, execution_id) =
        issue_single_use_authorization(state, authority, approval_id, pending, accepted_at)?;
    let candidate = &pending.prepared.candidate;

    let store = state.local_store(authority)?;
    // Durable single-use consumption happens before any file effect; a
    // duplicate spec or replay fails closed here.
    store
        .consume_spec_record(&consumption)
        .map_err(|error| match error {
            StoreError::AlreadyConsumed => {
                conflict_error("This approval was already consumed; review the changes again.")
            }
            _ => recovery_error(),
        })?;

    let effect_io = GovernedWorkspaceIo {
        broker: &state.workspace,
        workspace_id: scope.workspace_id.as_str().to_owned(),
        grant_epoch: scope.grant_epoch,
        governed_edit_epoch: scope.governed_edit_epoch,
        expected_root_identity_hash: scope.root_identity_hash,
        workspace_target_hash: canonical_hash(
            "workspace-target",
            1,
            &candidate.draft.workspace_target,
        )
        .map_err(|_| recovery_error())?,
    };
    let execution_store = DurableExecutionStore {
        store,
        workspace_id: scope.workspace_id.as_str().to_owned(),
        workspace_grant_epoch: scope.grant_epoch,
        correlation_id: request_id.to_string(),
    };
    let journal_id = new_id("journal")?;
    let checkpoint_id = new_id("checkpoint")?;
    let outcome = PatchExecutor::new(&effect_io, &execution_store)
        .apply(ExecutionRequest {
            journal_id,
            checkpoint_id,
            candidate,
            patch: &pending.prepared.patch,
            spec: &spec,
            consumption: &consumption,
            started_at: accepted_at,
            completed_at: now(),
        })
        .map_err(|error| {
            state.record_event(ProjectionEventKind::ExecutionStateChanged {
                execution_id: execution_id.clone(),
                state: "failed".to_owned(),
            });
            map_execution_error(&error)
        })?;

    state.record_event(ProjectionEventKind::ExecutionStateChanged {
        execution_id: outcome.result.execution_id.clone(),
        state: journal_state_label(outcome.journal.state).to_owned(),
    });
    state.record_event(ProjectionEventKind::CheckpointChanged {
        checkpoint_id: outcome.checkpoint.checkpoint_id.clone(),
        rollback_available: true,
    });

    Ok(ChangesExecutionWire {
        execution_id: outcome.result.execution_id.clone(),
        checkpoint_id: outcome.checkpoint.checkpoint_id.clone(),
        completed_at: outcome.result.completed_at,
        undoable: true,
        files: outcome
            .result
            .files
            .iter()
            .map(|file| AppliedFileWire {
                relative_path: file.relative_path.clone(),
                operation: format!("{:?}", file.operation).to_lowercase(),
                exists: file.exists,
                content_hash: file.content_hash.map(|hash| hash.to_string()),
            })
            .collect(),
    })
}

fn request_rollback(
    state: &HostState,
    execution_id: &ContractId,
    accepted_at: UnixMillis,
) -> Result<HostCommandData, LocalError> {
    let authority = state.ready_authority()?;
    let store = state.local_store(&authority)?;
    let result_row = store
        .load_execution_result(execution_id.as_str())
        .map_err(|_| recovery_error())?
        .ok_or_else(|| not_found_error("The applied change is not available for undo."))?;
    let journal_row = store
        .load_effect_journal(&result_row.journal_id)
        .map_err(|_| recovery_error())?
        .ok_or_else(recovery_error)?;
    let (_, checkpoint_bytes) = store
        .load_execution_checkpoint(&result_row.checkpoint_id)
        .map_err(|_| recovery_error())?
        .ok_or_else(recovery_error)?;

    let checkpoint: LocalCheckpoint =
        serde_json::from_slice(&checkpoint_bytes).map_err(|_| recovery_error())?;
    let result: LocalExecutionResult =
        serde_json::from_str(&result_row.result_json).map_err(|_| recovery_error())?;

    let workspace_id =
        ContractId::new(journal_row.workspace_id.clone()).map_err(|_| recovery_error())?;
    let scope = authorize_edits_scope(state, &workspace_id, journal_row.workspace_grant_epoch)?;
    let effect_io = GovernedWorkspaceIo {
        broker: &state.workspace,
        workspace_id: scope.workspace_id.as_str().to_owned(),
        grant_epoch: scope.grant_epoch,
        governed_edit_epoch: scope.governed_edit_epoch,
        expected_root_identity_hash: scope.root_identity_hash,
        workspace_target_hash: checkpoint.workspace_target_hash,
    };
    let plan = plan_rollback(
        &effect_io,
        new_id("rollback")?,
        &checkpoint,
        &result,
        accepted_at,
    )
    .map_err(|error| map_execution_error(&error))?;

    if !plan.conflicts.is_empty() {
        return Ok(HostCommandData::ChangesUndoUnavailable(
            crate::wire::ChangesUndoUnavailableWire {
                execution_id: execution_id.clone(),
                reason: "The workspace changed after this change was applied; undo requires \
                         a fresh proposal."
                    .to_owned(),
                conflicts: plan
                    .conflicts
                    .iter()
                    .map(|conflict| UndoConflictWire {
                        relative_path: conflict.relative_path.clone(),
                        expected_exists: conflict.expected_exists,
                        current_exists: conflict.current_exists,
                    })
                    .collect(),
            },
        ));
    }
    if plan.operations.is_empty() {
        return Err(conflict_error(
            "The workspace already matches the state before this change.",
        ));
    }

    let changes = plan
        .operations
        .iter()
        .map(|operation| match operation {
            desktop_runtime::PatchOperation::Create {
                relative_path,
                content,
                ..
            }
            | desktop_runtime::PatchOperation::Replace {
                relative_path,
                content,
                ..
            } => ProposedFileChange::SetContent {
                relative_path: relative_path.clone(),
                content: content.clone(),
            },
            desktop_runtime::PatchOperation::Delete { relative_path, .. } => {
                ProposedFileChange::Delete {
                    relative_path: relative_path.clone(),
                }
            }
        })
        .collect::<Vec<_>>();
    prepare_review(
        state,
        &authority,
        &scope,
        &changes,
        ChangesProposalKind::Undo,
        Some(execution_id.clone()),
        accepted_at,
    )
}

fn changes_history(
    state: &HostState,
    workspace_id: &ContractId,
    workspace_grant_epoch: u64,
) -> Result<HostCommandData, LocalError> {
    let authority = state.ready_authority()?;
    let scope = authorize_edits_scope(state, workspace_id, workspace_grant_epoch)?;
    let store = state.local_store(&authority)?;
    let results = store
        .list_recent_execution_results(50)
        .map_err(|_| recovery_error())?;
    let open = store
        .list_open_effect_journals()
        .map_err(|_| recovery_error())?;

    let mut entries = Vec::new();
    for result in results {
        let Some(journal) = store
            .load_effect_journal(&result.journal_id)
            .map_err(|_| recovery_error())?
        else {
            continue;
        };
        if journal.workspace_id != scope.workspace_id.as_str() {
            continue;
        }
        entries.push(history_entry(&result, &journal, scope.grant_epoch));
    }
    let open_journals = open
        .iter()
        .filter(|journal| journal.workspace_id == scope.workspace_id.as_str())
        .filter(|journal| {
            matches!(
                journal.state.as_str(),
                "recovery_required" | "restoring" | "manual_review"
            )
        })
        .map(|journal| OpenJournalWire {
            journal_id: journal.journal_id.clone(),
            execution_id: journal.execution_id.clone(),
            state: journal.state.clone(),
            updated_at: journal.updated_at.clone(),
            recovery_availability: recovery_availability_for_open_journal(
                &journal.state,
                journal.workspace_grant_epoch,
                scope.grant_epoch,
            ),
        })
        .collect();
    Ok(HostCommandData::ChangesHistory(ChangesHistoryWire {
        workspace_id: workspace_id.clone(),
        entries,
        open_journals,
    }))
}

fn recovery_availability_for_open_journal(
    state: &str,
    journal_grant_epoch: u64,
    current_grant_epoch: u64,
) -> RecoveryAvailabilityWire {
    match state {
        "recovery_required" => {
            if journal_grant_epoch <= current_grant_epoch {
                RecoveryAvailabilityWire::ReviewAvailable
            } else {
                RecoveryAvailabilityWire::Quarantined
            }
        }
        "restoring" | "manual_review" => RecoveryAvailabilityWire::ManualReview,
        _ => RecoveryAvailabilityWire::Quarantined,
    }
}

fn history_entry(
    result: &ExecutionResultRow,
    journal: &EffectJournalRow,
    current_epoch: u64,
) -> ChangesHistoryEntryWire {
    ChangesHistoryEntryWire {
        execution_id: result.execution_id.clone(),
        journal_state: journal.state.clone(),
        file_count: result.file_count,
        completed_at: result.completed_at.clone(),
        undoable: journal.state == "completed" && journal.workspace_grant_epoch == current_epoch,
    }
}

/// Boot-time reconciliation of interrupted effect journals. The workspace
/// broker restores read-only grants at startup, so no file can be observed or
/// restored here; every disposition is decided from the durable state machine
/// alone and interrupted effects remain quarantined for explicit review.
pub(crate) fn reconcile_execution_journals(store: &LocalStore) -> Result<(), StoreError> {
    for journal in store.list_open_effect_journals()? {
        match journal.state.as_str() {
            // No file operation can have started in these states.
            "prepared" | "checkpoint_durable" | "preconditions_verified" => {
                store.update_effect_journal(
                    &journal.journal_id,
                    "recovered",
                    &journal.journal_json,
                    Some(&reconcile_evidence(&journal, "no file effect had started")),
                )?;
            }
            // The result is durable; only the final marker was interrupted.
            "result_recorded" => {
                store.update_effect_journal(
                    &journal.journal_id,
                    "completed",
                    &journal.journal_json,
                    None,
                )?;
            }
            // Effects may be partial and cannot be observed at boot.
            "applying" | "effects_applied" | "postimages_verified" => {
                store.update_effect_journal(
                    &journal.journal_id,
                    "recovery_required",
                    &journal.journal_json,
                    Some(&reconcile_evidence(
                        &journal,
                        "interrupted mid-effect; review required",
                    )),
                )?;
            }
            "restoring" => {
                store.update_effect_journal(
                    &journal.journal_id,
                    "manual_review",
                    &journal.journal_json,
                    Some(&reconcile_evidence(
                        &journal,
                        "reviewed recovery was interrupted",
                    )),
                )?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn reconcile_evidence(journal: &EffectJournalRow, reason: &str) -> EvidenceAppend {
    EvidenceAppend {
        stream_id: format!("execution:{}", journal.execution_id),
        event_type: "execution.boot-reconciled".to_owned(),
        payload_hash: sha256_bytes(reason.as_bytes()).to_string(),
        payload_ref: None,
        correlation_id: "boot_reconciliation".to_owned(),
        causation_id: None,
        redaction_level: "metadata".to_owned(),
        retention_class: "evidence".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use desktop_runtime::{ApprovalChoice, ContractId, ProposedFileChange, RelativeWorkspacePath};
    use desktop_store::{
        EffectJournalUpsert, EvidenceAppend, ExecutionCheckpointAppend, ExecutionResultAppend,
        LocalStore, StoreError,
    };

    use super::{
        authorize_edits_scope, changes_history, changes_proposal_binding, decide_approval,
        propose_changes, reconcile_execution_journals, recovery_availability_for_open_journal,
        request_rollback,
    };
    use crate::state::{now, HostState};
    use crate::wire::{ChangesReviewWire, HostCommandData, RecoveryAvailabilityWire};

    struct EditsFixture {
        _store_dir: tempfile::TempDir,
        _workspace_dir: tempfile::TempDir,
        state: HostState,
        workspace_id: ContractId,
        grant_epoch: u64,
        workspace_root: std::path::PathBuf,
    }

    fn fixture() -> Result<EditsFixture, Box<dyn std::error::Error>> {
        let store_dir = tempfile::tempdir()?;
        let workspace_dir = tempfile::tempdir()?;
        fs::write(workspace_dir.path().join("main.rs"), "fn main() {}\n")?;

        let state = HostState::initialize(Some(store_dir.path().to_path_buf()))
            .map_err(|error| error.safe_message)?;
        let authority = state
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        let projection = state
            .workspace
            .grant("project_test", workspace_dir.path())?;
        let binding = state
            .workspace
            .authority_binding(&projection.workspace_id)?;
        state
            .persist_workspace(
                &authority,
                projection.clone(),
                workspace_dir.path(),
                &binding.root_identity_hash,
                &ContractId::new("request_grant")?,
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
                &ContractId::new("request_enable")?,
            )
            .map_err(|error| error.safe_message)?;
        drop(authority);

        let workspace_root = workspace_dir.path().to_path_buf();
        Ok(EditsFixture {
            _store_dir: store_dir,
            _workspace_dir: workspace_dir,
            workspace_id: ContractId::new(enabled.workspace_id.clone())?,
            grant_epoch: enabled.grant_epoch,
            state,
            workspace_root,
        })
    }

    fn review_from(data: HostCommandData) -> Result<ChangesReviewWire, String> {
        match data {
            HostCommandData::ChangesReview(review) => Ok(review),
            other => Err(format!("expected a changes review, got {other:?}")),
        }
    }

    fn apply(
        fixture: &EditsFixture,
        review: &ChangesReviewWire,
        request: &str,
    ) -> Result<HostCommandData, desktop_runtime::LocalError> {
        decide_approval(
            &fixture.state,
            &ContractId::new(request).map_err(|_| crate::state::recovery_error())?,
            &review.approval_id,
            review.review.candidate_hash,
            review.displayed_diff_hash,
            ApprovalChoice::Apply,
            now(),
        )
    }

    const TEST_HASH: &str =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn unique_test_hash(value: usize) -> String {
        format!("sha256:{value:064x}")
    }

    fn seed_journal(
        store: &LocalStore,
        index: usize,
        target_state: &str,
        workspace_id: &str,
        workspace_grant_epoch: u64,
    ) -> Result<String, StoreError> {
        let journal_id = format!("journal_boot_{index}");
        let execution_id = format!("execution_boot_{index}");
        let checkpoint_id = format!("checkpoint_boot_{index}");
        let candidate_hash = unique_test_hash(index * 10 + 1);
        let spec_hash = unique_test_hash(index * 10 + 4);
        let consumption_hash = unique_test_hash(index * 10 + 5);
        store.persist_execution_checkpoint(&ExecutionCheckpointAppend {
            checkpoint_id: checkpoint_id.clone(),
            workspace_target_hash: unique_test_hash(index * 10 + 2),
            candidate_hash: candidate_hash.clone(),
            manifest_hash: unique_test_hash(index * 10 + 3),
            entry_count: 0,
            checkpoint_json: br"{}".to_vec(),
        })?;
        store.create_effect_journal(
            &EffectJournalUpsert {
                journal_id: journal_id.clone(),
                execution_id: execution_id.clone(),
                checkpoint_id,
                candidate_hash,
                spec_hash: spec_hash.clone(),
                consumption_hash: consumption_hash.clone(),
                workspace_id: workspace_id.to_owned(),
                workspace_grant_epoch,
                state: "prepared".to_owned(),
                journal_json: "{}".to_owned(),
            },
            &EvidenceAppend {
                stream_id: format!("execution:{execution_id}"),
                event_type: "execution.journal-created".to_owned(),
                payload_hash: TEST_HASH.to_owned(),
                payload_ref: None,
                correlation_id: "test_boot_reconciliation".to_owned(),
                causation_id: None,
                redaction_level: "metadata".to_owned(),
                retention_class: "evidence".to_owned(),
            },
        )?;

        if target_state == "prepared" {
            return Ok(journal_id);
        }

        let path = [
            "checkpoint_durable",
            "preconditions_verified",
            "applying",
            "effects_applied",
            "postimages_verified",
        ];
        if target_state == "result_recorded" {
            for state in path {
                store.update_effect_journal(&journal_id, state, "{}", None)?;
            }
            store.record_execution_result(
                &ExecutionResultAppend {
                    execution_id: execution_id.clone(),
                    journal_id: journal_id.clone(),
                    checkpoint_id: format!("checkpoint_boot_{index}"),
                    candidate_hash: unique_test_hash(index * 10 + 1),
                    spec_hash,
                    consumption_hash,
                    result_hash: unique_test_hash(index * 10 + 6),
                    result_json: "{}".to_owned(),
                    file_count: 1,
                    journal_json: "{}".to_owned(),
                },
                &EvidenceAppend {
                    stream_id: format!("execution:{execution_id}"),
                    event_type: "execution.result-recorded".to_owned(),
                    payload_hash: TEST_HASH.to_owned(),
                    payload_ref: None,
                    correlation_id: "test_boot_reconciliation".to_owned(),
                    causation_id: None,
                    redaction_level: "metadata".to_owned(),
                    retention_class: "evidence".to_owned(),
                },
            )?;
        } else if target_state == "recovery_required" {
            store.update_effect_journal(&journal_id, target_state, "{}", None)?;
        } else if target_state == "restoring" || target_state == "manual_review" {
            store.update_effect_journal(&journal_id, "recovery_required", "{}", None)?;
            if target_state == "restoring" {
                store.update_effect_journal(&journal_id, target_state, "{}", None)?;
            } else {
                store.update_effect_journal(&journal_id, "manual_review", "{}", None)?;
            }
        } else {
            for state in path {
                store.update_effect_journal(&journal_id, state, "{}", None)?;
                if state == target_state {
                    break;
                }
            }
        }
        Ok(journal_id)
    }

    #[test]
    fn boot_reconciliation_is_metadata_only_and_preserves_reviewable_recovery(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let store_dir = tempfile::tempdir()?;
        let workspace_dir = tempfile::tempdir()?;
        let sentinel = workspace_dir.path().join("boot-sentinel.txt");
        fs::write(&sentinel, "workspace must remain untouched\n")?;
        let bytes_before = fs::read(&sentinel)?;
        let metadata_before = fs::metadata(&sentinel)?;
        let size_before = metadata_before.len();
        let modified_before = metadata_before.modified()?;
        let readonly_before = metadata_before.permissions().readonly();

        let state = HostState::initialize(Some(store_dir.path().to_path_buf()))
            .map_err(|error| error.safe_message)?;
        let authority = state
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        let store = state
            .local_store(&authority)
            .map_err(|error| error.safe_message)?;
        let transitions = [
            ("prepared", "recovered"),
            ("checkpoint_durable", "recovered"),
            ("preconditions_verified", "recovered"),
            ("result_recorded", "completed"),
            ("applying", "recovery_required"),
            ("effects_applied", "recovery_required"),
            ("postimages_verified", "recovery_required"),
            ("recovery_required", "recovery_required"),
            ("restoring", "manual_review"),
            ("manual_review", "manual_review"),
        ];

        for (index, (initial, expected)) in transitions.into_iter().enumerate() {
            let journal_id = seed_journal(store, index, initial, "workspace_boot", 1)
                .map_err(|error| format!("failed to seed {initial}: {error:?}"))?;
            reconcile_execution_journals(store)
                .map_err(|error| format!("failed to reconcile {initial}: {error:?}"))?;
            let journal = store
                .load_effect_journal(&journal_id)?
                .ok_or("seeded journal must remain durable")?;
            assert_eq!(journal.state, expected, "{initial} boot disposition");
        }
        drop(authority);

        let metadata_after = fs::metadata(&sentinel)?;
        assert_eq!(fs::read(&sentinel)?, bytes_before);
        assert_eq!(metadata_after.len(), size_before);
        assert_eq!(metadata_after.modified()?, modified_before);
        assert_eq!(metadata_after.permissions().readonly(), readonly_before);
        Ok(())
    }

    #[test]
    fn governed_changes_proposes_applies_and_undoes_a_reviewed_change(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let changes = vec![
            ProposedFileChange::SetContent {
                relative_path: RelativeWorkspacePath::new("main.rs")?,
                content: "fn main() { updated(); }\n".to_owned(),
            },
            ProposedFileChange::SetContent {
                relative_path: RelativeWorkspacePath::new("created.rs")?,
                content: "pub fn created() {}\n".to_owned(),
            },
        ];
        let review = review_from(
            propose_changes(
                &fixture.state,
                &fixture.workspace_id,
                fixture.grant_epoch,
                &changes,
                now(),
            )
            .map_err(|error| error.safe_message)?,
        )?;
        assert_eq!(review.review.files.len(), 2);

        let decision =
            apply(&fixture, &review, "request_apply").map_err(|error| error.safe_message)?;
        let HostCommandData::ChangesDecision(decision) = decision else {
            return Err("expected a decision".into());
        };
        assert_eq!(decision.disposition, "applied");
        let execution = decision.execution.ok_or("expected an execution")?;
        assert_eq!(
            fs::read_to_string(fixture.workspace_root.join("main.rs"))?,
            "fn main() { updated(); }\n"
        );
        assert_eq!(
            fs::read_to_string(fixture.workspace_root.join("created.rs"))?,
            "pub fn created() {}\n"
        );

        let authority = fixture
            .state
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        let scope =
            authorize_edits_scope(&fixture.state, &fixture.workspace_id, fixture.grant_epoch)
                .map_err(|error| error.safe_message)?;
        let next_binding = changes_proposal_binding(&fixture.state, &authority, &scope, now())
            .map_err(|error| error.safe_message)?;
        assert_eq!(
            next_binding.workspace_target.base_checkpoint_id,
            execution.checkpoint_id
        );
        drop(authority);

        // A second decision against the same approval fails closed.
        let replay = apply(&fixture, &review, "request_replay");
        assert!(replay.is_err());

        // Undo produces a fresh reviewed proposal that restores the checkpoint.
        let undo_review = review_from(
            request_rollback(&fixture.state, &execution.execution_id, now())
                .map_err(|error| error.safe_message)?,
        )?;
        let undo_decision =
            apply(&fixture, &undo_review, "request_undo").map_err(|error| error.safe_message)?;
        let HostCommandData::ChangesDecision(undo_decision) = undo_decision else {
            return Err("expected a decision".into());
        };
        assert_eq!(undo_decision.disposition, "applied");
        assert_eq!(
            fs::read_to_string(fixture.workspace_root.join("main.rs"))?,
            "fn main() {}\n"
        );
        assert!(!fixture.workspace_root.join("created.rs").exists());
        Ok(())
    }

    #[test]
    fn edit_epoch_change_after_review_invalidates_the_pending_proposal(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let review = review_from(
            propose_changes(
                &fixture.state,
                &fixture.workspace_id,
                fixture.grant_epoch,
                &[ProposedFileChange::SetContent {
                    relative_path: RelativeWorkspacePath::new("main.rs")?,
                    content: "fn main() { rebound(); }
"
                    .to_owned(),
                }],
                now(),
            )
            .map_err(|error| error.safe_message)?,
        )?;

        // Re-enabling edits advances the governed-edit epoch (ADR-0002);
        // the binding epoch the renderer holds is unchanged, but the
        // reviewed proposal was pinned to the previous edit authority.
        fixture
            .state
            .workspace
            .enable_governed_edits(fixture.workspace_id.as_str())?;
        let result = apply(&fixture, &review, "request_apply_stale_edit_epoch");
        assert!(result.is_err());
        assert_eq!(
            fs::read_to_string(fixture.workspace_root.join("main.rs"))?,
            "fn main() {}
"
        );
        Ok(())
    }

    #[test]
    fn context_read_withdrawal_leaves_a_reviewed_proposal_applicable(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let review = review_from(
            propose_changes(
                &fixture.state,
                &fixture.workspace_id,
                fixture.grant_epoch,
                &[ProposedFileChange::SetContent {
                    relative_path: RelativeWorkspacePath::new("main.rs")?,
                    content: "fn main() { survives_signout(); }
"
                    .to_owned(),
                }],
                now(),
            )
            .map_err(|error| error.safe_message)?,
        )?;

        // Withdrawing D2 context-read authority (model sign-out) must not
        // invalidate D3 edit authority (ADR-0002 independence).
        fixture
            .state
            .workspace
            .advance_context_read_epoch(fixture.workspace_id.as_str())?;
        let decision = apply(&fixture, &review, "request_apply_after_signout")
            .map_err(|error| error.safe_message)?;
        let HostCommandData::ChangesDecision(decision) = decision else {
            return Err("expected a decision".into());
        };
        assert_eq!(decision.disposition, "applied");
        assert_eq!(
            fs::read_to_string(fixture.workspace_root.join("main.rs"))?,
            "fn main() { survives_signout(); }
"
        );
        Ok(())
    }

    #[test]
    fn external_edit_after_review_fails_closed_without_effects(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let review = review_from(
            propose_changes(
                &fixture.state,
                &fixture.workspace_id,
                fixture.grant_epoch,
                &[ProposedFileChange::SetContent {
                    relative_path: RelativeWorkspacePath::new("main.rs")?,
                    content: "fn main() { governed(); }\n".to_owned(),
                }],
                now(),
            )
            .map_err(|error| error.safe_message)?,
        )?;

        fs::write(fixture.workspace_root.join("main.rs"), "// external edit\n")?;
        let result = apply(&fixture, &review, "request_apply");
        assert!(result.is_err());
        assert_eq!(
            fs::read_to_string(fixture.workspace_root.join("main.rs"))?,
            "// external edit\n"
        );
        Ok(())
    }

    #[test]
    fn proposals_require_governed_edit_authority() -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let stale_epoch = fixture.grant_epoch + 1;
        let result = propose_changes(
            &fixture.state,
            &fixture.workspace_id,
            stale_epoch,
            &[ProposedFileChange::Delete {
                relative_path: RelativeWorkspacePath::new("main.rs")?,
            }],
            now(),
        );
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn recovery_availability_is_closed_and_bound_to_current_journal_authority() {
        assert_eq!(
            recovery_availability_for_open_journal("recovery_required", 8, 8),
            RecoveryAvailabilityWire::ReviewAvailable,
        );
        assert_eq!(
            recovery_availability_for_open_journal("recovery_required", 7, 8),
            RecoveryAvailabilityWire::ReviewAvailable,
        );
        assert_eq!(
            recovery_availability_for_open_journal("recovery_required", 9, 8),
            RecoveryAvailabilityWire::Quarantined,
        );
        for state in ["restoring", "manual_review"] {
            assert_eq!(
                recovery_availability_for_open_journal(state, 8, 8),
                RecoveryAvailabilityWire::ManualReview,
            );
        }
    }

    #[test]
    fn changes_history_serializes_authenticated_recovery_availability(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let authority = fixture
            .state
            .ready_authority()
            .map_err(|error| error.safe_message)?;
        let store = fixture
            .state
            .local_store(&authority)
            .map_err(|error| error.safe_message)?;
        seed_journal(
            store,
            100,
            "recovery_required",
            fixture.workspace_id.as_str(),
            fixture.grant_epoch,
        )?;
        // An epoch from a foreign grant lineage (ADR-0002: the binding
        // epoch no longer advances on edit enablement); foreign-lineage
        // journals must quarantine. The past-epoch review-available case is
        // pinned by recovery_availability_is_closed_and_bound_to_current_
        // journal_authority.
        seed_journal(
            store,
            101,
            "recovery_required",
            fixture.workspace_id.as_str(),
            fixture.grant_epoch + 1,
        )?;
        seed_journal(
            store,
            102,
            "manual_review",
            fixture.workspace_id.as_str(),
            fixture.grant_epoch,
        )?;
        drop(authority);

        let HostCommandData::ChangesHistory(history) =
            changes_history(&fixture.state, &fixture.workspace_id, fixture.grant_epoch)
                .map_err(|error| error.safe_message)?
        else {
            return Err("expected changes history".into());
        };
        let availability = history
            .open_journals
            .iter()
            .map(|journal| journal.recovery_availability)
            .collect::<Vec<_>>();
        assert_eq!(
            availability,
            [
                RecoveryAvailabilityWire::ReviewAvailable,
                RecoveryAvailabilityWire::Quarantined,
                RecoveryAvailabilityWire::ManualReview,
            ],
        );
        let serialized = serde_json::to_value(&history)?;
        let projected = serialized["openJournals"]
            .as_array()
            .ok_or("open journals must serialize as an array")?
            .iter()
            .map(|journal| journal["recoveryAvailability"].as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            projected,
            [
                Some("review_available"),
                Some("quarantined"),
                Some("manual_review"),
            ],
        );
        Ok(())
    }
}
