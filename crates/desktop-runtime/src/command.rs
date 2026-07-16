use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{
    BmadHelpIntent, ContractId, LocalResult, RelativeWorkspacePath, Sha256Digest, UnixMillis,
};

/// User-facing approval outcomes. Only `Apply` can lead to spec issuance.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalChoice {
    Apply,
    Revise,
    Discard,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadLibraryProjectionScope {
    InstalledMethod,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadProjectionInvalidationScope {
    Library,
}

/// Narrow desktop commands accepted by the local runtime.
///
/// Notably absent: arbitrary paths, SQL, shell text, executable paths, provider
/// endpoints, access tokens, and a direct "execute spec" primitive.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", content = "input", rename_all = "snake_case")]
pub enum LocalCommand {
    GetBootState,
    SelectWorkspace,
    ListWorkspaces,
    RevokeWorkspace {
        workspace_id: ContractId,
    },
    ListWorkspaceEntries {
        workspace_id: ContractId,
        cursor: Option<String>,
        limit: u16,
    },
    ReadWorkspaceText {
        workspace_id: ContractId,
        relative_path: RelativeWorkspacePath,
        max_bytes: u32,
    },
    SearchWorkspace {
        workspace_id: ContractId,
        query: String,
        max_results: u16,
    },
    ScanBmad {
        workspace_id: ContractId,
    },
    BmadLibrarySnapshot {
        scope: BmadLibraryProjectionScope,
        cursor: Option<String>,
    },
    CreateBmadHelpRun {
        workspace_id: ContractId,
        workspace_grant_epoch: u64,
        current_intent: BmadHelpIntent,
    },
    LatestBmadHelpRun {
        workspace_id: ContractId,
        workspace_grant_epoch: u64,
    },
    PreviewContext {
        workspace_id: ContractId,
        relative_paths: Vec<RelativeWorkspacePath>,
    },
    CreateSession {
        workspace_id: ContractId,
    },
    SubmitTask {
        session_id: ContractId,
        prompt: String,
        context_manifest_hash: Sha256Digest,
    },
    CancelTask {
        task_id: ContractId,
    },
    DecideApproval {
        approval_id: ContractId,
        candidate_hash: Sha256Digest,
        displayed_diff_hash: Sha256Digest,
        choice: ApprovalChoice,
    },
    RequestRollback {
        execution_id: ContractId,
    },
    MaterializeEvidence {
        session_id: ContractId,
    },
    ExportEvidence {
        bundle_id: ContractId,
    },
}

impl LocalCommand {
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::GetBootState => "app.get_boot_state",
            Self::SelectWorkspace => "workspace.select_folder",
            Self::ListWorkspaces => "workspace.list",
            Self::RevokeWorkspace { .. } => "workspace.revoke",
            Self::ListWorkspaceEntries { .. } => "workspace.list_entries",
            Self::ReadWorkspaceText { .. } => "workspace.read_text",
            Self::SearchWorkspace { .. } => "workspace.search",
            Self::ScanBmad { .. } => "bmad.scan",
            Self::BmadLibrarySnapshot { .. } => "bmad.library.snapshot",
            Self::CreateBmadHelpRun { .. } => "run.create",
            Self::LatestBmadHelpRun { .. } => "bmad.help.latest",
            Self::PreviewContext { .. } => "context.preview",
            Self::CreateSession { .. } => "session.create",
            Self::SubmitTask { .. } => "task.submit",
            Self::CancelTask { .. } => "task.cancel",
            Self::DecideApproval { .. } => "approval.decide",
            Self::RequestRollback { .. } => "rollback.request",
            Self::MaterializeEvidence { .. } => "evidence.materialize",
            Self::ExportEvidence { .. } => "evidence.export",
        }
    }

    #[must_use]
    pub const fn is_mutating(&self) -> bool {
        !matches!(
            self,
            Self::GetBootState
                | Self::ListWorkspaces
                | Self::ListWorkspaceEntries { .. }
                | Self::ReadWorkspaceText { .. }
                | Self::SearchWorkspace { .. }
                | Self::ScanBmad { .. }
                | Self::BmadLibrarySnapshot { .. }
                | Self::LatestBmadHelpRun { .. }
                | Self::PreviewContext { .. }
        )
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommandReceipt {
    pub request_id: ContractId,
    pub accepted_at: UnixMillis,
    pub operation_id: Option<ContractId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectionScope {
    pub workspace_id: Option<ContractId>,
    pub session_id: Option<ContractId>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ProjectionCursor(pub u64);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectionSnapshot {
    pub sequence: u64,
    pub generated_at: UnixMillis,
    pub boot_mode: String,
    pub workspace_count: u32,
    pub active_session_id: Option<ContractId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectionEvent {
    pub sequence: u64,
    pub occurred_at: UnixMillis,
    pub event: ProjectionEventKind,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", content = "projection", rename_all = "snake_case")]
pub enum ProjectionEventKind {
    BootStateChanged {
        mode: String,
    },
    WorkspaceChanged {
        workspace_id: ContractId,
    },
    SessionChanged {
        session_id: ContractId,
        state: String,
    },
    ApprovalRequired {
        approval_id: ContractId,
        candidate_hash: Sha256Digest,
    },
    ExecutionStateChanged {
        execution_id: ContractId,
        state: String,
    },
    CheckpointChanged {
        checkpoint_id: ContractId,
        rollback_available: bool,
    },
    EvidenceChanged {
        stream_id: ContractId,
    },
    ConnectivityChanged {
        state: String,
    },
    UpdateStateChanged {
        state: String,
    },
    #[serde(rename = "bmad.projection_changed")]
    BmadProjectionChanged {
        scope: BmadProjectionInvalidationScope,
    },
}

#[async_trait]
pub trait LocalRuntimeCommandBus: Send + Sync {
    async fn execute(
        &self,
        request_id: ContractId,
        command: LocalCommand,
    ) -> LocalResult<CommandReceipt>;
}

#[async_trait]
pub trait RendererProjection: Send + Sync {
    async fn snapshot(&self, scope: ProjectionScope) -> LocalResult<ProjectionSnapshot>;

    async fn events_after(
        &self,
        scope: ProjectionScope,
        cursor: ProjectionCursor,
    ) -> LocalResult<Vec<ProjectionEvent>>;
}

#[cfg(test)]
mod tests {
    use super::{ApprovalChoice, BmadLibraryProjectionScope, LocalCommand};
    use crate::{sha256_bytes, ContractId, RelativeWorkspacePath};

    fn id(value: &str) -> Result<ContractId, Box<dyn std::error::Error>> {
        Ok(ContractId::new(value)?)
    }

    #[test]
    fn catalog_contains_no_generic_effect_primitive() -> Result<(), Box<dyn std::error::Error>> {
        let workspace_id = id("workspace_1")?;
        let session_id = id("session_1")?;
        let task_id = id("task_1")?;
        let approval_id = id("approval_1")?;
        let execution_id = id("execution_1")?;
        let bundle_id = id("bundle_1")?;
        let digest = sha256_bytes(b"test");
        let commands = [
            LocalCommand::GetBootState,
            LocalCommand::SelectWorkspace,
            LocalCommand::ListWorkspaces,
            LocalCommand::RevokeWorkspace {
                workspace_id: workspace_id.clone(),
            },
            LocalCommand::ListWorkspaceEntries {
                workspace_id: workspace_id.clone(),
                cursor: None,
                limit: 1,
            },
            LocalCommand::ReadWorkspaceText {
                workspace_id: workspace_id.clone(),
                relative_path: RelativeWorkspacePath::new("README.md")?,
                max_bytes: 1,
            },
            LocalCommand::SearchWorkspace {
                workspace_id: workspace_id.clone(),
                query: "query".to_owned(),
                max_results: 1,
            },
            LocalCommand::ScanBmad {
                workspace_id: workspace_id.clone(),
            },
            LocalCommand::BmadLibrarySnapshot {
                scope: BmadLibraryProjectionScope::InstalledMethod,
                cursor: None,
            },
            LocalCommand::CreateBmadHelpRun {
                workspace_id: workspace_id.clone(),
                workspace_grant_epoch: 1,
                current_intent: crate::BmadHelpIntent::new("find the next Method step")?,
            },
            LocalCommand::LatestBmadHelpRun {
                workspace_id: workspace_id.clone(),
                workspace_grant_epoch: 1,
            },
            LocalCommand::PreviewContext {
                workspace_id: workspace_id.clone(),
                relative_paths: vec![RelativeWorkspacePath::new("README.md")?],
            },
            LocalCommand::CreateSession { workspace_id },
            LocalCommand::SubmitTask {
                session_id: session_id.clone(),
                prompt: "task".to_owned(),
                context_manifest_hash: digest,
            },
            LocalCommand::CancelTask { task_id },
            LocalCommand::DecideApproval {
                approval_id,
                candidate_hash: digest,
                displayed_diff_hash: digest,
                choice: ApprovalChoice::Apply,
            },
            LocalCommand::RequestRollback { execution_id },
            LocalCommand::MaterializeEvidence { session_id },
            LocalCommand::ExportEvidence { bundle_id },
        ];
        for command in commands {
            let name = command.name();
            assert!(!name.contains("shell"));
            assert!(!name.contains("spawn"));
            assert!(!name.contains("write_path"));
            assert!(!name.contains("execute_spec"));
        }
        Ok(())
    }

    #[test]
    fn latest_help_run_is_an_explicit_read_only_capability(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let command = LocalCommand::LatestBmadHelpRun {
            workspace_id: id("workspace_1")?,
            workspace_grant_epoch: 1,
        };

        assert_eq!(command.name(), "bmad.help.latest");
        assert!(!command.is_mutating());
        Ok(())
    }
}
