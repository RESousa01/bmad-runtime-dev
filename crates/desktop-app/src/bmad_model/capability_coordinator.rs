//! Generic reviewed model-capability lifecycle (readiness Task 6).
//!
//! One coordinator drives the D2 lifecycle — prepare, one reviewed
//! decision, single-use consumption, transport dispatch, output
//! verification, durable capability-run persistence — for every ADR-0005
//! capability. Every stage is bound to one closure-ledger capability
//! identity: a manifest, decision, or output minted for capability A can
//! never be replayed against capability B, and a result whose archetype
//! differs from the run's declared output schema terminates the flow.

use desktop_cloud::{AuthorizedModelRequest, CloudError, DispatchedModelRequest, RawModelOutput};
use desktop_egress::{
    ApproveDecisionInput, CancelDecisionInput, ConsentService, ConsumeDecisionInput,
    ContextEgressManifest, EgressError, MemoryDecisionLedger, ModelInvocationBinding,
    PendingContextDecision,
};
use desktop_runtime::{
    canonical_hash, BmadCapabilityOutput, BmadCapabilityRun, BmadCapabilityRunParams,
    BmadClosureCapabilityId, ContractId, Sha256Digest, UnixMillis,
};
use desktop_store::LocalStore;
use serde::Serialize;

use super::context::derived_contract_id;

/// Maximum decision validity after approval, mirroring the Help lifecycle.
const DECISION_WINDOW_MS: u64 = 5 * 60 * 1_000;

/// Transport seam shared by deterministic, offline, and (later) production
/// capability dispatch. Mirrors the Help transport so Task 9 can supply the
/// deployed round trip once.
pub(crate) trait BmadCapabilityTransport: Send + Sync {
    fn send(
        &self,
        request: AuthorizedModelRequest,
        deterministic_fixture: &str,
        now: UnixMillis,
    ) -> Result<(DispatchedModelRequest, RawModelOutput), CloudError>;
}

/// Verifies raw model output into exactly one sealed capability output.
/// Implementations must treat the raw output as untrusted data.
pub(crate) trait BmadCapabilityOutputVerifier {
    fn verify(
        &self,
        capability_id: &BmadClosureCapabilityId,
        output: &RawModelOutput,
    ) -> Result<BmadCapabilityOutput, BmadCapabilityCoordinatorError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BmadCapabilityCoordinatorError {
    Unauthorized,
    Conflict,
    CapabilityBindingMismatch,
    ConsentBindingMismatch,
    ConsentExpired,
    ConsentAlreadyConsumed,
    OutputRejected,
    ResultArchetypeMismatch,
    Transport,
    Store,
    Integrity,
}

/// The egress-safe purpose label for one capability: `bmm:x` -> `bmm.x`.
pub(crate) fn capability_purpose(capability_id: &BmadClosureCapabilityId) -> String {
    capability_id.as_str().replacen(':', ".", 1)
}

pub(crate) struct PrepareCapabilityRunInput {
    pub capability_id: BmadClosureCapabilityId,
    pub workspace_id: ContractId,
    pub workspace_grant_epoch: u64,
    pub workspace_context_read_epoch: u64,
    pub run_id: ContractId,
    pub instruction_hash: Sha256Digest,
    pub output_schema_id: String,
    pub manifest: ContextEgressManifest,
    pub invocation_binding: ModelInvocationBinding,
    pub deterministic_fixture: String,
    pub created_at: UnixMillis,
}

pub(crate) struct ApproveCapabilityRunInput {
    pub capability_id: BmadClosureCapabilityId,
    pub manifest_hash: Sha256Digest,
    pub approved_at: UnixMillis,
}

pub(crate) struct CancelCapabilityRunInput {
    pub capability_id: BmadClosureCapabilityId,
    pub manifest_hash: Sha256Digest,
    pub decision_id: ContractId,
    pub cancelled_at: UnixMillis,
}

pub(crate) struct SubmitCapabilityRunInput {
    pub capability_id: BmadClosureCapabilityId,
    pub manifest_hash: Sha256Digest,
    pub decision_id: ContractId,
    pub submitted_at: UnixMillis,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CapabilityReviewProjection {
    pub capability_id: String,
    pub run_id: ContractId,
    pub manifest_hash: Sha256Digest,
    pub expires_at: UnixMillis,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CapabilityApprovedProjection {
    pub capability_id: String,
    pub manifest_hash: Sha256Digest,
    pub decision_id: ContractId,
    pub expires_at: UnixMillis,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CapabilityCompletedProjection {
    pub capability_id: String,
    pub run_id: ContractId,
    pub result_kind: String,
}

struct PreparedCapabilityRun {
    capability_id: BmadClosureCapabilityId,
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    workspace_context_read_epoch: u64,
    run_id: ContractId,
    instruction_hash: Sha256Digest,
    output_schema_id: String,
    manifest: ContextEgressManifest,
    invocation_binding: ModelInvocationBinding,
    deterministic_fixture: String,
}

struct ApprovedCapabilityRun {
    prepared: PreparedCapabilityRun,
    decision: PendingContextDecision,
    issued_at: UnixMillis,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CapabilityTerminalReason {
    Cancelled,
    ConsentExpired,
    ConsentConsumed,
    OutputRejected,
    Invalidated,
}

enum ActiveCapabilityRun {
    ReviewRequired(Box<PreparedCapabilityRun>),
    Approved(Box<ApprovedCapabilityRun>),
    Terminal(CapabilityTerminalReason),
}

#[derive(Serialize)]
struct CapabilityDecisionIdentity<'a> {
    schema_version: &'static str,
    capability_id: &'a str,
    run_id: &'a ContractId,
    manifest_hash: Sha256Digest,
    binding_hash: Sha256Digest,
    issued_at: UnixMillis,
}

#[derive(Serialize)]
struct CapabilityInvocationIdentity<'a> {
    schema_version: &'static str,
    capability_id: &'a str,
    decision_id: &'a ContractId,
    manifest_hash: Sha256Digest,
    consumed_at: UnixMillis,
}

pub(crate) struct BmadCapabilityCoordinator {
    ledger: MemoryDecisionLedger,
    active: Option<ActiveCapabilityRun>,
}

impl BmadCapabilityCoordinator {
    pub(crate) fn new() -> Self {
        Self {
            ledger: MemoryDecisionLedger::default(),
            active: None,
        }
    }

    /// Withdraws any in-flight capability review, for sign-out and
    /// workspace-revocation paths (ADR-0002).
    pub(crate) fn invalidate(&mut self) {
        if self.active.is_some() {
            self.active = Some(ActiveCapabilityRun::Terminal(
                CapabilityTerminalReason::Invalidated,
            ));
        }
    }

    #[cfg(test)]
    pub(crate) fn terminal_reason_for_test(&self) -> Option<CapabilityTerminalReason> {
        match self.active.as_ref() {
            Some(ActiveCapabilityRun::Terminal(reason)) => Some(*reason),
            _ => None,
        }
    }

    /// Opens one reviewed capability flow. The manifest and invocation
    /// binding must have been minted for exactly this capability: their
    /// purpose label is the capability identity, so cross-capability
    /// substitution fails closed here.
    pub(crate) fn prepare(
        &mut self,
        input: PrepareCapabilityRunInput,
    ) -> Result<CapabilityReviewProjection, BmadCapabilityCoordinatorError> {
        if matches!(
            self.active,
            Some(ActiveCapabilityRun::ReviewRequired(_) | ActiveCapabilityRun::Approved(_))
        ) {
            return Err(BmadCapabilityCoordinatorError::Conflict);
        }
        let expected_purpose = capability_purpose(&input.capability_id);
        if input.manifest.draft.purpose != expected_purpose
            || input.invocation_binding.draft.purpose != expected_purpose
        {
            return Err(BmadCapabilityCoordinatorError::CapabilityBindingMismatch);
        }
        if input.invocation_binding.draft.manifest_hash != input.manifest.manifest_hash {
            return Err(BmadCapabilityCoordinatorError::ConsentBindingMismatch);
        }
        if input
            .invocation_binding
            .draft
            .canonical_output_schema_id
            .as_str()
            != input.output_schema_id
        {
            return Err(BmadCapabilityCoordinatorError::ResultArchetypeMismatch);
        }
        let projection = CapabilityReviewProjection {
            capability_id: input.capability_id.as_str().to_owned(),
            run_id: input.run_id.clone(),
            manifest_hash: input.manifest.manifest_hash,
            expires_at: input.manifest.draft.expires_at,
        };
        self.active = Some(ActiveCapabilityRun::ReviewRequired(Box::new(
            PreparedCapabilityRun {
                capability_id: input.capability_id,
                workspace_id: input.workspace_id,
                workspace_grant_epoch: input.workspace_grant_epoch,
                workspace_context_read_epoch: input.workspace_context_read_epoch,
                run_id: input.run_id,
                instruction_hash: input.instruction_hash,
                output_schema_id: input.output_schema_id,
                manifest: input.manifest,
                invocation_binding: input.invocation_binding,
                deterministic_fixture: input.deterministic_fixture,
            },
        )));
        Ok(projection)
    }

    /// Approves the pending review, minting one single-use decision bound
    /// to this capability, manifest, and binding.
    pub(crate) fn approve(
        &mut self,
        input: &ApproveCapabilityRunInput,
    ) -> Result<CapabilityApprovedProjection, BmadCapabilityCoordinatorError> {
        let prepared = match self.active.as_ref() {
            Some(ActiveCapabilityRun::ReviewRequired(prepared)) => prepared,
            Some(ActiveCapabilityRun::Approved(_)) => {
                return Err(BmadCapabilityCoordinatorError::Conflict);
            }
            Some(ActiveCapabilityRun::Terminal(_)) | None => {
                return Err(BmadCapabilityCoordinatorError::Unauthorized);
            }
        };
        if input.capability_id != prepared.capability_id {
            return Err(BmadCapabilityCoordinatorError::CapabilityBindingMismatch);
        }
        if input.manifest_hash != prepared.manifest.manifest_hash {
            return Err(BmadCapabilityCoordinatorError::ConsentBindingMismatch);
        }
        if input.approved_at < prepared.manifest.draft.created_at
            || input.approved_at >= prepared.manifest.draft.expires_at
        {
            self.active = Some(ActiveCapabilityRun::Terminal(
                CapabilityTerminalReason::ConsentExpired,
            ));
            return Err(BmadCapabilityCoordinatorError::ConsentExpired);
        }
        let expires_at = UnixMillis(
            input
                .approved_at
                .0
                .checked_add(DECISION_WINDOW_MS)
                .map_or(prepared.manifest.draft.expires_at.0, |maximum| {
                    maximum.min(prepared.manifest.draft.expires_at.0)
                }),
        );
        let decision_digest = canonical_hash(
            "bmad-capability-context-decision-id",
            1,
            &CapabilityDecisionIdentity {
                schema_version: "sapphirus.bmad-capability-decision.v1",
                capability_id: prepared.capability_id.as_str(),
                run_id: &prepared.run_id,
                manifest_hash: prepared.manifest.manifest_hash,
                binding_hash: prepared.invocation_binding.binding_hash,
                issued_at: input.approved_at,
            },
        )
        .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?;
        let decision_id = derived_contract_id("decision", decision_digest)
            .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?;
        let Some(ActiveCapabilityRun::ReviewRequired(prepared)) = self.active.take() else {
            return Err(BmadCapabilityCoordinatorError::Conflict);
        };
        let decision = match ConsentService::new(&self.ledger).approve(ApproveDecisionInput {
            manifest: &prepared.manifest,
            binding: &prepared.invocation_binding,
            decision_id: decision_id.clone(),
            issued_at: input.approved_at,
            expires_at,
        }) {
            Ok(decision) => decision,
            Err(error) => {
                self.active = Some(ActiveCapabilityRun::ReviewRequired(prepared));
                return Err(map_egress_error(error));
            }
        };
        let projection = CapabilityApprovedProjection {
            capability_id: prepared.capability_id.as_str().to_owned(),
            manifest_hash: input.manifest_hash,
            decision_id,
            expires_at,
        };
        self.active = Some(ActiveCapabilityRun::Approved(Box::new(
            ApprovedCapabilityRun {
                prepared: *prepared,
                decision,
                issued_at: input.approved_at,
            },
        )));
        Ok(projection)
    }

    /// Cancels the approved decision without consuming it.
    pub(crate) fn cancel(
        &mut self,
        input: &CancelCapabilityRunInput,
    ) -> Result<(), BmadCapabilityCoordinatorError> {
        let approved = match self.active.as_ref() {
            Some(ActiveCapabilityRun::Approved(approved)) => approved,
            Some(ActiveCapabilityRun::ReviewRequired(_)) => {
                return Err(BmadCapabilityCoordinatorError::ConsentBindingMismatch);
            }
            Some(ActiveCapabilityRun::Terminal(_)) | None => {
                return Err(BmadCapabilityCoordinatorError::Unauthorized);
            }
        };
        if input.capability_id != approved.prepared.capability_id {
            return Err(BmadCapabilityCoordinatorError::CapabilityBindingMismatch);
        }
        if input.manifest_hash != approved.prepared.manifest.manifest_hash
            || input.decision_id != *approved.decision.decision_id()
        {
            return Err(BmadCapabilityCoordinatorError::ConsentBindingMismatch);
        }
        let Some(ActiveCapabilityRun::Approved(approved)) = self.active.take() else {
            return Err(BmadCapabilityCoordinatorError::Conflict);
        };
        let _ = ConsentService::new(&self.ledger).cancel(CancelDecisionInput {
            decision: &approved.decision,
            cancelled_at: input.cancelled_at,
        });
        self.active = Some(ActiveCapabilityRun::Terminal(
            CapabilityTerminalReason::Cancelled,
        ));
        Ok(())
    }

    /// Consumes the decision exactly once, dispatches through the
    /// transport, verifies the output, and durably records the run and its
    /// result.
    pub(crate) fn submit(
        &mut self,
        input: &SubmitCapabilityRunInput,
        transport: &dyn BmadCapabilityTransport,
        verifier: &dyn BmadCapabilityOutputVerifier,
        store: &LocalStore,
    ) -> Result<CapabilityCompletedProjection, BmadCapabilityCoordinatorError> {
        self.validate_approved_binding(input)?;
        let Some(ActiveCapabilityRun::Approved(approved)) = self.active.take() else {
            return Err(BmadCapabilityCoordinatorError::Conflict);
        };
        if input.submitted_at < approved.issued_at
            || input.submitted_at > approved.decision.expires_at()
        {
            let _ = ConsentService::new(&self.ledger).cancel(CancelDecisionInput {
                decision: &approved.decision,
                cancelled_at: input.submitted_at,
            });
            self.active = Some(ActiveCapabilityRun::Terminal(
                CapabilityTerminalReason::ConsentExpired,
            ));
            return Err(BmadCapabilityCoordinatorError::ConsentExpired);
        }

        let invocation_digest = canonical_hash(
            "bmad-capability-invocation-id",
            1,
            &CapabilityInvocationIdentity {
                schema_version: "sapphirus.bmad-capability-invocation.v1",
                capability_id: approved.prepared.capability_id.as_str(),
                decision_id: approved.decision.decision_id(),
                manifest_hash: approved.prepared.manifest.manifest_hash,
                consumed_at: input.submitted_at,
            },
        )
        .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?;
        let invocation_id = derived_contract_id("invoke", invocation_digest)
            .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?;
        let consumption = match ConsentService::new(&self.ledger).consume(ConsumeDecisionInput {
            decision: &approved.decision,
            binding: &approved.prepared.invocation_binding,
            invocation_id,
            consumed_at: input.submitted_at,
        }) {
            Ok(consumption) => consumption,
            Err(error) => {
                self.active = Some(ActiveCapabilityRun::Terminal(
                    CapabilityTerminalReason::ConsentConsumed,
                ));
                return Err(map_egress_error(error));
            }
        };
        let decision_id = consumption.decision_id().clone();
        let output = match dispatch_and_verify(
            &approved,
            consumption,
            input.submitted_at,
            transport,
            verifier,
        ) {
            Ok(output) => output,
            Err((reason, error)) => {
                self.active = Some(ActiveCapabilityRun::Terminal(reason));
                return Err(error);
            }
        };

        let run = BmadCapabilityRun::open(BmadCapabilityRunParams {
            run_id: approved.prepared.run_id.clone(),
            capability_id: approved.prepared.capability_id.clone(),
            workspace_id: approved.prepared.workspace_id.clone(),
            instruction_hash: approved.prepared.instruction_hash,
            context_manifest_hash: approved.prepared.manifest.manifest_hash,
            output_schema_id: approved.prepared.output_schema_id.clone(),
            consent_evidence_id: decision_id,
            created_at: input.submitted_at,
        })
        .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?;
        store
            .create_bmad_capability_run(&run)
            .map_err(|_| BmadCapabilityCoordinatorError::Store)?;
        let result_kind = match &output {
            BmadCapabilityOutput::DocumentArtifact(_) => "document_artifact",
            BmadCapabilityOutput::GovernedChangeSet(_) => "governed_change_set",
            BmadCapabilityOutput::InactiveBuilderDraft(_) => "inactive_builder_draft",
        };
        store
            .record_bmad_capability_result(&run.run_id, &output)
            .map_err(|_| BmadCapabilityCoordinatorError::Store)?;

        let projection = CapabilityCompletedProjection {
            capability_id: approved.prepared.capability_id.as_str().to_owned(),
            run_id: approved.prepared.run_id.clone(),
            result_kind: result_kind.to_owned(),
        };
        self.active = None;
        Ok(projection)
    }
}

impl BmadCapabilityCoordinator {
    fn validate_approved_binding(
        &self,
        input: &SubmitCapabilityRunInput,
    ) -> Result<(), BmadCapabilityCoordinatorError> {
        let approved = match self.active.as_ref() {
            Some(ActiveCapabilityRun::Approved(approved)) => approved,
            Some(ActiveCapabilityRun::ReviewRequired(_)) => {
                return Err(BmadCapabilityCoordinatorError::ConsentBindingMismatch);
            }
            Some(ActiveCapabilityRun::Terminal(_)) | None => {
                return Err(BmadCapabilityCoordinatorError::Unauthorized);
            }
        };
        if input.capability_id != approved.prepared.capability_id {
            return Err(BmadCapabilityCoordinatorError::CapabilityBindingMismatch);
        }
        if input.manifest_hash != approved.prepared.manifest.manifest_hash
            || input.decision_id != *approved.decision.decision_id()
        {
            return Err(BmadCapabilityCoordinatorError::ConsentBindingMismatch);
        }
        Ok(())
    }
}

fn dispatch_and_verify(
    approved: &ApprovedCapabilityRun,
    consumption: desktop_egress::DecisionConsumption,
    submitted_at: UnixMillis,
    transport: &dyn BmadCapabilityTransport,
    verifier: &dyn BmadCapabilityOutputVerifier,
) -> Result<BmadCapabilityOutput, (CapabilityTerminalReason, BmadCapabilityCoordinatorError)> {
    let authorized_request = AuthorizedModelRequest::new(
        &approved.prepared.manifest,
        &approved.prepared.invocation_binding,
        consumption,
    )
    .map_err(|_| {
        (
            CapabilityTerminalReason::ConsentConsumed,
            BmadCapabilityCoordinatorError::Integrity,
        )
    })?;
    let (_dispatched, raw_output) = transport
        .send(
            authorized_request,
            &approved.prepared.deterministic_fixture,
            submitted_at,
        )
        .map_err(|_| {
            (
                CapabilityTerminalReason::ConsentConsumed,
                BmadCapabilityCoordinatorError::Transport,
            )
        })?;
    let output = verifier
        .verify(&approved.prepared.capability_id, &raw_output)
        .map_err(|_| {
            (
                CapabilityTerminalReason::OutputRejected,
                BmadCapabilityCoordinatorError::OutputRejected,
            )
        })?;
    if output.schema_id() != approved.prepared.output_schema_id {
        return Err((
            CapabilityTerminalReason::OutputRejected,
            BmadCapabilityCoordinatorError::ResultArchetypeMismatch,
        ));
    }
    Ok(output)
}

const fn map_egress_error(error: EgressError) -> BmadCapabilityCoordinatorError {
    match error {
        EgressError::DecisionExpired => BmadCapabilityCoordinatorError::ConsentExpired,
        EgressError::DecisionAlreadyConsumed | EgressError::DecisionCancelled => {
            BmadCapabilityCoordinatorError::ConsentAlreadyConsumed
        }
        EgressError::DecisionBindingMismatch | EgressError::DecisionAlreadyExists => {
            BmadCapabilityCoordinatorError::ConsentBindingMismatch
        }
        _ => BmadCapabilityCoordinatorError::Integrity,
    }
}
