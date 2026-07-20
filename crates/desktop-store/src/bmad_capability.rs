//! Durable persistence for generic sealed BMAD capability runs (Task 5).
//!
//! Runs are immutable rows; each run records at most one terminal result,
//! stored as an encrypted content-addressed payload whose schema version is
//! the run's declared archetype schema. An archetype substitution — a
//! result whose kind differs from the run's declared output schema — fails
//! closed before any payload is written.

use desktop_runtime::{canonical_json_bytes, BmadCapabilityOutput, BmadCapabilityRun, ContractId};
use rusqlite::{params, OptionalExtension, TransactionBehavior};

use super::{canonical_now, is_unique_violation, LocalStore, StoreError};

const CAPABILITY_RESULT_PAYLOAD_KIND: &str = "bmad_capability_result";

/// One stored capability run with its optional terminal result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadCapabilityRunRecord {
    pub run_id: String,
    pub capability_id: String,
    pub workspace_id: String,
    pub instruction_hash: String,
    pub context_manifest_hash: String,
    pub output_schema_id: String,
    pub consent_evidence_id: String,
    pub created_at_ms: u64,
    pub result_kind: Option<String>,
    pub result_json: Option<String>,
}

impl LocalStore {
    /// Durably opens one capability run. The run must not yet carry a
    /// result; results are recorded separately so the archetype check and
    /// payload write happen in one place.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::AlreadyConsumed`] for a duplicate run or
    /// consent evidence identifier and [`StoreError::Inconsistent`] when
    /// the run already carries a result.
    pub fn create_bmad_capability_run(&self, run: &BmadCapabilityRun) -> Result<(), StoreError> {
        if run.result.is_some() {
            return Err(StoreError::Inconsistent);
        }
        let connection = self.connection.lock();
        let result = connection.execute(
            "INSERT INTO bmad_capability_runs
             (run_id, capability_id, workspace_id, instruction_hash,
              context_manifest_hash, output_schema_id, consent_evidence_id,
              created_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                run.run_id.as_str(),
                run.capability_id.as_str(),
                run.workspace_id.as_str(),
                run.instruction_hash.to_string(),
                run.context_manifest_hash.to_string(),
                run.output_schema_id,
                run.consent_evidence_id.as_str(),
                i64::try_from(run.created_at.0).map_err(|_| StoreError::Inconsistent)?,
            ],
        );
        match result {
            Ok(_) => Ok(()),
            Err(error) if is_unique_violation(&error) => Err(StoreError::AlreadyConsumed),
            Err(error) => Err(StoreError::Sqlite(error)),
        }
    }

    /// Records the single terminal result for one run.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Inconsistent`] for an unknown run or when
    /// the result archetype does not match the run's declared output
    /// schema, and
    /// [`StoreError::AlreadyConsumed`] when a result already exists.
    pub fn record_bmad_capability_result(
        &self,
        run_id: &ContractId,
        output: &BmadCapabilityOutput,
    ) -> Result<(), StoreError> {
        let declared: Option<String> = {
            let connection = self.connection.lock();
            connection
                .query_row(
                    "SELECT output_schema_id FROM bmad_capability_runs WHERE run_id = ?1",
                    params![run_id.as_str()],
                    |row| row.get(0),
                )
                .optional()?
        };
        let declared = declared.ok_or(StoreError::Inconsistent)?;
        if output.schema_id() != declared {
            return Err(StoreError::Inconsistent);
        }
        let result_kind = match output {
            BmadCapabilityOutput::DocumentArtifact(_) => "document_artifact",
            BmadCapabilityOutput::GovernedChangeSet(_) => "governed_change_set",
            BmadCapabilityOutput::InactiveBuilderDraft(_) => "inactive_builder_draft",
        };
        let serialized = canonical_json_bytes(output).map_err(|_| StoreError::Inconsistent)?;
        let payload = self.put_payload(
            CAPABILITY_RESULT_PAYLOAD_KIND,
            output.schema_id(),
            &serialized,
        )?;
        let mut connection = self.connection.lock();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let inserted = transaction.execute(
            "INSERT INTO bmad_capability_results
             (run_id, result_kind, result_content_hash, result_payload_kind,
              result_schema_version, recorded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                run_id.as_str(),
                result_kind,
                payload.content_hash,
                CAPABILITY_RESULT_PAYLOAD_KIND,
                payload.schema_version,
                canonical_now(),
            ],
        );
        match inserted {
            Ok(_) => {
                transaction.commit()?;
                Ok(())
            }
            Err(error) if is_unique_violation(&error) => Err(StoreError::AlreadyConsumed),
            Err(error) => Err(StoreError::Sqlite(error)),
        }
    }

    /// Returns the newest capability run for one workspace and capability,
    /// with its decrypted result when recorded.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot be read or the
    /// result payload fails authenticated decryption.
    pub fn latest_bmad_capability_run(
        &self,
        workspace_id: &ContractId,
        capability_id: &str,
    ) -> Result<Option<BmadCapabilityRunRecord>, StoreError> {
        let run_id: Option<String> = {
            let connection = self.connection.lock();
            connection
                .query_row(
                    "SELECT run_id FROM bmad_capability_runs
                     WHERE workspace_id = ?1 AND capability_id = ?2
                     ORDER BY created_at_ms DESC, run_id DESC LIMIT 1",
                    params![workspace_id.as_str(), capability_id],
                    |row| row.get(0),
                )
                .optional()?
        };
        match run_id {
            Some(run_id) => {
                let run_id = ContractId::new(run_id).map_err(|_| StoreError::Inconsistent)?;
                self.bmad_capability_run(&run_id)
            }
            None => Ok(None),
        }
    }

    /// Loads one capability run and, when present, its decrypted result.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot be read or the
    /// result payload fails authenticated decryption.
    pub fn bmad_capability_run(
        &self,
        run_id: &ContractId,
    ) -> Result<Option<BmadCapabilityRunRecord>, StoreError> {
        let row = {
            let connection = self.connection.lock();
            connection
                .query_row(
                    "SELECT r.run_id, r.capability_id, r.workspace_id,
                            r.instruction_hash, r.context_manifest_hash,
                            r.output_schema_id, r.consent_evidence_id,
                            r.created_at_ms, o.result_kind, o.result_content_hash,
                            o.result_schema_version
                     FROM bmad_capability_runs r
                     LEFT JOIN bmad_capability_results o ON o.run_id = r.run_id
                     WHERE r.run_id = ?1",
                    params![run_id.as_str()],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, String>(4)?,
                            row.get::<_, String>(5)?,
                            row.get::<_, String>(6)?,
                            row.get::<_, i64>(7)?,
                            row.get::<_, Option<String>>(8)?,
                            row.get::<_, Option<String>>(9)?,
                            row.get::<_, Option<String>>(10)?,
                        ))
                    },
                )
                .optional()?
        };
        let Some((
            run_id,
            capability_id,
            workspace_id,
            instruction_hash,
            context_manifest_hash,
            output_schema_id,
            consent_evidence_id,
            created_at_ms,
            result_kind,
            result_content_hash,
            result_schema_version,
        )) = row
        else {
            return Ok(None);
        };
        let result_json = match (&result_content_hash, &result_schema_version) {
            (Some(content_hash), Some(schema_version)) => {
                let stored: Option<(u64, u32)> = {
                    let connection = self.connection.lock();
                    connection
                        .query_row(
                            "SELECT byte_count, key_version FROM payloads
                             WHERE content_hash = ?1 AND kind = ?2
                               AND schema_version = ?3",
                            params![content_hash, CAPABILITY_RESULT_PAYLOAD_KIND, schema_version],
                            |row| Ok((row.get(0)?, row.get(1)?)),
                        )
                        .optional()?
                };
                let (byte_count, key_version) = stored.ok_or(StoreError::Inconsistent)?;
                let reference = super::PayloadRef {
                    content_hash: content_hash.clone(),
                    kind: CAPABILITY_RESULT_PAYLOAD_KIND.to_owned(),
                    schema_version: schema_version.clone(),
                    byte_count,
                    key_version,
                };
                let bytes = self.get_payload(&reference)?;
                Some(String::from_utf8(bytes).map_err(|_| StoreError::Inconsistent)?)
            }
            _ => None,
        };
        Ok(Some(BmadCapabilityRunRecord {
            run_id,
            capability_id,
            workspace_id,
            instruction_hash,
            context_manifest_hash,
            output_schema_id,
            consent_evidence_id,
            created_at_ms: u64::try_from(created_at_ms).map_err(|_| StoreError::Inconsistent)?,
            result_kind,
            result_json,
        }))
    }
}
