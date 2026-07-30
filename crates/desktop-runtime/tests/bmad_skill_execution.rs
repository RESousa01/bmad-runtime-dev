#![allow(clippy::expect_used, clippy::panic)]

//! Shared descriptor-to-Method execution bridge characterization (MTH-08).

use std::sync::Mutex;

use desktop_runtime::generated_contracts::{
    BmadActionPlanKind, BmadArtifactCompletionHookRunHook, BmadArtifactKind,
    BmadInvocationDeclaredAction, BmadInvocationDeclaredActionName, BmadInvocationMode,
};
use desktop_runtime::{
    sha256_bytes, AuthorityRef, BmadDomainRuntime, BmadMethodSkill, BmadPrdSkill, BmadResource,
    BmadResourceSet, BmadRuntimePort, BmadSkillAuthorityRequirement, BmadSkillDescriptor,
    BmadSkillDescriptorInput, BmadSkillExecutionBindingInput, BmadSkillExecutionBranch,
    BmadSkillExecutionContract, BmadSkillExecutionContractInput, BmadSkillExecutionError,
    BmadSkillExecutionRequest, BmadSkillExecutionStep, BmadSkillExecutionWorkflow,
    BmadSkillRecoverySemantics, BmadSkillSupportState, BmadSkillToolIntent, ContractId,
    CreateMethodSession, MethodAdvanceDisposition, MethodAdvanceReceipt, MethodAdvanceRequest,
    MethodArtifactExpectation, MethodArtifactProvenance, MethodContextDecision, MethodExactBinding,
    MethodExecutionProfile, MethodExecutionProfileData, MethodInvocationModes, MethodModelBinding,
    MethodModelBindingData, MethodPersistenceEvent, MethodResourcePolicy, MethodSession,
    MethodSessionRepository, MethodSessionScope, MethodSessionService, MethodState, Sha256Digest,
    UnixMillis,
};
use desktop_runtime::{BmadCapabilityKey, MethodEvidenceClass};

fn id(value: &str) -> ContractId {
    ContractId::new(value).expect("test identifier")
}

fn digest(label: &str) -> Sha256Digest {
    sha256_bytes(label.as_bytes())
}

fn descriptor() -> BmadSkillDescriptor {
    descriptor_with_state(BmadSkillSupportState::Exact)
}

fn descriptor_with_state(support_state: BmadSkillSupportState) -> BmadSkillDescriptor {
    let action_name: BmadInvocationDeclaredActionName = "create".try_into().expect("action");
    BmadSkillDescriptor::new(BmadSkillDescriptorInput {
        canonical_id: "bmm:bmad-fake".to_owned(),
        source_path: "src/bmm-skills/bmad-fake/SKILL.md".to_owned(),
        source_sha256: digest("fake skill source"),
        declared_actions: vec![BmadInvocationDeclaredAction {
            declared_inputs: Vec::new(),
            name: action_name,
        }],
        supported_modes: vec![BmadInvocationMode::Interactive],
        predecessor: None,
        follower: None,
        output_locations: vec!["planning_artifacts".to_owned()],
        artifact_kinds: vec![BmadArtifactKind::Document],
        completion_hooks: vec![BmadArtifactCompletionHookRunHook::PersistArtifacts],
        declared_resources: vec![BmadResource::new(
            "src/bmm-skills/bmad-fake/SKILL.md",
            digest("fake skill source"),
        )
        .expect("source resource")],
        support_state,
    })
    .expect("descriptor")
}

fn execution_profile() -> MethodExecutionProfile {
    MethodExecutionProfile::from_source(
        MethodExecutionProfileData {
            entrypoint_kind: "step_jit".to_owned(),
            invocation_modes: MethodInvocationModes {
                interactive: true,
                headless: false,
                actions: vec!["create".to_owned()],
            },
            required_runtimes: Vec::new(),
            resource_policy: MethodResourcePolicy {
                entrypoint_timing: "invocation_start".to_owned(),
                resource_timing: "current_step_only".to_owned(),
                declared_resource_paths: vec!["src/bmm-skills/bmad-fake/SKILL.md".to_owned()],
            },
            declared_tool_intents: vec!["file_read".to_owned(), "external_handoff".to_owned()],
            state_hints: vec!["artifact_workspace".to_owned()],
            completion_evidence: vec!["artifact".to_owned()],
            customization_profile: "method_skill_toml".to_owned(),
            validation_profile: "MethodStepWorkflowV6".to_owned(),
        },
        digest("execution profile"),
    )
    .expect("execution profile")
}

fn artifact_expectation() -> MethodArtifactExpectation {
    MethodArtifactExpectation::from_source(MethodArtifactExpectation {
        expectation_kind: "method_artifact".to_owned(),
        expectation_id: id("expectation_01J00000000000000000000000"),
        artifact_kind: "document".to_owned(),
        required: true,
        storage_scope: "app_local".to_owned(),
        expected_media_type: "text/markdown".to_owned(),
        expected_content_schema_hash: None,
        source_output_hint: Some("planning_artifacts/prd.md".to_owned()),
        completion_evidence_class: MethodEvidenceClass::Authoritative,
        expectation_hash: digest("artifact expectation"),
    })
    .expect("artifact expectation")
}

fn exact_binding() -> MethodExactBinding {
    let execution_profile = execution_profile();
    let model_binding = MethodModelBinding::from_source(
        MethodModelBindingData {
            binding_kind: "method_model".to_owned(),
            provider_id: "test-provider".to_owned(),
            model_id: "test-model".to_owned(),
            deployment_id: "test-deployment".to_owned(),
            model_profile_hash: digest("model profile"),
            model_capability_hash: digest("model capability"),
            context_window_profile_hash: digest("context window"),
            egress_profile_hash: digest("egress"),
            request_schema_hash: digest("request schema"),
            response_schema_hash: digest("response schema"),
        },
        digest("model binding"),
    )
    .expect("model binding");
    MethodExactBinding {
        capability_key: BmadCapabilityKey {
            package_version_id: id("pkgver_01J00000000000000000000000"),
            module_code: "bmm".to_owned(),
            skill_name: "bmad-fake".to_owned(),
            normalized_action: Some("create".to_owned()),
        },
        invocation_mode: BmadInvocationMode::Interactive,
        package_descriptor_hash: digest("package descriptor"),
        package_source_hash: digest("package source"),
        instruction_projection_hash: digest("instructions"),
        capability_catalog_hash: digest("catalog"),
        agent_roster_hash: None,
        agent_binding_hash: None,
        agent_binding: None,
        distribution_profile: "sapphirus_package".to_owned(),
        install_profile: "SapphirusManagedV1".to_owned(),
        entrypoint_kind: "step_jit".to_owned(),
        execution_profile_hash: execution_profile.profile_hash,
        execution_profile,
        validation_profile: "MethodStepWorkflowV6".to_owned(),
        validation_profile_hash: digest("validation"),
        config_graph_hash: digest("config graph"),
        config_resolution_hash: digest("config resolution"),
        customization_hash: digest("customization"),
        resource_set_hash: digest("will be replaced by bridge"),
        model_binding_hash: model_binding.binding_hash,
        model_binding,
        method_schema_hash: digest("method schema"),
        egress_profile_hash: digest("egress"),
        artifact_expectations: vec![artifact_expectation()],
    }
}

fn contract() -> BmadSkillExecutionContract {
    contract_for_descriptor(&descriptor()).expect("explicit execution contract")
}

fn contract_for_descriptor(
    descriptor: &BmadSkillDescriptor,
) -> Result<BmadSkillExecutionContract, BmadSkillExecutionError> {
    let binding = exact_binding();
    BmadSkillExecutionContract::new(BmadSkillExecutionContractInput {
        descriptor: descriptor.clone(),
        package_name: "bmad-method".to_owned(),
        package_version: "6.10.0".to_owned(),
        package_version_id: binding.capability_key.package_version_id.clone(),
        package_descriptor_hash: binding.package_descriptor_hash,
        package_source_hash: binding.package_source_hash,
        instruction_projection_hash: binding.instruction_projection_hash,
        capability_catalog_hash: binding.capability_catalog_hash,
        distribution_profile: binding.distribution_profile.clone(),
        install_profile: binding.install_profile.clone(),
        execution_profile: binding.execution_profile.clone(),
        validation_profile_hash: binding.validation_profile_hash,
        method_schema_hash: binding.method_schema_hash,
        branches: vec![BmadSkillExecutionBranch {
            action: "create".to_owned(),
            mode: BmadInvocationMode::Interactive,
            workflow: BmadSkillExecutionWorkflow {
                initial_step_key: "draft".to_owned(),
                steps: vec![BmadSkillExecutionStep {
                    step_key: "draft".to_owned(),
                    next_step_key: None,
                    source_resource_path: descriptor.source_path().to_owned(),
                    source_resource_hash: descriptor.source_sha256(),
                }],
            },
            artifact_expectations: binding.artifact_expectations.clone(),
            required_status_fields: Vec::new(),
        }],
        tool_intents: vec![
            BmadSkillToolIntent::FileRead,
            BmadSkillToolIntent::ExternalHandoff,
        ],
        recovery: BmadSkillRecoverySemantics::MethodCheckpoint,
    })
}

fn request() -> BmadSkillExecutionRequest {
    let mut exact_binding = exact_binding();
    let resources =
        BmadResourceSet::new(descriptor().declared_resources().to_vec()).expect("sealed resources");
    exact_binding.resource_set_hash =
        BmadSkillExecutionContract::resource_set_hash(&resources).expect("resource set hash");
    BmadSkillExecutionRequest {
        mode: BmadInvocationMode::Interactive,
        action: Some("create".to_owned()),
        exact_binding,
        resources,
        resolved_config_graph_hash: digest("config graph"),
        resolved_config_hash: digest("config resolution"),
        resolved_customization_hash: digest("customization"),
    }
}

fn prepared_prd(action: &str) -> desktop_runtime::BmadPreparedSkillExecution {
    let binding = exact_binding();
    let request = BmadPrdSkill
        .execution_contract()
        .expect("canonical PRD contract")
        .execution_request(BmadSkillExecutionBindingInput {
            mode: BmadInvocationMode::Interactive,
            action: action.to_owned(),
            model_binding: binding.model_binding,
            resolved_config_graph_hash: binding.config_graph_hash,
            resolved_config_hash: binding.config_resolution_hash,
            resolved_customization_hash: binding.customization_hash,
            egress_profile_hash: binding.egress_profile_hash,
        })
        .expect("canonical PRD exact request");
    BmadDomainRuntime
        .prepare_skill_execution(&BmadPrdSkill, request)
        .expect("canonical PRD prepares")
}

fn review_decision(binding: &MethodExactBinding) -> MethodContextDecision {
    MethodContextDecision {
        decision_id: id("decision_01J00000000000000000000000"),
        manifest_hash: digest("manifest"),
        consent_hash: digest("consent"),
        context_digest: digest("context"),
        binding_hash: binding.binding_hash().expect("binding hash"),
        reviewed_at: UnixMillis(2_000),
    }
}

fn advance_request(
    session: &MethodSession,
    decision_id: ContractId,
    idempotency_key: &str,
) -> MethodAdvanceRequest {
    let invocation_id = id("invoke_01J00000000000000000000000");
    let session_authority_hash = session.session_authority_hash().expect("session authority");
    let d2_model_invocation_binding_hash = digest("model invocation");
    MethodAdvanceRequest {
        invocation_id: invocation_id.clone(),
        idempotency_key: idempotency_key.to_owned(),
        decision_id,
        decision_consumption_hash: digest("decision consumption"),
        model_request_id: id("modelreq_01J00000000000000000000000"),
        model_request_hash: digest("model request"),
        session_authority_hash,
        d2_model_invocation_binding_hash,
        model_bridge_binding_hash: session
            .model_bridge_binding_hash(&d2_model_invocation_binding_hash)
            .expect("model bridge binding"),
        expected_version: session.version(),
    }
}

struct DescriptorOnlySkill;

impl BmadMethodSkill for DescriptorOnlySkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        descriptor()
    }
}

struct FakeExecutionSkill;

impl BmadMethodSkill for FakeExecutionSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        descriptor()
    }

    fn execution_contract(&self) -> Result<BmadSkillExecutionContract, BmadSkillExecutionError> {
        Ok(contract())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("in-memory repository failure")]
struct RepositoryError;

#[derive(Default)]
struct Repository {
    sessions: Mutex<Vec<MethodSession>>,
}

impl Repository {
    fn sessions(&self) -> Vec<MethodSession> {
        self.sessions.lock().expect("session lock").clone()
    }
}

impl MethodSessionRepository for Repository {
    type Error = RepositoryError;

    fn create_method_session(&self, session: &MethodSession) -> Result<(), Self::Error> {
        self.sessions
            .lock()
            .map_err(|_| RepositoryError)?
            .push(session.clone());
        Ok(())
    }

    fn load_method_session(
        &self,
        scope: &MethodSessionScope,
        session_id: &ContractId,
    ) -> Result<Option<MethodSession>, Self::Error> {
        Ok(self
            .sessions
            .lock()
            .map_err(|_| RepositoryError)?
            .iter()
            .find(|session| session.scope() == *scope && session.session_id() == *session_id)
            .cloned())
    }

    fn begin_method_advance(
        &self,
        scope: &MethodSessionScope,
        session_id: &ContractId,
        observed_binding: &MethodExactBinding,
        request: MethodAdvanceRequest,
    ) -> Result<MethodAdvanceReceipt, Self::Error> {
        let mut sessions = self.sessions.lock().map_err(|_| RepositoryError)?;
        let session = sessions
            .iter_mut()
            .find(|session| session.scope() == *scope && session.session_id() == *session_id)
            .ok_or(RepositoryError)?;
        if session.current_binding().ok() != Some(observed_binding) {
            return Err(RepositoryError);
        }
        session.begin_advance(request).map_err(|_| RepositoryError)
    }

    fn persist_method_transition(
        &self,
        session: &MethodSession,
        _: u64,
        _: MethodPersistenceEvent,
    ) -> Result<(), Self::Error> {
        let mut sessions = self.sessions.lock().map_err(|_| RepositoryError)?;
        let current = sessions
            .iter_mut()
            .find(|current| current.session_id() == session.session_id())
            .ok_or(RepositoryError)?;
        *current = session.clone();
        Ok(())
    }

    fn validate_method_artifact_refs(
        &self,
        _: &MethodArtifactProvenance,
        _: &MethodExactBinding,
        _: MethodAdvanceDisposition,
        _: &[String],
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn create_session() -> CreateMethodSession {
    CreateMethodSession {
        session_id: id("session_01J00000000000000000000000"),
        owner_scope_ref: id("ownerscope_01J00000000000000000000000"),
        project_id: id("project_01J00000000000000000000000"),
        run_id: id("run_01J00000000000000000000000"),
        authority_ref: AuthorityRef {
            authority_kind: "desktop_local_store".to_owned(),
            authority_id: id("authority_01J00000000000000000000000"),
            installation_id: id("install_01J00000000000000000000000"),
            local_store_id: id("store_01J00000000000000000000000"),
            authority_epoch: 1,
        },
        created_at: UnixMillis(1_000),
    }
}

// Break caught: treating descriptor-only adapters as executable lets a pending
// adapter enter the durable Method lifecycle without an explicit, source-bound
// execution contract.
#[test]
fn bmad_skill_execution_rejects_the_default_descriptor_only_contract() {
    let runtime = BmadDomainRuntime;
    let error = runtime
        .prepare_skill_execution(&DescriptorOnlySkill, request())
        .expect_err("descriptor-only adapters remain unavailable");

    assert_eq!(error, BmadSkillExecutionError::ContractUnavailable);
}

// Break caught: a classified stand-in is still not the pinned production
// implementation and must never enter the durable Method lifecycle.
#[test]
fn bmad_skill_execution_rejects_shim_contracts() {
    let error = contract_for_descriptor(&descriptor_with_state(BmadSkillSupportState::Shim))
        .expect_err("shim adapters remain unavailable to execution");

    assert_eq!(error, BmadSkillExecutionError::ContractInvalid);
}

// Break caught: bypassing the existing Method service, or letting preparation
// perform persistence itself, would create a parallel session lifecycle.
#[test]
fn bmad_skill_execution_prepares_inert_data_for_the_existing_method_service() {
    let runtime = BmadDomainRuntime;
    assert_eq!(contract().package_name(), "bmad-method");
    assert_eq!(contract().package_version(), "6.10.0");
    let prepared = runtime
        .prepare_skill_execution(&FakeExecutionSkill, request())
        .expect("explicit contract prepares");

    assert_eq!(prepared.descriptor().canonical_id(), "bmm:bmad-fake");
    assert_eq!(prepared.mode(), BmadInvocationMode::Interactive);
    assert_eq!(prepared.action(), Some("create"));
    assert!(prepared
        .step_table()
        .table_hash()
        .to_string()
        .starts_with("sha256:"));
    assert_eq!(prepared.required_plan_kinds(), [BmadActionPlanKind::Read]);
    assert_eq!(
        prepared.authority_requirements(),
        [
            BmadSkillAuthorityRequirement::ActionPlan(BmadActionPlanKind::Read),
            BmadSkillAuthorityRequirement::ExternalHandoff,
        ]
    );
    assert_eq!(
        prepared.recovery(),
        BmadSkillRecoverySemantics::MethodCheckpoint
    );

    let repository = Repository::default();
    assert!(
        repository.sessions().is_empty(),
        "prepare has no persistence port"
    );
    let service = MethodSessionService::new(&repository);
    let created = service.create(create_session()).expect("create session");
    let bound = service
        .bind_invocation(
            &created.scope(),
            &created.session_id(),
            created.version(),
            prepared.exact_binding().clone(),
            prepared.step_table().clone(),
        )
        .expect("the established service owns the durable bind");

    assert_eq!(bound.state(), MethodState::CapabilityBound);
    assert_eq!(bound.current_binding(), Ok(prepared.exact_binding()));
    assert_eq!(repository.sessions(), [bound]);
}

// Break caught: accepting a stale resource closure or an undeclared action
// would let a contract run with inputs other than the descriptor it sealed.
#[test]
fn bmad_skill_execution_rejects_resource_and_action_mismatches_without_effects() {
    let runtime = BmadDomainRuntime;
    let mut stale_resource = request();
    stale_resource.resources = BmadResourceSet::new(vec![BmadResource::new(
        "src/bmm-skills/bmad-fake/SKILL.md",
        digest("different source bytes"),
    )
    .expect("stale resource")])
    .expect("sealed stale resource set");
    assert_eq!(
        runtime
            .prepare_skill_execution(&FakeExecutionSkill, stale_resource)
            .expect_err("resource drift is rejected"),
        BmadSkillExecutionError::ResourceMismatch
    );

    let mut wrong_action = request();
    wrong_action.action = Some("update".to_owned());
    assert_eq!(
        runtime
            .prepare_skill_execution(&FakeExecutionSkill, wrong_action)
            .expect_err("undeclared action is rejected"),
        BmadSkillExecutionError::UnsupportedInvocation
    );

    let mut stale_config = request();
    stale_config.resolved_config_hash = digest("different resolved config");
    assert_eq!(
        runtime
            .prepare_skill_execution(&FakeExecutionSkill, stale_config)
            .expect_err("resolved configuration drift is rejected"),
        BmadSkillExecutionError::BindingMismatch
    );
}

// Break caught: proving PRD branch data only through golden fixtures leaves
// cancellation, failure, retry, repair, restart, resume, and upgrade outside
// the executable Method lifecycle used by production dispatch.
#[test]
#[allow(clippy::too_many_lines)] // One scenario proves all recovery transitions share one persisted session.
fn canonical_prd_uses_the_existing_executable_method_recovery_lifecycle() {
    let create = prepared_prd("create");
    let update = prepared_prd("update");

    // Upgrade/rebind, restart, resume, and cancel all operate on the persisted
    // canonical PRD session rather than on an adapter-owned state machine.
    let repository = Repository::default();
    let service = MethodSessionService::new(&repository);
    let created = service
        .create(create_session())
        .expect("create PRD session");
    let bound = service
        .bind_invocation(
            &created.scope(),
            &created.session_id(),
            created.version(),
            create.exact_binding().clone(),
            create.step_table().clone(),
        )
        .expect("bind create branch");
    let upgraded = service
        .rebind_invocation(
            &bound.scope(),
            &bound.session_id(),
            bound.version(),
            update.exact_binding().clone(),
            update.step_table().clone(),
        )
        .expect("upgrade to update branch");
    assert_eq!(upgraded.state(), MethodState::CapabilityBound);
    let upgraded = service
        .request_context_review(
            &upgraded.scope(),
            &upgraded.session_id(),
            upgraded.version(),
        )
        .expect("review upgraded branch");
    let restarted: MethodSession =
        serde_json::from_slice(&serde_json::to_vec(&upgraded).expect("serialize checkpoint"))
            .expect("restart from durable checkpoint");
    assert_eq!(restarted, upgraded);
    assert_eq!(
        service
            .resume(&upgraded.scope(), &upgraded.session_id())
            .expect("resume persisted PRD"),
        upgraded
    );
    let cancelled = service
        .cancel(
            &upgraded.scope(),
            &upgraded.session_id(),
            upgraded.version(),
        )
        .expect("cancel PRD");
    assert_eq!(cancelled.state(), MethodState::Cancelled);

    // A failed/incomplete run is a persisted terminal transition.
    let failed_repository = Repository::default();
    let failed_service = MethodSessionService::new(&failed_repository);
    let failed_created = failed_service
        .create(create_session())
        .expect("create failure session");
    let failed_bound = failed_service
        .bind_invocation(
            &failed_created.scope(),
            &failed_created.session_id(),
            failed_created.version(),
            create.exact_binding().clone(),
            create.step_table().clone(),
        )
        .expect("bind failure session");
    let failed_review = failed_service
        .request_context_review(
            &failed_bound.scope(),
            &failed_bound.session_id(),
            failed_bound.version(),
        )
        .expect("request failed-run review");
    let failed_decision = review_decision(create.exact_binding());
    let failed_ready = failed_service
        .record_context_review(
            &failed_review.scope(),
            &failed_review.session_id(),
            failed_review.version(),
            failed_decision.clone(),
        )
        .expect("accept failed-run review");
    let failed_request = advance_request(&failed_ready, failed_decision.decision_id, "prd-failure");
    let (failed_advancing, _) = failed_service
        .begin_advance(
            &failed_ready.scope(),
            &failed_ready.session_id(),
            create.exact_binding(),
            failed_request,
        )
        .expect("begin failed run");
    let incomplete = failed_service
        .record_incomplete(
            &failed_advancing.scope(),
            &failed_advancing.session_id(),
            failed_advancing.version(),
        )
        .expect("persist incomplete failure");
    assert_eq!(incomplete.state(), MethodState::Incomplete);

    // A malformed advance is repairable before consumption, and retrying the
    // accepted idempotency key returns the exact authoritative receipt.
    let retry_repository = Repository::default();
    let retry_service = MethodSessionService::new(&retry_repository);
    let retry_created = retry_service
        .create(create_session())
        .expect("create retry session");
    let retry_bound = retry_service
        .bind_invocation(
            &retry_created.scope(),
            &retry_created.session_id(),
            retry_created.version(),
            create.exact_binding().clone(),
            create.step_table().clone(),
        )
        .expect("bind retry session");
    let review_required = retry_service
        .request_context_review(
            &retry_bound.scope(),
            &retry_bound.session_id(),
            retry_bound.version(),
        )
        .expect("request PRD review");
    let decision = review_decision(create.exact_binding());
    let ready = retry_service
        .record_context_review(
            &review_required.scope(),
            &review_required.session_id(),
            review_required.version(),
            decision.clone(),
        )
        .expect("accept PRD review");
    let mut malformed = advance_request(
        &ready,
        id("decision_01J00000000000000000000099"),
        "prd-retry",
    );
    assert!(retry_service
        .begin_advance(
            &ready.scope(),
            &ready.session_id(),
            create.exact_binding(),
            malformed.clone(),
        )
        .is_err());
    malformed.decision_id = decision.decision_id;
    let (advancing, receipt) = retry_service
        .begin_advance(
            &ready.scope(),
            &ready.session_id(),
            create.exact_binding(),
            malformed.clone(),
        )
        .expect("repair malformed request");
    assert_eq!(advancing.state(), MethodState::Advancing);
    let (_, retried) = retry_service
        .begin_advance(
            &ready.scope(),
            &ready.session_id(),
            create.exact_binding(),
            malformed,
        )
        .expect("idempotent retry");
    assert_eq!(retried, receipt);
}
