use desktop_runtime::{
    CommandReceipt, ContractId, LocalError, ProjectionEvent, ProjectionSnapshot,
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
    ContextPreview(ContextPreviewProjection),
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
