//! Source-derived conformance for the canonical `bmad-review` capability.

#![allow(clippy::expect_used, clippy::panic)]

use std::sync::Mutex;

use desktop_runtime::{
    canonical_hash,
    generated_contracts::{
        BmadActionPlanBudget, BmadActionPlanKind, BmadArtifactCompletionHookRunHook,
        BmadArtifactKind, BmadInvocationMode,
    },
    plan_source_steps, record_repair, record_retry, sha256_bytes, AuthorityRef,
    BmadDomainRuntime, BmadMethodSkill, BmadModelCallUsage, BmadReviewSkill,
    BmadRuntimePort, BmadSkillHostBindingInput, BmadSkillRecoverySemantics,
    BmadSkillToolIntent, BmadSourceStep, ContractId, CreateMethodSession,
    MethodArtifactProvenance, MethodExactBinding, MethodModelBinding, MethodModelBindingData,
    MethodPersistenceEvent, MethodSession, MethodSessionRepository, MethodSessionScope,
    MethodSessionService, MethodState, Sha256Digest, UnixMillis,
};

const SKILL_ROOT: &str = "src/core-skills/bmad-review";

fn id(value: &str) -> ContractId {
    ContractId::new(value).expect("canonical test identifier")
}

fn digest(label: &str) -> Sha256Digest {
    sha256_bytes(label.as_bytes())
}

fn model_binding() -> MethodModelBinding {
    let data = MethodModelBindingData {
        binding_kind: "method_model".to_owned(),
        provider_id: "test-provider".to_owned(),
        model_id: "test-model".to_owned(),
        deployment_id: "test-deployment".to_owned(),
        model_profile_hash: digest("model profile"),
        model_capability_hash: digest("model capability"),
        context_window_profile_hash: digest("context window"),
        egress_profile_hash: digest("egress profile"),
        request_schema_hash: digest("request schema"),
        response_schema_hash: digest("review response schema"),
    };
    let binding_hash =
        canonical_hash("bmad-method-model-binding", 1, &data).expect("model binding hash");
    MethodModelBinding::from_source(data, binding_hash).expect("valid model binding")
}

fn execution_host_input(mode: BmadInvocationMode) -> BmadSkillHostBindingInput {
    let model_binding = model_binding();
    BmadSkillHostBindingInput {
        mode,
        action: None,
        resolved_config_graph_hash: digest("review config graph"),
        resolved_config_hash: digest("review config"),
        resolved_customization_hash: digest("review customization"),
        egress_profile_hash: model_binding.data.egress_profile_hash,
        model_binding,
    }
}

#[derive(Debug)]
struct RepositoryError;

impl std::fmt::Display for RepositoryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("repository error")
    }
}

impl std::error::Error for RepositoryError {}

#[derive(Default)]
struct Repository {
    sessions: Mutex<Vec<MethodSession>>,
    transitions: Mutex<Vec<(MethodPersistenceEvent, MethodState)>>,
}

impl MethodSessionRepository for &Repository {
    type Error = RepositoryError;

    fn create_method_session(&self, session: &MethodSession) -> Result<(), Self::Error> {
        self.sessions.lock().expect("sessions").push(session.clone());
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
            .expect("sessions")
            .iter()
            .find(|session| session.scope() == *scope && session.session_id() == *session_id)
            .cloned())
    }

    fn persist_method_transition(
        &self,
        session: &MethodSession,
        expected_previous_version: u64,
        event: MethodPersistenceEvent,
    ) -> Result<(), Self::Error> {
        let mut sessions = self.sessions.lock().expect("sessions");
        let current = sessions
            .iter_mut()
            .find(|candidate| candidate.session_id() == session.session_id())
            .ok_or(RepositoryError)?;
        if current.version() != expected_previous_version {
            return Err(RepositoryError);
        }
        *current = session.clone();
        self.transitions
            .lock()
            .expect("transitions")
            .push((event, session.state()));
        Ok(())
    }

    fn begin_method_advance(
        &self,
        scope: &MethodSessionScope,
        session_id: &ContractId,
        observed_binding: &MethodExactBinding,
        request: desktop_runtime::MethodAdvanceRequest,
    ) -> Result<desktop_runtime::MethodAdvanceReceipt, Self::Error> {
        let mut sessions = self.sessions.lock().expect("sessions");
        let current = sessions
            .iter_mut()
            .find(|candidate| {
                candidate.scope() == *scope && candidate.session_id() == *session_id
            })
            .ok_or(RepositoryError)?;
        if current.current_binding().map_err(|_| RepositoryError)? != observed_binding {
            return Err(RepositoryError);
        }
        let receipt = current.begin_advance(request).map_err(|_| RepositoryError)?;
        Ok(receipt)
    }

    fn validate_method_artifact_refs(
        &self,
        _: &MethodArtifactProvenance,
        _: &MethodExactBinding,
        _: desktop_runtime::MethodAdvanceDisposition,
        _: &[String],
    ) -> Result<(), Self::Error> {
        Err(RepositoryError)
    }
}

fn create_session(mode: BmadInvocationMode) -> CreateMethodSession {
    create_session_named(mode, "primary")
}

fn create_session_named(mode: BmadInvocationMode, case_name: &str) -> CreateMethodSession {
    let suffix = match mode {
        BmadInvocationMode::Interactive => "interactive",
        BmadInvocationMode::Headless => "headless",
    };
    CreateMethodSession {
        session_id: id(&format!("session_review_{suffix}_{case_name}")),
        owner_scope_ref: id("ownerscope_review"),
        project_id: id("project_review"),
        run_id: id(&format!("run_review_{suffix}_{case_name}")),
        authority_ref: AuthorityRef {
            authority_kind: "desktop_local_store".to_owned(),
            authority_id: id("authority_review"),
            installation_id: id("install_review"),
            local_store_id: id("store_review"),
            authority_epoch: 1,
        },
        created_at: UnixMillis(1_000),
    }
}

fn context_decision(
    binding: &MethodExactBinding,
    case_name: &str,
) -> desktop_runtime::MethodContextDecision {
    desktop_runtime::MethodContextDecision {
        decision_id: id(&format!("decision_review_{case_name}")),
        manifest_hash: digest("review manifest"),
        consent_hash: digest("review consent"),
        context_digest: digest("review context"),
        binding_hash: binding.binding_hash().expect("binding hash"),
        reviewed_at: UnixMillis(2_000),
    }
}

fn advance_request(
    session: &MethodSession,
    decision_id: ContractId,
    case_name: &str,
) -> desktop_runtime::MethodAdvanceRequest {
    let invocation_id = id(&format!("invoke_review_{case_name}"));
    let session_authority_hash = session
        .session_authority_hash()
        .expect("session authority");
    let d2_model_invocation_binding_hash = digest(&format!("d2 review {case_name}"));
    let model_bridge_binding_hash = session
        .model_bridge_binding_hash(&d2_model_invocation_binding_hash)
        .expect("bridge binding");
    desktop_runtime::MethodAdvanceRequest {
        model_request_id: id(&format!("modelreq_review_{case_name}")),
        decision_consumption_hash: digest(&format!("consumption {case_name}")),
        model_request_hash: digest(&format!("request {case_name}")),
        session_authority_hash,
        d2_model_invocation_binding_hash,
        model_bridge_binding_hash,
        invocation_id,
        idempotency_key: format!("review-{case_name}"),
        decision_id,
        expected_version: session.version(),
    }
}

#[test]
fn bmad_review_descriptor_closes_every_pinned_member_and_source_contract_field() {
    let descriptor = BmadReviewSkill.descriptor();
    assert_eq!(descriptor.canonical_id(), "core:bmad-review");
    assert_eq!(descriptor.source_path(), format!("{SKILL_ROOT}/SKILL.md"));
    assert!(descriptor.declared_actions().is_empty());
    assert_eq!(
        descriptor.supported_modes(),
        [BmadInvocationMode::Interactive, BmadInvocationMode::Headless]
    );
    assert_eq!(descriptor.output_locations(), ["report_path"]);
    assert_eq!(descriptor.artifact_kinds(), [BmadArtifactKind::Report]);
    assert_eq!(
        descriptor.completion_hooks(),
        [BmadArtifactCompletionHookRunHook::PersistArtifacts]
    );
    assert_eq!(descriptor.declared_resources().len(), 11);

    for resource in descriptor.declared_resources() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/bmad-foundation/source/method/6.10.0")
            .join(resource.path());
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()));
        assert_eq!(
            resource.content_hash(),
            sha256_bytes(&bytes),
            "{} must keep its pinned identity",
            resource.path()
        );
    }
}

#[test]
fn bmad_review_contract_preserves_source_branches_and_distinct_authority_intents() {
    let contract = BmadReviewSkill
        .execution_contract()
        .expect("canonical review opts into generic execution");
    let request = contract
        .bind_host_request(execution_host_input(BmadInvocationMode::Interactive))
        .expect("host facts bind without a handwritten exact binding");
    let prepared = BmadDomainRuntime
        .prepare_skill_execution(&BmadReviewSkill, request)
        .expect("review prepares through the shared compiler");

    assert_eq!(
        prepared.tool_intents(),
        [
            BmadSkillToolIntent::FileRead,
            BmadSkillToolIntent::FileWrite,
            BmadSkillToolIntent::Process,
            BmadSkillToolIntent::Network,
            BmadSkillToolIntent::Browser,
            BmadSkillToolIntent::Subagent,
            BmadSkillToolIntent::ExternalHandoff,
        ]
    );
    assert_eq!(
        prepared.required_plan_kinds(),
        [
            BmadActionPlanKind::Read,
            BmadActionPlanKind::WriteProposal,
            BmadActionPlanKind::CommandProposal,
            BmadActionPlanKind::Read,
            BmadActionPlanKind::Read,
            BmadActionPlanKind::ModelCall,
            BmadActionPlanKind::ArtifactPlan,
        ]
    );
    assert_eq!(
        prepared.step_table().ordered_keys(),
        [
            "resolve_customization",
            "load_content",
            "select_lenses",
            "announce_plan",
            "run_independent_lenses",
            "run_dependent_lenses",
            "assemble_present",
        ]
    );
    assert_eq!(
        prepared.recovery(),
        BmadSkillRecoverySemantics::MethodCheckpoint
    );
}

#[test]
fn both_modes_enter_the_existing_durable_method_session_service_and_resume_exactly() {
    for mode in [
        BmadInvocationMode::Interactive,
        BmadInvocationMode::Headless,
    ] {
        let contract = BmadReviewSkill.execution_contract().expect("review contract");
        let request = contract
            .bind_host_request(execution_host_input(mode))
            .expect("host binding");
        let repository = Repository::default();
        let service = MethodSessionService::new(&repository);
        let dispatched = BmadDomainRuntime
            .dispatch_skill_execution(
                &service,
                &BmadReviewSkill,
                request,
                create_session(mode),
            )
            .expect("generic dispatch creates and binds through Method service");
        let bound = dispatched.session();
        assert_eq!(bound.state(), MethodState::CapabilityBound);
        assert_eq!(
            repository.transitions.lock().expect("transitions").as_slice(),
            &[(MethodPersistenceEvent::CapabilityBound, MethodState::CapabilityBound)],
            "dispatch must persist the bound state before review"
        );
        let review_required = service
            .request_context_review(&bound.scope(), &bound.session_id(), bound.version())
            .expect("review transition");
        assert_eq!(review_required.state(), MethodState::ContextReviewRequired);
        let resumed = service
            .resume(&review_required.scope(), &review_required.session_id())
            .expect("restart resumes authoritative state");
        assert_eq!(resumed, review_required);
    }
}

#[test]
fn review_recovery_guards_execute_cancel_fail_retry_repair_resume_and_stale_upgrade() {
    let contract = BmadReviewSkill.execution_contract().expect("review contract");

    let repository = Repository::default();
    let service = MethodSessionService::new(&repository);
    let cancel_request = contract
        .bind_host_request(execution_host_input(BmadInvocationMode::Interactive))
        .expect("cancel binding");
    let cancelled_dispatch = BmadDomainRuntime
        .dispatch_skill_execution(
            &service,
            &BmadReviewSkill,
            cancel_request,
            create_session_named(BmadInvocationMode::Interactive, "cancel"),
        )
        .expect("cancel dispatch");
    let cancelled = service
        .cancel(
            &cancelled_dispatch.session().scope(),
            &cancelled_dispatch.session().session_id(),
            cancelled_dispatch.session().version(),
        )
        .expect("cancel persists");
    assert_eq!(cancelled.state(), MethodState::Cancelled);

    let fail_request = contract
        .bind_host_request(execution_host_input(BmadInvocationMode::Headless))
        .expect("failure binding");
    let failed_dispatch = BmadDomainRuntime
        .dispatch_skill_execution(
            &service,
            &BmadReviewSkill,
            fail_request,
            create_session_named(BmadInvocationMode::Headless, "failure"),
        )
        .expect("failure dispatch");
    let failure_review = service
        .request_context_review(
            &failed_dispatch.session().scope(),
            &failed_dispatch.session().session_id(),
            failed_dispatch.session().version(),
        )
        .expect("failure review request");
    let failure_decision = context_decision(
        failure_review.current_binding().expect("failure binding"),
        "failure",
    );
    let ready_to_fail = service
        .record_context_review(
            &failure_review.scope(),
            &failure_review.session_id(),
            failure_review.version(),
            failure_decision.clone(),
        )
        .expect("failure review accepted");
    let (advancing, _) = service
        .begin_advance(
            &ready_to_fail.scope(),
            &ready_to_fail.session_id(),
            ready_to_fail.current_binding().expect("ready binding"),
            advance_request(&ready_to_fail, failure_decision.decision_id, "failure"),
        )
        .expect("failure invocation starts");
    let incomplete = service
        .record_incomplete(
            &advancing.scope(),
            &advancing.session_id(),
            advancing.version(),
        )
        .expect("failure persists as incomplete");
    assert_eq!(incomplete.state(), MethodState::Incomplete);

    let mut upgrade_input = execution_host_input(BmadInvocationMode::Interactive);
    let initial_request = contract
        .bind_host_request(upgrade_input.clone())
        .expect("initial upgrade binding");
    let upgraded_dispatch = BmadDomainRuntime
        .dispatch_skill_execution(
            &service,
            &BmadReviewSkill,
            initial_request,
            create_session_named(BmadInvocationMode::Interactive, "upgrade"),
        )
        .expect("upgrade dispatch");
    let review_required = service
        .request_context_review(
            &upgraded_dispatch.session().scope(),
            &upgraded_dispatch.session().session_id(),
            upgraded_dispatch.session().version(),
        )
        .expect("initial review");
    upgrade_input.resolved_config_hash = digest("upgraded review config");
    let upgraded_request = contract
        .bind_host_request(upgrade_input)
        .expect("upgraded binding");
    let upgraded_prepared = BmadDomainRuntime
        .prepare_skill_execution(&BmadReviewSkill, upgraded_request)
        .expect("upgraded preparation");
    let rebound = service
        .rebind_invocation(
            &review_required.scope(),
            &review_required.session_id(),
            review_required.version(),
            upgraded_prepared.exact_binding().clone(),
            upgraded_prepared.step_table().clone(),
        )
        .expect("stale exact inputs require a governed rebind");
    assert_eq!(rebound.state(), MethodState::CapabilityBound);
    assert_ne!(
        rebound.current_binding().expect("rebound binding"),
        review_required.current_binding().expect("prior binding")
    );
    let resumed = service
        .resume(&rebound.scope(), &rebound.session_id())
        .expect("restart resumes upgraded state");
    assert_eq!(resumed, rebound);

    let budget: BmadActionPlanBudget = serde_json::from_value(serde_json::json!({
        "maxModelCalls": 1,
        "maxRepairs": 1,
        "maxRetries": 1,
        "maxTokens": 1000
    }))
    .expect("review recovery budget");
    let plans = plan_source_steps(
        "capability_01J00000000000000000000042",
        "session_01J00000000000000000000042",
        &budget,
        &[BmadSourceStep {
            plan_id: "plan_01J00000000000000000000042".to_owned(),
            source_id: "source_01J00000000000000000000042".to_owned(),
            step_index: 0,
            text: "model:repair_review_findings".to_owned(),
        }],
    )
    .expect("review model recovery plan");
    let model_plan = plans.first().expect("model plan");
    let mut usage = BmadModelCallUsage::default();
    record_retry(model_plan, &mut usage).expect("retry stays within review budget");
    record_repair(model_plan, &mut usage).expect("repair stays within review budget");
    assert_eq!((usage.retries, usage.repairs), (1, 1));

    let transitions = repository.transitions.lock().expect("transitions");
    for required in [
        MethodPersistenceEvent::CapabilityBound,
        MethodPersistenceEvent::Cancelled,
        MethodPersistenceEvent::Incomplete,
        MethodPersistenceEvent::ContextReviewRequested,
        MethodPersistenceEvent::CapabilityRebound,
    ] {
        assert!(
            transitions.iter().any(|(event, _)| *event == required),
            "missing persisted recovery transition {required:?}"
        );
    }
}

#[test]
fn the_adapter_remains_declarative_and_cannot_bypass_governed_ports() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/bmad/skills/adapters/bmad_review.rs"),
    )
    .expect("adapter source");
    for forbidden in [
        "std::fs",
        "std::process",
        "std::net",
        "tokio::net",
        "reqwest",
        "Command::new",
        "MethodSessionService",
        "ModelAccessPort",
        "ArtifactPort",
        "ActionPlanPort",
    ] {
        assert!(
            !source.lines().any(|line| {
                line.split("//")
                    .next()
                    .unwrap_or_default()
                    .contains(forbidden)
            }),
            "adapter must not reference {forbidden}"
        );
    }
}
