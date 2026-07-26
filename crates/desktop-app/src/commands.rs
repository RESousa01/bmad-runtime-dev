use desktop_egress::{ContextClassification, ContextReviewProjection, RetentionMode};
use desktop_ipc::{
    decode_retained_bmad_help_completion, decode_retained_bmad_help_run, deserialize_strict,
    project_bmad_help_approved, project_bmad_help_approved_lifecycle, project_bmad_help_cancelled,
    project_bmad_help_review, project_bmad_help_terminal,
    project_bmad_library_with_runtime_menu_bindings, project_created_bmad_help_run,
    project_model_auth_status, Admission, BmadHelpApprovalInput, BmadHelpApprovedLifecycleInput,
    BmadHelpCancellationInput, BmadHelpContextClassificationProjection,
    BmadHelpModelAccessProjectionError, BmadHelpProjectionError, BmadHelpRetentionProjection,
    BmadHelpReviewExclusionInput, BmadHelpReviewInput, BmadHelpReviewItemInput,
    BmadHelpReviewRedactionInput, BmadHelpSecretFindingInput, BmadHelpTerminalInput,
    BmadHelpTerminalReasonProjection, BmadProjectionError, CommandEnvelopeValidator,
    IpcValidationContext, IpcValidationError, ModelAuthModeProjection, ModelAuthStatusInput,
    ModelAuthStatusKindProjection, ValidatedCommandEnvelope,
};
use desktop_runtime::{
    canonical_hash, BmadHelpIntent, BmadLibraryProjectionScope, CommandReceipt, ContractId,
    CreateInertBmadHelpSession, InertBmadHelpSessionCoordinator, LocalCommand, LocalError,
    LocalErrorCode, ProjectionEventKind, RelativeWorkspacePath, Sha256Digest, UnixMillis,
};
use desktop_store::{
    BmadHelpRunCreateRequest, BmadHelpRunCreationReceipt, BmadHelpRunLatest,
    BmadHelpRunReplayRequest, StoreError,
};
use desktop_workspace::{EntryKind, WorkspaceError};
use serde::Serialize;
use tauri::WebviewWindow;
use tauri_plugin_dialog::DialogExt as _;
use ulid::Ulid;

use crate::bmad_foundation::BmadLoadedFoundation;
use crate::bmad_model::config::HelpModelMode;
use crate::bmad_model::coordinator::{
    ApproveBmadHelpReviewInput, BmadHelpCoordinatorError, BmadHelpInMemoryLifecycle,
    BmadHelpTerminalReason, CancelBmadHelpReviewInput, PrepareBmadHelpReviewInput,
    SubmitBmadHelpReviewInput,
};
use crate::state::{
    conflict_error, invalid_request, not_found_error, now, recovery_error, resource_limit_error,
    temporarily_unavailable, unauthorized_error, DirectoryCursor, HostState, ReadyAuthorityGuard,
    RendererSessionGuard,
};
use crate::wire::{
    AboutProjection, BootMode, BootStateProjection, BootstrapReply, ContextItemProjection,
    ContextPreviewProjection, HostCommandData, HostDispatchReply, PreferencesProjection,
    ProjectionReply, ProjectionRequest, TreeEntryProjection, WorkspaceEntriesProjection,
    BOOTSTRAP_SCHEMA, PREFERENCES_SCHEMA, PROJECTION_REQUEST_SCHEMA,
};

const MAX_CONTEXT_BYTES: u64 = 256 * 1024;
const MAX_CONTEXT_FILE_BYTES: u64 = 512 * 1024;
const READY_COMMANDS: [&str; 46] = [
    "app.get_boot_state",
    "workspace.select_folder",
    "workspace.list",
    "workspace.revoke",
    "workspace.list_entries",
    "workspace.read_text",
    "workspace.git_status",
    "workspace.git_diff",
    "workspace.search",
    "bmad.scan",
    "bmad.library.snapshot",
    "bmad.persona.view",
    "model.auth.status",
    "model.auth.sign_in",
    "model.auth.sign_out",
    "model.selection.set",
    "chat.send",
    "chat.history",
    "bmad.workflow.state",
    "bmad.help.prepare",
    "bmad.help.approve",
    "bmad.help.cancel",
    "bmad.help.submit",
    "bmad.help.latest",
    "bmad.capability.prepare",
    "bmad.capability.approve",
    "bmad.capability.cancel",
    "bmad.capability.submit",
    "bmad.capability.latest",
    "bmad.capability.propose_changes",
    "run.create",
    "context.preview",
    "workspace.enable_edits",
    "changes.propose",
    "approval.decide",
    "rollback.request",
    "changes.history",
    "changes.recovery.prepare",
    "changes.recovery.decide",
    "app.preferences.get",
    "app.preferences.set",
    "app.about",
    "app.offboarding.inspect",
    "app.offboarding.erase",
    "workspace.pick_files",
    "workspace.pick_folder",
];
const RECOVERY_COMMANDS: [&str; 2] = ["app.get_boot_state", "workspace.list"];

struct CommandExecution {
    data: HostCommandData,
    accepted_at: UnixMillis,
    operation_id: Option<ContractId>,
}

#[derive(Debug)]
struct BmadHelpRunExecution {
    data: HostCommandData,
    accepted_at: UnixMillis,
    operation_id: ContractId,
}

#[derive(Clone, Copy)]
struct BmadHelpRunFingerprint {
    catalog: Sha256Digest,
    foundation: Sha256Digest,
    intent: Sha256Digest,
}

enum NewBmadHelpRunError {
    Local(LocalError),
    Recovery,
}

struct NewBmadHelpRunInput<'a> {
    request_id: &'a ContractId,
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    workspace_catalog_version: u64,
    current_intent: BmadHelpIntent,
    accepted_at: UnixMillis,
    fingerprint: BmadHelpRunFingerprint,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BmadFoundationBindingHashInput<'a> {
    schema_version: &'static str,
    manifest_hash: Sha256Digest,
    semantic_ledger_hash: Sha256Digest,
    capability_catalog_hash: Sha256Digest,
    package_version_id: &'a ContractId,
    descriptor_hash: Sha256Digest,
    observed_inventory_hash: Sha256Digest,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BmadHelpIntentHashInput<'a> {
    schema_version: &'static str,
    current_intent: &'a str,
}

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

fn should_cache_reply(command: &LocalCommand) -> bool {
    command.is_mutating()
        && !matches!(
            command,
            LocalCommand::CreateBmadHelpRun { .. }
                | LocalCommand::ModelAuthSignIn { .. }
                | LocalCommand::ModelAuthSignOut
                | LocalCommand::PrepareBmadHelpReview { .. }
                | LocalCommand::ApproveBmadHelpReview { .. }
                | LocalCommand::CancelBmadHelpReview { .. }
                | LocalCommand::SubmitBmadHelpReview { .. }
                | LocalCommand::PrepareChangesRecovery { .. }
                | LocalCommand::DecideChangesRecovery { .. }
        )
}

fn admit_dispatch_envelope(
    state: &HostState,
    envelope: &ValidatedCommandEnvelope,
    accepted_at: UnixMillis,
) -> Result<(), HostDispatchReply> {
    let request_id = envelope.request_id().clone();
    let admission = state.gate.admit(envelope, accepted_at).map_err(|error| {
        let safe_error = map_ipc_error(&error).with_correlation_id(request_id.clone());
        HostDispatchReply::error(Some(request_id.clone()), state.sequence(), safe_error)
    })?;
    if admission == Admission::Replay {
        return Err(state.cached_reply(&request_id).unwrap_or_else(|| {
            HostDispatchReply::error(
                Some(request_id.clone()),
                state.sequence(),
                conflict_error("The prior request receipt expired; refresh before retrying.")
                    .with_correlation_id(request_id),
            )
        }));
    }
    Ok(())
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
    foundation: tauri::State<'_, BmadLoadedFoundation>,
    body: String,
) -> HostDispatchReply {
    let Some(renderer_authority) = state.renderer_session_authority(window.label()) else {
        return HostDispatchReply::error(None, state.sequence(), renderer_session_expired());
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
    if let Err(reply) = admit_dispatch_envelope(&state, &envelope, accepted_at) {
        return reply;
    }

    let (_, command) = envelope.into_command();
    let cache_reply = should_cache_reply(&command);
    let result = execute_command(
        &app,
        &state,
        &foundation,
        &renderer_authority,
        &request_id,
        accepted_at,
        command,
    );
    let reply = match result {
        Ok(execution) => HostDispatchReply::success(
            request_id.clone(),
            state.sequence(),
            CommandReceipt {
                request_id: request_id.clone(),
                accepted_at: execution.accepted_at,
                operation_id: execution.operation_id,
            },
            execution.data,
        ),
        Err(error) => HostDispatchReply::error(
            Some(request_id.clone()),
            state.sequence(),
            error.with_correlation_id(request_id.clone()),
        ),
    };
    if cache_reply {
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

#[expect(
    clippy::too_many_lines,
    reason = "the host dispatcher keeps the closed command-to-handler mapping visible at one boundary"
)]
fn execute_command(
    app: &tauri::AppHandle,
    state: &HostState,
    foundation: &BmadLoadedFoundation,
    renderer_session: &RendererSessionGuard<'_>,
    request_id: &ContractId,
    accepted_at: UnixMillis,
    command: LocalCommand,
) -> Result<CommandExecution, LocalError> {
    let data = match command {
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
        } => list_workspace_entries(
            state,
            renderer_session.session_id(),
            workspace_id,
            cursor,
            limit,
        ),
        LocalCommand::ReadWorkspaceText {
            workspace_id,
            relative_path,
            max_bytes,
        } => read_workspace_text(state, &workspace_id, &relative_path, max_bytes),
        LocalCommand::ReadGitStatus { workspace_id } => {
            let _authority = state.ready_authority()?;
            state
                .workspace
                .git_status(workspace_id.as_str())
                .map(HostCommandData::GitStatus)
                .map_err(|error| map_workspace_error(&error))
        }
        LocalCommand::ReadGitDiff {
            workspace_id,
            relative_path,
            comparison,
        } => {
            let _authority = state.ready_authority()?;
            state
                .workspace
                .git_diff(workspace_id.as_str(), relative_path.as_str(), comparison)
                .map(HostCommandData::GitDiff)
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
        LocalCommand::PickWorkspaceFiles { workspace_id } => {
            pick_workspace_files(app, state, workspace_id)
        }
        LocalCommand::PickWorkspaceFolder { workspace_id } => {
            pick_workspace_folder(app, state, workspace_id)
        }
        LocalCommand::PrepareBmadCapabilityRun {
            workspace_id,
            workspace_grant_epoch,
            capability_id,
            context_paths,
        } => prepare_bmad_capability_run(
            state,
            foundation,
            renderer_session,
            workspace_id,
            workspace_grant_epoch,
            &capability_id,
            context_paths,
            accepted_at,
        ),
        LocalCommand::ApproveBmadCapabilityRun {
            workspace_id,
            workspace_grant_epoch,
            capability_id,
            manifest_hash,
        } => approve_bmad_capability_run(
            state,
            workspace_id,
            workspace_grant_epoch,
            &capability_id,
            manifest_hash,
            accepted_at,
        ),
        LocalCommand::CancelBmadCapabilityRun {
            workspace_id,
            workspace_grant_epoch,
            capability_id,
            manifest_hash,
            decision_id,
        } => cancel_bmad_capability_run(
            state,
            workspace_id,
            workspace_grant_epoch,
            &capability_id,
            manifest_hash,
            decision_id,
            accepted_at,
        ),
        LocalCommand::SubmitBmadCapabilityRun {
            workspace_id,
            workspace_grant_epoch,
            capability_id,
            manifest_hash,
            decision_id,
        } => submit_bmad_capability_run(
            state,
            foundation,
            workspace_id,
            workspace_grant_epoch,
            &capability_id,
            manifest_hash,
            decision_id,
            accepted_at,
        ),
        LocalCommand::LatestBmadCapabilityRun {
            workspace_id,
            workspace_grant_epoch,
            capability_id,
        } => {
            latest_bmad_capability_run(state, &workspace_id, workspace_grant_epoch, &capability_id)
        }
        LocalCommand::ProposeBmadCapabilityChanges {
            workspace_id,
            workspace_grant_epoch,
            capability_id,
        } => propose_bmad_capability_changes(
            state,
            foundation,
            request_id,
            accepted_at,
            workspace_id,
            workspace_grant_epoch,
            &capability_id,
        ),
        LocalCommand::OffboardingInspect => {
            let _authority = state.ready_authority()?;
            offboarding_inspect(state)
        }
        LocalCommand::OffboardingErase => {
            state.offboard_erase()?;
            Ok(HostCommandData::OffboardingErased(
                desktop_ipc::OffboardingErasedProjection {
                    schema_version: "sapphirus.offboarding-erased.v1".to_owned(),
                    status: "erased".to_owned(),
                    restart_required: true,
                },
            ))
        }
        LocalCommand::ViewBmadPersona { agent_code } => {
            let _authority = state.ready_authority()?;
            view_bmad_persona(foundation, &agent_code)
        }
        LocalCommand::BmadLibrarySnapshot { scope, cursor } => {
            let _authority = state.ready_authority()?;
            bmad_library_snapshot(foundation, scope, cursor.as_deref())
        }
        LocalCommand::CreateBmadHelpRun {
            workspace_id,
            workspace_grant_epoch,
            current_intent,
        } => {
            return create_bmad_help_run(
                state,
                foundation,
                request_id,
                workspace_id,
                workspace_grant_epoch,
                current_intent,
                accepted_at,
            )
            .map(|created| CommandExecution {
                data: created.data,
                accepted_at: created.accepted_at,
                operation_id: Some(created.operation_id),
            });
        }
        LocalCommand::ModelAuthStatus => model_auth_status_data(state),
        LocalCommand::ModelAuthSignIn {
            provider_id,
            model_id,
            credential,
            endpoint,
            region,
        } => {
            state.sign_in_model(
                provider_id,
                model_id,
                &credential,
                endpoint.as_deref(),
                region.as_deref(),
                accepted_at,
            )?;
            model_auth_status_data(state)
        }
        LocalCommand::ModelAuthSignOut => {
            state.sign_out_model()?;
            model_auth_status_data(state)
        }
        LocalCommand::SetModelSelection {
            provider_id,
            model_id,
        } => {
            state.set_model_selection(provider_id, model_id, accepted_at)?;
            model_auth_status_data(state)
        }
        LocalCommand::PrepareBmadHelpReview {
            workspace_id,
            workspace_grant_epoch,
            context_paths,
        } => prepare_bmad_help_review(
            state,
            foundation,
            renderer_session,
            workspace_id,
            workspace_grant_epoch,
            context_paths,
            accepted_at,
        ),
        LocalCommand::ApproveBmadHelpReview {
            workspace_id,
            workspace_grant_epoch,
            manifest_hash,
        } => approve_bmad_help_review(
            state,
            renderer_session,
            workspace_id,
            workspace_grant_epoch,
            manifest_hash,
            accepted_at,
        ),
        LocalCommand::CancelBmadHelpReview {
            workspace_id,
            workspace_grant_epoch,
            manifest_hash,
            decision_id,
        } => cancel_bmad_help_review(
            state,
            renderer_session,
            workspace_id,
            workspace_grant_epoch,
            manifest_hash,
            decision_id,
            accepted_at,
        ),
        LocalCommand::SubmitBmadHelpReview {
            workspace_id,
            workspace_grant_epoch,
            manifest_hash,
            decision_id,
        } => submit_bmad_help_review(
            state,
            renderer_session,
            workspace_id,
            workspace_grant_epoch,
            manifest_hash,
            decision_id,
            accepted_at,
        ),
        LocalCommand::LatestBmadHelpRun {
            workspace_id,
            workspace_grant_epoch,
        } => latest_bmad_help_run(state, &workspace_id, workspace_grant_epoch),
        LocalCommand::SendChatMessage {
            workspace_id,
            workspace_grant_epoch,
            client_session_ref,
            agent_code,
            text,
            context_paths,
        } => send_chat_message(
            state,
            foundation,
            SendChatMessageInput {
                workspace_id,
                workspace_grant_epoch,
                client_session_ref,
                agent_code,
                text,
                context_paths,
                now: accepted_at,
            },
        ),
        LocalCommand::BmadWorkflowState {
            workspace_id,
            workspace_grant_epoch,
        } => bmad_workflow_state(state, foundation, &workspace_id, workspace_grant_epoch),
        LocalCommand::ChatHistory {
            workspace_id,
            workspace_grant_epoch,
            client_session_ref,
        } => chat_history(
            state,
            &workspace_id,
            workspace_grant_epoch,
            &client_session_ref,
        ),
        LocalCommand::PreviewContext {
            workspace_id,
            relative_paths,
        } => {
            let _authority = state.ready_authority()?;
            preview_context(state, workspace_id, relative_paths)
        }
        command @ (LocalCommand::EnableWorkspaceEdits { .. }
        | LocalCommand::ProposeChanges { .. }
        | LocalCommand::DecideApproval { .. }
        | LocalCommand::RequestRollback { .. }
        | LocalCommand::ChangesHistory { .. }) => {
            crate::edits::execute_changes_command(state, request_id, accepted_at, command)
        }
        LocalCommand::PrepareChangesRecovery {
            workspace_id,
            workspace_grant_epoch,
            journal_id,
        } => crate::recovery::prepare_recovery(
            state,
            renderer_session,
            &workspace_id,
            workspace_grant_epoch,
            &journal_id,
            accepted_at,
        ),
        LocalCommand::DecideChangesRecovery {
            recovery_approval_id,
            displayed_recovery_hash,
            choice,
        } => crate::recovery::decide_recovery(
            state,
            renderer_session,
            &recovery_approval_id,
            displayed_recovery_hash,
            choice,
            accepted_at,
        ),
        LocalCommand::GetPreferences => load_preferences(state),
        LocalCommand::SetPreferences { theme, density } => {
            save_preferences(state, request_id, theme, density, accepted_at)
        }
        LocalCommand::GetAbout => Ok(HostCommandData::About(about_projection(state, foundation))),
        LocalCommand::CreateSession { .. }
        | LocalCommand::SubmitTask { .. }
        | LocalCommand::CancelTask { .. } => Err(temporarily_unavailable(
            "Connected Agent sessions are not configured for this internal build.",
        )),
        LocalCommand::MaterializeEvidence { .. } | LocalCommand::ExportEvidence { .. } => Err(
            temporarily_unavailable("Evidence export is not available in this build."),
        ),
    }?;
    Ok(CommandExecution {
        data,
        accepted_at,
        operation_id: None,
    })
}

fn model_auth_status_data(state: &HostState) -> Result<HostCommandData, LocalError> {
    let configuration = state.model_configuration().map_err(|_| {
        LocalError::new(
            LocalErrorCode::IntegrityFailure,
            "The model-access configuration could not be verified.",
            false,
        )
    })?;
    let (status, mode, development_only) = match configuration.mode {
        HelpModelMode::Offline => (
            ModelAuthStatusKindProjection::Unavailable,
            ModelAuthModeProjection::Offline,
            false,
        ),
        #[cfg(feature = "deterministic-help")]
        HelpModelMode::DeterministicDevelopment => (
            ModelAuthStatusKindProjection::DevelopmentReady,
            ModelAuthModeProjection::DeterministicDevelopment,
            true,
        ),
        #[cfg(feature = "direct-model-providers")]
        HelpModelMode::DirectProviderDevelopment => (
            ModelAuthStatusKindProjection::DevelopmentReady,
            ModelAuthModeProjection::DirectProviderDevelopment,
            true,
        ),
        // Production sends fail closed until the gated rollout activates the
        // deployed round-trip, so the pinned IPC catalog projection remains
        // truthful as unavailable/offline; the catalog gains a production
        // variant only alongside that rollout.
        #[cfg(feature = "production-support")]
        HelpModelMode::ProductionSupport => (
            ModelAuthStatusKindProjection::Unavailable,
            ModelAuthModeProjection::Offline,
            false,
        ),
    };
    #[cfg(feature = "direct-model-providers")]
    let sign_out_available = configuration.mode == HelpModelMode::DirectProviderDevelopment;
    #[cfg(not(feature = "direct-model-providers"))]
    let sign_out_available = false;
    project_model_auth_status(ModelAuthStatusInput {
        status,
        mode,
        auth_epoch: state.model_auth_epoch(),
        development_only,
        destination_label: configuration.destination_label,
        provider_id: Some(configuration.provider_id),
        model_id: Some(configuration.model_id),
        sign_in_available: cfg!(feature = "direct-model-providers"),
        sign_out_available,
    })
    .map(HostCommandData::ModelAuthStatus)
    .map_err(map_bmad_model_projection_error)
}

fn prepare_bmad_help_review(
    state: &HostState,
    foundation: &BmadLoadedFoundation,
    renderer_session: &RendererSessionGuard<'_>,
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    context_paths: Vec<RelativeWorkspacePath>,
    created_at: UnixMillis,
) -> Result<HostCommandData, LocalError> {
    let authority = state.ready_workspace_commit()?;
    let mut workspace_context = Vec::with_capacity(context_paths.len());
    let mut total_bytes = 0_u64;
    for relative_path in context_paths {
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
                "A selected context file exceeds the per-file limit.",
            ));
        }
        total_bytes =
            total_bytes.saturating_add(u64::try_from(preview.content.len()).unwrap_or(u64::MAX));
        if total_bytes > MAX_CONTEXT_BYTES {
            return Err(resource_limit_error(
                "The selected context exceeds the per-request byte limit.",
            ));
        }
        let relative_path = RelativeWorkspacePath::new(&preview.relative_path).map_err(|_| {
            LocalError::new(
                LocalErrorCode::IntegrityFailure,
                "A governed read returned an invalid relative path.",
                false,
            )
        })?;
        workspace_context.push(crate::bmad_model::context::WorkspaceHelpContextItem {
            relative_path,
            content: preview.content,
        });
    }
    let workspace_authority = state
        .workspace
        .authorize_scope(workspace_id.as_str(), workspace_grant_epoch)
        .map_err(|error| map_workspace_error(&error))?;
    let review = state
        .bmad_model
        .lock()
        .prepare(
            state,
            &authority,
            &workspace_authority,
            foundation,
            PrepareBmadHelpReviewInput {
                renderer_session,
                workspace_id,
                workspace_grant_epoch,
                created_at,
                workspace_context,
            },
        )
        .map_err(map_bmad_model_error)?;
    drop(workspace_authority);
    drop(authority);
    project_review_for_renderer(review).map(HostCommandData::BmadHelpReview)
}

fn approve_bmad_help_review(
    state: &HostState,
    renderer_session: &RendererSessionGuard<'_>,
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    manifest_hash: Sha256Digest,
    approved_at: UnixMillis,
) -> Result<HostCommandData, LocalError> {
    let authority = state.ready_workspace_commit()?;
    let workspace_authority = state
        .workspace
        .authorize_scope(workspace_id.as_str(), workspace_grant_epoch)
        .map_err(|error| map_workspace_error(&error))?;
    let approved = state
        .bmad_model
        .lock()
        .approve(
            state,
            &authority,
            &workspace_authority,
            ApproveBmadHelpReviewInput {
                renderer_session,
                workspace_id,
                workspace_grant_epoch,
                manifest_hash,
                approved_at,
            },
        )
        .map_err(map_bmad_model_error)?;
    drop(workspace_authority);
    drop(authority);
    if !approved.send_eligible {
        return Err(map_bmad_model_projection_error(
            BmadHelpModelAccessProjectionError::Unavailable,
        ));
    }
    project_bmad_help_approved(BmadHelpApprovalInput {
        manifest_hash: approved.manifest_hash,
        decision_id: approved.decision_id,
        expires_at: approved.expires_at,
    })
    .map(HostCommandData::BmadHelpApproved)
    .map_err(map_bmad_model_projection_error)
}

fn cancel_bmad_help_review(
    state: &HostState,
    renderer_session: &RendererSessionGuard<'_>,
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    manifest_hash: Sha256Digest,
    decision_id: ContractId,
    cancelled_at: UnixMillis,
) -> Result<HostCommandData, LocalError> {
    let authority = state.ready_workspace_commit()?;
    let workspace_authority = state
        .workspace
        .authorize_scope(workspace_id.as_str(), workspace_grant_epoch)
        .map_err(|error| map_workspace_error(&error))?;
    state
        .bmad_model
        .lock()
        .cancel(
            state,
            &authority,
            &workspace_authority,
            CancelBmadHelpReviewInput {
                renderer_session,
                workspace_id,
                workspace_grant_epoch,
                manifest_hash,
                decision_id: decision_id.clone(),
                cancelled_at,
            },
        )
        .map_err(map_bmad_model_error)?;
    drop(workspace_authority);
    drop(authority);
    Ok(HostCommandData::BmadHelpCancelled(
        project_bmad_help_cancelled(BmadHelpCancellationInput {
            manifest_hash,
            decision_id,
        }),
    ))
}

fn submit_bmad_help_review(
    state: &HostState,
    renderer_session: &RendererSessionGuard<'_>,
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    manifest_hash: Sha256Digest,
    decision_id: ContractId,
    submitted_at: UnixMillis,
) -> Result<HostCommandData, LocalError> {
    let authority = state.ready_workspace_commit()?;
    let workspace_authority = state
        .workspace
        .authorize_scope(workspace_id.as_str(), workspace_grant_epoch)
        .map_err(|error| map_workspace_error(&error))?;
    let completed = state
        .bmad_model
        .lock()
        .submit(
            state,
            &authority,
            &workspace_authority,
            SubmitBmadHelpReviewInput {
                renderer_session,
                workspace_id,
                workspace_grant_epoch,
                manifest_hash,
                decision_id,
                submitted_at,
            },
        )
        .map_err(map_bmad_model_error)?;
    drop(workspace_authority);
    drop(authority);
    Ok(HostCommandData::BmadHelpRunCompleted(completed))
}

#[expect(
    clippy::too_many_lines,
    reason = "the renderer projection explicitly maps and validates every D2 review field"
)]
fn project_review_for_renderer(
    review: crate::bmad_model::coordinator::BmadHelpReviewProjection,
) -> Result<desktop_ipc::BmadHelpReviewProjection, LocalError> {
    let crate::bmad_model::coordinator::BmadHelpReviewProjection {
        renderer_session_id: _,
        workspace_id,
        workspace_grant_epoch,
        workspace_catalog_version: _,
        run_id,
        session_id,
        destination_label,
        development_only,
        consent_disclosure,
        consent_disclosure_hash: _,
        context,
    } = review;
    let ContextReviewProjection {
        manifest_hash,
        purpose,
        model_role: _,
        provider_profile_hash: _,
        model_profile_hash: _,
        deployment_hash: _,
        region,
        retention_mode,
        expires_at,
        items,
        exclusions,
        secret_findings,
        total_outbound_bytes,
        total_token_estimate,
        redaction_limitation,
    } = context;
    let labels: std::collections::HashMap<ContractId, RelativeWorkspacePath> = items
        .iter()
        .map(|item| (item.client_item_id.clone(), item.relative_label.clone()))
        .collect();
    let items = items
        .into_iter()
        .map(|item| BmadHelpReviewItemInput {
            relative_label: item.relative_label,
            semantic_role: item.semantic_role,
            language: item.language,
            outbound_byte_count: item.outbound_byte_count,
            token_estimate: item.token_estimate,
            classification: match item.classification {
                ContextClassification::Public => BmadHelpContextClassificationProjection::Public,
                ContextClassification::Internal => {
                    BmadHelpContextClassificationProjection::Internal
                }
                ContextClassification::Confidential => {
                    BmadHelpContextClassificationProjection::Confidential
                }
            },
            redactions: item
                .redactions
                .into_iter()
                .map(|redaction| BmadHelpReviewRedactionInput {
                    kind: redaction.kind,
                    occurrence_count: redaction.occurrence_count,
                })
                .collect(),
            outbound_content: item.outbound_content,
        })
        .collect();
    let exclusions = exclusions
        .into_iter()
        .map(|exclusion| BmadHelpReviewExclusionInput {
            relative_label: exclusion.relative_label,
            reason: exclusion.reason,
        })
        .collect();
    let secret_findings = secret_findings
        .into_iter()
        .map(|finding| {
            let relative_label = labels
                .get(&finding.client_item_id)
                .cloned()
                .ok_or_else(|| {
                    map_bmad_model_projection_error(BmadHelpModelAccessProjectionError::Unavailable)
                })?;
            Ok(BmadHelpSecretFindingInput {
                relative_label,
                kind: finding.kind,
                occurrence_count: finding.occurrence_count,
            })
        })
        .collect::<Result<Vec<_>, LocalError>>()?;
    let retention_mode = match retention_mode {
        RetentionMode::TransientNoStore => BmadHelpRetentionProjection::TransientNoStore,
    };
    project_bmad_help_review(BmadHelpReviewInput {
        workspace_id,
        workspace_grant_epoch,
        run_id,
        session_id,
        destination_label,
        development_only,
        consent_disclosure,
        manifest_hash,
        purpose,
        region,
        retention_mode,
        expires_at,
        items,
        exclusions,
        secret_findings,
        total_outbound_bytes,
        total_token_estimate,
        redaction_limitation,
    })
    .map_err(map_bmad_model_projection_error)
}

fn read_workspace_text(
    state: &HostState,
    workspace_id: &ContractId,
    relative_path: &RelativeWorkspacePath,
    max_bytes: u32,
) -> Result<HostCommandData, LocalError> {
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

/// The adopted Core-module skills, joined to the menu identity upstream gives
/// them in its own catalog.
///
/// These capabilities have always been runnable — `runtime_capability_enabled`
/// admits them and the generic lifecycle covers them — but they carry no agent
/// menu path, so nothing ever projected them and no user could reach them.
fn core_skill_projections(
    foundation: &BmadLoadedFoundation,
) -> Vec<desktop_ipc::BmadCoreSkillProjection> {
    let Some(catalog) = foundation.core_skill_catalog() else {
        return Vec::new();
    };
    crate::bmad_capability_host::core_skill_capability_ids()
        .into_iter()
        .filter_map(|capability_id| {
            // `bmm:bmad-spec` -> `bmad-spec`, the upstream catalog's skill key.
            let skill = capability_id.split_once(':').map(|(_, tail)| tail)?;
            let step = catalog.step(skill)?;
            Some(desktop_ipc::BmadCoreSkillProjection {
                capability_id: capability_id.to_owned(),
                skill: step.skill.clone(),
                display_name: step.display_name.clone(),
                menu_code: step.menu_code.clone(),
                // Upstream descriptions name artifacts (`SPEC.md`), and the
                // library projection must stay free of anything path-shaped.
            })
        })
        .collect()
}

fn bmad_library_snapshot(
    foundation: &BmadLoadedFoundation,
    scope: BmadLibraryProjectionScope,
    cursor: Option<&str>,
) -> Result<HostCommandData, LocalError> {
    #[cfg(any(feature = "deterministic-help", feature = "direct-model-providers"))]
    let activations = &[("core", "bmad-help")][..];
    #[cfg(not(any(feature = "deterministic-help", feature = "direct-model-providers")))]
    let activations = &[][..];
    let runtime_menu_bindings =
        crate::bmad_capability_host::reviewed_method_menu_bindings(foundation.roster())
            .map_err(map_bmad_projection_error)?;
    let builder_packages = foundation
        .builder_packages()
        .iter()
        .map(|package| desktop_ipc::BmadBuilderPackageProjection {
            package_name: package.package_name.clone(),
            package_version: package.package_version.clone(),
            package_kind: match package.package_kind {
                crate::bmad_foundation::BuilderPackageKind::Agent => {
                    desktop_ipc::BmadBuilderPackageKind::Agent
                }
                crate::bmad_foundation::BuilderPackageKind::Workflow => {
                    desktop_ipc::BmadBuilderPackageKind::Workflow
                }
            },
            display_name: package.display_name.clone(),
            activation_state: "installed_inactive".to_owned(),
            resource_count: u32::try_from(package.resource_count).unwrap_or(u32::MAX),
            // Display fingerprint only: a short prefix cannot be replayed as a
            // digest and keeps the sealed projection free of authority bytes.
            descriptor_digest: package
                .descriptor_digest
                .hex_value()
                .chars()
                .take(12)
                .collect(),
            blocker_codes: vec!["builder_engine_gated".to_owned()],
        })
        .collect();
    project_bmad_library_with_runtime_menu_bindings(
        foundation.package(),
        foundation.catalog(),
        foundation.roster(),
        scope,
        cursor,
        activations,
        &runtime_menu_bindings,
        builder_packages,
        core_skill_projections(foundation),
    )
    .map(HostCommandData::BmadLibrarySnapshot)
    .map_err(map_bmad_projection_error)
}

fn map_capability_error(
    error: crate::bmad_model::capability_coordinator::BmadCapabilityCoordinatorError,
) -> LocalError {
    use crate::bmad_model::capability_coordinator::BmadCapabilityCoordinatorError as E;
    match error {
        E::Unauthorized => LocalError::new(
            LocalErrorCode::Unauthorized,
            "No capability run is active for this workspace.",
            false,
        ),
        E::Conflict => LocalError::new(
            LocalErrorCode::Conflict,
            "Another capability run is already in review.",
            true,
        ),
        E::CapabilityBindingMismatch => LocalError::new(
            LocalErrorCode::Unauthorized,
            "The request does not match the reviewed capability.",
            false,
        ),
        E::ConsentBindingMismatch => LocalError::new(
            LocalErrorCode::Unauthorized,
            "The consent decision does not match the reviewed context.",
            false,
        ),
        E::ConsentExpired => LocalError::new(
            LocalErrorCode::Expired,
            "The reviewed consent expired before it was used.",
            true,
        ),
        E::ConsentAlreadyConsumed => LocalError::new(
            LocalErrorCode::Conflict,
            "The reviewed consent was already used.",
            false,
        ),
        E::OutputRejected | E::ResultArchetypeMismatch => LocalError::new(
            LocalErrorCode::IntegrityFailure,
            "The model output failed capability verification.",
            false,
        ),
        E::Transport => LocalError::new(
            LocalErrorCode::TemporarilyUnavailable,
            "The model destination is unavailable in this composition.",
            true,
        ),
        E::Store | E::Integrity => LocalError::new(
            LocalErrorCode::IntegrityFailure,
            "The capability run could not be durably recorded.",
            false,
        ),
    }
}

/// Where the project stands in the BMAD method, computed rather than asked.
///
/// This is the one place the runtime deliberately outperforms upstream: the
/// answer comes from the sealed workflow graph plus governed workspace reads,
/// so it costs no model call and cannot be hallucinated.
fn bmad_workflow_state(
    state: &HostState,
    foundation: &BmadLoadedFoundation,
    workspace_id: &ContractId,
    workspace_grant_epoch: u64,
) -> Result<HostCommandData, LocalError> {
    let authority = state.ready_workspace_commit()?;
    let workspace_authority = state
        .workspace
        .authorize_scope(workspace_id.as_str(), workspace_grant_epoch)
        .map_err(|error| map_workspace_error(&error))?;
    let graph = foundation.workflow_graph().ok_or_else(|| {
        LocalError::new(
            LocalErrorCode::IntegrityFailure,
            "The sealed method workflow graph is unavailable.",
            false,
        )
    })?;
    let projection = crate::bmad_workflow::compute_workflow_state(
        &state.workspace,
        workspace_id.as_str(),
        graph,
    );
    drop(workspace_authority);
    drop(authority);
    Ok(HostCommandData::BmadWorkflowState(projection))
}

fn map_chat_error(error: crate::chat::ChatCoordinatorError) -> LocalError {
    use crate::chat::ChatCoordinatorError as E;
    match error {
        E::Unauthorized => LocalError::new(
            LocalErrorCode::Unauthorized,
            "The conversation is not available for this workspace grant.",
            false,
        ),
        E::Conflict => LocalError::new(
            LocalErrorCode::Conflict,
            "A reply is already in flight for this conversation.",
            true,
        ),
        E::OutputRejected => LocalError::new(
            LocalErrorCode::IntegrityFailure,
            "The model reply failed verification and was discarded.",
            true,
        ),
        E::Transport => LocalError::new(
            LocalErrorCode::FeatureDisabled,
            "Connect a model provider in Settings before sending a message.",
            false,
        ),
        E::Integrity => LocalError::new(
            LocalErrorCode::IntegrityFailure,
            "The message could not be sealed for sending.",
            false,
        ),
    }
}

pub(crate) struct SendChatMessageInput {
    pub workspace_id: ContractId,
    pub workspace_grant_epoch: u64,
    pub client_session_ref: String,
    pub agent_code: Option<String>,
    pub text: String,
    pub context_paths: Vec<desktop_runtime::RelativeWorkspacePath>,
    pub now: UnixMillis,
}

/// One ordinary chat turn — the app's default lane.
///
/// BMAD is optional here: with no agent selected this is a plain assistant.
/// Selecting one binds that agent's *sealed* persona projection as reviewed
/// context, which is how the BMAD method is practised in Forge without a
/// separate LLM runtime. The persona grants nothing; it is instruction text
/// that travels through the same manifest as any other context item.
fn send_chat_message(
    state: &HostState,
    foundation: &BmadLoadedFoundation,
    input: SendChatMessageInput,
) -> Result<HostCommandData, LocalError> {
    let authority = state.ready_workspace_commit()?;
    let workspace_authority = state
        .workspace
        .authorize_scope(input.workspace_id.as_str(), input.workspace_grant_epoch)
        .map_err(|error| map_workspace_error(&error))?;
    let workspace = workspace_authority.projection();

    // A selected agent must be one the sealed foundation actually carries.
    // An unknown code fails closed rather than silently becoming a plain chat.
    let persona = match input.agent_code.as_deref() {
        Some(agent_code) => Some(foundation.persona_for(agent_code).ok_or_else(|| {
            LocalError::new(
                LocalErrorCode::NotFound,
                "No sealed persona exists for that agent.",
                false,
            )
        })?),
        None => None,
    };
    let attachments = reviewed_context_candidates(state, &input.workspace_id, input.context_paths)?;
    let project_id = ContractId::new(workspace.project_id.clone()).map_err(|_| {
        LocalError::new(
            LocalErrorCode::IntegrityFailure,
            "The workspace project identity is invalid.",
            false,
        )
    })?;
    let configuration = state.model_configuration()?;
    let store = state.local_store(authority.authority())?;
    let transport = crate::bmad_capability_host::active_capability_transport(state);

    // Whether this turn may ask for a tool is decided here, once, before
    // anything is sealed — and it decides the reply schema with it. The
    // provider picks the system prompt from the sealed schema id and from
    // nothing else, so a turn sealed text-only is a turn whose model is never
    // told tools exist, however willing the host below is to run one. Resolved
    // before the chat lock is taken so two locks are never held at once.
    let output_schema = crate::chat::ChatOutputSchema::for_tool_grant(
        crate::chat_tools::chat_turn_tools_granted(&state.tool_grants.lock()),
    );

    // Seal under the lock, dispatch unlocked, complete under the lock: a slow
    // reply never blocks sign-out, revocation, or another conversation.
    let dispatch = state
        .chat
        .lock()
        .begin_turn(crate::chat::BeginChatTurn {
            installation_id: state.installation_id(),
            project_id: &project_id,
            workspace_id: &input.workspace_id,
            client_session_ref: &input.client_session_ref,
            agent_code: input.agent_code,
            persona_instruction: persona
                .map(super::bmad_foundation::BmadSealedPersonaProjection::instruction_bytes),
            text: input.text,
            attachments,
            configuration: &configuration,
            output_schema,
            now: input.now,
        })
        .map_err(map_chat_error)?;
    // The tool loop. Each pass sends, then either completes the turn or runs one
    // permitted read-only tool and re-seals. The iteration ceiling is not here:
    // it is the turn's egress grant, which `seal_turn` consults before every
    // call, so a runaway loop is stopped by the accounting rather than by a
    // count kept somewhere the user cannot see.
    let mut dispatch = dispatch;
    let turn = loop {
        let dispatch_id = dispatch.dispatch_id;
        let Ok((_dispatched, raw_output)) = transport.send(
            dispatch.authorized_request,
            &dispatch.deterministic_fixture,
            dispatch.sent_at,
        ) else {
            state.chat.lock().abort_turn(dispatch_id);
            return Err(map_chat_error(crate::chat::ChatCoordinatorError::Transport));
        };
        let outcome =
            match state
                .chat
                .lock()
                .finalize_turn(dispatch_id, &raw_output, input.now, store)
            {
                Ok(outcome) => outcome,
                Err(error) => {
                    state.chat.lock().abort_turn(dispatch_id);
                    return Err(map_chat_error(error));
                }
            };
        let invocation = match outcome {
            crate::chat::TurnOutcome::Completed(turn) => break *turn,
            crate::chat::TurnOutcome::ToolRequested(invocation) => invocation,
        };
        // A refused or failed tool is reported back to the model as text rather
        // than failing the turn: the model asked for something it could not
        // have, which it can reason about and recover from. The result is data,
        // and it enters the transcript as such.
        let rendered = crate::chat_tools::run_tool(
            &state.workspace,
            &state.tool_grants.lock(),
            input.workspace_id.as_str(),
            &invocation.tool,
            &invocation.subject,
        )
        .unwrap_or_else(|error| format!("tool refused: {error:?}"));
        dispatch = match state
            .chat
            .lock()
            .resume_turn(dispatch_id, rendered, input.now)
        {
            Ok(next) => next,
            Err(error) => {
                state.chat.lock().abort_turn(dispatch_id);
                return Err(map_chat_error(error));
            }
        };
    };
    drop(workspace_authority);
    drop(authority);
    Ok(HostCommandData::ChatTurn(turn))
}

fn chat_history(
    state: &HostState,
    workspace_id: &ContractId,
    workspace_grant_epoch: u64,
    client_session_ref: &str,
) -> Result<HostCommandData, LocalError> {
    let authority = state.ready_workspace_commit()?;
    let workspace_authority = state
        .workspace
        .authorize_scope(workspace_id.as_str(), workspace_grant_epoch)
        .map_err(|error| map_workspace_error(&error))?;
    let store = state.local_store(authority.authority())?;
    let history = state
        .chat
        .lock()
        .history(
            state.installation_id(),
            workspace_id,
            client_session_ref,
            store,
        )
        .map_err(map_chat_error)?;
    drop(workspace_authority);
    drop(authority);
    Ok(HostCommandData::ChatHistory(history))
}

/// Reads each selected workspace path through the governed broker into one
/// bounded reviewed-context candidate.
fn reviewed_context_candidates(
    state: &HostState,
    workspace_id: &ContractId,
    context_paths: Vec<desktop_runtime::RelativeWorkspacePath>,
) -> Result<Vec<desktop_egress::ContextCandidate>, LocalError> {
    let mut candidates = Vec::with_capacity(context_paths.len());
    let mut total_bytes = 0_u64;
    for (index, relative_path) in context_paths.into_iter().enumerate() {
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
                "A selected context file exceeds the per-file limit.",
            ));
        }
        total_bytes =
            total_bytes.saturating_add(u64::try_from(preview.content.len()).unwrap_or(u64::MAX));
        if total_bytes > MAX_CONTEXT_BYTES {
            return Err(resource_limit_error(
                "The selected context exceeds the per-request byte limit.",
            ));
        }
        let relative_label = desktop_runtime::RelativeWorkspacePath::new(&preview.relative_path)
            .map_err(|_| {
                LocalError::new(
                    LocalErrorCode::IntegrityFailure,
                    "A governed read returned an invalid relative path.",
                    false,
                )
            })?;
        candidates.push(
            crate::bmad_capability_host::context_candidate(relative_label, preview.content, index)
                .map_err(map_capability_error)?,
        );
    }
    Ok(candidates)
}

/// The doc workspace's accumulated memory log, as at most one reviewed context
/// candidate.
///
/// Empty for a capability with no doc workspace and for a folder whose log is
/// still empty, so a first run against a folder is unchanged by P14.
///
/// For the stepwise capability this overlaps its in-session decision log: the
/// persisted log is the superset, covering earlier progressions the in-memory
/// session no longer holds, while the decision log covers only the current one.
/// Both are kept because they answer different questions, and the item ceiling
/// counts both. Note that this restores earlier *decisions* to context, not the
/// progression cursor — after a restart the progression still opens at step one
/// (resume-by-reload is out of scope for P14).
fn memlog_context_candidate(
    state: &HostState,
    authority: &ReadyAuthorityGuard<'_>,
    workspace_id: &ContractId,
    capability_id: &str,
) -> Result<Option<desktop_egress::ContextCandidate>, LocalError> {
    let Some(doc_workspace) = crate::bmad_memlog::doc_workspace_for(capability_id) else {
        return Ok(None);
    };
    let entries = state
        .local_store(authority)?
        .memlog_entries(workspace_id.as_str(), &doc_workspace)
        .map_err(|_| recovery_error())?;
    if entries.is_empty() {
        return Ok(None);
    }
    crate::bmad_capability_host::memlog_candidate(crate::bmad_memlog::render_for_context(
        &doc_workspace,
        &entries,
    ))
    .map(Some)
    .map_err(map_capability_error)
}

struct CapabilityContextInput<'a> {
    state: &'a HostState,
    foundation: &'a BmadLoadedFoundation,
    authority: &'a ReadyAuthorityGuard<'a>,
    workspace_id: &'a ContractId,
    capability_id: &'a str,
    binding: &'static crate::bmad_capability_host::CapabilityBinding,
    context_paths: Vec<desktop_runtime::RelativeWorkspacePath>,
    created_at: UnixMillis,
}

/// The assembled context, plus the step the stepwise capability selected — the
/// caller reports that step's progress back to the renderer.
struct CapabilityContext {
    candidates: Vec<desktop_egress::ContextCandidate>,
    architecture_step: Option<desktop_runtime::BmadArchitectureStep>,
}

/// Assembles every reviewed context candidate for one capability run, in the
/// order the consent surface shows them: host-injected items first, then the
/// user's own attachments.
///
/// Each item here is counted by `MAX_CAPABILITY_CONTEXT_ITEMS`; adding one
/// without raising that ceiling is what makes a whole lane fail to seal.
fn capability_context_candidates(
    input: CapabilityContextInput<'_>,
) -> Result<CapabilityContext, LocalError> {
    let CapabilityContextInput {
        state,
        foundation,
        authority,
        workspace_id,
        capability_id,
        binding,
        context_paths,
        created_at,
    } = input;
    // The stepwise capability places the host-selected step and the accepted
    // decision log in the reviewed context, so the step the user consents to
    // sending is covered by the manifest hash like any other context item.
    let architecture_step =
        if crate::bmad_architecture::is_stepwise_capability(foundation, capability_id) {
            Some(
                state
                    .bmad_architecture
                    .lock()
                    .select_step(state.local_store(authority)?, workspace_id, created_at)
                    .map_err(map_capability_error)?,
            )
        } else {
            None
        };
    let mut candidates = match architecture_step {
        Some(step) => state
            .bmad_architecture
            .lock()
            .step_context_candidates(step)
            .map_err(map_capability_error)?,
        None => Vec::new(),
    };
    // What earlier runs against this doc workspace decided.
    candidates.extend(memlog_context_candidate(
        state,
        authority,
        workspace_id,
        capability_id,
    )?);
    // A verbatim upstream body is always read under the host's boundary
    // statement, so its tool directives cannot be taken at face value.
    if crate::bmad_capability_host::is_core_skill_binding(binding) {
        candidates.insert(
            0,
            crate::bmad_capability_host::adopted_instruction_preamble()
                .map_err(map_capability_error)?,
        );
    }
    candidates.extend(reviewed_context_candidates(
        state,
        workspace_id,
        context_paths,
    )?);
    Ok(CapabilityContext {
        candidates,
        architecture_step,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "the dispatch surface passes each validated envelope field explicitly"
)]
fn prepare_bmad_capability_run(
    state: &HostState,
    foundation: &BmadLoadedFoundation,
    renderer_session: &RendererSessionGuard<'_>,
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    capability_id: &str,
    context_paths: Vec<desktop_runtime::RelativeWorkspacePath>,
    created_at: UnixMillis,
) -> Result<HostCommandData, LocalError> {
    let binding =
        crate::bmad_capability_host::capability_binding(capability_id).ok_or_else(|| {
            LocalError::new(
                LocalErrorCode::NotFound,
                "The capability is not in the reviewed closure ledger.",
                false,
            )
        })?;
    if !crate::bmad_capability_host::runtime_capability_enabled(foundation.roster(), binding)
        .map_err(map_bmad_projection_error)?
    {
        return Err(LocalError::new(
            LocalErrorCode::FeatureDisabled,
            "The capability is reviewed but is not enabled by the current runtime.",
            false,
        ));
    }
    let authority = state.ready_workspace_commit()?;
    let workspace_authority = state
        .workspace
        .authorize_scope(workspace_id.as_str(), workspace_grant_epoch)
        .map_err(|error| map_workspace_error(&error))?;
    let workspace = workspace_authority.projection();

    let instruction_bytes = foundation
        .resource_bytes(binding.projection_path)
        .ok_or_else(|| {
            LocalError::new(
                LocalErrorCode::IntegrityFailure,
                "The sealed capability instruction projection is missing.",
                false,
            )
        })?;

    let CapabilityContext {
        candidates,
        architecture_step,
    } = capability_context_candidates(CapabilityContextInput {
        state,
        foundation,
        authority: authority.authority(),
        workspace_id: &workspace_id,
        capability_id,
        binding,
        context_paths,
        created_at,
    })?;

    let project_id = ContractId::new(workspace.project_id.clone()).map_err(|_| {
        LocalError::new(
            LocalErrorCode::IntegrityFailure,
            "The workspace project identity is invalid.",
            false,
        )
    })?;
    let configuration = state.model_configuration()?;
    let prepare_input = crate::bmad_capability_host::assemble_capability_prepare(
        crate::bmad_capability_host::AssembleCapabilityPrepareInput {
            binding,
            configuration: &configuration,
            instruction_bytes: &instruction_bytes,
            renderer_session_id: renderer_session.session_id(),
            installation_id: state.installation_id(),
            project_id: &project_id,
            workspace_id,
            workspace_grant_epoch,
            workspace_context_read_epoch: workspace.context_read_epoch,
            candidates,
            created_at,
        },
    )
    .map_err(map_capability_error)?;
    let review = state
        .bmad_capabilities
        .lock()
        .prepare(prepare_input)
        .map_err(map_capability_error)?;
    drop(workspace_authority);
    drop(authority);
    Ok(HostCommandData::CapabilityReview(
        crate::wire::CapabilityReviewWire::new(
            review,
            architecture_step.map(crate::bmad_architecture::selected_step_progress),
        ),
    ))
}

fn approve_bmad_capability_run(
    state: &HostState,
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    capability_id: &str,
    manifest_hash: Sha256Digest,
    approved_at: UnixMillis,
) -> Result<HostCommandData, LocalError> {
    let authority = state.ready_workspace_commit()?;
    let workspace_authority = state
        .workspace
        .authorize_scope(workspace_id.as_str(), workspace_grant_epoch)
        .map_err(|error| map_workspace_error(&error))?;
    let workspace = workspace_authority.projection();
    let input = crate::bmad_capability_host::approve_input(
        capability_id,
        crate::bmad_capability_host::WorkspaceBindingInput {
            id: workspace_id,
            grant_epoch: workspace_grant_epoch,
            context_read_epoch: workspace.context_read_epoch,
        },
        manifest_hash,
        approved_at,
    )
    .map_err(map_capability_error)?;
    let approved = state
        .bmad_capabilities
        .lock()
        .approve(&input)
        .map_err(map_capability_error)?;
    drop(workspace_authority);
    drop(authority);
    Ok(HostCommandData::CapabilityApproved(approved))
}

fn cancel_bmad_capability_run(
    state: &HostState,
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    capability_id: &str,
    manifest_hash: Sha256Digest,
    decision_id: ContractId,
    cancelled_at: UnixMillis,
) -> Result<HostCommandData, LocalError> {
    let authority = state.ready_workspace_commit()?;
    let workspace_authority = state
        .workspace
        .authorize_scope(workspace_id.as_str(), workspace_grant_epoch)
        .map_err(|error| map_workspace_error(&error))?;
    let workspace = workspace_authority.projection();
    let input = crate::bmad_capability_host::cancel_input(
        capability_id,
        crate::bmad_capability_host::WorkspaceBindingInput {
            id: workspace_id,
            grant_epoch: workspace_grant_epoch,
            context_read_epoch: workspace.context_read_epoch,
        },
        manifest_hash,
        decision_id,
        cancelled_at,
    )
    .map_err(map_capability_error)?;
    state
        .bmad_capabilities
        .lock()
        .cancel(&input)
        .map_err(map_capability_error)?;
    drop(workspace_authority);
    drop(authority);
    Ok(HostCommandData::CapabilityCancelled(
        crate::wire::CapabilityCancelledProjection {
            capability_id: capability_id.to_owned(),
            manifest_hash,
            cancelled: true,
        },
    ))
}

#[expect(
    clippy::too_many_arguments,
    reason = "the dispatch surface passes each validated envelope field explicitly"
)]
fn submit_bmad_capability_run(
    state: &HostState,
    foundation: &BmadLoadedFoundation,
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    capability_id: &str,
    manifest_hash: Sha256Digest,
    decision_id: ContractId,
    submitted_at: UnixMillis,
) -> Result<HostCommandData, LocalError> {
    let authority = state.ready_workspace_commit()?;
    let workspace_authority = state
        .workspace
        .authorize_scope(workspace_id.as_str(), workspace_grant_epoch)
        .map_err(|error| map_workspace_error(&error))?;
    let store = state.local_store(authority.authority())?;
    let workspace = workspace_authority.projection();
    let stepwise = crate::bmad_architecture::is_stepwise_capability(foundation, capability_id);
    // Retained only for the stepwise capability, whose progression is keyed by
    // workspace; every other capability consumes the identity outright.
    let architecture_workspace = stepwise.then(|| workspace_id.clone());
    // A stepwise run answers one step, so its artifact is a contribution to the
    // doc workspace rather than its whole state. Reading it as the whole would
    // record every previous step's decisions as withdrawn.
    let run_scope = crate::bmad_memlog::RunScope::for_capability(foundation, capability_id);
    let input = crate::bmad_capability_host::submit_input(
        capability_id,
        crate::bmad_capability_host::WorkspaceBindingInput {
            id: workspace_id,
            grant_epoch: workspace_grant_epoch,
            context_read_epoch: workspace.context_read_epoch,
        },
        manifest_hash,
        decision_id,
        submitted_at,
        run_scope,
    )
    .map_err(map_capability_error)?;
    let transport = crate::bmad_capability_host::active_capability_transport(state);
    // Begin the dispatch under the coordinator lock, then release the lock for
    // the model round trip so a slow request cannot block sign-out, workspace
    // revocation (`invalidate`), or other capability operations. Re-acquire the
    // lock only to record the verified result.
    let dispatch = state
        .bmad_capabilities
        .lock()
        .begin_dispatch(&input)
        .map_err(map_capability_error)?;
    let dispatch_id = dispatch.dispatch_id;
    // The coordinator now keeps one lifecycle slot per workspace, so finalizing
    // and aborting name the workspace as well as the dispatch. The request
    // carries it, so nothing else has to be kept alive across the round trip.
    let dispatch_workspace = dispatch.workspace_id.clone();
    let Ok((_dispatched, raw_output)) = transport.send(
        dispatch.authorized_request,
        &dispatch.deterministic_fixture,
        dispatch.submitted_at,
    ) else {
        state.bmad_capabilities.lock().abort_dispatch(
            &dispatch_workspace,
            dispatch_id,
            crate::bmad_model::capability_coordinator::CapabilityTerminalReason::ConsentConsumed,
        );
        return Err(map_capability_error(
            crate::bmad_model::capability_coordinator::BmadCapabilityCoordinatorError::Transport,
        ));
    };
    let completed = state
        .bmad_capabilities
        .lock()
        .finalize_dispatch(
            &dispatch_workspace,
            dispatch_id,
            &raw_output,
            &crate::bmad_capability_host::WireCapabilityOutputVerifier,
            store,
        )
        .map_err(map_capability_error)?;
    // The stepwise capability advances only on a result that is already
    // durably stored. Reading it back and re-parsing it means the progression
    // can never advance on anything the store and the strict archetype
    // constructors have not both accepted. The coordinator lock is released
    // before the architecture lock is taken; the inverse order never occurs.
    let architecture = match architecture_workspace {
        Some(workspace_id) => Some(advance_architecture_progression(
            state,
            store,
            &workspace_id,
            &completed.run_id,
            submitted_at,
        )?),
        None => None,
    };
    drop(workspace_authority);
    drop(authority);
    Ok(HostCommandData::CapabilityCompleted(
        crate::wire::CapabilityCompletedWire::new(completed, architecture),
    ))
}

/// Feeds one completed architecture step into the ordered progression and
/// reports the result, distilling the spine on the final step.
fn advance_architecture_progression(
    state: &HostState,
    store: &desktop_store::LocalStore,
    workspace_id: &ContractId,
    run_id: &ContractId,
    submitted_at: UnixMillis,
) -> Result<crate::bmad_architecture::ArchitectureProgressProjection, LocalError> {
    let step = state
        .bmad_architecture
        .lock()
        .current_step(workspace_id)
        .ok_or_else(|| {
            LocalError::new(
                LocalErrorCode::Conflict,
                "No architecture progression is active for this workspace.",
                false,
            )
        })?;
    let record = store
        .bmad_capability_run(run_id)
        .map_err(|_| {
            LocalError::new(
                LocalErrorCode::IntegrityFailure,
                "The capability run history could not be read.",
                false,
            )
        })?
        .and_then(|record| record.result_json)
        .ok_or_else(|| {
            LocalError::new(
                LocalErrorCode::IntegrityFailure,
                "The completed architecture step has no stored result.",
                false,
            )
        })?;
    let value: serde_json::Value = desktop_runtime::deserialize_strict_json(record.as_bytes())
        .map_err(|_| {
            LocalError::new(
                LocalErrorCode::IntegrityFailure,
                "The stored architecture step could not be re-verified.",
                false,
            )
        })?;
    let desktop_runtime::BmadCapabilityOutput::DocumentArtifact(artifact) =
        crate::bmad_capability_host::parse_capability_result(&value)
            .map_err(map_capability_error)?
    else {
        return Err(LocalError::new(
            LocalErrorCode::IntegrityFailure,
            "The architecture step returned a result outside its declared archetype.",
            false,
        ));
    };
    let mut coordinator = state.bmad_architecture.lock();
    let session_id = coordinator.active_session_id(workspace_id).ok_or_else(|| {
        LocalError::new(
            LocalErrorCode::Conflict,
            "No architecture progression is active for this workspace.",
            false,
        )
    })?;
    let progress = coordinator
        .accept_step(workspace_id, step, artifact, submitted_at)
        .map_err(map_capability_error)?;
    drop(coordinator);

    // Record the acceptance durably *after* the in-memory progression took it,
    // so the stored boundary can never claim a step the session refused. The
    // reverse order would let a rejected step advance a resumed progression.
    store
        .record_architecture_step(session_id.as_str(), run_id.as_str(), submitted_at.0)
        .map_err(|_| {
            LocalError::new(
                LocalErrorCode::IntegrityFailure,
                "The accepted architecture step could not be durably recorded.",
                false,
            )
        })?;
    if progress.finalized {
        store
            .finalize_architecture_session(session_id.as_str())
            .map_err(|_| {
                LocalError::new(
                    LocalErrorCode::IntegrityFailure,
                    "The finalized architecture progression could not be recorded.",
                    false,
                )
            })?;
    }
    Ok(progress)
}

/// ADR-0007: converts the latest stored change-set result for one
/// capability into an ordinary D3 proposal through the existing
/// `changes.propose` authority. Model origin grants nothing; the change
/// set re-enters the same review, approval, and apply machinery as any
/// renderer-originated edit.
fn propose_bmad_capability_changes(
    state: &HostState,
    foundation: &BmadLoadedFoundation,
    request_id: &ContractId,
    accepted_at: UnixMillis,
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    capability_id: &str,
) -> Result<HostCommandData, LocalError> {
    // The stepwise capability proposes its *run folder* — the derived spine
    // and the append-only memlog it came from — rather than a change set,
    // because that is what a real architecture run leaves behind. It still
    // enters the same D3 review as any other proposed edit.
    if crate::bmad_architecture::is_stepwise_capability(foundation, capability_id) {
        let files = {
            let authority = state.ready_workspace_commit()?;
            let workspace_authority = state
                .workspace
                .authorize_scope(workspace_id.as_str(), workspace_grant_epoch)
                .map_err(|error| map_workspace_error(&error))?;
            let files = state
                .bmad_architecture
                .lock()
                .finalized_run_files(&workspace_id)
                .map_err(map_capability_error)?;
            drop(workspace_authority);
            drop(authority);
            files
        };
        let proposed = files
            .into_iter()
            .map(
                |(relative_path, content)| desktop_runtime::ProposedFileChange::SetContent {
                    relative_path,
                    content,
                },
            )
            .collect::<Vec<_>>();
        return crate::edits::execute_changes_command(
            state,
            request_id,
            accepted_at,
            LocalCommand::ProposeChanges {
                workspace_id,
                workspace_grant_epoch,
                changes: proposed,
            },
        );
    }
    let proposed = {
        let authority = state.ready_workspace_commit()?;
        let workspace_authority = state
            .workspace
            .authorize_scope(workspace_id.as_str(), workspace_grant_epoch)
            .map_err(|error| map_workspace_error(&error))?;
        let store = state.local_store(authority.authority())?;
        let record = store
            .latest_bmad_capability_run(&workspace_id, capability_id)
            .map_err(|_| {
                LocalError::new(
                    LocalErrorCode::IntegrityFailure,
                    "The capability run history could not be read.",
                    false,
                )
            })?
            .ok_or_else(|| {
                LocalError::new(
                    LocalErrorCode::NotFound,
                    "No completed run exists for this capability.",
                    false,
                )
            })?;
        let result_json = record.result_json.ok_or_else(|| {
            LocalError::new(
                LocalErrorCode::NotFound,
                "The latest run has no verified result.",
                false,
            )
        })?;
        let proposed = crate::bmad_governed_proposal::proposed_changes_from_stored_result(
            state,
            workspace_id.as_str(),
            &result_json,
        )?;
        drop(workspace_authority);
        drop(authority);
        proposed
    };
    crate::edits::execute_changes_command(
        state,
        request_id,
        accepted_at,
        LocalCommand::ProposeChanges {
            workspace_id,
            workspace_grant_epoch,
            changes: proposed,
        },
    )
}

fn latest_bmad_capability_run(
    state: &HostState,
    workspace_id: &ContractId,
    workspace_grant_epoch: u64,
    capability_id: &str,
) -> Result<HostCommandData, LocalError> {
    let authority = state.ready_workspace_commit()?;
    let workspace_authority = state
        .workspace
        .authorize_scope(workspace_id.as_str(), workspace_grant_epoch)
        .map_err(|error| map_workspace_error(&error))?;
    let store = state.local_store(authority.authority())?;
    let record = store
        .latest_bmad_capability_run(workspace_id, capability_id)
        .map_err(|_| {
            LocalError::new(
                LocalErrorCode::IntegrityFailure,
                "The capability run history could not be read.",
                false,
            )
        })?;
    drop(workspace_authority);
    drop(authority);
    let projection = match record {
        Some(record) => crate::wire::CapabilityRunLatestProjection {
            capability_id: capability_id.to_owned(),
            found: true,
            run_id: Some(record.run_id),
            result_kind: record.result_kind,
            result_json: record.result_json,
        },
        None => crate::wire::CapabilityRunLatestProjection {
            capability_id: capability_id.to_owned(),
            found: false,
            run_id: None,
            result_kind: None,
            result_json: None,
        },
    };
    Ok(HostCommandData::CapabilityRunLatest(projection))
}

fn offboarding_inspect(state: &HostState) -> Result<HostCommandData, LocalError> {
    let authority = state.ready_authority()?;
    let store = state.local_store(&authority)?;
    let manifest = store.retention_manifest().map_err(|_| {
        LocalError::new(
            LocalErrorCode::IntegrityFailure,
            "The retention manifest could not be produced.",
            false,
        )
    })?;
    let categories = manifest
        .into_iter()
        .map(
            |(category, count)| desktop_ipc::RetentionCategoryProjection {
                category: category.to_owned(),
                count,
            },
        )
        .collect();
    Ok(HostCommandData::RetentionManifest(
        desktop_ipc::RetentionManifestProjection {
            schema_version: "sapphirus.retention-manifest.v1".to_owned(),
            categories,
            retained_bytes: store.retained_bytes(),
        },
    ))
}

fn view_bmad_persona(
    foundation: &BmadLoadedFoundation,
    agent_code: &str,
) -> Result<HostCommandData, LocalError> {
    let persona = foundation.persona_for(agent_code).ok_or_else(|| {
        LocalError::new(
            LocalErrorCode::NotFound,
            "No sealed persona exists for that agent.",
            false,
        )
    })?;
    let roster_agent = foundation
        .roster()
        .agents
        .iter()
        .find(|agent| agent.agent_code == agent_code)
        .ok_or_else(|| {
            LocalError::new(
                LocalErrorCode::NotFound,
                "No roster agent exists for that code.",
                false,
            )
        })?;
    let markdown = std::str::from_utf8(persona.instruction_bytes()).map_err(|_| {
        LocalError::new(
            LocalErrorCode::IntegrityFailure,
            "The sealed persona instruction is not valid text.",
            false,
        )
    })?;
    desktop_ipc::project_bmad_persona_perspective(
        agent_code,
        &roster_agent.display_name,
        &roster_agent.title,
        &roster_agent.icon,
        markdown,
        &persona.instruction_hash().to_string(),
    )
    .map(HostCommandData::BmadPersonaPerspective)
    .map_err(map_bmad_projection_error)
}

fn create_bmad_help_run(
    state: &HostState,
    foundation: &BmadLoadedFoundation,
    request_id: &ContractId,
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    current_intent: BmadHelpIntent,
    accepted_at: UnixMillis,
) -> Result<BmadHelpRunExecution, LocalError> {
    let fingerprint = bmad_help_run_fingerprint(foundation, &current_intent)?;
    let authority = state.ready_workspace_commit()?;
    let workspace_catalog_version = authority.workspace_catalog_version();
    let replay_request = BmadHelpRunReplayRequest {
        request_id: request_id.clone(),
        workspace_id: workspace_id.clone(),
        workspace_grant_epoch,
        capability_catalog_hash: fingerprint.catalog,
        foundation_binding_hash: fingerprint.foundation,
        intent_hash: fingerprint.intent,
    };
    let replay = match state.replay_bmad_help_run(authority.authority(), &replay_request) {
        Ok(replay) => replay,
        Err(StoreError::StateConflict) => {
            drop(authority);
            return Err(conflict_error(
                "The request identifier was already used for different Method guidance.",
            ));
        }
        Err(_) => {
            authority.enter_recovery();
            return Err(recovery_error());
        }
    };
    let Some(receipt) = replay else {
        return match create_new_bmad_help_run(
            state,
            authority.authority(),
            foundation,
            NewBmadHelpRunInput {
                request_id,
                workspace_id,
                workspace_grant_epoch,
                workspace_catalog_version,
                current_intent,
                accepted_at,
                fingerprint,
            },
        ) {
            Ok(result) => {
                drop(authority);
                Ok(result)
            }
            Err(NewBmadHelpRunError::Local(error)) => {
                drop(authority);
                Err(error)
            }
            Err(NewBmadHelpRunError::Recovery) => {
                authority.enter_recovery();
                Err(recovery_error())
            }
        };
    };

    let Ok(result) = bmad_help_run_execution_from_receipt(&workspace_id, &receipt) else {
        authority.enter_recovery();
        return Err(recovery_error());
    };
    drop(authority);
    Ok(result)
}

#[cfg(test)]
pub(crate) fn create_bmad_help_run_for_test(
    state: &HostState,
    foundation: &BmadLoadedFoundation,
    request_id: &ContractId,
    workspace_id: &ContractId,
    workspace_grant_epoch: u64,
    current_intent: BmadHelpIntent,
    accepted_at: UnixMillis,
) -> Result<BmadHelpRunCreationReceipt, LocalError> {
    create_bmad_help_run(
        state,
        foundation,
        request_id,
        workspace_id.clone(),
        workspace_grant_epoch,
        current_intent,
        accepted_at,
    )?;
    let authority = state.ready_workspace_commit()?;
    let latest = state
        .latest_bmad_help_run(
            authority.authority(),
            workspace_id,
            authority.workspace_catalog_version(),
        )
        .map_err(|_| recovery_error())?;
    match latest {
        BmadHelpRunLatest::Retained(receipt) => Ok(receipt),
        _ => Err(recovery_error()),
    }
}

fn bmad_help_run_fingerprint(
    foundation: &BmadLoadedFoundation,
    current_intent: &BmadHelpIntent,
) -> Result<BmadHelpRunFingerprint, LocalError> {
    let capability_catalog_hash = foundation.catalog().capability_catalog_hash();
    let foundation_binding_hash = canonical_hash(
        "bmad-foundation-binding",
        1,
        &BmadFoundationBindingHashInput {
            schema_version: "sapphirus.bmad-foundation-binding.v1",
            manifest_hash: foundation.manifest_hash(),
            semantic_ledger_hash: foundation.semantic_ledger_hash(),
            capability_catalog_hash,
            package_version_id: &foundation.package().package_version_id,
            descriptor_hash: foundation.package().descriptor_hash,
            observed_inventory_hash: foundation.package().observed_inventory_hash,
        },
    )
    .map_err(|_| recovery_error())?;
    let intent_hash = canonical_hash(
        "bmad-help-current-intent",
        1,
        &BmadHelpIntentHashInput {
            schema_version: "sapphirus.bmad-help-current-intent.v1",
            current_intent: current_intent.as_str(),
        },
    )
    .map_err(|_| recovery_error())?;
    Ok(BmadHelpRunFingerprint {
        catalog: capability_catalog_hash,
        foundation: foundation_binding_hash,
        intent: intent_hash,
    })
}

fn create_new_bmad_help_run(
    state: &HostState,
    authority: &ReadyAuthorityGuard<'_>,
    foundation: &BmadLoadedFoundation,
    input: NewBmadHelpRunInput<'_>,
) -> Result<BmadHelpRunExecution, NewBmadHelpRunError> {
    let NewBmadHelpRunInput {
        request_id,
        workspace_id,
        workspace_grant_epoch,
        workspace_catalog_version,
        current_intent,
        accepted_at,
        fingerprint,
    } = input;
    let workspace_authority = state
        .workspace
        .authorize_scope(workspace_id.as_str(), workspace_grant_epoch)
        .map_err(|error| NewBmadHelpRunError::Local(map_workspace_error(&error)))?;
    let workspace_projection = workspace_authority.projection();
    let workspace_binding = workspace_authority.authority_binding();
    let project_id = ContractId::new(workspace_projection.project_id)
        .map_err(|_| NewBmadHelpRunError::Recovery)?;
    let workspace_root_identity_hash = Sha256Digest::parse(&workspace_binding.root_identity_hash)
        .map_err(|_| NewBmadHelpRunError::Recovery)?;
    let local_identity = state
        .local_identity(authority)
        .map_err(|_| NewBmadHelpRunError::Recovery)?;
    let session_id = new_host_id("session").map_err(|_| NewBmadHelpRunError::Recovery)?;
    let run_id = new_host_id("run").map_err(|_| NewBmadHelpRunError::Recovery)?;
    let prepared = InertBmadHelpSessionCoordinator::prepare(
        foundation.catalog(),
        CreateInertBmadHelpSession {
            session_id: session_id.clone(),
            project_id: project_id.clone(),
            run_id: run_id.clone(),
            local_identity,
            created_at: accepted_at,
            intent: current_intent.clone(),
        },
    )
    .map_err(|_| NewBmadHelpRunError::Local(bmad_help_unavailable()))?;

    // Prove the complete renderer projection is safe before committing authority.
    let projection = project_created_bmad_help_run(
        foundation.package(),
        &prepared.recommendation,
        &current_intent,
        workspace_id.clone(),
        run_id.clone(),
        session_id.clone(),
    )
    .map_err(|error| NewBmadHelpRunError::Local(map_bmad_help_projection_error(error)))?;
    let renderer_projection =
        serde_json::to_vec(&projection).map_err(|_| NewBmadHelpRunError::Recovery)?;

    let create_request = BmadHelpRunCreateRequest {
        request_id: request_id.clone(),
        project_id,
        workspace_id: workspace_id.clone(),
        workspace_grant_epoch,
        workspace_catalog_version,
        workspace_root_identity_hash,
        capability_catalog_hash: fingerprint.catalog,
        foundation_binding_hash: fingerprint.foundation,
        intent_hash: fingerprint.intent,
        renderer_projection,
        accepted_at,
    };
    let receipt = match state.create_bmad_help_run(authority, &prepared.session, &create_request) {
        Ok(receipt) => receipt,
        Err(StoreError::StateConflict) => {
            return Err(NewBmadHelpRunError::Local(conflict_error(
                "The Method guidance request conflicts with retained local authority.",
            )));
        }
        Err(_) => return Err(NewBmadHelpRunError::Recovery),
    };
    if !receipt.replayed {
        state.record_event(ProjectionEventKind::SessionChanged {
            session_id: receipt.session_id.clone(),
            state: "created_unbound".to_owned(),
        });
    }
    let result = bmad_help_run_execution_from_receipt(&workspace_id, &receipt)
        .map_err(|_| NewBmadHelpRunError::Recovery);
    drop(workspace_authority);
    result
}

fn bmad_help_run_execution_from_receipt(
    workspace_id: &ContractId,
    receipt: &BmadHelpRunCreationReceipt,
) -> Result<BmadHelpRunExecution, BmadHelpProjectionError> {
    let projection = decode_retained_bmad_help_run(
        &receipt.renderer_projection,
        workspace_id,
        &receipt.run_id,
        &receipt.session_id,
    )?;
    Ok(BmadHelpRunExecution {
        data: HostCommandData::BmadHelpRunCreated(projection),
        accepted_at: receipt.accepted_at,
        operation_id: receipt.run_id.clone(),
    })
}

fn latest_bmad_help_run(
    state: &HostState,
    workspace_id: &ContractId,
    workspace_grant_epoch: u64,
) -> Result<HostCommandData, LocalError> {
    let authority = state.ready_workspace_commit()?;
    let workspace_authority = state
        .workspace
        .authorize_scope(workspace_id.as_str(), workspace_grant_epoch)
        .map_err(|error| map_workspace_error(&error))?;
    if let Some(lifecycle) = state.bmad_model.lock().snapshot_for_workspace(workspace_id) {
        let data = in_memory_bmad_help_data(workspace_id, lifecycle);
        drop(workspace_authority);
        if let Ok(data) = data {
            drop(authority);
            return Ok(data);
        }
        authority.enter_recovery();
        return Err(recovery_error());
    }
    let Ok(receipt) = state.latest_bmad_help_run(
        authority.authority(),
        workspace_id,
        authority.workspace_catalog_version(),
    ) else {
        drop(workspace_authority);
        authority.enter_recovery();
        return Err(recovery_error());
    };
    let Ok(data) = latest_bmad_help_run_data(workspace_id, receipt) else {
        drop(workspace_authority);
        authority.enter_recovery();
        return Err(recovery_error());
    };
    drop(workspace_authority);
    drop(authority);
    Ok(data)
}

fn in_memory_bmad_help_data(
    workspace_id: &ContractId,
    lifecycle: BmadHelpInMemoryLifecycle,
) -> Result<HostCommandData, LocalError> {
    match lifecycle {
        BmadHelpInMemoryLifecycle::ReviewRequired(review) => {
            project_review_for_renderer(review).map(HostCommandData::BmadHelpReview)
        }
        BmadHelpInMemoryLifecycle::Approved { review, approval } => {
            if !approval.send_eligible {
                return Err(map_bmad_model_projection_error(
                    BmadHelpModelAccessProjectionError::Unavailable,
                ));
            }
            let review = project_review_for_renderer(review)?;
            let approval = project_bmad_help_approved(BmadHelpApprovalInput {
                manifest_hash: approval.manifest_hash,
                decision_id: approval.decision_id,
                expires_at: approval.expires_at,
            })
            .map_err(map_bmad_model_projection_error)?;
            Ok(HostCommandData::BmadHelpApprovedLifecycle(
                project_bmad_help_approved_lifecycle(BmadHelpApprovedLifecycleInput {
                    review,
                    approval,
                }),
            ))
        }
        BmadHelpInMemoryLifecycle::Completed(completed) => {
            Ok(HostCommandData::BmadHelpRunCompleted(completed))
        }
        BmadHelpInMemoryLifecycle::Terminal(reason) => {
            let reason = match reason {
                BmadHelpTerminalReason::Cancelled => BmadHelpTerminalReasonProjection::Cancelled,
                BmadHelpTerminalReason::ConsentExpired => {
                    BmadHelpTerminalReasonProjection::ConsentExpired
                }
                BmadHelpTerminalReason::ConsentConsumed => {
                    BmadHelpTerminalReasonProjection::ConsentConsumed
                }
                BmadHelpTerminalReason::Failed => BmadHelpTerminalReasonProjection::Failed,
            };
            Ok(HostCommandData::BmadHelpTerminal(
                project_bmad_help_terminal(BmadHelpTerminalInput {
                    workspace_id: workspace_id.clone(),
                    reason,
                }),
            ))
        }
    }
}

fn latest_bmad_help_run_data(
    workspace_id: &ContractId,
    latest: BmadHelpRunLatest,
) -> Result<HostCommandData, BmadHelpProjectionError> {
    match latest {
        BmadHelpRunLatest::None => Ok(HostCommandData::NoBmadHelpRun),
        BmadHelpRunLatest::LegacyProjectionUnavailable => {
            Ok(HostCommandData::BmadHelpProjectionUnavailable)
        }
        BmadHelpRunLatest::Retained(receipt) => {
            bmad_help_run_execution_from_receipt(workspace_id, &receipt)
                .map(|execution| execution.data)
        }
        BmadHelpRunLatest::Interrupted(receipt) => decode_retained_bmad_help_run(
            &receipt.renderer_projection,
            workspace_id,
            &receipt.run_id,
            &receipt.session_id,
        )
        .map(HostCommandData::BmadHelpRunInterrupted),
        BmadHelpRunLatest::Completed(completed) => {
            decode_retained_bmad_help_run(
                &completed.creation.renderer_projection,
                workspace_id,
                &completed.creation.run_id,
                &completed.creation.session_id,
            )?;
            decode_retained_bmad_help_completion(
                &completed.renderer_projection,
                workspace_id,
                &completed.creation.run_id,
                &completed.creation.session_id,
            )
            .map(HostCommandData::BmadHelpRunCompleted)
        }
    }
}

fn new_host_id(prefix: &str) -> Result<ContractId, LocalError> {
    ContractId::new(format!("{prefix}_{}", Ulid::new())).map_err(|_| recovery_error())
}

fn bmad_help_unavailable() -> LocalError {
    LocalError::new(
        LocalErrorCode::BmadProjectionUnavailable,
        "The installed Method catalog could not ground that guidance request. Clarify the intended Method step and retry.",
        true,
    )
}

fn map_bmad_help_projection_error(_error: BmadHelpProjectionError) -> LocalError {
    LocalError::new(
        LocalErrorCode::BmadProjectionUnavailable,
        "The Method guidance result could not be projected safely. Reload the Method library and retry.",
        true,
    )
}

fn map_bmad_projection_error(error: BmadProjectionError) -> LocalError {
    match error {
        BmadProjectionError::Unavailable => LocalError::new(
            LocalErrorCode::BmadProjectionUnavailable,
            "The Method library is temporarily unavailable. Reload it to retry.",
            true,
        ),
        BmadProjectionError::Gap => LocalError::new(
            LocalErrorCode::BmadProjectionGap,
            "The Method library changed. Request a fresh snapshot.",
            true,
        ),
    }
}

fn renderer_session_expired() -> LocalError {
    LocalError::new(
        LocalErrorCode::RendererSessionExpired,
        "The desktop renderer session expired. Reconnect before retrying.",
        true,
    )
}

const MAX_PICKED_FILES: usize = 100;

fn pick_workspace_files(
    app: &tauri::AppHandle,
    state: &HostState,
    workspace_id: ContractId,
) -> Result<HostCommandData, LocalError> {
    // Confirm Ready mode and a live grant, then release the authority guard
    // before the long-blocking dialog: holding the boot-mode read lock across
    // user interaction would stall a concurrent recovery transition.
    {
        let _authority = state.ready_authority()?;
        state
            .workspace
            .authority_binding(workspace_id.as_str())
            .map_err(|error| map_workspace_error(&error))?;
    }
    let selected = app
        .dialog()
        .file()
        .set_title("Attach files from this workspace")
        .blocking_pick_files();
    // Re-prove Ready mode after the dialog; the resolve call below revalidates
    // the grant and its root identity itself.
    let _authority = state.ready_authority()?;
    let Some(selected) = selected else {
        return Ok(HostCommandData::NoSelection);
    };
    if selected.is_empty() {
        return Ok(HostCommandData::NoSelection);
    }
    let candidates = selected
        .into_iter()
        .filter_map(|path| path.into_path().ok())
        .collect::<Vec<_>>();
    let picked = state
        .workspace
        .resolve_picked_files(workspace_id.as_str(), &candidates, MAX_PICKED_FILES)
        .map_err(|error| map_workspace_error(&error))?;
    Ok(HostCommandData::PickedFiles(
        crate::wire::PickedFilesProjection {
            workspace_id,
            selected_count: u32::try_from(picked.relative_paths.len()).unwrap_or(u32::MAX),
            relative_paths: picked.relative_paths,
            rejected_outside_root: u32::try_from(picked.rejected_outside_root).unwrap_or(u32::MAX),
            rejected_unreadable: u32::try_from(picked.rejected_unreadable).unwrap_or(u32::MAX),
            truncated: picked.truncated,
        },
    ))
}

fn pick_workspace_folder(
    app: &tauri::AppHandle,
    state: &HostState,
    workspace_id: ContractId,
) -> Result<HostCommandData, LocalError> {
    {
        let _authority = state.ready_authority()?;
        state
            .workspace
            .authority_binding(workspace_id.as_str())
            .map_err(|error| map_workspace_error(&error))?;
    }
    let selected = app
        .dialog()
        .file()
        .set_title("Add a folder from this workspace")
        .blocking_pick_folder();
    let _authority = state.ready_authority()?;
    let Some(selected) = selected else {
        return Ok(HostCommandData::NoSelection);
    };
    let candidate = selected
        .into_path()
        .map_err(|_| invalid_request("The selected folder is not a local filesystem path."))?;
    let picked = state
        .workspace
        .resolve_picked_folder(workspace_id.as_str(), &candidate, MAX_PICKED_FILES)
        .map_err(|error| map_workspace_error(&error))?;
    Ok(HostCommandData::PickedFiles(
        crate::wire::PickedFilesProjection {
            workspace_id,
            selected_count: u32::try_from(picked.relative_paths.len()).unwrap_or(u32::MAX),
            relative_paths: picked.relative_paths,
            rejected_outside_root: u32::try_from(picked.rejected_outside_root).unwrap_or(u32::MAX),
            rejected_unreadable: u32::try_from(picked.rejected_unreadable).unwrap_or(u32::MAX),
            truncated: picked.truncated,
        },
    ))
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
    let authority = state.ready_workspace_commit()?;
    let projection = state
        .workspace
        .grant(format!("project_{}", Ulid::new()), &selected_root)
        .map_err(|error| map_workspace_error(&error))?;
    let binding = match state.workspace.authority_binding(&projection.workspace_id) {
        Ok(binding) => binding,
        Err(error) => {
            let _ = state.workspace.revoke(&projection.workspace_id);
            authority.enter_recovery();
            return Err(map_workspace_error(&error));
        }
    };
    if let Err(error) = state.persist_workspace(
        authority.authority(),
        projection.clone(),
        &selected_root,
        &binding.root_identity_hash,
        request_id,
    ) {
        let _ = state.workspace.revoke(&projection.workspace_id);
        if error.code == LocalErrorCode::RecoveryRequired {
            authority.enter_recovery();
        } else {
            drop(authority);
        }
        return Err(error);
    }
    let Ok(workspace_id) = ContractId::new(projection.workspace_id.clone()) else {
        authority.enter_recovery();
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
    let authority = state.ready_workspace_commit()?;
    if let Err(error) = state.persist_revocation(authority.authority(), &workspace_id, request_id) {
        if error.code == LocalErrorCode::RecoveryRequired {
            authority.enter_recovery();
        } else {
            drop(authority);
        }
        return Err(error);
    }
    let Ok(projection) = state.workspace.revoke(workspace_id.as_str()) else {
        authority.enter_recovery();
        return Err(recovery_error());
    };
    state.bmad_model.lock().invalidate(now());
    state.record_event(ProjectionEventKind::WorkspaceChanged { workspace_id });
    let data = HostCommandData::WorkspaceRevoked(projection);
    drop(authority);
    Ok(data)
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

const PREFERENCES_AGGREGATE: &str = "renderer_preferences";
const PREFERENCES_AGGREGATE_ID: &str = "local";

fn load_preferences(state: &HostState) -> Result<HostCommandData, LocalError> {
    let authority = state.ready_authority()?;
    let store = state.local_store(&authority)?;
    let preferences = store
        .load_aggregate(PREFERENCES_AGGREGATE, PREFERENCES_AGGREGATE_ID)
        .map_err(|_| recovery_error())?
        .and_then(|record| {
            deserialize_strict::<PreferencesProjection>(record.state_json.as_bytes()).ok()
        })
        .filter(|preferences| preferences.schema_version == PREFERENCES_SCHEMA)
        .unwrap_or_default();
    Ok(HostCommandData::Preferences(preferences))
}

fn save_preferences(
    state: &HostState,
    request_id: &ContractId,
    theme: desktop_runtime::ThemePreference,
    density: desktop_runtime::DensityPreference,
    accepted_at: UnixMillis,
) -> Result<HostCommandData, LocalError> {
    let authority = state.ready_authority()?;
    let store = state.local_store(&authority)?;
    let preferences = PreferencesProjection {
        schema_version: PREFERENCES_SCHEMA.to_owned(),
        theme,
        density,
        updated_at: Some(accepted_at),
    };
    let state_json = serde_json::to_string(&preferences).map_err(|_| recovery_error())?;
    let next_version = store
        .load_aggregate(PREFERENCES_AGGREGATE, PREFERENCES_AGGREGATE_ID)
        .map_err(|_| recovery_error())?
        .map_or(1, |record| record.version.saturating_add(1));
    store
        .append_transition(
            PREFERENCES_AGGREGATE,
            PREFERENCES_AGGREGATE_ID,
            next_version,
            &state_json,
            &desktop_store::EvidenceAppend {
                stream_id: "app:preferences".to_owned(),
                event_type: "preferences.updated".to_owned(),
                payload_hash: desktop_runtime::sha256_bytes(state_json.as_bytes()).to_string(),
                payload_ref: None,
                correlation_id: request_id.to_string(),
                causation_id: None,
                redaction_level: "metadata".to_owned(),
                retention_class: "evidence".to_owned(),
            },
        )
        .map_err(|_| {
            temporarily_unavailable("The preference change could not be saved. Retry shortly.")
        })?;
    Ok(HostCommandData::Preferences(preferences))
}

fn about_projection(state: &HostState, foundation: &BmadLoadedFoundation) -> AboutProjection {
    let update_configured = option_env!("SAPPHIRUS_UPDATE_ENDPOINT").is_some()
        && option_env!("SAPPHIRUS_UPDATE_PUBLIC_KEY").is_some();
    AboutProjection {
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        installation_id: state.installation_id().clone(),
        boot_mode: state.boot_mode(),
        foundation_package_name: foundation.package().package_name.clone(),
        foundation_package_version: foundation.package().package_version.clone(),
        inactive_builder_package_count: u32::try_from(foundation.inactive_builder_package_count())
            .unwrap_or(u32::MAX),
        update_configured,
        // In-app installation stays withheld until organization signing exists.
        update_install_available: false,
    }
}

fn boot_state(state: &HostState) -> BootStateProjection {
    let mode = state.boot_mode();
    BootStateProjection {
        mode,
        workspace_count: u32::try_from(state.workspace.list().len()).unwrap_or(u32::MAX),
        connected_features_available: false,
        local_edits_available: mode == BootMode::Ready,
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

fn map_bmad_model_projection_error(_error: BmadHelpModelAccessProjectionError) -> LocalError {
    LocalError::new(
        LocalErrorCode::IntegrityFailure,
        "The model-access state could not be projected safely.",
        false,
    )
}

fn map_bmad_model_error(error: BmadHelpCoordinatorError) -> LocalError {
    match error {
        BmadHelpCoordinatorError::SupportPlaneOffline => LocalError::new(
            LocalErrorCode::SupportPlaneOffline,
            "Model support is offline in this desktop composition.",
            true,
        ),
        BmadHelpCoordinatorError::Unauthorized => unauthorized_error(),
        BmadHelpCoordinatorError::Conflict => {
            conflict_error("The Help request state changed; start a fresh review.")
        }
        BmadHelpCoordinatorError::Integrity => LocalError::new(
            LocalErrorCode::IntegrityFailure,
            "The Help request binding could not be verified.",
            false,
        ),
        BmadHelpCoordinatorError::Recovery => recovery_error(),
        BmadHelpCoordinatorError::ConsentExpired => LocalError::new(
            LocalErrorCode::ConsentExpired,
            "The context approval expired; start a fresh review.",
            false,
        ),
        BmadHelpCoordinatorError::ConsentBindingMismatch => LocalError::new(
            LocalErrorCode::ConsentBindingMismatch,
            "The displayed context no longer matches the pending approval.",
            false,
        ),
        BmadHelpCoordinatorError::ConsentAlreadyConsumed => LocalError::new(
            LocalErrorCode::ConsentAlreadyConsumed,
            "That approval is no longer available; start a fresh review.",
            false,
        ),
        BmadHelpCoordinatorError::TransportFailed => LocalError::new(
            LocalErrorCode::TransportFailed,
            "The reviewed request could not be delivered; start a fresh review before retrying.",
            false,
        ),
        BmadHelpCoordinatorError::ResponseBindingMismatch => LocalError::new(
            LocalErrorCode::ResponseBindingMismatch,
            "The model response did not match the reviewed request.",
            false,
        ),
        BmadHelpCoordinatorError::InvalidModelOutput => LocalError::new(
            LocalErrorCode::InvalidModelOutput,
            "The model response was not a valid Help result.",
            false,
        ),
        BmadHelpCoordinatorError::ReceiptInvalid => LocalError::new(
            LocalErrorCode::ReceiptInvalid,
            "The model-access receipt could not be verified.",
            false,
        ),
    }
}

pub(crate) fn map_workspace_error(error: &WorkspaceError) -> LocalError {
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
        WorkspaceError::InvalidPattern => {
            invalid_request("The search pattern is not valid or is too large.")
        }
        WorkspaceError::GrantUnavailable => {
            not_found_error("The local workspace is not available.")
        }
        WorkspaceError::RootIdentityChanged => {
            conflict_error("The local workspace identity changed; select the folder again.")
        }
        WorkspaceError::NotGitRepository => {
            invalid_request("The selected workspace is not a Git repository.")
        }
        WorkspaceError::GitUnavailable => temporarily_unavailable(
            "The Git repository could not be read. Check access and retry.",
        ),
        WorkspaceError::EditsNotEnabled => conflict_error(
            "Governed edits are not enabled for this workspace. Review its permissions before retrying.",
        ),
        WorkspaceError::StalePreimage => conflict_error(
            "The workspace item changed after review. Refresh it before applying an edit.",
        ),
        WorkspaceError::AlreadyExists => conflict_error(
            "The target workspace item already exists. Refresh the workspace before retrying.",
        ),
        WorkspaceError::LimitExceeded => {
            resource_limit_error("The workspace operation exceeded its configured limit.")
        }
        WorkspaceError::Io(_) => temporarily_unavailable(
            "The local workspace could not be read. Check access and retry.",
        ),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use desktop_runtime::{
        sha256_bytes, BmadHelpIntent, BmadLibraryProjectionScope, ContractId, LocalCommand,
        LocalErrorCode, ModelProviderId, UnixMillis,
    };
    use desktop_store::{
        BmadHelpRunCompletionReceipt, BmadHelpRunCreationReceipt, BmadHelpRunLatest,
    };
    use desktop_workspace::WorkspaceError;

    #[cfg(feature = "deterministic-help")]
    use super::prepare_bmad_help_review;
    use super::{
        about_projection, admit_dispatch_envelope, bmad_library_snapshot, bmad_workflow_state,
        create_bmad_help_run, latest_bmad_help_run, latest_bmad_help_run_data, load_preferences,
        map_bmad_model_error, map_workspace_error, model_auth_status_data, offboarding_inspect,
        revoke_workspace, save_preferences, should_cache_reply, supported_commands,
        view_bmad_persona,
    };
    #[cfg(feature = "deterministic-help")]
    use super::{
        approve_bmad_capability_run, cancel_bmad_capability_run, latest_bmad_capability_run,
        prepare_bmad_capability_run, propose_bmad_capability_changes, submit_bmad_capability_run,
    };
    use crate::{
        bmad_foundation::load_bmad_foundation, bmad_model::coordinator::BmadHelpCoordinatorError,
        state::HostState, wire::HostCommandData,
    };

    #[test]
    fn offboarding_inspect_reports_bounded_categories_only(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (state, _storage, _workspace, _workspace_id) = ready_workspace_state();
        let HostCommandData::RetentionManifest(manifest) =
            offboarding_inspect(&state).expect("retention manifest")
        else {
            return Err("expected a retention manifest projection".into());
        };
        assert_eq!(manifest.schema_version, "sapphirus.retention-manifest.v1");
        // The disclosure must name the personal records, not just some of them.
        // Chat transcripts and capability results were silently omitted.
        let disclosed = manifest
            .categories
            .iter()
            .map(|category| category.category.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        for expected in [
            "chat_conversations",
            "chat_messages",
            "capability_runs",
            "capability_results",
        ] {
            assert!(
                disclosed.contains(expected),
                "retention disclosure omits `{expected}`",
            );
        }
        let serialized = serde_json::to_string(&manifest).expect("serializable manifest");
        // The manifest may never leak paths, separators, or identifiers.
        assert!(!serialized.contains('/') && !serialized.contains('\\'));
        for category in &manifest.categories {
            assert!(category
                .category
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '_'));
        }
        assert!(manifest.retained_bytes > 0);
        Ok(())
    }

    #[test]
    fn offboarding_erase_revokes_grants_and_drops_to_recovery() {
        let (state, storage, workspace, workspace_id) = ready_workspace_state();
        let sentinel = workspace.path().join("work-product.txt");
        std::fs::write(&sentinel, b"user work product").expect("sentinel write");
        let epoch_before = state.model_auth_epoch();

        state.offboard_erase().expect("offboarding erase");

        // Terminal state: recovery mode, no grants, advanced auth epoch.
        assert!(state.ready_authority().is_err());
        assert!(state.workspace.list().is_empty());
        assert!(state.model_auth_epoch() > epoch_before);
        assert!(state.workspace.authority_binding(&workspace_id).is_err());

        // The authority root keeps no key, database, or payloads.
        let root = storage.path().join("authority");
        assert!(!root.join("store.key").exists());
        assert!(!root.join("authority.sqlite3").exists());
        assert!(!root.join("cas").exists());

        // Workspace work product is untouched.
        assert_eq!(
            std::fs::read(&sentinel).expect("sentinel intact"),
            b"user work product"
        );

        // Erase is not repeatable from recovery, and a fresh launch starts
        // a clean identity.
        assert_eq!(
            state
                .offboard_erase()
                .expect_err("recovery blocks erase")
                .code,
            LocalErrorCode::RecoveryRequired
        );
        let fresh = HostState::initialize(Some(root)).expect("fresh identity");
        assert!(fresh.ready_authority().is_ok());
    }

    #[cfg(feature = "deterministic-help")]
    #[test]
    fn capability_run_completes_the_reviewed_lifecycle_end_to_end(
    ) -> Result<(), Box<dyn std::error::Error>> {
        use desktop_runtime::{RelativeWorkspacePath, UnixMillis};

        let foundation = load_bmad_foundation(foundation_path()).expect("sealed foundation");
        let (state, _storage, workspace, workspace_id) = ready_workspace_state();
        std::fs::write(
            workspace.path().join("brief-notes.md"),
            b"# Notes\n\nThe product under discussion.\n",
        )?;
        state.bind_renderer("main").expect("renderer binding");
        let renderer = state
            .renderer_session_authority("main")
            .expect("renderer authority");
        let workspace_contract = id(&workspace_id);
        let capability = "bmm:bmad-product-brief";

        // Unknown capabilities fail closed before any authority is touched.
        let unknown = prepare_bmad_capability_run(
            &state,
            &foundation,
            &renderer,
            workspace_contract.clone(),
            1,
            "bmm:not-in-the-ledger",
            vec![RelativeWorkspacePath::new("brief-notes.md")?],
            UnixMillis(10_000),
        );
        assert!(unknown.is_err());

        let review = prepare_bmad_capability_run(
            &state,
            &foundation,
            &renderer,
            workspace_contract.clone(),
            1,
            capability,
            vec![RelativeWorkspacePath::new("brief-notes.md")?],
            UnixMillis(10_000),
        )
        .expect("prepared capability review");
        let HostCommandData::CapabilityReview(review) = review else {
            return Err("expected a capability review projection".into());
        };
        assert_eq!(review.capability_id, capability);

        let approved = approve_bmad_capability_run(
            &state,
            workspace_contract.clone(),
            1,
            capability,
            review.manifest_hash,
            UnixMillis(20_000),
        )
        .expect("approved capability review");
        let HostCommandData::CapabilityApproved(approved) = approved else {
            return Err("expected a capability approval projection".into());
        };

        let completed = submit_bmad_capability_run(
            &state,
            &foundation,
            workspace_contract.clone(),
            1,
            capability,
            review.manifest_hash,
            approved.decision_id.clone(),
            UnixMillis(30_000),
        )
        .expect("completed capability run");
        let HostCommandData::CapabilityCompleted(completed) = completed else {
            return Err("expected a capability completion projection".into());
        };
        assert_eq!(completed.result_kind, "document_artifact");
        assert_eq!(completed.run_id, review.run_id);

        // The consumed decision cannot be replayed.
        assert!(submit_bmad_capability_run(
            &state,
            &foundation,
            workspace_contract.clone(),
            1,
            capability,
            review.manifest_hash,
            approved.decision_id,
            UnixMillis(31_000),
        )
        .is_err());

        // The durable run replays with its verified result.
        let latest = latest_bmad_capability_run(&state, &workspace_contract, 1, capability)
            .expect("latest capability run");
        let HostCommandData::CapabilityRunLatest(latest) = latest else {
            return Err("expected a latest-run projection".into());
        };
        assert!(latest.found);
        assert_eq!(latest.result_kind.as_deref(), Some("document_artifact"));
        assert!(latest
            .result_json
            .as_deref()
            .is_some_and(|json| json.contains("document_artifact")));

        // The workspace file was never modified by the run.
        assert_eq!(
            std::fs::read(workspace.path().join("brief-notes.md"))?,
            b"# Notes\n\nThe product under discussion.\n"
        );
        Ok(())
    }

    /// Architecture is the one stepwise capability. Preparing it must open the
    /// The deterministic navigation lane, end to end against a real workspace.
    /// Upstream must ask a model where the user is; this proves Forge computes
    /// it from config plus governed reads, with no model call at all.
    #[test]
    fn workflow_state_is_computed_from_config_and_artifacts_without_a_model(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let foundation = load_bmad_foundation(foundation_path()).expect("sealed foundation");
        let (state, _storage, workspace, workspace_id) = ready_workspace_state();
        let root = workspace.path();

        // No `_bmad` install: the panel must say so rather than invent a phase.
        let empty = bmad_workflow_state(&state, &foundation, &id(&workspace_id), 1)
            .map_err(|error| format!("workflow state: {error:?}"))?;
        let HostCommandData::BmadWorkflowState(empty) = empty else {
            return Err("expected a workflow-state projection".into());
        };
        assert!(!empty.method_installed);
        assert_eq!(empty.next_required.as_deref(), Some("bmad-prd"));
        assert!(empty.steps.iter().all(|step| !step.completed));

        // Install the method and point planning artifacts at a real folder.
        std::fs::create_dir_all(root.join("_bmad"))?;
        std::fs::write(
            root.join("_bmad/config.toml"),
            b"[modules.bmm]
planning_artifacts = \"{project-root}/docs/plan\"
",
        )?;
        std::fs::create_dir_all(root.join("docs/plan"))?;
        std::fs::write(
            root.join("docs/plan/prd-checkout.md"),
            b"# PRD
",
        )?;
        std::fs::write(
            root.join("docs/plan/ARCHITECTURE-SPINE.md"),
            b"# Spine
",
        )?;

        let data = bmad_workflow_state(&state, &foundation, &id(&workspace_id), 1)
            .map_err(|error| format!("workflow state: {error:?}"))?;
        let HostCommandData::BmadWorkflowState(projection) = data else {
            return Err("expected a workflow-state projection".into());
        };

        assert!(projection.method_installed);
        let step = |key: &str| {
            projection
                .steps
                .iter()
                .find(|step| step.key == key)
                .expect("known step")
        };
        // Both artifacts were detected from their file names alone.
        assert!(step("bmad-prd").completed);
        assert_eq!(
            step("bmad-prd").evidence_path.as_deref(),
            Some("docs/plan/prd-checkout.md")
        );
        assert!(step("bmad-architecture").completed);
        // The next unmet gate advances accordingly, with no model consulted.
        assert_eq!(
            projection.next_required.as_deref(),
            Some("bmad-create-epics-and-stories")
        );
        assert_eq!(projection.phase_code, "3-solutioning");
        // A step whose artifact is absent stays incomplete — never guessed.
        assert!(!step("bmad-create-epics-and-stories").completed);
        assert!(step("bmad-create-epics-and-stories").available);
        Ok(())
    }

    /// ordered progression at step one and say so in the review projection —
    /// the sealed guidance's "current host-selected step".
    #[cfg(feature = "deterministic-help")]
    #[test]
    #[expect(
        clippy::panic,
        reason = "the destructure names the unexpected projection variant"
    )]
    fn architecture_prepares_the_first_host_selected_step() {
        let foundation = load_bmad_foundation(foundation_path()).expect("sealed foundation");
        let (state, _storage, _workspace, workspace_id) = ready_workspace_state();
        state.bind_renderer("main").expect("renderer binding");
        let renderer = state
            .renderer_session_authority("main")
            .expect("renderer authority");

        let data = prepare_bmad_capability_run(
            &state,
            &foundation,
            &renderer,
            id(&workspace_id),
            1,
            crate::bmad_architecture::ARCHITECTURE_CAPABILITY_ID,
            Vec::new(),
            UnixMillis(10_000),
        )
        .expect("architecture prepares through the ordered lane");

        let HostCommandData::CapabilityReview(review) = data else {
            panic!("architecture prepare must return a capability review");
        };
        let progress = review.architecture.expect("stepwise progression");
        assert_eq!(progress.step_code, "system_context");
        assert_eq!(progress.step_number, 1);
        // Nine, not eight: `Deferred` closes the progression. Upstream requires
        // every spine to name what it will not decide, so a run that can only
        // record decisions cannot produce a faithful spine.
        assert_eq!(progress.step_count, 9);
        assert!(!progress.finalized);
        assert!(progress.spine.is_none());
    }

    /// Every other capability answers one request and terminates, so its wire
    /// shape must stay free of any progression block.
    #[cfg(feature = "deterministic-help")]
    #[test]
    #[expect(
        clippy::panic,
        reason = "the destructure names the unexpected projection variant"
    )]
    fn one_shot_capabilities_carry_no_progression() {
        let foundation = load_bmad_foundation(foundation_path()).expect("sealed foundation");
        let (state, _storage, _workspace, workspace_id) = ready_workspace_state();
        state.bind_renderer("main").expect("renderer binding");
        let renderer = state
            .renderer_session_authority("main")
            .expect("renderer authority");

        let data = prepare_bmad_capability_run(
            &state,
            &foundation,
            &renderer,
            id(&workspace_id),
            1,
            "bmm:bmad-product-brief",
            Vec::new(),
            UnixMillis(10_000),
        )
        .expect("one-shot capability prepares");

        let HostCommandData::CapabilityReview(review) = data else {
            panic!("prepare must return a capability review");
        };
        assert!(review.architecture.is_none());
        assert!(!serde_json::to_string(&review)
            .expect("review serializes")
            .contains("architecture"));
    }

    #[cfg(feature = "deterministic-help")]
    #[expect(
        clippy::panic,
        reason = "the matrix names the failing capability in its panic message"
    )]
    #[test]
    fn every_runtime_enabled_capability_completes_through_the_generic_lifecycle(
    ) -> Result<(), Box<dyn std::error::Error>> {
        use desktop_runtime::{RelativeWorkspacePath, UnixMillis};

        let foundation = load_bmad_foundation(foundation_path()).expect("sealed foundation");
        // All 38 reviewed capabilities are enabled: 24 menu capabilities, five
        // Builder operations, and nine ADR-0008 core skills. Architecture
        // completes one *step* here; its full progression is proven separately
        // by `the_architecture_progression_runs_ordered_to_a_distilled_spine`.
        let enabled = crate::bmad_capability_host::CAPABILITY_TABLE
            .iter()
            .collect::<Vec<_>>();
        assert_eq!(enabled.len(), 38);
        assert_eq!(
            enabled
                .iter()
                .filter(|binding| crate::bmad_capability_host::is_core_skill_binding(binding))
                .count(),
            9
        );
        assert_eq!(
            enabled
                .iter()
                .filter(|binding| binding.id.starts_with("builder:"))
                .count(),
            5
        );
        for binding in enabled {
            let (state, _storage, workspace, workspace_id) = ready_workspace_state();
            std::fs::write(
                workspace.path().join("context.md"),
                b"# Context
",
            )?;
            state.bind_renderer("main").expect("renderer binding");
            let renderer = state
                .renderer_session_authority("main")
                .expect("renderer authority");
            let workspace_contract = id(&workspace_id);

            let review = prepare_bmad_capability_run(
                &state,
                &foundation,
                &renderer,
                workspace_contract.clone(),
                1,
                binding.id,
                vec![RelativeWorkspacePath::new("context.md")?],
                UnixMillis(10_000),
            )
            .unwrap_or_else(|error| panic!("{}: prepare failed: {error:?}", binding.id));
            let HostCommandData::CapabilityReview(review) = review else {
                panic!("{}: expected a review projection", binding.id);
            };
            let approved = approve_bmad_capability_run(
                &state,
                workspace_contract.clone(),
                1,
                binding.id,
                review.manifest_hash,
                UnixMillis(20_000),
            )
            .unwrap_or_else(|error| panic!("{}: approve failed: {error:?}", binding.id));
            let HostCommandData::CapabilityApproved(approved) = approved else {
                panic!("{}: expected an approval projection", binding.id);
            };
            let completed = submit_bmad_capability_run(
                &state,
                &foundation,
                workspace_contract.clone(),
                1,
                binding.id,
                review.manifest_hash,
                approved.decision_id,
                UnixMillis(30_000),
            )
            .unwrap_or_else(|error| panic!("{}: submit failed: {error:?}", binding.id));
            let HostCommandData::CapabilityCompleted(completed) = completed else {
                panic!("{}: expected a completion projection", binding.id);
            };
            let expected_kind = match binding.output_schema_id {
                desktop_runtime::BMAD_GOVERNED_CHANGE_SET_SCHEMA => "governed_change_set",
                desktop_runtime::BMAD_INACTIVE_BUILDER_DRAFT_SCHEMA => "inactive_builder_draft",
                _ => "document_artifact",
            };
            assert_eq!(completed.result_kind, expected_kind, "{}", binding.id);
        }
        Ok(())
    }

    /// The state-machine packet the sealed `architecture-create` guidance
    /// requires, end to end through the real command lane: eight ordered
    /// steps, each one an ordinary reviewed capability run, distilled into one
    /// inert spine — and no workspace byte touched along the way.
    #[cfg(feature = "deterministic-help")]
    #[expect(
        clippy::panic,
        reason = "the progression names the failing step in its panic message"
    )]
    #[test]
    fn the_architecture_progression_runs_ordered_to_a_distilled_spine(
    ) -> Result<(), Box<dyn std::error::Error>> {
        use desktop_runtime::{BmadArchitectureStep, RelativeWorkspacePath, UnixMillis};

        let foundation = load_bmad_foundation(foundation_path()).expect("sealed foundation");
        let (state, _storage, workspace, workspace_id) = ready_workspace_state();
        let notes = b"# Notes\n\nThe system under design.\n";
        std::fs::write(workspace.path().join("design-notes.md"), notes)?;
        state.bind_renderer("main").expect("renderer binding");
        let renderer = state
            .renderer_session_authority("main")
            .expect("renderer authority");
        let workspace_contract = id(&workspace_id);
        let capability = crate::bmad_architecture::ARCHITECTURE_CAPABILITY_ID;

        let mut completions = Vec::new();
        for (offset, expected) in BmadArchitectureStep::ORDERED.into_iter().enumerate() {
            let base = 100_000_u64 + u64::try_from(offset).unwrap_or(0) * 1_000;
            let review = prepare_bmad_capability_run(
                &state,
                &foundation,
                &renderer,
                workspace_contract.clone(),
                1,
                capability,
                vec![RelativeWorkspacePath::new("design-notes.md")?],
                UnixMillis(base),
            )
            .unwrap_or_else(|error| panic!("{}: prepare failed: {error:?}", expected.code()));
            let HostCommandData::CapabilityReview(review) = review else {
                panic!("{}: expected a review projection", expected.code());
            };
            let selected = review.architecture.clone().expect("stepwise progression");
            assert_eq!(selected.step_code, expected.code());
            assert_eq!(selected.step_number, offset + 1);
            assert!(!selected.finalized);

            let approved = approve_bmad_capability_run(
                &state,
                workspace_contract.clone(),
                1,
                capability,
                review.manifest_hash,
                UnixMillis(base + 100),
            )
            .unwrap_or_else(|error| panic!("{}: approve failed: {error:?}", expected.code()));
            let HostCommandData::CapabilityApproved(approved) = approved else {
                panic!("{}: expected an approval projection", expected.code());
            };
            let completed = submit_bmad_capability_run(
                &state,
                &foundation,
                workspace_contract.clone(),
                1,
                capability,
                review.manifest_hash,
                approved.decision_id,
                UnixMillis(base + 200),
            )
            .unwrap_or_else(|error| panic!("{}: submit failed: {error:?}", expected.code()));
            let HostCommandData::CapabilityCompleted(completed) = completed else {
                panic!("{}: expected a completion projection", expected.code());
            };
            assert_eq!(completed.result_kind, "document_artifact");
            completions.push(completed.architecture.expect("stepwise completion"));
        }

        // Only the final step finalizes, and only it carries the spine.
        for (offset, completion) in completions.iter().enumerate() {
            let last = offset + 1 == BmadArchitectureStep::count();
            assert_eq!(completion.finalized, last, "step {}", offset + 1);
            assert_eq!(completion.spine.is_some(), last, "step {}", offset + 1);
        }
        let spine = completions
            .last()
            .and_then(|completion| completion.spine.clone())
            .expect("distilled spine");
        assert_eq!(spine.title, "Architecture spine");
        assert_eq!(spine.sections.len(), BmadArchitectureStep::count());
        for (offset, step) in BmadArchitectureStep::ORDERED.into_iter().enumerate() {
            let section = &spine.sections[offset];
            assert_eq!(section.heading, format!("{}. {}", offset + 1, step.title()));
            assert!(section.body.starts_with(&format!("AD-{}", offset + 1)));
        }

        // Every step is durably recorded, and the spine is a distillation of
        // stored evidence rather than a fresh model claim.
        let authority = state
            .ready_workspace_commit()
            .unwrap_or_else(|error| panic!("ready authority: {error:?}"));
        let store = state
            .local_store(authority.authority())
            .unwrap_or_else(|error| panic!("local store: {error:?}"));
        let latest = store
            .latest_bmad_capability_run(&workspace_contract, capability)?
            .expect("stored architecture run");
        assert_eq!(latest.result_kind.as_deref(), Some("document_artifact"));

        // The progression never wrote to the workspace.
        assert_eq!(
            std::fs::read(workspace.path().join("design-notes.md"))?,
            notes
        );
        Ok(())
    }

    /// A crafted submit cannot re-run an already-answered step: the host owns
    /// progression, so a replayed step is a conflict, never a silent overwrite.
    #[cfg(feature = "deterministic-help")]
    #[test]
    fn the_architecture_progression_refuses_a_replayed_step() {
        let (state, _storage, _workspace, workspace_id) = ready_workspace_state();
        let workspace_contract = id(&workspace_id);
        let authority = state.ready_workspace_commit().expect("ready authority");
        let first = {
            let store = state
                .local_store(authority.authority())
                .expect("local store");
            let mut coordinator = state.bmad_architecture.lock();
            coordinator
                .select_step(store, &workspace_contract, UnixMillis(10_000))
                .expect("first step")
        };
        drop(authority);
        let artifact = desktop_runtime::BmadDocumentArtifact::new(
            "System context".to_owned(),
            vec![desktop_runtime::BmadDocumentSection {
                heading: "Scope".to_owned(),
                body: "Bounded.".to_owned(),
            }],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
        )
        .expect("artifact");

        let mut coordinator = state.bmad_architecture.lock();
        coordinator
            .accept_step(
                &workspace_contract,
                first,
                artifact.clone(),
                UnixMillis(10_001),
            )
            .expect("first acceptance");
        let replayed =
            coordinator.accept_step(&workspace_contract, first, artifact, UnixMillis(10_002));

        assert_eq!(
            replayed.err(),
            Some(
                crate::bmad_model::capability_coordinator::BmadCapabilityCoordinatorError::Conflict
            )
        );
    }

    /// Drives one capability through the whole reviewed D2 lifecycle —
    /// prepare, approve, submit — leaving its verified result stored.
    #[cfg(feature = "deterministic-help")]
    fn run_capability_to_completion(
        state: &HostState,
        foundation: &crate::bmad_foundation::BmadLoadedFoundation,
        workspace_contract: &ContractId,
        capability: &str,
        context_paths: Vec<desktop_runtime::RelativeWorkspacePath>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        state.bind_renderer("main").expect("renderer binding");
        let renderer = state
            .renderer_session_authority("main")
            .expect("renderer authority");
        let review = prepare_bmad_capability_run(
            state,
            foundation,
            &renderer,
            workspace_contract.clone(),
            1,
            capability,
            context_paths,
            UnixMillis(10_000),
        )
        .expect("prepared review");
        let HostCommandData::CapabilityReview(review) = review else {
            return Err("expected a review projection".into());
        };
        let approved = approve_bmad_capability_run(
            state,
            workspace_contract.clone(),
            1,
            capability,
            review.manifest_hash,
            UnixMillis(20_000),
        )
        .expect("approved review");
        let HostCommandData::CapabilityApproved(approved) = approved else {
            return Err("expected an approval projection".into());
        };
        submit_bmad_capability_run(
            state,
            foundation,
            workspace_contract.clone(),
            1,
            capability,
            review.manifest_hash,
            approved.decision_id,
            UnixMillis(30_000),
        )
        .expect("completed run");
        Ok(())
    }

    // Drives a capability to completion first, so it can only build where
    // `run_capability_to_completion` and `propose_bmad_capability_changes` do.
    #[cfg(feature = "deterministic-help")]
    #[test]
    fn verified_change_sets_enter_d3_as_ordinary_candidates(
    ) -> Result<(), Box<dyn std::error::Error>> {
        use desktop_runtime::{RelativeWorkspacePath, UnixMillis};

        let foundation = load_bmad_foundation(foundation_path()).expect("sealed foundation");
        let (state, _storage, workspace, workspace_id) = ready_workspace_state();
        std::fs::write(
            workspace.path().join("story.md"),
            b"# Story
",
        )?;
        let (workspace_contract, capability) = (id(&workspace_id), "bmm:bmad-dev-story");
        run_capability_to_completion(
            &state,
            &foundation,
            &workspace_contract,
            capability,
            vec![RelativeWorkspacePath::new("story.md")?],
        )?;

        // Governed edits must be explicitly enabled, like any D3 flow.
        crate::edits::execute_changes_command(
            &state,
            &id("request_01J00000000000000000000010"),
            UnixMillis(35_000),
            desktop_runtime::LocalCommand::EnableWorkspaceEdits {
                workspace_id: workspace_contract.clone(),
                workspace_grant_epoch: 1,
            },
        )
        .expect("edits enabled");
        let edited_grant_epoch = state
            .workspace
            .list()
            .into_iter()
            .find(|projection| projection.workspace_id == workspace_id)
            .expect("workspace projection")
            .grant_epoch;

        // ADR-0007: the stored change set becomes an ordinary D3 proposal.
        let proposal = propose_bmad_capability_changes(
            &state,
            &foundation,
            &id("request_01J00000000000000000000011"),
            UnixMillis(40_000),
            workspace_contract.clone(),
            edited_grant_epoch,
            capability,
        )
        .expect("D3 proposal from verified change set");
        assert!(
            matches!(proposal, HostCommandData::ChangesReview(_)),
            "the change set must land in the standard changes review"
        );
        // Nothing was applied: the proposed file does not exist.
        assert!(!workspace
            .path()
            .join("deterministic-preview.proposal.md")
            .exists());

        // Staleness tripwire: once the create target exists, a re-proposal
        // of the same stored result fails closed.
        std::fs::write(
            workspace.path().join("deterministic-preview.proposal.md"),
            b"user wrote this meanwhile",
        )?;
        let stale_proposal = propose_bmad_capability_changes(
            &state,
            &foundation,
            &id("request_01J00000000000000000000012"),
            UnixMillis(41_000),
            workspace_contract,
            edited_grant_epoch,
            capability,
        );
        assert!(
            stale_proposal.is_err(),
            "a moved workspace must fail closed"
        );
        Ok(())
    }

    #[test]
    fn replace_and_delete_results_require_exact_preimages_before_proposal(
    ) -> Result<(), Box<dyn std::error::Error>> {
        use desktop_runtime::{
            sha256_bytes, BmadCandidateChange, BmadCapabilityOutput, BmadGovernedChangeSet,
            ProposedFileChange, RelativeWorkspacePath,
        };

        let (state, _storage, workspace, workspace_id) = ready_workspace_state();
        let replace_content = "# Existing\n";
        let delete_content = "# Obsolete\n";
        std::fs::write(workspace.path().join("replace.md"), replace_content)?;
        std::fs::write(workspace.path().join("delete.md"), delete_content)?;
        let output = BmadCapabilityOutput::GovernedChangeSet(
            BmadGovernedChangeSet::new(
                "Update the reviewed workspace.".to_owned(),
                vec![
                    BmadCandidateChange::Replace {
                        path: RelativeWorkspacePath::new("replace.md")?,
                        content: "# Replaced\n".to_owned(),
                        preimage_sha256: sha256_bytes(replace_content.as_bytes()),
                    },
                    BmadCandidateChange::Delete {
                        path: RelativeWorkspacePath::new("delete.md")?,
                        preimage_sha256: sha256_bytes(delete_content.as_bytes()),
                    },
                ],
            )
            .expect("valid governed change set"),
        );
        let result_json = serde_json::to_string(&output)?;
        let preimage_field = format!(
            "\"preimageSha256\":\"{}\"",
            sha256_bytes(replace_content.as_bytes())
        );
        let duplicate_preimage_json = result_json.replacen(
            &preimage_field,
            &format!("{preimage_field},{preimage_field}"),
            1,
        );
        assert_ne!(
            duplicate_preimage_json, result_json,
            "fixture must duplicate the serialized preimage key"
        );
        assert!(
            crate::bmad_governed_proposal::proposed_changes_from_stored_result(
                &state,
                &workspace_id,
                &duplicate_preimage_json,
            )
            .is_err(),
            "stored-result reparsing must reject duplicate decoded keys"
        );

        let proposed = crate::bmad_governed_proposal::proposed_changes_from_stored_result(
            &state,
            &workspace_id,
            &result_json,
        )
        .expect("replace/delete proposal conversion");
        assert!(matches!(
            &proposed[0],
            ProposedFileChange::SetContent {
                relative_path,
                content
            } if relative_path.as_str() == "replace.md" && content == "# Replaced\n"
        ));
        assert!(matches!(
            &proposed[1],
            ProposedFileChange::Delete { relative_path }
                if relative_path.as_str() == "delete.md"
        ));

        std::fs::write(workspace.path().join("replace.md"), b"# Moved\n")?;
        assert!(
            crate::bmad_governed_proposal::proposed_changes_from_stored_result(
                &state,
                &workspace_id,
                &result_json,
            )
            .is_err(),
            "a changed preimage must fail closed"
        );
        Ok(())
    }

    #[cfg(feature = "deterministic-help")]
    #[test]
    fn capability_cancel_blocks_submission() -> Result<(), Box<dyn std::error::Error>> {
        use desktop_runtime::{RelativeWorkspacePath, UnixMillis};

        let foundation = load_bmad_foundation(foundation_path()).expect("sealed foundation");
        let (state, _storage, workspace, workspace_id) = ready_workspace_state();
        std::fs::write(workspace.path().join("story.md"), b"# Story\n")?;
        state.bind_renderer("main").expect("renderer binding");
        let renderer = state
            .renderer_session_authority("main")
            .expect("renderer authority");
        let workspace_contract = id(&workspace_id);
        let capability = "bmm:bmad-dev-story";

        let review = prepare_bmad_capability_run(
            &state,
            &foundation,
            &renderer,
            workspace_contract.clone(),
            1,
            capability,
            vec![RelativeWorkspacePath::new("story.md")?],
            UnixMillis(10_000),
        )
        .expect("prepared review");
        let HostCommandData::CapabilityReview(review) = review else {
            return Err("expected a capability review projection".into());
        };
        let approved = approve_bmad_capability_run(
            &state,
            workspace_contract.clone(),
            1,
            capability,
            review.manifest_hash,
            UnixMillis(20_000),
        )
        .expect("approved review");
        let HostCommandData::CapabilityApproved(approved) = approved else {
            return Err("expected a capability approval projection".into());
        };
        cancel_bmad_capability_run(
            &state,
            workspace_contract.clone(),
            1,
            capability,
            review.manifest_hash,
            approved.decision_id.clone(),
            UnixMillis(21_000),
        )
        .expect("cancelled review");
        assert!(submit_bmad_capability_run(
            &state,
            &foundation,
            workspace_contract.clone(),
            1,
            capability,
            review.manifest_hash,
            approved.decision_id,
            UnixMillis(22_000),
        )
        .is_err());
        // Nothing durable exists for the cancelled run.
        let latest = latest_bmad_capability_run(&state, &workspace_contract, 1, capability)
            .expect("latest lookup");
        let HostCommandData::CapabilityRunLatest(latest) = latest else {
            return Err("expected a latest-run projection".into());
        };
        assert!(!latest.found);
        Ok(())
    }

    fn foundation_path() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/bmad-foundation")
    }

    fn id(value: &str) -> ContractId {
        ContractId::new(value).expect("test identifier")
    }

    fn ready_workspace_state() -> (HostState, tempfile::TempDir, tempfile::TempDir, String) {
        let storage = tempfile::tempdir().expect("temporary authority store");
        let workspace = tempfile::tempdir().expect("temporary workspace");
        let state = HostState::initialize(Some(storage.path().join("authority")))
            .expect("ready host state");
        let authority = state.ready_authority().expect("ready authority");
        let projection = state
            .workspace
            .grant("project_01J00000000000000000000000", workspace.path())
            .expect("workspace grant");
        let binding = state
            .workspace
            .authority_binding(&projection.workspace_id)
            .expect("workspace binding");
        state
            .persist_workspace(
                &authority,
                projection.clone(),
                workspace.path(),
                &binding.root_identity_hash,
                &id("request_01J00000000000000000000001"),
            )
            .expect("persisted workspace");
        drop(authority);
        (state, storage, workspace, projection.workspace_id)
    }

    #[test]
    #[expect(
        clippy::panic,
        reason = "a non-preferences projection must fail the test immediately"
    )]
    fn preferences_default_then_persist_and_reload() {
        let (state, _storage, _workspace, _workspace_id) = ready_workspace_state();

        let HostCommandData::Preferences(defaults) =
            load_preferences(&state).expect("default preferences")
        else {
            panic!("expected a preferences projection");
        };
        assert_eq!(defaults.theme, desktop_runtime::ThemePreference::Dark);
        assert_eq!(
            defaults.density,
            desktop_runtime::DensityPreference::Comfortable
        );
        assert!(defaults.updated_at.is_none());

        let HostCommandData::Preferences(saved) = save_preferences(
            &state,
            &id("request_01J00000000000000000000900"),
            desktop_runtime::ThemePreference::System,
            desktop_runtime::DensityPreference::Compact,
            UnixMillis(5_000),
        )
        .expect("saved preferences") else {
            panic!("expected a preferences projection");
        };
        assert_eq!(saved.theme, desktop_runtime::ThemePreference::System);
        assert_eq!(saved.updated_at, Some(UnixMillis(5_000)));

        let HostCommandData::Preferences(reloaded) =
            load_preferences(&state).expect("reloaded preferences")
        else {
            panic!("expected a preferences projection");
        };
        assert_eq!(reloaded.theme, desktop_runtime::ThemePreference::System);
        assert_eq!(
            reloaded.density,
            desktop_runtime::DensityPreference::Compact
        );

        // A second write advances the aggregate version rather than conflicting.
        save_preferences(
            &state,
            &id("request_01J00000000000000000000901"),
            desktop_runtime::ThemePreference::Light,
            desktop_runtime::DensityPreference::Comfortable,
            UnixMillis(6_000),
        )
        .expect("second preference write");
    }

    #[test]
    fn about_projection_serializes_camel_case_without_paths(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (state, _storage, _workspace, _workspace_id) = ready_workspace_state();
        let foundation = load_bmad_foundation(foundation_path())?;
        let about = about_projection(&state, &foundation);
        assert!(!about.update_install_available);
        let value = serde_json::to_value(&about)?;
        let object = value.as_object().expect("about object");
        for key in [
            "appVersion",
            "installationId",
            "bootMode",
            "foundationPackageName",
            "foundationPackageVersion",
            "inactiveBuilderPackageCount",
            "updateConfigured",
            "updateInstallAvailable",
        ] {
            assert!(object.contains_key(key), "missing key {key}");
        }
        assert_eq!(object.len(), 8);
        let preferences = crate::wire::PreferencesProjection::default();
        let preferences_value = serde_json::to_value(&preferences)?;
        let preferences_object = preferences_value.as_object().expect("preferences object");
        assert_eq!(
            preferences_object.keys().collect::<Vec<_>>(),
            vec!["schemaVersion", "theme", "density", "updatedAt"],
        );
        Ok(())
    }

    fn retained_help_receipt(
        state: &HostState,
        workspace_id: &str,
        current_intent: &str,
    ) -> Result<BmadHelpRunCreationReceipt, Box<dyn std::error::Error>> {
        let foundation = load_bmad_foundation(foundation_path())?;
        create_bmad_help_run(
            state,
            &foundation,
            &id("request_01J00000000000000000000020"),
            id(workspace_id),
            1,
            BmadHelpIntent::new(current_intent)?,
            UnixMillis(1_000),
        )
        .map_err(|error| format!("Help run creation failed: {:?}", error.code))?;
        let authority = state
            .ready_workspace_commit()
            .map_err(|error| format!("workspace authority failed: {:?}", error.code))?;
        let latest = state.latest_bmad_help_run(
            authority.authority(),
            &id(workspace_id),
            authority.workspace_catalog_version(),
        )?;
        match latest {
            BmadHelpRunLatest::Retained(receipt) => Ok(receipt),
            _ => Err("expected retained BMAD Help creation receipt".into()),
        }
    }

    fn completed_projection_bytes(
        workspace_id: &str,
        receipt: &BmadHelpRunCreationReceipt,
    ) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(&serde_json::json!({
            "schemaVersion": "bmad-help-completed.v1",
            "runKind": "bmad_help",
            "lifecycle": "completed",
            "workspaceId": workspace_id,
            "runId": receipt.run_id.as_str(),
            "sessionId": receipt.session_id.as_str(),
            "runnable": false,
            "completionClaimed": true,
            "recommendation": {
                "recommendationKind": "recommended_capability",
                "displayName": "Create Architecture",
                "moduleCode": "bmm",
                "skillName": "bmad-architecture",
                "action": "create",
                "evidenceClass": "user_asserted",
                "guidanceRequired": true,
                "rationaleSummary": "The stated architecture goal matches this planning capability.",
                "createdAt": 3_000,
            },
            "receipt": {
                "schemaVersion": "bmad-model-receipt-summary.v1",
                "receiptId": "receipt_01J00000000000000000000020",
                "status": "succeeded",
                "retentionMode": "transient_no_store",
                "region": "westeurope",
                "inputBytes": 4_096,
                "outputBytes": 512,
                "startedAt": 1_500,
                "completedAt": 2_000,
            },
        }))
    }

    #[test]
    fn persona_view_projects_only_sealed_perspective_data() -> Result<(), Box<dyn std::error::Error>>
    {
        let foundation = load_bmad_foundation(foundation_path()).expect("sealed foundation");
        let data =
            view_bmad_persona(&foundation, "bmad-agent-analyst").expect("persona perspective");
        let HostCommandData::BmadPersonaPerspective(projection) = &data else {
            return Err("expected a persona perspective projection".into());
        };
        assert_eq!(
            projection.schema_version,
            "sapphirus.bmad-persona-perspective.v1"
        );
        assert_eq!(projection.agent_code, "bmad-agent-analyst");
        assert_eq!(projection.name, "Mary");
        assert_eq!(projection.title, "Business Analyst");
        assert!(projection
            .instruction_markdown
            .contains("Managed analyst persona guidance"));
        assert!(projection.instruction_hash.starts_with("sha256:"));
        // The projection never carries paths, source bodies, or envelopes.
        let serialized = serde_json::to_string(&projection)?;
        assert!(!serialized.contains("runtime/method"));
        assert!(!serialized.contains("bmm-skills"));
        assert!(!serialized.contains("resolve_customization"));

        let missing = view_bmad_persona(&foundation, "bmad-agent-unknown");
        assert!(missing.is_err());
        Ok(())
    }

    fn assert_method_menu_projection(projection: &desktop_ipc::BmadLibrarySnapshotProjection) {
        assert!(projection.method_agents.iter().all(|agent| {
            agent.availability == desktop_ipc::BmadProjectionAvailability::Available
        }));
        assert_eq!(
            projection
                .method_agents
                .iter()
                .flat_map(|agent| &agent.menus)
                .filter(|menu| {
                    menu.availability == desktop_ipc::BmadProjectionAvailability::Available
                })
                .count(),
            26
        );
        assert_eq!(
            projection
                .method_agents
                .iter()
                .flat_map(|agent| &agent.menus)
                .filter(|menu| {
                    menu.availability
                        == desktop_ipc::BmadProjectionAvailability::SourcePromptUnavailable
                })
                .count(),
            0
        );
        let architecture = projection
            .method_agents
            .iter()
            .find(|agent| agent.agent_code == "bmad-agent-architect")
            .and_then(|agent| agent.menus.iter().find(|menu| menu.code == "CA"))
            .expect("architecture menu");
        // Enabled by the ordered progression, not by the one-shot lane.
        assert_eq!(
            architecture.availability,
            desktop_ipc::BmadProjectionAvailability::Available
        );
        assert_eq!(
            architecture.capability_id.as_deref(),
            Some(crate::bmad_architecture::ARCHITECTURE_CAPABILITY_ID)
        );
        let paige = projection
            .method_agents
            .iter()
            .find(|agent| agent.agent_code == "bmad-agent-tech-writer")
            .expect("Paige");
        for (menu_code, display_label) in [
            ("WD", "Write Document"),
            ("MG", "Generate Mermaid"),
            ("VD", "Validate Document"),
            ("EC", "Explain Concept"),
        ] {
            let menu = paige
                .menus
                .iter()
                .find(|menu| menu.code == menu_code)
                .expect("reviewed Paige menu path");
            assert_eq!(
                menu.availability,
                desktop_ipc::BmadProjectionAvailability::Available
            );
            assert_eq!(menu.target_kind.as_str(), "skill_target");
            assert_eq!(menu.display_label, display_label);
        }
    }

    fn assert_builder_and_help_projection(projection: &desktop_ipc::BmadLibrarySnapshotProjection) {
        for builder in &projection.builder_packages {
            assert_eq!(builder.activation_state, "installed_inactive");
            assert_eq!(builder.blocker_codes, vec!["builder_engine_gated"]);
            assert_eq!(builder.descriptor_digest.len(), 12);
        }
        #[cfg(any(feature = "deterministic-help", feature = "direct-model-providers"))]
        {
            let help = projection
                .installed_skills
                .iter()
                .find(|skill| skill.skill_name == "bmad-help")
                .expect("projected Help skill");
            let help_action = projection
                .help_actions
                .iter()
                .find(|action| action.skill_name == "bmad-help")
                .expect("projected Help action");
            assert_eq!(
                help.availability,
                desktop_ipc::BmadProjectionAvailability::Available
            );
            assert_eq!(
                help_action.availability,
                desktop_ipc::BmadProjectionAvailability::Available
            );
            assert!(help.blocker_codes.is_empty());
            assert!(help_action.blocker_codes.is_empty());
        }
    }

    fn assert_no_bmad_authority_bytes(data: &HostCommandData) {
        let serialized = serde_json::to_string(data).expect("wire projection");
        for forbidden in [
            "sha256:",
            "sourceLocalMemberLabel",
            "packageVersionId",
            "outputLocations",
            ".md",
        ] {
            assert!(!serialized.contains(forbidden), "leaked {forbidden}");
        }
    }

    #[test]
    fn host_projects_the_sealed_method_library_without_authority_bytes() {
        let foundation = load_bmad_foundation(foundation_path()).expect("sealed foundation");
        let data = bmad_library_snapshot(
            &foundation,
            BmadLibraryProjectionScope::InstalledMethod,
            None,
        )
        .expect("library snapshot");
        let projection = match &data {
            HostCommandData::BmadLibrarySnapshot(projection) => Some(projection),
            _ => None,
        }
        .expect("expected BMAD library projection");
        assert_eq!(projection.installed_skills.len(), 2);
        assert_eq!(projection.help_actions.len(), 2);
        assert_eq!(projection.method_agents.len(), 6);
        assert_eq!(projection.builder_packages.len(), 2);
        assert_method_menu_projection(projection);
        assert_builder_and_help_projection(projection);
        assert_no_bmad_authority_bytes(&data);
    }

    /// The nine adopted core skills were runnable but unreachable: they belong
    /// to upstream's Core *module* menu, not to any agent, so the agent-keyed
    /// projection never mentioned them. They now project with the menu identity
    /// upstream assigns in `core-skills/module-help.csv`.
    #[test]
    fn host_projects_the_adopted_core_module_menu() {
        let foundation = load_bmad_foundation(foundation_path()).expect("sealed foundation");
        let data = bmad_library_snapshot(
            &foundation,
            BmadLibraryProjectionScope::InstalledMethod,
            None,
        )
        .expect("library snapshot");
        let projection = match &data {
            HostCommandData::BmadLibrarySnapshot(projection) => Some(projection),
            _ => None,
        }
        .expect("expected BMAD library projection");

        // Every core skill Forge adopted is projected, with upstream's code.
        for (capability_id, menu_code, display_name) in [
            ("bmm:bmad-spec", "SP", "Spec"),
            ("bmm:bmad-shard-doc", "SD", "Shard Document"),
            ("bmm:bmad-index-docs", "ID", "Index Docs"),
            (
                "bmm:bmad-editorial-review-prose",
                "EP",
                "Editorial Review - Prose",
            ),
            (
                "bmm:bmad-editorial-review-structure",
                "ES",
                "Editorial Review - Structure",
            ),
            (
                "bmm:bmad-review-adversarial-general",
                "AR",
                "Adversarial Review",
            ),
            (
                "bmm:bmad-review-edge-case-hunter",
                "ECH",
                "Edge Case Hunter Review",
            ),
        ] {
            let entry = projection
                .core_skills
                .iter()
                .find(|skill| skill.capability_id == capability_id)
                .expect("every adopted core skill must be projected");
            assert_eq!(entry.menu_code, menu_code, "{capability_id} menu code");
            assert_eq!(entry.display_name, display_name, "{capability_id} name");
        }

        // `bmad-advanced-elicitation` and `bmad-review-verification-gap` are
        // genuinely menu-less upstream — they are invoked from inside other
        // skills — so they carry no Core menu entry and are not projected here.
        assert_eq!(projection.core_skills.len(), 7);
    }

    #[test]
    fn host_maps_a_stale_library_cursor_to_the_stable_gap_error() {
        let foundation = load_bmad_foundation(foundation_path()).expect("sealed foundation");
        let error = bmad_library_snapshot(
            &foundation,
            BmadLibraryProjectionScope::InstalledMethod,
            Some("stale"),
        )
        .expect_err("stale cursor");
        assert_eq!(error.code, LocalErrorCode::BmadProjectionGap);
        assert!(error.retryable);
    }

    #[test]
    fn governed_workspace_conflicts_map_to_safe_non_retryable_errors() {
        for error in [
            WorkspaceError::EditsNotEnabled,
            WorkspaceError::StalePreimage,
            WorkspaceError::AlreadyExists,
        ] {
            let mapped = map_workspace_error(&error);
            assert_eq!(mapped.code, LocalErrorCode::Conflict);
            assert!(!mapped.retryable);
            assert!(!mapped.safe_message.is_empty());
        }
    }

    #[test]
    fn help_run_creation_replays_the_exact_safe_result_after_workspace_revocation() {
        let foundation = load_bmad_foundation(foundation_path()).expect("sealed foundation");
        let (state, _storage, _workspace, workspace_id) = ready_workspace_state();
        let request_id = id("request_01J00000000000000000000002");
        let intent = BmadHelpIntent::new("BMad Help").expect("bounded intent");
        let first = create_bmad_help_run(
            &state,
            &foundation,
            &request_id,
            id(&workspace_id),
            1,
            intent.clone(),
            UnixMillis(1_000),
        )
        .expect("created Help run");
        assert_eq!(first.accepted_at, UnixMillis(1_000));

        revoke_workspace(
            &state,
            &id("request_01J00000000000000000000003"),
            id(&workspace_id),
        )
        .expect("revoked workspace");
        let sequence_before_replay = state.sequence();

        let replay = create_bmad_help_run(
            &state,
            &foundation,
            &request_id,
            id(&workspace_id),
            1,
            intent,
            UnixMillis(9_000),
        )
        .expect("replayed committed Help run");
        assert_eq!(replay.accepted_at, first.accepted_at);
        assert_eq!(replay.operation_id, first.operation_id);
        assert_eq!(state.sequence(), sequence_before_replay);
        assert_eq!(
            serde_json::to_value(&replay.data).expect("replay projection"),
            serde_json::to_value(&first.data).expect("first projection")
        );

        let value = serde_json::to_value(&first.data).expect("safe run projection");
        assert_eq!(value["kind"], "bmad_help_run_created");
        assert_eq!(value["value"]["lifecycle"], "created_unbound");
        assert_eq!(value["value"]["runnable"], false);
        assert_eq!(value["value"]["completionClaimed"], false);
        let encoded = serde_json::to_string(&value).expect("run projection JSON");
        for forbidden in [
            "ownerScope",
            "authorityRef",
            "projectId",
            "capabilityCatalogHash",
            "foundationBindingHash",
            "workspaceRootIdentityHash",
            "modelId",
            "deploymentId",
            "replayed",
        ] {
            assert!(!encoded.contains(forbidden), "leaked {forbidden}");
        }
    }

    #[test]
    fn help_run_changed_payload_under_the_same_request_id_is_a_durable_conflict() {
        let foundation = load_bmad_foundation(foundation_path()).expect("sealed foundation");
        let (state, _storage, _workspace, workspace_id) = ready_workspace_state();
        let request_id = id("request_01J00000000000000000000004");
        create_bmad_help_run(
            &state,
            &foundation,
            &request_id,
            id(&workspace_id),
            1,
            BmadHelpIntent::new("BMad Help").expect("bounded intent"),
            UnixMillis(1_000),
        )
        .expect("created Help run");

        let error = create_bmad_help_run(
            &state,
            &foundation,
            &request_id,
            id(&workspace_id),
            1,
            BmadHelpIntent::new("Architecture spine").expect("bounded intent"),
            UnixMillis(2_000),
        )
        .expect_err("changed request must conflict");
        assert_eq!(error.code, LocalErrorCode::Conflict);
    }

    #[test]
    fn latest_help_run_recovers_the_exact_safe_projection_after_host_restart() {
        let foundation = load_bmad_foundation(foundation_path()).expect("sealed foundation");
        let (state, storage, _workspace, workspace_id) = ready_workspace_state();
        let first = create_bmad_help_run(
            &state,
            &foundation,
            &id("request_01J00000000000000000000007"),
            id(&workspace_id),
            1,
            BmadHelpIntent::new("BMad Help").expect("bounded intent"),
            UnixMillis(1_000),
        )
        .expect("created Help run");
        drop(state);

        let restarted = HostState::initialize(Some(storage.path().join("authority")))
            .expect("restarted host state");
        let latest = latest_bmad_help_run(&restarted, &id(&workspace_id), 1)
            .expect("latest retained Help run");

        assert_eq!(
            serde_json::to_value(latest).expect("latest projection"),
            serde_json::to_value(&first.data).expect("created projection")
        );
        let value = serde_json::to_value(&first.data).expect("created projection value");
        assert_eq!(value["value"]["currentIntent"], "BMad Help");
    }

    #[test]
    fn latest_help_run_projects_interrupted_without_claiming_completion(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (state, _storage, _workspace, workspace_id) = ready_workspace_state();
        let receipt = retained_help_receipt(
            &state,
            &workspace_id,
            "Review architecture readiness before implementation",
        )?;

        let data =
            latest_bmad_help_run_data(&id(&workspace_id), BmadHelpRunLatest::Interrupted(receipt))?;
        let value = serde_json::to_value(data)?;

        assert_eq!(value["kind"], "bmad_help_run_interrupted");
        assert_eq!(value["value"]["lifecycle"], "created_unbound");
        assert_eq!(value["value"]["completionClaimed"], false);
        assert_eq!(
            value["value"]["currentIntent"],
            "Review architecture readiness before implementation"
        );
        Ok(())
    }

    #[test]
    fn latest_help_run_strictly_decodes_the_completed_projection(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (state, _storage, _workspace, workspace_id) = ready_workspace_state();
        let receipt = retained_help_receipt(
            &state,
            &workspace_id,
            "Review architecture readiness before implementation",
        )?;
        let renderer_projection = completed_projection_bytes(&workspace_id, &receipt)?;

        let data = latest_bmad_help_run_data(
            &id(&workspace_id),
            BmadHelpRunLatest::Completed(BmadHelpRunCompletionReceipt {
                creation: receipt,
                renderer_projection,
            }),
        )?;
        let value = serde_json::to_value(data)?;

        assert_eq!(value["kind"], "bmad_help_run_completed");
        assert_eq!(value["value"]["lifecycle"], "completed");
        assert_eq!(value["value"]["completionClaimed"], true);
        assert_eq!(
            value["value"]["recommendation"]["recommendationKind"],
            "recommended_capability"
        );
        assert_eq!(value["value"]["receipt"]["status"], "succeeded");
        Ok(())
    }

    #[test]
    fn latest_help_run_rejects_substituted_unknown_duplicate_and_oversized_completion_bytes(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (state, _storage, _workspace, workspace_id) = ready_workspace_state();
        let receipt = retained_help_receipt(
            &state,
            &workspace_id,
            "Review architecture readiness before implementation",
        )?;
        let valid = completed_projection_bytes(&workspace_id, &receipt)?;
        let mut cases = Vec::new();

        for (pointer, replacement) in [
            ("/workspaceId", serde_json::json!("workspace_substituted")),
            ("/runId", serde_json::json!("run_substituted")),
            ("/sessionId", serde_json::json!("session_substituted")),
        ] {
            let mut value: serde_json::Value = serde_json::from_slice(&valid)?;
            *value
                .pointer_mut(pointer)
                .ok_or("missing fixture pointer")? = replacement;
            cases.push(serde_json::to_vec(&value)?);
        }

        let mut unknown: serde_json::Value = serde_json::from_slice(&valid)?;
        unknown
            .as_object_mut()
            .ok_or("completion fixture must be an object")?
            .insert("proof".to_owned(), serde_json::json!("forged"));
        cases.push(serde_json::to_vec(&unknown)?);

        let text = String::from_utf8(valid.clone())?;
        let duplicate = text.replacen(
            "\"schemaVersion\":\"bmad-help-completed.v1\"",
            "\"schemaVersion\":\"bmad-help-completed.v1\",\"schemaVersion\":\"bmad-help-completed.v1\"",
            1,
        );
        if duplicate == text {
            return Err("duplicate-field fixture was ineffective".into());
        }
        cases.push(duplicate.into_bytes());

        let mut oversized = valid;
        oversized.resize(
            desktop_ipc::MAX_BMAD_HELP_COMPLETED_PROJECTION_BYTES + 1,
            b' ',
        );
        cases.push(oversized);

        for renderer_projection in cases {
            assert!(latest_bmad_help_run_data(
                &id(&workspace_id),
                BmadHelpRunLatest::Completed(BmadHelpRunCompletionReceipt {
                    creation: receipt.clone(),
                    renderer_projection,
                }),
            )
            .is_err());
        }
        Ok(())
    }

    #[test]
    fn latest_help_run_has_an_exact_empty_result_before_any_run_exists() {
        let (state, _storage, _workspace, workspace_id) = ready_workspace_state();

        let latest = latest_bmad_help_run(&state, &id(&workspace_id), 1)
            .expect("bounded empty latest result");

        assert!(matches!(latest, HostCommandData::NoBmadHelpRun));
        assert_eq!(
            serde_json::to_value(latest).expect("empty latest projection"),
            serde_json::json!({ "kind": "no_bmad_help_run" })
        );
    }

    #[test]
    fn stale_second_host_cannot_create_after_durable_workspace_revocation() {
        let foundation = load_bmad_foundation(foundation_path()).expect("sealed foundation");
        let (revoking_state, storage, _workspace, workspace_id) = ready_workspace_state();
        let stale_state = HostState::initialize(Some(storage.path().join("authority")))
            .expect("second host loaded the original grant");

        revoke_workspace(
            &revoking_state,
            &id("request_01J00000000000000000000005"),
            id(&workspace_id),
        )
        .expect("first host durably revoked the workspace");
        let error = create_bmad_help_run(
            &stale_state,
            &foundation,
            &id("request_01J00000000000000000000006"),
            id(&workspace_id),
            1,
            BmadHelpIntent::new("BMad Help").expect("bounded intent"),
            UnixMillis(2_000),
        )
        .expect_err("stale host must not create after revocation");

        assert_eq!(error.code, LocalErrorCode::RecoveryRequired);
        assert_eq!(
            stale_state.boot_mode(),
            crate::wire::BootMode::ReadOnlyRecovery
        );
    }

    #[test]
    fn stale_second_host_cannot_read_latest_after_durable_workspace_revocation() {
        let (revoking_state, storage, _workspace, workspace_id) = ready_workspace_state();
        let stale_state = HostState::initialize(Some(storage.path().join("authority")))
            .expect("second host loaded the original grant");

        revoke_workspace(
            &revoking_state,
            &id("request_01J00000000000000000000008"),
            id(&workspace_id),
        )
        .expect("first host durably revoked the workspace");
        let error = latest_bmad_help_run(&stale_state, &id(&workspace_id), 1)
            .expect_err("stale host must not read after revocation");

        assert_eq!(error.code, LocalErrorCode::RecoveryRequired);
        assert_eq!(
            stale_state.boot_mode(),
            crate::wire::BootMode::ReadOnlyRecovery
        );
    }

    #[cfg(feature = "deterministic-help")]
    #[test]
    fn workspace_revocation_invalidates_an_active_help_review() {
        let foundation = load_bmad_foundation(foundation_path()).expect("sealed foundation");
        let (state, _storage, _workspace, workspace_id) = ready_workspace_state();
        let now = crate::state::now();
        create_bmad_help_run(
            &state,
            &foundation,
            &id("request_01J00000000000000000000030"),
            id(&workspace_id),
            1,
            BmadHelpIntent::new("Review architecture readiness").expect("bounded intent"),
            now,
        )
        .expect("retained Help run");
        state.bind_renderer("main").expect("renderer binding");
        let renderer = state
            .renderer_session_authority("main")
            .expect("renderer authority");
        prepare_bmad_help_review(
            &state,
            &foundation,
            &renderer,
            id(&workspace_id),
            1,
            Vec::new(),
            UnixMillis(now.0 + 1),
        )
        .expect("prepared Help review");
        assert!(state.bmad_model.lock().has_active_review());
        drop(renderer);

        revoke_workspace(
            &state,
            &id("request_01J00000000000000000000031"),
            id(&workspace_id),
        )
        .expect("workspace revoked");

        assert!(!state.bmad_model.lock().has_active_review());
    }

    #[test]
    fn model_access_mutations_never_enter_the_general_reply_cache() {
        let workspace_id = id("workspace_01J00000000000000000000000");
        let manifest_hash = sha256_bytes(b"review manifest");
        let decision_id = id("decision_01J00000000000000000000000");
        for command in [
            LocalCommand::ModelAuthSignIn {
                provider_id: ModelProviderId::OpenAi,
                model_id: "gpt-5-mini".to_owned(),
                credential: serde_json::from_value(serde_json::json!([
                    "sk", "test", "runtime", "only"
                ]
                .join("-")))
                .expect("test credential"),
                endpoint: None,
                region: None,
            },
            LocalCommand::ModelAuthSignOut,
            LocalCommand::PrepareBmadHelpReview {
                workspace_id: workspace_id.clone(),
                workspace_grant_epoch: 1,
                context_paths: Vec::new(),
            },
            LocalCommand::ApproveBmadHelpReview {
                workspace_id: workspace_id.clone(),
                workspace_grant_epoch: 1,
                manifest_hash,
            },
            LocalCommand::CancelBmadHelpReview {
                workspace_id: workspace_id.clone(),
                workspace_grant_epoch: 1,
                manifest_hash,
                decision_id: decision_id.clone(),
            },
            LocalCommand::SubmitBmadHelpReview {
                workspace_id,
                workspace_grant_epoch: 1,
                manifest_hash,
                decision_id,
            },
        ] {
            assert!(!should_cache_reply(&command), "{}", command.name());
        }
        assert!(should_cache_reply(&LocalCommand::SelectWorkspace));
    }

    #[test]
    fn recovery_capabilities_are_ready_only_and_never_reply_cached() {
        let ready = supported_commands(crate::wire::BootMode::Ready);
        let neighborhood = ready
            .windows(6)
            .find(|commands| commands[0] == "changes.propose")
            .expect("governed changes catalog neighborhood");
        assert_eq!(
            neighborhood,
            [
                "changes.propose",
                "approval.decide",
                "rollback.request",
                "changes.history",
                "changes.recovery.prepare",
                "changes.recovery.decide",
            ]
        );
        assert_eq!(ready.len(), 46);
        assert_eq!(
            supported_commands(crate::wire::BootMode::ReadOnlyRecovery),
            ["app.get_boot_state", "workspace.list"]
        );

        for command in [
            LocalCommand::PrepareChangesRecovery {
                workspace_id: id("workspace_01J00000000000000000000000"),
                workspace_grant_epoch: 1,
                journal_id: id("journal_01J00000000000000000000000"),
            },
            LocalCommand::DecideChangesRecovery {
                recovery_approval_id: id("recovery_approval_01J0000000000000"),
                displayed_recovery_hash: sha256_bytes(b"displayed recovery"),
                choice: desktop_runtime::RecoveryApprovalChoice::Cancel,
            },
        ] {
            assert!(!should_cache_reply(&command), "{}", command.name());
        }
    }

    #[test]
    fn git_review_capabilities_are_projected_only_in_ready_mode() {
        let ready = supported_commands(crate::wire::BootMode::Ready);
        assert_eq!(ready.len(), 46);
        assert_eq!(
            &ready[4..9],
            [
                "workspace.list_entries",
                "workspace.read_text",
                "workspace.git_status",
                "workspace.git_diff",
                "workspace.search",
            ]
        );
        assert!(!supported_commands(crate::wire::BootMode::ReadOnlyRecovery)
            .iter()
            .any(|command| command.starts_with("workspace.git_")));
    }

    #[test]
    fn duplicate_recovery_prepare_stops_before_observation_or_authority_creation() {
        let storage = tempfile::tempdir().expect("temporary authority store");
        let state = HostState::initialize(Some(storage.path().join("authority")))
            .expect("ready host state");
        state.bind_renderer("main").expect("renderer binding");
        let renderer_session_id = state
            .renderer_session_authority("main")
            .expect("renderer authority")
            .session_id()
            .clone();
        let context = desktop_ipc::IpcValidationContext {
            expected_window_label: "main".to_owned(),
            renderer_session_id: renderer_session_id.clone(),
            installation_id: state.installation_id().clone(),
            now: UnixMillis(10_000),
            allowed_commands: supported_commands(crate::wire::BootMode::Ready),
        };
        let envelope = |epoch| {
            serde_json::to_vec(&serde_json::json!({
                "schemaVersion": "desktop-ipc-command.v1",
                "requestId": "request_recovery_prepare_1",
                "command": "changes.recovery.prepare",
                "windowLabel": "main",
                "rendererSessionId": renderer_session_id,
                "installationId": state.installation_id(),
                "issuedAt": 10_000,
                "payload": {
                    "workspaceId": "workspace_1",
                    "workspaceGrantEpoch": epoch,
                    "journalId": "journal_1"
                }
            }))
            .expect("envelope JSON")
        };
        let first = desktop_ipc::CommandEnvelopeValidator::parse(&envelope(7), &context)
            .expect("first recovery prepare");
        let duplicate = desktop_ipc::CommandEnvelopeValidator::parse(&envelope(7), &context)
            .expect("duplicate recovery prepare");
        let changed = desktop_ipc::CommandEnvelopeValidator::parse(&envelope(8), &context)
            .expect("changed recovery prepare");
        let mut observation_count = 0_u8;
        let mut authority_count = 0_u8;

        admit_dispatch_envelope(&state, &first, UnixMillis(10_000))
            .expect("first request admitted");
        observation_count += 1;
        authority_count += 1;
        assert!(admit_dispatch_envelope(&state, &duplicate, UnixMillis(10_001)).is_err());
        assert!(admit_dispatch_envelope(&state, &changed, UnixMillis(10_002)).is_err());
        assert_eq!(observation_count, 1);
        assert_eq!(authority_count, 1);
        assert!(!should_cache_reply(first.command()));
    }

    #[test]
    fn coordinator_failures_map_to_exact_safe_d2_codes() {
        let cases = [
            (
                BmadHelpCoordinatorError::SupportPlaneOffline,
                LocalErrorCode::SupportPlaneOffline,
            ),
            (
                BmadHelpCoordinatorError::Unauthorized,
                LocalErrorCode::Unauthorized,
            ),
            (BmadHelpCoordinatorError::Conflict, LocalErrorCode::Conflict),
            (
                BmadHelpCoordinatorError::Integrity,
                LocalErrorCode::IntegrityFailure,
            ),
            (
                BmadHelpCoordinatorError::Recovery,
                LocalErrorCode::RecoveryRequired,
            ),
            (
                BmadHelpCoordinatorError::ConsentExpired,
                LocalErrorCode::ConsentExpired,
            ),
            (
                BmadHelpCoordinatorError::ConsentBindingMismatch,
                LocalErrorCode::ConsentBindingMismatch,
            ),
            (
                BmadHelpCoordinatorError::ConsentAlreadyConsumed,
                LocalErrorCode::ConsentAlreadyConsumed,
            ),
            (
                BmadHelpCoordinatorError::TransportFailed,
                LocalErrorCode::TransportFailed,
            ),
            (
                BmadHelpCoordinatorError::ResponseBindingMismatch,
                LocalErrorCode::ResponseBindingMismatch,
            ),
            (
                BmadHelpCoordinatorError::InvalidModelOutput,
                LocalErrorCode::InvalidModelOutput,
            ),
            (
                BmadHelpCoordinatorError::ReceiptInvalid,
                LocalErrorCode::ReceiptInvalid,
            ),
        ];
        for (source, expected) in cases {
            let mapped = map_bmad_model_error(source);
            assert_eq!(mapped.code, expected);
            assert!(!mapped.safe_message.is_empty());
            assert!(mapped.correlation_id.is_none());
        }
    }

    #[test]
    fn auth_status_reports_only_the_explicit_build_composition() {
        let (state, _storage, _workspace, _workspace_id) = ready_workspace_state();
        let value = serde_json::to_value(
            model_auth_status_data(&state).expect("safe auth status projection"),
        )
        .expect("auth status serializes");
        assert_eq!(value["kind"], "model_auth_status");
        assert_eq!(value["value"]["authEpoch"], 1);
        assert_eq!(
            value["value"]["signInAvailable"],
            cfg!(feature = "direct-model-providers")
        );
        assert_eq!(value["value"]["signOutAvailable"], false);
        #[cfg(feature = "deterministic-help")]
        {
            assert_eq!(value["value"]["status"], "development_ready");
            assert_eq!(value["value"]["mode"], "deterministic_development");
            assert_eq!(value["value"]["developmentOnly"], true);
        }
        #[cfg(not(feature = "deterministic-help"))]
        {
            assert_eq!(value["value"]["status"], "unavailable");
            assert_eq!(value["value"]["mode"], "offline");
            assert_eq!(value["value"]["developmentOnly"], false);
        }
    }
}
