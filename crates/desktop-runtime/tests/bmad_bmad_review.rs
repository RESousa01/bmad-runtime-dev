//! Source-derived conformance for the canonical `bmad-review` capability.

#![allow(clippy::expect_used, clippy::panic)]

use std::sync::Mutex;

use desktop_runtime::{
    canonical_hash,
    generated_contracts::{
        BmadActionPlanKind, BmadArtifactCompletionHookRunHook, BmadArtifactKind,
        BmadInvocationMode,
    },
    sha256_bytes, AuthorityRef, BmadDomainRuntime, BmadMethodSkill,
    BmadRuntimePort, BmadSkillHostBindingInput, BmadSkillRecoverySemantics,
    BmadSkillToolIntent, BmadReviewSkill, ContractId, CreateMethodSession,
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
        _: MethodPersistenceEvent,
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
        Ok(())
    }

    fn begin_method_advance(
        &self,
        _: &MethodSessionScope,
        _: &ContractId,
        _: &MethodExactBinding,
        _: desktop_runtime::MethodAdvanceRequest,
    ) -> Result<desktop_runtime::MethodAdvanceReceipt, Self::Error> {
        Err(RepositoryError)
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
    let suffix = match mode {
        BmadInvocationMode::Interactive => "interactive",
        BmadInvocationMode::Headless => "headless",
    };
    CreateMethodSession {
        session_id: id(&format!("session_review_{suffix}")),
        owner_scope_ref: id("ownerscope_review"),
        project_id: id("project_review"),
        run_id: id(&format!("run_review_{suffix}")),
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
