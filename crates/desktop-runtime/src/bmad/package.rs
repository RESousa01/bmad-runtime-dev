use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::Value;
use unicode_normalization::UnicodeNormalization;

use crate::{
    canonical_hash, canonical_hash_without_field, generated_contracts, sha256_bytes,
    RelativeWorkspacePath, Sha256Digest,
};

use super::{BmadKernelError, BmadKernelErrorCode};

const DESCRIPTOR_PATH: &str = "normalized/bmad-help.package.json";
const SEMANTIC_LEDGER_PATH: &str = "semantic-source-ledger.json";
const MAX_SOURCE_ENTRIES: usize = 4_096;
const MAX_SOURCE_ENTRY_BYTES: usize = 1_048_576;
const MAX_SOURCE_TOTAL_BYTES: usize = 16_777_216;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadSourceKind {
    SealedFoundation,
    MethodComposite,
    BuilderPackage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadLocationClass {
    SourceTree,
    ManagedProjection,
    ManagedMetadata,
    FinalInstall,
    HostNativeAgents,
    HostNativeClaude,
    BmadControl,
}

impl BmadLocationClass {
    const fn contributes_to_final_inventory(self) -> bool {
        matches!(
            self,
            Self::ManagedProjection
                | Self::FinalInstall
                | Self::HostNativeAgents
                | Self::HostNativeClaude
                | Self::BmadControl
        )
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::SourceTree => "source_tree",
            Self::ManagedProjection => "managed_projection",
            Self::ManagedMetadata => "managed_metadata",
            Self::FinalInstall => "final_install",
            Self::HostNativeAgents => "host_native_agents",
            Self::HostNativeClaude => "host_native_claude",
            Self::BmadControl => "bmad_control",
        }
    }
}

#[derive(Clone, Debug)]
pub struct BmadSourceEntry {
    path: RelativeWorkspacePath,
    bytes: Vec<u8>,
    content_hash: Sha256Digest,
    source_kind: BmadSourceKind,
    location: BmadLocationClass,
}

impl BmadSourceEntry {
    /// Creates one bounded source entry and derives its hash from observed bytes.
    ///
    /// # Errors
    ///
    /// Returns an error for unsafe paths, non-canonical Unicode, directional
    /// controls, or bytes larger than the per-entry bound.
    pub fn new(
        path: impl Into<String>,
        bytes: Vec<u8>,
        source_kind: BmadSourceKind,
        location: BmadLocationClass,
    ) -> Result<Self, BmadKernelError> {
        let path = path.into();
        if bytes.len() > MAX_SOURCE_ENTRY_BYTES || !is_safe_source_path(&path) {
            return Err(if bytes.len() > MAX_SOURCE_ENTRY_BYTES {
                BmadKernelErrorCode::SourceLimitExceeded
            } else {
                BmadKernelErrorCode::SourcePathInvalid
            }
            .into());
        }
        let path =
            RelativeWorkspacePath::new(path).map_err(|_| BmadKernelErrorCode::SourcePathInvalid)?;
        let content_hash = sha256_bytes(&bytes);
        Ok(Self {
            path,
            bytes,
            content_hash,
            source_kind,
            location,
        })
    }

    #[must_use]
    pub fn path(&self) -> &str {
        self.path.as_str()
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[must_use]
    pub const fn content_hash(&self) -> Sha256Digest {
        self.content_hash
    }

    #[must_use]
    pub const fn source_kind(&self) -> BmadSourceKind {
        self.source_kind
    }

    #[must_use]
    pub const fn location(&self) -> BmadLocationClass {
        self.location
    }
}

#[derive(Clone, Debug)]
pub struct BmadSourceSnapshot {
    entries: Vec<BmadSourceEntry>,
    observed_inventory_hash: Sha256Digest,
}

impl BmadSourceSnapshot {
    /// Seals a deterministic, bounded source snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error when aggregate bounds or case-insensitive path
    /// uniqueness are violated.
    pub fn new(mut entries: Vec<BmadSourceEntry>) -> Result<Self, BmadKernelError> {
        let total_bytes = entries
            .iter()
            .try_fold(0_usize, |total, entry| total.checked_add(entry.bytes.len()));
        if entries.len() > MAX_SOURCE_ENTRIES
            || total_bytes.is_none_or(|value| value > MAX_SOURCE_TOTAL_BYTES)
        {
            return Err(BmadKernelErrorCode::SourceLimitExceeded.into());
        }
        entries.sort_by(|left, right| left.path.canonical_cmp(&right.path));
        let mut aliases = BTreeSet::new();
        if entries
            .iter()
            .any(|entry| !aliases.insert(entry.path.case_folded()))
        {
            return Err(BmadKernelErrorCode::SourceAliasConflict.into());
        }
        let observed_inventory_hash = compute_observed_inventory_hash(&entries)?;
        Ok(Self {
            entries,
            observed_inventory_hash,
        })
    }

    #[must_use]
    pub fn entries(&self) -> &[BmadSourceEntry] {
        &self.entries
    }

    #[must_use]
    pub const fn observed_inventory_hash(&self) -> Sha256Digest {
        self.observed_inventory_hash
    }

    fn entry(&self, path: &str) -> Option<&BmadSourceEntry> {
        self.entries.iter().find(|entry| entry.path() == path)
    }
}

#[derive(Serialize)]
struct ObservedInventoryEntry<'a> {
    path: &'a str,
    #[serde(rename = "locationKind")]
    location_kind: &'a str,
    #[serde(rename = "contentHash")]
    content_hash: Sha256Digest,
    #[serde(rename = "byteLength")]
    byte_length: usize,
}

fn compute_observed_inventory_hash(
    entries: &[BmadSourceEntry],
) -> Result<Sha256Digest, BmadKernelError> {
    let inventory = entries
        .iter()
        .filter(|entry| entry.location.contributes_to_final_inventory())
        .map(|entry| ObservedInventoryEntry {
            path: entry.path(),
            location_kind: entry.location.as_str(),
            content_hash: entry.content_hash,
            byte_length: entry.bytes.len(),
        })
        .collect::<Vec<_>>();
    canonical_hash("bmad-final-composite-inventory", 1, &inventory)
        .map_err(|_| BmadKernelErrorCode::FinalInventoryMismatch.into())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadEntrypointKind {
    Direct,
    Inline,
    StepJit,
    ScriptRendered,
    CompatibilityShim,
}

impl BmadEntrypointKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Inline => "inline",
            Self::StepJit => "step_jit",
            Self::ScriptRendered => "script_rendered",
            Self::CompatibilityShim => "compatibility_shim",
        }
    }

    fn parse(value: &str) -> Result<Self, BmadKernelError> {
        match value {
            "direct" => Ok(Self::Direct),
            "inline" => Ok(Self::Inline),
            "step_jit" => Ok(Self::StepJit),
            "script_rendered" => Ok(Self::ScriptRendered),
            "compatibility_shim" => Ok(Self::CompatibilityShim),
            _ => Err(BmadKernelErrorCode::DescriptorInvalid.into()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct BmadLoadedSkill {
    pub module_code: String,
    pub skill_name: String,
    pub entrypoint_kind: BmadEntrypointKind,
    pub capability_enabled: bool,
    pub structurally_eligible: bool,
}

#[derive(Clone, Debug)]
pub struct BmadLoadedPackage {
    pub package_name: String,
    pub package_version: String,
    pub descriptor_hash: Sha256Digest,
    pub observed_inventory_hash: Sha256Digest,
    pub skills: Vec<BmadLoadedSkill>,
}

pub struct BmadPackageLoader;

impl BmadPackageLoader {
    /// Parses and validates a normalized descriptor without executing content.
    ///
    /// # Errors
    ///
    /// Returns a stable kernel error when the generated contract, self-hash,
    /// semantic ledger, final inventory, or managed-resource binding fails.
    pub fn load(
        snapshot: &BmadSourceSnapshot,
        expected_semantic_ledger_hash: Sha256Digest,
    ) -> Result<BmadLoadedPackage, BmadKernelError> {
        verify_semantic_ledger(snapshot, expected_semantic_ledger_hash)?;
        let descriptor_entry = snapshot
            .entry(DESCRIPTOR_PATH)
            .ok_or(BmadKernelErrorCode::DescriptorMissing)?;
        if descriptor_entry.location != BmadLocationClass::ManagedMetadata {
            return Err(BmadKernelErrorCode::DescriptorInvalid.into());
        }

        let descriptor_value: Value = serde_json::from_slice(descriptor_entry.bytes())
            .map_err(|_| BmadKernelErrorCode::DescriptorInvalid)?;
        let exact_descriptor =
            serde_json::from_slice::<generated_contracts::BmadPackageDescriptor>(
                descriptor_entry.bytes(),
            )
            .map_err(|_| BmadKernelErrorCode::DescriptorInvalid)?;
        drop(exact_descriptor);

        let descriptor_hash = value_digest(&descriptor_value, "descriptorHash")?;
        let computed_hash = canonical_hash_without_field(
            "bmad-package-descriptor",
            1,
            &descriptor_value,
            "descriptorHash",
        )
        .map_err(|_| BmadKernelErrorCode::DescriptorInvalid)?;
        if descriptor_hash != computed_hash {
            return Err(BmadKernelErrorCode::DescriptorHashMismatch.into());
        }

        verify_managed_resources(snapshot, &descriptor_value)?;
        let declared_final = value_digest(&descriptor_value, "finalCompositeInventoryHash")?;
        if declared_final != snapshot.observed_inventory_hash {
            return Err(BmadKernelErrorCode::FinalInventoryMismatch.into());
        }

        let package_name = value_string(&descriptor_value, "packageName")?.to_owned();
        let package_version = value_string(&descriptor_value, "packageVersion")?.to_owned();
        let install_profile = value_string(&descriptor_value, "installProfile")?;
        if install_profile != "SapphirusManagedV1" {
            return Err(BmadKernelErrorCode::DescriptorInvalid.into());
        }
        let skills = load_skills(
            &descriptor_value,
            descriptor_entry.source_kind == BmadSourceKind::SealedFoundation,
        )?;
        Ok(BmadLoadedPackage {
            package_name,
            package_version,
            descriptor_hash,
            observed_inventory_hash: snapshot.observed_inventory_hash,
            skills,
        })
    }
}

fn verify_semantic_ledger(
    snapshot: &BmadSourceSnapshot,
    expected_hash: Sha256Digest,
) -> Result<(), BmadKernelError> {
    let ledger = snapshot
        .entry(SEMANTIC_LEDGER_PATH)
        .ok_or(BmadKernelErrorCode::SemanticLedgerMismatch)?;
    if ledger.location != BmadLocationClass::ManagedMetadata || ledger.content_hash != expected_hash
    {
        return Err(BmadKernelErrorCode::SemanticLedgerMismatch.into());
    }
    Ok(())
}

fn verify_managed_resources(
    snapshot: &BmadSourceSnapshot,
    descriptor: &Value,
) -> Result<(), BmadKernelError> {
    let actual = snapshot
        .entries
        .iter()
        .filter(|entry| entry.location == BmadLocationClass::ManagedProjection)
        .map(|entry| {
            (
                entry.path().to_owned(),
                (entry.content_hash, entry.bytes.len() as u64),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let declared = managed_inventory(descriptor)?;
    if actual != declared || declared.is_empty() {
        return Err(BmadKernelErrorCode::ManagedResourceMismatch.into());
    }

    let projections = descriptor
        .get("instructionProjections")
        .and_then(Value::as_array)
        .ok_or(BmadKernelErrorCode::DescriptorInvalid)?;
    let mut projected = BTreeMap::new();
    for projection in projections {
        let managed = projection
            .get("managedInstruction")
            .and_then(Value::as_object)
            .ok_or(BmadKernelErrorCode::DescriptorInvalid)?;
        if managed.get("format").and_then(Value::as_str) != Some("SapphirusManagedV1") {
            return Err(BmadKernelErrorCode::ManagedResourceMismatch.into());
        }
        let path = object_string(managed, "path")?.to_owned();
        let hash = object_digest(managed, "contentHash")?;
        if projected.insert(path, hash).is_some() {
            return Err(BmadKernelErrorCode::ManagedResourceMismatch.into());
        }
    }
    let declared_hashes = declared
        .into_iter()
        .map(|(path, (hash, _))| (path, hash))
        .collect::<BTreeMap<_, _>>();
    if projected != declared_hashes {
        return Err(BmadKernelErrorCode::ManagedResourceMismatch.into());
    }
    Ok(())
}

fn managed_inventory(
    descriptor: &Value,
) -> Result<BTreeMap<String, (Sha256Digest, u64)>, BmadKernelError> {
    let inventory = descriptor
        .get("resourceInventory")
        .and_then(Value::as_array)
        .ok_or(BmadKernelErrorCode::DescriptorInvalid)?;
    let mut managed = BTreeMap::new();
    for entry in inventory {
        let object = entry
            .as_object()
            .ok_or(BmadKernelErrorCode::DescriptorInvalid)?;
        if object.get("locationKind").and_then(Value::as_str) != Some("managed_projection") {
            continue;
        }
        let path = object_string(object, "path")?.to_owned();
        let hash = object_digest(object, "contentHash")?;
        let length = object
            .get("byteLength")
            .and_then(Value::as_u64)
            .ok_or(BmadKernelErrorCode::DescriptorInvalid)?;
        if managed.insert(path, (hash, length)).is_some() {
            return Err(BmadKernelErrorCode::ManagedResourceMismatch.into());
        }
    }
    Ok(managed)
}

fn load_skills(
    descriptor: &Value,
    sealed_foundation: bool,
) -> Result<Vec<BmadLoadedSkill>, BmadKernelError> {
    let values = descriptor
        .get("skills")
        .and_then(Value::as_array)
        .ok_or(BmadKernelErrorCode::DescriptorInvalid)?;
    let mut identities = BTreeSet::new();
    let mut skills = Vec::with_capacity(values.len());
    for value in values {
        let object = value
            .as_object()
            .ok_or(BmadKernelErrorCode::DescriptorInvalid)?;
        let module_code = object_string(object, "moduleCode")?.to_owned();
        let skill_name = object_string(object, "skillName")?.to_owned();
        if !identities.insert((module_code.clone(), skill_name.clone())) {
            return Err(BmadKernelErrorCode::DescriptorInvalid.into());
        }
        let execution = object
            .get("executionProfile")
            .and_then(Value::as_object)
            .ok_or(BmadKernelErrorCode::DescriptorInvalid)?;
        let entrypoint_kind =
            BmadEntrypointKind::parse(object_string(execution, "entrypointKind")?)?;
        let structurally_eligible = sealed_foundation
            && module_code == "core"
            && skill_name == "bmad-help"
            && entrypoint_kind == BmadEntrypointKind::Direct;
        skills.push(BmadLoadedSkill {
            module_code,
            skill_name,
            entrypoint_kind,
            capability_enabled: false,
            structurally_eligible,
        });
    }
    Ok(skills)
}

fn value_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, BmadKernelError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| BmadKernelErrorCode::DescriptorInvalid.into())
}

fn value_digest(value: &Value, field: &str) -> Result<Sha256Digest, BmadKernelError> {
    Sha256Digest::parse(value_string(value, field)?)
        .map_err(|_| BmadKernelErrorCode::DescriptorInvalid.into())
}

fn object_string<'a>(
    value: &'a serde_json::Map<String, Value>,
    field: &str,
) -> Result<&'a str, BmadKernelError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| BmadKernelErrorCode::DescriptorInvalid.into())
}

fn object_digest(
    value: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<Sha256Digest, BmadKernelError> {
    Sha256Digest::parse(object_string(value, field)?)
        .map_err(|_| BmadKernelErrorCode::DescriptorInvalid.into())
}

fn is_safe_source_path(path: &str) -> bool {
    path.nfc().eq(path.chars()) && !path.chars().any(is_directional_control)
}

fn is_directional_control(character: char) -> bool {
    matches!(
        character,
        '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}
