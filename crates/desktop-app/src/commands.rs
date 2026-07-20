use desktop_egress::{ContextClassification, ContextReviewProjection, RetentionMode};
use desktop_ipc::{
    decode_retained_bmad_help_completion, decode_retained_bmad_help_run, deserialize_strict,
    project_bmad_help_approved, project_bmad_help_approved_lifecycle, project_bmad_help_cancelled,
    project_bmad_help_review, project_bmad_help_terminal, project_bmad_library_with_activations,
    project_created_bmad_help_run, project_model_auth_status, Admission, BmadHelpApprovalInput,
    BmadHelpApprovedLifecycleInput, BmadHelpCancellationInput,
    BmadHelpContextClassificationProjection, BmadHelpModelAccessProjectionError,
    BmadHelpProjectionError, BmadHelpRetentionProjection, BmadHelpReviewExclusionInput,
    BmadHelpReviewInput, BmadHelpReviewItemInput, BmadHelpReviewRedactionInput,
    BmadHelpSecretFindingInput, BmadHelpTerminalInput, BmadHelpTerminalReasonProjection,
    BmadProjectionError, CommandEnvelopeValidator, IpcValidationContext, IpcValidationError,
    ModelAuthModeProjection, ModelAuthStatusInput, ModelAuthStatusKindProjection,
    ValidatedCommandEnvelope,
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
use crate::bmad_model::config::{current_help_model_configuration, HelpModelMode};
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
const READY_COMMANDS: [&str; 33] = [
    "app.get_boot_state",
    "workspace.select_folder",
    "workspace.list",
    "workspace.revoke",
    "workspace.list_entries",
    "workspace.read_text",
    "workspace.search",
    "bmad.scan",
    "bmad.library.snapshot",
    "bmad.persona.view",
    "model.auth.status",
    "model.auth.sign_in",
    "model.auth.sign_out",
    "bmad.help.prepare",
    "bmad.help.approve",
    "bmad.help.cancel",
    "bmad.help.submit",
    "bmad.help.latest",
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
                | LocalCommand::ModelAuthSignIn
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
        // The bmad.capability.* verticals stay outside the reviewed READY
        // catalog until their host composition lands (readiness Task 7);
        // the envelope allowlist makes these arms unreachable today, and
        // they fail closed if that ever regresses.
        LocalCommand::PrepareBmadCapabilityRun { .. }
        | LocalCommand::ApproveBmadCapabilityRun { .. }
        | LocalCommand::CancelBmadCapabilityRun { .. }
        | LocalCommand::SubmitBmadCapabilityRun { .. }
        | LocalCommand::LatestBmadCapabilityRun { .. } => Err(LocalError::new(
            LocalErrorCode::TemporarilyUnavailable,
            "Capability runs are not yet composed in this build.",
            false,
        )),
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
        LocalCommand::ModelAuthSignIn => Err(model_sign_in_unavailable()),
        LocalCommand::ModelAuthSignOut => {
            state.sign_out_model()?;
            model_auth_status_data(state)
        }
        LocalCommand::PrepareBmadHelpReview {
            workspace_id,
            workspace_grant_epoch,
        } => prepare_bmad_help_review(
            state,
            foundation,
            renderer_session,
            workspace_id,
            workspace_grant_epoch,
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
    let configuration = current_help_model_configuration().map_err(|_| {
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
    project_model_auth_status(ModelAuthStatusInput {
        status,
        mode,
        auth_epoch: state.model_auth_epoch(),
        development_only,
        destination_label: configuration.destination_label.to_owned(),
    })
    .map(HostCommandData::ModelAuthStatus)
    .map_err(map_bmad_model_projection_error)
}

fn model_sign_in_unavailable() -> LocalError {
    LocalError::new(
        LocalErrorCode::IdentityUnavailable,
        "Interactive model sign-in is unavailable in this desktop composition.",
        false,
    )
}

fn prepare_bmad_help_review(
    state: &HostState,
    foundation: &BmadLoadedFoundation,
    renderer_session: &RendererSessionGuard<'_>,
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    created_at: UnixMillis,
) -> Result<HostCommandData, LocalError> {
    let authority = state.ready_workspace_commit()?;
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

fn bmad_library_snapshot(
    foundation: &BmadLoadedFoundation,
    scope: BmadLibraryProjectionScope,
    cursor: Option<&str>,
) -> Result<HostCommandData, LocalError> {
    #[cfg(feature = "deterministic-help")]
    let activations = &[("core", "bmad-help")][..];
    #[cfg(not(feature = "deterministic-help"))]
    let activations = &[][..];
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
    project_bmad_library_with_activations(
        foundation.package(),
        foundation.catalog(),
        foundation.roster(),
        scope,
        cursor,
        activations,
        builder_packages,
    )
    .map(HostCommandData::BmadLibrarySnapshot)
    .map_err(map_bmad_projection_error)
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
        WorkspaceError::GrantUnavailable => {
            not_found_error("The local workspace is not available.")
        }
        WorkspaceError::RootIdentityChanged => {
            conflict_error("The local workspace identity changed; select the folder again.")
        }
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
        LocalErrorCode, UnixMillis,
    };
    use desktop_store::{
        BmadHelpRunCompletionReceipt, BmadHelpRunCreationReceipt, BmadHelpRunLatest,
    };
    use desktop_workspace::WorkspaceError;

    #[cfg(feature = "deterministic-help")]
    use super::prepare_bmad_help_review;
    use super::{
        about_projection, admit_dispatch_envelope, bmad_library_snapshot, create_bmad_help_run,
        latest_bmad_help_run, latest_bmad_help_run_data, load_preferences, map_bmad_model_error,
        map_workspace_error, model_auth_status_data, offboarding_inspect, revoke_workspace,
        save_preferences, should_cache_reply, supported_commands, view_bmad_persona,
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
        assert_eq!(manifest.categories.len(), 8);
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

    #[test]
    fn host_projects_the_sealed_method_library_without_authority_bytes(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let foundation = load_bmad_foundation(foundation_path()).expect("sealed foundation");
        let data = bmad_library_snapshot(
            &foundation,
            BmadLibraryProjectionScope::InstalledMethod,
            None,
        )
        .expect("library snapshot");
        let HostCommandData::BmadLibrarySnapshot(projection) = &data else {
            return Err("expected BMAD library projection".into());
        };
        assert_eq!(projection.installed_skills.len(), 2);
        assert_eq!(projection.help_actions.len(), 2);
        assert_eq!(projection.method_agents.len(), 6);
        assert_eq!(projection.builder_packages.len(), 2);
        for builder in &projection.builder_packages {
            assert_eq!(builder.activation_state, "installed_inactive");
            assert_eq!(builder.blocker_codes, vec!["builder_engine_gated"]);
            assert_eq!(builder.descriptor_digest.len(), 12);
        }
        #[cfg(feature = "deterministic-help")]
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
        let serialized = serde_json::to_string(&data).expect("wire projection");
        for forbidden in [
            "sha256:",
            "sourceLocalMemberLabel",
            "packageVersionId",
            "outputLocations",
            ".md",
        ] {
            assert!(!serialized.contains(forbidden), "leaked {forbidden}");
        }
        Ok(())
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
            LocalCommand::ModelAuthSignIn,
            LocalCommand::ModelAuthSignOut,
            LocalCommand::PrepareBmadHelpReview {
                workspace_id: workspace_id.clone(),
                workspace_grant_epoch: 1,
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
        assert_eq!(ready.len(), 33);
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
        assert_eq!(value["value"]["signInAvailable"], false);
        assert_eq!(value["value"]["signOutAvailable"], true);
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
