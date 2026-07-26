//! Durable storage for the method's working memory (P14).
//!
//! Upstream's memlog is "the dense, chronological record of everything that
//! mattered in a piece of work", and it "persists ACROSS sessions, so a fresh
//! session can load it and continue". Downstream artifacts — a brief, a PRD, a
//! spine — are *derived* from it rather than being its source.
//!
//! Two invariants come straight from upstream and are enforced here rather
//! than by convention:
//!
//! * **Append-only.** There is no update and no delete. Upstream: "There is no
//!   edit or delete subcommand by design; history is never rewritten."
//! * **Chronological within a doc workspace.** `sequence` is allocated inside
//!   the appending transaction, so concurrent appends cannot interleave into
//!   the same slot or leave a gap.

use rusqlite::{params, TransactionBehavior};

use super::{LocalStore, StoreError};

/// One entry as stored. `sequence` is assigned by the store, never by a caller.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredMemlogEntry {
    pub sequence: u64,
    pub kind: String,
    pub author: Option<String>,
    pub text: String,
    pub recorded_at_ms: u64,
}

/// What a caller supplies to append one entry.
#[derive(Clone, Copy, Debug)]
pub struct MemlogAppend<'a> {
    pub entry_id: &'a str,
    pub workspace_id: &'a str,
    /// The run folder the log belongs to, e.g. `docs/architecture`. Several
    /// runs over time append to the same one; that is where continuity lives.
    pub doc_workspace: &'a str,
    pub kind: &'a str,
    pub author: Option<&'a str>,
    pub text: &'a str,
    pub recorded_at_ms: u64,
}

/// Per-doc-workspace ceiling. A log is working memory, not an archive: an
/// unbounded one would grow past anything that could be replayed into a
/// reviewed context and would make the store a silent sink for model prose.
const MAX_ENTRIES_PER_DOC_WORKSPACE: u64 = 2_048;
/// Upstream keeps every entry to "one line each, the gist in the user's
/// meaning". This bounds a single entry well above that without inviting an
/// artifact to be pasted in whole.
const MAX_ENTRY_BYTES: usize = 4_096;

impl LocalStore {
    /// Appends one entry, allocating its sequence inside the transaction.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Inconsistent`] when the entry is empty, over the
    /// per-entry bound, or the doc workspace is already at its ceiling, and
    /// [`StoreError`] for any database failure.
    pub fn append_memlog_entry(&self, append: MemlogAppend<'_>) -> Result<u64, StoreError> {
        if append.text.trim().is_empty()
            || append.text.len() > MAX_ENTRY_BYTES
            || append.kind.is_empty()
            || append.doc_workspace.is_empty()
        {
            return Err(StoreError::Inconsistent);
        }
        let mut connection = self.connection.lock();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let count: u64 = transaction.query_row(
            "SELECT COUNT(*) FROM bmad_memlog_entries
               WHERE workspace_id = ?1 AND doc_workspace = ?2",
            params![append.workspace_id, append.doc_workspace],
            |row| row.get(0),
        )?;
        if count >= MAX_ENTRIES_PER_DOC_WORKSPACE {
            return Err(StoreError::Inconsistent);
        }
        // MAX(sequence) rather than COUNT, so the ordering is stable even if a
        // future migration ever removes rows.
        let next: u64 = transaction
            .query_row(
                "SELECT COALESCE(MAX(sequence) + 1, 0) FROM bmad_memlog_entries
                   WHERE workspace_id = ?1 AND doc_workspace = ?2",
                params![append.workspace_id, append.doc_workspace],
                |row| row.get(0),
            )
            .unwrap_or(0);
        transaction.execute(
            "INSERT INTO bmad_memlog_entries
               (entry_id, workspace_id, doc_workspace, sequence, kind, author, text,
                recorded_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                append.entry_id,
                append.workspace_id,
                append.doc_workspace,
                next,
                append.kind,
                append.author,
                append.text,
                append.recorded_at_ms,
            ],
        )?;
        transaction.commit()?;
        Ok(next)
    }

    /// Every entry for one doc workspace, oldest first.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot be read.
    pub fn memlog_entries(
        &self,
        workspace_id: &str,
        doc_workspace: &str,
    ) -> Result<Vec<StoredMemlogEntry>, StoreError> {
        let connection = self.connection.lock();
        let mut statement = connection.prepare(
            "SELECT sequence, kind, author, text, recorded_at_ms
               FROM bmad_memlog_entries
               WHERE workspace_id = ?1 AND doc_workspace = ?2
               ORDER BY sequence ASC",
        )?;
        let rows = statement.query_map(params![workspace_id, doc_workspace], |row| {
            Ok(StoredMemlogEntry {
                sequence: row.get(0)?,
                kind: row.get(1)?,
                author: row.get(2)?,
                text: row.get(3)?,
                recorded_at_ms: row.get(4)?,
            })
        })?;
        let mut entries = Vec::new();
        for entry in rows {
            entries.push(entry?);
        }
        Ok(entries)
    }
}
