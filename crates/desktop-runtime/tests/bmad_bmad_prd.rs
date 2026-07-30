//! Source-derived conformance tests for the canonical `bmad-prd` adapter.

#![allow(clippy::expect_used, clippy::panic)]

use desktop_runtime::{
    generated_contracts::{
        BmadActionPlanKind, BmadArtifactCompletionHookRunHook, BmadArtifactKind,
        BmadInvocationDeclaredActionName, BmadInvocationMode,
    },
    sha256_bytes, BmadDomainRuntime, BmadMethodSkill, BmadPrdSkill, BmadRuntimePort,
    BmadSkillAuthorityRequirement, BmadSkillExecutionBindingInput, BmadSkillExecutionRequest,
    BmadSkillRecoverySemantics, BmadSkillSupportState, BmadSkillToolIntent, ContractId,
    MethodArtifactExpectation, MethodEvidenceClass, MethodModelBinding, MethodModelBindingData,
    Sha256Digest,
};

const SKILL_ROOT: &str = "src/bmm-skills/2-plan-workflows/bmad-prd";

fn contract_id(value: &str) -> ContractId {
    ContractId::new(value).expect("canonical test identifier")
}

fn test_digest(label: &str) -> Sha256Digest {
    sha256_bytes(label.as_bytes())
}

fn prd_artifact_expectation(
    expectation_id: &str,
    expected_media_type: &str,
    output_hint: &str,
) -> MethodArtifactExpectation {
    MethodArtifactExpectation::from_source(MethodArtifactExpectation {
        expectation_kind: "method_artifact".to_owned(),
        expectation_id: contract_id(expectation_id),
        artifact_kind: "document".to_owned(),
        required: true,
        storage_scope: "app_local".to_owned(),
        expected_media_type: expected_media_type.to_owned(),
        expected_content_schema_hash: None,
        source_output_hint: Some(output_hint.to_owned()),
        completion_evidence_class: MethodEvidenceClass::Authoritative,
        expectation_hash: test_digest(expectation_id),
    })
    .expect("source-derived PRD artifact expectation")
}

fn prd_request(mode: BmadInvocationMode, action: &str) -> BmadSkillExecutionRequest {
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
    BmadPrdSkill
        .execution_contract()
        .expect("PRD contract")
        .execution_request(BmadSkillExecutionBindingInput {
            mode,
            action: action.to_owned(),
            model_binding,
            resolved_config_graph_hash: test_digest("test config graph"),
            resolved_config_hash: test_digest("test config resolution"),
            resolved_customization_hash: test_digest("test customization"),
            egress_profile_hash,
        })
        .expect("source-derived PRD invocation request")
}

fn expected_artifacts(action: &str) -> Vec<MethodArtifactExpectation> {
    match action {
        "create" | "update" => vec![
            prd_artifact_expectation(
                "expectation_01J00000000000000000000008",
                "text/markdown",
                "{doc_workspace}/prd.md",
            ),
            prd_artifact_expectation(
                "expectation_01J00000000000000000000009",
                "text/markdown",
                "{doc_workspace}/.memlog.md",
            ),
        ],
        "validate" => vec![
            prd_artifact_expectation(
                "expectation_01J00000000000000000000010",
                "text/markdown",
                "{doc_workspace}/validation-report.md",
            ),
            prd_artifact_expectation(
                "expectation_01J00000000000000000000011",
                "text/html",
                "{doc_workspace}/validation-report.html",
            ),
        ],
        _ => panic!("unexpected PRD action"),
    }
}

fn expected_steps(action: &str) -> Vec<&'static str> {
    match action {
        "create" => vec![
            "activation",
            "create_discovery",
            "reviewer_gate",
            "finalize",
        ],
        "update" => vec![
            "activation",
            "update_reconcile",
            "reviewer_gate",
            "finalize",
        ],
        "validate" => vec!["activation", "validate_reviewer_gate", "validation_report"],
        _ => panic!("unexpected PRD action"),
    }
}

fn action_names() -> Vec<String> {
    BmadPrdSkill
        .descriptor()
        .declared_actions()
        .iter()
        .map(|action| action.name.to_string())
        .collect()
}

fn assert_action_name_contract_compiles() {
    for action in ["create", "update", "validate"] {
        let _: BmadInvocationDeclaredActionName = action.try_into().expect("declared PRD action");
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
    assert_action_name_contract_compiles();
    assert_eq!(action_names(), ["create", "update", "validate"]);
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

    for action in ["create", "update", "validate"] {
        for mode in [
            BmadInvocationMode::Interactive,
            BmadInvocationMode::Headless,
        ] {
            let prepared = runtime
                .prepare_skill_execution(&BmadPrdSkill, prd_request(mode, action))
                .expect("canonical PRD action binding prepares");

            assert_eq!(prepared.descriptor().canonical_id(), "bmm:bmad-prd");
            assert_eq!(prepared.mode(), mode);
            assert_eq!(prepared.action(), Some(action));
            assert_eq!(prepared.resources().resources().len(), 8);
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
                prepared.authority_requirements(),
                [
                    BmadSkillAuthorityRequirement::ActionPlan(BmadActionPlanKind::Read),
                    BmadSkillAuthorityRequirement::ActionPlan(BmadActionPlanKind::WriteProposal),
                    BmadSkillAuthorityRequirement::ActionPlan(BmadActionPlanKind::CommandProposal,),
                    BmadSkillAuthorityRequirement::WebAccess,
                    BmadSkillAuthorityRequirement::ActionPlan(BmadActionPlanKind::ModelCall),
                    BmadSkillAuthorityRequirement::BrowserControl,
                    BmadSkillAuthorityRequirement::ExternalHandoff,
                ]
            );
            assert_eq!(
                prepared.recovery(),
                BmadSkillRecoverySemantics::MethodCheckpoint
            );
            assert_eq!(prepared.artifact_expectations(), expected_artifacts(action));
            let expected_steps = expected_steps(action);
            assert_eq!(
                prepared
                    .workflow_steps()
                    .iter()
                    .map(|step| step.step_key.as_str())
                    .collect::<Vec<_>>(),
                expected_steps,
                "{action}/{mode:?} must retain the source-derived branch"
            );
            if mode == BmadInvocationMode::Headless {
                assert_eq!(
                    prepared.required_status_fields(),
                    ["status", "intent", "assumptions", "open_questions"]
                );
            } else {
                assert!(prepared.required_status_fields().is_empty());
            }
        }
    }
}
