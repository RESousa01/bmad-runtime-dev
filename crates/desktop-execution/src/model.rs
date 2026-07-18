use desktop_runtime::{
    canonical_hash, ApprovedExecutionSpec, ContractId, DomainValidationError, PatchOperation,
    PatchSet, RelativeWorkspacePath, Sha256Digest, SpecConsumptionRecord, UnixMillis,
    WindowsPatchCandidate,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum WorkspaceIoError {
    #[error("workspace object was not found")]
    NotFound,
    #[error("workspace object already exists")]
    AlreadyExists,
    #[error("workspace capability no longer authorizes the operation")]
    CapabilityRevoked,
    #[error("workspace content is not supported UTF-8 text")]
    UnsupportedContent,
    #[error("workspace operation could not be completed")]
    Unavailable,
}

/// Identity-bound observation of one governed workspace path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceFileObservation {
    pub content: Option<Vec<u8>>,
    pub file_identity_hash: Option<Sha256Digest>,
}

/// A selected-root broker. Implementations must revalidate the root identity,
/// grant epoch, reparse/file identity, hardlink policy, and same-volume atomic
/// replacement immediately around each method call.
pub trait WorkspaceFileIo: Send + Sync {
    /// Returns the hash of the currently authorized workspace target.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceIoError`] when the workspace capability cannot be
    /// revalidated or observed.
    fn workspace_target_hash(&self) -> Result<Sha256Digest, WorkspaceIoError>;

    /// Returns at most the broker's configured bounded file size.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceIoError`] when the path is unauthorized, its identity
    /// changed, its content is unsupported, or the read cannot complete.
    fn read_file(
        &self,
        path: &RelativeWorkspacePath,
        expected_file_identity_hash: Option<Sha256Digest>,
    ) -> Result<Option<Vec<u8>>, WorkspaceIoError>;

    /// Observes content and file identity for recovery planning.
    ///
    /// Existing adapters which cannot expose a broker-owned identity fail
    /// closed for recovery because the default observation has no identity.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceIoError`] when the governed path cannot be observed.
    fn observe_recovery_file(
        &self,
        path: &RelativeWorkspacePath,
    ) -> Result<WorkspaceFileObservation, WorkspaceIoError> {
        Ok(WorkspaceFileObservation {
            content: self.read_file(path, None)?,
            file_identity_hash: None,
        })
    }

    /// Runs recovery validation and effects in one broker-owned authority
    /// scope. Production workspace adapters override this method to retain the
    /// grant barrier across the entire callback.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceIoError`] when the recovery scope or callback fails.
    fn with_recovery_transaction(
        &self,
        transaction: &mut dyn FnMut(&dyn WorkspaceFileIo) -> Result<(), WorkspaceIoError>,
    ) -> Result<(), WorkspaceIoError>
    where
        Self: Sized,
    {
        transaction(self)
    }

    /// Create a new file and durably flush the file and owning directory.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceIoError`] when the path is unauthorized or already
    /// exists, the content is unsupported, or the durable create cannot complete.
    fn create_utf8_durable(
        &self,
        path: &RelativeWorkspacePath,
        content: &str,
    ) -> Result<(), WorkspaceIoError>;

    /// Atomically replace an existing file on the same volume, then durably
    /// flush the replacement and owning directory.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceIoError`] when an expected identity or hash does not
    /// match, the path is unauthorized, or the durable replacement fails.
    fn replace_utf8_durable(
        &self,
        path: &RelativeWorkspacePath,
        expected_content_hash: Sha256Digest,
        expected_file_identity_hash: Sha256Digest,
        content: &str,
    ) -> Result<(), WorkspaceIoError>;

    /// Delete an existing exact preimage and durably flush the owning directory.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceIoError`] when an expected identity or hash does not
    /// match, the path is unauthorized, or the durable deletion fails.
    fn delete_durable(
        &self,
        path: &RelativeWorkspacePath,
        expected_content_hash: Sha256Digest,
        expected_file_identity_hash: Sha256Digest,
    ) -> Result<(), WorkspaceIoError>;
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("durable execution storage is unavailable")]
pub struct JournalStoreError;

/// Store ordering is part of the safety contract. Each method returns only
/// after its data is durable. `record_result` atomically records the result,
/// domain transition, evidence event, and outbox entry.
pub trait ExecutionStore: Send + Sync {
    /// Persists a checkpoint before any governed file effect begins.
    ///
    /// # Errors
    ///
    /// Returns [`JournalStoreError`] when the checkpoint cannot be made durable.
    fn persist_checkpoint(&self, checkpoint: &LocalCheckpoint) -> Result<(), JournalStoreError>;

    /// Persists a newly prepared effect journal.
    ///
    /// # Errors
    ///
    /// Returns [`JournalStoreError`] when the journal cannot be made durable.
    fn create_journal(&self, journal: &EffectJournal) -> Result<(), JournalStoreError>;

    /// Persists a journal state transition.
    ///
    /// # Errors
    ///
    /// Returns [`JournalStoreError`] when the transition cannot be made durable.
    fn update_journal(&self, journal: &EffectJournal) -> Result<(), JournalStoreError>;

    /// Atomically records the execution result and its journal transition.
    ///
    /// # Errors
    ///
    /// Returns [`JournalStoreError`] when the result transaction cannot be made
    /// durable.
    fn record_result(
        &self,
        result: &LocalExecutionResult,
        journal: &EffectJournal,
    ) -> Result<(), JournalStoreError>;
}

#[derive(Clone, Debug)]
pub struct ExecutionRequest<'a> {
    pub journal_id: ContractId,
    pub checkpoint_id: ContractId,
    pub candidate: &'a WindowsPatchCandidate,
    pub patch: &'a PatchSet,
    pub spec: &'a ApprovedExecutionSpec,
    pub consumption: &'a SpecConsumptionRecord,
    pub started_at: UnixMillis,
    pub completed_at: UnixMillis,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum CheckpointFileState {
    Absent,
    Utf8 {
        content: String,
        #[serde(rename = "contentHash")]
        content_hash: Sha256Digest,
    },
}

impl CheckpointFileState {
    pub(crate) fn from_bytes(bytes: Option<Vec<u8>>) -> Result<Self, ExecutionError> {
        match bytes {
            None => Ok(Self::Absent),
            Some(bytes) => {
                if bytes.contains(&0) {
                    return Err(ExecutionError::UnsupportedContent);
                }
                let content =
                    String::from_utf8(bytes).map_err(|_| ExecutionError::UnsupportedContent)?;
                let content_hash = desktop_runtime::sha256_bytes(content.as_bytes());
                Ok(Self::Utf8 {
                    content,
                    content_hash,
                })
            }
        }
    }

    #[must_use]
    pub const fn content_hash(&self) -> Option<Sha256Digest> {
        match self {
            Self::Absent => None,
            Self::Utf8 { content_hash, .. } => Some(*content_hash),
        }
    }

    #[must_use]
    pub const fn exists(&self) -> bool {
        matches!(self, Self::Utf8 { .. })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CheckpointEntry {
    pub relative_path: RelativeWorkspacePath,
    pub before: CheckpointFileState,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalCheckpoint {
    pub schema_version: String,
    pub checkpoint_id: ContractId,
    pub workspace_target_hash: Sha256Digest,
    pub candidate_hash: Sha256Digest,
    pub entries: Vec<CheckpointEntry>,
    pub created_at: UnixMillis,
    pub manifest_hash: Sha256Digest,
}

impl LocalCheckpoint {
    pub(crate) fn seal(
        checkpoint_id: ContractId,
        workspace_target_hash: Sha256Digest,
        candidate_hash: Sha256Digest,
        entries: Vec<CheckpointEntry>,
        created_at: UnixMillis,
    ) -> Result<Self, ExecutionError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Draft<'a> {
            schema_version: &'static str,
            checkpoint_id: &'a ContractId,
            workspace_target_hash: Sha256Digest,
            candidate_hash: Sha256Digest,
            entries: &'a [CheckpointEntry],
            created_at: UnixMillis,
        }

        let manifest_hash = canonical_hash(
            "local-checkpoint",
            1,
            &Draft {
                schema_version: "sapphirus.local-checkpoint.v1",
                checkpoint_id: &checkpoint_id,
                workspace_target_hash,
                candidate_hash,
                entries: &entries,
                created_at,
            },
        )
        .map_err(DomainValidationError::from)?;
        Ok(Self {
            schema_version: "sapphirus.local-checkpoint.v1".to_owned(),
            checkpoint_id,
            workspace_target_hash,
            candidate_hash,
            entries,
            created_at,
            manifest_hash,
        })
    }

    /// Verifies checkpoint ordering, UTF-8 content, and the canonical manifest.
    ///
    /// # Errors
    ///
    /// Returns [`ExecutionError`] when checkpoint content or its manifest hash
    /// is invalid.
    pub fn verify(&self) -> Result<(), ExecutionError> {
        let recreated = Self::seal(
            self.checkpoint_id.clone(),
            self.workspace_target_hash,
            self.candidate_hash,
            self.entries.clone(),
            self.created_at,
        )?;
        if recreated.schema_version != self.schema_version
            || recreated.manifest_hash != self.manifest_hash
            || self.entries.windows(2).any(|pair| {
                !pair[0]
                    .relative_path
                    .canonical_cmp(&pair[1].relative_path)
                    .is_lt()
            })
        {
            return Err(ExecutionError::IntegrityFailure);
        }
        for entry in &self.entries {
            if let CheckpointFileState::Utf8 {
                content,
                content_hash,
            } = &entry.before
            {
                if content.contains('\0')
                    || desktop_runtime::sha256_bytes(content.as_bytes()) != *content_hash
                {
                    return Err(ExecutionError::IntegrityFailure);
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JournalState {
    Prepared,
    CheckpointDurable,
    PreconditionsVerified,
    Applying,
    EffectsApplied,
    PostimagesVerified,
    ResultRecorded,
    Completed,
    RecoveryRequired,
    Restoring,
    Recovered,
    ManualReview,
}

impl JournalState {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Recovered | Self::ManualReview)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JournalOperationState {
    Pending,
    Applying,
    Applied,
    Verified,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JournalOperation {
    pub ordinal: u16,
    pub relative_path: RelativeWorkspacePath,
    pub operation: ResultFileOperation,
    pub preimage_hash: Option<Sha256Digest>,
    pub postimage_hash: Option<Sha256Digest>,
    pub state: JournalOperationState,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultFileOperation {
    Created,
    Modified,
    Deleted,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectJournal {
    pub schema_version: String,
    pub journal_id: ContractId,
    pub execution_id: ContractId,
    pub candidate_hash: Sha256Digest,
    pub spec_hash: Sha256Digest,
    pub consumption_hash: Sha256Digest,
    pub checkpoint_id: ContractId,
    pub workspace_target_hash: Sha256Digest,
    pub patch_ref: String,
    pub patch_hash: Sha256Digest,
    pub state: JournalState,
    pub operations: Vec<JournalOperation>,
    pub created_at: UnixMillis,
    pub updated_at: UnixMillis,
}

impl EffectJournal {
    /// Verifies the immutable effect plan and its per-file hash invariants.
    ///
    /// # Errors
    ///
    /// Returns [`ExecutionError::IntegrityFailure`] when the journal schema,
    /// ordering, paths, or operation hashes are inconsistent.
    pub fn verify_plan(&self) -> Result<(), ExecutionError> {
        if self.schema_version != "sapphirus.local-effect-journal.v1"
            || self.operations.is_empty()
            || self.operations.len() > desktop_runtime::HARD_MAX_CHANGED_FILES as usize
            || self.updated_at < self.created_at
            || self.patch_ref != format!("cas://sha256/{}", self.patch_hash.hex_value())
        {
            return Err(ExecutionError::IntegrityFailure);
        }
        let mut paths = std::collections::BTreeSet::new();
        for (index, operation) in self.operations.iter().enumerate() {
            let valid_hashes = match operation.operation {
                ResultFileOperation::Created => {
                    operation.preimage_hash.is_none() && operation.postimage_hash.is_some()
                }
                ResultFileOperation::Modified => {
                    operation.preimage_hash.is_some() && operation.postimage_hash.is_some()
                }
                ResultFileOperation::Deleted => {
                    operation.preimage_hash.is_some() && operation.postimage_hash.is_none()
                }
            };
            if usize::from(operation.ordinal) != index.saturating_add(1)
                || !valid_hashes
                || !paths.insert(operation.relative_path.case_folded())
            {
                return Err(ExecutionError::IntegrityFailure);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileObservation {
    pub relative_path: RelativeWorkspacePath,
    pub operation: ResultFileOperation,
    pub exists: bool,
    pub content_hash: Option<Sha256Digest>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Compact, host-internal result used for journal recovery and rollback.
///
/// This is not the generated cross-boundary `ExecutionResultManifest`. The
/// composition root must materialize that canonical contract from this result
/// plus the bound candidate, approval, policy, host, and workspace evidence.
pub struct LocalExecutionResult {
    pub schema_version: String,
    pub execution_id: ContractId,
    pub journal_id: ContractId,
    pub checkpoint_id: ContractId,
    pub candidate_hash: Sha256Digest,
    pub spec_hash: Sha256Digest,
    pub consumption_hash: Sha256Digest,
    pub files: Vec<FileObservation>,
    pub completed_at: UnixMillis,
    pub result_hash: Sha256Digest,
}

impl LocalExecutionResult {
    pub(crate) fn seal(
        journal: &EffectJournal,
        files: Vec<FileObservation>,
        completed_at: UnixMillis,
    ) -> Result<Self, ExecutionError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Draft<'a> {
            schema_version: &'static str,
            execution_id: &'a ContractId,
            journal_id: &'a ContractId,
            checkpoint_id: &'a ContractId,
            candidate_hash: Sha256Digest,
            spec_hash: Sha256Digest,
            consumption_hash: Sha256Digest,
            files: &'a [FileObservation],
            completed_at: UnixMillis,
        }

        let result_hash = canonical_hash(
            "windows-local-result",
            1,
            &Draft {
                schema_version: "sapphirus.windows-local-result.v1",
                execution_id: &journal.execution_id,
                journal_id: &journal.journal_id,
                checkpoint_id: &journal.checkpoint_id,
                candidate_hash: journal.candidate_hash,
                spec_hash: journal.spec_hash,
                consumption_hash: journal.consumption_hash,
                files: &files,
                completed_at,
            },
        )
        .map_err(DomainValidationError::from)?;
        Ok(Self {
            schema_version: "sapphirus.windows-local-result.v1".to_owned(),
            execution_id: journal.execution_id.clone(),
            journal_id: journal.journal_id.clone(),
            checkpoint_id: journal.checkpoint_id.clone(),
            candidate_hash: journal.candidate_hash,
            spec_hash: journal.spec_hash,
            consumption_hash: journal.consumption_hash,
            files,
            completed_at,
            result_hash,
        })
    }

    /// Verifies file observations and the canonical execution-result hash.
    ///
    /// # Errors
    ///
    /// Returns [`ExecutionError`] when result content, ordering, or its bound
    /// hash is invalid.
    pub fn verify(&self) -> Result<(), ExecutionError> {
        if self.schema_version != "sapphirus.windows-local-result.v1"
            || self.files.windows(2).any(|pair| {
                !pair[0]
                    .relative_path
                    .canonical_cmp(&pair[1].relative_path)
                    .is_lt()
            })
            || self.files.iter().any(|file| match file.operation {
                ResultFileOperation::Created | ResultFileOperation::Modified => {
                    !file.exists || file.content_hash.is_none()
                }
                ResultFileOperation::Deleted => file.exists || file.content_hash.is_some(),
            })
        {
            return Err(ExecutionError::IntegrityFailure);
        }
        let actual = desktop_runtime::canonical_hash_without_field(
            "windows-local-result",
            1,
            self,
            "resultHash",
        )
        .map_err(DomainValidationError::from)?;
        if actual != self.result_hash {
            return Err(ExecutionError::IntegrityFailure);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryDisposition {
    Complete,
    RestoreCheckpoint,
    ManualReview,
    NoEffect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryPlan {
    pub journal_id: ContractId,
    pub execution_id: ContractId,
    pub checkpoint_id: ContractId,
    pub workspace_target_hash: Sha256Digest,
    pub disposition: RecoveryDisposition,
    pub operations: Vec<RecoveryOperation>,
    pub plan_hash: Sha256Digest,
    pub reason: RecoveryReason,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryOperation {
    pub relative_path: RelativeWorkspacePath,
    pub expected_current_exists: bool,
    pub expected_current_content_hash: Option<Sha256Digest>,
    pub expected_current_file_identity_hash: Option<Sha256Digest>,
    pub restore_to: CheckpointFileState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryReason {
    NoEffectObserved,
    PostimagesVerified,
    CompleteCheckpointCoverage,
    IncompleteCheckpointCoverage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryRestoreResult {
    pub journal_id: ContractId,
    pub restored_count: usize,
}

#[derive(Clone, Debug)]
pub struct ExecutionOutcome {
    pub checkpoint: LocalCheckpoint,
    pub journal: EffectJournal,
    pub result: LocalExecutionResult,
}

#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("execution authorization is invalid or does not bind this patch")]
    AuthorizationMismatch,
    #[error("workspace preimage changed before the governed effect")]
    PreconditionFailed,
    #[error("workspace content is not supported bounded UTF-8 text")]
    UnsupportedContent,
    #[error("workspace capability or atomic file operation failed")]
    WorkspaceFailure,
    #[error("durable checkpoint or journal storage failed")]
    StoreFailure,
    #[error("effect may be partial and requires journal recovery")]
    RecoveryRequired,
    #[error("checkpoint, journal, or result integrity verification failed")]
    IntegrityFailure,
    #[error(transparent)]
    InvalidDomain(#[from] DomainValidationError),
}

pub(crate) fn journal_operation(index: u16, operation: &PatchOperation) -> JournalOperation {
    let (operation_kind, postimage_hash) = match operation {
        PatchOperation::Create { .. } => (ResultFileOperation::Created, operation.postimage_hash()),
        PatchOperation::Replace { .. } => {
            (ResultFileOperation::Modified, operation.postimage_hash())
        }
        PatchOperation::Delete { .. } => (ResultFileOperation::Deleted, None),
    };
    JournalOperation {
        ordinal: index.saturating_add(1),
        relative_path: operation.relative_path().clone(),
        operation: operation_kind,
        preimage_hash: operation.preimage_hash(),
        postimage_hash,
        state: JournalOperationState::Pending,
    }
}
