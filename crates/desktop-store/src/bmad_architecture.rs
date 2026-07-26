//! Durable boundary for the architecture progression (P14 finding 1).
//!
//! The progression itself lived only in the host's memory, so a restart lost
//! which step a workspace had reached and reopened at step one. Upstream's
//! claim for a memlog is precisely the opposite: "a fresh session can load it
//! and continue".
//!
//! Only the *boundary* is stored here. Each accepted step's artifact is already
//! durable as its own capability run, so a session is a session id, the
//! workspace it reasoned about, and the ordered runs that answered its steps.
//! Restoring re-reads those runs, which is what keeps a resumed progression
//! bound to evidence the store already holds rather than to remembered state.

use rusqlite::{params, OptionalExtension, TransactionBehavior};

use super::{LocalStore, StoreError};

/// A progression that can be continued: its identity, when it opened, and the
/// runs that answered its steps in order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResumableArchitectureSession {
    pub session_id: String,
    pub opened_at_ms: u64,
    /// One entry per accepted step, in acceptance order.
    pub steps: Vec<ArchitectureSessionStep>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchitectureSessionStep {
    pub run_id: String,
    pub accepted_at_ms: u64,
}

impl LocalStore {
    /// Records a newly opened progression. Idempotent for the same identity, so
    /// re-selecting a step in an open session does not error.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot be written.
    pub fn open_architecture_session(
        &self,
        session_id: &str,
        workspace_id: &str,
        opened_at_ms: u64,
    ) -> Result<(), StoreError> {
        let connection = self.connection.lock();
        connection.execute(
            "INSERT INTO bmad_architecture_sessions
               (session_id, workspace_id, opened_at_ms, finalized)
             VALUES (?1, ?2, ?3, 0)
             ON CONFLICT(session_id) DO NOTHING",
            params![session_id, workspace_id, opened_at_ms],
        )?;
        Ok(())
    }

    /// Records one accepted step against its session.
    ///
    /// The ordinal is allocated inside the transaction, and a run may answer at
    /// most one step, so a replayed or duplicated acceptance cannot silently
    /// advance a progression twice.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::StateConflict`] when the session is unknown or
    /// already finalized, and [`StoreError`] for any database failure.
    pub fn record_architecture_step(
        &self,
        session_id: &str,
        run_id: &str,
        accepted_at_ms: u64,
    ) -> Result<u64, StoreError> {
        let mut connection = self.connection.lock();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let finalized: Option<i64> = transaction
            .query_row(
                "SELECT finalized FROM bmad_architecture_sessions WHERE session_id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .optional()?;
        if finalized != Some(0) {
            return Err(StoreError::StateConflict);
        }
        let ordinal: u64 = transaction.query_row(
            "SELECT COUNT(*) FROM bmad_architecture_session_steps WHERE session_id = ?1",
            params![session_id],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO bmad_architecture_session_steps
               (session_id, ordinal, run_id, accepted_at_ms)
             VALUES (?1, ?2, ?3, ?4)",
            params![session_id, ordinal, run_id, accepted_at_ms],
        )?;
        transaction.commit()?;
        Ok(ordinal)
    }

    /// Closes a progression. A finalized session is never resumed: its spine
    /// belongs to the workspace it was reasoned about and is not continued.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot be written.
    pub fn finalize_architecture_session(&self, session_id: &str) -> Result<(), StoreError> {
        let connection = self.connection.lock();
        connection.execute(
            "UPDATE bmad_architecture_sessions SET finalized = 1 WHERE session_id = ?1",
            params![session_id],
        )?;
        Ok(())
    }

    /// The newest unfinalized progression for a workspace, with its steps in
    /// acceptance order, or `None` when there is nothing to continue.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot be read.
    pub fn resumable_architecture_session(
        &self,
        workspace_id: &str,
    ) -> Result<Option<ResumableArchitectureSession>, StoreError> {
        let connection = self.connection.lock();
        let session: Option<(String, u64)> = connection
            .query_row(
                "SELECT session_id, opened_at_ms FROM bmad_architecture_sessions
                   WHERE workspace_id = ?1 AND finalized = 0
                   ORDER BY opened_at_ms DESC, session_id DESC LIMIT 1",
                params![workspace_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((session_id, opened_at_ms)) = session else {
            return Ok(None);
        };
        let mut statement = connection.prepare(
            "SELECT run_id, accepted_at_ms FROM bmad_architecture_session_steps
               WHERE session_id = ?1 ORDER BY ordinal ASC",
        )?;
        let steps = statement
            .query_map(params![session_id], |row| {
                Ok(ArchitectureSessionStep {
                    run_id: row.get(0)?,
                    accepted_at_ms: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(ResumableArchitectureSession {
            session_id,
            opened_at_ms,
            steps,
        }))
    }
}
