//! The `bmad-ux` (DESIGN.md / EXPERIENCE.md UX spines) skill adapter.
//!
//! The pinned source (`packages/bmad-foundation/source/method/6.10.0/src/
//! bmm-skills/2-plan-workflows/bmad-ux/SKILL.md`) produces two peer
//! documents: "Produce two peer contracts: **`DESIGN.md`** ... and
//! **`EXPERIENCE.md`**". Both are markdown documents persisted as the run
//! proceeds (Finalize: "Set both files' `status: final`"), so
//! `artifact_kinds` carries one `BmadArtifactKind::Document` entry -- the
//! field records the *kind* of artifact produced, and both files share that
//! one kind, matching `bmad-prd`'s and `bmad-spec`'s single-`Document`
//! declarations rather than double-counting per file.
//! `BmadArtifactCompletionHookRunHook::PersistArtifacts` follows for the
//! same reason those two skills declare it.
//!
//! Create, Update, and Validate are explicit source action branches. The
//! catalog still counts one primary capability (`action: null` is the
//! top-level discovery row), while the descriptor carries the three typed
//! action choices accepted by the generic runtime.
//!
//! `predecessor`/`follower` stay `None`: the source explicitly states "UX
//! may lead, follow, or stand alone" and Finalize names three legitimate
//! next skills side by side ("Common next: `bmad-architecture`,
//! `bmad-create-epics-and-stories`, `bmad-dev-story`") -- no single fixed
//! predecessor or follower, the same non-fixed-ordering shape `bmad-prd`'s
//! adapter documents.

use crate::generated_contracts::{
    BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationDeclaredAction,
    BmadInvocationDeclaredActionName, BmadInvocationMode,
};
use crate::{
    BmadMethodSkill, BmadResource, BmadSkillDescriptor, BmadSkillDescriptorInput,
    BmadSkillExecutionBranch, BmadSkillExecutionContract, BmadSkillExecutionContractInput,
    BmadSkillExecutionError, BmadSkillExecutionStep, BmadSkillExecutionWorkflow,
    BmadSkillRecoverySemantics, BmadSkillSupportState, BmadSkillToolIntent, ContractId,
    MethodArtifactExpectation, MethodEvidenceClass, MethodExecutionProfile,
    MethodExecutionProfileData, MethodInvocationModes, MethodResourcePolicy, Sha256Digest,
};

/// Adapter for the pinned `bmad-ux` Method skill (MTH-10).
pub struct BmadUxSkill;

impl BmadMethodSkill for BmadUxSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "bmm:bmad-ux".to_owned(),
            source_path: "src/bmm-skills/2-plan-workflows/bmad-ux/SKILL.md".to_owned(),
            // Matches capability-catalog.json's row for this rowId; see
            // `bmad_method_plan_skills.rs`, which binds against the catalog
            // directly rather than trusting this literal alone. `from_bytes`
            // is infallible (a `const fn` over a fixed-size array), so no
            // fallible `parse` + panic-on-error is needed here.
            source_sha256: Sha256Digest::from_bytes([
                0x25, 0x0d, 0x27, 0x94, 0xf2, 0xb8, 0xe3, 0x16, 0xbb, 0x1b, 0xa4, 0x66, 0x6f, 0x07,
                0x28, 0x97, 0x0a, 0xfd, 0x8a, 0x5d, 0x27, 0x07, 0xac, 0x1c, 0xce, 0xac, 0x6c, 0x21,
                0xe6, 0x37, 0x30, 0xb6,
            ]),
            declared_actions: ["create", "update", "validate"]
                .into_iter()
                .map(declared_action)
                .collect(),
            supported_modes: vec![
                BmadInvocationMode::Interactive,
                BmadInvocationMode::Headless,
            ],
            predecessor: None,
            follower: None,
            // Step 3 ("Load config") explicitly resolves `{planning_artifacts}`.
            output_locations: vec!["planning_artifacts".to_owned()],
            // Both DESIGN.md and EXPERIENCE.md are markdown documents; see
            // module doc for why this stays a single `Document` entry.
            artifact_kinds: vec![BmadArtifactKind::Document],
            completion_hooks: vec![BmadArtifactCompletionHookRunHook::PersistArtifacts],
            declared_resources: vec![
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-ux/SKILL.md",
                    "sha256:250d2794f2b8e316bb1ba4666f0728970afd8a5d2707ac1cceac6c21e63730b6",
                ),
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-ux/customize.toml",
                    "sha256:61f8bfbb020beb159b8da570ed416b02ec4a9ba3709aa0d08b417648d85328d8",
                ),
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-ux/assets/color-themes.md",
                    "sha256:48fac7943e8584ccf7dd1bcdfdfd4513366a086a3a84965b6b302ccf6607c171",
                ),
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-ux/assets/design-directions.md",
                    "sha256:eab73b948da76a27abc908e1e7d6b124e8ff0362e6dd1990f7e60b9541cbcb98",
                ),
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-ux/assets/design-example-editorial.md",
                    "sha256:a659166845b1a4ed3f9aef942c5935537a97382b10303684fbb78ed86265870e",
                ),
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-ux/assets/design-example-mobile.md",
                    "sha256:a93fb9f114f2e83af0c961c0d7909b3859fc80bcd713715517cd639a53a0372a",
                ),
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-ux/assets/design-example-shadcn.md",
                    "sha256:25e0dfb2bb21483973a8f0a6ed650288b3dc2ae5ed02aa0f091bb9b6b0b39fc0",
                ),
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-ux/assets/excalidraw-wireframe.md",
                    "sha256:660bbcdebff50cd044a3752eda43f3cb303aca4b0f521b55a270da93b89ddb18",
                ),
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-ux/assets/experience-example-mobile.md",
                    "sha256:f8cd54c92193cd8c34a472df9c212ac7330976f6a115b729721dcb60034dc4f0",
                ),
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-ux/assets/experience-example-shadcn.md",
                    "sha256:d6ca16e87b768e8b41f352c02ee08af21d7cdac04ca21e6ae1556864b4d27590",
                ),
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-ux/assets/headless-schemas.md",
                    "sha256:e3a773dde85286f1a6f6dd8b26fc17e09ccea96f6257dfe7fb436353cd79242d",
                ),
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-ux/assets/key-screens.md",
                    "sha256:ff1c5060673dd595edd61e76806bae876264ec6895c2eaadf8a6190785bf5f7b",
                ),
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-ux/assets/validation-report-template.html",
                    "sha256:8656945636fabe96624f8ef8fce931b073b5aecbe42d490584c1d5c7f31e1353",
                ),
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-ux/references/creative-tools.md",
                    "sha256:349b6b5e8353c2fa1efe7caa49cca50577900f55f32005ccd6f7b43e4daec558",
                ),
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-ux/references/design-md-spec.md",
                    "sha256:d04d5f663607aba3f3181e818ae33856e6caf5e560dcb16fa64ed80b7cf3d590",
                ),
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-ux/references/headless.md",
                    "sha256:8da7e23d269b1644f45b5104a243a6a27fd8d075aebb95d9b7752f0d143f0f5d",
                ),
                source_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-ux/references/validate.md",
                    "sha256:336d5be5ca860dc6948606b68b5a14fed593d41508ad83e123879bd466c984db",
                ),
            ],
            support_state: BmadSkillSupportState::Exact,
        })
        .unwrap_or_else(|_| unreachable!("this adapter's fields satisfy the harness's own bounds"))
    }

    fn execution_contract(&self) -> Result<BmadSkillExecutionContract, BmadSkillExecutionError> {
        let descriptor = self.descriptor();
        let source_hash = descriptor.source_sha256();
        let validation_hash =
            digest("sha256:336d5be5ca860dc6948606b68b5a14fed593d41508ad83e123879bd466c984db")?;
        let design = artifact_expectation(
            "expectation_01J00000000000000000000010",
            "{doc_workspace}/DESIGN.md",
            "sha256:d04d5f663607aba3f3181e818ae33856e6caf5e560dcb16fa64ed80b7cf3d590",
        )?;
        let experience = artifact_expectation(
            "expectation_01J00000000000000000000011",
            "{doc_workspace}/EXPERIENCE.md",
            "sha256:8da7e23d269b1644f45b5104a243a6a27fd8d075aebb95d9b7752f0d143f0f5d",
        )?;
        let validation = artifact_expectation(
            "expectation_01J00000000000000000000012",
            "{doc_workspace}/validation-report.{md,html}",
            "sha256:8656945636fabe96624f8ef8fce931b073b5aecbe42d490584c1d5c7f31e1353",
        )?;
        let mut branches = Vec::new();
        for mode in [
            BmadInvocationMode::Interactive,
            BmadInvocationMode::Headless,
        ] {
            branches.push(BmadSkillExecutionBranch {
                action: "create".to_owned(),
                mode,
                workflow: BmadSkillExecutionWorkflow {
                    initial_step_key: "create_discovery".to_owned(),
                    steps: vec![
                        execution_step("create_discovery", Some("create_finalize"), source_hash),
                        execution_step("create_finalize", None, source_hash),
                    ],
                },
                artifact_expectations: vec![design.clone(), experience.clone()],
                required_status_fields: Vec::new(),
            });
            branches.push(BmadSkillExecutionBranch {
                action: "update".to_owned(),
                mode,
                workflow: BmadSkillExecutionWorkflow {
                    initial_step_key: "update_finalize".to_owned(),
                    steps: vec![execution_step("update_finalize", None, source_hash)],
                },
                artifact_expectations: vec![design.clone(), experience.clone()],
                required_status_fields: Vec::new(),
            });
            branches.push(BmadSkillExecutionBranch {
                action: "validate".to_owned(),
                mode,
                workflow: BmadSkillExecutionWorkflow {
                    initial_step_key: "validate".to_owned(),
                    steps: vec![BmadSkillExecutionStep {
                        step_key: "validate".to_owned(),
                        next_step_key: None,
                        source_resource_path:
                            "src/bmm-skills/2-plan-workflows/bmad-ux/references/validate.md"
                                .to_owned(),
                        source_resource_hash: validation_hash,
                    }],
                },
                artifact_expectations: vec![validation.clone()],
                required_status_fields: Vec::new(),
            });
        }

        BmadSkillExecutionContract::new(BmadSkillExecutionContractInput {
            descriptor: descriptor.clone(),
            package_name: "bmad-method".to_owned(),
            package_version: "6.10.0".to_owned(),
            package_version_id: contract_id(
                "pkgver_8B33C55A4D67D0B258FEDBB75D1CB09DBC7F711BC9BDC794D8B052B31FCE6D86",
            )?,
            package_descriptor_hash: digest(
                "sha256:11de78a7306e80753517c6cb8574bbef17f003c1615da895bcb462808fb340a1",
            )?,
            package_source_hash: digest(
                "sha256:8b33c55a4d67d0b258fedbb75d1cb09dbc7f711bc9bdc794d8b052b31fce6d86",
            )?,
            instruction_projection_hash: digest(
                "sha256:4b3740d899149d3d622e1c948e187edc80bb60eddaa94c6f81cc84bdbdbcb3a1",
            )?,
            capability_catalog_hash: digest(
                "sha256:141c49232e202e4981244e13d336f840998c065503d418e218516da7e6b16515",
            )?,
            distribution_profile: "sapphirus_package".to_owned(),
            install_profile: "SapphirusManagedV1".to_owned(),
            execution_profile: execution_profile(&descriptor)?,
            validation_profile_hash: digest(
                "sha256:3e851de810792a1caacb2f4e4c1ac2134359f5b9da8c3870c6c37b967b03efba",
            )?,
            method_schema_hash: digest(
                "sha256:dda02f453dcf4edbb3fc66e0ad3a7c59d7087f5b44b526e40c435456c8c248c2",
            )?,
            branches,
            tool_intents: vec![
                BmadSkillToolIntent::FileRead,
                BmadSkillToolIntent::FileWrite,
                BmadSkillToolIntent::Process,
                BmadSkillToolIntent::Web,
                BmadSkillToolIntent::Subagent,
                BmadSkillToolIntent::Browser,
                BmadSkillToolIntent::ExternalHandoff,
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
                actions: vec![
                    "create".to_owned(),
                    "update".to_owned(),
                    "validate".to_owned(),
                ],
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
                "browser".to_owned(),
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
        digest("sha256:250d2794f2b8e316bb1ba4666f0728970afd8a5d2707ac1cceac6c21e63730b6")?,
    )
    .map_err(|_| BmadSkillExecutionError::ContractInvalid)
}

fn execution_step(
    step_key: &str,
    next_step_key: Option<&str>,
    source_resource_hash: Sha256Digest,
) -> BmadSkillExecutionStep {
    BmadSkillExecutionStep {
        step_key: step_key.to_owned(),
        next_step_key: next_step_key.map(str::to_owned),
        source_resource_path: "src/bmm-skills/2-plan-workflows/bmad-ux/SKILL.md".to_owned(),
        source_resource_hash,
    }
}

fn artifact_expectation(
    id: &str,
    source_output_hint: &str,
    expectation_hash: &str,
) -> Result<MethodArtifactExpectation, BmadSkillExecutionError> {
    MethodArtifactExpectation::from_source(MethodArtifactExpectation {
        expectation_kind: "method_artifact".to_owned(),
        expectation_id: contract_id(id)?,
        artifact_kind: "document".to_owned(),
        required: true,
        storage_scope: "app_local".to_owned(),
        expected_media_type: "text/markdown".to_owned(),
        expected_content_schema_hash: None,
        source_output_hint: Some(source_output_hint.to_owned()),
        completion_evidence_class: MethodEvidenceClass::Authoritative,
        expectation_hash: digest(expectation_hash)?,
    })
    .map_err(|_| BmadSkillExecutionError::ContractInvalid)
}

fn declared_action(name: &str) -> BmadInvocationDeclaredAction {
    let name: BmadInvocationDeclaredActionName = name
        .try_into()
        .unwrap_or_else(|_| unreachable!("the three pinned UX action names are canonical"));
    BmadInvocationDeclaredAction {
        declared_inputs: Vec::new(),
        name,
    }
}

fn source_resource(path: &str, content_hash: &str) -> BmadResource {
    let content_hash = Sha256Digest::parse(content_hash)
        .unwrap_or_else(|_| unreachable!("all pinned UX digests are canonical"));
    BmadResource::new(path, content_hash)
        .unwrap_or_else(|_| unreachable!("all pinned UX resource paths are canonical"))
}

fn digest(value: &str) -> Result<Sha256Digest, BmadSkillExecutionError> {
    Sha256Digest::parse(value).map_err(|_| BmadSkillExecutionError::ContractInvalid)
}

fn contract_id(value: &str) -> Result<ContractId, BmadSkillExecutionError> {
    ContractId::new(value).map_err(|_| BmadSkillExecutionError::ContractInvalid)
}
