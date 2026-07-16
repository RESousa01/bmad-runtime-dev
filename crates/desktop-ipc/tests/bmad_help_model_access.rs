use desktop_ipc::{
    project_bmad_help_approved, project_bmad_help_approved_lifecycle, project_bmad_help_cancelled,
    project_bmad_help_review, project_bmad_help_terminal, project_model_auth_status, Admission,
    AdmissionPolicy, BmadHelpApprovalInput, BmadHelpApprovedLifecycleInput,
    BmadHelpCancellationInput, BmadHelpContextClassificationProjection,
    BmadHelpRetentionProjection, BmadHelpReviewExclusionInput, BmadHelpReviewInput,
    BmadHelpReviewItemInput, BmadHelpReviewRedactionInput, BmadHelpSecretFindingInput,
    BmadHelpTerminalInput, BmadHelpTerminalReasonProjection, CommandEnvelopeValidator,
    IpcValidationContext, IpcValidationError, ModelAuthModeProjection, ModelAuthStatusInput,
    ModelAuthStatusKindProjection, RequestGate,
};
use desktop_runtime::{
    ContractId, LocalErrorCode, RelativeWorkspacePath, Sha256Digest, UnixMillis,
};
use serde_json::{json, Map, Value};

const MANIFEST_HASH: &str =
    "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn id(value: &str) -> Result<ContractId, Box<dyn std::error::Error>> {
    Ok(ContractId::new(value)?)
}

fn context(allowed_commands: &[&str]) -> Result<IpcValidationContext, Box<dyn std::error::Error>> {
    Ok(IpcValidationContext {
        expected_window_label: "main".to_owned(),
        renderer_session_id: id("renderer_test")?,
        installation_id: id("installation_test")?,
        now: UnixMillis(10_000),
        allowed_commands: allowed_commands
            .iter()
            .map(|command| (*command).to_owned())
            .collect(),
    })
}

fn envelope(command: &str, payload: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": "desktop-ipc-command.v1",
        "requestId": "request_test",
        "command": command,
        "windowLabel": "main",
        "rendererSessionId": "renderer_test",
        "installationId": "installation_test",
        "issuedAt": 10_000,
        "payload": payload,
    }))
    .expect("test command envelope serializes")
}

fn exact_commands() -> [(&'static str, Value, bool); 8] {
    [
        ("model.auth.status", json!({}), false),
        ("model.auth.sign_in", json!({}), true),
        ("model.auth.sign_out", json!({}), true),
        (
            "bmad.help.prepare",
            json!({"workspaceId": "workspace_test", "workspaceGrantEpoch": 1}),
            true,
        ),
        (
            "bmad.help.approve",
            json!({
                "workspaceId": "workspace_test",
                "workspaceGrantEpoch": 1,
                "manifestHash": MANIFEST_HASH,
            }),
            true,
        ),
        (
            "bmad.help.cancel",
            json!({
                "workspaceId": "workspace_test",
                "workspaceGrantEpoch": 1,
                "manifestHash": MANIFEST_HASH,
                "decisionId": "decision_test",
            }),
            true,
        ),
        (
            "bmad.help.submit",
            json!({
                "workspaceId": "workspace_test",
                "workspaceGrantEpoch": 1,
                "manifestHash": MANIFEST_HASH,
                "decisionId": "decision_test",
            }),
            true,
        ),
        (
            "bmad.help.latest",
            json!({"workspaceId": "workspace_test", "workspaceGrantEpoch": 1}),
            false,
        ),
    ]
}

#[test]
fn exact_eight_commands_are_capability_gated_and_correctly_classified(
) -> Result<(), Box<dyn std::error::Error>> {
    for (command, payload, mutating) in exact_commands() {
        let bytes = envelope(command, payload.clone());
        let validated = CommandEnvelopeValidator::parse(&bytes, &context(&[command])?)?;
        assert_eq!(validated.command().name(), command);
        assert_eq!(validated.command().is_mutating(), mutating, "{command}");

        let error = CommandEnvelopeValidator::parse(&bytes, &context(&[])?).unwrap_err();
        assert!(
            matches!(error, IpcValidationError::CapabilityUnavailable),
            "{command}: {error:?}"
        );
    }
    Ok(())
}

#[test]
fn every_payload_is_closed_against_renderer_authority_fields(
) -> Result<(), Box<dyn std::error::Error>> {
    for forbidden in [
        "provider",
        "model",
        "deployment",
        "region",
        "schema",
        "package",
        "destination",
        "context",
        "result",
        "receipt",
        "authority",
    ] {
        for (command, payload, _) in exact_commands() {
            let mut fields = match payload {
                Value::Object(fields) => fields,
                _ => Map::new(),
            };
            fields.insert(
                forbidden.to_owned(),
                Value::String("renderer_owned".to_owned()),
            );
            let error = CommandEnvelopeValidator::parse(
                &envelope(command, Value::Object(fields)),
                &context(&[command])?,
            )
            .unwrap_err();
            assert!(
                matches!(error, IpcValidationError::InvalidPayload),
                "{command} accepted {forbidden}: {error:?}"
            );
        }
    }
    Ok(())
}

#[test]
fn workspace_epochs_hashes_and_decision_ids_are_bounded() -> Result<(), Box<dyn std::error::Error>>
{
    for command in [
        "bmad.help.prepare",
        "bmad.help.approve",
        "bmad.help.cancel",
        "bmad.help.submit",
        "bmad.help.latest",
    ] {
        let mut payload = exact_commands()
            .into_iter()
            .find(|(candidate, _, _)| *candidate == command)
            .expect("command fixture")
            .1;
        payload["workspaceGrantEpoch"] = json!(0);
        assert!(matches!(
            CommandEnvelopeValidator::parse(&envelope(command, payload), &context(&[command])?),
            Err(IpcValidationError::InvalidPayload)
        ));
    }

    for command in ["bmad.help.approve", "bmad.help.cancel", "bmad.help.submit"] {
        let mut payload = exact_commands()
            .into_iter()
            .find(|(candidate, _, _)| *candidate == command)
            .expect("command fixture")
            .1;
        payload["manifestHash"] = json!("not-a-sha256-digest");
        assert!(matches!(
            CommandEnvelopeValidator::parse(&envelope(command, payload), &context(&[command])?),
            Err(IpcValidationError::InvalidPayload)
        ));
    }

    for command in ["bmad.help.cancel", "bmad.help.submit"] {
        let mut payload = exact_commands()
            .into_iter()
            .find(|(candidate, _, _)| *candidate == command)
            .expect("command fixture")
            .1;
        payload["decisionId"] = json!("");
        assert!(matches!(
            CommandEnvelopeValidator::parse(&envelope(command, payload), &context(&[command])?),
            Err(IpcValidationError::InvalidPayload)
        ));
    }
    Ok(())
}

#[test]
fn help_review_mutations_reject_ambiguous_replay() -> Result<(), Box<dyn std::error::Error>> {
    for command in [
        "bmad.help.prepare",
        "bmad.help.approve",
        "bmad.help.cancel",
        "bmad.help.submit",
    ] {
        let payload = exact_commands()
            .into_iter()
            .find(|(candidate, _, _)| *candidate == command)
            .expect("command fixture")
            .1;
        let validated =
            CommandEnvelopeValidator::parse(&envelope(command, payload), &context(&[command])?)?;
        let gate = RequestGate::new(AdmissionPolicy::default());
        assert_eq!(gate.admit(&validated, UnixMillis(10_000))?, Admission::New);
        assert!(matches!(
            gate.admit(&validated, UnixMillis(10_001)),
            Err(IpcValidationError::IdempotencyConflict)
        ));
    }
    Ok(())
}

#[test]
fn local_context_preview_still_rejects_a_model_target() -> Result<(), Box<dyn std::error::Error>> {
    let command = "context.preview";
    let payload = json!({
        "workspaceId": "workspace_test",
        "relativePaths": ["README.md"],
        "modelTarget": {
            "provider": "renderer-provider",
            "model": "renderer-model",
            "deployment": "renderer-deployment",
        },
    });
    assert!(matches!(
        CommandEnvelopeValidator::parse(&envelope(command, payload), &context(&[command])?),
        Err(IpcValidationError::InvalidPayload)
    ));
    Ok(())
}

#[test]
fn auth_projection_is_closed_mode_consistent_and_bounded() -> Result<(), Box<dyn std::error::Error>>
{
    let projection = project_model_auth_status(ModelAuthStatusInput {
        status: ModelAuthStatusKindProjection::DevelopmentReady,
        mode: ModelAuthModeProjection::DeterministicDevelopment,
        auth_epoch: 7,
        development_only: true,
        destination_label: "Deterministic local model - development only".to_owned(),
    })?;
    let value = serde_json::to_value(&projection)?;
    assert_eq!(
        value,
        json!({
            "status": "development_ready",
            "mode": "deterministic_development",
            "authEpoch": 7,
            "developmentOnly": true,
            "destinationLabel": "Deterministic local model - development only",
            "signInAvailable": false,
            "signOutAvailable": true,
        })
    );

    assert!(project_model_auth_status(ModelAuthStatusInput {
        status: ModelAuthStatusKindProjection::DevelopmentReady,
        mode: ModelAuthModeProjection::Offline,
        auth_epoch: 7,
        development_only: true,
        destination_label: "Model support unavailable".to_owned(),
    })
    .is_err());
    assert!(project_model_auth_status(ModelAuthStatusInput {
        status: ModelAuthStatusKindProjection::Unavailable,
        mode: ModelAuthModeProjection::Offline,
        auth_epoch: 0,
        development_only: false,
        destination_label: "Model support unavailable".to_owned(),
    })
    .is_err());
    Ok(())
}

#[test]
fn review_projection_contains_exact_inert_bytes_but_no_authority_hashes(
) -> Result<(), Box<dyn std::error::Error>> {
    let projection = project_bmad_help_review(BmadHelpReviewInput {
        workspace_id: id("workspace_test")?,
        workspace_grant_epoch: 1,
        run_id: id("run_test")?,
        session_id: id("session_test")?,
        destination_label: "Deterministic local model - development only".to_owned(),
        development_only: true,
        consent_disclosure: "Only these exact reviewed bytes will be sent once.".to_owned(),
        manifest_hash: Sha256Digest::parse(MANIFEST_HASH)?,
        purpose: "bmad_help".to_owned(),
        region: "localdev".to_owned(),
        retention_mode: BmadHelpRetentionProjection::TransientNoStore,
        expires_at: UnixMillis(20_000),
        items: vec![BmadHelpReviewItemInput {
            relative_label: RelativeWorkspacePath::new("review/current-intent.txt")?,
            semantic_role: "current_intent".to_owned(),
            language: Some("text".to_owned()),
            outbound_byte_count: 25,
            token_estimate: 6,
            classification: BmadHelpContextClassificationProjection::Internal,
            redactions: vec![BmadHelpReviewRedactionInput {
                kind: "secret_pattern".to_owned(),
                occurrence_count: 1,
            }],
            outbound_content: "exact inert outbound text".to_owned(),
        }],
        exclusions: vec![BmadHelpReviewExclusionInput {
            relative_label: RelativeWorkspacePath::new(".env")?,
            reason: "secret-bearing filename".to_owned(),
        }],
        secret_findings: vec![BmadHelpSecretFindingInput {
            relative_label: RelativeWorkspacePath::new("review/current-intent.txt")?,
            kind: "secret_pattern".to_owned(),
            occurrence_count: 1,
        }],
        total_outbound_bytes: 25,
        total_token_estimate: 6,
        redaction_limitation: "Redaction reduces risk but cannot prove absence.".to_owned(),
    })?;

    let value = serde_json::to_value(&projection)?;
    assert_eq!(value["manifestHash"], MANIFEST_HASH);
    assert_eq!(
        value["items"][0]["outboundContent"],
        "exact inert outbound text"
    );
    assert_eq!(
        value["secretFindings"][0]["relativeLabel"],
        "review/current-intent.txt"
    );
    let serialized = serde_json::to_string(&value)?;
    for forbidden in [
        "providerProfileHash",
        "modelProfileHash",
        "deploymentHash",
        "outboundContentHash",
        "consentDisclosureHash",
        "schemaHash",
        "receiptProof",
        "accessToken",
        "authorityHash",
    ] {
        assert!(!serialized.contains(forbidden), "leaked {forbidden}");
    }

    let approval = project_bmad_help_approved(BmadHelpApprovalInput {
        manifest_hash: Sha256Digest::parse(MANIFEST_HASH)?,
        decision_id: id("decision_test")?,
        expires_at: UnixMillis(20_000),
    })?;
    let approved_lifecycle = project_bmad_help_approved_lifecycle(BmadHelpApprovedLifecycleInput {
        review: projection,
        approval,
    });
    let lifecycle_value = serde_json::to_value(approved_lifecycle)?;
    assert_eq!(lifecycle_value["review"]["manifestHash"], MANIFEST_HASH);
    assert_eq!(lifecycle_value["approval"]["decisionId"], "decision_test");
    Ok(())
}

#[test]
fn review_projection_rejects_count_drift_and_oversized_exact_bytes(
) -> Result<(), Box<dyn std::error::Error>> {
    let base = || BmadHelpReviewInput {
        workspace_id: id("workspace_test").expect("workspace id"),
        workspace_grant_epoch: 1,
        run_id: id("run_test").expect("run id"),
        session_id: id("session_test").expect("session id"),
        destination_label: "Deterministic local model - development only".to_owned(),
        development_only: true,
        consent_disclosure: "Only these exact reviewed bytes will be sent once.".to_owned(),
        manifest_hash: Sha256Digest::parse(MANIFEST_HASH).expect("digest"),
        purpose: "bmad_help".to_owned(),
        region: "localdev".to_owned(),
        retention_mode: BmadHelpRetentionProjection::TransientNoStore,
        expires_at: UnixMillis(20_000),
        items: vec![BmadHelpReviewItemInput {
            relative_label: RelativeWorkspacePath::new("review/current-intent.txt")
                .expect("relative label"),
            semantic_role: "current_intent".to_owned(),
            language: Some("text".to_owned()),
            outbound_byte_count: 5,
            token_estimate: 2,
            classification: BmadHelpContextClassificationProjection::Internal,
            redactions: Vec::new(),
            outbound_content: "exact".to_owned(),
        }],
        exclusions: Vec::new(),
        secret_findings: Vec::new(),
        total_outbound_bytes: 6,
        total_token_estimate: 2,
        redaction_limitation: "Redaction reduces risk but cannot prove absence.".to_owned(),
    };
    assert!(project_bmad_help_review(base()).is_err());

    let mut oversized = base();
    oversized.items[0].outbound_content = "x".repeat(96 * 1024);
    oversized.items[0].outbound_byte_count = 96 * 1024;
    oversized.total_outbound_bytes = 96 * 1024;
    assert!(project_bmad_help_review(oversized).is_err());

    let mut absolute_path = base();
    absolute_path.items[0].outbound_content = "/home/rodrigo/project".to_owned();
    absolute_path.items[0].outbound_byte_count = 21;
    absolute_path.total_outbound_bytes = 21;
    assert!(project_bmad_help_review(absolute_path).is_err());
    Ok(())
}

#[test]
fn approval_and_cancellation_projections_are_exact_and_authority_minimal(
) -> Result<(), Box<dyn std::error::Error>> {
    let manifest_hash = Sha256Digest::parse(MANIFEST_HASH)?;
    let approved = project_bmad_help_approved(BmadHelpApprovalInput {
        manifest_hash,
        decision_id: id("decision_test")?,
        expires_at: UnixMillis(20_000),
    })?;
    assert_eq!(
        serde_json::to_value(approved)?,
        json!({
            "manifestHash": MANIFEST_HASH,
            "decisionId": "decision_test",
            "expiresAt": 20_000,
            "sendEligible": true,
        })
    );

    let cancelled = project_bmad_help_cancelled(BmadHelpCancellationInput {
        manifest_hash,
        decision_id: id("decision_test")?,
    });
    assert_eq!(
        serde_json::to_value(cancelled)?,
        json!({
            "manifestHash": MANIFEST_HASH,
            "decisionId": "decision_test",
        })
    );

    assert!(project_bmad_help_approved(BmadHelpApprovalInput {
        manifest_hash,
        decision_id: id("decision_test")?,
        expires_at: UnixMillis(0),
    })
    .is_err());
    Ok(())
}

#[test]
fn d2_error_codes_have_exact_renderer_safe_names() -> Result<(), Box<dyn std::error::Error>> {
    let codes = [
        (LocalErrorCode::IdentityUnavailable, "identity_unavailable"),
        (
            LocalErrorCode::AuthenticationRequired,
            "authentication_required",
        ),
        (
            LocalErrorCode::ReauthenticationRequired,
            "reauthentication_required",
        ),
        (LocalErrorCode::TenantMismatch, "tenant_mismatch"),
        (
            LocalErrorCode::EntitlementUnavailable,
            "entitlement_unavailable",
        ),
        (LocalErrorCode::FeatureDisabled, "feature_disabled"),
        (LocalErrorCode::ContextRejected, "context_rejected"),
        (LocalErrorCode::ContextDrift, "context_drift"),
        (LocalErrorCode::ConsentRequired, "consent_required"),
        (LocalErrorCode::ConsentExpired, "consent_expired"),
        (
            LocalErrorCode::ConsentBindingMismatch,
            "consent_binding_mismatch",
        ),
        (
            LocalErrorCode::ConsentAlreadyConsumed,
            "consent_already_consumed",
        ),
        (LocalErrorCode::SupportPlaneOffline, "support_plane_offline"),
        (LocalErrorCode::TransportFailed, "transport_failed"),
        (
            LocalErrorCode::ResponseBindingMismatch,
            "response_binding_mismatch",
        ),
        (LocalErrorCode::InvalidModelOutput, "invalid_model_output"),
        (LocalErrorCode::ReceiptInvalid, "receipt_invalid"),
    ];
    for (code, expected) in codes {
        assert_eq!(serde_json::to_value(code)?, expected);
    }
    Ok(())
}

#[test]
fn terminal_projection_is_closed_and_never_resumable() -> Result<(), Box<dyn std::error::Error>> {
    let projection = project_bmad_help_terminal(BmadHelpTerminalInput {
        workspace_id: id("workspace_test")?,
        reason: BmadHelpTerminalReasonProjection::ConsentConsumed,
    });
    assert_eq!(
        serde_json::to_value(projection)?,
        json!({
            "workspaceId": "workspace_test",
            "reason": "consent_consumed",
            "resumable": false,
            "sendEligible": false,
        })
    );
    Ok(())
}
