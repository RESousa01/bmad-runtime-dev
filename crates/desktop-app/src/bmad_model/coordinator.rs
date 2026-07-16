use std::fmt;

use desktop_egress::{
    ContextEgressManifest, ContextReviewProjection, MemoryDecisionLedger, ModelInvocationBinding,
    ModelInvocationBindingDraft,
};
use desktop_ipc::decode_retained_bmad_help_run;
use desktop_runtime::{
    canonical_hash, BmadCompiledHelpInvocation, BmadHelpBindingCompiler, BmadHelpIntent,
    ContractId, MethodServiceError, MethodSessionRepository, MethodSessionService, MethodState,
    Sha256Digest, UnixMillis,
};
use desktop_store::{BmadHelpRunCreationReceipt, BmadHelpRunLatest, StoreError};
use serde::Serialize;

use super::config::{current_help_model_configuration, HelpModelMode};
use super::context::{
    derive_deterministic_policy, derived_contract_id, prepare_help_context, HelpContextInput,
};
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BmadHelpCoordinatorError {
    SupportPlaneOffline,
    Unauthorized,
    Conflict,
    Integrity,
    Recovery,
}

pub(crate) struct BmadHelpCoordinator {
    ledger: MemoryDecisionLedger,
    active: Option<PreparedBmadHelpReview>,
}

impl fmt::Debug for BmadHelpCoordinator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BmadHelpCoordinator")
            .field("ledger", &"<redacted>")
            .field("active", &self.active.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

impl BmadHelpCoordinator {
    #[must_use]
    pub fn new() -> Self {
        Self {
            ledger: MemoryDecisionLedger::default(),
            active: None,
        }
    }

    pub fn invalidate(&mut self) {
        self.active = None;
    }

    #[must_use]
    pub const fn has_active_review(&self) -> bool {
        self.active.is_some()
    }

    #[cfg(test)]
    #[must_use]
    pub fn active_fixture_for_test(&self) -> &str {
        self.active
            .as_ref()
            .map_or("", |active| active.deterministic_fixture.as_str())
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

        self.invalidate();
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
        self.active = Some(PreparedBmadHelpReview {
            renderer_session_id: input.renderer_session.session_id().clone(),
            workspace_id: input.workspace_id,
            workspace_grant_epoch: input.workspace_grant_epoch,
            workspace_catalog_version,
            creation,
            compiled,
            manifest,
            invocation_binding,
            deterministic_fixture: policy.deterministic_fixture,
        });
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
