use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{canonical_hash, AuthorityRef, ContractId, Sha256Digest, UnixMillis};

use super::{MethodContextDecision, MethodError, MethodErrorCode, MethodExactBinding};

const METHOD_SESSION_SCHEMA: &str = "sapphirus.bmad-method-session-state.v1";
const MAX_IDEMPOTENCY_BYTES: usize = 128;
const MAX_STEP_KEY_BYTES: usize = 128;
const MAX_WORKING_ARTIFACT_REFS: usize = 128;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MethodState {
    Created,
    CapabilityBound,
    ContextReviewRequired,
    Ready,
    Advancing,
    AwaitingUser,
    Completed,
    Refused,
    Incomplete,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MethodSessionScope {
    pub owner_scope_ref: ContractId,
    pub project_id: ContractId,
    pub run_id: ContractId,
    pub authority_ref: AuthorityRef,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MethodArtifactProvenance {
    pub session_id: ContractId,
    pub scope: MethodSessionScope,
    pub binding_ordinal: u64,
    pub binding_hash: Sha256Digest,
    pub decision_id: ContractId,
    pub invocation_id: ContractId,
}

#[derive(Clone, Debug)]
pub struct CreateMethodSession {
    pub session_id: ContractId,
    pub owner_scope_ref: ContractId,
    pub project_id: ContractId,
    pub run_id: ContractId,
    pub authority_ref: AuthorityRef,
    pub created_at: UnixMillis,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MethodStepTable {
    initial_step_key: String,
    steps: BTreeMap<String, Option<String>>,
    table_hash: Sha256Digest,
}

impl MethodStepTable {
    /// Creates a closed, capability-specific step table.
    ///
    /// # Errors
    ///
    /// Rejects duplicate, missing, cyclic, or malformed step keys.
    pub fn new<'a, I>(initial_step_key: &'a str, steps: I) -> Result<Self, MethodError>
    where
        I: IntoIterator<Item = (&'a str, Option<&'a str>)>,
    {
        if !valid_step_key(initial_step_key) {
            return Err(MethodError::new(MethodErrorCode::MethodResultInvalid));
        }
        let mut normalized = BTreeMap::new();
        for (current, next) in steps {
            if !valid_step_key(current) || next.is_some_and(|value| !valid_step_key(value)) {
                return Err(MethodError::new(MethodErrorCode::MethodResultInvalid));
            }
            if normalized
                .insert(current.to_owned(), next.map(str::to_owned))
                .is_some()
            {
                return Err(MethodError::new(MethodErrorCode::MethodResultInvalid));
            }
        }
        if normalized.is_empty() || !normalized.contains_key(initial_step_key) {
            return Err(MethodError::new(MethodErrorCode::MethodResultInvalid));
        }
        for next in normalized.values().flatten() {
            if !normalized.contains_key(next) {
                return Err(MethodError::new(MethodErrorCode::MethodResultInvalid));
            }
        }
        validate_acyclic(initial_step_key, &normalized)?;
        let hash_input = (&initial_step_key, &normalized);
        let table_hash = canonical_hash("bmad-method-step-table", 1, &hash_input)?;
        Ok(Self {
            initial_step_key: initial_step_key.to_owned(),
            steps: normalized,
            table_hash,
        })
    }

    #[must_use]
    pub fn table_hash(&self) -> &Sha256Digest {
        &self.table_hash
    }

    fn expected_next(&self, current: &str) -> Option<&Option<String>> {
        self.steps.get(current)
    }

    fn verify(&self) -> Result<(), MethodError> {
        validate_acyclic(&self.initial_step_key, &self.steps)?;
        let hash_input = (&self.initial_step_key.as_str(), &self.steps);
        if canonical_hash("bmad-method-step-table", 1, &hash_input)? != self.table_hash {
            return Err(MethodError::new(
                MethodErrorCode::MethodStoreRecoveryRequired,
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MethodAdvanceRequest {
    pub invocation_id: ContractId,
    pub idempotency_key: String,
    pub decision_id: ContractId,
    pub decision_consumption_hash: Sha256Digest,
    pub model_request_id: ContractId,
    pub model_request_hash: Sha256Digest,
    pub session_authority_hash: Sha256Digest,
    pub d2_model_invocation_binding_hash: Sha256Digest,
    pub model_bridge_binding_hash: Sha256Digest,
    pub expected_version: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MethodAdvanceReceipt {
    pub consumption_id: ContractId,
    pub decision_id: ContractId,
    pub invocation_id: ContractId,
    pub idempotency_key: String,
    pub decision_consumption_hash: Sha256Digest,
    pub model_request_id: ContractId,
    pub model_request_hash: Sha256Digest,
    pub session_authority_hash: Sha256Digest,
    pub d2_model_invocation_binding_hash: Sha256Digest,
    pub model_bridge_binding_hash: Sha256Digest,
    pub aggregate_version: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DecisionConsumptionIdInput<'a> {
    session_id: &'a ContractId,
    decision_id: &'a ContractId,
    invocation_id: &'a ContractId,
    idempotency_key: &'a str,
    decision_consumption_hash: &'a Sha256Digest,
    model_request_id: &'a ContractId,
    model_request_hash: &'a Sha256Digest,
    session_authority_hash: &'a Sha256Digest,
    d2_model_invocation_binding_hash: &'a Sha256Digest,
    model_bridge_binding_hash: &'a Sha256Digest,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MethodSessionAuthorityHashInput<'a> {
    session_id: &'a ContractId,
    scope: &'a MethodSessionScope,
    method_binding_hash: &'a Sha256Digest,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
// The canonical preimage keys intentionally state that every bridge input is a hash.
#[allow(clippy::struct_field_names)]
struct MethodModelBridgeBindingHashInput<'a> {
    session_authority_hash: &'a Sha256Digest,
    d2_model_invocation_binding_hash: &'a Sha256Digest,
    method_binding_hash: &'a Sha256Digest,
    model_binding_hash: &'a Sha256Digest,
    response_schema_hash: &'a Sha256Digest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MethodDecisionConsumption {
    decision: MethodContextDecision,
    receipt: MethodAdvanceReceipt,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MethodAdvanceDisposition {
    AwaitingUser,
    ContextReviewRequired,
    Completed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MethodPersistenceEvent {
    CapabilityBound,
    CapabilityRebound,
    ContextReviewRequested,
    ContextReviewAccepted,
    ResultAccepted,
    UserTurnRecorded,
    Refused,
    Incomplete,
    Cancelled,
}

impl MethodPersistenceEvent {
    #[must_use]
    pub const fn event_type(self) -> &'static str {
        match self {
            Self::CapabilityBound => "bmad.method.capability_bound",
            Self::CapabilityRebound => "bmad.method.capability_rebound",
            Self::ContextReviewRequested => "bmad.method.context_review_requested",
            Self::ContextReviewAccepted => "bmad.method.context_review_accepted",
            Self::ResultAccepted => "bmad.method.result_accepted",
            Self::UserTurnRecorded => "bmad.method.user_turn_recorded",
            Self::Refused => "bmad.method.refused",
            Self::Incomplete => "bmad.method.incomplete",
            Self::Cancelled => "bmad.method.cancelled",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MethodAdvanceResult {
    pub disposition: MethodAdvanceDisposition,
    pub current_step_key: String,
    pub next_step_key: Option<String>,
    pub working_artifact_refs: Vec<String>,
}

impl MethodAdvanceResult {
    /// Parses the closed model-result boundary. Unknown fields fail closed.
    ///
    /// # Errors
    ///
    /// Returns `method_result_invalid` for malformed or authority-bearing input.
    pub fn parse_json(source: &[u8]) -> Result<Self, MethodError> {
        serde_json::from_slice(source)
            .map_err(|_| MethodError::new(MethodErrorCode::MethodResultInvalid))
    }
}

/// Exact pre-call lineage plus post-call evidence supplied by trusted host code.
///
/// `accepted_method_result_hash` is the canonical accepted
/// [`MethodAdvanceResult`] projection. `model_response_payload_hash` is the
/// SHA-256 hash of D2's exact raw JSON bytes. `model_receipt_evidence_hash` is
/// reserved for a trusted-host canonical hash of the complete, already-verified
/// D2 receipt using purpose `model-access-receipt-evidence`, version 1. BMAD
/// retains and self-binds these values but does not independently authenticate
/// their transport preimages in this runtime-only foundation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MethodVerifiedResultBindingData {
    pub invocation_id: ContractId,
    pub decision_id: ContractId,
    pub decision_consumption_hash: Sha256Digest,
    pub model_request_id: ContractId,
    pub model_request_hash: Sha256Digest,
    pub session_authority_hash: Sha256Digest,
    pub d2_model_invocation_binding_hash: Sha256Digest,
    pub model_bridge_binding_hash: Sha256Digest,
    pub method_binding_hash: Sha256Digest,
    pub model_binding_hash: Sha256Digest,
    pub response_schema_hash: Sha256Digest,
    pub model_response_payload_hash: Sha256Digest,
    pub accepted_method_result_hash: Sha256Digest,
    pub model_receipt_evidence_hash: Sha256Digest,
}

/// A BMAD result whose accepted projection and verified lineage are sealed.
///
/// This envelope intentionally does not implement `Deserialize`: untrusted
/// model or renderer bytes cannot mint transition authority. This is a trusted
/// Rust-host anti-drift boundary, not cryptographic proof that D2 verification
/// ran. Actual composition remains blocked until D2 exposes opaque verified
/// output and production receipt verification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodVerifiedAdvanceResult {
    result: MethodAdvanceResult,
    binding: MethodVerifiedResultBindingData,
    verification_hash: Sha256Digest,
}

impl MethodVerifiedAdvanceResult {
    /// Seals trusted-host evidence to an accepted BMAD result projection.
    ///
    /// # Errors
    ///
    /// Returns `method_result_invalid` when `accepted_method_result_hash` is
    /// not the canonical hash of the accepted BMAD projection. This constructor
    /// does not authenticate the raw payload or receipt evidence preimages.
    pub fn from_trusted_host_evidence(
        result: MethodAdvanceResult,
        binding: MethodVerifiedResultBindingData,
    ) -> Result<Self, MethodError> {
        if method_advance_result_hash(&result)? != binding.accepted_method_result_hash {
            return Err(MethodError::new(MethodErrorCode::MethodResultInvalid));
        }
        let verification_hash = verified_result_binding_hash(&binding)?;
        Ok(Self {
            result,
            binding,
            verification_hash,
        })
    }

    #[must_use]
    pub const fn result(&self) -> &MethodAdvanceResult {
        &self.result
    }

    #[must_use]
    pub const fn binding(&self) -> &MethodVerifiedResultBindingData {
        &self.binding
    }

    #[must_use]
    pub const fn verification_hash(&self) -> &Sha256Digest {
        &self.verification_hash
    }

    pub(super) fn verify(&self) -> Result<(), MethodError> {
        if method_advance_result_hash(&self.result)? != self.binding.accepted_method_result_hash
            || verified_result_binding_hash(&self.binding)? != self.verification_hash
        {
            return Err(MethodError::new(MethodErrorCode::MethodResultInvalid));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MethodCheckpoint {
    pub checkpoint_id: ContractId,
    pub turn_ordinal: u64,
    pub binding_ordinal: u64,
    pub invocation_id: ContractId,
    pub method_binding_hash: Sha256Digest,
    pub capability_step_table_hash: Sha256Digest,
    pub advance_disposition: MethodAdvanceDisposition,
    pub current_step_key: String,
    pub next_step_key: Option<String>,
    pub context_decision_id: ContractId,
    pub context_digest: Sha256Digest,
    pub decision_consumption_hash: Sha256Digest,
    pub model_request_id: ContractId,
    pub model_request_hash: Sha256Digest,
    pub session_authority_hash: Sha256Digest,
    pub d2_model_invocation_binding_hash: Sha256Digest,
    pub model_bridge_binding_hash: Sha256Digest,
    pub model_binding_hash: Sha256Digest,
    pub response_schema_hash: Sha256Digest,
    pub model_response_payload_hash: Sha256Digest,
    pub accepted_method_result_hash: Sha256Digest,
    pub model_receipt_evidence_hash: Sha256Digest,
    pub verified_result_binding_hash: Sha256Digest,
    pub working_artifact_refs: Vec<String>,
    pub recorded_at: UnixMillis,
    pub checkpoint_hash: Sha256Digest,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CheckpointHashInput<'a> {
    checkpoint_id: &'a ContractId,
    turn_ordinal: u64,
    binding_ordinal: u64,
    invocation_id: &'a ContractId,
    method_binding_hash: &'a Sha256Digest,
    capability_step_table_hash: &'a Sha256Digest,
    advance_disposition: MethodAdvanceDisposition,
    current_step_key: &'a str,
    next_step_key: &'a Option<String>,
    context_decision_id: &'a ContractId,
    context_digest: &'a Sha256Digest,
    decision_consumption_hash: &'a Sha256Digest,
    model_request_id: &'a ContractId,
    model_request_hash: &'a Sha256Digest,
    session_authority_hash: &'a Sha256Digest,
    d2_model_invocation_binding_hash: &'a Sha256Digest,
    model_bridge_binding_hash: &'a Sha256Digest,
    model_binding_hash: &'a Sha256Digest,
    response_schema_hash: &'a Sha256Digest,
    model_response_payload_hash: &'a Sha256Digest,
    accepted_method_result_hash: &'a Sha256Digest,
    model_receipt_evidence_hash: &'a Sha256Digest,
    verified_result_binding_hash: &'a Sha256Digest,
    working_artifact_refs: &'a [String],
    recorded_at: UnixMillis,
}

impl MethodCheckpoint {
    fn hash_input(&self) -> CheckpointHashInput<'_> {
        CheckpointHashInput {
            checkpoint_id: &self.checkpoint_id,
            turn_ordinal: self.turn_ordinal,
            binding_ordinal: self.binding_ordinal,
            invocation_id: &self.invocation_id,
            method_binding_hash: &self.method_binding_hash,
            capability_step_table_hash: &self.capability_step_table_hash,
            advance_disposition: self.advance_disposition,
            current_step_key: &self.current_step_key,
            next_step_key: &self.next_step_key,
            context_decision_id: &self.context_decision_id,
            context_digest: &self.context_digest,
            decision_consumption_hash: &self.decision_consumption_hash,
            model_request_id: &self.model_request_id,
            model_request_hash: &self.model_request_hash,
            session_authority_hash: &self.session_authority_hash,
            d2_model_invocation_binding_hash: &self.d2_model_invocation_binding_hash,
            model_bridge_binding_hash: &self.model_bridge_binding_hash,
            model_binding_hash: &self.model_binding_hash,
            response_schema_hash: &self.response_schema_hash,
            model_response_payload_hash: &self.model_response_payload_hash,
            accepted_method_result_hash: &self.accepted_method_result_hash,
            model_receipt_evidence_hash: &self.model_receipt_evidence_hash,
            verified_result_binding_hash: &self.verified_result_binding_hash,
            working_artifact_refs: &self.working_artifact_refs,
            recorded_at: self.recorded_at,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MethodRendererProjection {
    pub session_id: ContractId,
    pub state: MethodState,
    pub version: u64,
    pub turn_ordinal: u64,
    pub current_step_key: Option<String>,
    pub recoverable: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MethodBindingRevision {
    ordinal: u64,
    binding: MethodExactBinding,
    step_table: MethodStepTable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MethodSession {
    schema_version: String,
    session_id: ContractId,
    scope: MethodSessionScope,
    created_at: UnixMillis,
    state: MethodState,
    version: u64,
    turn_ordinal: u64,
    binding_ordinal: u64,
    binding_history: Vec<MethodBindingRevision>,
    exact_binding: Option<MethodExactBinding>,
    step_table: Option<MethodStepTable>,
    current_step_key: Option<String>,
    pending_review: Option<MethodContextDecision>,
    active_invocation: Option<MethodAdvanceReceipt>,
    consumed_decisions: BTreeMap<ContractId, MethodDecisionConsumption>,
    idempotent_advances: BTreeMap<String, MethodAdvanceReceipt>,
    checkpoints: Vec<MethodCheckpoint>,
}

impl MethodSession {
    /// Creates an authority-owned, non-runnable Method session.
    ///
    /// # Errors
    ///
    /// Rejects non-local or malformed authority data.
    pub fn create(input: CreateMethodSession) -> Result<Self, MethodError> {
        if input.authority_ref.authority_kind != "desktop_local_store"
            || input.authority_ref.authority_epoch == 0
        {
            return Err(MethodError::new(MethodErrorCode::MethodBindingStale));
        }
        Ok(Self {
            schema_version: METHOD_SESSION_SCHEMA.to_owned(),
            session_id: input.session_id,
            scope: MethodSessionScope {
                owner_scope_ref: input.owner_scope_ref,
                project_id: input.project_id,
                run_id: input.run_id,
                authority_ref: input.authority_ref,
            },
            created_at: input.created_at,
            state: MethodState::Created,
            version: 1,
            turn_ordinal: 0,
            binding_ordinal: 0,
            binding_history: Vec::new(),
            exact_binding: None,
            step_table: None,
            current_step_key: None,
            pending_review: None,
            active_invocation: None,
            consumed_decisions: BTreeMap::new(),
            idempotent_advances: BTreeMap::new(),
            checkpoints: Vec::new(),
        })
    }

    #[must_use]
    pub const fn state(&self) -> MethodState {
        self.state
    }

    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    #[must_use]
    pub const fn turn_ordinal(&self) -> u64 {
        self.turn_ordinal
    }

    #[must_use]
    pub fn session_id(&self) -> ContractId {
        self.session_id.clone()
    }

    #[must_use]
    pub fn scope(&self) -> MethodSessionScope {
        self.scope.clone()
    }

    #[must_use]
    pub fn checkpoints(&self) -> &[MethodCheckpoint] {
        &self.checkpoints
    }

    /// Returns the current host binding for persistence and drift checks.
    ///
    /// # Errors
    ///
    /// Returns `method_binding_stale` before the first capability binding.
    pub fn current_binding(&self) -> Result<&MethodExactBinding, MethodError> {
        self.exact_binding
            .as_ref()
            .ok_or_else(|| MethodError::new(MethodErrorCode::MethodBindingStale))
    }

    /// Derives the exact authority hash that D2 must bind to this Method session.
    ///
    /// # Errors
    ///
    /// Returns `method_binding_stale` before capability binding or when the
    /// exact Method binding cannot be hashed.
    pub fn session_authority_hash(&self) -> Result<Sha256Digest, MethodError> {
        let method_binding_hash = self.current_binding()?.binding_hash()?;
        method_session_authority_hash(&self.session_id, &self.scope, &method_binding_hash)
    }

    /// Binds one opaque D2 model-invocation binding to current Method authority.
    ///
    /// # Errors
    ///
    /// Returns `method_binding_stale` before capability binding or when exact
    /// Method authority cannot be hashed.
    pub fn model_bridge_binding_hash(
        &self,
        d2_model_invocation_binding_hash: &Sha256Digest,
    ) -> Result<Sha256Digest, MethodError> {
        let binding = self.current_binding()?;
        let method_binding_hash = binding.binding_hash()?;
        let session_authority_hash =
            method_session_authority_hash(&self.session_id, &self.scope, &method_binding_hash)?;
        method_model_bridge_binding_hash(
            &session_authority_hash,
            d2_model_invocation_binding_hash,
            &method_binding_hash,
            &binding.model_binding_hash,
            &binding.model_binding.data.response_schema_hash,
        )
    }

    /// Binds an artifact to this authority/session and one exact invocation.
    ///
    /// # Errors
    ///
    /// Returns `method_result_invalid` when the invocation is not active or checkpointed.
    pub fn artifact_provenance_for(
        &self,
        invocation_id: &ContractId,
    ) -> Result<MethodArtifactProvenance, MethodError> {
        let (binding_ordinal, decision_id) = if let Some(active) = self
            .active_invocation
            .as_ref()
            .filter(|receipt| &receipt.invocation_id == invocation_id)
        {
            (self.binding_ordinal, active.decision_id.clone())
        } else {
            let checkpoint = self
                .checkpoints
                .iter()
                .find(|checkpoint| &checkpoint.invocation_id == invocation_id)
                .ok_or_else(|| MethodError::new(MethodErrorCode::MethodResultInvalid))?;
            (
                checkpoint.binding_ordinal,
                checkpoint.context_decision_id.clone(),
            )
        };
        let revision = self
            .binding_history
            .get(
                usize::try_from(binding_ordinal.saturating_sub(1))
                    .map_err(|_| MethodError::new(MethodErrorCode::MethodResultInvalid))?,
            )
            .ok_or_else(|| MethodError::new(MethodErrorCode::MethodResultInvalid))?;
        Ok(MethodArtifactProvenance {
            session_id: self.session_id.clone(),
            scope: self.scope.clone(),
            binding_ordinal,
            binding_hash: revision.binding.binding_hash()?,
            decision_id,
            invocation_id: invocation_id.clone(),
        })
    }

    /// Binds the exact package/model inputs and handwritten step table.
    ///
    /// # Errors
    ///
    /// Returns a stable Method error for stale state or invalid bindings.
    pub fn bind_capability(
        &mut self,
        expected_version: u64,
        binding: MethodExactBinding,
        step_table: MethodStepTable,
    ) -> Result<(), MethodError> {
        self.require(expected_version, MethodState::Created)?;
        binding.validate()?;
        self.current_step_key = Some(step_table.initial_step_key.clone());
        self.binding_history.push(MethodBindingRevision {
            ordinal: 1,
            binding: binding.clone(),
            step_table: step_table.clone(),
        });
        self.exact_binding = Some(binding);
        self.step_table = Some(step_table);
        self.binding_ordinal = 1;
        self.transition(MethodState::CapabilityBound)
    }

    /// Replaces drifted exact inputs and invalidates any pending review.
    ///
    /// # Errors
    ///
    /// Returns a stable conflict or binding error while a model call is active
    /// or when the aggregate version/input is invalid.
    pub fn rebind_capability(
        &mut self,
        expected_version: u64,
        binding: MethodExactBinding,
        step_table: MethodStepTable,
    ) -> Result<(), MethodError> {
        if self.version != expected_version
            || matches!(
                self.state,
                MethodState::Created
                    | MethodState::Advancing
                    | MethodState::Completed
                    | MethodState::Refused
                    | MethodState::Incomplete
                    | MethodState::Cancelled
            )
        {
            return Err(MethodError::new(MethodErrorCode::MethodStateConflict));
        }
        binding.validate()?;
        self.binding_ordinal = self
            .binding_ordinal
            .checked_add(1)
            .ok_or_else(|| MethodError::new(MethodErrorCode::MethodStateConflict))?;
        self.current_step_key = Some(step_table.initial_step_key.clone());
        self.binding_history.push(MethodBindingRevision {
            ordinal: self.binding_ordinal,
            binding: binding.clone(),
            step_table: step_table.clone(),
        });
        self.exact_binding = Some(binding);
        self.step_table = Some(step_table);
        self.pending_review = None;
        self.transition(MethodState::CapabilityBound)
    }

    /// Invalidates prior review and enters context review.
    ///
    /// # Errors
    ///
    /// Returns `method_state_conflict` unless the expected transition is current.
    pub fn request_context_review(&mut self, expected_version: u64) -> Result<(), MethodError> {
        if self.version != expected_version
            || !matches!(
                self.state,
                MethodState::CapabilityBound | MethodState::AwaitingUser
            )
        {
            return Err(MethodError::new(MethodErrorCode::MethodStateConflict));
        }
        self.pending_review = None;
        self.transition(MethodState::ContextReviewRequired)
    }

    /// Accepts a host-reviewed decision bound to the exact current inputs.
    ///
    /// # Errors
    ///
    /// Returns a stable Method error for stale state, binding drift, or replay.
    pub fn record_context_review(
        &mut self,
        expected_version: u64,
        decision: MethodContextDecision,
    ) -> Result<(), MethodError> {
        self.require(expected_version, MethodState::ContextReviewRequired)?;
        let binding = self
            .exact_binding
            .as_ref()
            .ok_or_else(|| MethodError::new(MethodErrorCode::MethodBindingStale))?;
        if decision.binding_hash != binding.binding_hash()?
            || self.consumed_decisions.contains_key(&decision.decision_id)
        {
            return Err(MethodError::new(MethodErrorCode::MethodBindingStale));
        }
        self.pending_review = Some(decision);
        self.transition(MethodState::Ready)
    }

    /// Checks that the pending review still binds every exact input.
    ///
    /// # Errors
    ///
    /// Returns the stable resource, model, or general binding drift code.
    pub fn validate_review_for(&self, binding: &MethodExactBinding) -> Result<(), MethodError> {
        let current = self
            .exact_binding
            .as_ref()
            .ok_or_else(|| MethodError::new(MethodErrorCode::MethodBindingStale))?;
        if current.resource_set_hash != binding.resource_set_hash {
            return Err(MethodError::new(MethodErrorCode::MethodResourceDrift));
        }
        if current.model_binding_hash != binding.model_binding_hash
            || current.egress_profile_hash != binding.egress_profile_hash
        {
            return Err(MethodError::new(MethodErrorCode::MethodModelBindingDrift));
        }
        let decision = self
            .pending_review
            .as_ref()
            .ok_or_else(|| MethodError::new(MethodErrorCode::MethodBindingStale))?;
        if decision.binding_hash != binding.binding_hash()? {
            return Err(MethodError::new(MethodErrorCode::MethodBindingStale));
        }
        Ok(())
    }

    /// Atomically consumes the pending decision in aggregate memory.
    ///
    /// # Errors
    ///
    /// Returns a stable conflict, replay, or stale-binding error.
    pub fn begin_advance(
        &mut self,
        request: MethodAdvanceRequest,
    ) -> Result<MethodAdvanceReceipt, MethodError> {
        validate_idempotency_key(&request.idempotency_key)?;
        if let Some(existing) = self.idempotent_advances.get(&request.idempotency_key) {
            if existing.invocation_id == request.invocation_id
                && existing.decision_id == request.decision_id
                && existing.decision_consumption_hash == request.decision_consumption_hash
                && existing.model_request_id == request.model_request_id
                && existing.model_request_hash == request.model_request_hash
                && existing.session_authority_hash == request.session_authority_hash
                && existing.d2_model_invocation_binding_hash
                    == request.d2_model_invocation_binding_hash
                && existing.model_bridge_binding_hash == request.model_bridge_binding_hash
                && request
                    .expected_version
                    .checked_add(1)
                    .is_some_and(|version| version == existing.aggregate_version)
            {
                return Ok(existing.clone());
            }
            return Err(MethodError::new(MethodErrorCode::MethodStateConflict));
        }
        if self.consumed_decisions.contains_key(&request.decision_id) {
            return Err(MethodError::new(
                MethodErrorCode::ContextDecisionAlreadyConsumed,
            ));
        }
        self.require(request.expected_version, MethodState::Ready)?;
        let review = self
            .pending_review
            .as_ref()
            .ok_or_else(|| MethodError::new(MethodErrorCode::MethodBindingStale))?;
        if review.decision_id != request.decision_id {
            return Err(MethodError::new(MethodErrorCode::MethodBindingStale));
        }
        let expected_session_authority_hash = self.session_authority_hash()?;
        let expected_model_bridge_binding_hash =
            self.model_bridge_binding_hash(&request.d2_model_invocation_binding_hash)?;
        if request.session_authority_hash != expected_session_authority_hash
            || request.model_bridge_binding_hash != expected_model_bridge_binding_hash
        {
            return Err(MethodError::new(MethodErrorCode::MethodBindingStale));
        }
        let next_version = self
            .version
            .checked_add(1)
            .ok_or_else(|| MethodError::new(MethodErrorCode::MethodStateConflict))?;
        let consumption_id = decision_consumption_id(&DecisionConsumptionIdInput {
            session_id: &self.session_id,
            decision_id: &request.decision_id,
            invocation_id: &request.invocation_id,
            idempotency_key: &request.idempotency_key,
            decision_consumption_hash: &request.decision_consumption_hash,
            model_request_id: &request.model_request_id,
            model_request_hash: &request.model_request_hash,
            session_authority_hash: &request.session_authority_hash,
            d2_model_invocation_binding_hash: &request.d2_model_invocation_binding_hash,
            model_bridge_binding_hash: &request.model_bridge_binding_hash,
        })?;
        let consumed_decision = review.clone();
        let receipt = MethodAdvanceReceipt {
            consumption_id,
            decision_id: request.decision_id,
            invocation_id: request.invocation_id,
            idempotency_key: request.idempotency_key,
            decision_consumption_hash: request.decision_consumption_hash,
            model_request_id: request.model_request_id,
            model_request_hash: request.model_request_hash,
            session_authority_hash: request.session_authority_hash,
            d2_model_invocation_binding_hash: request.d2_model_invocation_binding_hash,
            model_bridge_binding_hash: request.model_bridge_binding_hash,
            aggregate_version: next_version,
        };
        self.pending_review = None;
        self.state = MethodState::Advancing;
        self.version = next_version;
        self.active_invocation = Some(receipt.clone());
        self.consumed_decisions.insert(
            receipt.decision_id.clone(),
            MethodDecisionConsumption {
                decision: consumed_decision,
                receipt: receipt.clone(),
            },
        );
        self.idempotent_advances
            .insert(receipt.idempotency_key.clone(), receipt.clone());
        Ok(receipt)
    }

    /// Accepts sealed model content only when every exact binding matches and
    /// the result follows the handwritten step table.
    ///
    /// # Errors
    ///
    /// Returns `method_result_invalid` for invented steps/content or a stable
    /// conflict when the invocation/version is stale.
    pub fn accept_result(
        &mut self,
        expected_version: u64,
        verified_result: MethodVerifiedAdvanceResult,
        recorded_at: UnixMillis,
    ) -> Result<MethodCheckpoint, MethodError> {
        let (receipt, next_state) =
            self.validate_advance_result(expected_version, &verified_result)?;
        let checkpoint = self.build_checkpoint(&receipt, &verified_result, recorded_at)?;
        let next_version = self
            .version
            .checked_add(1)
            .ok_or_else(|| MethodError::new(MethodErrorCode::MethodStateConflict))?;
        self.current_step_key = verified_result.result.next_step_key;
        self.turn_ordinal = checkpoint.turn_ordinal;
        self.state = next_state;
        self.version = next_version;
        self.active_invocation = None;
        self.checkpoints.push(checkpoint.clone());
        Ok(checkpoint)
    }

    fn validate_advance_result(
        &self,
        expected_version: u64,
        verified_result: &MethodVerifiedAdvanceResult,
    ) -> Result<(MethodAdvanceReceipt, MethodState), MethodError> {
        self.require(expected_version, MethodState::Advancing)?;
        verified_result.verify()?;
        let proof = verified_result.binding();
        let result = verified_result.result();
        let receipt = self
            .active_invocation
            .clone()
            .ok_or_else(|| MethodError::new(MethodErrorCode::MethodResultInvalid))?;
        let binding = self
            .exact_binding
            .as_ref()
            .ok_or_else(|| MethodError::new(MethodErrorCode::MethodBindingStale))?;
        let method_binding_hash = binding.binding_hash()?;
        let expected_session_authority_hash =
            method_session_authority_hash(&self.session_id, &self.scope, &method_binding_hash)?;
        let expected_model_bridge_binding_hash = method_model_bridge_binding_hash(
            &expected_session_authority_hash,
            &proof.d2_model_invocation_binding_hash,
            &method_binding_hash,
            &binding.model_binding_hash,
            &binding.model_binding.data.response_schema_hash,
        )?;
        if receipt.invocation_id != proof.invocation_id
            || receipt.decision_id != proof.decision_id
            || receipt.decision_consumption_hash != proof.decision_consumption_hash
            || receipt.model_request_id != proof.model_request_id
            || receipt.model_request_hash != proof.model_request_hash
            || receipt.session_authority_hash != proof.session_authority_hash
            || receipt.d2_model_invocation_binding_hash != proof.d2_model_invocation_binding_hash
            || receipt.model_bridge_binding_hash != proof.model_bridge_binding_hash
            || proof.session_authority_hash != expected_session_authority_hash
            || proof.model_bridge_binding_hash != expected_model_bridge_binding_hash
            || method_binding_hash != proof.method_binding_hash
            || binding.model_binding_hash != proof.model_binding_hash
            || binding.model_binding.data.response_schema_hash != proof.response_schema_hash
        {
            return Err(MethodError::new(MethodErrorCode::MethodResultInvalid));
        }
        let step_table = self
            .step_table
            .as_ref()
            .ok_or_else(|| MethodError::new(MethodErrorCode::MethodResultInvalid))?;
        let current = self
            .current_step_key
            .as_deref()
            .ok_or_else(|| MethodError::new(MethodErrorCode::MethodResultInvalid))?;
        let expected_next = step_table
            .expected_next(current)
            .ok_or_else(|| MethodError::new(MethodErrorCode::MethodResultInvalid))?;
        if result.current_step_key != current
            || &result.next_step_key != expected_next
            || expected_next.is_none()
                != (result.disposition == MethodAdvanceDisposition::Completed)
            || result.working_artifact_refs.len() > MAX_WORKING_ARTIFACT_REFS
            || result
                .working_artifact_refs
                .iter()
                .any(|value| !valid_artifact_ref(value))
            || result
                .working_artifact_refs
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(MethodError::new(MethodErrorCode::MethodResultInvalid));
        }
        let next_state = match result.disposition {
            MethodAdvanceDisposition::AwaitingUser => MethodState::AwaitingUser,
            MethodAdvanceDisposition::ContextReviewRequired => MethodState::ContextReviewRequired,
            MethodAdvanceDisposition::Completed => {
                if result.next_step_key.is_some() {
                    return Err(MethodError::new(MethodErrorCode::MethodResultInvalid));
                }
                MethodState::Completed
            }
        };
        Ok((receipt, next_state))
    }

    fn build_checkpoint(
        &self,
        receipt: &MethodAdvanceReceipt,
        verified_result: &MethodVerifiedAdvanceResult,
        recorded_at: UnixMillis,
    ) -> Result<MethodCheckpoint, MethodError> {
        let result = verified_result.result();
        let proof = verified_result.binding();
        let step_table = self
            .step_table
            .as_ref()
            .ok_or_else(|| MethodError::new(MethodErrorCode::MethodResultInvalid))?;
        let next_turn = self
            .turn_ordinal
            .checked_add(1)
            .ok_or_else(|| MethodError::new(MethodErrorCode::MethodStateConflict))?;
        let checkpoint_id_digest = canonical_hash(
            "bmad-method-checkpoint-id",
            1,
            &(&self.session_id, next_turn, &receipt.invocation_id),
        )?;
        let checkpoint_id = ContractId::new(format!(
            "checkpoint_{}",
            checkpoint_id_digest
                .to_string()
                .trim_start_matches("sha256:")
        ))
        .map_err(|_| MethodError::new(MethodErrorCode::MethodResultInvalid))?;
        let consumption = self
            .consumed_decisions
            .get(&receipt.decision_id)
            .ok_or_else(|| MethodError::new(MethodErrorCode::MethodResultInvalid))?;
        let context_digest = &consumption.decision.context_digest;
        let hash_input = CheckpointHashInput {
            checkpoint_id: &checkpoint_id,
            turn_ordinal: next_turn,
            binding_ordinal: self.binding_ordinal,
            invocation_id: &receipt.invocation_id,
            method_binding_hash: &proof.method_binding_hash,
            capability_step_table_hash: step_table.table_hash(),
            advance_disposition: result.disposition,
            current_step_key: &result.current_step_key,
            next_step_key: &result.next_step_key,
            context_decision_id: &receipt.decision_id,
            context_digest,
            decision_consumption_hash: &proof.decision_consumption_hash,
            model_request_id: &proof.model_request_id,
            model_request_hash: &proof.model_request_hash,
            session_authority_hash: &proof.session_authority_hash,
            d2_model_invocation_binding_hash: &proof.d2_model_invocation_binding_hash,
            model_bridge_binding_hash: &proof.model_bridge_binding_hash,
            model_binding_hash: &proof.model_binding_hash,
            response_schema_hash: &proof.response_schema_hash,
            model_response_payload_hash: &proof.model_response_payload_hash,
            accepted_method_result_hash: &proof.accepted_method_result_hash,
            model_receipt_evidence_hash: &proof.model_receipt_evidence_hash,
            verified_result_binding_hash: verified_result.verification_hash(),
            working_artifact_refs: &result.working_artifact_refs,
            recorded_at,
        };
        let checkpoint_hash = checkpoint_hash(&hash_input)?;
        let checkpoint = MethodCheckpoint {
            checkpoint_id,
            turn_ordinal: next_turn,
            binding_ordinal: self.binding_ordinal,
            invocation_id: receipt.invocation_id.clone(),
            method_binding_hash: proof.method_binding_hash,
            capability_step_table_hash: *step_table.table_hash(),
            advance_disposition: result.disposition,
            current_step_key: result.current_step_key.clone(),
            next_step_key: result.next_step_key.clone(),
            context_decision_id: receipt.decision_id.clone(),
            context_digest: *context_digest,
            decision_consumption_hash: proof.decision_consumption_hash,
            model_request_id: proof.model_request_id.clone(),
            model_request_hash: proof.model_request_hash,
            session_authority_hash: proof.session_authority_hash,
            d2_model_invocation_binding_hash: proof.d2_model_invocation_binding_hash,
            model_bridge_binding_hash: proof.model_bridge_binding_hash,
            model_binding_hash: proof.model_binding_hash,
            response_schema_hash: proof.response_schema_hash,
            model_response_payload_hash: proof.model_response_payload_hash,
            accepted_method_result_hash: proof.accepted_method_result_hash,
            model_receipt_evidence_hash: proof.model_receipt_evidence_hash,
            verified_result_binding_hash: *verified_result.verification_hash(),
            working_artifact_refs: result.working_artifact_refs.clone(),
            recorded_at,
            checkpoint_hash,
        };
        Ok(checkpoint)
    }

    /// Records that another user turn requires a fresh context review.
    ///
    /// # Errors
    ///
    /// Returns `method_state_conflict` for a stale/non-awaiting session.
    pub fn record_user_turn(&mut self, expected_version: u64) -> Result<(), MethodError> {
        self.require(expected_version, MethodState::AwaitingUser)?;
        self.transition(MethodState::ContextReviewRequired)
    }

    /// Records a terminal model refusal without creating a checkpoint.
    ///
    /// # Errors
    ///
    /// Returns `method_state_conflict` unless an invocation is advancing.
    pub fn record_refusal(&mut self, expected_version: u64) -> Result<(), MethodError> {
        self.require(expected_version, MethodState::Advancing)?;
        self.active_invocation = None;
        self.transition(MethodState::Refused)
    }

    /// Records a terminal incomplete model result without inventing completion.
    ///
    /// # Errors
    ///
    /// Returns `method_state_conflict` unless an invocation is advancing.
    pub fn record_incomplete(&mut self, expected_version: u64) -> Result<(), MethodError> {
        self.require(expected_version, MethodState::Advancing)?;
        self.active_invocation = None;
        self.transition(MethodState::Incomplete)
    }

    /// Cancels a non-terminal session without reviving consumed decisions.
    ///
    /// # Errors
    ///
    /// Returns `method_state_conflict` for stale or terminal sessions.
    pub fn cancel(&mut self, expected_version: u64) -> Result<(), MethodError> {
        if self.version != expected_version
            || !matches!(
                self.state,
                MethodState::CapabilityBound
                    | MethodState::ContextReviewRequired
                    | MethodState::Ready
                    | MethodState::Advancing
                    | MethodState::AwaitingUser
            )
        {
            return Err(MethodError::new(MethodErrorCode::MethodStateConflict));
        }
        self.pending_review = None;
        self.active_invocation = None;
        self.transition(MethodState::Cancelled)
    }

    #[must_use]
    pub fn resume(&self) -> Option<&MethodCheckpoint> {
        self.checkpoints.last()
    }

    #[must_use]
    pub fn renderer_projection(&self) -> MethodRendererProjection {
        MethodRendererProjection {
            session_id: self.session_id.clone(),
            state: self.state,
            version: self.version,
            turn_ordinal: self.turn_ordinal,
            current_step_key: self.current_step_key.clone(),
            recoverable: matches!(
                self.state,
                MethodState::ContextReviewRequired
                    | MethodState::Ready
                    | MethodState::AwaitingUser
                    | MethodState::Incomplete
                    | MethodState::Refused
            ),
        }
    }

    /// Serializes the authority state for encrypted host persistence.
    ///
    /// # Errors
    ///
    /// Returns a recovery-required error if serialization fails.
    pub fn to_persisted_json(&self) -> Result<String, MethodError> {
        serde_json::to_string(self)
            .map_err(|_| MethodError::new(MethodErrorCode::MethodStoreRecoveryRequired))
    }

    /// Reconstructs and validates authority state loaded from encrypted persistence.
    ///
    /// # Errors
    ///
    /// Returns a recovery-required error for malformed or internally inconsistent state.
    pub fn from_persisted_json(source: &str) -> Result<Self, MethodError> {
        let value: Self = serde_json::from_str(source)
            .map_err(|_| MethodError::new(MethodErrorCode::MethodStoreRecoveryRequired))?;
        value.validate_restored()?;
        Ok(value)
    }

    fn validate_restored(&self) -> Result<(), MethodError> {
        if self.schema_version != METHOD_SESSION_SCHEMA || self.version == 0 {
            return Err(recovery_error());
        }
        if self.turn_ordinal
            != u64::try_from(self.checkpoints.len()).map_err(|_| recovery_error())?
            || self.exact_binding.is_some() != self.step_table.is_some()
        {
            return Err(recovery_error());
        }
        self.validate_binding_history()?;
        self.validate_restored_consumptions()?;
        self.validate_restored_checkpoints()?;
        self.validate_restored_state_shape()
    }

    fn validate_binding_history(&self) -> Result<(), MethodError> {
        if self.binding_history.len()
            != usize::try_from(self.binding_ordinal).map_err(|_| recovery_error())?
        {
            return Err(recovery_error());
        }
        for (index, revision) in self.binding_history.iter().enumerate() {
            let expected_ordinal = u64::try_from(index)
                .map_err(|_| recovery_error())?
                .checked_add(1)
                .ok_or_else(recovery_error)?;
            if revision.ordinal != expected_ordinal {
                return Err(recovery_error());
            }
            revision.binding.validate().map_err(|_| recovery_error())?;
            revision.step_table.verify().map_err(|_| recovery_error())?;
        }
        if let Some(current) = self.binding_history.last() {
            if self.exact_binding.as_ref() != Some(&current.binding)
                || self.step_table.as_ref() != Some(&current.step_table)
            {
                return Err(recovery_error());
            }
        }
        Ok(())
    }

    fn validate_restored_consumptions(&self) -> Result<(), MethodError> {
        for (decision, consumption) in &self.consumed_decisions {
            let receipt = &consumption.receipt;
            let expected_consumption_id = decision_consumption_id(&DecisionConsumptionIdInput {
                session_id: &self.session_id,
                decision_id: &receipt.decision_id,
                invocation_id: &receipt.invocation_id,
                idempotency_key: &receipt.idempotency_key,
                decision_consumption_hash: &receipt.decision_consumption_hash,
                model_request_id: &receipt.model_request_id,
                model_request_hash: &receipt.model_request_hash,
                session_authority_hash: &receipt.session_authority_hash,
                d2_model_invocation_binding_hash: &receipt.d2_model_invocation_binding_hash,
                model_bridge_binding_hash: &receipt.model_bridge_binding_hash,
            })
            .map_err(|_| recovery_error())?;
            let mut matching_binding = None;
            for revision in &self.binding_history {
                if revision
                    .binding
                    .binding_hash()
                    .map_err(|_| recovery_error())?
                    == consumption.decision.binding_hash
                {
                    matching_binding = Some(&revision.binding);
                    break;
                }
            }
            let binding = matching_binding.ok_or_else(recovery_error)?;
            let expected_session_authority_hash = method_session_authority_hash(
                &self.session_id,
                &self.scope,
                &consumption.decision.binding_hash,
            )
            .map_err(|_| recovery_error())?;
            let expected_model_bridge_binding_hash = method_model_bridge_binding_hash(
                &expected_session_authority_hash,
                &receipt.d2_model_invocation_binding_hash,
                &consumption.decision.binding_hash,
                &binding.model_binding_hash,
                &binding.model_binding.data.response_schema_hash,
            )
            .map_err(|_| recovery_error())?;
            if decision != &consumption.decision.decision_id
                || decision != &receipt.decision_id
                || receipt.consumption_id != expected_consumption_id
                || receipt.session_authority_hash != expected_session_authority_hash
                || receipt.model_bridge_binding_hash != expected_model_bridge_binding_hash
                || self.idempotent_advances.get(&receipt.idempotency_key) != Some(receipt)
            {
                return Err(recovery_error());
            }
        }
        if self.idempotent_advances.len() != self.consumed_decisions.len() {
            return Err(recovery_error());
        }
        Ok(())
    }

    fn validate_restored_checkpoint_result(
        checkpoint: &MethodCheckpoint,
        revision: &MethodBindingRevision,
    ) -> Result<(), MethodError> {
        let expected_next_step = revision
            .step_table
            .expected_next(&checkpoint.current_step_key)
            .ok_or_else(recovery_error)?;
        let restored_result = MethodAdvanceResult {
            disposition: checkpoint.advance_disposition,
            current_step_key: checkpoint.current_step_key.clone(),
            next_step_key: checkpoint.next_step_key.clone(),
            working_artifact_refs: checkpoint.working_artifact_refs.clone(),
        };
        let expected_result_hash =
            method_advance_result_hash(&restored_result).map_err(|_| recovery_error())?;
        if &checkpoint.next_step_key != expected_next_step
            || expected_next_step.is_none()
                != (checkpoint.advance_disposition == MethodAdvanceDisposition::Completed)
            || checkpoint.working_artifact_refs.len() > MAX_WORKING_ARTIFACT_REFS
            || checkpoint
                .working_artifact_refs
                .iter()
                .any(|value| !valid_artifact_ref(value))
            || checkpoint
                .working_artifact_refs
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || checkpoint.accepted_method_result_hash != expected_result_hash
        {
            return Err(recovery_error());
        }
        Ok(())
    }

    fn validate_restored_checkpoints(&self) -> Result<(), MethodError> {
        for (index, checkpoint) in self.checkpoints.iter().enumerate() {
            let expected_ordinal = u64::try_from(index)
                .map_err(|_| recovery_error())?
                .checked_add(1)
                .ok_or_else(recovery_error)?;
            let consumption = self
                .consumed_decisions
                .get(&checkpoint.context_decision_id)
                .ok_or_else(recovery_error)?;
            let revision = self
                .binding_history
                .get(
                    usize::try_from(checkpoint.binding_ordinal.saturating_sub(1))
                        .map_err(|_| recovery_error())?,
                )
                .ok_or_else(recovery_error)?;
            let expected_method_hash = revision
                .binding
                .binding_hash()
                .map_err(|_| recovery_error())?;
            let expected_model_hash = &revision.binding.model_binding_hash;
            let expected_response_schema_hash =
                &revision.binding.model_binding.data.response_schema_hash;
            let expected_table_hash = revision.step_table.table_hash();
            Self::validate_restored_checkpoint_result(checkpoint, revision)?;
            let expected_session_authority_hash =
                method_session_authority_hash(&self.session_id, &self.scope, &expected_method_hash)
                    .map_err(|_| recovery_error())?;
            let expected_model_bridge_binding_hash = method_model_bridge_binding_hash(
                &expected_session_authority_hash,
                &checkpoint.d2_model_invocation_binding_hash,
                &expected_method_hash,
                expected_model_hash,
                expected_response_schema_hash,
            )
            .map_err(|_| recovery_error())?;
            let restored_verified_binding = MethodVerifiedResultBindingData {
                invocation_id: checkpoint.invocation_id.clone(),
                decision_id: checkpoint.context_decision_id.clone(),
                decision_consumption_hash: checkpoint.decision_consumption_hash,
                model_request_id: checkpoint.model_request_id.clone(),
                model_request_hash: checkpoint.model_request_hash,
                session_authority_hash: checkpoint.session_authority_hash,
                d2_model_invocation_binding_hash: checkpoint.d2_model_invocation_binding_hash,
                model_bridge_binding_hash: checkpoint.model_bridge_binding_hash,
                method_binding_hash: checkpoint.method_binding_hash,
                model_binding_hash: checkpoint.model_binding_hash,
                response_schema_hash: checkpoint.response_schema_hash,
                model_response_payload_hash: checkpoint.model_response_payload_hash,
                accepted_method_result_hash: checkpoint.accepted_method_result_hash,
                model_receipt_evidence_hash: checkpoint.model_receipt_evidence_hash,
            };
            let expected_verified_binding_hash =
                verified_result_binding_hash(&restored_verified_binding)
                    .map_err(|_| recovery_error())?;
            if checkpoint.turn_ordinal != expected_ordinal
                || checkpoint.binding_ordinal == 0
                || checkpoint.binding_ordinal > self.binding_ordinal
                || checkpoint.context_digest != consumption.decision.context_digest
                || consumption.decision.binding_hash != expected_method_hash
                || checkpoint.invocation_id != consumption.receipt.invocation_id
                || checkpoint.decision_consumption_hash
                    != consumption.receipt.decision_consumption_hash
                || checkpoint.model_request_id != consumption.receipt.model_request_id
                || checkpoint.model_request_hash != consumption.receipt.model_request_hash
                || checkpoint.session_authority_hash != consumption.receipt.session_authority_hash
                || checkpoint.d2_model_invocation_binding_hash
                    != consumption.receipt.d2_model_invocation_binding_hash
                || checkpoint.model_bridge_binding_hash
                    != consumption.receipt.model_bridge_binding_hash
                || checkpoint_hash(&checkpoint.hash_input())? != checkpoint.checkpoint_hash
                || checkpoint.method_binding_hash != expected_method_hash
                || checkpoint.session_authority_hash != expected_session_authority_hash
                || checkpoint.model_bridge_binding_hash != expected_model_bridge_binding_hash
                || &checkpoint.model_binding_hash != expected_model_hash
                || &checkpoint.response_schema_hash != expected_response_schema_hash
                || checkpoint.verified_result_binding_hash != expected_verified_binding_hash
                || &checkpoint.capability_step_table_hash != expected_table_hash
            {
                return Err(recovery_error());
            }
        }
        Ok(())
    }

    fn validate_restored_state_shape(&self) -> Result<(), MethodError> {
        let is_created = self.state == MethodState::Created;
        if is_created
            != (self.binding_history.is_empty()
                && self.exact_binding.is_none()
                && self.step_table.is_none()
                && self.current_step_key.is_none())
            || (!is_created
                && (self.binding_ordinal == 0
                    || self.exact_binding.is_none()
                    || self.step_table.is_none()))
            || (self.state == MethodState::Completed && self.current_step_key.is_some())
        {
            return Err(recovery_error());
        }
        if let Some(last) = self.checkpoints.last() {
            if last.binding_ordinal == self.binding_ordinal
                && self.current_step_key != last.next_step_key
            {
                return Err(recovery_error());
            }
        }
        if matches!(self.state, MethodState::Advancing) != self.active_invocation.is_some()
            || matches!(self.state, MethodState::Ready) != self.pending_review.is_some()
        {
            return Err(recovery_error());
        }
        if let Some(active) = &self.active_invocation {
            if self.idempotent_advances.get(&active.idempotency_key) != Some(active)
                || !self.consumed_decisions.contains_key(&active.decision_id)
            {
                return Err(recovery_error());
            }
        }
        if let (Some(review), Some(exact_binding)) = (&self.pending_review, &self.exact_binding) {
            if review.binding_hash != exact_binding.binding_hash()?
                || self.consumed_decisions.contains_key(&review.decision_id)
            {
                return Err(recovery_error());
            }
        }
        Ok(())
    }

    fn require(&self, expected_version: u64, state: MethodState) -> Result<(), MethodError> {
        if self.version != expected_version || self.state != state {
            return Err(MethodError::new(MethodErrorCode::MethodStateConflict));
        }
        Ok(())
    }

    fn transition(&mut self, state: MethodState) -> Result<(), MethodError> {
        self.version = self
            .version
            .checked_add(1)
            .ok_or_else(|| MethodError::new(MethodErrorCode::MethodStateConflict))?;
        self.state = state;
        Ok(())
    }
}

fn validate_acyclic(
    initial: &str,
    steps: &BTreeMap<String, Option<String>>,
) -> Result<(), MethodError> {
    let mut seen = BTreeSet::new();
    let mut current = Some(initial);
    while let Some(step) = current {
        if !seen.insert(step.to_owned()) {
            return Err(MethodError::new(MethodErrorCode::MethodResultInvalid));
        }
        current = steps.get(step).and_then(Option::as_deref);
    }
    if seen.len() != steps.len() {
        return Err(MethodError::new(MethodErrorCode::MethodResultInvalid));
    }
    Ok(())
}

fn checkpoint_hash(value: &CheckpointHashInput<'_>) -> Result<Sha256Digest, MethodError> {
    Ok(canonical_hash("bmad-method-checkpoint", 1, value)?)
}

fn decision_consumption_id(
    value: &DecisionConsumptionIdInput<'_>,
) -> Result<ContractId, MethodError> {
    let digest = canonical_hash("bmad-context-decision-consumption-id", 1, value)?;
    ContractId::new(format!(
        "consume_{}",
        digest.to_string().trim_start_matches("sha256:")
    ))
    .map_err(|_| MethodError::new(MethodErrorCode::MethodStateConflict))
}

fn method_session_authority_hash(
    session_id: &ContractId,
    scope: &MethodSessionScope,
    method_binding_hash: &Sha256Digest,
) -> Result<Sha256Digest, MethodError> {
    Ok(canonical_hash(
        "bmad-method-session-authority",
        1,
        &MethodSessionAuthorityHashInput {
            session_id,
            scope,
            method_binding_hash,
        },
    )?)
}

fn method_model_bridge_binding_hash(
    session_authority_hash: &Sha256Digest,
    d2_model_invocation_binding_hash: &Sha256Digest,
    method_binding_hash: &Sha256Digest,
    model_binding_hash: &Sha256Digest,
    response_schema_hash: &Sha256Digest,
) -> Result<Sha256Digest, MethodError> {
    Ok(canonical_hash(
        "bmad-method-d2-bridge-binding",
        1,
        &MethodModelBridgeBindingHashInput {
            session_authority_hash,
            d2_model_invocation_binding_hash,
            method_binding_hash,
            model_binding_hash,
            response_schema_hash,
        },
    )?)
}

fn method_advance_result_hash(result: &MethodAdvanceResult) -> Result<Sha256Digest, MethodError> {
    Ok(canonical_hash("bmad-method-advance-result", 1, result)?)
}

fn verified_result_binding_hash(
    binding: &MethodVerifiedResultBindingData,
) -> Result<Sha256Digest, MethodError> {
    Ok(canonical_hash(
        "bmad-method-verified-result-binding",
        1,
        binding,
    )?)
}

fn recovery_error() -> MethodError {
    MethodError::new(MethodErrorCode::MethodStoreRecoveryRequired)
}

fn valid_step_key(value: &str) -> bool {
    (1..=MAX_STEP_KEY_BYTES).contains(&value.len())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        })
}

fn validate_idempotency_key(value: &str) -> Result<(), MethodError> {
    if !(1..=MAX_IDEMPOTENCY_BYTES).contains(&value.len()) || value.chars().any(char::is_control) {
        return Err(MethodError::new(MethodErrorCode::MethodStateConflict));
    }
    Ok(())
}

fn valid_artifact_ref(value: &str) -> bool {
    (1..=256).contains(&value.len())
        && value.starts_with("cas://sha256/")
        && value.strip_prefix("cas://sha256/").is_some_and(|digest| {
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        })
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod verified_result_tests {
    use super::*;

    fn result() -> MethodAdvanceResult {
        MethodAdvanceResult {
            disposition: MethodAdvanceDisposition::Completed,
            current_step_key: "respond".to_owned(),
            next_step_key: None,
            working_artifact_refs: Vec::new(),
        }
    }

    fn binding(result: &MethodAdvanceResult) -> MethodVerifiedResultBindingData {
        MethodVerifiedResultBindingData {
            invocation_id: ContractId::new("invoke_01J00000000000000000000000")
                .expect("invocation id"),
            decision_id: ContractId::new("decision_01J00000000000000000000000")
                .expect("decision id"),
            decision_consumption_hash: crate::sha256_bytes(b"consumption"),
            model_request_id: ContractId::new("modelreq_01J00000000000000000000000")
                .expect("model request id"),
            model_request_hash: crate::sha256_bytes(b"request"),
            session_authority_hash: crate::sha256_bytes(b"session-authority"),
            d2_model_invocation_binding_hash: crate::sha256_bytes(b"d2-invocation-binding"),
            model_bridge_binding_hash: crate::sha256_bytes(b"model-bridge-binding"),
            method_binding_hash: crate::sha256_bytes(b"method-binding"),
            model_binding_hash: crate::sha256_bytes(b"model-binding"),
            response_schema_hash: crate::sha256_bytes(b"response-schema"),
            model_response_payload_hash: crate::sha256_bytes(b"exact-raw-json-bytes"),
            accepted_method_result_hash: canonical_hash("bmad-method-advance-result", 1, result)
                .expect("accepted result hash"),
            model_receipt_evidence_hash: crate::sha256_bytes(b"trusted-host-receipt-evidence"),
        }
    }

    #[test]
    fn sealed_result_rejects_inner_result_tampering() {
        let result = result();
        let mut verified = MethodVerifiedAdvanceResult::from_trusted_host_evidence(
            result.clone(),
            binding(&result),
        )
        .expect("verified result");
        verified.result.current_step_key = "tampered".to_owned();

        assert_eq!(
            verified
                .verify()
                .expect_err("accepted projection hash must be recomputed")
                .code(),
            MethodErrorCode::MethodResultInvalid
        );
    }

    #[test]
    fn sealed_result_rejects_verification_hash_tampering() {
        let result = result();
        let mut verified = MethodVerifiedAdvanceResult::from_trusted_host_evidence(
            result.clone(),
            binding(&result),
        )
        .expect("verified result");
        verified.verification_hash = crate::sha256_bytes(b"tampered-binding-hash");

        assert_eq!(
            verified
                .verify()
                .expect_err("sealed binding hash must be recomputed")
                .code(),
            MethodErrorCode::MethodResultInvalid
        );
    }

    #[test]
    fn sealed_result_rejects_receipt_evidence_hash_tampering() {
        let result = result();
        let mut verified = MethodVerifiedAdvanceResult::from_trusted_host_evidence(
            result.clone(),
            binding(&result),
        )
        .expect("verified result");
        verified.binding.model_receipt_evidence_hash =
            crate::sha256_bytes(b"different-d2-receipt-evidence");

        assert_eq!(
            verified
                .verify()
                .expect_err("trusted-host receipt evidence is part of the sealed binding")
                .code(),
            MethodErrorCode::MethodResultInvalid
        );
    }

    #[test]
    fn sealed_result_rejects_raw_payload_hash_tampering() {
        let result = result();
        let mut verified = MethodVerifiedAdvanceResult::from_trusted_host_evidence(
            result.clone(),
            binding(&result),
        )
        .expect("verified result");
        verified.binding.model_response_payload_hash =
            crate::sha256_bytes(b"different-raw-response-bytes");

        assert_eq!(
            verified
                .verify()
                .expect_err("the raw-response hash is part of the sealed binding")
                .code(),
            MethodErrorCode::MethodResultInvalid
        );
    }
}
