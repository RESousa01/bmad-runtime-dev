use desktop_ipc::{
    deserialize_strict, Admission, CommandEnvelopeValidator, IpcValidationContext,
    IpcValidationError,
};
use desktop_runtime::{
    canonical_hash, CommandReceipt, ContractId, LocalCommand, LocalError, LocalErrorCode,
    ProjectionEventKind,
};
use desktop_workspace::{EntryKind, WorkspaceError};
use serde::Serialize;
use tauri::WebviewWindow;
use tauri_plugin_dialog::DialogExt as _;
use ulid::Ulid;

use crate::state::{
    conflict_error, invalid_request, not_found_error, now, recovery_error, resource_limit_error,
    temporarily_unavailable, unauthorized_error, DirectoryCursor, HostState, RendererSessionGuard,
};
use crate::wire::{
    BootMode, BootStateProjection, BootstrapReply, ContextItemProjection, ContextPreviewProjection,
    HostCommandData, HostDispatchReply, ProjectionReply, ProjectionRequest, TreeEntryProjection,
    WorkspaceEntriesProjection, BOOTSTRAP_SCHEMA, PROJECTION_REQUEST_SCHEMA,
};

const MAX_CONTEXT_BYTES: u64 = 256 * 1024;
const MAX_CONTEXT_FILE_BYTES: u64 = 512 * 1024;
const READY_COMMANDS: [&str; 9] = [
    "app.get_boot_state",
    "workspace.select_folder",
    "workspace.list",
    "workspace.revoke",
    "workspace.list_entries",
    "workspace.read_text",
    "workspace.search",
    "bmad.scan",
    "context.preview",
];
const RECOVERY_COMMANDS: [&str; 2] = ["app.get_boot_state", "workspace.list"];

fn supported_commands(boot_mode: BootMode) -> Vec<String> {
    let commands: &[&str] = if boot_mode == BootMode::Ready {
        &READY_COMMANDS
    } else {
        &RECOVERY_COMMANDS
    };
    commands
        .iter()
        .map(|command| (*command).to_owned())
        .collect()
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri injects command arguments through owned boundary wrapper types"
)]
pub(crate) fn host_bootstrap(
    window: WebviewWindow,
    state: tauri::State<'_, HostState>,
) -> Result<BootstrapReply, String> {
    let renderer_session_id = state
        .bind_renderer(window.label())
        .map_err(|_| "The desktop host could not initialize a renderer session.".to_owned())?;
    let (boot_mode, workspaces, projection_sequence) = loop {
        let before = state.sequence();
        let boot_mode = state.boot_mode();
        let workspaces = state.workspace.list();
        let after = state.sequence();
        if before == after {
            break (boot_mode, workspaces, after);
        }
    };
    let supported_commands = supported_commands(boot_mode);

    Ok(BootstrapReply {
        schema_version: BOOTSTRAP_SCHEMA.to_owned(),
        renderer_session_id,
        installation_id: state.installation_id().clone(),
        window_label: window.label().to_owned(),
        boot_mode,
        supported_commands,
        workspaces,
        projection_sequence,
    })
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri injects command arguments through owned boundary wrapper types"
)]
pub(crate) fn host_dispatch(
    app: tauri::AppHandle,
    window: WebviewWindow,
    state: tauri::State<'_, HostState>,
    body: String,
) -> HostDispatchReply {
    let Some(renderer_authority) = state.renderer_session_authority(window.label()) else {
        return HostDispatchReply::error(None, state.sequence(), unauthorized_error());
    };
    let renderer_session_id = renderer_authority.session_id().clone();
    let accepted_at = now();
    let context = IpcValidationContext {
        expected_window_label: window.label().to_owned(),
        renderer_session_id,
        installation_id: state.installation_id().clone(),
        now: accepted_at,
        allowed_commands: supported_commands(state.boot_mode()),
    };
    let envelope = match CommandEnvelopeValidator::parse(body.as_bytes(), &context) {
        Ok(envelope) => envelope,
        Err(error) => {
            return HostDispatchReply::error(None, state.sequence(), map_ipc_error(&error));
        }
    };
    let request_id = envelope.request_id().clone();
    let admission = match state.gate.admit(&envelope, accepted_at) {
        Ok(admission) => admission,
        Err(error) => {
            let safe_error = map_ipc_error(&error).with_correlation_id(request_id.clone());
            return HostDispatchReply::error(Some(request_id), state.sequence(), safe_error);
        }
    };
    if admission == Admission::Replay {
        return state.cached_reply(&request_id).unwrap_or_else(|| {
            HostDispatchReply::error(
                Some(request_id.clone()),
                state.sequence(),
                conflict_error("The prior request receipt expired; refresh before retrying.")
                    .with_correlation_id(request_id),
            )
        });
    }

    let (_, command) = envelope.into_command();
    let is_mutating = command.is_mutating();
    let result = execute_command(
        &app,
        &state,
        &context.renderer_session_id,
        &request_id,
        command,
    );
    let receipt = CommandReceipt {
        request_id: request_id.clone(),
        accepted_at,
        operation_id: None,
    };
    let reply = match result {
        Ok(data) => HostDispatchReply::success(request_id.clone(), state.sequence(), receipt, data),
        Err(error) => HostDispatchReply::error(
            Some(request_id.clone()),
            state.sequence(),
            error.with_correlation_id(request_id.clone()),
        ),
    };
    if is_mutating {
        state.cache_reply(request_id, reply.clone());
    }
    drop(renderer_authority);
    reply
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri injects command arguments through owned boundary wrapper types"
)]
pub(crate) fn host_projection_snapshot(
    window: WebviewWindow,
    state: tauri::State<'_, HostState>,
    body: String,
) -> ProjectionReply {
    let request = match parse_projection_request(&body) {
        Ok(request) => request,
        Err(error) => return ProjectionReply::error(None, error),
    };
    if request.after_sequence.is_some() {
        return ProjectionReply::error(
            Some(request.renderer_session_id),
            invalid_request("A snapshot request cannot include an event cursor."),
        );
    }
    let renderer_authority = match validate_projection_binding(&window, &state, &request) {
        Ok(authority) => authority,
        Err(error) => return ProjectionReply::error(Some(request.renderer_session_id), error),
    };
    let reply = ProjectionReply::snapshot(request.renderer_session_id, state.snapshot());
    drop(renderer_authority);
    reply
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri injects command arguments through owned boundary wrapper types"
)]
pub(crate) fn host_projection_events(
    window: WebviewWindow,
    state: tauri::State<'_, HostState>,
    body: String,
) -> ProjectionReply {
    let request = match parse_projection_request(&body) {
        Ok(request) => request,
        Err(error) => return ProjectionReply::error(None, error),
    };
    let Some(after_sequence) = request.after_sequence else {
        return ProjectionReply::error(
            Some(request.renderer_session_id),
            invalid_request("An event request requires a projection cursor."),
        );
    };
    let renderer_authority = match validate_projection_binding(&window, &state, &request) {
        Ok(authority) => authority,
        Err(error) => return ProjectionReply::error(Some(request.renderer_session_id), error),
    };
    let reply = match state.events_after(after_sequence) {
        Ok(events) => ProjectionReply::events(request.renderer_session_id, events),
        Err(error) => ProjectionReply::error(Some(request.renderer_session_id), error),
    };
    drop(renderer_authority);
    reply
}

fn execute_command(
    app: &tauri::AppHandle,
    state: &HostState,
    renderer_session_id: &ContractId,
    request_id: &ContractId,
    command: LocalCommand,
) -> Result<HostCommandData, LocalError> {
    match command {
        LocalCommand::GetBootState => Ok(HostCommandData::BootState(boot_state(state))),
        LocalCommand::SelectWorkspace => select_workspace(app, state, request_id),
        LocalCommand::ListWorkspaces => Ok(HostCommandData::WorkspaceList(state.workspace.list())),
        LocalCommand::RevokeWorkspace { workspace_id } => {
            revoke_workspace(state, request_id, workspace_id)
        }
        LocalCommand::ListWorkspaceEntries {
            workspace_id,
            cursor,
            limit,
        } => list_workspace_entries(state, renderer_session_id, workspace_id, cursor, limit),
        LocalCommand::ReadWorkspaceText {
            workspace_id,
            relative_path,
            max_bytes,
        } => {
            let _authority = state.ready_authority()?;
            state
                .workspace
                .read_text(
                    workspace_id.as_str(),
                    relative_path.as_str(),
                    u64::from(max_bytes),
                )
                .map(HostCommandData::WorkspaceText)
                .map_err(|error| map_workspace_error(&error))
        }
        LocalCommand::SearchWorkspace {
            workspace_id,
            query,
            max_results,
        } => {
            let _authority = state.ready_authority()?;
            state
                .workspace
                .search(workspace_id.as_str(), &query, usize::from(max_results))
                .map(HostCommandData::SearchResults)
                .map_err(|error| map_workspace_error(&error))
        }
        LocalCommand::ScanBmad { workspace_id } => {
            let _authority = state.ready_authority()?;
            state
                .workspace
                .scan_bmad(workspace_id.as_str(), 256)
                .map(HostCommandData::BmadScan)
                .map_err(|error| map_workspace_error(&error))
        }
        LocalCommand::PreviewContext {
            workspace_id,
            relative_paths,
        } => {
            let _authority = state.ready_authority()?;
            preview_context(state, workspace_id, relative_paths)
        }
        LocalCommand::CreateSession { .. }
        | LocalCommand::SubmitTask { .. }
        | LocalCommand::CancelTask { .. } => Err(temporarily_unavailable(
            "Connected Agent sessions are not configured for this internal build.",
        )),
        LocalCommand::DecideApproval { .. } | LocalCommand::RequestRollback { .. } => {
            Err(temporarily_unavailable(
                "Governed local changes are unavailable until the authority chain is ready.",
            ))
        }
        LocalCommand::MaterializeEvidence { .. } | LocalCommand::ExportEvidence { .. } => Err(
            temporarily_unavailable("Evidence export is not available in this build."),
        ),
    }
}

fn select_workspace(
    app: &tauri::AppHandle,
    state: &HostState,
    request_id: &ContractId,
) -> Result<HostCommandData, LocalError> {
    if state.boot_mode() != BootMode::Ready {
        return Err(recovery_error());
    }
    let selected = app
        .dialog()
        .file()
        .set_title("Choose a local workspace")
        .blocking_pick_folder();
    let Some(selected) = selected else {
        return Ok(HostCommandData::NoSelection);
    };
    let selected_root = selected
        .into_path()
        .map_err(|_| invalid_request("The selected folder is not a local filesystem path."))?;
    let authority = state.ready_authority()?;
    let projection = state
        .workspace
        .grant(format!("project_{}", Ulid::new()), &selected_root)
        .map_err(|error| map_workspace_error(&error))?;
    let binding = match state.workspace.authority_binding(&projection.workspace_id) {
        Ok(binding) => binding,
        Err(error) => {
            let _ = state.workspace.revoke(&projection.workspace_id);
            drop(authority);
            state.enter_recovery();
            return Err(map_workspace_error(&error));
        }
    };
    if let Err(error) = state.persist_workspace(
        &authority,
        projection.clone(),
        &selected_root,
        &binding.root_identity_hash,
        request_id,
    ) {
        let _ = state.workspace.revoke(&projection.workspace_id);
        drop(authority);
        if error.code == LocalErrorCode::RecoveryRequired {
            state.enter_recovery();
        }
        return Err(error);
    }
    let Ok(workspace_id) = ContractId::new(projection.workspace_id.clone()) else {
        drop(authority);
        state.enter_recovery();
        return Err(recovery_error());
    };
    state.record_event(ProjectionEventKind::WorkspaceChanged { workspace_id });
    Ok(HostCommandData::WorkspaceSelected(projection))
}

fn revoke_workspace(
    state: &HostState,
    request_id: &ContractId,
    workspace_id: ContractId,
) -> Result<HostCommandData, LocalError> {
    let authority = state.ready_authority()?;
    if let Err(error) = state.persist_revocation(&authority, &workspace_id, request_id) {
        drop(authority);
        if error.code == LocalErrorCode::RecoveryRequired {
            state.enter_recovery();
        }
        return Err(error);
    }
    let Ok(projection) = state.workspace.revoke(workspace_id.as_str()) else {
        drop(authority);
        state.enter_recovery();
        return Err(recovery_error());
    };
    state.record_event(ProjectionEventKind::WorkspaceChanged { workspace_id });
    Ok(HostCommandData::WorkspaceRevoked(projection))
}

fn list_workspace_entries(
    state: &HostState,
    renderer_session_id: &ContractId,
    workspace_id: ContractId,
    cursor: Option<String>,
    limit: u16,
) -> Result<HostCommandData, LocalError> {
    let _authority = state.ready_authority()?;
    let binding = state
        .workspace
        .authority_binding(workspace_id.as_str())
        .map_err(|error| map_workspace_error(&error))?;
    let target = if let Some(cursor) = cursor {
        state.resolve_cursor(&cursor, renderer_session_id, &workspace_id)?
    } else {
        DirectoryCursor {
            renderer_session_id: renderer_session_id.clone(),
            workspace_id: workspace_id.clone(),
            grant_epoch: binding.grant_epoch,
            relative_directory: ".".to_owned(),
            after: None,
        }
    };
    let page = state
        .workspace
        .list_entries_page(
            workspace_id.as_str(),
            &target.relative_directory,
            target.after.as_deref(),
            usize::from(limit),
        )
        .map_err(|error| map_workspace_error(&error))?;
    let entries = page
        .entries
        .into_iter()
        .map(|entry| {
            let child_cursor = (entry.kind == EntryKind::Directory).then(|| {
                state.insert_cursor(DirectoryCursor {
                    renderer_session_id: renderer_session_id.clone(),
                    workspace_id: workspace_id.clone(),
                    grant_epoch: binding.grant_epoch,
                    relative_directory: entry.relative_path.clone(),
                    after: None,
                })
            });
            TreeEntryProjection {
                relative_path: entry.relative_path,
                kind: entry.kind,
                size_bytes: entry.size_bytes,
                child_cursor,
            }
        })
        .collect();
    let next_cursor = page.next_after.map(|after| {
        state.insert_cursor(DirectoryCursor {
            renderer_session_id: renderer_session_id.clone(),
            workspace_id: workspace_id.clone(),
            grant_epoch: binding.grant_epoch,
            relative_directory: target.relative_directory,
            after: Some(after),
        })
    });
    Ok(HostCommandData::WorkspaceEntries(
        WorkspaceEntriesProjection {
            workspace_id,
            entries,
            next_cursor,
        },
    ))
}

fn preview_context(
    state: &HostState,
    workspace_id: ContractId,
    relative_paths: Vec<desktop_runtime::RelativeWorkspacePath>,
) -> Result<HostCommandData, LocalError> {
    let mut items = Vec::with_capacity(relative_paths.len());
    let mut hash_items = Vec::with_capacity(relative_paths.len());
    let mut total_bytes = 0_u64;
    let mut estimated_tokens = 0_u64;
    for relative_path in relative_paths {
        let preview = state
            .workspace
            .read_text(
                workspace_id.as_str(),
                relative_path.as_str(),
                MAX_CONTEXT_FILE_BYTES,
            )
            .map_err(|error| map_workspace_error(&error))?;
        if preview.truncated {
            return Err(resource_limit_error(
                "A selected context file exceeds the per-file preview limit.",
            ));
        }
        let byte_count = u64::try_from(preview.content.len()).unwrap_or(u64::MAX);
        total_bytes = total_bytes.saturating_add(byte_count);
        if total_bytes > MAX_CONTEXT_BYTES {
            return Err(resource_limit_error(
                "The selected context exceeds the per-request byte limit.",
            ));
        }
        let item_tokens = byte_count.saturating_add(3) / 4;
        estimated_tokens = estimated_tokens.saturating_add(item_tokens);
        let end_line = u32::try_from(preview.content.lines().count().max(1)).unwrap_or(u32::MAX);
        hash_items.push(ContextManifestItem {
            relative_path: preview.relative_path.clone(),
            start_line: 1,
            end_line,
            reason: "Selected for this task".to_owned(),
            content_hash: preview.content_hash.clone(),
            classification: "source".to_owned(),
            redactions: Vec::new(),
            byte_count,
            estimated_tokens: item_tokens,
        });
        items.push(ContextItemProjection {
            relative_path: preview.relative_path,
            start_line: 1,
            end_line,
            reason: "Selected for this task".to_owned(),
            content_hash: preview.content_hash,
            classification: "source".to_owned(),
            redactions: Vec::new(),
            byte_count,
            estimated_tokens: item_tokens,
            content: preview.content,
        });
    }
    let manifest_hash = canonical_hash(
        "context-manifest",
        1,
        &ContextManifestHashInput {
            workspace_id: workspace_id.as_str(),
            items: &hash_items,
        },
    )
    .map_err(|_| recovery_error())?
    .to_string();
    Ok(HostCommandData::ContextPreview(ContextPreviewProjection {
        workspace_id,
        manifest_hash,
        items,
        total_bytes,
        estimated_tokens,
        model_target: None,
    }))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ContextManifestHashInput<'a> {
    workspace_id: &'a str,
    items: &'a [ContextManifestItem],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ContextManifestItem {
    relative_path: String,
    start_line: u32,
    end_line: u32,
    reason: String,
    content_hash: String,
    classification: String,
    redactions: Vec<String>,
    byte_count: u64,
    estimated_tokens: u64,
}

fn boot_state(state: &HostState) -> BootStateProjection {
    let mode = state.boot_mode();
    BootStateProjection {
        mode,
        workspace_count: u32::try_from(state.workspace.list().len()).unwrap_or(u32::MAX),
        connected_features_available: false,
        local_edits_available: false,
        recovery_message: (mode == BootMode::ReadOnlyRecovery).then(|| {
            "Local authority storage could not be verified. File changes remain blocked.".to_owned()
        }),
    }
}

fn parse_projection_request(body: &str) -> Result<ProjectionRequest, LocalError> {
    let request: ProjectionRequest =
        deserialize_strict(body.as_bytes()).map_err(|error| map_ipc_error(&error))?;
    if request.schema_version != PROJECTION_REQUEST_SCHEMA {
        return Err(invalid_request(
            "The projection request schema is unsupported.",
        ));
    }
    Ok(request)
}

fn validate_projection_binding<'a>(
    window: &WebviewWindow,
    state: &'a HostState,
    request: &ProjectionRequest,
) -> Result<RendererSessionGuard<'a>, LocalError> {
    let authority = state
        .renderer_session_authority(window.label())
        .ok_or_else(unauthorized_error)?;
    if authority.session_id() != &request.renderer_session_id
        || state.installation_id() != &request.installation_id
    {
        return Err(unauthorized_error());
    }
    if let Some(workspace_id) = &request.workspace_id {
        state
            .workspace
            .authority_binding(workspace_id.as_str())
            .map_err(|_| not_found_error("The requested local workspace is not available."))?;
    }
    if request.session_id.is_some() {
        return Err(not_found_error(
            "The requested Agent session is not available.",
        ));
    }
    Ok(authority)
}

fn map_ipc_error(error: &IpcValidationError) -> LocalError {
    match error {
        IpcValidationError::BindingMismatch => unauthorized_error(),
        IpcValidationError::RateLimited => LocalError::new(
            LocalErrorCode::ResourceLimit,
            "The renderer sent too many requests. Wait briefly and retry.",
            true,
        ),
        IpcValidationError::IdempotencyConflict => {
            conflict_error("The request identifier was already used for different content.")
        }
        IpcValidationError::AdmissionUnavailable => {
            temporarily_unavailable("Request admission is temporarily unavailable.")
        }
        IpcValidationError::CapabilityUnavailable => conflict_error(
            "Desktop capabilities changed; refresh the current view before retrying.",
        ),
        IpcValidationError::EnvelopeTooLarge
        | IpcValidationError::InvalidJson
        | IpcValidationError::StructuralLimit
        | IpcValidationError::UnsupportedSchema
        | IpcValidationError::UnknownCommand
        | IpcValidationError::InvalidPayload
        | IpcValidationError::InvalidTimestamp => {
            invalid_request("The renderer request is invalid or unsupported.")
        }
    }
}

fn map_workspace_error(error: &WorkspaceError) -> LocalError {
    match error {
        WorkspaceError::UnsupportedRoot => invalid_request(
            "Choose a fixed local NTFS folder without reparse points or cloud placeholders.",
        ),
        WorkspaceError::InvalidRelativePath
        | WorkspaceError::OutsideWorkspace
        | WorkspaceError::PathBlocked
        | WorkspaceError::UnsupportedText => {
            invalid_request("The requested workspace item is not available for text review.")
        }
        WorkspaceError::GrantUnavailable => {
            not_found_error("The local workspace is not available.")
        }
        WorkspaceError::RootIdentityChanged => {
            conflict_error("The local workspace identity changed; select the folder again.")
        }
        WorkspaceError::LimitExceeded => {
            resource_limit_error("The workspace operation exceeded its configured limit.")
        }
        WorkspaceError::Io(_) => temporarily_unavailable(
            "The local workspace could not be read. Check access and retry.",
        ),
    }
}
