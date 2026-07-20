use desktop_runtime::{
    deserialize_strict_json, ApprovalChoice, BmadHelpIntent, CommandReceipt, ContractId,
    LocalCommand, LocalError, ProjectionEvent, ProposedFileChange, RecoveryApprovalChoice,
    RelativeWorkspacePath, Sha256Digest, UnixMillis, HARD_MAX_CHANGED_FILES,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::bmad::{valid_bmad_cursor, BmadLibrarySnapshotPayload, BmadPersonaViewPayload};

pub const MAX_COMMAND_BYTES: usize = 128 * 1024;
const MAX_JSON_DEPTH: usize = 16;
const MAX_JSON_NODES: usize = 2_048;
const MAX_COLLECTION_ITEMS: usize = 256;
const MAX_STRING_BYTES: usize = 64 * 1024;
const MAX_PROMPT_BYTES: usize = 16 * 1024;
const MAX_QUERY_BYTES: usize = 512;
const MAX_CONTEXT_PATHS: usize = 100;
const MAX_READ_BYTES: u32 = 1024 * 1024;
const MAX_LIST_ENTRIES: u16 = 500;
const MAX_SEARCH_RESULTS: u16 = 200;
const MAX_REQUEST_AGE_MS: u64 = 5 * 60 * 1000;
const MAX_FUTURE_SKEW_MS: u64 = 30 * 1000;
const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug)]
pub struct IpcValidationContext {
    pub expected_window_label: String,
    pub renderer_session_id: ContractId,
    pub installation_id: ContractId,
    pub now: UnixMillis,
    /// The exact command names projected by the composition root for this
    /// renderer session. Known commands from a later phase remain unreachable
    /// until the host explicitly includes them here.
    pub allowed_commands: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedCommandEnvelope {
    pub(crate) request_id: ContractId,
    pub(crate) window_label: String,
    pub(crate) renderer_session_id: ContractId,
    pub(crate) installation_id: ContractId,
    pub(crate) issued_at: UnixMillis,
    pub(crate) command: LocalCommand,
}

impl ValidatedCommandEnvelope {
    #[must_use]
    pub fn request_id(&self) -> &ContractId {
        &self.request_id
    }

    #[must_use]
    pub fn command(&self) -> &LocalCommand {
        &self.command
    }

    #[must_use]
    pub fn renderer_session_id(&self) -> &ContractId {
        &self.renderer_session_id
    }

    #[must_use]
    pub fn into_command(self) -> (ContractId, LocalCommand) {
        (self.request_id, self.command)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawCommandEnvelope {
    schema_version: String,
    request_id: ContractId,
    command: String,
    window_label: String,
    renderer_session_id: ContractId,
    installation_id: ContractId,
    issued_at: UnixMillis,
    payload: Value,
}

#[derive(Debug, Error)]
pub enum IpcValidationError {
    #[error("command envelope exceeds its byte limit")]
    EnvelopeTooLarge,
    #[error("command envelope is not valid strict JSON")]
    InvalidJson,
    #[error("command envelope exceeds its structural limits")]
    StructuralLimit,
    #[error("command envelope schema is unsupported")]
    UnsupportedSchema,
    #[error("command is not in the desktop capability catalog")]
    UnknownCommand,
    #[error("command is known but unavailable in the current capability projection")]
    CapabilityUnavailable,
    #[error("command payload is invalid")]
    InvalidPayload,
    #[error("renderer session, installation, or window binding failed")]
    BindingMismatch,
    #[error("command timestamp is stale or in the future")]
    InvalidTimestamp,
    #[error("renderer command rate limit was exceeded")]
    RateLimited,
    #[error("request identifier was reused for different command content")]
    IdempotencyConflict,
    #[error("request admission state is unavailable")]
    AdmissionUnavailable,
}

pub struct CommandEnvelopeValidator;

impl CommandEnvelopeValidator {
    /// Parses and validates a renderer command against its native-host context.
    ///
    /// # Errors
    ///
    /// Returns [`IpcValidationError`] when the envelope is malformed, exceeds
    /// a boundary limit, fails its renderer bindings, names an unavailable
    /// capability, or contains an invalid payload.
    pub fn parse(
        bytes: &[u8],
        context: &IpcValidationContext,
    ) -> Result<ValidatedCommandEnvelope, IpcValidationError> {
        if bytes.len() > MAX_COMMAND_BYTES {
            return Err(IpcValidationError::EnvelopeTooLarge);
        }
        let value: Value =
            deserialize_strict_json(bytes).map_err(|_| IpcValidationError::InvalidJson)?;
        let mut nodes = 0;
        validate_structure(&value, 0, &mut nodes)?;
        let raw: RawCommandEnvelope =
            serde_json::from_value(value).map_err(|_| IpcValidationError::InvalidJson)?;

        if raw.schema_version != "desktop-ipc-command.v1" {
            return Err(IpcValidationError::UnsupportedSchema);
        }
        if raw.window_label != context.expected_window_label
            || raw.renderer_session_id != context.renderer_session_id
            || raw.installation_id != context.installation_id
        {
            return Err(IpcValidationError::BindingMismatch);
        }
        validate_timestamp(raw.issued_at, context.now)?;
        if !is_known_command(&raw.command) {
            return Err(IpcValidationError::UnknownCommand);
        }
        if !context
            .allowed_commands
            .iter()
            .any(|allowed| allowed == &raw.command)
        {
            return Err(IpcValidationError::CapabilityUnavailable);
        }
        let command = parse_command(&raw.command, raw.payload)?;

        Ok(ValidatedCommandEnvelope {
            request_id: raw.request_id,
            window_label: raw.window_label,
            renderer_session_id: raw.renderer_session_id,
            installation_id: raw.installation_id,
            issued_at: raw.issued_at,
            command,
        })
    }
}

fn is_known_command(command: &str) -> bool {
    matches!(
        command,
        "app.get_boot_state"
            | "workspace.select_folder"
            | "workspace.list"
            | "workspace.revoke"
            | "workspace.list_entries"
            | "workspace.read_text"
            | "workspace.search"
            | "bmad.scan"
            | "bmad.library.snapshot"
            | "bmad.persona.view"
            | "model.auth.status"
            | "model.auth.sign_in"
            | "model.auth.sign_out"
            | "bmad.help.prepare"
            | "bmad.help.approve"
            | "bmad.help.cancel"
            | "bmad.help.submit"
            | "bmad.help.latest"
            | "bmad.capability.prepare"
            | "bmad.capability.approve"
            | "bmad.capability.cancel"
            | "bmad.capability.submit"
            | "bmad.capability.latest"
            | "run.create"
            | "context.preview"
            | "workspace.enable_edits"
            | "changes.propose"
            | "approval.decide"
            | "rollback.request"
            | "changes.history"
            | "changes.recovery.prepare"
            | "changes.recovery.decide"
            | "app.preferences.get"
            | "app.preferences.set"
            | "app.about"
            | "app.offboarding.inspect"
            | "app.offboarding.erase"
            | "workspace.pick_files"
    )
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum IpcReply {
    Ok { receipt: CommandReceipt },
    Error { error: LocalError },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectionEventEnvelope {
    pub schema_version: String,
    pub renderer_session_id: ContractId,
    pub event: ProjectionEvent,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyPayload {}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceIdPayload {
    workspace_id: ContractId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PreferencesPayload {
    theme: desktop_runtime::ThemePreference,
    density: desktop_runtime::DensityPreference,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ListEntriesPayload {
    workspace_id: ContractId,
    cursor: Option<String>,
    limit: u16,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReadTextPayload {
    workspace_id: ContractId,
    relative_path: RelativeWorkspacePath,
    max_bytes: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SearchPayload {
    workspace_id: ContractId,
    query: String,
    max_results: u16,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContextPayload {
    workspace_id: ContractId,
    relative_paths: Vec<RelativeWorkspacePath>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SessionIdPayload {
    session_id: ContractId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SubmitTaskPayload {
    session_id: ContractId,
    prompt: String,
    context_manifest_hash: Sha256Digest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TaskIdPayload {
    task_id: ContractId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ApprovalPayload {
    approval_id: ContractId,
    candidate_hash: Sha256Digest,
    displayed_diff_hash: Sha256Digest,
    choice: ApprovalChoice,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExecutionIdPayload {
    execution_id: ContractId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BundleIdPayload {
    bundle_id: ContractId,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum BmadRunKindPayload {
    BmadHelp,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateBmadRunPayload {
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    run_kind: BmadRunKindPayload,
    current_intent: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LatestBmadHelpRunPayload {
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BmadHelpWorkspacePayload {
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceEpochPayload {
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BmadHelpManifestPayload {
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    manifest_hash: Sha256Digest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BmadHelpDecisionPayload {
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    manifest_hash: Sha256Digest,
    decision_id: ContractId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProposeChangesPayload {
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    changes: Vec<ProposedFileChange>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PrepareChangesRecoveryPayload {
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    journal_id: ContractId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DecideChangesRecoveryPayload {
    recovery_approval_id: ContractId,
    displayed_recovery_hash: Sha256Digest,
    choice: RecoveryApprovalChoice,
}

#[expect(
    clippy::too_many_lines,
    reason = "one flat dispatch row per reviewed catalog command"
)]
fn parse_command(command: &str, payload: Value) -> Result<LocalCommand, IpcValidationError> {
    match command {
        "app.get_boot_state" => {
            parse_empty(payload)?;
            Ok(LocalCommand::GetBootState)
        }
        "workspace.select_folder" => {
            parse_empty(payload)?;
            Ok(LocalCommand::SelectWorkspace)
        }
        "workspace.list" => {
            parse_empty(payload)?;
            Ok(LocalCommand::ListWorkspaces)
        }
        "workspace.revoke" => {
            let input: WorkspaceIdPayload = parse_payload(payload)?;
            Ok(LocalCommand::RevokeWorkspace {
                workspace_id: input.workspace_id,
            })
        }
        "workspace.list_entries" => parse_list_entries(payload),
        "workspace.read_text" => parse_read_text(payload),
        "workspace.search" => parse_search(payload),
        "bmad.scan" => {
            let input: WorkspaceIdPayload = parse_payload(payload)?;
            Ok(LocalCommand::ScanBmad {
                workspace_id: input.workspace_id,
            })
        }
        "bmad.library.snapshot" => parse_bmad_library_snapshot(payload),
        "bmad.persona.view" => parse_bmad_persona_view(payload),
        "app.offboarding.inspect" => {
            parse_empty(payload)?;
            Ok(LocalCommand::OffboardingInspect)
        }
        "app.offboarding.erase" => parse_offboarding_erase(payload),
        "model.auth.status" => {
            parse_empty(payload)?;
            Ok(LocalCommand::ModelAuthStatus)
        }
        "model.auth.sign_in" => {
            parse_empty(payload)?;
            Ok(LocalCommand::ModelAuthSignIn)
        }
        "model.auth.sign_out" => {
            parse_empty(payload)?;
            Ok(LocalCommand::ModelAuthSignOut)
        }
        "bmad.help.prepare" => parse_bmad_help_prepare(payload),
        "bmad.help.approve" => parse_bmad_help_approve(payload),
        "bmad.help.cancel" => parse_bmad_help_cancel(payload),
        "bmad.help.submit" => parse_bmad_help_submit(payload),
        "bmad.help.latest" => parse_bmad_help_latest(payload),
        "bmad.capability.prepare" => parse_bmad_capability_prepare(payload),
        "bmad.capability.approve" => parse_bmad_capability_approve(payload),
        "bmad.capability.cancel" => parse_bmad_capability_cancel(payload),
        "bmad.capability.submit" => parse_bmad_capability_submit(payload),
        "bmad.capability.latest" => parse_bmad_capability_latest(payload),
        "run.create" => parse_bmad_run_create(payload),
        "context.preview" => parse_context_preview(payload),
        "workspace.enable_edits" => {
            let input = parse_workspace_epoch(payload)?;
            Ok(LocalCommand::EnableWorkspaceEdits {
                workspace_id: input.workspace_id,
                workspace_grant_epoch: input.workspace_grant_epoch,
            })
        }
        "changes.propose" => parse_propose_changes(payload),
        "approval.decide" => {
            let input: ApprovalPayload = parse_payload(payload)?;
            Ok(LocalCommand::DecideApproval {
                approval_id: input.approval_id,
                candidate_hash: input.candidate_hash,
                displayed_diff_hash: input.displayed_diff_hash,
                choice: input.choice,
            })
        }
        "rollback.request" => {
            let input: ExecutionIdPayload = parse_payload(payload)?;
            Ok(LocalCommand::RequestRollback {
                execution_id: input.execution_id,
            })
        }
        "changes.history" => {
            let input = parse_workspace_epoch(payload)?;
            Ok(LocalCommand::ChangesHistory {
                workspace_id: input.workspace_id,
                workspace_grant_epoch: input.workspace_grant_epoch,
            })
        }
        "changes.recovery.prepare" => {
            let input: PrepareChangesRecoveryPayload = parse_payload(payload)?;
            validate_workspace_grant_epoch(input.workspace_grant_epoch)?;
            Ok(LocalCommand::PrepareChangesRecovery {
                workspace_id: input.workspace_id,
                workspace_grant_epoch: input.workspace_grant_epoch,
                journal_id: input.journal_id,
            })
        }
        "changes.recovery.decide" => {
            let input: DecideChangesRecoveryPayload = parse_payload(payload)?;
            Ok(LocalCommand::DecideChangesRecovery {
                recovery_approval_id: input.recovery_approval_id,
                displayed_recovery_hash: input.displayed_recovery_hash,
                choice: input.choice,
            })
        }
        "app.preferences.get" | "app.preferences.set" | "app.about" | "workspace.pick_files" => {
            parse_app_command(command, payload)
        }
        _ => parse_later_phase_command(command, payload),
    }
}

fn parse_app_command(command: &str, payload: Value) -> Result<LocalCommand, IpcValidationError> {
    match command {
        "app.preferences.get" => {
            parse_empty(payload)?;
            Ok(LocalCommand::GetPreferences)
        }
        "app.preferences.set" => {
            let input: PreferencesPayload = parse_payload(payload)?;
            Ok(LocalCommand::SetPreferences {
                theme: input.theme,
                density: input.density,
            })
        }
        "app.about" => {
            parse_empty(payload)?;
            Ok(LocalCommand::GetAbout)
        }
        "workspace.pick_files" => {
            let input: WorkspaceIdPayload = parse_payload(payload)?;
            Ok(LocalCommand::PickWorkspaceFiles {
                workspace_id: input.workspace_id,
            })
        }
        _ => Err(IpcValidationError::UnknownCommand),
    }
}

fn parse_workspace_epoch(payload: Value) -> Result<WorkspaceEpochPayload, IpcValidationError> {
    let input: WorkspaceEpochPayload = parse_payload(payload)?;
    if input.workspace_grant_epoch == 0 || input.workspace_grant_epoch > MAX_SAFE_JSON_INTEGER {
        return Err(IpcValidationError::InvalidPayload);
    }
    Ok(input)
}

fn parse_propose_changes(payload: Value) -> Result<LocalCommand, IpcValidationError> {
    let input: ProposeChangesPayload = parse_payload(payload)?;
    if input.workspace_grant_epoch == 0 || input.workspace_grant_epoch > MAX_SAFE_JSON_INTEGER {
        return Err(IpcValidationError::InvalidPayload);
    }
    if input.changes.is_empty() || input.changes.len() > HARD_MAX_CHANGED_FILES as usize {
        return Err(IpcValidationError::InvalidPayload);
    }
    let unique_paths: std::collections::BTreeSet<_> = input
        .changes
        .iter()
        .map(|change| change.relative_path().case_folded())
        .collect();
    if unique_paths.len() != input.changes.len() {
        return Err(IpcValidationError::InvalidPayload);
    }
    for change in &input.changes {
        if let ProposedFileChange::SetContent { content, .. } = change {
            if content.contains('\0') || content.len() > MAX_STRING_BYTES {
                return Err(IpcValidationError::InvalidPayload);
            }
        }
    }
    Ok(LocalCommand::ProposeChanges {
        workspace_id: input.workspace_id,
        workspace_grant_epoch: input.workspace_grant_epoch,
        changes: input.changes,
    })
}

fn parse_bmad_help_latest(payload: Value) -> Result<LocalCommand, IpcValidationError> {
    let input: LatestBmadHelpRunPayload = parse_payload(payload)?;
    validate_workspace_grant_epoch(input.workspace_grant_epoch)?;
    Ok(LocalCommand::LatestBmadHelpRun {
        workspace_id: input.workspace_id,
        workspace_grant_epoch: input.workspace_grant_epoch,
    })
}

fn parse_bmad_help_prepare(payload: Value) -> Result<LocalCommand, IpcValidationError> {
    let input: BmadHelpWorkspacePayload = parse_payload(payload)?;
    validate_workspace_grant_epoch(input.workspace_grant_epoch)?;
    Ok(LocalCommand::PrepareBmadHelpReview {
        workspace_id: input.workspace_id,
        workspace_grant_epoch: input.workspace_grant_epoch,
    })
}

fn parse_bmad_help_approve(payload: Value) -> Result<LocalCommand, IpcValidationError> {
    let input: BmadHelpManifestPayload = parse_payload(payload)?;
    validate_workspace_grant_epoch(input.workspace_grant_epoch)?;
    Ok(LocalCommand::ApproveBmadHelpReview {
        workspace_id: input.workspace_id,
        workspace_grant_epoch: input.workspace_grant_epoch,
        manifest_hash: input.manifest_hash,
    })
}

fn parse_bmad_help_cancel(payload: Value) -> Result<LocalCommand, IpcValidationError> {
    let input: BmadHelpDecisionPayload = parse_payload(payload)?;
    validate_workspace_grant_epoch(input.workspace_grant_epoch)?;
    Ok(LocalCommand::CancelBmadHelpReview {
        workspace_id: input.workspace_id,
        workspace_grant_epoch: input.workspace_grant_epoch,
        manifest_hash: input.manifest_hash,
        decision_id: input.decision_id,
    })
}

fn parse_bmad_help_submit(payload: Value) -> Result<LocalCommand, IpcValidationError> {
    let input: BmadHelpDecisionPayload = parse_payload(payload)?;
    validate_workspace_grant_epoch(input.workspace_grant_epoch)?;
    Ok(LocalCommand::SubmitBmadHelpReview {
        workspace_id: input.workspace_id,
        workspace_grant_epoch: input.workspace_grant_epoch,
        manifest_hash: input.manifest_hash,
        decision_id: input.decision_id,
    })
}

fn validate_workspace_grant_epoch(epoch: u64) -> Result<(), IpcValidationError> {
    if epoch == 0 || epoch > MAX_SAFE_JSON_INTEGER {
        return Err(IpcValidationError::InvalidPayload);
    }
    Ok(())
}

fn parse_bmad_run_create(payload: Value) -> Result<LocalCommand, IpcValidationError> {
    let input: CreateBmadRunPayload = parse_payload(payload)?;
    validate_workspace_grant_epoch(input.workspace_grant_epoch)?;
    let BmadRunKindPayload::BmadHelp = input.run_kind;
    let current_intent = BmadHelpIntent::new(input.current_intent)
        .map_err(|_| IpcValidationError::InvalidPayload)?;
    Ok(LocalCommand::CreateBmadHelpRun {
        workspace_id: input.workspace_id,
        workspace_grant_epoch: input.workspace_grant_epoch,
        current_intent,
    })
}

fn parse_bmad_library_snapshot(payload: Value) -> Result<LocalCommand, IpcValidationError> {
    let input: BmadLibrarySnapshotPayload = parse_payload(payload)?;
    if input
        .cursor
        .as_deref()
        .is_some_and(|cursor| !valid_bmad_cursor(cursor))
    {
        return Err(IpcValidationError::InvalidPayload);
    }
    Ok(LocalCommand::BmadLibrarySnapshot {
        scope: input.scope,
        cursor: input.cursor,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BmadCapabilityWorkspacePayload {
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    capability_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BmadCapabilityPreparePayload {
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    capability_id: String,
    context_paths: Vec<RelativeWorkspacePath>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BmadCapabilityManifestPayload {
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    capability_id: String,
    manifest_hash: Sha256Digest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BmadCapabilityDecisionPayload {
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    capability_id: String,
    manifest_hash: Sha256Digest,
    decision_id: ContractId,
}

const MAX_CAPABILITY_CONTEXT_PATHS: usize = 100;

/// Validates the closed ADR-0005 closure-ledger capability identifier:
/// `^(bmm|builder):[a-z][a-z0-9._-]{2,80}$`.
fn validate_capability_id(value: &str) -> Result<(), IpcValidationError> {
    let suffix = value
        .strip_prefix("bmm:")
        .or_else(|| value.strip_prefix("builder:"))
        .ok_or(IpcValidationError::InvalidPayload)?;
    let mut characters = suffix.chars();
    let first = characters
        .next()
        .ok_or(IpcValidationError::InvalidPayload)?;
    if !first.is_ascii_lowercase()
        || suffix.len() < 3
        || suffix.len() > 81
        || !characters
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
    {
        return Err(IpcValidationError::InvalidPayload);
    }
    Ok(())
}

fn parse_bmad_capability_prepare(payload: Value) -> Result<LocalCommand, IpcValidationError> {
    let input: BmadCapabilityPreparePayload = parse_payload(payload)?;
    validate_workspace_grant_epoch(input.workspace_grant_epoch)?;
    validate_capability_id(&input.capability_id)?;
    if input.context_paths.is_empty() || input.context_paths.len() > MAX_CAPABILITY_CONTEXT_PATHS {
        return Err(IpcValidationError::InvalidPayload);
    }
    Ok(LocalCommand::PrepareBmadCapabilityRun {
        workspace_id: input.workspace_id,
        workspace_grant_epoch: input.workspace_grant_epoch,
        capability_id: input.capability_id,
        context_paths: input.context_paths,
    })
}

fn parse_bmad_capability_approve(payload: Value) -> Result<LocalCommand, IpcValidationError> {
    let input: BmadCapabilityManifestPayload = parse_payload(payload)?;
    validate_workspace_grant_epoch(input.workspace_grant_epoch)?;
    validate_capability_id(&input.capability_id)?;
    Ok(LocalCommand::ApproveBmadCapabilityRun {
        workspace_id: input.workspace_id,
        workspace_grant_epoch: input.workspace_grant_epoch,
        capability_id: input.capability_id,
        manifest_hash: input.manifest_hash,
    })
}

fn parse_bmad_capability_cancel(payload: Value) -> Result<LocalCommand, IpcValidationError> {
    let input: BmadCapabilityDecisionPayload = parse_payload(payload)?;
    validate_workspace_grant_epoch(input.workspace_grant_epoch)?;
    validate_capability_id(&input.capability_id)?;
    Ok(LocalCommand::CancelBmadCapabilityRun {
        workspace_id: input.workspace_id,
        workspace_grant_epoch: input.workspace_grant_epoch,
        capability_id: input.capability_id,
        manifest_hash: input.manifest_hash,
        decision_id: input.decision_id,
    })
}

fn parse_bmad_capability_submit(payload: Value) -> Result<LocalCommand, IpcValidationError> {
    let input: BmadCapabilityDecisionPayload = parse_payload(payload)?;
    validate_workspace_grant_epoch(input.workspace_grant_epoch)?;
    validate_capability_id(&input.capability_id)?;
    Ok(LocalCommand::SubmitBmadCapabilityRun {
        workspace_id: input.workspace_id,
        workspace_grant_epoch: input.workspace_grant_epoch,
        capability_id: input.capability_id,
        manifest_hash: input.manifest_hash,
        decision_id: input.decision_id,
    })
}

fn parse_bmad_capability_latest(payload: Value) -> Result<LocalCommand, IpcValidationError> {
    let input: BmadCapabilityWorkspacePayload = parse_payload(payload)?;
    validate_workspace_grant_epoch(input.workspace_grant_epoch)?;
    validate_capability_id(&input.capability_id)?;
    Ok(LocalCommand::LatestBmadCapabilityRun {
        workspace_id: input.workspace_id,
        workspace_grant_epoch: input.workspace_grant_epoch,
        capability_id: input.capability_id,
    })
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OffboardingErasePayload {
    confirm: String,
}

/// The exact ADR-0004 confirmation phrase; anything else fails closed.
const OFFBOARDING_ERASE_CONFIRMATION: &str = "erase-local-authority-data";

fn parse_offboarding_erase(payload: Value) -> Result<LocalCommand, IpcValidationError> {
    let input: OffboardingErasePayload = parse_payload(payload)?;
    if input.confirm != OFFBOARDING_ERASE_CONFIRMATION {
        return Err(IpcValidationError::InvalidPayload);
    }
    Ok(LocalCommand::OffboardingErase)
}

fn parse_bmad_persona_view(payload: Value) -> Result<LocalCommand, IpcValidationError> {
    let input: BmadPersonaViewPayload = parse_payload(payload)?;
    let code = input.agent_code;
    let suffix = code.strip_prefix("bmad-agent-");
    let valid = code.len() <= 64
        && suffix.is_some_and(|rest| {
            rest.bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_lowercase())
                && rest
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        });
    if !valid {
        return Err(IpcValidationError::InvalidPayload);
    }
    Ok(LocalCommand::ViewBmadPersona { agent_code: code })
}

fn parse_list_entries(payload: Value) -> Result<LocalCommand, IpcValidationError> {
    let input: ListEntriesPayload = parse_payload(payload)?;
    if input.limit == 0 || input.limit > MAX_LIST_ENTRIES {
        return Err(IpcValidationError::InvalidPayload);
    }
    if input
        .cursor
        .as_ref()
        .is_some_and(|cursor| cursor.len() > 256)
    {
        return Err(IpcValidationError::InvalidPayload);
    }
    Ok(LocalCommand::ListWorkspaceEntries {
        workspace_id: input.workspace_id,
        cursor: input.cursor,
        limit: input.limit,
    })
}

fn parse_read_text(payload: Value) -> Result<LocalCommand, IpcValidationError> {
    let input: ReadTextPayload = parse_payload(payload)?;
    if input.max_bytes == 0 || input.max_bytes > MAX_READ_BYTES {
        return Err(IpcValidationError::InvalidPayload);
    }
    Ok(LocalCommand::ReadWorkspaceText {
        workspace_id: input.workspace_id,
        relative_path: input.relative_path,
        max_bytes: input.max_bytes,
    })
}

fn parse_search(payload: Value) -> Result<LocalCommand, IpcValidationError> {
    let input: SearchPayload = parse_payload(payload)?;
    if input.query.trim().is_empty()
        || input.query.len() > MAX_QUERY_BYTES
        || input.query.contains('\0')
        || input.max_results == 0
        || input.max_results > MAX_SEARCH_RESULTS
    {
        return Err(IpcValidationError::InvalidPayload);
    }
    Ok(LocalCommand::SearchWorkspace {
        workspace_id: input.workspace_id,
        query: input.query,
        max_results: input.max_results,
    })
}

fn parse_context_preview(payload: Value) -> Result<LocalCommand, IpcValidationError> {
    let input: ContextPayload = parse_payload(payload)?;
    if input.relative_paths.is_empty() || input.relative_paths.len() > MAX_CONTEXT_PATHS {
        return Err(IpcValidationError::InvalidPayload);
    }
    let unique_paths: std::collections::BTreeSet<_> = input
        .relative_paths
        .iter()
        .map(RelativeWorkspacePath::case_folded)
        .collect();
    if unique_paths.len() != input.relative_paths.len() {
        return Err(IpcValidationError::InvalidPayload);
    }
    Ok(LocalCommand::PreviewContext {
        workspace_id: input.workspace_id,
        relative_paths: input.relative_paths,
    })
}

fn parse_later_phase_command(
    command: &str,
    payload: Value,
) -> Result<LocalCommand, IpcValidationError> {
    match command {
        "session.create" => {
            let input: WorkspaceIdPayload = parse_payload(payload)?;
            Ok(LocalCommand::CreateSession {
                workspace_id: input.workspace_id,
            })
        }
        "task.submit" => {
            let input: SubmitTaskPayload = parse_payload(payload)?;
            if input.prompt.trim().is_empty()
                || input.prompt.len() > MAX_PROMPT_BYTES
                || input.prompt.contains('\0')
            {
                return Err(IpcValidationError::InvalidPayload);
            }
            Ok(LocalCommand::SubmitTask {
                session_id: input.session_id,
                prompt: input.prompt,
                context_manifest_hash: input.context_manifest_hash,
            })
        }
        "task.cancel" => {
            let input: TaskIdPayload = parse_payload(payload)?;
            Ok(LocalCommand::CancelTask {
                task_id: input.task_id,
            })
        }
        "evidence.materialize" => {
            let input: SessionIdPayload = parse_payload(payload)?;
            Ok(LocalCommand::MaterializeEvidence {
                session_id: input.session_id,
            })
        }
        "evidence.export" => {
            let input: BundleIdPayload = parse_payload(payload)?;
            Ok(LocalCommand::ExportEvidence {
                bundle_id: input.bundle_id,
            })
        }
        _ => Err(IpcValidationError::UnknownCommand),
    }
}

fn parse_empty(payload: Value) -> Result<(), IpcValidationError> {
    let _: EmptyPayload = parse_payload(payload)?;
    Ok(())
}

fn parse_payload<T>(payload: Value) -> Result<T, IpcValidationError>
where
    T: DeserializeOwned,
{
    serde_json::from_value(payload).map_err(|_| IpcValidationError::InvalidPayload)
}

fn validate_timestamp(issued_at: UnixMillis, now: UnixMillis) -> Result<(), IpcValidationError> {
    let oldest = now.0.saturating_sub(MAX_REQUEST_AGE_MS);
    let newest = now.0.saturating_add(MAX_FUTURE_SKEW_MS);
    if issued_at.0 < oldest || issued_at.0 > newest {
        return Err(IpcValidationError::InvalidTimestamp);
    }
    Ok(())
}

fn validate_structure(
    value: &Value,
    depth: usize,
    nodes: &mut usize,
) -> Result<(), IpcValidationError> {
    *nodes = nodes
        .checked_add(1)
        .ok_or(IpcValidationError::StructuralLimit)?;
    if depth > MAX_JSON_DEPTH || *nodes > MAX_JSON_NODES {
        return Err(IpcValidationError::StructuralLimit);
    }
    match value {
        Value::String(text) if text.len() > MAX_STRING_BYTES => {
            Err(IpcValidationError::StructuralLimit)
        }
        Value::Array(items) => {
            if items.len() > MAX_COLLECTION_ITEMS {
                return Err(IpcValidationError::StructuralLimit);
            }
            for item in items {
                validate_structure(item, depth + 1, nodes)?;
            }
            Ok(())
        }
        Value::Object(fields) => {
            if fields.len() > MAX_COLLECTION_ITEMS {
                return Err(IpcValidationError::StructuralLimit);
            }
            for (key, item) in fields {
                if key.len() > MAX_STRING_BYTES {
                    return Err(IpcValidationError::StructuralLimit);
                }
                validate_structure(item, depth + 1, nodes)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use desktop_runtime::{
        sha256_bytes, ContractId, LocalCommand, RecoveryApprovalChoice, UnixMillis,
    };

    use super::{CommandEnvelopeValidator, IpcValidationContext, IpcValidationError};

    fn id(value: &str) -> Result<ContractId, Box<dyn std::error::Error>> {
        Ok(ContractId::new(value)?)
    }

    fn context() -> Result<IpcValidationContext, Box<dyn std::error::Error>> {
        Ok(IpcValidationContext {
            expected_window_label: "main".to_owned(),
            renderer_session_id: id("rs_test")?,
            installation_id: id("install_test")?,
            now: UnixMillis(10_000),
            allowed_commands: vec![
                "app.get_boot_state".to_owned(),
                "workspace.read_text".to_owned(),
                "bmad.scan".to_owned(),
            ],
        })
    }

    #[test]
    fn accepts_a_bound_catalog_command() -> Result<(), Box<dyn std::error::Error>> {
        let json = br#"{
          "schemaVersion":"desktop-ipc-command.v1",
          "requestId":"req_001",
          "command":"workspace.read_text",
          "windowLabel":"main",
          "rendererSessionId":"rs_test",
          "installationId":"install_test",
          "issuedAt":10000,
          "payload":{"workspaceId":"workspace_1","relativePath":"src/App.tsx","maxBytes":1000}
        }"#;
        let envelope = CommandEnvelopeValidator::parse(json, &context()?)?;
        assert_eq!(envelope.command.name(), "workspace.read_text");
        Ok(())
    }

    #[test]
    fn rejects_duplicate_keys_before_domain_parsing() -> Result<(), Box<dyn std::error::Error>> {
        let json = br#"{
          "schemaVersion":"desktop-ipc-command.v1",
          "requestId":"req_001",
          "requestId":"req_002",
          "command":"app.get_boot_state",
          "windowLabel":"main",
          "rendererSessionId":"rs_test",
          "installationId":"install_test",
          "issuedAt":10000,
          "payload":{}
        }"#;
        let error = CommandEnvelopeValidator::parse(json, &context()?).err();
        assert!(matches!(error, Some(IpcValidationError::InvalidJson)));
        Ok(())
    }

    #[test]
    fn rejects_prohibited_command_even_with_empty_payload() -> Result<(), Box<dyn std::error::Error>>
    {
        let json = br#"{
          "schemaVersion":"desktop-ipc-command.v1",
          "requestId":"req_001",
          "command":"run_shell",
          "windowLabel":"main",
          "rendererSessionId":"rs_test",
          "installationId":"install_test",
          "issuedAt":10000,
          "payload":{}
        }"#;
        let error = CommandEnvelopeValidator::parse(json, &context()?).err();
        assert!(matches!(error, Some(IpcValidationError::UnknownCommand)));
        Ok(())
    }

    #[test]
    fn rejects_every_later_phase_command_as_unknown_even_if_the_context_lists_it(
    ) -> Result<(), Box<dyn std::error::Error>> {
        for command in [
            "session.create",
            "task.submit",
            "task.cancel",
            "evidence.materialize",
            "evidence.export",
        ] {
            let mut malicious_context = context()?;
            malicious_context.allowed_commands.push(command.to_owned());
            let json = format!(
                r#"{{
                  "schemaVersion":"desktop-ipc-command.v1",
                  "requestId":"req_001",
                  "command":"{command}",
                  "windowLabel":"main",
                  "rendererSessionId":"rs_test",
                  "installationId":"install_test",
                  "issuedAt":10000,
                  "payload":{{}}
                }}"#,
            );
            let error = CommandEnvelopeValidator::parse(json.as_bytes(), &malicious_context).err();
            assert!(matches!(error, Some(IpcValidationError::UnknownCommand)));
        }
        Ok(())
    }

    #[test]
    fn preferences_and_about_commands_parse_strictly() -> Result<(), Box<dyn std::error::Error>> {
        let mut preferences_context = context()?;
        preferences_context.allowed_commands.extend([
            "app.preferences.get".to_owned(),
            "app.preferences.set".to_owned(),
            "app.about".to_owned(),
        ]);

        let set = br#"{
          "schemaVersion":"desktop-ipc-command.v1",
          "requestId":"req_001",
          "command":"app.preferences.set",
          "windowLabel":"main",
          "rendererSessionId":"rs_test",
          "installationId":"install_test",
          "issuedAt":10000,
          "payload":{"theme":"system","density":"compact"}
        }"#;
        let envelope = CommandEnvelopeValidator::parse(set, &preferences_context)?;
        assert!(matches!(
            envelope.command,
            LocalCommand::SetPreferences {
                theme: desktop_runtime::ThemePreference::System,
                density: desktop_runtime::DensityPreference::Compact,
            }
        ));

        let unknown_field = br#"{
          "schemaVersion":"desktop-ipc-command.v1",
          "requestId":"req_002",
          "command":"app.preferences.set",
          "windowLabel":"main",
          "rendererSessionId":"rs_test",
          "installationId":"install_test",
          "issuedAt":10000,
          "payload":{"theme":"dark","density":"compact","accent":"crimson"}
        }"#;
        assert!(matches!(
            CommandEnvelopeValidator::parse(unknown_field, &preferences_context).err(),
            Some(IpcValidationError::InvalidPayload)
        ));

        let invalid_theme = br#"{
          "schemaVersion":"desktop-ipc-command.v1",
          "requestId":"req_003",
          "command":"app.preferences.set",
          "windowLabel":"main",
          "rendererSessionId":"rs_test",
          "installationId":"install_test",
          "issuedAt":10000,
          "payload":{"theme":"crimson","density":"compact"}
        }"#;
        assert!(matches!(
            CommandEnvelopeValidator::parse(invalid_theme, &preferences_context).err(),
            Some(IpcValidationError::InvalidPayload)
        ));

        let about = br#"{
          "schemaVersion":"desktop-ipc-command.v1",
          "requestId":"req_004",
          "command":"app.about",
          "windowLabel":"main",
          "rendererSessionId":"rs_test",
          "installationId":"install_test",
          "issuedAt":10000,
          "payload":{}
        }"#;
        let envelope = CommandEnvelopeValidator::parse(about, &preferences_context)?;
        assert!(matches!(envelope.command, LocalCommand::GetAbout));
        Ok(())
    }

    #[test]
    fn rejects_a_d1_command_removed_by_the_recovery_projection(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let json = br#"{
          "schemaVersion":"desktop-ipc-command.v1",
          "requestId":"req_001",
          "command":"workspace.read_text",
          "windowLabel":"main",
          "rendererSessionId":"rs_test",
          "installationId":"install_test",
          "issuedAt":10000,
          "payload":{"workspaceId":"workspace_1","relativePath":"README.md","maxBytes":1000}
        }"#;
        let mut recovery = context()?;
        recovery.allowed_commands =
            vec!["app.get_boot_state".to_owned(), "workspace.list".to_owned()];
        let error = CommandEnvelopeValidator::parse(json, &recovery).err();
        assert!(matches!(
            error,
            Some(IpcValidationError::CapabilityUnavailable)
        ));
        Ok(())
    }

    #[test]
    fn accepts_read_only_bmad_scan_without_activation_fields(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let json = br#"{
          "schemaVersion":"desktop-ipc-command.v1",
          "requestId":"req_001",
          "command":"bmad.scan",
          "windowLabel":"main",
          "rendererSessionId":"rs_test",
          "installationId":"install_test",
          "issuedAt":10000,
          "payload":{"workspaceId":"workspace_1"}
        }"#;
        let envelope = CommandEnvelopeValidator::parse(json, &context()?)?;
        assert_eq!(envelope.command.name(), "bmad.scan");
        assert!(!envelope.command.is_mutating());
        Ok(())
    }

    #[test]
    fn rejects_renderer_session_mismatch() -> Result<(), Box<dyn std::error::Error>> {
        let json = br#"{
          "schemaVersion":"desktop-ipc-command.v1",
          "requestId":"req_001",
          "command":"app.get_boot_state",
          "windowLabel":"main",
          "rendererSessionId":"rs_attacker",
          "installationId":"install_test",
          "issuedAt":10000,
          "payload":{}
        }"#;
        let error = CommandEnvelopeValidator::parse(json, &context()?).err();
        assert!(matches!(error, Some(IpcValidationError::BindingMismatch)));
        Ok(())
    }

    fn recovery_context() -> Result<IpcValidationContext, Box<dyn std::error::Error>> {
        let mut value = context()?;
        value.allowed_commands.extend([
            "changes.recovery.prepare".to_owned(),
            "changes.recovery.decide".to_owned(),
        ]);
        Ok(value)
    }

    #[test]
    fn capability_parsers_bind_the_closed_closure_identifier(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let manifest_hash = format!("sha256:{}", "a".repeat(64));
        let prepare = serde_json::json!({
            "workspaceId": "workspace_01J00000000000000000000000",
            "workspaceGrantEpoch": 1,
            "capabilityId": "bmm:bmad-product-brief",
            "contextPaths": ["docs/brief-notes.md"],
        });
        assert!(matches!(
            super::parse_bmad_capability_prepare(prepare)?,
            LocalCommand::PrepareBmadCapabilityRun { capability_id, .. }
                if capability_id == "bmm:bmad-product-brief"
        ));
        let approve = serde_json::json!({
            "workspaceId": "workspace_01J00000000000000000000000",
            "workspaceGrantEpoch": 1,
            "capabilityId": "builder:agent.analyze",
            "manifestHash": manifest_hash,
        });
        assert!(matches!(
            super::parse_bmad_capability_approve(approve)?,
            LocalCommand::ApproveBmadCapabilityRun { capability_id, .. }
                if capability_id == "builder:agent.analyze"
        ));
        for bad_capability in [
            "",
            "bmm:",
            "bmm:AB",
            "shell:rm",
            "bmad-product-brief",
            "bmm:with space",
            "bmm:../escape",
        ] {
            let latest = serde_json::json!({
                "workspaceId": "workspace_01J00000000000000000000000",
                "workspaceGrantEpoch": 1,
                "capabilityId": bad_capability,
            });
            assert!(
                super::parse_bmad_capability_latest(latest).is_err(),
                "accepted forged capability id: {bad_capability}"
            );
        }
        Ok(())
    }

    #[test]
    fn capability_payloads_stay_closed_and_bounded() -> Result<(), Box<dyn std::error::Error>> {
        let manifest_hash = format!("sha256:{}", "a".repeat(64));
        // Unknown fields fail closed.
        let extra = serde_json::json!({
            "workspaceId": "workspace_01J00000000000000000000000",
            "workspaceGrantEpoch": 1,
            "capabilityId": "bmm:bmad-dev-story",
            "manifestHash": manifest_hash,
            "decisionId": "decision_01J00000000000000000000000",
            "approveAll": true,
        });
        assert!(super::parse_bmad_capability_submit(extra).is_err());
        // Absolute and escaping context paths cannot parse.
        for bad_path in ["C:/Windows/secret.txt", "../outside.txt"] {
            let prepare = serde_json::json!({
                "workspaceId": "workspace_01J00000000000000000000000",
                "workspaceGrantEpoch": 1,
                "capabilityId": "bmm:bmad-product-brief",
                "contextPaths": [bad_path],
            });
            assert!(
                super::parse_bmad_capability_prepare(prepare).is_err(),
                "accepted invalid context path: {bad_path}"
            );
        }
        // Empty context selections fail closed.
        let empty = serde_json::json!({
            "workspaceId": "workspace_01J00000000000000000000000",
            "workspaceGrantEpoch": 1,
            "capabilityId": "bmm:bmad-product-brief",
            "contextPaths": [],
        });
        assert!(super::parse_bmad_capability_prepare(empty).is_err());
        // The full envelope admits a reviewed capability command once the
        // validation context allows it, and still rejects empty payloads.
        let mut admitted = context()?;
        admitted
            .allowed_commands
            .push("bmad.capability.latest".to_owned());
        let envelope = r#"{
          "schemaVersion":"desktop-ipc-command.v1",
          "requestId":"req_capability",
          "command":"bmad.capability.latest",
          "windowLabel":"main",
          "rendererSessionId":"rs_test",
          "installationId":"install_test",
          "issuedAt":10000,
          "payload":{"workspaceId":"workspace_01J00000000000000000000000","workspaceGrantEpoch":1,"capabilityId":"bmm:bmad-product-brief"}
        }"#;
        assert!(matches!(
            CommandEnvelopeValidator::parse(envelope.as_bytes(), &admitted)?.command(),
            LocalCommand::LatestBmadCapabilityRun { .. }
        ));
        let empty = r#"{
          "schemaVersion":"desktop-ipc-command.v1",
          "requestId":"req_capability",
          "command":"bmad.capability.latest",
          "windowLabel":"main",
          "rendererSessionId":"rs_test",
          "installationId":"install_test",
          "issuedAt":10000,
          "payload":{}
        }"#;
        assert!(CommandEnvelopeValidator::parse(empty.as_bytes(), &admitted).is_err());
        Ok(())
    }

    fn offboarding_context() -> Result<IpcValidationContext, Box<dyn std::error::Error>> {
        let mut value = context()?;
        value
            .allowed_commands
            .push("app.offboarding.inspect".to_owned());
        value
            .allowed_commands
            .push("app.offboarding.erase".to_owned());
        Ok(value)
    }

    fn offboarding_envelope(command: &str, payload: &str) -> String {
        format!(
            r#"{{
              "schemaVersion":"desktop-ipc-command.v1",
              "requestId":"req_offboarding",
              "command":"{command}",
              "windowLabel":"main",
              "rendererSessionId":"rs_test",
              "installationId":"install_test",
              "issuedAt":10000,
              "payload":{payload}
            }}"#
        )
    }

    #[test]
    fn parses_offboarding_inspect_with_empty_payload_only() -> Result<(), Box<dyn std::error::Error>>
    {
        let valid = offboarding_envelope("app.offboarding.inspect", "{}");
        assert!(matches!(
            CommandEnvelopeValidator::parse(valid.as_bytes(), &offboarding_context()?)?.command(),
            LocalCommand::OffboardingInspect
        ));
        let extra = offboarding_envelope("app.offboarding.inspect", r#"{"extra":true}"#);
        assert!(
            CommandEnvelopeValidator::parse(extra.as_bytes(), &offboarding_context()?).is_err()
        );
        Ok(())
    }

    #[test]
    fn offboarding_erase_requires_the_exact_confirmation_phrase(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let valid = offboarding_envelope(
            "app.offboarding.erase",
            r#"{"confirm":"erase-local-authority-data"}"#,
        );
        assert!(matches!(
            CommandEnvelopeValidator::parse(valid.as_bytes(), &offboarding_context()?)?.command(),
            LocalCommand::OffboardingErase
        ));
        for bad in [
            "{}",
            r#"{"confirm":""}"#,
            r#"{"confirm":"erase"}"#,
            r#"{"confirm":"Erase-Local-Authority-Data"}"#,
            r#"{"confirm":"erase-local-authority-data "}"#,
            r#"{"confirm":"erase-local-authority-data","extra":true}"#,
            r#"{"confirm":true}"#,
        ] {
            let payload = offboarding_envelope("app.offboarding.erase", bad);
            assert!(
                CommandEnvelopeValidator::parse(payload.as_bytes(), &offboarding_context()?)
                    .is_err(),
                "accepted invalid erase payload: {bad}"
            );
        }
        Ok(())
    }

    fn persona_context() -> Result<IpcValidationContext, Box<dyn std::error::Error>> {
        let mut value = context()?;
        value.allowed_commands.push("bmad.persona.view".to_owned());
        Ok(value)
    }

    #[test]
    fn parses_only_the_closed_persona_view_payload_impl() -> Result<(), Box<dyn std::error::Error>>
    {
        let valid = br#"{
          "schemaVersion":"desktop-ipc-command.v1",
          "requestId":"req_persona",
          "command":"bmad.persona.view",
          "windowLabel":"main",
          "rendererSessionId":"rs_test",
          "installationId":"install_test",
          "issuedAt":10000,
          "payload":{"agentCode":"bmad-agent-analyst"}
        }"#;
        assert!(matches!(
            CommandEnvelopeValidator::parse(valid, &persona_context()?)?.command(),
            LocalCommand::ViewBmadPersona { agent_code } if agent_code == "bmad-agent-analyst"
        ));

        for bad_code in [
            "",
            "bmad-agent-",
            "not-an-agent",
            "bmad-agent-UPPER",
            "bmad-agent-with/slash",
            "bmad-agent-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ] {
            let payload = format!(
                r#"{{
                  "schemaVersion":"desktop-ipc-command.v1",
                  "requestId":"req_persona_bad",
                  "command":"bmad.persona.view",
                  "windowLabel":"main",
                  "rendererSessionId":"rs_test",
                  "installationId":"install_test",
                  "issuedAt":10000,
                  "payload":{{"agentCode":"{bad_code}"}}
                }}"#,
            );
            assert!(
                CommandEnvelopeValidator::parse(payload.as_bytes(), &persona_context()?).is_err(),
                "{bad_code:?} must be rejected",
            );
        }
        // Extra payload fields fail closed.
        let extra = br#"{
          "schemaVersion":"desktop-ipc-command.v1",
          "requestId":"req_persona_extra",
          "command":"bmad.persona.view",
          "windowLabel":"main",
          "rendererSessionId":"rs_test",
          "installationId":"install_test",
          "issuedAt":10000,
          "payload":{"agentCode":"bmad-agent-analyst","workspaceId":"workspace_1"}
        }"#;
        assert!(CommandEnvelopeValidator::parse(extra, &persona_context()?).is_err());
        Ok(())
    }

    #[test]
    fn parses_only_the_closed_recovery_command_payloads() -> Result<(), Box<dyn std::error::Error>>
    {
        let prepare = br#"{
          "schemaVersion":"desktop-ipc-command.v1",
          "requestId":"req_prepare",
          "command":"changes.recovery.prepare",
          "windowLabel":"main",
          "rendererSessionId":"rs_test",
          "installationId":"install_test",
          "issuedAt":10000,
          "payload":{"workspaceId":"workspace_1","workspaceGrantEpoch":7,"journalId":"journal_1"}
        }"#;
        assert!(matches!(
            CommandEnvelopeValidator::parse(prepare, &recovery_context()?)?.command(),
            LocalCommand::PrepareChangesRecovery {
                workspace_grant_epoch: 7,
                ..
            }
        ));

        let decide = format!(
            r#"{{
              "schemaVersion":"desktop-ipc-command.v1",
              "requestId":"req_decide",
              "command":"changes.recovery.decide",
              "windowLabel":"main",
              "rendererSessionId":"rs_test",
              "installationId":"install_test",
              "issuedAt":10000,
              "payload":{{"recoveryApprovalId":"recovery_approval_1","displayedRecoveryHash":"{}","choice":"restore"}}
            }}"#,
            sha256_bytes(b"displayed recovery")
        );
        assert!(matches!(
            CommandEnvelopeValidator::parse(decide.as_bytes(), &recovery_context()?)?.command(),
            LocalCommand::DecideChangesRecovery {
                choice: RecoveryApprovalChoice::Restore,
                ..
            }
        ));
        Ok(())
    }

    #[test]
    fn rejects_malformed_recovery_payloads() -> Result<(), Box<dyn std::error::Error>> {
        let digest = sha256_bytes(b"displayed recovery").to_string();
        let cases = [
            (
                "changes.recovery.prepare",
                r#"{"workspaceId":"workspace_1","workspaceGrantEpoch":0,"journalId":"journal_1"}"#.to_owned(),
            ),
            (
                "changes.recovery.prepare",
                r#"{"workspaceId":"workspace_1","workspaceGrantEpoch":9007199254740992,"journalId":"journal_1"}"#.to_owned(),
            ),
            (
                "changes.recovery.prepare",
                r#"{"workspaceId":"workspace_1","workspaceGrantEpoch":7,"journalId":"bad id"}"#.to_owned(),
            ),
            (
                "changes.recovery.prepare",
                r#"{"workspaceId":"workspace_1","workspaceGrantEpoch":7,"journalId":"journal_1","absolutePath":"C:\\\\private.txt"}"#.to_owned(),
            ),
            (
                "changes.recovery.prepare",
                r#"{"workspaceId":"workspace_1","workspaceGrantEpoch":7,"journalId":"journal_1","shellText":"restore private"}"#.to_owned(),
            ),
            (
                "changes.recovery.prepare",
                r#"{"workspaceId":"workspace_1","workspaceGrantEpoch":7,"journalId":"journal_1","provider":"remote"}"#.to_owned(),
            ),
            (
                "changes.recovery.decide",
                format!(r#"{{"recoveryApprovalId":"recovery_approval_1","displayedRecoveryHash":"{digest}","choice":"apply"}}"#),
            ),
            (
                "changes.recovery.decide",
                r#"{"recoveryApprovalId":"recovery_approval_1","displayedRecoveryHash":"not-a-hash","choice":"cancel"}"#.to_owned(),
            ),
            (
                "changes.recovery.decide",
                format!(r#"{{"recoveryApprovalId":"bad id","displayedRecoveryHash":"{digest}","choice":"cancel"}}"#),
            ),
            (
                "changes.recovery.decide",
                format!(r#"{{"recoveryApprovalId":"recovery_approval_1","displayedRecoveryHash":"{digest}","choice":"cancel","checkpointContent":"private"}}"#),
            ),
        ];
        for (index, (command, payload)) in cases.into_iter().enumerate() {
            let json = format!(
                r#"{{
                  "schemaVersion":"desktop-ipc-command.v1",
                  "requestId":"req_{index}",
                  "command":"{command}",
                  "windowLabel":"main",
                  "rendererSessionId":"rs_test",
                  "installationId":"install_test",
                  "issuedAt":10000,
                  "payload":{payload}
                }}"#
            );
            assert!(matches!(
                CommandEnvelopeValidator::parse(json.as_bytes(), &recovery_context()?).err(),
                Some(IpcValidationError::InvalidPayload)
            ));
        }
        Ok(())
    }

    #[test]
    fn rejects_recovery_binding_timestamp_and_capability_drift(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let valid_payload =
            r#"{"workspaceId":"workspace_1","workspaceGrantEpoch":7,"journalId":"journal_1"}"#;
        for (field, value) in [
            ("rendererSessionId", "rs_attacker"),
            ("installationId", "install_attacker"),
        ] {
            let mut envelope = serde_json::json!({
                "schemaVersion": "desktop-ipc-command.v1",
                "requestId": "req_binding",
                "command": "changes.recovery.prepare",
                "windowLabel": "main",
                "rendererSessionId": "rs_test",
                "installationId": "install_test",
                "issuedAt": 10_000,
                "payload": serde_json::from_str::<serde_json::Value>(valid_payload)?,
            });
            envelope[field] = serde_json::json!(value);
            assert!(matches!(
                CommandEnvelopeValidator::parse(
                    &serde_json::to_vec(&envelope)?,
                    &recovery_context()?
                )
                .err(),
                Some(IpcValidationError::BindingMismatch)
            ));
        }

        let stale = format!(
            r#"{{"schemaVersion":"desktop-ipc-command.v1","requestId":"req_stale","command":"changes.recovery.prepare","windowLabel":"main","rendererSessionId":"rs_test","installationId":"install_test","issuedAt":500000,"payload":{valid_payload}}}"#
        );
        assert!(matches!(
            CommandEnvelopeValidator::parse(stale.as_bytes(), &recovery_context()?).err(),
            Some(IpcValidationError::InvalidTimestamp)
        ));

        let mut unavailable = context()?;
        unavailable.allowed_commands =
            vec!["app.get_boot_state".to_owned(), "workspace.list".to_owned()];
        let json = format!(
            r#"{{"schemaVersion":"desktop-ipc-command.v1","requestId":"req_unavailable","command":"changes.recovery.prepare","windowLabel":"main","rendererSessionId":"rs_test","installationId":"install_test","issuedAt":10000,"payload":{valid_payload}}}"#
        );
        assert!(matches!(
            CommandEnvelopeValidator::parse(json.as_bytes(), &unavailable).err(),
            Some(IpcValidationError::CapabilityUnavailable)
        ));
        Ok(())
    }

    #[test]
    fn rejects_duplicate_recovery_payload_keys_before_parsing(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let json = br#"{
          "schemaVersion":"desktop-ipc-command.v1",
          "requestId":"req_duplicate",
          "command":"changes.recovery.prepare",
          "windowLabel":"main",
          "rendererSessionId":"rs_test",
          "installationId":"install_test",
          "issuedAt":10000,
          "payload":{"workspaceId":"workspace_1","workspaceGrantEpoch":7,"workspaceGrantEpoch":8,"journalId":"journal_1"}
        }"#;
        assert!(matches!(
            CommandEnvelopeValidator::parse(json, &recovery_context()?).err(),
            Some(IpcValidationError::InvalidJson)
        ));
        Ok(())
    }
}
