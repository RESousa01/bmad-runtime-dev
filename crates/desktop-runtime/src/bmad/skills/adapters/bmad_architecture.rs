//! The `bmad-architecture` skill adapter.
//!
//! Without this module, MTH-12 has no typed descriptor to hand the runtime,
//! Help, or the compatibility ledger -- they would each have to re-derive
//! this skill's shape from the pinned `SKILL.md` prose themselves.
//!
//! The pinned source (`packages/bmad-foundation/source/method/6.10.0/src/
//! bmm-skills/3-solutioning/bmad-architecture/SKILL.md`) coaches (or, on the
//! Fast path, drafts) an "architecture spine" -- a lean invariants document
//! distilled from an append-only `.memlog.md` and written to
//! `ARCHITECTURE-SPINE.md` under `{workflow.spine_output_path}`. Activation
//! step 3 explicitly names both operating modes: "Headless (no interactive
//! user) -> follow `references/headless.md` for the whole run. Otherwise
//! greet `{user_name}`" -- so, unlike some sibling skills in this batch,
//! headless is source-declared here, not only catalog-declared.
//!
//! `declared_actions` stays empty: the source detects intent (create /
//! update / validate) from conversation and input rather than exposing it as
//! a fixed, separately-invocable action, and the catalog row for this rowId
//! records `action: null`.
//!
//! This crate also carries `crate::bmad::architecture` -- a host-owned state
//! machine that validates the *ordered progression* of a sealed
//! architecture-creation guidance flow (`SystemContext`, `Components`, ...,
//! `Deferred`) and distills accepted steps into a
//! [`crate::bmad::capability::BmadDocumentArtifact`]. That module is a
//! runtime *behavior* validator for one specific guidance flow; this
//! descriptor is a *declared-shape* description of the whole skill. They do
//! not contradict each other -- `architecture.rs`'s step breadth is Forge's
//! own reading of "sweep the breadth the altitude owns" and is documented
//! there as "not sealed BMAD source", while this descriptor's fields (modes,
//! output location, artifact kind) come straight from `SKILL.md` prose. Read
//! together: the fine-grained step ordering `architecture.rs` enforces is
//! invisible at this descriptor's altitude, which only asserts that the
//! skill is invocable in both modes and produces one persisted document.

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

/// Adapter for the pinned `bmad-architecture` Method skill (MTH-12).
pub struct BmadArchitectureSkill;

impl BmadMethodSkill for BmadArchitectureSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "bmm:bmad-architecture".to_owned(),
            source_path: "src/bmm-skills/3-solutioning/bmad-architecture/SKILL.md".to_owned(),
            // Matches capability-catalog.json's row for this rowId; see
            // `bmad_method_solutioning_skills.rs`, which binds against the
            // catalog directly rather than trusting this literal alone.
            // `from_bytes` is infallible (a `const fn` over a fixed-size
            // array), so no fallible `parse` + panic-on-error is needed here.
            source_sha256: Sha256Digest::from_bytes([
                0x89, 0xa3, 0x16, 0xfd, 0x74, 0xd8, 0x49, 0x0d, 0x1c, 0xeb, 0x2e, 0xc7, 0xc4, 0x0e,
                0xa5, 0x03, 0x50, 0x4f, 0xd4, 0xc7, 0xa2, 0xc1, 0xe3, 0x10, 0x89, 0xbf, 0xc6, 0x28,
                0xc8, 0x6c, 0x45, 0xbf,
            ]),
            // Create/Update/Validate intent is detected conversationally,
            // not exposed as a fixed action; catalog row records
            // `action: null`. See module doc.
            declared_actions: Vec::new(),
            supported_modes: vec![
                BmadInvocationMode::Interactive,
                BmadInvocationMode::Headless,
            ],
            predecessor: None,
            // Finalize step 7 requires the next recommendation to lead with
            // bmad-spec. Preserve that primary edge while the source's other
            // conditional suggestions remain descriptive guidance.
            follower: Some("bmm:bmad-spec".to_owned()),
            // "bind `{doc_workspace}` to `{workflow.spine_output_path}/
            // {workflow.run_folder_pattern}/`" -- resolved under the same
            // `{planning_artifacts}` config surface every sibling skill in
            // this catalog resolves from `_bmad/bmm/config.yaml`.
            output_locations: vec!["planning_artifacts".to_owned()],
            // "seed `ARCHITECTURE-SPINE.md` from `{workflow.spine_template}`"
            // -- a single markdown document.
            artifact_kinds: vec![BmadArtifactKind::Document],
            // The memlog is written via `memlog.py append` as the run
            // progresses, and the spine itself is distilled and saved at
            // Finalize/Update/Close -- persisted, not merely proposed.
            completion_hooks: vec![BmadArtifactCompletionHookRunHook::PersistArtifacts],
            // The complete recursive source directory is immutable package
            // data. Declaring it here binds preparation to every retained
            // source byte without executing the pinned Python helper.
            declared_resources: vec![
                source_resource(
                    "src/bmm-skills/3-solutioning/bmad-architecture/SKILL.md",
                    "sha256:89a316fd74d8490d1ceb2ec7c40ea503504fd4c7a2c1e31089bfc628c86c45bf",
                ),
                source_resource(
                    "src/bmm-skills/3-solutioning/bmad-architecture/customize.toml",
                    "sha256:a6bb60a3adf976031f2870203285b1fc7a56e71e21232cd23bba544b178b5f15",
                ),
                source_resource(
                    "src/bmm-skills/3-solutioning/bmad-architecture/assets/spine-template.md",
                    "sha256:ef4ff795624eb5439fae54a06edb389feb5a0cf79cb01ae007af51109335d198",
                ),
                source_resource(
                    "src/bmm-skills/3-solutioning/bmad-architecture/references/headless.md",
                    "sha256:4db2708f335c94cb024b5eba022b1e3d88b1ec61c94c4716626d9b89684f5833",
                ),
                source_resource(
                    "src/bmm-skills/3-solutioning/bmad-architecture/references/reviewer-gate.md",
                    "sha256:d32e32a3c1d59b5612b947004f3f6fef1117a13ce9f1ffa428d616e0b5d4db69",
                ),
                source_resource(
                    "src/bmm-skills/3-solutioning/bmad-architecture/scripts/lint_spine.py",
                    "sha256:040e0f1c13d31e6a21c518df943d1d7de7b75d8d98a07603b0983f3f3fb500db",
                ),
                source_resource(
                    "src/bmm-skills/3-solutioning/bmad-architecture/scripts/tests/test_lint_spine.py",
                    "sha256:e86bb9a62c5cca74ddc89c843386f2b992610fcc1792d40e03bc9cd445b77681",
                ),
            ],
            // Forge preserves the source-visible intents and outputs while
            // replacing direct scripts, web lookups, subagents, and file
            // effects with typed host plans. That is an explicit adaptation,
            // not byte-for-byte runtime adoption.
            support_state: BmadSkillSupportState::Adapted,
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
                "sha256:036d3e0fdef725b894c2a5a95c4560d3250dd54b14640fecb26aa88c459c0c31",
            )?,
            package_source_hash: digest(
                "sha256:8b33c55a4d67d0b258fedbb75d1cb09dbc7f711bc9bdc794d8b052b31fce6d86",
            )?,
            instruction_projection_hash: digest(
                "sha256:fc4baf320e3e3e16149680630f7efc28e757b0780d7b56b6ae935f87a43b6b46",
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
                initial_step_key: "facilitate_architecture".to_owned(),
                steps: vec![BmadSkillExecutionStep {
                    step_key: "facilitate_architecture".to_owned(),
                    next_step_key: None,
                    source_resource_path: "src/bmm-skills/3-solutioning/bmad-architecture/SKILL.md"
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
                    expectation_id: contract_id("expectation_01J00000000000000000000012")?,
                    artifact_kind: "document".to_owned(),
                    required: true,
                    storage_scope: "app_local".to_owned(),
                    expected_media_type: "text/markdown".to_owned(),
                    expected_content_schema_hash: None,
                    source_output_hint: Some(
                        "{doc_workspace}/ARCHITECTURE-SPINE.md or \
                         {doc_workspace}/validation-report.{md,html}"
                            .to_owned(),
                    ),
                    completion_evidence_class: MethodEvidenceClass::Authoritative,
                    expectation_hash: digest(
                        "sha256:2c980deefda7f459d7adaa2c0f63288041ad2fb92b2094fa6e803096d52fc4a3",
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

fn source_resource(path: &str, content_hash: &str) -> BmadResource {
    BmadResource::new(
        path,
        Sha256Digest::parse(content_hash)
            .unwrap_or_else(|_| unreachable!("all pinned architecture hashes are canonical")),
    )
    .unwrap_or_else(|_| unreachable!("all pinned architecture resource paths are canonical"))
}

fn digest(value: &str) -> Result<Sha256Digest, BmadSkillExecutionError> {
    Sha256Digest::parse(value).map_err(|_| BmadSkillExecutionError::ContractInvalid)
}

fn contract_id(value: &str) -> Result<ContractId, BmadSkillExecutionError> {
    ContractId::new(value).map_err(|_| BmadSkillExecutionError::ContractInvalid)
}
