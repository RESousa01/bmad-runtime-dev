use std::{fmt, sync::Arc};

use serde_json::json;

use crate::{canonical_hash, generated_contracts, Sha256Digest};

use super::{
    BmadCapabilityKey, BmadCatalog, BmadCatalogAvailability, BmadEntrypointKind, BmadHelpAction,
    BmadInstalledSkillRecord, BmadKernelError, BmadKernelErrorCode, BmadSealedHelpInvocation,
    MethodExactBinding, MethodExecutionProfile, MethodExecutionProfileData, MethodInvocationModes,
    MethodModelBinding, MethodModelBindingData, MethodResourcePolicy, MethodRuntimeRequirement,
    MethodStepTable,
};

const HELP_ACTION_GRAPH_HASH: &str =
    "sha256:bf744053f998e035b9a9013f158e2a89db26a6cb59b2d4b2574cdeb33bbc16aa";
const HELP_CUSTOMIZATION_HASH: &str =
    "sha256:41d2f0d68d0a47e8fb9eeccd89f0409f2ab08a72eb1a76500d87a0919ecb9c8a";
const HELP_VALIDATION_PROFILE_HASH: &str =
    "sha256:ddd086622be73b637cdcb3562b4459ac3853f1aeae34a53c43775af66e4cbdf0";

const ARCHITECTURE_ROW: [&str; 13] = [
    "BMad Method",
    "bmad-architecture",
    "Architecture",
    "CA",
    "Offer once requirements exist (a PRD or spec; plus UX if present) and the user is ready to move from what to how. Also offer any time independently-built parts risk diverging. Produces the architecture spine: the invariants that keep features epics and stories consistent. Comes before epics and stories and scales from a quick spine to a full architecture (brownfield: ratifies the existing codebase).",
    "",
    "",
    "3-solutioning",
    "",
    "",
    "true",
    "planning_artifacts",
    "architecture",
];
const HELP_ROW: [&str; 13] = [
    "Core",
    "bmad-help",
    "BMad Help",
    "BH",
    "",
    "",
    "",
    "anytime",
    "",
    "",
    "false",
    "",
    "",
];

/// Trusted, inert D2 model/profile facts used to compile a Help plan.
///
/// This value is a host assertion. It is deliberately not consent evidence and
/// does not prove that egress or a model invocation occurred.
#[derive(Clone)]
pub struct BmadTrustedHelpModelProfileData {
    pub provider_id: String,
    pub model_id: String,
    pub deployment_id: String,
    pub model_profile_hash: Sha256Digest,
    pub model_capability_hash: Sha256Digest,
    pub context_window_profile_hash: Sha256Digest,
    pub egress_profile_hash: Sha256Digest,
    pub request_schema_hash: Sha256Digest,
}

/// Opaque, validated model/profile facts for an inert Help binding.
#[derive(Clone)]
pub struct BmadTrustedHelpModelProfile {
    binding: MethodModelBinding,
}

impl fmt::Debug for BmadTrustedHelpModelProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BmadTrustedHelpModelProfile")
            .field("model", &"<redacted>")
            .field("binding_hash", &self.binding.binding_hash)
            .finish()
    }
}

impl BmadTrustedHelpModelProfile {
    /// Validates host-owned model/profile facts and binds the generated Help
    /// proposal closure as the only accepted response schema.
    ///
    /// # Errors
    ///
    /// Fails closed when the host assertion is malformed or a generated schema
    /// identity cannot be parsed.
    pub fn from_host_assertion(
        source: BmadTrustedHelpModelProfileData,
    ) -> Result<Self, BmadKernelError> {
        let response_schema_hash =
            generated_digest(generated_contracts::BMAD_METHOD_HELP_PROPOSAL_SCHEMA_CLOSURE_SHA256)?;
        let data = MethodModelBindingData {
            binding_kind: "method_model".to_owned(),
            provider_id: source.provider_id,
            model_id: source.model_id,
            deployment_id: source.deployment_id,
            model_profile_hash: source.model_profile_hash,
            model_capability_hash: source.model_capability_hash,
            context_window_profile_hash: source.context_window_profile_hash,
            egress_profile_hash: source.egress_profile_hash,
            request_schema_hash: source.request_schema_hash,
            response_schema_hash,
        };
        let binding_hash = canonical_hash("bmad-method-model-binding", 1, &data)
            .map_err(|_| binding_mismatch())?;
        let binding =
            MethodModelBinding::from_source(data, binding_hash).map_err(|_| binding_mismatch())?;
        Ok(Self { binding })
    }

    #[must_use]
    pub const fn model_binding_hash(&self) -> Sha256Digest {
        self.binding.binding_hash
    }

    #[must_use]
    pub const fn egress_profile_hash(&self) -> Sha256Digest {
        self.binding.data.egress_profile_hash
    }

    #[must_use]
    pub const fn request_schema_hash(&self) -> Sha256Digest {
        self.binding.data.request_schema_hash
    }
}

/// Exact, sealed Help composition. It remains intentionally non-runnable.
///
/// The aggregate owns shared instruction bytes and native catalog candidates,
/// but has no Serde implementation or arbitrary-byte constructor.
///
/// ```compile_fail
/// use desktop_runtime::BmadCompiledHelpInvocation;
/// let _ = BmadCompiledHelpInvocation::new(Vec::new());
/// ```
///
/// ```compile_fail
/// use desktop_runtime::BmadCompiledHelpInvocation;
/// fn requires_serialize<T: serde::Serialize>() {}
/// requires_serialize::<BmadCompiledHelpInvocation>();
/// ```
///
/// ```compile_fail
/// use desktop_runtime::BmadCompiledHelpInvocation;
/// fn requires_deserialize<T: for<'de> serde::Deserialize<'de>>() {}
/// requires_deserialize::<BmadCompiledHelpInvocation>();
/// ```
#[derive(Clone)]
pub struct BmadCompiledHelpInvocation {
    instruction_bytes: Arc<[u8]>,
    catalog_candidates: Arc<[BmadHelpAction]>,
    exact_binding: MethodExactBinding,
    step_table: MethodStepTable,
    request_schema_hash: Sha256Digest,
    proposal_schema_closure_hash: Sha256Digest,
    recommendation_schema_closure_hash: Sha256Digest,
    result_schema_closure_hash: Sha256Digest,
    customization_hash: Sha256Digest,
    validation_profile_hash: Sha256Digest,
}

impl fmt::Debug for BmadCompiledHelpInvocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BmadCompiledHelpInvocation")
            .field(
                "instruction_bytes",
                &format_args!("<redacted:{} bytes>", self.instruction_bytes.len()),
            )
            .field("catalog_candidates", &self.catalog_candidates.len())
            .field("capability_key", &self.exact_binding.capability_key)
            .field("runnable", &false)
            .finish_non_exhaustive()
    }
}

impl BmadCompiledHelpInvocation {
    #[must_use]
    pub const fn runnable(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn completion_claimed(&self) -> bool {
        false
    }

    #[must_use]
    pub fn instruction_bytes(&self) -> &[u8] {
        &self.instruction_bytes
    }

    #[must_use]
    pub fn catalog_candidates(&self) -> &[BmadHelpAction] {
        &self.catalog_candidates
    }

    #[must_use]
    pub const fn exact_binding(&self) -> &MethodExactBinding {
        &self.exact_binding
    }

    #[must_use]
    pub const fn step_table(&self) -> &MethodStepTable {
        &self.step_table
    }

    #[must_use]
    pub const fn request_schema_hash(&self) -> Sha256Digest {
        self.request_schema_hash
    }

    #[must_use]
    pub const fn proposal_schema_closure_hash(&self) -> Sha256Digest {
        self.proposal_schema_closure_hash
    }

    #[must_use]
    pub const fn recommendation_schema_closure_hash(&self) -> Sha256Digest {
        self.recommendation_schema_closure_hash
    }

    #[must_use]
    pub const fn result_schema_closure_hash(&self) -> Sha256Digest {
        self.result_schema_closure_hash
    }

    #[must_use]
    pub const fn customization_hash(&self) -> Sha256Digest {
        self.customization_hash
    }

    #[must_use]
    pub const fn validation_profile_hash(&self) -> Sha256Digest {
        self.validation_profile_hash
    }
}

pub struct BmadHelpBindingCompiler;

impl BmadHelpBindingCompiler {
    /// Compiles the exact inert Help plan from sealed inputs only.
    ///
    /// # Errors
    ///
    /// Rejects any source, catalog, execution, schema, validation,
    /// customization, or model-binding mismatch.
    pub fn compile(
        source: &BmadSealedHelpInvocation,
        catalog: &BmadCatalog,
        model: &BmadTrustedHelpModelProfile,
    ) -> Result<BmadCompiledHelpInvocation, BmadKernelError> {
        verify_source_identity(source)?;
        verify_exact_catalog(source, catalog)?;

        let capability_key = BmadCapabilityKey {
            package_version_id: source.package_version_id().clone(),
            module_code: "core".to_owned(),
            skill_name: "bmad-help".to_owned(),
            normalized_action: None,
        };
        let execution_data = help_execution_profile();
        if canonical_hash("bmad-execution-profile", 1, &execution_data)
            .map_err(|_| binding_mismatch())?
            != source.execution_profile_hash()
        {
            return Err(binding_mismatch());
        }
        let execution_profile =
            MethodExecutionProfile::from_source(execution_data, source.execution_profile_hash())
                .map_err(|_| binding_mismatch())?;

        let customization_hash = help_customization_hash(&capability_key)?;
        let validation_profile_hash = help_validation_profile_hash()?;
        let proposal_schema_closure_hash =
            generated_digest(generated_contracts::BMAD_METHOD_HELP_PROPOSAL_SCHEMA_CLOSURE_SHA256)?;
        let recommendation_schema_closure_hash = generated_digest(
            generated_contracts::BMAD_METHOD_HELP_RECOMMENDATION_SCHEMA_CLOSURE_SHA256,
        )?;
        let result_schema_closure_hash = generated_digest(
            generated_contracts::BMAD_METHOD_ADVANCE_RESULT_SCHEMA_CLOSURE_SHA256,
        )?;
        let exact_binding = MethodExactBinding {
            capability_key,
            package_descriptor_hash: source.descriptor_hash(),
            package_source_hash: source.source_snapshot_hash(),
            instruction_projection_hash: source.projection_hash(),
            capability_catalog_hash: catalog.capability_catalog_hash(),
            agent_roster_hash: None,
            agent_binding_hash: None,
            agent_binding: None,
            distribution_profile: source.distribution_profile().to_owned(),
            install_profile: source.install_profile().to_owned(),
            entrypoint_kind: "direct".to_owned(),
            execution_profile_hash: source.execution_profile_hash(),
            execution_profile,
            validation_profile: "MethodOfficialSkillV6".to_owned(),
            validation_profile_hash,
            config_graph_hash: source.central_config_graph_hash(),
            config_resolution_hash: source.central_config_resolution_hash(),
            customization_hash,
            resource_set_hash: source.resource_set_hash(),
            model_binding_hash: model.binding.binding_hash,
            model_binding: model.binding.clone(),
            method_schema_hash: result_schema_closure_hash,
            egress_profile_hash: model.binding.data.egress_profile_hash,
            artifact_expectations: Vec::new(),
        };
        exact_binding
            .binding_hash()
            .map_err(|_| binding_mismatch())?;
        let step_table = MethodStepTable::new("recommend", [("recommend", None)])
            .map_err(|_| binding_mismatch())?;

        Ok(BmadCompiledHelpInvocation {
            instruction_bytes: source.instruction_arc(),
            catalog_candidates: Arc::from(catalog.help_actions.clone()),
            exact_binding,
            step_table,
            request_schema_hash: model.binding.data.request_schema_hash,
            proposal_schema_closure_hash,
            recommendation_schema_closure_hash,
            result_schema_closure_hash,
            customization_hash,
            validation_profile_hash,
        })
    }
}

fn verify_source_identity(source: &BmadSealedHelpInvocation) -> Result<(), BmadKernelError> {
    if source.package_name() != "bmad-method"
        || source.package_version() != "6.10.0"
        || source.module_code() != "core"
        || source.skill_name() != "bmad-help"
        || source.distribution_profile() != "sapphirus_package"
        || source.install_profile() != "SapphirusManagedV1"
        || source.validation_profile() != "MethodOfficialSkillV6"
    {
        return Err(binding_mismatch());
    }
    Ok(())
}

fn verify_exact_catalog(
    source: &BmadSealedHelpInvocation,
    catalog: &BmadCatalog,
) -> Result<(), BmadKernelError> {
    catalog.verify_integrity().map_err(|_| binding_mismatch())?;
    let action_graph_hash = generated_digest(HELP_ACTION_GRAPH_HASH)?;
    let expected_catalog_hash = canonical_hash(
        "bmad-capability-catalog-binding",
        1,
        &(
            source.package_version_id(),
            source.descriptor_hash(),
            source.observed_inventory_hash(),
            action_graph_hash,
        ),
    )
    .map_err(|_| binding_mismatch())?;
    if catalog.capability_catalog_hash() != expected_catalog_hash
        || catalog.installed_skills.len() != 2
        || catalog.help_actions.len() != 2
        || !installed_skill_matches(
            &catalog.installed_skills[0],
            "bmm",
            "bmad-architecture",
            "Create Architecture",
            "Create the architecture spine that keeps independently built units consistent.",
            BmadEntrypointKind::StepJit,
            &["create"],
            "MethodStepWorkflowV6",
            "sha256:8ac94c1e6c5be5fd6e7d446ba3800c0f170d80e730464a859b43c58732cf1063",
            false,
        )?
        || !installed_skill_matches(
            &catalog.installed_skills[1],
            "core",
            "bmad-help",
            "BMad Help",
            "Provide source-grounded Method guidance from the installed catalog.",
            BmadEntrypointKind::Direct,
            &[],
            "MethodOfficialSkillV6",
            "sha256:02b16af451ee6b4a60d0e446f0ea911b6b57f0c646845ed8bdd81ce09f2e1485",
            true,
        )?
        || !help_action_matches(
            &catalog.help_actions[0],
            source,
            expected_catalog_hash,
            "bmm",
            "bmad-architecture",
            Some("create"),
            &ARCHITECTURE_ROW,
        )
        || !help_action_matches(
            &catalog.help_actions[1],
            source,
            expected_catalog_hash,
            "core",
            "bmad-help",
            None,
            &HELP_ROW,
        )
    {
        return Err(binding_mismatch());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn installed_skill_matches(
    skill: &BmadInstalledSkillRecord,
    module_code: &str,
    skill_name: &str,
    display_name: &str,
    description: &str,
    entrypoint_kind: BmadEntrypointKind,
    actions: &[&str],
    validation_profile: &str,
    execution_profile_hash: &str,
    structurally_eligible: bool,
) -> Result<bool, BmadKernelError> {
    Ok(skill.module_code == module_code
        && skill.skill_name == skill_name
        && skill.display_name == display_name
        && skill.description == description
        && skill.entrypoint_kind == entrypoint_kind
        && skill
            .actions
            .iter()
            .map(String::as_str)
            .eq(actions.iter().copied())
        && skill.distribution_profile == "sapphirus_package"
        && skill.install_profile == "SapphirusManagedV1"
        && skill.validation_profile == validation_profile
        && skill.execution_profile_hash == generated_digest(execution_profile_hash)?
        && !skill.capability_enabled
        && skill.structurally_eligible == structurally_eligible
        && !skill.hidden_from_help)
}

fn help_action_matches(
    action: &BmadHelpAction,
    source: &BmadSealedHelpInvocation,
    capability_catalog_hash: Sha256Digest,
    module_code: &str,
    skill_name: &str,
    normalized_action: Option<&str>,
    source_row: &[&str; 13],
) -> bool {
    action.key.capability_catalog_hash == capability_catalog_hash
        && action.key.package_version_id == *source.package_version_id()
        && action.key.module_code == module_code
        && action.key.skill_name == skill_name
        && action.key.action.as_deref() == normalized_action
        && action.module_code == module_code
        && action.skill_name == skill_name
        && action.action.as_deref() == normalized_action
        && action.availability == BmadCatalogAvailability::CapabilityDisabled
        && !action.network_reference_present
        && action.source_ordinal == 0
        && action
            .raw_source_row()
            .iter()
            .map(String::as_str)
            .eq(source_row.iter().copied())
}

fn help_execution_profile() -> MethodExecutionProfileData {
    MethodExecutionProfileData {
        entrypoint_kind: "direct".to_owned(),
        invocation_modes: MethodInvocationModes {
            interactive: true,
            headless: false,
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
            declared_resource_paths: Vec::new(),
        },
        declared_tool_intents: Vec::new(),
        state_hints: Vec::new(),
        completion_evidence: vec!["artifact".to_owned()],
        customization_profile: "method_skill_toml".to_owned(),
        validation_profile: "MethodOfficialSkillV6".to_owned(),
    }
}

fn help_customization_hash(
    capability_key: &BmadCapabilityKey,
) -> Result<Sha256Digest, BmadKernelError> {
    let actual = canonical_hash(
        "bmad-help-empty-customization",
        1,
        &json!({"capabilityKey": capability_key, "layers": []}),
    )
    .map_err(|_| binding_mismatch())?;
    if actual != generated_digest(HELP_CUSTOMIZATION_HASH)? {
        return Err(binding_mismatch());
    }
    Ok(actual)
}

fn help_validation_profile_hash() -> Result<Sha256Digest, BmadKernelError> {
    let descriptor = json!({
        "profile": "MethodOfficialSkillV6",
        "proposalByteLimit": 65_536,
        "safeTextPolicy": "unicode_scalar_no_c0_del_bidi",
        "rules": [
            "standalone_proposal_schema",
            "catalog_capability_exact_match",
            "evidence_token_known_unique",
            "evidence_class_no_upgrade",
            "guidance_catalog_derived",
            "no_recommendation_provable",
            "method_lineage_exact"
        ]
    });
    let actual = canonical_hash("bmad-help-validation-profile", 1, &descriptor)
        .map_err(|_| binding_mismatch())?;
    if actual != generated_digest(HELP_VALIDATION_PROFILE_HASH)? {
        return Err(binding_mismatch());
    }
    Ok(actual)
}

fn generated_digest(value: &str) -> Result<Sha256Digest, BmadKernelError> {
    Sha256Digest::parse(value).map_err(|_| binding_mismatch())
}

const fn binding_mismatch() -> BmadKernelError {
    BmadKernelError::new(BmadKernelErrorCode::SealedHelpBindingMismatch)
}
