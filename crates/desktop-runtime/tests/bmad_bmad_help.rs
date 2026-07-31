//! Source-derived conformance tests for the canonical `bmad-help` adapter.

#![allow(clippy::expect_used, clippy::panic)]

use std::sync::Mutex;

use desktop_runtime::generated_contracts::{
    BmadActionPlanKind, BmadInvocationMode, BmadProblemFailureClass,
};
use desktop_runtime::{
    build_problem, sha256_bytes, AuthorityRef, BmadCapabilityKey, BmadDomainRuntime,
    BmadFailureCancellation, BmadFailureOperation, BmadFailureRequest, BmadHelpSkill,
    BmadMethodSkill, BmadResourceSet, BmadSkillExecutionContract, BmadSkillExecutionRequest,
    BmadSkillRecoverySemantics, BmadSkillSupportState, ContractId, CreateMethodSession,
    MethodAdvanceDisposition, MethodAdvanceReceipt, MethodAdvanceRequest, MethodArtifactProvenance,
    MethodExactBinding, MethodExecutionProfile, MethodExecutionProfileData, MethodInvocationModes,
    MethodModelBinding, MethodModelBindingData, MethodPersistenceEvent, MethodResourcePolicy,
    MethodRuntimeRequirement, MethodSession, MethodSessionRepository, MethodSessionScope,
    MethodSessionService, MethodState, Sha256Digest, UnixMillis,
};

const SKILL_ROOT: &str = "src/core-skills/bmad-help";

fn id(value: &str) -> ContractId {
    ContractId::new(value).expect("test identifier")
}

fn digest(value: &str) -> Sha256Digest {
    Sha256Digest::parse(value).expect("pinned digest")
}

#[derive(Debug, thiserror::Error)]
#[error("Help repository failure")]
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
    let session_id = match mode {
        BmadInvocationMode::Interactive => "session_01J00000000000000000000040",
        BmadInvocationMode::Headless => "session_01J00000000000000000000041",
    };
    CreateMethodSession {
        session_id: id(session_id),
        owner_scope_ref: id("owner_01J00000000000000000000040"),
        project_id: id("project_01J00000000000000000000040"),
        run_id: id("run_01J00000000000000000000040"),
        authority_ref: AuthorityRef {
            authority_kind: "desktop_local_store".to_owned(),
            authority_id: id("authority_01J00000000000000000000040"),
            installation_id: id("installation_01J00000000000000000040"),
            local_store_id: id("store_01J000000000000000000000040"),
            authority_epoch: 1,
        },
        created_at: UnixMillis(1_000),
    }
}

fn help_request(mode: BmadInvocationMode) -> BmadSkillExecutionRequest {
    let descriptor = BmadHelpSkill.descriptor();
    let resources =
        BmadResourceSet::new(descriptor.declared_resources().to_vec()).expect("Help closure");
    let profile = MethodExecutionProfile::from_source(
        MethodExecutionProfileData {
            entrypoint_kind: "direct".to_owned(),
            invocation_modes: MethodInvocationModes {
                interactive: true,
                headless: true,
                actions: Vec::new(),
            },
            required_runtimes: vec![MethodRuntimeRequirement {
                runtime: "node".to_owned(),
                version_range: ">=20.12.0".to_owned(),
                required: true,
            }],
            resource_policy: MethodResourcePolicy {
                entrypoint_timing: "invocation_start".to_owned(),
                resource_timing: "all_declared_at_start".to_owned(),
                declared_resource_paths: descriptor
                    .declared_resources()
                    .iter()
                    .map(|resource| resource.path().to_owned())
                    .collect(),
            },
            declared_tool_intents: vec![
                "file_read".to_owned(),
                "process".to_owned(),
                "web".to_owned(),
                "subagent".to_owned(),
            ],
            state_hints: Vec::new(),
            completion_evidence: vec!["event".to_owned(), "status_evidence".to_owned()],
            customization_profile: "method_skill_toml".to_owned(),
            validation_profile: "MethodOfficialSkillV6".to_owned(),
        },
        digest("sha256:eab6bd03e6ec5fa86f52f2386ac504979f8159352b09896ee14221ac15a23245"),
    )
    .expect("Help execution profile");
    let model_binding = MethodModelBinding::from_source(
        MethodModelBindingData {
            binding_kind: "method_model".to_owned(),
            provider_id: "test-provider".to_owned(),
            model_id: "test-model".to_owned(),
            deployment_id: "test-deployment".to_owned(),
            model_profile_hash: sha256_bytes(b"model profile"),
            model_capability_hash: sha256_bytes(b"model capability"),
            context_window_profile_hash: sha256_bytes(b"context window"),
            egress_profile_hash: sha256_bytes(b"egress"),
            request_schema_hash: sha256_bytes(b"request schema"),
            response_schema_hash: sha256_bytes(b"response schema"),
        },
        sha256_bytes(b"model binding"),
    )
    .expect("model binding");
    let config_graph_hash = sha256_bytes(b"config graph");
    let config_resolution_hash = sha256_bytes(b"config resolution");
    let customization_hash = sha256_bytes(b"customization");
    BmadSkillExecutionRequest {
        mode,
        action: None,
        exact_binding: MethodExactBinding {
            capability_key: BmadCapabilityKey {
                package_version_id: id(
                    "pkgver_8B33C55A4D67D0B258FEDBB75D1CB09DBC7F711BC9BDC794D8B052B31FCE6D86",
                ),
                module_code: "core".to_owned(),
                skill_name: "bmad-help".to_owned(),
                normalized_action: None,
            },
            invocation_mode: mode,
            package_descriptor_hash: digest(
                "sha256:2ec79c3da90bfc30030f15d902d776202c7ac91ed7cf7dc8fa8adbf85cb9cf6a",
            ),
            package_source_hash: digest(
                "sha256:8b33c55a4d67d0b258fedbb75d1cb09dbc7f711bc9bdc794d8b052b31fce6d86",
            ),
            instruction_projection_hash: digest(
                "sha256:483d33cdeaea3ce95f48741380190baf2a85bdb40007d487f23c7c67b56567e5",
            ),
            capability_catalog_hash: digest(
                "sha256:f9d8ddce98c4e084049de94d78cef2e2d9018cff7ffeb639aafc80326f346dfd",
            ),
            agent_roster_hash: None,
            agent_binding_hash: None,
            agent_binding: None,
            distribution_profile: "sapphirus_package".to_owned(),
            install_profile: "SapphirusManagedV1".to_owned(),
            entrypoint_kind: "direct".to_owned(),
            execution_profile_hash: profile.profile_hash,
            execution_profile: profile,
            validation_profile: "MethodOfficialSkillV6".to_owned(),
            validation_profile_hash: digest(
                "sha256:ddd086622be73b637cdcb3562b4459ac3853f1aeae34a53c43775af66e4cbdf0",
            ),
            config_graph_hash,
            config_resolution_hash,
            customization_hash,
            resource_set_hash: BmadSkillExecutionContract::resource_set_hash(&resources)
                .expect("resource hash"),
            model_binding_hash: model_binding.binding_hash,
            model_binding,
            method_schema_hash: digest(
                "sha256:600432affbca9baec428903211df9c4c2a1fc6ba595959013236e1a0f16bc746",
            ),
            egress_profile_hash: sha256_bytes(b"egress"),
            artifact_expectations: Vec::new(),
        },
        resources,
        resolved_config_graph_hash: config_graph_hash,
        resolved_config_hash: config_resolution_hash,
        resolved_customization_hash: customization_hash,
    }
}

// Break caught: retaining only the Help entrypoint while omitting the four
// catalog/module members used by the sealed Help projection leaves runtime
// guidance detached from the pinned Method source closure.
#[test]
fn bmad_help_descriptor_closes_the_five_pinned_source_members() {
    let descriptor = BmadHelpSkill.descriptor();

    assert_eq!(descriptor.canonical_id(), "core:bmad-help");
    assert_eq!(
        descriptor.source_path(),
        "src/core-skills/bmad-help/SKILL.md"
    );
    assert_eq!(
        descriptor.supported_modes(),
        [
            BmadInvocationMode::Interactive,
            BmadInvocationMode::Headless
        ]
    );
    assert_eq!(descriptor.support_state(), BmadSkillSupportState::Adapted);
    assert_eq!(
        descriptor
            .declared_resources()
            .iter()
            .map(desktop_runtime::BmadResource::path)
            .collect::<Vec<_>>(),
        [
            "src/core-skills/bmad-help/SKILL.md",
            "src/bmm-skills/module-help.csv",
            "src/bmm-skills/module.yaml",
            "src/core-skills/module-help.csv",
            "src/core-skills/module.yaml",
        ]
    );
    for resource in descriptor.declared_resources() {
        let resource_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/bmad-foundation/source/method/6.10.0")
            .join(resource.path());
        let bytes = std::fs::read(&resource_path).unwrap_or_else(|error| {
            panic!(
                "could not read pinned Help resource {}: {error}",
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

// Break caught: a descriptor-only Help adapter can be visible in catalog and
// UI projections while production has no generic execution contract to
// prepare before the durable Method session lifecycle.
#[test]
fn bmad_help_exposes_an_explicit_generic_execution_contract() {
    let contract = BmadHelpSkill
        .execution_contract()
        .expect("canonical Help must opt into generic Method execution");

    assert_eq!(contract.package_name(), "bmad-method");
    assert_eq!(contract.package_version(), "6.10.0");
}

#[test]
fn bmad_help_prepares_both_modes_and_persists_the_established_method_lifecycle() {
    let repository = Repository::default();
    let service = MethodSessionService::new(&repository);

    for mode in [
        BmadInvocationMode::Interactive,
        BmadInvocationMode::Headless,
    ] {
        let dispatched = BmadDomainRuntime
            .dispatch_skill_execution(&service, &BmadHelpSkill, help_request(mode), creation(mode))
            .expect("Help dispatches through the generic runtime and durable Method service");
        let prepared = dispatched.prepared();
        assert_eq!(prepared.mode(), mode);
        assert_eq!(
            prepared.required_plan_kinds(),
            [
                BmadActionPlanKind::Read,
                BmadActionPlanKind::CommandProposal,
                BmadActionPlanKind::ModelCall,
            ]
        );
        assert_eq!(
            prepared.recovery(),
            BmadSkillRecoverySemantics::MethodCheckpoint
        );

        assert_eq!(dispatched.session().state(), MethodState::CapabilityBound);
        let restarted = service
            .resume(
                &dispatched.session().scope(),
                &dispatched.session().session_id(),
            )
            .expect("restart resumes the persisted CapabilityBound session");
        assert_eq!(restarted, *dispatched.session());

        let rebound = service
            .rebind_invocation(
                &restarted.scope(),
                &restarted.session_id(),
                restarted.version(),
                prepared.exact_binding().clone(),
                prepared.step_table().clone(),
            )
            .expect("stale upgrade rebind");
        let cancelled = service
            .cancel(&rebound.scope(), &rebound.session_id(), rebound.version())
            .expect("durable cancel");
        assert_eq!(cancelled.state(), MethodState::Cancelled);
    }
}

#[test]
fn bmad_help_failure_receipts_execute_retry_repair_and_cancel_branches() {
    let request = |class, cancellation| BmadFailureRequest {
        session_id: "session_01J00000000000000000000040".to_owned(),
        problem_id: "problem_01J00000000000000000000040".to_owned(),
        code: "bmad_help_failed".to_owned(),
        detail: Some("source-derived Help lifecycle fixture".to_owned()),
        failure_class: class,
        operation: BmadFailureOperation {
            session_id: "session_01J00000000000000000000040".to_owned(),
            step_id: "recommend".to_owned(),
            input_hash: sha256_bytes(b"Help input"),
        },
        attempt: 1,
        max_attempts: 2,
        drafts_preserved: true,
        accepted_inputs_preserved: true,
        state_hash: sha256_bytes(b"Help state"),
        cancellation,
    };
    let retry = build_problem(&request(BmadProblemFailureClass::Model, None))
        .expect("transient Help failure");
    assert!(retry.retry.eligible);
    let repair =
        build_problem(&request(BmadProblemFailureClass::Source, None)).expect("source repair");
    assert!(!repair.retry.eligible);
    let cancelled = build_problem(&request(
        BmadProblemFailureClass::Cancel,
        Some(BmadFailureCancellation {
            requested_at_sequence: 4,
            acknowledged: true,
            acknowledged_at_sequence: Some(5),
        }),
    ))
    .expect("cancel receipt");
    assert!(cancelled.cancellation.is_some());
}

// Break caught: adding direct I/O or provider access to this adapter would let
// pinned prose grant authority instead of producing typed inert plans.
#[test]
fn bmad_help_adapter_remains_a_thin_authority_boundary() {
    let source_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/bmad/skills/adapters/bmad_help.rs");
    let source = std::fs::read_to_string(&source_path)
        .unwrap_or_else(|error| panic!("{}: {error}", source_path.display()));
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
                "bmad_help.rs line {} references forbidden capability {token:?}: {line:?}",
                index + 1,
            );
        }
    }
    assert!(source.contains(SKILL_ROOT));
}
