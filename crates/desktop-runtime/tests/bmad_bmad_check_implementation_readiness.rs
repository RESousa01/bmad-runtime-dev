//! Source-derived conformance tests for `bmad-check-implementation-readiness`.

#![allow(clippy::expect_used, clippy::panic)]

use std::sync::Mutex;

use desktop_runtime::{
    generated_contracts::{
        BmadActionPlanKind, BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationMode,
    },
    sha256_bytes, AuthorityRef, BmadCheckImplementationReadinessSkill, BmadDomainRuntime,
    BmadImplementationReadinessAnswers, BmadMethodSkill, BmadSkillHostBindingInput,
    BmadSkillRecoverySemantics, BmadSkillSupportState, BmadSkillToolIntent, ContractId,
    CreateMethodSession, MethodAdvanceDisposition, MethodAdvanceReceipt, MethodAdvanceRequest,
    MethodArtifactProvenance, MethodContextDecision, MethodExactBinding, MethodModelBinding,
    MethodModelBindingData, MethodPersistenceEvent, MethodSession, MethodSessionRepository,
    MethodSessionScope, MethodSessionService, MethodState, Sha256Digest, UnixMillis,
};

const SKILL_ROOT: &str = "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness";

const RESOURCE_PATHS: [&str; 9] = [
    "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/SKILL.md",
    "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/customize.toml",
    "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/steps/step-01-document-discovery.md",
    "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/steps/step-02-prd-analysis.md",
    "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/steps/step-03-epic-coverage-validation.md",
    "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/steps/step-04-ux-alignment.md",
    "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/steps/step-05-epic-quality-review.md",
    "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/steps/step-06-final-assessment.md",
    "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/templates/readiness-report-template.md",
];

fn id(value: &str) -> ContractId {
    ContractId::new(value).expect("canonical test identifier")
}

fn test_digest(label: &str) -> Sha256Digest {
    sha256_bytes(label.as_bytes())
}

fn model_binding() -> MethodModelBinding {
    let egress_profile_hash = test_digest("readiness egress");
    MethodModelBinding::from_source(
        MethodModelBindingData {
            binding_kind: "method_model".to_owned(),
            provider_id: "test-provider".to_owned(),
            model_id: "test-model".to_owned(),
            deployment_id: "test-deployment".to_owned(),
            model_profile_hash: test_digest("readiness model profile"),
            model_capability_hash: test_digest("readiness model capability"),
            context_window_profile_hash: test_digest("readiness context window"),
            egress_profile_hash,
            request_schema_hash: test_digest("readiness request schema"),
            response_schema_hash: test_digest("readiness response schema"),
        },
        test_digest("readiness model binding"),
    )
    .expect("valid test model binding")
}

#[derive(Debug, thiserror::Error)]
#[error("readiness repository failure")]
struct RepositoryError;

#[derive(Default)]
struct Repository {
    sessions: Mutex<Vec<MethodSession>>,
    events: Mutex<Vec<MethodPersistenceEvent>>,
}

impl MethodSessionRepository for &Repository {
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
            .find(|candidate| candidate.scope() == *scope && candidate.session_id() == *session_id)
            .ok_or(RepositoryError)?;
        session
            .validate_review_for(observed_binding)
            .map_err(|_| RepositoryError)?;
        session.begin_advance(request).map_err(|_| RepositoryError)
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
            .find(|candidate| candidate.session_id() == session.session_id())
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

fn creation(mode: BmadInvocationMode) -> CreateMethodSession {
    creation_named(mode, "initial")
}

fn creation_named(mode: BmadInvocationMode, attempt: &str) -> CreateMethodSession {
    let suffix = match mode {
        BmadInvocationMode::Interactive => "interactive",
        BmadInvocationMode::Headless => "headless",
    };
    CreateMethodSession {
        session_id: id(&format!("session_readiness_{suffix}_{attempt}")),
        owner_scope_ref: id("ownerscope_readiness"),
        project_id: id("project_readiness"),
        run_id: id(&format!("run_readiness_{suffix}_{attempt}")),
        authority_ref: AuthorityRef {
            authority_kind: "desktop_local_store".to_owned(),
            authority_id: id("authority_readiness"),
            installation_id: id("install_readiness"),
            local_store_id: id("store_readiness"),
            authority_epoch: 1,
        },
        created_at: UnixMillis(1_000),
    }
}

fn review_decision(session: &MethodSession, label: &str) -> MethodContextDecision {
    MethodContextDecision {
        decision_id: id(&format!("decision_readiness_{label}")),
        manifest_hash: test_digest(&format!("manifest {label}")),
        consent_hash: test_digest(&format!("consent {label}")),
        context_digest: test_digest(&format!("context {label}")),
        binding_hash: session
            .current_binding()
            .expect("bound session")
            .binding_hash()
            .expect("binding hash"),
        reviewed_at: UnixMillis(2_000),
    }
}

fn advance_request(
    session: &MethodSession,
    decision: &MethodContextDecision,
    label: &str,
) -> MethodAdvanceRequest {
    let invocation_id = id(&format!("invoke_readiness_{label}"));
    let d2_model_invocation_binding_hash = test_digest(&format!("d2 invocation {label}"));
    MethodAdvanceRequest {
        model_request_id: id(&format!("modelreq_readiness_{label}")),
        idempotency_key: format!("readiness-{label}"),
        decision_id: decision.decision_id.clone(),
        decision_consumption_hash: test_digest(&format!("decision consumption {label}")),
        model_request_hash: test_digest(&format!("model request {label}")),
        session_authority_hash: session
            .session_authority_hash()
            .expect("session authority hash"),
        d2_model_invocation_binding_hash,
        model_bridge_binding_hash: session
            .model_bridge_binding_hash(&d2_model_invocation_binding_hash)
            .expect("bridge binding hash"),
        invocation_id,
        expected_version: session.version(),
    }
}

// Break caught: omitting any pinned micro-file makes a future step impossible
// to load and prevents the generic runtime from proving source closure.
#[test]
fn implementation_readiness_descriptor_closes_every_owned_source_member() {
    let descriptor = BmadCheckImplementationReadinessSkill.descriptor();

    assert_eq!(
        descriptor.canonical_id(),
        "bmm:bmad-check-implementation-readiness"
    );
    assert_eq!(descriptor.source_path(), RESOURCE_PATHS[0]);
    assert!(descriptor.declared_actions().is_empty());
    assert_eq!(
        descriptor.supported_modes(),
        [
            BmadInvocationMode::Interactive,
            BmadInvocationMode::Headless
        ]
    );
    assert_eq!(descriptor.predecessor(), None);
    assert_eq!(descriptor.follower(), Some("bmm:bmad-help"));
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
        RESOURCE_PATHS
    );

    for resource in descriptor.declared_resources() {
        let source_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/bmad-foundation/source/method/6.10.0")
            .join(resource.path());
        let bytes = std::fs::read(&source_path).unwrap_or_else(|error| {
            panic!(
                "could not read pinned resource {}: {error}",
                source_path.display()
            )
        });
        assert_eq!(
            resource.content_hash(),
            sha256_bytes(&bytes),
            "{} must retain its pinned SHA-256 identity",
            resource.path()
        );
    }
}

// Break caught: a catalog-visible descriptor without an explicit execution
// contract can never enter the shared Method compiler or durable lifecycle.
#[test]
fn implementation_readiness_exposes_an_explicit_generic_execution_contract() {
    let contract = BmadCheckImplementationReadinessSkill
        .execution_contract()
        .expect("the canonical readiness adapter must opt into generic Method execution");

    assert_eq!(contract.package_name(), "bmad-method");
    assert_eq!(contract.package_version(), "6.10.0");
}

// Break caught: flattening the six just-in-time source files into one prompt
// loses the ordered checkpoint boundary needed for restart and resume.
#[test]
fn implementation_readiness_contract_preserves_governed_effects_and_recovery() {
    let contract = BmadCheckImplementationReadinessSkill
        .execution_contract()
        .expect("source-bound execution contract");
    let debug = format!("{contract:#?}");

    for step in [
        "document_discovery",
        "prd_analysis",
        "epic_coverage_validation",
        "ux_alignment",
        "epic_quality_review",
        "final_assessment",
    ] {
        assert!(debug.contains(step), "missing ordered source step {step}");
    }
    for intent in [
        BmadSkillToolIntent::FileRead,
        BmadSkillToolIntent::FileWrite,
        BmadSkillToolIntent::Process,
        BmadSkillToolIntent::ExternalHandoff,
    ] {
        assert!(
            debug.contains(&format!("{intent:?}")),
            "missing typed tool intent {intent:?}"
        );
    }
    assert_eq!(
        BmadSkillToolIntent::FileRead.required_plan_kind(),
        BmadActionPlanKind::Read
    );
    assert_eq!(
        BmadSkillToolIntent::FileWrite.required_plan_kind(),
        BmadActionPlanKind::WriteProposal
    );
    assert_eq!(
        BmadSkillToolIntent::Process.required_plan_kind(),
        BmadActionPlanKind::CommandProposal
    );
    assert_eq!(
        BmadSkillToolIntent::ExternalHandoff.required_plan_kind(),
        BmadActionPlanKind::ArtifactPlan
    );
    assert!(debug.contains(&format!(
        "{:?}",
        BmadSkillRecoverySemantics::MethodCheckpoint
    )));
    assert!(debug.contains("implementation-readiness-report-{{date}}.md"));
}

// Break caught: treating `execution_contract()` or a catalog availability bit
// as dispatch would skip both the generic compiler and durable Method service.
#[test]
fn readiness_dispatch_prepares_and_persists_both_modes_through_method_service() {
    let repository = Repository::default();
    let service = MethodSessionService::new(&repository);
    let runtime = BmadDomainRuntime;
    let mut prepared = Vec::new();

    for mode in [
        BmadInvocationMode::Interactive,
        BmadInvocationMode::Headless,
    ] {
        let contract = BmadCheckImplementationReadinessSkill
            .execution_contract()
            .expect("source-bound readiness contract");
        let model_binding = model_binding();
        let request = contract
            .bind_host_request(BmadSkillHostBindingInput {
                mode,
                action: None,
                resolved_config_graph_hash: test_digest("readiness config graph"),
                resolved_config_hash: test_digest("readiness config"),
                resolved_customization_hash: test_digest("readiness customization"),
                egress_profile_hash: model_binding.data.egress_profile_hash,
                model_binding,
            })
            .expect("trusted host binding produces an exact request");
        let dispatched = runtime
            .dispatch_skill_execution(
                &service,
                &BmadCheckImplementationReadinessSkill,
                request,
                creation(mode),
            )
            .expect("generic compiler and Method service dispatch");
        assert_eq!(dispatched.session().state(), MethodState::CapabilityBound);
        assert_eq!(dispatched.prepared().mode(), mode);
        assert_eq!(
            dispatched.prepared().required_plan_kinds(),
            [
                BmadActionPlanKind::Read,
                BmadActionPlanKind::WriteProposal,
                BmadActionPlanKind::CommandProposal,
                BmadActionPlanKind::ArtifactPlan,
            ]
        );
        let resumed = service
            .resume(
                &dispatched.session().scope(),
                &dispatched.session().session_id(),
            )
            .expect("restart loads the authoritative session");
        assert_eq!(resumed, *dispatched.session());
        prepared.push(dispatched.prepared().clone());
    }

    assert_eq!(
        prepared[0].artifact_expectations(),
        prepared[1].artifact_expectations()
    );
    assert_eq!(prepared[0].step_table(), prepared[1].step_table());
}

#[test]
fn interactive_and_headless_render_the_same_real_six_section_report() {
    let answers = BmadImplementationReadinessAnswers {
        document_discovery: "PRD, architecture, UX, and epics were inventoried.".to_owned(),
        prd_analysis: "Functional and non-functional requirements are complete.".to_owned(),
        epic_coverage_validation: "Every requirement maps to an epic and story.".to_owned(),
        ux_alignment: "UX flows agree with the architecture constraints.".to_owned(),
        epic_quality_review: "Stories are independently testable and sequenced.".to_owned(),
        final_assessment: "READY. No critical issue remains.".to_owned(),
    };
    let interactive = BmadCheckImplementationReadinessSkill::render_reviewed_report(
        BmadInvocationMode::Interactive,
        "2026-07-29",
        &answers,
    )
    .expect("interactive output");
    let headless = BmadCheckImplementationReadinessSkill::render_reviewed_report(
        BmadInvocationMode::Headless,
        "2026-07-29",
        &answers,
    )
    .expect("headless output");

    assert_eq!(interactive, headless);
    assert_eq!(
        interactive.artifact_path(),
        "{planning_artifacts}/implementation-readiness-report-2026-07-29.md"
    );
    for heading in [
        "## Document Discovery",
        "## PRD Analysis",
        "## Epic Coverage Validation",
        "## UX Alignment",
        "## Epic Quality Review",
        "## Summary and Recommendations",
    ] {
        assert!(interactive.markdown().contains(heading));
    }

    let mut incomplete = answers;
    incomplete.ux_alignment.clear();
    assert!(
        BmadCheckImplementationReadinessSkill::render_reviewed_report(
            BmadInvocationMode::Headless,
            "2026-07-29",
            &incomplete,
        )
        .is_err(),
        "headless cannot skip a source menu checkpoint"
    );
}

#[test]
fn persisted_lifecycle_covers_cancel_fail_retry_repair_restart_resume_and_stale_upgrade() {
    let repository = Repository::default();
    let service = MethodSessionService::new(&repository);
    let runtime = BmadDomainRuntime;
    let contract = BmadCheckImplementationReadinessSkill
        .execution_contract()
        .expect("execution contract");

    let bind = |mode, config_label: &str| {
        let model_binding = model_binding();
        contract
            .bind_host_request(BmadSkillHostBindingInput {
                mode,
                action: None,
                resolved_config_graph_hash: test_digest(&format!("graph {config_label}")),
                resolved_config_hash: test_digest(&format!("config {config_label}")),
                resolved_customization_hash: test_digest(&format!("customization {config_label}")),
                egress_profile_hash: model_binding.data.egress_profile_hash,
                model_binding,
            })
            .expect("host request")
    };

    let cancelled = runtime
        .dispatch_skill_execution(
            &service,
            &BmadCheckImplementationReadinessSkill,
            bind(BmadInvocationMode::Interactive, "cancel"),
            creation_named(BmadInvocationMode::Interactive, "cancel"),
        )
        .expect("cancel branch dispatch");
    let cancelled = service
        .cancel(
            &cancelled.session().scope(),
            &cancelled.session().session_id(),
            cancelled.session().version(),
        )
        .expect("persist cancellation");
    assert_eq!(cancelled.state(), MethodState::Cancelled);

    let failed = runtime
        .dispatch_skill_execution(
            &service,
            &BmadCheckImplementationReadinessSkill,
            bind(BmadInvocationMode::Headless, "failed"),
            creation_named(BmadInvocationMode::Headless, "failed"),
        )
        .expect("failed attempt dispatch");
    let failed = service
        .request_context_review(
            &failed.session().scope(),
            &failed.session().session_id(),
            failed.session().version(),
        )
        .expect("request review");
    let decision = review_decision(&failed, "failed");
    let failed = service
        .record_context_review(
            &failed.scope(),
            &failed.session_id(),
            failed.version(),
            decision.clone(),
        )
        .expect("persist answers and approval");
    let request = advance_request(&failed, &decision, "failed");
    let binding = failed.current_binding().expect("binding").clone();
    let (advancing, _) = service
        .begin_advance(&failed.scope(), &failed.session_id(), &binding, request)
        .expect("persist model invocation and decision consumption");
    assert_eq!(advancing.state(), MethodState::Advancing);
    let failed = service
        .record_incomplete(
            &advancing.scope(),
            &advancing.session_id(),
            advancing.version(),
        )
        .expect("persist failure");
    assert_eq!(failed.state(), MethodState::Incomplete);

    let retry = runtime
        .dispatch_skill_execution(
            &service,
            &BmadCheckImplementationReadinessSkill,
            bind(BmadInvocationMode::Headless, "retry"),
            creation_named(BmadInvocationMode::Headless, "retry"),
        )
        .expect("retry is a distinct persisted attempt");
    assert_eq!(retry.session().state(), MethodState::CapabilityBound);
    assert_eq!(
        service
            .resume(&failed.scope(), &failed.session_id())
            .expect("failed attempt remains durable")
            .state(),
        MethodState::Incomplete
    );

    let review_required = service
        .request_context_review(
            &retry.session().scope(),
            &retry.session().session_id(),
            retry.session().version(),
        )
        .expect("repair branch enters review");
    let mut repaired_binding = retry.prepared().exact_binding().clone();
    repaired_binding.config_resolution_hash = test_digest("repaired config resolution");
    let repaired = service
        .rebind_invocation(
            &review_required.scope(),
            &review_required.session_id(),
            review_required.version(),
            repaired_binding,
            retry.prepared().step_table().clone(),
        )
        .expect("persist repaired configuration binding");
    assert_eq!(repaired.state(), MethodState::CapabilityBound);

    let stale_review = service
        .request_context_review(
            &repaired.scope(),
            &repaired.session_id(),
            repaired.version(),
        )
        .expect("stale-source review");
    let mut upgraded_binding = stale_review.current_binding().expect("binding").clone();
    upgraded_binding.package_source_hash = test_digest("upgraded pinned source");
    let upgraded = service
        .rebind_invocation(
            &stale_review.scope(),
            &stale_review.session_id(),
            stale_review.version(),
            upgraded_binding.clone(),
            retry.prepared().step_table().clone(),
        )
        .expect("persist stale-source upgrade");
    let resumed = service
        .resume(&upgraded.scope(), &upgraded.session_id())
        .expect("restart/resume authoritative state");
    assert_eq!(resumed, upgraded);
    assert_eq!(
        resumed.current_binding().expect("upgraded binding"),
        &upgraded_binding
    );

    let events = repository.events.lock().expect("event log");
    for event in [
        MethodPersistenceEvent::Cancelled,
        MethodPersistenceEvent::Incomplete,
        MethodPersistenceEvent::CapabilityRebound,
    ] {
        assert!(events.contains(&event), "missing persisted event {event:?}");
    }
}

// Break caught: importing a direct filesystem, process, network, scheduler,
// model, or artifact port would allow source prose to bypass Airlock.
#[test]
fn implementation_readiness_adapter_is_an_inert_authority_boundary() {
    let adapter_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/bmad/skills/adapters/bmad_check_implementation_readiness.rs");
    let source = std::fs::read_to_string(&adapter_path)
        .unwrap_or_else(|error| panic!("adapter source {}: {error}", adapter_path.display()));
    for (index, line) in source.lines().enumerate() {
        let code = line.split("//").next().unwrap_or_default();
        for forbidden in [
            "std::fs",
            "std::process",
            "std::net",
            "tokio::net",
            "reqwest",
            "Command::new",
            "ModelAccessPort",
            "ActionPlanPort",
            "ArtifactPort",
        ] {
            assert!(
                !code.contains(forbidden),
                "adapter line {} references forbidden authority {forbidden:?}: {line:?}",
                index + 1
            );
        }
    }
    assert!(source.contains(SKILL_ROOT));
}
