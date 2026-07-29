//! Source-derived conformance tests for `bmad-check-implementation-readiness`.

#![allow(clippy::expect_used, clippy::panic)]

use std::sync::Mutex;

use desktop_runtime::{
    generated_contracts::{
        BmadActionPlanKind, BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationMode,
    },
    sha256_bytes, AuthorityRef, BmadCheckImplementationReadinessSkill, BmadDomainRuntime,
    BmadMethodSkill, BmadSkillHostBindingInput, BmadSkillRecoverySemantics, BmadSkillSupportState,
    BmadSkillToolIntent, ContractId, CreateMethodSession, MethodAdvanceDisposition,
    MethodAdvanceReceipt, MethodAdvanceRequest, MethodArtifactProvenance, MethodExactBinding,
    MethodModelBinding, MethodModelBindingData, MethodPersistenceEvent, MethodSession,
    MethodSessionRepository, MethodSessionScope, MethodSessionService, MethodState, Sha256Digest,
    UnixMillis,
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
            .find(|candidate| candidate.session_id() == session.session_id())
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

fn creation(mode: BmadInvocationMode) -> CreateMethodSession {
    let suffix = match mode {
        BmadInvocationMode::Interactive => "interactive",
        BmadInvocationMode::Headless => "headless",
    };
    CreateMethodSession {
        session_id: id(&format!("session_readiness_{suffix}")),
        owner_scope_ref: id("ownerscope_readiness"),
        project_id: id("project_readiness"),
        run_id: id(&format!("run_readiness_{suffix}")),
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
    assert_eq!(descriptor.support_state(), BmadSkillSupportState::Exact);
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
        assert_eq!(
            dispatched.session().state(),
            MethodState::ContextReviewRequired
        );
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
