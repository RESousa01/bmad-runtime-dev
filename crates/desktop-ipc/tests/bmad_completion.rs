#![allow(clippy::expect_used)]

use std::collections::BTreeSet;

use desktop_ipc::{
    decode_retained_bmad_help_completion, project_completed_bmad_help_run, BmadHelpProjectionError,
    BmadHelpReceiptStatusProjection, BmadHelpReceiptSummaryInput, BmadHelpRetentionProjection,
    MAX_BMAD_HELP_COMPLETED_PROJECTION_BYTES,
};
use desktop_runtime::{
    canonical_hash_without_field, sha256_bytes, BmadArtifactClassification, BmadArtifactReference,
    BmadCapabilityKey, BmadHelpEvidenceClass, BmadHelpNoRecommendationReason,
    BmadMethodHelpRecommendation, ContractId, UnixMillis,
};
use serde_json::{json, Value};

const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

fn id(value: &str) -> ContractId {
    ContractId::new(value).expect("test identifier")
}

fn seal_recommendation(
    recommendation: BmadMethodHelpRecommendation,
) -> BmadMethodHelpRecommendation {
    let value = serde_json::to_value(&recommendation).expect("recommendation JSON");
    let hash = canonical_hash_without_field(
        "bmad-method-help-recommendation",
        1,
        &value,
        "recommendationHash",
    )
    .expect("recommendation hash");
    match recommendation {
        BmadMethodHelpRecommendation::RecommendedCapability {
            recommendation_id,
            session_id,
            capability_key,
            evidence_class,
            evidence_refs,
            guidance_required,
            rationale_summary,
            created_at,
            ..
        } => BmadMethodHelpRecommendation::RecommendedCapability {
            recommendation_id,
            session_id,
            capability_key,
            evidence_class,
            evidence_refs,
            guidance_required,
            rationale_summary,
            recommendation_hash: hash,
            created_at,
        },
        BmadMethodHelpRecommendation::NoRecommendation {
            recommendation_id,
            session_id,
            evidence_class,
            reason_code,
            created_at,
            ..
        } => BmadMethodHelpRecommendation::NoRecommendation {
            recommendation_id,
            session_id,
            evidence_class,
            reason_code,
            recommendation_hash: hash,
            created_at,
        },
    }
}

fn recommended() -> BmadMethodHelpRecommendation {
    seal_recommendation(BmadMethodHelpRecommendation::RecommendedCapability {
        recommendation_id: id("recommendation_01J77777777777777777777777"),
        session_id: id("session_1"),
        capability_key: BmadCapabilityKey {
            package_version_id: id("pkgver_01J77777777777777777777777"),
            module_code: "bmm".to_owned(),
            skill_name: "bmad-architecture".to_owned(),
            normalized_action: Some("create".to_owned()),
        },
        evidence_class: BmadHelpEvidenceClass::UserAsserted,
        evidence_refs: vec![BmadArtifactReference::new(
            id("artifact_01J77777777777777777777777"),
            format!(
                "cas://sha256/{}",
                sha256_bytes(b"LEAK_CANARY_EVIDENCE_BYTES").hex_value()
            ),
            sha256_bytes(b"LEAK_CANARY_EVIDENCE_BYTES"),
            26,
            "application/LEAK_CANARY+json",
            BmadArtifactClassification::Internal,
        )
        .expect("evidence reference")],
        guidance_required: true,
        rationale_summary: "The stated architecture goal matches this planning capability."
            .to_owned(),
        recommendation_hash: sha256_bytes(b"placeholder"),
        created_at: UnixMillis(3_000),
    })
}

fn no_recommendation() -> BmadMethodHelpRecommendation {
    seal_recommendation(BmadMethodHelpRecommendation::NoRecommendation {
        recommendation_id: id("recommendation_01J88888888888888888888888"),
        session_id: id("session_1"),
        evidence_class: BmadHelpEvidenceClass::Unknown,
        reason_code: BmadHelpNoRecommendationReason::CatalogEvidenceAbsent,
        recommendation_hash: sha256_bytes(b"placeholder"),
        created_at: UnixMillis(3_000),
    })
}

fn receipt() -> BmadHelpReceiptSummaryInput {
    BmadHelpReceiptSummaryInput {
        receipt_id: id("receipt_1"),
        status: BmadHelpReceiptStatusProjection::Succeeded,
        retention_mode: BmadHelpRetentionProjection::TransientNoStore,
        region: "westeurope".to_owned(),
        input_bytes: 4_096,
        output_bytes: 512,
        started_at: UnixMillis(1_000),
        completed_at: UnixMillis(2_000),
    }
}

fn completed_projection(
) -> Result<desktop_ipc::BmadHelpRunCompletedProjection, BmadHelpProjectionError> {
    project_completed_bmad_help_run(
        &recommended(),
        Some("Create Architecture"),
        id("workspace_1"),
        id("run_1"),
        id("session_1"),
        &receipt(),
    )
}

#[test]
fn completed_recommended_projection_has_exact_closed_display_shape() {
    let projection = completed_projection().expect("safe completed projection");
    let value = serde_json::to_value(&projection).expect("projection JSON");

    assert_eq!(
        value
            .as_object()
            .expect("projection object")
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "completionClaimed",
            "lifecycle",
            "receipt",
            "recommendation",
            "runId",
            "runKind",
            "runnable",
            "schemaVersion",
            "sessionId",
            "workspaceId",
        ])
    );
    assert_eq!(value["schemaVersion"], "bmad-help-completed.v1");
    assert_eq!(value["runKind"], "bmad_help");
    assert_eq!(value["lifecycle"], "completed");
    assert_eq!(value["workspaceId"], "workspace_1");
    assert_eq!(value["runId"], "run_1");
    assert_eq!(value["sessionId"], "session_1");
    assert_eq!(value["runnable"], false);
    assert_eq!(value["completionClaimed"], true);
    assert_eq!(
        value["recommendation"]
            .as_object()
            .expect("recommendation object")
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "action",
            "createdAt",
            "displayName",
            "evidenceClass",
            "guidanceRequired",
            "moduleCode",
            "rationaleSummary",
            "recommendationKind",
            "skillName",
        ])
    );
    assert_eq!(
        value["recommendation"]["recommendationKind"],
        "recommended_capability"
    );
    assert_eq!(
        value["recommendation"]["displayName"],
        "Create Architecture"
    );
    assert_eq!(value["recommendation"]["moduleCode"], "bmm");
    assert_eq!(value["recommendation"]["skillName"], "bmad-architecture");
    assert_eq!(value["recommendation"]["action"], "create");
    assert_eq!(value["recommendation"]["evidenceClass"], "user_asserted");
    assert_eq!(value["recommendation"]["guidanceRequired"], true);
    assert_eq!(value["recommendation"]["createdAt"], 3_000);

    assert_eq!(
        value["receipt"]
            .as_object()
            .expect("receipt object")
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "completedAt",
            "inputBytes",
            "outputBytes",
            "receiptId",
            "region",
            "retentionMode",
            "schemaVersion",
            "startedAt",
            "status",
        ])
    );
    assert_eq!(
        value["receipt"]["schemaVersion"],
        "bmad-model-receipt-summary.v1"
    );
    assert_eq!(value["receipt"]["receiptId"], "receipt_1");
    assert_eq!(value["receipt"]["status"], "succeeded");
    assert_eq!(value["receipt"]["retentionMode"], "transient_no_store");
    assert_eq!(value["receipt"]["region"], "westeurope");
}

#[test]
fn completed_projection_is_bounded_and_disclosure_safe() {
    let projection = completed_projection().expect("safe completed projection");
    let bytes = serde_json::to_vec(&projection).expect("projection bytes");
    assert!(bytes.len() <= MAX_BMAD_HELP_COMPLETED_PROJECTION_BYTES);
    let text = String::from_utf8(bytes).expect("projection UTF-8");
    for forbidden in [
        "LEAK_CANARY",
        "recommendationId",
        "packageVersionId",
        "recommendationHash",
        "evidenceRefs",
        "token",
        "artifact",
        "rawProposal",
        "payloadJson",
        "proof",
        "manifestHash",
        "consent",
        "bindingHash",
        "authority",
        "C:\\\\",
        "/Users/",
    ] {
        assert!(!text.contains(forbidden), "leaked {forbidden}");
    }
}

#[test]
fn completed_no_recommendation_projection_uses_only_the_closed_reason() {
    let projection = project_completed_bmad_help_run(
        &no_recommendation(),
        None,
        id("workspace_1"),
        id("run_1"),
        id("session_1"),
        &receipt(),
    )
    .expect("safe no-recommendation projection");
    let value = serde_json::to_value(&projection).expect("projection JSON");
    assert_eq!(
        value["recommendation"],
        json!({
            "recommendationKind": "no_recommendation",
            "reasonCode": "catalog_evidence_absent",
            "createdAt": 3_000,
        })
    );

    assert!(project_completed_bmad_help_run(
        &no_recommendation(),
        Some("injected display"),
        id("workspace_1"),
        id("run_1"),
        id("session_1"),
        &receipt(),
    )
    .is_err());
    assert!(project_completed_bmad_help_run(
        &recommended(),
        None,
        id("workspace_1"),
        id("run_1"),
        id("session_1"),
        &receipt(),
    )
    .is_err());
}

#[test]
fn completion_constructor_rejects_noncanonical_unsafe_or_path_like_display_data() {
    let mut wrong_session = recommended();
    if let BmadMethodHelpRecommendation::RecommendedCapability { session_id, .. } =
        &mut wrong_session
    {
        *session_id = id("session_other");
    }
    let wrong_session = seal_recommendation(wrong_session);
    assert!(project_completed_bmad_help_run(
        &wrong_session,
        Some("Create Architecture"),
        id("workspace_1"),
        id("run_1"),
        id("session_1"),
        &receipt(),
    )
    .is_err());

    for display_name in [
        "",
        "unsafe\nname",
        "unsafe\u{202e}name",
        "C:\\Users\\employee\\secret.md",
        "/home/employee/secret.md",
        "\\\\server\\share\\secret.md",
    ] {
        assert!(project_completed_bmad_help_run(
            &recommended(),
            Some(display_name),
            id("workspace_1"),
            id("run_1"),
            id("session_1"),
            &receipt(),
        )
        .is_err());
    }

    let mut hostile = recommended();
    if let BmadMethodHelpRecommendation::RecommendedCapability {
        rationale_summary, ..
    } = &mut hostile
    {
        *rationale_summary = "Read C:\\Users\\employee\\secret.md".to_owned();
    }
    let hostile = seal_recommendation(hostile);
    assert!(project_completed_bmad_help_run(
        &hostile,
        Some("Create Architecture"),
        id("workspace_1"),
        id("run_1"),
        id("session_1"),
        &receipt(),
    )
    .is_err());
}

#[test]
fn completion_constructor_rejects_invalid_receipt_metadata_and_chronology() {
    let mut cases = Vec::new();

    let mut invalid = receipt();
    invalid.region = "West-Europe".to_owned();
    cases.push(invalid);

    let mut invalid = receipt();
    invalid.input_bytes = 0;
    cases.push(invalid);

    let mut invalid = receipt();
    invalid.input_bytes = 4 * 1024 * 1024 + 1;
    cases.push(invalid);

    let mut invalid = receipt();
    invalid.output_bytes = 0;
    cases.push(invalid);

    let mut invalid = receipt();
    invalid.output_bytes = 1024 * 1024 + 1;
    cases.push(invalid);

    let mut invalid = receipt();
    invalid.started_at = UnixMillis(0);
    cases.push(invalid);

    let mut invalid = receipt();
    invalid.started_at = UnixMillis(2_001);
    cases.push(invalid);

    let mut invalid = receipt();
    invalid.completed_at = UnixMillis(MAX_SAFE_JSON_INTEGER + 1);
    cases.push(invalid);

    for invalid in cases {
        assert!(project_completed_bmad_help_run(
            &recommended(),
            Some("Create Architecture"),
            id("workspace_1"),
            id("run_1"),
            id("session_1"),
            &invalid,
        )
        .is_err());
    }

    let mut recommendation_before_receipt = recommended();
    if let BmadMethodHelpRecommendation::RecommendedCapability { created_at, .. } =
        &mut recommendation_before_receipt
    {
        *created_at = UnixMillis(1_999);
    }
    let recommendation_before_receipt = seal_recommendation(recommendation_before_receipt);
    assert!(project_completed_bmad_help_run(
        &recommendation_before_receipt,
        Some("Create Architecture"),
        id("workspace_1"),
        id("run_1"),
        id("session_1"),
        &receipt(),
    )
    .is_err());
}

#[test]
fn retained_completion_round_trips_and_binds_all_expected_identities() {
    let projection = completed_projection().expect("safe completed projection");
    let bytes = serde_json::to_vec(&projection).expect("projection bytes");
    assert_eq!(
        decode_retained_bmad_help_completion(
            &bytes,
            &id("workspace_1"),
            &id("run_1"),
            &id("session_1"),
        )
        .expect("strict retained projection"),
        projection
    );

    for (workspace_id, run_id, session_id) in [
        ("workspace_other", "run_1", "session_1"),
        ("workspace_1", "run_other", "session_1"),
        ("workspace_1", "run_1", "session_other"),
    ] {
        assert!(decode_retained_bmad_help_completion(
            &bytes,
            &id(workspace_id),
            &id(run_id),
            &id(session_id),
        )
        .is_err());
    }
}

#[test]
fn retained_completion_rejects_unknown_fields_union_smuggling_and_closed_literal_drift() {
    let value = serde_json::to_value(completed_projection().expect("safe projection"))
        .expect("projection JSON");

    for pointer in ["", "/recommendation", "/receipt"] {
        let mut hostile = value.clone();
        hostile
            .pointer_mut(pointer)
            .expect("fixture pointer")
            .as_object_mut()
            .expect("fixture object")
            .insert("proof".to_owned(), json!("forged"));
        assert_rejected(&hostile);
    }

    for (pointer, replacement) in [
        ("/schemaVersion", json!("bmad-help-completed.v2")),
        ("/runKind", json!("bmad_architecture")),
        ("/lifecycle", json!("advancing")),
        ("/runnable", json!(true)),
        ("/completionClaimed", json!(false)),
        (
            "/recommendation/recommendationKind",
            json!("arbitrary_result"),
        ),
        ("/recommendation/evidenceClass", json!("certain")),
        (
            "/receipt/schemaVersion",
            json!("bmad-model-receipt-summary.v2"),
        ),
        ("/receipt/status", json!("failed")),
        ("/receipt/retentionMode", json!("provider_store")),
        ("/receipt/region", json!("west-europe")),
        ("/receipt/startedAt", json!(2_001)),
        ("/receipt/completedAt", json!(MAX_SAFE_JSON_INTEGER + 1)),
        ("/receipt/inputBytes", json!(0)),
        ("/receipt/outputBytes", json!(1024 * 1024 + 1)),
        ("/recommendation/createdAt", json!(1_999)),
    ] {
        let mut hostile = value.clone();
        *hostile.pointer_mut(pointer).expect("fixture pointer") = replacement;
        assert_rejected(&hostile);
    }

    let mut smuggled = value.clone();
    smuggled["recommendation"]
        .as_object_mut()
        .expect("recommendation object")
        .insert("reasonCode".to_owned(), json!("catalog_evidence_absent"));
    assert_rejected(&smuggled);
}

#[test]
fn retained_completion_rejects_unsafe_duplicate_and_oversized_bytes() {
    let projection = completed_projection().expect("safe projection");
    let value = serde_json::to_value(&projection).expect("projection JSON");
    for (pointer, replacement) in [
        ("/recommendation/displayName", json!("unsafe\nname")),
        (
            "/recommendation/rationaleSummary",
            json!("unsafe\u{2066}text"),
        ),
        (
            "/recommendation/rationaleSummary",
            json!("Read /home/employee/secret.md"),
        ),
    ] {
        let mut hostile = value.clone();
        *hostile.pointer_mut(pointer).expect("fixture pointer") = replacement;
        assert_rejected(&hostile);
    }

    let bytes = serde_json::to_vec(&projection).expect("projection bytes");
    let text = String::from_utf8(bytes.clone()).expect("projection UTF-8");
    let duplicate = text.replacen(
        "\"schemaVersion\":\"bmad-help-completed.v1\"",
        "\"schemaVersion\":\"bmad-help-completed.v1\",\"schemaVersion\":\"bmad-help-completed.v1\"",
        1,
    );
    assert_ne!(duplicate, text, "duplicate fixture must be effective");
    assert!(decode_retained_bmad_help_completion(
        duplicate.as_bytes(),
        &id("workspace_1"),
        &id("run_1"),
        &id("session_1"),
    )
    .is_err());

    let mut oversized = bytes;
    oversized.resize(MAX_BMAD_HELP_COMPLETED_PROJECTION_BYTES + 1, b' ');
    assert!(decode_retained_bmad_help_completion(
        &oversized,
        &id("workspace_1"),
        &id("run_1"),
        &id("session_1"),
    )
    .is_err());
}

fn assert_rejected(value: &Value) {
    let bytes = serde_json::to_vec(value).expect("hostile projection JSON");
    assert!(decode_retained_bmad_help_completion(
        &bytes,
        &id("workspace_1"),
        &id("run_1"),
        &id("session_1"),
    )
    .is_err());
}
