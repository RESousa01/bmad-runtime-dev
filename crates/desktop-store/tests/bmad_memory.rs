#![allow(clippy::expect_used)]

use desktop_runtime::generated_contracts::{
    BmadMemoryMemoryEntry, BmadMemoryMemoryEntryMemoryClass, BmadMemoryMemoryEntryMutability,
    BmadMemoryMemoryEntryRetention, BmadMemoryMemoryEntrySensitivity, BmadMemoryMemoryProvenance,
    BmadMemoryMemoryProvenanceOrigin, BmadMemoryMemoryScope, BmadMemoryMemoryScopeKind,
    CommonContractId, CommonSha256,
};
use desktop_store::{BmadMemoryScopeQuery, KeyProtector, LocalStore, StoreError};

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

const WORKSPACE: &str = "ws_01J77777777777777777777777";
const OTHER_WORKSPACE: &str = "ws_01J77777777777777777777778";
const AT: u64 = 1_700_000_000_000;

fn id(text: &str) -> CommonContractId {
    CommonContractId::try_from(text).expect("valid contract id in test fixture")
}

fn hash(byte: u8) -> CommonSha256 {
    let digest = desktop_runtime::sha256_bytes(&[byte]);
    CommonSha256::try_from(digest.to_string()).expect("valid sha256 in test fixture")
}

fn session_scope(session: &str) -> BmadMemoryMemoryScope {
    BmadMemoryMemoryScope {
        installed_agent_id: None,
        kind: BmadMemoryMemoryScopeKind::Session,
        session_id: Some(id(session)),
        workspace_id: None,
    }
}

fn installed_agent_scope(agent: &str) -> BmadMemoryMemoryScope {
    BmadMemoryMemoryScope {
        installed_agent_id: Some(id(agent)),
        kind: BmadMemoryMemoryScopeKind::InstalledAgent,
        session_id: None,
        workspace_id: None,
    }
}

fn fact(
    entry_id: &str,
    byte: u8,
    origin: BmadMemoryMemoryProvenanceOrigin,
    mutability: BmadMemoryMemoryEntryMutability,
    retention: BmadMemoryMemoryEntryRetention,
    sequence: i64,
    scope: BmadMemoryMemoryScope,
) -> BmadMemoryMemoryEntry {
    BmadMemoryMemoryEntry {
        content_hash: hash(byte),
        entry_id: id(entry_id),
        memory_class: BmadMemoryMemoryEntryMemoryClass::SessionFact,
        mutability,
        provenance: BmadMemoryMemoryProvenance {
            origin,
            recorded_at_sequence: sequence,
            source_entry_ids: Vec::new(),
            source_session_id: None,
        },
        retention,
        scope,
        sensitivity: BmadMemoryMemoryEntrySensitivity::Ordinary,
        superseded_by_entry_id: None,
    }
}

fn memlog(
    entry_id: &str,
    byte: u8,
    sequence: i64,
    scope: BmadMemoryMemoryScope,
) -> BmadMemoryMemoryEntry {
    BmadMemoryMemoryEntry {
        content_hash: hash(byte),
        entry_id: id(entry_id),
        memory_class: BmadMemoryMemoryEntryMemoryClass::MemlogEntry,
        mutability: BmadMemoryMemoryEntryMutability::AppendOnly,
        provenance: BmadMemoryMemoryProvenance {
            origin: BmadMemoryMemoryProvenanceOrigin::HostObserved,
            recorded_at_sequence: sequence,
            source_entry_ids: Vec::new(),
            source_session_id: None,
        },
        retention: BmadMemoryMemoryEntryRetention::Bounded,
        scope,
        sensitivity: BmadMemoryMemoryEntrySensitivity::Ordinary,
        superseded_by_entry_id: None,
    }
}

fn summary(
    entry_id: &str,
    byte: u8,
    sources: &[BmadMemoryMemoryEntry],
    scope: BmadMemoryMemoryScope,
) -> BmadMemoryMemoryEntry {
    BmadMemoryMemoryEntry {
        content_hash: hash(byte),
        entry_id: id(entry_id),
        memory_class: BmadMemoryMemoryEntryMemoryClass::CompactionSummary,
        mutability: BmadMemoryMemoryEntryMutability::Replaceable,
        provenance: BmadMemoryMemoryProvenance {
            origin: BmadMemoryMemoryProvenanceOrigin::ModelSummarized,
            recorded_at_sequence: sources
                .iter()
                .map(|source| source.provenance.recorded_at_sequence)
                .max()
                .unwrap_or(0),
            source_entry_ids: sources
                .iter()
                .map(|source| source.entry_id.clone())
                .collect(),
            source_session_id: None,
        },
        retention: BmadMemoryMemoryEntryRetention::Bounded,
        scope,
        sensitivity: BmadMemoryMemoryEntrySensitivity::Ordinary,
        superseded_by_entry_id: None,
    }
}

// Break caught: appending the same entry_id twice must fail closed rather
// than silently overwriting an existing row — that is what "append-only"
// means at the storage layer.
#[test]
fn append_rejects_a_duplicate_entry_id() -> Result<(), StoreError> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let entry = memlog(
        "entry_01J777777777777777777AAA",
        1,
        1,
        session_scope("sess_01J777777777777777777AAA"),
    );
    store.append_bmad_memory_entry(WORKSPACE, &entry, AT)?;
    let result = store.append_bmad_memory_entry(WORKSPACE, &entry, AT + 1);
    assert!(matches!(result, Err(StoreError::AlreadyConsumed)));
    Ok(())
}

// Break caught: restart recovery must reproduce every entry exactly,
// including which rows a compaction already retired.
#[test]
fn restart_recovers_every_entry_with_full_fidelity() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let scope = session_scope("sess_01J777777777777777777AAA");
    {
        let store = LocalStore::open(directory.path(), &TestProtector)?;
        let fact_one = fact(
            "entry_01J777777777777777777AA1",
            1,
            BmadMemoryMemoryProvenanceOrigin::UserSupplied,
            BmadMemoryMemoryEntryMutability::Replaceable,
            BmadMemoryMemoryEntryRetention::Bounded,
            1,
            scope.clone(),
        );
        let replaceable = fact(
            "entry_01J777777777777777777AA2",
            2,
            BmadMemoryMemoryProvenanceOrigin::ModelSummarized,
            BmadMemoryMemoryEntryMutability::Replaceable,
            BmadMemoryMemoryEntryRetention::Bounded,
            2,
            scope.clone(),
        );
        store.append_bmad_memory_entry(WORKSPACE, &fact_one, AT)?;
        store.append_bmad_memory_entry(WORKSPACE, &replaceable, AT)?;

        let mut retired = replaceable.clone();
        retired.superseded_by_entry_id = Some(id("entry_01J777777777777777777AA3"));
        let summary_entry = summary(
            "entry_01J777777777777777777AA3",
            3,
            &[replaceable],
            scope.clone(),
        );
        store.commit_bmad_memory_compaction(WORKSPACE, &[retired], &summary_entry, AT + 1)?;
    }

    let reopened = LocalStore::open(directory.path(), &TestProtector)?;
    let entries = reopened.bmad_memory_entries_for_workspace(WORKSPACE)?;
    assert_eq!(entries.len(), 3);
    let retired_row = entries
        .iter()
        .find(|stored| stored.entry.entry_id == id("entry_01J777777777777777777AA2"))
        .expect("retired source recovered");
    assert_eq!(
        retired_row.entry.superseded_by_entry_id,
        Some(id("entry_01J777777777777777777AA3"))
    );
    Ok(())
}

// Break caught: retention purge must remove `session` rows but leave
// `pinned` rows in the same workspace untouched.
#[test]
fn retention_purge_removes_session_rows_and_spares_pinned_rows(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let scope = session_scope("sess_01J777777777777777777AAA");
    let session_row = fact(
        "entry_01J777777777777777777BB1",
        1,
        BmadMemoryMemoryProvenanceOrigin::UserSupplied,
        BmadMemoryMemoryEntryMutability::Replaceable,
        BmadMemoryMemoryEntryRetention::Session,
        1,
        scope.clone(),
    );
    let pinned_row = fact(
        "entry_01J777777777777777777BB2",
        2,
        BmadMemoryMemoryProvenanceOrigin::UserSupplied,
        BmadMemoryMemoryEntryMutability::Replaceable,
        BmadMemoryMemoryEntryRetention::Pinned,
        2,
        scope,
    );
    store.append_bmad_memory_entry(WORKSPACE, &session_row, AT)?;
    store.append_bmad_memory_entry(WORKSPACE, &pinned_row, AT)?;

    let purged =
        store.purge_bmad_memory_retention(WORKSPACE, BmadMemoryMemoryEntryRetention::Session)?;
    assert_eq!(purged, 1);

    let remaining = store.bmad_memory_entries_for_workspace(WORKSPACE)?;
    assert_eq!(remaining.len(), 1);
    assert_eq!(
        remaining[0].entry.entry_id,
        id("entry_01J777777777777777777BB2")
    );
    Ok(())
}

// Break caught: deleting a whole scope (the export/erasure path) must leave
// nothing behind for that scope, while another scope in the same workspace
// is unaffected.
#[test]
fn delete_scope_is_complete_and_isolated() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let agent_a = installed_agent_scope("agent_01J777777777777777777AAA");
    let agent_b = installed_agent_scope("agent_01J777777777777777777BBB");
    let entry_a = fact(
        "entry_01J777777777777777777CC1",
        1,
        BmadMemoryMemoryProvenanceOrigin::UserSupplied,
        BmadMemoryMemoryEntryMutability::Replaceable,
        BmadMemoryMemoryEntryRetention::Bounded,
        1,
        agent_a.clone(),
    );
    let entry_b = fact(
        "entry_01J777777777777777777CC2",
        2,
        BmadMemoryMemoryProvenanceOrigin::UserSupplied,
        BmadMemoryMemoryEntryMutability::Replaceable,
        BmadMemoryMemoryEntryRetention::Bounded,
        2,
        agent_b,
    );
    store.append_bmad_memory_entry(WORKSPACE, &entry_a, AT)?;
    store.append_bmad_memory_entry(WORKSPACE, &entry_b, AT)?;

    let removed = store.delete_bmad_memory_scope(
        WORKSPACE,
        BmadMemoryScopeQuery {
            kind: BmadMemoryMemoryScopeKind::InstalledAgent,
            identifier: Some("agent_01J777777777777777777AAA"),
        },
    )?;
    assert_eq!(removed, 1);

    let remaining = store.bmad_memory_entries_for_workspace(WORKSPACE)?;
    assert_eq!(remaining.len(), 1);
    assert_eq!(
        remaining[0].entry.entry_id,
        id("entry_01J777777777777777777CC2")
    );
    Ok(())
}

// Break caught: an entry scoped to one workspace must never surface when
// another workspace queries the same scope kind — the physical partition is
// the isolation boundary, not just the scope's own identifier.
#[test]
fn scope_query_isolates_by_workspace() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let scope = session_scope("sess_01J777777777777777777AAA");
    let entry = fact(
        "entry_01J777777777777777777DD1",
        1,
        BmadMemoryMemoryProvenanceOrigin::UserSupplied,
        BmadMemoryMemoryEntryMutability::Replaceable,
        BmadMemoryMemoryEntryRetention::Bounded,
        1,
        scope,
    );
    store.append_bmad_memory_entry(WORKSPACE, &entry, AT)?;

    let other_workspace_view = store.bmad_memory_entries_for_scope(
        OTHER_WORKSPACE,
        BmadMemoryScopeQuery {
            kind: BmadMemoryMemoryScopeKind::Session,
            identifier: Some("sess_01J777777777777777777AAA"),
        },
    )?;
    assert!(other_workspace_view.is_empty());

    let same_workspace_view = store.bmad_memory_entries_for_scope(
        WORKSPACE,
        BmadMemoryScopeQuery {
            kind: BmadMemoryMemoryScopeKind::Session,
            identifier: Some("sess_01J777777777777777777AAA"),
        },
    )?;
    assert_eq!(same_workspace_view.len(), 1);
    Ok(())
}

// Break caught: a row whose JSON content_hash disagrees with the indexed
// column it was written under must fail closed with `Inconsistent`, not
// silently return whatever the (wrong) JSON says.
#[test]
fn corrupted_content_hash_index_fails_closed() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let scope = session_scope("sess_01J777777777777777777AAA");
    let entry = fact(
        "entry_01J777777777777777777EE1",
        1,
        BmadMemoryMemoryProvenanceOrigin::UserSupplied,
        BmadMemoryMemoryEntryMutability::Replaceable,
        BmadMemoryMemoryEntryRetention::Bounded,
        1,
        scope,
    );
    store.append_bmad_memory_entry(WORKSPACE, &entry, AT)?;

    let database_path = store.database_path();
    let tampered_hash = hash(9).to_string();
    rusqlite::Connection::open(&database_path)?.execute(
        "UPDATE bmad_memory_entries SET content_hash = ?1 WHERE entry_id = ?2",
        rusqlite::params![tampered_hash, "entry_01J777777777777777777EE1"],
    )?;

    let result = store.bmad_memory_entries_for_workspace(WORKSPACE);
    assert!(matches!(result, Err(StoreError::Inconsistent)));
    Ok(())
}
