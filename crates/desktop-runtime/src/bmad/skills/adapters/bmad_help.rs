//! Canonical adapter for pinned Method `bmad-help`.
//!
//! The adapter declares the five-member source closure already sealed by the
//! Help package descriptor: the entrypoint plus both installed module
//! catalogs and module descriptors. The existing Help coordinator remains the
//! only executor. This adapter contributes an inert, source-bound execution
//! contract which the coordinator must prepare before it binds the existing
//! durable [`crate::MethodSessionService`] lifecycle.
//!
//! The support state remains `Adapted`: mutable remote module documentation is
//! never fetched as source authority. The host may satisfy the source's
//! general-question branch only through typed read/model plans and a reviewed
//! immutable context snapshot.

use crate::generated_contracts::BmadInvocationMode;
use crate::{
    BmadMethodSkill, BmadResource, BmadSkillDescriptor, BmadSkillDescriptorInput,
    BmadSkillExecutionBranch, BmadSkillExecutionContract, BmadSkillExecutionContractInput,
    BmadSkillExecutionError, BmadSkillExecutionStep, BmadSkillExecutionWorkflow,
    BmadSkillRecoverySemantics, BmadSkillSupportState, BmadSkillToolIntent, ContractId,
    MethodExecutionProfile, MethodExecutionProfileData, MethodInvocationModes,
    MethodResourcePolicy, MethodRuntimeRequirement, Sha256Digest,
};

/// Adapter for pinned Method task MTH-40.
pub struct BmadHelpSkill;

impl BmadMethodSkill for BmadHelpSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "core:bmad-help".to_owned(),
            source_path: "src/core-skills/bmad-help/SKILL.md".to_owned(),
            source_sha256: Sha256Digest::from_bytes([
                0x97, 0x0c, 0x78, 0xec, 0x4e, 0xb6, 0x49, 0xc7, 0xed, 0xdd, 0x6d, 0xe0, 0xcf, 0x83,
                0xdf, 0xa9, 0xab, 0x1e, 0x63, 0xd4, 0x1f, 0x04, 0xd0, 0x24, 0x23, 0xe6, 0x28, 0x28,
                0xcb, 0x1f, 0xb5, 0xb2,
            ]),
            declared_actions: Vec::new(),
            supported_modes: vec![
                BmadInvocationMode::Interactive,
                BmadInvocationMode::Headless,
            ],
            predecessor: None,
            follower: None,
            output_locations: Vec::new(),
            artifact_kinds: Vec::new(),
            completion_hooks: Vec::new(),
            declared_resources: vec![
                source_resource(
                    "src/core-skills/bmad-help/SKILL.md",
                    [
                        0x97, 0x0c, 0x78, 0xec, 0x4e, 0xb6, 0x49, 0xc7, 0xed, 0xdd, 0x6d, 0xe0,
                        0xcf, 0x83, 0xdf, 0xa9, 0xab, 0x1e, 0x63, 0xd4, 0x1f, 0x04, 0xd0, 0x24,
                        0x23, 0xe6, 0x28, 0x28, 0xcb, 0x1f, 0xb5, 0xb2,
                    ],
                ),
                source_resource(
                    "src/bmm-skills/module-help.csv",
                    [
                        0x77, 0xf8, 0xc0, 0x8d, 0x62, 0x30, 0xb2, 0x91, 0xaa, 0x05, 0x6d, 0x21,
                        0xa6, 0x65, 0x66, 0x93, 0x2a, 0xd3, 0x74, 0xc9, 0x44, 0x28, 0x9e, 0xca,
                        0x67, 0x98, 0xe1, 0xbd, 0x5b, 0x6f, 0xd6, 0x77,
                    ],
                ),
                source_resource(
                    "src/bmm-skills/module.yaml",
                    [
                        0xde, 0x59, 0x3a, 0x66, 0x56, 0x73, 0x73, 0x02, 0x25, 0x7b, 0x7f, 0x48,
                        0x20, 0x71, 0x46, 0xed, 0x03, 0x43, 0xee, 0x81, 0x6c, 0xa3, 0x64, 0x2e,
                        0xd1, 0xf7, 0x09, 0x80, 0x53, 0x5a, 0x16, 0x59,
                    ],
                ),
                source_resource(
                    "src/core-skills/module-help.csv",
                    [
                        0x95, 0xcd, 0x2d, 0xa3, 0x32, 0x33, 0x26, 0x4a, 0x6e, 0x14, 0x08, 0x6e,
                        0xb5, 0x63, 0x53, 0x52, 0xaf, 0x0d, 0x8f, 0x9e, 0x24, 0x0c, 0x2a, 0xdb,
                        0x80, 0xe3, 0x20, 0x20, 0xfa, 0x97, 0x94, 0xf3,
                    ],
                ),
                source_resource(
                    "src/core-skills/module.yaml",
                    [
                        0x75, 0xcb, 0x5e, 0xcc, 0xdd, 0x19, 0x3b, 0x32, 0x82, 0xa9, 0x48, 0xf5,
                        0x7b, 0x27, 0x27, 0x31, 0x12, 0xaa, 0xbd, 0x61, 0x9e, 0xb2, 0x9f, 0xcc,
                        0xa4, 0x42, 0xd3, 0x30, 0x03, 0xa9, 0x14, 0xfb,
                    ],
                ),
            ],
            support_state: BmadSkillSupportState::Adapted,
        })
        .unwrap_or_else(|_| unreachable!("the pinned Help descriptor is structurally valid"))
    }

    fn execution_contract(&self) -> Result<BmadSkillExecutionContract, BmadSkillExecutionError> {
        let descriptor = self.descriptor();
        let source_hash = descriptor.source_sha256();
        let workflow = BmadSkillExecutionWorkflow {
            initial_step_key: "recommend".to_owned(),
            steps: vec![BmadSkillExecutionStep {
                step_key: "recommend".to_owned(),
                next_step_key: None,
                source_resource_path: "src/core-skills/bmad-help/SKILL.md".to_owned(),
                source_resource_hash: source_hash,
            }],
        };
        BmadSkillExecutionContract::new(BmadSkillExecutionContractInput {
            descriptor,
            package_name: "bmad-method".to_owned(),
            package_version: "6.10.0".to_owned(),
            package_version_id: contract_id(
                "pkgver_8B33C55A4D67D0B258FEDBB75D1CB09DBC7F711BC9BDC794D8B052B31FCE6D86",
            )?,
            package_descriptor_hash: digest(
                "sha256:2ec79c3da90bfc30030f15d902d776202c7ac91ed7cf7dc8fa8adbf85cb9cf6a",
            )?,
            package_source_hash: digest(
                "sha256:8b33c55a4d67d0b258fedbb75d1cb09dbc7f711bc9bdc794d8b052b31fce6d86",
            )?,
            instruction_projection_hash: digest(
                "sha256:483d33cdeaea3ce95f48741380190baf2a85bdb40007d487f23c7c67b56567e5",
            )?,
            capability_catalog_hash: digest(
                "sha256:f9d8ddce98c4e084049de94d78cef2e2d9018cff7ffeb639aafc80326f346dfd",
            )?,
            distribution_profile: "sapphirus_package".to_owned(),
            install_profile: "SapphirusManagedV1".to_owned(),
            execution_profile: execution_profile(&self.descriptor())?,
            validation_profile_hash: digest(
                "sha256:ddd086622be73b637cdcb3562b4459ac3853f1aeae34a53c43775af66e4cbdf0",
            )?,
            method_schema_hash: digest(
                "sha256:600432affbca9baec428903211df9c4c2a1fc6ba595959013236e1a0f16bc746",
            )?,
            branches: [BmadInvocationMode::Interactive, BmadInvocationMode::Headless]
                .into_iter()
                .map(|mode| BmadSkillExecutionBranch {
                    action: String::new(),
                    mode,
                    workflow: workflow.clone(),
                    artifact_expectations: Vec::new(),
                    required_status_fields: Vec::new(),
                })
                .collect(),
            tool_intents: vec![
                BmadSkillToolIntent::FileRead,
                BmadSkillToolIntent::Process,
                BmadSkillToolIntent::Web,
                BmadSkillToolIntent::Subagent,
            ],
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
        digest("sha256:eab6bd03e6ec5fa86f52f2386ac504979f8159352b09896ee14221ac15a23245")?,
    )
    .map_err(|_| BmadSkillExecutionError::ContractInvalid)
}

fn source_resource(path: &str, content_hash: [u8; 32]) -> BmadResource {
    BmadResource::new(path, Sha256Digest::from_bytes(content_hash))
        .unwrap_or_else(|_| unreachable!("all pinned Help resource paths are canonical"))
}

fn digest(value: &str) -> Result<Sha256Digest, BmadSkillExecutionError> {
    Sha256Digest::parse(value).map_err(|_| BmadSkillExecutionError::ContractInvalid)
}

fn contract_id(value: &str) -> Result<ContractId, BmadSkillExecutionError> {
    ContractId::new(value).map_err(|_| BmadSkillExecutionError::ContractInvalid)
}
