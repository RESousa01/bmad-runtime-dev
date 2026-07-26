//! Durable storage for the method's working memory (P14).
//!
//! Upstream's memlog is "the dense, chronological record of everything that
//! mattered in a piece of work", and it "persists ACROSS sessions, so a fresh
//! session can load it and continue". Downstream artifacts — a brief, a PRD, a
//! spine — are *derived* from it rather than being its source.
//!
//! Three invariants come from upstream's `Memlog` type and are enforced here
//! rather than by convention, so a stored log can always be rebuilt into one:
//!
//! * **Append-only.** There is no update and no delete. Upstream: "There is no
//!   edit or delete subcommand by design; history is never rewritten."
//! * **Chronological within a doc workspace.** `sequence` is allocated inside
//!   the appending transaction, and an entry may not be recorded before the
//!   entry preceding it — the same rule `Memlog::append` applies in memory.
//! * **Bounded.** Per entry and per doc workspace, so a log stays working
//!   memory rather than becoming an unbounded sink for model prose.

use desktop_runtime::MemlogEntryKind;
use rusqlite::{params, TransactionBehavior};

use super::{LocalStore, StoreError};

/// One entry as stored. `sequence` is assigned by the store, never by a caller.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredMemlogEntry {
    pub sequence: u64,
    pub kind: MemlogEntryKind,
    pub author: Option<String>,
    /// What the entry is about — a section heading for a document capability.
    /// `None` for entries that are not about one identifiable thing.
    pub subject: Option<String>,
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
    pub kind: MemlogEntryKind,
    pub author: Option<&'a str>,
    pub subject: Option<&'a str>,
    pub text: &'a str,
    pub recorded_at_ms: u64,
}

/// What happened to an append.
///
/// Reaching the ceiling is an ordinary, expected end state rather than a
/// failure: the caller's own work is already durable, and refusing to record
/// more memory must never retroactively fail it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemlogAppendOutcome {
    Appended(u64),
    AtCapacity,
}

/// Per-doc-workspace ceiling. A log is working memory, not an archive. Kept
/// under `Memlog`'s own 4096-entry ceiling so a stored log always rebuilds.
pub const MAX_MEMLOG_ENTRIES_PER_DOC_WORKSPACE: u64 = 2_048;
/// Per-entry ceiling, matching `Memlog`'s. Upstream keeps entries to "one line
/// each, the gist in the user's meaning"; a caller with longer source material
/// is responsible for condensing it to that before appending.
pub const MAX_MEMLOG_ENTRY_BYTES: usize = 4_096;
/// Per-subject ceiling, matching the document artifact's heading bound.
pub const MAX_MEMLOG_SUBJECT_BYTES: usize = 200;

impl LocalStore {
    /// Appends one entry, allocating its sequence inside the transaction.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Inconsistent`] when the entry is empty, over a
    /// reviewed bound, or would land before the entry preceding it, and
    /// [`StoreError`] for any database failure. A full doc workspace is
    /// reported as [`MemlogAppendOutcome::AtCapacity`], not an error.
    pub fn append_memlog_entry(
        &self,
        append: MemlogAppend<'_>,
    ) -> Result<MemlogAppendOutcome, StoreError> {
        if append.text.trim().is_empty()
            || append.text.len() > MAX_MEMLOG_ENTRY_BYTES
            || append.doc_workspace.is_empty()
            || append.subject.is_some_and(|subject| {
                subject.is_empty() || subject.len() > MAX_MEMLOG_SUBJECT_BYTES
            })
        {
            return Err(StoreError::Inconsistent);
        }
        let mut connection = self.connection.lock();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (count, last_sequence, last_recorded_at): (u64, Option<u64>, Option<u64>) = transaction
            .query_row(
                "SELECT COUNT(*), MAX(sequence),
                        (SELECT recorded_at_ms FROM bmad_memlog_entries
                           WHERE workspace_id = ?1 AND doc_workspace = ?2
                           ORDER BY sequence DESC LIMIT 1)
                   FROM bmad_memlog_entries
                   WHERE workspace_id = ?1 AND doc_workspace = ?2",
                params![append.workspace_id, append.doc_workspace],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
        if count >= MAX_MEMLOG_ENTRIES_PER_DOC_WORKSPACE {
            return Ok(MemlogAppendOutcome::AtCapacity);
        }
        // The same rule `Memlog::append` enforces: a log that moves backwards in
        // time is not a chronology, and could not be rebuilt into one.
        if last_recorded_at.is_some_and(|previous| append.recorded_at_ms < previous) {
            return Err(StoreError::Inconsistent);
        }
        // MAX(sequence) rather than COUNT, so the ordering is stable even if a
        // future migration ever removes rows.
        let next = last_sequence.map_or(0, |sequence| sequence.saturating_add(1));
        transaction.execute(
            "INSERT INTO bmad_memlog_entries
               (entry_id, workspace_id, doc_workspace, sequence, kind, author, subject, text,
                recorded_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                append.entry_id,
                append.workspace_id,
                append.doc_workspace,
                next,
                append.kind.code(),
                append.author,
                append.subject,
                append.text,
                append.recorded_at_ms,
            ],
        )?;
        transaction.commit()?;
        Ok(MemlogAppendOutcome::Appended(next))
    }

    /// Every entry for one doc workspace, oldest first.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Inconsistent`] when a stored row carries a kind
    /// outside the reviewed vocabulary, and [`StoreError`] when the database
    /// cannot be read.
    pub fn memlog_entries(
        &self,
        workspace_id: &str,
        doc_workspace: &str,
    ) -> Result<Vec<StoredMemlogEntry>, StoreError> {
        let connection = self.connection.lock();
        let mut statement = connection.prepare(
            "SELECT sequence, kind, author, subject, text, recorded_at_ms
               FROM bmad_memlog_entries
               WHERE workspace_id = ?1 AND doc_workspace = ?2
               ORDER BY sequence ASC",
        )?;
        let rows = statement.query_map(params![workspace_id, doc_workspace], |row| {
            Ok((
                row.get::<_, u64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, u64>(5)?,
            ))
        })?;
        let mut entries = Vec::new();
        for row in rows {
            let (sequence, kind, author, subject, text, recorded_at_ms) = row?;
            entries.push(StoredMemlogEntry {
                sequence,
                kind: MemlogEntryKind::from_code(&kind).ok_or(StoreError::Inconsistent)?,
                author,
                subject,
                text,
                recorded_at_ms,
            });
        }
        Ok(entries)
    }
}
