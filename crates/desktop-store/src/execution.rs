//! Durable checkpoint, effect-journal, and execution-result storage for the
//! governed local edits engine.
//!
//! This module deliberately depends only on validated plain values so the
//! adapter-crate boundary stays intact: the composition root serializes the
//! engine's checkpoint, journal, and result types and this store enforces
//! labels, hashes, sizes, and the journal state machine before anything
//! becomes durable. Checkpoint content is retained as encrypted CAS data;
//! journal and result records stay inline like other authority rows.

use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

use super::{
    append_evidence_in_transaction, canonical_now, is_unique_violation, validate_label,
    validate_sha256, EvidenceAppend, LocalStore, PayloadRef, StoreError,
};

pub const EXECUTION_CHECKPOINT_KIND: &str = "execution_checkpoint";
pub const EXECUTION_CHECKPOINT_SCHEMA: &str = "sapphirus.local-checkpoint.v1";

/// Journal and result records are bounded metadata, never file content.
pub const MAX_EXECUTION_JOURNAL_BYTES: usize = 262_144;
pub const MAX_EXECUTION_RESULT_BYTES: usize = 262_144;
/// Checkpoint JSON retains full UTF-8 preimages below the governed patch
/// byte ceiling; the bound covers worst-case JSON escaping overhead.
pub const MAX_EXECUTION_CHECKPOINT_BYTES: usize = 8 * 1024 * 1024;

const OPEN_JOURNAL_STATES_SQL: &str = "state NOT IN ('completed', 'recovered')";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionCheckpointAppend {
    pub checkpoint_id: String,
    pub workspace_target_hash: String,
    pub candidate_hash: String,
    pub manifest_hash: String,
    pub entry_count: u32,
    pub checkpoint_json: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionCheckpointRow {
    pub checkpoint_id: String,
    pub workspace_target_hash: String,
    pub candidate_hash: String,
    pub manifest_hash: String,
    pub entry_count: u32,
    pub payload: PayloadRef,
    pub recorded_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectJournalUpsert {
    pub journal_id: String,
    pub execution_id: String,
    pub checkpoint_id: String,
    pub candidate_hash: String,
    pub spec_hash: String,
    pub consumption_hash: String,
    pub workspace_id: String,
    pub workspace_grant_epoch: u64,
    pub state: String,
    pub journal_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectJournalRow {
    pub journal_id: String,
    pub execution_id: String,
    pub checkpoint_id: String,
    pub candidate_hash: String,
    pub spec_hash: String,
    pub consumption_hash: String,
    pub workspace_id: String,
    pub workspace_grant_epoch: u64,
    pub state: String,
    pub journal_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionResultAppend {
    pub execution_id: String,
    pub journal_id: String,
    pub checkpoint_id: String,
    pub candidate_hash: String,
    pub spec_hash: String,
    pub consumption_hash: String,
    pub result_hash: String,
    pub result_json: String,
    pub file_count: u32,
    pub journal_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionResultRow {
    pub execution_id: String,
    pub journal_id: String,
    pub checkpoint_id: String,
    pub candidate_hash: String,
    pub spec_hash: String,
    pub consumption_hash: String,
    pub result_hash: String,
    pub result_json: String,
    pub file_count: u32,
    pub completed_at: String,
}

fn is_terminal_journal_state(state: &str) -> bool {
    matches!(state, "completed" | "recovered" | "manual_review")
}

fn is_known_journal_state(state: &str) -> bool {
    matches!(
        state,
        "prepared"
            | "checkpoint_durable"
            | "preconditions_verified"
            | "applying"
            | "effects_applied"
            | "postimages_verified"
            | "result_recorded"
            | "completed"
            | "recovery_required"
            | "restoring"
            | "recovered"
            | "manual_review"
    )
}

fn allowed_journal_transition(current: &str, next: &str) -> bool {
    if next == current {
        return !is_terminal_journal_state(current);
    }
    if next == "recovery_required" {
        return !is_terminal_journal_state(current);
    }
    match current {
        "prepared" => matches!(next, "checkpoint_durable" | "recovered"),
        "checkpoint_durable" => matches!(next, "preconditions_verified" | "recovered"),
        "preconditions_verified" => matches!(next, "applying" | "recovered"),
        "applying" => next == "effects_applied",
        "effects_applied" => next == "postimages_verified",
        "postimages_verified" => next == "result_recorded",
        "result_recorded" => next == "completed",
        "recovery_required" => matches!(next, "restoring" | "recovered" | "manual_review"),
        "restoring" => matches!(next, "recovered" | "manual_review"),
        _ => false,
    }
}

fn validate_journal_json(journal_json: &str) -> Result<(), StoreError> {
    if journal_json.len() > MAX_EXECUTION_JOURNAL_BYTES {
        return Err(StoreError::Inconsistent);
    }
    let parsed: serde_json::Value = serde_json::from_str(journal_json)?;
    if !parsed.is_object() {
        return Err(StoreError::Inconsistent);
    }
    Ok(())
}

fn validate_epoch(value: u64) -> Result<(), StoreError> {
    if value == 0 || value > 9_007_199_254_740_991 {
        return Err(StoreError::Inconsistent);
    }
    Ok(())
}

impl LocalStore {
    /// Persists an execution checkpoint before any governed file effect.
    ///
    /// The checkpoint content becomes encrypted CAS data; the relational row
    /// binds its identifiers and hashes for recovery and rollback lookups.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when validation fails, the checkpoint identifier
    /// or manifest hash already exists, or durable storage fails.
    pub fn persist_execution_checkpoint(
        &self,
        record: &ExecutionCheckpointAppend,
    ) -> Result<(), StoreError> {
        validate_label(&record.checkpoint_id)?;
        validate_sha256(&record.workspace_target_hash)?;
        validate_sha256(&record.candidate_hash)?;
        validate_sha256(&record.manifest_hash)?;
        if record.checkpoint_json.is_empty()
            || record.checkpoint_json.len() > MAX_EXECUTION_CHECKPOINT_BYTES
        {
            return Err(StoreError::Inconsistent);
        }
        let payload = self.put_payload(
            EXECUTION_CHECKPOINT_KIND,
            EXECUTION_CHECKPOINT_SCHEMA,
            &record.checkpoint_json,
        )?;
        let recorded_at = canonical_now();
        let result = self.connection.lock().execute(
            "INSERT INTO execution_checkpoints
             (checkpoint_id, workspace_target_hash, candidate_hash, manifest_hash,
              entry_count, state_content_hash, state_kind, state_schema_version, recorded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                record.checkpoint_id,
                record.workspace_target_hash,
                record.candidate_hash,
                record.manifest_hash,
                record.entry_count,
                payload.content_hash,
                payload.kind,
                payload.schema_version,
                recorded_at
            ],
        );
        match result {
            Ok(_) => Ok(()),
            Err(error) if is_unique_violation(&error) => Err(StoreError::StateConflict),
            Err(error) => Err(StoreError::Sqlite(error)),
        }
    }

    /// Creates a prepared effect journal and its creation evidence atomically.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when validation fails, the journal or execution
    /// identifier already exists, or the transaction cannot commit.
    pub fn create_effect_journal(
        &self,
        record: &EffectJournalUpsert,
        event: &EvidenceAppend,
    ) -> Result<(), StoreError> {
        validate_label(&record.journal_id)?;
        validate_label(&record.execution_id)?;
        validate_label(&record.checkpoint_id)?;
        validate_label(&record.workspace_id)?;
        validate_sha256(&record.candidate_hash)?;
        validate_sha256(&record.spec_hash)?;
        validate_sha256(&record.consumption_hash)?;
        validate_epoch(record.workspace_grant_epoch)?;
        if record.state != "prepared" {
            return Err(StoreError::StateConflict);
        }
        validate_journal_json(&record.journal_json)?;

        let now = canonical_now();
        let mut connection = self.connection.lock();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let inserted = transaction.execute(
            "INSERT INTO effect_journals
             (journal_id, execution_id, checkpoint_id, candidate_hash, spec_hash,
              consumption_hash, workspace_id, workspace_grant_epoch, state, journal_json,
              created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                record.journal_id,
                record.execution_id,
                record.checkpoint_id,
                record.candidate_hash,
                record.spec_hash,
                record.consumption_hash,
                record.workspace_id,
                record.workspace_grant_epoch,
                record.state,
                record.journal_json,
                now,
                now
            ],
        );
        if let Err(error) = inserted {
            return Err(if is_unique_violation(&error) {
                StoreError::StateConflict
            } else {
                StoreError::Sqlite(error)
            });
        }
        append_evidence_in_transaction(&transaction, event, &now)?;
        transaction.commit()?;
        Ok(())
    }

    /// Persists an effect-journal transition after validating the durable
    /// journal state machine.
    ///
    /// Recovery-significant transitions may carry an evidence event that is
    /// recorded in the same transaction.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the journal is absent, the transition is
    /// not allowed from the durable state, validation fails, or the
    /// transaction cannot commit.
    pub fn update_effect_journal(
        &self,
        journal_id: &str,
        state: &str,
        journal_json: &str,
        event: Option<&EvidenceAppend>,
    ) -> Result<(), StoreError> {
        validate_label(journal_id)?;
        if !is_known_journal_state(state) {
            return Err(StoreError::Inconsistent);
        }
        validate_journal_json(journal_json)?;

        let now = canonical_now();
        let mut connection = self.connection.lock();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current: Option<String> = transaction
            .query_row(
                "SELECT state FROM effect_journals WHERE journal_id = ?1",
                params![journal_id],
                |row| row.get(0),
            )
            .optional()?;
        let current = current.ok_or(StoreError::Inconsistent)?;
        if !allowed_journal_transition(&current, state) {
            return Err(StoreError::StateConflict);
        }
        transaction.execute(
            "UPDATE effect_journals
             SET state = ?2, journal_json = ?3, updated_at = ?4
             WHERE journal_id = ?1",
            params![journal_id, state, journal_json, now],
        )?;
        if let Some(event) = event {
            append_evidence_in_transaction(&transaction, event, &now)?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Atomically records an execution result, the `result_recorded` journal
    /// transition, and the result evidence event.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when validation fails, the journal is not in a
    /// state that allows result recording, a uniqueness invariant is violated,
    /// or the transaction cannot commit.
    pub fn record_execution_result(
        &self,
        record: &ExecutionResultAppend,
        event: &EvidenceAppend,
    ) -> Result<(), StoreError> {
        validate_label(&record.execution_id)?;
        validate_label(&record.journal_id)?;
        validate_label(&record.checkpoint_id)?;
        validate_sha256(&record.candidate_hash)?;
        validate_sha256(&record.spec_hash)?;
        validate_sha256(&record.consumption_hash)?;
        validate_sha256(&record.result_hash)?;
        if record.file_count == 0 {
            return Err(StoreError::Inconsistent);
        }
        if record.result_json.is_empty() || record.result_json.len() > MAX_EXECUTION_RESULT_BYTES {
            return Err(StoreError::Inconsistent);
        }
        let parsed: serde_json::Value = serde_json::from_str(&record.result_json)?;
        if !parsed.is_object() {
            return Err(StoreError::Inconsistent);
        }
        validate_journal_json(&record.journal_json)?;

        let now = canonical_now();
        let mut connection = self.connection.lock();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current: Option<String> = transaction
            .query_row(
                "SELECT state FROM effect_journals WHERE journal_id = ?1",
                params![record.journal_id],
                |row| row.get(0),
            )
            .optional()?;
        let current = current.ok_or(StoreError::Inconsistent)?;
        if !allowed_journal_transition(&current, "result_recorded") {
            return Err(StoreError::StateConflict);
        }
        transaction.execute(
            "UPDATE effect_journals
             SET state = 'result_recorded', journal_json = ?2, updated_at = ?3
             WHERE journal_id = ?1",
            params![record.journal_id, record.journal_json, now],
        )?;
        let inserted = transaction.execute(
            "INSERT INTO execution_results
             (execution_id, journal_id, checkpoint_id, candidate_hash, spec_hash,
              consumption_hash, result_hash, result_json, file_count, completed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                record.execution_id,
                record.journal_id,
                record.checkpoint_id,
                record.candidate_hash,
                record.spec_hash,
                record.consumption_hash,
                record.result_hash,
                record.result_json,
                record.file_count,
                now
            ],
        );
        if let Err(error) = inserted {
            return Err(if is_unique_violation(&error) {
                StoreError::AlreadyConsumed
            } else {
                StoreError::Sqlite(error)
            });
        }
        append_evidence_in_transaction(&transaction, event, &now)?;
        transaction.commit()?;
        Ok(())
    }

    /// Loads one durable effect journal.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the identifier is invalid or `SQLite`
    /// cannot complete the query.
    pub fn load_effect_journal(
        &self,
        journal_id: &str,
    ) -> Result<Option<EffectJournalRow>, StoreError> {
        validate_label(journal_id)?;
        self.connection
            .lock()
            .query_row(
                &format!(
                    "SELECT {EFFECT_JOURNAL_COLUMNS} FROM effect_journals WHERE journal_id = ?1"
                ),
                params![journal_id],
                effect_journal_from_row,
            )
            .optional()
            .map_err(StoreError::from)
    }

    /// Lists journals that have not durably completed or been recovered.
    ///
    /// The result includes `manual_review` journals so the host can surface
    /// them; the caller decides which states block new governed effects.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when `SQLite` cannot complete the query.
    pub fn list_open_effect_journals(&self) -> Result<Vec<EffectJournalRow>, StoreError> {
        let connection = self.connection.lock();
        let mut statement = connection.prepare(&format!(
            "SELECT {EFFECT_JOURNAL_COLUMNS} FROM effect_journals
             WHERE {OPEN_JOURNAL_STATES_SQL}
             ORDER BY created_at, journal_id"
        ))?;
        let rows = statement
            .query_map([], effect_journal_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Loads one execution checkpoint row and its decrypted content bytes.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the identifier is invalid, metadata is
    /// inconsistent, or the encrypted payload cannot be authenticated.
    pub fn load_execution_checkpoint(
        &self,
        checkpoint_id: &str,
    ) -> Result<Option<(ExecutionCheckpointRow, Vec<u8>)>, StoreError> {
        validate_label(checkpoint_id)?;
        let row = self
            .connection
            .lock()
            .query_row(
                "SELECT c.checkpoint_id, c.workspace_target_hash, c.candidate_hash,
                        c.manifest_hash, c.entry_count, c.state_content_hash, c.state_kind,
                        c.state_schema_version, c.recorded_at, p.byte_count, p.key_version
                 FROM execution_checkpoints c
                 JOIN payloads p
                   ON p.content_hash = c.state_content_hash
                  AND p.kind = c.state_kind
                  AND p.schema_version = c.state_schema_version
                 WHERE c.checkpoint_id = ?1",
                params![checkpoint_id],
                |row| {
                    Ok(ExecutionCheckpointRow {
                        checkpoint_id: row.get(0)?,
                        workspace_target_hash: row.get(1)?,
                        candidate_hash: row.get(2)?,
                        manifest_hash: row.get(3)?,
                        entry_count: row.get(4)?,
                        payload: PayloadRef {
                            content_hash: row.get(5)?,
                            kind: row.get(6)?,
                            schema_version: row.get(7)?,
                            byte_count: row.get(9)?,
                            key_version: row.get(10)?,
                        },
                        recorded_at: row.get(8)?,
                    })
                },
            )
            .optional()?;
        match row {
            None => Ok(None),
            Some(row) => {
                let bytes = self.get_payload(&row.payload)?;
                Ok(Some((row, bytes)))
            }
        }
    }

    /// Loads one durable execution result.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the identifier is invalid or `SQLite`
    /// cannot complete the query.
    pub fn load_execution_result(
        &self,
        execution_id: &str,
    ) -> Result<Option<ExecutionResultRow>, StoreError> {
        validate_label(execution_id)?;
        self.connection
            .lock()
            .query_row(
                &format!(
                    "SELECT {EXECUTION_RESULT_COLUMNS} FROM execution_results
                     WHERE execution_id = ?1"
                ),
                params![execution_id],
                execution_result_from_row,
            )
            .optional()
            .map_err(StoreError::from)
    }

    /// Lists the most recent durable execution results, newest first.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when `SQLite` cannot complete the query.
    pub fn list_recent_execution_results(
        &self,
        limit: u32,
    ) -> Result<Vec<ExecutionResultRow>, StoreError> {
        let limit = limit.clamp(1, 256);
        let connection = self.connection.lock();
        let mut statement = connection.prepare(&format!(
            "SELECT {EXECUTION_RESULT_COLUMNS} FROM execution_results
             ORDER BY rowid DESC LIMIT ?1"
        ))?;
        let rows = statement
            .query_map(params![limit], execution_result_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Returns the newest completed checkpoint for one exact workspace grant.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the workspace identifier is invalid or
    /// `SQLite` cannot complete the query.
    pub fn latest_completed_checkpoint(
        &self,
        workspace_id: &str,
        workspace_grant_epoch: u64,
    ) -> Result<Option<String>, StoreError> {
        validate_label(workspace_id)?;
        self.connection
            .lock()
            .query_row(
                "SELECT r.checkpoint_id
                 FROM execution_results r
                 JOIN effect_journals j ON j.journal_id = r.journal_id
                 WHERE j.workspace_id = ?1
                   AND j.workspace_grant_epoch = ?2
                   AND j.state = 'completed'
                 ORDER BY r.rowid DESC
                 LIMIT 1",
                params![workspace_id, workspace_grant_epoch],
                |row| row.get(0),
            )
            .optional()
            .map_err(StoreError::from)
    }

    pub(crate) fn verify_execution_integrity(&self) -> Result<(), StoreError> {
        let connection = self.connection.lock();
        let mut statement = connection.prepare(
            "SELECT journal_id, state, journal_json,
                    EXISTS(SELECT 1 FROM execution_results r WHERE r.journal_id = j.journal_id)
             FROM effect_journals j",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, bool>(3)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (journal_id, state, journal_json, has_result) in rows {
            validate_label(&journal_id)?;
            if !is_known_journal_state(&state) {
                return Err(StoreError::Inconsistent);
            }
            validate_journal_json(&journal_json)?;
            let requires_result = matches!(state.as_str(), "result_recorded" | "completed");
            if requires_result && !has_result {
                return Err(StoreError::Inconsistent);
            }
        }
        Ok(())
    }
}

const EFFECT_JOURNAL_COLUMNS: &str = "journal_id, execution_id, checkpoint_id, candidate_hash, \
     spec_hash, consumption_hash, workspace_id, workspace_grant_epoch, state, journal_json, \
     created_at, updated_at";

const EXECUTION_RESULT_COLUMNS: &str = "execution_id, journal_id, checkpoint_id, candidate_hash, \
     spec_hash, consumption_hash, result_hash, result_json, file_count, completed_at";

fn effect_journal_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EffectJournalRow> {
    Ok(EffectJournalRow {
        journal_id: row.get(0)?,
        execution_id: row.get(1)?,
        checkpoint_id: row.get(2)?,
        candidate_hash: row.get(3)?,
        spec_hash: row.get(4)?,
        consumption_hash: row.get(5)?,
        workspace_id: row.get(6)?,
        workspace_grant_epoch: row.get(7)?,
        state: row.get(8)?,
        journal_json: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

fn execution_result_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ExecutionResultRow> {
    Ok(ExecutionResultRow {
        execution_id: row.get(0)?,
        journal_id: row.get(1)?,
        checkpoint_id: row.get(2)?,
        candidate_hash: row.get(3)?,
        spec_hash: row.get(4)?,
        consumption_hash: row.get(5)?,
        result_hash: row.get(6)?,
        result_json: row.get(7)?,
        file_count: row.get(8)?,
        completed_at: row.get(9)?,
    })
}

#[cfg(test)]
mod tests {
    use super::{allowed_journal_transition, EffectJournalUpsert, ExecutionCheckpointAppend};
    use crate::{EvidenceAppend, KeyProtector, LocalStore, StoreError};

    const TEST_HASH: &str =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn unique_test_hash(value: usize) -> String {
        format!("sha256:{value:064x}")
    }

    #[derive(Debug)]
    struct TestProtector;

    impl KeyProtector for TestProtector {
        fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, StoreError> {
            Ok(plaintext.iter().map(|byte| byte ^ 0xA5).collect())
        }

        fn unprotect(&self, protected: &[u8]) -> Result<Vec<u8>, StoreError> {
            Ok(protected.iter().map(|byte| byte ^ 0xA5).collect())
        }
    }

    fn seed_open_journal(store: &LocalStore, index: usize, state: &str) -> Result<(), StoreError> {
        let journal_id = format!("journal_open_{index}");
        let execution_id = format!("execution_open_{index}");
        let checkpoint_id = format!("checkpoint_open_{index}");
        let candidate_hash = unique_test_hash(index * 10 + 1);
        store.persist_execution_checkpoint(&ExecutionCheckpointAppend {
            checkpoint_id: checkpoint_id.clone(),
            workspace_target_hash: unique_test_hash(index * 10 + 2),
            candidate_hash: candidate_hash.clone(),
            manifest_hash: unique_test_hash(index * 10 + 3),
            entry_count: 0,
            checkpoint_json: br#"{}"#.to_vec(),
        })?;
        store.create_effect_journal(
            &EffectJournalUpsert {
                journal_id: journal_id.clone(),
                execution_id: execution_id.clone(),
                checkpoint_id,
                candidate_hash,
                spec_hash: unique_test_hash(index * 10 + 4),
                consumption_hash: unique_test_hash(index * 10 + 5),
                workspace_id: "workspace_open".to_owned(),
                workspace_grant_epoch: 1,
                state: "prepared".to_owned(),
                journal_json: "{}".to_owned(),
            },
            &EvidenceAppend {
                stream_id: format!("execution:{execution_id}"),
                event_type: "execution.journal-created".to_owned(),
                payload_hash: TEST_HASH.to_owned(),
                payload_ref: None,
                correlation_id: "test_open_journals".to_owned(),
                causation_id: None,
                redaction_level: "metadata".to_owned(),
                retention_class: "evidence".to_owned(),
            },
        )?;
        store.update_effect_journal(&journal_id, "recovery_required", "{}", None)?;
        if state != "recovery_required" {
            store.update_effect_journal(&journal_id, state, "{}", None)?;
        }
        Ok(())
    }

    #[test]
    fn happy_path_transitions_are_allowed_in_order() {
        let states = [
            "prepared",
            "checkpoint_durable",
            "preconditions_verified",
            "applying",
            "effects_applied",
            "postimages_verified",
            "result_recorded",
            "completed",
        ];
        for pair in states.windows(2) {
            assert!(
                allowed_journal_transition(pair[0], pair[1]),
                "{} -> {} must be allowed",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn terminal_states_accept_no_transition() {
        for terminal in ["completed", "recovered", "manual_review"] {
            for next in ["prepared", "applying", "recovery_required", terminal] {
                assert!(
                    !allowed_journal_transition(terminal, next),
                    "{terminal} -> {next} must be rejected"
                );
            }
        }
    }

    #[test]
    fn recovery_transitions_are_bounded() {
        assert!(allowed_journal_transition("applying", "recovery_required"));
        assert!(allowed_journal_transition("recovery_required", "restoring"));
        assert!(allowed_journal_transition("recovery_required", "recovered"));
        assert!(allowed_journal_transition(
            "recovery_required",
            "manual_review"
        ));
        assert!(allowed_journal_transition("restoring", "recovered"));
        assert!(allowed_journal_transition("restoring", "manual_review"));
        assert!(!allowed_journal_transition("manual_review", "restoring"));
        assert!(allowed_journal_transition("prepared", "recovered"));
        assert!(!allowed_journal_transition("applying", "completed"));
        assert!(!allowed_journal_transition(
            "recovery_required",
            "completed"
        ));
        assert!(!allowed_journal_transition("recovered", "restoring"));
    }

    #[test]
    fn unresolved_recovery_journals_remain_open() -> Result<(), StoreError> {
        let directory = tempfile::tempdir()?;
        let store = LocalStore::open(directory.path(), &TestProtector)?;
        for (index, state) in ["recovery_required", "restoring", "manual_review"]
            .into_iter()
            .enumerate()
        {
            seed_open_journal(&store, index, state)?;
        }

        let states = store
            .list_open_effect_journals()?
            .into_iter()
            .map(|journal| journal.state)
            .collect::<Vec<_>>();
        assert_eq!(states, ["recovery_required", "restoring", "manual_review"]);
        Ok(())
    }
}
