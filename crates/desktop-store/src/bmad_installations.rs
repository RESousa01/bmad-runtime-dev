//! Durable persistence for governed installation and atomic catalog
//! activation (ENG-16).
//!
//! Without this module a caller could observe two installations of the same
//! package active at once, or observe a catalog pointer flip without the
//! evidence that justified it. Every non-activation transition is an
//! optimistic compare-and-swap on `version`, exactly like
//! `bmad_sessions::commit_bmad_session_transition`. Activation is different
//! on purpose: it moves the row being promoted and — when one exists — the
//! row it supersedes in the *same* `TransactionBehavior::Immediate`
//! transaction, so a crash between the two can never leave both rows
//! inactive or both active. The database's own partial unique index
//! (`bmad_installations_active_by_package`, one row per `package_id` with
//! `active = 1`) backs that invariant even against a bug in this module.

use desktop_runtime::generated_contracts::BmadInstallation;
use rusqlite::{params, OptionalExtension, TransactionBehavior};

use super::{is_unique_violation, LocalStore, StoreError};

/// One recovered installation row.
#[derive(Clone, Debug)]
pub struct BmadInstallationRecord {
    pub installation: BmadInstallation,
    pub workspace_id: String,
    pub active: bool,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

/// One durable row of the append-only installation transition log.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadInstallationEventRow {
    pub from_version: i64,
    pub to_version: i64,
    pub command: String,
    pub from_state: String,
    pub to_state: String,
    pub installation_hash: String,
}

/// The pair of rows written by one atomic activation: the installation being
/// promoted, and — when it is superseding a previously active installation
/// of the same package — the installation being demoted.
pub struct BmadInstallationActivationCommit<'a> {
    pub activated: &'a BmadInstallation,
    pub activated_from_version: u64,
    pub demoted: Option<&'a BmadInstallation>,
    pub demoted_from_version: Option<u64>,
}

impl LocalStore {
    /// Durably creates one installation at version 1, in state `planned` and
    /// never active — a package cannot be planned and already serving
    /// traffic in the same write.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::AlreadyConsumed`] when the installation
    /// identifier is already taken, and [`StoreError::Inconsistent`] when
    /// the contract does not start at version 1, is already active, or
    /// cannot be represented as JSON.
    pub fn create_bmad_installation(
        &self,
        installation: &BmadInstallation,
        workspace_id: &str,
        now_ms: u64,
    ) -> Result<(), StoreError> {
        if installation.version.get() != 1 || installation.activation.active {
            return Err(StoreError::Inconsistent);
        }
        let json = serde_json::to_string(installation)?;
        let connection = self.connection.lock();
        let result = connection.execute(
            "INSERT INTO bmad_installations
             (installation_id, workspace_id, package_id, state, version, active,
              installation_hash, installation_json, created_at_ms, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7, ?8, ?8)",
            params![
                installation.installation_id.as_str(),
                workspace_id,
                installation.package_id.as_str(),
                installation.state.to_string(),
                i64::try_from(installation.version.get()).map_err(|_| StoreError::Inconsistent)?,
                installation.installation_hash.as_str(),
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

    /// Commits one non-activation transition (review, approval, checkpoint,
    /// write, validation) under an optimistic version guard, appending the
    /// event that explains it in the same transaction.
    ///
    /// A replay of an already-applied transition (`to_version` already
    /// recorded) writes nothing and reports success, so a redelivered
    /// command converges instead of erroring or double-applying.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Inconsistent`] for an unknown installation, one
    /// whose stored version is not `from_version`, or one that transitions
    /// into `activated` (activation must go through
    /// [`Self::commit_bmad_installation_activation`] so the catalog pointer
    /// and its evidence move together).
    pub fn commit_bmad_installation_transition(
        &self,
        installation: &BmadInstallation,
        command: &str,
        from_version: u64,
        now_ms: u64,
    ) -> Result<(), StoreError> {
        if installation.activation.active
            || installation.version.get() != from_version + 1
            || command.is_empty()
        {
            return Err(StoreError::Inconsistent);
        }
        let mut connection = self.connection.lock();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if event_already_recorded(&transaction, installation)? {
            return Ok(());
        }
        let from_state = apply_installation_row(&transaction, installation, from_version, now_ms)?;
        append_installation_event(
            &transaction,
            installation,
            command,
            from_version,
            &from_state,
            now_ms,
        )
        .map_err(StoreError::Sqlite)?;
        transaction.commit()?;
        Ok(())
    }

    /// Atomically activates one installation and, when superseding a prior
    /// active installation of the same package, demotes it in the same
    /// transaction. The database's partial unique index on
    /// `(package_id) WHERE active = 1` rejects any attempt that would leave
    /// two installations of one package active together.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Inconsistent`] when either row's version guard
    /// fails, and [`StoreError::AlreadyConsumed`] when the active-package
    /// invariant would be violated or the transition was already applied.
    pub fn commit_bmad_installation_activation(
        &self,
        commit: &BmadInstallationActivationCommit<'_>,
        now_ms: u64,
    ) -> Result<(), StoreError> {
        if !commit.activated.activation.active {
            return Err(StoreError::Inconsistent);
        }
        let mut connection = self.connection.lock();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

        if event_already_recorded(&transaction, commit.activated)? {
            return Ok(());
        }

        // The demoted row must be written before the promoted one: SQLite
        // checks a UNIQUE index immediately at the statement that would
        // violate it, not at commit, so activating the new row first would
        // momentarily leave two installations of the same package active
        // and be rejected by `bmad_installations_active_by_package` even
        // though the transaction was going to resolve to exactly one.
        if let (Some(demoted), Some(demoted_from_version)) =
            (commit.demoted, commit.demoted_from_version)
        {
            if demoted.activation.active {
                return Err(StoreError::Inconsistent);
            }
            let demoted_from_state =
                apply_installation_row(&transaction, demoted, demoted_from_version, now_ms)?;
            append_installation_event(
                &transaction,
                demoted,
                "supersede",
                demoted_from_version,
                &demoted_from_state,
                now_ms,
            )
            .map_err(StoreError::Sqlite)?;
        }

        let activated_from_state = apply_installation_row(
            &transaction,
            commit.activated,
            commit.activated_from_version,
            now_ms,
        )?;
        let appended = append_installation_event(
            &transaction,
            commit.activated,
            "activate",
            commit.activated_from_version,
            &activated_from_state,
            now_ms,
        );
        match appended {
            Ok(_) => {}
            Err(error) if is_unique_violation(&error) => return Err(StoreError::AlreadyConsumed),
            Err(error) => return Err(StoreError::Sqlite(error)),
        }

        let result = transaction.commit();
        match result {
            Ok(()) => Ok(()),
            Err(error) if is_unique_violation(&error) => Err(StoreError::AlreadyConsumed),
            Err(error) => Err(StoreError::Sqlite(error)),
        }
    }

    /// Commits a rollback: the installation is never active after this call
    /// (failure must not promote a row that never earned activation), and
    /// its stored `activation.catalogHash` becomes exactly the hash the
    /// caller computed from the checkpoint.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Inconsistent`] when the installation is active
    /// or the version guard fails.
    pub fn commit_bmad_installation_rollback(
        &self,
        installation: &BmadInstallation,
        from_version: u64,
        now_ms: u64,
    ) -> Result<(), StoreError> {
        if installation.activation.active {
            return Err(StoreError::Inconsistent);
        }
        let mut connection = self.connection.lock();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if event_already_recorded(&transaction, installation)? {
            return Ok(());
        }
        let from_state = apply_installation_row(&transaction, installation, from_version, now_ms)?;
        append_installation_event(
            &transaction,
            installation,
            "rollback",
            from_version,
            &from_state,
            now_ms,
        )
        .map_err(StoreError::Sqlite)?;
        transaction.commit()?;
        Ok(())
    }

    /// Recovers one installation exactly as it was last accepted, whether or
    /// not it is the package's currently active row. A superseded
    /// installation is never deleted, so its original manifest and
    /// destinations remain resolvable by id after an upgrade.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Inconsistent`] when the stored document does
    /// not parse or has drifted from its indexed version.
    pub fn load_bmad_installation(
        &self,
        installation_id: &str,
    ) -> Result<Option<BmadInstallationRecord>, StoreError> {
        let connection = self.connection.lock();
        let row = connection
            .query_row(
                "SELECT installation_json, workspace_id, active, version,
                        created_at_ms, updated_at_ms
                 FROM bmad_installations WHERE installation_id = ?1",
                params![installation_id],
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
        let Some((json, workspace_id, active, version, created_at_ms, updated_at_ms)) = row else {
            return Ok(None);
        };
        let installation: BmadInstallation = serde_json::from_str(&json)?;
        let indexed = u64::try_from(version).map_err(|_| StoreError::Inconsistent)?;
        if installation.version.get() != indexed || installation.activation.active != (active != 0)
        {
            return Err(StoreError::Inconsistent);
        }
        Ok(Some(BmadInstallationRecord {
            installation,
            workspace_id,
            active: active != 0,
            created_at_ms: u64::try_from(created_at_ms).map_err(|_| StoreError::Inconsistent)?,
            updated_at_ms: u64::try_from(updated_at_ms).map_err(|_| StoreError::Inconsistent)?,
        }))
    }

    /// Resolves the installation the catalog is currently permitted to
    /// serve for `package_id`, if any. This is the "is it served" query: a
    /// row that is merely `written` or `validated` never appears here,
    /// because it never reaches `active = 1`.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Inconsistent`] when the stored document does
    /// not parse or has drifted from its indexed version.
    pub fn active_bmad_installation_for_package(
        &self,
        package_id: &str,
    ) -> Result<Option<BmadInstallationRecord>, StoreError> {
        let installation_id = {
            let connection = self.connection.lock();
            connection
                .query_row(
                    "SELECT installation_id FROM bmad_installations
                     WHERE package_id = ?1 AND active = 1",
                    params![package_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
        };
        match installation_id {
            Some(installation_id) => self.load_bmad_installation(&installation_id),
            None => Ok(None),
        }
    }

    /// Returns the append-only transition log for one installation in
    /// order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Sqlite`] when the log cannot be read.
    pub fn bmad_installation_event_log(
        &self,
        installation_id: &str,
    ) -> Result<Vec<BmadInstallationEventRow>, StoreError> {
        let connection = self.connection.lock();
        let mut statement = connection.prepare(
            "SELECT from_version, to_version, command, from_state, to_state, installation_hash
             FROM bmad_installation_events WHERE installation_id = ?1 ORDER BY to_version",
        )?;
        let rows = statement
            .query_map(params![installation_id], |row| {
                Ok(BmadInstallationEventRow {
                    from_version: row.get::<_, i64>(0)?,
                    to_version: row.get::<_, i64>(1)?,
                    command: row.get(2)?,
                    from_state: row.get(3)?,
                    to_state: row.get(4)?,
                    installation_hash: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

/// Updates one installation row under the version guard and returns the
/// state it held immediately before the update, so the caller can record an
/// honest `from_state` on the event rather than a placeholder.
fn apply_installation_row(
    transaction: &rusqlite::Transaction<'_>,
    installation: &BmadInstallation,
    from_version: u64,
    now_ms: u64,
) -> Result<String, StoreError> {
    let previous_state: String = transaction
        .query_row(
            "SELECT state FROM bmad_installations WHERE installation_id = ?1 AND version = ?2",
            params![
                installation.installation_id.as_str(),
                i64::try_from(from_version).map_err(|_| StoreError::Inconsistent)?,
            ],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(StoreError::Inconsistent)?;
    let json = serde_json::to_string(installation)?;
    let updated = transaction.execute(
        "UPDATE bmad_installations
         SET state = ?1, version = ?2, active = ?3, installation_hash = ?4,
             installation_json = ?5, updated_at_ms = ?6
         WHERE installation_id = ?7 AND version = ?8",
        params![
            installation.state.to_string(),
            i64::try_from(installation.version.get()).map_err(|_| StoreError::Inconsistent)?,
            i64::from(installation.activation.active),
            installation.installation_hash.as_str(),
            json,
            i64::try_from(now_ms).map_err(|_| StoreError::Inconsistent)?,
            installation.installation_id.as_str(),
            i64::try_from(from_version).map_err(|_| StoreError::Inconsistent)?,
        ],
    )?;
    if updated != 1 {
        return Err(StoreError::Inconsistent);
    }
    Ok(previous_state)
}

/// Whether `installation`'s current `version` was already recorded as an
/// event's `to_version` with the same `installation_hash` — a genuine
/// redelivery of the exact same accepted transition. Checked before
/// touching the row so a redelivered command converges without ever
/// attempting (and failing) the already-applied compare-and-swap.
///
/// A `to_version` that was already recorded with a *different* hash is not
/// a replay — it is two distinct writers racing for the same version, and
/// must be refused rather than silently treated as a no-op.
fn event_already_recorded(
    transaction: &rusqlite::Transaction<'_>,
    installation: &BmadInstallation,
) -> Result<bool, StoreError> {
    let existing: Option<String> = transaction
        .query_row(
            "SELECT installation_hash FROM bmad_installation_events
             WHERE installation_id = ?1 AND to_version = ?2",
            params![
                installation.installation_id.as_str(),
                i64::try_from(installation.version.get()).map_err(|_| StoreError::Inconsistent)?,
            ],
            |row| row.get(0),
        )
        .optional()?;
    match existing {
        None => Ok(false),
        Some(hash) if hash == installation.installation_hash.as_str() => Ok(true),
        Some(_) => Err(StoreError::AlreadyConsumed),
    }
}

/// Appends one transition event.
fn append_installation_event(
    transaction: &rusqlite::Transaction<'_>,
    installation: &BmadInstallation,
    command: &str,
    from_version: u64,
    from_state: &str,
    now_ms: u64,
) -> Result<usize, rusqlite::Error> {
    transaction.execute(
        "INSERT INTO bmad_installation_events
         (installation_id, to_version, from_version, command, from_state, to_state,
          installation_hash, recorded_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            installation.installation_id.as_str(),
            i64::try_from(installation.version.get()).unwrap_or(i64::MAX),
            i64::try_from(from_version).unwrap_or(i64::MAX),
            command,
            from_state,
            installation.state.to_string(),
            installation.installation_hash.as_str(),
            i64::try_from(now_ms).unwrap_or(i64::MAX),
        ],
    )
}
