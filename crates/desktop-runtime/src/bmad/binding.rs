use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{canonical_hash, ContractId, Sha256Digest, UnixMillis};

use super::{MethodError, MethodErrorCode};

const MAX_ARTIFACT_EXPECTATIONS: usize = 64;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BmadCapabilityKey {
    pub package_version_id: ContractId,
    pub module_code: String,
    pub skill_name: String,
    pub normalized_action: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MethodEvidenceClass {
    Authoritative,
    UserAsserted,
    Heuristic,
    Contextual,
    Unknown,
}

impl MethodEvidenceClass {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Authoritative => "authoritative",
            Self::UserAsserted => "user_asserted",
            Self::Heuristic => "heuristic",
            Self::Contextual => "contextual",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MethodArtifactExpectation {
    pub expectation_kind: String,
    pub expectation_id: ContractId,
    pub artifact_kind: String,
    pub required: bool,
    pub storage_scope: String,
    pub expected_media_type: String,
    pub expected_content_schema_hash: Option<Sha256Digest>,
    pub source_output_hint: Option<String>,
    pub completion_evidence_class: MethodEvidenceClass,
    pub expectation_hash: Sha256Digest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MethodRuntimeRequirement {
    pub runtime: String,
    pub version_range: String,
    pub required: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MethodInvocationModes {
    pub interactive: bool,
    pub headless: bool,
    pub actions: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MethodResourcePolicy {
    pub entrypoint_timing: String,
    pub resource_timing: String,
    pub declared_resource_paths: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MethodExecutionProfileData {
    pub entrypoint_kind: String,
    pub invocation_modes: MethodInvocationModes,
    pub required_runtimes: Vec<MethodRuntimeRequirement>,
    pub resource_policy: MethodResourcePolicy,
    pub declared_tool_intents: Vec<String>,
    pub state_hints: Vec<String>,
    pub completion_evidence: Vec<String>,
    pub customization_profile: String,
    pub validation_profile: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MethodExecutionProfile {
    #[serde(flatten)]
    pub data: MethodExecutionProfileData,
    pub profile_hash: Sha256Digest,
}

impl MethodExecutionProfile {
    /// Seals an exact descriptive execution profile.
    ///
    /// # Errors
    ///
    /// Returns `method_binding_stale` for unsupported or malformed metadata.
    pub fn from_source(
        data: MethodExecutionProfileData,
        profile_hash: Sha256Digest,
    ) -> Result<Self, MethodError> {
        validate_execution_profile_data(&data)?;
        Ok(Self { data, profile_hash })
    }

    fn validate(&self) -> Result<(), MethodError> {
        validate_execution_profile_data(&self.data)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MethodModelBindingData {
    pub binding_kind: String,
    pub provider_id: String,
    pub model_id: String,
    pub deployment_id: String,
    pub model_profile_hash: Sha256Digest,
    pub model_capability_hash: Sha256Digest,
    pub context_window_profile_hash: Sha256Digest,
    pub egress_profile_hash: Sha256Digest,
    pub request_schema_hash: Sha256Digest,
    pub response_schema_hash: Sha256Digest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MethodModelBinding {
    #[serde(flatten)]
    pub data: MethodModelBindingData,
    pub binding_hash: Sha256Digest,
}

impl MethodModelBinding {
    /// Seals exact provider, model, deployment, schema, capability, and egress inputs.
    ///
    /// # Errors
    ///
    /// Returns `method_binding_stale` for malformed identifiers.
    pub fn from_source(
        data: MethodModelBindingData,
        binding_hash: Sha256Digest,
    ) -> Result<Self, MethodError> {
        validate_model_binding_data(&data)?;
        Ok(Self { data, binding_hash })
    }

    fn validate(&self) -> Result<(), MethodError> {
        validate_model_binding_data(&self.data)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MethodAgentBindingData {
    pub binding_kind: String,
    pub roster_hash: Sha256Digest,
    pub module_source_hash: Sha256Digest,
    pub module_code: String,
    pub agent_record_hash: Sha256Digest,
    pub agent_code: String,
    pub agent_name: String,
    pub agent_title: String,
    pub persona_hash: Sha256Digest,
    pub customization_graph_hash: Sha256Digest,
    pub menu_item_hash: Sha256Digest,
    pub menu_code: String,
    pub menu_target_kind: String,
    pub menu_capability_key: BmadCapabilityKey,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MethodAgentBinding {
    #[serde(flatten)]
    pub data: MethodAgentBindingData,
    pub agent_binding_hash: Sha256Digest,
}

impl MethodAgentBinding {
    /// Seals the exact roster, persona, menu, and target identity.
    ///
    /// # Errors
    ///
    /// Returns `method_binding_stale` for malformed identity data.
    pub fn from_source(
        data: MethodAgentBindingData,
        agent_binding_hash: Sha256Digest,
    ) -> Result<Self, MethodError> {
        validate_agent_binding_data(&data)?;
        Ok(Self {
            data,
            agent_binding_hash,
        })
    }

    fn validate(&self) -> Result<(), MethodError> {
        validate_agent_binding_data(&self.data)
    }
}

impl MethodArtifactExpectation {
    /// Preserves an artifact expectation exactly as emitted by the BMAD source contract.
    ///
    /// # Errors
    ///
    /// Returns `method_binding_stale` for malformed metadata.
    pub fn from_source(source: Self) -> Result<Self, MethodError> {
        source.validate_metadata()?;
        Ok(source)
    }

    pub(crate) fn validate(&self) -> Result<(), MethodError> {
        self.validate_metadata()
    }

    fn validate_metadata(&self) -> Result<(), MethodError> {
        if self.expectation_kind != "method_artifact"
            || self.storage_scope != "app_local"
            || !valid_token(&self.artifact_kind)
            || !(1..=128).contains(&self.expected_media_type.len())
            || !self.expected_media_type.is_ascii()
            || self.expected_media_type.chars().any(char::is_control)
            || self
                .source_output_hint
                .as_ref()
                .is_some_and(|hint| hint.is_empty() || hint.len() > 1024 || hint.contains('\0'))
        {
            return Err(MethodError::new(MethodErrorCode::MethodBindingStale));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MethodExactBinding {
    pub capability_key: BmadCapabilityKey,
    pub package_descriptor_hash: Sha256Digest,
    pub package_source_hash: Sha256Digest,
    pub instruction_projection_hash: Sha256Digest,
    pub capability_catalog_hash: Sha256Digest,
    pub agent_roster_hash: Option<Sha256Digest>,
    pub agent_binding_hash: Option<Sha256Digest>,
    pub agent_binding: Option<MethodAgentBinding>,
    pub distribution_profile: String,
    pub install_profile: String,
    pub entrypoint_kind: String,
    pub execution_profile_hash: Sha256Digest,
    pub execution_profile: MethodExecutionProfile,
    pub validation_profile: String,
    pub validation_profile_hash: Sha256Digest,
    pub config_graph_hash: Sha256Digest,
    pub config_resolution_hash: Sha256Digest,
    pub customization_hash: Sha256Digest,
    pub resource_set_hash: Sha256Digest,
    pub model_binding_hash: Sha256Digest,
    pub model_binding: MethodModelBinding,
    pub method_schema_hash: Sha256Digest,
    pub egress_profile_hash: Sha256Digest,
    pub artifact_expectations: Vec<MethodArtifactExpectation>,
}

impl MethodExactBinding {
    /// Computes the immutable binding digest used by context review decisions.
    ///
    /// # Errors
    ///
    /// Returns a Method validation error if canonical serialization fails.
    pub fn binding_hash(&self) -> Result<Sha256Digest, MethodError> {
        self.validate()?;
        Ok(canonical_hash("bmad-method-exact-binding", 1, self)?)
    }

    pub(crate) fn validate(&self) -> Result<(), MethodError> {
        let key = &self.capability_key;
        if !valid_token(&key.module_code)
            || !valid_token(&key.skill_name)
            || key
                .normalized_action
                .as_deref()
                .is_some_and(|action| !valid_token(action))
            || !matches!(
                self.distribution_profile.as_str(),
                "method_source_tree"
                    | "method_claude_plugin"
                    | "method_web_bundle_v1"
                    | "builder_source_tree"
                    | "builder_plugin"
                    | "sapphirus_package"
            )
            || !matches!(
                self.install_profile.as_str(),
                "MethodCliV6" | "StandaloneBuilderSetupV2" | "SapphirusManagedV1"
            )
            || !matches!(
                self.entrypoint_kind.as_str(),
                "direct" | "inline" | "step_jit" | "script_rendered" | "compatibility_shim"
            )
            || !valid_profile(&self.validation_profile)
        {
            return Err(MethodError::new(MethodErrorCode::MethodBindingStale));
        }
        self.execution_profile.validate()?;
        self.model_binding.validate()?;
        if self.execution_profile_hash != self.execution_profile.profile_hash
            || self.entrypoint_kind != self.execution_profile.data.entrypoint_kind
            || self.validation_profile != self.execution_profile.data.validation_profile
            || self.model_binding_hash != self.model_binding.binding_hash
            || self.egress_profile_hash != self.model_binding.data.egress_profile_hash
            || self.agent_binding_hash.as_ref()
                != self
                    .agent_binding
                    .as_ref()
                    .map(|value| &value.agent_binding_hash)
            || self.agent_roster_hash.as_ref()
                != self
                    .agent_binding
                    .as_ref()
                    .map(|value| &value.data.roster_hash)
        {
            return Err(MethodError::new(MethodErrorCode::MethodBindingStale));
        }
        if let Some(agent) = &self.agent_binding {
            agent.validate()?;
            if agent.data.menu_capability_key != self.capability_key {
                return Err(MethodError::new(MethodErrorCode::MethodBindingStale));
            }
        }
        if self.artifact_expectations.len() > MAX_ARTIFACT_EXPECTATIONS {
            return Err(MethodError::new(MethodErrorCode::MethodBindingStale));
        }
        let mut expectation_ids = BTreeSet::new();
        for expectation in &self.artifact_expectations {
            expectation.validate()?;
            if !expectation_ids.insert(expectation.expectation_id.as_str()) {
                return Err(MethodError::new(MethodErrorCode::MethodBindingStale));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MethodContextDecision {
    pub decision_id: ContractId,
    pub manifest_hash: Sha256Digest,
    pub consent_hash: Sha256Digest,
    pub context_digest: Sha256Digest,
    pub binding_hash: Sha256Digest,
    pub reviewed_at: UnixMillis,
}

fn valid_token(value: &str) -> bool {
    (1..=128).contains(&value.len())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        })
}

fn valid_profile(value: &str) -> bool {
    (1..=128).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn validate_execution_profile_data(data: &MethodExecutionProfileData) -> Result<(), MethodError> {
    if !matches!(
        data.entrypoint_kind.as_str(),
        "direct" | "inline" | "step_jit" | "script_rendered" | "compatibility_shim"
    ) || !matches!(
        data.resource_policy.resource_timing.as_str(),
        "all_declared_at_start"
            | "current_step_only"
            | "render_inputs_only"
            | "forward_target_only"
            | "on_demand_declared"
    ) || data.resource_policy.entrypoint_timing != "invocation_start"
        || !matches!(
            data.customization_profile.as_str(),
            "none"
                | "method_skill_toml"
                | "method_agent_toml"
                | "builder_agent_toml"
                | "builder_workflow_toml"
        )
        || !matches!(
            data.validation_profile.as_str(),
            "MethodOfficialSkillV6"
                | "MethodStepWorkflowV6"
                | "BuilderOutcomeSkillV2"
                | "BuilderAgentV2Stateless"
                | "MethodCliInstalledV6"
                | "StandaloneBuilderSetupV2"
        )
        || data.required_runtimes.len() > 8
        || data.invocation_modes.actions.len() > 64
        || data.resource_policy.declared_resource_paths.len() > 4096
        || data.declared_tool_intents.len() > 16
        || data.state_hints.len() > 16
        || data.completion_evidence.len() > 16
        || !all_unique(&data.invocation_modes.actions)
        || !all_unique(&data.resource_policy.declared_resource_paths)
        || !all_unique(&data.declared_tool_intents)
        || !all_unique(&data.state_hints)
        || !all_unique(&data.completion_evidence)
        || data
            .invocation_modes
            .actions
            .iter()
            .any(|value| !valid_token(value))
        || data.declared_tool_intents.iter().any(|value| {
            !matches!(
                value.as_str(),
                "file_read"
                    | "file_write"
                    | "process"
                    | "web"
                    | "subagent"
                    | "browser"
                    | "external_handoff"
            )
        })
        || data.state_hints.iter().any(|value| {
            !matches!(
                value.as_str(),
                "memlog" | "artifact_workspace" | "sprint_status" | "story" | "spec"
            )
        })
        || data
            .completion_evidence
            .iter()
            .any(|value| !matches!(value.as_str(), "artifact" | "event" | "status_evidence"))
    {
        return Err(MethodError::new(MethodErrorCode::MethodBindingStale));
    }
    for runtime in &data.required_runtimes {
        if !matches!(runtime.runtime.as_str(), "node" | "python" | "uv" | "host")
            || runtime.version_range.is_empty()
            || runtime.version_range.len() > 128
        {
            return Err(MethodError::new(MethodErrorCode::MethodBindingStale));
        }
    }
    if data
        .resource_policy
        .declared_resource_paths
        .iter()
        .any(|path| {
            path.is_empty()
                || path.len() > 1024
                || path.starts_with('/')
                || path.contains('\\')
                || path.contains(':')
                || path.split(['/', '\\']).any(|part| part == "..")
        })
    {
        return Err(MethodError::new(MethodErrorCode::MethodBindingStale));
    }
    Ok(())
}

fn validate_model_binding_data(data: &MethodModelBindingData) -> Result<(), MethodError> {
    if data.binding_kind != "method_model"
        || !valid_provider_id(&data.provider_id)
        || data.model_id.is_empty()
        || data.model_id.len() > 256
        || data.deployment_id.is_empty()
        || data.deployment_id.len() > 256
        || data.model_id.chars().any(char::is_control)
        || data.deployment_id.chars().any(char::is_control)
    {
        return Err(MethodError::new(MethodErrorCode::MethodBindingStale));
    }
    Ok(())
}

fn validate_agent_binding_data(data: &MethodAgentBindingData) -> Result<(), MethodError> {
    if data.binding_kind != "agent"
        || !valid_token(&data.module_code)
        || !valid_token(&data.agent_code)
        || data.agent_name.is_empty()
        || data.agent_name.len() > 256
        || data.agent_name.chars().any(char::is_control)
        || data.agent_title.is_empty()
        || data.agent_title.len() > 256
        || data.agent_title.chars().any(char::is_control)
        || !valid_profile(&data.menu_code)
        || !valid_token(&data.menu_target_kind)
        || !valid_token(&data.menu_capability_key.module_code)
        || !valid_token(&data.menu_capability_key.skill_name)
        || data
            .menu_capability_key
            .normalized_action
            .as_deref()
            .is_some_and(|value| !valid_token(value))
    {
        return Err(MethodError::new(MethodErrorCode::MethodBindingStale));
    }
    Ok(())
}

fn all_unique(values: &[String]) -> bool {
    values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn valid_provider_id(value: &str) -> bool {
    (1..=128).contains(&value.len())
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}
