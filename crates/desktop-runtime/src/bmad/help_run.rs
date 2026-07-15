use std::fmt;
use std::sync::Arc;

use serde_json::{json, Map, Value};

use crate::{canonical_hash, canonical_hash_without_field, ContractId, Sha256Digest};

use super::{
    BmadKernelError, BmadKernelErrorCode, BmadLoadedPackage, BmadLocationClass, BmadSourceKind,
    BmadSourceSnapshot,
};

const ADOPTION_LEDGER_PATH: &str = "adoption-ledger.json";
const SEMANTIC_LEDGER_PATH: &str = "semantic-source-ledger.json";
const HELP_INSTRUCTION_PATH: &str = "runtime/method/6.10.0/bmad-help.instructions.md";
const HELP_INSTRUCTION_FORMAT: &str = "SapphirusManagedV1";
const MAX_HELP_INSTRUCTION_BYTES: usize = 65_536;
const HELP_SOURCE_MEMBER_IDS: [&str; 5] = [
    "method-001",
    "method-002",
    "method-003",
    "method-004",
    "method-005",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadHostInputReplacement {
    tool_intent: String,
    input_kind: String,
    input_schema_hash: Sha256Digest,
}

impl BmadHostInputReplacement {
    #[must_use]
    pub fn tool_intent(&self) -> &str {
        &self.tool_intent
    }

    #[must_use]
    pub fn input_kind(&self) -> &str {
        &self.input_kind
    }

    #[must_use]
    pub const fn input_schema_hash(&self) -> Sha256Digest {
        self.input_schema_hash
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadQualifiedHelpSource {
    path: String,
    content_hash: Sha256Digest,
    treatment: String,
}

impl BmadQualifiedHelpSource {
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub const fn content_hash(&self) -> Sha256Digest {
        self.content_hash
    }

    #[must_use]
    pub fn treatment(&self) -> &str {
        &self.treatment
    }
}

/// Package-owned, read-only Help invocation source.
///
/// The only constructor is kept inside the BMAD runtime. The type deliberately
/// implements neither Serde trait nor a content-bearing `Debug` representation.
///
/// ```compile_fail
/// use desktop_runtime::BmadSealedHelpInvocation;
/// let _ = BmadSealedHelpInvocation::new(Vec::new());
/// ```
#[derive(Clone)]
pub struct BmadSealedHelpInvocation {
    package_name: String,
    package_version: String,
    package_version_id: ContractId,
    descriptor_hash: Sha256Digest,
    source_snapshot_hash: Sha256Digest,
    observed_inventory_hash: Sha256Digest,
    final_inventory_hash: Sha256Digest,
    module_code: String,
    module_name: String,
    skill_name: String,
    source_entrypoint_path: String,
    source_entrypoint_hash: Sha256Digest,
    resource_set_hash: Sha256Digest,
    projection_hash: Sha256Digest,
    skill_descriptor_hash: Sha256Digest,
    execution_profile_hash: Sha256Digest,
    distribution_profile: String,
    install_profile: String,
    validation_profile: String,
    managed_instruction_path: String,
    managed_instruction_format: String,
    managed_instruction_hash: Sha256Digest,
    instruction_bytes: Arc<[u8]>,
    blocked_tool_intents: Vec<String>,
    host_input_replacements: Vec<BmadHostInputReplacement>,
    source_closure: Vec<BmadQualifiedHelpSource>,
    source_member_ids: Vec<String>,
    module_metadata_hash: Sha256Digest,
    module_help_catalog_hash: Sha256Digest,
    central_config_graph_hash: Sha256Digest,
    central_config_resolution_hash: Sha256Digest,
    semantic_ledger_hash: Sha256Digest,
    adoption_ledger_hash: Sha256Digest,
}

impl fmt::Debug for BmadSealedHelpInvocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BmadSealedHelpInvocation")
            .field("package_name", &self.package_name)
            .field("package_version", &self.package_version)
            .field("package_version_id", &self.package_version_id)
            .field("module_code", &self.module_code)
            .field("skill_name", &self.skill_name)
            .field("descriptor_hash", &self.descriptor_hash)
            .field("projection_hash", &self.projection_hash)
            .field("managed_instruction_hash", &self.managed_instruction_hash)
            .field(
                "instruction_bytes",
                &format_args!("<redacted:{} bytes>", self.instruction_bytes.len()),
            )
            .finish_non_exhaustive()
    }
}

impl BmadSealedHelpInvocation {
    #[must_use]
    pub fn package_name(&self) -> &str {
        &self.package_name
    }

    #[must_use]
    pub fn package_version(&self) -> &str {
        &self.package_version
    }

    #[must_use]
    pub const fn package_version_id(&self) -> &ContractId {
        &self.package_version_id
    }

    #[must_use]
    pub const fn descriptor_hash(&self) -> Sha256Digest {
        self.descriptor_hash
    }

    #[must_use]
    pub const fn source_snapshot_hash(&self) -> Sha256Digest {
        self.source_snapshot_hash
    }

    #[must_use]
    pub const fn observed_inventory_hash(&self) -> Sha256Digest {
        self.observed_inventory_hash
    }

    #[must_use]
    pub const fn final_inventory_hash(&self) -> Sha256Digest {
        self.final_inventory_hash
    }

    #[must_use]
    pub fn module_code(&self) -> &str {
        &self.module_code
    }

    #[must_use]
    pub fn module_name(&self) -> &str {
        &self.module_name
    }

    #[must_use]
    pub fn skill_name(&self) -> &str {
        &self.skill_name
    }

    #[must_use]
    pub fn source_entrypoint_path(&self) -> &str {
        &self.source_entrypoint_path
    }

    #[must_use]
    pub const fn source_entrypoint_hash(&self) -> Sha256Digest {
        self.source_entrypoint_hash
    }

    #[must_use]
    pub const fn resource_set_hash(&self) -> Sha256Digest {
        self.resource_set_hash
    }

    #[must_use]
    pub const fn projection_hash(&self) -> Sha256Digest {
        self.projection_hash
    }

    #[must_use]
    pub const fn skill_descriptor_hash(&self) -> Sha256Digest {
        self.skill_descriptor_hash
    }

    #[must_use]
    pub const fn execution_profile_hash(&self) -> Sha256Digest {
        self.execution_profile_hash
    }

    #[must_use]
    pub fn distribution_profile(&self) -> &str {
        &self.distribution_profile
    }

    #[must_use]
    pub fn install_profile(&self) -> &str {
        &self.install_profile
    }

    #[must_use]
    pub fn validation_profile(&self) -> &str {
        &self.validation_profile
    }

    #[must_use]
    pub fn managed_instruction_path(&self) -> &str {
        &self.managed_instruction_path
    }

    #[must_use]
    pub fn managed_instruction_format(&self) -> &str {
        &self.managed_instruction_format
    }

    #[must_use]
    pub const fn managed_instruction_hash(&self) -> Sha256Digest {
        self.managed_instruction_hash
    }

    #[must_use]
    pub fn instruction_bytes(&self) -> &[u8] {
        &self.instruction_bytes
    }

    pub(super) fn instruction_arc(&self) -> Arc<[u8]> {
        Arc::clone(&self.instruction_bytes)
    }

    #[must_use]
    pub fn blocked_tool_intents(&self) -> &[String] {
        &self.blocked_tool_intents
    }

    #[must_use]
    pub fn host_input_replacements(&self) -> &[BmadHostInputReplacement] {
        &self.host_input_replacements
    }

    #[must_use]
    pub fn source_closure(&self) -> &[BmadQualifiedHelpSource] {
        &self.source_closure
    }

    #[must_use]
    pub fn source_member_ids(&self) -> &[String] {
        &self.source_member_ids
    }

    #[must_use]
    pub const fn module_metadata_hash(&self) -> Sha256Digest {
        self.module_metadata_hash
    }

    #[must_use]
    pub const fn module_help_catalog_hash(&self) -> Sha256Digest {
        self.module_help_catalog_hash
    }

    #[must_use]
    pub const fn central_config_graph_hash(&self) -> Sha256Digest {
        self.central_config_graph_hash
    }

    #[must_use]
    pub const fn central_config_resolution_hash(&self) -> Sha256Digest {
        self.central_config_resolution_hash
    }

    #[must_use]
    pub const fn semantic_ledger_hash(&self) -> Sha256Digest {
        self.semantic_ledger_hash
    }

    #[must_use]
    pub const fn adoption_ledger_hash(&self) -> Sha256Digest {
        self.adoption_ledger_hash
    }
}

#[derive(Debug)]
pub struct BmadLoadedMethodPackage {
    package: BmadLoadedPackage,
    help_invocation: BmadSealedHelpInvocation,
}

impl BmadLoadedMethodPackage {
    pub(super) const fn new(
        package: BmadLoadedPackage,
        help_invocation: BmadSealedHelpInvocation,
    ) -> Self {
        Self {
            package,
            help_invocation,
        }
    }

    #[must_use]
    pub const fn package(&self) -> &BmadLoadedPackage {
        &self.package
    }

    #[must_use]
    pub const fn help_invocation(&self) -> &BmadSealedHelpInvocation {
        &self.help_invocation
    }
}

struct QualifiedPackageIdentity {
    package_version_id: ContractId,
    source_snapshot: Sha256Digest,
    final_inventory: Sha256Digest,
}

struct QualifiedSkillIdentity {
    source_entrypoint: Sha256Digest,
    resource_set: Sha256Digest,
    projection: Sha256Digest,
    skill_descriptor: Sha256Digest,
    execution_profile: Sha256Digest,
}

struct QualifiedModuleIdentity {
    metadata: Sha256Digest,
    help_catalog: Sha256Digest,
}

struct QualifiedConfigIdentity {
    graph: Sha256Digest,
    resolution: Sha256Digest,
}

pub(super) fn seal_help_invocation(
    snapshot: &BmadSourceSnapshot,
    descriptor: &Value,
    package: &BmadLoadedPackage,
    expected_semantic_ledger_hash: Sha256Digest,
    expected_adoption_ledger_hash: Sha256Digest,
) -> Result<BmadSealedHelpInvocation, BmadKernelError> {
    let semantic_entry = qualified_ledger(
        snapshot,
        SEMANTIC_LEDGER_PATH,
        expected_semantic_ledger_hash,
        BmadKernelErrorCode::SemanticLedgerMismatch,
    )?;
    let adoption_entry = qualified_ledger(
        snapshot,
        ADOPTION_LEDGER_PATH,
        expected_adoption_ledger_hash,
        BmadKernelErrorCode::AdoptionLedgerMismatch,
    )?;
    let semantic: Value = serde_json::from_slice(semantic_entry.bytes())
        .map_err(|_| BmadKernelErrorCode::SemanticLedgerMismatch)?;
    let adoption: Value = serde_json::from_slice(adoption_entry.bytes())
        .map_err(|_| BmadKernelErrorCode::AdoptionLedgerMismatch)?;
    let identity = validate_package_identity(snapshot, descriptor, package)?;
    let projection = validate_help_projection(descriptor, identity.source_snapshot)?;
    let (managed_instruction_hash, instruction_bytes) =
        validate_managed_instruction(snapshot, projection)?;
    let host_input_replacements = validate_host_replacements(projection)?;
    let source_closure = build_source_closure(projection)?;
    let skill = validate_help_skill(descriptor, projection)?;
    let module = validate_core_module(descriptor)?;
    let config = validate_central_config(descriptor, &identity.package_version_id)?;
    verify_ledger_closure(&semantic, &adoption, managed_instruction_hash)?;

    Ok(BmadSealedHelpInvocation {
        package_name: package.package_name.clone(),
        package_version: package.package_version.clone(),
        package_version_id: identity.package_version_id,
        descriptor_hash: package.descriptor_hash,
        source_snapshot_hash: identity.source_snapshot,
        observed_inventory_hash: package.observed_inventory_hash,
        final_inventory_hash: identity.final_inventory,
        module_code: "core".to_owned(),
        module_name: "Core".to_owned(),
        skill_name: "bmad-help".to_owned(),
        source_entrypoint_path: "src/core-skills/bmad-help/SKILL.md".to_owned(),
        source_entrypoint_hash: skill.source_entrypoint,
        resource_set_hash: skill.resource_set,
        projection_hash: skill.projection,
        skill_descriptor_hash: skill.skill_descriptor,
        execution_profile_hash: skill.execution_profile,
        distribution_profile: "sapphirus_package".to_owned(),
        install_profile: HELP_INSTRUCTION_FORMAT.to_owned(),
        validation_profile: "MethodOfficialSkillV6".to_owned(),
        managed_instruction_path: HELP_INSTRUCTION_PATH.to_owned(),
        managed_instruction_format: HELP_INSTRUCTION_FORMAT.to_owned(),
        managed_instruction_hash,
        instruction_bytes,
        blocked_tool_intents: vec!["file_read".to_owned(), "web".to_owned()],
        host_input_replacements,
        source_closure,
        source_member_ids: HELP_SOURCE_MEMBER_IDS
            .iter()
            .map(|member| (*member).to_owned())
            .collect(),
        module_metadata_hash: module.metadata,
        module_help_catalog_hash: module.help_catalog,
        central_config_graph_hash: config.graph,
        central_config_resolution_hash: config.resolution,
        semantic_ledger_hash: expected_semantic_ledger_hash,
        adoption_ledger_hash: expected_adoption_ledger_hash,
    })
}

fn validate_package_identity(
    snapshot: &BmadSourceSnapshot,
    descriptor: &Value,
    package: &BmadLoadedPackage,
) -> Result<QualifiedPackageIdentity, BmadKernelError> {
    let source_snapshot_hash = digest_field(descriptor, "sourceSnapshotHash")?;
    let final_inventory_hash = digest_field(descriptor, "finalCompositeInventoryHash")?;
    let source_identity = object_field(descriptor, "sourceIdentity")?;
    let root = descriptor_object(descriptor)?;
    let package_version_id = ContractId::new(string_field(root, "packageVersionId")?)
        .map_err(|_| BmadKernelErrorCode::SealedHelpMismatch)?;
    if string_field(source_identity, "sourceId")? != "method"
        || string_field(source_identity, "packageName")? != "bmad-method"
        || string_field(source_identity, "packageVersion")? != "6.10.0"
        || string_field(source_identity, "provenanceStatus")? != "blocked_provenance"
        || digest_map_field(source_identity, "sourceTreeHash")? != source_snapshot_hash
        || string_field(root, "distributionProfile")? != "sapphirus_package"
        || string_field(root, "installProfile")? != HELP_INSTRUCTION_FORMAT
        || package.package_name != "bmad-method"
        || package.package_version != "6.10.0"
        || package.package_version_id != package_version_id
        || package.descriptor_hash != digest_field(descriptor, "descriptorHash")?
        || package.observed_inventory_hash != snapshot.observed_inventory_hash()
        || final_inventory_hash != snapshot.observed_inventory_hash()
    {
        return sealed_mismatch();
    }
    Ok(QualifiedPackageIdentity {
        package_version_id,
        source_snapshot: source_snapshot_hash,
        final_inventory: final_inventory_hash,
    })
}

fn validate_help_projection(
    descriptor: &Value,
    source_snapshot_hash: Sha256Digest,
) -> Result<&Value, BmadKernelError> {
    let projection = exactly_one(
        array_field(descriptor, "instructionProjections")?,
        |candidate| {
            candidate
                .get("managedInstruction")
                .and_then(Value::as_object)
                .and_then(|managed| managed.get("path"))
                .and_then(Value::as_str)
                == Some(HELP_INSTRUCTION_PATH)
        },
    )?;
    verify_self_hash(projection, "bmad-instruction-projection", "projectionHash")?;
    if digest_field(projection, "sourceIdentityHash")? != source_snapshot_hash
        || projection.get("sourceSections") != Some(&json!([]))
        || projection.get("blockedToolIntents") != Some(&json!(["file_read", "web"]))
        || projection.get("sourceEntrypoint") != Some(&expected_source_entrypoint())
        || projection.get("sourceResources") != Some(&expected_source_resources())
    {
        return sealed_mismatch();
    }
    validate_projection_inventory(descriptor, projection)?;
    Ok(projection)
}

fn validate_projection_inventory(
    descriptor: &Value,
    projection: &Value,
) -> Result<(), BmadKernelError> {
    let inventory = array_field(descriptor, "resourceInventory")?;
    let mut sources = vec![projection
        .get("sourceEntrypoint")
        .ok_or(BmadKernelErrorCode::SealedHelpMismatch)?];
    sources.extend(array_field(projection, "sourceResources")?);
    for source in sources {
        let source_object = source
            .as_object()
            .ok_or(BmadKernelErrorCode::SealedHelpMismatch)?;
        let path = string_field(source_object, "path")?;
        let inventory_entry = exactly_one(inventory, |candidate| {
            candidate.get("path").and_then(Value::as_str) == Some(path)
        })?;
        if inventory_entry.get("locationKind").and_then(Value::as_str) != Some("source_tree")
            || inventory_entry.get("contentHash") != source.get("contentHash")
            || inventory_entry.get("treatment") != source.get("treatment")
        {
            return sealed_mismatch();
        }
    }
    Ok(())
}

fn validate_managed_instruction(
    snapshot: &BmadSourceSnapshot,
    projection: &Value,
) -> Result<(Sha256Digest, Arc<[u8]>), BmadKernelError> {
    let managed = object_field(projection, "managedInstruction")?;
    let instruction_hash = digest_map_field(managed, "contentHash")?;
    let entry = snapshot
        .entry(HELP_INSTRUCTION_PATH)
        .ok_or(BmadKernelErrorCode::SealedHelpMismatch)?;
    if string_field(managed, "path")? != HELP_INSTRUCTION_PATH
        || string_field(managed, "format")? != HELP_INSTRUCTION_FORMAT
        || entry.source_kind() != BmadSourceKind::SealedFoundation
        || entry.location() != BmadLocationClass::ManagedProjection
        || entry.bytes().len() != 1_283
        || entry.bytes().len() > MAX_HELP_INSTRUCTION_BYTES
        || entry.content_hash() != instruction_hash
    {
        return sealed_mismatch();
    }
    Ok((instruction_hash, entry.bytes_arc()))
}

fn validate_host_replacements(
    projection: &Value,
) -> Result<Vec<BmadHostInputReplacement>, BmadKernelError> {
    let replacements = projection
        .get("hostInputReplacements")
        .ok_or(BmadKernelErrorCode::SealedHelpMismatch)?;
    if replacements != &expected_host_replacements() {
        return sealed_mismatch();
    }
    replacements
        .as_array()
        .ok_or(BmadKernelErrorCode::SealedHelpMismatch)?
        .iter()
        .map(|replacement| {
            let object = replacement
                .as_object()
                .ok_or(BmadKernelErrorCode::SealedHelpMismatch)?;
            Ok(BmadHostInputReplacement {
                tool_intent: string_field(object, "toolIntent")?.to_owned(),
                input_kind: string_field(object, "inputKind")?.to_owned(),
                input_schema_hash: digest_map_field(object, "inputSchemaHash")?,
            })
        })
        .collect()
}

fn build_source_closure(
    projection: &Value,
) -> Result<Vec<BmadQualifiedHelpSource>, BmadKernelError> {
    let mut closure = Vec::with_capacity(5);
    closure.push(qualified_source(
        projection
            .get("sourceEntrypoint")
            .ok_or(BmadKernelErrorCode::SealedHelpMismatch)?,
    )?);
    for resource in array_field(projection, "sourceResources")? {
        closure.push(qualified_source(resource)?);
    }
    closure.sort_by(|left, right| left.path.cmp(&right.path));
    if closure
        .windows(2)
        .any(|members| members[0].path == members[1].path)
    {
        return sealed_mismatch();
    }
    Ok(closure)
}

fn validate_help_skill(
    descriptor: &Value,
    projection: &Value,
) -> Result<QualifiedSkillIdentity, BmadKernelError> {
    let skill = exactly_one(array_field(descriptor, "skills")?, |candidate| {
        candidate.get("moduleCode").and_then(Value::as_str) == Some("core")
            && candidate.get("skillName").and_then(Value::as_str) == Some("bmad-help")
    })?;
    verify_self_hash(skill, "bmad-skill-descriptor", "skillDescriptorHash")?;
    let execution = skill
        .get("executionProfile")
        .ok_or(BmadKernelErrorCode::SealedHelpMismatch)?;
    verify_self_hash(execution, "bmad-execution-profile", "profileHash")?;
    let mut observed_execution = execution.clone();
    observed_execution
        .as_object_mut()
        .ok_or(BmadKernelErrorCode::SealedHelpMismatch)?
        .remove("profileHash");
    let resource_preimage = json!({
        "sourceEntrypoint": projection["sourceEntrypoint"].clone(),
        "sourceResources": projection["sourceResources"].clone(),
        "managedInstruction": projection["managedInstruction"].clone()
    });
    let computed_resource_set_hash =
        canonical_hash("bmad-skill-resource-set", 1, &resource_preimage)
            .map_err(|_| BmadKernelErrorCode::SealedHelpMismatch)?;
    let source_entrypoint_hash = digest_field(skill, "sourceEntrypointHash")?;
    let resource_set_hash = digest_field(skill, "resourceSetHash")?;
    let projection_hash = digest_field(projection, "projectionHash")?;
    if observed_execution != expected_execution_profile()
        || string_field(descriptor_object(skill)?, "sourceEntrypointPath")?
            != "src/core-skills/bmad-help/SKILL.md"
        || source_entrypoint_hash != digest_field(&expected_source_entrypoint(), "contentHash")?
        || resource_set_hash != computed_resource_set_hash
        || digest_field(skill, "instructionProjectionHash")? != projection_hash
        || string_field(descriptor_object(skill)?, "distributionProfile")? != "sapphirus_package"
        || string_field(descriptor_object(skill)?, "installProfile")? != HELP_INSTRUCTION_FORMAT
    {
        return sealed_mismatch();
    }
    Ok(QualifiedSkillIdentity {
        source_entrypoint: source_entrypoint_hash,
        resource_set: resource_set_hash,
        projection: projection_hash,
        skill_descriptor: digest_field(skill, "skillDescriptorHash")?,
        execution_profile: digest_field(execution, "profileHash")?,
    })
}

fn validate_core_module(descriptor: &Value) -> Result<QualifiedModuleIdentity, BmadKernelError> {
    let module = exactly_one(array_field(descriptor, "modules")?, |candidate| {
        candidate.get("moduleCode").and_then(Value::as_str) == Some("core")
    })?;
    if module != &expected_core_module() {
        return sealed_mismatch();
    }
    Ok(QualifiedModuleIdentity {
        metadata: digest_field(module, "metadataSourceHash")?,
        help_catalog: digest_field(module, "helpCatalogSourceHash")?,
    })
}

fn validate_central_config(
    descriptor: &Value,
    package_version_id: &ContractId,
) -> Result<QualifiedConfigIdentity, BmadKernelError> {
    let graph = exactly_one(array_field(descriptor, "configGraphs")?, |candidate| {
        candidate.get("graphKind").and_then(Value::as_str) == Some("method_central_toml")
    })?;
    verify_self_hash(graph, "bmad-config-graph", "graphHash")?;
    let expected_scope = json!({
        "packageVersionId": package_version_id.as_str(),
        "moduleCode": null,
        "skillName": null
    });
    if graph.get("scope") != Some(&expected_scope)
        || graph.get("layers") != Some(&json!([]))
        || graph.get("mergeSemantics") != Some(&expected_merge_semantics())
    {
        return sealed_mismatch();
    }
    let graph_hash = digest_field(graph, "graphHash")?;
    let resolution = exactly_one(array_field(descriptor, "configResolutions")?, |candidate| {
        candidate.get("graphKind").and_then(Value::as_str) == Some("method_central_toml")
    })?;
    verify_self_hash(resolution, "bmad-config-resolution", "resolutionHash")?;
    if resolution.get("scope") != Some(&expected_scope)
        || digest_field(resolution, "graphHash")? != graph_hash
        || resolution.get("orderedLayerHashes") != Some(&json!([]))
        || resolution.get("resolvedEntries") != Some(&json!([]))
        || resolution.get("warnings") != Some(&json!([]))
    {
        return sealed_mismatch();
    }
    Ok(QualifiedConfigIdentity {
        graph: graph_hash,
        resolution: digest_field(resolution, "resolutionHash")?,
    })
}

fn expected_source_entrypoint() -> Value {
    json!({
        "path": "src/core-skills/bmad-help/SKILL.md",
        "contentHash": "sha256:718077d741e20d9c94f3c2b7827047f2d18a90b85c3cc2eecd449e28b7b0d642",
        "treatment": "adapt"
    })
}

fn expected_source_resources() -> Value {
    json!([
        {"path": "src/bmm-skills/module-help.csv", "contentHash": "sha256:ad4373d7e58a31aaef601ae39cf76b26bae7fd420b108e44660427384652d4bf", "treatment": "adopt"},
        {"path": "src/bmm-skills/module.yaml", "contentHash": "sha256:5a2a4ff761b3a4f92730442386486f32318152fc0dfdd225dc6765a3bc2ec100", "treatment": "adopt"},
        {"path": "src/core-skills/module-help.csv", "contentHash": "sha256:e801caeb1bf6484277867067c60be3c2aeec39beaa75254e64ddf8ce8f3b617d", "treatment": "adopt"},
        {"path": "src/core-skills/module.yaml", "contentHash": "sha256:46f8972746f0d4e49358fdf94b0c1ba856fd7a8eb66abc75d5aaff0624540479", "treatment": "adopt"}
    ])
}

fn expected_host_replacements() -> Value {
    json!([
        {"toolIntent": "file_read", "inputKind": "catalog_snapshot", "inputSchemaHash": "sha256:4dc4d3136db3c7ac2a40c61f12658db27791f525e8559f67bdaac7a018a50ddc"},
        {"toolIntent": "web", "inputKind": "unavailable_fact", "inputSchemaHash": "sha256:00584aeb615fd1e6ba32e4e781862cf77b6525b2a7c0dca095e6ba9adf084697"}
    ])
}

fn expected_execution_profile() -> Value {
    json!({
        "entrypointKind": "direct",
        "invocationModes": {"interactive": true, "headless": false, "actions": []},
        "requiredRuntimes": [{"runtime": "node", "versionRange": ">=20.12.0", "required": true}],
        "resourcePolicy": {"entrypointTiming": "invocation_start", "resourceTiming": "all_declared_at_start", "declaredResourcePaths": []},
        "declaredToolIntents": [], "stateHints": [], "completionEvidence": ["artifact"],
        "customizationProfile": "method_skill_toml", "validationProfile": "MethodOfficialSkillV6"
    })
}

fn expected_core_module() -> Value {
    json!({
        "moduleCode": "core", "moduleName": "Core", "moduleVersion": null,
        "metadataOrigin": "source",
        "metadataSourceHash": "sha256:46f8972746f0d4e49358fdf94b0c1ba856fd7a8eb66abc75d5aaff0624540479",
        "helpCatalogSourceHash": "sha256:e801caeb1bf6484277867067c60be3c2aeec39beaa75254e64ddf8ce8f3b617d",
        "agentRosterSourceHash": null
    })
}

fn expected_merge_semantics() -> Value {
    json!({
        "scalarRule": "later_replaces", "tableRule": "recursive_merge",
        "keyedTableArrayRule": "merge_by_code_or_id_when_all_items_keyed",
        "otherArrayRule": "append", "deletionOperator": "none"
    })
}

fn qualified_ledger<'a>(
    snapshot: &'a BmadSourceSnapshot,
    path: &str,
    expected_hash: Sha256Digest,
    mismatch: BmadKernelErrorCode,
) -> Result<&'a super::BmadSourceEntry, BmadKernelError> {
    let entry = snapshot.entry(path).ok_or(mismatch)?;
    if entry.location() != BmadLocationClass::ManagedMetadata
        || entry.source_kind() != BmadSourceKind::SealedFoundation
        || entry.content_hash() != expected_hash
    {
        return Err(mismatch.into());
    }
    Ok(entry)
}

fn verify_ledger_closure(
    semantic: &Value,
    adoption: &Value,
    instruction_hash: Sha256Digest,
) -> Result<(), BmadKernelError> {
    if semantic.get("schemaVersion").and_then(Value::as_str)
        != Some("sapphirus.bmad.semantic-source-ledger/v1")
        || adoption.get("schemaVersion").and_then(Value::as_str)
            != Some("sapphirus.bmad.adoption-ledger/v1")
        || adoption.get("operationalAuthority").and_then(Value::as_str) != Some("none")
        || adoption.get("promotionEligibility").and_then(Value::as_str)
            != Some("blocked_provenance")
    {
        return sealed_mismatch();
    }

    let projection = exactly_one(array_field(adoption, "runtimeProjections")?, |candidate| {
        candidate.get("path").and_then(Value::as_str) == Some(HELP_INSTRUCTION_PATH)
    })?;
    if projection
        != &json!({
            "path": HELP_INSTRUCTION_PATH,
            "classification": "method",
            "sourceIdentity": {
                "sourceId": "method",
                "skill": "bmad-help",
                "profile": "MethodOfficialSkillV6"
            },
            "state": "sealed_read_only",
            "actions": [],
            "action": null,
            "authority": "none",
            "distributionProfile": "sapphirus_package",
            "installProfile": HELP_INSTRUCTION_FORMAT,
            "entrypointKind": "direct",
            "validationProfile": "MethodOfficialSkillV6",
            "sourceMemberIds": HELP_SOURCE_MEMBER_IDS
        })
    {
        return sealed_mismatch();
    }

    let output = exactly_one(array_field(semantic, "managedOutputs")?, |candidate| {
        candidate.get("path").and_then(Value::as_str) == Some(HELP_INSTRUCTION_PATH)
    })?;
    let raw_instruction_hash = instruction_hash
        .to_string()
        .strip_prefix("sha256:")
        .ok_or(BmadKernelErrorCode::SealedHelpMismatch)?
        .to_owned();
    if output
        != &json!({
            "path": HELP_INSTRUCTION_PATH,
            "byteLength": 1283,
            "sha256": raw_instruction_hash
        })
    {
        return sealed_mismatch();
    }

    let source_members = array_field(semantic, "sourceMembers")?;
    let source_decisions = array_field(adoption, "sourceDecisions")?;
    let expected_members = [
        (
            "method-001",
            "src/core-skills/bmad-help/SKILL.md",
            "718077d741e20d9c94f3c2b7827047f2d18a90b85c3cc2eecd449e28b7b0d642",
        ),
        (
            "method-002",
            "src/core-skills/module.yaml",
            "46f8972746f0d4e49358fdf94b0c1ba856fd7a8eb66abc75d5aaff0624540479",
        ),
        (
            "method-003",
            "src/core-skills/module-help.csv",
            "e801caeb1bf6484277867067c60be3c2aeec39beaa75254e64ddf8ce8f3b617d",
        ),
        (
            "method-004",
            "src/bmm-skills/module.yaml",
            "5a2a4ff761b3a4f92730442386486f32318152fc0dfdd225dc6765a3bc2ec100",
        ),
        (
            "method-005",
            "src/bmm-skills/module-help.csv",
            "ad4373d7e58a31aaef601ae39cf76b26bae7fd420b108e44660427384652d4bf",
        ),
    ];
    for (id, path, hash) in expected_members {
        let member = exactly_one(source_members, |candidate| {
            candidate.get("id").and_then(Value::as_str) == Some(id)
        })?;
        let decision = exactly_one(source_decisions, |candidate| {
            candidate.get("sourceMemberId").and_then(Value::as_str) == Some(id)
        })?;
        if member.get("sourceId").and_then(Value::as_str) != Some("method")
            || member.get("member").and_then(Value::as_str) != Some(path)
            || member.get("sha256").and_then(Value::as_str) != Some(hash)
            || member.get("treatments") != decision.get("treatments")
        {
            return sealed_mismatch();
        }
    }
    Ok(())
}

fn qualified_source(value: &Value) -> Result<BmadQualifiedHelpSource, BmadKernelError> {
    let object = value
        .as_object()
        .ok_or(BmadKernelErrorCode::SealedHelpMismatch)?;
    Ok(BmadQualifiedHelpSource {
        path: string_field(object, "path")?.to_owned(),
        content_hash: digest_map_field(object, "contentHash")?,
        treatment: string_field(object, "treatment")?.to_owned(),
    })
}

fn verify_self_hash(value: &Value, purpose: &str, field: &str) -> Result<(), BmadKernelError> {
    let declared = digest_field(value, field)?;
    let computed = canonical_hash_without_field(purpose, 1, value, field)
        .map_err(|_| BmadKernelErrorCode::SealedHelpMismatch)?;
    if declared != computed {
        return sealed_mismatch();
    }
    Ok(())
}

fn exactly_one(
    values: &[Value],
    predicate: impl Fn(&Value) -> bool,
) -> Result<&Value, BmadKernelError> {
    let mut matches = values.iter().filter(|value| predicate(value));
    let result = matches
        .next()
        .ok_or(BmadKernelErrorCode::SealedHelpMismatch)?;
    if matches.next().is_some() {
        return sealed_mismatch();
    }
    Ok(result)
}

fn array_field<'a>(value: &'a Value, field: &str) -> Result<&'a [Value], BmadKernelError> {
    value
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| BmadKernelErrorCode::SealedHelpMismatch.into())
}

fn object_field<'a>(
    value: &'a Value,
    field: &str,
) -> Result<&'a Map<String, Value>, BmadKernelError> {
    value
        .get(field)
        .and_then(Value::as_object)
        .ok_or_else(|| BmadKernelErrorCode::SealedHelpMismatch.into())
}

fn descriptor_object(value: &Value) -> Result<&Map<String, Value>, BmadKernelError> {
    value
        .as_object()
        .ok_or_else(|| BmadKernelErrorCode::SealedHelpMismatch.into())
}

fn string_field<'a>(
    value: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, BmadKernelError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| BmadKernelErrorCode::SealedHelpMismatch.into())
}

fn digest_map_field(
    value: &Map<String, Value>,
    field: &str,
) -> Result<Sha256Digest, BmadKernelError> {
    Sha256Digest::parse(string_field(value, field)?)
        .map_err(|_| BmadKernelErrorCode::SealedHelpMismatch.into())
}

fn digest_field(value: &Value, field: &str) -> Result<Sha256Digest, BmadKernelError> {
    let object = descriptor_object(value)?;
    digest_map_field(object, field)
}

fn sealed_mismatch<T>() -> Result<T, BmadKernelError> {
    Err(BmadKernelErrorCode::SealedHelpMismatch.into())
}
