use desktop_ipc::{
    BmadHelpApprovedLifecycleProjection, BmadHelpApprovedProjection, BmadHelpCancelledProjection,
    BmadHelpReviewProjection, BmadHelpRunCompletedProjection, BmadHelpRunCreatedProjection,
    BmadHelpTerminalProjection, BmadLibrarySnapshotProjection, BmadPersonaPerspectiveProjection,
    ModelAuthStatusProjection,
};
use desktop_runtime::{
    ChangesReviewProjection, CommandReceipt, ContractId, LocalError, ProjectionEvent,
    ProjectionSnapshot, RelativeWorkspacePath, Sha256Digest, UnixMillis,
};
use desktop_workspace::{
    BmadScanProjection, EntryKind, SearchMatch, TextPreview, WorkspaceProjection,
};
use serde::{Deserialize, Serialize};

pub const BOOTSTRAP_SCHEMA: &str = "desktop-bootstrap.v1";
pub const DISPATCH_REPLY_SCHEMA: &str = "desktop-dispatch-reply.v1";
pub const PROJECTION_REQUEST_SCHEMA: &str = "desktop-projection-request.v1";
pub const PROJECTION_REPLY_SCHEMA: &str = "desktop-projection-reply.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BootMode {
    Ready,
    ReadOnlyRecovery,
}

impl BootMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::ReadOnlyRecovery => "read_only_recovery",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapReply {
    pub schema_version: String,
    pub renderer_session_id: ContractId,
    pub installation_id: ContractId,
    pub window_label: String,
    pub boot_mode: BootMode,
    pub supported_commands: Vec<String>,
    pub workspaces: Vec<WorkspaceProjection>,
    pub projection_sequence: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostDispatchReply {
    pub schema_version: String,
    pub request_id: Option<ContractId>,
    pub sequence: u64,
    #[serde(flatten)]
    pub outcome: HostDispatchOutcome,
}

impl HostDispatchReply {
    pub fn success(
        request_id: ContractId,
        sequence: u64,
        receipt: CommandReceipt,
        data: HostCommandData,
    ) -> Self {
        Self {
            schema_version: DISPATCH_REPLY_SCHEMA.to_owned(),
            request_id: Some(request_id),
            sequence,
            outcome: HostDispatchOutcome::Ok {
                receipt,
                data: Box::new(data),
            },
        }
    }

    pub fn error(request_id: Option<ContractId>, sequence: u64, error: LocalError) -> Self {
        Self {
            schema_version: DISPATCH_REPLY_SCHEMA.to_owned(),
            request_id,
            sequence,
            outcome: HostDispatchOutcome::Error { error },
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum HostDispatchOutcome {
    Ok {
        receipt: CommandReceipt,
        data: Box<HostCommandData>,
    },
    Error {
        error: LocalError,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CapabilityCancelledProjection {
    pub capability_id: String,
    pub manifest_hash: desktop_runtime::Sha256Digest,
    pub cancelled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CapabilityRunLatestProjection {
    pub capability_id: String,
    pub found: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_json: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum HostCommandData {
    BootState(BootStateProjection),
    NoSelection,
    WorkspaceSelected(WorkspaceProjection),
    WorkspaceList(Vec<WorkspaceProjection>),
    WorkspaceRevoked(WorkspaceProjection),
    WorkspaceEntries(WorkspaceEntriesProjection),
    WorkspaceText(TextPreview),
    SearchResults(Vec<SearchMatch>),
    BmadScan(BmadScanProjection),
    BmadLibrarySnapshot(BmadLibrarySnapshotProjection),
    BmadPersonaPerspective(BmadPersonaPerspectiveProjection),
    CapabilityReview(crate::bmad_model::capability_coordinator::CapabilityReviewProjection),
    CapabilityApproved(crate::bmad_model::capability_coordinator::CapabilityApprovedProjection),
    CapabilityCancelled(CapabilityCancelledProjection),
    CapabilityCompleted(crate::bmad_model::capability_coordinator::CapabilityCompletedProjection),
    CapabilityRunLatest(CapabilityRunLatestProjection),
    RetentionManifest(desktop_ipc::RetentionManifestProjection),
    OffboardingErased(desktop_ipc::OffboardingErasedProjection),
    ModelAuthStatus(ModelAuthStatusProjection),
    BmadHelpReview(BmadHelpReviewProjection),
    BmadHelpApproved(BmadHelpApprovedProjection),
    BmadHelpApprovedLifecycle(BmadHelpApprovedLifecycleProjection),
    BmadHelpCancelled(BmadHelpCancelledProjection),
    BmadHelpTerminal(BmadHelpTerminalProjection),
    BmadHelpRunCreated(BmadHelpRunCreatedProjection),
    BmadHelpRunInterrupted(BmadHelpRunCreatedProjection),
    BmadHelpRunCompleted(BmadHelpRunCompletedProjection),
    NoBmadHelpRun,
    BmadHelpProjectionUnavailable,
    ContextPreview(ContextPreviewProjection),
    WorkspaceEditsEnabled(desktop_workspace::WorkspaceProjection),
    ChangesReview(ChangesReviewWire),
    ChangesDecision(ChangesDecisionWire),
    #[allow(
        dead_code,
        reason = "the Task 3 recovery projection is routed by the Task 4 command boundary"
    )]
    ChangesRecoveryPrepared(ChangesRecoveryPreparedWire),
    #[allow(
        dead_code,
        reason = "the Task 3 recovery projection is routed by the Task 4 command boundary"
    )]
    ChangesRecoveryDecision(ChangesRecoveryDecisionWire),
    ChangesUndoUnavailable(ChangesUndoUnavailableWire),
    ChangesHistory(ChangesHistoryWire),
    Preferences(PreferencesProjection),
    About(AboutProjection),
    PickedFiles(PickedFilesProjection),
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PickedFilesProjection {
    pub workspace_id: ContractId,
    pub relative_paths: Vec<String>,
    pub selected_count: u32,
    pub rejected_outside_root: u32,
    pub rejected_unreadable: u32,
    pub truncated: bool,
}

pub const PREFERENCES_SCHEMA: &str = "desktop-preferences.v1";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreferencesProjection {
    pub schema_version: String,
    pub theme: desktop_runtime::ThemePreference,
    pub density: desktop_runtime::DensityPreference,
    pub updated_at: Option<desktop_runtime::UnixMillis>,
}

impl Default for PreferencesProjection {
    fn default() -> Self {
        Self {
            schema_version: PREFERENCES_SCHEMA.to_owned(),
            theme: desktop_runtime::ThemePreference::Dark,
            density: desktop_runtime::DensityPreference::Comfortable,
            updated_at: None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutProjection {
    pub app_version: String,
    pub installation_id: ContractId,
    pub boot_mode: BootMode,
    pub foundation_package_name: String,
    pub foundation_package_version: String,
    pub inactive_builder_package_count: u32,
    pub update_configured: bool,
    pub update_install_available: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangesReviewWire {
    pub approval_id: ContractId,
    pub displayed_diff_hash: Sha256Digest,
    pub review: ChangesReviewProjection,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangesDecisionWire {
    pub approval_id: ContractId,
    pub disposition: String,
    pub execution: Option<ChangesExecutionWire>,
}

#[allow(
    dead_code,
    reason = "the Task 3 recovery projection is routed by the Task 4 command boundary"
)]
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ChangesRecoveryPreparedWire {
    ReviewRequired {
        recovery_approval_id: ContractId,
        displayed_recovery_hash: Sha256Digest,
        journal_id: ContractId,
        execution_id: ContractId,
        operations: Vec<RecoveryOperationSummaryWire>,
        expires_at: UnixMillis,
    },
    AlreadyRecovered {
        journal_id: ContractId,
        execution_id: ContractId,
    },
    ManualReview {
        journal_id: ContractId,
        execution_id: ContractId,
        reason_code: RecoveryManualReviewReasonWire,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryManualReviewReasonWire {
    CheckpointIncompleteOrInconsistent,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryOperationSummaryWire {
    pub relative_path: RelativeWorkspacePath,
    pub operation: String,
    pub explanation: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangesRecoveryDecisionWire {
    pub recovery_approval_id: ContractId,
    pub disposition: String,
    pub journal_id: ContractId,
    pub execution_id: ContractId,
    pub restored_files: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangesExecutionWire {
    pub execution_id: ContractId,
    pub checkpoint_id: ContractId,
    pub completed_at: UnixMillis,
    pub undoable: bool,
    pub files: Vec<AppliedFileWire>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedFileWire {
    pub relative_path: RelativeWorkspacePath,
    pub operation: String,
    pub exists: bool,
    pub content_hash: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangesUndoUnavailableWire {
    pub execution_id: ContractId,
    pub reason: String,
    pub conflicts: Vec<UndoConflictWire>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoConflictWire {
    pub relative_path: RelativeWorkspacePath,
    pub expected_exists: bool,
    pub current_exists: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangesHistoryWire {
    pub workspace_id: ContractId,
    pub entries: Vec<ChangesHistoryEntryWire>,
    pub open_journals: Vec<OpenJournalWire>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangesHistoryEntryWire {
    pub execution_id: String,
    pub journal_state: String,
    pub file_count: u32,
    pub completed_at: String,
    pub undoable: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenJournalWire {
    pub journal_id: String,
    pub execution_id: String,
    pub state: String,
    pub updated_at: String,
    pub recovery_availability: RecoveryAvailabilityWire,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryAvailabilityWire {
    ReviewAvailable,
    Quarantined,
    ManualReview,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootStateProjection {
    pub mode: BootMode,
    pub workspace_count: u32,
    pub connected_features_available: bool,
    pub local_edits_available: bool,
    pub recovery_message: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEntriesProjection {
    pub workspace_id: ContractId,
    pub entries: Vec<TreeEntryProjection>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeEntryProjection {
    pub relative_path: String,
    pub kind: EntryKind,
    pub size_bytes: u64,
    pub child_cursor: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextPreviewProjection {
    pub workspace_id: ContractId,
    pub manifest_hash: String,
    pub items: Vec<ContextItemProjection>,
    pub total_bytes: u64,
    pub estimated_tokens: u64,
    pub model_target: Option<ModelTargetProjection>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextItemProjection {
    pub relative_path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub reason: String,
    pub content_hash: String,
    pub classification: String,
    pub redactions: Vec<String>,
    pub byte_count: u64,
    pub estimated_tokens: u64,
    pub content: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelTargetProjection {
    pub model: String,
    pub deployment: String,
    pub region: String,
    pub retention: String,
    pub schema_hash: String,
    pub profile_hash: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectionRequest {
    pub schema_version: String,
    pub renderer_session_id: ContractId,
    pub installation_id: ContractId,
    pub workspace_id: Option<ContractId>,
    pub session_id: Option<ContractId>,
    pub after_sequence: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectionReply {
    pub schema_version: String,
    pub renderer_session_id: Option<ContractId>,
    #[serde(flatten)]
    pub outcome: ProjectionReplyOutcome,
}

impl ProjectionReply {
    pub fn snapshot(renderer_session_id: ContractId, snapshot: ProjectionSnapshot) -> Self {
        Self {
            schema_version: PROJECTION_REPLY_SCHEMA.to_owned(),
            renderer_session_id: Some(renderer_session_id),
            outcome: ProjectionReplyOutcome::Snapshot { snapshot },
        }
    }

    pub fn events(renderer_session_id: ContractId, events: Vec<ProjectionEvent>) -> Self {
        Self {
            schema_version: PROJECTION_REPLY_SCHEMA.to_owned(),
            renderer_session_id: Some(renderer_session_id),
            outcome: ProjectionReplyOutcome::Events { events },
        }
    }

    pub fn error(renderer_session_id: Option<ContractId>, error: LocalError) -> Self {
        Self {
            schema_version: PROJECTION_REPLY_SCHEMA.to_owned(),
            renderer_session_id,
            outcome: ProjectionReplyOutcome::Error { error },
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ProjectionReplyOutcome {
    Snapshot { snapshot: ProjectionSnapshot },
    Events { events: Vec<ProjectionEvent> },
    Error { error: LocalError },
}
