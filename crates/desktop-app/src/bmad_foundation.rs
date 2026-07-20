use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use desktop_ipc::deserialize_strict;
use desktop_runtime::{
    canonical_hash_without_field, sha256_bytes, BmadAgentRoster, BmadCatalog, BmadCatalogBuilder,
    BmadHelpCatalogSource, BmadKernelError, BmadLoadedMethodPackage, BmadLoadedPackage,
    BmadLocationClass, BmadPackageLoader, BmadSealedHelpInvocation, BmadSourceEntry,
    BmadSourceKind, BmadSourceSnapshot, RelativeWorkspacePath, Sha256Digest,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const MAX_RESOURCE_BYTES: u64 = 1_048_576;
const MAX_TOTAL_BYTES: usize = 16_777_216;
const EXPECTED_SEMANTIC_LEDGER_HASH: &str =
    "sha256:0813c069f75518a659fc7ec75cbd72f5c4b9ca896748e94b2e4d1777d870aa94";
const EXPECTED_MANIFEST_HASH: &str =
    "sha256:e0320ac6c913fd06ec4cb7522c2772869afbcc83914c4199f10a48b008de06bd";
const DESCRIPTOR_PATH: &str = "normalized/bmad-help.package.json";
const HELP_ACTION_GRAPH_PATH: &str = "normalized/bmad-help-action-graph.json";
const ADOPTION_LEDGER_PATH: &str = "adoption-ledger.json";
const SEMANTIC_LEDGER_PATH: &str = "semantic-source-ledger.json";
const HELP_INSTRUCTION_PATH: &str = "runtime/method/6.10.0/bmad-help.instructions.md";
const METHOD_RUNTIME_PATHS: [&str; 31] = [
    "runtime/method/6.10.0/analyst-persona.instructions.md",
    "runtime/method/6.10.0/architect-persona.instructions.md",
    "runtime/method/6.10.0/architecture-create.instructions.md",
    HELP_INSTRUCTION_PATH,
    "runtime/method/6.10.0/brainstorming.instructions.md",
    "runtime/method/6.10.0/code-review.instructions.md",
    "runtime/method/6.10.0/correct-course.instructions.md",
    "runtime/method/6.10.0/create-epics-and-stories.instructions.md",
    "runtime/method/6.10.0/create-story.instructions.md",
    "runtime/method/6.10.0/dev-persona.instructions.md",
    "runtime/method/6.10.0/dev-story.instructions.md",
    "runtime/method/6.10.0/document-project.instructions.md",
    "runtime/method/6.10.0/domain-research.instructions.md",
    "runtime/method/6.10.0/explain-concept.instructions.md",
    "runtime/method/6.10.0/implementation-readiness.instructions.md",
    "runtime/method/6.10.0/market-research.instructions.md",
    "runtime/method/6.10.0/mermaid-gen.instructions.md",
    "runtime/method/6.10.0/pm-persona.instructions.md",
    "runtime/method/6.10.0/prd.instructions.md",
    "runtime/method/6.10.0/prfaq.instructions.md",
    "runtime/method/6.10.0/product-brief.instructions.md",
    "runtime/method/6.10.0/qa-tests.instructions.md",
    "runtime/method/6.10.0/quick-dev.instructions.md",
    "runtime/method/6.10.0/retrospective.instructions.md",
    "runtime/method/6.10.0/sprint-planning.instructions.md",
    "runtime/method/6.10.0/tech-writer-persona.instructions.md",
    "runtime/method/6.10.0/technical-research.instructions.md",
    "runtime/method/6.10.0/ux-design.instructions.md",
    "runtime/method/6.10.0/ux-designer-persona.instructions.md",
    "runtime/method/6.10.0/validate-doc.instructions.md",
    "runtime/method/6.10.0/write-document.instructions.md",
];
const EXPECTED_RESOURCE_PATHS: [&str; 76] = [
    "NOTICE.md",
    "adoption-ledger.json",
    "licenses/BMAD-BUILDER-MIT.txt",
    "licenses/BMAD-METHOD-MIT.txt",
    "normalized/bmad-analyst.package.json",
    "normalized/bmad-architect.package.json",
    "normalized/bmad-architecture.package.json",
    "normalized/bmad-brainstorming.package.json",
    "normalized/bmad-check-implementation-readiness.package.json",
    "normalized/bmad-code-review.package.json",
    "normalized/bmad-correct-course.package.json",
    "normalized/bmad-create-epics-and-stories.package.json",
    "normalized/bmad-create-story.package.json",
    "normalized/bmad-dev-story.package.json",
    "normalized/bmad-dev.package.json",
    "normalized/bmad-document-project.package.json",
    "normalized/bmad-domain-research.package.json",
    "normalized/bmad-help-action-graph.json",
    "normalized/bmad-help.package.json",
    "normalized/bmad-market-research.package.json",
    "normalized/bmad-pm.package.json",
    "normalized/bmad-prd.package.json",
    "normalized/bmad-prfaq.package.json",
    "normalized/bmad-product-brief.package.json",
    "normalized/bmad-qa-generate-e2e-tests.package.json",
    "normalized/bmad-quick-dev.package.json",
    "normalized/bmad-retrospective.package.json",
    "normalized/bmad-sprint-planning.package.json",
    "normalized/bmad-tech-writer-explain-concept.package.json",
    "normalized/bmad-tech-writer-mermaid-gen.package.json",
    "normalized/bmad-tech-writer-validate-doc.package.json",
    "normalized/bmad-tech-writer-write-document.package.json",
    "normalized/bmad-tech-writer.package.json",
    "normalized/bmad-technical-research.package.json",
    "normalized/bmad-ux-designer.package.json",
    "normalized/bmad-ux.package.json",
    "normalized/bmm-agent-roster.json",
    "normalized/builder-agent.package.json",
    "normalized/builder-workflow.package.json",
    "runtime/builder/2.1.0/agent-analyze.instructions.md",
    "runtime/builder/2.1.0/agent-create-rebuild.instructions.md",
    "runtime/builder/2.1.0/agent-edit.instructions.md",
    "runtime/builder/2.1.0/workflow-analyze.instructions.md",
    "runtime/builder/2.1.0/workflow-build-edit.instructions.md",
    "runtime/method/6.10.0/analyst-persona.instructions.md",
    "runtime/method/6.10.0/architect-persona.instructions.md",
    "runtime/method/6.10.0/architecture-create.instructions.md",
    "runtime/method/6.10.0/bmad-help.instructions.md",
    "runtime/method/6.10.0/brainstorming.instructions.md",
    "runtime/method/6.10.0/code-review.instructions.md",
    "runtime/method/6.10.0/correct-course.instructions.md",
    "runtime/method/6.10.0/create-epics-and-stories.instructions.md",
    "runtime/method/6.10.0/create-story.instructions.md",
    "runtime/method/6.10.0/dev-persona.instructions.md",
    "runtime/method/6.10.0/dev-story.instructions.md",
    "runtime/method/6.10.0/document-project.instructions.md",
    "runtime/method/6.10.0/domain-research.instructions.md",
    "runtime/method/6.10.0/explain-concept.instructions.md",
    "runtime/method/6.10.0/implementation-readiness.instructions.md",
    "runtime/method/6.10.0/market-research.instructions.md",
    "runtime/method/6.10.0/mermaid-gen.instructions.md",
    "runtime/method/6.10.0/pm-persona.instructions.md",
    "runtime/method/6.10.0/prd.instructions.md",
    "runtime/method/6.10.0/prfaq.instructions.md",
    "runtime/method/6.10.0/product-brief.instructions.md",
    "runtime/method/6.10.0/qa-tests.instructions.md",
    "runtime/method/6.10.0/quick-dev.instructions.md",
    "runtime/method/6.10.0/retrospective.instructions.md",
    "runtime/method/6.10.0/sprint-planning.instructions.md",
    "runtime/method/6.10.0/tech-writer-persona.instructions.md",
    "runtime/method/6.10.0/technical-research.instructions.md",
    "runtime/method/6.10.0/ux-design.instructions.md",
    "runtime/method/6.10.0/ux-designer-persona.instructions.md",
    "runtime/method/6.10.0/validate-doc.instructions.md",
    "runtime/method/6.10.0/write-document.instructions.md",
    "semantic-source-ledger.json",
];

/// Every sealed persona projection envelope, joined to the roster by
/// `capability.skillName` == the roster agent code (ADR-0003 breadth).
const PERSONA_ENVELOPE_PATHS: [&str; 6] = [
    "normalized/bmad-analyst.package.json",
    "normalized/bmad-architect.package.json",
    "normalized/bmad-dev.package.json",
    "normalized/bmad-pm.package.json",
    "normalized/bmad-tech-writer.package.json",
    "normalized/bmad-ux-designer.package.json",
];

type FoundationResources = BTreeMap<String, Arc<[u8]>>;

#[derive(Debug, Error)]
pub enum BmadFoundationError {
    #[error("the bundled BMAD foundation root is unavailable")]
    RootUnavailable,
    #[error("the bundled BMAD foundation manifest is invalid")]
    ManifestInvalid,
    #[error("the bundled BMAD foundation resource inventory drifted")]
    ResourceMismatch,
    #[error("the bundled BMAD foundation exceeded its read limits")]
    LimitExceeded,
    #[error("the bundled BMAD foundation could not be read")]
    Io(#[from] std::io::Error),
    #[error("the bundled BMAD package did not pass native validation")]
    Kernel(#[from] BmadKernelError),
}

#[derive(Debug)]
pub struct BmadLoadedFoundation {
    method_package: BmadLoadedMethodPackage,
    catalog: BmadCatalog,
    roster: BmadAgentRoster,
    manifest_hash: Sha256Digest,
    semantic_ledger_hash: Sha256Digest,
    builder_packages: Vec<BmadBuilderPackageSummary>,
    personas: Vec<BmadSealedPersonaProjection>,
}

/// One roster agent's sealed persona instruction, verified against its
/// envelope's canonical hash chain and the roster's persona source hash.
/// Sealed read-only instruction data: not an agent identity, not a
/// dispatcher, and never projected to the renderer.
pub struct BmadSealedPersonaProjection {
    agent_code: String,
    instruction_path: String,
    instruction_hash: Sha256Digest,
    instruction_bytes: Arc<[u8]>,
    persona_source_hash: Sha256Digest,
}

impl BmadSealedPersonaProjection {
    #[must_use]
    pub fn agent_code(&self) -> &str {
        &self.agent_code
    }

    #[must_use]
    pub fn instruction_path(&self) -> &str {
        &self.instruction_path
    }

    #[must_use]
    pub const fn instruction_hash(&self) -> Sha256Digest {
        self.instruction_hash
    }

    #[must_use]
    pub fn instruction_bytes(&self) -> &[u8] {
        &self.instruction_bytes
    }

    #[must_use]
    pub const fn persona_source_hash(&self) -> Sha256Digest {
        self.persona_source_hash
    }
}

impl std::fmt::Debug for BmadSealedPersonaProjection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BmadSealedPersonaProjection")
            .field("agent_code", &self.agent_code)
            .field("instruction_hash", &self.instruction_hash)
            .finish_non_exhaustive()
    }
}

/// Kind of an installed-but-inactive Builder package.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuilderPackageKind {
    Agent,
    Workflow,
}

/// Display-safe summary of a validated, inactive Builder package. No
/// instruction content or filesystem detail is retained here.
#[derive(Clone, Debug)]
pub struct BmadBuilderPackageSummary {
    pub package_name: String,
    pub package_version: String,
    pub package_kind: BuilderPackageKind,
    pub display_name: String,
    pub resource_count: usize,
    pub descriptor_digest: Sha256Digest,
}

impl BmadLoadedFoundation {
    #[must_use]
    pub const fn package(&self) -> &BmadLoadedPackage {
        self.method_package.package()
    }

    #[must_use]
    pub const fn help_invocation(&self) -> &BmadSealedHelpInvocation {
        self.method_package.help_invocation()
    }

    #[must_use]
    pub const fn catalog(&self) -> &BmadCatalog {
        &self.catalog
    }

    #[must_use]
    pub const fn roster(&self) -> &BmadAgentRoster {
        &self.roster
    }

    #[must_use]
    pub const fn manifest_hash(&self) -> Sha256Digest {
        self.manifest_hash
    }

    #[must_use]
    pub const fn semantic_ledger_hash(&self) -> Sha256Digest {
        self.semantic_ledger_hash
    }

    #[must_use]
    pub fn inactive_builder_package_count(&self) -> usize {
        self.builder_packages.len()
    }

    #[must_use]
    pub fn builder_packages(&self) -> &[BmadBuilderPackageSummary] {
        &self.builder_packages
    }

    /// Every roster agent's sealed persona projection (six, sorted by
    /// agent code).
    #[must_use]
    pub fn personas(&self) -> &[BmadSealedPersonaProjection] {
        &self.personas
    }

    /// The sealed persona projection for one roster agent code.
    #[must_use]
    pub fn persona_for(&self, agent_code: &str) -> Option<&BmadSealedPersonaProjection> {
        self.personas
            .iter()
            .find(|persona| persona.agent_code == agent_code)
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RuntimeManifest {
    schema_version: String,
    foundation_version: String,
    semantic_ledger_hash: Sha256Digest,
    resources: Vec<RuntimeResource>,
    manifest_hash: Sha256Digest,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RuntimeResource {
    path: String,
    content_hash: Sha256Digest,
    byte_length: u64,
    content_kind: RuntimeContentKind,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum RuntimeContentKind {
    LegalNotice,
    ManagedInstruction,
    NormalizedContract,
    ProvenanceLedger,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FoundationHelpActionGraph {
    schema_version: String,
    package_version_id: String,
    sources: Vec<FoundationHelpActionSource>,
    graph_hash: Sha256Digest,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FoundationHelpActionSource {
    module_code: String,
    source_member_hash: Sha256Digest,
    rows: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BuilderFoundationPackage {
    schema_version: String,
    package_name: String,
    package_version: String,
    authoring_kind: String,
    lifecycle_state: String,
    activation_authority: String,
    validation_profile: String,
    resources: Vec<BuilderFoundationResource>,
    package_hash: Sha256Digest,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BuilderFoundationResource {
    path: String,
    content_hash: Sha256Digest,
    byte_length: u64,
    content_kind: RuntimeContentKind,
    source_member_ids: Vec<String>,
    actions: Vec<String>,
    entrypoint_kind: String,
}

/// Reads and validates the exact code-signed BMAD runtime resource set.
///
/// # Errors
///
/// Returns a stable error for missing or linked resources, bounded-read
/// overflow, manifest/resource drift, or a native contract/semantic failure.
pub fn load_bmad_foundation(
    root: impl AsRef<Path>,
) -> Result<BmadLoadedFoundation, BmadFoundationError> {
    let root = root.as_ref();
    let root_metadata =
        std::fs::symlink_metadata(root).map_err(|_| BmadFoundationError::RootUnavailable)?;
    if !root_metadata.is_dir() || root_metadata.file_type().is_symlink() {
        return Err(BmadFoundationError::RootUnavailable);
    }

    let manifest_bytes = read_bounded_regular(root, "runtime-manifest.json")?;
    let manifest: RuntimeManifest =
        deserialize_strict(&manifest_bytes).map_err(|_| BmadFoundationError::ManifestInvalid)?;
    validate_manifest_identity(&manifest)?;
    let roster_content_hash = manifest
        .resources
        .iter()
        .find(|resource| resource.path == "normalized/bmm-agent-roster.json")
        .map(|resource| resource.content_hash)
        .ok_or(BmadFoundationError::ResourceMismatch)?;
    let resources = read_manifest_resources(root, &manifest)?;
    let semantic_ledger_hash = validate_semantic_ledger(&manifest, &resources)?;
    let adoption_ledger_hash = sha256_bytes(required_bytes(&resources, ADOPTION_LEDGER_PATH)?);
    let method_package =
        load_method_package(&resources, semantic_ledger_hash, adoption_ledger_hash)?;
    let catalog = load_help_catalog(&resources, method_package.package())?;
    let roster = load_roster(
        &resources,
        method_package.package(),
        &catalog,
        roster_content_hash,
    )?;
    let builder_packages = validate_builder_packages(&resources)?;
    let personas = load_persona_projections(&resources)?;

    Ok(BmadLoadedFoundation {
        method_package,
        catalog,
        roster,
        manifest_hash: manifest.manifest_hash,
        semantic_ledger_hash,
        builder_packages,
        personas,
    })
}

/// Loads and verifies every sealed persona projection: strict envelope
/// shape, re-derived canonical projection and envelope hashes, managed
/// instruction bytes matching the pinned content hash, and the source
/// entrypoint hash matching the roster's persona source identity.
#[expect(
    clippy::too_many_lines,
    reason = "one linear verification chain over the persona envelopes"
)]
fn load_persona_projections(
    resources: &FoundationResources,
) -> Result<Vec<BmadSealedPersonaProjection>, BmadFoundationError> {
    use desktop_runtime::canonical_hash_without_field;

    let roster_value: serde_json::Value = serde_json::from_slice(required_bytes(
        resources,
        "normalized/bmm-agent-roster.json",
    )?)
    .map_err(|_| BmadFoundationError::ResourceMismatch)?;
    let roster_agents = roster_value
        .get("agents")
        .and_then(serde_json::Value::as_array)
        .ok_or(BmadFoundationError::ResourceMismatch)?;
    let persona_source_by_agent = roster_agents
        .iter()
        .map(|agent| {
            let code = agent
                .get("agentCode")
                .and_then(serde_json::Value::as_str)
                .ok_or(BmadFoundationError::ResourceMismatch)?;
            let hash = agent
                .get("personaSourceHash")
                .and_then(serde_json::Value::as_str)
                .ok_or(BmadFoundationError::ResourceMismatch)?;
            Ok((code.to_owned(), hash.to_owned()))
        })
        .collect::<Result<BTreeMap<_, _>, BmadFoundationError>>()?;

    let mut personas = Vec::with_capacity(PERSONA_ENVELOPE_PATHS.len());
    for envelope_path in PERSONA_ENVELOPE_PATHS {
        let envelope: serde_json::Value =
            serde_json::from_slice(required_bytes(resources, envelope_path)?)
                .map_err(|_| BmadFoundationError::ResourceMismatch)?;
        let envelope_hash = envelope
            .get("projectionEnvelopeHash")
            .and_then(serde_json::Value::as_str)
            .ok_or(BmadFoundationError::ResourceMismatch)?;
        let computed_envelope = canonical_hash_without_field(
            "bmad-foundation-method-projection",
            1,
            &envelope,
            "projectionEnvelopeHash",
        )
        .map_err(|_| BmadFoundationError::ResourceMismatch)?;
        if envelope_hash != computed_envelope.to_string()
            || envelope
                .get("schemaVersion")
                .and_then(serde_json::Value::as_str)
                != Some("sapphirus.bmad-foundation-method-projection.v1")
            || envelope
                .get("lifecycleState")
                .and_then(serde_json::Value::as_str)
                != Some("sealed_read_only")
        {
            return Err(BmadFoundationError::ResourceMismatch);
        }
        let projection = envelope
            .get("instructionProjection")
            .ok_or(BmadFoundationError::ResourceMismatch)?;
        let projection_hash = projection
            .get("projectionHash")
            .and_then(serde_json::Value::as_str)
            .ok_or(BmadFoundationError::ResourceMismatch)?;
        let computed_projection = canonical_hash_without_field(
            "bmad-instruction-projection",
            1,
            projection,
            "projectionHash",
        )
        .map_err(|_| BmadFoundationError::ResourceMismatch)?;
        if projection_hash != computed_projection.to_string() {
            return Err(BmadFoundationError::ResourceMismatch);
        }

        let agent_code = envelope
            .pointer("/capability/skillName")
            .and_then(serde_json::Value::as_str)
            .ok_or(BmadFoundationError::ResourceMismatch)?;
        let instruction_path = projection
            .pointer("/managedInstruction/path")
            .and_then(serde_json::Value::as_str)
            .ok_or(BmadFoundationError::ResourceMismatch)?;
        let declared_hash = projection
            .pointer("/managedInstruction/contentHash")
            .and_then(serde_json::Value::as_str)
            .ok_or(BmadFoundationError::ResourceMismatch)?;
        let bytes = resources
            .get(instruction_path)
            .ok_or(BmadFoundationError::ResourceMismatch)?;
        let observed_hash = sha256_bytes(bytes);
        if observed_hash.to_string() != declared_hash {
            return Err(BmadFoundationError::ResourceMismatch);
        }

        let entrypoint_hash = projection
            .pointer("/sourceEntrypoint/contentHash")
            .and_then(serde_json::Value::as_str)
            .ok_or(BmadFoundationError::ResourceMismatch)?;
        let roster_hash = persona_source_by_agent
            .get(agent_code)
            .ok_or(BmadFoundationError::ResourceMismatch)?;
        if entrypoint_hash != roster_hash {
            return Err(BmadFoundationError::ResourceMismatch);
        }
        personas.push(BmadSealedPersonaProjection {
            agent_code: agent_code.to_owned(),
            instruction_path: instruction_path.to_owned(),
            instruction_hash: observed_hash,
            instruction_bytes: Arc::clone(bytes),
            persona_source_hash: Sha256Digest::parse(entrypoint_hash)
                .map_err(|_| BmadFoundationError::ResourceMismatch)?,
        });
    }
    if personas.len() != PERSONA_ENVELOPE_PATHS.len() {
        return Err(BmadFoundationError::ResourceMismatch);
    }
    personas.sort_by(|left, right| left.agent_code.cmp(&right.agent_code));
    Ok(personas)
}

fn validate_manifest_identity(manifest: &RuntimeManifest) -> Result<(), BmadFoundationError> {
    let expected_manifest = Sha256Digest::parse(EXPECTED_MANIFEST_HASH)
        .map_err(|_| BmadFoundationError::ManifestInvalid)?;
    let expected_ledger = Sha256Digest::parse(EXPECTED_SEMANTIC_LEDGER_HASH)
        .map_err(|_| BmadFoundationError::ManifestInvalid)?;
    let computed = canonical_hash_without_field(
        "bmad-foundation-runtime-manifest",
        1,
        manifest,
        "manifestHash",
    )
    .map_err(|_| BmadFoundationError::ManifestInvalid)?;
    let paths = manifest
        .resources
        .iter()
        .map(|resource| resource.path.as_str())
        .collect::<Vec<_>>();
    if manifest.schema_version != "sapphirus.bmad-foundation-runtime-manifest.v1"
        || manifest.foundation_version != "0.1.0-beta.1"
        || manifest.manifest_hash != computed
        || manifest.manifest_hash != expected_manifest
        || manifest.semantic_ledger_hash != expected_ledger
        || paths != EXPECTED_RESOURCE_PATHS
    {
        return Err(BmadFoundationError::ManifestInvalid);
    }
    Ok(())
}

fn read_manifest_resources(
    root: &Path,
    manifest: &RuntimeManifest,
) -> Result<FoundationResources, BmadFoundationError> {
    let mut total = 0_usize;
    let mut observed = BTreeMap::new();
    for resource in &manifest.resources {
        RelativeWorkspacePath::new(resource.path.clone())
            .map_err(|_| BmadFoundationError::ManifestInvalid)?;
        if !content_kind_matches_path(resource.content_kind, &resource.path) {
            return Err(BmadFoundationError::ManifestInvalid);
        }
        let bytes = read_bounded_regular(root, &resource.path)?;
        total = total
            .checked_add(bytes.len())
            .filter(|value| *value <= MAX_TOTAL_BYTES)
            .ok_or(BmadFoundationError::LimitExceeded)?;
        if bytes.len() as u64 != resource.byte_length
            || sha256_bytes(&bytes) != resource.content_hash
            || observed
                .insert(resource.path.clone(), Arc::from(bytes))
                .is_some()
        {
            return Err(BmadFoundationError::ResourceMismatch);
        }
    }
    Ok(observed)
}

fn validate_semantic_ledger(
    manifest: &RuntimeManifest,
    resources: &FoundationResources,
) -> Result<Sha256Digest, BmadFoundationError> {
    let bytes = resources
        .get(SEMANTIC_LEDGER_PATH)
        .ok_or(BmadFoundationError::ResourceMismatch)?;
    let observed = sha256_bytes(bytes);
    if observed != manifest.semantic_ledger_hash {
        return Err(BmadFoundationError::ResourceMismatch);
    }
    Ok(observed)
}

fn load_method_package(
    resources: &FoundationResources,
    semantic_ledger_hash: Sha256Digest,
    adoption_ledger_hash: Sha256Digest,
) -> Result<BmadLoadedMethodPackage, BmadFoundationError> {
    let mut entries = vec![source_entry(
        resources,
        SEMANTIC_LEDGER_PATH,
        BmadLocationClass::ManagedMetadata,
    )?];
    entries.push(source_entry(
        resources,
        DESCRIPTOR_PATH,
        BmadLocationClass::ManagedMetadata,
    )?);
    entries.push(source_entry(
        resources,
        ADOPTION_LEDGER_PATH,
        BmadLocationClass::ManagedMetadata,
    )?);
    for path in METHOD_RUNTIME_PATHS {
        entries.push(source_entry(
            resources,
            path,
            BmadLocationClass::ManagedProjection,
        )?);
    }
    let snapshot = BmadSourceSnapshot::new(entries)?;
    BmadPackageLoader::load(&snapshot, semantic_ledger_hash, adoption_ledger_hash)
        .map_err(Into::into)
}

fn load_help_catalog(
    resources: &FoundationResources,
    package: &BmadLoadedPackage,
) -> Result<BmadCatalog, BmadFoundationError> {
    let graph: FoundationHelpActionGraph =
        deserialize_strict(required_bytes(resources, HELP_ACTION_GRAPH_PATH)?)
            .map_err(|_| BmadFoundationError::ResourceMismatch)?;
    let computed =
        canonical_hash_without_field("bmad-foundation-help-action-graph", 1, &graph, "graphHash")
            .map_err(|_| BmadFoundationError::ResourceMismatch)?;
    let expected_sources = [
        (
            "bmm",
            "sha256:ad4373d7e58a31aaef601ae39cf76b26bae7fd420b108e44660427384652d4bf",
        ),
        (
            "core",
            "sha256:e801caeb1bf6484277867067c60be3c2aeec39beaa75254e64ddf8ce8f3b617d",
        ),
    ];
    if graph.schema_version != "sapphirus.bmad-foundation-help-action-graph.v1"
        || graph.package_version_id != package.package_version_id.as_str()
        || graph.graph_hash != computed
        || graph.sources.len() != expected_sources.len()
        || graph
            .sources
            .iter()
            .zip(expected_sources)
            .any(|(source, (module_code, source_hash))| {
                source.module_code != module_code
                    || source.source_member_hash.to_string() != source_hash
                    || source.rows.len() != 1
            })
    {
        return Err(BmadFoundationError::ResourceMismatch);
    }
    let sources = graph
        .sources
        .iter()
        .map(|source| BmadHelpCatalogSource::from_rows(&source.module_code, &source.rows))
        .collect::<Result<Vec<_>, _>>()?;
    let catalog = BmadCatalogBuilder::build_bound(package, &sources, graph.graph_hash)?;
    if catalog.installed_skills.len() != package.skills.len() || catalog.help_actions.len() != 2 {
        return Err(BmadFoundationError::ResourceMismatch);
    }
    Ok(catalog)
}

fn load_roster(
    resources: &FoundationResources,
    package: &BmadLoadedPackage,
    catalog: &BmadCatalog,
    trusted_content_hash: Sha256Digest,
) -> Result<BmadAgentRoster, BmadFoundationError> {
    let roster = BmadAgentRoster::load_normalized(
        required_bytes(resources, "normalized/bmm-agent-roster.json")?,
        catalog,
        &package.package_version_id,
        trusted_content_hash,
    )?;
    if roster.agents.len() != 6 {
        return Err(BmadFoundationError::ResourceMismatch);
    }
    Ok(roster)
}

fn validate_builder_packages(
    resources: &FoundationResources,
) -> Result<Vec<BmadBuilderPackageSummary>, BmadFoundationError> {
    let expected = [
        (
            "normalized/builder-agent.package.json",
            "stateless_agent",
            "BuilderAgentV2Stateless",
            BuilderPackageKind::Agent,
            "Builder agent",
        ),
        (
            "normalized/builder-workflow.package.json",
            "simple_inline_workflow",
            "BuilderOutcomeSkillV2",
            BuilderPackageKind::Workflow,
            "Builder workflow",
        ),
    ];
    let mut summaries = Vec::with_capacity(expected.len());
    for (path, kind, profile, package_kind, display_name) in expected {
        let package: BuilderFoundationPackage =
            deserialize_strict(required_bytes(resources, path)?)
                .map_err(|_| BmadFoundationError::ResourceMismatch)?;
        let computed = canonical_hash_without_field(
            "bmad-foundation-builder-package",
            1,
            &package,
            "packageHash",
        )
        .map_err(|_| BmadFoundationError::ResourceMismatch)?;
        if package.schema_version != "sapphirus.bmad-foundation-builder-package.v1"
            || package.package_name != "bmad-builder"
            || package.package_version != "2.1.0"
            || package.authoring_kind != kind
            || package.lifecycle_state != "inactive_data"
            || package.activation_authority != "none"
            || package.validation_profile != profile
            || package.package_hash != computed
            || package.resources.is_empty()
            || package.resources.iter().any(|resource| {
                resource.source_member_ids.is_empty()
                    || resource.actions.is_empty()
                    || resources
                        .get(&resource.path)
                        .is_none_or(|bytes| resource.content_hash != sha256_bytes(bytes))
            })
        {
            return Err(BmadFoundationError::ResourceMismatch);
        }
        summaries.push(BmadBuilderPackageSummary {
            package_name: package.package_name,
            package_version: package.package_version,
            package_kind,
            display_name: display_name.to_owned(),
            resource_count: package.resources.len(),
            descriptor_digest: package.package_hash,
        });
    }
    Ok(summaries)
}

fn source_entry(
    resources: &FoundationResources,
    path: &str,
    location: BmadLocationClass,
) -> Result<BmadSourceEntry, BmadFoundationError> {
    BmadSourceEntry::new(
        path,
        Arc::clone(
            resources
                .get(path)
                .ok_or(BmadFoundationError::ResourceMismatch)?,
        ),
        BmadSourceKind::SealedFoundation,
        location,
    )
    .map_err(Into::into)
}

fn required_bytes<'a>(
    resources: &'a FoundationResources,
    path: &str,
) -> Result<&'a [u8], BmadFoundationError> {
    resources
        .get(path)
        .map(AsRef::as_ref)
        .ok_or(BmadFoundationError::ResourceMismatch)
}

fn content_kind_matches_path(kind: RuntimeContentKind, path: &str) -> bool {
    matches!(
        (kind, path),
        (RuntimeContentKind::ManagedInstruction, value) if value.starts_with("runtime/")
    ) || matches!(
        (kind, path),
        (RuntimeContentKind::NormalizedContract, value) if value.starts_with("normalized/")
    ) || matches!((kind, path), (RuntimeContentKind::LegalNotice, "NOTICE.md"))
        || matches!(
            (kind, path),
            (RuntimeContentKind::LegalNotice, value) if value.starts_with("licenses/")
        )
        || matches!(
            (kind, path),
            (
                RuntimeContentKind::ProvenanceLedger,
                "adoption-ledger.json" | "semantic-source-ledger.json"
            )
        )
}

fn read_bounded_regular(root: &Path, relative: &str) -> Result<Vec<u8>, BmadFoundationError> {
    RelativeWorkspacePath::new(relative.to_owned())
        .map_err(|_| BmadFoundationError::ManifestInvalid)?;
    let mut path = PathBuf::from(root);
    let segments = relative.split('/').collect::<Vec<_>>();
    for (index, segment) in segments.iter().enumerate() {
        path.push(segment);
        let metadata = std::fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink()
            || (index + 1 < segments.len() && !metadata.is_dir())
            || (index + 1 == segments.len()
                && (!metadata.is_file() || metadata.len() > MAX_RESOURCE_BYTES))
        {
            return Err(if metadata.len() > MAX_RESOURCE_BYTES {
                BmadFoundationError::LimitExceeded
            } else {
                BmadFoundationError::ResourceMismatch
            });
        }
    }
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_RESOURCE_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_RESOURCE_BYTES {
        return Err(BmadFoundationError::LimitExceeded);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{
        load_bmad_foundation, load_method_package, read_bounded_regular, read_manifest_resources,
        required_bytes, validate_manifest_identity, validate_semantic_ledger, BmadFoundationError,
        BmadSealedPersonaProjection, RuntimeManifest, ADOPTION_LEDGER_PATH, HELP_INSTRUCTION_PATH,
    };
    use desktop_ipc::deserialize_strict;
    use desktop_runtime::sha256_bytes;

    fn foundation_path() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/bmad-foundation")
    }

    #[test]
    fn loads_the_exact_sealed_method_and_inactive_builder_foundation() {
        let foundation = load_bmad_foundation(foundation_path()).expect("sealed foundation");
        assert_eq!(foundation.package().package_name, "bmad-method");
        assert_eq!(foundation.package().skills.len(), 2);
        assert_eq!(foundation.help_invocation().skill_name(), "bmad-help");
        assert_eq!(
            foundation.help_invocation().instruction_bytes().len(),
            1_283
        );
        assert_eq!(
            foundation
                .help_invocation()
                .adoption_ledger_hash()
                .to_string(),
            "sha256:c0a97ffddb064e06bcfe11f8a72789ffb02dc06c0c1706cad100ee4d6dd1e72f"
        );
        assert_eq!(foundation.catalog().installed_skills.len(), 2);
        assert_eq!(foundation.catalog().help_actions.len(), 2);
        assert!(foundation.catalog().help_actions.iter().any(|action| {
            action.module_code == "bmm"
                && action.skill_name == "bmad-architecture"
                && action.action.as_deref() == Some("create")
        }));
        assert_eq!(foundation.roster().agents.len(), 6);
        assert!(foundation.roster().agents.iter().any(|agent| {
            agent.agent_code == "bmad-agent-architect"
                && agent.display_name == "Winston"
                && agent.title == "System Architect"
        }));
        assert_eq!(foundation.inactive_builder_package_count(), 2);
        assert!(foundation
            .package()
            .skills
            .iter()
            .all(|skill| !skill.capability_enabled));
        assert_eq!(
            foundation.manifest_hash().to_string(),
            "sha256:e0320ac6c913fd06ec4cb7522c2772869afbcc83914c4199f10a48b008de06bd"
        );
        assert_eq!(
            foundation.semantic_ledger_hash().to_string(),
            "sha256:0813c069f75518a659fc7ec75cbd72f5c4b9ca896748e94b2e4d1777d870aa94"
        );
    }

    #[test]
    fn shares_manifest_owned_help_bytes_through_the_sealed_wrapper() {
        let root = foundation_path();
        let manifest_bytes =
            read_bounded_regular(&root, "runtime-manifest.json").expect("runtime manifest");
        let manifest: RuntimeManifest =
            deserialize_strict(&manifest_bytes).expect("strict runtime manifest");
        validate_manifest_identity(&manifest).expect("manifest identity");
        let resources = read_manifest_resources(&root, &manifest).expect("manifest resources");
        let semantic_hash =
            validate_semantic_ledger(&manifest, &resources).expect("semantic ledger");
        let adoption_hash = sha256_bytes(
            required_bytes(&resources, ADOPTION_LEDGER_PATH).expect("adoption ledger"),
        );
        let original_pointer = required_bytes(&resources, HELP_INSTRUCTION_PATH)
            .expect("manifest-owned Help bytes")
            .as_ptr();
        let method = load_method_package(&resources, semantic_hash, adoption_hash)
            .expect("sealed Method package");
        assert_eq!(
            original_pointer,
            method.help_invocation().instruction_bytes().as_ptr()
        );
    }

    #[test]
    fn rejects_a_manifest_bound_resource_after_byte_tampering() {
        let temporary = tempfile::tempdir().expect("temporary foundation");
        copy_tree(&foundation_path(), temporary.path());
        std::fs::write(
            temporary
                .path()
                .join("runtime/method/6.10.0/bmad-help.instructions.md"),
            b"tampered",
        )
        .expect("tamper resource");
        assert!(matches!(
            load_bmad_foundation(temporary.path()),
            Err(BmadFoundationError::ResourceMismatch)
        ));
    }

    #[test]
    fn rejects_runtime_manifest_identity_tampering_before_package_loading() {
        let temporary = tempfile::tempdir().expect("temporary foundation");
        copy_tree(&foundation_path(), temporary.path());
        let manifest_path = temporary.path().join("runtime-manifest.json");
        let manifest = std::fs::read_to_string(&manifest_path).expect("runtime manifest");
        let tampered = manifest.replace(
            "sha256:e0320ac6c913fd06ec4cb7522c2772869afbcc83914c4199f10a48b008de06bd",
            "sha256:1111111111111111111111111111111111111111111111111111111111111111",
        );
        std::fs::write(manifest_path, tampered).expect("tamper runtime manifest");
        assert!(matches!(
            load_bmad_foundation(temporary.path()),
            Err(BmadFoundationError::ManifestInvalid)
        ));
    }

    #[test]
    fn every_roster_agent_has_a_chain_verified_sealed_persona() {
        let foundation = load_bmad_foundation(foundation_path()).expect("sealed foundation");
        let personas = foundation.personas();
        assert_eq!(personas.len(), 6);
        let codes = personas
            .iter()
            .map(BmadSealedPersonaProjection::agent_code)
            .collect::<Vec<_>>();
        assert_eq!(
            codes,
            [
                "bmad-agent-analyst",
                "bmad-agent-architect",
                "bmad-agent-dev",
                "bmad-agent-pm",
                "bmad-agent-tech-writer",
                "bmad-agent-ux-designer",
            ],
        );
        for persona in personas {
            assert!(!persona.instruction_bytes().is_empty());
            assert_eq!(
                persona.instruction_hash(),
                sha256_bytes(persona.instruction_bytes()),
            );
            assert!(persona
                .instruction_path()
                .starts_with("runtime/method/6.10.0/"));
            let debug_output = format!("{persona:?}");
            assert!(!debug_output.contains("Working stance"));
        }
        let mary = foundation
            .persona_for("bmad-agent-analyst")
            .expect("Mary's sealed persona");
        let text = std::str::from_utf8(mary.instruction_bytes()).expect("utf8 instruction");
        assert!(text.contains("Managed analyst persona guidance"));
        assert!(foundation.persona_for("bmad-agent-unknown").is_none());
    }

    #[test]
    fn a_tampered_persona_envelope_fails_the_foundation_closed() {
        let temporary = tempfile::tempdir().expect("temporary foundation");
        copy_tree(&foundation_path(), temporary.path());
        let envelope_path = temporary
            .path()
            .join("normalized")
            .join("bmad-analyst.package.json");
        let envelope = std::fs::read_to_string(&envelope_path).expect("persona envelope");
        // Swap Mary's managed-instruction binding to Amelia's path: every
        // hash still parses, but the projection hash chain breaks.
        let tampered = envelope.replace(
            "analyst-persona.instructions.md",
            "dev-persona.instructions.md",
        );
        assert_ne!(envelope, tampered);
        std::fs::write(&envelope_path, tampered).expect("tamper persona envelope");
        assert!(matches!(
            load_bmad_foundation(temporary.path()),
            Err(BmadFoundationError::ResourceMismatch | BmadFoundationError::ManifestInvalid)
        ));
    }

    fn copy_tree(source: &std::path::Path, target: &std::path::Path) {
        std::fs::create_dir_all(target).expect("target directory");
        for entry in std::fs::read_dir(source).expect("source directory") {
            let entry = entry.expect("source entry");
            let destination = target.join(entry.file_name());
            if entry.file_type().expect("file type").is_dir() {
                copy_tree(&entry.path(), &destination);
            } else {
                std::fs::copy(entry.path(), destination).expect("copy file");
            }
        }
    }
}
