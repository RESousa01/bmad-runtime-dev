//! Durable storage for typed BMAD memory entries (ENG-10).
//!
//! One row per entry, mirroring `memlog.rs`'s append-only shape: a
//! `memlog_entry` row is only ever inserted, never updated or deleted.
//! `replaceable` entries (facts and compaction summaries) may have their own
//! row's `superseded_by_entry_id` and `entry_json` rewritten when a
//! compaction commits, but only inside the same immediate transaction that
//! inserts the summary row superseding them — the row is retired in place,
//! never deleted, until a retention purge removes it.
//!
//! `content_hash` and `recorded_at_sequence` are stored twice: once inside
//! `entry_json` (the contract's own view) and once as indexed columns. Every
//! read re-derives them from the parsed JSON and compares — a row where the
//! two disagree did not get that way through this module, so it is reported
//! as [`StoreError::Inconsistent`] rather than silently trusted.

use desktop_runtime::generated_contracts::{
    BmadMemoryMemoryEntry, BmadMemoryMemoryEntryMutability, BmadMemoryMemoryScopeKind,
};
use rusqlite::{params, TransactionBehavior};

use super::{is_unique_violation, LocalStore, StoreError};

/// One durably recovered memory entry alongside the store's own bookkeeping.
#[derive(Clone, Debug)]
pub struct StoredBmadMemoryEntry {
    pub entry: BmadMemoryMemoryEntry,
    pub workspace_id: String,
    pub created_at_ms: u64,
}

/// One scope to query or purge by. `identifier` is the scope's own id
/// (`sessionId`/`workspaceId`/`installedAgentId`); `user` scope carries none.
#[derive(Clone, Copy, Debug)]
pub struct BmadMemoryScopeQuery<'a> {
    pub kind: BmadMemoryMemoryScopeKind,
    pub identifier: Option<&'a str>,
}

fn scope_kind_code(kind: BmadMemoryMemoryScopeKind) -> &'static str {
    match kind {
        BmadMemoryMemoryScopeKind::Session => "session",
        BmadMemoryMemoryScopeKind::Project => "project",
        BmadMemoryMemoryScopeKind::User => "user",
        BmadMemoryMemoryScopeKind::InstalledAgent => "installed_agent",
    }
}

fn retention_code(
    retention: desktop_runtime::generated_contracts::BmadMemoryMemoryEntryRetention,
) -> &'static str {
    use desktop_runtime::generated_contracts::BmadMemoryMemoryEntryRetention as Retention;
    match retention {
        Retention::Session => "session",
        Retention::Bounded => "bounded",
        Retention::Pinned => "pinned",
    }
}

fn mutability_code(mutability: BmadMemoryMemoryEntryMutability) -> &'static str {
    match mutability {
        BmadMemoryMemoryEntryMutability::AppendOnly => "append_only",
        BmadMemoryMemoryEntryMutability::Replaceable => "replaceable",
    }
}

fn entry_row_values(
    entry: &BmadMemoryMemoryEntry,
) -> Result<(String, &'static str, &'static str, i64, &'static str), StoreError> {
    let json = serde_json::to_string(entry)?;
    Ok((
        json,
        scope_kind_code(entry.scope.kind),
        retention_code(entry.retention),
        entry.provenance.recorded_at_sequence,
        mutability_code(entry.mutability),
    ))
}

/// Re-derives an entry from its stored row and proves the indexed columns
/// still agree with what the JSON itself says.
fn decode_and_verify_entry(
    entry_json: &str,
    indexed_content_hash: &str,
    indexed_sequence: i64,
) -> Result<BmadMemoryMemoryEntry, StoreError> {
    let entry: BmadMemoryMemoryEntry = serde_json::from_str(entry_json)?;
    if entry.content_hash.as_str() != indexed_content_hash
        || entry.provenance.recorded_at_sequence != indexed_sequence
    {
        return Err(StoreError::Inconsistent);
    }
    Ok(entry)
}

impl LocalStore {
    /// Durably appends one entry. Never updates an existing row — this is
    /// the only way a row enters `bmad_memory_entries`.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::AlreadyConsumed`] when `entry_id` is already
    /// taken, and [`StoreError::Inconsistent`] when the entry cannot be
    /// represented as JSON.
    pub fn append_bmad_memory_entry(
        &self,
        workspace_id: &str,
        entry: &BmadMemoryMemoryEntry,
        now_ms: u64,
    ) -> Result<(), StoreError> {
        if workspace_id.is_empty() {
            return Err(StoreError::Inconsistent);
        }
        let (json, scope_kind, retention, sequence, mutability) = entry_row_values(entry)?;
        let connection = self.connection.lock();
        let result = connection.execute(
            "INSERT INTO bmad_memory_entries
             (entry_id, workspace_id, memory_class, scope_kind, scope_session_id,
              scope_workspace_id, scope_installed_agent_id, origin, sensitivity,
              retention, mutability, content_hash, recorded_at_sequence,
              superseded_by_entry_id, entry_json, created_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                entry.entry_id.as_str(),
                workspace_id,
                entry.memory_class.to_string(),
                scope_kind,
                entry.scope.session_id.as_deref().map(String::as_str),
                entry.scope.workspace_id.as_deref().map(String::as_str),
                entry
                    .scope
                    .installed_agent_id
                    .as_deref()
                    .map(String::as_str),
                entry.provenance.origin.to_string(),
                entry.sensitivity.to_string(),
                retention,
                mutability,
                entry.content_hash.as_str(),
                sequence,
                entry.superseded_by_entry_id.as_deref().map(String::as_str),
                json,
                i64::try_from(now_ms).map_err(|_| StoreError::Inconsistent)?,
            ],
        );
        match result {
            Ok(_) => Ok(()),
            Err(error) if is_unique_violation(&error) => Err(StoreError::AlreadyConsumed),
            Err(error) => Err(StoreError::Sqlite(error)),
        }
    }

    /// Commits one compaction: retires its `replaceable` sources in place and
    /// appends the summary row, atomically.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Inconsistent`] when `summary` is not a
    /// `compaction_summary`, when a named source row does not exist, or when
    /// a named source row is stored as `append_only` — that guard must never
    /// trigger for a summary produced by `desktop_runtime::bmad::memory`'s
    /// `compact`, so tripping it here means an untrusted caller bypassed the
    /// domain function.
    pub fn commit_bmad_memory_compaction(
        &self,
        workspace_id: &str,
        updated_sources: &[BmadMemoryMemoryEntry],
        summary: &BmadMemoryMemoryEntry,
        now_ms: u64,
    ) -> Result<(), StoreError> {
        if workspace_id.is_empty()
            || summary.memory_class
                != desktop_runtime::generated_contracts::BmadMemoryMemoryEntryMemoryClass::CompactionSummary
        {
            return Err(StoreError::Inconsistent);
        }
        let now = i64::try_from(now_ms).map_err(|_| StoreError::Inconsistent)?;
        let mut connection = self.connection.lock();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

        for source in updated_sources {
            let (json, _, _, sequence, mutability) = entry_row_values(source)?;
            let stored_mutability: String = transaction
                .query_row(
                    "SELECT mutability FROM bmad_memory_entries WHERE entry_id = ?1",
                    params![source.entry_id.as_str()],
                    |row| row.get(0),
                )
                .map_err(|_| StoreError::Inconsistent)?;
            if stored_mutability != mutability_code(BmadMemoryMemoryEntryMutability::Replaceable)
                || mutability != mutability_code(BmadMemoryMemoryEntryMutability::Replaceable)
            {
                return Err(StoreError::Inconsistent);
            }
            let updated = transaction.execute(
                "UPDATE bmad_memory_entries
                 SET entry_json = ?1, superseded_by_entry_id = ?2, recorded_at_sequence = ?3
                 WHERE entry_id = ?4",
                params![
                    json,
                    source.superseded_by_entry_id.as_deref().map(String::as_str),
                    sequence,
                    source.entry_id.as_str(),
                ],
            )?;
            if updated != 1 {
                return Err(StoreError::Inconsistent);
            }
        }

        let (json, scope_kind, retention, sequence, mutability) = entry_row_values(summary)?;
        let inserted = transaction.execute(
            "INSERT INTO bmad_memory_entries
             (entry_id, workspace_id, memory_class, scope_kind, scope_session_id,
              scope_workspace_id, scope_installed_agent_id, origin, sensitivity,
              retention, mutability, content_hash, recorded_at_sequence,
              superseded_by_entry_id, entry_json, created_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                summary.entry_id.as_str(),
                workspace_id,
                summary.memory_class.to_string(),
                scope_kind,
                summary.scope.session_id.as_deref().map(String::as_str),
                summary.scope.workspace_id.as_deref().map(String::as_str),
                summary
                    .scope
                    .installed_agent_id
                    .as_deref()
                    .map(String::as_str),
                summary.provenance.origin.to_string(),
                summary.sensitivity.to_string(),
                retention,
                mutability,
                summary.content_hash.as_str(),
                sequence,
                summary
                    .superseded_by_entry_id
                    .as_deref()
                    .map(String::as_str),
                json,
                now,
            ],
        );
        match inserted {
            Ok(_) => {}
            Err(error) if is_unique_violation(&error) => return Err(StoreError::AlreadyConsumed),
            Err(error) => return Err(StoreError::Sqlite(error)),
        }

        transaction.commit()?;
        Ok(())
    }

    /// Every entry visible to exactly one scope in one workspace, oldest
    /// first — an entry scoped to a different workspace, session, or
    /// installed agent is never returned.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Inconsistent`] when a stored row's JSON
    /// disagrees with its own indexed `content_hash` or
    /// `recorded_at_sequence`.
    pub fn bmad_memory_entries_for_scope(
        &self,
        workspace_id: &str,
        scope: BmadMemoryScopeQuery<'_>,
    ) -> Result<Vec<StoredBmadMemoryEntry>, StoreError> {
        let connection = self.connection.lock();
        let mut statement = connection.prepare(
            "SELECT entry_json, content_hash, recorded_at_sequence, created_at_ms
             FROM bmad_memory_entries
             WHERE workspace_id = ?1 AND scope_kind = ?2
               AND ((?3 IS NULL AND scope_session_id IS NULL AND scope_workspace_id IS NULL
                     AND scope_installed_agent_id IS NULL)
                    OR scope_session_id = ?3 OR scope_workspace_id = ?3
                    OR scope_installed_agent_id = ?3)
             ORDER BY recorded_at_sequence ASC, entry_id ASC",
        )?;
        let rows = statement.query_map(
            params![workspace_id, scope_kind_code(scope.kind), scope.identifier],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )?;
        let mut entries = Vec::new();
        for row in rows {
            let (json, content_hash, sequence, created_at_ms) = row?;
            let entry = decode_and_verify_entry(&json, &content_hash, sequence)?;
            entries.push(StoredBmadMemoryEntry {
                entry,
                workspace_id: workspace_id.to_owned(),
                created_at_ms: u64::try_from(created_at_ms)
                    .map_err(|_| StoreError::Inconsistent)?,
            });
        }
        Ok(entries)
    }

    /// Every entry in one workspace regardless of scope, oldest first.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Inconsistent`] under the same corruption check
    /// as [`Self::bmad_memory_entries_for_scope`].
    pub fn bmad_memory_entries_for_workspace(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<StoredBmadMemoryEntry>, StoreError> {
        let connection = self.connection.lock();
        let mut statement = connection.prepare(
            "SELECT entry_json, content_hash, recorded_at_sequence, created_at_ms
             FROM bmad_memory_entries
             WHERE workspace_id = ?1
             ORDER BY recorded_at_sequence ASC, entry_id ASC",
        )?;
        let rows = statement.query_map(params![workspace_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;
        let mut entries = Vec::new();
        for row in rows {
            let (json, content_hash, sequence, created_at_ms) = row?;
            let entry = decode_and_verify_entry(&json, &content_hash, sequence)?;
            entries.push(StoredBmadMemoryEntry {
                entry,
                workspace_id: workspace_id.to_owned(),
                created_at_ms: u64::try_from(created_at_ms)
                    .map_err(|_| StoreError::Inconsistent)?,
            });
        }
        Ok(entries)
    }

    /// Deletes every row of one retention class in one workspace and reports
    /// how many rows were removed. `pinned` rows are only ever removed by
    /// naming `pinned` explicitly — the ordinary sweep is expected to pass
    /// `session`, leaving `bounded` and `pinned` rows untouched.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot be written.
    pub fn purge_bmad_memory_retention(
        &self,
        workspace_id: &str,
        retention: desktop_runtime::generated_contracts::BmadMemoryMemoryEntryRetention,
    ) -> Result<u64, StoreError> {
        let connection = self.connection.lock();
        let removed = connection.execute(
            "DELETE FROM bmad_memory_entries WHERE workspace_id = ?1 AND retention = ?2",
            params![workspace_id, retention_code(retention)],
        )?;
        Ok(u64::try_from(removed).unwrap_or(u64::MAX))
    }

    /// Deletes every entry in one scope, regardless of retention — the
    /// complete-deletion path an export/erasure request needs. Returns the
    /// number of rows removed.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot be written.
    pub fn delete_bmad_memory_scope(
        &self,
        workspace_id: &str,
        scope: BmadMemoryScopeQuery<'_>,
    ) -> Result<u64, StoreError> {
        let connection = self.connection.lock();
        let removed = connection.execute(
            "DELETE FROM bmad_memory_entries
             WHERE workspace_id = ?1 AND scope_kind = ?2
               AND ((?3 IS NULL AND scope_session_id IS NULL AND scope_workspace_id IS NULL
                     AND scope_installed_agent_id IS NULL)
                    OR scope_session_id = ?3 OR scope_workspace_id = ?3
                    OR scope_installed_agent_id = ?3)",
            params![workspace_id, scope_kind_code(scope.kind), scope.identifier],
        )?;
        Ok(u64::try_from(removed).unwrap_or(u64::MAX))
    }
}
