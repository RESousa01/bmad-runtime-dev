//! The Foundry model command lane, at the renderer boundary.
//!
//! Five commands were added here: `model.catalog.get`, `model.catalog.refresh`,
//! `model.selection.get`, `model.response.send` and `model.response.stop`. What
//! this file asserts is not that they work, but that they cannot be *used* for
//! anything beyond what they name — which is the property that would decay
//! silently as payloads grow fields.

#![allow(clippy::expect_used)]

use desktop_ipc::{
    Admission, AdmissionPolicy, CommandEnvelopeValidator, IpcValidationContext,
    IpcValidationError, RequestGate,
};
use desktop_runtime::{ContractId, LocalCommand, UnixMillis};
use serde_json::{json, Value};

/// The commands this lane added, in reviewed catalog order.
const FOUNDRY_MODEL_COMMANDS: [&str; 5] = [
    "model.catalog.get",
    "model.catalog.refresh",
    "model.selection.get",
    "model.response.send",
    "model.response.stop",
];

/// Every command the renderer may send once the host advertises Ready, in the
/// order pinned across the five catalog sites.
const READY_COMMANDS: [&str; 51] = [
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
    "model.catalog.get",
    "model.catalog.refresh",
    "model.selection.get",
    "model.response.send",
    "model.response.stop",
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

fn id(value: &str) -> ContractId {
    ContractId::new(value).expect("test identifier")
}

fn context() -> IpcValidationContext {
    IpcValidationContext {
        expected_window_label: "main".to_owned(),
        renderer_session_id: id("renderer_session_1"),
        installation_id: id("installation_1"),
        now: UnixMillis(10_000),
        allowed_commands: FOUNDRY_MODEL_COMMANDS
            .iter()
            .map(|command| (*command).to_owned())
            .collect(),
    }
}

fn envelope(request_id: &str, command: &str, payload: &Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": "desktop-ipc-command.v1",
        "requestId": request_id,
        "command": command,
        "windowLabel": "main",
        "rendererSessionId": "renderer_session_1",
        "installationId": "installation_1",
        "issuedAt": 10_000,
        "payload": payload,
    }))
    .expect("command JSON")
}

fn scope_payload() -> Value {
    json!({ "workspaceId": "workspace_1", "workspaceGrantEpoch": 3 })
}

fn send_payload() -> Value {
    json!({
        "workspaceId": "workspace_1",
        "workspaceGrantEpoch": 3,
        "taskId": "task_1",
        "clientSessionRef": "client_session_1",
        "prompt": "Summarize the changes I have staged.",
    })
}

fn stop_payload() -> Value {
    json!({
        "workspaceId": "workspace_1",
        "workspaceGrantEpoch": 3,
        "taskId": "task_1",
        "runId": "run_1",
    })
}

fn parse(command: &str, payload: &Value) -> Result<LocalCommand, IpcValidationError> {
    CommandEnvelopeValidator::parse(&envelope("request_1", command, payload), &context())
        .map(|parsed| parsed.command().clone())
}

#[test]
fn the_five_commands_parse_into_their_own_closed_variants() {
    assert!(matches!(
        parse("model.catalog.get", &scope_payload()),
        Ok(LocalCommand::GetModelCatalog { .. })
    ));
    assert!(matches!(
        parse("model.catalog.refresh", &scope_payload()),
        Ok(LocalCommand::RefreshModelCatalog { .. })
    ));
    assert!(matches!(
        parse("model.selection.get", &scope_payload()),
        Ok(LocalCommand::GetModelSelection { .. })
    ));
    assert!(matches!(
        parse("model.response.send", &send_payload()),
        Ok(LocalCommand::SendModelResponse { .. })
    ));
    assert!(matches!(
        parse("model.response.stop", &stop_payload()),
        Ok(LocalCommand::StopModelResponse { .. })
    ));
}

/// The load-bearing one.
///
/// Which model answers, where it lives, what it is allowed to do and how large
/// a request may be are all host decisions. `deny_unknown_fields` is what makes
/// that structural rather than conventional: none of these fields is ignored,
/// each is a rejection, so a renderer cannot influence any of them even by
/// sending them.
#[test]
fn no_payload_can_carry_connection_capability_or_hash_authority() {
    let forbidden = [
        json!({ "deployment": "gpt-4o-prod" }),
        json!({ "deploymentName": "gpt-4o-prod" }),
        json!({ "endpoint": "https://contoso.openai.azure.com" }),
        json!({ "baseUrl": "https://contoso.openai.azure.com" }),
        json!({ "apiKey": "sk-not-a-real-key" }),
        json!({ "credential": "sk-not-a-real-key" }),
        json!({ "bearer": "token" }),
        json!({ "capabilities": ["tools"] }),
        json!({ "capabilityHash": "0".repeat(64) }),
        json!({ "limits": { "maxOutputTokens": 4096 } }),
        json!({ "maxOutputTokens": 4096 }),
        json!({ "profileHash": "0".repeat(64) }),
        json!({ "catalogHash": "0".repeat(64) }),
        json!({ "modelProfileHash": "0".repeat(64) }),
        json!({ "region": "swedencentral" }),
        json!({ "subscriptionId": "00000000-0000-0000-0000-000000000000" }),
        json!({ "resourceGroup": "rg-forge" }),
    ];

    for (command, base) in [
        ("model.catalog.get", scope_payload()),
        ("model.catalog.refresh", scope_payload()),
        ("model.selection.get", scope_payload()),
        ("model.response.send", send_payload()),
        ("model.response.stop", stop_payload()),
    ] {
        // The base payload is accepted, so a rejection below is attributable to
        // the added field and not to the payload being wrong all along.
        assert!(
            parse(command, &base).is_ok(),
            "{command} rejected its own reviewed payload"
        );
        for extra in &forbidden {
            let mut payload = base.clone();
            let (key, value) = extra
                .as_object()
                .and_then(|object| object.iter().next())
                .expect("single-key probe");
            payload
                .as_object_mut()
                .expect("object payload")
                .insert(key.clone(), value.clone());
            assert!(
                matches!(
                    parse(command, &payload),
                    Err(IpcValidationError::InvalidPayload)
                ),
                "{command} accepted renderer-supplied `{key}`"
            );
        }
    }
}

#[test]
fn the_scope_commands_reject_a_missing_or_unsafe_grant_epoch() {
    for command in [
        "model.catalog.get",
        "model.catalog.refresh",
        "model.selection.get",
    ] {
        for payload in [
            json!({ "workspaceId": "workspace_1" }),
            json!({ "workspaceId": "workspace_1", "workspaceGrantEpoch": 0 }),
            json!({ "workspaceId": "workspace_1", "workspaceGrantEpoch": 9_007_199_254_740_992_u64 }),
            json!({ "workspaceGrantEpoch": 3 }),
        ] {
            assert!(
                matches!(
                    parse(command, &payload),
                    Err(IpcValidationError::InvalidPayload)
                ),
                "{command} accepted {payload}"
            );
        }
    }
}

#[test]
fn send_rejects_an_empty_oversized_or_nul_bearing_prompt() {
    for prompt in [
        String::new(),
        "   \t \n ".to_owned(),
        "a".repeat(16 * 1024 + 1),
        "prompt\0injected".to_owned(),
    ] {
        let mut payload = send_payload();
        payload
            .as_object_mut()
            .expect("object payload")
            .insert("prompt".to_owned(), json!(prompt));
        assert!(matches!(
            parse("model.response.send", &payload),
            Err(IpcValidationError::InvalidPayload)
        ));
    }
}

/// A run cannot be stopped without naming the task that owns it. Dropping
/// `taskId` would let a renderer that learned a run identifier stop a run
/// belonging to a task it never owned.
#[test]
fn stop_requires_both_the_task_and_the_run() {
    for payload in [
        json!({ "workspaceId": "workspace_1", "workspaceGrantEpoch": 3, "runId": "run_1" }),
        json!({ "workspaceId": "workspace_1", "workspaceGrantEpoch": 3, "taskId": "task_1" }),
    ] {
        assert!(matches!(
            parse("model.response.stop", &payload),
            Err(IpcValidationError::InvalidPayload)
        ));
    }
}

#[test]
fn a_command_the_host_has_not_advertised_stays_unreachable() {
    let mut narrowed = context();
    narrowed.allowed_commands = vec!["model.catalog.get".to_owned()];
    for command in ["model.catalog.refresh", "model.response.send"] {
        let payload = if command == "model.response.send" {
            send_payload()
        } else {
            scope_payload()
        };
        assert!(matches!(
            CommandEnvelopeValidator::parse(&envelope("request_1", command, &payload), &narrowed),
            Err(IpcValidationError::CapabilityUnavailable)
        ));
    }
}

#[test]
fn effect_classification_matches_the_reviewed_lane() {
    let reads = [
        parse("model.catalog.get", &scope_payload()).expect("catalog get"),
        parse("model.selection.get", &scope_payload()).expect("selection get"),
    ];
    for command in reads {
        assert!(!command.is_mutating(), "{} is mutating", command.name());
        assert!(
            !command.requires_request_tracking(),
            "{} is request-tracked",
            command.name()
        );
    }

    let writes = [
        parse("model.catalog.refresh", &scope_payload()).expect("catalog refresh"),
        parse("model.response.send", &send_payload()).expect("response send"),
        parse("model.response.stop", &stop_payload()).expect("response stop"),
    ];
    for command in writes {
        assert!(command.is_mutating(), "{} is not mutating", command.name());
        assert!(
            command.requires_request_tracking(),
            "{} is not request-tracked",
            command.name()
        );
    }
}

fn admit(command: &str, payload: &Value, request_id: &str) -> Result<Admission, IpcValidationError> {
    let gate = RequestGate::new(AdmissionPolicy::default());
    let parsed = CommandEnvelopeValidator::parse(&envelope(request_id, command, payload), &context())
        .expect("valid envelope");
    gate.admit(&parsed, UnixMillis(10_000))?;
    gate.admit(&parsed, UnixMillis(10_001))
}

/// Sending and stopping are one-shot; refreshing is fingerprinted.
///
/// The distinction is the whole reason the gate has more than one mutation
/// identity. Replaying a send would answer — and bill — twice under one request
/// id. Replaying a stop would let a stale renderer cancel whatever run occupied
/// the task next. Replaying a refresh is harmless: it is the same read of the
/// same catalog, so the second attempt is served the first one's reply instead
/// of hitting the support plane again.
#[test]
fn the_response_lifecycle_is_one_shot_and_the_refresh_is_replayable() {
    assert!(matches!(
        admit("model.response.send", &send_payload(), "request_send"),
        Err(IpcValidationError::IdempotencyConflict)
    ));
    assert!(matches!(
        admit("model.response.stop", &stop_payload(), "request_stop"),
        Err(IpcValidationError::IdempotencyConflict)
    ));
    assert!(matches!(
        admit("model.catalog.refresh", &scope_payload(), "request_refresh"),
        Ok(Admission::Replay)
    ));
    // The reads are not tracked at all, so a repeat is simply a new request.
    assert!(matches!(
        admit("model.catalog.get", &scope_payload(), "request_get"),
        Ok(Admission::New)
    ));
}

/// A one-shot request id cannot be reused for a different command either. If it
/// could, a renderer could burn a send's id on a cheap read and then reuse it.
#[test]
fn a_one_shot_request_id_cannot_alias_another_command() {
    let gate = RequestGate::new(AdmissionPolicy::default());
    let send = CommandEnvelopeValidator::parse(
        &envelope("request_shared", "model.response.send", &send_payload()),
        &context(),
    )
    .expect("valid send");
    let refresh = CommandEnvelopeValidator::parse(
        &envelope("request_shared", "model.catalog.refresh", &scope_payload()),
        &context(),
    )
    .expect("valid refresh");

    assert!(matches!(
        gate.admit(&send, UnixMillis(10_000)),
        Ok(Admission::New)
    ));
    assert!(matches!(
        gate.admit(&refresh, UnixMillis(10_001)),
        Err(IpcValidationError::IdempotencyConflict)
    ));
}

/// The machine-readable minimum-capability assertion.
///
/// The catalog is a closed list of named, bounded operations. This lane must
/// not have introduced a general-purpose one — no way to name a URL, a
/// filesystem path, a shell command, or a secret — because a single such
/// command makes every other bound in the system decorative.
///
/// Emits `artifacts/foundry/rust/ipc.json` so the same fact is checkable
/// outside this process.
#[test]
fn the_catalog_gained_no_generic_http_path_shell_or_secret_command() {
    const FORBIDDEN_FRAGMENTS: [&str; 20] = [
        "http",
        "https",
        "url",
        "uri",
        "origin",
        "endpoint",
        "request.raw",
        "fetch",
        "download",
        "upload",
        "shell",
        "exec",
        "spawn",
        "process",
        "eval",
        "script",
        "write_path",
        "read_path",
        "secret",
        "credential",
    ];

    for command in READY_COMMANDS {
        let folded = command.to_ascii_lowercase();
        for fragment in FORBIDDEN_FRAGMENTS {
            assert!(
                !folded.contains(fragment),
                "`{command}` names a generic `{fragment}` capability"
            );
        }
    }

    // The five new names are present exactly once each, and every one of them
    // is scoped to the model catalog or one task's response.
    for command in FOUNDRY_MODEL_COMMANDS {
        assert_eq!(
            READY_COMMANDS
                .iter()
                .filter(|ready| **ready == command)
                .count(),
            1,
            "`{command}` is not in the catalog exactly once"
        );
        assert!(
            command.starts_with("model."),
            "`{command}` escaped the model namespace"
        );
    }

    let evidence = json!({
        "schemaVersion": "sapphirus.foundry-ipc-evidence.v1",
        "readyCommandCount": READY_COMMANDS.len(),
        "readyCommands": READY_COMMANDS.as_slice(),
        "addedCommands": FOUNDRY_MODEL_COMMANDS.as_slice(),
        "forbiddenNameFragments": FORBIDDEN_FRAGMENTS.as_slice(),
        "genericCapabilityFindings": Vec::<String>::new(),
    });
    let destination = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../artifacts/foundry/rust");
    std::fs::create_dir_all(&destination).expect("evidence directory");
    let mut encoded = serde_json::to_string_pretty(&evidence).expect("evidence JSON");
    encoded.push('\n');
    std::fs::write(destination.join("ipc.json"), encoded).expect("evidence file");
}

/// `model.selection.set` now carries either the catalog-backed pick or the
/// legacy direct-provider shape, and the two are mutually exclusive because
/// each variant is `deny_unknown_fields` over a disjoint field set.
#[test]
fn model_selection_set_admits_a_managed_profile_pick_and_still_the_legacy_shape() {
    let mut selection_context = context();
    selection_context
        .allowed_commands
        .push("model.selection.set".to_owned());

    assert!(matches!(
        CommandEnvelopeValidator::parse(
            &envelope(
                "request_managed",
                "model.selection.set",
                &json!({
                    "workspaceId": "workspace_1",
                    "workspaceGrantEpoch": 3,
                    "profileId": "balanced-reasoning",
                    "variantId": "standard",
                }),
            ),
            &selection_context,
        )
        .expect("valid managed selection")
        .command(),
        LocalCommand::SetModelProfileSelection {
            profile_id,
            variant_id: Some(variant_id),
            ..
        } if profile_id == "balanced-reasoning" && variant_id == "standard"
    ));

    // The variant is optional: a profile has its own default.
    assert!(matches!(
        CommandEnvelopeValidator::parse(
            &envelope(
                "request_managed_default",
                "model.selection.set",
                &json!({
                    "workspaceId": "workspace_1",
                    "workspaceGrantEpoch": 3,
                    "profileId": "balanced",
                }),
            ),
            &selection_context,
        )
        .expect("valid managed selection without a variant")
        .command(),
        LocalCommand::SetModelProfileSelection {
            variant_id: None,
            ..
        }
    ));

    for bad_payload in [
        json!({ "workspaceId": "workspace_1", "workspaceGrantEpoch": 3, "profileId": "Bad" }),
        json!({ "workspaceId": "workspace_1", "workspaceGrantEpoch": 3, "profileId": "ok", "variantId": "Bad" }),
        json!({ "workspaceId": "workspace_1", "workspaceGrantEpoch": 0, "profileId": "balanced" }),
        json!({ "workspaceId": "workspace_1", "workspaceGrantEpoch": 3, "profileId": "balanced", "modelId": "gpt-5" }),
    ] {
        assert!(
            matches!(
                CommandEnvelopeValidator::parse(
                    &envelope("request_managed_bad", "model.selection.set", &bad_payload),
                    &selection_context,
                )
                .err(),
                Some(IpcValidationError::InvalidPayload)
            ),
            "accepted invalid managed selection: {bad_payload}"
        );
    }
}

#[test]
fn the_ready_catalog_holds_the_five_new_commands_in_reviewed_order() {
    let start = READY_COMMANDS
        .iter()
        .position(|command| *command == "model.catalog.get")
        .expect("catalog get is in the ready catalog");
    assert_eq!(&READY_COMMANDS[start..start + 5], &FOUNDRY_MODEL_COMMANDS);
    assert_eq!(READY_COMMANDS[start - 1], "model.selection.set");
    assert_eq!(READY_COMMANDS[start + 5], "chat.send");
}
