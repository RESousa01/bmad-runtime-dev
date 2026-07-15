use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use desktop_ipc::deserialize_strict;
use desktop_runtime::{
    canonical_hash_without_field, sha256_bytes, BmadKernelError, BmadLoadedPackage,
    BmadLocationClass, BmadPackageLoader, BmadSourceEntry, BmadSourceKind, BmadSourceSnapshot,
    RelativeWorkspacePath, Sha256Digest,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const MAX_RESOURCE_BYTES: u64 = 1_048_576;
const MAX_TOTAL_BYTES: usize = 16_777_216;
const EXPECTED_SEMANTIC_LEDGER_HASH: &str =
    "sha256:574ab4d79a8f954c9743741cf9912f5283a255b88a80b07550ed379865d8cc4f";
const EXPECTED_MANIFEST_HASH: &str =
    "sha256:81abbf71108d07f5b8ce2f54a04371b3a86e04cec49018dfe9cd1cfcb4f4c2e4";
const DESCRIPTOR_PATH: &str = "normalized/bmad-help.package.json";
const SEMANTIC_LEDGER_PATH: &str = "semantic-source-ledger.json";
const METHOD_RUNTIME_PATHS: [&str; 3] = [
    "runtime/method/6.10.0/architect-persona.instructions.md",
    "runtime/method/6.10.0/architecture-create.instructions.md",
    "runtime/method/6.10.0/bmad-help.instructions.md",
];
const EXPECTED_RESOURCE_PATHS: [&str; 19] = [
    "NOTICE.md",
    "adoption-ledger.json",
    "licenses/BMAD-BUILDER-MIT.txt",
    "licenses/BMAD-METHOD-MIT.txt",
    "normalized/bmad-architect.package.json",
    "normalized/bmad-architecture.package.json",
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
    package: BmadLoadedPackage,
    manifest_hash: Sha256Digest,
    semantic_ledger_hash: Sha256Digest,
    roster_agent_count: usize,
    inactive_builder_package_count: usize,
}

impl BmadLoadedFoundation {
    #[must_use]
    pub const fn package(&self) -> &BmadLoadedPackage {
        &self.package
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
    pub const fn roster_agent_count(&self) -> usize {
        self.roster_agent_count
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NormalizedRoster {
    package_version_id: String,
    agents: Vec<serde_json::Value>,
    roster_hash: Sha256Digest,
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
    let resources = read_manifest_resources(root, &manifest)?;
    let semantic_ledger_hash = validate_semantic_ledger(&manifest, &resources)?;
    let package = load_method_package(&resources, semantic_ledger_hash)?;
    let roster_agent_count = validate_roster(&resources)?;
    let inactive_builder_package_count = validate_builder_packages(&resources)?;

    Ok(BmadLoadedFoundation {
        package,
        manifest_hash: manifest.manifest_hash,
        semantic_ledger_hash,
        roster_agent_count,
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
) -> Result<BTreeMap<String, Vec<u8>>, BmadFoundationError> {
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
            || observed.insert(resource.path.clone(), bytes).is_some()
        {
            return Err(BmadFoundationError::ResourceMismatch);
        }
    }
    Ok(observed)
}

fn validate_semantic_ledger(
    manifest: &RuntimeManifest,
    resources: &BTreeMap<String, Vec<u8>>,
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
    resources: &BTreeMap<String, Vec<u8>>,
    semantic_ledger_hash: Sha256Digest,
) -> Result<BmadLoadedPackage, BmadFoundationError> {
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
    for path in METHOD_RUNTIME_PATHS {
        entries.push(source_entry(
            resources,
            path,
            BmadLocationClass::ManagedProjection,
        )?);
    }
    let snapshot = BmadSourceSnapshot::new(entries)?;
    BmadPackageLoader::load(&snapshot, semantic_ledger_hash).map_err(Into::into)
}

fn validate_roster(resources: &BTreeMap<String, Vec<u8>>) -> Result<usize, BmadFoundationError> {
    let roster: NormalizedRoster = deserialize_strict(required_bytes(
        resources,
        "normalized/bmm-agent-roster.json",
    )?)
    .map_err(|_| BmadFoundationError::ResourceMismatch)?;
    if roster.package_version_id.is_empty()
        || roster.agents.len() != 6
        || roster.roster_hash.to_string().is_empty()
    {
        return Err(BmadFoundationError::ResourceMismatch);
    }
    Ok(roster.agents.len())
}

fn validate_builder_packages(
    resources: &BTreeMap<String, Vec<u8>>,
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
    resources: &BTreeMap<String, Vec<u8>>,
    path: &str,
    location: BmadLocationClass,
) -> Result<BmadSourceEntry, BmadFoundationError> {
    BmadSourceEntry::new(
        path,
        required_bytes(resources, path)?.to_vec(),
        BmadSourceKind::SealedFoundation,
        location,
    )
    .map_err(Into::into)
}

fn required_bytes<'a>(
    resources: &'a BTreeMap<String, Vec<u8>>,
    path: &str,
) -> Result<&'a [u8], BmadFoundationError> {
    resources
        .get(path)
        .map(Vec::as_slice)
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

    use super::{load_bmad_foundation, BmadFoundationError};

    fn foundation_path() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/bmad-foundation")
    }

    #[test]
    fn loads_the_exact_sealed_method_and_inactive_builder_foundation() {
        let foundation = load_bmad_foundation(foundation_path()).expect("sealed foundation");
        assert_eq!(foundation.package().package_name, "bmad-method");
        assert_eq!(foundation.package().skills.len(), 2);
        assert_eq!(foundation.roster_agent_count(), 6);
        assert_eq!(foundation.inactive_builder_package_count(), 2);
        assert!(foundation
            .package()
            .skills
            .iter()
            .all(|skill| !skill.capability_enabled));
        assert_eq!(
            foundation.manifest_hash().to_string(),
            "sha256:81abbf71108d07f5b8ce2f54a04371b3a86e04cec49018dfe9cd1cfcb4f4c2e4"
        );
        assert_eq!(
            foundation.semantic_ledger_hash().to_string(),
            "sha256:574ab4d79a8f954c9743741cf9912f5283a255b88a80b07550ed379865d8cc4f"
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
