#![allow(clippy::expect_used)]

use desktop_runtime::{
    canonical_hash_without_field, sha256_bytes, BuilderAnalysisRun, BuilderAuthoringAction,
    BuilderDraft, BuilderDraftRecord, BuilderDraftRevision, BuilderDraftState, BuilderErrorCode,
    ContractId,
};

fn fixture<T: serde::de::DeserializeOwned>(name: &str) -> T {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/contracts/fixtures/valid/bmad")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixture source"))
        .expect("fixture shape")
}

#[test]
fn inactive_builder_aggregate_versions_immutable_source_records() {
    let source: BuilderDraftRecord = fixture("builder-agent-draft.json");
    let revision: BuilderDraftRevision = fixture("builder-agent-revision.json");
    let analysis: BuilderAnalysisRun = fixture("builder-agent-analysis-deterministic.json");
    let mut draft = BuilderDraft::create(source).expect("draft");
    assert_eq!(draft.state(), BuilderDraftState::Drafting);
    assert_eq!(draft.version(), 1);

    draft
        .append_revision(1, revision.clone())
        .expect("first immutable revision");
    assert_eq!(draft.state(), BuilderDraftState::DraftReady);
    assert_eq!(draft.current_revision(), Some(&revision));
    draft
        .record_analysis(2, analysis.clone())
        .expect("analysis");
    assert_eq!(draft.state(), BuilderDraftState::Analyzed);
    assert_eq!(draft.analyses(), &[analysis]);
    draft.accept_for_review(3).expect("user accepts review");
    assert_eq!(draft.state(), BuilderDraftState::UserAccepted);

    let restored =
        BuilderDraft::from_persisted_json(&draft.to_persisted_json().expect("persisted aggregate"))
            .expect("authenticated shape reconstructs");
    assert_eq!(restored, draft);
}

#[test]
fn actions_are_kind_checked_and_convert_is_unrepresentable() {
    let source: BuilderDraftRecord = fixture("builder-agent-draft.json");
    let mut revision: BuilderDraftRevision = fixture("builder-agent-revision.json");
    revision.authoring_action = BuilderAuthoringAction::workflow_build();
    let mut draft = BuilderDraft::create(source).expect("draft");
    assert_eq!(
        draft
            .append_revision(1, revision)
            .expect_err("workflow build cannot author an agent")
            .code(),
        BuilderErrorCode::BuilderActionInvalidForKind
    );
    assert!(serde_json::from_str::<BuilderAuthoringAction>(
        r#"{"builderKind":"workflow","action":"convert"}"#
    )
    .is_err());
}

#[test]
fn blocked_superseded_and_abandoned_are_closed_inactive_states() {
    let source: BuilderDraftRecord = fixture("builder-workflow-draft.json");
    let revision: BuilderDraftRevision = fixture("builder-workflow-revision.json");

    let mut blocked = BuilderDraft::create(source.clone()).expect("draft");
    blocked.block(1).expect("block");
    assert_eq!(blocked.state(), BuilderDraftState::Blocked);

    let mut superseded = BuilderDraft::create(source.clone()).expect("draft");
    superseded.append_revision(1, revision).expect("revision");
    superseded.supersede_revision(2).expect("supersede");
    assert_eq!(superseded.state(), BuilderDraftState::Superseded);

    let mut abandoned = BuilderDraft::create(source).expect("draft");
    abandoned.abandon(1).expect("abandon");
    assert_eq!(abandoned.state(), BuilderDraftState::Abandoned);
}

#[test]
fn canonical_builder_fixtures_round_trip_without_identity_changes() {
    for name in ["builder-agent-draft.json", "builder-workflow-draft.json"] {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../packages/contracts/fixtures/valid/bmad")
                .join(name),
        )
        .expect("fixture source");
        let value: BuilderDraftRecord = serde_json::from_str(&source).expect("draft record");
        assert_eq!(
            serde_json::to_value(value).expect("round trip"),
            serde_json::from_str::<serde_json::Value>(&source).expect("fixture json")
        );
    }
    for name in [
        "builder-agent-revision.json",
        "builder-workflow-revision.json",
    ] {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../packages/contracts/fixtures/valid/bmad")
                .join(name),
        )
        .expect("fixture source");
        let value: BuilderDraftRevision = serde_json::from_str(&source).expect("revision record");
        assert_eq!(
            serde_json::to_value(value).expect("round trip"),
            serde_json::from_str::<serde_json::Value>(&source).expect("fixture json")
        );
    }
    for name in [
        "builder-agent-analysis-deterministic.json",
        "builder-agent-analysis-model-lens.json",
        "builder-workflow-analysis-deterministic.json",
        "builder-workflow-analysis-model-lens.json",
    ] {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../packages/contracts/fixtures/valid/bmad")
                .join(name),
        )
        .expect("fixture source");
        let value: BuilderAnalysisRun = serde_json::from_str(&source).expect("analysis record");
        assert_eq!(
            serde_json::to_value(value).expect("round trip"),
            serde_json::from_str::<serde_json::Value>(&source).expect("fixture json")
        );
    }
}

#[test]
fn model_lens_analysis_binds_one_exact_decision_and_never_becomes_evaluation() {
    let source: BuilderDraftRecord = fixture("builder-agent-draft.json");
    let revision: BuilderDraftRevision = fixture("builder-agent-revision.json");
    let analysis: BuilderAnalysisRun = fixture("builder-agent-analysis-model-lens.json");
    let mut draft = BuilderDraft::create(source).expect("draft");
    draft.append_revision(1, revision).expect("revision");
    draft
        .record_analysis(2, analysis.clone())
        .expect("source-grounded analysis");
    assert_eq!(draft.state(), BuilderDraftState::Analyzed);
    assert_eq!(analysis.evaluation_claim, "none");

    let mut replay = analysis;
    replay.analysis_id =
        ContractId::new("agentanalysis_01J99999999999999999999999").expect("valid analysis id");
    replay.analysis_hash =
        canonical_hash_without_field("bmad-builder-analysis", 1, &replay, "analysisHash")
            .expect("rehashed replay");
    assert_eq!(
        draft
            .record_analysis(3, replay)
            .expect_err("one context decision cannot authorize two analyses")
            .code(),
        BuilderErrorCode::BuilderRevisionStale
    );
}

#[test]
fn renderer_projection_contains_no_draft_file_bytes() {
    let source: BuilderDraftRecord = fixture("builder-workflow-draft.json");
    let revision: BuilderDraftRevision = fixture("builder-workflow-revision.json");
    let mut draft = BuilderDraft::create(source).expect("draft");
    let marker = revision.proposed_file_set.files[0].content.clone();
    draft.append_revision(1, revision).expect("revision");
    let projection = serde_json::to_string(&draft.renderer_projection()).expect("projection");
    assert!(!projection.contains(&marker));
    assert!(projection.contains("evaluation_unavailable"));
    assert!(!projection.contains("active"));
}

#[test]
fn edit_appends_a_new_revision_and_retains_the_parent() {
    let source: BuilderDraftRecord = fixture("builder-agent-draft.json");
    let first: BuilderDraftRevision = fixture("builder-agent-revision.json");
    let mut draft = BuilderDraft::create(source).expect("draft");
    draft.append_revision(1, first.clone()).expect("first");

    let mut edit = first.clone();
    edit.revision_id =
        ContractId::new("agentrevision_01J99999999999999999999999").expect("revision id");
    edit.authoring_action = BuilderAuthoringAction::edit(edit.builder_kind);
    edit.ordinal = 2;
    edit.parent_revision_hash = Some(first.revision_hash);
    edit.raw_result_hash = sha256_bytes(b"edited raw result");
    edit.revision_hash =
        canonical_hash_without_field("bmad-builder-revision", 1, &edit, "revisionHash")
            .expect("revision hash");
    draft.append_revision(2, edit.clone()).expect("edit");

    assert_eq!(draft.revisions(), &[first, edit.clone()]);
    assert_eq!(draft.current_revision(), Some(&edit));
}

#[test]
fn future_targets_effects_and_authority_fields_fail_during_strict_parsing() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/contracts/fixtures/valid/bmad/builder-agent-draft.json"),
    )
    .expect("fixture source");
    let future_target = source.replace("\"agent\"", "\"autonomous_agent\"");
    assert!(serde_json::from_str::<BuilderDraftRecord>(&future_target).is_err());
    let active = source.replace("\"none\"", "\"active\"");
    let active: BuilderDraftRecord = serde_json::from_str(&active).expect("closed string shape");
    assert!(BuilderDraft::create(active).is_err());
    let authority = source.replace(
        "\"draftEffect\": \"none\"",
        "\"draftEffect\": \"none\", \"command\": \"node build.js\"",
    );
    assert!(serde_json::from_str::<BuilderDraftRecord>(&authority).is_err());
}

#[test]
fn builder_authority_surface_has_no_future_lifecycle_operations() {
    let surface = include_str!("../src/bmad/builder_ports.rs").to_ascii_lowercase();
    for operation in [
        "register",
        "evaluate",
        "publish",
        "promote",
        "activate",
        "rollback",
        "materialize",
        "execute",
        "convert",
        "install",
    ] {
        assert!(
            !surface.contains(&format!("pub fn {operation}")),
            "future lifecycle operation leaked into Builder authority: {operation}"
        );
    }
}
