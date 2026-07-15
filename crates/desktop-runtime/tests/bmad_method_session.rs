#![allow(clippy::expect_used)]

use desktop_runtime::{
    canonical_hash, sha256_bytes, AuthorityRef, BmadArtifactEvidence, BmadCapabilityKey,
    BmadHelpActionKey, BmadKernelErrorCode, ContractId, CreateMethodSession,
    MethodAdvanceDisposition, MethodAdvanceReceipt, MethodAdvanceRequest, MethodAdvanceResult,
    MethodAgentBinding, MethodArtifactExpectation, MethodCheckpoint, MethodContextDecision,
    MethodError, MethodErrorCode, MethodEvidenceClass, MethodExactBinding, MethodExecutionProfile,
    MethodExecutionProfileData, MethodInvocationModes, MethodModelBinding, MethodModelBindingData,
    MethodResourcePolicy, MethodSession, MethodState, MethodStepTable, MethodVerifiedAdvanceResult,
    MethodVerifiedResultBindingData, UnixMillis,
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
        capability_catalog_hash: sha256_bytes(b"2:catalog"),
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

fn advance_request(
    session: &MethodSession,
    invocation_value: &str,
    idempotency_key: &str,
    decision_id: ContractId,
    expected_version: u64,
) -> MethodAdvanceRequest {
    let invocation_id = id(invocation_value);
    let session_authority_hash = session
        .session_authority_hash()
        .expect("session authority hash");
    let d2_model_invocation_binding_hash =
        sha256_bytes(format!("{}:d2-model-invocation-binding", invocation_id.as_str()).as_bytes());
    let model_bridge_binding_hash = session
        .model_bridge_binding_hash(&d2_model_invocation_binding_hash)
        .expect("Method/D2 bridge binding hash");
    MethodAdvanceRequest {
        model_request_id: id(&invocation_id.as_str().replacen("invoke_", "modelreq_", 1)),
        decision_consumption_hash: sha256_bytes(
            format!("{}:decision-consumption", invocation_id.as_str()).as_bytes(),
        ),
        model_request_hash: sha256_bytes(
            format!("{}:model-request", invocation_id.as_str()).as_bytes(),
        ),
        session_authority_hash,
        d2_model_invocation_binding_hash,
        model_bridge_binding_hash,
        invocation_id,
        idempotency_key: idempotency_key.to_owned(),
        decision_id,
        expected_version,
    }
}

fn accepted_result(
    disposition: MethodAdvanceDisposition,
    current_step_key: &str,
    next_step_key: Option<&str>,
) -> MethodAdvanceResult {
    MethodAdvanceResult {
        disposition,
        current_step_key: current_step_key.to_owned(),
        next_step_key: next_step_key.map(str::to_owned),
        working_artifact_refs: Vec::new(),
    }
}

fn verified_binding(
    exact: &MethodExactBinding,
    receipt: &MethodAdvanceReceipt,
    result: &MethodAdvanceResult,
) -> MethodVerifiedResultBindingData {
    MethodVerifiedResultBindingData {
        invocation_id: receipt.invocation_id.clone(),
        decision_id: receipt.decision_id.clone(),
        decision_consumption_hash: receipt.decision_consumption_hash,
        model_request_id: receipt.model_request_id.clone(),
        model_request_hash: receipt.model_request_hash,
        session_authority_hash: receipt.session_authority_hash,
        d2_model_invocation_binding_hash: receipt.d2_model_invocation_binding_hash,
        model_bridge_binding_hash: receipt.model_bridge_binding_hash,
        method_binding_hash: exact.binding_hash().expect("exact binding hash"),
        model_binding_hash: exact.model_binding_hash,
        response_schema_hash: exact.model_binding.data.response_schema_hash,
        model_response_payload_hash: sha256_bytes(
            format!("{}:exact-raw-json-bytes", receipt.model_request_id.as_str()).as_bytes(),
        ),
        accepted_method_result_hash: canonical_hash("bmad-method-advance-result", 1, result)
            .expect("accepted Method result hash"),
        model_receipt_evidence_hash: canonical_hash(
            "model-access-receipt-evidence",
            1,
            &(
                receipt.model_request_id.as_str(),
                receipt.model_request_hash,
                "complete-already-verified-test-receipt",
            ),
        )
        .expect("trusted-host receipt evidence hash"),
    }
}

fn verified_result(
    exact: &MethodExactBinding,
    receipt: &MethodAdvanceReceipt,
    result: MethodAdvanceResult,
) -> MethodVerifiedAdvanceResult {
    let binding = verified_binding(exact, receipt, &result);
    MethodVerifiedAdvanceResult::from_trusted_host_evidence(result, binding)
        .expect("sealed trusted-host result evidence")
}

fn advancing_session() -> (
    MethodSession,
    MethodExactBinding,
    MethodAdvanceReceipt,
    MethodAdvanceResult,
) {
    let (mut session, exact, review) = ready_session();
    let request = advance_request(
        &session,
        "invoke_01J00000000000000000000000",
        "verified-result",
        review.decision_id,
        4,
    );
    let receipt = session.begin_advance(request).expect("begin advance");
    let result = accepted_result(
        MethodAdvanceDisposition::ContextReviewRequired,
        "discover",
        Some("decide"),
    );
    (session, exact, receipt, result)
}

#[test]
fn transition_api_accepts_only_sealed_verified_results() {
    let _: fn(
        &mut MethodSession,
        u64,
        MethodVerifiedAdvanceResult,
        UnixMillis,
    ) -> Result<MethodCheckpoint, MethodError> = MethodSession::accept_result;
}

#[test]
fn verified_result_constructor_rejects_a_noncanonical_accepted_result_hash() {
    let (_session, exact, receipt, result) = advancing_session();
    let mut proof = verified_binding(&exact, &receipt, &result);
    proof.accepted_method_result_hash =
        sha256_bytes(b"raw-json-payload-hash-is-not-the-bmad-result-hash");

    assert_eq!(
        MethodVerifiedAdvanceResult::from_trusted_host_evidence(result, proof)
            .expect_err("the claimed result hash must bind the accepted BMAD projection")
            .code(),
        MethodErrorCode::MethodResultInvalid
    );
}

#[test]
fn raw_response_bytes_and_the_accepted_method_projection_have_distinct_hash_domains() {
    let compact = br#"{"disposition":"completed","currentStepKey":"respond","nextStepKey":null,"workingArtifactRefs":[]}"#;
    let reordered = br#"{
      "workingArtifactRefs": [],
      "nextStepKey": null,
      "currentStepKey": "respond",
      "disposition": "completed"
    }"#;
    let compact_result = MethodAdvanceResult::parse_json(compact).expect("compact result");
    let reordered_result = MethodAdvanceResult::parse_json(reordered).expect("reordered result");

    assert_eq!(compact_result, reordered_result);
    assert_ne!(sha256_bytes(compact), sha256_bytes(reordered));
    assert_eq!(
        canonical_hash("bmad-method-advance-result", 1, &compact_result)
            .expect("compact accepted result hash"),
        canonical_hash("bmad-method-advance-result", 1, &reordered_result)
            .expect("reordered accepted result hash")
    );
}

#[test]
fn begin_advance_rejects_cross_session_and_bridge_drift_without_mutation() {
    let (session, exact, review) = ready_session();
    let baseline = session.clone();
    let mut foreign = MethodSession::create(CreateMethodSession {
        session_id: id("session_01J99999999999999999999999"),
        owner_scope_ref: id("ownerscope_01J99999999999999999999999"),
        project_id: id("project_01J99999999999999999999999"),
        run_id: id("run_01J99999999999999999999999"),
        authority_ref: AuthorityRef {
            authority_kind: "desktop_local_store".to_owned(),
            authority_id: id("authority_01J99999999999999999999999"),
            installation_id: id("install_01J99999999999999999999999"),
            local_store_id: id("store_01J99999999999999999999999"),
            authority_epoch: 1,
        },
        created_at: UnixMillis(1_000),
    })
    .expect("foreign session");
    foreign
        .bind_capability(
            1,
            exact,
            MethodStepTable::new("discover", [("discover", Some("decide")), ("decide", None)])
                .expect("foreign step table"),
        )
        .expect("foreign binding");

    let foreign_request = advance_request(
        &foreign,
        "invoke_01J88888888888888888888888",
        "cross-session",
        review.decision_id.clone(),
        4,
    );
    let mut valid_request = advance_request(
        &session,
        "invoke_01J77777777777777777777777",
        "bridge-drift",
        review.decision_id,
        4,
    );
    let mut d2_binding_drift = valid_request.clone();
    d2_binding_drift.d2_model_invocation_binding_hash =
        sha256_bytes(b"substituted-d2-invocation-binding");
    let mut bridge_hash_drift = valid_request.clone();
    bridge_hash_drift.model_bridge_binding_hash = sha256_bytes(b"substituted-bridge-binding");
    valid_request.session_authority_hash = sha256_bytes(b"substituted-session-authority");

    for request in [
        foreign_request,
        valid_request,
        d2_binding_drift,
        bridge_hash_drift,
    ] {
        let mut candidate = baseline.clone();
        assert_eq!(
            candidate
                .begin_advance(request)
                .expect_err("cross-session and bridge drift must fail before consumption")
                .code(),
            MethodErrorCode::MethodBindingStale
        );
        assert_eq!(candidate, baseline);
    }
}

#[test]
fn acceptance_rejects_every_drifted_pre_call_lineage_field_without_mutation() {
    let (session, exact, receipt, result) = advancing_session();
    let baseline = session.clone();
    let proof = verified_binding(&exact, &receipt, &result);
    let mut mismatches = Vec::new();

    let mut invocation = proof.clone();
    invocation.invocation_id = id("invoke_01J99999999999999999999999");
    mismatches.push(("invocation", invocation));
    let mut decision = proof.clone();
    decision.decision_id = id("decision_01J99999999999999999999999");
    mismatches.push(("decision", decision));
    let mut consumption = proof.clone();
    consumption.decision_consumption_hash = sha256_bytes(b"different-consumption");
    mismatches.push(("decision consumption", consumption));
    let mut request_id = proof.clone();
    request_id.model_request_id = id("modelreq_01J99999999999999999999999");
    mismatches.push(("model request id", request_id));
    let mut request_hash = proof.clone();
    request_hash.model_request_hash = sha256_bytes(b"different-request");
    mismatches.push(("model request hash", request_hash));
    let mut session_authority = proof.clone();
    session_authority.session_authority_hash = sha256_bytes(b"different-session-authority");
    mismatches.push(("session authority", session_authority));
    let mut d2_invocation_binding = proof.clone();
    d2_invocation_binding.d2_model_invocation_binding_hash =
        sha256_bytes(b"different-d2-invocation-binding");
    mismatches.push(("D2 invocation binding", d2_invocation_binding));
    let mut bridge_binding = proof.clone();
    bridge_binding.model_bridge_binding_hash = sha256_bytes(b"different-bridge-binding");
    mismatches.push(("Method/D2 bridge binding", bridge_binding));
    let mut method_binding = proof.clone();
    method_binding.method_binding_hash = sha256_bytes(b"different-method-binding");
    mismatches.push(("Method binding", method_binding));
    let mut model_binding = proof.clone();
    model_binding.model_binding_hash = sha256_bytes(b"different-model-binding");
    mismatches.push(("model binding", model_binding));
    let mut response_schema = proof;
    response_schema.response_schema_hash = sha256_bytes(b"different-response-schema");
    mismatches.push(("response schema", response_schema));

    for (field, mismatched_proof) in mismatches {
        let envelope = MethodVerifiedAdvanceResult::from_trusted_host_evidence(
            result.clone(),
            mismatched_proof,
        )
        .expect("the sealed envelope can carry trusted-host lineage evidence");
        let mut candidate = baseline.clone();
        let error = candidate
            .accept_result(5, envelope, UnixMillis(2_000))
            .expect_err("mismatched lineage must fail");
        assert_eq!(
            error.code(),
            MethodErrorCode::MethodResultInvalid,
            "unexpected error for {field}"
        );
        assert_eq!(candidate, baseline, "{field} rejection mutated authority");
    }
}

#[test]
fn valid_verified_result_writes_and_restores_every_exact_lineage_field() {
    let (mut session, exact, receipt, result) = advancing_session();
    let proof = verified_binding(&exact, &receipt, &result);
    assert_ne!(
        proof.accepted_method_result_hash, proof.model_response_payload_hash,
        "the accepted BMAD projection hash is distinct from exact raw-response bytes"
    );
    assert_ne!(
        proof.accepted_method_result_hash, proof.model_receipt_evidence_hash,
        "the accepted BMAD projection hash is distinct from trusted-host receipt evidence"
    );
    let envelope = MethodVerifiedAdvanceResult::from_trusted_host_evidence(result, proof.clone())
        .expect("trusted-host verified result envelope");
    let verification_hash = *envelope.verification_hash();

    let checkpoint = session
        .accept_result(5, envelope, UnixMillis(2_000))
        .expect("exact proof advances");
    assert_eq!(
        checkpoint.advance_disposition,
        MethodAdvanceDisposition::ContextReviewRequired
    );
    assert_eq!(checkpoint.method_binding_hash, proof.method_binding_hash);
    assert_eq!(
        checkpoint.decision_consumption_hash,
        proof.decision_consumption_hash
    );
    assert_eq!(checkpoint.model_request_id, proof.model_request_id);
    assert_eq!(checkpoint.model_request_hash, proof.model_request_hash);
    assert_eq!(
        checkpoint.session_authority_hash,
        proof.session_authority_hash
    );
    assert_eq!(
        checkpoint.d2_model_invocation_binding_hash,
        proof.d2_model_invocation_binding_hash
    );
    assert_eq!(
        checkpoint.model_bridge_binding_hash,
        proof.model_bridge_binding_hash
    );
    assert_eq!(checkpoint.model_binding_hash, proof.model_binding_hash);
    assert_eq!(checkpoint.response_schema_hash, proof.response_schema_hash);
    assert_eq!(
        checkpoint.model_response_payload_hash,
        proof.model_response_payload_hash
    );
    assert_eq!(
        checkpoint.accepted_method_result_hash,
        proof.accepted_method_result_hash
    );
    assert_eq!(
        checkpoint.model_receipt_evidence_hash,
        proof.model_receipt_evidence_hash
    );
    assert_eq!(checkpoint.verified_result_binding_hash, verification_hash);

    let persisted = session.to_persisted_json().expect("persisted state");
    let restored = MethodSession::from_persisted_json(&persisted).expect("proof-bound restart");
    assert_eq!(restored, session);

    let mut tampered: serde_json::Value =
        serde_json::from_str(&persisted).expect("persisted json value");
    tampered["checkpoints"][0]["modelReceiptEvidenceHash"] =
        serde_json::to_value(sha256_bytes(b"tampered-post-call-receipt"))
            .expect("tampered digest json");
    assert_eq!(
        MethodSession::from_persisted_json(
            &serde_json::to_string(&tampered).expect("tampered persisted json")
        )
        .expect_err("checkpoint lineage tampering fails recovery")
        .code(),
        MethodErrorCode::MethodStoreRecoveryRequired
    );

    let mut semantic_tamper: serde_json::Value =
        serde_json::from_str(&persisted).expect("persisted semantic tamper source");
    semantic_tamper["checkpoints"][0]["advanceDisposition"] =
        serde_json::Value::String("awaiting_user".to_owned());
    let mut checkpoint_hash_input = semantic_tamper["checkpoints"][0].clone();
    checkpoint_hash_input
        .as_object_mut()
        .expect("checkpoint object")
        .remove("checkpointHash");
    semantic_tamper["checkpoints"][0]["checkpointHash"] = serde_json::to_value(
        canonical_hash("bmad-method-checkpoint", 1, &checkpoint_hash_input)
            .expect("recomputed public checkpoint hash"),
    )
    .expect("recomputed checkpoint digest json");
    assert_eq!(
        MethodSession::from_persisted_json(
            &serde_json::to_string(&semantic_tamper).expect("semantic tamper json")
        )
        .expect_err("restore must recompute accepted projection semantics")
        .code(),
        MethodErrorCode::MethodStoreRecoveryRequired
    );
}

fn assert_replay_lineage_drift_is_rejected(
    session: &mut MethodSession,
    first: &MethodAdvanceRequest,
) {
    let replay_baseline = session.clone();
    let mut mismatched_replays = Vec::new();
    let mut invocation_drift = (*first).clone();
    invocation_drift.invocation_id = id("invoke_01J99999999999999999999999");
    mismatched_replays.push(invocation_drift);
    let mut decision_drift = (*first).clone();
    decision_drift.decision_id = id("decision_01J99999999999999999999999");
    mismatched_replays.push(decision_drift);
    let mut consumption_drift = (*first).clone();
    consumption_drift.decision_consumption_hash = sha256_bytes(b"replay-consumption-drift");
    mismatched_replays.push(consumption_drift);
    let mut request_id_drift = (*first).clone();
    request_id_drift.model_request_id = id("modelreq_01J99999999999999999999999");
    mismatched_replays.push(request_id_drift);
    let mut request_hash_drift = (*first).clone();
    request_hash_drift.model_request_hash = sha256_bytes(b"replay-request-drift");
    mismatched_replays.push(request_hash_drift);
    let mut authority_drift = (*first).clone();
    authority_drift.session_authority_hash = sha256_bytes(b"replay-authority-drift");
    mismatched_replays.push(authority_drift);
    let mut d2_binding_drift = (*first).clone();
    d2_binding_drift.d2_model_invocation_binding_hash = sha256_bytes(b"replay-d2-binding-drift");
    mismatched_replays.push(d2_binding_drift);
    let mut bridge_drift = (*first).clone();
    bridge_drift.model_bridge_binding_hash = sha256_bytes(b"replay-bridge-drift");
    mismatched_replays.push(bridge_drift);
    for replay in mismatched_replays {
        assert_eq!(
            session
                .begin_advance(replay)
                .expect_err("same idempotency key cannot drift exact request lineage")
                .code(),
            MethodErrorCode::MethodStateConflict
        );
        assert_eq!(&*session, &replay_baseline);
    }
}

#[test]
fn method_state_machine_requires_exact_steps_and_new_review_per_invocation() {
    let (mut session, exact, first_decision) = ready_session();
    assert_eq!(session.state(), MethodState::Ready);
    assert_eq!(session.version(), 4);

    let first = advance_request(
        &session,
        "invoke_01J00000000000000000000000",
        "advance-1",
        first_decision.decision_id.clone(),
        4,
    );
    let receipt = session.begin_advance(first.clone()).expect("begin advance");
    assert_eq!(session.state(), MethodState::Advancing);
    assert_eq!(
        session
            .begin_advance(first.clone())
            .expect("idempotent retry"),
        receipt
    );
    assert_replay_lineage_drift_is_rejected(&mut session, &first);
    let mut stale_retry = first;
    stale_retry.expected_version = 5;
    assert_eq!(
        session
            .begin_advance(stale_retry)
            .expect_err("idempotent retries still bind the observed version")
            .code(),
        MethodErrorCode::MethodStateConflict
    );

    let invented_result = accepted_result(
        MethodAdvanceDisposition::ContextReviewRequired,
        "invented",
        Some("decide"),
    );
    let invented_step = session.accept_result(
        5,
        verified_result(&exact, &receipt, invented_result),
        UnixMillis(2_000),
    );
    assert_eq!(
        invented_step.expect_err("invented steps fail").code(),
        MethodErrorCode::MethodResultInvalid
    );

    let result = accepted_result(
        MethodAdvanceDisposition::ContextReviewRequired,
        "discover",
        Some("decide"),
    );
    session
        .accept_result(
            5,
            verified_result(&exact, &receipt, result),
            UnixMillis(2_000),
        )
        .expect("accepted result");
    assert_eq!(session.turn_ordinal(), 1);
    assert_eq!(session.state(), MethodState::ContextReviewRequired);
    assert_eq!(session.checkpoints().len(), 1);

    let replay_request = advance_request(
        &session,
        "invoke_01J11111111111111111111111",
        "advance-2",
        first_decision.decision_id,
        6,
    );
    let replay = session.begin_advance(replay_request);
    assert_eq!(
        replay
            .expect_err("a consumed decision never revives")
            .code(),
        MethodErrorCode::ContextDecisionAlreadyConsumed
    );
}

#[test]
fn restored_consumption_recomputes_the_exact_request_lineage_id() {
    let (session, _exact, receipt, _result) = advancing_session();
    let mut persisted: serde_json::Value = serde_json::from_str(
        &session
            .to_persisted_json()
            .expect("advancing session state"),
    )
    .expect("advancing session json");
    let drift = serde_json::to_value(sha256_bytes(b"persisted-consumption-drift"))
        .expect("drift digest json");
    persisted["activeInvocation"]["decisionConsumptionHash"] = drift.clone();
    persisted["consumedDecisions"][receipt.decision_id.as_str()]["receipt"]
        ["decisionConsumptionHash"] = drift.clone();
    persisted["idempotentAdvances"][receipt.idempotency_key.as_str()]["decisionConsumptionHash"] =
        drift;

    assert_eq!(
        MethodSession::from_persisted_json(
            &serde_json::to_string(&persisted).expect("tampered advancing state")
        )
        .expect_err("consumption id must be recomputed from all exact request lineage")
        .code(),
        MethodErrorCode::MethodStoreRecoveryRequired
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
    let first_request = advance_request(
        &session,
        first_invocation.as_str(),
        "pre-rebind",
        first_decision.decision_id,
        4,
    );
    let first_receipt = session
        .begin_advance(first_request)
        .expect("first invocation");
    let first_result = accepted_result(
        MethodAdvanceDisposition::AwaitingUser,
        "discover",
        Some("decide"),
    );
    session
        .accept_result(
            5,
            verified_result(&first_binding, &first_receipt, first_result),
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
    let second_request = advance_request(
        &session,
        second_invocation.as_str(),
        "post-rebind",
        second_decision.decision_id,
        9,
    );
    let second_receipt = session
        .begin_advance(second_request)
        .expect("second invocation");
    let second_result = accepted_result(MethodAdvanceDisposition::Completed, "only", None);
    session
        .accept_result(
            10,
            verified_result(&second_binding, &second_receipt, second_result),
            UnixMillis(3_000),
        )
        .expect("completed second capability");

    let action = BmadHelpActionKey {
        capability_catalog_hash: sha256_bytes(b"2:catalog"),
        package_version_id: id("pkgver_01J00000000000000000000000"),
        module_code: "bmm".to_owned(),
        skill_name: "bmad-architecture".to_owned(),
        action: Some("validate".to_owned()),
    };
    assert_eq!(
        BmadArtifactEvidence::from_completed_session(action.clone(), &session, &first_invocation)
            .expect_err("old invocation cannot evidence the rebound capability")
            .code(),
        BmadKernelErrorCode::HelpEvidenceInsufficient
    );
    let mut stale_catalog_action = action.clone();
    stale_catalog_action.capability_catalog_hash = sha256_bytes(b"stale-catalog");
    assert_eq!(
        BmadArtifactEvidence::from_completed_session(
            stale_catalog_action,
            &session,
            &second_invocation,
        )
        .expect_err("same capability from a stale catalog cannot become authoritative evidence")
        .code(),
        BmadKernelErrorCode::HelpEvidenceInsufficient
    );
    BmadArtifactEvidence::from_completed_session(action, &session, &second_invocation)
        .expect("exact catalog-bound invocation can become authoritative evidence");
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
    let first_request = advance_request(
        &session,
        "invoke_01J00000000000000000000000",
        "turn-one",
        first_decision.decision_id,
        4,
    );
    let first = session.begin_advance(first_request).expect("first advance");
    let first_result = accepted_result(
        MethodAdvanceDisposition::AwaitingUser,
        "discover",
        Some("decide"),
    );
    session
        .accept_result(
            5,
            verified_result(&exact, &first, first_result),
            UnixMillis(2_000),
        )
        .expect("await user");
    session.record_user_turn(6).expect("user turn");
    let second_decision = decision(&exact, "decision_01J11111111111111111111111");
    session
        .record_context_review(7, second_decision.clone())
        .expect("fresh review");
    let second_request = advance_request(
        &session,
        "invoke_01J11111111111111111111111",
        "turn-two",
        second_decision.decision_id,
        8,
    );
    let second = session
        .begin_advance(second_request)
        .expect("second advance");
    let second_result = accepted_result(MethodAdvanceDisposition::Completed, "decide", None);
    session
        .accept_result(
            9,
            verified_result(&exact, &second, second_result),
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

    let request = advance_request(
        &session,
        "invoke_01J00000000000000000000000",
        "refusal",
        review.decision_id,
        4,
    );
    let receipt = session.begin_advance(request).expect("advance");
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
    let request = advance_request(
        &session,
        "invoke_01J22222222222222222222222",
        "terminal-disposition",
        review.decision_id,
        4,
    );
    let receipt = session.begin_advance(request).expect("advance");
    let result = accepted_result(MethodAdvanceDisposition::AwaitingUser, "respond", None);
    let error = session
        .accept_result(
            5,
            verified_result(&exact, &receipt, result),
            UnixMillis(2_000),
        )
        .expect_err("terminal table edge requires completed");
    assert_eq!(error.code(), MethodErrorCode::MethodResultInvalid);
    assert_eq!(session.state(), MethodState::Advancing);
}

#[test]
fn rebind_preserves_history_and_requires_a_fresh_review() {
    let (mut session, first_binding, first_review) = ready_session();
    let first_request = advance_request(
        &session,
        "invoke_01J33333333333333333333333",
        "pre-rebind-turn",
        first_review.decision_id.clone(),
        4,
    );
    let first = session.begin_advance(first_request).expect("advance");
    let first_result = accepted_result(
        MethodAdvanceDisposition::ContextReviewRequired,
        "discover",
        Some("decide"),
    );
    session
        .accept_result(
            5,
            verified_result(&first_binding, &first, first_result),
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
    let old_review_request = advance_request(
        &restored,
        "invoke_01J44444444444444444444444",
        "old-review-replay",
        first_review.decision_id,
        9,
    );
    assert_eq!(
        restored
            .clone()
            .begin_advance(old_review_request)
            .expect_err("old review cannot authorize rebound inputs")
            .code(),
        MethodErrorCode::ContextDecisionAlreadyConsumed
    );
    let mut fresh = restored;
    let fresh_request = advance_request(
        &fresh,
        "invoke_01J55555555555555555555555",
        "fresh-review",
        fresh_review.decision_id,
        9,
    );
    fresh
        .begin_advance(fresh_request)
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
