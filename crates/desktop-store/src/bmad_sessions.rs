//! Durable persistence for the generic BMAD skill lifecycle (ENG-05).
//!
//! The session row is the accepted state. Advancing it is a compare-and-swap
//! on `version` inside one immediate transaction that also appends the event
//! explaining the move — so a crash between the two is not representable.
//! Restart recovery therefore never replays commands: it loads the row it
//! already agreed to, and the append-only event log stays available as an
//! explanation rather than as a source of truth.

use desktop_runtime::{generated_contracts::BmadSession, BmadSessionEvent, BmadSessionProjection};
use rusqlite::{params, OptionalExtension, TransactionBehavior};

use super::{is_unique_violation, LocalStore, StoreError};

/// One recovered session with its renderer-safe projection.
#[derive(Clone, Debug)]
pub struct BmadSessionRecord {
    pub session: BmadSession,
    pub projection: BmadSessionProjection,
    pub workspace_id: String,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

impl LocalStore {
    /// Durably creates one session at version 1.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::AlreadyConsumed`] when the session identifier is
    /// already taken and [`StoreError::Inconsistent`] when the contract does
    /// not start at version 1 or cannot be represented as JSON.
    pub fn create_bmad_session(
        &self,
        session: &BmadSession,
        workspace_id: &str,
        now_ms: u64,
    ) -> Result<(), StoreError> {
        if session.version.get() != 1 {
            return Err(StoreError::Inconsistent);
        }
        if workspace_id.is_empty() || session.identities.workspace_id.as_str() != workspace_id {
            return Err(StoreError::Inconsistent);
        }
        let json = serde_json::to_string(session)?;
        let now = now_ms;
        let connection = self.connection.lock();
        let result = connection.execute(
            "INSERT INTO bmad_sessions
             (session_id, capability_id, workspace_id, state, version, evidence_hash,
              source_id, package_id, config_hash, customization_hash, model_id,
              task_id, policy_hash, session_json, created_at_ms, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15)",
            params![
                session.session_id.as_str(),
                session.capability_id.as_str(),
                workspace_id,
                session.state.to_string(),
                i64::try_from(session.version.get()).map_err(|_| StoreError::Inconsistent)?,
                session.evidence_hash.as_str(),
                session.identities.source_id.as_str(),
                session.identities.package_id.as_str(),
                session.identities.config_hash.as_str(),
                session.identities.customization_hash.as_str(),
                session.identities.model_id.as_str(),
                session.identities.task_id.as_str(),
                session.identities.policy_hash.as_str(),
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

    /// Commits one accepted transition: the new session state and the event
    /// that produced it, atomically, under an optimistic version guard.
    ///
    /// An idempotent event writes nothing and reports success, so a caller
    /// retrying a redelivered command converges rather than erroring.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Inconsistent`] for an unknown session, a session
    /// whose stored version is not the event's `from_version`, or an event
    /// that does not describe the supplied session, and
    /// [`StoreError::AlreadyConsumed`] when that version was already appended.
    pub fn commit_bmad_session_transition(
        &self,
        session: &BmadSession,
        event: &BmadSessionEvent,
        now_ms: u64,
    ) -> Result<(), StoreError> {
        if event.session_id != session.session_id.as_str() {
            return Err(StoreError::Inconsistent);
        }
        if event.idempotent {
            return Ok(());
        }
        if session.version.get() != event.to_version {
            return Err(StoreError::Inconsistent);
        }
        let json = serde_json::to_string(session)?;
        let now = now_ms;
        let mut connection = self.connection.lock();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

        let updated = transaction.execute(
            "UPDATE bmad_sessions
             SET state = ?1, version = ?2, evidence_hash = ?3, session_json = ?4,
                 updated_at_ms = ?5
             WHERE session_id = ?6 AND version = ?7",
            params![
                session.state.to_string(),
                i64::try_from(event.to_version).map_err(|_| StoreError::Inconsistent)?,
                session.evidence_hash.as_str(),
                json,
                i64::try_from(now).map_err(|_| StoreError::Inconsistent)?,
                session.session_id.as_str(),
                i64::try_from(event.from_version).map_err(|_| StoreError::Inconsistent)?,
            ],
        )?;
        if updated != 1 {
            return Err(StoreError::Inconsistent);
        }

        let appended = transaction.execute(
            "INSERT INTO bmad_session_events
             (session_id, to_version, from_version, command, from_state, to_state,
              previous_evidence_hash, evidence_hash, recorded_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                event.session_id.as_str(),
                i64::try_from(event.to_version).map_err(|_| StoreError::Inconsistent)?,
                i64::try_from(event.from_version).map_err(|_| StoreError::Inconsistent)?,
                event.command,
                event.from_state.to_string(),
                event.to_state.to_string(),
                event.previous_evidence_hash.to_string(),
                event.evidence_hash.to_string(),
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

    /// Recovers one session exactly as it was last accepted.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Inconsistent`] when the stored document does not
    /// parse as a session or has drifted from its indexed version.
    pub fn load_bmad_session(
        &self,
        session_id: &str,
    ) -> Result<Option<BmadSessionRecord>, StoreError> {
        let connection = self.connection.lock();
        let row = connection
            .query_row(
                "SELECT session_json, workspace_id, version, created_at_ms, updated_at_ms
                 FROM bmad_sessions WHERE session_id = ?1",
                params![session_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                },
            )
            .optional()?;
        let Some((json, workspace_id, version, created_at_ms, updated_at_ms)) = row else {
            return Ok(None);
        };
        let session: BmadSession = serde_json::from_str(&json)?;
        let indexed = u64::try_from(version).map_err(|_| StoreError::Inconsistent)?;
        if session.version.get() != indexed {
            return Err(StoreError::Inconsistent);
        }
        let projection = desktop_runtime::project_bmad_session(&session);
        Ok(Some(BmadSessionRecord {
            session,
            projection,
            workspace_id,
            created_at_ms: u64::try_from(created_at_ms).map_err(|_| StoreError::Inconsistent)?,
            updated_at_ms: u64::try_from(updated_at_ms).map_err(|_| StoreError::Inconsistent)?,
        }))
    }

    /// Returns the append-only transition log for one session in order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Sqlite`] when the log cannot be read.
    pub fn bmad_session_event_log(
        &self,
        session_id: &str,
    ) -> Result<Vec<BmadSessionEventRow>, StoreError> {
        let connection = self.connection.lock();
        let mut statement = connection.prepare(
            "SELECT from_version, to_version, command, from_state, to_state,
                    previous_evidence_hash, evidence_hash
             FROM bmad_session_events WHERE session_id = ?1 ORDER BY to_version",
        )?;
        let rows = statement
            .query_map(params![session_id], |row| {
                Ok(BmadSessionEventRow {
                    from_version: row.get::<_, i64>(0)?,
                    to_version: row.get::<_, i64>(1)?,
                    command: row.get(2)?,
                    from_state: row.get(3)?,
                    to_state: row.get(4)?,
                    previous_evidence_hash: row.get(5)?,
                    evidence_hash: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

/// One durable row of the append-only session transition log.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadSessionEventRow {
    pub from_version: i64,
    pub to_version: i64,
    pub command: String,
    pub from_state: String,
    pub to_state: String,
    pub previous_evidence_hash: String,
    pub evidence_hash: String,
}
