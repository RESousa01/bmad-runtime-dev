//! SRC-05 historical characterization. These assertions describe the pre-migration
//! boundary; they are migration evidence, not permission to preserve defects.
#![allow(clippy::expect_used)]

use desktop_runtime::{
    sha256_bytes, BmadCapabilityRun, BmadCapabilityRunError, BmadCapabilityRunParams,
    BmadClosureCapabilityId, BuilderActionName, BuilderAuthoringAction, BuilderDraft,
    BuilderDraftRecord, BuilderDraftRevision, BuilderDraftState, BuilderKind, ContractId,
    UnixMillis,
};

fn contract_fixture<T: serde::de::DeserializeOwned>(name: &str) -> T {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/contracts/fixtures/valid/bmad")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixture source"))
        .expect("fixture shape")
}

fn id(value: &str) -> ContractId {
    ContractId::new(value).expect("test identifier")
}

#[test]
fn src_05_rust_builder_001_bmad_current_characterization_freezes_store_integrity() {
    let evidence: serde_json::Value = serde_json::from_str(include_str!(
        "../../../packages/bmad-fixtures/fixtures/current/source-and-catalog.json"
    ))
    .expect("historical source/catalog evidence");
    let historical_ids = evidence["closureLedger"]["builderCapabilityIds"]
        .as_array()
        .expect("Builder capability IDs");
    assert_eq!(
        historical_ids,
        &[
            "builder:agent.analyze",
            "builder:agent.create_rebuild",
            "builder:agent.edit",
            "builder:workflow.analyze",
            "builder:workflow.build_edit",
        ]
    );

    let actions = [
        BuilderAuthoringAction {
            builder_kind: BuilderKind::Agent,
            action: BuilderActionName::Analyze,
        },
        BuilderAuthoringAction {
            builder_kind: BuilderKind::Agent,
            action: BuilderActionName::CreateRebuild,
        },
        BuilderAuthoringAction::edit(BuilderKind::Agent),
        BuilderAuthoringAction::analyze(BuilderKind::Workflow),
        BuilderAuthoringAction::workflow_build(),
    ];
    assert_eq!(
        actions
            .iter()
            .map(|action| serde_json::to_value(action).expect("action"))
            .collect::<Vec<_>>(),
        vec![
            serde_json::json!({"builderKind":"agent","action":"analyze"}),
            serde_json::json!({"builderKind":"agent","action":"create_rebuild"}),
            serde_json::json!({"builderKind":"agent","action":"edit"}),
            serde_json::json!({"builderKind":"workflow","action":"analyze"}),
            serde_json::json!({"builderKind":"workflow","action":"build"}),
        ]
    );
    assert!(
        serde_json::from_str::<BuilderAuthoringAction>(
            r#"{"builderKind":"workflow","action":"convert"}"#,
        )
        .is_err(),
        "Convert was unsupported before migration"
    );

    for (record_name, revision_name, expected_kind) in [
        (
            "builder-agent-draft.json",
            "builder-agent-revision.json",
            BuilderKind::Agent,
        ),
        (
            "builder-workflow-draft.json",
            "builder-workflow-revision.json",
            BuilderKind::Workflow,
        ),
    ] {
        let record: BuilderDraftRecord = contract_fixture(record_name);
        let revision: BuilderDraftRevision = contract_fixture(revision_name);
        let file_bytes = revision.proposed_file_set.files[0].content.clone();
        let mut draft = BuilderDraft::create(record.clone()).expect("inactive draft");
        draft
            .append_revision(1, revision.clone())
            .expect("immutable first revision");

        assert_eq!(draft.state(), BuilderDraftState::DraftReady);
        assert_eq!(draft.record(), &record);
        assert_eq!(draft.current_revision(), Some(&revision));

        let projection =
            serde_json::to_value(draft.renderer_projection()).expect("renderer projection");
        assert_eq!(
            projection["builderKind"],
            serde_json::to_value(expected_kind).expect("Builder kind")
        );
        assert_eq!(projection["draftEffect"], "none");
        assert_eq!(projection["evaluationCapability"], "evaluation_unavailable");
        assert!(
            !projection.to_string().contains(&file_bytes),
            "renderer projection exposed immutable draft bytes"
        );

        let persisted = draft.to_persisted_json().expect("persisted aggregate");
        let restored = BuilderDraft::from_persisted_json(&persisted).expect("store restore");
        assert_eq!(restored, draft);

        let mut tampered: serde_json::Value =
            serde_json::from_str(&persisted).expect("persisted JSON");
        tampered["unexpectedAuthority"] = serde_json::json!("activate");
        assert!(
            BuilderDraft::from_persisted_json(&tampered.to_string()).is_err(),
            "unknown store authority survived strict recovery"
        );
    }

    let unsupported = BmadCapabilityRun::open(BmadCapabilityRunParams {
        run_id: id("run_01ARZ3NDEKTSV4RRFFQ69G5FAV"),
        capability_id: BmadClosureCapabilityId::new("bmm:bmad-help").expect("capability"),
        workspace_id: id("workspace_01ARZ3NDEKTSV4RRFFQ69G5FAV"),
        instruction_hash: sha256_bytes(b"instruction"),
        context_manifest_hash: sha256_bytes(b"context"),
        output_schema_id: "sapphirus.future-output.v1".to_owned(),
        consent_evidence_id: id("consent_01ARZ3NDEKTSV4RRFFQ69G5FAV"),
        created_at: UnixMillis(1_000),
    })
    .expect_err("unknown output state");
    assert_eq!(unsupported, BmadCapabilityRunError::UnknownOutputSchema);
}
