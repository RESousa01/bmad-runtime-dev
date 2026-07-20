#![allow(clippy::expect_used)]

use desktop_runtime::{
    canonical_hash, sha256_bytes, AuthorityRef, BmadArtifactClassification, BmadArtifactReference,
    BmadCanonicalHelpRecords, BmadCapabilityKey, BmadCatalogBuilder, BmadHelpBindingCompiler,
    BmadHelpCatalogSource, BmadHelpEvidenceClass, BmadHelpEvidenceToken, BmadHelpMaterializer,
    BmadHelpRecordIds, BmadLoadedMethodPackage, BmadLocationClass, BmadPackageLoader,
    BmadSourceEntry, BmadSourceKind, BmadSourceSnapshot, BmadTrustedHelpModelProfile,
    BmadTrustedHelpModelProfileData, BmadVerifiedHelpProposal, ContractId, CreateMethodSession,
    MethodAdvanceDisposition, MethodAdvanceReceipt, MethodAdvanceRequest, MethodAdvanceResult,
    MethodArtifactExpectation, MethodCheckpoint, MethodContextDecision, MethodEvidenceClass,
    MethodExactBinding, MethodExecutionProfile, MethodExecutionProfileData, MethodInvocationModes,
    MethodModelBinding, MethodModelBindingData, MethodPersistenceEvent, MethodResourcePolicy,
    MethodSession, MethodSessionRepository, MethodSessionService, MethodState, MethodStepTable,
    MethodVerifiedAdvanceResult, MethodVerifiedResultBindingData, Sha256Digest, UnixMillis,
};
use desktop_store::{
    BmadHelpRunCreateRequest, BmadHelpRunLatest, EvidenceAppend, KeyProtector, LocalStore,
    PayloadRef, StoreError,
};
use serde_json::{json, Value};
use std::sync::{Arc, Barrier};

const COMPLETED_RENDERER_PROJECTION: &[u8] = br#"{"schemaVersion":"sapphirus.bmad-help-completed-projection.v1","lifecycle":"completed","recommendation":{"kind":"recommended_capability","displayName":"Create Architecture"},"receipt":{"status":"succeeded","region":"westeurope","retention":"transient_no_store"}}"#;
const CREATED_RENDERER_PROJECTION: &[u8] = br#"{"schemaVersion":"bmad-help-run.v1","runKind":"bmad_help","lifecycle":"created_unbound","workspaceId":"workspace_01J77777777777777777777777","runId":"run_01J77777777777777777777777","sessionId":"session_01J77777777777777777777777","runnable":false,"completionClaimed":false}"#;

#[derive(Debug)]
struct TestProtector;

impl KeyProtector for TestProtector {
    fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, StoreError> {
        Ok(plaintext.to_vec())
    }

    fn unprotect(&self, protected: &[u8]) -> Result<Vec<u8>, StoreError> {
        Ok(protected.to_vec())
    }
}

fn id(value: &str) -> ContractId {
    ContractId::new(value).expect("valid id")
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
        invocation_id: invocation_id.clone(),
        idempotency_key: idempotency_key.to_owned(),
        decision_id,
        decision_consumption_hash: sha256_bytes(
            format!("{}:decision-consumption", invocation_id.as_str()).as_bytes(),
        ),
        model_request_id: id(&invocation_id.as_str().replacen("invoke_", "modelreq_", 1)),
        model_request_hash: sha256_bytes(
            format!("{}:model-request", invocation_id.as_str()).as_bytes(),
        ),
        session_authority_hash,
        d2_model_invocation_binding_hash,
        model_bridge_binding_hash,
        expected_version,
    }
}

fn completed_result(working_artifact_refs: Vec<String>) -> MethodAdvanceResult {
    MethodAdvanceResult {
        disposition: MethodAdvanceDisposition::Completed,
        current_step_key: "respond".to_owned(),
        next_step_key: None,
        working_artifact_refs,
    }
}

fn verified_result(
    binding: &MethodExactBinding,
    receipt: &MethodAdvanceReceipt,
    result: MethodAdvanceResult,
    evidence_label: &str,
) -> MethodVerifiedAdvanceResult {
    let verified_binding = MethodVerifiedResultBindingData {
        invocation_id: receipt.invocation_id.clone(),
        decision_id: receipt.decision_id.clone(),
        decision_consumption_hash: receipt.decision_consumption_hash,
        model_request_id: receipt.model_request_id.clone(),
        model_request_hash: receipt.model_request_hash,
        session_authority_hash: receipt.session_authority_hash,
        d2_model_invocation_binding_hash: receipt.d2_model_invocation_binding_hash,
        model_bridge_binding_hash: receipt.model_bridge_binding_hash,
        method_binding_hash: binding.binding_hash().expect("exact Method binding hash"),
        model_binding_hash: binding.model_binding_hash,
        response_schema_hash: binding.model_binding.data.response_schema_hash,
        model_response_payload_hash: sha256_bytes(
            format!("{evidence_label}:exact-raw-response-json").as_bytes(),
        ),
        accepted_method_result_hash: canonical_hash("bmad-method-advance-result", 1, &result)
            .expect("canonical accepted Method result hash"),
        model_receipt_evidence_hash: canonical_hash(
            "model-access-receipt-evidence",
            1,
            &(
                receipt.model_request_id.as_str(),
                receipt.model_request_hash,
                evidence_label,
            ),
        )
        .expect("canonical verified receipt evidence hash"),
        canonical_advance_result: None,
        canonical_advance_result_hash: None,
    };
    MethodVerifiedAdvanceResult::from_trusted_host_evidence(result, verified_binding)
        .expect("trusted host result evidence seals")
}

fn assert_active_invocation(
    session: &MethodSession,
    expected: &ContractId,
) -> Result<(), Box<dyn std::error::Error>> {
    let value: serde_json::Value = serde_json::from_str(&session.to_persisted_json()?)?;
    assert_eq!(
        value["activeInvocation"]["invocationId"].as_str(),
        Some(expected.as_str())
    );
    Ok(())
}

fn relation_count(
    connection: &rusqlite::Connection,
    statement: &str,
    session_id: &ContractId,
) -> Result<u64, rusqlite::Error> {
    connection.query_row(statement, [session_id.as_str()], |row| row.get(0))
}

fn assert_no_result_residue(
    database_path: &std::path::Path,
    session_id: &ContractId,
) -> Result<(), Box<dyn std::error::Error>> {
    let connection = rusqlite::Connection::open(database_path)?;
    assert_eq!(
        relation_count(
            &connection,
            "SELECT COUNT(*) FROM bmad_method_checkpoints WHERE session_id = ?1",
            session_id,
        )?,
        0
    );
    assert_eq!(
        relation_count(
            &connection,
            "SELECT COUNT(*) FROM evidence_events
             WHERE stream_id = 'bmad-method:' || ?1
               AND event_type = 'bmad.method.result_accepted'",
            session_id,
        )?,
        0
    );
    assert_eq!(
        relation_count(
            &connection,
            "SELECT COUNT(*) FROM outbox o
             JOIN evidence_events e ON e.event_id = o.event_id
             WHERE e.stream_id = 'bmad-method:' || ?1
               AND e.event_type = 'bmad.method.result_accepted'",
            session_id,
        )?,
        0
    );
    Ok(())
}

fn assert_no_help_finalization_residue(
    store: &LocalStore,
    session_id: &ContractId,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_no_result_residue(&store.database_path(), session_id)?;
    let connection = rusqlite::Connection::open(store.database_path())?;
    assert_eq!(
        connection.query_row(
            "SELECT COUNT(*) FROM payloads
             WHERE kind IN ('bmad_help_raw_proposal',
                            'bmad_help_canonical_recommendation',
                            'bmad_help_completed_renderer_projection')",
            [],
            |row| row.get::<_, u64>(0),
        )?,
        0
    );
    assert_eq!(
        relation_count(
            &connection,
            "SELECT COUNT(*) FROM evidence_events
             WHERE stream_id = 'bmad-method:' || ?1
               AND event_type IN ('bmad.help.proposal.retained',
                                  'bmad.help.recommendation.retained',
                                  'bmad.help.completed_projection.retained')",
            session_id,
        )?,
        0
    );
    assert_eq!(
        relation_count(
            &connection,
            "SELECT COUNT(*) FROM outbox o
             JOIN evidence_events e ON e.event_id = o.event_id
             WHERE e.stream_id = 'bmad-method:' || ?1
               AND e.event_type IN ('bmad.help.proposal.retained',
                                    'bmad.help.recommendation.retained',
                                    'bmad.help.completed_projection.retained',
                                    'bmad.method.result_accepted')",
            session_id,
        )?,
        0
    );
    Ok(())
}

fn append_decoy_reserved_help_event(
    store: &LocalStore,
    session_id: &ContractId,
    correlation_id: &ContractId,
    causation_id: &ContractId,
) -> Result<(), Box<dyn std::error::Error>> {
    let decoy = store.put_payload(
        "bmad_help_decoy",
        "sapphirus.bmad-help-decoy.v1",
        br#"{"decoy":true}"#,
    )?;
    let digest = decoy
        .content_hash
        .strip_prefix("sha256:")
        .expect("sha256 content hash");
    store.append_transition(
        "test_probe",
        "reserved_help_event_probe",
        1,
        r#"{"state":"injected"}"#,
        &EvidenceAppend {
            stream_id: format!("bmad-method:{}", session_id.as_str()),
            event_type: "bmad.help.proposal.retained".to_owned(),
            payload_hash: decoy.content_hash.clone(),
            payload_ref: Some(format!("cas://sha256/{digest}")),
            correlation_id: correlation_id.to_string(),
            causation_id: Some(causation_id.to_string()),
            redaction_level: "summary".to_owned(),
            retention_class: "authority".to_owned(),
        },
    )?;
    Ok(())
}

fn assert_replay_lineage_drift_conflicts(
    store: &LocalStore,
    session: &MethodSession,
    binding: &MethodExactBinding,
    request: &MethodAdvanceRequest,
) {
    let assert_conflict = |request| {
        assert!(matches!(
            store.begin_method_advance(&session.scope(), &session.session_id(), binding, request,),
            Err(StoreError::StateConflict)
        ));
    };

    let mut drifted = request.clone();
    drifted.decision_consumption_hash = sha256_bytes(b"drifted decision consumption");
    assert_conflict(drifted);
    let mut drifted = request.clone();
    drifted.model_request_id = id("modelreq_01J99999999999999999999999");
    assert_conflict(drifted);
    let mut drifted = request.clone();
    drifted.model_request_hash = sha256_bytes(b"drifted model request");
    assert_conflict(drifted);
    let mut drifted = request.clone();
    drifted.session_authority_hash = sha256_bytes(b"drifted session authority");
    assert_conflict(drifted);
    let mut drifted = request.clone();
    drifted.d2_model_invocation_binding_hash = sha256_bytes(b"drifted D2 binding");
    assert_conflict(drifted);
    let mut drifted = request.clone();
    drifted.model_bridge_binding_hash = sha256_bytes(b"drifted bridge binding");
    assert_conflict(drifted);
}

fn assert_checkpoint_lineage(
    checkpoint: &MethodCheckpoint,
    receipt: &MethodAdvanceReceipt,
    expected: &MethodVerifiedResultBindingData,
    verified_hash: &Sha256Digest,
) {
    assert_eq!(checkpoint.invocation_id, receipt.invocation_id);
    assert_eq!(
        checkpoint.advance_aggregate_version,
        receipt.aggregate_version
    );
    assert_eq!(checkpoint.context_decision_id, receipt.decision_id);
    assert_eq!(
        checkpoint.decision_consumption_hash,
        expected.decision_consumption_hash
    );
    assert_eq!(checkpoint.model_request_id, expected.model_request_id);
    assert_eq!(checkpoint.model_request_hash, expected.model_request_hash);
    assert_eq!(
        checkpoint.session_authority_hash,
        expected.session_authority_hash
    );
    assert_eq!(
        checkpoint.d2_model_invocation_binding_hash,
        expected.d2_model_invocation_binding_hash
    );
    assert_eq!(
        checkpoint.model_bridge_binding_hash,
        expected.model_bridge_binding_hash
    );
    assert_eq!(checkpoint.method_binding_hash, expected.method_binding_hash);
    assert_eq!(checkpoint.model_binding_hash, expected.model_binding_hash);
    assert_eq!(
        checkpoint.response_schema_hash,
        expected.response_schema_hash
    );
    assert_eq!(
        checkpoint.model_response_payload_hash,
        expected.model_response_payload_hash
    );
    assert_eq!(
        checkpoint.accepted_method_result_hash,
        expected.accepted_method_result_hash
    );
    assert_eq!(
        checkpoint.model_receipt_evidence_hash,
        expected.model_receipt_evidence_hash
    );
    assert_eq!(&checkpoint.verified_result_binding_hash, verified_hash);
    assert_ne!(
        checkpoint.model_response_payload_hash,
        checkpoint.accepted_method_result_hash
    );
    assert_ne!(
        checkpoint.accepted_method_result_hash,
        checkpoint.model_receipt_evidence_hash
    );
}

fn assert_result_relational_counts(
    database_path: &std::path::Path,
    session_id: &ContractId,
) -> Result<(), Box<dyn std::error::Error>> {
    let connection = rusqlite::Connection::open(database_path)?;
    assert_eq!(
        relation_count(
            &connection,
            "SELECT COUNT(*) FROM bmad_method_checkpoints WHERE session_id = ?1",
            session_id,
        )?,
        1
    );
    assert_eq!(
        relation_count(
            &connection,
            "SELECT COUNT(*) FROM evidence_events
             WHERE stream_id = 'bmad-method:' || ?1
               AND event_type = 'bmad.method.result_accepted'",
            session_id,
        )?,
        1
    );
    assert_eq!(
        relation_count(
            &connection,
            "SELECT COUNT(*) FROM outbox o
             JOIN evidence_events e ON e.event_id = o.event_id
             WHERE e.stream_id = 'bmad-method:' || ?1
               AND e.event_type = 'bmad.method.result_accepted'",
            session_id,
        )?,
        1
    );
    Ok(())
}

fn method_binding(
    artifact_expectations: Vec<MethodArtifactExpectation>,
) -> Result<MethodExactBinding, Box<dyn std::error::Error>> {
    let digest = |label: &str| sha256_bytes(label.as_bytes());
    let execution_profile = MethodExecutionProfile::from_source(
        MethodExecutionProfileData {
            entrypoint_kind: "direct".to_owned(),
            invocation_modes: MethodInvocationModes {
                interactive: true,
                headless: false,
                actions: vec!["create".to_owned()],
            },
            required_runtimes: Vec::new(),
            resource_policy: MethodResourcePolicy {
                entrypoint_timing: "invocation_start".to_owned(),
                resource_timing: "all_declared_at_start".to_owned(),
                declared_resource_paths: Vec::new(),
            },
            declared_tool_intents: Vec::new(),
            state_hints: Vec::new(),
            completion_evidence: Vec::new(),
            customization_profile: "none".to_owned(),
            validation_profile: "MethodStepWorkflowV6".to_owned(),
        },
        digest("execution"),
    )?;
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
    )?;
    Ok(MethodExactBinding {
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
        entrypoint_kind: "direct".to_owned(),
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
        artifact_expectations,
    })
}

fn ready_session(
    store: &LocalStore,
) -> Result<(MethodSession, MethodExactBinding, MethodContextDecision), Box<dyn std::error::Error>>
{
    ready_session_with_expectations(store, Vec::new())
}

fn ready_session_with_expectations(
    store: &LocalStore,
    artifact_expectations: Vec<MethodArtifactExpectation>,
) -> Result<(MethodSession, MethodExactBinding, MethodContextDecision), Box<dyn std::error::Error>>
{
    let mut session = MethodSession::create(CreateMethodSession {
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
    .expect("create");
    store.create_method_session(&session)?;
    let digest = |label: &str| sha256_bytes(label.as_bytes());
    let binding = method_binding(artifact_expectations)?;
    session
        .bind_capability(
            1,
            binding.clone(),
            MethodStepTable::new("respond", [("respond", None)]).expect("steps"),
        )
        .expect("bind");
    store.persist_method_transition(&session, 1, MethodPersistenceEvent::CapabilityBound)?;
    session.request_context_review(2).expect("review request");
    store.persist_method_transition(&session, 2, MethodPersistenceEvent::ContextReviewRequested)?;
    let review = MethodContextDecision {
        decision_id: id("decision_01J00000000000000000000000"),
        manifest_hash: digest("manifest"),
        consent_hash: digest("consent"),
        context_digest: digest("context"),
        binding_hash: binding.binding_hash().expect("hash"),
        reviewed_at: UnixMillis(2_000),
    };
    session
        .record_context_review(3, review.clone())
        .expect("review");
    store.persist_method_transition(&session, 3, MethodPersistenceEvent::ContextReviewAccepted)?;
    Ok((session, binding, review))
}

fn persist_completed_session(
    store: LocalStore,
    evidence_label: &str,
) -> Result<(MethodSessionService<LocalStore>, MethodSession), Box<dyn std::error::Error>> {
    let (session, binding, review) = ready_session(&store)?;
    let request = advance_request(
        &session,
        "invoke_01J55555555555555555555555",
        evidence_label,
        review.decision_id,
        4,
    );
    let receipt =
        store.begin_method_advance(&session.scope(), &session.session_id(), &binding, request)?;
    let service = MethodSessionService::new(store);
    let completed = service.accept_result(
        &session.scope(),
        &session.session_id(),
        5,
        verified_result(
            &binding,
            &receipt,
            completed_result(Vec::new()),
            evidence_label,
        ),
        UnixMillis(6_000),
    )?;
    Ok((service, completed))
}

const HELP_DESCRIPTOR_PATH: &str = "normalized/bmad-help.package.json";
const HELP_SEMANTIC_LEDGER_PATH: &str = "semantic-source-ledger.json";
const HELP_ADOPTION_LEDGER_PATH: &str = "adoption-ledger.json";
const HELP_INSTRUCTION_PATH: &str = "runtime/method/6.10.0/bmad-help.instructions.md";

fn help_foundation_path(relative: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/bmad-foundation")
        .join(relative)
}

fn help_source_entry(path: &str, location: BmadLocationClass) -> BmadSourceEntry {
    BmadSourceEntry::new(
        path,
        std::fs::read(help_foundation_path(path)).expect("foundation resource"),
        BmadSourceKind::SealedFoundation,
        location,
    )
    .expect("valid source entry")
}

fn loaded_help_method() -> BmadLoadedMethodPackage {
    let semantic = help_source_entry(
        HELP_SEMANTIC_LEDGER_PATH,
        BmadLocationClass::ManagedMetadata,
    );
    let adoption = help_source_entry(
        HELP_ADOPTION_LEDGER_PATH,
        BmadLocationClass::ManagedMetadata,
    );
    let semantic_hash = semantic.content_hash();
    let adoption_hash = adoption.content_hash();
    let snapshot = BmadSourceSnapshot::new(vec![
        semantic,
        adoption,
        help_source_entry(HELP_DESCRIPTOR_PATH, BmadLocationClass::ManagedMetadata),
        help_source_entry(
            "runtime/method/6.10.0/architect-persona.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        help_source_entry(
            "runtime/method/6.10.0/architecture-create.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        help_source_entry(HELP_INSTRUCTION_PATH, BmadLocationClass::ManagedProjection),
        help_source_entry(
            "runtime/method/6.10.0/analyst-persona.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        help_source_entry(
            "runtime/method/6.10.0/dev-persona.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        help_source_entry(
            "runtime/method/6.10.0/pm-persona.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        help_source_entry(
            "runtime/method/6.10.0/tech-writer-persona.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        help_source_entry(
            "runtime/method/6.10.0/ux-designer-persona.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
    ])
    .expect("sealed Help snapshot");
    BmadPackageLoader::load(&snapshot, semantic_hash, adoption_hash).expect("qualified Method")
}

fn compiled_help() -> desktop_runtime::BmadCompiledHelpInvocation {
    let loaded = loaded_help_method();
    let graph: Value = serde_json::from_slice(
        &std::fs::read(help_foundation_path(
            "normalized/bmad-help-action-graph.json",
        ))
        .expect("Help action graph"),
    )
    .expect("Help action graph JSON");
    let sources = graph["sources"]
        .as_array()
        .expect("catalog sources")
        .iter()
        .map(|source| {
            let rows: Vec<Vec<String>> =
                serde_json::from_value(source["rows"].clone()).expect("normalized rows");
            BmadHelpCatalogSource::from_rows(
                source["moduleCode"].as_str().expect("module code"),
                &rows,
            )
            .expect("catalog source")
        })
        .collect::<Vec<_>>();
    let graph_hash = Sha256Digest::parse(graph["graphHash"].as_str().expect("graph hash"))
        .expect("qualified graph hash");
    let catalog = BmadCatalogBuilder::build_bound(loaded.package(), &sources, graph_hash)
        .expect("bound Help catalog");
    let model = BmadTrustedHelpModelProfile::from_host_assertion(BmadTrustedHelpModelProfileData {
        provider_id: "azure-openai-managed".to_owned(),
        model_id: "gpt-5.2".to_owned(),
        deployment_id: "sapphirus-help".to_owned(),
        model_profile_hash: sha256_bytes(b"qualified model profile"),
        model_capability_hash: sha256_bytes(b"qualified model capability"),
        context_window_profile_hash: sha256_bytes(b"qualified context window"),
        egress_profile_hash: sha256_bytes(b"qualified egress profile"),
        request_schema_hash: sha256_bytes(b"qualified D2 request schema"),
    })
    .expect("trusted inert model profile");
    BmadHelpBindingCompiler::compile(loaded.help_invocation(), &catalog, &model)
        .expect("compiled Help")
}

fn help_evidence_token(
    compiled: &desktop_runtime::BmadCompiledHelpInvocation,
) -> BmadHelpEvidenceToken {
    let content_hash = sha256_bytes(b"Help store evidence artifact");
    let artifact = BmadArtifactReference::new(
        id("artifact_01J77777777777777777777777"),
        format!("cas://sha256/{}", content_hash.hex_value()),
        content_hash,
        64,
        "application/json",
        BmadArtifactClassification::Internal,
    )
    .expect("artifact reference");
    BmadHelpEvidenceToken::from_host_fact(
        id("evidence_01J77777777777777777777777"),
        compiled.catalog_candidates()[0].key.clone(),
        BmadHelpEvidenceClass::Authoritative,
        artifact,
    )
    .expect("host evidence token")
}

fn advancing_help_records(
    store: &LocalStore,
) -> Result<(MethodSession, BmadCanonicalHelpRecords), Box<dyn std::error::Error>> {
    let session = MethodSession::create(CreateMethodSession {
        session_id: id("session_01J77777777777777777777777"),
        owner_scope_ref: id("ownerscope_01J77777777777777777777777"),
        project_id: id("project_01J77777777777777777777777"),
        run_id: id("run_01J77777777777777777777777"),
        authority_ref: AuthorityRef {
            authority_kind: "desktop_local_store".to_owned(),
            authority_id: id("authority_01J77777777777777777777777"),
            installation_id: id("install_01J77777777777777777777777"),
            local_store_id: id("store_01J77777777777777777777777"),
            authority_epoch: 1,
        },
        created_at: UnixMillis(1_000),
    })?;
    store.create_method_session(&session)?;
    advancing_help_records_from_created(store, session)
}

fn advancing_help_records_from_created(
    store: &LocalStore,
    mut session: MethodSession,
) -> Result<(MethodSession, BmadCanonicalHelpRecords), Box<dyn std::error::Error>> {
    let base = compiled_help();
    let token = help_evidence_token(&base);
    let compiled = base.with_evidence_allowlist(vec![token.clone()])?;
    session.bind_capability(
        1,
        compiled.exact_binding().clone(),
        compiled.step_table().clone(),
    )?;
    store.persist_method_transition(&session, 1, MethodPersistenceEvent::CapabilityBound)?;
    session.request_context_review(2)?;
    store.persist_method_transition(&session, 2, MethodPersistenceEvent::ContextReviewRequested)?;
    let decision = MethodContextDecision {
        decision_id: id("decision_01J77777777777777777777777"),
        manifest_hash: sha256_bytes(b"Help manifest"),
        consent_hash: sha256_bytes(b"Help consent"),
        context_digest: sha256_bytes(b"Help context"),
        binding_hash: compiled.exact_binding().binding_hash()?,
        reviewed_at: UnixMillis(2_000),
    };
    session.record_context_review(3, decision.clone())?;
    store.persist_method_transition(&session, 3, MethodPersistenceEvent::ContextReviewAccepted)?;
    let request = advance_request(
        &session,
        "invoke_01J77777777777777777777777",
        "Help-store-finalization",
        decision.decision_id,
        4,
    );
    let receipt = store.begin_method_advance(
        &session.scope(),
        &session.session_id(),
        compiled.exact_binding(),
        request,
    )?;
    let advancing = store
        .load_method_session(&session.scope(), &session.session_id())?
        .expect("authoritative advancing Help session");
    let key = &compiled.catalog_candidates()[0].key;
    let raw = serde_json::to_vec(&json!({
        "proposalKind": "recommended_capability",
        "capabilityKey": {
            "packageVersionId": key.package_version_id,
            "moduleCode": key.module_code,
            "skillName": key.skill_name,
            "normalizedAction": key.action
        },
        "evidenceTokenIds": [token.token_id()],
        "rationaleSummary": "Use the exact catalog capability supported by local evidence."
    }))?;
    let verified = BmadVerifiedHelpProposal::from_trusted_host_evidence(
        raw,
        receipt,
        canonical_hash(
            "model-access-receipt-evidence",
            1,
            &json!({"verified": true, "providerReceipt": "store-fixture"}),
        )?,
    )?;
    let records = BmadHelpMaterializer::materialize(
        &compiled,
        &advancing,
        &verified,
        BmadHelpRecordIds {
            recommendation_id: id("recommendation_01J77777777777777777777777"),
            result_id: id("result_01J77777777777777777777777"),
        },
        UnixMillis(1_784_024_000_000),
    )?;
    Ok((advancing, records))
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one atomic-lineage fixture asserts every proposal, recommendation, completed projection, checkpoint, evidence, and outbox binding"
)]
fn help_finalization_atomically_retains_distinct_proposal_and_recommendation_lineage(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let (session, records) = advancing_help_records(&store)?;
    let raw = records.raw_proposal_bytes().to_vec();
    let recommendation = records.recommendation_bytes().to_vec();

    let completed = store.finalize_bmad_help(
        &session.scope(),
        &session.session_id(),
        session.version(),
        &records,
        COMPLETED_RENDERER_PROJECTION,
        UnixMillis(6_000),
    )?;

    assert_eq!(completed.state(), MethodState::Completed);
    assert_eq!(completed.version(), session.version() + 1);
    let checkpoint = completed.resume().expect("canonical Help checkpoint");
    assert_eq!(checkpoint.model_response_payload_hash, sha256_bytes(&raw));
    assert_eq!(
        checkpoint
            .canonical_advance_result
            .as_ref()
            .expect("canonical result lineage")
            .recommendation_content_ref,
        *records.recommendation_content_ref()
    );
    assert_ne!(raw, recommendation);

    let connection = rusqlite::Connection::open(store.database_path())?;
    let payload_rows = {
        let mut statement = connection.prepare(
            "SELECT content_hash, kind, schema_version, byte_count, key_version
             FROM payloads WHERE kind IN (?1, ?2, ?3) ORDER BY kind",
        )?;
        let rows = statement
            .query_map(
                [
                    "bmad_help_raw_proposal",
                    "bmad_help_canonical_recommendation",
                    "bmad_help_completed_renderer_projection",
                ],
                |row| {
                    Ok(PayloadRef {
                        content_hash: row.get(0)?,
                        kind: row.get(1)?,
                        schema_version: row.get(2)?,
                        byte_count: row.get(3)?,
                        key_version: row.get(4)?,
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    assert_eq!(payload_rows.len(), 3);
    let retained = payload_rows
        .iter()
        .map(|payload| Ok((payload.kind.clone(), store.get_payload(payload)?)))
        .collect::<Result<std::collections::BTreeMap<_, _>, StoreError>>()?;
    assert_eq!(retained["bmad_help_raw_proposal"], raw);
    assert_eq!(
        retained["bmad_help_canonical_recommendation"],
        recommendation
    );
    assert_eq!(
        retained["bmad_help_completed_renderer_projection"],
        COMPLETED_RENDERER_PROJECTION
    );
    assert_eq!(
        payload_rows
            .iter()
            .find(|payload| payload.kind == "bmad_help_raw_proposal")
            .expect("raw proposal payload")
            .schema_version,
        "sapphirus.bmad-method-help-proposal.v1"
    );
    assert_eq!(
        payload_rows
            .iter()
            .find(|payload| payload.kind == "bmad_help_canonical_recommendation")
            .expect("canonical recommendation payload")
            .schema_version,
        "sapphirus.bmad-method-help-recommendation.v1"
    );
    assert_eq!(
        payload_rows
            .iter()
            .find(|payload| payload.kind == "bmad_help_completed_renderer_projection")
            .expect("completed renderer projection payload")
            .schema_version,
        "sapphirus.bmad-help-completed-projection.v1"
    );
    assert_eq!(
        relation_count(
            &connection,
            "SELECT COUNT(*) FROM evidence_events
             WHERE stream_id = 'bmad-method:' || ?1
               AND event_type IN ('bmad.help.proposal.retained',
                                  'bmad.help.recommendation.retained',
                                  'bmad.help.completed_projection.retained',
                                  'bmad.method.result_accepted')",
            &session.session_id(),
        )?,
        4
    );
    drop(connection);
    drop(store);
    let reopened = LocalStore::open(directory.path(), &TestProtector)?;
    reopened.verify_integrity()?;
    let restored = reopened
        .load_method_session(&session.scope(), &session.session_id())?
        .expect("restored completed Help session");
    assert_eq!(restored, completed);

    Ok(())
}

#[test]
fn completed_help_projection_is_authenticated_and_recovers_after_restart(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    store.append_transition(
        "workspace_catalog",
        "local",
        1,
        r#"{"schemaVersion":"workspace-catalog.v1","entries":[{"workspaceId":"workspace_01J77777777777777777777777","grantEpoch":7,"revoked":false}]}"#,
        &EvidenceAppend {
            stream_id: "workspace:catalog".to_owned(),
            event_type: "workspace.granted".to_owned(),
            payload_hash: sha256_bytes(b"workspace catalog v1").to_string(),
            payload_ref: None,
            correlation_id: "request_workspace_catalog_seed".to_owned(),
            causation_id: None,
            redaction_level: "metadata".to_owned(),
            retention_class: "evidence".to_owned(),
        },
    )?;
    let identity = store.local_identity()?;
    let candidate = MethodSession::create(CreateMethodSession {
        session_id: id("session_01J77777777777777777777777"),
        owner_scope_ref: identity.owner_scope_ref().clone(),
        project_id: id("project_01J77777777777777777777777"),
        run_id: id("run_01J77777777777777777777777"),
        authority_ref: identity.authority_ref()?,
        created_at: UnixMillis(1_000),
    })?;
    let created = store.create_bmad_help_run(
        &candidate,
        &BmadHelpRunCreateRequest {
            request_id: id("request_01J77777777777777777777777"),
            project_id: id("project_01J77777777777777777777777"),
            workspace_id: id("workspace_01J77777777777777777777777"),
            workspace_grant_epoch: 7,
            workspace_catalog_version: 1,
            workspace_root_identity_hash: sha256_bytes(b"workspace root"),
            capability_catalog_hash: sha256_bytes(b"catalog"),
            foundation_binding_hash: sha256_bytes(b"foundation"),
            intent_hash: sha256_bytes(b"intent"),
            renderer_projection: CREATED_RENDERER_PROJECTION.to_vec(),
            accepted_at: UnixMillis(1_000),
        },
    )?;
    let (advancing, records) = advancing_help_records_from_created(&store, candidate)?;
    store.finalize_bmad_help(
        &advancing.scope(),
        &advancing.session_id(),
        advancing.version(),
        &records,
        COMPLETED_RENDERER_PROJECTION,
        UnixMillis(6_000),
    )?;
    drop(store);

    let reopened = LocalStore::open(directory.path(), &TestProtector)?;
    reopened.verify_integrity()?;
    let latest = reopened.latest_bmad_help_run(&id("workspace_01J77777777777777777777777"), 1)?;
    let BmadHelpRunLatest::Completed(completed) = latest else {
        return Err("expected durable completed Help projection".into());
    };
    assert_eq!(completed.creation.session_id, created.session_id);
    assert_eq!(completed.creation.scope, created.scope);
    assert_eq!(completed.renderer_projection, COMPLETED_RENDERER_PROJECTION);
    Ok(())
}

#[test]
fn generic_method_persistence_cannot_bypass_help_finalization(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let (advancing, records) = advancing_help_records(&store)?;
    let mut caller_copy = advancing.clone();
    caller_copy.accept_result(
        advancing.version(),
        records.verified_result().clone(),
        UnixMillis(6_000),
    )?;

    assert!(matches!(
        store.persist_method_transition(
            &caller_copy,
            advancing.version(),
            MethodPersistenceEvent::ResultAccepted,
        ),
        Err(StoreError::StateConflict)
    ));
    let retained = store
        .load_method_session(&advancing.scope(), &advancing.session_id())?
        .expect("original authoritative Help session");
    assert_eq!(retained, advancing);
    assert_no_result_residue(&store.database_path(), &advancing.session_id())?;

    Ok(())
}

#[test]
fn stale_help_finalization_leaves_no_authoritative_residue(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let (advancing, records) = advancing_help_records(&store)?;

    assert!(matches!(
        store.finalize_bmad_help(
            &advancing.scope(),
            &advancing.session_id(),
            advancing.version() + 1,
            &records,
            COMPLETED_RENDERER_PROJECTION,
            UnixMillis(6_000),
        ),
        Err(StoreError::StateConflict)
    ));
    let retained = store
        .load_method_session(&advancing.scope(), &advancing.session_id())?
        .expect("original authoritative Help session");
    assert_eq!(retained, advancing);
    assert_no_help_finalization_residue(&store, &advancing.session_id())?;
    store.verify_integrity()?;

    Ok(())
}

#[test]
fn conflicting_registered_help_payload_metadata_leaves_finalization_unadvanced(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let (advancing, records) = advancing_help_records(&store)?;
    let preexisting = store.put_payload(
        "bmad_help_raw_proposal",
        "sapphirus.bmad-method-help-proposal.v1",
        records.raw_proposal_bytes(),
    )?;
    let connection = rusqlite::Connection::open(store.database_path())?;
    connection.execute(
        "UPDATE payloads SET byte_count = byte_count + 1
         WHERE content_hash = ?1 AND kind = ?2 AND schema_version = ?3",
        rusqlite::params![
            preexisting.content_hash,
            preexisting.kind,
            preexisting.schema_version
        ],
    )?;
    drop(connection);

    assert!(matches!(
        store.finalize_bmad_help(
            &advancing.scope(),
            &advancing.session_id(),
            advancing.version(),
            &records,
            COMPLETED_RENDERER_PROJECTION,
            UnixMillis(6_000),
        ),
        Err(StoreError::Inconsistent)
    ));
    let retained = store
        .load_method_session(&advancing.scope(), &advancing.session_id())?
        .expect("original authoritative Help session");
    assert_eq!(retained, advancing);
    assert_no_result_residue(&store.database_path(), &advancing.session_id())?;
    let connection = rusqlite::Connection::open(store.database_path())?;
    assert_eq!(
        connection.query_row(
            "SELECT COUNT(*) FROM payloads
             WHERE kind IN ('bmad_help_raw_proposal',
                            'bmad_help_canonical_recommendation')",
            [],
            |row| row.get::<_, u64>(0),
        )?,
        1,
        "only the deliberately pre-existing conflicting row may remain registered"
    );
    assert_eq!(
        relation_count(
            &connection,
            "SELECT COUNT(*) FROM evidence_events
             WHERE stream_id = 'bmad-method:' || ?1
               AND event_type IN ('bmad.help.proposal.retained',
                                  'bmad.help.recommendation.retained')",
            &advancing.session_id(),
        )?,
        0
    );

    Ok(())
}

#[test]
fn relational_help_finalization_failure_rolls_back_all_authoritative_rows(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let (advancing, records) = advancing_help_records(&store)?;
    let connection = rusqlite::Connection::open(store.database_path())?;
    connection.execute_batch(
        "CREATE TRIGGER reject_help_recommendation_evidence
         BEFORE INSERT ON evidence_events
         WHEN NEW.event_type = 'bmad.help.recommendation.retained'
         BEGIN SELECT RAISE(ABORT, 'injected Help recommendation evidence failure'); END;",
    )?;
    drop(connection);

    assert!(matches!(
        store.finalize_bmad_help(
            &advancing.scope(),
            &advancing.session_id(),
            advancing.version(),
            &records,
            COMPLETED_RENDERER_PROJECTION,
            UnixMillis(6_000),
        ),
        Err(StoreError::Sqlite(_))
    ));
    let retained = store
        .load_method_session(&advancing.scope(), &advancing.session_id())?
        .expect("original authoritative Help session");
    assert_eq!(retained, advancing);
    assert_no_help_finalization_residue(&store, &advancing.session_id())?;
    let connection = rusqlite::Connection::open(store.database_path())?;
    connection.execute_batch("DROP TRIGGER reject_help_recommendation_evidence;")?;
    drop(connection);
    store.verify_integrity()?;

    Ok(())
}

#[test]
fn restart_integrity_rejects_missing_swapped_and_metadata_drifted_help_records(
) -> Result<(), Box<dyn std::error::Error>> {
    for tamper in ["missing", "missing_result", "swapped", "metadata"] {
        let directory = tempfile::tempdir()?;
        let store = LocalStore::open(directory.path(), &TestProtector)?;
        let (advancing, records) = advancing_help_records(&store)?;
        store.finalize_bmad_help(
            &advancing.scope(),
            &advancing.session_id(),
            advancing.version(),
            &records,
            COMPLETED_RENDERER_PROJECTION,
            UnixMillis(6_000),
        )?;
        let database_path = store.database_path();
        drop(store);
        let connection = rusqlite::Connection::open(&database_path)?;
        match tamper {
            "missing" => {
                connection.execute(
                    "DELETE FROM outbox WHERE event_id IN
                     (SELECT event_id FROM evidence_events
                      WHERE event_type = 'bmad.help.proposal.retained')",
                    [],
                )?;
                connection.execute(
                    "DELETE FROM evidence_events
                     WHERE event_type = 'bmad.help.proposal.retained'",
                    [],
                )?;
            }
            "missing_result" => {
                connection.execute(
                    "DELETE FROM outbox WHERE event_id IN
                     (SELECT event_id FROM evidence_events
                      WHERE event_type = 'bmad.method.result_accepted')",
                    [],
                )?;
                connection.execute(
                    "DELETE FROM evidence_events
                     WHERE event_type = 'bmad.method.result_accepted'",
                    [],
                )?;
            }
            "swapped" => {
                let (proposal_hash, proposal_ref, recommendation_hash, recommendation_ref) =
                    connection.query_row(
                        "SELECT
                           MAX(CASE WHEN event_type = 'bmad.help.proposal.retained'
                               THEN payload_hash END),
                           MAX(CASE WHEN event_type = 'bmad.help.proposal.retained'
                               THEN payload_ref END),
                           MAX(CASE WHEN event_type = 'bmad.help.recommendation.retained'
                               THEN payload_hash END),
                           MAX(CASE WHEN event_type = 'bmad.help.recommendation.retained'
                               THEN payload_ref END)
                         FROM evidence_events",
                        [],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, String>(2)?,
                                row.get::<_, String>(3)?,
                            ))
                        },
                    )?;
                connection.execute(
                    "UPDATE evidence_events SET payload_hash = ?1, payload_ref = ?2
                     WHERE event_type = 'bmad.help.proposal.retained'",
                    rusqlite::params![recommendation_hash, recommendation_ref],
                )?;
                connection.execute(
                    "UPDATE evidence_events SET payload_hash = ?1, payload_ref = ?2
                     WHERE event_type = 'bmad.help.recommendation.retained'",
                    rusqlite::params![proposal_hash, proposal_ref],
                )?;
            }
            "metadata" => {
                connection.execute(
                    "UPDATE payloads SET byte_count = byte_count + 1
                     WHERE kind = 'bmad_help_canonical_recommendation'",
                    [],
                )?;
            }
            _ => unreachable!(),
        }
        drop(connection);
        let reopened = LocalStore::open(directory.path(), &TestProtector)?;
        assert!(
            reopened.verify_integrity().is_err(),
            "{tamper} Help retention tamper must fail closed"
        );
    }

    Ok(())
}

#[test]
fn restart_integrity_rejects_reserved_help_events_with_wrong_payload_metadata(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let (advancing, records) = advancing_help_records(&store)?;
    let completed = store.finalize_bmad_help(
        &advancing.scope(),
        &advancing.session_id(),
        advancing.version(),
        &records,
        COMPLETED_RENDERER_PROJECTION,
        UnixMillis(6_000),
    )?;
    let checkpoint = completed.resume().expect("canonical Help checkpoint");
    append_decoy_reserved_help_event(
        &store,
        &completed.session_id(),
        &checkpoint.invocation_id,
        &checkpoint.checkpoint_id,
    )?;

    assert!(matches!(
        store.verify_integrity(),
        Err(StoreError::Inconsistent)
    ));

    Ok(())
}

#[test]
fn restart_integrity_rejects_reserved_help_events_without_a_finalization(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    append_decoy_reserved_help_event(
        &store,
        &id("session_01J88888888888888888888888"),
        &id("invoke_01J88888888888888888888888"),
        &id("checkpoint_01J88888888888888888888888"),
    )?;

    assert!(matches!(
        store.verify_integrity(),
        Err(StoreError::Inconsistent)
    ));

    Ok(())
}

#[test]
fn method_repository_atomically_consumes_one_decision_and_recovers_after_restart(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let (session, binding, review) = ready_session(&store)?;

    let request = advance_request(
        &session,
        "invoke_01J00000000000000000000000",
        "method-advance-1",
        review.decision_id.clone(),
        session.version(),
    );
    let receipt = store.begin_method_advance(
        &session.scope(),
        &session.session_id(),
        &binding,
        request.clone(),
    )?;
    assert_eq!(
        store.begin_method_advance(
            &session.scope(),
            &session.session_id(),
            &binding,
            request.clone(),
        )?,
        receipt
    );
    assert_replay_lineage_drift_conflicts(&store, &session, &binding, &request);
    assert_eq!(
        store.begin_method_advance(
            &session.scope(),
            &session.session_id(),
            &binding,
            request.clone(),
        )?,
        receipt
    );
    let mut changed_version = request.clone();
    changed_version.expected_version = 99;
    assert!(store
        .begin_method_advance(
            &session.scope(),
            &session.session_id(),
            &binding,
            changed_version,
        )
        .is_err());

    let mut wrong_authority = session.scope();
    wrong_authority.authority_ref.authority_id = id("authority_01J99999999999999999999999");
    assert!(store
        .begin_method_advance(&wrong_authority, &session.session_id(), &binding, request,)
        .is_err());

    let second = store.begin_method_advance(
        &session.scope(),
        &session.session_id(),
        &binding,
        advance_request(
            &session,
            "invoke_01J11111111111111111111111",
            "method-advance-2",
            review.decision_id,
            receipt.aggregate_version,
        ),
    );
    assert!(second.is_err());
    drop(store);

    let reopened = LocalStore::open(directory.path(), &TestProtector)?;
    let restored = reopened
        .load_method_session(&session.scope(), &session.session_id())?
        .expect("stored session");
    assert_eq!(restored.version(), receipt.aggregate_version);
    assert_eq!(restored.state(), desktop_runtime::MethodState::Advancing);
    Ok(())
}

#[test]
fn fresh_store_reaches_compiled_v10_schema() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    assert_eq!(store.schema_version()?, 11);
    let tables = store.schema_table_names()?;
    assert!(tables.contains(&"bmad_method_decision_consumptions".to_owned()));
    assert!(tables.contains(&"bmad_method_sessions".to_owned()));
    assert!(tables.contains(&"execution_checkpoints".to_owned()));
    assert!(tables.contains(&"effect_journals".to_owned()));
    assert!(tables.contains(&"execution_results".to_owned()));
    Ok(())
}

#[test]
fn fresh_and_v4_upgraded_stores_have_identical_v10_catalogs(
) -> Result<(), Box<dyn std::error::Error>> {
    let fresh_directory = tempfile::tempdir()?;
    let fresh = LocalStore::open(fresh_directory.path(), &TestProtector)?;
    let expected = fresh.schema_catalog()?;

    let upgraded_directory = tempfile::tempdir()?;
    let upgraded = LocalStore::open(upgraded_directory.path(), &TestProtector)?;
    let database_path = upgraded.database_path();
    drop(upgraded);
    let connection = rusqlite::Connection::open(&database_path)?;
    connection.execute_batch(
        "DROP TABLE bmad_capability_results;
         DROP TABLE bmad_capability_runs;
         DROP TABLE execution_results;
         DROP TABLE effect_journals;
         DROP TABLE execution_checkpoints;
         DROP TABLE bmad_help_run_creations;
         DROP TABLE bmad_builder_analysis_decisions;
         DROP TABLE bmad_builder_analyses;
         DROP TABLE bmad_builder_revisions;
         DROP TABLE bmad_builder_drafts;
         DROP TABLE bmad_method_artifacts;
         DROP TABLE bmad_method_decision_consumptions;
         DROP TABLE bmad_method_checkpoints;
         DROP TABLE bmad_method_sessions;
         PRAGMA user_version = 4;",
    )?;
    drop(connection);

    let reopened = LocalStore::open(upgraded_directory.path(), &TestProtector)?;
    assert_eq!(reopened.schema_version()?, 11);
    assert_eq!(reopened.schema_catalog()?, expected);
    Ok(())
}

#[test]
fn accepted_result_persists_checkpoint_state_evidence_and_outbox_atomically(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let (session, binding, review) = ready_session(&store)?;
    let request = advance_request(
        &session,
        "invoke_01J00000000000000000000000",
        "checkpoint",
        review.decision_id,
        4,
    );
    let receipt =
        store.begin_method_advance(&session.scope(), &session.session_id(), &binding, request)?;
    let result = verified_result(
        &binding,
        &receipt,
        completed_result(Vec::new()),
        "atomic-success",
    );
    let expected_binding = result.binding().clone();
    let expected_verified_hash = *result.verification_hash();
    let service = MethodSessionService::new(store);
    let completed = service.accept_result(
        &session.scope(),
        &session.session_id(),
        5,
        result,
        UnixMillis(3_000),
    )?;
    assert_eq!(completed.state(), desktop_runtime::MethodState::Completed);
    assert_checkpoint_lineage(
        completed.resume().expect("completed checkpoint"),
        &receipt,
        &expected_binding,
        &expected_verified_hash,
    );
    assert_result_relational_counts(&service.repository().database_path(), &session.session_id())?;
    service.repository().verify_integrity()?;
    drop(service);

    let reopened = LocalStore::open(directory.path(), &TestProtector)?;
    let restored = reopened
        .load_method_session(&session.scope(), &session.session_id())?
        .expect("completed session");
    assert_eq!(restored.state(), desktop_runtime::MethodState::Completed);
    assert_eq!(restored.resume().map(|value| value.turn_ordinal), Some(1));
    assert_checkpoint_lineage(
        restored.resume().expect("restored checkpoint"),
        &receipt,
        &expected_binding,
        &expected_verified_hash,
    );
    Ok(())
}

#[test]
fn concurrent_distinct_invocations_cannot_share_one_decision(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let primary = Arc::new(LocalStore::open(directory.path(), &TestProtector)?);
    let (session, binding, review) = ready_session(primary.as_ref())?;
    let secondary = Arc::new(LocalStore::open(directory.path(), &TestProtector)?);
    let stores = [Arc::clone(&primary), secondary];
    let barrier = Arc::new(Barrier::new(3));
    let mut handles = Vec::new();
    for (ordinal, store) in stores.into_iter().enumerate() {
        let barrier = Arc::clone(&barrier);
        let scope = session.scope();
        let session_id = session.session_id();
        let binding = binding.clone();
        let request = advance_request(
            &session,
            if ordinal == 0 {
                "invoke_01J00000000000000000000000"
            } else {
                "invoke_01J11111111111111111111111"
            },
            &format!("race-{ordinal}"),
            review.decision_id.clone(),
            4,
        );
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            store.begin_method_advance(&scope, &session_id, &binding, request)
        }));
    }
    barrier.wait();
    let results = handles
        .into_iter()
        .map(|handle| handle.join().expect("worker did not panic"))
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(results.iter().filter(|result| result.is_err()).count(), 1);
    primary.verify_integrity()?;
    let restored = primary
        .load_method_session(&session.scope(), &session.session_id())?
        .expect("stored session");
    assert_eq!(restored.version(), 5);
    Ok(())
}

#[test]
fn method_repository_isolates_owner_project_run_and_authority_scope(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let (session, _, _) = ready_session(&store)?;
    let mut wrong_owner = session.scope();
    wrong_owner.owner_scope_ref = id("ownerscope_01J99999999999999999999999");
    let mut wrong_project = session.scope();
    wrong_project.project_id = id("project_01J99999999999999999999999");
    let mut wrong_run = session.scope();
    wrong_run.run_id = id("run_01J99999999999999999999999");
    let mut wrong_authority = session.scope();
    wrong_authority.authority_ref.authority_id = id("authority_01J99999999999999999999999");
    for scope in [wrong_owner, wrong_project, wrong_run, wrong_authority] {
        assert!(store
            .load_method_session(&scope, &session.session_id())?
            .is_none());
    }
    assert!(store
        .load_method_session(&session.scope(), &session.session_id())?
        .is_some());
    Ok(())
}

#[test]
fn drift_is_checked_before_atomic_decision_consumption_and_can_be_rebound(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let (session, binding, review) = ready_session(&store)?;
    let mut rebound = binding.clone();
    rebound.package_source_hash = sha256_bytes(b"updated package source");
    let stale_request = advance_request(
        &session,
        "invoke_01J66666666666666666666666",
        "drifted-binding",
        review.decision_id,
        4,
    );
    assert!(store
        .begin_method_advance(
            &session.scope(),
            &session.session_id(),
            &rebound,
            stale_request,
        )
        .is_err());
    assert_eq!(
        store
            .load_method_session(&session.scope(), &session.session_id())?
            .expect("ready session")
            .state(),
        desktop_runtime::MethodState::Ready
    );

    let service = MethodSessionService::new(store);
    let rebound_session = service.rebind_invocation(
        &session.scope(),
        &session.session_id(),
        4,
        rebound.clone(),
        MethodStepTable::new("respond", [("respond", None)])?,
    )?;
    assert_eq!(rebound_session.version(), 5);
    let review_required =
        service.request_context_review(&session.scope(), &session.session_id(), 5)?;
    assert_eq!(review_required.version(), 6);
    let fresh_review = MethodContextDecision {
        decision_id: id("decision_01J66666666666666666666666"),
        manifest_hash: sha256_bytes(b"new manifest"),
        consent_hash: sha256_bytes(b"new consent"),
        context_digest: sha256_bytes(b"new context"),
        binding_hash: rebound.binding_hash()?,
        reviewed_at: UnixMillis(3_000),
    };
    let ready = service.record_context_review(
        &session.scope(),
        &session.session_id(),
        6,
        fresh_review.clone(),
    )?;
    assert_eq!(ready.version(), 7);
    let (_, receipt) = service.begin_advance(
        &session.scope(),
        &session.session_id(),
        &rebound,
        advance_request(
            &ready,
            "invoke_01J77777777777777777777777",
            "fresh-rebound-review",
            fresh_review.decision_id,
            7,
        ),
    )?;
    assert_eq!(receipt.aggregate_version, 8);
    Ok(())
}

#[test]
fn coordinator_authenticates_artifacts_and_enforces_required_expectations(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let expectation = MethodArtifactExpectation::from_source(MethodArtifactExpectation {
        expectation_kind: "method_artifact".to_owned(),
        expectation_id: id("expectation_01J00000000000000000000000"),
        artifact_kind: "architecture_document".to_owned(),
        required: true,
        storage_scope: "app_local".to_owned(),
        expected_media_type: "application/json".to_owned(),
        expected_content_schema_hash: Some(sha256_bytes(b"architecture schema")),
        source_output_hint: Some("architecture.md".to_owned()),
        completion_evidence_class: MethodEvidenceClass::Authoritative,
        expectation_hash: sha256_bytes(b"source expectation hash"),
    })?;
    let (session, binding, review) =
        ready_session_with_expectations(&store, vec![expectation.clone()])?;
    let service = MethodSessionService::new(store);
    let (advancing, receipt) = service.begin_advance(
        &session.scope(),
        &session.session_id(),
        &binding,
        advance_request(
            &session,
            "invoke_01J88888888888888888888888",
            "artifact-evidence",
            review.decision_id,
            4,
        ),
    )?;
    let invented_artifact_ref = format!("cas://sha256/{}", "a".repeat(64));
    let missing = service.accept_result(
        &session.scope(),
        &session.session_id(),
        5,
        verified_result(
            &binding,
            &receipt,
            completed_result(vec![invented_artifact_ref]),
            "missing-artifact",
        ),
        UnixMillis(4_000),
    );
    assert!(missing.is_err(), "invented CAS references must fail");
    let retained = service
        .repository()
        .load_method_session(&session.scope(), &session.session_id())?
        .expect("artifact rejection retains advancing state");
    assert_eq!(retained.version(), 5);
    assert_eq!(retained.state(), desktop_runtime::MethodState::Advancing);
    assert_active_invocation(&retained, &receipt.invocation_id)?;
    assert_no_result_residue(&service.repository().database_path(), &session.session_id())?;

    let provenance = advancing.artifact_provenance_for(&receipt.invocation_id)?;
    let artifact_ref = service.repository().put_method_artifact(
        &provenance,
        &expectation,
        br#"{"kind":"architecture"}"#,
    )?;
    let mut cross_session = provenance.clone();
    cross_session.session_id = id("session_01J99999999999999999999999");
    assert!(service
        .repository()
        .validate_method_artifact_refs(
            &cross_session,
            &binding,
            MethodAdvanceDisposition::Completed,
            std::slice::from_ref(&artifact_ref),
        )
        .is_err());
    let completed = service.accept_result(
        &session.scope(),
        &session.session_id(),
        5,
        verified_result(
            &binding,
            &receipt,
            completed_result(vec![artifact_ref]),
            "stored-artifact",
        ),
        UnixMillis(4_000),
    )?;
    assert_eq!(completed.state(), desktop_runtime::MethodState::Completed);
    Ok(())
}

#[test]
fn failed_result_transaction_leaves_state_and_evidence_unadvanced(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let (session, binding, review) = ready_session(&store)?;
    let receipt = store.begin_method_advance(
        &session.scope(),
        &session.session_id(),
        &binding,
        advance_request(
            &session,
            "invoke_01J99999999999999999999999",
            "injected-failure",
            review.decision_id,
            4,
        ),
    )?;
    drop(store);
    let connection = rusqlite::Connection::open(directory.path().join("authority.sqlite3"))?;
    connection.execute_batch(
        "CREATE TRIGGER reject_method_result_evidence
         BEFORE INSERT ON evidence_events
         WHEN NEW.event_type = 'bmad.method.result_accepted'
         BEGIN SELECT RAISE(ABORT, 'injected result evidence failure'); END;",
    )?;
    drop(connection);

    let reopened = LocalStore::open(directory.path(), &TestProtector)?;
    let service = MethodSessionService::new(reopened);
    assert!(service
        .accept_result(
            &session.scope(),
            &session.session_id(),
            5,
            verified_result(
                &binding,
                &receipt,
                completed_result(Vec::new()),
                "injected-failure",
            ),
            UnixMillis(5_000),
        )
        .is_err());
    let retained = service
        .repository()
        .load_method_session(&session.scope(), &session.session_id())?
        .expect("retained advancing state");
    assert_eq!(retained.version(), 5);
    assert_eq!(retained.state(), desktop_runtime::MethodState::Advancing);
    assert!(retained.checkpoints().is_empty());
    assert_active_invocation(&retained, &receipt.invocation_id)?;
    assert_no_result_residue(&service.repository().database_path(), &session.session_id())?;
    Ok(())
}

#[test]
fn checkpoint_index_hash_tampering_is_detected_by_explicit_integrity_verification(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let (service, completed) = persist_completed_session(store, "checkpoint-index-tamper")?;
    let database_path = service.repository().database_path();
    drop(service);

    let connection = rusqlite::Connection::open(&database_path)?;
    connection.execute(
        "UPDATE bmad_method_checkpoints SET checkpoint_hash = ?1 WHERE session_id = ?2",
        rusqlite::params![
            sha256_bytes(b"tampered checkpoint index hash").to_string(),
            completed.session_id().as_str(),
        ],
    )?;
    drop(connection);

    let reopened = LocalStore::open(directory.path(), &TestProtector)?;
    assert!(matches!(
        reopened.verify_integrity(),
        Err(StoreError::Inconsistent)
    ));
    Ok(())
}

#[test]
fn session_projection_repointed_to_registered_stale_payload_fails_closed(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let (service, completed) = persist_completed_session(store, "stale-projection")?;
    let database_path = service.repository().database_path();
    drop(service);

    let connection = rusqlite::Connection::open(&database_path)?;
    let stale: (String, u64, u32) = connection.query_row(
        "SELECT e.payload_hash, p.byte_count, p.key_version
         FROM evidence_events e
         JOIN payloads p
           ON p.content_hash = e.payload_hash
          AND p.kind = 'bmad_method_session'
          AND p.schema_version = 'sapphirus.bmad-method-session-state.v1'
         WHERE e.stream_id = 'bmad-method:' || ?1
           AND e.event_type = 'bmad.method.context_review_accepted'",
        [completed.session_id().as_str()],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    connection.execute(
        "UPDATE bmad_method_sessions
         SET state_content_hash = ?1, state_byte_count = ?2, state_key_version = ?3
         WHERE session_id = ?4",
        rusqlite::params![stale.0, stale.1, stale.2, completed.session_id().as_str(),],
    )?;
    drop(connection);

    let reopened = LocalStore::open(directory.path(), &TestProtector)?;
    assert!(matches!(
        reopened.load_method_session(&completed.scope(), &completed.session_id()),
        Err(StoreError::Inconsistent)
    ));
    assert!(reopened.verify_integrity().is_err());
    Ok(())
}

#[test]
fn frozen_pre_lineage_created_v1_session_restores_without_a_schema_migration(
) -> Result<(), Box<dyn std::error::Error>> {
    const PRE_BMAD_06_CREATED_V1: &str = r#"{
        "schemaVersion":"sapphirus.bmad-method-session-state.v1",
        "sessionId":"session_01J00000000000000000000000",
        "scope":{
            "ownerScopeRef":"ownerscope_01J00000000000000000000000",
            "projectId":"project_01J00000000000000000000000",
            "runId":"run_01J00000000000000000000000",
            "authorityRef":{
                "authorityKind":"desktop_local_store",
                "authorityId":"authority_01J00000000000000000000000",
                "installationId":"install_01J00000000000000000000000",
                "localStoreId":"store_01J00000000000000000000000",
                "authorityEpoch":1
            }
        },
        "createdAt":1000,
        "state":"created",
        "version":1,
        "turnOrdinal":0,
        "bindingOrdinal":0,
        "bindingHistory":[],
        "exactBinding":null,
        "stepTable":null,
        "currentStepKey":null,
        "pendingReview":null,
        "activeInvocation":null,
        "consumedDecisions":{},
        "idempotentAdvances":{},
        "checkpoints":[]
    }"#;

    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let frozen = MethodSession::from_persisted_json(PRE_BMAD_06_CREATED_V1)?;
    store.create_method_session(&frozen)?;
    drop(store);

    let reopened = LocalStore::open(directory.path(), &TestProtector)?;
    assert_eq!(reopened.schema_version()?, 11);
    let restored = reopened
        .load_method_session(&frozen.scope(), &frozen.session_id())?
        .expect("frozen Created/unbound session");
    assert_eq!(restored.state(), desktop_runtime::MethodState::Created);
    assert_eq!(restored.version(), 1);
    assert!(restored.checkpoints().is_empty());
    Ok(())
}

#[test]
fn migration_interruptions_roll_back_and_reopen_to_complete_v10(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let expected = store.schema_catalog()?;
    let database_path = store.database_path();
    drop(store);
    let connection = rusqlite::Connection::open(&database_path)?;
    connection.execute_batch(
        "PRAGMA foreign_keys = OFF;
         DROP TABLE bmad_capability_results;
         DROP TABLE bmad_capability_runs;
         DROP TABLE execution_results;
         DROP TABLE effect_journals;
         DROP TABLE execution_checkpoints;
         DROP TABLE bmad_help_run_creations;
         DROP TABLE bmad_builder_analysis_decisions;
         DROP TABLE bmad_builder_analyses;
         DROP TABLE bmad_builder_revisions;
         DROP TABLE bmad_builder_drafts;
         DROP TABLE bmad_method_artifacts;
         DROP TABLE bmad_method_decision_consumptions;
         DROP TABLE bmad_method_checkpoints;
         DROP TABLE bmad_method_sessions;
         DROP TABLE outbox;
         DROP TABLE evidence_events;
         DROP TABLE aggregates;
         DROP TABLE spec_consumptions;
         DROP TABLE payloads;
         DROP TABLE store_meta;
         PRAGMA user_version = 0;",
    )?;
    connection.execute_batch(
        "BEGIN IMMEDIATE;
         CREATE TABLE store_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL) STRICT;",
    )?;
    drop(connection);
    let after_base_interrupt = LocalStore::open(directory.path(), &TestProtector)?;
    assert_eq!(after_base_interrupt.schema_catalog()?, expected);
    drop(after_base_interrupt);

    let connection = rusqlite::Connection::open(&database_path)?;
    connection.execute_batch(
        "PRAGMA foreign_keys = OFF;
         DROP TABLE bmad_capability_results;
         DROP TABLE bmad_capability_runs;
         DROP TABLE execution_results;
         DROP TABLE effect_journals;
         DROP TABLE execution_checkpoints;
         DROP TABLE bmad_help_run_creations;
         DROP TABLE bmad_builder_analysis_decisions;
         DROP TABLE bmad_builder_analyses;
         DROP TABLE bmad_builder_revisions;
         DROP TABLE bmad_builder_drafts;
         DROP TABLE bmad_method_artifacts;
         DROP TABLE bmad_method_decision_consumptions;
         DROP TABLE bmad_method_checkpoints;
         DROP TABLE bmad_method_sessions;
         PRAGMA user_version = 4;",
    )?;
    connection.execute_batch(
        "BEGIN IMMEDIATE;
         CREATE TABLE bmad_method_artifacts (content_hash TEXT PRIMARY KEY) STRICT;",
    )?;
    drop(connection);
    let after_v5_interrupt = LocalStore::open(directory.path(), &TestProtector)?;
    assert_eq!(after_v5_interrupt.schema_catalog()?, expected);
    Ok(())
}

#[test]
fn migration_failure_can_open_retained_method_history_read_only(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let (session, _, _) = ready_session(&store)?;
    let database_path = store.database_path();
    drop(store);
    let connection = rusqlite::Connection::open(database_path)?;
    connection.execute_batch("DROP TABLE bmad_method_checkpoints;")?;
    drop(connection);
    assert!(LocalStore::open(directory.path(), &TestProtector).is_err());

    let recovery = LocalStore::open_read_only_recovery(directory.path(), &TestProtector)?;
    assert_eq!(recovery.schema_version()?, 11);
    assert!(!recovery
        .schema_table_names()?
        .contains(&"bmad_method_checkpoints".to_owned()));
    let retained = recovery
        .load_method_session(&session.scope(), &session.session_id())?
        .expect("retained session remains readable");
    assert_eq!(retained.state(), desktop_runtime::MethodState::Ready);
    Ok(())
}
