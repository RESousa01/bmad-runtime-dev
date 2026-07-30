//! Source-derived conformance tests for the canonical `bmad-deep-recon` adapter.

#![allow(clippy::expect_used, clippy::panic)]

use std::sync::Mutex;

use desktop_runtime::{
    generated_contracts::{
        BmadActionPlanBudget, BmadArtifactCompletionHookRunHook, BmadArtifactKind,
        BmadInvocationMode,
    },
    plan_source_steps, record_repair, record_retry, sha256_bytes, AuthorityRef, BmadDeepReconSkill,
    BmadDomainRuntime, BmadMethodSkill, BmadModelCallUsage, BmadSkillHostBindingInput,
    BmadSkillSupportState, BmadSkillToolIntent, BmadSourceStep, ContractId, CreateMethodSession,
    MethodAdvanceDisposition, MethodAdvanceReceipt, MethodAdvanceRequest, MethodArtifactProvenance,
    MethodContextDecision, MethodExactBinding, MethodModelBinding, MethodModelBindingData,
    MethodPersistenceEvent, MethodSession, MethodSessionRepository, MethodSessionScope,
    MethodSessionService, MethodState, Sha256Digest, UnixMillis,
};
use serde_json::json;

const SKILL_ROOT: &str = "src/core-skills/bmad-deep-recon";

fn id(value: &str) -> ContractId {
    ContractId::new(value).expect("canonical test identifier")
}

fn digest(label: &str) -> Sha256Digest {
    sha256_bytes(label.as_bytes())
}

fn model_binding() -> MethodModelBinding {
    let egress_profile_hash = digest("deep recon egress");
    MethodModelBinding::from_source(
        MethodModelBindingData {
            binding_kind: "method_model".to_owned(),
            provider_id: "test-provider".to_owned(),
            model_id: "test-model".to_owned(),
            deployment_id: "test-deployment".to_owned(),
            model_profile_hash: digest("deep recon model profile"),
            model_capability_hash: digest("deep recon model capability"),
            context_window_profile_hash: digest("deep recon context window"),
            egress_profile_hash,
            request_schema_hash: digest("deep recon request schema"),
            response_schema_hash: digest("deep recon response schema"),
        },
        digest("deep recon model binding"),
    )
    .expect("valid test model binding")
}

fn host_request(
    action: &str,
    mode: BmadInvocationMode,
) -> desktop_runtime::BmadSkillExecutionRequest {
    let contract = BmadDeepReconSkill
        .execution_contract_for_action(Some(action))
        .expect("source action contract");
    let model_binding = model_binding();
    contract
        .bind_host_request(BmadSkillHostBindingInput {
            mode,
            action: Some(action.to_owned()),
            resolved_config_graph_hash: digest("deep recon config graph"),
            resolved_config_hash: digest("deep recon config"),
            resolved_customization_hash: digest("deep recon customization"),
            egress_profile_hash: model_binding.data.egress_profile_hash,
            model_binding,
        })
        .expect("trusted host request")
}

fn creation(suffix: &str) -> CreateMethodSession {
    CreateMethodSession {
        session_id: id(&format!("session_deep_recon_{suffix}")),
        owner_scope_ref: id("ownerscope_deep_recon"),
        project_id: id("project_deep_recon"),
        run_id: id(&format!("run_deep_recon_{suffix}")),
        authority_ref: AuthorityRef {
            authority_kind: "desktop_local_store".to_owned(),
            authority_id: id("authority_deep_recon"),
            installation_id: id("install_deep_recon"),
            local_store_id: id("store_deep_recon"),
            authority_epoch: 1,
        },
        created_at: UnixMillis(1_000),
    }
}

fn advance_request(
    session: &MethodSession,
    decision_id: ContractId,
    suffix: &str,
) -> MethodAdvanceRequest {
    let invocation_id = id(&format!("invoke_deep_recon_{suffix}"));
    let d2_model_invocation_binding_hash = digest(&format!("{suffix}:d2 binding"));
    MethodAdvanceRequest {
        model_request_id: id(&format!("modelreq_deep_recon_{suffix}")),
        decision_consumption_hash: digest(&format!("{suffix}:decision consumption")),
        model_request_hash: digest(&format!("{suffix}:model request")),
        session_authority_hash: session
            .session_authority_hash()
            .expect("session authority hash"),
        d2_model_invocation_binding_hash,
        model_bridge_binding_hash: session
            .model_bridge_binding_hash(&d2_model_invocation_binding_hash)
            .expect("Method/D2 bridge binding"),
        invocation_id,
        idempotency_key: format!("deep-recon-{suffix}"),
        decision_id,
        expected_version: session.version(),
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Deep Recon test repository failure")]
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

// Break caught: omitting any pinned Deep Recon member leaves the generic
// runtime unable to bind the source-faithful workflow before execution.
#[test]
fn bmad_deep_recon_descriptor_closes_every_pinned_member() {
    let descriptor = BmadDeepReconSkill.descriptor();

    assert_eq!(descriptor.canonical_id(), "core:bmad-deep-recon");
    assert_eq!(descriptor.source_path(), format!("{SKILL_ROOT}/SKILL.md"));
    assert_eq!(
        descriptor.supported_modes(),
        [
            BmadInvocationMode::Interactive,
            BmadInvocationMode::Headless
        ]
    );
    assert_eq!(
        descriptor.artifact_kinds(),
        [BmadArtifactKind::Document, BmadArtifactKind::FileSet]
    );
    assert_eq!(descriptor.follower(), Some("core:bmad-help"));
    assert_eq!(descriptor.output_locations(), ["research_output_path"]);
    assert_eq!(
        descriptor.completion_hooks(),
        [BmadArtifactCompletionHookRunHook::PersistArtifacts]
    );
    assert_eq!(descriptor.support_state(), BmadSkillSupportState::Exact);
    assert_eq!(descriptor.declared_resources().len(), 20);

    for resource in descriptor.declared_resources() {
        let resource_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/bmad-foundation/source/method/6.10.0")
            .join(resource.path());
        let bytes = std::fs::read(&resource_path).unwrap_or_else(|error| {
            panic!(
                "could not read pinned resource {}: {error}",
                resource_path.display()
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

// Break caught: a descriptor-only Deep Recon row can be visible in Help while
// remaining impossible to prepare through the shared Method compiler.
#[test]
fn bmad_deep_recon_exposes_the_generic_execution_contract() {
    let contract = BmadDeepReconSkill
        .execution_contract()
        .expect("Deep Recon must opt into generic Method execution");

    assert_eq!(contract.package_name(), "bmad-method");
    assert_eq!(contract.package_version(), "6.10.0");
}

// Break caught: collapsing the five source services to an availability flag
// makes Refresh/Deepen non-resumable and silently routes Draft/Process through
// Run even though the source declares different transition sequences.
#[test]
fn bmad_deep_recon_compiles_every_source_action_and_distinct_authority_intent() {
    let expected = [
        ("draft", vec!["select", "draft_brief", "handoff"]),
        (
            "process",
            vec!["select", "process_imports", "synthesize", "finalize"],
        ),
        (
            "run",
            vec!["select", "research", "verify", "synthesize", "finalize"],
        ),
        (
            "refresh",
            vec![
                "select",
                "resume",
                "research",
                "verify",
                "synthesize",
                "finalize",
            ],
        ),
        (
            "deepen",
            vec![
                "select",
                "resume",
                "research",
                "verify",
                "synthesize",
                "finalize",
            ],
        ),
    ];
    for (action, steps) in expected {
        let contract = BmadDeepReconSkill
            .execution_contract_for_action(Some(action))
            .expect("source action compiles");
        assert_eq!(
            contract
                .workflow()
                .steps
                .iter()
                .map(|step| step.step_key.as_str())
                .collect::<Vec<_>>(),
            steps
        );
    }
    assert_eq!(
        BmadDeepReconSkill
            .execution_contract()
            .expect("default Run contract")
            .tool_intents(),
        [
            BmadSkillToolIntent::FileRead,
            BmadSkillToolIntent::FileWrite,
            BmadSkillToolIntent::Process,
            BmadSkillToolIntent::Web,
            BmadSkillToolIntent::Subagent,
            BmadSkillToolIntent::Browser,
            BmadSkillToolIntent::ExternalHandoff,
        ]
    );
}

// Break caught: exposing the adapter and compiler without sending every source
// branch through the durable service leaves Help-visible actions non-runnable.
#[test]
fn bmad_deep_recon_dispatches_every_action_and_mode_to_persisted_capability_bound() {
    let repository = Repository::default();
    let service = MethodSessionService::new(&repository);
    let runtime = BmadDomainRuntime;

    for mode in [
        BmadInvocationMode::Interactive,
        BmadInvocationMode::Headless,
    ] {
        for action in ["draft", "process", "run", "refresh", "deepen"] {
            let suffix = format!(
                "{action}_{}",
                match mode {
                    BmadInvocationMode::Interactive => "interactive",
                    BmadInvocationMode::Headless => "headless",
                }
            );
            let dispatched = runtime
                .dispatch_skill_execution(
                    &service,
                    &BmadDeepReconSkill,
                    host_request(action, mode),
                    creation(&suffix),
                )
                .expect("shared compiler and Method service dispatch");
            assert_eq!(dispatched.session().state(), MethodState::CapabilityBound);
            assert_eq!(dispatched.prepared().mode(), mode);
            assert_eq!(dispatched.prepared().action(), Some(action));
            assert_eq!(
                service
                    .resume(
                        &dispatched.session().scope(),
                        &dispatched.session().session_id()
                    )
                    .expect("persisted bound session"),
                *dispatched.session()
            );
        }
    }

    let events = repository.events.lock().expect("event lock");
    assert_eq!(events.len(), 10);
    assert!(events
        .iter()
        .all(|event| *event == MethodPersistenceEvent::CapabilityBound));
}

// Break caught: declaring checkpoint recovery in an adapter without executing
// the failure and recovery branches makes restart, repair, and source upgrades
// documentation-only promises.
#[test]
fn bmad_deep_recon_executes_cancel_fail_retry_repair_restart_and_stale_upgrade() {
    let repository = Repository::default();
    let service = MethodSessionService::new(&repository);
    let runtime = BmadDomainRuntime;

    let cancelled = runtime
        .dispatch_skill_execution(
            &service,
            &BmadDeepReconSkill,
            host_request("draft", BmadInvocationMode::Interactive),
            creation("lifecycle_cancel"),
        )
        .expect("cancel fixture dispatches");
    let cancelled = service
        .cancel(
            &cancelled.session().scope(),
            &cancelled.session().session_id(),
            cancelled.session().version(),
        )
        .expect("cancel persists");
    assert_eq!(cancelled.state(), MethodState::Cancelled);

    let dispatched = runtime
        .dispatch_skill_execution(
            &service,
            &BmadDeepReconSkill,
            host_request("run", BmadInvocationMode::Headless),
            creation("lifecycle_failure"),
        )
        .expect("failure fixture dispatches");
    let mut failing = dispatched.session().clone();
    failing
        .request_context_review(failing.version())
        .expect("review required");
    let exact = failing.current_binding().expect("exact binding").clone();
    let decision = MethodContextDecision {
        decision_id: id("decision_deep_recon_failure"),
        manifest_hash: digest("deep recon manifest"),
        consent_hash: digest("deep recon consent"),
        context_digest: digest("deep recon context"),
        binding_hash: exact.binding_hash().expect("binding hash"),
        reviewed_at: UnixMillis(2_000),
    };
    failing
        .record_context_review(failing.version(), decision.clone())
        .expect("review accepted");
    failing
        .begin_advance(advance_request(&failing, decision.decision_id, "failure"))
        .expect("advance begins");
    failing
        .record_incomplete(failing.version())
        .expect("terminal failure persists in aggregate state");
    assert_eq!(failing.state(), MethodState::Incomplete);

    let mut upgrading = dispatched.session().clone();
    let mut upgraded_binding = upgrading
        .current_binding()
        .expect("current binding")
        .clone();
    upgraded_binding.package_source_hash = digest("reviewed Deep Recon source upgrade");
    assert!(upgrading
        .rebind_capability(
            upgrading.version() - 1,
            upgraded_binding.clone(),
            dispatched.prepared().step_table().clone(),
        )
        .is_err());
    upgrading
        .rebind_capability(
            upgrading.version(),
            upgraded_binding,
            dispatched.prepared().step_table().clone(),
        )
        .expect("fresh source upgrade rebinds");
    assert_eq!(upgrading.state(), MethodState::CapabilityBound);
    let persisted = upgrading.to_persisted_json().expect("session serializes");
    let restarted =
        MethodSession::from_persisted_json(&persisted).expect("restart resumes exactly");
    assert_eq!(restarted, upgrading);

    let budget: BmadActionPlanBudget = serde_json::from_value(json!({
        "maxModelCalls": 1,
        "maxRepairs": 1,
        "maxRetries": 1,
        "maxTokens": 1000
    }))
    .expect("bounded model budget");
    let plans = plan_source_steps(
        "capability_01J00000000000000000000038",
        "session_01J00000000000000000000038",
        &budget,
        &[BmadSourceStep {
            plan_id: "plan_01J00000000000000000000038".to_owned(),
            source_id: "source_01J00000000000000000000038".to_owned(),
            step_index: 0,
            text: "model:verify-research".to_owned(),
        }],
    )
    .expect("model plan compiles");
    let mut usage = BmadModelCallUsage::default();
    record_retry(&plans[0], &mut usage).expect("bounded retry executes");
    record_repair(&plans[0], &mut usage).expect("bounded repair executes");
    assert!(record_retry(&plans[0], &mut usage).is_err());
    assert!(record_repair(&plans[0], &mut usage).is_err());
}
