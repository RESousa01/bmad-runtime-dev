#![allow(clippy::expect_used, clippy::panic)]

//! Shared descriptor-to-Method execution bridge characterization (MTH-08).

use std::sync::Mutex;

use desktop_runtime::generated_contracts::{
    BmadActionPlanKind, BmadArtifactCompletionHookRunHook, BmadArtifactKind,
    BmadInvocationDeclaredAction, BmadInvocationDeclaredActionName, BmadInvocationMode,
};
use desktop_runtime::{
    sha256_bytes, AuthorityRef, BmadDomainRuntime, BmadMethodSkill, BmadResource, BmadResourceSet,
    BmadRuntimePort, BmadSkillDescriptor, BmadSkillDescriptorInput, BmadSkillExecutionContract,
    BmadSkillExecutionContractInput, BmadSkillExecutionError, BmadSkillExecutionRequest,
    BmadSkillExecutionStep, BmadSkillExecutionWorkflow, BmadSkillRecoverySemantics,
    BmadSkillSupportState, BmadSkillToolIntent, ContractId, CreateMethodSession,
    MethodAdvanceDisposition, MethodAdvanceReceipt, MethodAdvanceRequest,
    MethodArtifactExpectation, MethodArtifactProvenance, MethodExactBinding,
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
        workflow: BmadSkillExecutionWorkflow {
            initial_step_key: "draft".to_owned(),
            steps: vec![BmadSkillExecutionStep {
                step_key: "draft".to_owned(),
                next_step_key: None,
                source_resource_path: descriptor.source_path().to_owned(),
                source_resource_hash: descriptor.source_sha256(),
            }],
        },
        action_branches: vec![desktop_runtime::BmadSkillExecutionActionBranch {
            action: "create".to_owned(),
            initial_step_key: "draft".to_owned(),
            artifact_expectation_ids: vec![artifact_expectation().expectation_id],
        }],
        tool_intents: vec![
            BmadSkillToolIntent::FileRead,
            BmadSkillToolIntent::ExternalHandoff,
        ],
        artifact_expectations: binding.artifact_expectations.clone(),
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
        _: &MethodSessionScope,
        _: &ContractId,
        _: &MethodExactBinding,
        _: MethodAdvanceRequest,
    ) -> Result<MethodAdvanceReceipt, Self::Error> {
        Err(RepositoryError)
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
        Err(RepositoryError)
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
    assert_eq!(
        prepared.required_plan_kinds(),
        [BmadActionPlanKind::Read, BmadActionPlanKind::ArtifactPlan]
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
