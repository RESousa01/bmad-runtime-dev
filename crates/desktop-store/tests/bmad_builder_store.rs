#![allow(clippy::expect_used)]

use desktop_runtime::{
    canonical_hash_without_field, AuthorityRef, BuilderAnalysisRun, BuilderAuthoringService,
    BuilderDraftRecord, BuilderDraftRepository, BuilderDraftRevision, BuilderDraftState,
    BuilderPersistenceEvent, ContractId,
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
    let analyzed = service.record_analysis(
        &scope,
        &created.record().draft_id,
        revised.version(),
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
    assert_eq!((events, outbox), (3, 3));
    Ok(())
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
fn v5_upgrade_and_interrupted_v6_migration_match_fresh_schema(
) -> Result<(), Box<dyn std::error::Error>> {
    let fresh_directory = tempfile::tempdir()?;
    let fresh = LocalStore::open(fresh_directory.path(), &TestProtector)?;
    let expected = fresh.schema_catalog()?;
    assert_eq!(fresh.schema_version()?, 6);

    for interrupted in [false, true] {
        let directory = tempfile::tempdir()?;
        let store = LocalStore::open(directory.path(), &TestProtector)?;
        let database_path = store.database_path();
        drop(store);
        let connection = rusqlite::Connection::open(&database_path)?;
        connection.execute_batch(
            "PRAGMA foreign_keys = OFF;
             DROP TABLE bmad_builder_analyses;
             DROP TABLE bmad_builder_revisions;
             DROP TABLE bmad_builder_drafts;
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
        assert_eq!(reopened.schema_version()?, 6);
        assert_eq!(reopened.schema_catalog()?, expected);
    }
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
fn v6_schema_contains_no_future_builder_lifecycle_tables() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let catalog = store
        .schema_catalog()?
        .into_iter()
        .map(|entry| format!("{} {} {} {}", entry.0, entry.1, entry.2, entry.3))
        .collect::<Vec<_>>()
        .join("\n")
        .to_ascii_lowercase();
    for forbidden in [
        "builder_activation",
        "builder_evaluation",
        "builder_promotion",
        "builder_publication",
        "builder_registration",
        "builder_rollback",
    ] {
        assert!(!catalog.contains(forbidden));
    }
    Ok(())
}
