#![allow(clippy::expect_used)]

use std::collections::BTreeSet;

use desktop_ipc::{
    project_created_bmad_help_run, CommandEnvelopeValidator, IpcValidationContext,
    IpcValidationError, MAX_BMAD_HELP_RUN_PROJECTION_BYTES,
};
use desktop_runtime::{
    sha256_bytes, BmadCatalogAvailability, BmadEntrypointKind, BmadHelpActionKey,
    BmadHelpConfidence, BmadHelpRecommendation, BmadHelpSourceRef, BmadLoadedPackage,
    BmadLoadedSkill, ContractId, LocalCommand, UnixMillis,
};
use serde_json::{json, Value};

const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

fn id(value: &str) -> ContractId {
    ContractId::new(value).expect("test identifier")
}

fn context(allowed_commands: &[&str]) -> IpcValidationContext {
    IpcValidationContext {
        expected_window_label: "main".to_owned(),
        renderer_session_id: id("renderer_session_1"),
        installation_id: id("installation_1"),
        now: UnixMillis(10_000),
        allowed_commands: allowed_commands.iter().map(ToString::to_string).collect(),
    }
}

fn envelope(payload: &Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": "desktop-ipc-command.v1",
        "requestId": "request_1",
        "command": "run.create",
        "windowLabel": "main",
        "rendererSessionId": "renderer_session_1",
        "installationId": "installation_1",
        "issuedAt": 10_000,
        "payload": payload,
    }))
    .expect("command JSON")
}

fn valid_payload() -> Value {
    json!({
        "workspaceId": "workspace_1",
        "workspaceGrantEpoch": 7,
        "runKind": "bmad_help",
        "currentIntent": "Help me choose the next planning step",
    })
}

fn recommendation() -> BmadHelpRecommendation {
    let action = BmadHelpActionKey {
        capability_catalog_hash: sha256_bytes(b"LEAK_CANARY_CAPABILITY_CATALOG"),
        package_version_id: id("pkgver_LEAK_CANARY_PACKAGE_VERSION"),
        module_code: "core".to_owned(),
        skill_name: "bmad-help".to_owned(),
        action: None,
    };
    BmadHelpRecommendation {
        action: action.clone(),
        display_name: "BMad Help".to_owned(),
        reason: "The current intent matches the installed Help catalog.".to_owned(),
        required_guidance: true,
        confidence: BmadHelpConfidence::Unknown,
        availability: BmadCatalogAvailability::CapabilityDisabled,
        expected_outputs: vec!["next-step recommendation".to_owned()],
        source_refs: vec![BmadHelpSourceRef {
            capability_catalog_hash: action.capability_catalog_hash,
            package_version_id: action.package_version_id.clone(),
            module_code: action.module_code.clone(),
            skill_name: action.skill_name.clone(),
            action: action.action.clone(),
            source_ordinal: 4_294_967_296,
        }],
        blocker_codes: vec!["bmad_capability_disabled".to_owned()],
        alternatives: vec![BmadHelpActionKey {
            capability_catalog_hash: sha256_bytes(b"LEAK_CANARY_ALTERNATIVE_HASH"),
            package_version_id: id("pkgver_LEAK_CANARY_ALTERNATIVE_PACKAGE"),
            module_code: "authority_model_config_path_cas_prompt_canary".to_owned(),
            skill_name: "leak_canary_alternative_skill".to_owned(),
            action: Some("leak_canary_alternative_action".to_owned()),
        }],
        completion_claimed: false,
    }
}

fn package() -> BmadLoadedPackage {
    BmadLoadedPackage {
        package_name: "bmad-method".to_owned(),
        package_version: "6.10.0".to_owned(),
        package_version_id: id("pkgver_LEAK_CANARY_PACKAGE_VERSION"),
        descriptor_hash: sha256_bytes(b"LEAK_CANARY_DESCRIPTOR_HASH"),
        observed_inventory_hash: sha256_bytes(b"LEAK_CANARY_INVENTORY_HASH"),
        skills: vec![BmadLoadedSkill {
            module_code: "core".to_owned(),
            skill_name: "bmad-help".to_owned(),
            display_name: "BMad Help".to_owned(),
            description: "Provide catalog-grounded guidance.".to_owned(),
            entrypoint_kind: BmadEntrypointKind::Direct,
            actions: Vec::new(),
            distribution_profile: "sapphirus_package".to_owned(),
            install_profile: "SapphirusManagedV1".to_owned(),
            validation_profile: "MethodOfficialSkillV6".to_owned(),
            execution_profile_hash: sha256_bytes(b"LEAK_CANARY_EXECUTION_PROFILE_HASH"),
            capability_enabled: false,
            structurally_eligible: true,
        }],
    }
}

#[test]
fn run_create_is_known_but_requires_host_capability_advertisement() {
    let bytes = envelope(&valid_payload());
    assert!(matches!(
        CommandEnvelopeValidator::parse(&bytes, &context(&[])),
        Err(IpcValidationError::CapabilityUnavailable)
    ));

    let validated = CommandEnvelopeValidator::parse(&bytes, &context(&["run.create"]))
        .expect("advertised run.create");
    assert!(matches!(
        validated.command(),
        LocalCommand::CreateBmadHelpRun { .. }
    ));
    if let LocalCommand::CreateBmadHelpRun {
        workspace_id,
        workspace_grant_epoch,
        current_intent,
    } = validated.command()
    {
        assert_eq!(workspace_id.as_str(), "workspace_1");
        assert_eq!(*workspace_grant_epoch, 7);
        assert_eq!(
            current_intent.as_str(),
            "Help me choose the next planning step"
        );
    }
    assert_eq!(validated.command().name(), "run.create");
    assert!(validated.command().is_mutating());
}

#[test]
fn run_create_accepts_only_bmad_help_safe_epoch_and_exact_bounded_intent() {
    let mut at_bounds = valid_payload();
    at_bounds["workspaceGrantEpoch"] = json!(MAX_SAFE_JSON_INTEGER);
    at_bounds["currentIntent"] = json!("x".repeat(4_096));
    assert!(
        CommandEnvelopeValidator::parse(&envelope(&at_bounds), &context(&["run.create"])).is_ok()
    );

    let invalid_payloads = [
        json!({
            "workspaceId": "workspace_1",
            "workspaceGrantEpoch": 1,
            "runKind": "bmad_architecture",
            "currentIntent": "plan architecture"
        }),
        json!({
            "workspaceId": "workspace_1",
            "workspaceGrantEpoch": 0,
            "runKind": "bmad_help",
            "currentIntent": "find next step"
        }),
        json!({
            "workspaceId": "workspace_1",
            "workspaceGrantEpoch": MAX_SAFE_JSON_INTEGER + 1,
            "runKind": "bmad_help",
            "currentIntent": "find next step"
        }),
        json!({
            "workspaceId": "workspace_1",
            "workspaceGrantEpoch": 1,
            "runKind": "bmad_help",
            "currentIntent": ""
        }),
        json!({
            "workspaceId": "workspace_1",
            "workspaceGrantEpoch": 1,
            "runKind": "bmad_help",
            "currentIntent": "x".repeat(4_097)
        }),
        json!({
            "workspaceId": "workspace_1",
            "workspaceGrantEpoch": 1,
            "runKind": "bmad_help",
            "currentIntent": "unsafe\nintent"
        }),
        json!({
            "workspaceId": "workspace_1",
            "workspaceGrantEpoch": 1,
            "runKind": "bmad_help",
            "currentIntent": "unsafe\u{202e}intent"
        }),
    ];

    for payload in invalid_payloads {
        assert!(matches!(
            CommandEnvelopeValidator::parse(&envelope(&payload), &context(&["run.create"])),
            Err(IpcValidationError::InvalidPayload)
        ));
    }
}

#[test]
fn run_create_rejects_every_renderer_supplied_authority_or_execution_field() {
    for field in [
        "ownerId",
        "ownerScopeId",
        "projectId",
        "runId",
        "sessionId",
        "authorityRef",
        "authorityEpoch",
        "model",
        "modelId",
        "deployment",
        "deploymentId",
        "capabilityKey",
        "capabilityCatalogHash",
        "effect",
        "effects",
        "toolDefinitions",
        "runnable",
        "completionClaimed",
        "replayed",
    ] {
        let mut payload = valid_payload();
        payload
            .as_object_mut()
            .expect("payload object")
            .insert(field.to_owned(), json!("renderer-controlled"));
        assert!(
            matches!(
                CommandEnvelopeValidator::parse(&envelope(&payload), &context(&["run.create"])),
                Err(IpcValidationError::InvalidPayload)
            ),
            "accepted forbidden field {field}"
        );
    }
}

#[test]
fn created_help_run_projection_is_exact_inert_and_disclosure_safe() {
    let internal = recommendation();
    let hidden_hashes = [
        internal.action.capability_catalog_hash.to_string(),
        internal.alternatives[0].capability_catalog_hash.to_string(),
    ];
    let projection = project_created_bmad_help_run(
        &package(),
        &internal,
        id("workspace_1"),
        id("run_1"),
        id("session_1"),
    )
    .expect("safe inert run projection");
    let value = serde_json::to_value(&projection).expect("projection JSON");

    let keys = value
        .as_object()
        .expect("projection object")
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        keys,
        BTreeSet::from([
            "completionClaimed",
            "lifecycle",
            "recommendation",
            "runId",
            "runKind",
            "runnable",
            "schemaVersion",
            "sessionId",
            "workspaceId",
        ])
    );
    assert_eq!(value["schemaVersion"], "bmad-help-run.v1");
    assert_eq!(value["runKind"], "bmad_help");
    assert_eq!(value["lifecycle"], "created_unbound");
    assert_eq!(value["workspaceId"], "workspace_1");
    assert_eq!(value["runId"], "run_1");
    assert_eq!(value["sessionId"], "session_1");
    assert_eq!(value["runnable"], false);
    assert_eq!(value["completionClaimed"], false);
    assert_eq!(
        value["recommendation"]["schemaVersion"],
        "bmad-help-recommendation.v1"
    );

    let json = serde_json::to_string(&value).expect("projection text");
    for forbidden in [
        "ownerId",
        "ownerScopeId",
        "projectId",
        "authorityRef",
        "authorityEpoch",
        "packageVersionId",
        "capabilityCatalogHash",
        "sourceRefs",
        "sourceOrdinal",
        "modelId",
        "deploymentId",
        "executionProfileHash",
        "effect",
        "replay",
        "LEAK_CANARY",
        "authority_model_config_path_cas_prompt_canary",
    ] {
        assert!(!json.contains(forbidden), "leaked {forbidden}");
    }
    for hidden_hash in hidden_hashes {
        assert!(!json.contains(&hidden_hash), "leaked internal hash");
    }
    assert!(json.len() <= MAX_BMAD_HELP_RUN_PROJECTION_BYTES);
}

#[test]
fn created_help_run_projection_rejects_completion_or_unsafe_recommendations() {
    let mut completed = recommendation();
    completed.completion_claimed = true;
    assert!(project_created_bmad_help_run(
        &package(),
        &completed,
        id("workspace_1"),
        id("run_1"),
        id("session_1"),
    )
    .is_err());

    let mut unsafe_reason = recommendation();
    unsafe_reason.reason = "unsafe\u{2066}reason".to_owned();
    assert!(project_created_bmad_help_run(
        &package(),
        &unsafe_reason,
        id("workspace_1"),
        id("run_1"),
        id("session_1"),
    )
    .is_err());
}
