#![allow(clippy::expect_used)]

use desktop_runtime::{
    sha256_bytes, AuthorityRef, BmadArtifactEvidence, BmadCapabilityKey, BmadHelpActionKey,
    BmadKernelErrorCode, ContractId, CreateMethodSession, MethodAdvanceDisposition,
    MethodAdvanceRequest, MethodAdvanceResult, MethodAgentBinding, MethodArtifactExpectation,
    MethodContextDecision, MethodErrorCode, MethodEvidenceClass, MethodExactBinding,
    MethodExecutionProfile, MethodExecutionProfileData, MethodInvocationModes, MethodModelBinding,
    MethodModelBindingData, MethodResourcePolicy, MethodSession, MethodState, MethodStepTable,
    UnixMillis,
};

fn id(value: &str) -> ContractId {
    ContractId::new(value).expect("test identifiers are valid")
}

fn binding(seed: u8) -> MethodExactBinding {
    let digest = |label: &str| sha256_bytes(format!("{seed}:{label}").as_bytes());
    let execution_profile = MethodExecutionProfile::from_source(
        MethodExecutionProfileData {
            entrypoint_kind: "step_jit".to_owned(),
            invocation_modes: MethodInvocationModes {
                interactive: true,
                headless: false,
                actions: vec!["create".to_owned()],
            },
            required_runtimes: Vec::new(),
            resource_policy: MethodResourcePolicy {
                entrypoint_timing: "invocation_start".to_owned(),
                resource_timing: "current_step_only".to_owned(),
                declared_resource_paths: Vec::new(),
            },
            declared_tool_intents: Vec::new(),
            state_hints: vec!["artifact_workspace".to_owned()],
            completion_evidence: vec!["artifact".to_owned()],
            customization_profile: "method_agent_toml".to_owned(),
            validation_profile: "MethodStepWorkflowV6".to_owned(),
        },
        digest("execution"),
    )
    .expect("execution profile");
    let model_binding = MethodModelBinding::from_source(
        MethodModelBindingData {
            binding_kind: "method_model".to_owned(),
            provider_id: "test-provider".to_owned(),
            model_id: "test-model".to_owned(),
            deployment_id: "test-deployment".to_owned(),
            model_profile_hash: digest("model-profile"),
            model_capability_hash: digest("model-capability"),
            context_window_profile_hash: digest("context-window"),
            egress_profile_hash: digest("egress"),
            request_schema_hash: digest("request-schema"),
            response_schema_hash: digest("response-schema"),
        },
        digest("model"),
    )
    .expect("model binding");
    MethodExactBinding {
        capability_key: BmadCapabilityKey {
            package_version_id: id("pkgver_01J00000000000000000000000"),
            module_code: "bmm".to_owned(),
            skill_name: "bmad-architecture".to_owned(),
            normalized_action: Some("create".to_owned()),
        },
        package_descriptor_hash: digest("descriptor"),
        package_source_hash: digest("source"),
        instruction_projection_hash: digest("instructions"),
        capability_catalog_hash: digest("catalog"),
        agent_roster_hash: None,
        agent_binding_hash: None,
        agent_binding: None,
        distribution_profile: "sapphirus_package".to_owned(),
        install_profile: "SapphirusManagedV1".to_owned(),
        entrypoint_kind: "step_jit".to_owned(),
        execution_profile_hash: execution_profile.profile_hash,
        execution_profile,
        validation_profile: "MethodStepWorkflowV6".to_owned(),
        validation_profile_hash: digest("validation"),
        config_graph_hash: digest("config-graph"),
        config_resolution_hash: digest("config"),
        customization_hash: digest("customization"),
        resource_set_hash: digest("resources"),
        model_binding_hash: model_binding.binding_hash,
        model_binding,
        method_schema_hash: digest("schema"),
        egress_profile_hash: digest("egress"),
        artifact_expectations: Vec::new(),
    }
}

fn decision(exact: &MethodExactBinding, value: &str) -> MethodContextDecision {
    MethodContextDecision {
        decision_id: id(value),
        manifest_hash: sha256_bytes(b"manifest"),
        consent_hash: sha256_bytes(b"consent"),
        context_digest: sha256_bytes(b"context"),
        binding_hash: exact.binding_hash().expect("binding hashes"),
        reviewed_at: UnixMillis(1_000),
    }
}

fn create_session() -> MethodSession {
    MethodSession::create(CreateMethodSession {
        session_id: id("session_01J00000000000000000000000"),
        owner_scope_ref: id("ownerscope_01J00000000000000000000000"),
        project_id: id("project_01J00000000000000000000000"),
        run_id: id("run_01J00000000000000000000000"),
        authority_ref: AuthorityRef {
            authority_kind: "desktop_local_store".to_owned(),
            authority_id: id("authority_01J00000000000000000000000"),
            installation_id: id("install_01J00000000000000000000000"),
            local_store_id: id("store_01J00000000000000000000000"),
            authority_epoch: 1,
        },
        created_at: UnixMillis(1_000),
    })
    .expect("session creation")
}

fn ready_session() -> (MethodSession, MethodExactBinding, MethodContextDecision) {
    let mut session = create_session();
    let exact = binding(1);
    session
        .bind_capability(
            1,
            exact.clone(),
            MethodStepTable::new("discover", [("discover", Some("decide")), ("decide", None)])
                .expect("step table"),
        )
        .expect("capability binding");
    session
        .request_context_review(2)
        .expect("context review request");
    let review = decision(&exact, "decision_01J00000000000000000000000");
    session
        .record_context_review(3, review.clone())
        .expect("accepted context review");
    (session, exact, review)
}

#[test]
fn method_state_machine_requires_exact_steps_and_new_review_per_invocation() {
    let (mut session, _exact, first_decision) = ready_session();
    assert_eq!(session.state(), MethodState::Ready);
    assert_eq!(session.version(), 4);

    let first = MethodAdvanceRequest {
        invocation_id: id("invoke_01J00000000000000000000000"),
        idempotency_key: "advance-1".to_owned(),
        decision_id: first_decision.decision_id.clone(),
        expected_version: 4,
    };
    let receipt = session.begin_advance(first.clone()).expect("begin advance");
    assert_eq!(session.state(), MethodState::Advancing);
    assert_eq!(
        session
            .begin_advance(first.clone())
            .expect("idempotent retry"),
        receipt
    );
    let mut stale_retry = first;
    stale_retry.expected_version = 5;
    assert_eq!(
        session
            .begin_advance(stale_retry)
            .expect_err("idempotent retries still bind the observed version")
            .code(),
        MethodErrorCode::MethodStateConflict
    );

    let invented_step = session.accept_result(
        5,
        &receipt.invocation_id,
        MethodAdvanceResult {
            disposition: MethodAdvanceDisposition::ContextReviewRequired,
            current_step_key: "invented".to_owned(),
            next_step_key: Some("decide".to_owned()),
            working_artifact_refs: Vec::new(),
        },
        UnixMillis(2_000),
    );
    assert_eq!(
        invented_step.expect_err("invented steps fail").code(),
        MethodErrorCode::MethodResultInvalid
    );

    session
        .accept_result(
            5,
            &receipt.invocation_id,
            MethodAdvanceResult {
                disposition: MethodAdvanceDisposition::ContextReviewRequired,
                current_step_key: "discover".to_owned(),
                next_step_key: Some("decide".to_owned()),
                working_artifact_refs: Vec::new(),
            },
            UnixMillis(2_000),
        )
        .expect("accepted result");
    assert_eq!(session.turn_ordinal(), 1);
    assert_eq!(session.state(), MethodState::ContextReviewRequired);
    assert_eq!(session.checkpoints().len(), 1);

    let replay = session.begin_advance(MethodAdvanceRequest {
        invocation_id: id("invoke_01J11111111111111111111111"),
        idempotency_key: "advance-2".to_owned(),
        decision_id: first_decision.decision_id,
        expected_version: 6,
    });
    assert_eq!(
        replay
            .expect_err("a consumed decision never revives")
            .code(),
        MethodErrorCode::ContextDecisionAlreadyConsumed
    );
}

#[test]
fn drifted_binding_invalidates_review_and_resume_is_read_only() {
    let (mut session, _exact, decision) = ready_session();
    let original_version = session.version();
    let drifted = binding(2);
    assert_ne!(
        decision.binding_hash,
        drifted.binding_hash().expect("binding hash")
    );
    assert_eq!(
        session
            .validate_review_for(&drifted)
            .expect_err("drift invalidates review")
            .code(),
        MethodErrorCode::MethodResourceDrift,
    );
    assert!(session.resume().is_none());
    assert_eq!(session.version(), original_version);

    session
        .cancel(original_version)
        .expect("cancel ready session");
    assert_eq!(session.state(), MethodState::Cancelled);
}

#[test]
fn authoritative_help_evidence_rejects_a_pre_rebind_invocation() {
    let mut session = create_session();
    let first_binding = binding(1);
    session
        .bind_capability(
            1,
            first_binding.clone(),
            MethodStepTable::new("discover", [("discover", Some("decide")), ("decide", None)])
                .expect("first step table"),
        )
        .expect("first capability");
    session.request_context_review(2).expect("first review");
    let first_decision = decision(&first_binding, "decision_01J11111111111111111111111");
    session
        .record_context_review(3, first_decision.clone())
        .expect("first decision");
    let first_invocation = id("invoke_01J11111111111111111111111");
    session
        .begin_advance(MethodAdvanceRequest {
            invocation_id: first_invocation.clone(),
            idempotency_key: "pre-rebind".to_owned(),
            decision_id: first_decision.decision_id,
            expected_version: 4,
        })
        .expect("first invocation");
    session
        .accept_result(
            5,
            &first_invocation,
            MethodAdvanceResult {
                disposition: MethodAdvanceDisposition::AwaitingUser,
                current_step_key: "discover".to_owned(),
                next_step_key: Some("decide".to_owned()),
                working_artifact_refs: Vec::new(),
            },
            UnixMillis(2_000),
        )
        .expect("first checkpoint");

    let mut second_binding = binding(2);
    second_binding.capability_key.normalized_action = Some("validate".to_owned());
    session
        .rebind_capability(
            6,
            second_binding.clone(),
            MethodStepTable::new("only", [("only", None)]).expect("second step table"),
        )
        .expect("second capability");
    session.request_context_review(7).expect("second review");
    let second_decision = decision(&second_binding, "decision_01J22222222222222222222222");
    session
        .record_context_review(8, second_decision.clone())
        .expect("second decision");
    let second_invocation = id("invoke_01J22222222222222222222222");
    session
        .begin_advance(MethodAdvanceRequest {
            invocation_id: second_invocation.clone(),
            idempotency_key: "post-rebind".to_owned(),
            decision_id: second_decision.decision_id,
            expected_version: 9,
        })
        .expect("second invocation");
    session
        .accept_result(
            10,
            &second_invocation,
            MethodAdvanceResult {
                disposition: MethodAdvanceDisposition::Completed,
                current_step_key: "only".to_owned(),
                next_step_key: None,
                working_artifact_refs: Vec::new(),
            },
            UnixMillis(3_000),
        )
        .expect("completed second capability");

    let action = BmadHelpActionKey {
        package_version_id: id("pkgver_01J00000000000000000000000"),
        module_code: "bmm".to_owned(),
        skill_name: "bmad-architecture".to_owned(),
        action: Some("validate".to_owned()),
    };
    assert_eq!(
        BmadArtifactEvidence::from_completed_session(action, &session, &first_invocation)
            .expect_err("old invocation cannot evidence the rebound capability")
            .code(),
        BmadKernelErrorCode::HelpEvidenceInsufficient
    );
}

#[test]
fn model_result_parser_rejects_authority_and_tool_smuggling() {
    let source = br#"{
      "disposition":"completed",
      "currentStepKey":"decide",
      "nextStepKey":null,
      "workingArtifactRefs":[],
      "authority":{"kind":"model"},
      "tools":["shell"]
    }"#;
    assert_eq!(
        MethodAdvanceResult::parse_json(source)
            .expect_err("unknown authority fields fail closed")
            .code(),
        MethodErrorCode::MethodResultInvalid,
    );
}

#[test]
fn iterative_turns_require_fresh_decisions_and_finish_on_the_handwritten_table() {
    let (mut session, exact, first_decision) = ready_session();
    let first = session
        .begin_advance(MethodAdvanceRequest {
            invocation_id: id("invoke_01J00000000000000000000000"),
            idempotency_key: "turn-one".to_owned(),
            decision_id: first_decision.decision_id,
            expected_version: 4,
        })
        .expect("first advance");
    session
        .accept_result(
            5,
            &first.invocation_id,
            MethodAdvanceResult {
                disposition: MethodAdvanceDisposition::AwaitingUser,
                current_step_key: "discover".to_owned(),
                next_step_key: Some("decide".to_owned()),
                working_artifact_refs: Vec::new(),
            },
            UnixMillis(2_000),
        )
        .expect("await user");
    session.record_user_turn(6).expect("user turn");
    let second_decision = decision(&exact, "decision_01J11111111111111111111111");
    session
        .record_context_review(7, second_decision.clone())
        .expect("fresh review");
    let second = session
        .begin_advance(MethodAdvanceRequest {
            invocation_id: id("invoke_01J11111111111111111111111"),
            idempotency_key: "turn-two".to_owned(),
            decision_id: second_decision.decision_id,
            expected_version: 8,
        })
        .expect("second advance");
    session
        .accept_result(
            9,
            &second.invocation_id,
            MethodAdvanceResult {
                disposition: MethodAdvanceDisposition::Completed,
                current_step_key: "decide".to_owned(),
                next_step_key: None,
                working_artifact_refs: Vec::new(),
            },
            UnixMillis(3_000),
        )
        .expect("complete");
    assert_eq!(session.state(), MethodState::Completed);
    assert_eq!(session.turn_ordinal(), 2);
    assert_eq!(session.resume().map(|value| value.turn_ordinal), Some(2));
    assert_eq!(session.version(), 10);
}

#[test]
fn drift_categories_and_terminal_failure_transitions_are_stable() {
    let (mut session, exact, review) = ready_session();
    let mut model_drift = exact.clone();
    model_drift.model_binding_hash = sha256_bytes(b"different model");
    assert_eq!(
        session
            .validate_review_for(&model_drift)
            .expect_err("model drift")
            .code(),
        MethodErrorCode::MethodModelBindingDrift,
    );
    let mut package_drift = exact;
    package_drift.package_source_hash = sha256_bytes(b"different package");
    assert_eq!(
        session
            .validate_review_for(&package_drift)
            .expect_err("package drift")
            .code(),
        MethodErrorCode::MethodBindingStale,
    );

    let receipt = session
        .begin_advance(MethodAdvanceRequest {
            invocation_id: id("invoke_01J00000000000000000000000"),
            idempotency_key: "refusal".to_owned(),
            decision_id: review.decision_id,
            expected_version: 4,
        })
        .expect("advance");
    assert_eq!(receipt.aggregate_version, 5);
    session.record_refusal(5).expect("refusal");
    assert_eq!(session.state(), MethodState::Refused);
    assert_eq!(session.turn_ordinal(), 0);
}

#[test]
fn unsupported_profiles_fail_before_capability_binding() {
    let mut session = create_session();
    let mut unsupported = binding(1);
    unsupported.entrypoint_kind = "arbitrary_shell".to_owned();
    assert_eq!(
        session
            .bind_capability(
                1,
                unsupported,
                MethodStepTable::new("respond", [("respond", None)]).expect("steps"),
            )
            .expect_err("unsupported entrypoint")
            .code(),
        MethodErrorCode::MethodBindingStale,
    );
    assert_eq!(session.state(), MethodState::Created);
    assert_eq!(session.version(), 1);
}

#[test]
fn renderer_projection_contains_no_sensitive_or_authority_bytes() {
    let (session, _, _) = ready_session();
    let source = serde_json::to_string(&session.renderer_projection()).expect("projection json");
    for forbidden in [
        "authorityRef",
        "ownerScopeRef",
        "packageDescriptorHash",
        "contextDigest",
        "consentHash",
        "prompt",
        "token",
        "cas://",
        "C:\\",
    ] {
        assert!(!source.contains(forbidden), "projection leaked {forbidden}");
    }
}

#[test]
fn canonical_no_action_capability_key_is_bindable() {
    let mut session = create_session();
    let mut exact = binding(1);
    exact.capability_key.skill_name = "bmad-help".to_owned();
    exact.capability_key.normalized_action = None;
    session
        .bind_capability(
            1,
            exact,
            MethodStepTable::new("respond", [("respond", None)]).expect("steps"),
        )
        .expect("nullable action is canonical");
    assert_eq!(session.state(), MethodState::CapabilityBound);
}

#[test]
fn a_terminal_step_cannot_enter_a_nonterminal_state() {
    let mut session = create_session();
    let exact = binding(1);
    session
        .bind_capability(
            1,
            exact.clone(),
            MethodStepTable::new("respond", [("respond", None)]).expect("steps"),
        )
        .expect("bind");
    session.request_context_review(2).expect("review request");
    let review = decision(&exact, "decision_01J22222222222222222222222");
    session
        .record_context_review(3, review.clone())
        .expect("review");
    let receipt = session
        .begin_advance(MethodAdvanceRequest {
            invocation_id: id("invoke_01J22222222222222222222222"),
            idempotency_key: "terminal-disposition".to_owned(),
            decision_id: review.decision_id,
            expected_version: 4,
        })
        .expect("advance");
    let error = session
        .accept_result(
            5,
            &receipt.invocation_id,
            MethodAdvanceResult {
                disposition: MethodAdvanceDisposition::AwaitingUser,
                current_step_key: "respond".to_owned(),
                next_step_key: None,
                working_artifact_refs: Vec::new(),
            },
            UnixMillis(2_000),
        )
        .expect_err("terminal table edge requires completed");
    assert_eq!(error.code(), MethodErrorCode::MethodResultInvalid);
    assert_eq!(session.state(), MethodState::Advancing);
}

#[test]
fn rebind_preserves_history_and_requires_a_fresh_review() {
    let (mut session, _, first_review) = ready_session();
    let first = session
        .begin_advance(MethodAdvanceRequest {
            invocation_id: id("invoke_01J33333333333333333333333"),
            idempotency_key: "pre-rebind-turn".to_owned(),
            decision_id: first_review.decision_id.clone(),
            expected_version: 4,
        })
        .expect("advance");
    session
        .accept_result(
            5,
            &first.invocation_id,
            MethodAdvanceResult {
                disposition: MethodAdvanceDisposition::ContextReviewRequired,
                current_step_key: "discover".to_owned(),
                next_step_key: Some("decide".to_owned()),
                working_artifact_refs: Vec::new(),
            },
            UnixMillis(2_000),
        )
        .expect("first checkpoint");

    let rebound = binding(2);
    session
        .rebind_capability(
            6,
            rebound.clone(),
            MethodStepTable::new("discover", [("discover", Some("decide")), ("decide", None)])
                .expect("steps"),
        )
        .expect("rebind");
    session
        .request_context_review(7)
        .expect("new review request");
    let fresh_review = decision(&rebound, "decision_01J33333333333333333333333");
    session
        .record_context_review(8, fresh_review.clone())
        .expect("fresh review");
    let restored =
        MethodSession::from_persisted_json(&session.to_persisted_json().expect("persisted state"))
            .expect("all binding revisions reconstruct");
    assert_eq!(restored.checkpoints()[0].binding_ordinal, 1);
    assert_eq!(restored.state(), MethodState::Ready);
    assert_eq!(
        restored
            .clone()
            .begin_advance(MethodAdvanceRequest {
                invocation_id: id("invoke_01J44444444444444444444444"),
                idempotency_key: "old-review-replay".to_owned(),
                decision_id: first_review.decision_id,
                expected_version: 9,
            })
            .expect_err("old review cannot authorize rebound inputs")
            .code(),
        MethodErrorCode::ContextDecisionAlreadyConsumed
    );
    let mut fresh = restored;
    fresh
        .begin_advance(MethodAdvanceRequest {
            invocation_id: id("invoke_01J55555555555555555555555"),
            idempotency_key: "fresh-review".to_owned(),
            decision_id: fresh_review.decision_id,
            expected_version: 9,
        })
        .expect("fresh review advances");
}

#[test]
fn evidence_class_values_are_closed() {
    assert_eq!(MethodEvidenceClass::Authoritative.as_str(), "authoritative");
    assert!(serde_json::from_str::<MethodEvidenceClass>("\"model_claimed\"").is_err());
}

#[test]
fn canonical_bmad_fixture_maps_without_changing_source_identity() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/contracts/fixtures/valid/bmad/method-no-agent-direct.json");
    let envelope: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("fixture source"))
            .expect("valid fixture json");
    let document = &envelope["payload"];
    let profile: MethodExecutionProfile =
        serde_json::from_value(document["executionProfile"].clone()).expect("exact profile");
    let model: MethodModelBinding =
        serde_json::from_value(document["modelBinding"].clone()).expect("exact model binding");
    assert_eq!(
        serde_json::to_value(&profile).expect("profile json"),
        document["executionProfile"]
    );
    assert_eq!(
        serde_json::to_value(&model).expect("model json"),
        document["modelBinding"]
    );
    let digest =
        |field: &str| serde_json::from_value(document[field].clone()).expect("fixture digest");
    let exact = MethodExactBinding {
        capability_key: serde_json::from_value(document["capabilityKey"].clone())
            .expect("capability key"),
        package_descriptor_hash: digest("packageDescriptorHash"),
        package_source_hash: digest("packageSourceHash"),
        instruction_projection_hash: digest("instructionProjectionHash"),
        capability_catalog_hash: digest("capabilityCatalogHash"),
        agent_roster_hash: None,
        agent_binding_hash: None,
        agent_binding: None,
        distribution_profile: document["distributionProfile"]
            .as_str()
            .expect("distribution profile")
            .to_owned(),
        install_profile: document["installProfile"]
            .as_str()
            .expect("install profile")
            .to_owned(),
        entrypoint_kind: profile.data.entrypoint_kind.clone(),
        execution_profile_hash: digest("executionProfileHash"),
        execution_profile: profile,
        validation_profile: document["validationProfile"]
            .as_str()
            .expect("validation profile")
            .to_owned(),
        validation_profile_hash: digest("validationProfileHash"),
        config_graph_hash: digest("configGraphHash"),
        config_resolution_hash: digest("configResolutionHash"),
        customization_hash: digest("customizationHash"),
        resource_set_hash: digest("resourceSetHash"),
        model_binding_hash: model.binding_hash,
        egress_profile_hash: model.data.egress_profile_hash,
        model_binding: model,
        method_schema_hash: digest("methodSchemaHash"),
        artifact_expectations: serde_json::from_value(document["artifactExpectations"].clone())
            .expect("artifact expectations"),
    };
    exact
        .binding_hash()
        .expect("canonical source binding remains structurally valid");
}

#[test]
fn canonical_agent_binding_round_trips_without_changing_source_identity() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/contracts/fixtures/valid/bmad/method-architect-iterative.json");
    let envelope: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("fixture source"))
            .expect("valid fixture json");
    let source = &envelope["payload"]["agentBinding"];
    let agent: MethodAgentBinding =
        serde_json::from_value(source.clone()).expect("exact agent binding");
    assert_eq!(serde_json::to_value(&agent).expect("agent json"), *source);
}

#[test]
fn valid_unique_source_arrays_preserve_their_original_order() {
    let mut exact = binding(7);
    exact.execution_profile.data.invocation_modes.actions =
        vec!["update".to_owned(), "create".to_owned()];
    exact
        .execution_profile
        .data
        .resource_policy
        .declared_resource_paths = vec!["z-last.md".to_owned(), "a-first.md".to_owned()];
    exact.execution_profile.data.declared_tool_intents =
        vec!["web".to_owned(), "file_read".to_owned()];
    exact.execution_profile.data.state_hints =
        vec!["story".to_owned(), "artifact_workspace".to_owned()];
    exact.execution_profile.data.completion_evidence =
        vec!["status_evidence".to_owned(), "artifact".to_owned()];
    exact.artifact_expectations = vec![
        artifact_expectation("expectation_01J22222222222222222222222", "second"),
        artifact_expectation("expectation_01J11111111111111111111111", "first"),
    ];

    exact
        .binding_hash()
        .expect("BMAD uniqueness does not imply source-order normalization");

    let mut duplicate = exact;
    duplicate.execution_profile.data.invocation_modes.actions =
        vec!["create".to_owned(), "create".to_owned()];
    assert_eq!(
        duplicate
            .binding_hash()
            .expect_err("duplicate source members remain invalid")
            .code(),
        MethodErrorCode::MethodBindingStale
    );
}

fn artifact_expectation(id_value: &str, artifact_kind: &str) -> MethodArtifactExpectation {
    MethodArtifactExpectation {
        expectation_kind: "method_artifact".to_owned(),
        expectation_id: id(id_value),
        artifact_kind: artifact_kind.to_owned(),
        required: false,
        storage_scope: "app_local".to_owned(),
        expected_media_type: "text/markdown".to_owned(),
        expected_content_schema_hash: None,
        source_output_hint: None,
        completion_evidence_class: MethodEvidenceClass::Authoritative,
        expectation_hash: sha256_bytes(format!("{artifact_kind}:source-hash").as_bytes()),
    }
}
