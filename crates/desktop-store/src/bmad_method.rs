use std::collections::{BTreeMap, BTreeSet};

use desktop_runtime::{
    canonical_hash, canonical_json_bytes, ContractId, DesktopLocalIdentity,
    MethodAdvanceDisposition, MethodAdvanceReceipt, MethodAdvanceRequest,
    MethodArtifactExpectation, MethodArtifactProvenance, MethodError, MethodErrorCode,
    MethodEvidenceClass, MethodExactBinding, MethodPersistenceEvent, MethodSession,
    MethodSessionRepository, MethodSessionScope, Sha256Digest, UnixMillis,
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;

use super::{
    append_evidence_in_transaction, canonical_now, is_unique_violation, EvidenceAppend, LocalStore,
    PayloadRef, StoreError,
};

const METHOD_STATE_KIND: &str = "bmad_method_session";
const METHOD_STATE_SCHEMA: &str = "sapphirus.bmad-method-session-state.v1";
const METHOD_ARTIFACT_KIND: &str = "bmad_method_artifact";
const METHOD_ARTIFACT_SCHEMA: &str = "sapphirus.bmad-method-artifact.v1";
const HELP_RUN_RENDERER_PROJECTION_KIND: &str = "bmad_help_run_renderer_projection";
const HELP_RUN_RENDERER_PROJECTION_SCHEMA: &str = "sapphirus.bmad-help-run-renderer-projection.v1";
const HELP_RUN_RENDERER_PROJECTION_RETAINED: &str = "retained";
const HELP_RUN_RENDERER_PROJECTION_LEGACY_UNRETAINED: &str = "legacy_unretained";
const HELP_RUN_CREATE_PURPOSE: &str = "bmad-help-run-create-request";
const HELP_RUN_PROJECTION_BINDING_PURPOSE: &str = "bmad-help-run-projection-binding";
const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;
pub const MAX_BMAD_HELP_RUN_RENDERER_PROJECTION_BYTES: usize = 66_560;
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
struct HelpRunEvidenceIntegrityRow {
    stream_id: String,
    sequence: u64,
    payload: PayloadRef,
    payload_ref: String,
    correlation_id: String,
}

#[derive(Debug)]
struct MethodIntegritySnapshot {
    sessions: Vec<MethodIntegrityRow>,
    checkpoints: Vec<CheckpointIntegrityRow>,
    receipts: Vec<ReceiptIntegrityRow>,
    help_run_creations: Vec<StoredHelpRunCreation>,
    help_run_events: Vec<HelpRunEvidenceIntegrityRow>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadHelpRunCreateRequest {
    pub request_id: ContractId,
    pub project_id: ContractId,
    pub workspace_id: ContractId,
    pub workspace_grant_epoch: u64,
    pub workspace_catalog_version: u64,
    pub workspace_root_identity_hash: Sha256Digest,
    pub capability_catalog_hash: Sha256Digest,
    pub foundation_binding_hash: Sha256Digest,
    pub intent_hash: Sha256Digest,
    pub renderer_projection: Vec<u8>,
    pub accepted_at: UnixMillis,
}

/// Renderer-reproducible facts used only to recover an already committed Help run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadHelpRunReplayRequest {
    pub request_id: ContractId,
    pub workspace_id: ContractId,
    pub workspace_grant_epoch: u64,
    pub capability_catalog_hash: Sha256Digest,
    pub foundation_binding_hash: Sha256Digest,
    pub intent_hash: Sha256Digest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadHelpRunCreationReceipt {
    pub request_id: ContractId,
    pub session_id: ContractId,
    pub run_id: ContractId,
    pub accepted_at: UnixMillis,
    pub replayed: bool,
    pub renderer_projection: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BmadHelpRunLatest {
    None,
    LegacyProjectionUnavailable,
    Retained(BmadHelpRunCreationReceipt),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BmadHelpRunCreateFingerprint<'a> {
    schema_version: &'static str,
    run_kind: &'static str,
    request_id: &'a ContractId,
    project_id: &'a ContractId,
    workspace_id: &'a ContractId,
    workspace_grant_epoch: u64,
    workspace_root_identity_hash: Sha256Digest,
    capability_catalog_hash: Sha256Digest,
    foundation_binding_hash: Sha256Digest,
    intent_hash: Sha256Digest,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BmadHelpRunProjectionBinding<'a> {
    schema_version: &'static str,
    request_fingerprint: Sha256Digest,
    session_id: &'a ContractId,
    run_id: &'a ContractId,
    workspace_catalog_version: u64,
    creation_ordinal: u64,
    accepted_at: u64,
    renderer_projection_hash: Sha256Digest,
}

#[derive(Clone, Debug)]
struct StoredHelpRunCreation {
    owner_scope_ref: String,
    installation_id: String,
    request_id: String,
    request_fingerprint: String,
    session_id: String,
    project_id: String,
    run_id: String,
    authority_id: String,
    authority_epoch: u64,
    local_store_id: String,
    workspace_id: String,
    workspace_grant_epoch: u64,
    workspace_catalog_version: u64,
    workspace_root_identity_hash: String,
    capability_catalog_hash: String,
    foundation_binding_hash: String,
    intent_hash: String,
    renderer_projection_state: String,
    renderer_projection_content_hash: Option<String>,
    renderer_projection_kind: Option<String>,
    renderer_projection_schema_version: Option<String>,
    renderer_projection_byte_count: Option<u64>,
    renderer_projection_key_version: Option<u32>,
    renderer_projection_binding_hash: Option<String>,
    creation_ordinal: u64,
    accepted_at: u64,
}

struct NewHelpRunCreationRow<'a> {
    session: &'a MethodSession,
    request: &'a BmadHelpRunCreateRequest,
    identity: &'a DesktopLocalIdentity,
    request_fingerprint: &'a Sha256Digest,
    renderer_projection: &'a PayloadRef,
    renderer_projection_binding_hash: &'a Sha256Digest,
    creation_ordinal: u64,
}

#[derive(Clone, Debug)]
struct VerifiedMethodSession {
    owner_scope_ref: String,
    project_id: String,
    run_id: String,
    authority_kind: String,
    authority_id: String,
    installation_id: String,
    local_store_id: String,
    authority_epoch: u64,
    created_at: u64,
}

type VerifiedMethodSessions = BTreeMap<String, VerifiedMethodSession>;

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

impl LocalStore {
    /// Recovers an authenticated prior BMAD Help run without creating authority.
    ///
    /// This lookup deliberately requires no current workspace authorization so
    /// a committed reply remains replayable after grant revocation. Every
    /// renderer-reproducible request fact must match the retained receipt; the
    /// hidden project/root binding is still authenticated from durable state.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::StateConflict`] when the exact request key exists
    /// with different submitted facts, and an integrity/authentication error
    /// when retained authority cannot be reconstructed exactly.
    pub fn replay_bmad_help_run(
        &self,
        request: &BmadHelpRunReplayRequest,
    ) -> Result<Option<BmadHelpRunCreationReceipt>, StoreError> {
        validate_replay_request(request)?;
        let identity = self.load_local_identity()?;
        let stored = query_help_run_creation(
            &self.connection.lock(),
            identity.owner_scope_ref().as_str(),
            identity.installation_id().as_str(),
            request.request_id.as_str(),
        )?;
        let Some(stored) = stored else {
            return Ok(None);
        };
        if stored.workspace_id != request.workspace_id.as_str()
            || stored.workspace_grant_epoch != request.workspace_grant_epoch
            || stored.capability_catalog_hash != request.capability_catalog_hash.to_string()
            || stored.foundation_binding_hash != request.foundation_binding_hash.to_string()
            || stored.intent_hash != request.intent_hash.to_string()
        {
            return Err(StoreError::StateConflict);
        }
        self.authenticate_help_run_creation(&identity, &stored, true)
            .map(Some)
    }

    /// Loads the newest authenticated BMAD Help run for one currently authorized workspace.
    ///
    /// The workspace-catalog version check and latest-row selection share one
    /// read transaction. A second process that has already committed a catalog
    /// transition therefore makes a stale caller fail closed instead of
    /// receiving a projection selected after that transition.
    /// Released v8 rows remain authenticated authority history but did not
    /// retain renderer bytes; they return
    /// [`BmadHelpRunLatest::LegacyProjectionUnavailable`] rather than being
    /// misreported as no run.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::WorkspaceAuthorityStale`] when the caller's exact
    /// catalog version is no longer current, or an integrity/authentication
    /// error when the retained run cannot be reconstructed exactly.
    pub fn latest_bmad_help_run(
        &self,
        workspace_id: &ContractId,
        expected_workspace_catalog_version: u64,
    ) -> Result<BmadHelpRunLatest, StoreError> {
        validate_workspace_catalog_version(expected_workspace_catalog_version)?;
        let identity = self.load_local_identity()?;
        let stored = {
            let mut connection = self.connection.lock();
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Deferred)?;
            verify_workspace_catalog_version(&transaction, expected_workspace_catalog_version)?;
            let stored = query_latest_help_run_creation(
                &transaction,
                identity.owner_scope_ref().as_str(),
                identity.installation_id().as_str(),
                workspace_id.as_str(),
            )?;
            transaction.commit()?;
            stored
        };
        match stored {
            Some(row) => self.authenticate_latest_help_run(&identity, &row),
            None => Ok(BmadHelpRunLatest::None),
        }
    }

    /// Atomically accepts one store-owned, non-runnable BMAD Help run creation.
    ///
    /// The idempotency key is the exact local owner scope, installation, and
    /// request identifier. Replays return the originally committed session,
    /// run, and acceptance time; candidate identifiers and timestamps never
    /// replace committed authority.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::StateConflict`] when an existing request key has a
    /// different fingerprint or the candidate is not an exact Created/unbound
    /// session for the current sealed local identity and request scope.
    pub fn create_bmad_help_run(
        &self,
        candidate: &MethodSession,
        request: &BmadHelpRunCreateRequest,
    ) -> Result<BmadHelpRunCreationReceipt, StoreError> {
        validate_help_run_request(request)?;
        let identity = self.load_local_identity()?;
        let request_fingerprint = help_run_request_fingerprint(request)?;
        let mut connection = self.connection.lock();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

        if let Some(stored) = query_help_run_creation(
            &transaction,
            identity.owner_scope_ref().as_str(),
            identity.installation_id().as_str(),
            request.request_id.as_str(),
        )? {
            if stored.request_fingerprint != request_fingerprint.to_string() {
                return Err(StoreError::StateConflict);
            }
            drop(transaction);
            drop(connection);
            return self.authenticate_help_run_creation(&identity, &stored, true);
        }

        verify_workspace_catalog_version(&transaction, request.workspace_catalog_version)?;

        let state_json = validate_new_help_run_candidate(candidate, request, &identity)?;
        let payload = self.prepare_payload(
            METHOD_STATE_KIND,
            METHOD_STATE_SCHEMA,
            state_json.as_bytes(),
        )?;
        let renderer_projection = self.prepare_payload(
            HELP_RUN_RENDERER_PROJECTION_KIND,
            HELP_RUN_RENDERER_PROJECTION_SCHEMA,
            &request.renderer_projection,
        )?;
        let creation_ordinal = next_help_run_creation_ordinal(&transaction)?;
        let projection_binding_hash = help_run_projection_binding_hash(
            request_fingerprint,
            &candidate.session_id(),
            &candidate.scope().run_id,
            request.workspace_catalog_version,
            creation_ordinal,
            request.accepted_at,
            desktop_runtime::sha256_bytes(&request.renderer_projection),
        )?;
        let occurred_at = canonical_now();
        register_payload_in_transaction(&transaction, &payload, &occurred_at)?;
        register_payload_in_transaction(&transaction, &renderer_projection, &occurred_at)?;
        insert_created_method_session(&transaction, candidate, &payload, &occurred_at)?;
        let payload_uri = payload_uri(&payload)?;
        let event = EvidenceAppend {
            stream_id: format!("bmad-method:{}", candidate.session_id().as_str()),
            event_type: "bmad.help.run.created".to_owned(),
            payload_hash: payload.content_hash.clone(),
            payload_ref: Some(payload_uri),
            correlation_id: request.request_id.to_string(),
            causation_id: None,
            redaction_level: "summary".to_owned(),
            retention_class: "authority".to_owned(),
        };
        let _ = append_evidence_in_transaction(&transaction, &event, &occurred_at)?;
        insert_help_run_creation(
            &transaction,
            &NewHelpRunCreationRow {
                session: candidate,
                request,
                identity: &identity,
                request_fingerprint: &request_fingerprint,
                renderer_projection: &renderer_projection,
                renderer_projection_binding_hash: &projection_binding_hash,
                creation_ordinal,
            },
        )?;
        transaction.commit()?;

        Ok(BmadHelpRunCreationReceipt {
            request_id: request.request_id.clone(),
            session_id: candidate.session_id(),
            run_id: candidate.scope().run_id,
            accepted_at: request.accepted_at,
            replayed: false,
            renderer_projection: request.renderer_projection.clone(),
        })
    }

    fn prepare_payload(
        &self,
        kind: &str,
        schema_version: &str,
        plaintext: &[u8],
    ) -> Result<PayloadRef, StoreError> {
        let content_hash = format!("sha256:{}", hex::encode(Sha256::digest(plaintext)));
        let path = self.cas_path(kind, schema_version, &content_hash)?;
        if !path.exists() {
            let encrypted = self.encrypt(kind, schema_version, &content_hash, plaintext)?;
            Self::persist_cas(&path, &encrypted)?;
        }
        let retained = self.decrypt(
            kind,
            schema_version,
            &content_hash,
            self.key_version,
            &fs::read(path)?,
        )?;
        if retained != plaintext {
            return Err(StoreError::Authentication);
        }
        Ok(PayloadRef {
            content_hash,
            kind: kind.to_owned(),
            schema_version: schema_version.to_owned(),
            byte_count: u64::try_from(plaintext.len()).map_err(|_| StoreError::Inconsistent)?,
            key_version: self.key_version,
        })
    }

    fn authenticate_help_run_creation(
        &self,
        identity: &DesktopLocalIdentity,
        stored: &StoredHelpRunCreation,
        replayed: bool,
    ) -> Result<BmadHelpRunCreationReceipt, StoreError> {
        match self.authenticate_help_run(identity, stored, replayed)? {
            BmadHelpRunLatest::Retained(receipt) => Ok(receipt),
            BmadHelpRunLatest::None | BmadHelpRunLatest::LegacyProjectionUnavailable => {
                Err(StoreError::Inconsistent)
            }
        }
    }

    fn authenticate_latest_help_run(
        &self,
        identity: &DesktopLocalIdentity,
        stored: &StoredHelpRunCreation,
    ) -> Result<BmadHelpRunLatest, StoreError> {
        self.authenticate_help_run(identity, stored, true)
    }

    fn authenticate_help_run(
        &self,
        identity: &DesktopLocalIdentity,
        stored: &StoredHelpRunCreation,
        replayed: bool,
    ) -> Result<BmadHelpRunLatest, StoreError> {
        let scope = MethodSessionScope {
            owner_scope_ref: parse_contract_id(&stored.owner_scope_ref)?,
            project_id: parse_contract_id(&stored.project_id)?,
            run_id: parse_contract_id(&stored.run_id)?,
            authority_ref: identity
                .authority_ref()
                .map_err(|_| StoreError::Inconsistent)?,
        };
        let session_id = parse_contract_id(&stored.session_id)?;
        let session = self
            .load_method_session(&scope, &session_id)?
            .ok_or(StoreError::Inconsistent)?;
        let renderer_projection = stored_help_run_renderer_projection(stored)?
            .as_ref()
            .map(|reference| self.get_payload(reference))
            .transpose()?;
        let verified = verified_method_session(&session)?;
        verify_help_run_creation(stored, &verified, identity, renderer_projection.as_deref())?;
        let Some(renderer_projection) = renderer_projection else {
            return Ok(BmadHelpRunLatest::LegacyProjectionUnavailable);
        };
        Ok(BmadHelpRunLatest::Retained(BmadHelpRunCreationReceipt {
            request_id: parse_contract_id(&stored.request_id)?,
            session_id,
            run_id: scope.run_id,
            accepted_at: UnixMillis(stored.accepted_at),
            replayed,
            renderer_projection,
        }))
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

fn validate_help_run_request(request: &BmadHelpRunCreateRequest) -> Result<(), StoreError> {
    if request.workspace_grant_epoch == 0
        || request.workspace_grant_epoch > MAX_SAFE_JSON_INTEGER
        || validate_workspace_catalog_version(request.workspace_catalog_version).is_err()
        || request.renderer_projection.is_empty()
        || request.renderer_projection.len() > MAX_BMAD_HELP_RUN_RENDERER_PROJECTION_BYTES
        || request.accepted_at.0 > MAX_SAFE_JSON_INTEGER
    {
        return Err(StoreError::StateConflict);
    }
    Ok(())
}

fn validate_workspace_catalog_version(version: u64) -> Result<(), StoreError> {
    if version == 0 || version > MAX_SAFE_JSON_INTEGER {
        Err(StoreError::StateConflict)
    } else {
        Ok(())
    }
}

fn validate_replay_request(request: &BmadHelpRunReplayRequest) -> Result<(), StoreError> {
    if request.workspace_grant_epoch == 0 || request.workspace_grant_epoch > MAX_SAFE_JSON_INTEGER {
        return Err(StoreError::StateConflict);
    }
    Ok(())
}

fn verify_workspace_catalog_version(
    transaction: &rusqlite::Transaction<'_>,
    expected_version: u64,
) -> Result<(), StoreError> {
    let retained = transaction
        .query_row(
            "SELECT version FROM aggregates
             WHERE aggregate_type = 'workspace_catalog' AND aggregate_id = 'local'",
            [],
            |row| row.get::<_, u64>(0),
        )
        .optional()?;
    if retained == Some(expected_version) {
        Ok(())
    } else {
        Err(StoreError::WorkspaceAuthorityStale)
    }
}

fn next_help_run_creation_ordinal(
    transaction: &rusqlite::Transaction<'_>,
) -> Result<u64, StoreError> {
    let current = transaction.query_row(
        "SELECT MAX(creation_ordinal) FROM bmad_help_run_creations",
        [],
        |row| row.get::<_, Option<u64>>(0),
    )?;
    current
        .unwrap_or(0)
        .checked_add(1)
        .filter(|ordinal| *ordinal <= MAX_SAFE_JSON_INTEGER)
        .ok_or(StoreError::Inconsistent)
}

fn help_run_request_fingerprint(
    request: &BmadHelpRunCreateRequest,
) -> Result<Sha256Digest, StoreError> {
    canonical_hash(
        HELP_RUN_CREATE_PURPOSE,
        1,
        &BmadHelpRunCreateFingerprint {
            schema_version: "sapphirus.bmad-help-run-create-request.v1",
            run_kind: "bmad_help",
            request_id: &request.request_id,
            project_id: &request.project_id,
            workspace_id: &request.workspace_id,
            workspace_grant_epoch: request.workspace_grant_epoch,
            workspace_root_identity_hash: request.workspace_root_identity_hash,
            capability_catalog_hash: request.capability_catalog_hash,
            foundation_binding_hash: request.foundation_binding_hash,
            intent_hash: request.intent_hash,
        },
    )
    .map_err(|_| StoreError::Inconsistent)
}

fn help_run_projection_binding_hash(
    request_fingerprint: Sha256Digest,
    session_id: &ContractId,
    run_id: &ContractId,
    workspace_catalog_version: u64,
    creation_ordinal: u64,
    accepted_at: UnixMillis,
    renderer_projection_hash: Sha256Digest,
) -> Result<Sha256Digest, StoreError> {
    canonical_hash(
        HELP_RUN_PROJECTION_BINDING_PURPOSE,
        1,
        &BmadHelpRunProjectionBinding {
            schema_version: "sapphirus.bmad-help-run-projection-binding.v1",
            request_fingerprint,
            session_id,
            run_id,
            workspace_catalog_version,
            creation_ordinal,
            accepted_at: accepted_at.0,
            renderer_projection_hash,
        },
    )
    .map_err(|_| StoreError::Inconsistent)
}

fn validate_new_help_run_candidate(
    candidate: &MethodSession,
    request: &BmadHelpRunCreateRequest,
    identity: &DesktopLocalIdentity,
) -> Result<String, StoreError> {
    let scope = candidate.scope();
    if candidate.version() != 1
        || candidate.state() != desktop_runtime::MethodState::Created
        || candidate.resume().is_some()
        || candidate.current_binding().is_ok()
        || scope.project_id != request.project_id
        || scope.owner_scope_ref != *identity.owner_scope_ref()
        || scope.authority_ref
            != identity
                .authority_ref()
                .map_err(|_| StoreError::Inconsistent)?
    {
        return Err(StoreError::StateConflict);
    }
    let state_json = candidate.to_persisted_json()?;
    if method_created_at(&state_json)? != request.accepted_at.0 {
        return Err(StoreError::StateConflict);
    }
    Ok(state_json)
}

fn register_payload_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    payload: &PayloadRef,
    occurred_at: &str,
) -> Result<(), StoreError> {
    transaction.execute(
        "INSERT OR IGNORE INTO payloads
         (content_hash, kind, schema_version, byte_count, key_version, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            payload.content_hash,
            payload.kind,
            payload.schema_version,
            payload.byte_count,
            payload.key_version,
            occurred_at,
        ],
    )?;
    let retained = transaction
        .query_row(
            "SELECT byte_count, key_version FROM payloads
             WHERE content_hash = ?1 AND kind = ?2 AND schema_version = ?3",
            params![payload.content_hash, payload.kind, payload.schema_version],
            |row| Ok((row.get::<_, u64>(0)?, row.get::<_, u32>(1)?)),
        )
        .optional()?;
    if retained == Some((payload.byte_count, payload.key_version)) {
        Ok(())
    } else {
        Err(StoreError::Inconsistent)
    }
}

fn insert_created_method_session(
    transaction: &rusqlite::Transaction<'_>,
    session: &MethodSession,
    payload: &PayloadRef,
    occurred_at: &str,
) -> Result<(), StoreError> {
    let scope = session.scope();
    let inserted = transaction.execute(
        "INSERT INTO bmad_method_sessions
         (session_id, owner_scope_ref, project_id, run_id, authority_id, version, state,
          state_content_hash, state_kind, state_schema_version, state_byte_count,
          state_key_version, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            session.session_id().as_str(),
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
        Ok(1) => Ok(()),
        Ok(_) => Err(StoreError::Inconsistent),
        Err(error) if is_unique_violation(&error) => Err(StoreError::StateConflict),
        Err(error) => Err(StoreError::Sqlite(error)),
    }
}

fn insert_help_run_creation(
    transaction: &rusqlite::Transaction<'_>,
    row: &NewHelpRunCreationRow<'_>,
) -> Result<(), StoreError> {
    let scope = row.session.scope();
    let inserted = transaction.execute(
        "INSERT INTO bmad_help_run_creations
         (owner_scope_ref, installation_id, request_id, request_fingerprint,
          session_id, project_id, run_id, authority_id, authority_epoch,
          local_store_id, workspace_id, workspace_grant_epoch,
          workspace_catalog_version, workspace_root_identity_hash, capability_catalog_hash,
          foundation_binding_hash, intent_hash, renderer_projection_state,
          renderer_projection_content_hash,
          renderer_projection_kind, renderer_projection_schema_version,
          renderer_projection_byte_count, renderer_projection_key_version,
          renderer_projection_binding_hash, creation_ordinal, accepted_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                 ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)",
        params![
            row.identity.owner_scope_ref().as_str(),
            row.identity.installation_id().as_str(),
            row.request.request_id.as_str(),
            row.request_fingerprint.to_string(),
            row.session.session_id().as_str(),
            scope.project_id.as_str(),
            scope.run_id.as_str(),
            row.identity.authority_id().as_str(),
            row.identity.authority_epoch(),
            row.identity.local_store_id().as_str(),
            row.request.workspace_id.as_str(),
            row.request.workspace_grant_epoch,
            row.request.workspace_catalog_version,
            row.request.workspace_root_identity_hash.to_string(),
            row.request.capability_catalog_hash.to_string(),
            row.request.foundation_binding_hash.to_string(),
            row.request.intent_hash.to_string(),
            HELP_RUN_RENDERER_PROJECTION_RETAINED,
            row.renderer_projection.content_hash,
            row.renderer_projection.kind,
            row.renderer_projection.schema_version,
            row.renderer_projection.byte_count,
            row.renderer_projection.key_version,
            row.renderer_projection_binding_hash.to_string(),
            row.creation_ordinal,
            row.request.accepted_at.0,
        ],
    );
    match inserted {
        Ok(1) => Ok(()),
        Ok(_) => Err(StoreError::Inconsistent),
        Err(error) if is_unique_violation(&error) => Err(StoreError::StateConflict),
        Err(error) => Err(StoreError::Sqlite(error)),
    }
}

fn query_help_run_creation(
    connection: &rusqlite::Connection,
    owner_scope_ref: &str,
    installation_id: &str,
    request_id: &str,
) -> Result<Option<StoredHelpRunCreation>, StoreError> {
    connection
        .query_row(
            "SELECT owner_scope_ref, installation_id, request_id, request_fingerprint,
                    session_id, project_id, run_id, authority_id, authority_epoch,
                    local_store_id, workspace_id, workspace_grant_epoch,
                    workspace_catalog_version, workspace_root_identity_hash, capability_catalog_hash,
                    foundation_binding_hash, intent_hash, renderer_projection_state,
                    renderer_projection_content_hash,
                    renderer_projection_kind, renderer_projection_schema_version,
                    renderer_projection_byte_count, renderer_projection_key_version,
                    renderer_projection_binding_hash, creation_ordinal, accepted_at
             FROM bmad_help_run_creations
             WHERE owner_scope_ref = ?1 AND installation_id = ?2 AND request_id = ?3",
            params![owner_scope_ref, installation_id, request_id],
            stored_help_run_creation_from_row,
        )
        .optional()
        .map_err(StoreError::from)
}

fn query_latest_help_run_creation(
    connection: &rusqlite::Connection,
    owner_scope_ref: &str,
    installation_id: &str,
    workspace_id: &str,
) -> Result<Option<StoredHelpRunCreation>, StoreError> {
    connection
        .query_row(
            "SELECT owner_scope_ref, installation_id, request_id, request_fingerprint,
                    session_id, project_id, run_id, authority_id, authority_epoch,
                    local_store_id, workspace_id, workspace_grant_epoch,
                    workspace_catalog_version, workspace_root_identity_hash, capability_catalog_hash,
                    foundation_binding_hash, intent_hash, renderer_projection_state,
                    renderer_projection_content_hash,
                    renderer_projection_kind, renderer_projection_schema_version,
                    renderer_projection_byte_count, renderer_projection_key_version,
                    renderer_projection_binding_hash, creation_ordinal, accepted_at
             FROM bmad_help_run_creations
             WHERE owner_scope_ref = ?1 AND installation_id = ?2 AND workspace_id = ?3
             ORDER BY creation_ordinal DESC
             LIMIT 1",
            params![owner_scope_ref, installation_id, workspace_id],
            stored_help_run_creation_from_row,
        )
        .optional()
        .map_err(StoreError::from)
}

fn stored_help_run_creation_from_row(
    row: &rusqlite::Row<'_>,
) -> Result<StoredHelpRunCreation, rusqlite::Error> {
    Ok(StoredHelpRunCreation {
        owner_scope_ref: row.get(0)?,
        installation_id: row.get(1)?,
        request_id: row.get(2)?,
        request_fingerprint: row.get(3)?,
        session_id: row.get(4)?,
        project_id: row.get(5)?,
        run_id: row.get(6)?,
        authority_id: row.get(7)?,
        authority_epoch: row.get(8)?,
        local_store_id: row.get(9)?,
        workspace_id: row.get(10)?,
        workspace_grant_epoch: row.get(11)?,
        workspace_catalog_version: row.get(12)?,
        workspace_root_identity_hash: row.get(13)?,
        capability_catalog_hash: row.get(14)?,
        foundation_binding_hash: row.get(15)?,
        intent_hash: row.get(16)?,
        renderer_projection_state: row.get(17)?,
        renderer_projection_content_hash: row.get(18)?,
        renderer_projection_kind: row.get(19)?,
        renderer_projection_schema_version: row.get(20)?,
        renderer_projection_byte_count: row.get(21)?,
        renderer_projection_key_version: row.get(22)?,
        renderer_projection_binding_hash: row.get(23)?,
        creation_ordinal: row.get(24)?,
        accepted_at: row.get(25)?,
    })
}

fn stored_help_run_renderer_projection(
    stored: &StoredHelpRunCreation,
) -> Result<Option<PayloadRef>, StoreError> {
    match (
        stored.renderer_projection_state.as_str(),
        stored.renderer_projection_content_hash.as_ref(),
        stored.renderer_projection_kind.as_ref(),
        stored.renderer_projection_schema_version.as_ref(),
        stored.renderer_projection_byte_count,
        stored.renderer_projection_key_version,
        stored.renderer_projection_binding_hash.as_ref(),
    ) {
        (HELP_RUN_RENDERER_PROJECTION_LEGACY_UNRETAINED, None, None, None, None, None, None) => {
            Ok(None)
        }
        (
            HELP_RUN_RENDERER_PROJECTION_RETAINED,
            Some(content_hash),
            Some(kind),
            Some(schema_version),
            Some(byte_count),
            Some(key_version),
            Some(_),
        ) => Ok(Some(PayloadRef {
            content_hash: content_hash.clone(),
            kind: kind.clone(),
            schema_version: schema_version.clone(),
            byte_count,
            key_version,
        })),
        _ => Err(StoreError::Inconsistent),
    }
}

fn verified_method_session(session: &MethodSession) -> Result<VerifiedMethodSession, StoreError> {
    let scope = session.scope();
    let source = session.to_persisted_json()?;
    Ok(VerifiedMethodSession {
        owner_scope_ref: scope.owner_scope_ref.to_string(),
        project_id: scope.project_id.to_string(),
        run_id: scope.run_id.to_string(),
        authority_kind: scope.authority_ref.authority_kind,
        authority_id: scope.authority_ref.authority_id.to_string(),
        installation_id: scope.authority_ref.installation_id.to_string(),
        local_store_id: scope.authority_ref.local_store_id.to_string(),
        authority_epoch: scope.authority_ref.authority_epoch,
        created_at: method_created_at(&source)?,
    })
}

fn method_created_at(source: &str) -> Result<u64, StoreError> {
    serde_json::from_str::<serde_json::Value>(source)?
        .as_object()
        .and_then(|value| value.get("createdAt"))
        .and_then(serde_json::Value::as_u64)
        .filter(|value| *value <= MAX_SAFE_JSON_INTEGER)
        .ok_or(StoreError::Inconsistent)
}

fn verify_help_run_creation(
    stored: &StoredHelpRunCreation,
    session: &VerifiedMethodSession,
    identity: &DesktopLocalIdentity,
    renderer_projection: Option<&[u8]>,
) -> Result<(), StoreError> {
    let request =
        stored_help_run_request(stored, renderer_projection.unwrap_or_default().to_vec())?;
    let session_id = parse_contract_id(&stored.session_id)?;
    let run_id = parse_contract_id(&stored.run_id)?;
    let request_fingerprint =
        Sha256Digest::parse(&stored.request_fingerprint).map_err(|_| StoreError::Inconsistent)?;
    verify_help_run_projection(
        stored,
        request_fingerprint,
        &session_id,
        &run_id,
        renderer_projection,
    )?;
    if stored.workspace_grant_epoch == 0
        || stored.workspace_grant_epoch > MAX_SAFE_JSON_INTEGER
        || stored.workspace_catalog_version == 0
        || stored.workspace_catalog_version > MAX_SAFE_JSON_INTEGER
        || stored.creation_ordinal == 0
        || stored.creation_ordinal > MAX_SAFE_JSON_INTEGER
        || stored.accepted_at > MAX_SAFE_JSON_INTEGER
        || stored.owner_scope_ref != identity.owner_scope_ref().as_str()
        || stored.installation_id != identity.installation_id().as_str()
        || stored.authority_id != identity.authority_id().as_str()
        || stored.authority_epoch != identity.authority_epoch()
        || stored.local_store_id != identity.local_store_id().as_str()
        || stored.owner_scope_ref != session.owner_scope_ref
        || stored.project_id != session.project_id
        || stored.run_id != session.run_id
        || session.authority_kind != "desktop_local_store"
        || stored.authority_id != session.authority_id
        || stored.installation_id != session.installation_id
        || stored.local_store_id != session.local_store_id
        || stored.authority_epoch != session.authority_epoch
        || stored.accepted_at != session.created_at
    {
        return Err(StoreError::Inconsistent);
    }
    let recomputed = help_run_request_fingerprint(&request)?;
    if recomputed != request_fingerprint {
        return Err(StoreError::Inconsistent);
    }
    Ok(())
}

fn verify_help_run_projection(
    stored: &StoredHelpRunCreation,
    request_fingerprint: Sha256Digest,
    session_id: &ContractId,
    run_id: &ContractId,
    renderer_projection: Option<&[u8]>,
) -> Result<(), StoreError> {
    let retained = stored_help_run_renderer_projection(stored)?;
    let (Some(reference), Some(renderer_projection)) = (retained.as_ref(), renderer_projection)
    else {
        return if retained.is_none() && renderer_projection.is_none() {
            Ok(())
        } else {
            Err(StoreError::Inconsistent)
        };
    };
    let renderer_projection_hash = desktop_runtime::sha256_bytes(renderer_projection);
    let retained_projection_binding_hash = Sha256Digest::parse(
        stored
            .renderer_projection_binding_hash
            .as_deref()
            .ok_or(StoreError::Inconsistent)?,
    )
    .map_err(|_| StoreError::Inconsistent)?;
    let expected_projection_binding_hash = help_run_projection_binding_hash(
        request_fingerprint,
        session_id,
        run_id,
        stored.workspace_catalog_version,
        stored.creation_ordinal,
        UnixMillis(stored.accepted_at),
        renderer_projection_hash,
    )?;
    let max_bytes = u64::try_from(MAX_BMAD_HELP_RUN_RENDERER_PROJECTION_BYTES)
        .map_err(|_| StoreError::Inconsistent)?;
    let actual_bytes =
        u64::try_from(renderer_projection.len()).map_err(|_| StoreError::Inconsistent)?;
    if reference.kind != HELP_RUN_RENDERER_PROJECTION_KIND
        || reference.schema_version != HELP_RUN_RENDERER_PROJECTION_SCHEMA
        || reference.byte_count == 0
        || reference.byte_count > max_bytes
        || reference.byte_count != actual_bytes
        || reference.content_hash != renderer_projection_hash.to_string()
        || retained_projection_binding_hash != expected_projection_binding_hash
    {
        Err(StoreError::Inconsistent)
    } else {
        Ok(())
    }
}

fn stored_help_run_request(
    stored: &StoredHelpRunCreation,
    renderer_projection: Vec<u8>,
) -> Result<BmadHelpRunCreateRequest, StoreError> {
    Ok(BmadHelpRunCreateRequest {
        request_id: parse_contract_id(&stored.request_id)?,
        project_id: parse_contract_id(&stored.project_id)?,
        workspace_id: parse_contract_id(&stored.workspace_id)?,
        workspace_grant_epoch: stored.workspace_grant_epoch,
        workspace_catalog_version: stored.workspace_catalog_version,
        workspace_root_identity_hash: Sha256Digest::parse(&stored.workspace_root_identity_hash)
            .map_err(|_| StoreError::Inconsistent)?,
        capability_catalog_hash: Sha256Digest::parse(&stored.capability_catalog_hash)
            .map_err(|_| StoreError::Inconsistent)?,
        foundation_binding_hash: Sha256Digest::parse(&stored.foundation_binding_hash)
            .map_err(|_| StoreError::Inconsistent)?,
        intent_hash: Sha256Digest::parse(&stored.intent_hash)
            .map_err(|_| StoreError::Inconsistent)?,
        renderer_projection,
        accepted_at: UnixMillis(stored.accepted_at),
    })
}

fn parse_contract_id(value: &str) -> Result<ContractId, StoreError> {
    ContractId::new(value.to_owned()).map_err(|_| StoreError::Inconsistent)
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
        let (expected_checkpoints, verified_sessions) =
            self.verify_method_session_rows(snapshot.sessions)?;
        self.verify_method_checkpoint_rows(snapshot.checkpoints, &expected_checkpoints)?;
        verify_method_receipt_rows(snapshot.receipts)?;
        let identity = self.load_local_identity()?;
        verify_help_run_creation_rows(
            self,
            &snapshot.help_run_creations,
            snapshot.help_run_events,
            &verified_sessions,
            &identity,
        )
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
    ) -> Result<(ExpectedCheckpoints, VerifiedMethodSessions), StoreError> {
        let mut expected_checkpoints = BTreeMap::new();
        let mut verified_sessions = BTreeMap::new();
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
            if verified_sessions
                .insert(row.session_id.clone(), verified_method_session(&session)?)
                .is_some()
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
        Ok((expected_checkpoints, verified_sessions))
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
        help_run_creations: load_help_run_creation_rows(connection)?,
        help_run_events: load_help_run_evidence_rows(connection)?,
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

fn load_help_run_creation_rows(
    connection: &rusqlite::Connection,
) -> Result<Vec<StoredHelpRunCreation>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT owner_scope_ref, installation_id, request_id, request_fingerprint,
                session_id, project_id, run_id, authority_id, authority_epoch,
                local_store_id, workspace_id, workspace_grant_epoch,
                workspace_catalog_version, workspace_root_identity_hash, capability_catalog_hash,
                foundation_binding_hash, intent_hash, renderer_projection_state,
                renderer_projection_content_hash,
                renderer_projection_kind, renderer_projection_schema_version,
                renderer_projection_byte_count, renderer_projection_key_version,
                renderer_projection_binding_hash, creation_ordinal, accepted_at
         FROM bmad_help_run_creations
         ORDER BY owner_scope_ref, installation_id, request_id",
    )?;
    let rows = statement
        .query_map([], stored_help_run_creation_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn load_help_run_evidence_rows(
    connection: &rusqlite::Connection,
) -> Result<Vec<HelpRunEvidenceIntegrityRow>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT e.stream_id, e.sequence, e.payload_hash, e.payload_ref,
                p.byte_count, p.key_version, e.correlation_id
         FROM evidence_events e
         JOIN payloads p
           ON p.content_hash = e.payload_hash
          AND p.kind = ?1
          AND p.schema_version = ?2
         WHERE e.event_type = 'bmad.help.run.created'
         ORDER BY e.stream_id, e.sequence",
    )?;
    let rows = statement
        .query_map(params![METHOD_STATE_KIND, METHOD_STATE_SCHEMA], |row| {
            Ok(HelpRunEvidenceIntegrityRow {
                stream_id: row.get(0)?,
                sequence: row.get(1)?,
                payload: PayloadRef {
                    content_hash: row.get(2)?,
                    kind: METHOD_STATE_KIND.to_owned(),
                    schema_version: METHOD_STATE_SCHEMA.to_owned(),
                    byte_count: row.get(4)?,
                    key_version: row.get(5)?,
                },
                payload_ref: row.get(3)?,
                correlation_id: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn verify_help_run_creation_rows(
    store: &LocalStore,
    rows: &[StoredHelpRunCreation],
    events: Vec<HelpRunEvidenceIntegrityRow>,
    sessions: &VerifiedMethodSessions,
    identity: &DesktopLocalIdentity,
) -> Result<(), StoreError> {
    if rows.len() != events.len() {
        return Err(StoreError::Inconsistent);
    }
    let mut receipts = BTreeMap::new();
    for row in rows {
        let session = sessions
            .get(&row.session_id)
            .ok_or(StoreError::Inconsistent)?;
        let renderer_projection = stored_help_run_renderer_projection(row)?
            .as_ref()
            .map(|reference| store.get_payload(reference))
            .transpose()?;
        verify_help_run_creation(row, session, identity, renderer_projection.as_deref())?;
        if receipts.insert(row.session_id.as_str(), row).is_some() {
            return Err(StoreError::Inconsistent);
        }
    }
    let mut matched = BTreeSet::new();
    for event in events {
        let session_id = event
            .stream_id
            .strip_prefix("bmad-method:")
            .ok_or(StoreError::Inconsistent)?;
        let receipt = receipts.get(session_id).ok_or(StoreError::Inconsistent)?;
        if event.sequence != 1
            || event.stream_id != format!("bmad-method:{}", receipt.session_id)
            || event.correlation_id != receipt.request_id
            || event.payload_ref != payload_uri(&event.payload)?
            || !matched.insert(session_id.to_owned())
        {
            return Err(StoreError::Inconsistent);
        }
        let bytes = store.get_payload(&event.payload)?;
        let source = std::str::from_utf8(&bytes).map_err(|_| StoreError::Inconsistent)?;
        let initial = MethodSession::from_persisted_json(source)?;
        let request = stored_help_run_request(receipt, Vec::new())?;
        if initial.session_id().as_str() != receipt.session_id
            || initial.scope().run_id.as_str() != receipt.run_id
            || validate_new_help_run_candidate(&initial, &request, identity)? != source
        {
            return Err(StoreError::Inconsistent);
        }
    }
    if matched.len() != rows.len() {
        return Err(StoreError::Inconsistent);
    }
    Ok(())
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
