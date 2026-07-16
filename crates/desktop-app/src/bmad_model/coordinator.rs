use std::fmt;
use std::sync::Arc;

#[cfg(feature = "deterministic-help")]
use desktop_cloud::verify_dispatched_model_response;
use desktop_cloud::{AuthorizedModelRequest, CloudError, ModelReceiptStatus};
use desktop_egress::{
    ApproveDecisionInput, CancelDecisionInput, ConsentService, ConsumeDecisionInput,
    ContextDecisionEvidence, ContextEgressManifest, ContextReviewProjection, DecisionEvidenceInput,
    EgressError, MemoryDecisionLedger, ModelInvocationBinding, ModelInvocationBindingDraft,
    PendingContextDecision, RetentionMode,
};
use desktop_ipc::{
    decode_retained_bmad_help_run, project_completed_bmad_help_run,
    BmadHelpReceiptStatusProjection, BmadHelpReceiptSummaryInput, BmadHelpRetentionProjection,
    BmadHelpRunCompletedProjection,
};
use desktop_runtime::{
    canonical_hash, BmadCompiledHelpInvocation, BmadHelpBindingCompiler, BmadHelpIntent,
    BmadHelpMaterializer, BmadHelpRecordIds, BmadMethodHelpRecommendation,
    BmadVerifiedHelpProposal, ContractId, MethodAdvanceRequest, MethodServiceError, MethodSession,
    MethodSessionRepository, MethodSessionService, MethodState, Sha256Digest, UnixMillis,
};
use desktop_store::{BmadHelpRunCreationReceipt, BmadHelpRunLatest, StoreError};
use serde::Serialize;

use super::bridge::{bridge_method_context_decision, BmadHelpDecisionBridgeExpectation};
use super::config::{current_help_model_configuration, HelpModelMode};
use super::context::{
    derive_deterministic_policy, derived_contract_id, prepare_help_context, HelpContextInput,
};
use super::transport::BmadHelpTransport;
#[cfg(feature = "deterministic-help")]
use super::transport::DeterministicHelpTransport;
#[cfg(not(feature = "deterministic-help"))]
use super::transport::OfflineHelpTransport;
use super::verification::BmadHelpProposalValidator;
#[cfg(feature = "deterministic-help")]
use super::verification::{deterministic_receipt_verifier, DeterministicReceiptVerifier};
use crate::bmad_foundation::BmadLoadedFoundation;
use crate::state::{HostState, RendererSessionGuard};

const CONSENT_DISCLOSURE: &str =
    "Only the exact reviewed context shown here will be sent once. Redaction reduces risk but cannot prove that every secret was detected.";

pub(crate) struct PrepareBmadHelpReviewInput<'a> {
    pub renderer_session: &'a RendererSessionGuard<'a>,
    pub workspace_id: ContractId,
    pub workspace_grant_epoch: u64,
    pub created_at: UnixMillis,
}

pub(crate) struct ApproveBmadHelpReviewInput<'a> {
    pub renderer_session: &'a RendererSessionGuard<'a>,
    pub workspace_id: ContractId,
    pub workspace_grant_epoch: u64,
    pub manifest_hash: Sha256Digest,
    pub approved_at: UnixMillis,
}

pub(crate) struct CancelBmadHelpReviewInput<'a> {
    pub renderer_session: &'a RendererSessionGuard<'a>,
    pub workspace_id: ContractId,
    pub workspace_grant_epoch: u64,
    pub manifest_hash: Sha256Digest,
    pub decision_id: ContractId,
    pub cancelled_at: UnixMillis,
}

pub(crate) struct SubmitBmadHelpReviewInput<'a> {
    pub renderer_session: &'a RendererSessionGuard<'a>,
    pub workspace_id: ContractId,
    pub workspace_grant_epoch: u64,
    pub manifest_hash: Sha256Digest,
    pub decision_id: ContractId,
    pub submitted_at: UnixMillis,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BmadHelpReviewProjection {
    pub renderer_session_id: ContractId,
    pub workspace_id: ContractId,
    pub workspace_grant_epoch: u64,
    pub workspace_catalog_version: u64,
    pub run_id: ContractId,
    pub session_id: ContractId,
    pub destination_label: String,
    pub development_only: bool,
    pub consent_disclosure: String,
    pub consent_disclosure_hash: Sha256Digest,
    pub context: ContextReviewProjection,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BmadHelpApprovedProjection {
    pub manifest_hash: Sha256Digest,
    pub decision_id: ContractId,
    pub expires_at: UnixMillis,
    pub send_eligible: bool,
}

pub(super) struct PreparedBmadHelpReview {
    pub renderer_session_id: ContractId,
    pub workspace_id: ContractId,
    pub workspace_grant_epoch: u64,
    pub workspace_catalog_version: u64,
    pub creation: BmadHelpRunCreationReceipt,
    pub compiled: BmadCompiledHelpInvocation,
    pub manifest: ContextEgressManifest,
    pub invocation_binding: ModelInvocationBinding,
    pub deterministic_fixture: String,
}

impl fmt::Debug for PreparedBmadHelpReview {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedBmadHelpReview")
            .field("renderer_session_id", &self.renderer_session_id)
            .field("workspace_id", &self.workspace_id)
            .field("workspace_grant_epoch", &self.workspace_grant_epoch)
            .field("workspace_catalog_version", &self.workspace_catalog_version)
            .field("run_id", &self.creation.run_id)
            .field("manifest", &"<redacted>")
            .field("compiled", &"<redacted>")
            .field("invocation_binding", &"<redacted>")
            .field("deterministic_fixture", &"<redacted>")
            .finish()
    }
}

struct ApprovedBmadHelpReview {
    prepared: PreparedBmadHelpReview,
    decision: PendingContextDecision,
    issued_at: UnixMillis,
}

impl fmt::Debug for ApprovedBmadHelpReview {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApprovedBmadHelpReview")
            .field("prepared", &"<redacted>")
            .field("decision", &"<redacted>")
            .field("issued_at", &self.issued_at)
            .finish()
    }
}

#[derive(Clone, Debug)]
struct BmadHelpAdvancing {
    workspace: ContractId,
    run: ContractId,
    session: ContractId,
    invocation: ContractId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BmadHelpTerminalReason {
    Cancelled,
    ConsentExpired,
    ConsentConsumed,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BmadHelpTerminalState {
    reason: BmadHelpTerminalReason,
}

enum ActiveBmadHelp {
    ReviewRequired(Box<PreparedBmadHelpReview>),
    Approved(Box<ApprovedBmadHelpReview>),
    Advancing(BmadHelpAdvancing),
    Completed(Box<BmadHelpRunCompletedProjection>),
    Terminal(BmadHelpTerminalState),
}

impl fmt::Debug for ActiveBmadHelp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReviewRequired(_) => formatter.write_str("ReviewRequired(<redacted>)"),
            Self::Approved(_) => formatter.write_str("Approved(<redacted>)"),
            Self::Advancing(value) => formatter.debug_tuple("Advancing").field(value).finish(),
            Self::Completed(_) => formatter.write_str("Completed(<renderer-safe>)"),
            Self::Terminal(value) => formatter.debug_tuple("Terminal").field(value).finish(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BmadHelpCoordinatorError {
    SupportPlaneOffline,
    Unauthorized,
    Conflict,
    Integrity,
    Recovery,
    ConsentExpired,
    ConsentBindingMismatch,
    ConsentAlreadyConsumed,
    TransportFailed,
    ResponseBindingMismatch,
    InvalidModelOutput,
    ReceiptInvalid,
}

pub(crate) struct BmadHelpCoordinator {
    ledger: MemoryDecisionLedger,
    active: Option<ActiveBmadHelp>,
    transport: Box<dyn BmadHelpTransport>,
    #[cfg(feature = "deterministic-help")]
    receipt_verifier: Option<DeterministicReceiptVerifier>,
}

impl fmt::Debug for BmadHelpCoordinator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BmadHelpCoordinator")
            .field("ledger", &"<redacted>")
            .field("active", &self.active)
            .field("transport", &"<redacted>")
            .field("receipt_verifier", &"<redacted>")
            .finish()
    }
}

impl BmadHelpCoordinator {
    #[must_use]
    pub fn new() -> Self {
        #[cfg(feature = "deterministic-help")]
        let transport: Box<dyn BmadHelpTransport> = Box::new(DeterministicHelpTransport);
        #[cfg(not(feature = "deterministic-help"))]
        let transport: Box<dyn BmadHelpTransport> = Box::new(OfflineHelpTransport);
        Self {
            ledger: MemoryDecisionLedger::default(),
            active: None,
            transport,
            #[cfg(feature = "deterministic-help")]
            receipt_verifier: deterministic_receipt_verifier().ok(),
        }
    }

    #[cfg(test)]
    pub(super) fn with_transport_for_test(transport: Box<dyn BmadHelpTransport>) -> Self {
        Self {
            ledger: MemoryDecisionLedger::default(),
            active: None,
            transport,
            #[cfg(feature = "deterministic-help")]
            receipt_verifier: deterministic_receipt_verifier().ok(),
        }
    }

    pub fn invalidate(&mut self, invalidated_at: UnixMillis) {
        if let Some(ActiveBmadHelp::Approved(approved)) = self.active.take() {
            let _ = ConsentService::new(&self.ledger).cancel(CancelDecisionInput {
                decision: &approved.decision,
                cancelled_at: invalidated_at,
            });
        }
        self.active = None;
    }

    #[must_use]
    pub const fn has_active_review(&self) -> bool {
        matches!(
            self.active,
            Some(ActiveBmadHelp::ReviewRequired(_) | ActiveBmadHelp::Approved(_))
        )
    }

    #[cfg(test)]
    #[must_use]
    pub fn active_fixture_for_test(&self) -> &str {
        match self.active.as_ref() {
            Some(ActiveBmadHelp::ReviewRequired(active)) => &active.deterministic_fixture,
            Some(ActiveBmadHelp::Approved(active)) => &active.prepared.deterministic_fixture,
            _ => "",
        }
    }

    #[expect(
        clippy::needless_pass_by_value,
        reason = "the transition input is a small one-use command capability whose complete value is validated together"
    )]
    pub fn approve(
        &mut self,
        state: &HostState,
        input: ApproveBmadHelpReviewInput<'_>,
    ) -> Result<BmadHelpApprovedProjection, BmadHelpCoordinatorError> {
        let prepared = match self.active.as_ref() {
            Some(ActiveBmadHelp::ReviewRequired(prepared)) => prepared,
            Some(ActiveBmadHelp::Approved(_)) => return Err(BmadHelpCoordinatorError::Conflict),
            Some(
                ActiveBmadHelp::Advancing(_)
                | ActiveBmadHelp::Completed(_)
                | ActiveBmadHelp::Terminal(_),
            ) => return Err(BmadHelpCoordinatorError::ConsentAlreadyConsumed),
            None => return Err(BmadHelpCoordinatorError::Unauthorized),
        };
        if input.manifest_hash != prepared.manifest.manifest_hash {
            return Err(BmadHelpCoordinatorError::ConsentBindingMismatch);
        }
        validate_active_authority(
            state,
            input.renderer_session,
            prepared,
            &input.workspace_id,
            input.workspace_grant_epoch,
            MethodState::ContextReviewRequired,
            3,
        )?;
        let Some(ActiveBmadHelp::ReviewRequired(prepared)) = self.active.take() else {
            return Err(BmadHelpCoordinatorError::Conflict);
        };
        if input.approved_at < prepared.manifest.draft.created_at
            || input.approved_at >= prepared.manifest.draft.expires_at
        {
            self.active = Some(ActiveBmadHelp::Terminal(BmadHelpTerminalState {
                reason: BmadHelpTerminalReason::ConsentExpired,
            }));
            return Err(BmadHelpCoordinatorError::ConsentExpired);
        }
        let expires_at = UnixMillis(
            input
                .approved_at
                .0
                .checked_add(5 * 60 * 1_000)
                .map_or(prepared.manifest.draft.expires_at.0, |maximum| {
                    maximum.min(prepared.manifest.draft.expires_at.0)
                }),
        );
        let decision_digest = canonical_hash(
            "bmad-help-context-decision-id",
            1,
            &ContextDecisionIdentity {
                session_id: &prepared.creation.session_id,
                manifest_hash: prepared.manifest.manifest_hash,
                invocation_binding_hash: prepared.invocation_binding.binding_hash,
                issued_at: input.approved_at,
            },
        )
        .map_err(|_| BmadHelpCoordinatorError::Integrity)?;
        let decision_id = derived_contract_id("decision", decision_digest)
            .map_err(|_| BmadHelpCoordinatorError::Integrity)?;
        let decision = match ConsentService::new(&self.ledger).approve(ApproveDecisionInput {
            manifest: &prepared.manifest,
            binding: &prepared.invocation_binding,
            decision_id: decision_id.clone(),
            issued_at: input.approved_at,
            expires_at,
        }) {
            Ok(decision) => decision,
            Err(error) => {
                self.active = Some(ActiveBmadHelp::ReviewRequired(prepared));
                return Err(map_egress_error(error));
            }
        };
        self.active = Some(ActiveBmadHelp::Approved(Box::new(ApprovedBmadHelpReview {
            prepared: *prepared,
            decision,
            issued_at: input.approved_at,
        })));
        Ok(BmadHelpApprovedProjection {
            manifest_hash: input.manifest_hash,
            decision_id,
            expires_at,
            send_eligible: true,
        })
    }

    #[expect(
        clippy::needless_pass_by_value,
        reason = "the transition input is a small one-use command capability whose complete value is validated together"
    )]
    pub fn cancel(
        &mut self,
        state: &HostState,
        input: CancelBmadHelpReviewInput<'_>,
    ) -> Result<(), BmadHelpCoordinatorError> {
        let approved = match self.active.as_ref() {
            Some(ActiveBmadHelp::Approved(approved)) => approved,
            Some(ActiveBmadHelp::ReviewRequired(_)) => {
                return Err(BmadHelpCoordinatorError::ConsentBindingMismatch);
            }
            Some(
                ActiveBmadHelp::Advancing(_)
                | ActiveBmadHelp::Completed(_)
                | ActiveBmadHelp::Terminal(_),
            ) => return Err(BmadHelpCoordinatorError::ConsentAlreadyConsumed),
            None => return Err(BmadHelpCoordinatorError::Unauthorized),
        };
        if input.manifest_hash != approved.prepared.manifest.manifest_hash
            || input.decision_id != *approved.decision.decision_id()
        {
            return Err(BmadHelpCoordinatorError::ConsentBindingMismatch);
        }
        validate_active_authority(
            state,
            input.renderer_session,
            &approved.prepared,
            &input.workspace_id,
            input.workspace_grant_epoch,
            MethodState::ContextReviewRequired,
            3,
        )?;
        let Some(ActiveBmadHelp::Approved(approved)) = self.active.take() else {
            return Err(BmadHelpCoordinatorError::Conflict);
        };
        ConsentService::new(&self.ledger)
            .cancel(CancelDecisionInput {
                decision: &approved.decision,
                cancelled_at: input.cancelled_at,
            })
            .map_err(map_egress_error)?;
        self.active = Some(ActiveBmadHelp::Terminal(BmadHelpTerminalState {
            reason: BmadHelpTerminalReason::Cancelled,
        }));
        Ok(())
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the one-shot submit path keeps every consent, Method v4/v5, dispatch, verification, materialization, projection, and v6 finalization edge visibly ordered"
    )]
    #[expect(
        clippy::needless_pass_by_value,
        reason = "the transition input is a small one-use command capability whose complete value is validated together"
    )]
    pub fn submit(
        &mut self,
        state: &HostState,
        input: SubmitBmadHelpReviewInput<'_>,
    ) -> Result<BmadHelpRunCompletedProjection, BmadHelpCoordinatorError> {
        let approved = match self.active.as_ref() {
            Some(ActiveBmadHelp::Approved(approved)) => approved,
            Some(ActiveBmadHelp::ReviewRequired(_)) => {
                return Err(BmadHelpCoordinatorError::ConsentBindingMismatch);
            }
            Some(
                ActiveBmadHelp::Advancing(_)
                | ActiveBmadHelp::Completed(_)
                | ActiveBmadHelp::Terminal(_),
            ) => return Err(BmadHelpCoordinatorError::ConsentAlreadyConsumed),
            None => return Err(BmadHelpCoordinatorError::Unauthorized),
        };
        if input.manifest_hash != approved.prepared.manifest.manifest_hash
            || input.decision_id != *approved.decision.decision_id()
        {
            return Err(BmadHelpCoordinatorError::ConsentBindingMismatch);
        }
        validate_active_authority(
            state,
            input.renderer_session,
            &approved.prepared,
            &input.workspace_id,
            input.workspace_grant_epoch,
            MethodState::ContextReviewRequired,
            3,
        )?;
        let Some(ActiveBmadHelp::Approved(approved)) = self.active.take() else {
            return Err(BmadHelpCoordinatorError::Conflict);
        };
        if input.submitted_at < approved.issued_at
            || input.submitted_at > approved.decision.expires_at()
        {
            let _ = ConsentService::new(&self.ledger).cancel(CancelDecisionInput {
                decision: &approved.decision,
                cancelled_at: input.submitted_at,
            });
            self.active = Some(ActiveBmadHelp::Terminal(BmadHelpTerminalState {
                reason: BmadHelpTerminalReason::ConsentExpired,
            }));
            return Err(BmadHelpCoordinatorError::ConsentExpired);
        }

        let Ok(authority) = state.ready_workspace_commit() else {
            self.active = Some(ActiveBmadHelp::Approved(approved));
            return Err(BmadHelpCoordinatorError::Unauthorized);
        };
        if authority.workspace_catalog_version() != approved.prepared.workspace_catalog_version {
            self.active = Some(ActiveBmadHelp::Approved(approved));
            return Err(BmadHelpCoordinatorError::Unauthorized);
        }
        let Ok(workspace_authority) = state
            .workspace
            .authorize_scope(input.workspace_id.as_str(), input.workspace_grant_epoch)
        else {
            self.active = Some(ActiveBmadHelp::Approved(approved));
            return Err(BmadHelpCoordinatorError::Unauthorized);
        };
        let workspace = workspace_authority.projection();
        if workspace.workspace_id != approved.prepared.workspace_id.as_str()
            || workspace.grant_epoch != approved.prepared.workspace_grant_epoch
            || workspace.project_id != approved.prepared.creation.scope.project_id.as_str()
        {
            self.active = Some(ActiveBmadHelp::Approved(approved));
            return Err(BmadHelpCoordinatorError::Unauthorized);
        }
        let store = match state.method_store(authority.authority()) {
            Ok(store) => store,
            Err(error) => {
                let mapped = map_store_error(&error);
                self.active = Some(ActiveBmadHelp::Approved(approved));
                return Err(mapped);
            }
        };
        let session_v3 = match store.load_method_session(
            &approved.prepared.creation.scope,
            &approved.prepared.creation.session_id,
        ) {
            Ok(Some(session))
                if session.state() == MethodState::ContextReviewRequired
                    && session.version() == 3 =>
            {
                session
            }
            Ok(_) => {
                self.active = Some(ActiveBmadHelp::Approved(approved));
                return Err(BmadHelpCoordinatorError::Conflict);
            }
            Err(error) => {
                let mapped = map_store_error(&error);
                self.active = Some(ActiveBmadHelp::Approved(approved));
                return Err(mapped);
            }
        };

        let method_decision_result = (|| {
            let consent_service = ConsentService::new(&self.ledger);
            let evidence = consent_service
                .evidence(DecisionEvidenceInput {
                    decision: &approved.decision,
                    observed_at: input.submitted_at,
                })
                .map_err(map_egress_error)?;
            bridge_decision(evidence, &session_v3, &approved, input.submitted_at)
        })();
        let method_decision = match method_decision_result {
            Ok(decision) => decision,
            Err(error) => {
                self.active = Some(ActiveBmadHelp::Approved(approved));
                return Err(error);
            }
        };
        let service = MethodSessionService::new(store);
        let ready_session = match service.record_context_review(
            &approved.prepared.creation.scope,
            &approved.prepared.creation.session_id,
            3,
            method_decision,
        ) {
            Ok(session) => session,
            Err(error) => {
                self.active = Some(ActiveBmadHelp::Approved(approved));
                return Err(map_service_error(error));
            }
        };
        if ready_session.state() != MethodState::Ready || ready_session.version() != 4 {
            self.active = Some(ActiveBmadHelp::Terminal(BmadHelpTerminalState {
                reason: BmadHelpTerminalReason::Failed,
            }));
            return Err(BmadHelpCoordinatorError::Integrity);
        }

        let invocation_digest = canonical_hash(
            "bmad-help-model-invocation-id",
            1,
            &InvocationIdentity {
                session_id: &approved.prepared.creation.session_id,
                decision_id: approved.decision.decision_id(),
                submitted_at: input.submitted_at,
            },
        )
        .map_err(|_| BmadHelpCoordinatorError::Integrity)?;
        let invocation_id = derived_contract_id("invoke", invocation_digest)
            .map_err(|_| BmadHelpCoordinatorError::Integrity)?;
        let consumption = match ConsentService::new(&self.ledger).consume(ConsumeDecisionInput {
            decision: &approved.decision,
            binding: &approved.prepared.invocation_binding,
            invocation_id: invocation_id.clone(),
            consumed_at: input.submitted_at,
        }) {
            Ok(consumption) => consumption,
            Err(error) => {
                self.active = Some(ActiveBmadHelp::Terminal(BmadHelpTerminalState {
                    reason: terminal_reason_for_egress(error),
                }));
                return Err(map_egress_error(error));
            }
        };
        let decision_id = consumption.decision_id().clone();
        let decision_consumption_hash = consumption.consumption_hash();
        let authorized_request = match AuthorizedModelRequest::new(
            &approved.prepared.manifest,
            &approved.prepared.invocation_binding,
            consumption,
        ) {
            Ok(request) => request,
            Err(error) => {
                self.active = Some(ActiveBmadHelp::Terminal(BmadHelpTerminalState {
                    reason: BmadHelpTerminalReason::ConsentConsumed,
                }));
                return Err(map_cloud_error(&error));
            }
        };
        let model_request_id = authorized_request.request_id().clone();
        let model_request_hash = authorized_request.request_hash();
        let d2_binding_hash = approved.prepared.invocation_binding.binding_hash;
        let session_authority_hash = ready_session
            .session_authority_hash()
            .map_err(|_| BmadHelpCoordinatorError::Integrity)?;
        let model_bridge_binding_hash = ready_session
            .model_bridge_binding_hash(&d2_binding_hash)
            .map_err(|_| BmadHelpCoordinatorError::Integrity)?;
        let advance_receipt = match store.begin_method_advance(
            &approved.prepared.creation.scope,
            &approved.prepared.creation.session_id,
            approved.prepared.compiled.exact_binding(),
            MethodAdvanceRequest {
                invocation_id: invocation_id.clone(),
                idempotency_key: format!("bmad-help-{}", invocation_id.as_str()),
                decision_id,
                decision_consumption_hash,
                model_request_id,
                model_request_hash,
                session_authority_hash,
                d2_model_invocation_binding_hash: d2_binding_hash,
                model_bridge_binding_hash,
                expected_version: 4,
            },
        ) {
            Ok(receipt) => receipt,
            Err(error) => {
                self.active = Some(ActiveBmadHelp::Terminal(BmadHelpTerminalState {
                    reason: BmadHelpTerminalReason::ConsentConsumed,
                }));
                return Err(map_store_error(&error));
            }
        };
        let advancing = BmadHelpAdvancing {
            workspace: approved.prepared.workspace_id.clone(),
            run: approved.prepared.creation.run_id.clone(),
            session: approved.prepared.creation.session_id.clone(),
            invocation: invocation_id.clone(),
        };
        self.active = Some(ActiveBmadHelp::Advancing(advancing));

        let (dispatched, raw_output) = match self.transport.send(
            authorized_request,
            &approved.prepared.deterministic_fixture,
            input.submitted_at,
        ) {
            Ok(output) => output,
            Err(error) => {
                self.active = Some(ActiveBmadHelp::Terminal(BmadHelpTerminalState {
                    reason: BmadHelpTerminalReason::ConsentConsumed,
                }));
                return Err(map_cloud_error(&error));
            }
        };
        let validator = BmadHelpProposalValidator::new(
            approved
                .prepared
                .manifest
                .draft
                .canonical_output_schema_id
                .clone(),
            approved
                .prepared
                .manifest
                .draft
                .canonical_output_schema_hash,
        );
        #[cfg(feature = "deterministic-help")]
        let verified = match self.receipt_verifier.as_ref() {
            Some(verifier) => {
                verify_dispatched_model_response(dispatched, raw_output, &validator, verifier)
            }
            None => Err(CloudError::ReceiptInvalid),
        };
        #[cfg(not(feature = "deterministic-help"))]
        let verified: Result<desktop_cloud::VerifiedModelOutput, CloudError> = {
            drop((dispatched, raw_output, validator));
            Err(CloudError::Offline)
        };
        let verified = match verified {
            Ok(verified) => verified,
            Err(error) => {
                self.active = Some(ActiveBmadHelp::Terminal(BmadHelpTerminalState {
                    reason: BmadHelpTerminalReason::ConsentConsumed,
                }));
                return Err(map_cloud_error(&error));
            }
        };
        let receipt_evidence_hash =
            canonical_hash("bmad-model-receipt-evidence", 1, verified.receipt())
                .map_err(|_| BmadHelpCoordinatorError::Integrity)?;
        let proposal = BmadVerifiedHelpProposal::from_trusted_host_evidence(
            Arc::<[u8]>::from(verified.payload_bytes()),
            advance_receipt,
            receipt_evidence_hash,
        )
        .map_err(|_| BmadHelpCoordinatorError::InvalidModelOutput)?;
        let session_v5 = store
            .load_method_session(
                &approved.prepared.creation.scope,
                &approved.prepared.creation.session_id,
            )
            .map_err(|error| map_store_error(&error))?
            .ok_or(BmadHelpCoordinatorError::Integrity)?;
        if session_v5.state() != MethodState::Advancing || session_v5.version() != 5 {
            self.active = Some(ActiveBmadHelp::Terminal(BmadHelpTerminalState {
                reason: BmadHelpTerminalReason::ConsentConsumed,
            }));
            return Err(BmadHelpCoordinatorError::Integrity);
        }
        let record_digest = canonical_hash(
            "bmad-help-canonical-record-ids",
            1,
            &CanonicalRecordIdentity {
                session_id: &approved.prepared.creation.session_id,
                invocation_id: &invocation_id,
                receipt_id: &verified.receipt().receipt_id,
            },
        )
        .map_err(|_| BmadHelpCoordinatorError::Integrity)?;
        let records = BmadHelpMaterializer::materialize(
            &approved.prepared.compiled,
            &session_v5,
            &proposal,
            BmadHelpRecordIds {
                recommendation_id: derived_contract_id("recommendation", record_digest)
                    .map_err(|_| BmadHelpCoordinatorError::Integrity)?,
                result_id: derived_contract_id(
                    "result",
                    canonical_hash("bmad-help-result-id", 1, &record_digest)
                        .map_err(|_| BmadHelpCoordinatorError::Integrity)?,
                )
                .map_err(|_| BmadHelpCoordinatorError::Integrity)?,
            },
            input.submitted_at,
        )
        .map_err(|_| BmadHelpCoordinatorError::InvalidModelOutput)?;
        let display_name =
            recommendation_display_name(records.recommendation(), &approved.prepared.compiled)?;
        let receipt = verified.receipt();
        if receipt.status != ModelReceiptStatus::Succeeded
            || receipt.retention_mode != RetentionMode::TransientNoStore
        {
            self.active = Some(ActiveBmadHelp::Terminal(BmadHelpTerminalState {
                reason: BmadHelpTerminalReason::ConsentConsumed,
            }));
            return Err(BmadHelpCoordinatorError::ReceiptInvalid);
        }
        let completed_projection = project_completed_bmad_help_run(
            records.recommendation(),
            display_name.as_deref(),
            approved.prepared.workspace_id,
            approved.prepared.creation.run_id.clone(),
            approved.prepared.creation.session_id.clone(),
            &BmadHelpReceiptSummaryInput {
                receipt_id: receipt.receipt_id.clone(),
                status: BmadHelpReceiptStatusProjection::Succeeded,
                retention_mode: BmadHelpRetentionProjection::TransientNoStore,
                region: receipt.region.clone(),
                input_bytes: receipt.input_bytes,
                output_bytes: receipt.output_bytes,
                started_at: receipt.started_at,
                completed_at: receipt.completed_at,
            },
        )
        .map_err(|_| BmadHelpCoordinatorError::InvalidModelOutput)?;
        let completed_bytes = serde_json::to_vec(&completed_projection)
            .map_err(|_| BmadHelpCoordinatorError::InvalidModelOutput)?;
        let completed_session = match store.finalize_bmad_help(
            &approved.prepared.creation.scope,
            &approved.prepared.creation.session_id,
            5,
            &records,
            &completed_bytes,
            input.submitted_at,
        ) {
            Ok(session) => session,
            Err(error) => {
                self.active = Some(ActiveBmadHelp::Terminal(BmadHelpTerminalState {
                    reason: BmadHelpTerminalReason::ConsentConsumed,
                }));
                return Err(map_store_error(&error));
            }
        };
        if completed_session.state() != MethodState::Completed || completed_session.version() != 6 {
            self.active = Some(ActiveBmadHelp::Terminal(BmadHelpTerminalState {
                reason: BmadHelpTerminalReason::ConsentConsumed,
            }));
            return Err(BmadHelpCoordinatorError::Integrity);
        }
        self.active = Some(ActiveBmadHelp::Completed(Box::new(
            completed_projection.clone(),
        )));
        Ok(completed_projection)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the audited prepare path intentionally keeps authentication, v1-v3 persistence, and exact D2 binding in one visible authority order"
    )]
    pub fn prepare(
        &mut self,
        state: &HostState,
        foundation: &BmadLoadedFoundation,
        input: PrepareBmadHelpReviewInput<'_>,
    ) -> Result<BmadHelpReviewProjection, BmadHelpCoordinatorError> {
        let authority = state
            .ready_workspace_commit()
            .map_err(|_| BmadHelpCoordinatorError::Unauthorized)?;
        let workspace_catalog_version = authority.workspace_catalog_version();
        let workspace_authority = state
            .workspace
            .authorize_scope(input.workspace_id.as_str(), input.workspace_grant_epoch)
            .map_err(|_| BmadHelpCoordinatorError::Unauthorized)?;
        let workspace = workspace_authority.projection();
        if workspace.workspace_id != input.workspace_id.as_str()
            || workspace.grant_epoch != input.workspace_grant_epoch
        {
            return Err(BmadHelpCoordinatorError::Unauthorized);
        }
        let latest = state
            .latest_bmad_help_run(
                authority.authority(),
                &input.workspace_id,
                workspace_catalog_version,
            )
            .map_err(|error| map_store_error(&error))?;
        let creation = match latest {
            BmadHelpRunLatest::Retained(creation) => creation,
            BmadHelpRunLatest::None | BmadHelpRunLatest::LegacyProjectionUnavailable => {
                return Err(BmadHelpCoordinatorError::Unauthorized);
            }
            BmadHelpRunLatest::Interrupted(_) | BmadHelpRunLatest::Completed(_) => {
                return Err(BmadHelpCoordinatorError::Conflict);
            }
        };
        if creation.scope.project_id.as_str() != workspace.project_id
            || creation.scope.run_id != creation.run_id
        {
            return Err(BmadHelpCoordinatorError::Integrity);
        }
        let retained = decode_retained_bmad_help_run(
            &creation.renderer_projection,
            &input.workspace_id,
            &creation.run_id,
            &creation.session_id,
        )
        .map_err(|_| BmadHelpCoordinatorError::Integrity)?;
        let intent = BmadHelpIntent::new(retained.current_intent().to_owned())
            .map_err(|_| BmadHelpCoordinatorError::Integrity)?;
        let store = state
            .method_store(authority.authority())
            .map_err(|error| map_store_error(&error))?;
        let retained_session = store
            .load_method_session(&creation.scope, &creation.session_id)
            .map_err(|error| map_store_error(&error))?
            .ok_or(BmadHelpCoordinatorError::Integrity)?;
        if retained_session.state() != MethodState::Created || retained_session.version() != 1 {
            return Err(BmadHelpCoordinatorError::Conflict);
        }

        let configuration =
            current_help_model_configuration().map_err(|_| BmadHelpCoordinatorError::Integrity)?;
        if configuration.mode == HelpModelMode::Offline {
            return Err(BmadHelpCoordinatorError::SupportPlaneOffline);
        }

        self.invalidate(input.created_at);
        let base_compiled = BmadHelpBindingCompiler::compile(
            foundation.help_invocation(),
            foundation.catalog(),
            &configuration.trusted_profile,
        )
        .map_err(|_| BmadHelpCoordinatorError::Integrity)?;
        let policy = derive_deterministic_policy(&base_compiled, &intent)
            .map_err(|_| BmadHelpCoordinatorError::Integrity)?;
        let compiled = base_compiled
            .with_evidence_allowlist(policy.evidence_tokens.clone())
            .map_err(|_| BmadHelpCoordinatorError::Integrity)?;
        let service = MethodSessionService::new(store);
        service
            .bind_invocation(
                &creation.scope,
                &creation.session_id,
                1,
                compiled.exact_binding().clone(),
                compiled.step_table().clone(),
            )
            .map_err(map_service_error)?;
        let reviewed_session = service
            .request_context_review(&creation.scope, &creation.session_id, 2)
            .map_err(map_service_error)?;
        if reviewed_session.state() != MethodState::ContextReviewRequired
            || reviewed_session.version() != 3
        {
            return Err(BmadHelpCoordinatorError::Integrity);
        }

        let manifest = prepare_help_context(HelpContextInput {
            compiled: &compiled,
            intent: &intent,
            policy: &policy,
            configuration: &configuration,
            tenant_ref: state.installation_id().clone(),
            project_ref: creation.scope.project_id.clone(),
            run_ref: creation.run_id.clone(),
            created_at: input.created_at,
        })
        .map_err(|_| BmadHelpCoordinatorError::Integrity)?;
        let consent_disclosure_hash = canonical_hash(
            "bmad-help-consent-disclosure",
            1,
            &ConsentDisclosureBinding {
                schema_version: "sapphirus.bmad-help-consent-disclosure.v1",
                disclosure: CONSENT_DISCLOSURE,
                manifest_hash: manifest.manifest_hash,
                destination_label: configuration.destination_label,
            },
        )
        .map_err(|_| BmadHelpCoordinatorError::Integrity)?;
        let request_digest = canonical_hash(
            "bmad-help-model-request-id",
            1,
            &ModelRequestIdentity {
                creation_request_id: &creation.request_id,
                session_authority_hash: reviewed_session
                    .session_authority_hash()
                    .map_err(|_| BmadHelpCoordinatorError::Integrity)?,
                manifest_hash: manifest.manifest_hash,
            },
        )
        .map_err(|_| BmadHelpCoordinatorError::Integrity)?;
        let invocation_binding = ModelInvocationBindingDraft {
            schema_version: "sapphirus.model-invocation-binding.v1".to_owned(),
            request_id: derived_contract_id("modelreq", request_digest)
                .map_err(|_| BmadHelpCoordinatorError::Integrity)?,
            tenant_ref: manifest.draft.tenant_ref.clone(),
            project_ref: manifest.draft.project_ref.clone(),
            run_ref: manifest.draft.run_ref.clone(),
            installation_id: state.installation_id().clone(),
            session_authority_hash: reviewed_session
                .session_authority_hash()
                .map_err(|_| BmadHelpCoordinatorError::Integrity)?,
            manifest_hash: manifest.manifest_hash,
            purpose: manifest.draft.purpose.clone(),
            model_role: manifest.draft.model_role.clone(),
            canonical_output_schema_id: manifest.draft.canonical_output_schema_id.clone(),
            canonical_output_schema_hash: manifest.draft.canonical_output_schema_hash,
            provider_profile_hash: manifest.draft.provider_profile_hash,
            model_profile_hash: manifest.draft.model_profile_hash,
            deployment_hash: manifest.draft.deployment_hash,
            policy_hash: manifest.draft.policy_hash,
            region: manifest.draft.region.clone(),
            retention_mode: manifest.draft.retention_mode,
            consent_disclosure_hash,
        }
        .seal()
        .map_err(|_| BmadHelpCoordinatorError::Integrity)?;
        let projection = BmadHelpReviewProjection {
            renderer_session_id: input.renderer_session.session_id().clone(),
            workspace_id: input.workspace_id.clone(),
            workspace_grant_epoch: input.workspace_grant_epoch,
            workspace_catalog_version,
            run_id: creation.run_id.clone(),
            session_id: creation.session_id.clone(),
            destination_label: configuration.destination_label.to_owned(),
            development_only: true,
            consent_disclosure: CONSENT_DISCLOSURE.to_owned(),
            consent_disclosure_hash,
            context: manifest.review_projection(),
        };
        self.active = Some(ActiveBmadHelp::ReviewRequired(Box::new(
            PreparedBmadHelpReview {
                renderer_session_id: input.renderer_session.session_id().clone(),
                workspace_id: input.workspace_id,
                workspace_grant_epoch: input.workspace_grant_epoch,
                workspace_catalog_version,
                creation,
                compiled,
                manifest,
                invocation_binding,
                deterministic_fixture: policy.deterministic_fixture,
            },
        )));
        Ok(projection)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConsentDisclosureBinding<'a> {
    schema_version: &'static str,
    disclosure: &'static str,
    manifest_hash: Sha256Digest,
    destination_label: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelRequestIdentity<'a> {
    creation_request_id: &'a ContractId,
    session_authority_hash: Sha256Digest,
    manifest_hash: Sha256Digest,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ContextDecisionIdentity<'a> {
    session_id: &'a ContractId,
    manifest_hash: Sha256Digest,
    invocation_binding_hash: Sha256Digest,
    issued_at: UnixMillis,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InvocationIdentity<'a> {
    session_id: &'a ContractId,
    decision_id: &'a ContractId,
    submitted_at: UnixMillis,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[expect(
    clippy::struct_field_names,
    reason = "the canonical identity field names intentionally mirror the three qualified contract identifiers in the hashed domain"
)]
struct CanonicalRecordIdentity<'a> {
    session_id: &'a ContractId,
    invocation_id: &'a ContractId,
    receipt_id: &'a ContractId,
}

fn bridge_decision(
    evidence: ContextDecisionEvidence<'_>,
    session: &MethodSession,
    approved: &ApprovedBmadHelpReview,
    observed_at: UnixMillis,
) -> Result<desktop_runtime::MethodContextDecision, BmadHelpCoordinatorError> {
    let session_authority_hash = session
        .session_authority_hash()
        .map_err(|_| BmadHelpCoordinatorError::Integrity)?;
    let method_binding_hash = approved
        .prepared
        .compiled
        .exact_binding()
        .binding_hash()
        .map_err(|_| BmadHelpCoordinatorError::Integrity)?;
    bridge_method_context_decision(
        evidence,
        &approved.prepared.manifest,
        &approved.prepared.invocation_binding,
        approved.prepared.compiled.exact_binding(),
        &BmadHelpDecisionBridgeExpectation {
            decision_id: approved.decision.decision_id().clone(),
            manifest_hash: approved.prepared.manifest.manifest_hash,
            d2_binding_hash: approved.prepared.invocation_binding.binding_hash,
            session_authority_hash,
            method_binding_hash,
            issued_at: approved.issued_at,
            expires_at: approved.decision.expires_at(),
            observed_at,
        },
    )
    .map_err(|_| BmadHelpCoordinatorError::ConsentBindingMismatch)
}

fn recommendation_display_name(
    recommendation: &BmadMethodHelpRecommendation,
    compiled: &BmadCompiledHelpInvocation,
) -> Result<Option<String>, BmadHelpCoordinatorError> {
    let BmadMethodHelpRecommendation::RecommendedCapability { capability_key, .. } = recommendation
    else {
        return Ok(None);
    };
    compiled
        .catalog_candidates()
        .iter()
        .find(|candidate| {
            candidate.key.package_version_id == capability_key.package_version_id
                && candidate.key.module_code == capability_key.module_code
                && candidate.key.skill_name == capability_key.skill_name
                && candidate.key.action == capability_key.normalized_action
        })
        .map(|candidate| Some(candidate.display_name.clone()))
        .ok_or(BmadHelpCoordinatorError::Integrity)
}

fn validate_active_authority(
    state: &HostState,
    renderer_session: &RendererSessionGuard<'_>,
    prepared: &PreparedBmadHelpReview,
    workspace_id: &ContractId,
    workspace_grant_epoch: u64,
    expected_state: MethodState,
    expected_version: u64,
) -> Result<(), BmadHelpCoordinatorError> {
    if renderer_session.session_id() != &prepared.renderer_session_id
        || workspace_id != &prepared.workspace_id
        || workspace_grant_epoch != prepared.workspace_grant_epoch
    {
        return Err(BmadHelpCoordinatorError::Unauthorized);
    }
    prepared
        .manifest
        .verify()
        .map_err(|_| BmadHelpCoordinatorError::ConsentBindingMismatch)?;
    prepared
        .invocation_binding
        .verify_for(&prepared.manifest)
        .map_err(|_| BmadHelpCoordinatorError::ConsentBindingMismatch)?;
    let authority = state
        .ready_workspace_commit()
        .map_err(|_| BmadHelpCoordinatorError::Unauthorized)?;
    if authority.workspace_catalog_version() != prepared.workspace_catalog_version {
        return Err(BmadHelpCoordinatorError::Unauthorized);
    }
    let workspace_authority = state
        .workspace
        .authorize_scope(workspace_id.as_str(), workspace_grant_epoch)
        .map_err(|_| BmadHelpCoordinatorError::Unauthorized)?;
    let workspace = workspace_authority.projection();
    if workspace.workspace_id != workspace_id.as_str()
        || workspace.grant_epoch != workspace_grant_epoch
        || workspace.project_id != prepared.creation.scope.project_id.as_str()
    {
        return Err(BmadHelpCoordinatorError::Unauthorized);
    }
    let latest = state
        .latest_bmad_help_run(
            authority.authority(),
            workspace_id,
            prepared.workspace_catalog_version,
        )
        .map_err(|error| map_store_error(&error))?;
    let current_creation = match latest {
        BmadHelpRunLatest::Interrupted(creation) => creation,
        BmadHelpRunLatest::Completed(_) => return Err(BmadHelpCoordinatorError::Conflict),
        BmadHelpRunLatest::Retained(_)
        | BmadHelpRunLatest::None
        | BmadHelpRunLatest::LegacyProjectionUnavailable => {
            return Err(BmadHelpCoordinatorError::Integrity);
        }
    };
    if !same_creation(&current_creation, &prepared.creation) {
        return Err(BmadHelpCoordinatorError::Unauthorized);
    }
    let store = state
        .method_store(authority.authority())
        .map_err(|error| map_store_error(&error))?;
    let session = store
        .load_method_session(&prepared.creation.scope, &prepared.creation.session_id)
        .map_err(|error| map_store_error(&error))?
        .ok_or(BmadHelpCoordinatorError::Integrity)?;
    if session.state() != expected_state
        || session.version() != expected_version
        || session
            .session_authority_hash()
            .map_err(|_| BmadHelpCoordinatorError::Integrity)?
            != prepared.invocation_binding.draft.session_authority_hash
        || session
            .current_binding()
            .and_then(desktop_runtime::MethodExactBinding::binding_hash)
            .map_err(|_| BmadHelpCoordinatorError::Integrity)?
            != prepared
                .compiled
                .exact_binding()
                .binding_hash()
                .map_err(|_| BmadHelpCoordinatorError::Integrity)?
    {
        return Err(BmadHelpCoordinatorError::Integrity);
    }
    Ok(())
}

fn same_creation(left: &BmadHelpRunCreationReceipt, right: &BmadHelpRunCreationReceipt) -> bool {
    left.request_id == right.request_id
        && left.session_id == right.session_id
        && left.run_id == right.run_id
        && left.scope == right.scope
        && left.intent_hash == right.intent_hash
        && left.renderer_projection == right.renderer_projection
}

fn map_egress_error(error: EgressError) -> BmadHelpCoordinatorError {
    match error {
        EgressError::DecisionExpired | EgressError::InvalidLifetime => {
            BmadHelpCoordinatorError::ConsentExpired
        }
        EgressError::DecisionAlreadyConsumed | EgressError::DecisionCancelled => {
            BmadHelpCoordinatorError::ConsentAlreadyConsumed
        }
        EgressError::DecisionBindingMismatch
        | EgressError::DecisionIntegrity
        | EgressError::DecisionUnknown
        | EgressError::DecisionAlreadyExists => BmadHelpCoordinatorError::ConsentBindingMismatch,
        _ => BmadHelpCoordinatorError::Integrity,
    }
}

const fn terminal_reason_for_egress(error: EgressError) -> BmadHelpTerminalReason {
    match error {
        EgressError::DecisionExpired | EgressError::InvalidLifetime => {
            BmadHelpTerminalReason::ConsentExpired
        }
        EgressError::DecisionAlreadyConsumed | EgressError::DecisionCancelled => {
            BmadHelpTerminalReason::ConsentConsumed
        }
        _ => BmadHelpTerminalReason::Failed,
    }
}

const fn map_cloud_error(error: &CloudError) -> BmadHelpCoordinatorError {
    match error {
        CloudError::Offline => BmadHelpCoordinatorError::SupportPlaneOffline,
        CloudError::AuthenticationRequired
        | CloudError::EntitlementUnavailable
        | CloudError::FeatureDisabled
        | CloudError::IdentityUnavailable
        | CloudError::ReauthenticationRequired
        | CloudError::TenantMismatch
        | CloudError::SessionInvalidated => BmadHelpCoordinatorError::Unauthorized,
        CloudError::ContextDrift | CloudError::ConsentBindingMismatch => {
            BmadHelpCoordinatorError::ConsentBindingMismatch
        }
        CloudError::ResponseBindingMismatch => BmadHelpCoordinatorError::ResponseBindingMismatch,
        CloudError::InvalidModelOutput => BmadHelpCoordinatorError::InvalidModelOutput,
        CloudError::ReceiptInvalid => BmadHelpCoordinatorError::ReceiptInvalid,
        CloudError::InvalidSupportOrigin | CloudError::TransportFailed => {
            BmadHelpCoordinatorError::TransportFailed
        }
    }
}

fn map_store_error(error: &StoreError) -> BmadHelpCoordinatorError {
    if matches!(error, StoreError::StateConflict) {
        BmadHelpCoordinatorError::Conflict
    } else {
        BmadHelpCoordinatorError::Recovery
    }
}

fn map_service_error(error: MethodServiceError<StoreError>) -> BmadHelpCoordinatorError {
    match error {
        MethodServiceError::Domain(error)
            if error.code() == desktop_runtime::MethodErrorCode::MethodStateConflict =>
        {
            BmadHelpCoordinatorError::Conflict
        }
        MethodServiceError::Domain(_) => BmadHelpCoordinatorError::Integrity,
        MethodServiceError::Repository(error) => map_store_error(&error),
    }
}
