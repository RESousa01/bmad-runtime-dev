//! Source-derived conformance tests for the canonical `bmad-prd` adapter.

#![allow(clippy::expect_used, clippy::panic)]

use desktop_runtime::{
    generated_contracts::{
        BmadActionPlanKind, BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationMode,
    },
    sha256_bytes, BmadCapabilityKey, BmadDomainRuntime, BmadMethodSkill, BmadPrdSkill,
    BmadResourceSet, BmadRuntimePort, BmadSkillExecutionContract, BmadSkillExecutionRequest,
    BmadSkillRecoverySemantics, BmadSkillSupportState, BmadSkillToolIntent, ContractId,
    MethodArtifactExpectation, MethodEvidenceClass, MethodExactBinding, MethodExecutionProfile,
    MethodExecutionProfileData, MethodInvocationModes, MethodModelBinding, MethodModelBindingData,
    MethodResourcePolicy, Sha256Digest,
};

const SKILL_ROOT: &str = "src/bmm-skills/2-plan-workflows/bmad-prd";

fn contract_id(value: &str) -> ContractId {
    ContractId::new(value).expect("canonical test identifier")
}

fn digest(value: &str) -> Sha256Digest {
    Sha256Digest::parse(value).expect("canonical pinned digest")
}

fn test_digest(label: &str) -> Sha256Digest {
    sha256_bytes(label.as_bytes())
}

fn prd_execution_profile() -> MethodExecutionProfile {
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
                declared_resource_paths: BmadPrdSkill
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
    .expect("source-derived PRD execution profile")
}

fn prd_artifact_expectation() -> MethodArtifactExpectation {
    MethodArtifactExpectation::from_source(MethodArtifactExpectation {
        expectation_kind: "method_artifact".to_owned(),
        expectation_id: contract_id("expectation_01J00000000000000000000008"),
        artifact_kind: "document".to_owned(),
        required: true,
        storage_scope: "app_local".to_owned(),
        expected_media_type: "text/markdown".to_owned(),
        expected_content_schema_hash: None,
        source_output_hint: Some(
            "{doc_workspace}/prd.md or {doc_workspace}/validation-report.{md,html}".to_owned(),
        ),
        completion_evidence_class: MethodEvidenceClass::Authoritative,
        expectation_hash: digest(
            "sha256:3b263d0c06079b668d7a0b495a2a91b096f93084a5f529628be13c858f876c22",
        ),
    })
    .expect("source-derived PRD artifact expectation")
}

fn prd_request(mode: BmadInvocationMode) -> BmadSkillExecutionRequest {
    let descriptor = BmadPrdSkill.descriptor();
    let resources =
        BmadResourceSet::new(descriptor.declared_resources().to_vec()).expect("closed resources");
    let execution_profile = prd_execution_profile();
    let egress_profile_hash = test_digest("test egress profile");
    let model_binding = MethodModelBinding::from_source(
        MethodModelBindingData {
            binding_kind: "method_model".to_owned(),
            provider_id: "test-provider".to_owned(),
            model_id: "test-model".to_owned(),
            deployment_id: "test-deployment".to_owned(),
            model_profile_hash: test_digest("test model profile"),
            model_capability_hash: test_digest("test model capability"),
            context_window_profile_hash: test_digest("test context window"),
            egress_profile_hash,
            request_schema_hash: test_digest("test request schema"),
            response_schema_hash: test_digest("test response schema"),
        },
        test_digest("test model binding"),
    )
    .expect("valid inert model binding");
    let resolved_config_graph_hash = test_digest("test config graph");
    let resolved_config_hash = test_digest("test config resolution");
    let resolved_customization_hash = test_digest("test customization");
    let exact_binding = MethodExactBinding {
        capability_key: BmadCapabilityKey {
            package_version_id: contract_id(
                "pkgver_8B33C55A4D67D0B258FEDBB75D1CB09DBC7F711BC9BDC794D8B052B31FCE6D86",
            ),
            module_code: "bmm".to_owned(),
            skill_name: "bmad-prd".to_owned(),
            normalized_action: None,
        },
        package_descriptor_hash: digest(
            "sha256:b3d3a62bee24df5565de4182f4774765c3abb5d829e05e3c72f362ae823601bc",
        ),
        package_source_hash: digest(
            "sha256:8b33c55a4d67d0b258fedbb75d1cb09dbc7f711bc9bdc794d8b052b31fce6d86",
        ),
        instruction_projection_hash: digest(
            "sha256:020773a36111d798aaadbed06a1aaa40ae4c830140ebcd2b4f035245beb29d17",
        ),
        capability_catalog_hash: digest(
            "sha256:56e4a4583b6e35790afbe8f968b2cc5f78d6d8baee54ec5167a8bf4db03b1875",
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
            .expect("closed PRD resource-set hash"),
        model_binding_hash: model_binding.binding_hash,
        model_binding,
        method_schema_hash: digest(
            "sha256:dda02f453dcf4edbb3fc66e0ad3a7c59d7087f5b44b526e40c435456c8c248c2",
        ),
        egress_profile_hash,
        artifact_expectations: vec![prd_artifact_expectation()],
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

// Break caught: omitting a pinned `bmad-prd` member (or silently changing
// its declared execution shape) leaves the adapter unable to prove a closed,
// source-faithful package boundary before generic runtime execution begins.
#[test]
fn bmad_prd_descriptor_closes_the_pinned_skill_and_exposes_its_execution_shape() {
    let descriptor = BmadPrdSkill.descriptor();

    assert_eq!(descriptor.canonical_id(), "bmm:bmad-prd");
    assert_eq!(
        descriptor.source_path(),
        "src/bmm-skills/2-plan-workflows/bmad-prd/SKILL.md"
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
        [
            "src/bmm-skills/2-plan-workflows/bmad-prd/SKILL.md",
            "src/bmm-skills/2-plan-workflows/bmad-prd/customize.toml",
            "src/bmm-skills/2-plan-workflows/bmad-prd/assets/headless-schemas.md",
            "src/bmm-skills/2-plan-workflows/bmad-prd/assets/prd-template.md",
            "src/bmm-skills/2-plan-workflows/bmad-prd/assets/prd-validation-checklist.md",
            "src/bmm-skills/2-plan-workflows/bmad-prd/assets/validation-report-template.html",
            "src/bmm-skills/2-plan-workflows/bmad-prd/references/headless.md",
            "src/bmm-skills/2-plan-workflows/bmad-prd/references/validate.md",
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
            "{} must retain its pinned SHA-256 identity",
            resource.path()
        );
    }
}

// Break caught: adding a direct I/O, process, network, scheduler, or model
// dependency to this thin adapter would let pinned prose bypass the generic
// runtime's typed ports and Airlock policy.
#[test]
fn bmad_prd_adapter_is_a_descriptor_only_authority_boundary() {
    let source_result = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/bmad/skills/adapters/bmad_prd.rs"),
    );
    let source = source_result.unwrap_or_else(|error| panic!("bmad-prd adapter source: {error}"));
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
                "bmad_prd.rs line {} references forbidden capability {token:?}: {line:?}",
                index + 1,
            );
        }
    }
    assert!(source.contains(SKILL_ROOT));
}

// Break caught: treating a descriptor as a runnable PRD implementation when
// the generic runtime has no entry point that accepts the descriptor together
// with resolved configuration and resource closure. Without that binding, the
// runtime cannot execute or prove Create/Update/Validate semantics, turn the
// source's `uv` directives into typed plans, or attach PRD-specific durable
// recovery evidence.
#[test]
fn bmad_prd_requires_a_descriptor_bound_generic_run_interface() {
    let runtime_source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bmad/mod.rs"),
    )
    .unwrap_or_else(|error| panic!("BmadRuntimePort source: {error}"));
    let trait_source = runtime_source
        .split_once("pub trait BmadRuntimePort")
        .and_then(|(_, remainder)| remainder.split_once("pub struct BmadDomainRuntime"))
        .map_or("", |(trait_source, _)| trait_source);

    assert!(
        trait_source.contains("BmadSkillDescriptor"),
        "MTH-08 BLOCKED: BmadRuntimePort has no descriptor-bound run request. It only accepts prebuilt session/artifact intents, so bmad-prd cannot bind source resources, resolved config/customization, modeled Create/Update/Validate stages, typed action plans, or durable recovery to one generic execution. A shared runtime interface is required."
    );
}

// Break caught: declaring the PRD descriptor and its source closure without
// explicitly opting the adapter into the shared Method preparation bridge
// leaves the canonical capability visible but impossible to execute.
#[test]
fn bmad_prd_exposes_an_explicit_generic_execution_contract() {
    let contract = BmadPrdSkill
        .execution_contract()
        .expect("the canonical PRD adapter must opt into generic Method execution");

    assert_eq!(contract.package_name(), "bmad-method");
    assert_eq!(contract.package_version(), "6.10.0");
}

// Break caught: an adapter contract can validate in isolation while no exact
// host binding can actually prepare it. Both source-declared modes must cross
// the shared compiler and produce only inert plans for the existing Method
// service.
#[test]
fn bmad_prd_prepares_through_the_generic_runtime_in_both_declared_modes() {
    let runtime = BmadDomainRuntime;

    for mode in [
        BmadInvocationMode::Interactive,
        BmadInvocationMode::Headless,
    ] {
        let prepared = runtime
            .prepare_skill_execution(&BmadPrdSkill, prd_request(mode))
            .expect("canonical PRD exact binding prepares");

        assert_eq!(prepared.descriptor().canonical_id(), "bmm:bmad-prd");
        assert_eq!(prepared.mode(), mode);
        assert_eq!(prepared.action(), None);
        assert_eq!(prepared.resources().resources().len(), 8);
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
            prepared.required_plan_kinds(),
            [
                BmadActionPlanKind::Read,
                BmadActionPlanKind::WriteProposal,
                BmadActionPlanKind::CommandProposal,
                BmadActionPlanKind::Read,
                BmadActionPlanKind::ModelCall,
                BmadActionPlanKind::ArtifactPlan,
            ]
        );
        assert_eq!(
            prepared.recovery(),
            BmadSkillRecoverySemantics::MethodCheckpoint
        );
        assert_eq!(
            prepared.artifact_expectations(),
            [prd_artifact_expectation()]
        );
        assert!(prepared
            .step_table()
            .table_hash()
            .to_string()
            .starts_with("sha256:"));
    }
}
