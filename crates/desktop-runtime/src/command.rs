use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{
    BmadHelpIntent, ContractId, LocalResult, ProposedFileChange, RelativeWorkspacePath,
    Sha256Digest, UnixMillis,
};

/// User-facing approval outcomes. Only `Apply` can lead to spec issuance.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalChoice {
    Apply,
    Revise,
    Discard,
}

/// User-facing outcomes for one fresh, recovery-only approval.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryApprovalChoice {
    Restore,
    Cancel,
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

/// Renderer theme preference persisted by the local authority store.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    Light,
    Dark,
    System,
}

/// Renderer interface-density preference persisted by the local authority store.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DensityPreference {
    Comfortable,
    Compact,
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
    PickWorkspaceFiles {
        workspace_id: ContractId,
    },
    BmadLibrarySnapshot {
        scope: BmadLibraryProjectionScope,
        cursor: Option<String>,
    },
    ViewBmadPersona {
        agent_code: String,
    },
    OffboardingInspect,
    OffboardingErase,
    CreateBmadHelpRun {
        workspace_id: ContractId,
        workspace_grant_epoch: u64,
        current_intent: BmadHelpIntent,
    },
    ModelAuthStatus,
    ModelAuthSignIn,
    ModelAuthSignOut,
    PrepareBmadHelpReview {
        workspace_id: ContractId,
        workspace_grant_epoch: u64,
    },
    ApproveBmadHelpReview {
        workspace_id: ContractId,
        workspace_grant_epoch: u64,
        manifest_hash: Sha256Digest,
    },
    CancelBmadHelpReview {
        workspace_id: ContractId,
        workspace_grant_epoch: u64,
        manifest_hash: Sha256Digest,
        decision_id: ContractId,
    },
    SubmitBmadHelpReview {
        workspace_id: ContractId,
        workspace_grant_epoch: u64,
        manifest_hash: Sha256Digest,
        decision_id: ContractId,
    },
    LatestBmadHelpRun {
        workspace_id: ContractId,
        workspace_grant_epoch: u64,
    },
    PreviewContext {
        workspace_id: ContractId,
        relative_paths: Vec<RelativeWorkspacePath>,
    },
    EnableWorkspaceEdits {
        workspace_id: ContractId,
        workspace_grant_epoch: u64,
    },
    ProposeChanges {
        workspace_id: ContractId,
        workspace_grant_epoch: u64,
        changes: Vec<ProposedFileChange>,
    },
    ChangesHistory {
        workspace_id: ContractId,
        workspace_grant_epoch: u64,
    },
    PrepareChangesRecovery {
        workspace_id: ContractId,
        workspace_grant_epoch: u64,
        journal_id: ContractId,
    },
    DecideChangesRecovery {
        recovery_approval_id: ContractId,
        displayed_recovery_hash: Sha256Digest,
        choice: RecoveryApprovalChoice,
    },
    GetPreferences,
    SetPreferences {
        theme: ThemePreference,
        density: DensityPreference,
    },
    GetAbout,
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
            Self::PickWorkspaceFiles { .. } => "workspace.pick_files",
            Self::BmadLibrarySnapshot { .. } => "bmad.library.snapshot",
            Self::ViewBmadPersona { .. } => "bmad.persona.view",
            Self::OffboardingInspect => "app.offboarding.inspect",
            Self::OffboardingErase => "app.offboarding.erase",
            Self::CreateBmadHelpRun { .. } => "run.create",
            Self::ModelAuthStatus => "model.auth.status",
            Self::ModelAuthSignIn => "model.auth.sign_in",
            Self::ModelAuthSignOut => "model.auth.sign_out",
            Self::PrepareBmadHelpReview { .. } => "bmad.help.prepare",
            Self::ApproveBmadHelpReview { .. } => "bmad.help.approve",
            Self::CancelBmadHelpReview { .. } => "bmad.help.cancel",
            Self::SubmitBmadHelpReview { .. } => "bmad.help.submit",
            Self::LatestBmadHelpRun { .. } => "bmad.help.latest",
            Self::PreviewContext { .. } => "context.preview",
            Self::EnableWorkspaceEdits { .. } => "workspace.enable_edits",
            Self::ProposeChanges { .. } => "changes.propose",
            Self::ChangesHistory { .. } => "changes.history",
            Self::PrepareChangesRecovery { .. } => "changes.recovery.prepare",
            Self::DecideChangesRecovery { .. } => "changes.recovery.decide",
            Self::GetPreferences => "app.preferences.get",
            Self::SetPreferences { .. } => "app.preferences.set",
            Self::GetAbout => "app.about",
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
                | Self::ViewBmadPersona { .. }
                | Self::OffboardingInspect
                | Self::ModelAuthStatus
                | Self::LatestBmadHelpRun { .. }
                | Self::PreviewContext { .. }
                | Self::ChangesHistory { .. }
                | Self::PrepareChangesRecovery { .. }
                | Self::GetPreferences
                | Self::GetAbout
        )
    }

    /// Returns whether a request identifier must be admitted exactly once or
    /// fingerprinted even when the command has no filesystem effect.
    #[must_use]
    pub const fn requires_request_tracking(&self) -> bool {
        self.is_mutating() || matches!(self, Self::PrepareChangesRecovery { .. })
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
#[serde(
    tag = "type",
    content = "projection",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
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
    use super::{
        ApprovalChoice, BmadLibraryProjectionScope, LocalCommand, ProjectionEventKind,
        RecoveryApprovalChoice,
    };
    use crate::{sha256_bytes, ContractId, RelativeWorkspacePath};

    fn id(value: &str) -> Result<ContractId, Box<dyn std::error::Error>> {
        Ok(ContractId::new(value)?)
    }

    /// The renderer's projection parser asserts exact camelCase keys per event
    /// (`apps/desktop-ui/src/lib/hostClient/projectionProtocol.ts`); this pins
    /// the host-side serialization to that contract.
    #[test]
    fn projection_events_serialize_with_camel_case_fields() -> Result<(), Box<dyn std::error::Error>>
    {
        assert_eq!(
            serde_json::to_value(ProjectionEventKind::WorkspaceChanged {
                workspace_id: id("workspace_1")?,
            })?,
            serde_json::json!({
                "type": "workspace_changed",
                "projection": { "workspaceId": "workspace_1" }
            })
        );
        assert_eq!(
            serde_json::to_value(ProjectionEventKind::CheckpointChanged {
                checkpoint_id: id("checkpoint_1")?,
                rollback_available: true,
            })?,
            serde_json::json!({
                "type": "checkpoint_changed",
                "projection": { "checkpointId": "checkpoint_1", "rollbackAvailable": true }
            })
        );
        assert_eq!(
            serde_json::to_value(ProjectionEventKind::ApprovalRequired {
                approval_id: id("approval_1")?,
                candidate_hash: sha256_bytes(b"candidate"),
            })?["projection"]["approvalId"],
            serde_json::json!("approval_1")
        );
        Ok(())
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

    #[test]
    fn recovery_commands_have_closed_names_and_effect_classification(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let prepare = LocalCommand::PrepareChangesRecovery {
            workspace_id: id("workspace_1")?,
            workspace_grant_epoch: 7,
            journal_id: id("journal_1")?,
        };
        let decide = LocalCommand::DecideChangesRecovery {
            recovery_approval_id: id("recovery_approval_1")?,
            displayed_recovery_hash: sha256_bytes(b"displayed recovery"),
            choice: RecoveryApprovalChoice::Restore,
        };

        assert_eq!(prepare.name(), "changes.recovery.prepare");
        assert!(!prepare.is_mutating());
        assert!(prepare.requires_request_tracking());
        assert_eq!(decide.name(), "changes.recovery.decide");
        assert!(decide.is_mutating());
        assert!(decide.requires_request_tracking());
        assert_eq!(
            serde_json::to_value(RecoveryApprovalChoice::Cancel)?,
            serde_json::json!("cancel")
        );
        Ok(())
    }
}
