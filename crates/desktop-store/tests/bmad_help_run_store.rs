#![allow(clippy::expect_used)]

use desktop_runtime::{sha256_bytes, ContractId, CreateMethodSession, MethodSession, UnixMillis};
use desktop_store::{
    BmadHelpRunCreateRequest, BmadHelpRunReplayRequest, EvidenceAppend, KeyProtector, LocalStore,
    StoreError,
};
use rusqlite::Connection;

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

fn open_store(root: &std::path::Path) -> Result<LocalStore, Box<dyn std::error::Error>> {
    let store = LocalStore::open(root, &TestProtector)?;
    if store
        .load_aggregate("workspace_catalog", "local")?
        .is_none()
    {
        store.append_transition(
            "workspace_catalog",
            "local",
            1,
            r#"{"schemaVersion":"workspace-catalog.v1","entries":[{"workspaceId":"workspace_01J00000000000000000000000","grantEpoch":7,"revoked":false}]}"#,
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
    }
    Ok(store)
}

fn candidate(
    store: &LocalStore,
    session_id: &str,
    run_id: &str,
    accepted_at: UnixMillis,
) -> Result<MethodSession, Box<dyn std::error::Error>> {
    let identity = store.local_identity()?;
    Ok(MethodSession::create(CreateMethodSession {
        session_id: id(session_id),
        owner_scope_ref: identity.owner_scope_ref().clone(),
        project_id: id("project_01J00000000000000000000000"),
        run_id: id(run_id),
        authority_ref: identity.authority_ref()?,
        created_at: accepted_at,
    })?)
}

fn request(request_id: &str, accepted_at: UnixMillis) -> BmadHelpRunCreateRequest {
    BmadHelpRunCreateRequest {
        request_id: id(request_id),
        project_id: id("project_01J00000000000000000000000"),
        workspace_id: id("workspace_01J00000000000000000000000"),
        workspace_grant_epoch: 7,
        workspace_catalog_version: 1,
        workspace_root_identity_hash: sha256_bytes(b"revalidated workspace root identity"),
        capability_catalog_hash: sha256_bytes(b"sealed capability catalog"),
        foundation_binding_hash: sha256_bytes(b"sealed foundation binding"),
        intent_hash: sha256_bytes(b"help me plan this project"),
        accepted_at,
    }
}

fn replay_request(request: &BmadHelpRunCreateRequest) -> BmadHelpRunReplayRequest {
    BmadHelpRunReplayRequest {
        request_id: request.request_id.clone(),
        workspace_id: request.workspace_id.clone(),
        workspace_grant_epoch: request.workspace_grant_epoch,
        capability_catalog_hash: request.capability_catalog_hash,
        foundation_binding_hash: request.foundation_binding_hash,
        intent_hash: request.intent_hash,
    }
}

fn authority_counts(connection: &Connection) -> Result<(u64, u64, u64, u64, u64), rusqlite::Error> {
    connection.query_row(
        "SELECT
           (SELECT COUNT(*) FROM payloads),
           (SELECT COUNT(*) FROM bmad_method_sessions),
           (SELECT COUNT(*) FROM evidence_events),
           (SELECT COUNT(*) FROM outbox),
           (SELECT COUNT(*) FROM bmad_help_run_creations)",
        [],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        },
    )
}

#[test]
fn help_run_creation_is_atomic_and_replays_original_ids_and_time(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = open_store(directory.path())?;
    let first_time = UnixMillis(1_000);
    let first = store.create_bmad_help_run(
        &candidate(
            &store,
            "session_01J00000000000000000000000",
            "run_01J00000000000000000000000",
            first_time,
        )?,
        &request("request_01J00000000000000000000000", first_time),
    )?;

    assert!(!first.replayed);
    assert_eq!(first.session_id, id("session_01J00000000000000000000000"));
    assert_eq!(first.run_id, id("run_01J00000000000000000000000"));
    assert_eq!(first.accepted_at, first_time);

    let replay_time = UnixMillis(9_000);
    let replay = store.create_bmad_help_run(
        &candidate(
            &store,
            "session_01J11111111111111111111111",
            "run_01J11111111111111111111111",
            replay_time,
        )?,
        &request("request_01J00000000000000000000000", replay_time),
    )?;
    assert!(replay.replayed);
    assert_eq!(replay.session_id, first.session_id);
    assert_eq!(replay.run_id, first.run_id);
    assert_eq!(replay.accepted_at, first_time);

    let connection = Connection::open(store.database_path())?;
    assert_eq!(authority_counts(&connection)?, (2, 1, 2, 2, 1));
    store.verify_integrity()?;
    Ok(())
}

#[test]
fn same_key_with_changed_request_fingerprint_conflicts_without_writes(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = open_store(directory.path())?;
    let accepted_at = UnixMillis(1_000);
    let request_id = "request_01J00000000000000000000000";
    store.create_bmad_help_run(
        &candidate(
            &store,
            "session_01J00000000000000000000000",
            "run_01J00000000000000000000000",
            accepted_at,
        )?,
        &request(request_id, accepted_at),
    )?;
    let connection = Connection::open(store.database_path())?;
    let before = authority_counts(&connection)?;

    let mut changed = request(request_id, UnixMillis(2_000));
    changed.intent_hash = sha256_bytes(b"a different intent");
    let result = store.create_bmad_help_run(
        &candidate(
            &store,
            "session_01J22222222222222222222222",
            "run_01J22222222222222222222222",
            UnixMillis(2_000),
        )?,
        &changed,
    );
    assert!(matches!(result, Err(StoreError::StateConflict)));
    assert_eq!(authority_counts(&connection)?, before);

    let mut changed_root = request(request_id, UnixMillis(2_000));
    changed_root.workspace_root_identity_hash = sha256_bytes(b"different workspace root identity");
    let mut changed_foundation = request(request_id, UnixMillis(2_000));
    changed_foundation.foundation_binding_hash = sha256_bytes(b"different foundation");
    for changed_binding in [changed_root, changed_foundation] {
        let result = store.create_bmad_help_run(
            &candidate(
                &store,
                "session_01J22222222222222222222222",
                "run_01J22222222222222222222222",
                UnixMillis(2_000),
            )?,
            &changed_binding,
        );
        assert!(matches!(result, Err(StoreError::StateConflict)));
        assert_eq!(authority_counts(&connection)?, before);
    }
    Ok(())
}

#[test]
fn lookup_only_replay_survives_lost_workspace_authority_and_never_creates(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = open_store(directory.path())?;
    let accepted_at = UnixMillis(1_000);
    let create = request("request_01J00000000000000000000000", accepted_at);
    let original = store.create_bmad_help_run(
        &candidate(
            &store,
            "session_01J00000000000000000000000",
            "run_01J00000000000000000000000",
            accepted_at,
        )?,
        &create,
    )?;
    let database_path = store.database_path();
    drop(store);
    let store = open_store(directory.path())?;
    let connection = Connection::open(database_path)?;
    let before = authority_counts(&connection)?;

    let replay = store
        .replay_bmad_help_run(&replay_request(&create))?
        .expect("committed request is replayable without a live workspace guard");
    assert!(replay.replayed);
    assert_eq!(replay.session_id, original.session_id);
    assert_eq!(replay.run_id, original.run_id);
    assert_eq!(replay.accepted_at, original.accepted_at);
    assert_eq!(authority_counts(&connection)?, before);

    let absent_create = request("request_01J99999999999999999999999", UnixMillis(9_000));
    assert!(store
        .replay_bmad_help_run(&replay_request(&absent_create))?
        .is_none());
    assert_eq!(authority_counts(&connection)?, before);

    let mut changed = replay_request(&create);
    changed.foundation_binding_hash = sha256_bytes(b"changed sealed foundation");
    assert!(matches!(
        store.replay_bmad_help_run(&changed),
        Err(StoreError::StateConflict)
    ));
    assert_eq!(authority_counts(&connection)?, before);
    Ok(())
}

#[test]
fn failed_receipt_insert_rolls_back_payload_registration_session_evidence_and_outbox(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = open_store(directory.path())?;
    let connection = Connection::open(store.database_path())?;
    let before = authority_counts(&connection)?;
    connection.execute_batch(
        "CREATE TRIGGER reject_help_run_receipt
         BEFORE INSERT ON bmad_help_run_creations
         BEGIN SELECT RAISE(ABORT, 'injected receipt failure'); END;",
    )?;

    let accepted_at = UnixMillis(1_000);
    assert!(store
        .create_bmad_help_run(
            &candidate(
                &store,
                "session_01J00000000000000000000000",
                "run_01J00000000000000000000000",
                accepted_at,
            )?,
            &request("request_01J00000000000000000000000", accepted_at),
        )
        .is_err());
    assert_eq!(authority_counts(&connection)?, before);
    Ok(())
}

#[test]
fn candidate_authority_must_match_the_sealed_store_identity_without_writes(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = open_store(directory.path())?;
    let identity = store.local_identity()?;
    let mut forged_authority = identity.authority_ref()?;
    forged_authority.authority_id = id("authority_01J99999999999999999999999");
    let accepted_at = UnixMillis(1_000);
    let forged = MethodSession::create(CreateMethodSession {
        session_id: id("session_01J00000000000000000000000"),
        owner_scope_ref: identity.owner_scope_ref().clone(),
        project_id: id("project_01J00000000000000000000000"),
        run_id: id("run_01J00000000000000000000000"),
        authority_ref: forged_authority,
        created_at: accepted_at,
    })?;
    let connection = Connection::open(store.database_path())?;
    let before = authority_counts(&connection)?;
    assert!(matches!(
        store.create_bmad_help_run(
            &forged,
            &request("request_01J00000000000000000000000", accepted_at)
        ),
        Err(StoreError::StateConflict)
    ));
    assert_eq!(authority_counts(&connection)?, before);
    Ok(())
}

#[test]
fn integrity_binds_receipt_to_authenticated_method_scope() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = tempfile::tempdir()?;
    let store = open_store(directory.path())?;
    let accepted_at = UnixMillis(1_000);
    store.create_bmad_help_run(
        &candidate(
            &store,
            "session_01J00000000000000000000000",
            "run_01J00000000000000000000000",
            accepted_at,
        )?,
        &request("request_01J00000000000000000000000", accepted_at),
    )?;
    store.verify_integrity()?;
    Connection::open(store.database_path())?.execute(
        "UPDATE bmad_help_run_creations SET workspace_id = ?1",
        ["workspace_01J99999999999999999999999"],
    )?;
    assert!(matches!(
        store.verify_integrity(),
        Err(StoreError::Inconsistent)
    ));
    Ok(())
}

#[test]
fn stale_workspace_catalog_cannot_create_after_another_store_commits_revocation(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let stale_store = open_store(directory.path())?;
    let revoking_store = LocalStore::open(directory.path(), &TestProtector)?;
    let revoked_catalog = r#"{"schemaVersion":"workspace-catalog.v1","entries":[{"workspaceId":"workspace_01J00000000000000000000000","grantEpoch":8,"revoked":true}]}"#;
    revoking_store.append_transition(
        "workspace_catalog",
        "local",
        2,
        revoked_catalog,
        &EvidenceAppend {
            stream_id: "workspace:catalog".to_owned(),
            event_type: "workspace.revoked".to_owned(),
            payload_hash: sha256_bytes(revoked_catalog.as_bytes()).to_string(),
            payload_ref: None,
            correlation_id: "request_workspace_revoke".to_owned(),
            causation_id: None,
            redaction_level: "metadata".to_owned(),
            retention_class: "evidence".to_owned(),
        },
    )?;

    let accepted_at = UnixMillis(1_000);
    assert!(matches!(
        stale_store.create_bmad_help_run(
            &candidate(
                &stale_store,
                "session_01J00000000000000000000000",
                "run_01J00000000000000000000000",
                accepted_at,
            )?,
            &request("request_01J00000000000000000000000", accepted_at),
        ),
        Err(StoreError::WorkspaceAuthorityStale)
    ));
    let connection = Connection::open(stale_store.database_path())?;
    assert_eq!(
        connection.query_row("SELECT COUNT(*) FROM bmad_help_run_creations", [], |row| {
            row.get::<_, u64>(0)
        })?,
        0
    );
    Ok(())
}

#[test]
fn integrity_rejects_help_receipt_rekey_or_one_sided_deletion(
) -> Result<(), Box<dyn std::error::Error>> {
    for tamper in ["rekey_receipt", "delete_receipt", "delete_event"] {
        let directory = tempfile::tempdir()?;
        let store = open_store(directory.path())?;
        let accepted_at = UnixMillis(1_000);
        store.create_bmad_help_run(
            &candidate(
                &store,
                "session_01J00000000000000000000000",
                "run_01J00000000000000000000000",
                accepted_at,
            )?,
            &request("request_01J00000000000000000000000", accepted_at),
        )?;
        let connection = Connection::open(store.database_path())?;
        match tamper {
            "rekey_receipt" => {
                connection.execute(
                    "UPDATE bmad_help_run_creations SET request_id = ?1",
                    ["request_01J99999999999999999999999"],
                )?;
            }
            "delete_receipt" => {
                connection.execute("DELETE FROM bmad_help_run_creations", [])?;
            }
            "delete_event" => {
                connection.execute(
                    "DELETE FROM outbox WHERE event_id IN
                     (SELECT event_id FROM evidence_events
                       WHERE event_type = 'bmad.help.run.created')",
                    [],
                )?;
                connection.execute(
                    "DELETE FROM evidence_events WHERE event_type = 'bmad.help.run.created'",
                    [],
                )?;
            }
            _ => unreachable!(),
        }
        assert!(matches!(
            store.verify_integrity(),
            Err(StoreError::Inconsistent)
        ));
    }
    Ok(())
}

#[test]
fn fresh_v7_upgrade_and_interrupted_v8_migration_are_equivalent(
) -> Result<(), Box<dyn std::error::Error>> {
    let fresh_directory = tempfile::tempdir()?;
    let fresh = LocalStore::open(fresh_directory.path(), &TestProtector)?;
    assert_eq!(fresh.schema_version()?, 8);
    let expected = fresh.schema_catalog()?;

    let upgrade_directory = tempfile::tempdir()?;
    let upgrade = LocalStore::open(upgrade_directory.path(), &TestProtector)?;
    let upgrade_path = upgrade.database_path();
    drop(upgrade);
    let connection = Connection::open(&upgrade_path)?;
    connection.execute_batch(
        "DROP TABLE bmad_help_run_creations;
         PRAGMA user_version = 7;",
    )?;
    drop(connection);
    let upgraded = LocalStore::open(upgrade_directory.path(), &TestProtector)?;
    assert_eq!(upgraded.schema_version()?, 8);
    assert_eq!(upgraded.schema_catalog()?, expected);

    let interrupted_directory = tempfile::tempdir()?;
    let interrupted = LocalStore::open(interrupted_directory.path(), &TestProtector)?;
    let interrupted_path = interrupted.database_path();
    drop(interrupted);
    let connection = Connection::open(&interrupted_path)?;
    connection.execute_batch(
        "DROP TABLE bmad_help_run_creations;
         PRAGMA user_version = 7;
         BEGIN IMMEDIATE;
         CREATE TABLE bmad_help_run_creations (interrupted TEXT) STRICT;",
    )?;
    drop(connection);
    let resumed = LocalStore::open(interrupted_directory.path(), &TestProtector)?;
    assert_eq!(resumed.schema_version()?, 8);
    assert_eq!(resumed.schema_catalog()?, expected);
    Ok(())
}
