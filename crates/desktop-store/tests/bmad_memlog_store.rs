#![allow(clippy::expect_used)]

use desktop_store::{KeyProtector, LocalStore, MemlogAppend, StoreError};
use std::sync::{Arc, Barrier};

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

const WORKSPACE: &str = "workspace_01J77777777777777777777777";

fn append<'a>(entry_id: &'a str, doc_workspace: &'a str, text: &'a str) -> MemlogAppend<'a> {
    MemlogAppend {
        entry_id,
        workspace_id: WORKSPACE,
        doc_workspace,
        kind: "decision",
        author: Some("analyst"),
        text,
        recorded_at_ms: 1_700_000_000_000,
    }
}

#[test]
fn entries_read_back_in_append_order() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;

    assert_eq!(
        store.append_memlog_entry(append("e1", "docs/prd", "first"))?,
        0
    );
    assert_eq!(
        store.append_memlog_entry(append("e2", "docs/prd", "second"))?,
        1
    );
    assert_eq!(
        store.append_memlog_entry(append("e3", "docs/prd", "third"))?,
        2
    );

    let entries = store.memlog_entries(WORKSPACE, "docs/prd")?;
    let texts: Vec<&str> = entries.iter().map(|entry| entry.text.as_str()).collect();
    assert_eq!(texts, vec!["first", "second", "third"]);
    let sequences: Vec<u64> = entries.iter().map(|entry| entry.sequence).collect();
    assert_eq!(sequences, vec![0, 1, 2]);
    Ok(())
}

#[test]
fn doc_workspaces_keep_independent_sequences() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;

    store.append_memlog_entry(append("e1", "docs/prd", "prd one"))?;
    // A different run folder starts its own log rather than continuing the
    // first one; continuity is per doc workspace, not per workspace.
    assert_eq!(
        store.append_memlog_entry(append("e2", "docs/architecture", "arch one"))?,
        0
    );
    store.append_memlog_entry(append("e3", "docs/prd", "prd two"))?;

    assert_eq!(store.memlog_entries(WORKSPACE, "docs/prd")?.len(), 2);
    assert_eq!(
        store.memlog_entries(WORKSPACE, "docs/architecture")?.len(),
        1
    );
    assert!(store
        .memlog_entries("workspace_other", "docs/prd")?
        .is_empty());
    Ok(())
}

#[test]
fn concurrent_appends_allocate_distinct_sequences() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = Arc::new(LocalStore::open(directory.path(), &TestProtector)?);
    let barrier = Arc::new(Barrier::new(8));

    let handles: Vec<_> = (0..8)
        .map(|index| {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                let entry_id = format!("entry_{index}");
                let text = format!("line {index}");
                store.append_memlog_entry(MemlogAppend {
                    entry_id: &entry_id,
                    workspace_id: WORKSPACE,
                    doc_workspace: "docs/prd",
                    kind: "decision",
                    author: None,
                    text: &text,
                    recorded_at_ms: 1_700_000_000_000,
                })
            })
        })
        .collect();

    let mut allocated: Vec<u64> = handles
        .into_iter()
        .map(|handle| handle.join().expect("thread panicked"))
        .collect::<Result<Vec<u64>, StoreError>>()?;
    allocated.sort_unstable();
    assert_eq!(allocated, (0..8).collect::<Vec<u64>>());

    let entries = store.memlog_entries(WORKSPACE, "docs/prd")?;
    assert_eq!(entries.len(), 8);
    Ok(())
}

#[test]
fn empty_and_oversized_entries_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;

    assert!(matches!(
        store.append_memlog_entry(append("e1", "docs/prd", "   ")),
        Err(StoreError::Inconsistent)
    ));
    let oversized = "x".repeat(4_097);
    assert!(matches!(
        store.append_memlog_entry(append("e2", "docs/prd", &oversized)),
        Err(StoreError::Inconsistent)
    ));
    assert!(matches!(
        store.append_memlog_entry(MemlogAppend {
            kind: "",
            ..append("e3", "docs/prd", "text")
        }),
        Err(StoreError::Inconsistent)
    ));
    assert!(store.memlog_entries(WORKSPACE, "docs/prd")?.is_empty());
    Ok(())
}

#[test]
fn a_v12_store_gains_the_memlog_table_on_open() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let expected = store.schema_catalog()?;
    let database_path = store.database_path();
    drop(store);

    // Rewind a fresh store to v12 by removing only what v13 introduced.
    let connection = rusqlite::Connection::open(&database_path)?;
    connection.execute_batch(
        "DROP TABLE bmad_memlog_entries;
         PRAGMA user_version = 12;",
    )?;
    drop(connection);

    let upgraded = LocalStore::open(directory.path(), &TestProtector)?;
    assert_eq!(upgraded.schema_catalog()?, expected);
    // The upgraded store is usable, not merely present.
    assert_eq!(
        upgraded.append_memlog_entry(append("e1", "docs/prd", "after upgrade"))?,
        0
    );
    Ok(())
}

#[test]
fn an_interrupted_v13_upgrade_recovers_on_reopen() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let expected = store.schema_catalog()?;
    let database_path = store.database_path();
    drop(store);

    let connection = rusqlite::Connection::open(&database_path)?;
    connection.execute_batch(
        "DROP TABLE bmad_memlog_entries;
         PRAGMA user_version = 12;",
    )?;
    // A crash mid-migration: the batch opens its transaction and never
    // commits, so dropping the connection rolls the table creation back. The
    // reopen has to redo the whole step, not resume a half-applied one.
    connection.execute_batch(
        "BEGIN IMMEDIATE;
         CREATE TABLE bmad_memlog_entries (entry_id TEXT PRIMARY KEY) STRICT;",
    )?;
    drop(connection);

    let reopened = LocalStore::open(directory.path(), &TestProtector)?;
    assert_eq!(reopened.schema_version()?, 13);
    assert_eq!(reopened.schema_catalog()?, expected);
    assert_eq!(
        reopened.append_memlog_entry(append("e1", "docs/prd", "after recovery"))?,
        0
    );
    Ok(())
}

#[test]
fn a_half_applied_v13_schema_fails_closed() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let database_path = store.database_path();
    drop(store);

    // The table present while user_version still reads 12 is a state the
    // transactional migration cannot produce, so it means the file was edited
    // underneath us. Opening must refuse rather than migrate over it.
    let connection = rusqlite::Connection::open(&database_path)?;
    connection.execute_batch("PRAGMA user_version = 12;")?;
    drop(connection);

    assert!(LocalStore::open(directory.path(), &TestProtector).is_err());
    Ok(())
}
