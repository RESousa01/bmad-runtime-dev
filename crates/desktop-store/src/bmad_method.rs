use std::collections::{BTreeMap, BTreeSet};

use desktop_runtime::{
    canonical_hash, canonical_json_bytes, ContractId, MethodAdvanceDisposition,
    MethodAdvanceReceipt, MethodAdvanceRequest, MethodArtifactExpectation,
    MethodArtifactProvenance, MethodError, MethodErrorCode, MethodEvidenceClass,
    MethodExactBinding, MethodPersistenceEvent, MethodSession, MethodSessionRepository,
    MethodSessionScope,
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

use super::{
    append_evidence_in_transaction, canonical_now, is_unique_violation, EvidenceAppend, LocalStore,
    PayloadRef, StoreError,
};

const METHOD_STATE_KIND: &str = "bmad_method_session";
const METHOD_STATE_SCHEMA: &str = "sapphirus.bmad-method-session-state.v1";
const METHOD_ARTIFACT_KIND: &str = "bmad_method_artifact";
const METHOD_ARTIFACT_SCHEMA: &str = "sapphirus.bmad-method-artifact.v1";
type CheckpointKey = (String, u64);
type CheckpointIdentity = (String, String);
type ExpectedCheckpoints = BTreeMap<CheckpointKey, CheckpointIdentity>;

#[derive(Debug)]
struct StoredMethodStateRef {
    version: u64,
    state: String,
    payload: PayloadRef,
}

#[derive(Debug)]
struct MethodIntegrityRow {
    session_id: String,
    owner_scope_ref: String,
    project_id: String,
    run_id: String,
    authority_id: String,
    version: u64,
    state: String,
    payload: PayloadRef,
}

#[derive(Debug)]
struct CheckpointIntegrityRow {
    checkpoint_id: String,
    session_id: String,
    turn_ordinal: u64,
    checkpoint_hash: String,
    payload: PayloadRef,
}

#[derive(Debug)]
struct ReceiptIntegrityRow {
    consumption_id: String,
    decision_id: String,
    invocation_id: String,
    idempotency_key: String,
    source: String,
    expected_hash: String,
}

#[derive(Debug)]
struct MethodIntegritySnapshot {
    sessions: Vec<MethodIntegrityRow>,
    checkpoints: Vec<CheckpointIntegrityRow>,
    receipts: Vec<ReceiptIntegrityRow>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredMethodArtifact {
    provenance: MethodArtifactProvenance,
    expectation_id: ContractId,
    artifact_kind: String,
    media_type: String,
    content_schema_hash: Option<desktop_runtime::Sha256Digest>,
    evidence_class: MethodEvidenceClass,
    artifact_content_hash: desktop_runtime::Sha256Digest,
    content: Vec<u8>,
}

#[derive(Debug)]
struct StoredMethodArtifactIndex {
    expectation_id: String,
    artifact_kind: String,
    media_type: String,
    content_schema_hash: Option<String>,
    evidence_class: String,
    session_id: String,
    owner_scope_ref: String,
    project_id: String,
    run_id: String,
    authority_id: String,
    binding_ordinal: u64,
    binding_hash: String,
    decision_id: String,
    invocation_id: String,
}

impl StoredMethodArtifactIndex {
    fn matches_provenance(&self, provenance: &MethodArtifactProvenance) -> bool {
        self.session_id == provenance.session_id.as_str()
            && self.owner_scope_ref == provenance.scope.owner_scope_ref.as_str()
            && self.project_id == provenance.scope.project_id.as_str()
            && self.run_id == provenance.scope.run_id.as_str()
            && self.authority_id == provenance.scope.authority_ref.authority_id.as_str()
            && self.binding_ordinal == provenance.binding_ordinal
            && self.binding_hash == provenance.binding_hash.to_string()
            && self.decision_id == provenance.decision_id.as_str()
            && self.invocation_id == provenance.invocation_id.as_str()
    }
}

impl MethodSessionRepository for LocalStore {
    type Error = StoreError;

    fn create_method_session(&self, session: &MethodSession) -> Result<(), Self::Error> {
        if session.version() != 1
            || session.state() != desktop_runtime::MethodState::Created
            || session.resume().is_some()
        {
            return Err(StoreError::StateConflict);
        }
        let state_json = session.to_persisted_json()?;
        let payload = self.put_payload(
            METHOD_STATE_KIND,
            METHOD_STATE_SCHEMA,
            state_json.as_bytes(),
        )?;
        let scope = session.scope();
        let session_id = session.session_id();
        let occurred_at = canonical_now();
        let payload_uri = payload_uri(&payload)?;
        let event = EvidenceAppend {
            stream_id: format!("bmad-method:{}", session_id.as_str()),
            event_type: "bmad.method.created".to_owned(),
            payload_hash: payload.content_hash.clone(),
            payload_ref: Some(payload_uri),
            correlation_id: session_id.to_string(),
            causation_id: None,
            redaction_level: "summary".to_owned(),
            retention_class: "authority".to_owned(),
        };
        let mut connection = self.connection.lock();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let inserted = transaction.execute(
            "INSERT INTO bmad_method_sessions
             (session_id, owner_scope_ref, project_id, run_id, authority_id, version, state,
              state_content_hash, state_kind, state_schema_version, state_byte_count,
              state_key_version, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                session_id.as_str(),
                scope.owner_scope_ref.as_str(),
                scope.project_id.as_str(),
                scope.run_id.as_str(),
                scope.authority_ref.authority_id.as_str(),
                session.version(),
                state_name(session),
                payload.content_hash,
                payload.kind,
                payload.schema_version,
                payload.byte_count,
                payload.key_version,
                occurred_at,
            ],
        );
        match inserted {
            Ok(1) => {}
            Ok(_) => return Err(StoreError::Inconsistent),
            Err(error) if is_unique_violation(&error) => return Err(StoreError::StateConflict),
            Err(error) => return Err(StoreError::Sqlite(error)),
        }
        let _ = append_evidence_in_transaction(&transaction, &event, &occurred_at)?;
        transaction.commit()?;
        Ok(())
    }

    fn load_method_session(
        &self,
        scope: &MethodSessionScope,
        session_id: &ContractId,
    ) -> Result<Option<MethodSession>, Self::Error> {
        let Some(reference) = self.load_method_state_ref(scope, session_id)? else {
            return Ok(None);
        };
        let bytes = self.get_payload(&reference.payload)?;
        let source = std::str::from_utf8(&bytes).map_err(|_| StoreError::Inconsistent)?;
        let session = MethodSession::from_persisted_json(source)?;
        if session.version() != reference.version
            || state_name(&session) != reference.state
            || session.session_id() != *session_id
            || session.scope() != *scope
        {
            return Err(StoreError::Inconsistent);
        }
        Ok(Some(session))
    }

    fn begin_method_advance(
        &self,
        scope: &MethodSessionScope,
        session_id: &ContractId,
        observed_binding: &MethodExactBinding,
        request: MethodAdvanceRequest,
    ) -> Result<MethodAdvanceReceipt, Self::Error> {
        if let Some(existing) =
            self.load_method_receipt(scope, session_id, &request.idempotency_key)?
        {
            return same_receipt_or_conflict(existing, &request);
        }
        let mut session = self
            .load_method_session(scope, session_id)?
            .ok_or(StoreError::Inconsistent)?;
        session.validate_review_for(observed_binding)?;
        let prior_version = session.version();
        let receipt = session.begin_advance(request.clone())?;
        let state_json = session.to_persisted_json()?;
        let payload = self.put_payload(
            METHOD_STATE_KIND,
            METHOD_STATE_SCHEMA,
            state_json.as_bytes(),
        )?;
        let payload_uri = payload_uri(&payload)?;
        let receipt_json = String::from_utf8(
            canonical_json_bytes(&receipt).map_err(|_| StoreError::Inconsistent)?,
        )
        .map_err(|_| StoreError::Inconsistent)?;
        let receipt_hash = canonical_hash("bmad-method-advance-receipt", 1, &receipt)
            .map_err(|_| StoreError::Inconsistent)?
            .to_string();
        let occurred_at = canonical_now();
        let event = EvidenceAppend {
            stream_id: format!("bmad-method:{}", session_id.as_str()),
            event_type: "bmad.method.advance_started".to_owned(),
            payload_hash: payload.content_hash.clone(),
            payload_ref: Some(payload_uri),
            correlation_id: receipt.invocation_id.to_string(),
            causation_id: Some(receipt.decision_id.to_string()),
            redaction_level: "summary".to_owned(),
            retention_class: "authority".to_owned(),
        };

        let mut connection = self.connection.lock();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(existing) =
            query_method_receipt(&transaction, scope, session_id, &request.idempotency_key)?
        {
            return same_receipt_or_conflict(existing, &request);
        }
        insert_method_consumption(
            &transaction,
            &receipt,
            scope,
            session_id,
            &receipt_json,
            &receipt_hash,
            &occurred_at,
        )?;
        update_method_projection(
            &transaction,
            &session,
            scope,
            &payload,
            &occurred_at,
            prior_version,
        )?;
        let _ = append_evidence_in_transaction(&transaction, &event, &occurred_at)?;
        transaction.commit()?;
        Ok(receipt)
    }

    fn persist_method_transition(
        &self,
        session: &MethodSession,
        expected_previous_version: u64,
        event_kind: MethodPersistenceEvent,
    ) -> Result<(), Self::Error> {
        if session.version() != expected_previous_version.saturating_add(1)
            || !event_matches_state(event_kind, session)
        {
            return Err(StoreError::StateConflict);
        }
        if event_kind == MethodPersistenceEvent::ResultAccepted {
            let checkpoint = session.resume().ok_or(StoreError::Inconsistent)?;
            let provenance = session.artifact_provenance_for(&checkpoint.invocation_id)?;
            let disposition = match session.state() {
                desktop_runtime::MethodState::AwaitingUser => {
                    MethodAdvanceDisposition::AwaitingUser
                }
                desktop_runtime::MethodState::ContextReviewRequired => {
                    MethodAdvanceDisposition::ContextReviewRequired
                }
                desktop_runtime::MethodState::Completed => MethodAdvanceDisposition::Completed,
                _ => return Err(StoreError::StateConflict),
            };
            self.validate_method_artifact_refs(
                &provenance,
                session.current_binding()?,
                disposition,
                &checkpoint.working_artifact_refs,
            )?;
        }
        let state_json = session.to_persisted_json()?;
        let payload = self.put_payload(
            METHOD_STATE_KIND,
            METHOD_STATE_SCHEMA,
            state_json.as_bytes(),
        )?;
        let payload_uri = payload_uri(&payload)?;
        let scope = session.scope();
        let session_id = session.session_id();
        let occurred_at = canonical_now();
        let event = EvidenceAppend {
            stream_id: format!("bmad-method:{}", session_id.as_str()),
            event_type: event_kind.event_type().to_owned(),
            payload_hash: payload.content_hash.clone(),
            payload_ref: Some(payload_uri),
            correlation_id: session_id.to_string(),
            causation_id: session
                .resume()
                .map(|checkpoint| checkpoint.checkpoint_id.to_string()),
            redaction_level: "summary".to_owned(),
            retention_class: "authority".to_owned(),
        };
        let mut connection = self.connection.lock();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        update_method_projection(
            &transaction,
            session,
            &scope,
            &payload,
            &occurred_at,
            expected_previous_version,
        )?;
        if event_kind == MethodPersistenceEvent::ResultAccepted {
            insert_method_checkpoint(&transaction, session, &payload, &occurred_at)?;
        }
        let _ = append_evidence_in_transaction(&transaction, &event, &occurred_at)?;
        transaction.commit()?;
        Ok(())
    }

    fn validate_method_artifact_refs(
        &self,
        provenance: &MethodArtifactProvenance,
        binding: &MethodExactBinding,
        disposition: MethodAdvanceDisposition,
        refs: &[String],
    ) -> Result<(), Self::Error> {
        let _ = binding.binding_hash()?;
        let mut prior = None;
        let mut matched = BTreeSet::new();
        for reference in refs {
            if prior.is_some_and(|value: &str| value >= reference.as_str()) {
                return Err(StoreError::Inconsistent);
            }
            let (index, envelope) = self.load_method_artifact(reference)?;
            let expectation = binding
                .artifact_expectations
                .iter()
                .find(|value| value.expectation_id.as_str() == index.expectation_id)
                .ok_or(StoreError::Inconsistent)?;
            if !index.matches_provenance(provenance)
                || expectation.artifact_kind != index.artifact_kind
                || expectation.expected_media_type != index.media_type
                || expectation
                    .expected_content_schema_hash
                    .as_ref()
                    .map(ToString::to_string)
                    != index.content_schema_hash
                || expectation.completion_evidence_class.as_str() != index.evidence_class
                || !matched.insert(index.expectation_id)
            {
                return Err(StoreError::Inconsistent);
            }
            if envelope.expectation_id != expectation.expectation_id
                || envelope.provenance != *provenance
                || envelope.artifact_kind != index.artifact_kind
                || envelope.media_type != index.media_type
                || envelope.content_schema_hash != expectation.expected_content_schema_hash
                || envelope.evidence_class != expectation.completion_evidence_class
                || envelope.artifact_content_hash
                    != desktop_runtime::sha256_bytes(&envelope.content)
            {
                return Err(StoreError::Inconsistent);
            }
            prior = Some(reference.as_str());
        }
        if disposition == MethodAdvanceDisposition::Completed
            && binding.artifact_expectations.iter().any(|expectation| {
                expectation.required && !matched.contains(expectation.expectation_id.as_str())
            })
        {
            return Err(StoreError::Inconsistent);
        }
        Ok(())
    }
}

fn insert_method_consumption(
    transaction: &rusqlite::Transaction<'_>,
    receipt: &MethodAdvanceReceipt,
    scope: &MethodSessionScope,
    session_id: &ContractId,
    receipt_json: &str,
    receipt_hash: &str,
    occurred_at: &str,
) -> Result<(), StoreError> {
    let inserted = transaction.execute(
        "INSERT INTO bmad_method_decision_consumptions
         (consumption_id, decision_id, invocation_id, idempotency_key, session_id,
          owner_scope_ref, project_id, run_id, authority_id, receipt_json, receipt_hash, consumed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            receipt.consumption_id.as_str(),
            receipt.decision_id.as_str(),
            receipt.invocation_id.as_str(),
            receipt.idempotency_key,
            session_id.as_str(),
            scope.owner_scope_ref.as_str(),
            scope.project_id.as_str(),
            scope.run_id.as_str(),
            scope.authority_ref.authority_id.as_str(),
            receipt_json,
            receipt_hash,
            occurred_at,
        ],
    );
    match inserted {
        Ok(1) => Ok(()),
        Ok(_) => Err(StoreError::Inconsistent),
        Err(error) if is_unique_violation(&error) => {
            Err(MethodError::new(MethodErrorCode::ContextDecisionAlreadyConsumed).into())
        }
        Err(error) => Err(StoreError::Sqlite(error)),
    }
}

fn update_method_projection(
    transaction: &rusqlite::Transaction<'_>,
    session: &MethodSession,
    scope: &MethodSessionScope,
    payload: &PayloadRef,
    occurred_at: &str,
    expected_previous_version: u64,
) -> Result<(), StoreError> {
    let updated = transaction.execute(
        "UPDATE bmad_method_sessions SET
           version = ?1,
           state = ?2,
           state_content_hash = ?3,
           state_kind = ?4,
           state_schema_version = ?5,
           state_byte_count = ?6,
           state_key_version = ?7,
           updated_at = ?8
         WHERE session_id = ?9
           AND owner_scope_ref = ?10
           AND project_id = ?11
           AND run_id = ?12
           AND authority_id = ?13
           AND version = ?14",
        params![
            session.version(),
            state_name(session),
            payload.content_hash,
            payload.kind,
            payload.schema_version,
            payload.byte_count,
            payload.key_version,
            occurred_at,
            session.session_id().as_str(),
            scope.owner_scope_ref.as_str(),
            scope.project_id.as_str(),
            scope.run_id.as_str(),
            scope.authority_ref.authority_id.as_str(),
            expected_previous_version,
        ],
    )?;
    if updated == 1 {
        Ok(())
    } else {
        Err(StoreError::StateConflict)
    }
}

fn insert_method_checkpoint(
    transaction: &rusqlite::Transaction<'_>,
    session: &MethodSession,
    payload: &PayloadRef,
    occurred_at: &str,
) -> Result<(), StoreError> {
    let checkpoint = session.resume().ok_or(StoreError::Inconsistent)?;
    let inserted = transaction.execute(
        "INSERT INTO bmad_method_checkpoints
         (checkpoint_id, session_id, turn_ordinal, checkpoint_hash,
          state_content_hash, state_kind, state_schema_version, recorded_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            checkpoint.checkpoint_id.as_str(),
            session.session_id().as_str(),
            checkpoint.turn_ordinal,
            checkpoint.checkpoint_hash.to_string(),
            payload.content_hash,
            payload.kind,
            payload.schema_version,
            occurred_at,
        ],
    );
    match inserted {
        Ok(1) => Ok(()),
        Ok(_) => Err(StoreError::Inconsistent),
        Err(error) if is_unique_violation(&error) => Err(StoreError::StateConflict),
        Err(error) => Err(StoreError::Sqlite(error)),
    }
}

impl LocalStore {
    fn load_method_artifact(
        &self,
        reference: &str,
    ) -> Result<(StoredMethodArtifactIndex, StoredMethodArtifact), StoreError> {
        let digest = reference
            .strip_prefix("cas://sha256/")
            .filter(|value| {
                value.len() == 64
                    && value
                        .bytes()
                        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            })
            .ok_or(StoreError::Inconsistent)?;
        let content_hash = format!("sha256:{digest}");
        let (payload, index) = {
            let connection = self.connection.lock();
            connection
                .query_row(
                    "SELECT p.byte_count, p.key_version, a.expectation_id,
                            a.artifact_kind, a.media_type, a.content_schema_hash,
                            a.evidence_class, a.session_id, a.owner_scope_ref,
                            a.project_id, a.run_id, a.authority_id, a.binding_ordinal,
                            a.binding_hash, a.decision_id, a.invocation_id
                     FROM payloads p
                     JOIN bmad_method_artifacts a
                       ON a.content_hash = p.content_hash
                      AND a.content_kind = p.kind
                      AND a.content_schema_version = p.schema_version
                     WHERE p.content_hash = ?1 AND p.kind = ?2 AND p.schema_version = ?3",
                    params![content_hash, METHOD_ARTIFACT_KIND, METHOD_ARTIFACT_SCHEMA],
                    |row| {
                        Ok((
                            PayloadRef {
                                content_hash: content_hash.clone(),
                                kind: METHOD_ARTIFACT_KIND.to_owned(),
                                schema_version: METHOD_ARTIFACT_SCHEMA.to_owned(),
                                byte_count: row.get(0)?,
                                key_version: row.get(1)?,
                            },
                            StoredMethodArtifactIndex {
                                expectation_id: row.get(2)?,
                                artifact_kind: row.get(3)?,
                                media_type: row.get(4)?,
                                content_schema_hash: row.get(5)?,
                                evidence_class: row.get(6)?,
                                session_id: row.get(7)?,
                                owner_scope_ref: row.get(8)?,
                                project_id: row.get(9)?,
                                run_id: row.get(10)?,
                                authority_id: row.get(11)?,
                                binding_ordinal: row.get(12)?,
                                binding_hash: row.get(13)?,
                                decision_id: row.get(14)?,
                                invocation_id: row.get(15)?,
                            },
                        ))
                    },
                )
                .optional()?
                .ok_or(StoreError::Inconsistent)?
        };
        let bytes = self.get_payload(&payload)?;
        let envelope: StoredMethodArtifact = serde_json::from_slice(&bytes)?;
        if canonical_json_bytes(&envelope).map_err(|_| StoreError::Inconsistent)? != bytes {
            return Err(StoreError::Inconsistent);
        }
        Ok((index, envelope))
    }

    /// Encrypts and stores one Method working artifact, returning its opaque CAS ref.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when authenticated CAS persistence fails.
    pub fn put_method_artifact(
        &self,
        provenance: &MethodArtifactProvenance,
        expectation: &MethodArtifactExpectation,
        plaintext: &[u8],
    ) -> Result<String, StoreError> {
        let envelope = StoredMethodArtifact {
            provenance: provenance.clone(),
            expectation_id: expectation.expectation_id.clone(),
            artifact_kind: expectation.artifact_kind.clone(),
            media_type: expectation.expected_media_type.clone(),
            content_schema_hash: expectation.expected_content_schema_hash,
            evidence_class: expectation.completion_evidence_class,
            artifact_content_hash: desktop_runtime::sha256_bytes(plaintext),
            content: plaintext.to_vec(),
        };
        let bytes = canonical_json_bytes(&envelope).map_err(|_| StoreError::Inconsistent)?;
        let payload = self.put_payload(METHOD_ARTIFACT_KIND, METHOD_ARTIFACT_SCHEMA, &bytes)?;
        let schema_hash = expectation
            .expected_content_schema_hash
            .as_ref()
            .map(ToString::to_string);
        let connection = self.connection.lock();
        let inserted = connection.execute(
            "INSERT OR IGNORE INTO bmad_method_artifacts
             (content_hash, content_kind, content_schema_version, expectation_id,
              artifact_kind, media_type, content_schema_hash, evidence_class,
              session_id, owner_scope_ref, project_id, run_id, authority_id,
              binding_ordinal, binding_hash, decision_id, invocation_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                     ?13, ?14, ?15, ?16, ?17)",
            params![
                payload.content_hash,
                payload.kind,
                payload.schema_version,
                expectation.expectation_id.as_str(),
                expectation.artifact_kind,
                expectation.expected_media_type,
                schema_hash,
                expectation.completion_evidence_class.as_str(),
                provenance.session_id.as_str(),
                provenance.scope.owner_scope_ref.as_str(),
                provenance.scope.project_id.as_str(),
                provenance.scope.run_id.as_str(),
                provenance.scope.authority_ref.authority_id.as_str(),
                provenance.binding_ordinal,
                provenance.binding_hash.to_string(),
                provenance.decision_id.as_str(),
                provenance.invocation_id.as_str(),
            ],
        )?;
        if inserted == 0 {
            let stored = connection
                .query_row(
                    "SELECT expectation_id, artifact_kind, media_type,
                            content_schema_hash, evidence_class, session_id,
                            owner_scope_ref, project_id, run_id, authority_id,
                            binding_ordinal, binding_hash, decision_id, invocation_id
                     FROM bmad_method_artifacts WHERE content_hash = ?1",
                    params![payload.content_hash],
                    |row| {
                        Ok(StoredMethodArtifactIndex {
                            expectation_id: row.get(0)?,
                            artifact_kind: row.get(1)?,
                            media_type: row.get(2)?,
                            content_schema_hash: row.get(3)?,
                            evidence_class: row.get(4)?,
                            session_id: row.get(5)?,
                            owner_scope_ref: row.get(6)?,
                            project_id: row.get(7)?,
                            run_id: row.get(8)?,
                            authority_id: row.get(9)?,
                            binding_ordinal: row.get(10)?,
                            binding_hash: row.get(11)?,
                            decision_id: row.get(12)?,
                            invocation_id: row.get(13)?,
                        })
                    },
                )
                .optional()?;
            if stored.as_ref().is_none_or(|index| {
                index.expectation_id != expectation.expectation_id.as_str()
                    || index.artifact_kind != expectation.artifact_kind
                    || index.media_type != expectation.expected_media_type
                    || index.content_schema_hash != schema_hash
                    || index.evidence_class != expectation.completion_evidence_class.as_str()
                    || !index.matches_provenance(provenance)
            }) {
                return Err(StoreError::Inconsistent);
            }
        }
        drop(connection);
        payload_uri(&payload)
    }

    pub(crate) fn verify_method_integrity(&self) -> Result<(), StoreError> {
        self.verify_method_artifacts()?;
        let snapshot = {
            let connection = self.connection.lock();
            load_method_integrity_snapshot(&connection)?
        };
        let expected = self.verify_method_session_rows(snapshot.sessions)?;
        self.verify_method_checkpoint_rows(snapshot.checkpoints, &expected)?;
        verify_method_receipt_rows(snapshot.receipts)
    }

    fn verify_method_artifacts(&self) -> Result<(), StoreError> {
        let rows = {
            let connection = self.connection.lock();
            let mut statement = connection.prepare(
                "SELECT a.content_hash, a.expectation_id, a.artifact_kind, a.media_type,
                        a.content_schema_hash, a.evidence_class, p.byte_count, p.key_version,
                        a.session_id, a.owner_scope_ref, a.project_id, a.run_id,
                        a.authority_id, a.binding_ordinal, a.binding_hash,
                        a.decision_id, a.invocation_id
                 FROM bmad_method_artifacts a
                 JOIN payloads p
                   ON p.content_hash = a.content_hash
                  AND p.kind = a.content_kind
                  AND p.schema_version = a.content_schema_version
                 ORDER BY a.content_hash",
            )?;
            let rows = statement
                .query_map([], |row| {
                    Ok((
                        PayloadRef {
                            content_hash: row.get(0)?,
                            kind: METHOD_ARTIFACT_KIND.to_owned(),
                            schema_version: METHOD_ARTIFACT_SCHEMA.to_owned(),
                            byte_count: row.get(6)?,
                            key_version: row.get(7)?,
                        },
                        StoredMethodArtifactIndex {
                            expectation_id: row.get(1)?,
                            artifact_kind: row.get(2)?,
                            media_type: row.get(3)?,
                            content_schema_hash: row.get(4)?,
                            evidence_class: row.get(5)?,
                            session_id: row.get(8)?,
                            owner_scope_ref: row.get(9)?,
                            project_id: row.get(10)?,
                            run_id: row.get(11)?,
                            authority_id: row.get(12)?,
                            binding_ordinal: row.get(13)?,
                            binding_hash: row.get(14)?,
                            decision_id: row.get(15)?,
                            invocation_id: row.get(16)?,
                        },
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };
        for (payload, index) in rows {
            let bytes = self.get_payload(&payload)?;
            let envelope: StoredMethodArtifact = serde_json::from_slice(&bytes)?;
            if canonical_json_bytes(&envelope).map_err(|_| StoreError::Inconsistent)? != bytes
                || envelope.expectation_id.as_str() != index.expectation_id
                || envelope.artifact_kind != index.artifact_kind
                || envelope.media_type != index.media_type
                || envelope.content_schema_hash.map(|value| value.to_string())
                    != index.content_schema_hash
                || envelope.evidence_class.as_str() != index.evidence_class
                || !index.matches_provenance(&envelope.provenance)
                || envelope.artifact_content_hash
                    != desktop_runtime::sha256_bytes(&envelope.content)
            {
                return Err(StoreError::Inconsistent);
            }
        }
        Ok(())
    }

    fn verify_method_session_rows(
        &self,
        rows: Vec<MethodIntegrityRow>,
    ) -> Result<ExpectedCheckpoints, StoreError> {
        let mut expected_checkpoints = BTreeMap::new();
        for row in rows {
            if row.payload.kind != METHOD_STATE_KIND
                || row.payload.schema_version != METHOD_STATE_SCHEMA
            {
                return Err(StoreError::Inconsistent);
            }
            let bytes = self.get_payload(&row.payload)?;
            let source = std::str::from_utf8(&bytes).map_err(|_| StoreError::Inconsistent)?;
            let session = MethodSession::from_persisted_json(source)?;
            let scope = session.scope();
            if session.session_id().as_str() != row.session_id
                || scope.owner_scope_ref.as_str() != row.owner_scope_ref
                || scope.project_id.as_str() != row.project_id
                || scope.run_id.as_str() != row.run_id
                || scope.authority_ref.authority_id.as_str() != row.authority_id
                || session.version() != row.version
                || state_name(&session) != row.state
            {
                return Err(StoreError::Inconsistent);
            }
            for checkpoint in session.checkpoints() {
                expected_checkpoints.insert(
                    (row.session_id.clone(), checkpoint.turn_ordinal),
                    (
                        checkpoint.checkpoint_id.to_string(),
                        checkpoint.checkpoint_hash.to_string(),
                    ),
                );
            }
        }
        Ok(expected_checkpoints)
    }

    fn verify_method_checkpoint_rows(
        &self,
        rows: Vec<CheckpointIntegrityRow>,
        expected: &ExpectedCheckpoints,
    ) -> Result<(), StoreError> {
        if rows.len() != expected.len() {
            return Err(StoreError::Inconsistent);
        }
        for row in rows {
            if expected.get(&(row.session_id.clone(), row.turn_ordinal))
                != Some(&(row.checkpoint_id.clone(), row.checkpoint_hash.clone()))
                || row.payload.kind != METHOD_STATE_KIND
                || row.payload.schema_version != METHOD_STATE_SCHEMA
            {
                return Err(StoreError::Inconsistent);
            }
            let bytes = self.get_payload(&row.payload)?;
            let source = std::str::from_utf8(&bytes).map_err(|_| StoreError::Inconsistent)?;
            let state = MethodSession::from_persisted_json(source)?;
            let checkpoint = state
                .checkpoints()
                .iter()
                .find(|value| value.turn_ordinal == row.turn_ordinal)
                .ok_or(StoreError::Inconsistent)?;
            if state.session_id().as_str() != row.session_id
                || checkpoint.checkpoint_id.as_str() != row.checkpoint_id
                || checkpoint.checkpoint_hash.to_string() != row.checkpoint_hash
            {
                return Err(StoreError::Inconsistent);
            }
        }
        Ok(())
    }

    fn load_method_state_ref(
        &self,
        scope: &MethodSessionScope,
        session_id: &ContractId,
    ) -> Result<Option<StoredMethodStateRef>, StoreError> {
        self.connection
            .lock()
            .query_row(
                "SELECT version, state, state_content_hash, state_kind, state_schema_version,
                        state_byte_count, state_key_version
                 FROM bmad_method_sessions
                 WHERE session_id = ?1 AND owner_scope_ref = ?2 AND project_id = ?3
                   AND run_id = ?4 AND authority_id = ?5",
                params![
                    session_id.as_str(),
                    scope.owner_scope_ref.as_str(),
                    scope.project_id.as_str(),
                    scope.run_id.as_str(),
                    scope.authority_ref.authority_id.as_str(),
                ],
                |row| {
                    Ok(StoredMethodStateRef {
                        version: row.get(0)?,
                        state: row.get(1)?,
                        payload: PayloadRef {
                            content_hash: row.get(2)?,
                            kind: row.get(3)?,
                            schema_version: row.get(4)?,
                            byte_count: row.get(5)?,
                            key_version: row.get(6)?,
                        },
                    })
                },
            )
            .optional()
            .map_err(StoreError::from)
    }

    fn load_method_receipt(
        &self,
        scope: &MethodSessionScope,
        session_id: &ContractId,
        idempotency_key: &str,
    ) -> Result<Option<MethodAdvanceReceipt>, StoreError> {
        query_method_receipt(&self.connection.lock(), scope, session_id, idempotency_key)
    }
}

fn load_method_integrity_snapshot(
    connection: &rusqlite::Connection,
) -> Result<MethodIntegritySnapshot, StoreError> {
    Ok(MethodIntegritySnapshot {
        sessions: load_method_session_rows(connection)?,
        checkpoints: load_method_checkpoint_rows(connection)?,
        receipts: load_method_receipt_rows(connection)?,
    })
}

fn load_method_session_rows(
    connection: &rusqlite::Connection,
) -> Result<Vec<MethodIntegrityRow>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT session_id, owner_scope_ref, project_id, run_id, authority_id,
                version, state, state_content_hash, state_kind, state_schema_version,
                state_byte_count, state_key_version
         FROM bmad_method_sessions ORDER BY session_id",
    )?;
    let rows = statement
        .query_map([], |row| {
            Ok(MethodIntegrityRow {
                session_id: row.get(0)?,
                owner_scope_ref: row.get(1)?,
                project_id: row.get(2)?,
                run_id: row.get(3)?,
                authority_id: row.get(4)?,
                version: row.get(5)?,
                state: row.get(6)?,
                payload: PayloadRef {
                    content_hash: row.get(7)?,
                    kind: row.get(8)?,
                    schema_version: row.get(9)?,
                    byte_count: row.get(10)?,
                    key_version: row.get(11)?,
                },
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn load_method_checkpoint_rows(
    connection: &rusqlite::Connection,
) -> Result<Vec<CheckpointIntegrityRow>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT c.checkpoint_id, c.session_id, c.turn_ordinal, c.checkpoint_hash,
                c.state_content_hash, c.state_kind, c.state_schema_version,
                p.byte_count, p.key_version
         FROM bmad_method_checkpoints c
         JOIN payloads p
           ON p.content_hash = c.state_content_hash
          AND p.kind = c.state_kind
          AND p.schema_version = c.state_schema_version
         ORDER BY c.session_id, c.turn_ordinal",
    )?;
    let rows = statement
        .query_map([], |row| {
            Ok(CheckpointIntegrityRow {
                checkpoint_id: row.get(0)?,
                session_id: row.get(1)?,
                turn_ordinal: row.get(2)?,
                checkpoint_hash: row.get(3)?,
                payload: PayloadRef {
                    content_hash: row.get(4)?,
                    kind: row.get(5)?,
                    schema_version: row.get(6)?,
                    byte_count: row.get(7)?,
                    key_version: row.get(8)?,
                },
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn load_method_receipt_rows(
    connection: &rusqlite::Connection,
) -> Result<Vec<ReceiptIntegrityRow>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT consumption_id, decision_id, invocation_id, idempotency_key,
                receipt_json, receipt_hash
         FROM bmad_method_decision_consumptions ORDER BY consumption_id",
    )?;
    let rows = statement
        .query_map([], |row| {
            Ok(ReceiptIntegrityRow {
                consumption_id: row.get(0)?,
                decision_id: row.get(1)?,
                invocation_id: row.get(2)?,
                idempotency_key: row.get(3)?,
                source: row.get(4)?,
                expected_hash: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn verify_method_receipt_rows(rows: Vec<ReceiptIntegrityRow>) -> Result<(), StoreError> {
    for row in rows {
        let receipt: MethodAdvanceReceipt = serde_json::from_str(&row.source)?;
        let actual_hash = canonical_hash("bmad-method-advance-receipt", 1, &receipt)
            .map_err(|_| StoreError::Inconsistent)?
            .to_string();
        if receipt.consumption_id.as_str() != row.consumption_id
            || receipt.decision_id.as_str() != row.decision_id
            || receipt.invocation_id.as_str() != row.invocation_id
            || receipt.idempotency_key != row.idempotency_key
            || actual_hash != row.expected_hash
        {
            return Err(StoreError::Inconsistent);
        }
    }
    Ok(())
}

fn query_method_receipt(
    connection: &rusqlite::Connection,
    scope: &MethodSessionScope,
    session_id: &ContractId,
    idempotency_key: &str,
) -> Result<Option<MethodAdvanceReceipt>, StoreError> {
    let stored = connection
        .query_row(
            "SELECT receipt_json, receipt_hash FROM bmad_method_decision_consumptions
             WHERE session_id = ?1 AND owner_scope_ref = ?2 AND project_id = ?3
               AND run_id = ?4 AND authority_id = ?5 AND idempotency_key = ?6",
            params![
                session_id.as_str(),
                scope.owner_scope_ref.as_str(),
                scope.project_id.as_str(),
                scope.run_id.as_str(),
                scope.authority_ref.authority_id.as_str(),
                idempotency_key,
            ],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    stored
        .map(|(source, expected_hash)| {
            let receipt: MethodAdvanceReceipt = serde_json::from_str(&source)?;
            let actual_hash = canonical_hash("bmad-method-advance-receipt", 1, &receipt)
                .map_err(|_| StoreError::Inconsistent)?
                .to_string();
            if actual_hash != expected_hash {
                return Err(StoreError::Inconsistent);
            }
            Ok(receipt)
        })
        .transpose()
}

fn same_receipt_or_conflict(
    receipt: MethodAdvanceReceipt,
    request: &MethodAdvanceRequest,
) -> Result<MethodAdvanceReceipt, StoreError> {
    if receipt.invocation_id == request.invocation_id
        && receipt.decision_id == request.decision_id
        && receipt.idempotency_key == request.idempotency_key
        && request
            .expected_version
            .checked_add(1)
            .is_some_and(|version| version == receipt.aggregate_version)
    {
        Ok(receipt)
    } else {
        Err(StoreError::StateConflict)
    }
}

fn payload_uri(payload: &PayloadRef) -> Result<String, StoreError> {
    let digest = payload
        .content_hash
        .strip_prefix("sha256:")
        .ok_or(StoreError::Inconsistent)?;
    Ok(format!("cas://sha256/{digest}"))
}

fn state_name(session: &MethodSession) -> &'static str {
    use desktop_runtime::MethodState;

    match session.state() {
        MethodState::Created => "created",
        MethodState::CapabilityBound => "capability_bound",
        MethodState::ContextReviewRequired => "context_review_required",
        MethodState::Ready => "ready",
        MethodState::Advancing => "advancing",
        MethodState::AwaitingUser => "awaiting_user",
        MethodState::Completed => "completed",
        MethodState::Refused => "refused",
        MethodState::Incomplete => "incomplete",
        MethodState::Cancelled => "cancelled",
    }
}

fn event_matches_state(event: MethodPersistenceEvent, session: &MethodSession) -> bool {
    use desktop_runtime::MethodState;

    match event {
        MethodPersistenceEvent::CapabilityBound | MethodPersistenceEvent::CapabilityRebound => {
            session.state() == MethodState::CapabilityBound
        }
        MethodPersistenceEvent::ContextReviewRequested
        | MethodPersistenceEvent::UserTurnRecorded => {
            session.state() == MethodState::ContextReviewRequired
        }
        MethodPersistenceEvent::ContextReviewAccepted => session.state() == MethodState::Ready,
        MethodPersistenceEvent::ResultAccepted => matches!(
            session.state(),
            MethodState::AwaitingUser | MethodState::ContextReviewRequired | MethodState::Completed
        ),
        MethodPersistenceEvent::Refused => session.state() == MethodState::Refused,
        MethodPersistenceEvent::Incomplete => session.state() == MethodState::Incomplete,
        MethodPersistenceEvent::Cancelled => session.state() == MethodState::Cancelled,
    }
}
