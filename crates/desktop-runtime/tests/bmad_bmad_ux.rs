//! Source-derived conformance tests for the canonical `bmad-ux` adapter.

#![allow(clippy::expect_used, clippy::panic)]

use std::sync::Mutex;

use desktop_runtime::{
    generated_contracts::{BmadActionPlanKind, BmadArtifactKind, BmadInvocationMode},
    sha256_bytes, AuthorityRef, BmadDomainRuntime, BmadMethodSkill, BmadRuntimePort,
    BmadSkillHostBindingInput, BmadSkillSupportState, BmadSkillToolIntent, BmadUxSkill, ContractId,
    CreateMethodSession, MethodAdvanceDisposition, MethodAdvanceReceipt, MethodAdvanceRequest,
    MethodArtifactProvenance, MethodContextDecision, MethodExactBinding, MethodModelBinding,
    MethodModelBindingData, MethodPersistenceEvent, MethodSession, MethodSessionRepository,
    MethodSessionScope, MethodSessionService, MethodState, Sha256Digest, UnixMillis,
};

const SKILL_ROOT: &str = "src/bmm-skills/2-plan-workflows/bmad-ux";

fn test_digest(label: &str) -> Sha256Digest {
    sha256_bytes(label.as_bytes())
}

fn host_binding(mode: BmadInvocationMode, action: Option<&str>) -> BmadSkillHostBindingInput {
    let model_binding = MethodModelBinding::from_source(
        MethodModelBindingData {
            binding_kind: "method_model".to_owned(),
            provider_id: "test-provider".to_owned(),
            model_id: "test-model".to_owned(),
            deployment_id: "test-deployment".to_owned(),
            model_profile_hash: test_digest("test model profile"),
            model_capability_hash: test_digest("test model capability"),
            context_window_profile_hash: test_digest("test context window"),
            egress_profile_hash: test_digest("test egress"),
            request_schema_hash: test_digest("test request schema"),
            response_schema_hash: test_digest("test response schema"),
        },
        test_digest("test model binding"),
    )
    .expect("valid inert model binding");
    BmadSkillHostBindingInput {
        mode,
        action: action.map(str::to_owned),
        egress_profile_hash: model_binding.data.egress_profile_hash,
        model_binding,
        resolved_config_graph_hash: test_digest("test config graph"),
        resolved_config_hash: test_digest("test config"),
        resolved_customization_hash: test_digest("test customization"),
    }
}

fn id(value: &str) -> ContractId {
    ContractId::new(value.to_owned()).expect("valid test identifier")
}

#[derive(Debug, thiserror::Error)]
#[error("in-memory repository failure")]
struct RepositoryError;

#[derive(Default)]
struct Repository {
    sessions: Mutex<Vec<MethodSession>>,
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

fn create_session(suffix: usize) -> CreateMethodSession {
    CreateMethodSession {
        session_id: id(&format!("session_ux_{suffix:02}")),
        owner_scope_ref: id("owner_scope_ux"),
        project_id: id("project_ux"),
        run_id: id(&format!("run_ux_{suffix:02}")),
        authority_ref: AuthorityRef {
            authority_kind: "desktop_local_store".to_owned(),
            authority_id: id("authority_ux"),
            installation_id: id("installation_ux"),
            local_store_id: id("local_store_ux"),
            authority_epoch: 1,
        },
        created_at: UnixMillis(1_000 + suffix as u64),
    }
}

fn reviewed_decision(session: &MethodSession, suffix: usize) -> MethodContextDecision {
    MethodContextDecision {
        decision_id: id(&format!("decision_ux_{suffix:02}")),
        manifest_hash: test_digest("review manifest"),
        consent_hash: test_digest("review consent"),
        context_digest: test_digest("review context"),
        binding_hash: session
            .current_binding()
            .expect("bound session")
            .binding_hash()
            .expect("binding hash"),
        reviewed_at: UnixMillis(2_000 + suffix as u64),
    }
}

fn advance_request(
    session: &MethodSession,
    decision: &MethodContextDecision,
    suffix: usize,
) -> MethodAdvanceRequest {
    let invocation_id = id(&format!("invoke_ux_{suffix:02}"));
    let d2_model_invocation_binding_hash = test_digest(&format!("d2 invocation binding {suffix}"));
    MethodAdvanceRequest {
        invocation_id: invocation_id.clone(),
        idempotency_key: format!("ux-retry-{suffix:02}"),
        decision_id: decision.decision_id.clone(),
        model_request_id: id(&format!("model_request_ux_{suffix:02}")),
        decision_consumption_hash: test_digest(&format!("decision consumption {suffix}")),
        model_request_hash: test_digest(&format!("model request {suffix}")),
        session_authority_hash: session.session_authority_hash().expect("session authority"),
        d2_model_invocation_binding_hash,
        model_bridge_binding_hash: session
            .model_bridge_binding_hash(&d2_model_invocation_binding_hash)
            .expect("model bridge"),
        expected_version: session.version(),
    }
}

// Break caught: omitting any pinned UX member, collapsing Create/Update/
// Validate into an undeclared conversational branch, or losing the two
// source-declared modes makes the generic runtime unable to bind the exact
// source semantics before any governed dispatch.
#[test]
fn bmad_ux_descriptor_closes_all_pinned_members_and_declares_source_actions() {
    let descriptor = BmadUxSkill.descriptor();

    assert_eq!(descriptor.canonical_id(), "bmm:bmad-ux");
    assert_eq!(
        descriptor.source_path(),
        "src/bmm-skills/2-plan-workflows/bmad-ux/SKILL.md"
    );
    assert_eq!(
        descriptor
            .declared_actions()
            .iter()
            .map(|action| action.name.as_str())
            .collect::<Vec<_>>(),
        ["create", "update", "validate"]
    );
    assert_eq!(
        descriptor.supported_modes(),
        [
            BmadInvocationMode::Interactive,
            BmadInvocationMode::Headless
        ]
    );
    assert_eq!(descriptor.artifact_kinds(), [BmadArtifactKind::Document]);
    assert_eq!(descriptor.support_state(), BmadSkillSupportState::Exact);
    assert_eq!(descriptor.declared_resources().len(), 17);

    for resource in descriptor.declared_resources() {
        assert!(
            resource.path().starts_with(SKILL_ROOT),
            "{} escapes the pinned UX subtree",
            resource.path()
        );
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

// Break caught: adding a direct I/O, process, network, scheduler, or model
// dependency to the adapter lets source prose bypass typed plans and Airlock.
#[test]
fn bmad_ux_adapter_remains_an_inert_authority_boundary() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/bmad/skills/adapters/bmad_ux.rs"),
    )
    .expect("bmad-ux adapter source");
    let forbidden = [
        "std::fs",
        "std::process",
        "std::net",
        "tokio::net",
        "reqwest",
        "Command::new",
        "ModelAccessPort",
        "ActionPlanPort",
        "ArtifactPort",
    ];
    for (index, line) in source.lines().enumerate() {
        let code = line.split("//").next().unwrap_or_default();
        for token in forbidden {
            assert!(
                !code.contains(token),
                "bmad_ux.rs line {} references forbidden capability {token:?}: {line:?}",
                index + 1,
            );
        }
    }
    assert!(source.contains(SKILL_ROOT));
}

// Break caught: a complete descriptor that never opts into the shared
// descriptor-bound Method compiler remains visible but cannot execute.
#[test]
fn bmad_ux_exposes_an_explicit_generic_execution_contract() {
    let contract = BmadUxSkill
        .execution_contract()
        .expect("the canonical UX adapter must opt into generic Method execution");

    assert_eq!(contract.package_name(), "bmad-method");
    assert_eq!(contract.package_version(), "6.10.0");
}

// Break caught: routing every UX intent through one plausible generic prompt
// loses the source's Create/Update/Validate branches and can incorrectly
// complete without both peer spines.
#[test]
fn bmad_ux_prepares_every_action_in_both_modes_with_source_artifact_parity() {
    let runtime = BmadDomainRuntime;
    let mut branch_hashes = Vec::new();

    for mode in [
        BmadInvocationMode::Interactive,
        BmadInvocationMode::Headless,
    ] {
        for action in ["create", "update", "validate"] {
            let request = BmadUxSkill
                .execution_contract()
                .expect("UX contract")
                .bind_host_request(host_binding(mode, Some(action)))
                .expect("source action binds");
            let prepared = runtime
                .prepare_skill_execution(&BmadUxSkill, request)
                .expect("source action prepares through generic runtime");

            assert_eq!(prepared.descriptor().canonical_id(), "bmm:bmad-ux");
            assert_eq!(prepared.mode(), mode);
            assert_eq!(prepared.action(), Some(action));
            assert_eq!(prepared.resources().resources().len(), 17);
            assert_eq!(
                prepared.tool_intents(),
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
            assert_eq!(
                prepared.required_plan_kinds(),
                [
                    BmadActionPlanKind::Read,
                    BmadActionPlanKind::WriteProposal,
                    BmadActionPlanKind::CommandProposal,
                    BmadActionPlanKind::Read,
                    BmadActionPlanKind::ModelCall,
                    BmadActionPlanKind::Read,
                    BmadActionPlanKind::ArtifactPlan,
                ]
            );
            let output_hints = prepared
                .artifact_expectations()
                .iter()
                .filter_map(|expectation| expectation.source_output_hint.as_deref())
                .collect::<Vec<_>>();
            if action == "validate" {
                assert_eq!(
                    output_hints,
                    ["{doc_workspace}/validation-report.{md,html}"]
                );
            } else {
                assert_eq!(
                    output_hints,
                    ["{doc_workspace}/DESIGN.md", "{doc_workspace}/EXPERIENCE.md"]
                );
            }
            if mode == BmadInvocationMode::Interactive {
                branch_hashes.push(*prepared.step_table().table_hash());
            } else {
                assert_eq!(
                    branch_hashes[match action {
                        "create" => 0,
                        "update" => 1,
                        _ => 2,
                    }],
                    *prepared.step_table().table_hash(),
                    "interactive and headless use the same source branch"
                );
            }
        }
    }
    assert_ne!(branch_hashes[0], branch_hashes[1]);
    assert_ne!(branch_hashes[1], branch_hashes[2]);
}

// Break caught: a production caller that compiles a UX contract but skips the
// MethodSessionService would leave no durable exact binding and could invent a
// parallel lifecycle outside the fail-closed Method authority boundary.
#[test]
fn bmad_ux_production_dispatch_binds_every_action_and_mode_through_method_sessions() {
    let mut suffix = 0;

    for mode in [
        BmadInvocationMode::Interactive,
        BmadInvocationMode::Headless,
    ] {
        for action in ["create", "update", "validate"] {
            suffix += 1;
            let request = BmadUxSkill
                .execution_contract()
                .expect("UX contract")
                .bind_host_request(host_binding(mode, Some(action)))
                .expect("source action binds");
            let service = MethodSessionService::new(Repository::default());
            let dispatched = BmadDomainRuntime
                .dispatch_skill_execution(&service, &BmadUxSkill, request, create_session(suffix))
                .expect("production dispatch");

            assert_eq!(dispatched.prepared().action(), Some(action));
            assert_eq!(dispatched.prepared().mode(), mode);
            assert_eq!(dispatched.session().state(), MethodState::CapabilityBound);
            assert_eq!(
                dispatched
                    .session()
                    .current_binding()
                    .expect("exact binding"),
                dispatched.prepared().exact_binding()
            );
        }
    }
}

// Break caught: replacing durable Method transitions with UX-specific retry,
// repair, cancellation, or resume flags creates split-brain lifecycle state.
#[test]
fn bmad_ux_uses_deterministic_method_lifecycle_for_failure_retry_repair_and_resume() {
    let request_for = |host: BmadSkillHostBindingInput| {
        BmadUxSkill
            .execution_contract()
            .expect("UX contract")
            .bind_host_request(host)
            .expect("source action binds")
    };

    let failed_service = MethodSessionService::new(Repository::default());
    let mut failed = BmadDomainRuntime
        .dispatch_skill_execution(
            &failed_service,
            &BmadUxSkill,
            request_for(host_binding(
                BmadInvocationMode::Interactive,
                Some("create"),
            )),
            create_session(20),
        )
        .expect("dispatch")
        .session()
        .clone();
    failed
        .request_context_review(failed.version())
        .expect("request review");
    let decision = reviewed_decision(&failed, 20);
    failed
        .record_context_review(failed.version(), decision.clone())
        .expect("record review");
    let request = advance_request(&failed, &decision, 20);
    let first = failed.begin_advance(request.clone()).expect("advance");
    let retried = failed.begin_advance(request).expect("idempotent retry");
    assert_eq!(retried, first);
    failed
        .record_incomplete(failed.version())
        .expect("fail incomplete");
    assert_eq!(failed.state(), MethodState::Incomplete);
    assert!(failed.renderer_projection().recoverable);
    let restarted =
        MethodSession::from_persisted_json(&failed.to_persisted_json().expect("persist"))
            .expect("restart and resume");
    assert_eq!(restarted.state(), MethodState::Incomplete);
    assert_eq!(restarted.version(), failed.version());

    let cancelled_service = MethodSessionService::new(Repository::default());
    let mut cancelled = BmadDomainRuntime
        .dispatch_skill_execution(
            &cancelled_service,
            &BmadUxSkill,
            request_for(host_binding(
                BmadInvocationMode::Interactive,
                Some("validate"),
            )),
            create_session(21),
        )
        .expect("dispatch")
        .session()
        .clone();
    cancelled
        .cancel(cancelled.version())
        .expect("cancel through Method");
    assert_eq!(cancelled.state(), MethodState::Cancelled);
    assert_eq!(
        MethodSession::from_persisted_json(
            &cancelled.to_persisted_json().expect("persist cancellation")
        )
        .expect("restart cancellation")
        .state(),
        MethodState::Cancelled
    );

    let upgraded_service = MethodSessionService::new(Repository::default());
    let mut upgraded = BmadDomainRuntime
        .dispatch_skill_execution(
            &upgraded_service,
            &BmadUxSkill,
            request_for(host_binding(
                BmadInvocationMode::Interactive,
                Some("update"),
            )),
            create_session(22),
        )
        .expect("dispatch")
        .session()
        .clone();
    let original_binding = upgraded.current_binding().expect("binding").clone();
    let original_binding_hash = original_binding.binding_hash().expect("binding hash");
    let replacement = request_for(BmadSkillHostBindingInput {
        resolved_config_hash: test_digest("repaired config"),
        ..host_binding(BmadInvocationMode::Interactive, Some("update"))
    });
    let replacement = BmadDomainRuntime
        .prepare_skill_execution(&BmadUxSkill, replacement)
        .expect("repair/upgrade preparation");
    upgraded
        .rebind_capability(
            upgraded.version(),
            replacement.exact_binding().clone(),
            replacement.step_table().clone(),
        )
        .expect("repair/upgrade rebind");
    assert_eq!(upgraded.state(), MethodState::CapabilityBound);
    assert_ne!(
        upgraded
            .current_binding()
            .expect("upgraded binding")
            .binding_hash()
            .expect("upgraded binding hash"),
        original_binding_hash
    );
    upgraded
        .request_context_review(upgraded.version())
        .expect("review repaired binding");
    let upgraded_decision = reviewed_decision(&upgraded, 22);
    upgraded
        .record_context_review(upgraded.version(), upgraded_decision)
        .expect("record repaired review");
    upgraded
        .validate_review_for(replacement.exact_binding())
        .expect("replacement binding remains valid");
    assert!(
        upgraded.validate_review_for(&original_binding).is_err(),
        "the pre-upgrade binding must be stale after repair"
    );
}
