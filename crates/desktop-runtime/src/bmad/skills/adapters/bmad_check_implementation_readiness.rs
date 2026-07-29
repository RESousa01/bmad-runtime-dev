//! The `bmad-check-implementation-readiness` Method adapter.
//!
//! The pinned skill is a six-step, just-in-time workflow. This adapter keeps
//! those step files as immutable resources and compiles their order into the
//! shared Method step table. It does not execute the Python customization
//! resolver, inspect project files, write the report, call a model, or invoke
//! `bmad-help`; those source-declared effects become typed tool intents and
//! therefore inert action-plan requirements at this authority boundary.

use crate::generated_contracts::{
    BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationMode,
};
use crate::{
    BmadMethodSkill, BmadResource, BmadSkillDescriptor, BmadSkillDescriptorInput,
    BmadSkillExecutionContract, BmadSkillExecutionContractInput, BmadSkillExecutionError,
    BmadSkillExecutionStep, BmadSkillExecutionWorkflow, BmadSkillRecoverySemantics,
    BmadSkillSupportState, BmadSkillToolIntent, ContractId, MethodArtifactExpectation,
    MethodEvidenceClass, MethodExecutionProfile, MethodExecutionProfileData, MethodInvocationModes,
    MethodResourcePolicy, MethodRuntimeRequirement, Sha256Digest,
};

const ROOT: &str = "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness";
const SKILL_PATH: &str =
    "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/SKILL.md";

/// Adapter for the pinned implementation-readiness workflow (MTH-13).
pub struct BmadCheckImplementationReadinessSkill;

impl BmadMethodSkill for BmadCheckImplementationReadinessSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "bmm:bmad-check-implementation-readiness".to_owned(),
            source_path: SKILL_PATH.to_owned(),
            source_sha256: source_hash(
                "sha256:a7e12eae2bdb5a95cc0534e6d5bf58e7ce4b947327a494d475ef98e67f03718a",
            ),
            declared_actions: Vec::new(),
            supported_modes: vec![
                BmadInvocationMode::Interactive,
                BmadInvocationMode::Headless,
            ],
            predecessor: None,
            // Final Assessment explicitly says to invoke `bmad-help`.
            follower: Some("bmm:bmad-help".to_owned()),
            output_locations: vec!["planning_artifacts".to_owned()],
            artifact_kinds: vec![BmadArtifactKind::Document],
            completion_hooks: vec![BmadArtifactCompletionHookRunHook::PersistArtifacts],
            declared_resources: vec![
                resource(
                    SKILL_PATH,
                    "sha256:a7e12eae2bdb5a95cc0534e6d5bf58e7ce4b947327a494d475ef98e67f03718a",
                ),
                resource(
                    &format!("{ROOT}/customize.toml"),
                    "sha256:571d81087b7a6d914cf7c2f21aa26354eac385bb2f5cfeabc591892cbb98d036",
                ),
                resource(
                    &format!("{ROOT}/steps/step-01-document-discovery.md"),
                    "sha256:c763b512d55906122433cb65c1bcd5b5b283e45eacdc07281c8ec7596b6b3980",
                ),
                resource(
                    &format!("{ROOT}/steps/step-02-prd-analysis.md"),
                    "sha256:38be2bf4b924c0b5b395b57d68f685d790ade7b1a6c10993d3c550675f87d954",
                ),
                resource(
                    &format!("{ROOT}/steps/step-03-epic-coverage-validation.md"),
                    "sha256:7b187f03a47cba0325fcfd10240410db9c59d93768342fc2dd3de2a01ec23356",
                ),
                resource(
                    &format!("{ROOT}/steps/step-04-ux-alignment.md"),
                    "sha256:f71e5f0d77615e885ae40fdee6b04c1dd6e472c871f87b515fe869cb5f6966fb",
                ),
                resource(
                    &format!("{ROOT}/steps/step-05-epic-quality-review.md"),
                    "sha256:d8a84e57f4e3a321734b5b5d093458ceb1e338744f18954c5a204f5ce3576185",
                ),
                resource(
                    &format!("{ROOT}/steps/step-06-final-assessment.md"),
                    "sha256:1b9c8c2a21950f85e052e05309bfcb3bfe38157ba6fca402ccf37444e7f1cee3",
                ),
                resource(
                    &format!("{ROOT}/templates/readiness-report-template.md"),
                    "sha256:0da97ab1e38818e642f36dc0ef24d2dae69fc6e0be59924dc2dbf44329738ff6",
                ),
            ],
            support_state: BmadSkillSupportState::Exact,
        })
        .unwrap_or_else(|_| unreachable!("pinned readiness descriptor is bounded and canonical"))
    }

    fn execution_contract(&self) -> Result<BmadSkillExecutionContract, BmadSkillExecutionError> {
        let descriptor = self.descriptor();
        let execution_profile = execution_profile(&descriptor)?;

        BmadSkillExecutionContract::new(BmadSkillExecutionContractInput {
            descriptor,
            package_name: "bmad-method".to_owned(),
            package_version: "6.10.0".to_owned(),
            package_version_id: contract_id(
                "pkgver_8B33C55A4D67D0B258FEDBB75D1CB09DBC7F711BC9BDC794D8B052B31FCE6D86",
            )?,
            package_descriptor_hash: digest(
                "sha256:cd406cecc48f302b9969f208a4adbf7b2a8d10a3d1d3fb67ccd3116042776c92",
            )?,
            package_source_hash: digest(
                "sha256:8b33c55a4d67d0b258fedbb75d1cb09dbc7f711bc9bdc794d8b052b31fce6d86",
            )?,
            instruction_projection_hash: digest(
                "sha256:8335100a0c468f03cf6187a0dc58dcc0435328727c1740f7c70b947122a58cca",
            )?,
            capability_catalog_hash: digest(
                "sha256:4aa54a86e1bd868cb84cfcdb4d790a088dd0d2f370ac230c1082092acb5be60f",
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
            workflow: workflow()?,
            tool_intents: vec![
                BmadSkillToolIntent::FileRead,
                BmadSkillToolIntent::FileWrite,
                BmadSkillToolIntent::Process,
                BmadSkillToolIntent::ExternalHandoff,
            ],
            artifact_expectations: vec![artifact_expectation()?],
            recovery: BmadSkillRecoverySemantics::MethodCheckpoint,
        })
    }
}

fn execution_profile(
    descriptor: &BmadSkillDescriptor,
) -> Result<MethodExecutionProfile, BmadSkillExecutionError> {
    MethodExecutionProfile::from_source(
        MethodExecutionProfileData {
            entrypoint_kind: "step_jit".to_owned(),
            invocation_modes: MethodInvocationModes {
                interactive: true,
                headless: true,
                actions: Vec::new(),
            },
            required_runtimes: vec![MethodRuntimeRequirement {
                runtime: "python".to_owned(),
                version_range: ">=3".to_owned(),
                required: false,
            }],
            resource_policy: MethodResourcePolicy {
                entrypoint_timing: "invocation_start".to_owned(),
                resource_timing: "current_step_only".to_owned(),
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
                "external_handoff".to_owned(),
            ],
            state_hints: vec!["memlog".to_owned(), "artifact_workspace".to_owned()],
            completion_evidence: vec![
                "artifact".to_owned(),
                "event".to_owned(),
                "status_evidence".to_owned(),
            ],
            customization_profile: "method_skill_toml".to_owned(),
            validation_profile: "MethodStepWorkflowV6".to_owned(),
        },
        digest("sha256:4dce9489113476ed4da1944a00b88c2b4eef3e1509a2c0be7b5bb1623c1d1fe2")?,
    )
    .map_err(|_| BmadSkillExecutionError::ContractInvalid)
}

fn workflow() -> Result<BmadSkillExecutionWorkflow, BmadSkillExecutionError> {
    let steps = [
        (
            "document_discovery",
            Some("prd_analysis"),
            "step-01-document-discovery.md",
            "sha256:c763b512d55906122433cb65c1bcd5b5b283e45eacdc07281c8ec7596b6b3980",
        ),
        (
            "prd_analysis",
            Some("epic_coverage_validation"),
            "step-02-prd-analysis.md",
            "sha256:38be2bf4b924c0b5b395b57d68f685d790ade7b1a6c10993d3c550675f87d954",
        ),
        (
            "epic_coverage_validation",
            Some("ux_alignment"),
            "step-03-epic-coverage-validation.md",
            "sha256:7b187f03a47cba0325fcfd10240410db9c59d93768342fc2dd3de2a01ec23356",
        ),
        (
            "ux_alignment",
            Some("epic_quality_review"),
            "step-04-ux-alignment.md",
            "sha256:f71e5f0d77615e885ae40fdee6b04c1dd6e472c871f87b515fe869cb5f6966fb",
        ),
        (
            "epic_quality_review",
            Some("final_assessment"),
            "step-05-epic-quality-review.md",
            "sha256:d8a84e57f4e3a321734b5b5d093458ceb1e338744f18954c5a204f5ce3576185",
        ),
        (
            "final_assessment",
            None,
            "step-06-final-assessment.md",
            "sha256:1b9c8c2a21950f85e052e05309bfcb3bfe38157ba6fca402ccf37444e7f1cee3",
        ),
    ];
    Ok(BmadSkillExecutionWorkflow {
        initial_step_key: "document_discovery".to_owned(),
        steps: steps
            .into_iter()
            .map(
                |(step_key, next_step_key, file, hash)| -> Result<_, BmadSkillExecutionError> {
                    Ok(BmadSkillExecutionStep {
                        step_key: step_key.to_owned(),
                        next_step_key: next_step_key.map(str::to_owned),
                        source_resource_path: format!("{ROOT}/steps/{file}"),
                        source_resource_hash: digest(hash)?,
                    })
                },
            )
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn artifact_expectation() -> Result<MethodArtifactExpectation, BmadSkillExecutionError> {
    MethodArtifactExpectation::from_source(MethodArtifactExpectation {
        expectation_kind: "method_artifact".to_owned(),
        expectation_id: contract_id("expectation_01J00000000000000000000013")?,
        artifact_kind: "document".to_owned(),
        required: true,
        storage_scope: "app_local".to_owned(),
        expected_media_type: "text/markdown".to_owned(),
        expected_content_schema_hash: None,
        source_output_hint: Some(
            "{planning_artifacts}/implementation-readiness-report-{{date}}.md".to_owned(),
        ),
        completion_evidence_class: MethodEvidenceClass::Authoritative,
        expectation_hash: digest(
            "sha256:77677e4bf282ffc1d8d843d3e9403c7b11895c687a425da15966d370fffa37b2",
        )?,
    })
    .map_err(|_| BmadSkillExecutionError::ContractInvalid)
}

fn resource(path: &str, hash: &str) -> BmadResource {
    BmadResource::new(path, source_hash(hash))
        .unwrap_or_else(|_| unreachable!("pinned readiness resource path is canonical"))
}

fn source_hash(value: &str) -> Sha256Digest {
    Sha256Digest::parse(value)
        .unwrap_or_else(|_| unreachable!("pinned readiness resource hash is canonical"))
}

fn digest(value: &str) -> Result<Sha256Digest, BmadSkillExecutionError> {
    Sha256Digest::parse(value).map_err(|_| BmadSkillExecutionError::ContractInvalid)
}

fn contract_id(value: &str) -> Result<ContractId, BmadSkillExecutionError> {
    ContractId::new(value).map_err(|_| BmadSkillExecutionError::ContractInvalid)
}
