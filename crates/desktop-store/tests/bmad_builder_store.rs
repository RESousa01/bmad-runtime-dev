#![allow(clippy::expect_used)]

use desktop_runtime::{
    canonical_hash_without_field, sha256_bytes, AuthorityRef, BuilderAnalysisRun,
    BuilderApprovalDecision, BuilderAuthoringService, BuilderDraft, BuilderDraftRecord,
    BuilderDraftRepository, BuilderDraftRevision, BuilderDraftState, BuilderLifecycle,
    BuilderLifecycleRepository, BuilderLifecycleState, BuilderModelAnalysisDecisionInput,
    BuilderPersistenceEvent, BuilderPromotion, BuilderPromotionApproval,
    BuilderPromotionInstallation, BuilderPromotionStage, ContractId, Sha256Digest,
};
use desktop_store::{KeyProtector, LocalStore, StoreError};
use sha2::{Digest, Sha256};

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
    ContractId::new(value).expect("valid contract id")
}

fn authority() -> AuthorityRef {
    AuthorityRef {
        authority_kind: "desktop_local_store".to_owned(),
        authority_id: id("authority_01J00000000000000000000000"),
        installation_id: id("install_01J00000000000000000000000"),
        local_store_id: id("store_01J00000000000000000000000"),
        authority_epoch: 1,
    }
}

fn fixture<T: serde::de::DeserializeOwned>(name: &str) -> T {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/contracts/fixtures/valid/bmad")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixture source"))
        .expect("fixture shape")
}

#[test]
fn builder_repository_persists_immutable_history_and_recovers_after_restart(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let service = BuilderAuthoringService::new(store);
    let source: BuilderDraftRecord = fixture("builder-agent-draft.json");
    let revision: BuilderDraftRevision = fixture("builder-agent-revision.json");
    let analysis: BuilderAnalysisRun = fixture("builder-agent-analysis-deterministic.json");
    let created = service.create_draft(source, authority())?;
    let scope = created.scope().expect("authority-bound scope");
    let revised =
        service.append_revision(&scope, &created.record().draft_id, 1, revision.clone())?;
    let analyzed =
        service.record_analysis(&scope, &created.record().draft_id, 2, analysis.clone())?;
    assert_eq!(revised.current_revision(), Some(&revision));
    assert_eq!(analyzed.analyses(), &[analysis]);
    assert_eq!(analyzed.state(), BuilderDraftState::Analyzed);
    drop(service);

    let reopened = LocalStore::open(directory.path(), &TestProtector)?;
    let restored = reopened
        .load_builder_draft(&scope, &created.record().draft_id)?
        .expect("retained draft");
    assert_eq!(restored, analyzed);
    reopened.verify_integrity()?;
    Ok(())
}

#[test]
fn builder_scope_isolates_owner_project_session_and_authority(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let service = BuilderAuthoringService::new(store);
    let created = service.create_draft(fixture("builder-workflow-draft.json"), authority())?;
    let scope = created.scope().expect("scope");
    assert!(service
        .repository()
        .load_builder_draft(&scope, &created.record().draft_id)?
        .is_some());

    let mut wrong = scope.clone();
    wrong.authoring_session_id = id("authorsession_01J99999999999999999999999");
    assert!(service
        .repository()
        .load_builder_draft(&wrong, &created.record().draft_id)?
        .is_none());
    wrong = scope;
    wrong.authority_ref.authority_id = id("authority_01J99999999999999999999999");
    assert!(service
        .repository()
        .load_builder_draft(&wrong, &created.record().draft_id)?
        .is_none());
    Ok(())
}

#[test]
fn model_analysis_decision_consumption_is_single_use_and_atomic(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let service = BuilderAuthoringService::new(store);
    let created = service.create_draft(fixture("builder-agent-draft.json"), authority())?;
    let scope = created.scope().expect("scope");
    let revised = service.append_revision(
        &scope,
        &created.record().draft_id,
        1,
        fixture("builder-agent-revision.json"),
    )?;
    let analysis: BuilderAnalysisRun = fixture("builder-agent-analysis-model-lens.json");
    let issued = service.issue_model_analysis_decision(
        &scope,
        &created.record().draft_id,
        revised.version(),
        decision_input(&analysis),
    )?;
    let analyzed = service.record_analysis(
        &scope,
        &created.record().draft_id,
        issued.version(),
        analysis.clone(),
    )?;
    let mut replay = analysis;
    replay.analysis_id = id("agentanalysis_01J99999999999999999999999");
    replay.analysis_hash =
        canonical_hash_without_field("bmad-builder-analysis", 1, &replay, "analysisHash")?;
    assert!(service
        .record_analysis(
            &scope,
            &created.record().draft_id,
            analyzed.version(),
            replay,
        )
        .is_err());
    let retained = service
        .repository()
        .load_builder_draft(&scope, &created.record().draft_id)?
        .expect("retained");
    assert_eq!(retained.analyses().len(), 1);

    let connection = rusqlite::Connection::open(service.repository().database_path())?;
    let (events, outbox): (u64, u64) = connection.query_row(
        "SELECT
           (SELECT COUNT(*) FROM evidence_events WHERE event_type LIKE 'bmad.builder.%'),
           (SELECT COUNT(*) FROM outbox o JOIN evidence_events e ON e.event_id = o.event_id
             WHERE e.event_type LIKE 'bmad.builder.%')",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!((events, outbox), (4, 4));
    let (issued_count, consumed_count): (u64, u64) = connection.query_row(
        "SELECT COUNT(*), COUNT(consumed_analysis_id)
         FROM bmad_builder_analysis_decisions",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!((issued_count, consumed_count), (1, 1));
    Ok(())
}

#[test]
fn forged_scope_cannot_consume_a_reviewed_decision() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let service = BuilderAuthoringService::new(store);
    let first = service.create_draft(fixture("builder-agent-draft.json"), authority())?;
    let first_scope = first.scope().expect("scope");
    let first = service.append_revision(
        &first_scope,
        &first.record().draft_id,
        1,
        fixture("builder-agent-revision.json"),
    )?;
    let analysis: BuilderAnalysisRun = fixture("builder-agent-analysis-model-lens.json");
    let issued = service.issue_model_analysis_decision(
        &first_scope,
        &first.record().draft_id,
        first.version(),
        decision_input(&analysis),
    )?;

    let mut wrong_scope = first_scope.clone();
    wrong_scope.owner_scope_ref = id("owner_01J999999999999999999999999");
    assert!(service
        .record_analysis(
            &wrong_scope,
            &first.record().draft_id,
            issued.version(),
            analysis.clone(),
        )
        .is_err());

    let connection = rusqlite::Connection::open(service.repository().database_path())?;
    let consumed: u64 = connection.query_row(
        "SELECT COUNT(*) FROM bmad_builder_analysis_decisions
         WHERE consumed_analysis_id IS NOT NULL",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(consumed, 0);
    Ok(())
}

#[test]
fn cross_draft_replay_and_failed_analysis_are_atomic() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let service = BuilderAuthoringService::new(store);
    let first = service.create_draft(fixture("builder-agent-draft.json"), authority())?;
    let first_scope = first.scope().expect("scope");
    let first = service.append_revision(
        &first_scope,
        &first.record().draft_id,
        1,
        fixture("builder-agent-revision.json"),
    )?;
    let analysis: BuilderAnalysisRun = fixture("builder-agent-analysis-model-lens.json");
    let issued = service.issue_model_analysis_decision(
        &first_scope,
        &first.record().draft_id,
        first.version(),
        decision_input(&analysis),
    )?;

    let mut second_source: BuilderDraftRecord = fixture("builder-agent-draft.json");
    second_source.draft_id = id("agentdraft_01J99999999999999999999999");
    second_source.authoring_session_id = id("authorsession_01J99999999999999999999999");
    let second = service.create_draft(second_source, authority())?;
    let second_scope = second.scope().expect("second scope");
    let mut second_revision: BuilderDraftRevision = fixture("builder-agent-revision.json");
    second_revision.draft_id = second.record().draft_id.clone();
    second_revision.revision_id = id("agentrevision_01J99999999999999999999999");
    second_revision.revision_hash =
        canonical_hash_without_field("bmad-builder-revision", 1, &second_revision, "revisionHash")?;
    let second = service.append_revision(
        &second_scope,
        &second.record().draft_id,
        second.version(),
        second_revision.clone(),
    )?;
    assert!(service
        .issue_model_analysis_decision(
            &second_scope,
            &second.record().draft_id,
            second.version(),
            decision_input(&analysis),
        )
        .is_err());

    let first_analyzed = service.record_analysis(
        &first_scope,
        &first.record().draft_id,
        issued.version(),
        analysis.clone(),
    )?;

    let mut second_analysis = analysis;
    second_analysis.draft_id = second.record().draft_id.clone();
    second_analysis.revision_id = second_revision.revision_id.clone();
    second_analysis.revision_hash = second_revision.revision_hash;
    for result in second_analysis
        .model_lens_results
        .as_mut()
        .expect("model lens results")
    {
        result.revision_id = second_revision.revision_id.clone();
        result.revision_hash = second_revision.revision_hash;
    }
    let second_binding = second_analysis
        .model_binding
        .as_mut()
        .expect("model binding");
    second_binding.context_decision_id = id("decision_01J77777777777777777777777");
    second_binding.invocation_id = id("invoke_01J77777777777777777777777");
    let second_issued = service.issue_model_analysis_decision(
        &second_scope,
        &second.record().draft_id,
        second.version(),
        decision_input(&second_analysis),
    )?;
    assert!(service
        .record_analysis(
            &second_scope,
            &second.record().draft_id,
            second_issued.version(),
            second_analysis,
        )
        .is_err());
    let retained_second = service
        .repository()
        .load_builder_draft(&second_scope, &second.record().draft_id)?
        .expect("second retained");
    assert_eq!(retained_second.version(), second_issued.version());
    assert!(retained_second.analyses().is_empty());
    assert!(retained_second.pending_analysis_decision().is_some());
    let connection = rusqlite::Connection::open(service.repository().database_path())?;
    let second_consumed: u64 = connection.query_row(
        "SELECT COUNT(*) FROM bmad_builder_analysis_decisions
         WHERE draft_id = ?1 AND consumed_analysis_id IS NOT NULL",
        [second.record().draft_id.as_str()],
        |row| row.get(0),
    )?;
    assert_eq!(second_consumed, 0);
    assert_eq!(first_analyzed.analyses().len(), 1);
    Ok(())
}

#[test]
fn pending_decision_invalidation_survives_every_edit_and_closure_restart(
) -> Result<(), Box<dyn std::error::Error>> {
    for transition in ["edit", "supersede", "accept", "block", "abandon"] {
        let directory = tempfile::tempdir()?;
        let store = LocalStore::open(directory.path(), &TestProtector)?;
        let service = BuilderAuthoringService::new(store);
        let issued = issued_builder(&service)?;
        let scope = issued.scope().expect("scope");
        let draft_id = issued.record().draft_id.clone();
        let transitioned = match transition {
            "edit" => service.append_revision(
                &scope,
                &draft_id,
                issued.version(),
                edited_revision(&issued)?,
            )?,
            "supersede" => service.supersede_revision(&scope, &draft_id, issued.version())?,
            "accept" => service.accept_for_review(&scope, &draft_id, issued.version())?,
            "block" => service.block(&scope, &draft_id, issued.version())?,
            "abandon" => service.abandon(&scope, &draft_id, issued.version())?,
            _ => unreachable!("closed test cases"),
        };
        assert!(transitioned.pending_analysis_decision().is_none());
        drop(service);

        let reopened = LocalStore::open(directory.path(), &TestProtector)?;
        let restored = reopened
            .load_builder_draft(&scope, &draft_id)?
            .expect("valid invalidation must survive restart");
        assert_eq!(restored, transitioned, "transition: {transition}");
        reopened.verify_integrity()?;
    }
    Ok(())
}

#[test]
fn pending_decision_index_cannot_claim_a_fabricated_consumption(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let service = BuilderAuthoringService::new(store);
    let _issued = issued_builder(&service)?;
    let connection = rusqlite::Connection::open(service.repository().database_path())?;
    connection.execute(
        "UPDATE bmad_builder_analysis_decisions
         SET disposition = 'consumed', consumed_analysis_id = 'analysis_forged',
             consumption_id = 'consume_forged',
             consumption_hash = 'sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff',
             consumed_at = '2026-07-14T12:00:00.000Z'",
        [],
    )?;
    drop(connection);
    assert!(service.repository().verify_integrity().is_err());
    Ok(())
}

fn issued_builder(
    service: &BuilderAuthoringService<LocalStore>,
) -> Result<BuilderDraft, Box<dyn std::error::Error>> {
    let created = service.create_draft(fixture("builder-agent-draft.json"), authority())?;
    let scope = created.scope().expect("scope");
    let revised = service.append_revision(
        &scope,
        &created.record().draft_id,
        created.version(),
        fixture("builder-agent-revision.json"),
    )?;
    let analysis: BuilderAnalysisRun = fixture("builder-agent-analysis-model-lens.json");
    Ok(service.issue_model_analysis_decision(
        &scope,
        &created.record().draft_id,
        revised.version(),
        decision_input(&analysis),
    )?)
}

fn edited_revision(
    draft: &BuilderDraft,
) -> Result<BuilderDraftRevision, Box<dyn std::error::Error>> {
    let parent = draft.current_revision().expect("current revision");
    let mut edit = parent.clone();
    edit.revision_id = id("agentrevision_01J99999999999999999999999");
    edit.authoring_action = desktop_runtime::BuilderAuthoringAction::edit(edit.builder_kind);
    edit.ordinal = 2;
    edit.parent_revision_hash = Some(parent.revision_hash);
    edit.raw_result_hash = desktop_runtime::sha256_bytes(b"persisted invalidating edit");
    edit.revision_hash =
        canonical_hash_without_field("bmad-builder-revision", 1, &edit, "revisionHash")?;
    Ok(edit)
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

#[test]
fn builder_payload_tamper_enters_read_only_recovery() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let service = BuilderAuthoringService::new(store);
    let created = service.create_draft(fixture("builder-workflow-draft.json"), authority())?;
    let scope = created.scope().expect("scope");
    let _ = service.append_revision(
        &scope,
        &created.record().draft_id,
        1,
        fixture("builder-workflow-revision.json"),
    )?;
    let database_path = service.repository().database_path();
    drop(service);

    let connection = rusqlite::Connection::open(database_path)?;
    let (content_hash, kind, schema): (String, String, String) = connection.query_row(
        "SELECT content_hash, content_kind, content_schema_version
         FROM bmad_builder_revisions LIMIT 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    drop(connection);
    let storage_preimage = format!("sapphirus:cas-storage:1\n{kind}\n{schema}\n{content_hash}");
    let digest = hex::encode(Sha256::digest(storage_preimage.as_bytes()));
    let payload = directory.path().join("cas").join(&digest[..2]).join(digest);
    let mut bytes = std::fs::read(&payload)?;
    let index = bytes.len() / 2;
    bytes[index] ^= 0x01;
    std::fs::write(payload, bytes)?;

    let reopened = LocalStore::open(directory.path(), &TestProtector)?;
    assert!(reopened.verify_integrity().is_err());
    assert!(reopened
        .load_builder_draft(&scope, &created.record().draft_id)
        .is_err());
    drop(reopened);
    let recovery = LocalStore::open_read_only_recovery(directory.path(), &TestProtector)?;
    assert!(recovery
        .load_builder_draft(&scope, &created.record().draft_id)
        .is_err());
    Ok(())
}

#[test]
fn v5_upgrade_and_interrupted_v10_migration_match_fresh_schema(
) -> Result<(), Box<dyn std::error::Error>> {
    let fresh_directory = tempfile::tempdir()?;
    let fresh = LocalStore::open(fresh_directory.path(), &TestProtector)?;
    let expected = fresh.schema_catalog()?;
    assert_eq!(fresh.schema_version()?, 24);

    for interrupted in [false, true] {
        let directory = tempfile::tempdir()?;
        let store = LocalStore::open(directory.path(), &TestProtector)?;
        let database_path = store.database_path();
        drop(store);
        let connection = rusqlite::Connection::open(&database_path)?;
        connection.execute_batch(
            "PRAGMA foreign_keys = OFF;
             DROP TABLE bmad_artifact_events;
DROP TABLE bmad_memory_entries;
             DROP TABLE bmad_artifacts;
             DROP TABLE bmad_session_events;
             DROP TABLE bmad_sessions;
             DROP TABLE bmad_builder_lifecycle_events;
             DROP TABLE bmad_builder_lifecycles;
             DROP TABLE bmad_installation_events;
             DROP TABLE bmad_installations;
             DROP TABLE model_profile_selections;
             DROP TABLE model_catalog_snapshots;
             DROP TABLE chat_messages;
             DROP TABLE chat_sessions;
             DROP TABLE bmad_memlog_entries;
             DROP TABLE bmad_architecture_session_steps;
             DROP TABLE payload_quarantine;
             DROP TABLE bmad_architecture_sessions;
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
             ALTER TABLE payloads DROP COLUMN verified_at;
             
             PRAGMA user_version = 5;",
        )?;
        if interrupted {
            connection.execute_batch(
                "BEGIN IMMEDIATE;
                 CREATE TABLE bmad_builder_drafts (draft_id TEXT PRIMARY KEY) STRICT;",
            )?;
        }
        drop(connection);

        let reopened = LocalStore::open(directory.path(), &TestProtector)?;
        assert_eq!(reopened.schema_version()?, 24);
        assert_eq!(reopened.schema_catalog()?, expected);
    }
    Ok(())
}

#[test]
fn populated_v6_model_analysis_is_refused_without_inventing_consent_history(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let service = BuilderAuthoringService::new(store);
    let issued = issued_builder(&service)?;
    let scope = issued.scope().expect("scope");
    let draft_id = issued.record().draft_id.clone();
    let analysis: BuilderAnalysisRun = fixture("builder-agent-analysis-model-lens.json");
    let _ = service.record_analysis(&scope, &draft_id, issued.version(), analysis)?;
    let database_path = service.repository().database_path();
    drop(service);

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
         ALTER TABLE payloads DROP COLUMN verified_at;
         
         PRAGMA user_version = 6;",
    )?;
    drop(connection);

    assert!(LocalStore::open(directory.path(), &TestProtector).is_err());
    let connection = rusqlite::Connection::open(&database_path)?;
    let version: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    assert_eq!(version, 6);
    let decision_table_count: u64 = connection.query_row(
        "SELECT COUNT(*) FROM sqlite_schema
         WHERE type = 'table' AND name = 'bmad_builder_analysis_decisions'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(decision_table_count, 0);
    Ok(())
}

#[test]
fn concurrent_builder_revisions_use_optimistic_projection_version(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let initial = LocalStore::open(directory.path(), &TestProtector)?;
    let service = BuilderAuthoringService::new(initial);
    let created = service.create_draft(fixture("builder-workflow-draft.json"), authority())?;
    let scope = created.scope().expect("scope");
    let draft_id = created.record().draft_id.clone();
    drop(service);

    let first_store = LocalStore::open(directory.path(), &TestProtector)?;
    let second_store = LocalStore::open(directory.path(), &TestProtector)?;
    let mut first = first_store
        .load_builder_draft(&scope, &draft_id)?
        .expect("first snapshot");
    let mut second = second_store
        .load_builder_draft(&scope, &draft_id)?
        .expect("second snapshot");
    let revision: BuilderDraftRevision = fixture("builder-workflow-revision.json");
    first.append_revision(1, revision.clone())?;
    second.append_revision(1, revision)?;
    first_store.persist_builder_transition(&first, 1, BuilderPersistenceEvent::RevisionAppended)?;
    assert!(second_store
        .persist_builder_transition(&second, 1, BuilderPersistenceEvent::RevisionAppended)
        .is_err());
    let retained = first_store
        .load_builder_draft(&scope, &draft_id)?
        .expect("winner retained");
    assert_eq!(retained.revisions().len(), 1);
    Ok(())
}

#[test]
fn v23_schema_carries_promotion_lifecycle_but_no_undesigned_surface(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let catalog = store
        .schema_catalog()?
        .into_iter()
        .map(|entry| format!("{} {} {} {}", entry.0, entry.1, entry.2, entry.3))
        .collect::<Vec<_>>()
        .join("\n")
        .to_ascii_lowercase();
    // ENG-15 adds the promotion lifecycle (validate/evaluate/promote/install/
    // activate/rollback) as `bmad_builder_lifecycles` and its event log, so
    // those substrings are now expected. What stays forbidden is surface this
    // task never designed: standalone evaluation/publication/registration
    // tables that would let a package activate without going through the
    // one promotion record this migration adds.
    for forbidden in [
        "builder_evaluation",
        "builder_publication",
        "builder_registration",
    ] {
        assert!(!catalog.contains(forbidden));
    }
    assert!(catalog.contains("bmad_builder_lifecycles"));
    Ok(())
}

fn subject_hash() -> Sha256Digest {
    sha256_bytes(b"builder-package-subject")
}

fn promotion_approval(decision: BuilderApprovalDecision) -> BuilderPromotionApproval {
    BuilderPromotionApproval {
        approval_id: None,
        decision,
        required: true,
    }
}

fn promotion_installation() -> BuilderPromotionInstallation {
    BuilderPromotionInstallation {
        atomic: serde_json::Value::Bool(true),
        catalog_generation: std::num::NonZeroU64::new(1).expect("catalog generation"),
        live_conformance_hash: desktop_runtime::generated_contracts::CommonSha256::try_from(
            sha256_bytes(b"live-conformance").to_string(),
        )
        .expect("conformance digest"),
    }
}

fn promotion(
    suffix: &str,
    from: BuilderLifecycleState,
    to: BuilderLifecycleState,
    stage: BuilderPromotionStage,
    decision: BuilderApprovalDecision,
) -> BuilderPromotion {
    let needs_install = matches!(
        to,
        BuilderLifecycleState::Installed | BuilderLifecycleState::Active
    );
    BuilderPromotion {
        promotion_id: id(&format!("promotion_01J0000000000000000000000{suffix}")),
        lifecycle_id: id("lifecycle_01J00000000000000000000000"),
        subject_hash: subject_hash(),
        from_state: from,
        to_state: to,
        stage,
        approval: promotion_approval(decision),
        installation: needs_install.then(promotion_installation),
        receipt_hash: sha256_bytes(b"unsealed"),
    }
    .seal()
    .expect("sealed promotion")
}

fn open_lifecycle(store: &LocalStore) -> Result<BuilderLifecycle, StoreError> {
    let lifecycle = BuilderLifecycle::open(
        id("lifecycle_01J00000000000000000000000"),
        id("agentdraft_01J00000000000000000000000"),
        subject_hash(),
    );
    store.create_builder_lifecycle(&lifecycle, 1_000)?;
    Ok(lifecycle)
}

const LIFECYCLE_WALK: [(
    &str,
    BuilderLifecycleState,
    BuilderLifecycleState,
    BuilderPromotionStage,
); 7] = [
    (
        "1",
        BuilderLifecycleState::Draft,
        BuilderLifecycleState::Validated,
        BuilderPromotionStage::Validation,
    ),
    (
        "2",
        BuilderLifecycleState::Validated,
        BuilderLifecycleState::Analysed,
        BuilderPromotionStage::Analysis,
    ),
    (
        "3",
        BuilderLifecycleState::Analysed,
        BuilderLifecycleState::Evaluated,
        BuilderPromotionStage::Evaluation,
    ),
    (
        "4",
        BuilderLifecycleState::Evaluated,
        BuilderLifecycleState::Reviewed,
        BuilderPromotionStage::Review,
    ),
    (
        "5",
        BuilderLifecycleState::Reviewed,
        BuilderLifecycleState::Approved,
        BuilderPromotionStage::Approval,
    ),
    (
        "6",
        BuilderLifecycleState::Approved,
        BuilderLifecycleState::Staged,
        BuilderPromotionStage::Staging,
    ),
    (
        "7",
        BuilderLifecycleState::Staged,
        BuilderLifecycleState::Installed,
        BuilderPromotionStage::Installation,
    ),
];

fn walk_to_installed(
    store: &LocalStore,
    lifecycle: &mut BuilderLifecycle,
    approval: BuilderApprovalDecision,
) -> Result<(), Box<dyn std::error::Error>> {
    for (index, (suffix, from, to, stage)) in LIFECYCLE_WALK.into_iter().enumerate() {
        let decision = if to == BuilderLifecycleState::Approved {
            approval
        } else {
            BuilderApprovalDecision::NotRequired
        };
        let step = promotion(suffix, from, to, stage, decision);
        let version = lifecycle.version();
        let event = lifecycle.promote(version, step.clone())?;
        store.commit_builder_lifecycle_transition(
            lifecycle,
            &step,
            &event,
            2_000 + index as u64,
        )?;
    }
    Ok(())
}

// Break caught: a store migration that added the promotion lifecycle rewrote
// the draft rows it passed over, so a draft authored before the upgrade came
// back with a different revision hash and its analysis history no longer
// verified. The migration must only ever add tables.
#[test]
fn drafts_written_before_the_v23_migration_survive_it_byte_for_byte(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let (scope, draft_id, before) = {
        let store = LocalStore::open(directory.path(), &TestProtector)?;
        let service = BuilderAuthoringService::new(store);
        let created = service.create_draft(fixture("builder-agent-draft.json"), authority())?;
        let scope = created.scope().expect("scope");
        let draft_id = created.record().draft_id.clone();
        service.append_revision(&scope, &draft_id, 1, fixture("builder-agent-revision.json"))?;
        let analysed = service.record_analysis(
            &scope,
            &draft_id,
            2,
            fixture("builder-agent-analysis-deterministic.json"),
        )?;
        (scope, draft_id, analysed)
    };
    let revision_hash_before = before
        .current_revision()
        .expect("revision")
        .revision_hash
        .to_string();
    let analysis_hash_before = before.analyses()[0].analysis_hash.to_string();

    // Wind the store back to a genuine v22: drop only what v23 added, so the
    // reopen below can be satisfied by nothing except the v22 -> v23 step.
    let database_path = directory.path().join("authority.sqlite3");
    let connection = rusqlite::Connection::open(&database_path)?;
    connection.execute_batch(
        "PRAGMA foreign_keys = OFF;
         DROP TABLE bmad_builder_lifecycle_events;
         DROP TABLE bmad_builder_lifecycles;
         DROP TABLE bmad_installation_events;
         DROP TABLE bmad_installations;
         PRAGMA user_version = 22;",
    )?;
    let wound_back: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    assert_eq!(wound_back, 22);
    drop(connection);

    let reopened = LocalStore::open(directory.path(), &TestProtector)?;
    assert_eq!(reopened.schema_version()?, 24);
    let after = reopened
        .load_builder_draft(&scope, &draft_id)?
        .expect("draft survives the migration");
    assert_eq!(after, before);
    assert_eq!(
        after
            .current_revision()
            .expect("revision")
            .revision_hash
            .to_string(),
        revision_hash_before
    );
    assert_eq!(
        after.analyses()[0].analysis_hash.to_string(),
        analysis_hash_before
    );
    reopened.verify_integrity()?;
    Ok(())
}

// Break caught: the analysis decision was consumed by one analysis and then
// consumed again by a second one, because the single-use guard lived only in
// the in-memory aggregate. The v23 tables must not have loosened it.
#[test]
fn the_single_use_analysis_decision_survives_the_lifecycle_migration(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    assert_eq!(store.schema_version()?, 24);
    let service = BuilderAuthoringService::new(store);
    let issued = issued_builder(&service)?;
    let scope = issued.scope().expect("scope");
    let draft_id = issued.record().draft_id.clone();
    let analysis: BuilderAnalysisRun = fixture("builder-agent-analysis-model-lens.json");
    let consumed =
        service.record_analysis(&scope, &draft_id, issued.version(), analysis.clone())?;
    assert_eq!(consumed.analysis_consumptions().len(), 1);

    let mut replay = analysis;
    replay.analysis_id = id("agentanalysis_01J99999999999999999999999");
    assert!(service
        .record_analysis(&scope, &draft_id, consumed.version(), replay)
        .is_err());
    let reloaded = service
        .repository()
        .load_builder_draft(&scope, &draft_id)?
        .expect("draft");
    assert_eq!(reloaded.analysis_consumptions().len(), 1);
    assert!(reloaded.pending_analysis_decision().is_none());
    service.repository().verify_integrity()?;
    Ok(())
}

// Break caught: the lifecycle state advanced without its receipt, because the
// row update and the event append were two transactions. A crash between them
// left a package `active` with nothing on record explaining why, and a stale
// writer silently overwrote the winner instead of losing.
#[test]
fn lifecycle_transitions_are_atomic_version_guarded_and_replay_safe(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let mut lifecycle = open_lifecycle(&store)?;
    let step = promotion(
        "1",
        BuilderLifecycleState::Draft,
        BuilderLifecycleState::Validated,
        BuilderPromotionStage::Validation,
        BuilderApprovalDecision::NotRequired,
    );
    let event = lifecycle.promote(1, step.clone())?;
    store.commit_builder_lifecycle_transition(&lifecycle, &step, &event, 2_000)?;

    // Replay of the exact accepted transition converges instead of erroring,
    // and leaves exactly one row behind it.
    store.commit_builder_lifecycle_transition(&lifecycle, &step, &event, 2_001)?;

    // A second writer holding the pre-transition snapshot must lose.
    let mut stale = BuilderLifecycle::open(
        id("lifecycle_01J00000000000000000000000"),
        id("agentdraft_01J00000000000000000000000"),
        subject_hash(),
    );
    let other = promotion(
        "9",
        BuilderLifecycleState::Draft,
        BuilderLifecycleState::Validated,
        BuilderPromotionStage::Validation,
        BuilderApprovalDecision::NotRequired,
    );
    let stale_event = stale.promote(1, other.clone())?;
    assert!(matches!(
        store.commit_builder_lifecycle_transition(&stale, &other, &stale_event, 2_002),
        Err(StoreError::Inconsistent)
    ));

    let reopened = LocalStore::open(directory.path(), &TestProtector)?;
    let restored = reopened
        .load_builder_lifecycle(&id("lifecycle_01J00000000000000000000000"))?
        .expect("lifecycle survives restart");
    assert_eq!(restored.state(), BuilderLifecycleState::Validated);
    assert_eq!(restored.version(), 2);
    assert_eq!(restored.promotions().len(), 1);

    let connection = rusqlite::Connection::open(directory.path().join("authority.sqlite3"))?;
    let events: u64 = connection.query_row(
        "SELECT COUNT(*) FROM bmad_builder_lifecycle_events WHERE lifecycle_id = ?1",
        rusqlite::params!["lifecycle_01J00000000000000000000000"],
        |row| row.get(0),
    )?;
    assert_eq!(events, 1);
    let receipts: u64 = connection.query_row(
        "SELECT COUNT(*) FROM evidence_events WHERE stream_id = ?1",
        rusqlite::params!["bmad-builder-lifecycle:lifecycle_01J00000000000000000000000"],
        |row| row.get(0),
    )?;
    // One receipt for the open, one for the accepted transition, none for the
    // replay: the outbox row rides the same transaction as each of them.
    assert_eq!(receipts, 2);
    let queued: u64 = connection.query_row("SELECT COUNT(*) FROM outbox", [], |row| row.get(0))?;
    assert!(queued >= receipts);
    Ok(())
}

// Break caught: a package was activated with no approved promotion on record,
// so `active` meant only that someone had asked. Both the write path and the
// restore path must refuse it, and the durable row must not move.
#[test]
fn no_package_reaches_active_without_an_approved_promotion(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let mut lifecycle = open_lifecycle(&store)?;
    walk_to_installed(&store, &mut lifecycle, BuilderApprovalDecision::Denied)?;
    assert_eq!(lifecycle.state(), BuilderLifecycleState::Installed);

    let activation = promotion(
        "8",
        BuilderLifecycleState::Installed,
        BuilderLifecycleState::Active,
        BuilderPromotionStage::Activation,
        BuilderApprovalDecision::NotRequired,
    );
    let version = lifecycle.version();
    assert!(lifecycle.promote(version, activation).is_err());

    let restored = store
        .load_builder_lifecycle(&id("lifecycle_01J00000000000000000000000"))?
        .expect("lifecycle");
    assert_eq!(restored.state(), BuilderLifecycleState::Installed);
    assert!(restored.approved_promotion().is_none());

    let connection = rusqlite::Connection::open(directory.path().join("authority.sqlite3"))?;
    let active_rows: u64 = connection.query_row(
        "SELECT COUNT(*) FROM bmad_builder_lifecycles WHERE state = 'active'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(active_rows, 0);
    drop(connection);

    // The same walk with a real approval does reach active, so the refusal
    // above is the approval gate and not an unrelated failure.
    let approved_directory = tempfile::tempdir()?;
    let approved_store = LocalStore::open(approved_directory.path(), &TestProtector)?;
    let mut approved = open_lifecycle(&approved_store)?;
    walk_to_installed(
        &approved_store,
        &mut approved,
        BuilderApprovalDecision::Approved,
    )?;
    let activation = promotion(
        "8",
        BuilderLifecycleState::Installed,
        BuilderLifecycleState::Active,
        BuilderPromotionStage::Activation,
        BuilderApprovalDecision::NotRequired,
    );
    let version = approved.version();
    let event = approved.promote(version, activation.clone())?;
    approved_store.commit_builder_lifecycle_transition(&approved, &activation, &event, 9_000)?;
    assert_eq!(
        approved_store
            .load_builder_lifecycle(&id("lifecycle_01J00000000000000000000000"))?
            .expect("lifecycle")
            .state(),
        BuilderLifecycleState::Active
    );
    Ok(())
}
