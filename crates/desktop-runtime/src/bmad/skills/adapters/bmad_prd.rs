//! The `bmad-prd` (PRD create/update/validate) skill adapter.
//!
//! The pinned source (`packages/bmad-foundation/source/method/6.10.0/src/
//! bmm-skills/2-plan-workflows/bmad-prd/SKILL.md`) is a workflow skill that
//! detects intent -- **Create**, **Update**, or **Validate** -- at
//! activation step 5 ("Detect intent") and drives a single working document,
//! `prd.md`, through Discovery and Finalize: "Bind `{doc_workspace}` ... .
//! Write `prd.md` with YAML frontmatter". That document is
//! `BmadArtifactKind::Document`, persisted as it is written (Finalize step 7
//! sets `status: final` in place), matching
//! `BmadArtifactCompletionHookRunHook::PersistArtifacts`.
//!
//! `declared_actions` stays empty even though Create/Update/Validate are
//! named, matching the catalog row's `action: null`: intent is detected
//! conversationally within one entry point (or forwarded verbatim by a
//! caller, per "Forwarded activation"), not selected as a fixed,
//! separately-invocable action the way [`BmadInvocationDeclaredAction`]
//! models one -- the same representational limit `bmad-product-brief`'s
//! adapter already documents for its own Create/Update/Validate modes.
//!
//! `follower` stays `None`: Finalize step 7 names three legitimate next
//! skills side by side ("Common next: `bmad-ux`, `bmad-architecture`,
//! `bmad-create-epics-and-stories`") plus a router (`bmad-help`), not one
//! fixed follower -- asserting any single one of the three would misrepresent
//! the source's own "any of these" framing. This is reported as a gap: see
//! the parent task's "what couldn't be represented" note.
//!
//! [`BmadInvocationDeclaredAction`]: crate::generated_contracts::BmadInvocationDeclaredAction

use crate::generated_contracts::{
    BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationMode,
};
use crate::{
    BmadMethodSkill, BmadResource, BmadSkillDescriptor, BmadSkillDescriptorInput,
    BmadSkillExecutionContract, BmadSkillExecutionContractInput, BmadSkillExecutionError,
    BmadSkillExecutionStep, BmadSkillExecutionWorkflow, BmadSkillRecoverySemantics,
    BmadSkillSupportState, BmadSkillToolIntent, ContractId, MethodArtifactExpectation,
    MethodEvidenceClass, MethodExecutionProfile, MethodExecutionProfileData, MethodInvocationModes,
    MethodResourcePolicy, Sha256Digest,
};

/// Adapter for the pinned `bmad-prd` Method skill (MTH-08).
pub struct BmadPrdSkill;

impl BmadMethodSkill for BmadPrdSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "bmm:bmad-prd".to_owned(),
            source_path: "src/bmm-skills/2-plan-workflows/bmad-prd/SKILL.md".to_owned(),
            // Matches capability-catalog.json's row for this rowId; see
            // `bmad_method_plan_skills.rs`, which binds against the catalog
            // directly rather than trusting this literal alone. `from_bytes`
            // is infallible (a `const fn` over a fixed-size array), so no
            // fallible `parse` + panic-on-error is needed here.
            source_sha256: Sha256Digest::from_bytes([
                0x5d, 0x7b, 0x47, 0x19, 0xa1, 0xfa, 0x9e, 0x4f, 0x7d, 0x4c, 0xbe, 0xeb, 0x4a, 0x41,
                0x2c, 0x79, 0xe1, 0x41, 0xbc, 0x77, 0xe8, 0xe2, 0x5b, 0xa4, 0x42, 0xb8, 0x5e, 0x1c,
                0x5c, 0x58, 0x7f, 0x3f,
            ]),
            declared_actions: Vec::new(),
            supported_modes: vec![
                BmadInvocationMode::Interactive,
                BmadInvocationMode::Headless,
            ],
            // No single predecessor is named: the only upstream reference is
            // a misroute suggestion ("one-pager → `bmad-product-brief`"),
            // which redirects away from bmad-prd rather than naming what
            // normally precedes it.
            predecessor: None,
            follower: None,
            // Step 3 ("Load config") explicitly resolves `{planning_artifacts}`.
            output_locations: vec!["planning_artifacts".to_owned()],
            // "Write `prd.md` with YAML frontmatter" -- the working artifact
            // is a single markdown document at a `{doc_workspace}` path.
            artifact_kinds: vec![BmadArtifactKind::Document],
            completion_hooks: vec![BmadArtifactCompletionHookRunHook::PersistArtifacts],
            // The complete recursively owned source directory is declared as
            // immutable package data. These identities give the generic
            // resolver the source-closure boundary without allowing this
            // adapter to read, execute, or otherwise grant authority to the
            // pinned files.
            declared_resources: vec![
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-prd/SKILL.md",
                    [
                        0x5d, 0x7b, 0x47, 0x19, 0xa1, 0xfa, 0x9e, 0x4f, 0x7d, 0x4c, 0xbe, 0xeb,
                        0x4a, 0x41, 0x2c, 0x79, 0xe1, 0x41, 0xbc, 0x77, 0xe8, 0xe2, 0x5b, 0xa4,
                        0x42, 0xb8, 0x5e, 0x1c, 0x5c, 0x58, 0x7f, 0x3f,
                    ],
                ),
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-prd/customize.toml",
                    [
                        0x7f, 0x9a, 0x3a, 0x35, 0x03, 0xa3, 0xf7, 0x58, 0x00, 0x3c, 0xe2, 0xf1,
                        0xde, 0x59, 0xc3, 0x1c, 0xc5, 0x36, 0x58, 0x22, 0x3e, 0x35, 0x5f, 0xc7,
                        0x60, 0x55, 0xf3, 0x14, 0x0e, 0xf3, 0x4e, 0x90,
                    ],
                ),
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-prd/assets/headless-schemas.md",
                    [
                        0x8d, 0x36, 0x21, 0x33, 0x3a, 0x72, 0x40, 0x05, 0x12, 0xbd, 0x62, 0x51,
                        0x1b, 0xc4, 0xc7, 0x3f, 0xc7, 0x45, 0x71, 0x15, 0x05, 0xe1, 0xf8, 0x52,
                        0xc6, 0x97, 0xff, 0x8c, 0x5f, 0xa2, 0xde, 0x93,
                    ],
                ),
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-prd/assets/prd-template.md",
                    [
                        0x41, 0xe7, 0x7b, 0xd5, 0xc4, 0xee, 0x81, 0xc3, 0xf3, 0x83, 0xb5, 0x5d,
                        0x6f, 0x51, 0x51, 0x47, 0xaf, 0xc9, 0xc4, 0xbd, 0x1a, 0x33, 0x17, 0xdd,
                        0xf5, 0x5a, 0x92, 0x82, 0xde, 0xae, 0x49, 0x5c,
                    ],
                ),
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-prd/assets/prd-validation-checklist.md",
                    [
                        0x41, 0xd9, 0xf0, 0x90, 0x64, 0x77, 0x2a, 0x92, 0xec, 0x90, 0x8d, 0x97,
                        0x06, 0x18, 0xe4, 0x7b, 0xa6, 0x3d, 0xc8, 0x6b, 0x5c, 0x5a, 0x1a, 0x21,
                        0xd5, 0xa2, 0xc6, 0x76, 0xd3, 0xba, 0x8e, 0x3f,
                    ],
                ),
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-prd/assets/validation-report-template.html",
                    [
                        0xa8, 0x0b, 0xec, 0xdd, 0x82, 0x05, 0xd2, 0xd7, 0xc1, 0x87, 0x38, 0x85,
                        0x58, 0x30, 0xb0, 0x96, 0x17, 0x2a, 0x49, 0x6b, 0x96, 0xb5, 0xca, 0x8a,
                        0x08, 0x20, 0xec, 0x55, 0xc9, 0xbb, 0x8b, 0xf9,
                    ],
                ),
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-prd/references/headless.md",
                    [
                        0x4d, 0xad, 0x58, 0xeb, 0xee, 0x63, 0x93, 0x3a, 0x53, 0x17, 0x90, 0x16,
                        0x51, 0x3f, 0xf9, 0x4d, 0x14, 0x3c, 0xe9, 0xf6, 0x8e, 0x22, 0x64, 0x89,
                        0xd3, 0x90, 0xe2, 0xc3, 0x6b, 0xf1, 0xd8, 0x48,
                    ],
                ),
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-prd/references/validate.md",
                    [
                        0x71, 0xf2, 0x87, 0x74, 0xe8, 0x5c, 0x67, 0x3a, 0x5c, 0x48, 0xaf, 0x60,
                        0x7a, 0xf5, 0x41, 0xe6, 0xa0, 0x8e, 0x10, 0x07, 0x6a, 0x2d, 0x46, 0xd0,
                        0x76, 0x38, 0x85, 0xfa, 0xfc, 0x74, 0xf2, 0x01,
                    ],
                ),
            ],
            support_state: BmadSkillSupportState::Exact,
        })
        .unwrap_or_else(|_| unreachable!("this adapter's fields satisfy the harness's own bounds"))
    }

    fn execution_contract(&self) -> Result<BmadSkillExecutionContract, BmadSkillExecutionError> {
        let descriptor = self.descriptor();
        let execution_profile = execution_profile(&descriptor)?;
        let source_hash = descriptor.source_sha256();

        BmadSkillExecutionContract::new(BmadSkillExecutionContractInput {
            descriptor,
            package_name: "bmad-method".to_owned(),
            package_version: "6.10.0".to_owned(),
            package_version_id: contract_id(
                "pkgver_8B33C55A4D67D0B258FEDBB75D1CB09DBC7F711BC9BDC794D8B052B31FCE6D86",
            )?,
            package_descriptor_hash: digest(
                "sha256:b3d3a62bee24df5565de4182f4774765c3abb5d829e05e3c72f362ae823601bc",
            )?,
            package_source_hash: digest(
                "sha256:8b33c55a4d67d0b258fedbb75d1cb09dbc7f711bc9bdc794d8b052b31fce6d86",
            )?,
            instruction_projection_hash: digest(
                "sha256:020773a36111d798aaadbed06a1aaa40ae4c830140ebcd2b4f035245beb29d17",
            )?,
            capability_catalog_hash: digest(
                "sha256:d6a3198652479968f7b7f6e2eacb16af2611143631200c2a5dfc3f492400de00",
            )?,
            distribution_profile: "sapphirus_package".to_owned(),
            install_profile: "SapphirusManagedV1".to_owned(),
            execution_profile,
            validation_profile_hash: digest(
                "sha256:3e851de810792a1caacb2f4e4c1ac2134359f5b9da8c3870c6c37b967b03efba",
            )?,
            method_schema_hash: digest(
                "sha256:dda02f453dcf4edbb3fc66e0ad3a7c59d7087f5b44b526e40c435456c8c248c2",
            )?,
            workflow: BmadSkillExecutionWorkflow {
                initial_step_key: "facilitate_prd".to_owned(),
                steps: vec![BmadSkillExecutionStep {
                    step_key: "facilitate_prd".to_owned(),
                    next_step_key: None,
                    source_resource_path: "src/bmm-skills/2-plan-workflows/bmad-prd/SKILL.md"
                        .to_owned(),
                    source_resource_hash: source_hash,
                }],
            },
            tool_intents: vec![
                BmadSkillToolIntent::FileRead,
                BmadSkillToolIntent::FileWrite,
                BmadSkillToolIntent::Process,
                BmadSkillToolIntent::Web,
                BmadSkillToolIntent::Subagent,
                BmadSkillToolIntent::ExternalHandoff,
            ],
            artifact_expectations: vec![MethodArtifactExpectation::from_source(
                MethodArtifactExpectation {
                    expectation_kind: "method_artifact".to_owned(),
                    expectation_id: contract_id("expectation_01J00000000000000000000008")?,
                    artifact_kind: "document".to_owned(),
                    required: true,
                    storage_scope: "app_local".to_owned(),
                    expected_media_type: "text/markdown".to_owned(),
                    expected_content_schema_hash: None,
                    source_output_hint: Some(
                        "{doc_workspace}/prd.md or {doc_workspace}/validation-report.{md,html}"
                            .to_owned(),
                    ),
                    completion_evidence_class: MethodEvidenceClass::Authoritative,
                    expectation_hash: digest(
                        "sha256:3b263d0c06079b668d7a0b495a2a91b096f93084a5f529628be13c858f876c22",
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
            required_runtimes: Vec::new(),
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
        digest("sha256:1e1652a19e34c8306315e5f47a5b791582d472604217f65780463ed8d768d64f")?,
    )
    .map_err(|_| BmadSkillExecutionError::ContractInvalid)
}

fn source_resource(path: &str, content_hash: [u8; 32]) -> BmadResource {
    BmadResource::new(path, Sha256Digest::from_bytes(content_hash))
        .unwrap_or_else(|_| unreachable!("all pinned bmad-prd resource paths are canonical"))
}

fn digest(value: &str) -> Result<Sha256Digest, BmadSkillExecutionError> {
    Sha256Digest::parse(value).map_err(|_| BmadSkillExecutionError::ContractInvalid)
}

fn contract_id(value: &str) -> Result<ContractId, BmadSkillExecutionError> {
    ContractId::new(value).map_err(|_| BmadSkillExecutionError::ContractInvalid)
}
