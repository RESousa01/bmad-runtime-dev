#![allow(clippy::expect_used)]

use desktop_runtime::{
    canonical_hash_without_field, sha256_bytes, AuthorityRef, BuilderAnalysisRun,
    BuilderAuthoringAction, BuilderDraft, BuilderDraftRecord, BuilderDraftRevision,
    BuilderDraftState, BuilderErrorCode, BuilderModelAnalysisDecisionInput, ContractId,
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
    draft.bind_authority(authority()).expect("authority");
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
    draft.bind_authority(authority()).expect("authority");
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
    draft.bind_authority(authority()).expect("authority");
    draft.append_revision(1, revision).expect("revision");
    assert_eq!(
        draft
            .record_analysis(2, analysis.clone())
            .expect_err("an unreviewed model result has no authority")
            .code(),
        BuilderErrorCode::BuilderContextDecisionMissing
    );
    let decision = draft
        .issue_model_analysis_decision(2, decision_input(&analysis))
        .expect("host-issued exact-revision decision");
    draft
        .record_analysis(3, analysis.clone())
        .expect("source-grounded analysis");
    assert_eq!(draft.state(), BuilderDraftState::Analyzed);
    assert_eq!(draft.analyses()[0].evaluation_claim, "none");
    assert_eq!(draft.analysis_consumptions().len(), 1);
    assert_eq!(
        draft.analysis_consumptions()[0].decision_id,
        decision.decision_id
    );

    let mut replay = analysis;
    replay.analysis_id =
        ContractId::new("agentanalysis_01J99999999999999999999999").expect("valid analysis id");
    replay.analysis_hash =
        canonical_hash_without_field("bmad-builder-analysis", 1, &replay, "analysisHash")
            .expect("rehashed replay");
    assert_eq!(
        draft
            .record_analysis(4, replay)
            .expect_err("one context decision cannot authorize two analyses")
            .code(),
        BuilderErrorCode::BuilderContextDecisionMissing
    );
}

#[test]
fn model_lens_rejects_forged_and_revision_drifted_decisions() {
    let source: BuilderDraftRecord = fixture("builder-agent-draft.json");
    let revision: BuilderDraftRevision = fixture("builder-agent-revision.json");
    let analysis: BuilderAnalysisRun = fixture("builder-agent-analysis-model-lens.json");
    let mut draft = BuilderDraft::create(source).expect("draft");
    draft.bind_authority(authority()).expect("authority");
    draft
        .append_revision(1, revision.clone())
        .expect("revision");
    draft
        .issue_model_analysis_decision(2, decision_input(&analysis))
        .expect("decision");

    let mut forged = analysis.clone();
    forged
        .model_binding
        .as_mut()
        .expect("model binding")
        .context_decision_id =
        ContractId::new("decision_01J99999999999999999999999").expect("decision id");
    assert_eq!(
        draft
            .record_analysis(3, forged)
            .expect_err("fresh caller IDs cannot fabricate consent")
            .code(),
        BuilderErrorCode::BuilderContextDecisionInvalid
    );
    assert!(draft.pending_analysis_decision().is_some());

    let mut edit = revision.clone();
    edit.revision_id =
        ContractId::new("agentrevision_01J99999999999999999999999").expect("revision id");
    edit.authoring_action = BuilderAuthoringAction::edit(edit.builder_kind);
    edit.ordinal = 2;
    edit.parent_revision_hash = Some(revision.revision_hash);
    edit.raw_result_hash = sha256_bytes(b"decision-invalidating edit");
    edit.revision_hash =
        canonical_hash_without_field("bmad-builder-revision", 1, &edit, "revisionHash")
            .expect("revision hash");
    draft.append_revision(3, edit).expect("new exact revision");
    assert!(draft.pending_analysis_decision().is_none());
    assert_eq!(
        draft
            .record_analysis(4, analysis)
            .expect_err("a decision cannot cross a revision boundary")
            .code(),
        BuilderErrorCode::BuilderRevisionStale
    );
}

fn decision_input(analysis: &BuilderAnalysisRun) -> BuilderModelAnalysisDecisionInput {
    let binding = analysis.model_binding.as_ref().expect("model binding");
    BuilderModelAnalysisDecisionInput {
        decision_id: binding.context_decision_id.clone(),
        invocation_id: binding.invocation_id.clone(),
        source_member_set_hash: analysis.source_member_set_hash,
        deterministic_facts_hash: analysis.deterministic_facts_hash,
        model_hash: binding.model_hash,
        deployment_hash: binding.deployment_hash,
        model_profile_hash: binding.model_profile_hash,
        schema_hash: binding.schema_hash,
        consent_hash: binding.consent_hash,
        reviewed_at: analysis.created_at.clone(),
    }
}

fn authority() -> AuthorityRef {
    AuthorityRef {
        authority_kind: "desktop_local_store".to_owned(),
        authority_id: ContractId::new("authority_01J00000000000000000000000")
            .expect("authority id"),
        installation_id: ContractId::new("install_01J00000000000000000000000")
            .expect("installation id"),
        local_store_id: ContractId::new("store_01J00000000000000000000000").expect("store id"),
        authority_epoch: 1,
    }
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
