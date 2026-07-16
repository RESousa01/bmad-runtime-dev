#![allow(clippy::expect_used)]

use desktop_runtime::{
    ContractId, CreateMethodSession, MethodSession, MethodSessionRepository, UnixMillis,
};
use desktop_store::EvidenceAppend;
use desktop_store::{KeyProtector, LocalStore, StoreError};
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

const IDENTITY_META_KEY: &str = "desktop_local_identity_ref";
const IDENTITY_STATE_KEY: &str = "desktop_local_identity_state";
const IDENTITY_KIND: &str = "desktop-local-identity";
const IDENTITY_SCHEMA: &str = "sapphirus.desktop-local-identity.v1";

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

#[test]
fn local_identity_is_canonical_and_stable_across_reopen() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let first = store.local_identity()?;
    let authority = first.authority_ref()?;

    assert!(first
        .installation_id()
        .as_str()
        .starts_with("installation_"));
    assert!(first.authority_id().as_str().starts_with("authority_"));
    assert!(first.owner_scope_ref().as_str().starts_with("owner_scope_"));
    assert_eq!(first.local_store_id().as_str(), store.store_id());
    assert_eq!(first.authority_epoch(), 1);
    assert_eq!(authority.authority_kind, "desktop_local_store");
    drop(store);

    let reopened = LocalStore::open(directory.path(), &TestProtector)?;
    assert_eq!(reopened.local_identity()?, first);
    reopened.verify_integrity()?;
    Ok(())
}

#[test]
fn store_id_only_history_initializes_identity_once() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let first_identity = store.local_identity()?;
    let database_path = store.database_path();
    let pointer = identity_pointer(&database_path)?;
    let identity_path = identity_cas_path(directory.path(), &pointer)?;
    drop(store);

    let connection = Connection::open(&database_path)?;
    connection.execute(
        "DELETE FROM store_meta WHERE key = ?1",
        params![IDENTITY_META_KEY],
    )?;
    connection.execute(
        "DELETE FROM store_meta WHERE key = ?1",
        params![IDENTITY_STATE_KEY],
    )?;
    connection.execute(
        "DELETE FROM payloads WHERE kind = ?1 AND schema_version = ?2",
        params![IDENTITY_KIND, IDENTITY_SCHEMA],
    )?;
    drop(connection);
    fs::remove_file(identity_path)?;

    let upgraded = LocalStore::open(directory.path(), &TestProtector)?;
    let initialized = upgraded.local_identity()?;
    assert_ne!(initialized, first_identity);
    assert_eq!(initialized.local_store_id().as_str(), upgraded.store_id());
    drop(upgraded);

    let reopened = LocalStore::open(directory.path(), &TestProtector)?;
    assert_eq!(reopened.local_identity()?, initialized);
    Ok(())
}

#[test]
fn retained_sealed_marker_prevents_replacement_after_pointer_and_payload_deletion(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let database_path = store.database_path();
    let pointer = identity_pointer(&database_path)?;
    let identity_path = identity_cas_path(directory.path(), &pointer)?;
    drop(store);

    let connection = Connection::open(&database_path)?;
    connection.execute(
        "DELETE FROM store_meta WHERE key = ?1",
        params![IDENTITY_META_KEY],
    )?;
    connection.execute(
        "DELETE FROM payloads WHERE kind = ?1 AND schema_version = ?2",
        params![IDENTITY_KIND, IDENTITY_SCHEMA],
    )?;
    drop(connection);
    fs::remove_file(identity_path)?;

    assert!(matches!(
        LocalStore::open(directory.path(), &TestProtector),
        Err(StoreError::Inconsistent)
    ));
    assert!(matches!(
        LocalStore::open_read_only_recovery(directory.path(), &TestProtector),
        Err(StoreError::Inconsistent)
    ));
    Ok(())
}

#[test]
fn missing_store_id_does_not_fabricate_a_replacement_before_identity_rejection(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let database_path = store.database_path();
    drop(store);

    Connection::open(&database_path)?
        .execute("DELETE FROM store_meta WHERE key = 'store_id'", [])?;
    assert!(matches!(
        LocalStore::open(directory.path(), &TestProtector),
        Err(StoreError::Inconsistent)
    ));
    let retained_store_ids: u64 = Connection::open(&database_path)?.query_row(
        "SELECT COUNT(*) FROM store_meta WHERE key = 'store_id'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(retained_store_ids, 0);
    Ok(())
}

#[test]
fn missing_store_id_with_d1_history_cannot_masquerade_as_an_empty_store(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    store.append_transition(
        "workspace",
        "workspace_01J00000000000000000000000",
        1,
        "{}",
        &EvidenceAppend {
            stream_id: "workspace:01J00000000000000000000000".to_owned(),
            event_type: "workspace.selected".to_owned(),
            payload_hash: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_owned(),
            payload_ref: None,
            correlation_id: "correlation_01J00000000000000000000000".to_owned(),
            causation_id: None,
            redaction_level: "summary".to_owned(),
            retention_class: "evidence".to_owned(),
        },
    )?;
    let database_path = store.database_path();
    let pointer = identity_pointer(&database_path)?;
    let identity_path = identity_cas_path(directory.path(), &pointer)?;
    drop(store);

    let connection = Connection::open(&database_path)?;
    connection.execute(
        "DELETE FROM store_meta WHERE key IN ('store_id', ?1, ?2)",
        params![IDENTITY_META_KEY, IDENTITY_STATE_KEY],
    )?;
    connection.execute(
        "DELETE FROM payloads WHERE kind = ?1 AND schema_version = ?2",
        params![IDENTITY_KIND, IDENTITY_SCHEMA],
    )?;
    drop(connection);
    fs::remove_file(identity_path)?;

    assert!(matches!(
        LocalStore::open(directory.path(), &TestProtector),
        Err(StoreError::Inconsistent)
    ));
    let replacement_store_ids: u64 = Connection::open(&database_path)?.query_row(
        "SELECT COUNT(*) FROM store_meta WHERE key = 'store_id'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(replacement_store_ids, 0);
    Ok(())
}

#[test]
fn retained_method_history_prevents_identity_deletion_from_masquerading_as_legacy(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let identity = store.local_identity()?;
    let session = MethodSession::create(CreateMethodSession {
        session_id: ContractId::new("session_01J00000000000000000000000")?,
        owner_scope_ref: identity.owner_scope_ref().clone(),
        project_id: ContractId::new("project_01J00000000000000000000000")?,
        run_id: ContractId::new("run_01J00000000000000000000000")?,
        authority_ref: identity.authority_ref()?,
        created_at: UnixMillis(1_000),
    })?;
    store.create_method_session(&session)?;
    let database_path = store.database_path();
    let pointer = identity_pointer(&database_path)?;
    let identity_path = identity_cas_path(directory.path(), &pointer)?;
    drop(store);

    let connection = Connection::open(&database_path)?;
    connection.execute(
        "DELETE FROM store_meta WHERE key IN (?1, ?2)",
        params![IDENTITY_META_KEY, IDENTITY_STATE_KEY],
    )?;
    connection.execute(
        "DELETE FROM payloads WHERE kind = ?1 AND schema_version = ?2",
        params![IDENTITY_KIND, IDENTITY_SCHEMA],
    )?;
    drop(connection);
    fs::remove_file(identity_path)?;

    assert!(matches!(
        LocalStore::open(directory.path(), &TestProtector),
        Err(StoreError::Inconsistent)
    ));
    let replacement_traces: u64 = Connection::open(&database_path)?.query_row(
        "SELECT
           (SELECT COUNT(*) FROM store_meta WHERE key IN (?1, ?2)) +
           (SELECT COUNT(*) FROM payloads WHERE kind = ?3 AND schema_version = ?4)",
        params![
            IDENTITY_META_KEY,
            IDENTITY_STATE_KEY,
            IDENTITY_KIND,
            IDENTITY_SCHEMA
        ],
        |row| row.get(0),
    )?;
    assert_eq!(replacement_traces, 0);
    Ok(())
}

#[test]
fn missing_identity_pointer_with_retained_record_fails_closed(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let database_path = store.database_path();
    drop(store);

    Connection::open(&database_path)?.execute(
        "DELETE FROM store_meta WHERE key = ?1",
        params![IDENTITY_META_KEY],
    )?;

    assert!(matches!(
        LocalStore::open(directory.path(), &TestProtector),
        Err(StoreError::Inconsistent)
    ));
    assert!(matches!(
        LocalStore::open_read_only_recovery(directory.path(), &TestProtector),
        Err(StoreError::Inconsistent)
    ));
    Ok(())
}

#[test]
fn altered_identity_pointer_fails_closed_without_replacement(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let database_path = store.database_path();
    drop(store);

    Connection::open(&database_path)?.execute(
        "UPDATE store_meta SET value = ?1 WHERE key = ?2",
        params![
            "cas://sha256/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            IDENTITY_META_KEY
        ],
    )?;

    assert!(matches!(
        LocalStore::open(directory.path(), &TestProtector),
        Err(StoreError::Inconsistent)
    ));
    assert!(matches!(
        LocalStore::open_read_only_recovery(directory.path(), &TestProtector),
        Err(StoreError::Inconsistent)
    ));
    Ok(())
}

#[test]
fn tampered_identity_ciphertext_fails_authentication_in_normal_and_recovery_open(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let pointer = identity_pointer(&store.database_path())?;
    let identity_path = identity_cas_path(directory.path(), &pointer)?;
    drop(store);

    let mut bytes = fs::read(&identity_path)?;
    let last = bytes.last_mut().expect("identity ciphertext is non-empty");
    *last ^= 0x01;
    fs::write(identity_path, bytes)?;

    assert!(matches!(
        LocalStore::open(directory.path(), &TestProtector),
        Err(StoreError::Authentication)
    ));
    assert!(matches!(
        LocalStore::open_read_only_recovery(directory.path(), &TestProtector),
        Err(StoreError::Authentication)
    ));
    Ok(())
}

fn identity_pointer(database_path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    Ok(Connection::open(database_path)?.query_row(
        "SELECT value FROM store_meta WHERE key = ?1",
        params![IDENTITY_META_KEY],
        |row| row.get(0),
    )?)
}

fn identity_cas_path(root: &Path, pointer: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let digest = pointer
        .strip_prefix("cas://sha256/")
        .ok_or("identity pointer is not a CAS reference")?;
    let content_hash = format!("sha256:{digest}");
    let storage_preimage =
        format!("sapphirus:cas-storage:1\n{IDENTITY_KIND}\n{IDENTITY_SCHEMA}\n{content_hash}");
    let storage_digest = hex::encode(Sha256::digest(storage_preimage.as_bytes()));
    Ok(root
        .join("cas")
        .join(&storage_digest[..2])
        .join(storage_digest))
}
