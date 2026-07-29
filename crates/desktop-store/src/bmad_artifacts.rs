//! Durable persistence for the BMAD artifact lifecycle (ENG-07).
//!
//! The artifact row is the accepted state. Advancing it is a compare-and-swap
//! on a store-owned `sequence` inside one immediate transaction that also
//! appends the event explaining the move — so a crash between the two is not
//! representable. Restart recovery therefore never replays commands: it loads
//! the row it already agreed to, and the append-only event log stays
//! available as an explanation rather than as a source of truth. This mirrors
//! `bmad_sessions.rs`'s shape.
//!
//! `sequence` is deliberately not the same counter as the artifact contract's
//! own `revision` field. `revision` is a *content* counter: it only advances
//! when `revise` replaces the raw output and content hash, by design (a
//! revision bumps `revision` and resets validation; `validate`/`approve`/
//! `write`/`record_completion_hook` all move the lifecycle forward without
//! producing new content, so they must not bump it). `sequence` advances on
//! every accepted transition regardless, because the store's optimistic
//! concurrency guard needs a counter that strictly increases once per commit
//! to detect a stale writer.

use desktop_runtime::generated_contracts::BmadArtifact;
use rusqlite::{params, OptionalExtension, TransactionBehavior};

use super::{is_unique_violation, LocalStore, StoreError};

/// One recovered artifact, with the store's own transition sequence alongside
/// the contract's content revision.
#[derive(Clone, Debug)]
pub struct BmadArtifactRecord {
    pub artifact: BmadArtifact,
    pub workspace_id: String,
    pub sequence: u64,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

/// One accepted artifact transition to append alongside the new row.
///
/// `from_sequence`/`to_sequence` are the store's own transition counter, not
/// the artifact contract's `revision` field — see the module doc.
#[derive(Clone, Debug)]
pub struct BmadArtifactEvent {
    pub artifact_id: String,
    pub to_sequence: u64,
    pub from_sequence: u64,
    pub command: String,
    pub from_state: String,
    pub to_state: String,
}

impl LocalStore {
    /// Durably creates one artifact at sequence 1.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::AlreadyConsumed`] when the artifact identifier is
    /// already taken and [`StoreError::Inconsistent`] when the contract does
    /// not start at content revision 1, the workspace is empty, or the
    /// contract cannot be represented as JSON.
    pub fn create_bmad_artifact(
        &self,
        artifact: &BmadArtifact,
        workspace_id: &str,
        now_ms: u64,
    ) -> Result<(), StoreError> {
        if artifact.revision.get() != 1 {
            return Err(StoreError::Inconsistent);
        }
        if workspace_id.is_empty() {
            return Err(StoreError::Inconsistent);
        }
        let json = serde_json::to_string(artifact)?;
        let now = now_ms;
        let connection = self.connection.lock();
        let result = connection.execute(
            "INSERT INTO bmad_artifacts
             (artifact_id, session_id, workspace_id, state, sequence, revision,
              content_hash, raw_output_hash, evidence_hash, source_id, template_hash,
              inputs_hash, artifact_json, created_at_ms, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13)",
            params![
                artifact.artifact_id.as_str(),
                artifact.session_id.as_str(),
                workspace_id,
                artifact.state.to_string(),
                i64::try_from(artifact.revision.get()).map_err(|_| StoreError::Inconsistent)?,
                artifact.content_hash.as_str(),
                artifact
                    .raw_output_hash
                    .as_ref()
                    .map(|hash| hash.as_str().to_owned()),
                artifact.evidence_hash.as_str(),
                artifact.lineage.source_id.as_str(),
                artifact.lineage.template_hash.as_str(),
                artifact.lineage.inputs_hash.as_str(),
                json,
                i64::try_from(now).map_err(|_| StoreError::Inconsistent)?,
            ],
        );
        match result {
            Ok(_) => Ok(()),
            Err(error) if is_unique_violation(&error) => Err(StoreError::AlreadyConsumed),
            Err(error) => Err(StoreError::Sqlite(error)),
        }
    }

    /// Commits one accepted transition: the new artifact state and the event
    /// that produced it, atomically, under an optimistic sequence guard.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Inconsistent`] for an unknown artifact, an
    /// artifact whose stored sequence is not the event's `from_sequence`, or
    /// an event that does not describe the supplied artifact, and
    /// [`StoreError::AlreadyConsumed`] when that sequence was already
    /// appended.
    pub fn commit_bmad_artifact_transition(
        &self,
        artifact: &BmadArtifact,
        event: &BmadArtifactEvent,
        now_ms: u64,
    ) -> Result<(), StoreError> {
        if event.artifact_id != artifact.artifact_id.as_str() {
            return Err(StoreError::Inconsistent);
        }
        let json = serde_json::to_string(artifact)?;
        let now = now_ms;
        let mut connection = self.connection.lock();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

        let updated = transaction.execute(
            "UPDATE bmad_artifacts
             SET state = ?1, sequence = ?2, revision = ?3, content_hash = ?4,
                 raw_output_hash = ?5, evidence_hash = ?6, artifact_json = ?7,
                 updated_at_ms = ?8
             WHERE artifact_id = ?9 AND sequence = ?10",
            params![
                artifact.state.to_string(),
                i64::try_from(event.to_sequence).map_err(|_| StoreError::Inconsistent)?,
                i64::try_from(artifact.revision.get()).map_err(|_| StoreError::Inconsistent)?,
                artifact.content_hash.as_str(),
                artifact
                    .raw_output_hash
                    .as_ref()
                    .map(|hash| hash.as_str().to_owned()),
                artifact.evidence_hash.as_str(),
                json,
                i64::try_from(now).map_err(|_| StoreError::Inconsistent)?,
                artifact.artifact_id.as_str(),
                i64::try_from(event.from_sequence).map_err(|_| StoreError::Inconsistent)?,
            ],
        )?;
        if updated != 1 {
            return Err(StoreError::Inconsistent);
        }

        let appended = transaction.execute(
            "INSERT INTO bmad_artifact_events
             (artifact_id, to_revision, from_revision, command, from_state, to_state,
              recorded_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                event.artifact_id,
                i64::try_from(event.to_sequence).map_err(|_| StoreError::Inconsistent)?,
                i64::try_from(event.from_sequence).map_err(|_| StoreError::Inconsistent)?,
                event.command,
                event.from_state,
                event.to_state,
                i64::try_from(now).map_err(|_| StoreError::Inconsistent)?,
            ],
        );
        match appended {
            Ok(_) => {}
            Err(error) if is_unique_violation(&error) => return Err(StoreError::AlreadyConsumed),
            Err(error) => return Err(StoreError::Sqlite(error)),
        }

        transaction.commit()?;
        Ok(())
    }

    /// Recovers one artifact exactly as it was last accepted.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Inconsistent`] when the stored document does not
    /// parse as an artifact or its content revision has drifted from the
    /// indexed column.
    pub fn load_bmad_artifact(
        &self,
        artifact_id: &str,
    ) -> Result<Option<BmadArtifactRecord>, StoreError> {
        let connection = self.connection.lock();
        let row = connection
            .query_row(
                "SELECT artifact_json, workspace_id, sequence, revision, created_at_ms,
                        updated_at_ms
                 FROM bmad_artifacts WHERE artifact_id = ?1",
                params![artifact_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                },
            )
            .optional()?;
        let Some((json, workspace_id, sequence, revision, created_at_ms, updated_at_ms)) = row
        else {
            return Ok(None);
        };
        let artifact: BmadArtifact = serde_json::from_str(&json)?;
        let indexed_revision = u64::try_from(revision).map_err(|_| StoreError::Inconsistent)?;
        if artifact.revision.get() != indexed_revision {
            return Err(StoreError::Inconsistent);
        }
        Ok(Some(BmadArtifactRecord {
            artifact,
            workspace_id,
            sequence: u64::try_from(sequence).map_err(|_| StoreError::Inconsistent)?,
            created_at_ms: u64::try_from(created_at_ms).map_err(|_| StoreError::Inconsistent)?,
            updated_at_ms: u64::try_from(updated_at_ms).map_err(|_| StoreError::Inconsistent)?,
        }))
    }

    /// Returns the append-only transition log for one artifact in order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Sqlite`] when the log cannot be read.
    pub fn bmad_artifact_event_log(
        &self,
        artifact_id: &str,
    ) -> Result<Vec<BmadArtifactEventRow>, StoreError> {
        let connection = self.connection.lock();
        let mut statement = connection.prepare(
            "SELECT from_revision, to_revision, command, from_state, to_state
             FROM bmad_artifact_events WHERE artifact_id = ?1 ORDER BY to_revision",
        )?;
        let rows = statement
            .query_map(params![artifact_id], |row| {
                Ok(BmadArtifactEventRow {
                    from_sequence: row.get::<_, i64>(0)?,
                    to_sequence: row.get::<_, i64>(1)?,
                    command: row.get(2)?,
                    from_state: row.get(3)?,
                    to_state: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

/// One durable row of the append-only artifact transition log.
///
/// `from_sequence`/`to_sequence` are the store's own transition counter — see
/// the module doc for why this is not the contract's `revision` field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadArtifactEventRow {
    pub from_sequence: i64,
    pub to_sequence: i64,
    pub command: String,
    pub from_state: String,
    pub to_state: String,
}
