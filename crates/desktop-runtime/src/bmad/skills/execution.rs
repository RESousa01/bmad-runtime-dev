//! Inert compilation of an opted-in Method-skill descriptor into the existing
//! Method binding and step-table engine.
//!
//! This is deliberately a preparation boundary. It validates source-derived
//! declarations and returns data for the host's existing persistence, Airlock,
//! model, and artifact ports; it neither owns a session nor performs an effect.

use std::collections::BTreeSet;

use thiserror::Error;

use crate::generated_contracts::{BmadActionPlanKind, BmadArtifactKind, BmadInvocationMode};
use crate::{
    canonical_hash, BmadCapabilityKey, ContractId, MethodServiceError, MethodSession, Sha256Digest,
};

use super::harness::{BmadMethodSkill, BmadSkillDescriptor, BmadSkillSupportState};
use crate::bmad::{
    BmadResource, BmadResourceSet, MethodArtifactExpectation, MethodExactBinding,
    MethodExecutionProfile, MethodModelBinding, MethodStepTable,
};

const MAX_WORKFLOW_STEPS: usize = 64;
const MAX_TOOL_INTENTS: usize = 16;
const MAX_ARTIFACT_EXPECTATIONS: usize = 64;
const MAX_PACKAGE_ID_BYTES: usize = 128;

/// Typed, non-authorizing intent a source-derived skill may require a host to
/// plan. Each variant maps deterministically to the established action-plan
/// vocabulary; a declaration is never an effect grant.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BmadSkillToolIntent {
    FileRead,
    FileWrite,
    Process,
    Web,
    Network,
    Subagent,
    Browser,
    ExternalHandoff,
}

impl BmadSkillToolIntent {
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::FileRead => "file_read",
            Self::FileWrite => "file_write",
            Self::Process => "process",
            Self::Web => "web",
            Self::Network => "network",
            Self::Subagent => "subagent",
            Self::Browser => "browser",
            Self::ExternalHandoff => "external_handoff",
        }
    }

    /// Returns the only generic action-plan kind this intent may ask the host
    /// to construct. Approval remains an action-plan and host concern.
    #[must_use]
    pub const fn required_plan_kind(self) -> BmadActionPlanKind {
        match self {
            Self::FileRead | Self::Web | Self::Network | Self::Browser => {
                BmadActionPlanKind::Read
            }
            Self::FileWrite => BmadActionPlanKind::WriteProposal,
            Self::Process => BmadActionPlanKind::CommandProposal,
            Self::Subagent => BmadActionPlanKind::ModelCall,
            Self::ExternalHandoff => BmadActionPlanKind::ArtifactPlan,
        }
    }
}

/// Recovery declaration consumed by the existing Method session lifecycle.
///
/// It intentionally contains no state or transition method: checkpoints and
/// review invalidation remain owned by [`crate::bmad::MethodSession`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BmadSkillRecoverySemantics {
    MethodCheckpoint,
}

/// One source-pinned member that declares a workflow transition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadSkillExecutionStep {
    pub step_key: String,
    pub next_step_key: Option<String>,
    pub source_resource_path: String,
    pub source_resource_hash: Sha256Digest,
}

/// Bounded, source-member-bound input for a [`MethodStepTable`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadSkillExecutionWorkflow {
    pub initial_step_key: String,
    pub steps: Vec<BmadSkillExecutionStep>,
}

/// Trusted, explicit opt-in data for one executable Method skill.
///
/// Constructing this contract checks the source descriptor, resource closure,
/// execution profile, typed plan intents, artifact expectations, and workflow
/// provenance. A descriptor-only adapter receives no implicit contract.
#[derive(Clone, Debug)]
pub struct BmadSkillExecutionContractInput {
    pub descriptor: BmadSkillDescriptor,
    pub package_name: String,
    pub package_version: String,
    pub package_version_id: ContractId,
    pub package_descriptor_hash: Sha256Digest,
    pub package_source_hash: Sha256Digest,
    pub instruction_projection_hash: Sha256Digest,
    pub capability_catalog_hash: Sha256Digest,
    pub distribution_profile: String,
    pub install_profile: String,
    pub execution_profile: MethodExecutionProfile,
    pub validation_profile_hash: Sha256Digest,
    pub method_schema_hash: Sha256Digest,
    pub workflow: BmadSkillExecutionWorkflow,
    pub tool_intents: Vec<BmadSkillToolIntent>,
    pub artifact_expectations: Vec<MethodArtifactExpectation>,
    pub recovery: BmadSkillRecoverySemantics,
}

/// A sealed, explicit descriptor execution contract.
///
/// The type exposes only validated projections. It has no persistence,
/// filesystem, process, network, model, or scheduler capability.
#[derive(Clone, Debug)]
pub struct BmadSkillExecutionContract {
    descriptor: BmadSkillDescriptor,
    descriptor_hash: Sha256Digest,
    package_name: String,
    package_version: String,
    package_version_id: ContractId,
    package_descriptor_hash: Sha256Digest,
    package_source_hash: Sha256Digest,
    instruction_projection_hash: Sha256Digest,
    capability_catalog_hash: Sha256Digest,
    distribution_profile: String,
    install_profile: String,
    execution_profile: MethodExecutionProfile,
    validation_profile_hash: Sha256Digest,
    method_schema_hash: Sha256Digest,
    resource_set_hash: Sha256Digest,
    workflow: BmadSkillExecutionWorkflow,
    tool_intents: Vec<BmadSkillToolIntent>,
    artifact_expectations: Vec<MethodArtifactExpectation>,
    recovery: BmadSkillRecoverySemantics,
}

/// Host-owned dynamic identities required to bind a sealed skill contract to
/// one exact, still-inert Method request.
#[derive(Clone, Debug)]
pub struct BmadSkillHostBindingInput {
    pub mode: BmadInvocationMode,
    pub action: Option<String>,
    pub resolved_config_graph_hash: Sha256Digest,
    pub resolved_config_hash: Sha256Digest,
    pub resolved_customization_hash: Sha256Digest,
    pub model_binding: MethodModelBinding,
    pub egress_profile_hash: Sha256Digest,
}

/// One source-validated preparation bound into the established durable Method
/// session service.
#[derive(Clone, Debug)]
pub struct BmadDispatchedSkillExecution {
    prepared: BmadPreparedSkillExecution,
    session: MethodSession,
}

impl BmadDispatchedSkillExecution {
    #[must_use]
    pub const fn prepared(&self) -> &BmadPreparedSkillExecution {
        &self.prepared
    }

    #[must_use]
    pub const fn session(&self) -> &MethodSession {
        &self.session
    }

    pub(crate) const fn new(prepared: BmadPreparedSkillExecution, session: MethodSession) -> Self {
        Self { prepared, session }
    }
}

/// Fail-closed preparation or established-service persistence failure.
#[derive(Debug)]
pub enum BmadSkillDispatchError<E> {
    Preparation(BmadSkillExecutionError),
    Lifecycle(MethodServiceError<E>),
}

impl BmadSkillExecutionContract {
    /// Validates an explicit, source-bound execution declaration.
    ///
    /// # Errors
    ///
    /// Returns a closed error for malformed package identity, unsupported
    /// descriptor state, stale source/resource declarations, or invalid
    /// workflow/profile metadata.
    pub fn new(input: BmadSkillExecutionContractInput) -> Result<Self, BmadSkillExecutionError> {
        if !valid_package_identity(&input.package_name)
            || !valid_package_identity(&input.package_version)
            || !matches!(
                input.descriptor.support_state(),
                BmadSkillSupportState::Exact | BmadSkillSupportState::Adapted
            )
            || input.tool_intents.len() > MAX_TOOL_INTENTS
            || input.artifact_expectations.len() > MAX_ARTIFACT_EXPECTATIONS
        {
            return Err(BmadSkillExecutionError::ContractInvalid);
        }
        let (_, _) = canonical_skill_identity(&input.descriptor)?;
        let descriptor_resources =
            BmadResourceSet::new(input.descriptor.declared_resources().to_vec())
                .map_err(|_| BmadSkillExecutionError::ResourceMismatch)?;
        let source_member = descriptor_resources
            .resources()
            .iter()
            .find(|member| member.path() == input.descriptor.source_path());
        if source_member
            .is_none_or(|member| member.content_hash() != input.descriptor.source_sha256())
        {
            return Err(BmadSkillExecutionError::ResourceMismatch);
        }
        validate_execution_profile(&input.descriptor, &input.execution_profile)?;
        validate_tool_intents(&input.execution_profile, &input.tool_intents)?;
        validate_artifact_expectations(
            input.descriptor.artifact_kinds(),
            &input.artifact_expectations,
        )?;
        validate_workflow(&input.workflow, &descriptor_resources)?;

        let descriptor_hash = descriptor_hash(&input.descriptor)?;
        let resource_set_hash = Self::resource_set_hash(&descriptor_resources)?;
        Ok(Self {
            descriptor: input.descriptor,
            descriptor_hash,
            package_name: input.package_name,
            package_version: input.package_version,
            package_version_id: input.package_version_id,
            package_descriptor_hash: input.package_descriptor_hash,
            package_source_hash: input.package_source_hash,
            instruction_projection_hash: input.instruction_projection_hash,
            capability_catalog_hash: input.capability_catalog_hash,
            distribution_profile: input.distribution_profile,
            install_profile: input.install_profile,
            execution_profile: input.execution_profile,
            validation_profile_hash: input.validation_profile_hash,
            method_schema_hash: input.method_schema_hash,
            resource_set_hash,
            workflow: input.workflow,
            tool_intents: input.tool_intents,
            artifact_expectations: input.artifact_expectations,
            recovery: input.recovery,
        })
    }

    #[must_use]
    pub fn package_name(&self) -> &str {
        &self.package_name
    }

    #[must_use]
    pub fn package_version(&self) -> &str {
        &self.package_version
    }

    /// Binds trusted host identities without duplicating sealed package
    /// constants in desktop dispatch code.
    ///
    /// # Errors
    ///
    /// Returns a closed binding error if the dynamic identities disagree with
    /// this contract.
    pub fn bind_host_request(
        &self,
        input: BmadSkillHostBindingInput,
    ) -> Result<BmadSkillExecutionRequest, BmadSkillExecutionError> {
        let (module_code, skill_name) = canonical_skill_identity(&self.descriptor)?;
        let resources = BmadResourceSet::new(self.descriptor.declared_resources().to_vec())
            .map_err(|_| BmadSkillExecutionError::ResourceMismatch)?;
        let exact_binding = MethodExactBinding {
            capability_key: BmadCapabilityKey {
                package_version_id: self.package_version_id.clone(),
                module_code: module_code.to_owned(),
                skill_name: skill_name.to_owned(),
                normalized_action: input.action.clone(),
            },
            package_descriptor_hash: self.package_descriptor_hash,
            package_source_hash: self.package_source_hash,
            instruction_projection_hash: self.instruction_projection_hash,
            capability_catalog_hash: self.capability_catalog_hash,
            agent_roster_hash: None,
            agent_binding_hash: None,
            agent_binding: None,
            distribution_profile: self.distribution_profile.clone(),
            install_profile: self.install_profile.clone(),
            entrypoint_kind: self.execution_profile.data.entrypoint_kind.clone(),
            execution_profile_hash: self.execution_profile.profile_hash,
            execution_profile: self.execution_profile.clone(),
            validation_profile: self.execution_profile.data.validation_profile.clone(),
            validation_profile_hash: self.validation_profile_hash,
            config_graph_hash: input.resolved_config_graph_hash,
            config_resolution_hash: input.resolved_config_hash,
            customization_hash: input.resolved_customization_hash,
            resource_set_hash: self.resource_set_hash,
            model_binding_hash: input.model_binding.binding_hash,
            model_binding: input.model_binding,
            method_schema_hash: self.method_schema_hash,
            egress_profile_hash: input.egress_profile_hash,
            artifact_expectations: self.artifact_expectations.clone(),
        };
        let request = BmadSkillExecutionRequest {
            mode: input.mode,
            action: input.action,
            exact_binding,
            resources,
            resolved_config_graph_hash: input.resolved_config_graph_hash,
            resolved_config_hash: input.resolved_config_hash,
            resolved_customization_hash: input.resolved_customization_hash,
        };
        validate_invocation(&self.descriptor, &self.execution_profile, &request)?;
        validate_binding(self, &request)?;
        Ok(request)
    }

    /// Computes the sealed resource-set digest used in an exact Method binding.
    ///
    /// # Errors
    ///
    /// Returns a closed hash error only when canonical serialization fails.
    pub fn resource_set_hash(
        resources: &BmadResourceSet,
    ) -> Result<Sha256Digest, BmadSkillExecutionError> {
        let projection = resources
            .resources()
            .iter()
            .map(|resource| (resource.path(), resource.content_hash()))
            .collect::<Vec<_>>();
        canonical_hash("bmad-skill-execution-resource-set", 1, &projection)
            .map_err(|_| BmadSkillExecutionError::ContractInvalid)
    }

    fn prepare(
        &self,
        descriptor: BmadSkillDescriptor,
        request: BmadSkillExecutionRequest,
    ) -> Result<BmadPreparedSkillExecution, BmadSkillExecutionError> {
        if descriptor_hash(&descriptor)? != self.descriptor_hash
            || descriptor_hash(&self.descriptor)? != self.descriptor_hash
        {
            return Err(BmadSkillExecutionError::DescriptorMismatch);
        }
        validate_invocation(&descriptor, &self.execution_profile, &request)?;
        validate_resources(&descriptor, request.resources(), self.resource_set_hash)?;
        validate_binding(self, &request)?;
        let step_table = build_step_table(&self.workflow)?;
        let required_plan_kinds = self
            .tool_intents
            .iter()
            .map(|intent| intent.required_plan_kind())
            .collect();
        Ok(BmadPreparedSkillExecution {
            descriptor,
            mode: request.mode,
            action: request.action,
            exact_binding: request.exact_binding,
            resources: request.resources,
            step_table,
            tool_intents: self.tool_intents.clone(),
            required_plan_kinds,
            artifact_expectations: self.artifact_expectations.clone(),
            recovery: self.recovery,
        })
    }
}

/// Per-invocation, host-supplied immutable inputs for one preparation attempt.
#[derive(Clone, Debug)]
pub struct BmadSkillExecutionRequest {
    pub mode: BmadInvocationMode,
    pub action: Option<String>,
    pub exact_binding: MethodExactBinding,
    pub resources: BmadResourceSet,
    pub resolved_config_graph_hash: Sha256Digest,
    pub resolved_config_hash: Sha256Digest,
    pub resolved_customization_hash: Sha256Digest,
}

impl BmadSkillExecutionRequest {
    #[must_use]
    pub const fn resources(&self) -> &BmadResourceSet {
        &self.resources
    }
}

/// Fail-closed reasons a descriptor cannot become an inert execution plan.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum BmadSkillExecutionError {
    #[error("skill exposes no explicit execution contract")]
    ContractUnavailable,
    #[error("skill execution contract is invalid or stale")]
    ContractInvalid,
    #[error("skill descriptor does not match its execution contract")]
    DescriptorMismatch,
    #[error("skill source or resource closure does not match its contract")]
    ResourceMismatch,
    #[error("skill invocation mode or action is not declared")]
    UnsupportedInvocation,
    #[error("exact Method binding does not match the execution contract")]
    BindingMismatch,
    #[error("skill workflow is malformed or not source-bound")]
    WorkflowInvalid,
}

/// Prepared, inert data that can be handed to the established Method service.
///
/// No session is created or persisted here. In particular this is not a second
/// executor, registry, or recovery state machine.
#[derive(Clone, Debug)]
pub struct BmadPreparedSkillExecution {
    descriptor: BmadSkillDescriptor,
    mode: BmadInvocationMode,
    action: Option<String>,
    exact_binding: MethodExactBinding,
    resources: BmadResourceSet,
    step_table: MethodStepTable,
    tool_intents: Vec<BmadSkillToolIntent>,
    required_plan_kinds: Vec<BmadActionPlanKind>,
    artifact_expectations: Vec<MethodArtifactExpectation>,
    recovery: BmadSkillRecoverySemantics,
}

impl BmadPreparedSkillExecution {
    #[must_use]
    pub const fn descriptor(&self) -> &BmadSkillDescriptor {
        &self.descriptor
    }

    #[must_use]
    pub fn mode(&self) -> BmadInvocationMode {
        self.mode
    }

    #[must_use]
    pub fn action(&self) -> Option<&str> {
        self.action.as_deref()
    }

    #[must_use]
    pub const fn exact_binding(&self) -> &MethodExactBinding {
        &self.exact_binding
    }

    #[must_use]
    pub const fn resources(&self) -> &BmadResourceSet {
        &self.resources
    }

    #[must_use]
    pub const fn step_table(&self) -> &MethodStepTable {
        &self.step_table
    }

    #[must_use]
    pub fn tool_intents(&self) -> &[BmadSkillToolIntent] {
        &self.tool_intents
    }

    #[must_use]
    pub fn required_plan_kinds(&self) -> &[BmadActionPlanKind] {
        &self.required_plan_kinds
    }

    #[must_use]
    pub fn artifact_expectations(&self) -> &[MethodArtifactExpectation] {
        &self.artifact_expectations
    }

    #[must_use]
    pub const fn recovery(&self) -> BmadSkillRecoverySemantics {
        self.recovery
    }
}

/// Compatibility name for callers that prefer the noun-first form.
pub type BmadSkillExecutionPreparedPlan = BmadPreparedSkillExecution;

/// Stateless compiler for the shared Method-skill execution seam.
pub struct BmadSkillExecutionCompiler;

impl BmadSkillExecutionCompiler {
    /// Prepares one opted-in skill without invoking any effect-capable port.
    ///
    /// # Errors
    ///
    /// Returns a closed error for descriptor-only adapters, stale contracts,
    /// source/resource drift, invocation mismatches, or exact-binding drift.
    pub fn prepare(
        skill: &dyn BmadMethodSkill,
        request: BmadSkillExecutionRequest,
    ) -> Result<BmadPreparedSkillExecution, BmadSkillExecutionError> {
        let descriptor = skill.descriptor();
        let contract = skill.execution_contract()?;
        contract.prepare(descriptor, request)
    }
}

fn canonical_skill_identity(
    descriptor: &BmadSkillDescriptor,
) -> Result<(&str, &str), BmadSkillExecutionError> {
    let (module, skill) = descriptor
        .canonical_id()
        .split_once(':')
        .ok_or(BmadSkillExecutionError::ContractInvalid)?;
    if !valid_token(module) || !valid_token(skill) || skill.contains(':') {
        return Err(BmadSkillExecutionError::ContractInvalid);
    }
    Ok((module, skill))
}

fn descriptor_hash(
    descriptor: &BmadSkillDescriptor,
) -> Result<Sha256Digest, BmadSkillExecutionError> {
    canonical_hash(
        "bmad-skill-execution-descriptor",
        1,
        &serde_json::json!({
            "canonicalId": descriptor.canonical_id(),
            "sourcePath": descriptor.source_path(),
            "sourceSha256": descriptor.source_sha256(),
            "declaredActions": descriptor.declared_actions(),
            "supportedModes": descriptor.supported_modes(),
            "outputLocations": descriptor.output_locations(),
            "artifactKinds": descriptor.artifact_kinds(),
            "completionHooks": descriptor.completion_hooks(),
            "resources": descriptor.declared_resources().iter().map(|resource| {
                serde_json::json!({
                    "path": resource.path(),
                    "contentHash": resource.content_hash(),
                })
            }).collect::<Vec<_>>(),
            "supportState": descriptor.support_state(),
        }),
    )
    .map_err(|_| BmadSkillExecutionError::ContractInvalid)
}

fn validate_execution_profile(
    descriptor: &BmadSkillDescriptor,
    profile: &MethodExecutionProfile,
) -> Result<(), BmadSkillExecutionError> {
    MethodExecutionProfile::from_source(profile.data.clone(), profile.profile_hash)
        .map_err(|_| BmadSkillExecutionError::ContractInvalid)?;
    let descriptor_interactive = descriptor
        .supported_modes()
        .contains(&BmadInvocationMode::Interactive);
    let descriptor_headless = descriptor
        .supported_modes()
        .contains(&BmadInvocationMode::Headless);
    let descriptor_actions = descriptor
        .declared_actions()
        .iter()
        .map(|action| action.name.to_string())
        .collect::<BTreeSet<_>>();
    let profile_actions = profile
        .data
        .invocation_modes
        .actions
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let declared_resources = descriptor
        .declared_resources()
        .iter()
        .map(BmadResource::path)
        .collect::<BTreeSet<_>>();
    if profile.data.invocation_modes.interactive != descriptor_interactive
        || profile.data.invocation_modes.headless != descriptor_headless
        || profile_actions != descriptor_actions
        || profile
            .data
            .resource_policy
            .declared_resource_paths
            .iter()
            .any(|path| !declared_resources.contains(path.as_str()))
    {
        return Err(BmadSkillExecutionError::ContractInvalid);
    }
    Ok(())
}

fn validate_tool_intents(
    profile: &MethodExecutionProfile,
    intents: &[BmadSkillToolIntent],
) -> Result<(), BmadSkillExecutionError> {
    let declared = profile
        .data
        .declared_tool_intents
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let typed = intents
        .iter()
        .map(|intent| intent.source_name().to_owned())
        .collect::<BTreeSet<_>>();
    if declared != typed || typed.len() != intents.len() {
        return Err(BmadSkillExecutionError::ContractInvalid);
    }
    Ok(())
}

fn validate_artifact_expectations(
    artifact_kinds: &[BmadArtifactKind],
    expectations: &[MethodArtifactExpectation],
) -> Result<(), BmadSkillExecutionError> {
    let allowed = artifact_kinds
        .iter()
        .map(ToString::to_string)
        .collect::<BTreeSet<_>>();
    let mut ids = BTreeSet::new();
    for expectation in expectations {
        MethodArtifactExpectation::from_source(expectation.clone())
            .map_err(|_| BmadSkillExecutionError::ContractInvalid)?;
        if !allowed.contains(&expectation.artifact_kind)
            || !ids.insert(expectation.expectation_id.as_str())
        {
            return Err(BmadSkillExecutionError::ContractInvalid);
        }
    }
    Ok(())
}

fn validate_workflow(
    workflow: &BmadSkillExecutionWorkflow,
    resources: &BmadResourceSet,
) -> Result<(), BmadSkillExecutionError> {
    if workflow.steps.is_empty() || workflow.steps.len() > MAX_WORKFLOW_STEPS {
        return Err(BmadSkillExecutionError::WorkflowInvalid);
    }
    for step in &workflow.steps {
        let source = resources
            .resources()
            .iter()
            .find(|resource| resource.path() == step.source_resource_path);
        if source.is_none_or(|resource| resource.content_hash() != step.source_resource_hash) {
            return Err(BmadSkillExecutionError::WorkflowInvalid);
        }
    }
    build_step_table(workflow).map(|_| ())
}

fn build_step_table(
    workflow: &BmadSkillExecutionWorkflow,
) -> Result<MethodStepTable, BmadSkillExecutionError> {
    MethodStepTable::new(
        &workflow.initial_step_key,
        workflow
            .steps
            .iter()
            .map(|step| (step.step_key.as_str(), step.next_step_key.as_deref())),
    )
    .map_err(|_| BmadSkillExecutionError::WorkflowInvalid)
}

fn validate_invocation(
    descriptor: &BmadSkillDescriptor,
    profile: &MethodExecutionProfile,
    request: &BmadSkillExecutionRequest,
) -> Result<(), BmadSkillExecutionError> {
    let mode_declared = descriptor.supported_modes().contains(&request.mode)
        && match request.mode {
            BmadInvocationMode::Interactive => profile.data.invocation_modes.interactive,
            BmadInvocationMode::Headless => profile.data.invocation_modes.headless,
        };
    let action_declared = match request.action.as_deref() {
        Some(action) => descriptor
            .declared_actions()
            .iter()
            .any(|declared| declared.name.as_str() == action),
        None => descriptor.declared_actions().is_empty(),
    };
    if !mode_declared || !action_declared {
        return Err(BmadSkillExecutionError::UnsupportedInvocation);
    }
    Ok(())
}

fn validate_resources(
    descriptor: &BmadSkillDescriptor,
    resources: &BmadResourceSet,
    expected_hash: Sha256Digest,
) -> Result<(), BmadSkillExecutionError> {
    let descriptor_resources = BmadResourceSet::new(descriptor.declared_resources().to_vec())
        .map_err(|_| BmadSkillExecutionError::ResourceMismatch)?;
    if resources != &descriptor_resources
        || BmadSkillExecutionContract::resource_set_hash(resources)? != expected_hash
    {
        return Err(BmadSkillExecutionError::ResourceMismatch);
    }
    Ok(())
}

fn validate_binding(
    contract: &BmadSkillExecutionContract,
    request: &BmadSkillExecutionRequest,
) -> Result<(), BmadSkillExecutionError> {
    request
        .exact_binding
        .binding_hash()
        .map_err(|_| BmadSkillExecutionError::BindingMismatch)?;
    let (module, skill) = canonical_skill_identity(&contract.descriptor)?;
    let binding = &request.exact_binding;
    if binding.capability_key.package_version_id != contract.package_version_id
        || binding.capability_key.module_code != module
        || binding.capability_key.skill_name != skill
        || binding.capability_key.normalized_action != request.action
        || binding.package_descriptor_hash != contract.package_descriptor_hash
        || binding.package_source_hash != contract.package_source_hash
        || binding.instruction_projection_hash != contract.instruction_projection_hash
        || binding.capability_catalog_hash != contract.capability_catalog_hash
        || binding.distribution_profile != contract.distribution_profile
        || binding.install_profile != contract.install_profile
        || binding.execution_profile != contract.execution_profile
        || binding.validation_profile_hash != contract.validation_profile_hash
        || binding.method_schema_hash != contract.method_schema_hash
        || binding.resource_set_hash != contract.resource_set_hash
        || binding.config_graph_hash != request.resolved_config_graph_hash
        || binding.config_resolution_hash != request.resolved_config_hash
        || binding.customization_hash != request.resolved_customization_hash
        || binding.artifact_expectations != contract.artifact_expectations
    {
        return Err(BmadSkillExecutionError::BindingMismatch);
    }
    Ok(())
}

fn valid_package_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_PACKAGE_ID_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn valid_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_PACKAGE_ID_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
}
