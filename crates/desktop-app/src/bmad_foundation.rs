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
    "sha256:574ab4d79a8f954c9743741cf9912f5283a255b88a80b07550ed379865d8cc4f";
const EXPECTED_MANIFEST_HASH: &str =
    "sha256:ee97e0ebc6cff9d31fbe136a6eb52b28a084fa72351fb4ab68ca79fd66ee1fc1";
const DESCRIPTOR_PATH: &str = "normalized/bmad-help.package.json";
const HELP_ACTION_GRAPH_PATH: &str = "normalized/bmad-help-action-graph.json";
const ADOPTION_LEDGER_PATH: &str = "adoption-ledger.json";
const SEMANTIC_LEDGER_PATH: &str = "semantic-source-ledger.json";
const HELP_INSTRUCTION_PATH: &str = "runtime/method/6.10.0/bmad-help.instructions.md";
const METHOD_RUNTIME_PATHS: [&str; 3] = [
    "runtime/method/6.10.0/architect-persona.instructions.md",
    "runtime/method/6.10.0/architecture-create.instructions.md",
    HELP_INSTRUCTION_PATH,
];
const EXPECTED_RESOURCE_PATHS: [&str; 20] = [
    "NOTICE.md",
    "adoption-ledger.json",
    "licenses/BMAD-BUILDER-MIT.txt",
    "licenses/BMAD-METHOD-MIT.txt",
    "normalized/bmad-architect.package.json",
    "normalized/bmad-architecture.package.json",
    "normalized/bmad-help-action-graph.json",
    "normalized/bmad-help.package.json",
    "normalized/bmm-agent-roster.json",
    "normalized/builder-agent.package.json",
    "normalized/builder-workflow.package.json",
    "runtime/builder/2.1.0/agent-analyze.instructions.md",
    "runtime/builder/2.1.0/agent-create-rebuild.instructions.md",
    "runtime/builder/2.1.0/agent-edit.instructions.md",
    "runtime/builder/2.1.0/workflow-analyze.instructions.md",
    "runtime/builder/2.1.0/workflow-build-edit.instructions.md",
    "runtime/method/6.10.0/architect-persona.instructions.md",
    "runtime/method/6.10.0/architecture-create.instructions.md",
    "runtime/method/6.10.0/bmad-help.instructions.md",
    "semantic-source-ledger.json",
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
    inactive_builder_package_count: usize,
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
    pub const fn inactive_builder_package_count(&self) -> usize {
        self.inactive_builder_package_count
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
    let inactive_builder_package_count = validate_builder_packages(&resources)?;

    Ok(BmadLoadedFoundation {
        method_package,
        catalog,
        roster,
        manifest_hash: manifest.manifest_hash,
        semantic_ledger_hash,
        inactive_builder_package_count,
    })
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
) -> Result<usize, BmadFoundationError> {
    let expected = [
        (
            "normalized/builder-agent.package.json",
            "stateless_agent",
            "BuilderAgentV2Stateless",
        ),
        (
            "normalized/builder-workflow.package.json",
            "simple_inline_workflow",
            "BuilderOutcomeSkillV2",
        ),
    ];
    for (path, kind, profile) in expected {
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
    }
    Ok(expected.len())
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
        RuntimeManifest, ADOPTION_LEDGER_PATH, HELP_INSTRUCTION_PATH,
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
            "sha256:7e187635bfe004dcf01ca30f8d22f1f810dd1e1ddd0646349123305e3025414d"
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
            "sha256:ee97e0ebc6cff9d31fbe136a6eb52b28a084fa72351fb4ab68ca79fd66ee1fc1"
        );
        assert_eq!(
            foundation.semantic_ledger_hash().to_string(),
            "sha256:574ab4d79a8f954c9743741cf9912f5283a255b88a80b07550ed379865d8cc4f"
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
            "sha256:ee97e0ebc6cff9d31fbe136a6eb52b28a084fa72351fb4ab68ca79fd66ee1fc1",
            "sha256:1111111111111111111111111111111111111111111111111111111111111111",
        );
        std::fs::write(manifest_path, tampered).expect("tamper runtime manifest");
        assert!(matches!(
            load_bmad_foundation(temporary.path()),
            Err(BmadFoundationError::ManifestInvalid)
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
