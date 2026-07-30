//! Source-derived conformance tests for the canonical `bmad-spec` adapter.

#![allow(clippy::expect_used, clippy::panic)]

use std::sync::Mutex;

use desktop_runtime::{
    generated_contracts::{
        BmadActionPlanKind, BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationMode,
    },
    sha256_bytes, AuthorityRef, BmadCapabilityKey, BmadDomainRuntime, BmadMethodSkill,
    BmadResourceSet, BmadRuntimePort, BmadSkillAuthorityRequirement, BmadSkillExecutionContract,
    BmadSkillExecutionRequest, BmadSkillRecoverySemantics, BmadSkillSupportState,
    BmadSkillToolIntent, BmadSpecSkill, ContractId, CreateMethodSession, MethodAdvanceDisposition,
    MethodAdvanceReceipt, MethodAdvanceRequest, MethodArtifactExpectation,
    MethodArtifactProvenance, MethodContextDecision, MethodEvidenceClass, MethodExactBinding,
    MethodExecutionProfile, MethodExecutionProfileData, MethodInvocationModes, MethodModelBinding,
    MethodModelBindingData, MethodPersistenceEvent, MethodResourcePolicy, MethodSession,
    MethodSessionRepository, MethodSessionScope, MethodSessionService, MethodState, Sha256Digest,
    UnixMillis,
};

const SKILL_ROOT: &str = "src/bmm-skills/2-plan-workflows/bmad-spec";

fn id(value: &str) -> ContractId {
    ContractId::new(value).expect("canonical test identifier")
}

fn digest(value: &str) -> Sha256Digest {
    Sha256Digest::parse(value).expect("canonical pinned digest")
}

fn test_digest(label: &str) -> Sha256Digest {
    sha256_bytes(label.as_bytes())
}

fn spec_execution_profile() -> MethodExecutionProfile {
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
                declared_resource_paths: BmadSpecSkill
                    .descriptor()
                    .declared_resources()
                    .iter()
                    .map(|resource| resource.path().to_owned())
                    .collect(),
            },
            declared_tool_intents: [
                "file_read",
                "file_write",
                "process",
                "subagent",
                "external_handoff",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            state_hints: vec![
                "memlog".to_owned(),
                "artifact_workspace".to_owned(),
                "spec".to_owned(),
            ],
            completion_evidence: vec![
                "artifact".to_owned(),
                "event".to_owned(),
                "status_evidence".to_owned(),
            ],
            customization_profile: "method_skill_toml".to_owned(),
            validation_profile: "MethodOfficialSkillV6".to_owned(),
        },
        digest("sha256:269d749b9ef8a62894ddccf44c675b04a6cde46b15355cceb1fbe1ecdba3067b"),
    )
    .expect("source-derived Spec execution profile")
}

fn artifact_expectations() -> Vec<MethodArtifactExpectation> {
    [
        (
            "expectation_01J00000000000000000000009",
            "document",
            true,
            "text/markdown",
            "{spec-folder}/SPEC.md",
            MethodEvidenceClass::Authoritative,
            "sha256:db24d958e193c232022080224408e928ac2633b9c6dbbc505c45723685cc9326",
        ),
        (
            "expectation_01J00000000000000000000010",
            "document",
            true,
            "text/markdown",
            "{spec-folder}/.memlog.md",
            MethodEvidenceClass::Authoritative,
            "sha256:fdc0e62fddba6c7bd31b348439ec648c6f731e1195d1ba4989562b13d1ca3e75",
        ),
        (
            "expectation_01J00000000000000000000011",
            "file_set",
            false,
            "text/markdown",
            "{spec-folder}/<content-typed-companion>.md",
            MethodEvidenceClass::Authoritative,
            "sha256:0f264d8e356163fd317bb580e471a5a3bef8474f6429eed0738b7d2a44341eb0",
        ),
        (
            "expectation_01J00000000000000000000012",
            "file_set",
            false,
            "application/yaml",
            "{spec-folder}/stories.yaml (interactive Story Breakdown only)",
            MethodEvidenceClass::Contextual,
            "sha256:4052e3c8ba1d144ff19a6c378ead08a72ca455bbf3da54231be1e3e675a39825",
        ),
    ]
    .into_iter()
    .map(
        |(
            expectation_id,
            artifact_kind,
            required,
            expected_media_type,
            source_output_hint,
            completion_evidence_class,
            expectation_hash,
        )| {
            MethodArtifactExpectation::from_source(MethodArtifactExpectation {
                expectation_kind: "method_artifact".to_owned(),
                expectation_id: id(expectation_id),
                artifact_kind: artifact_kind.to_owned(),
                required,
                storage_scope: "app_local".to_owned(),
                expected_media_type: expected_media_type.to_owned(),
                expected_content_schema_hash: None,
                source_output_hint: Some(source_output_hint.to_owned()),
                completion_evidence_class,
                expectation_hash: digest(expectation_hash),
            })
            .expect("source-derived Spec artifact expectation")
        },
    )
    .collect()
}

fn spec_request(mode: BmadInvocationMode) -> BmadSkillExecutionRequest {
    let descriptor = BmadSpecSkill.descriptor();
    let resources =
        BmadResourceSet::new(descriptor.declared_resources().to_vec()).expect("closed resources");
    let execution_profile = spec_execution_profile();
    let egress_profile_hash = test_digest("Spec test egress profile");
    let model_binding = MethodModelBinding::from_source(
        MethodModelBindingData {
            binding_kind: "method_model".to_owned(),
            provider_id: "test-provider".to_owned(),
            model_id: "test-model".to_owned(),
            deployment_id: "test-deployment".to_owned(),
            model_profile_hash: test_digest("Spec model profile"),
            model_capability_hash: test_digest("Spec model capability"),
            context_window_profile_hash: test_digest("Spec context window"),
            egress_profile_hash,
            request_schema_hash: test_digest("Spec request schema"),
            response_schema_hash: test_digest("Spec response schema"),
        },
        test_digest("Spec model binding"),
    )
    .expect("valid inert model binding");
    let resolved_config_graph_hash = test_digest("Spec config graph");
    let resolved_config_hash = test_digest("Spec config resolution");
    let resolved_customization_hash = test_digest("Spec customization");
    let exact_binding = MethodExactBinding {
        capability_key: BmadCapabilityKey {
            package_version_id: id(
                "pkgver_8B33C55A4D67D0B258FEDBB75D1CB09DBC7F711BC9BDC794D8B052B31FCE6D86",
            ),
            module_code: "bmm".to_owned(),
            skill_name: "bmad-spec".to_owned(),
            normalized_action: None,
        },
        invocation_mode: mode,
        package_descriptor_hash: digest(
            "sha256:23280a02c31af2f9da1c3991278d542e796b075e198101e19dbbb92f7ea74e1e",
        ),
        package_source_hash: digest(
            "sha256:8b33c55a4d67d0b258fedbb75d1cb09dbc7f711bc9bdc794d8b052b31fce6d86",
        ),
        instruction_projection_hash: digest(
            "sha256:884facf72c30139829c9d1ec3cd63bcd651126abfecb7b5db2da4ed21af591a9",
        ),
        capability_catalog_hash: digest(
            "sha256:befeeeafad7a379eec5ab96f945b13e5cb239bc3531a7a281fe16a25b839315d",
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
            .expect("closed Spec resource-set hash"),
        model_binding_hash: model_binding.binding_hash,
        model_binding,
        method_schema_hash: digest(
            "sha256:dda02f453dcf4edbb3fc66e0ad3a7c59d7087f5b44b526e40c435456c8c248c2",
        ),
        egress_profile_hash,
        artifact_expectations: artifact_expectations()
            .into_iter()
            .filter(|expectation| {
                mode == BmadInvocationMode::Interactive
                    || expectation.expectation_id.as_str()
                        != "expectation_01J00000000000000000000012"
            })
            .collect(),
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
        let stored = sessions
            .iter_mut()
            .find(|session| session.scope() == *scope && session.session_id() == *session_id)
            .ok_or(RepositoryError)?;
        if stored.current_binding().map_err(|_| RepositoryError)? != observed_binding {
            return Err(RepositoryError);
        }
        stored.begin_advance(request).map_err(|_| RepositoryError)
    }

    fn persist_method_transition(
        &self,
        session: &MethodSession,
        _: u64,
        _: MethodPersistenceEvent,
    ) -> Result<(), Self::Error> {
        let mut sessions = self.sessions.lock().map_err(|_| RepositoryError)?;
        let stored = sessions
            .iter_mut()
            .find(|stored| stored.session_id() == session.session_id())
            .ok_or(RepositoryError)?;
        *stored = session.clone();
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

fn creation(suffix: &str) -> CreateMethodSession {
    CreateMethodSession {
        session_id: id(&format!("session_bmad_spec_{suffix}")),
        owner_scope_ref: id("ownerscope_bmad_spec"),
        project_id: id("project_bmad_spec"),
        run_id: id(&format!("run_bmad_spec_{suffix}")),
        authority_ref: AuthorityRef {
            authority_kind: "desktop_local_store".to_owned(),
            authority_id: id("authority_bmad_spec"),
            installation_id: id("install_bmad_spec"),
            local_store_id: id("store_bmad_spec"),
            authority_epoch: 1,
        },
        created_at: UnixMillis(1_000),
    }
}

fn review(binding: &MethodExactBinding, suffix: &str) -> MethodContextDecision {
    MethodContextDecision {
        decision_id: id(&format!("decision_bmad_spec_{suffix}")),
        manifest_hash: test_digest("Spec reviewed manifest"),
        consent_hash: test_digest("Spec reviewed consent"),
        context_digest: test_digest("Spec reviewed context"),
        binding_hash: binding.binding_hash().expect("Spec exact binding hash"),
        reviewed_at: UnixMillis(2_000),
    }
}

fn advance_request(
    session: &MethodSession,
    decision: &MethodContextDecision,
    suffix: &str,
) -> MethodAdvanceRequest {
    let d2_binding = test_digest(&format!("Spec D2 binding {suffix}"));
    MethodAdvanceRequest {
        invocation_id: id(&format!("invoke_bmad_spec_{suffix}")),
        idempotency_key: format!("bmad-spec-{suffix}-0001"),
        decision_id: decision.decision_id.clone(),
        decision_consumption_hash: test_digest(&format!("Spec consumption {suffix}")),
        model_request_id: id(&format!("modelreq_bmad_spec_{suffix}")),
        model_request_hash: test_digest(&format!("Spec request {suffix}")),
        session_authority_hash: session
            .session_authority_hash()
            .expect("Spec session authority"),
        d2_model_invocation_binding_hash: d2_binding,
        model_bridge_binding_hash: session
            .model_bridge_binding_hash(&d2_binding)
            .expect("Spec bridge binding"),
        expected_version: session.version(),
    }
}

// Break caught: omitting any pinned member or changing its declared execution
// shape makes source closure incomplete before generic preparation.
#[test]
fn bmad_spec_descriptor_closes_the_pinned_skill_and_execution_shape() {
    let descriptor = BmadSpecSkill.descriptor();
    assert_eq!(descriptor.canonical_id(), "bmm:bmad-spec");
    assert_eq!(
        descriptor.source_path(),
        "src/bmm-skills/2-plan-workflows/bmad-spec/SKILL.md"
    );
    assert!(descriptor.declared_actions().is_empty());
    assert_eq!(
        descriptor.supported_modes(),
        [
            BmadInvocationMode::Interactive,
            BmadInvocationMode::Headless
        ]
    );
    assert_eq!(descriptor.predecessor(), None);
    assert_eq!(descriptor.follower(), None);
    assert_eq!(descriptor.output_locations(), ["output_folder"]);
    assert_eq!(
        descriptor.artifact_kinds(),
        [BmadArtifactKind::Document, BmadArtifactKind::FileSet]
    );
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
            "src/bmm-skills/2-plan-workflows/bmad-spec/SKILL.md",
            "src/bmm-skills/2-plan-workflows/bmad-spec/customize.toml",
            "src/bmm-skills/2-plan-workflows/bmad-spec/assets/headless-schemas.md",
            "src/bmm-skills/2-plan-workflows/bmad-spec/assets/spec-template.md",
            "src/bmm-skills/2-plan-workflows/bmad-spec/assets/stories-schema.md",
        ]
    );
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
            "{}",
            resource.path()
        );
    }
}

// Break caught: a thin adapter that imports an effect-capable API would let
// pinned prose bypass the generic runtime's typed plans and Airlock.
#[test]
fn bmad_spec_adapter_has_no_direct_effect_authority() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/bmad/skills/adapters/bmad_spec.rs"),
    )
    .expect("Spec adapter source");
    for (index, line) in source.lines().enumerate() {
        let code = line.split("//").next().unwrap_or_default();
        for token in [
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
                !code.contains(token),
                "bmad_spec.rs line {} references forbidden capability {token:?}",
                index + 1
            );
        }
    }
    assert!(source.contains(SKILL_ROOT));
}

// Break caught: a descriptor-only adapter can be advertised while no generic
// Method execution contract exists, leaving the canonical capability inert.
#[test]
fn bmad_spec_exposes_an_explicit_generic_execution_contract() {
    let contract = BmadSpecSkill
        .execution_contract()
        .expect("the canonical Spec adapter must opt into generic Method execution");
    assert_eq!(contract.package_name(), "bmad-method");
    assert_eq!(contract.package_version(), "6.10.0");
}

// Break caught: mode parity can be claimed from fixtures while one mode does
// not prepare the same required outputs, typed intents, and source stages.
#[test]
fn bmad_spec_prepares_equivalent_required_outputs_in_both_modes() {
    let runtime = BmadDomainRuntime;
    let mut observations = Vec::new();
    for mode in [
        BmadInvocationMode::Interactive,
        BmadInvocationMode::Headless,
    ] {
        let prepared = runtime
            .prepare_skill_execution(&BmadSpecSkill, spec_request(mode))
            .expect("canonical Spec exact binding prepares");
        assert_eq!(prepared.descriptor().canonical_id(), "bmm:bmad-spec");
        assert_eq!(prepared.mode(), mode);
        assert_eq!(prepared.action(), None);
        assert_eq!(prepared.resources().resources().len(), 5);
        assert_eq!(
            prepared.tool_intents(),
            [
                BmadSkillToolIntent::FileRead,
                BmadSkillToolIntent::FileWrite,
                BmadSkillToolIntent::Process,
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
                BmadActionPlanKind::ModelCall,
            ]
        );
        assert!(prepared
            .authority_requirements()
            .contains(&BmadSkillAuthorityRequirement::ExternalHandoff));
        assert_eq!(
            prepared.recovery(),
            BmadSkillRecoverySemantics::MethodCheckpoint
        );
        observations.push((
            prepared.step_table().clone(),
            prepared
                .artifact_expectations()
                .iter()
                .filter(|expectation| expectation.required)
                .cloned()
                .collect::<Vec<_>>(),
        ));
    }
    assert_eq!(observations[0], observations[1]);
    assert_eq!(
        observations[0]
            .1
            .iter()
            .filter(|expectation| expectation.required)
            .map(|expectation| expectation.source_output_hint.as_deref())
            .collect::<Vec<_>>(),
        [
            Some("{spec-folder}/SPEC.md"),
            Some("{spec-folder}/.memlog.md")
        ]
    );
}

// Break caught: preparation without durable service dispatch leaves restart,
// cancellation, failure, retry, repair, resume, and upgrade as fixture prose.
#[test]
fn bmad_spec_dispatches_into_the_durable_method_service_and_recovers_deterministically() {
    let repository = Repository::default();
    let runtime = BmadDomainRuntime;
    let service = MethodSessionService::new(&repository);
    let dispatched = runtime
        .dispatch_skill_execution(
            &service,
            &BmadSpecSkill,
            spec_request(BmadInvocationMode::Interactive),
            creation("primary"),
        )
        .expect("canonical dispatch prepares and durably binds");
    let prepared = dispatched.prepared();
    let bound = dispatched.session();
    assert_eq!(bound.state(), MethodState::CapabilityBound);

    let independently_prepared = runtime
        .prepare_skill_execution(
            &BmadSpecSkill,
            spec_request(BmadInvocationMode::Interactive),
        )
        .expect("Spec preparation");
    assert_eq!(
        independently_prepared.exact_binding(),
        prepared.exact_binding()
    );

    let persisted = bound.to_persisted_json().expect("persisted bound state");
    let restarted = MethodSession::from_persisted_json(&persisted).expect("restart");
    assert_eq!(&restarted, bound);
    assert_eq!(
        service
            .resume(&bound.scope(), &bound.session_id())
            .expect("resume"),
        bound.clone()
    );

    let cancelled = service
        .cancel(&bound.scope(), &bound.session_id(), bound.version())
        .expect("cancel");
    assert_eq!(cancelled.state(), MethodState::Cancelled);

    let failed_dispatch = runtime
        .dispatch_skill_execution(
            &service,
            &BmadSpecSkill,
            spec_request(BmadInvocationMode::Headless),
            creation("failure"),
        )
        .expect("failure-path dispatch");
    let failed_review_required = service
        .request_context_review(
            &failed_dispatch.session().scope(),
            &failed_dispatch.session().session_id(),
            failed_dispatch.session().version(),
        )
        .expect("failure-path review request");
    let failed_decision = review(failed_dispatch.prepared().exact_binding(), "failure");
    let failed_ready = service
        .record_context_review(
            &failed_review_required.scope(),
            &failed_review_required.session_id(),
            failed_review_required.version(),
            failed_decision.clone(),
        )
        .expect("failure-path review");
    let (failed_advancing, _) = service
        .begin_advance(
            &failed_ready.scope(),
            &failed_ready.session_id(),
            failed_dispatch.prepared().exact_binding(),
            advance_request(&failed_ready, &failed_decision, "failure"),
        )
        .expect("failure-path advance");
    let failed = service
        .record_incomplete(
            &failed_advancing.scope(),
            &failed_advancing.session_id(),
            failed_advancing.version(),
        )
        .expect("incomplete failure is durable after a reviewed advance");
    assert_eq!(failed.state(), MethodState::Incomplete);
    assert_eq!(
        service
            .resume(&failed.scope(), &failed.session_id())
            .expect("failed attempt resumes at its terminal durable boundary"),
        failed
    );

    let retry_dispatch = runtime
        .dispatch_skill_execution(
            &service,
            &BmadSpecSkill,
            spec_request(BmadInvocationMode::Headless),
            creation("retry"),
        )
        .expect("retry starts a fresh exact durable session");
    let retry_bound = retry_dispatch.session();
    assert_eq!(retry_bound.state(), MethodState::CapabilityBound);

    let mut upgraded_binding = retry_dispatch.prepared().exact_binding().clone();
    upgraded_binding.capability_catalog_hash = test_digest("upgraded Spec catalog");
    let repaired = service
        .rebind_invocation(
            &retry_bound.scope(),
            &retry_bound.session_id(),
            retry_bound.version(),
            upgraded_binding,
            prepared.step_table().clone(),
        )
        .expect("repair and stale-source upgrade rebind");
    assert_eq!(repaired.state(), MethodState::CapabilityBound);
    let repair_review = service
        .request_context_review(
            &repaired.scope(),
            &repaired.session_id(),
            repaired.version(),
        )
        .expect("repair requires a fresh exact review");
    assert_eq!(repair_review.state(), MethodState::ContextReviewRequired);
    assert_eq!(repository.sessions().len(), 3);
}
