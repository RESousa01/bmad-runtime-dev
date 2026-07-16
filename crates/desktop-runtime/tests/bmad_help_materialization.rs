#![allow(clippy::expect_used)]

use desktop_runtime::{
    canonical_hash, sha256_bytes, AuthorityRef, BmadArtifactClassification, BmadArtifactReference,
    BmadCanonicalAdvanceResult, BmadCatalogBuilder, BmadHelpBindingCompiler, BmadHelpCatalogSource,
    BmadHelpEvidenceClass, BmadHelpEvidenceToken, BmadHelpMaterializer, BmadHelpRecordIds,
    BmadKernelErrorCode, BmadLoadedMethodPackage, BmadLocationClass, BmadMethodHelpRecommendation,
    BmadPackageLoader, BmadSourceEntry, BmadSourceKind, BmadSourceSnapshot,
    BmadTrustedHelpModelProfile, BmadTrustedHelpModelProfileData, BmadVerifiedHelpProposal,
    ContractId, CreateMethodSession, MethodAdvanceRequest, MethodContextDecision, MethodErrorCode,
    MethodSession, MethodState, MethodVerifiedAdvanceResult, Sha256Digest, UnixMillis,
};
use serde_json::{json, Value};

const DESCRIPTOR_PATH: &str = "normalized/bmad-help.package.json";
const SEMANTIC_LEDGER_PATH: &str = "semantic-source-ledger.json";
const ADOPTION_LEDGER_PATH: &str = "adoption-ledger.json";
const HELP_INSTRUCTION_PATH: &str = "runtime/method/6.10.0/bmad-help.instructions.md";

fn id(value: &str) -> ContractId {
    ContractId::new(value).expect("test identifier")
}

fn foundation_path(relative: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/bmad-foundation")
        .join(relative)
}

fn source_entry(path: &str, location: BmadLocationClass) -> BmadSourceEntry {
    BmadSourceEntry::new(
        path,
        std::fs::read(foundation_path(path)).expect("foundation resource"),
        BmadSourceKind::SealedFoundation,
        location,
    )
    .expect("valid source entry")
}

fn loaded_method() -> BmadLoadedMethodPackage {
    let semantic = source_entry(SEMANTIC_LEDGER_PATH, BmadLocationClass::ManagedMetadata);
    let adoption = source_entry(ADOPTION_LEDGER_PATH, BmadLocationClass::ManagedMetadata);
    let semantic_hash = semantic.content_hash();
    let adoption_hash = adoption.content_hash();
    let snapshot = BmadSourceSnapshot::new(vec![
        semantic,
        adoption,
        source_entry(DESCRIPTOR_PATH, BmadLocationClass::ManagedMetadata),
        source_entry(
            "runtime/method/6.10.0/architect-persona.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(
            "runtime/method/6.10.0/architecture-create.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(HELP_INSTRUCTION_PATH, BmadLocationClass::ManagedProjection),
    ])
    .expect("sealed snapshot");
    BmadPackageLoader::load(&snapshot, semantic_hash, adoption_hash).expect("qualified Method")
}

fn compiled_help() -> desktop_runtime::BmadCompiledHelpInvocation {
    let loaded = loaded_method();
    let graph: Value = serde_json::from_slice(
        &std::fs::read(foundation_path("normalized/bmad-help-action-graph.json"))
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
        .expect("bound catalog");
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

fn artifact(index: u8) -> BmadArtifactReference {
    let hash = sha256_bytes(&[index; 8]);
    BmadArtifactReference::new(
        id(&format!(
            "artifact_01J0000000000000000000000{}",
            char::from(b'0' + index)
        )),
        format!("cas://sha256/{}", hash.hex_value()),
        hash,
        64,
        "application/json",
        BmadArtifactClassification::Internal,
    )
    .expect("artifact reference")
}

fn evidence_token(
    compiled: &desktop_runtime::BmadCompiledHelpInvocation,
    index: usize,
    evidence_class: BmadHelpEvidenceClass,
) -> BmadHelpEvidenceToken {
    let action = &compiled.catalog_candidates()[index];
    BmadHelpEvidenceToken::from_host_fact(
        id(&format!("evidence_01J0000000000000000000000{index}")),
        action.key.clone(),
        evidence_class,
        artifact(u8::try_from(index + 1).expect("small fixture index")),
    )
    .expect("host evidence token")
}

fn indexed_evidence_token(
    compiled: &desktop_runtime::BmadCompiledHelpInvocation,
    index: usize,
) -> BmadHelpEvidenceToken {
    let content_hash = sha256_bytes(&index.to_le_bytes());
    let artifact_ref = BmadArtifactReference::new(
        id(&format!("artifact_{index:016X}")),
        format!("cas://sha256/{}", content_hash.hex_value()),
        content_hash,
        64,
        "application/json",
        BmadArtifactClassification::Internal,
    )
    .expect("indexed artifact reference");
    BmadHelpEvidenceToken::from_host_fact(
        id(&format!("evidence_{index:016X}")),
        compiled.catalog_candidates()[0].key.clone(),
        BmadHelpEvidenceClass::Authoritative,
        artifact_ref,
    )
    .expect("indexed evidence token")
}

fn advancing_session(
    compiled: &desktop_runtime::BmadCompiledHelpInvocation,
) -> (MethodSession, desktop_runtime::MethodAdvanceReceipt) {
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
    .expect("session");
    session
        .bind_capability(
            1,
            compiled.exact_binding().clone(),
            compiled.step_table().clone(),
        )
        .expect("bind Help");
    session.request_context_review(2).expect("request review");
    let decision = MethodContextDecision {
        decision_id: id("decision_01J00000000000000000000000"),
        manifest_hash: sha256_bytes(b"manifest"),
        consent_hash: sha256_bytes(b"consent"),
        context_digest: sha256_bytes(b"context"),
        binding_hash: compiled
            .exact_binding()
            .binding_hash()
            .expect("binding hash"),
        reviewed_at: UnixMillis(2_000),
    };
    session
        .record_context_review(3, decision.clone())
        .expect("accept review");
    let invocation_id = id("invoke_01J00000000000000000000000");
    let d2_binding = sha256_bytes(b"D2 Help invocation binding");
    let request = MethodAdvanceRequest {
        invocation_id,
        idempotency_key: "help-materialization-0001".to_owned(),
        decision_id: decision.decision_id,
        decision_consumption_hash: sha256_bytes(b"decision consumption"),
        model_request_id: id("modelreq_01J00000000000000000000000"),
        model_request_hash: sha256_bytes(b"exact D2 request"),
        session_authority_hash: session.session_authority_hash().expect("session authority"),
        d2_model_invocation_binding_hash: d2_binding,
        model_bridge_binding_hash: session
            .model_bridge_binding_hash(&d2_binding)
            .expect("bridge binding"),
        expected_version: 4,
    };
    let receipt = session.begin_advance(request).expect("begin Help advance");
    (session, receipt)
}

fn proposal_bytes(
    compiled: &desktop_runtime::BmadCompiledHelpInvocation,
    token_ids: &[ContractId],
) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "proposalKind": "recommended_capability",
        "capabilityKey": capability_value(compiled, 0),
        "evidenceTokenIds": token_ids,
        "rationaleSummary": "Use the exact catalog capability supported by local evidence."
    }))
    .expect("proposal JSON")
}

fn capability_value(compiled: &desktop_runtime::BmadCompiledHelpInvocation, index: usize) -> Value {
    let key = &compiled.catalog_candidates()[index].key;
    json!({
        "packageVersionId": key.package_version_id,
        "moduleCode": key.module_code,
        "skillName": key.skill_name,
        "normalizedAction": key.action
    })
}

fn materialize(
    compiled: &desktop_runtime::BmadCompiledHelpInvocation,
    session: &MethodSession,
    receipt: desktop_runtime::MethodAdvanceReceipt,
    raw: Vec<u8>,
) -> Result<desktop_runtime::BmadCanonicalHelpRecords, desktop_runtime::BmadKernelError> {
    let verified = BmadVerifiedHelpProposal::from_trusted_host_evidence(
        raw,
        receipt,
        canonical_hash(
            "model-access-receipt-evidence",
            1,
            &json!({"verified": true, "providerReceipt": "fixture"}),
        )
        .expect("receipt evidence hash"),
    )?;
    BmadHelpMaterializer::materialize(
        compiled,
        session,
        &verified,
        BmadHelpRecordIds {
            recommendation_id: id("recommendation_01J00000000000000000000000"),
            result_id: id("result_01J00000000000000000000000"),
        },
        UnixMillis(1_784_024_000_000),
    )
}

fn rehash_verified_result_binding(checkpoint: &mut Value) {
    let mut binding = json!({
        "invocationId": checkpoint["invocationId"].clone(),
        "decisionId": checkpoint["contextDecisionId"].clone(),
        "decisionConsumptionHash": checkpoint["decisionConsumptionHash"].clone(),
        "modelRequestId": checkpoint["modelRequestId"].clone(),
        "modelRequestHash": checkpoint["modelRequestHash"].clone(),
        "sessionAuthorityHash": checkpoint["sessionAuthorityHash"].clone(),
        "d2ModelInvocationBindingHash": checkpoint["d2ModelInvocationBindingHash"].clone(),
        "modelBridgeBindingHash": checkpoint["modelBridgeBindingHash"].clone(),
        "methodBindingHash": checkpoint["methodBindingHash"].clone(),
        "modelBindingHash": checkpoint["modelBindingHash"].clone(),
        "responseSchemaHash": checkpoint["responseSchemaHash"].clone(),
        "modelResponsePayloadHash": checkpoint["modelResponsePayloadHash"].clone(),
        "acceptedMethodResultHash": checkpoint["acceptedMethodResultHash"].clone(),
        "modelReceiptEvidenceHash": checkpoint["modelReceiptEvidenceHash"].clone(),
    });
    if let Some(value) = checkpoint.get("canonicalAdvanceResult") {
        binding["canonicalAdvanceResult"] = value.clone();
    }
    if let Some(value) = checkpoint.get("canonicalAdvanceResultHash") {
        binding["canonicalAdvanceResultHash"] = value.clone();
    }
    checkpoint["verifiedResultBindingHash"] = serde_json::to_value(
        canonical_hash("bmad-method-verified-result-binding", 1, &binding)
            .expect("verified result binding hash"),
    )
    .expect("verified result binding digest JSON");
}

fn rehash_canonical_result(checkpoint: &mut Value) {
    let data = &checkpoint["canonicalAdvanceResult"];
    let result = json!({
        "resultKind": "completion_candidate",
        "resultId": data["resultId"].clone(),
        "requestId": checkpoint["modelRequestId"].clone(),
        "invocationId": checkpoint["invocationId"].clone(),
        "responseSchemaHash": data["recommendationSchemaHash"].clone(),
        "responseContentRef": data["recommendationContentRef"].clone(),
        "producedArtifacts": [],
        "unresolvedOpenItemCount": 0,
        "receivedAt": data["receivedAt"].clone(),
    });
    checkpoint["canonicalAdvanceResultHash"] = serde_json::to_value(
        canonical_hash("bmad-method-canonical-advance-result", 1, &result)
            .expect("canonical result hash"),
    )
    .expect("canonical result digest JSON");
}

fn rehash_runtime_checkpoint(checkpoint: &mut Value) {
    let mut hash_input = checkpoint.clone();
    hash_input
        .as_object_mut()
        .expect("checkpoint object")
        .remove("checkpointHash");
    checkpoint["checkpointHash"] = serde_json::to_value(
        canonical_hash("bmad-method-runtime-checkpoint", 1, &hash_input)
            .expect("runtime checkpoint hash"),
    )
    .expect("runtime checkpoint digest JSON");
}

#[test]
fn materializer_creates_distinct_canonical_records_without_mutating_the_aggregate() {
    let base = compiled_help();
    let token = evidence_token(&base, 0, BmadHelpEvidenceClass::Authoritative);
    let compiled = base
        .with_evidence_allowlist(vec![token.clone()])
        .expect("compiled evidence allowlist");
    let (session, receipt) = advancing_session(&compiled);
    let before = (
        session.state(),
        session.version(),
        session.checkpoints().len(),
    );
    let raw = proposal_bytes(&compiled, std::slice::from_ref(token.token_id()));
    let records = materialize(&compiled, &session, receipt, raw.clone())
        .expect("canonical Help materialization");

    assert_eq!(records.raw_proposal_bytes(), raw);
    assert_eq!(records.model_response_payload_hash(), sha256_bytes(&raw));
    assert_ne!(records.recommendation_bytes(), raw);
    assert_eq!(
        records.recommendation_content_ref().content_hash,
        sha256_bytes(records.recommendation_bytes())
    );
    assert_eq!(
        records.canonical_result().response_content_ref,
        *records.recommendation_content_ref()
    );
    let binding = records.verified_result().binding();
    let canonical_data = binding
        .canonical_advance_result
        .as_ref()
        .expect("canonical reconstruction data");
    let reconstructed = BmadCanonicalAdvanceResult {
        result_kind: "completion_candidate".to_owned(),
        result_id: canonical_data.result_id.clone(),
        request_id: binding.model_request_id.clone(),
        invocation_id: binding.invocation_id.clone(),
        response_schema_hash: canonical_data.recommendation_schema_hash,
        response_content_ref: canonical_data.recommendation_content_ref.clone(),
        produced_artifacts: Vec::new(),
        unresolved_open_item_count: 0,
        result_hash: binding
            .canonical_advance_result_hash
            .expect("canonical result hash"),
        received_at: canonical_data.received_at,
    };
    assert_eq!(reconstructed, *records.canonical_result());
    assert_eq!(
        canonical_data.result_schema_hash,
        compiled.result_schema_closure_hash()
    );
    assert!(matches!(
        records.recommendation(),
        BmadMethodHelpRecommendation::RecommendedCapability {
            evidence_class: BmadHelpEvidenceClass::Authoritative,
            guidance_required: true,
            ..
        }
    ));
    session
        .validate_result(session.version(), records.verified_result())
        .expect("aggregate accepts the exact sealed result read-only");
    assert_eq!(
        (
            session.state(),
            session.version(),
            session.checkpoints().len()
        ),
        before
    );
    assert_eq!(before.0, MethodState::Advancing);
}

#[test]
fn canonical_lineage_rejects_partial_pairs_and_live_help_downgrade() {
    let base = compiled_help();
    let token = evidence_token(&base, 0, BmadHelpEvidenceClass::Authoritative);
    let compiled = base
        .with_evidence_allowlist(vec![token.clone()])
        .expect("compiled evidence allowlist");
    let (session, receipt) = advancing_session(&compiled);
    let raw = proposal_bytes(&compiled, std::slice::from_ref(token.token_id()));
    let records = materialize(&compiled, &session, receipt, raw).expect("materialization");

    let mut downgraded = records.verified_result().binding().clone();
    downgraded.canonical_advance_result = None;
    downgraded.canonical_advance_result_hash = None;
    let downgraded = MethodVerifiedAdvanceResult::from_trusted_host_evidence(
        records.verified_result().result().clone(),
        downgraded,
    )
    .expect("legacy-compatible envelope construction");
    assert_eq!(
        session
            .validate_result(session.version(), &downgraded)
            .expect_err("Help cannot downgrade to legacy lineage")
            .code(),
        MethodErrorCode::MethodResultInvalid
    );

    let mut missing_hash = records.verified_result().binding().clone();
    missing_hash.canonical_advance_result_hash = None;
    assert_eq!(
        MethodVerifiedAdvanceResult::from_trusted_host_evidence(
            records.verified_result().result().clone(),
            missing_hash,
        )
        .expect_err("canonical data without its hash must fail")
        .code(),
        MethodErrorCode::MethodResultInvalid
    );
    let mut missing_data = records.verified_result().binding().clone();
    missing_data.canonical_advance_result = None;
    assert_eq!(
        MethodVerifiedAdvanceResult::from_trusted_host_evidence(
            records.verified_result().result().clone(),
            missing_data,
        )
        .expect_err("canonical hash without reconstruction data must fail")
        .code(),
        MethodErrorCode::MethodResultInvalid
    );
}

#[test]
fn canonical_lineage_rejects_synchronized_restart_drift() {
    let base = compiled_help();
    let token = evidence_token(&base, 0, BmadHelpEvidenceClass::Authoritative);
    let compiled = base
        .with_evidence_allowlist(vec![token.clone()])
        .expect("compiled evidence allowlist");
    let (mut session, receipt) = advancing_session(&compiled);
    let raw = proposal_bytes(&compiled, std::slice::from_ref(token.token_id()));
    let records = materialize(&compiled, &session, receipt, raw).expect("materialization");
    session
        .accept_result(
            session.version(),
            records.verified_result().clone(),
            UnixMillis(1_784_024_000_001),
        )
        .expect("accept exact Help result");
    let persisted = session.to_persisted_json().expect("persisted Help session");
    assert_eq!(
        MethodSession::from_persisted_json(&persisted).expect("exact restart"),
        session
    );

    let mut drifted: Value = serde_json::from_str(&persisted).expect("persisted JSON");
    drifted["checkpoints"][0]["canonicalAdvanceResult"]["resultSchemaHash"] =
        serde_json::to_value(sha256_bytes(b"synchronized result schema drift"))
            .expect("drift digest JSON");
    rehash_verified_result_binding(&mut drifted["checkpoints"][0]);
    rehash_runtime_checkpoint(&mut drifted["checkpoints"][0]);
    assert_eq!(
        MethodSession::from_persisted_json(
            &serde_json::to_string(&drifted).expect("drifted persisted JSON")
        )
        .expect_err("fully rehashed canonical schema drift must fail restart")
        .code(),
        MethodErrorCode::MethodStoreRecoveryRequired
    );

    let mut downgraded: Value = serde_json::from_str(&persisted).expect("persisted JSON");
    let checkpoint = downgraded["checkpoints"][0]
        .as_object_mut()
        .expect("checkpoint object");
    checkpoint.remove("canonicalAdvanceResult");
    checkpoint.remove("canonicalAdvanceResultHash");
    rehash_verified_result_binding(&mut downgraded["checkpoints"][0]);
    rehash_runtime_checkpoint(&mut downgraded["checkpoints"][0]);
    assert_eq!(
        MethodSession::from_persisted_json(
            &serde_json::to_string(&downgraded).expect("downgraded persisted JSON")
        )
        .expect_err("fully rehashed Help lineage downgrade must fail restart")
        .code(),
        MethodErrorCode::MethodStoreRecoveryRequired
    );

    for (field, value) in [
        (
            "recommendationSchemaHash",
            serde_json::to_value(sha256_bytes(b"synchronized recommendation schema drift"))
                .expect("schema digest JSON"),
        ),
        ("ref", json!("attacker-controlled-reference")),
        ("mediaType", json!("text/plain")),
    ] {
        let mut drifted: Value = serde_json::from_str(&persisted).expect("persisted JSON");
        if field == "recommendationSchemaHash" {
            drifted["checkpoints"][0]["canonicalAdvanceResult"][field] = value;
        } else {
            drifted["checkpoints"][0]["canonicalAdvanceResult"]["recommendationContentRef"]
                [field] = value;
        }
        rehash_canonical_result(&mut drifted["checkpoints"][0]);
        rehash_verified_result_binding(&mut drifted["checkpoints"][0]);
        rehash_runtime_checkpoint(&mut drifted["checkpoints"][0]);
        assert_eq!(
            MethodSession::from_persisted_json(
                &serde_json::to_string(&drifted).expect("drifted persisted JSON")
            )
            .expect_err("fully rehashed canonical lineage drift must fail restart")
            .code(),
            MethodErrorCode::MethodStoreRecoveryRequired
        );
    }
}

#[test]
fn evidence_allowlist_enforces_the_proposal_contracts_per_capability_limit() {
    let compiled = compiled_help();
    let boundary = (0..64)
        .map(|index| indexed_evidence_token(&compiled, index))
        .collect::<Vec<_>>();
    compiled
        .with_evidence_allowlist(boundary)
        .expect("64 exact tokens remain representable");

    let overflow = (0..65)
        .map(|index| indexed_evidence_token(&compiled, index))
        .collect::<Vec<_>>();
    assert_eq!(
        compiled
            .with_evidence_allowlist(overflow)
            .expect_err("65 tokens for one capability are unrepresentable")
            .code(),
        BmadKernelErrorCode::SealedHelpBindingMismatch
    );
}

#[test]
fn materializer_derives_the_weakest_evidence_class_and_catalog_guidance() {
    let base = compiled_help();
    let authoritative = evidence_token(&base, 0, BmadHelpEvidenceClass::Authoritative);
    let contextual = BmadHelpEvidenceToken::from_host_fact(
        id("evidence_01J00000000000000000000009"),
        base.catalog_candidates()[0].key.clone(),
        BmadHelpEvidenceClass::Contextual,
        artifact(9),
    )
    .expect("contextual token");
    let compiled = base
        .with_evidence_allowlist(vec![authoritative.clone(), contextual.clone()])
        .expect("compiled evidence allowlist");
    let (session, receipt) = advancing_session(&compiled);
    let raw = proposal_bytes(
        &compiled,
        &[
            authoritative.token_id().clone(),
            contextual.token_id().clone(),
        ],
    );
    let records = materialize(&compiled, &session, receipt, raw).expect("materialization");

    assert!(matches!(
        records.recommendation(),
        BmadMethodHelpRecommendation::RecommendedCapability {
            evidence_class: BmadHelpEvidenceClass::Contextual,
            guidance_required: true,
            evidence_refs,
            ..
        } if evidence_refs.len() == 2
    ));
}

#[test]
fn materializer_rejects_structural_text_catalog_and_token_substitution() {
    let base = compiled_help();
    let token = evidence_token(&base, 0, BmadHelpEvidenceClass::Heuristic);
    let compiled = base
        .with_evidence_allowlist(vec![token.clone()])
        .expect("compiled evidence allowlist");
    let valid = proposal_bytes(&compiled, std::slice::from_ref(token.token_id()));
    let mut wrong_capability: Value = serde_json::from_slice(&valid).expect("proposal value");
    wrong_capability["capabilityKey"]["skillName"] = json!("invented-skill");
    let mut unknown_token: Value = serde_json::from_slice(&valid).expect("proposal value");
    unknown_token["evidenceTokenIds"] = json!(["evidence_01J99999999999999999999999"]);
    let mut unknown_field: Value = serde_json::from_slice(&valid).expect("proposal value");
    unknown_field["transition"] = json!("completed");
    let cases = [
        serde_json::to_vec(&unknown_field).expect("unknown-field proposal"),
        br#"{"proposalKind":"recommended_capability","proposalKind":"no_recommendation","reasonCode":"catalog_evidence_absent"}"#.to_vec(),
        serde_json::to_vec(&json!({
            "proposalKind": "recommended_capability",
            "capabilityKey": capability_value(&compiled, 0),
            "evidenceTokenIds": [token.token_id(), token.token_id()],
            "rationaleSummary": "duplicate tokens"
        }))
        .expect("duplicate token proposal"),
        serde_json::to_vec(&json!({
            "proposalKind": "recommended_capability",
            "capabilityKey": capability_value(&compiled, 0),
            "evidenceTokenIds": [token.token_id()],
            "rationaleSummary": "unsafe \u{202e} text"
        }))
        .expect("unsafe proposal"),
        serde_json::to_vec(&wrong_capability).expect("wrong capability"),
        serde_json::to_vec(&unknown_token).expect("unknown token"),
    ];

    for raw in cases {
        let (session, receipt) = advancing_session(&compiled);
        let before = (
            session.state(),
            session.version(),
            session.checkpoints().len(),
        );
        assert_eq!(
            materialize(&compiled, &session, receipt, raw)
                .expect_err("proposal substitution must fail")
                .code(),
            BmadKernelErrorCode::HelpProposalInvalid
        );
        assert_eq!(
            (
                session.state(),
                session.version(),
                session.checkpoints().len()
            ),
            before
        );
    }
}

#[test]
fn no_recommendation_requires_a_provable_host_reason() {
    let empty = compiled_help();
    let (session, receipt) = advancing_session(&empty);
    let raw = serde_json::to_vec(&json!({
        "proposalKind": "no_recommendation",
        "reasonCode": "catalog_evidence_absent"
    }))
    .expect("no-recommendation proposal");
    let records = materialize(&empty, &session, receipt, raw).expect("proven absence");
    assert!(matches!(
        records.recommendation(),
        BmadMethodHelpRecommendation::NoRecommendation {
            evidence_class: BmadHelpEvidenceClass::Unknown,
            ..
        }
    ));

    let token = evidence_token(&empty, 0, BmadHelpEvidenceClass::Heuristic);
    let compiled = empty
        .with_evidence_allowlist(vec![token])
        .expect("non-empty evidence allowlist");
    let (session, receipt) = advancing_session(&compiled);
    let invented = serde_json::to_vec(&json!({
        "proposalKind": "no_recommendation",
        "reasonCode": "dependency_unavailable"
    }))
    .expect("invented absence");
    assert_eq!(
        materialize(&compiled, &session, receipt, invented)
            .expect_err("dependency absence is not proven")
            .code(),
        BmadKernelErrorCode::HelpProposalInvalid
    );
}

#[test]
fn verified_output_rejects_the_help_proposal_byte_limit() {
    let compiled = compiled_help();
    let (_session, receipt) = advancing_session(&compiled);
    assert_eq!(
        BmadVerifiedHelpProposal::from_trusted_host_evidence(
            vec![b'x'; 65_537],
            receipt,
            sha256_bytes(b"receipt"),
        )
        .expect_err("oversized proposal")
        .code(),
        BmadKernelErrorCode::HelpProposalInvalid
    );
}
