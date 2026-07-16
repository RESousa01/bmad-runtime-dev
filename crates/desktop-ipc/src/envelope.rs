use desktop_runtime::{
    deserialize_strict_json, ApprovalChoice, BmadHelpIntent, CommandReceipt, ContractId,
    LocalCommand, LocalError, ProjectionEvent, ProposedFileChange, RelativeWorkspacePath,
    Sha256Digest, UnixMillis, HARD_MAX_CHANGED_FILES,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::bmad::{valid_bmad_cursor, BmadLibrarySnapshotPayload};

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
            | "bmad.help.latest"
            | "run.create"
            | "context.preview"
            | "workspace.enable_edits"
            | "changes.propose"
            | "approval.decide"
            | "rollback.request"
            | "changes.history"
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
struct WorkspaceEpochPayload {
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProposeChangesPayload {
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    changes: Vec<ProposedFileChange>,
}

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
        "bmad.help.latest" => parse_bmad_help_latest(payload),
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
        _ => parse_later_phase_command(command, payload),
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
    if input.workspace_grant_epoch == 0 || input.workspace_grant_epoch > MAX_SAFE_JSON_INTEGER {
        return Err(IpcValidationError::InvalidPayload);
    }
    Ok(LocalCommand::LatestBmadHelpRun {
        workspace_id: input.workspace_id,
        workspace_grant_epoch: input.workspace_grant_epoch,
    })
}

fn parse_bmad_run_create(payload: Value) -> Result<LocalCommand, IpcValidationError> {
    let input: CreateBmadRunPayload = parse_payload(payload)?;
    if input.workspace_grant_epoch == 0 || input.workspace_grant_epoch > MAX_SAFE_JSON_INTEGER {
        return Err(IpcValidationError::InvalidPayload);
    }
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
    use desktop_runtime::{ContractId, UnixMillis};

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
}
