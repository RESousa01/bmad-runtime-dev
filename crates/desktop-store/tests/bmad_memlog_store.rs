#![allow(clippy::expect_used)]

use desktop_runtime::MemlogEntryKind;
use desktop_store::{
    KeyProtector, LocalStore, MemlogAppend, MemlogAppendOutcome, StoreError,
    MAX_MEMLOG_ENTRIES_PER_DOC_WORKSPACE, MAX_MEMLOG_ENTRY_BYTES, MAX_MEMLOG_SUBJECT_BYTES,
};
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
const AT: u64 = 1_700_000_000_000;

fn append<'a>(entry_id: &'a str, doc_workspace: &'a str, text: &'a str) -> MemlogAppend<'a> {
    MemlogAppend {
        entry_id,
        workspace_id: WORKSPACE,
        doc_workspace,
        kind: MemlogEntryKind::Decision,
        author: Some("analyst"),
        subject: Some("Goals"),
        text,
        recorded_at_ms: AT,
    }
}

fn sequence_of(outcome: MemlogAppendOutcome) -> u64 {
    match outcome {
        MemlogAppendOutcome::Appended(sequence) => sequence,
        MemlogAppendOutcome::AtCapacity => {
            unreachable!("expected an append, not capacity")
        }
    }
}

#[test]
fn entries_read_back_in_append_order() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;

    for (index, text) in ["first", "second", "third"].into_iter().enumerate() {
        let entry_id = format!("e{index}");
        let outcome = store.append_memlog_entry(append(&entry_id, "docs/prd", text))?;
        assert_eq!(sequence_of(outcome), index as u64);
    }

    let entries = store.memlog_entries(WORKSPACE, "docs/prd")?;
    let texts: Vec<&str> = entries.iter().map(|entry| entry.text.as_str()).collect();
    assert_eq!(texts, vec!["first", "second", "third"]);
    let sequences: Vec<u64> = entries.iter().map(|entry| entry.sequence).collect();
    assert_eq!(sequences, vec![0, 1, 2]);
    // Kind and subject survive the round trip as typed values.
    assert!(entries
        .iter()
        .all(|entry| entry.kind == MemlogEntryKind::Decision));
    assert_eq!(entries[0].subject.as_deref(), Some("Goals"));
    Ok(())
}

#[test]
fn every_kind_round_trips_through_the_store() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;

    for (index, kind) in MemlogEntryKind::ALL.into_iter().enumerate() {
        let entry_id = format!("kind_{index}");
        store.append_memlog_entry(MemlogAppend {
            kind,
            subject: None,
            ..append(&entry_id, "docs/prd", "text")
        })?;
    }
    let stored: Vec<MemlogEntryKind> = store
        .memlog_entries(WORKSPACE, "docs/prd")?
        .into_iter()
        .map(|entry| entry.kind)
        .collect();
    assert_eq!(stored, MemlogEntryKind::ALL.to_vec());
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
        sequence_of(store.append_memlog_entry(append("e2", "docs/architecture", "arch one"))?),
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
                    kind: MemlogEntryKind::Decision,
                    author: None,
                    subject: None,
                    text: &text,
                    recorded_at_ms: AT,
                })
            })
        })
        .collect();

    let mut allocated: Vec<u64> = handles
        .into_iter()
        .map(|handle| handle.join().expect("thread panicked"))
        .collect::<Result<Vec<MemlogAppendOutcome>, StoreError>>()?
        .into_iter()
        .map(sequence_of)
        .collect();
    allocated.sort_unstable();
    assert_eq!(allocated, (0..8).collect::<Vec<u64>>());

    let entries = store.memlog_entries(WORKSPACE, "docs/prd")?;
    assert_eq!(entries.len(), 8);
    Ok(())
}

#[test]
fn out_of_bounds_entries_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;

    assert!(matches!(
        store.append_memlog_entry(append("e1", "docs/prd", "   ")),
        Err(StoreError::Inconsistent)
    ));
    let oversized = "x".repeat(MAX_MEMLOG_ENTRY_BYTES + 1);
    assert!(matches!(
        store.append_memlog_entry(append("e2", "docs/prd", &oversized)),
        Err(StoreError::Inconsistent)
    ));
    let long_subject = "x".repeat(MAX_MEMLOG_SUBJECT_BYTES + 1);
    assert!(matches!(
        store.append_memlog_entry(MemlogAppend {
            subject: Some(&long_subject),
            ..append("e3", "docs/prd", "text")
        }),
        Err(StoreError::Inconsistent)
    ));
    assert!(store.memlog_entries(WORKSPACE, "docs/prd")?.is_empty());
    Ok(())
}

/// The same rule `Memlog::append` enforces in memory: a stored log that moved
/// backwards in time could not be rebuilt into a chronology.
#[test]
fn an_entry_may_not_land_before_the_one_preceding_it() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;

    store.append_memlog_entry(MemlogAppend {
        recorded_at_ms: AT,
        ..append("e1", "docs/prd", "first")
    })?;
    assert!(matches!(
        store.append_memlog_entry(MemlogAppend {
            recorded_at_ms: AT - 1,
            ..append("e2", "docs/prd", "backwards")
        }),
        Err(StoreError::Inconsistent)
    ));
    // Equal timestamps are fine: several entries land in one reviewed run.
    store.append_memlog_entry(MemlogAppend {
        recorded_at_ms: AT,
        ..append("e3", "docs/prd", "same instant")
    })?;
    assert_eq!(store.memlog_entries(WORKSPACE, "docs/prd")?.len(), 2);
    Ok(())
}

/// A full log stops growing rather than erroring, because the caller's own
/// work is already durable by the time it records anything here.
#[test]
fn a_full_doc_workspace_reports_capacity_rather_than_failing(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;

    for index in 0..MAX_MEMLOG_ENTRIES_PER_DOC_WORKSPACE {
        let entry_id = format!("e{index}");
        store.append_memlog_entry(append(&entry_id, "docs/prd", "text"))?;
    }
    assert_eq!(
        store.append_memlog_entry(append("overflow", "docs/prd", "text"))?,
        MemlogAppendOutcome::AtCapacity
    );
    assert_eq!(
        store.memlog_entries(WORKSPACE, "docs/prd")?.len() as u64,
        MAX_MEMLOG_ENTRIES_PER_DOC_WORKSPACE
    );
    // A different doc workspace is unaffected by its neighbour being full.
    assert_eq!(
        sequence_of(store.append_memlog_entry(append("other", "docs/ux", "text"))?),
        0
    );
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
        "DROP TABLE bmad_architecture_session_steps;
         DROP TABLE payload_quarantine;
         DROP TABLE bmad_architecture_sessions;
         DROP TABLE bmad_memlog_entries;
         PRAGMA user_version = 12;",
    )?;
    drop(connection);

    let upgraded = LocalStore::open(directory.path(), &TestProtector)?;
    assert_eq!(upgraded.schema_catalog()?, expected);
    // The upgraded store is usable, not merely present.
    assert_eq!(
        sequence_of(upgraded.append_memlog_entry(append("e1", "docs/prd", "after upgrade"))?),
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
        "DROP TABLE bmad_architecture_session_steps;
         DROP TABLE payload_quarantine;
         DROP TABLE bmad_architecture_sessions;
         DROP TABLE bmad_memlog_entries;
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
    assert_eq!(reopened.schema_version()?, 17);
    assert_eq!(reopened.schema_catalog()?, expected);
    assert_eq!(
        sequence_of(reopened.append_memlog_entry(append("e1", "docs/prd", "after recovery"))?),
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
    connection.execute_batch(
        "DROP TABLE bmad_architecture_session_steps;
         DROP TABLE payload_quarantine;
         DROP TABLE bmad_architecture_sessions;
         PRAGMA user_version = 12;",
    )?;
    drop(connection);

    assert!(LocalStore::open(directory.path(), &TestProtector).is_err());
    Ok(())
}

/// A row carrying a kind outside the reviewed vocabulary means the file was
/// written by something other than this build; reading must refuse it.
#[test]
fn an_unknown_stored_kind_fails_closed() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    store.append_memlog_entry(append("e1", "docs/prd", "text"))?;
    let database_path = store.database_path();
    drop(store);

    let connection = rusqlite::Connection::open(&database_path)?;
    connection.execute_batch("UPDATE bmad_memlog_entries SET kind = 'smuggled';")?;
    drop(connection);

    let reopened = LocalStore::open(directory.path(), &TestProtector)?;
    assert!(matches!(
        reopened.memlog_entries(WORKSPACE, "docs/prd"),
        Err(StoreError::Inconsistent)
    ));
    Ok(())
}

/// The retention manifest is what `app.offboarding.inspect` shows the user.
/// A category that never moves off zero would be disclosure in name only.
#[test]
fn offboarding_inspection_counts_memlog_entries() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;

    let before = store
        .retention_manifest()?
        .into_iter()
        .find(|(label, _)| *label == "memlog_entries")
        .expect("the manifest must disclose the memlog category");
    assert_eq!(before.1, 0);

    store.append_memlog_entry(append("e1", "docs/prd", "one"))?;
    store.append_memlog_entry(append("e2", "docs/ux", "two"))?;

    let after = store
        .retention_manifest()?
        .into_iter()
        .find(|(label, _)| *label == "memlog_entries")
        .expect("memlog category");
    assert_eq!(after.1, 2);
    Ok(())
}

/// v13 shipped the memlog table without `subject`. A store that already ran
/// that migration must gain the column on open rather than be met with a query
/// against a column its file does not have — which is why v14 is a separate
/// version instead of an edit to v13.
#[test]
fn a_v13_store_gains_the_subject_column_on_open() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let database_path = store.database_path();
    let expected_columns = memlog_columns(&database_path)?;
    drop(store);

    // Rebuild the v13 table: same shape, no `subject`, one row of history to
    // prove the migration preserves what was already recorded. The DDL text is
    // written out here rather than compared against the migration's, so this
    // test turns on the column landing and not on incidental whitespace.
    let connection = rusqlite::Connection::open(&database_path)?;
    connection.execute_batch(
        "DROP TABLE bmad_architecture_session_steps;
         DROP TABLE payload_quarantine;
         DROP TABLE bmad_architecture_sessions;
         DROP TABLE bmad_memlog_entries;
         CREATE TABLE bmad_memlog_entries (
           entry_id TEXT PRIMARY KEY,
           workspace_id TEXT NOT NULL,
           doc_workspace TEXT NOT NULL,
           sequence INTEGER NOT NULL CHECK(sequence >= 0),
           kind TEXT NOT NULL,
           author TEXT,
           text TEXT NOT NULL CHECK(length(text) > 0),
           recorded_at_ms INTEGER NOT NULL
             CHECK(recorded_at_ms >= 0 AND recorded_at_ms <= 9007199254740991),
           UNIQUE(workspace_id, doc_workspace, sequence)
         ) STRICT;
         CREATE INDEX bmad_memlog_entries_by_doc_workspace
           ON bmad_memlog_entries(workspace_id, doc_workspace, sequence);
         INSERT INTO bmad_memlog_entries
           (entry_id, workspace_id, doc_workspace, sequence, kind, author, text, recorded_at_ms)
         VALUES ('legacy', 'workspace_01J77777777777777777777777', 'docs/prd', 0,
                 'decision', NULL, 'Goals: from v13', 1700000000000);
         PRAGMA user_version = 13;",
    )?;
    drop(connection);

    let upgraded = LocalStore::open(directory.path(), &TestProtector)?;
    assert_eq!(upgraded.schema_version()?, 17);
    assert_eq!(memlog_columns(&database_path)?, expected_columns);
    assert!(expected_columns.iter().any(|column| column == "subject"));

    // The pre-existing entry survives, with no subject, and reads back fine.
    let entries = upgraded.memlog_entries(WORKSPACE, "docs/prd")?;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].text, "Goals: from v13");
    assert_eq!(entries[0].subject, None);
    // And the log keeps growing from where v13 left it.
    assert_eq!(
        sequence_of(upgraded.append_memlog_entry(append("e1", "docs/prd", "after upgrade"))?),
        1
    );
    Ok(())
}

/// The memlog table's column names, in declaration order.
fn memlog_columns(
    database_path: &std::path::Path,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let connection = rusqlite::Connection::open(database_path)?;
    let mut statement =
        connection.prepare("SELECT name FROM pragma_table_info('bmad_memlog_entries')")?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(columns)
}
