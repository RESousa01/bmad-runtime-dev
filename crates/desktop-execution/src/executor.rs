use desktop_runtime::{
    canonical_hash, DeclaredWriteOperation, DomainValidationError, PatchOperation, Sha256Digest,
};

use crate::model::{
    journal_operation, CheckpointEntry, CheckpointFileState, EffectJournal, ExecutionError,
    ExecutionOutcome, ExecutionRequest, ExecutionStore, FileObservation, JournalOperationState,
    JournalState, LocalCheckpoint, LocalExecutionResult, ResultFileOperation, WorkspaceFileIo,
};

pub struct PatchExecutor<'a, W, S>
where
    W: WorkspaceFileIo,
    S: ExecutionStore,
{
    workspace: &'a W,
    store: &'a S,
}

impl<'a, W, S> PatchExecutor<'a, W, S>
where
    W: WorkspaceFileIo,
    S: ExecutionStore,
{
    #[must_use]
    pub const fn new(workspace: &'a W, store: &'a S) -> Self {
        Self { workspace, store }
    }

    /// Applies a fully authorized patch through checkpointed journal transitions.
    ///
    /// # Errors
    ///
    /// Returns [`ExecutionError`] when authorization, preimage validation,
    /// durable storage, a workspace effect, or postimage verification fails.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "the public execution boundary consumes its single-use authorization request"
    )]
    pub fn apply(&self, request: ExecutionRequest<'_>) -> Result<ExecutionOutcome, ExecutionError> {
        self.apply_authorized(&request)
    }

    fn apply_authorized(
        &self,
        request: &ExecutionRequest<'_>,
    ) -> Result<ExecutionOutcome, ExecutionError> {
        let workspace_target_hash = self.validate_authorization(request)?;
        let checkpoint = self.capture_checkpoint(request, workspace_target_hash)?;
        self.store
            .persist_checkpoint(&checkpoint)
            .map_err(|_| ExecutionError::StoreFailure)?;

        let mut journal = EffectJournal {
            schema_version: "sapphirus.local-effect-journal.v1".to_owned(),
            journal_id: request.journal_id.clone(),
            execution_id: request.consumption.draft.execution_id.clone(),
            candidate_hash: request.candidate.candidate_hash,
            spec_hash: request.spec.spec_hash,
            consumption_hash: request.consumption.consumption_hash,
            checkpoint_id: checkpoint.checkpoint_id.clone(),
            workspace_target_hash,
            patch_ref: request.candidate.draft.patch_ref.clone(),
            patch_hash: request.candidate.draft.patch_hash,
            state: JournalState::Prepared,
            operations: request
                .patch
                .operations
                .iter()
                .zip(0_u16..)
                .map(|(operation, index)| journal_operation(index, operation))
                .collect(),
            created_at: request.started_at,
            updated_at: request.started_at,
        };
        self.store
            .create_journal(&journal)
            .map_err(|_| ExecutionError::StoreFailure)?;
        self.persist_transition(
            &mut journal,
            JournalState::CheckpointDurable,
            request.started_at,
        )?;

        self.revalidate_checkpoint(request, &checkpoint)?;
        self.persist_transition(
            &mut journal,
            JournalState::PreconditionsVerified,
            request.started_at,
        )?;
        self.persist_transition(&mut journal, JournalState::Applying, request.started_at)?;

        self.apply_operations(request, &mut journal)?;

        if self
            .persist_transition(
                &mut journal,
                JournalState::EffectsApplied,
                request.completed_at,
            )
            .is_err()
        {
            self.mark_recovery_required(&mut journal, request.completed_at);
            return Err(ExecutionError::RecoveryRequired);
        }
        let files = match self.verify_postimages(request.patch) {
            Ok(files) => files,
            Err(error) => {
                self.mark_recovery_required(&mut journal, request.completed_at);
                return Err(error);
            }
        };
        for operation in &mut journal.operations {
            operation.state = JournalOperationState::Verified;
        }
        if self
            .persist_transition(
                &mut journal,
                JournalState::PostimagesVerified,
                request.completed_at,
            )
            .is_err()
        {
            self.mark_recovery_required(&mut journal, request.completed_at);
            return Err(ExecutionError::RecoveryRequired);
        }

        let result = LocalExecutionResult::seal(&journal, files, request.completed_at)?;
        journal.state = JournalState::ResultRecorded;
        journal.updated_at = request.completed_at;
        self.store
            .record_result(&result, &journal)
            .map_err(|_| ExecutionError::RecoveryRequired)?;
        self.persist_transition(&mut journal, JournalState::Completed, request.completed_at)
            .map_err(|_| ExecutionError::RecoveryRequired)?;

        Ok(ExecutionOutcome {
            checkpoint,
            journal,
            result,
        })
    }

    fn apply_operations(
        &self,
        request: &ExecutionRequest<'_>,
        journal: &mut EffectJournal,
    ) -> Result<(), ExecutionError> {
        for (index, operation) in request.patch.operations.iter().enumerate() {
            let Some(journal_operation) = journal.operations.get_mut(index) else {
                self.mark_recovery_required(journal, request.completed_at);
                return Err(ExecutionError::IntegrityFailure);
            };
            journal_operation.state = JournalOperationState::Applying;
            journal.updated_at = request.started_at;
            if self.store.update_journal(journal).is_err() {
                self.mark_recovery_required(journal, request.completed_at);
                return Err(ExecutionError::RecoveryRequired);
            }

            let Some(expected_preimage) = request.candidate.draft.preimages.get(index) else {
                self.mark_recovery_required(journal, request.completed_at);
                return Err(ExecutionError::IntegrityFailure);
            };
            if self
                .apply_operation(operation, expected_preimage.file_identity_hash)
                .is_err()
            {
                self.mark_recovery_required(journal, request.completed_at);
                return Err(ExecutionError::RecoveryRequired);
            }
            let Some(journal_operation) = journal.operations.get_mut(index) else {
                self.mark_recovery_required(journal, request.completed_at);
                return Err(ExecutionError::IntegrityFailure);
            };
            journal_operation.state = JournalOperationState::Applied;
            journal.updated_at = request.started_at;
            if self.store.update_journal(journal).is_err() {
                self.mark_recovery_required(journal, request.completed_at);
                return Err(ExecutionError::RecoveryRequired);
            }
        }
        Ok(())
    }

    fn validate_authorization(
        &self,
        request: &ExecutionRequest<'_>,
    ) -> Result<Sha256Digest, ExecutionError> {
        request.candidate.verify()?;
        request.patch.validate()?;
        request.spec.verify()?;
        request.consumption.verify()?;

        if request.completed_at < request.started_at
            || request.started_at < request.spec.draft.issued_at
            || request.started_at > request.spec.draft.expires_at
            || request.consumption.draft.consumed_at < request.spec.draft.issued_at
            || request.consumption.draft.consumed_at > request.started_at
            || request.spec.draft.candidate_hash != request.candidate.candidate_hash
            || request.consumption.draft.candidate_hash != request.candidate.candidate_hash
            || request.consumption.draft.spec_id != request.spec.draft.spec_id
            || request.consumption.draft.spec_hash != request.spec.spec_hash
            || request.consumption.draft.single_use_nonce_hash
                != request.spec.draft.single_use_nonce_hash
            || request.consumption.draft.authority_ref != request.spec.draft.authority_ref
            || request.spec.draft.authority_ref != request.candidate.draft.common.authority_ref
        {
            return Err(ExecutionError::AuthorizationMismatch);
        }

        let patch_hash = request.patch.content_hash()?;
        let target_hash = canonical_hash(
            "workspace-target",
            1,
            &request.candidate.draft.workspace_target,
        )
        .map_err(DomainValidationError::from)?;
        let current_target_hash = self
            .workspace
            .workspace_target_hash()
            .map_err(|_| ExecutionError::WorkspaceFailure)?;
        let mutable_input_set_hash = canonical_hash(
            "mutable-input-set",
            1,
            &request.candidate.draft.common.mutable_inputs,
        )
        .map_err(DomainValidationError::from)?;
        let executor_audience_hash = canonical_hash(
            "executor-audience",
            1,
            &request.spec.draft.executor_audience,
        )
        .map_err(DomainValidationError::from)?;
        if patch_hash != request.candidate.draft.patch_hash
            || target_hash != request.spec.draft.workspace_target_hash
            || current_target_hash != target_hash
            || request.spec.draft.mutable_input_set_hash != mutable_input_set_hash
            || request.consumption.draft.executor_audience_hash != executor_audience_hash
            || request.spec.draft.owner_scope_ref != request.candidate.draft.common.owner_scope_ref
            || request.spec.draft.project_id != request.candidate.draft.common.project_id
            || request.spec.draft.run_id != request.candidate.draft.common.run_id
            || request.spec.draft.proposal_id != request.candidate.draft.common.proposal_id
            || request.spec.draft.proposal_hash != request.candidate.draft.common.proposal_hash
            || request.spec.draft.candidate_id != request.candidate.draft.common.candidate_id
            || request.spec.draft.executor_audience != request.candidate.draft.executor_audience
            || request.patch.operations.len()
                != request.candidate.draft.common.declared_writes.len()
            || request.patch.operations.len() != request.candidate.draft.preimages.len()
            || request.patch.operations.len()
                > request.candidate.draft.common.limits.max_changed_files as usize
            || request.patch.changed_bytes()
                > request.candidate.draft.common.limits.max_changed_bytes
        {
            return Err(ExecutionError::AuthorizationMismatch);
        }

        for ((operation, write), preimage) in request
            .patch
            .operations
            .iter()
            .zip(&request.candidate.draft.common.declared_writes)
            .zip(&request.candidate.draft.preimages)
        {
            let declared_operation = match operation {
                PatchOperation::Create { .. } => DeclaredWriteOperation::Create,
                PatchOperation::Replace { .. } => DeclaredWriteOperation::Modify,
                PatchOperation::Delete { .. } => DeclaredWriteOperation::Delete,
            };
            if operation.relative_path() != &write.path_pattern
                || operation.relative_path() != &preimage.relative_path
                || operation.preimage_hash() != write.preimage_hash
                || operation.preimage_hash() != preimage.content_hash
                || declared_operation != write.operation
            {
                return Err(ExecutionError::AuthorizationMismatch);
            }
        }
        Ok(target_hash)
    }

    fn capture_checkpoint(
        &self,
        request: &ExecutionRequest<'_>,
        workspace_target_hash: Sha256Digest,
    ) -> Result<LocalCheckpoint, ExecutionError> {
        let mut entries = Vec::with_capacity(request.patch.operations.len());
        let mut checkpoint_bytes = 0_u64;
        for (operation, expected) in request
            .patch
            .operations
            .iter()
            .zip(&request.candidate.draft.preimages)
        {
            let bytes = self
                .workspace
                .read_file(operation.relative_path(), expected.file_identity_hash)
                .map_err(|_| ExecutionError::WorkspaceFailure)?;
            if let Some(content) = &bytes {
                checkpoint_bytes = checkpoint_bytes
                    .checked_add(content.len() as u64)
                    .ok_or(ExecutionError::UnsupportedContent)?;
            }
            if checkpoint_bytes > desktop_runtime::HARD_MAX_CHANGED_BYTES {
                return Err(ExecutionError::UnsupportedContent);
            }
            let before = CheckpointFileState::from_bytes(bytes)?;
            validate_preimage(operation, &before)?;
            entries.push(CheckpointEntry {
                relative_path: operation.relative_path().clone(),
                before,
            });
        }
        entries.sort_by(|left, right| left.relative_path.canonical_cmp(&right.relative_path));
        LocalCheckpoint::seal(
            request.checkpoint_id.clone(),
            workspace_target_hash,
            request.candidate.candidate_hash,
            entries,
            request.started_at,
        )
    }

    fn revalidate_checkpoint(
        &self,
        request: &ExecutionRequest<'_>,
        checkpoint: &LocalCheckpoint,
    ) -> Result<(), ExecutionError> {
        checkpoint.verify()?;
        for (operation, expected) in request
            .patch
            .operations
            .iter()
            .zip(&request.candidate.draft.preimages)
        {
            let current = CheckpointFileState::from_bytes(
                self.workspace
                    .read_file(operation.relative_path(), expected.file_identity_hash)
                    .map_err(|_| ExecutionError::WorkspaceFailure)?,
            )?;
            validate_preimage(operation, &current)?;
        }
        Ok(())
    }

    fn apply_operation(
        &self,
        operation: &PatchOperation,
        expected_file_identity_hash: Option<Sha256Digest>,
    ) -> Result<(), ExecutionError> {
        let result = match operation {
            PatchOperation::Create {
                relative_path,
                content,
                ..
            } => self.workspace.create_utf8_durable(relative_path, content),
            PatchOperation::Replace {
                relative_path,
                preimage_hash,
                content,
                ..
            } => expected_file_identity_hash.map_or(
                Err(crate::WorkspaceIoError::CapabilityRevoked),
                |file_identity_hash| {
                    self.workspace.replace_utf8_durable(
                        relative_path,
                        *preimage_hash,
                        file_identity_hash,
                        content,
                    )
                },
            ),
            PatchOperation::Delete {
                relative_path,
                preimage_hash,
            } => expected_file_identity_hash.map_or(
                Err(crate::WorkspaceIoError::CapabilityRevoked),
                |file_identity_hash| {
                    self.workspace
                        .delete_durable(relative_path, *preimage_hash, file_identity_hash)
                },
            ),
        };
        result.map_err(|_| ExecutionError::WorkspaceFailure)
    }

    fn verify_postimages(
        &self,
        patch: &desktop_runtime::PatchSet,
    ) -> Result<Vec<FileObservation>, ExecutionError> {
        let mut files = Vec::with_capacity(patch.operations.len());
        for operation in &patch.operations {
            let current = CheckpointFileState::from_bytes(
                self.workspace
                    .read_file(operation.relative_path(), None)
                    .map_err(|_| ExecutionError::WorkspaceFailure)?,
            )?;
            let (expected_hash, expected_exists, result_operation) = match operation {
                PatchOperation::Create { .. } => (
                    operation.postimage_hash(),
                    true,
                    ResultFileOperation::Created,
                ),
                PatchOperation::Replace { .. } => (
                    operation.postimage_hash(),
                    true,
                    ResultFileOperation::Modified,
                ),
                PatchOperation::Delete { .. } => (None, false, ResultFileOperation::Deleted),
            };
            if current.exists() != expected_exists || current.content_hash() != expected_hash {
                return Err(ExecutionError::RecoveryRequired);
            }
            files.push(FileObservation {
                relative_path: operation.relative_path().clone(),
                operation: result_operation,
                exists: expected_exists,
                content_hash: expected_hash,
            });
        }
        Ok(files)
    }

    fn persist_transition(
        &self,
        journal: &mut EffectJournal,
        state: JournalState,
        updated_at: desktop_runtime::UnixMillis,
    ) -> Result<(), ExecutionError> {
        if !valid_transition(journal.state, state) {
            return Err(ExecutionError::IntegrityFailure);
        }
        journal.state = state;
        journal.updated_at = updated_at;
        self.store
            .update_journal(journal)
            .map_err(|_| ExecutionError::StoreFailure)
    }

    fn mark_recovery_required(
        &self,
        journal: &mut EffectJournal,
        updated_at: desktop_runtime::UnixMillis,
    ) {
        journal.state = JournalState::RecoveryRequired;
        journal.updated_at = updated_at;
        if self.store.update_journal(journal).is_err() {
            // The pre-effect journal is already durable. Startup reconciliation
            // treats any nonterminal state as recovery-required even if this
            // final best-effort marker could not be flushed.
        }
    }
}

fn validate_preimage(
    operation: &PatchOperation,
    observed: &CheckpointFileState,
) -> Result<(), ExecutionError> {
    match operation {
        PatchOperation::Create { .. } if !observed.exists() => Ok(()),
        PatchOperation::Replace { preimage_hash, .. }
        | PatchOperation::Delete { preimage_hash, .. }
            if observed.content_hash() == Some(*preimage_hash) =>
        {
            Ok(())
        }
        _ => Err(ExecutionError::PreconditionFailed),
    }
}

const fn valid_transition(current: JournalState, next: JournalState) -> bool {
    matches!(
        (current, next),
        (JournalState::Prepared, JournalState::CheckpointDurable)
            | (
                JournalState::CheckpointDurable,
                JournalState::PreconditionsVerified
            )
            | (JournalState::PreconditionsVerified, JournalState::Applying)
            | (JournalState::Applying, JournalState::EffectsApplied)
            | (
                JournalState::EffectsApplied,
                JournalState::PostimagesVerified
            )
            | (
                JournalState::PostimagesVerified,
                JournalState::ResultRecorded
            )
            | (JournalState::ResultRecorded, JournalState::Completed)
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    use desktop_runtime::{
        canonical_hash, sha256_bytes, ApprovedExecutionSpecDraft, AuthorityRef, CandidateCommon,
        ContractId, DeclaredWrite, DeclaredWriteOperation, DeliveryModel, ExecutionLimits,
        InputKind, LocalPathPreimage, MutableInputBinding, NativePatchEngineAudience,
        PatchOperation, PatchSet, RelativeWorkspacePath, RollbackClass, SpecConsumptionRecordDraft,
        UnixMillis, WindowsPatchCandidateDraft, WorkspaceTarget,
    };

    use super::PatchExecutor;
    use crate::{
        plan_rollback, EffectJournal, ExecutionError, ExecutionRequest, ExecutionStore,
        JournalState, JournalStoreError, LocalCheckpoint, LocalExecutionResult, WorkspaceFileIo,
        WorkspaceIoError,
    };

    struct MemoryWorkspace {
        target_hash: desktop_runtime::Sha256Digest,
        files: Mutex<BTreeMap<String, Vec<u8>>>,
        fail_mutation_number: Option<usize>,
        mutation_count: Mutex<usize>,
    }

    impl MemoryWorkspace {
        fn new(
            target_hash: desktop_runtime::Sha256Digest,
            files: BTreeMap<String, Vec<u8>>,
        ) -> Self {
            Self {
                target_hash,
                files: Mutex::new(files),
                fail_mutation_number: None,
                mutation_count: Mutex::new(0),
            }
        }

        fn with_failure(mut self, mutation_number: usize) -> Self {
            self.fail_mutation_number = Some(mutation_number);
            self
        }

        fn should_fail(&self) -> Result<bool, WorkspaceIoError> {
            let mut count = self
                .mutation_count
                .lock()
                .map_err(|_| WorkspaceIoError::Unavailable)?;
            *count = count.saturating_add(1);
            Ok(self.fail_mutation_number == Some(*count))
        }

        fn set(&self, path: &str, content: &[u8]) -> Result<(), WorkspaceIoError> {
            self.files
                .lock()
                .map_err(|_| WorkspaceIoError::Unavailable)?
                .insert(path.to_owned(), content.to_vec());
            Ok(())
        }

        fn get(&self, path: &str) -> Result<Option<Vec<u8>>, WorkspaceIoError> {
            Ok(self
                .files
                .lock()
                .map_err(|_| WorkspaceIoError::Unavailable)?
                .get(path)
                .cloned())
        }
    }

    impl WorkspaceFileIo for MemoryWorkspace {
        fn workspace_target_hash(&self) -> Result<desktop_runtime::Sha256Digest, WorkspaceIoError> {
            Ok(self.target_hash)
        }

        fn read_file(
            &self,
            path: &RelativeWorkspacePath,
            _expected_file_identity_hash: Option<desktop_runtime::Sha256Digest>,
        ) -> Result<Option<Vec<u8>>, WorkspaceIoError> {
            self.get(path.as_str())
        }

        fn create_utf8_durable(
            &self,
            path: &RelativeWorkspacePath,
            content: &str,
        ) -> Result<(), WorkspaceIoError> {
            if self.should_fail()? {
                return Err(WorkspaceIoError::Unavailable);
            }
            let mut files = self
                .files
                .lock()
                .map_err(|_| WorkspaceIoError::Unavailable)?;
            if files.contains_key(path.as_str()) {
                return Err(WorkspaceIoError::AlreadyExists);
            }
            files.insert(path.as_str().to_owned(), content.as_bytes().to_vec());
            Ok(())
        }

        fn replace_utf8_durable(
            &self,
            path: &RelativeWorkspacePath,
            expected_content_hash: desktop_runtime::Sha256Digest,
            _expected_file_identity_hash: desktop_runtime::Sha256Digest,
            content: &str,
        ) -> Result<(), WorkspaceIoError> {
            if self.should_fail()? {
                return Err(WorkspaceIoError::Unavailable);
            }
            let mut files = self
                .files
                .lock()
                .map_err(|_| WorkspaceIoError::Unavailable)?;
            let Some(file) = files.get_mut(path.as_str()) else {
                return Err(WorkspaceIoError::NotFound);
            };
            if desktop_runtime::sha256_bytes(file) != expected_content_hash {
                return Err(WorkspaceIoError::CapabilityRevoked);
            }
            *file = content.as_bytes().to_vec();
            Ok(())
        }

        fn delete_durable(
            &self,
            path: &RelativeWorkspacePath,
            expected_content_hash: desktop_runtime::Sha256Digest,
            _expected_file_identity_hash: desktop_runtime::Sha256Digest,
        ) -> Result<(), WorkspaceIoError> {
            if self.should_fail()? {
                return Err(WorkspaceIoError::Unavailable);
            }
            let mut files = self
                .files
                .lock()
                .map_err(|_| WorkspaceIoError::Unavailable)?;
            let Some(current) = files.get(path.as_str()) else {
                return Err(WorkspaceIoError::NotFound);
            };
            if desktop_runtime::sha256_bytes(current) != expected_content_hash {
                return Err(WorkspaceIoError::CapabilityRevoked);
            }
            files.remove(path.as_str());
            Ok(())
        }
    }

    #[derive(Default)]
    struct MemoryStore {
        checkpoint: Mutex<Option<LocalCheckpoint>>,
        journal: Mutex<Option<EffectJournal>>,
        result: Mutex<Option<LocalExecutionResult>>,
    }

    impl MemoryStore {
        fn journal_state(&self) -> Result<Option<JournalState>, JournalStoreError> {
            Ok(self
                .journal
                .lock()
                .map_err(|_| JournalStoreError)?
                .as_ref()
                .map(|journal| journal.state))
        }
    }

    impl ExecutionStore for MemoryStore {
        fn persist_checkpoint(
            &self,
            checkpoint: &LocalCheckpoint,
        ) -> Result<(), JournalStoreError> {
            *self.checkpoint.lock().map_err(|_| JournalStoreError)? = Some(checkpoint.clone());
            Ok(())
        }

        fn create_journal(&self, journal: &EffectJournal) -> Result<(), JournalStoreError> {
            let mut stored = self.journal.lock().map_err(|_| JournalStoreError)?;
            if stored.is_some() {
                return Err(JournalStoreError);
            }
            *stored = Some(journal.clone());
            Ok(())
        }

        fn update_journal(&self, journal: &EffectJournal) -> Result<(), JournalStoreError> {
            *self.journal.lock().map_err(|_| JournalStoreError)? = Some(journal.clone());
            Ok(())
        }

        fn record_result(
            &self,
            result: &LocalExecutionResult,
            journal: &EffectJournal,
        ) -> Result<(), JournalStoreError> {
            *self.result.lock().map_err(|_| JournalStoreError)? = Some(result.clone());
            *self.journal.lock().map_err(|_| JournalStoreError)? = Some(journal.clone());
            Ok(())
        }
    }

    struct Fixture {
        candidate: desktop_runtime::WindowsPatchCandidate,
        patch: PatchSet,
        spec: desktop_runtime::ApprovedExecutionSpec,
        consumption: desktop_runtime::SpecConsumptionRecord,
        target_hash: desktop_runtime::Sha256Digest,
    }

    struct AuthorizationFixture {
        authority: AuthorityRef,
        audience: NativePatchEngineAudience,
        target: WorkspaceTarget,
        target_hash: desktop_runtime::Sha256Digest,
        mutable_inputs: Vec<MutableInputBinding>,
    }

    struct CandidateFixtureInput<'a> {
        authorization: &'a AuthorizationFixture,
        new_path: RelativeWorkspacePath,
        old_path: RelativeWorkspacePath,
        remove_path: RelativeWorkspacePath,
        old_hash: desktop_runtime::Sha256Digest,
        remove_hash: desktop_runtime::Sha256Digest,
        patch_hash: desktop_runtime::Sha256Digest,
    }

    impl Fixture {
        fn request(&self) -> Result<ExecutionRequest<'_>, Box<dyn std::error::Error>> {
            Ok(ExecutionRequest {
                journal_id: id("journal_1")?,
                checkpoint_id: id("checkpoint_1")?,
                candidate: &self.candidate,
                patch: &self.patch,
                spec: &self.spec,
                consumption: &self.consumption,
                started_at: UnixMillis(4_000),
                completed_at: UnixMillis(5_000),
            })
        }
    }

    fn id(value: &str) -> Result<ContractId, Box<dyn std::error::Error>> {
        Ok(ContractId::new(value)?)
    }

    fn path(value: &str) -> Result<RelativeWorkspacePath, Box<dyn std::error::Error>> {
        Ok(RelativeWorkspacePath::new(value)?)
    }

    fn authorization_fixture() -> Result<AuthorizationFixture, Box<dyn std::error::Error>> {
        let authority = AuthorityRef {
            authority_kind: "desktop_local_store".to_owned(),
            authority_id: id("authority_1")?,
            installation_id: id("install_1")?,
            local_store_id: id("store_1")?,
            authority_epoch: 1,
        };
        let audience = NativePatchEngineAudience {
            audience_kind: "native_patch_engine".to_owned(),
            installation_id: id("install_1")?,
            host_build_id: "desktop-test".to_owned(),
            host_binary_sha256: sha256_bytes(b"binary"),
            patch_engine_profile_hash: sha256_bytes(b"profile"),
        };
        let target = WorkspaceTarget {
            target_kind: "local_folder_capability".to_owned(),
            workspace_capability_id: id("workspace_1")?,
            grant_epoch: 1,
            root_identity_hash: sha256_bytes(b"root"),
            filesystem_capability_hash: sha256_bytes(b"filesystem"),
            base_checkpoint_id: id("checkpoint_0")?,
            workspace_manifest_hash: sha256_bytes(b"manifest"),
        };
        let target_hash = canonical_hash("workspace-target", 1, &target)?;
        let mutable_inputs = vec![MutableInputBinding {
            input_kind: InputKind::WorkspaceManifest,
            input_id: "manifest_1".to_owned(),
            content_hash: sha256_bytes(b"manifest"),
        }];
        Ok(AuthorizationFixture {
            authority,
            audience,
            target,
            target_hash,
            mutable_inputs,
        })
    }

    fn candidate_fixture(
        input: CandidateFixtureInput<'_>,
    ) -> Result<desktop_runtime::WindowsPatchCandidate, Box<dyn std::error::Error>> {
        let CandidateFixtureInput {
            authorization,
            new_path,
            old_path,
            remove_path,
            old_hash,
            remove_hash,
            patch_hash,
        } = input;
        Ok(WindowsPatchCandidateDraft {
            schema_version: "sapphirus.candidate-action.v1".to_owned(),
            common: CandidateCommon {
                candidate_id: id("candidate_1")?,
                project_id: id("project_1")?,
                run_id: id("run_1")?,
                proposal_id: id("proposal_1")?,
                proposal_hash: sha256_bytes(b"proposal"),
                authority_ref: authorization.authority.clone(),
                owner_scope_ref: id("owner_1")?,
                policy_context_hash: sha256_bytes(b"policy-context"),
                mutable_inputs: authorization.mutable_inputs.clone(),
                declared_writes: vec![
                    DeclaredWrite {
                        path_pattern: new_path.clone(),
                        operation: DeclaredWriteOperation::Create,
                        preimage_hash: None,
                    },
                    DeclaredWrite {
                        path_pattern: old_path.clone(),
                        operation: DeclaredWriteOperation::Modify,
                        preimage_hash: Some(old_hash),
                    },
                    DeclaredWrite {
                        path_pattern: remove_path.clone(),
                        operation: DeclaredWriteOperation::Delete,
                        preimage_hash: Some(remove_hash),
                    },
                ],
                limits: ExecutionLimits::governed_patch_defaults(),
                rollback_class: RollbackClass::FileTracked,
                created_at: UnixMillis(1_000),
                expires_at: UnixMillis(10_000),
            },
            delivery_model: DeliveryModel::WindowsLocal,
            action_kind: "patch_apply".to_owned(),
            workspace_target: authorization.target.clone(),
            executor_audience: authorization.audience.clone(),
            patch_ref: format!("cas://sha256/{}", patch_hash.hex_value()),
            patch_hash,
            preimages: vec![
                LocalPathPreimage {
                    relative_path: new_path,
                    exists: false,
                    file_identity_hash: None,
                    content_hash: None,
                    metadata_hash: None,
                },
                LocalPathPreimage {
                    relative_path: old_path,
                    exists: true,
                    file_identity_hash: Some(sha256_bytes(b"old-id")),
                    content_hash: Some(old_hash),
                    metadata_hash: Some(sha256_bytes(b"old-meta")),
                },
                LocalPathPreimage {
                    relative_path: remove_path,
                    exists: true,
                    file_identity_hash: Some(sha256_bytes(b"remove-id")),
                    content_hash: Some(remove_hash),
                    metadata_hash: Some(sha256_bytes(b"remove-meta")),
                },
            ],
        }
        .seal()?)
    }

    fn fixture() -> Result<Fixture, Box<dyn std::error::Error>> {
        let new_path = path("new.txt")?;
        let old_path = path("old.txt")?;
        let remove_path = path("remove.txt")?;
        let old_hash = sha256_bytes(b"old");
        let remove_hash = sha256_bytes(b"remove");
        let patch = PatchSet::new(vec![
            PatchOperation::create(new_path.clone(), "created".to_owned()),
            PatchOperation::replace(old_path.clone(), old_hash, "updated".to_owned()),
            PatchOperation::delete(remove_path.clone(), remove_hash),
        ]);
        let patch_hash = patch.content_hash()?;
        let authorization = authorization_fixture()?;
        let candidate = candidate_fixture(CandidateFixtureInput {
            authorization: &authorization,
            new_path,
            old_path,
            remove_path,
            old_hash,
            remove_hash,
            patch_hash,
        })?;
        let nonce_hash = sha256_bytes(b"0123456789abcdef");
        let spec = ApprovedExecutionSpecDraft {
            schema_version: "sapphirus.approved-execution-spec.v1".to_owned(),
            spec_id: id("spec_1")?,
            delivery_model: DeliveryModel::WindowsLocal,
            authority_ref: authorization.authority.clone(),
            owner_scope_ref: candidate.draft.common.owner_scope_ref.clone(),
            project_id: candidate.draft.common.project_id.clone(),
            run_id: candidate.draft.common.run_id.clone(),
            proposal_id: candidate.draft.common.proposal_id.clone(),
            proposal_hash: candidate.draft.common.proposal_hash,
            candidate_id: candidate.draft.common.candidate_id.clone(),
            candidate_hash: candidate.candidate_hash,
            approval_id: id("approval_1")?,
            approval_decision_hash: sha256_bytes(b"approval"),
            policy_version: "policy-1".to_owned(),
            policy_hash: sha256_bytes(b"policy"),
            workspace_target_hash: authorization.target_hash,
            mutable_input_set_hash: canonical_hash(
                "mutable-input-set",
                1,
                &authorization.mutable_inputs,
            )?,
            executor_audience: authorization.audience.clone(),
            issued_at: UnixMillis(2_000),
            expires_at: UnixMillis(9_000),
            single_use_nonce_hash: nonce_hash,
        }
        .seal()?;
        let audience_hash = canonical_hash("executor-audience", 1, &authorization.audience)?;
        let consumption = SpecConsumptionRecordDraft {
            schema_version: "sapphirus.spec-consumption.v1".to_owned(),
            consumption_id: id("consumption_1")?,
            delivery_model: DeliveryModel::WindowsLocal,
            authority_ref: authorization.authority,
            spec_id: spec.draft.spec_id.clone(),
            spec_hash: spec.spec_hash,
            candidate_hash: candidate.candidate_hash,
            single_use_nonce_hash: nonce_hash,
            executor_audience_hash: audience_hash,
            execution_id: id("execution_1")?,
            attempt_number: 1,
            consumed_at: UnixMillis(3_000),
        }
        .seal()?;
        Ok(Fixture {
            candidate,
            patch,
            spec,
            consumption,
            target_hash: authorization.target_hash,
        })
    }

    fn initial_files() -> BTreeMap<String, Vec<u8>> {
        BTreeMap::from([
            ("old.txt".to_owned(), b"old".to_vec()),
            ("remove.txt".to_owned(), b"remove".to_vec()),
        ])
    }

    #[test]
    fn applies_and_verifies_a_three_operation_patch() -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let workspace = MemoryWorkspace::new(fixture.target_hash, initial_files());
        let store = MemoryStore::default();
        let outcome = PatchExecutor::new(&workspace, &store).apply(fixture.request()?)?;
        assert_eq!(outcome.journal.state, JournalState::Completed);
        assert_eq!(workspace.get("new.txt")?, Some(b"created".to_vec()));
        assert_eq!(workspace.get("old.txt")?, Some(b"updated".to_vec()));
        assert_eq!(workspace.get("remove.txt")?, None);
        assert_eq!(outcome.result.files.len(), 3);
        outcome.result.verify()?;
        Ok(())
    }

    #[test]
    fn stale_preimage_fails_before_any_journal_or_write() -> Result<(), Box<dyn std::error::Error>>
    {
        let fixture = fixture()?;
        let workspace = MemoryWorkspace::new(fixture.target_hash, initial_files());
        workspace.set("old.txt", b"external edit")?;
        let store = MemoryStore::default();
        let result = PatchExecutor::new(&workspace, &store).apply(fixture.request()?);
        assert!(matches!(result, Err(ExecutionError::PreconditionFailed)));
        assert_eq!(workspace.get("new.txt")?, None);
        assert_eq!(store.journal_state()?, None);
        Ok(())
    }

    #[test]
    fn partial_failure_is_never_reported_as_success() -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let workspace = MemoryWorkspace::new(fixture.target_hash, initial_files()).with_failure(2);
        let store = MemoryStore::default();
        let result = PatchExecutor::new(&workspace, &store).apply(fixture.request()?);
        assert!(matches!(result, Err(ExecutionError::RecoveryRequired)));
        assert_eq!(store.journal_state()?, Some(JournalState::RecoveryRequired));
        assert_eq!(workspace.get("new.txt")?, Some(b"created".to_vec()));
        assert_eq!(workspace.get("old.txt")?, Some(b"old".to_vec()));
        Ok(())
    }

    #[test]
    fn rollback_requires_fresh_conflict_resolution_after_external_edit(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let workspace = MemoryWorkspace::new(fixture.target_hash, initial_files());
        let store = MemoryStore::default();
        let outcome = PatchExecutor::new(&workspace, &store).apply(fixture.request()?)?;
        workspace.set("old.txt", b"newer user work")?;
        let rollback = plan_rollback(
            &workspace,
            id("rollback_1")?,
            &outcome.checkpoint,
            &outcome.result,
            UnixMillis(6_000),
        )?;
        assert_eq!(rollback.conflicts.len(), 1);
        assert!(rollback.to_patch_set().is_err());
        Ok(())
    }
}
