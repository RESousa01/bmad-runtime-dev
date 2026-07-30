//! Canonical adapter for the pinned multi-lens `bmad-review` skill.
//!
//! The adapter is intentionally declarative. It closes every member owned by
//! the source subtree and describes the source's seven execution branches;
//! the shared compiler, Method session service, typed plans, and Airlock own
//! all execution and durable state.

use crate::generated_contracts::{
    BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationMode,
};
use crate::{
    BmadMethodSkill, BmadResource, BmadSkillDescriptor, BmadSkillDescriptorInput,
    BmadSkillExecutionContract, BmadSkillExecutionContractInput, BmadSkillExecutionError,
    BmadSkillExecutionStep, BmadSkillExecutionWorkflow, BmadSkillRecoverySemantics,
    BmadSkillSupportState, BmadSkillToolIntent, ContractId, MethodArtifactExpectation,
    MethodEvidenceClass, MethodExecutionProfile, MethodExecutionProfileData,
    MethodInvocationModes, MethodResourcePolicy, MethodRuntimeRequirement, Sha256Digest,
};

const SOURCE_PATH: &str = "src/core-skills/bmad-review/SKILL.md";

/// Adapter for MTH-42's canonical review implementation.
pub struct BmadReviewSkill;

impl BmadMethodSkill for BmadReviewSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "core:bmad-review".to_owned(),
            source_path: SOURCE_PATH.to_owned(),
            source_sha256: digest_bytes(
                "bc46cdf66b37b4d49d414bdc5afc54747a9b19b3fecae471d451b36959343ec3",
            ),
            declared_actions: Vec::new(),
            supported_modes: vec![
                BmadInvocationMode::Interactive,
                BmadInvocationMode::Headless,
            ],
            predecessor: None,
            follower: None,
            output_locations: vec!["report_path".to_owned()],
            artifact_kinds: vec![BmadArtifactKind::Report],
            completion_hooks: vec![BmadArtifactCompletionHookRunHook::PersistArtifacts],
            declared_resources: [
                (
                    "src/core-skills/bmad-review/SKILL.md",
                    "bc46cdf66b37b4d49d414bdc5afc54747a9b19b3fecae471d451b36959343ec3",
                ),
                (
                    "src/core-skills/bmad-review/customize.toml",
                    "38470c888a809ee030dc11d38868bb1ac5c1d547f06d218b711be5df53385c00",
                ),
                (
                    "src/core-skills/bmad-review/references/editorial-common.md",
                    "f0cdc72ea345b2c5dc3001477b3dfcc909ca11b78f7db8865b6763d18ffc66c8",
                ),
                (
                    "src/core-skills/bmad-review/references/lens-adversarial.md",
                    "b90f69ec58d15c43d286a60e0d4d36a49223ab466f351b34a309ddda449c99b2",
                ),
                (
                    "src/core-skills/bmad-review/references/lens-edge-case-hunter.md",
                    "f2709e3adb40943ea0f19d0797be379caf0875eaef9a55081dc1e170e8288b68",
                ),
                (
                    "src/core-skills/bmad-review/references/lens-prose.md",
                    "4cbcb1eaea8567d272f5db7c6cdfd74ed5a8aae6831053cb2f01cbfc2cb98b98",
                ),
                (
                    "src/core-skills/bmad-review/references/lens-structure.md",
                    "98c08221f517133e57ef6325013935360c3365ebd7dcb8411b2050d7df507ee3",
                ),
                (
                    "src/core-skills/bmad-review/references/lens-verification-gap.md",
                    "6a424589b2f95021467dd357874163c219158d5a0acdbeb0970ba9e61692f289",
                ),
                (
                    "src/core-skills/bmad-review/references/structure-models.md",
                    "f84f85070e1af75f2482757aba9d37d69d805a20826d185527e598f2ba6b9292",
                ),
                (
                    "src/core-skills/bmad-review/scripts/tests/test_word_metrics.py",
                    "3b5a285d3c9f611b3cda8c9c1d52d6b214c1f72e6e64a94ea5b0e220db5a8765",
                ),
                (
                    "src/core-skills/bmad-review/scripts/word_metrics.py",
                    "fb45007a7f3cfc18d13ee8bd4787facdaae297570193bc769456297dd33da78b",
                ),
            ]
            .into_iter()
            .map(|(path, hash)| source_resource(path, hash))
            .collect(),
            support_state: BmadSkillSupportState::Exact,
        })
        .unwrap_or_else(|_| unreachable!("the pinned review descriptor is canonical"))
    }

    fn execution_contract(&self) -> Result<BmadSkillExecutionContract, BmadSkillExecutionError> {
        let descriptor = self.descriptor();
        let source_hash = descriptor.source_sha256();
        let step_names = [
            "resolve_customization",
            "load_content",
            "select_lenses",
            "announce_plan",
            "run_independent_lenses",
            "run_dependent_lenses",
            "assemble_present",
        ];
        let steps = step_names
            .iter()
            .enumerate()
            .map(|(index, step)| BmadSkillExecutionStep {
                step_key: (*step).to_owned(),
                next_step_key: step_names.get(index + 1).map(|next| (*next).to_owned()),
                source_resource_path: SOURCE_PATH.to_owned(),
                source_resource_hash: source_hash,
            })
            .collect();
        let execution_profile = execution_profile(&descriptor)?;

        BmadSkillExecutionContract::new(BmadSkillExecutionContractInput {
            descriptor,
            package_name: "bmad-method".to_owned(),
            package_version: "6.10.0".to_owned(),
            package_version_id: contract_id(
                "pkgver_8B33C55A4D67D0B258FEDBB75D1CB09DBC7F711BC9BDC794D8B052B31FCE6D86",
            )?,
            package_descriptor_hash: digest(
                "sha256:8bfed9f6f0e29fd56013f51d12338409196a9654449ff730470c971585dc41bc",
            )?,
            package_source_hash: digest(
                "sha256:8b33c55a4d67d0b258fedbb75d1cb09dbc7f711bc9bdc794d8b052b31fce6d86",
            )?,
            instruction_projection_hash: digest(
                "sha256:2f21311006a41374538839354ce69bcf336da5c5f43760e17a3ecf58a588d605",
            )?,
            capability_catalog_hash: digest(
                "sha256:92b1cd6892be3685d8d583574361b56ba335a44846b2b2f5e77f177ebdc8dba6",
            )?,
            distribution_profile: "sapphirus_package".to_owned(),
            install_profile: "SapphirusManagedV1".to_owned(),
            execution_profile,
            validation_profile_hash: digest(
                "sha256:3e851de810792a1caacb2f4e4c1ac2134359f5b9da8c3870c6c37b967b03efba",
            )?,
            method_schema_hash: digest(
                "sha256:7e882e757a35b8d148c40d2a4ad2b41b16647f52d8f2a5d726df59824a238080",
            )?,
            workflow: BmadSkillExecutionWorkflow {
                initial_step_key: "resolve_customization".to_owned(),
                steps,
            },
            tool_intents: vec![
                BmadSkillToolIntent::FileRead,
                BmadSkillToolIntent::FileWrite,
                BmadSkillToolIntent::Process,
                BmadSkillToolIntent::Network,
                BmadSkillToolIntent::Browser,
                BmadSkillToolIntent::Subagent,
                BmadSkillToolIntent::ExternalHandoff,
            ],
            artifact_expectations: vec![MethodArtifactExpectation::from_source(
                MethodArtifactExpectation {
                    expectation_kind: "method_artifact".to_owned(),
                    expectation_id: contract_id("expectation_01J00000000000000000000042")?,
                    artifact_kind: "report".to_owned(),
                    required: true,
                    storage_scope: "app_local".to_owned(),
                    expected_media_type: "application/json".to_owned(),
                    expected_content_schema_hash: None,
                    source_output_hint: Some(
                        "{workflow.report_path} or governed findings JSON plus Markdown".to_owned(),
                    ),
                    completion_evidence_class: MethodEvidenceClass::Authoritative,
                    expectation_hash: digest(
                        "sha256:06c9436ad79a4168cd240214713f534cda62665ac5da184d6431effc43f9e7f3",
                    )?,
                },
            )
            .map_err(|_| BmadSkillExecutionError::ContractInvalid)?],
            recovery: BmadSkillRecoverySemantics::MethodCheckpoint,
        })
    }
}

fn execution_profile(
    descriptor: &BmadSkillDescriptor,
) -> Result<MethodExecutionProfile, BmadSkillExecutionError> {
    MethodExecutionProfile::from_source(
        MethodExecutionProfileData {
            entrypoint_kind: "direct".to_owned(),
            invocation_modes: MethodInvocationModes {
                interactive: true,
                headless: true,
                actions: Vec::new(),
            },
            required_runtimes: vec![MethodRuntimeRequirement {
                runtime: "uv".to_owned(),
                version_range: ">=0.8".to_owned(),
                required: false,
            }],
            resource_policy: MethodResourcePolicy {
                entrypoint_timing: "invocation_start".to_owned(),
                resource_timing: "on_demand_declared".to_owned(),
                declared_resource_paths: descriptor
                    .declared_resources()
                    .iter()
                    .map(|resource| resource.path().to_owned())
                    .collect(),
            },
            declared_tool_intents: vec![
                "file_read".to_owned(),
                "file_write".to_owned(),
                "process".to_owned(),
                "network".to_owned(),
                "browser".to_owned(),
                "subagent".to_owned(),
                "external_handoff".to_owned(),
            ],
            state_hints: vec!["artifact_workspace".to_owned(), "memlog".to_owned()],
            completion_evidence: vec![
                "artifact".to_owned(),
                "event".to_owned(),
                "status_evidence".to_owned(),
            ],
            customization_profile: "method_skill_toml".to_owned(),
            validation_profile: "MethodOfficialSkillV6".to_owned(),
        },
        digest("sha256:c1da109f36feae3aa74fb1d8dd7fc67a0f024c26874e648da2d0be4a52d9ca0f")?,
    )
    .map_err(|_| BmadSkillExecutionError::ContractInvalid)
}

fn source_resource(path: &str, hash: &str) -> BmadResource {
    BmadResource::new(path, digest_bytes(hash))
        .unwrap_or_else(|_| unreachable!("pinned review paths are canonical"))
}

fn digest_bytes(value: &str) -> Sha256Digest {
    Sha256Digest::parse(&format!("sha256:{value}"))
        .unwrap_or_else(|_| unreachable!("pinned review digests are canonical"))
}

fn digest(value: &str) -> Result<Sha256Digest, BmadSkillExecutionError> {
    Sha256Digest::parse(value).map_err(|_| BmadSkillExecutionError::ContractInvalid)
}

fn contract_id(value: &str) -> Result<ContractId, BmadSkillExecutionError> {
    ContractId::new(value).map_err(|_| BmadSkillExecutionError::ContractInvalid)
}
