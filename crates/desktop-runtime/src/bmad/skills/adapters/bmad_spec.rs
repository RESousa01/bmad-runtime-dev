//! The `bmad-spec` (SPEC kernel distiller) skill adapter.
//!
//! The pinned source (`packages/bmad-foundation/source/method/6.10.0/src/
//! bmm-skills/2-plan-workflows/bmad-spec/SKILL.md`) distills any intent
//! input into one canonical working document, `SPEC.md`, plus optional
//! companion files: "produces **SPEC.md** carrying the five-field kernel ...
//! plus companion files for load-bearing content". `SPEC.md` is
//! `BmadArtifactKind::Document`, and it is re-derived and written on every
//! create or update run ("`SPEC.md` and every spec-authored companion are
//! **derived on each run**"), matching
//! `BmadArtifactCompletionHookRunHook::PersistArtifacts`.
//!
//! `declared_actions` stays empty, matching the catalog row's `action:
//! null`: the source names no fixed action -- it detects create-vs-update by
//! whether a prior `.memlog.md` exists at the target folder, not by a
//! caller-selected action.
//!
//! Unlike `bmad-prd` and `bmad-ux`, this skill's own activation step 3
//! resolves `{output_folder}` (under the merged `core` config), not
//! `{planning_artifacts}`: "resolve `{user_name}`, ..., `{output_folder}`
//! (under `core`), and `{date}`", and the spec folder itself defaults to
//! `{output_folder}/specs/spec-{slug}/`. `output_locations` reflects that
//! divergence rather than reusing the sibling skills' `planning_artifacts`
//! literal.
//!
//! `predecessor`/`follower` stay `None`: the source explicitly describes
//! itself as callable from many directions ("Multiple skills may call to
//! update the same spec over time") with no single fixed upstream or
//! downstream skill named.

use crate::generated_contracts::{
    BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationMode,
};
use crate::{
    sha256_bytes, BmadMethodSkill, BmadResource, BmadSkillDescriptor, BmadSkillDescriptorInput,
    BmadSkillExecutionBranch, BmadSkillExecutionContract, BmadSkillExecutionContractInput,
    BmadSkillExecutionError, BmadSkillExecutionStep, BmadSkillExecutionWorkflow,
    BmadSkillRecoverySemantics, BmadSkillSupportState, BmadSkillToolIntent, ContractId,
    MethodArtifactExpectation, MethodEvidenceClass, MethodExecutionProfile,
    MethodExecutionProfileData, MethodInvocationModes, MethodResourcePolicy, Sha256Digest,
};

/// Adapter for the pinned `bmad-spec` Method skill (MTH-09).
pub struct BmadSpecSkill;

impl BmadMethodSkill for BmadSpecSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "bmm:bmad-spec".to_owned(),
            source_path: "src/bmm-skills/2-plan-workflows/bmad-spec/SKILL.md".to_owned(),
            // Matches capability-catalog.json's row for this rowId; see
            // `bmad_method_plan_skills.rs`, which binds against the catalog
            // directly rather than trusting this literal alone. `from_bytes`
            // is infallible (a `const fn` over a fixed-size array), so no
            // fallible `parse` + panic-on-error is needed here.
            source_sha256: Sha256Digest::from_bytes([
                0x5b, 0x56, 0x80, 0xdb, 0xf0, 0x1d, 0xbf, 0xc5, 0xb4, 0xa5, 0x48, 0x5e, 0x70, 0xf2,
                0x0e, 0xa4, 0x8a, 0xc2, 0x1a, 0x1d, 0x61, 0x37, 0xc4, 0xea, 0x2f, 0xf7, 0x04, 0x26,
                0x2d, 0xcc, 0x45, 0xd2,
            ]),
            declared_actions: Vec::new(),
            supported_modes: vec![
                BmadInvocationMode::Interactive,
                BmadInvocationMode::Headless,
            ],
            predecessor: None,
            follower: None,
            // Step 3 resolves `{output_folder}` (under `core`), the field
            // the spec workspace is actually bound under -- see module doc.
            output_locations: vec!["output_folder".to_owned()],
            // `SPEC.md` and `.memlog.md` are required documents. Optional
            // companions and interactive-only `stories.yaml` make the
            // workspace a governed file set as well.
            artifact_kinds: vec![BmadArtifactKind::Document, BmadArtifactKind::FileSet],
            completion_hooks: vec![BmadArtifactCompletionHookRunHook::PersistArtifacts],
            declared_resources: vec![
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-spec/SKILL.md",
                    [
                        0x5b, 0x56, 0x80, 0xdb, 0xf0, 0x1d, 0xbf, 0xc5, 0xb4, 0xa5, 0x48, 0x5e,
                        0x70, 0xf2, 0x0e, 0xa4, 0x8a, 0xc2, 0x1a, 0x1d, 0x61, 0x37, 0xc4, 0xea,
                        0x2f, 0xf7, 0x04, 0x26, 0x2d, 0xcc, 0x45, 0xd2,
                    ],
                ),
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-spec/customize.toml",
                    [
                        0xa8, 0x9e, 0x0f, 0x53, 0xf9, 0xb6, 0x38, 0x6c, 0x28, 0x1d, 0xd0, 0x15,
                        0x8a, 0x21, 0x80, 0x1a, 0x6b, 0x16, 0xf1, 0x22, 0x08, 0x5f, 0xd6, 0x8b,
                        0x02, 0x20, 0xcd, 0x40, 0x3c, 0x18, 0xf8, 0x30,
                    ],
                ),
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-spec/assets/headless-schemas.md",
                    [
                        0x83, 0xaf, 0xb3, 0x61, 0x47, 0x16, 0xdb, 0x23, 0x47, 0xd8, 0x35, 0x86,
                        0xa6, 0x7b, 0x9c, 0x52, 0x44, 0x92, 0xdd, 0x50, 0xdb, 0x52, 0x82, 0x43,
                        0x6c, 0xcb, 0x35, 0xc9, 0x9f, 0xb5, 0xf0, 0x8a,
                    ],
                ),
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-spec/assets/spec-template.md",
                    [
                        0x4b, 0x3a, 0x03, 0x2c, 0x04, 0x5f, 0xdd, 0x09, 0x03, 0x93, 0x34, 0x46,
                        0xfc, 0x07, 0xf9, 0x3e, 0x87, 0x99, 0x97, 0xf2, 0x8a, 0x02, 0x94, 0xed,
                        0x43, 0x44, 0xda, 0x71, 0x19, 0x2b, 0x20, 0xee,
                    ],
                ),
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-spec/assets/stories-schema.md",
                    [
                        0x4f, 0x81, 0x37, 0xc0, 0x0e, 0x27, 0x10, 0x28, 0x83, 0x54, 0xa4, 0x46,
                        0x6d, 0x1c, 0xfb, 0x90, 0x70, 0x4f, 0x4b, 0x2a, 0x04, 0xbd, 0x3b, 0x63,
                        0x92, 0x42, 0x18, 0xfd, 0x58, 0x9c, 0xc6, 0xa9,
                    ],
                ),
            ],
            support_state: BmadSkillSupportState::Adapted,
        })
        .unwrap_or_else(|_| unreachable!("this adapter's fields satisfy the harness's own bounds"))
    }

    fn execution_contract(&self) -> Result<BmadSkillExecutionContract, BmadSkillExecutionError> {
        let descriptor = self.descriptor();
        let source_hash = descriptor.source_sha256();
        let execution_profile = execution_profile(&descriptor)?;

        BmadSkillExecutionContract::new(BmadSkillExecutionContractInput {
            descriptor,
            package_name: "bmad-method".to_owned(),
            package_version: "6.10.0".to_owned(),
            package_version_id: contract_id(
                "pkgver_8B33C55A4D67D0B258FEDBB75D1CB09DBC7F711BC9BDC794D8B052B31FCE6D86",
            )?,
            package_descriptor_hash: digest(
                "sha256:23280a02c31af2f9da1c3991278d542e796b075e198101e19dbbb92f7ea74e1e",
            )?,
            package_source_hash: digest(
                "sha256:8b33c55a4d67d0b258fedbb75d1cb09dbc7f711bc9bdc794d8b052b31fce6d86",
            )?,
            instruction_projection_hash: digest(
                "sha256:884facf72c30139829c9d1ec3cd63bcd651126abfecb7b5db2da4ed21af591a9",
            )?,
            capability_catalog_hash: digest(
                "sha256:befeeeafad7a379eec5ab96f945b13e5cb239bc3531a7a281fe16a25b839315d",
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
            branches: [
                BmadInvocationMode::Interactive,
                BmadInvocationMode::Headless,
            ]
            .into_iter()
            .map(|mode| BmadSkillExecutionBranch {
                action: String::new(),
                mode,
                workflow: spec_workflow(source_hash),
                artifact_expectations: artifact_expectations(mode)
                    .unwrap_or_else(|_| unreachable!("Spec artifact declarations are valid")),
                required_status_fields: vec!["output_folder".to_owned(), "slug".to_owned()],
            })
            .collect(),
            tool_intents: vec![
                BmadSkillToolIntent::FileRead,
                BmadSkillToolIntent::FileWrite,
                BmadSkillToolIntent::Process,
                BmadSkillToolIntent::Subagent,
                BmadSkillToolIntent::ExternalHandoff,
            ],
            recovery: BmadSkillRecoverySemantics::MethodCheckpoint,
        })
    }
}

fn spec_workflow(source_hash: Sha256Digest) -> BmadSkillExecutionWorkflow {
    BmadSkillExecutionWorkflow {
        initial_step_key: "activate_spec".to_owned(),
        steps: [
            ("activate_spec", Some("resolve_spec_intent")),
            ("resolve_spec_intent", Some("distill_spec")),
            ("distill_spec", Some("validate_spec_coherence")),
            (
                "validate_spec_coherence",
                Some("validate_spec_preservation"),
            ),
            ("validate_spec_preservation", Some("complete_spec")),
            ("complete_spec", None),
        ]
        .into_iter()
        .map(|(step_key, next_step_key)| BmadSkillExecutionStep {
            step_key: step_key.to_owned(),
            next_step_key: next_step_key.map(str::to_owned),
            source_resource_path: "src/bmm-skills/2-plan-workflows/bmad-spec/SKILL.md".to_owned(),
            source_resource_hash: source_hash,
        })
        .collect(),
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
        sha256_bytes(b"bmad-spec-execution-profile-v1"),
    )
    .map_err(|_| BmadSkillExecutionError::ContractInvalid)
}

fn artifact_expectations(
    mode: BmadInvocationMode,
) -> Result<Vec<MethodArtifactExpectation>, BmadSkillExecutionError> {
    [
        (
            "expectation_01J00000000000000000000009",
            "document",
            true,
            "text/markdown",
            "{spec-folder}/SPEC.md",
            MethodEvidenceClass::Authoritative,
        ),
        (
            "expectation_01J00000000000000000000010",
            "document",
            true,
            "text/markdown",
            "{spec-folder}/.memlog.md",
            MethodEvidenceClass::Authoritative,
        ),
        (
            "expectation_01J00000000000000000000011",
            "file_set",
            false,
            "text/markdown",
            "{spec-folder}/<content-typed-companion>.md",
            MethodEvidenceClass::Authoritative,
        ),
        (
            "expectation_01J00000000000000000000012",
            "file_set",
            false,
            "application/yaml",
            "{spec-folder}/stories.yaml (interactive Story Breakdown only)",
            MethodEvidenceClass::Contextual,
        ),
    ]
    .into_iter()
    .filter(|(expectation_id, ..)| {
        mode == BmadInvocationMode::Interactive
            || *expectation_id != "expectation_01J00000000000000000000012"
    })
    .map(
        |(
            expectation_id,
            artifact_kind,
            required,
            expected_media_type,
            source_output_hint,
            completion_evidence_class,
        )| {
            MethodArtifactExpectation::from_source(MethodArtifactExpectation {
                expectation_kind: "method_artifact".to_owned(),
                expectation_id: contract_id(expectation_id)?,
                artifact_kind: artifact_kind.to_owned(),
                required,
                storage_scope: "app_local".to_owned(),
                expected_media_type: expected_media_type.to_owned(),
                expected_content_schema_hash: None,
                source_output_hint: Some(source_output_hint.to_owned()),
                completion_evidence_class,
                expectation_hash: sha256_bytes(
                    format!("bmad-spec:{expectation_id}:{source_output_hint}").as_bytes(),
                ),
            })
            .map_err(|_| BmadSkillExecutionError::ContractInvalid)
        },
    )
    .collect()
}

fn source_resource(path: &str, content_hash: [u8; 32]) -> BmadResource {
    BmadResource::new(path, Sha256Digest::from_bytes(content_hash))
        .unwrap_or_else(|_| unreachable!("all pinned bmad-spec resource paths are canonical"))
}

fn digest(value: &str) -> Result<Sha256Digest, BmadSkillExecutionError> {
    Sha256Digest::parse(value).map_err(|_| BmadSkillExecutionError::ContractInvalid)
}

fn contract_id(value: &str) -> Result<ContractId, BmadSkillExecutionError> {
    ContractId::new(value).map_err(|_| BmadSkillExecutionError::ContractInvalid)
}
