//! Source-bound adapter for Method 6.10.0 `bmad-deep-recon`.
//!
//! The adapter models the consolidated Draft, Process, Run, Refresh, and
//! Deepen services as action-selected workflows.  It only emits inert Method
//! step tables and typed tool intents; persistence and every effect remain
//! owned by the shared Method service and host Airlock.

use crate::generated_contracts::{
    BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationDeclaredAction,
    BmadInvocationMode,
};
use crate::{
    BmadMethodSkill, BmadResource, BmadSkillDescriptor, BmadSkillDescriptorInput,
    BmadSkillExecutionBranch, BmadSkillExecutionContract, BmadSkillExecutionContractInput,
    BmadSkillExecutionError, BmadSkillExecutionStep, BmadSkillExecutionWorkflow,
    BmadSkillRecoverySemantics, BmadSkillSupportState, BmadSkillToolIntent, ContractId,
    MethodArtifactExpectation, MethodEvidenceClass, MethodExecutionProfile,
    MethodExecutionProfileData, MethodInvocationModes, MethodResourcePolicy,
    MethodRuntimeRequirement, Sha256Digest,
};

/// Adapter for the pinned `bmad-deep-recon` Method skill.
pub struct BmadDeepReconSkill;

impl BmadMethodSkill for BmadDeepReconSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "core:bmad-deep-recon".to_owned(),
            source_path: "src/core-skills/bmad-deep-recon/SKILL.md".to_owned(),
            source_sha256: digest_bytes(
                "dce5f7db03094a617cc3fcae1711b1dc7ccebe379376e67216544956d375f0d2",
            ),
            declared_actions: ["draft", "process", "run", "refresh", "deepen"]
                .into_iter()
                .map(action)
                .collect(),
            supported_modes: vec![
                BmadInvocationMode::Interactive,
                BmadInvocationMode::Headless,
            ],
            predecessor: None,
            follower: Some("core:bmad-help".to_owned()),
            output_locations: vec!["research_output_path".to_owned()],
            artifact_kinds: vec![BmadArtifactKind::Document, BmadArtifactKind::FileSet],
            completion_hooks: vec![BmadArtifactCompletionHookRunHook::PersistArtifacts],
            declared_resources: resources(),
            support_state: BmadSkillSupportState::Exact,
        })
        .unwrap_or_else(|_| unreachable!("pinned Deep Recon descriptor is valid"))
    }

    fn execution_contract(&self) -> Result<BmadSkillExecutionContract, BmadSkillExecutionError> {
        contract(self.descriptor(), "run")
    }

    fn execution_contract_for_action(
        &self,
        action: Option<&str>,
    ) -> Result<BmadSkillExecutionContract, BmadSkillExecutionError> {
        contract(self.descriptor(), action.unwrap_or("run"))
    }
}

fn action(name: &str) -> BmadInvocationDeclaredAction {
    BmadInvocationDeclaredAction {
        declared_inputs: Vec::new(),
        name: name
            .try_into()
            .unwrap_or_else(|_| unreachable!("source action names are valid")),
    }
}

fn contract(
    descriptor: BmadSkillDescriptor,
    selected_action: &str,
) -> Result<BmadSkillExecutionContract, BmadSkillExecutionError> {
    let execution_profile = execution_profile(&descriptor)?;
    let artifact_expectations = vec![
        expectation(
            "expectation_01J00000000000000000000038",
            "document",
            "research_output_path/research.md",
            "sha256:55e8fbcf413e2e64c95f1c2252d14dcbbeea4f1b991d67e35a328cc4bc92c68f",
        )?,
        expectation(
            "expectation_01J00000000000000000000039",
            "file_set",
            "research_output_path/{brief.md,imports/,digests/,.memlog.md}",
            "sha256:16f8e2c0414d5f6e3251466be124a493d88e856cae2b1847d0708bead2844ae5",
        )?,
    ];
    let mut branches = Vec::new();
    for action in std::iter::once(selected_action).chain(
        ["draft", "process", "run", "refresh", "deepen"]
            .into_iter()
            .filter(|action| *action != selected_action),
    ) {
        let workflow = workflow(action)?;
        for mode in [
            BmadInvocationMode::Interactive,
            BmadInvocationMode::Headless,
        ] {
            branches.push(BmadSkillExecutionBranch {
                action: action.to_owned(),
                mode,
                workflow: workflow.clone(),
                artifact_expectations: artifact_expectations.clone(),
                required_status_fields: Vec::new(),
            });
        }
    }
    BmadSkillExecutionContract::new(BmadSkillExecutionContractInput {
        descriptor,
        package_name: "bmad-method".to_owned(),
        package_version: "6.10.0".to_owned(),
        package_version_id: contract_id(
            "pkgver_8B33C55A4D67D0B258FEDBB75D1CB09DBC7F711BC9BDC794D8B052B31FCE6D86",
        )?,
        package_descriptor_hash: digest(
            "sha256:23f0e2ced9ce9be4083eb1989404dc57c371e4bbd3df183a18427564bf267de2",
        )?,
        package_source_hash: digest(
            "sha256:8b33c55a4d67d0b258fedbb75d1cb09dbc7f711bc9bdc794d8b052b31fce6d86",
        )?,
        instruction_projection_hash: digest(
            "sha256:a7c8915c301df1ad97b5bf7900b113aef437a0382a3785c13f49d7982d8517ce",
        )?,
        capability_catalog_hash: digest(
            "sha256:1504c2276d970a389f7033aa2644d401cdc66bf1297db0ce3652cfa6e992e2c2",
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

fn workflow(action: &str) -> Result<BmadSkillExecutionWorkflow, BmadSkillExecutionError> {
    let paths: &[(&str, &str)] = match action {
        "draft" => &[
            ("select", "references/selection.md"),
            ("draft_brief", "references/draft.md"),
            ("handoff", "references/html-briefing.md"),
        ],
        "process" => &[
            ("select", "references/selection.md"),
            ("process_imports", "references/process.md"),
            ("synthesize", "references/synthesis.md"),
            ("finalize", "references/finalize.md"),
        ],
        "run" => &[
            ("select", "references/selection.md"),
            ("research", "references/run.md"),
            ("verify", "references/verification.md"),
            ("synthesize", "references/synthesis.md"),
            ("finalize", "references/finalize.md"),
        ],
        "refresh" | "deepen" => &[
            ("select", "references/selection.md"),
            ("resume", "references/lifecycle.md"),
            ("research", "references/run.md"),
            ("verify", "references/verification.md"),
            ("synthesize", "references/synthesis.md"),
            ("finalize", "references/finalize.md"),
        ],
        _ => return Err(BmadSkillExecutionError::UnsupportedInvocation),
    };
    let base = "src/core-skills/bmad-deep-recon/";
    let steps = paths
        .iter()
        .enumerate()
        .map(|(index, (key, relative))| {
            let path = format!("{base}{relative}");
            let member = resources()
                .into_iter()
                .find(|candidate| candidate.path() == path)
                .ok_or(BmadSkillExecutionError::ResourceMismatch)?;
            Ok(BmadSkillExecutionStep {
                step_key: (*key).to_owned(),
                next_step_key: paths.get(index + 1).map(|(next, _)| (*next).to_owned()),
                source_resource_path: path,
                source_resource_hash: member.content_hash(),
            })
        })
        .collect::<Result<Vec<_>, BmadSkillExecutionError>>()?;
    Ok(BmadSkillExecutionWorkflow {
        initial_step_key: paths[0].0.to_owned(),
        steps,
    })
}

fn execution_profile(
    descriptor: &BmadSkillDescriptor,
) -> Result<MethodExecutionProfile, BmadSkillExecutionError> {
    let data = MethodExecutionProfileData {
        entrypoint_kind: "direct".to_owned(),
        invocation_modes: MethodInvocationModes {
            interactive: true,
            headless: true,
            actions: ["draft", "process", "run", "refresh", "deepen"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        },
        required_runtimes: vec![MethodRuntimeRequirement {
            runtime: "python".to_owned(),
            version_range: ">=3.10".to_owned(),
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
        declared_tool_intents: [
            "file_read",
            "file_write",
            "process",
            "web",
            "subagent",
            "browser",
            "external_handoff",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        state_hints: vec!["memlog".to_owned(), "artifact_workspace".to_owned()],
        completion_evidence: vec![
            "artifact".to_owned(),
            "event".to_owned(),
            "status_evidence".to_owned(),
        ],
        customization_profile: "method_skill_toml".to_owned(),
        validation_profile: "MethodOfficialSkillV6".to_owned(),
    };
    let hash = crate::canonical_hash("bmad-execution-profile", 1, &data)
        .map_err(|_| BmadSkillExecutionError::ContractInvalid)?;
    MethodExecutionProfile::from_source(data, hash)
        .map_err(|_| BmadSkillExecutionError::ContractInvalid)
}

fn expectation(
    id: &str,
    kind: &str,
    output_hint: &str,
    hash: &str,
) -> Result<MethodArtifactExpectation, BmadSkillExecutionError> {
    MethodArtifactExpectation::from_source(MethodArtifactExpectation {
        expectation_kind: "method_artifact".to_owned(),
        expectation_id: contract_id(id)?,
        artifact_kind: kind.to_owned(),
        required: true,
        storage_scope: "app_local".to_owned(),
        expected_media_type: "text/markdown".to_owned(),
        expected_content_schema_hash: None,
        source_output_hint: Some(output_hint.to_owned()),
        completion_evidence_class: MethodEvidenceClass::Authoritative,
        expectation_hash: digest(hash)?,
    })
    .map_err(|_| BmadSkillExecutionError::ContractInvalid)
}

fn resources() -> Vec<BmadResource> {
    [
        (
            "assets/research.template.md",
            "87d7cfa85fa5036b5a309b7bfb88f7934f50bb81f3cff02d2b49b4c7345cfc38",
        ),
        (
            "customize.toml",
            "24d1383066034fd4fcf282279aaf0d29d21910fd1247dfe3dbad74a70e78814d",
        ),
        (
            "references/draft.md",
            "ac19b33a00ca529c3281df6076f87a833a3b11fc30915ca2c6e7590f7986b90e",
        ),
        (
            "references/finalize.md",
            "d2ba4bee615dfbca6e7725179fb5ebd856dd34a54ba3f306e25255dfd7b1d126",
        ),
        (
            "references/html-briefing.md",
            "cd3a88c1a5505fa7f1b715afc3adb34f3d64ade66f4c6b49a59f7e8cfcbe38d4",
        ),
        (
            "references/lifecycle.md",
            "2315fd9cb9223504639616b861ad593bfecb53d605efbd598ae2cf94fb6c4cdf",
        ),
        (
            "references/process.md",
            "9017d04774807a29103ea97331e555e2793f90098af65752b72f3ee09df85906",
        ),
        (
            "references/run.md",
            "8e259dc88820ac989c79620af5c37e24f00df21ac4af308ce3563c0e66c9406c",
        ),
        (
            "references/selection.md",
            "2b488c08f9550afe7fda32c2dc5d4591927afd7b9ad507fcce0ae8593c4b9d31",
        ),
        (
            "references/synthesis.md",
            "4d9ef1247ab45241f919010e94c99699abc0a6e18b5bd68fa93ae54afe59d172",
        ),
        (
            "references/verification.md",
            "0393ded4828fec69a4819d7236827910d641d09c3b61144c0211a9a78776fdd2",
        ),
        (
            "scripts/recon_kit.py",
            "8cb5187b5114b779532907383a013d40fdea1022bb9bed9a23bd8d0fdfa79e8c",
        ),
        (
            "scripts/tests/test_recon_kit.py",
            "1b4ddf482dd2d11a214ee2f821c35d7a8c41905b5d8d9cc69a293304bb8a1665",
        ),
        (
            "SKILL.md",
            "dce5f7db03094a617cc3fcae1711b1dc7ccebe379376e67216544956d375f0d2",
        ),
        (
            "types/academic-lit.md",
            "514b0b7b1184cb95363dedeb0c2298a9bf5c430c74146fe2114f31780fadb325",
        ),
        (
            "types/competitive.md",
            "39b53df87d295fdc4388719fcdf31ad2221e64afc136e669207f46d2727a3aec",
        ),
        (
            "types/domain.md",
            "3a34b2e62c3589f8c8d01158357ee9fba32648c9fdfe70035bb3ca971862896b",
        ),
        (
            "types/market.md",
            "81ac04499da441857672e29fcd7169436a7e239e67194288ac0d526ec514cb5e",
        ),
        (
            "types/technical.md",
            "55f71428c70547b89d67ef0c6cdad59caeb0ee21082cd1230c05618ba8d04707",
        ),
        (
            "types/user-voice.md",
            "1831ce46e53e8c68244d28e336c5ef9e53e64f21a00b74a817b9b2924c1e8e72",
        ),
    ]
    .into_iter()
    .map(|(relative, hash)| {
        BmadResource::new(
            format!("src/core-skills/bmad-deep-recon/{relative}"),
            digest_bytes(hash),
        )
        .unwrap_or_else(|_| unreachable!("pinned Deep Recon resource path is valid"))
    })
    .collect()
}

fn digest_bytes(hex: &str) -> Sha256Digest {
    Sha256Digest::parse(&format!("sha256:{hex}"))
        .unwrap_or_else(|_| unreachable!("pinned Deep Recon digest is valid"))
}

fn digest(value: &str) -> Result<Sha256Digest, BmadSkillExecutionError> {
    Sha256Digest::parse(value).map_err(|_| BmadSkillExecutionError::ContractInvalid)
}

fn contract_id(value: &str) -> Result<ContractId, BmadSkillExecutionError> {
    ContractId::new(value).map_err(|_| BmadSkillExecutionError::ContractInvalid)
}
