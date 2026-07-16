use std::{
    collections::{HashMap, HashSet},
    fmt,
};

use desktop_runtime::{canonical_hash, ContractId, Sha256Digest, UnixMillis};
use parking_lot::{Mutex, MutexGuard};
use serde::{Deserialize, Serialize};

use crate::{ContextEgressManifest, EgressError, RetentionMode};

const BINDING_SCHEMA: &str = "sapphirus.model-invocation-binding.v1";
const DECISION_SCHEMA: &str = "sapphirus.context-decision.v1";
const CONSUMPTION_SCHEMA: &str = "sapphirus.decision-consumption.v1";
const MAX_DECISION_LIFETIME_MS: u64 = 5 * 60 * 1_000;
const DEFAULT_MAX_TRACKED_DECISIONS: usize = 256;
const DEFAULT_MAX_SEEN_DECISION_IDS: usize = 4_096;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInvocationBindingDraft {
    pub schema_version: String,
    pub request_id: ContractId,
    pub tenant_ref: ContractId,
    pub project_ref: ContractId,
    pub run_ref: ContractId,
    pub installation_id: ContractId,
    pub session_authority_hash: Sha256Digest,
    pub manifest_hash: Sha256Digest,
    pub purpose: String,
    pub model_role: String,
    pub canonical_output_schema_id: ContractId,
    pub canonical_output_schema_hash: Sha256Digest,
    pub provider_profile_hash: Sha256Digest,
    pub model_profile_hash: Sha256Digest,
    pub deployment_hash: Sha256Digest,
    pub policy_hash: Sha256Digest,
    pub region: String,
    pub retention_mode: RetentionMode,
    pub consent_disclosure_hash: Sha256Digest,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInvocationBinding {
    #[serde(flatten)]
    pub draft: ModelInvocationBindingDraft,
    pub binding_hash: Sha256Digest,
}

impl ModelInvocationBindingDraft {
    /// Validates and seals all authority-owned model invocation inputs.
    ///
    /// # Errors
    ///
    /// Returns [`EgressError::InvalidInvocationBinding`] for an invalid shape
    /// or [`EgressError::CanonicalHash`] when canonical hashing fails.
    pub fn seal(self) -> Result<ModelInvocationBinding, EgressError> {
        validate_binding(&self)?;
        let binding_hash = canonical_hash("model-invocation-binding", 1, &self)
            .map_err(|_| EgressError::CanonicalHash)?;
        Ok(ModelInvocationBinding {
            draft: self,
            binding_hash,
        })
    }
}

impl ModelInvocationBinding {
    /// Revalidates the binding shape and canonical integrity hash.
    ///
    /// # Errors
    ///
    /// Returns [`EgressError`] when the binding is malformed or its integrity
    /// hash no longer matches.
    pub fn verify(&self) -> Result<(), EgressError> {
        validate_binding(&self.draft)?;
        let actual = canonical_hash("model-invocation-binding", 1, &self.draft)
            .map_err(|_| EgressError::CanonicalHash)?;
        if actual != self.binding_hash {
            return Err(EgressError::DecisionIntegrity);
        }
        Ok(())
    }

    /// Verifies that every model-controlled field matches the reviewed
    /// manifest exactly.
    ///
    /// # Errors
    ///
    /// Returns [`EgressError::DecisionBindingMismatch`] for any substitution.
    pub fn verify_for(&self, manifest: &ContextEgressManifest) -> Result<(), EgressError> {
        self.verify()?;
        manifest.verify()?;
        let binding = &self.draft;
        let reviewed = &manifest.draft;
        if binding.tenant_ref != reviewed.tenant_ref
            || binding.project_ref != reviewed.project_ref
            || binding.run_ref != reviewed.run_ref
            || binding.manifest_hash != manifest.manifest_hash
            || binding.purpose != reviewed.purpose
            || binding.model_role != reviewed.model_role
            || binding.canonical_output_schema_id != reviewed.canonical_output_schema_id
            || binding.canonical_output_schema_hash != reviewed.canonical_output_schema_hash
            || binding.provider_profile_hash != reviewed.provider_profile_hash
            || binding.model_profile_hash != reviewed.model_profile_hash
            || binding.deployment_hash != reviewed.deployment_hash
            || binding.policy_hash != reviewed.policy_hash
            || binding.region != reviewed.region
            || binding.retention_mode != reviewed.retention_mode
        {
            return Err(EgressError::DecisionBindingMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct PendingContextDecisionDraft {
    schema_version: String,
    decision_id: ContractId,
    manifest_hash: Sha256Digest,
    binding_hash: Sha256Digest,
    consent_disclosure_hash: Sha256Digest,
    policy_hash: Sha256Digest,
    installation_id: ContractId,
    session_authority_hash: Sha256Digest,
    issued_at: UnixMillis,
    expires_at: UnixMillis,
}

/// A pending decision is created only by [`ConsentService::approve`] and
/// cannot be cloned or retargeted by downstream code.
///
/// ```compile_fail
/// fn duplicate(decision: desktop_egress::PendingContextDecision) {
///     let replay = decision.clone();
/// }
/// ```
///
/// ```compile_fail
/// fn retarget(mut decision: desktop_egress::PendingContextDecision) {
///     decision.manifest_hash = desktop_runtime::sha256_bytes(b"other manifest");
///     decision.decision_hash = desktop_runtime::sha256_bytes(b"rehashed");
/// }
/// ```
#[derive(Eq, PartialEq)]
pub struct PendingContextDecision {
    schema_version: String,
    decision_id: ContractId,
    manifest_hash: Sha256Digest,
    binding_hash: Sha256Digest,
    consent_disclosure_hash: Sha256Digest,
    policy_hash: Sha256Digest,
    installation_id: ContractId,
    session_authority_hash: Sha256Digest,
    issued_at: UnixMillis,
    expires_at: UnixMillis,
    decision_hash: Sha256Digest,
}

impl fmt::Debug for PendingContextDecision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PendingContextDecision")
            .finish_non_exhaustive()
    }
}

impl PendingContextDecision {
    fn from_draft(draft: PendingContextDecisionDraft) -> Result<Self, EgressError> {
        validate_decision_lifetime(draft.issued_at, draft.expires_at)?;
        let decision_hash = canonical_hash("context-decision", 1, &draft)
            .map_err(|_| EgressError::CanonicalHash)?;
        Ok(Self {
            schema_version: draft.schema_version,
            decision_id: draft.decision_id,
            manifest_hash: draft.manifest_hash,
            binding_hash: draft.binding_hash,
            consent_disclosure_hash: draft.consent_disclosure_hash,
            policy_hash: draft.policy_hash,
            installation_id: draft.installation_id,
            session_authority_hash: draft.session_authority_hash,
            issued_at: draft.issued_at,
            expires_at: draft.expires_at,
            decision_hash,
        })
    }

    fn draft(&self) -> PendingContextDecisionDraft {
        PendingContextDecisionDraft {
            schema_version: self.schema_version.clone(),
            decision_id: self.decision_id.clone(),
            manifest_hash: self.manifest_hash,
            binding_hash: self.binding_hash,
            consent_disclosure_hash: self.consent_disclosure_hash,
            policy_hash: self.policy_hash,
            installation_id: self.installation_id.clone(),
            session_authority_hash: self.session_authority_hash,
            issued_at: self.issued_at,
            expires_at: self.expires_at,
        }
    }

    fn stored_copy(&self) -> Self {
        Self {
            schema_version: self.schema_version.clone(),
            decision_id: self.decision_id.clone(),
            manifest_hash: self.manifest_hash,
            binding_hash: self.binding_hash,
            consent_disclosure_hash: self.consent_disclosure_hash,
            policy_hash: self.policy_hash,
            installation_id: self.installation_id.clone(),
            session_authority_hash: self.session_authority_hash,
            issued_at: self.issued_at,
            expires_at: self.expires_at,
            decision_hash: self.decision_hash,
        }
    }

    #[must_use]
    pub fn decision_id(&self) -> &ContractId {
        &self.decision_id
    }

    #[must_use]
    pub const fn expires_at(&self) -> UnixMillis {
        self.expires_at
    }

    fn verify(&self) -> Result<(), EgressError> {
        if self.schema_version != DECISION_SCHEMA {
            return Err(EgressError::DecisionIntegrity);
        }
        validate_decision_lifetime(self.issued_at, self.expires_at)?;
        let actual = canonical_hash("context-decision", 1, &self.draft())
            .map_err(|_| EgressError::CanonicalHash)?;
        if actual != self.decision_hash {
            return Err(EgressError::DecisionIntegrity);
        }
        Ok(())
    }
}

/// A borrowed, integrity-checked view of one live consent decision.
///
/// The view deliberately implements neither `Clone` nor `Serialize`; it can
/// only be created from an intact [`PendingContextDecision`].
///
/// ```compile_fail,E0277
/// fn serialize(evidence: &desktop_egress::ContextDecisionEvidence<'_>) {
///     let _ = serde_json::to_string(evidence);
/// }
/// ```
pub struct ContextDecisionEvidence<'a> {
    decision: &'a PendingContextDecision,
    observed_at: UnixMillis,
    _ledger_state: MutexGuard<'a, MemoryDecisionLedgerState>,
}

impl fmt::Debug for ContextDecisionEvidence<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ContextDecisionEvidence")
            .finish_non_exhaustive()
    }
}

impl ContextDecisionEvidence<'_> {
    #[must_use]
    pub fn decision_id(&self) -> &ContractId {
        &self.decision.decision_id
    }

    #[must_use]
    pub const fn manifest_hash(&self) -> Sha256Digest {
        self.decision.manifest_hash
    }

    #[must_use]
    pub const fn invocation_binding_hash(&self) -> Sha256Digest {
        self.decision.binding_hash
    }

    #[must_use]
    pub const fn consent_disclosure_hash(&self) -> Sha256Digest {
        self.decision.consent_disclosure_hash
    }

    #[must_use]
    pub const fn policy_hash(&self) -> Sha256Digest {
        self.decision.policy_hash
    }

    #[must_use]
    pub fn installation_id(&self) -> &ContractId {
        &self.decision.installation_id
    }

    #[must_use]
    pub const fn session_authority_hash(&self) -> Sha256Digest {
        self.decision.session_authority_hash
    }

    #[must_use]
    pub const fn issued_at(&self) -> UnixMillis {
        self.decision.issued_at
    }

    #[must_use]
    pub const fn expires_at(&self) -> UnixMillis {
        self.decision.expires_at
    }

    #[must_use]
    pub const fn observed_at(&self) -> UnixMillis {
        self.observed_at
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct DecisionConsumptionDraft {
    schema_version: String,
    decision_id: ContractId,
    decision_hash: Sha256Digest,
    invocation_id: ContractId,
    manifest_hash: Sha256Digest,
    binding_hash: Sha256Digest,
    consent_disclosure_hash: Sha256Digest,
    policy_hash: Sha256Digest,
    installation_id: ContractId,
    session_authority_hash: Sha256Digest,
    consumed_at: UnixMillis,
}

#[derive(Debug, Eq, PartialEq)]
struct ConsumptionAuthority;

/// A consumed consent capability cannot be duplicated for a second model
/// request.
///
/// ```compile_fail
/// fn duplicate(consumption: desktop_egress::DecisionConsumption) {
///     let replay = consumption.clone();
/// }
/// ```
///
/// ```compile_fail
/// # use desktop_egress::DecisionConsumption;
/// fn retarget(mut consumption: DecisionConsumption) -> DecisionConsumption {
///     consumption.invocation_id = consumption.decision_id().clone();
///     consumption.consumption_hash = desktop_runtime::sha256_bytes(b"rehashed");
///     consumption
/// }
/// ```
#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionConsumption {
    schema_version: String,
    decision_id: ContractId,
    decision_hash: Sha256Digest,
    invocation_id: ContractId,
    manifest_hash: Sha256Digest,
    binding_hash: Sha256Digest,
    consent_disclosure_hash: Sha256Digest,
    policy_hash: Sha256Digest,
    installation_id: ContractId,
    session_authority_hash: Sha256Digest,
    consumed_at: UnixMillis,
    consumption_hash: Sha256Digest,
    #[serde(skip)]
    authority: ConsumptionAuthority,
}

impl DecisionConsumption {
    fn seal(draft: DecisionConsumptionDraft) -> Result<Self, EgressError> {
        let consumption_hash = canonical_hash("decision-consumption", 1, &draft)
            .map_err(|_| EgressError::CanonicalHash)?;
        Ok(Self {
            schema_version: draft.schema_version,
            decision_id: draft.decision_id,
            decision_hash: draft.decision_hash,
            invocation_id: draft.invocation_id,
            manifest_hash: draft.manifest_hash,
            binding_hash: draft.binding_hash,
            consent_disclosure_hash: draft.consent_disclosure_hash,
            policy_hash: draft.policy_hash,
            installation_id: draft.installation_id,
            session_authority_hash: draft.session_authority_hash,
            consumed_at: draft.consumed_at,
            consumption_hash,
            authority: ConsumptionAuthority,
        })
    }

    fn draft(&self) -> DecisionConsumptionDraft {
        DecisionConsumptionDraft {
            schema_version: self.schema_version.clone(),
            decision_id: self.decision_id.clone(),
            decision_hash: self.decision_hash,
            invocation_id: self.invocation_id.clone(),
            manifest_hash: self.manifest_hash,
            binding_hash: self.binding_hash,
            consent_disclosure_hash: self.consent_disclosure_hash,
            policy_hash: self.policy_hash,
            installation_id: self.installation_id.clone(),
            session_authority_hash: self.session_authority_hash,
            consumed_at: self.consumed_at,
        }
    }

    #[must_use]
    pub fn decision_id(&self) -> &ContractId {
        &self.decision_id
    }

    #[must_use]
    pub const fn decision_hash(&self) -> Sha256Digest {
        self.decision_hash
    }

    #[must_use]
    pub fn invocation_id(&self) -> &ContractId {
        &self.invocation_id
    }

    #[must_use]
    pub const fn manifest_hash(&self) -> Sha256Digest {
        self.manifest_hash
    }

    #[must_use]
    pub const fn binding_hash(&self) -> Sha256Digest {
        self.binding_hash
    }

    #[must_use]
    pub const fn consent_disclosure_hash(&self) -> Sha256Digest {
        self.consent_disclosure_hash
    }

    #[must_use]
    pub const fn policy_hash(&self) -> Sha256Digest {
        self.policy_hash
    }

    #[must_use]
    pub fn installation_id(&self) -> &ContractId {
        &self.installation_id
    }

    #[must_use]
    pub const fn session_authority_hash(&self) -> Sha256Digest {
        self.session_authority_hash
    }

    #[must_use]
    pub const fn consumed_at(&self) -> UnixMillis {
        self.consumed_at
    }

    #[must_use]
    pub const fn consumption_hash(&self) -> Sha256Digest {
        self.consumption_hash
    }

    /// Revalidates the immutable decision-consumption record.
    ///
    /// # Errors
    ///
    /// Returns [`EgressError::DecisionIntegrity`] when the schema or canonical
    /// content hash no longer matches.
    pub fn verify(&self) -> Result<(), EgressError> {
        if self.schema_version != CONSUMPTION_SCHEMA {
            return Err(EgressError::DecisionIntegrity);
        }
        let actual = canonical_hash("decision-consumption", 1, &self.draft())
            .map_err(|_| EgressError::CanonicalHash)?;
        if actual != self.consumption_hash {
            return Err(EgressError::DecisionIntegrity);
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct ApproveDecisionInput<'a> {
    pub manifest: &'a ContextEgressManifest,
    pub binding: &'a ModelInvocationBinding,
    pub decision_id: ContractId,
    pub issued_at: UnixMillis,
    pub expires_at: UnixMillis,
}

#[derive(Clone, Debug)]
pub struct ConsumeDecisionInput<'a> {
    pub decision: &'a PendingContextDecision,
    pub binding: &'a ModelInvocationBinding,
    pub invocation_id: ContractId,
    pub consumed_at: UnixMillis,
}

#[derive(Clone, Debug)]
pub struct CancelDecisionInput<'a> {
    pub decision: &'a PendingContextDecision,
    pub cancelled_at: UnixMillis,
}

#[derive(Clone, Copy, Debug)]
pub struct DecisionEvidenceInput<'a> {
    pub decision: &'a PendingContextDecision,
    pub observed_at: UnixMillis,
}

trait DecisionLedger: Send + Sync {
    fn insert_pending(&self, decision: &PendingContextDecision) -> Result<(), EgressError>;

    fn consume_if_pending(
        &self,
        input: ConsumeDecisionInput<'_>,
    ) -> Result<DecisionConsumption, EgressError>;

    fn cancel_if_pending(&self, input: CancelDecisionInput<'_>) -> Result<(), EgressError>;
}

#[derive(Debug)]
enum DecisionState {
    Pending(Box<PendingContextDecision>),
    Consumed { expires_at: UnixMillis },
    Cancelled { expires_at: UnixMillis },
    Expired { expires_at: UnixMillis },
}

impl DecisionState {
    fn expires_at(&self) -> UnixMillis {
        match self {
            Self::Pending(decision) => decision.expires_at,
            Self::Consumed { expires_at }
            | Self::Cancelled { expires_at }
            | Self::Expired { expires_at } => *expires_at,
        }
    }
}

/// In-memory consent authority used by the current desktop composition.
/// Registration is intentionally available only through
/// [`ConsentService::approve`].
///
/// ```compile_fail
/// # use desktop_egress::{ConsentService, MemoryDecisionLedger, PendingContextDecision};
/// fn reinsert(ledger: &MemoryDecisionLedger, decision: PendingContextDecision) {
///     ledger.insert_pending(&decision).unwrap();
/// }
/// ```
#[derive(Default)]
pub struct MemoryDecisionLedger {
    state: Mutex<MemoryDecisionLedgerState>,
}

#[derive(Default)]
struct MemoryDecisionLedgerState {
    decisions: HashMap<ContractId, DecisionState>,
    seen_decision_ids: HashSet<ContractId>,
}

impl fmt::Debug for MemoryDecisionLedger {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MemoryDecisionLedger")
            .finish_non_exhaustive()
    }
}

impl MemoryDecisionLedger {
    /// Maximum number of simultaneously relevant decisions retained in one
    /// in-process authority ledger.
    pub const MAX_TRACKED_DECISIONS: usize = DEFAULT_MAX_TRACKED_DECISIONS;

    /// Maximum number of identifier replay tombstones retained for one
    /// process lifetime. Saturation rejects new approvals fail-closed.
    pub const MAX_SEEN_DECISION_IDS: usize = DEFAULT_MAX_SEEN_DECISION_IDS;
}

impl DecisionLedger for MemoryDecisionLedger {
    fn insert_pending(&self, decision: &PendingContextDecision) -> Result<(), EgressError> {
        decision.verify()?;
        let mut state = self.state.lock();
        state
            .decisions
            .retain(|_, entry| entry.expires_at() >= decision.issued_at);
        if state.seen_decision_ids.contains(&decision.decision_id)
            || state.decisions.len() >= DEFAULT_MAX_TRACKED_DECISIONS
            || state.seen_decision_ids.len() >= DEFAULT_MAX_SEEN_DECISION_IDS
        {
            return Err(EgressError::DecisionAlreadyExists);
        }
        state.seen_decision_ids.insert(decision.decision_id.clone());
        state.decisions.insert(
            decision.decision_id.clone(),
            DecisionState::Pending(Box::new(decision.stored_copy())),
        );
        Ok(())
    }

    fn consume_if_pending(
        &self,
        input: ConsumeDecisionInput<'_>,
    ) -> Result<DecisionConsumption, EgressError> {
        input.decision.verify()?;
        input.binding.verify()?;
        let mut ledger = self.state.lock();
        let state = ledger
            .decisions
            .get_mut(&input.decision.decision_id)
            .ok_or(EgressError::DecisionUnknown)?;
        match state {
            DecisionState::Consumed { .. } => Err(EgressError::DecisionAlreadyConsumed),
            DecisionState::Cancelled { .. } => Err(EgressError::DecisionCancelled),
            DecisionState::Expired { .. } => Err(EgressError::DecisionExpired),
            DecisionState::Pending(stored) => {
                if input.consumed_at > stored.expires_at {
                    let expires_at = stored.expires_at;
                    *state = DecisionState::Expired { expires_at };
                    return Err(EgressError::DecisionExpired);
                }
                if input.consumed_at < stored.issued_at
                    || stored.as_ref() != input.decision
                    || input.binding.binding_hash != stored.binding_hash
                    || input.binding.draft.manifest_hash != stored.manifest_hash
                    || input.binding.draft.consent_disclosure_hash != stored.consent_disclosure_hash
                    || input.binding.draft.policy_hash != stored.policy_hash
                    || input.binding.draft.installation_id != stored.installation_id
                    || input.binding.draft.session_authority_hash != stored.session_authority_hash
                {
                    return Err(EgressError::DecisionBindingMismatch);
                }
                let consumption = DecisionConsumption::seal(DecisionConsumptionDraft {
                    schema_version: CONSUMPTION_SCHEMA.to_owned(),
                    decision_id: stored.decision_id.clone(),
                    decision_hash: stored.decision_hash,
                    invocation_id: input.invocation_id,
                    manifest_hash: stored.manifest_hash,
                    binding_hash: stored.binding_hash,
                    consent_disclosure_hash: stored.consent_disclosure_hash,
                    policy_hash: stored.policy_hash,
                    installation_id: stored.installation_id.clone(),
                    session_authority_hash: stored.session_authority_hash,
                    consumed_at: input.consumed_at,
                })?;
                let expires_at = stored.expires_at;
                *state = DecisionState::Consumed { expires_at };
                Ok(consumption)
            }
        }
    }

    fn cancel_if_pending(&self, input: CancelDecisionInput<'_>) -> Result<(), EgressError> {
        input.decision.verify()?;
        let mut ledger = self.state.lock();
        let state = ledger
            .decisions
            .get_mut(&input.decision.decision_id)
            .ok_or(EgressError::DecisionUnknown)?;
        match state {
            DecisionState::Consumed { .. } => Err(EgressError::DecisionAlreadyConsumed),
            DecisionState::Cancelled { .. } => Err(EgressError::DecisionCancelled),
            DecisionState::Expired { .. } => Err(EgressError::DecisionExpired),
            DecisionState::Pending(stored) => {
                if input.cancelled_at > stored.expires_at {
                    let expires_at = stored.expires_at;
                    *state = DecisionState::Expired { expires_at };
                    return Err(EgressError::DecisionExpired);
                }
                if input.cancelled_at < stored.issued_at || stored.as_ref() != input.decision {
                    return Err(EgressError::DecisionBindingMismatch);
                }
                let expires_at = stored.expires_at;
                *state = DecisionState::Cancelled { expires_at };
                Ok(())
            }
        }
    }
}

pub struct ConsentService<'a> {
    ledger: &'a MemoryDecisionLedger,
}

impl<'a> ConsentService<'a> {
    #[must_use]
    pub const fn new(ledger: &'a MemoryDecisionLedger) -> Self {
        Self { ledger }
    }

    /// Borrows verified bridge evidence only while the exact decision remains
    /// live and pending in this ledger.
    ///
    /// The returned view retains the ledger lock until it is dropped, so a
    /// concurrent consume or cancel cannot race bridge materialization.
    ///
    /// # Errors
    ///
    /// Returns [`EgressError`] when the decision is invalid, expired,
    /// terminal, unregistered, or does not match the ledger copy.
    pub fn evidence<'b>(
        &'b self,
        input: DecisionEvidenceInput<'b>,
    ) -> Result<ContextDecisionEvidence<'b>, EgressError> {
        input.decision.verify()?;
        let mut state = self.ledger.state.lock();
        {
            let decision_state = state
                .decisions
                .get_mut(&input.decision.decision_id)
                .ok_or(EgressError::DecisionUnknown)?;
            match decision_state {
                DecisionState::Consumed { .. } => {
                    return Err(EgressError::DecisionAlreadyConsumed);
                }
                DecisionState::Cancelled { .. } => {
                    return Err(EgressError::DecisionCancelled);
                }
                DecisionState::Expired { .. } => return Err(EgressError::DecisionExpired),
                DecisionState::Pending(stored) => {
                    if input.observed_at > stored.expires_at {
                        let expires_at = stored.expires_at;
                        *decision_state = DecisionState::Expired { expires_at };
                        return Err(EgressError::DecisionExpired);
                    }
                    if input.observed_at < stored.issued_at || stored.as_ref() != input.decision {
                        return Err(EgressError::DecisionBindingMismatch);
                    }
                }
            }
        }
        Ok(ContextDecisionEvidence {
            decision: input.decision,
            observed_at: input.observed_at,
            _ledger_state: state,
        })
    }

    /// Approves an exact reviewed manifest and invocation binding.
    ///
    /// # Errors
    ///
    /// Returns [`EgressError`] when binding, lifetime, manifest integrity, or
    /// decision uniqueness fails.
    pub fn approve(
        &self,
        input: ApproveDecisionInput<'_>,
    ) -> Result<PendingContextDecision, EgressError> {
        input.binding.verify_for(input.manifest)?;
        validate_decision_lifetime(input.issued_at, input.expires_at)?;
        if input.issued_at < input.manifest.draft.created_at
            || input.expires_at > input.manifest.draft.expires_at
        {
            return Err(EgressError::InvalidLifetime);
        }
        let binding = &input.binding.draft;
        let decision = PendingContextDecision::from_draft(PendingContextDecisionDraft {
            schema_version: DECISION_SCHEMA.to_owned(),
            decision_id: input.decision_id,
            manifest_hash: input.manifest.manifest_hash,
            binding_hash: input.binding.binding_hash,
            consent_disclosure_hash: binding.consent_disclosure_hash,
            policy_hash: binding.policy_hash,
            installation_id: binding.installation_id.clone(),
            session_authority_hash: binding.session_authority_hash,
            issued_at: input.issued_at,
            expires_at: input.expires_at,
        })?;
        self.ledger.insert_pending(&decision)?;
        Ok(decision)
    }

    /// Atomically consumes one exact pending decision.
    ///
    /// # Errors
    ///
    /// Returns [`EgressError`] when the decision is invalid, drifted, expired,
    /// unknown, or already consumed.
    pub fn consume(
        &self,
        input: ConsumeDecisionInput<'_>,
    ) -> Result<DecisionConsumption, EgressError> {
        input.decision.verify()?;
        input.binding.verify()?;
        if input.binding.binding_hash != input.decision.binding_hash {
            return Err(EgressError::DecisionBindingMismatch);
        }
        self.ledger.consume_if_pending(input)
    }

    /// Permanently cancels one exact pending decision without creating model
    /// invocation authority.
    ///
    /// # Errors
    ///
    /// Returns [`EgressError`] when the decision is invalid, expired, unknown,
    /// consumed, or already cancelled.
    pub fn cancel(&self, input: CancelDecisionInput<'_>) -> Result<(), EgressError> {
        input.decision.verify()?;
        self.ledger.cancel_if_pending(input)
    }
}

fn validate_binding(binding: &ModelInvocationBindingDraft) -> Result<(), EgressError> {
    if binding.schema_version != BINDING_SCHEMA
        || !is_safe_label(&binding.purpose, 128)
        || !is_safe_label(&binding.model_role, 128)
        || !(3..=64).contains(&binding.region.len())
        || !binding.region.bytes().all(|byte| byte.is_ascii_lowercase())
    {
        return Err(EgressError::InvalidInvocationBinding);
    }
    Ok(())
}

fn validate_decision_lifetime(
    issued_at: UnixMillis,
    expires_at: UnixMillis,
) -> Result<(), EgressError> {
    let lifetime = expires_at
        .0
        .checked_sub(issued_at.0)
        .ok_or(EgressError::InvalidLifetime)?;
    if lifetime == 0 || lifetime > MAX_DECISION_LIFETIME_MS {
        return Err(EgressError::InvalidLifetime);
    }
    Ok(())
}

fn is_safe_label(value: &str, maximum_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum_bytes
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}
