//! Durable model catalog snapshots and model selection history (FND-041).
//!
//! Two things survive a restart, and neither of them is a catalog.
//!
//! * **The snapshot** is metadata about the last catalog this desktop verified:
//!   its version, its content hash, its freshness, how many profiles it offered,
//!   and the instants around it. There is no profile row, no display text, and
//!   no cache validator. A restarted desktop can therefore tell the user what it
//!   last saw and render it as stale while it refetches, but it cannot rebuild a
//!   catalog from local state, and it has nothing with which to authorize a
//!   request. That is deliberate: a cached catalog that could authorize on its
//!   own is precisely the failure this shape makes unrepresentable.
//!
//! * **The selections** are the user's own choices. A row carries the stable
//!   profile and variant identifiers, the identity of the catalog the choice was
//!   made from, and when. Nothing else. There is no column that could hold a
//!   deployment name, an endpoint, a subscription, or a credential, so a future
//!   write cannot introduce one without a migration and a review.
//!
//! History is preserved rather than overwritten. When the selected profile
//! disappears the host clears *current* and keeps the row, which is what lets
//! the desktop offer the next best prior choice and explain what happened
//! instead of silently switching the model a user is mid-draft with.

use rusqlite::{params, OptionalExtension, TransactionBehavior};

use super::{LocalStore, StoreError};

/// How many selections one workspace keeps.
///
/// The history exists to answer "what else did you use here recently", which a
/// person answers from the last handful of choices, not from a year of them. An
/// unbounded table would grow for the life of the install to hold rows no
/// surface ever shows.
pub const MAX_RECENT_MODEL_SELECTIONS: usize = 8;

/// Metadata of the last catalog one workspace verified for one subject.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredCatalogSnapshot {
    pub workspace_id: String,
    pub subject_hash: String,
    pub catalog_version: String,
    pub catalog_hash: String,
    /// `fresh` or `stale_usable`, mirroring the catalog's own vocabulary.
    pub catalog_state: String,
    pub profile_count: u32,
    pub default_profile_id: Option<String>,
    pub generated_at_ms: u64,
    pub expires_at_ms: u64,
    /// When this desktop last verified the catalog this row describes.
    pub verified_at_ms: u64,
}

/// One persisted model selection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredModelSelection {
    pub profile_id: String,
    pub variant_id: String,
    pub catalog_version: String,
    pub catalog_hash: String,
    pub selected_at_ms: u64,
    /// Whether this is the workspace's current selection. At most one row per
    /// workspace may be current, enforced by a partial unique index.
    pub is_current: bool,
}

/// The monotonic epoch of a catalog version (`v<epoch>.<digest>`).
///
/// Returns `None` for anything that is not shaped like a catalog version, which
/// callers treat as a rejected write rather than as epoch zero.
fn catalog_epoch(catalog_version: &str) -> Option<u64> {
    let rest = catalog_version.strip_prefix('v')?;
    let (epoch, digest) = rest.split_once('.')?;
    if epoch.is_empty()
        || epoch.starts_with('0')
        || !epoch.bytes().all(|byte| byte.is_ascii_digit())
        || digest.len() != 16
        || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return None;
    }
    epoch.parse().ok()
}

fn stored_millis(value: u64) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| StoreError::Inconsistent)
}

fn read_millis(value: i64) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::Inconsistent)
}

impl LocalStore {
    /// Records the metadata of a catalog this desktop has just verified.
    ///
    /// A catalog epoch never moves backwards. Replaying an older catalog over a
    /// newer one, or replaying the same epoch carrying a different content hash,
    /// is rejected rather than stored: both are how a downgrade or a transplant
    /// would reach durable state, and a desktop that accepted either would keep
    /// telling the user about a catalog the support plane has already replaced.
    /// Re-recording the identical epoch and hash is idempotent and only advances
    /// `verified_at_ms`.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Inconsistent`] for a malformed catalog version, a
    /// downgrade, a same-epoch hash change, or a value the schema refuses.
    pub fn record_model_catalog_snapshot(
        &self,
        snapshot: &StoredCatalogSnapshot,
    ) -> Result<(), StoreError> {
        let incoming_epoch =
            catalog_epoch(&snapshot.catalog_version).ok_or(StoreError::Inconsistent)?;
        let mut connection = self.connection.lock();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: Option<(String, String)> = transaction
            .query_row(
                "SELECT catalog_version, catalog_hash FROM model_catalog_snapshots
                 WHERE workspace_id = ?1 AND subject_hash = ?2",
                params![
                    snapshot.workspace_id.as_str(),
                    snapshot.subject_hash.as_str()
                ],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((existing_version, existing_hash)) = existing {
            let existing_epoch =
                catalog_epoch(&existing_version).ok_or(StoreError::Inconsistent)?;
            if incoming_epoch < existing_epoch {
                return Err(StoreError::Inconsistent);
            }
            if incoming_epoch == existing_epoch && existing_hash != snapshot.catalog_hash {
                return Err(StoreError::Inconsistent);
            }
        }
        transaction.execute(
            "INSERT INTO model_catalog_snapshots
             (workspace_id, subject_hash, catalog_version, catalog_hash, catalog_state,
              profile_count, default_profile_id, generated_at_ms, expires_at_ms, verified_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(workspace_id, subject_hash) DO UPDATE SET
               catalog_version = excluded.catalog_version,
               catalog_hash = excluded.catalog_hash,
               catalog_state = excluded.catalog_state,
               profile_count = excluded.profile_count,
               default_profile_id = excluded.default_profile_id,
               generated_at_ms = excluded.generated_at_ms,
               expires_at_ms = excluded.expires_at_ms,
               verified_at_ms = excluded.verified_at_ms",
            params![
                snapshot.workspace_id.as_str(),
                snapshot.subject_hash.as_str(),
                snapshot.catalog_version.as_str(),
                snapshot.catalog_hash.as_str(),
                snapshot.catalog_state.as_str(),
                snapshot.profile_count,
                snapshot.default_profile_id.as_deref(),
                stored_millis(snapshot.generated_at_ms)?,
                stored_millis(snapshot.expires_at_ms)?,
                stored_millis(snapshot.verified_at_ms)?,
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// The last verified catalog metadata for one workspace and subject.
    ///
    /// What comes back is never a catalog and never authority. It is what the
    /// desktop shows while it refetches.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot be read or a stored
    /// instant is out of range.
    pub fn model_catalog_snapshot(
        &self,
        workspace_id: &str,
        subject_hash: &str,
    ) -> Result<Option<StoredCatalogSnapshot>, StoreError> {
        let row = {
            let connection = self.connection.lock();
            connection
                .query_row(
                    "SELECT catalog_version, catalog_hash, catalog_state, profile_count,
                            default_profile_id, generated_at_ms, expires_at_ms, verified_at_ms
                     FROM model_catalog_snapshots
                     WHERE workspace_id = ?1 AND subject_hash = ?2",
                    params![workspace_id, subject_hash],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, i64>(3)?,
                            row.get::<_, Option<String>>(4)?,
                            row.get::<_, i64>(5)?,
                            row.get::<_, i64>(6)?,
                            row.get::<_, i64>(7)?,
                        ))
                    },
                )
                .optional()?
        };
        let Some((
            catalog_version,
            catalog_hash,
            catalog_state,
            profile_count,
            default_profile_id,
            generated_at_ms,
            expires_at_ms,
            verified_at_ms,
        )) = row
        else {
            return Ok(None);
        };
        Ok(Some(StoredCatalogSnapshot {
            workspace_id: workspace_id.to_owned(),
            subject_hash: subject_hash.to_owned(),
            catalog_version,
            catalog_hash,
            catalog_state,
            profile_count: u32::try_from(profile_count).map_err(|_| StoreError::Inconsistent)?,
            default_profile_id,
            generated_at_ms: read_millis(generated_at_ms)?,
            expires_at_ms: read_millis(expires_at_ms)?,
            verified_at_ms: read_millis(verified_at_ms)?,
        }))
    }

    /// Records one workspace's current model selection.
    ///
    /// The previous current row is demoted rather than deleted, so it stays
    /// available as history. The write is one transaction: a crash can never
    /// leave a workspace with two current selections or with none while the user
    /// believed a choice had been saved.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Inconsistent`] for a malformed catalog version or a
    /// value the schema refuses.
    pub fn record_model_selection(
        &self,
        workspace_id: &str,
        selection: &StoredModelSelection,
        recorded_at_ms: u64,
    ) -> Result<(), StoreError> {
        if catalog_epoch(&selection.catalog_version).is_none() {
            return Err(StoreError::Inconsistent);
        }
        let selected_at = stored_millis(selection.selected_at_ms)?;
        let recorded_at = stored_millis(recorded_at_ms)?;
        let mut connection = self.connection.lock();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        // Demote first: the partial unique index admits at most one current row
        // per workspace, so the promotion below would otherwise conflict with
        // whatever was current before.
        transaction.execute(
            "UPDATE model_profile_selections SET is_current = 0 WHERE workspace_id = ?1",
            params![workspace_id],
        )?;
        transaction.execute(
            "INSERT INTO model_profile_selections
             (workspace_id, profile_id, variant_id, catalog_version, catalog_hash,
              selected_at_ms, recorded_at_ms, is_current)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(workspace_id, profile_id, variant_id) DO UPDATE SET
               catalog_version = excluded.catalog_version,
               catalog_hash = excluded.catalog_hash,
               selected_at_ms = excluded.selected_at_ms,
               recorded_at_ms = excluded.recorded_at_ms,
               is_current = excluded.is_current",
            params![
                workspace_id,
                selection.profile_id.as_str(),
                selection.variant_id.as_str(),
                selection.catalog_version.as_str(),
                selection.catalog_hash.as_str(),
                selected_at,
                recorded_at,
                i64::from(selection.is_current),
            ],
        )?;
        prune_selections(&transaction, workspace_id)?;
        transaction.commit()?;
        Ok(())
    }

    /// Clears one workspace's current selection while keeping the history.
    ///
    /// This is what the host calls when the selected profile has disappeared or
    /// become unavailable: the choice stops being current, the draft is
    /// untouched, and the row remains so the user can be told what they had and
    /// offered it again if it returns.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot be written.
    pub fn clear_current_model_selection(&self, workspace_id: &str) -> Result<(), StoreError> {
        let connection = self.connection.lock();
        connection.execute(
            "UPDATE model_profile_selections SET is_current = 0 WHERE workspace_id = ?1",
            params![workspace_id],
        )?;
        Ok(())
    }

    /// One workspace's current selection, if it still has one.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot be read or a stored
    /// instant is out of range.
    pub fn current_model_selection(
        &self,
        workspace_id: &str,
    ) -> Result<Option<StoredModelSelection>, StoreError> {
        Ok(self
            .recent_model_selections(workspace_id)?
            .into_iter()
            .find(|selection| selection.is_current))
    }

    /// One workspace's selection history, newest first.
    ///
    /// Identifier tiebreakers follow the timestamp so two selections recorded in
    /// the same millisecond always read back in the same order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot be read or a stored
    /// instant is out of range.
    pub fn recent_model_selections(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<StoredModelSelection>, StoreError> {
        let connection = self.connection.lock();
        let mut statement = connection.prepare(
            "SELECT profile_id, variant_id, catalog_version, catalog_hash,
                    selected_at_ms, is_current
             FROM model_profile_selections
             WHERE workspace_id = ?1
             ORDER BY selected_at_ms DESC, profile_id ASC, variant_id ASC",
        )?;
        let rows = statement
            .query_map(params![workspace_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(
                |(profile_id, variant_id, catalog_version, catalog_hash, selected_at, current)| {
                    Ok(StoredModelSelection {
                        profile_id,
                        variant_id,
                        catalog_version,
                        catalog_hash,
                        selected_at_ms: read_millis(selected_at)?,
                        is_current: current == 1,
                    })
                },
            )
            .collect()
    }
}

/// Trims one workspace's selection history to [`MAX_RECENT_MODEL_SELECTIONS`].
///
/// The current selection is never a pruning candidate. Dropping it would make
/// the workspace forget which model is in use as a side effect of the user
/// trying several — the same silent switch the whole lane exists to prevent.
fn prune_selections(
    transaction: &rusqlite::Transaction<'_>,
    workspace_id: &str,
) -> Result<(), StoreError> {
    transaction.execute(
        "DELETE FROM model_profile_selections
         WHERE workspace_id = ?1
           AND is_current = 0
           AND (profile_id, variant_id) NOT IN (
             SELECT profile_id, variant_id FROM model_profile_selections
             WHERE workspace_id = ?1
             ORDER BY is_current DESC, selected_at_ms DESC, profile_id ASC, variant_id ASC
             LIMIT ?2
           )",
        params![
            workspace_id,
            i64::try_from(MAX_RECENT_MODEL_SELECTIONS).map_err(|_| StoreError::Inconsistent)?
        ],
    )?;
    Ok(())
}
