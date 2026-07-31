//! Source-derived conformance tests for canonical `bmad-architecture` execution.

#![allow(clippy::expect_used, clippy::panic)]

use std::sync::Mutex;

use desktop_runtime::AuthorityRef;
use desktop_runtime::{
    build_problem,
    generated_contracts::{
        BmadActionPlanBudget, BmadActionPlanKind, BmadArtifactCompletionHookRunHook,
        BmadArtifactKind, BmadInvocationMode, BmadProblemFailureClass,
        BmadProblemRecoveryActionAction,
    },
    plan_source_steps, record_repair, record_retry, sha256_bytes, BmadArchitectureSkill,
    BmadCapabilityKey, BmadDomainRuntime, BmadFailureOperation, BmadFailureRequest,
    BmadMethodSkill, BmadModelCallUsage, BmadResourceSet, BmadRuntimePort, BmadSkillAuthorityRoute,
    BmadSkillExecutionContract, BmadSkillExecutionError, BmadSkillExecutionRequest,
    BmadSkillRecoverySemantics, BmadSkillSupportState, BmadSkillToolIntent, BmadSourceStep,
    ContractId, CreateMethodSession, MethodAdvanceDisposition, MethodAdvanceReceipt,
    MethodAdvanceRequest, MethodArtifactExpectation, MethodArtifactProvenance, MethodEvidenceClass,
    MethodExactBinding, MethodExecutionProfile, MethodExecutionProfileData, MethodInvocationModes,
    MethodModelBinding, MethodModelBindingData, MethodPersistenceEvent, MethodResourcePolicy,
    MethodSession, MethodSessionRepository, MethodSessionScope, MethodSessionService, MethodState,
    Sha256Digest, UnixMillis,
};

const SKILL_ROOT: &str = "src/bmm-skills/3-solutioning/bmad-architecture";

fn contract_id(value: &str) -> ContractId {
    ContractId::new(value).expect("canonical test identifier")
}

fn digest(value: &str) -> Sha256Digest {
    Sha256Digest::parse(value).expect("canonical pinned digest")
}

fn test_digest(label: &str) -> Sha256Digest {
    sha256_bytes(label.as_bytes())
}

fn architecture_execution_profile() -> MethodExecutionProfile {
    MethodExecutionProfile::from_source(
        MethodExecutionProfileData {
            entrypoint_kind: "direct".to_owned(),
            invocation_modes: MethodInvocationModes {
                interactive: true,
                headless: true,
                actions: Vec::new(),
            },
            required_runtimes: Vec::new(),
            resource_policy: MethodResourcePolicy {
                entrypoint_timing: "invocation_start".to_owned(),
                resource_timing: "all_declared_at_start".to_owned(),
                declared_resource_paths: BmadArchitectureSkill
                    .descriptor()
                    .declared_resources()
                    .iter()
                    .map(|resource| resource.path().to_owned())
                    .collect(),
            },
            declared_tool_intents: vec![
                "file_read".to_owned(),
                "file_write".to_owned(),
                "process".to_owned(),
                "web".to_owned(),
                "subagent".to_owned(),
                "external_handoff".to_owned(),
            ],
            state_hints: vec!["memlog".to_owned(), "artifact_workspace".to_owned()],
            completion_evidence: vec![
                "artifact".to_owned(),
                "event".to_owned(),
                "status_evidence".to_owned(),
            ],
            customization_profile: "method_skill_toml".to_owned(),
            validation_profile: "MethodOfficialSkillV6".to_owned(),
        },
        digest("sha256:1e1652a19e34c8306315e5f47a5b791582d472604217f65780463ed8d768d64f"),
    )
    .expect("source-derived architecture execution profile")
}

fn architecture_artifact_expectation() -> MethodArtifactExpectation {
    MethodArtifactExpectation::from_source(MethodArtifactExpectation {
        expectation_kind: "method_artifact".to_owned(),
        expectation_id: contract_id("expectation_01J00000000000000000000012"),
        artifact_kind: "document".to_owned(),
        required: true,
        storage_scope: "app_local".to_owned(),
        expected_media_type: "text/markdown".to_owned(),
        expected_content_schema_hash: None,
        source_output_hint: Some(
            "{doc_workspace}/ARCHITECTURE-SPINE.md or \
             {doc_workspace}/validation-report.{md,html}"
                .to_owned(),
        ),
        completion_evidence_class: MethodEvidenceClass::Authoritative,
        expectation_hash: test_digest("bmad-architecture artifact expectation"),
    })
    .expect("source-derived architecture artifact expectation")
}

fn architecture_request(mode: BmadInvocationMode) -> BmadSkillExecutionRequest {
    let descriptor = BmadArchitectureSkill.descriptor();
    let resources =
        BmadResourceSet::new(descriptor.declared_resources().to_vec()).expect("closed resources");
    let execution_profile = architecture_execution_profile();
    let egress_profile_hash = test_digest("test architecture egress profile");
    let model_binding = MethodModelBinding::from_source(
        MethodModelBindingData {
            binding_kind: "method_model".to_owned(),
            provider_id: "test-provider".to_owned(),
            model_id: "test-model".to_owned(),
            deployment_id: "test-deployment".to_owned(),
            model_profile_hash: test_digest("test architecture model profile"),
            model_capability_hash: test_digest("test architecture model capability"),
            context_window_profile_hash: test_digest("test architecture context window"),
            egress_profile_hash,
            request_schema_hash: test_digest("test architecture request schema"),
            response_schema_hash: test_digest("test architecture response schema"),
        },
        test_digest("test architecture model binding"),
    )
    .expect("valid inert model binding");
    let resolved_config_graph_hash = test_digest("test architecture config graph");
    let resolved_config_hash = test_digest("test architecture config resolution");
    let resolved_customization_hash = test_digest("test architecture customization");
    let exact_binding = MethodExactBinding {
        capability_key: BmadCapabilityKey {
            package_version_id: contract_id(
                "pkgver_8B33C55A4D67D0B258FEDBB75D1CB09DBC7F711BC9BDC794D8B052B31FCE6D86",
            ),
            module_code: "bmm".to_owned(),
            skill_name: "bmad-architecture".to_owned(),
            normalized_action: None,
        },
        invocation_mode: mode,
        package_descriptor_hash: digest(
            "sha256:036d3e0fdef725b894c2a5a95c4560d3250dd54b14640fecb26aa88c459c0c31",
        ),
        package_source_hash: digest(
            "sha256:8b33c55a4d67d0b258fedbb75d1cb09dbc7f711bc9bdc794d8b052b31fce6d86",
        ),
        instruction_projection_hash: digest(
            "sha256:fc4baf320e3e3e16149680630f7efc28e757b0780d7b56b6ae935f87a43b6b46",
        ),
        capability_catalog_hash: digest(
            "sha256:d6a3198652479968f7b7f6e2eacb16af2611143631200c2a5dfc3f492400de00",
        ),
        agent_roster_hash: None,
        agent_binding_hash: None,
        agent_binding: None,
        distribution_profile: "sapphirus_package".to_owned(),
        install_profile: "SapphirusManagedV1".to_owned(),
        entrypoint_kind: "direct".to_owned(),
        execution_profile_hash: execution_profile.profile_hash,
        execution_profile,
        validation_profile: "MethodOfficialSkillV6".to_owned(),
        validation_profile_hash: digest(
            "sha256:3e851de810792a1caacb2f4e4c1ac2134359f5b9da8c3870c6c37b967b03efba",
        ),
        config_graph_hash: resolved_config_graph_hash,
        config_resolution_hash: resolved_config_hash,
        customization_hash: resolved_customization_hash,
        resource_set_hash: BmadSkillExecutionContract::resource_set_hash(&resources)
            .expect("closed architecture resource-set hash"),
        model_binding_hash: model_binding.binding_hash,
        model_binding,
        method_schema_hash: digest(
            "sha256:dda02f453dcf4edbb3fc66e0ad3a7c59d7087f5b44b526e40c435456c8c248c2",
        ),
        egress_profile_hash,
        artifact_expectations: vec![architecture_artifact_expectation()],
    };
    BmadSkillExecutionRequest {
        mode,
        action: None,
        exact_binding,
        resources,
        resolved_config_graph_hash,
        resolved_config_hash,
        resolved_customization_hash,
    }
}

#[derive(Debug)]
struct RepositoryError;

#[derive(Default)]
struct ArchitectureRepository {
    sessions: Mutex<Vec<MethodSession>>,
    events: Mutex<Vec<MethodPersistenceEvent>>,
}

impl MethodSessionRepository for ArchitectureRepository {
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
        event: MethodPersistenceEvent,
    ) -> Result<(), Self::Error> {
        let mut sessions = self.sessions.lock().map_err(|_| RepositoryError)?;
        let current = sessions
            .iter_mut()
            .find(|current| current.session_id() == session.session_id())
            .ok_or(RepositoryError)?;
        *current = session.clone();
        self.events.lock().map_err(|_| RepositoryError)?.push(event);
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

fn create_architecture_session() -> CreateMethodSession {
    CreateMethodSession {
        session_id: contract_id("session_01J00000000000000000000012"),
        owner_scope_ref: contract_id("ownerscope_01J00000000000000000000012"),
        project_id: contract_id("project_01J00000000000000000000012"),
        run_id: contract_id("run_01J00000000000000000000012"),
        authority_ref: AuthorityRef {
            authority_kind: "desktop_local_store".to_owned(),
            authority_id: contract_id("authority_01J00000000000000000000012"),
            installation_id: contract_id("install_01J00000000000000000000012"),
            local_store_id: contract_id("store_01J00000000000000000000012"),
            authority_epoch: 1,
        },
        created_at: UnixMillis(1_000),
    }
}

// Break caught: omitting any recursively owned file makes the runtime's
// immutable source boundary incomplete before preparation starts.
#[test]
fn architecture_descriptor_closes_every_pinned_resource_and_declared_shape() {
    let descriptor = BmadArchitectureSkill.descriptor();

    assert_eq!(descriptor.canonical_id(), "bmm:bmad-architecture");
    assert_eq!(descriptor.source_path(), format!("{SKILL_ROOT}/SKILL.md"));
    assert!(descriptor.declared_actions().is_empty());
    assert_eq!(
        descriptor.supported_modes(),
        [
            BmadInvocationMode::Interactive,
            BmadInvocationMode::Headless
        ]
    );
    assert_eq!(descriptor.predecessor(), None);
    assert_eq!(descriptor.follower(), Some("bmm:bmad-spec"));
    assert_eq!(descriptor.output_locations(), ["planning_artifacts"]);
    assert_eq!(descriptor.artifact_kinds(), [BmadArtifactKind::Document]);
    assert_eq!(
        descriptor.completion_hooks(),
        [BmadArtifactCompletionHookRunHook::PersistArtifacts]
    );
    assert_eq!(descriptor.support_state(), BmadSkillSupportState::Adapted);
    assert_eq!(
        descriptor
            .declared_resources()
            .iter()
            .map(desktop_runtime::BmadResource::path)
            .collect::<Vec<_>>(),
        [
            "src/bmm-skills/3-solutioning/bmad-architecture/SKILL.md",
            "src/bmm-skills/3-solutioning/bmad-architecture/customize.toml",
            "src/bmm-skills/3-solutioning/bmad-architecture/assets/spine-template.md",
            "src/bmm-skills/3-solutioning/bmad-architecture/references/headless.md",
            "src/bmm-skills/3-solutioning/bmad-architecture/references/reviewer-gate.md",
            "src/bmm-skills/3-solutioning/bmad-architecture/scripts/lint_spine.py",
            "src/bmm-skills/3-solutioning/bmad-architecture/scripts/tests/test_lint_spine.py",
        ]
    );
    for resource in descriptor.declared_resources() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/bmad-foundation/source/method/6.10.0")
            .join(resource.path());
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()));
        assert_eq!(
            resource.content_hash(),
            sha256_bytes(&bytes),
            "{}",
            resource.path()
        );
    }
}

// Break caught: a visible descriptor without explicit generic execution
// opt-in remains normalized-only and cannot enter the governed runtime.
#[test]
fn architecture_exposes_an_explicit_generic_execution_contract() {
    let contract = BmadArchitectureSkill
        .execution_contract()
        .expect("canonical architecture must opt into generic Method execution");
    assert_eq!(contract.package_name(), "bmad-method");
    assert_eq!(contract.package_version(), "6.10.0");
}

// Break caught: one mode preparing while the other is merely documented
// violates the source's interactive/headless equivalence and recovery rules.
#[test]
fn architecture_prepares_both_modes_with_typed_plans_and_durable_recovery() {
    let runtime = BmadDomainRuntime;
    for mode in [
        BmadInvocationMode::Interactive,
        BmadInvocationMode::Headless,
    ] {
        let prepared = runtime
            .prepare_skill_execution(&BmadArchitectureSkill, architecture_request(mode))
            .expect("exact architecture binding prepares");
        assert_eq!(
            prepared.descriptor().canonical_id(),
            "bmm:bmad-architecture"
        );
        assert_eq!(prepared.mode(), mode);
        assert_eq!(prepared.action(), None);
        assert_eq!(prepared.resources().resources().len(), 7);
        assert_eq!(
            prepared.tool_intents(),
            [
                BmadSkillToolIntent::FileRead,
                BmadSkillToolIntent::FileWrite,
                BmadSkillToolIntent::Process,
                BmadSkillToolIntent::Web,
                BmadSkillToolIntent::Subagent,
                BmadSkillToolIntent::ExternalHandoff,
            ]
        );
        assert_eq!(
            prepared
                .tool_intents()
                .iter()
                .map(|intent| intent.authority_route())
                .collect::<Vec<_>>(),
            [
                BmadSkillAuthorityRoute::WorkspaceRead,
                BmadSkillAuthorityRoute::WorkspaceWrite,
                BmadSkillAuthorityRoute::Process,
                BmadSkillAuthorityRoute::Network,
                BmadSkillAuthorityRoute::Subagent,
                BmadSkillAuthorityRoute::ExternalHandoff,
            ]
        );
        assert_eq!(
            prepared.required_plan_kinds(),
            [
                BmadActionPlanKind::Read,
                BmadActionPlanKind::WriteProposal,
                BmadActionPlanKind::CommandProposal,
                BmadActionPlanKind::ModelCall,
            ]
        );
        assert_eq!(
            prepared.artifact_expectations(),
            [architecture_artifact_expectation()]
        );
        assert_eq!(
            prepared.recovery(),
            BmadSkillRecoverySemantics::MethodCheckpoint
        );
        assert!(prepared
            .step_table()
            .table_hash()
            .to_string()
            .starts_with("sha256:"));
    }
}

// Break caught: merely exposing an execution contract leaves the visible
// desktop capability outside the durable Method lifecycle. Canonical dispatch
// must compile and persist through the established service before any model or
// effect-capable port is reachable.
#[test]
fn architecture_dispatch_enters_the_existing_durable_method_lifecycle() {
    let repository = ArchitectureRepository::default();
    let service = MethodSessionService::new(&repository);
    let dispatched = BmadDomainRuntime
        .dispatch_skill_execution(
            &service,
            &BmadArchitectureSkill,
            architecture_request(BmadInvocationMode::Interactive),
            create_architecture_session(),
        )
        .expect("canonical architecture dispatch persists");

    assert_eq!(
        dispatched.prepared().descriptor().canonical_id(),
        "bmm:bmad-architecture"
    );
    assert_eq!(
        dispatched.session().state(),
        MethodState::CapabilityBound
    );
    assert_eq!(
        dispatched.session().current_binding(),
        Ok(dispatched.prepared().exact_binding())
    );
    assert_eq!(
        repository
            .sessions
            .lock()
            .expect("repository lock")
            .as_slice(),
        [dispatched.session().clone()]
    );
    assert_eq!(
        repository.events.lock().expect("event lock").as_slice(),
        [MethodPersistenceEvent::CapabilityBound]
    );

    let scope = dispatched.session().scope().clone();
    let session_id = dispatched.session().session_id();
    let mut upgraded_binding = dispatched.prepared().exact_binding().clone();
    upgraded_binding.customization_hash = test_digest("upgraded architecture customization");
    let rebound = service
        .rebind_invocation(
            &scope,
            &session_id,
            dispatched.session().version(),
            upgraded_binding.clone(),
            dispatched.prepared().step_table().clone(),
        )
        .expect("stale exact binding upgrades through the durable service");
    assert_eq!(rebound.state(), MethodState::CapabilityBound);
    assert_eq!(rebound.current_binding(), Ok(&upgraded_binding));

    let review_required = service
        .request_context_review(&scope, &session_id, rebound.version())
        .expect("upgraded binding requires a fresh context review");
    assert_eq!(review_required.state(), MethodState::ContextReviewRequired);

    let cancelled = service
        .cancel(&scope, &session_id, review_required.version())
        .expect("architecture run cancels durably");
    assert_eq!(cancelled.state(), MethodState::Cancelled);
    let resumed = service
        .resume(&scope, &session_id)
        .expect("restart reloads the authoritative cancelled session");
    assert_eq!(resumed, cancelled);
    assert_eq!(
        repository.events.lock().expect("event lock").as_slice(),
        [
            MethodPersistenceEvent::CapabilityBound,
            MethodPersistenceEvent::CapabilityRebound,
            MethodPersistenceEvent::ContextReviewRequested,
            MethodPersistenceEvent::Cancelled,
        ]
    );
}

// Break caught: documenting retry/repair without exercising the same bounded
// runtime mechanisms used by a dispatched Method plan leaves recovery prose
// disconnected from executable authority.
#[test]
fn architecture_failure_retry_and_repair_use_governed_runtime_budgets() {
    let plans = plan_source_steps(
        "capability_01J00000000000000000000012",
        "session_01J00000000000000000000012",
        &BmadActionPlanBudget {
            max_model_calls: 1,
            max_repairs: 1,
            max_retries: 1,
            max_tokens: 1_000,
        },
        &[BmadSourceStep {
            plan_id: "plan_01J00000000000000000000012".to_owned(),
            source_id: "source_01J00000000000000000000012".to_owned(),
            step_index: 0,
            text: "model:facilitate_architecture".to_owned(),
        }],
    )
    .expect("architecture model step compiles to a typed plan");
    let plan = plans.first().expect("one architecture plan");
    let mut usage = BmadModelCallUsage::default();
    record_retry(plan, &mut usage).expect("first retry is inside the sealed budget");
    record_repair(plan, &mut usage).expect("first repair is inside the sealed budget");
    assert_eq!((usage.retries, usage.repairs), (1, 1));

    let operation = BmadFailureOperation {
        session_id: "session_01J00000000000000000000012".to_owned(),
        step_id: "facilitate_architecture".to_owned(),
        input_hash: test_digest("architecture failure input"),
    };
    let failure = |failure_class| BmadFailureRequest {
        session_id: "session_01J00000000000000000000012".to_owned(),
        problem_id: "problem_01J00000000000000000000012".to_owned(),
        code: "architecture_step_failed".to_owned(),
        detail: None,
        failure_class,
        operation: operation.clone(),
        attempt: 1,
        max_attempts: 2,
        drafts_preserved: true,
        accepted_inputs_preserved: true,
        state_hash: test_digest("architecture durable failure state"),
        cancellation: None,
    };
    let retryable =
        build_problem(&failure(BmadProblemFailureClass::Tool)).expect("tool failure maps");
    assert!(retryable.retry.eligible);
    assert_eq!(
        retryable.recovery_actions[0].action,
        BmadProblemRecoveryActionAction::Retry
    );
    let repairable =
        build_problem(&failure(BmadProblemFailureClass::Validation)).expect("validation maps");
    assert!(!repairable.retry.eligible);
    assert_eq!(
        repairable.recovery_actions[0].action,
        BmadProblemRecoveryActionAction::Repair
    );
}

// Break caught: source prose inventing a callable action or a changed source
// member must fail before any effect-capable port is reached.
#[test]
fn architecture_preparation_rejects_undeclared_actions_and_source_drift() {
    let runtime = BmadDomainRuntime;
    let mut undeclared_action = architecture_request(BmadInvocationMode::Interactive);
    undeclared_action.action = Some("create".to_owned());
    assert!(matches!(
        runtime.prepare_skill_execution(&BmadArchitectureSkill, undeclared_action),
        Err(BmadSkillExecutionError::UnsupportedInvocation)
    ));

    let mut stale_source = architecture_request(BmadInvocationMode::Headless);
    stale_source.resources = BmadResourceSet::new(stale_source.resources.resources()[1..].to_vec())
        .expect("valid but incomplete resource set");
    assert!(matches!(
        runtime.prepare_skill_execution(&BmadArchitectureSkill, stale_source),
        Err(BmadSkillExecutionError::ResourceMismatch)
    ));
}
