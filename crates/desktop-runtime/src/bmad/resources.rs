//! Deterministic, host-supplied BMAD package resource closure resolution.
//!
//! This module intentionally accepts bytes already read by the host. It never opens a path,
//! follows a link, or reaches a network location: package-root canonicalization is a pure
//! operation over the pinned source identity and its immutable member inventory.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use thiserror::Error;

use crate::{sha256_bytes, RelativeWorkspacePath, Sha256Digest};

use super::kernel_error::{BmadKernelError, BmadKernelErrorCode};

/// One content-addressed package resource.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadResource {
    path: RelativeWorkspacePath,
    content_hash: Sha256Digest,
}

impl BmadResource {
    /// Creates a resource identity without reading from the filesystem.
    ///
    /// # Errors
    ///
    /// Returns `SourcePathInvalid` for non-relative or non-canonical paths.
    pub fn new(
        path: impl Into<String>,
        content_hash: Sha256Digest,
    ) -> Result<Self, BmadKernelError> {
        let path = RelativeWorkspacePath::new(path.into())
            .map_err(|_| BmadKernelErrorCode::SourcePathInvalid)?;
        Ok(Self { path, content_hash })
    }

    #[must_use]
    pub fn path(&self) -> &str {
        self.path.as_str()
    }

    #[must_use]
    pub const fn content_hash(&self) -> Sha256Digest {
        self.content_hash
    }
}

/// Deterministic, case-collision-free resource set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadResourceSet {
    resources: Vec<BmadResource>,
}

impl BmadResourceSet {
    /// Seals resources after sorting and case-insensitive collision checks.
    ///
    /// # Errors
    ///
    /// Returns `SourceAliasConflict` when two package paths collide on a
    /// case-insensitive host.
    pub fn new(mut resources: Vec<BmadResource>) -> Result<Self, BmadKernelError> {
        resources.sort_by(|left, right| left.path().cmp(right.path()));
        let mut seen = BTreeSet::new();
        if resources
            .iter()
            .any(|resource| !seen.insert(resource.path().to_lowercase()))
        {
            return Err(BmadKernelErrorCode::SourceAliasConflict.into());
        }
        Ok(Self { resources })
    }

    #[must_use]
    pub fn resources(&self) -> &[BmadResource] {
        &self.resources
    }
}

/// A source and package identity that pins every member considered by a resolver.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadPackageRoot {
    source_id: String,
    package_identity: String,
}

impl BmadPackageRoot {
    /// Constructs a package root from stable source and package identities.
    ///
    /// # Errors
    ///
    /// Returns [`BmadResourceResolverError::InvalidPackageRoot`] for empty or
    /// control-character-containing identities.
    pub fn new(
        source_id: impl Into<String>,
        package_identity: impl Into<String>,
    ) -> Result<Self, BmadResourceResolverError> {
        let source_id = source_id.into();
        let package_identity = package_identity.into();
        if !is_safe_identity(&source_id) || !is_safe_identity(&package_identity) {
            return Err(BmadResourceResolverError::InvalidPackageRoot);
        }
        Ok(Self {
            source_id,
            package_identity,
        })
    }

    #[must_use]
    pub fn source_id(&self) -> &str {
        &self.source_id
    }

    #[must_use]
    pub fn package_identity(&self) -> &str {
        &self.package_identity
    }
}

/// One regular-file member supplied by a host after pinned-package verification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadResourceInput {
    path: RelativeWorkspacePath,
    bytes: Vec<u8>,
}

impl BmadResourceInput {
    /// Creates an in-package regular-file input. Symlinks must never be materialized here.
    ///
    /// # Errors
    ///
    /// Returns [`BmadResourceResolverError::InvalidResourcePath`] when `path`
    /// is not a canonical, package-relative path.
    pub fn file(
        path: impl Into<String>,
        bytes: impl AsRef<[u8]>,
    ) -> Result<Self, BmadResourceResolverError> {
        let path = RelativeWorkspacePath::new(path.into())
            .map_err(|_| BmadResourceResolverError::InvalidResourcePath)?;
        Ok(Self {
            path,
            bytes: bytes.as_ref().to_vec(),
        })
    }
}

/// Whether a declared resource edge is required for activation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BmadResourceRequirement {
    /// A missing target prevents a skill from activating.
    Mandatory,
    /// A missing target is retained as evidence but does not prevent activation.
    Optional,
}

/// The source syntax that declared a resource edge.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BmadResourceReferenceKind {
    Markdown,
    Frontmatter,
    Toml,
    Yaml,
    Csv,
    Script,
    Template,
}

/// The typed treatment of one exact, source-declared resource edge.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BmadResourceEdgeDispositionKind {
    /// A local package member required for activation.
    PackageResourceMandatory,
    /// A local package member whose absence is explicit non-blocking evidence.
    PackageResourceOptional,
    /// A runtime artifact produced outside the immutable source package.
    GeneratedOutput {
        /// Stable identity of the generated artifact, independent of filesystem location.
        output_identity: String,
    },
    /// A remote authority that the local package is forbidden to resolve.
    ExternalAuthorityRejected {
        /// Stable identity of the rejected authority.
        authority_identity: String,
        /// Source-governance task that must remove or locally snapshot the authority.
        governance_owner: String,
    },
}

/// A disposition sealed to one exact source package member and literal.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct BmadResourceEdgeDisposition {
    source_id: String,
    package_identity: String,
    declared_by: RelativeWorkspacePath,
    declaring_member_hash: Sha256Digest,
    literal: String,
    kind: BmadResourceEdgeDispositionKind,
}

impl BmadResourceEdgeDisposition {
    /// Seals one typed edge decision to its immutable source identity.
    ///
    /// # Errors
    ///
    /// Returns [`BmadResourceResolverError::InvalidEdgeDisposition`] for an
    /// unsafe identity, member path, literal, or disposition payload.
    pub fn new(
        source_id: impl Into<String>,
        package_identity: impl Into<String>,
        declared_by: impl Into<String>,
        declaring_member_hash: Sha256Digest,
        literal: impl Into<String>,
        kind: BmadResourceEdgeDispositionKind,
    ) -> Result<Self, BmadResourceResolverError> {
        let source_id = source_id.into();
        let package_identity = package_identity.into();
        let declared_by = RelativeWorkspacePath::new(declared_by.into())
            .map_err(|_| BmadResourceResolverError::InvalidEdgeDisposition)?;
        let literal = literal.into();
        let payload_is_safe = match &kind {
            BmadResourceEdgeDispositionKind::PackageResourceMandatory
            | BmadResourceEdgeDispositionKind::PackageResourceOptional => true,
            BmadResourceEdgeDispositionKind::GeneratedOutput { output_identity } => {
                is_safe_identity(output_identity)
            }
            BmadResourceEdgeDispositionKind::ExternalAuthorityRejected {
                authority_identity,
                governance_owner,
            } => is_safe_identity(authority_identity) && is_safe_identity(governance_owner),
        };
        if !is_safe_identity(&source_id)
            || !is_safe_identity(&package_identity)
            || literal.is_empty()
            || literal.len() > 2_048
            || literal.chars().any(char::is_control)
            || !payload_is_safe
        {
            return Err(BmadResourceResolverError::InvalidEdgeDisposition);
        }
        Ok(Self {
            source_id,
            package_identity,
            declared_by,
            declaring_member_hash,
            literal,
            kind,
        })
    }

    #[must_use]
    pub fn source_id(&self) -> &str {
        &self.source_id
    }

    #[must_use]
    pub fn package_identity(&self) -> &str {
        &self.package_identity
    }

    #[must_use]
    pub fn declared_by(&self) -> &str {
        self.declared_by.as_str()
    }

    #[must_use]
    pub const fn declaring_member_hash(&self) -> Sha256Digest {
        self.declaring_member_hash
    }

    #[must_use]
    pub fn literal(&self) -> &str {
        &self.literal
    }

    #[must_use]
    pub const fn kind(&self) -> &BmadResourceEdgeDispositionKind {
        &self.kind
    }
}

/// Evidence for why a resource entered a resolved closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadResourceProvenance {
    declared_by: Option<RelativeWorkspacePath>,
    reference_kind: Option<BmadResourceReferenceKind>,
    requirement: BmadResourceRequirement,
}

impl BmadResourceProvenance {
    #[must_use]
    pub fn declared_by(&self) -> Option<&str> {
        self.declared_by.as_ref().map(RelativeWorkspacePath::as_str)
    }

    #[must_use]
    pub const fn reference_kind(&self) -> Option<BmadResourceReferenceKind> {
        self.reference_kind
    }

    #[must_use]
    pub const fn requirement(&self) -> BmadResourceRequirement {
        self.requirement
    }
}

/// A canonical resource identity returned by [`BmadResourceResolver`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadResolvedResource {
    path: RelativeWorkspacePath,
    source_id: String,
    package_identity: String,
    content_hash: Sha256Digest,
    media_type: &'static str,
    provenance: BmadResourceProvenance,
}

impl BmadResolvedResource {
    #[must_use]
    pub fn path(&self) -> &str {
        self.path.as_str()
    }

    #[must_use]
    pub fn source_id(&self) -> &str {
        &self.source_id
    }

    #[must_use]
    pub fn package_identity(&self) -> &str {
        &self.package_identity
    }

    #[must_use]
    pub const fn content_hash(&self) -> Sha256Digest {
        self.content_hash
    }

    #[must_use]
    pub const fn media_type(&self) -> &'static str {
        self.media_type
    }

    #[must_use]
    pub fn provenance(&self) -> &BmadResourceProvenance {
        &self.provenance
    }
}

/// The directed, per-skill closure evidence used as an activation gate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadResourceClosure {
    skill_root: RelativeWorkspacePath,
    resources: Vec<BmadResolvedResource>,
    missing: Vec<String>,
    optional_missing: Vec<String>,
    cycles: Vec<Vec<String>>,
    invalid_references: Vec<String>,
    edge_dispositions: Vec<BmadResourceEdgeDisposition>,
    activation_blockers: Vec<BmadResourceEdgeDisposition>,
    required_mandatory_edges: usize,
    closed_mandatory_edges: usize,
}

impl BmadResourceClosure {
    #[must_use]
    pub fn skill_root(&self) -> &str {
        self.skill_root.as_str()
    }

    #[must_use]
    pub fn resources(&self) -> &[BmadResolvedResource] {
        &self.resources
    }

    #[must_use]
    pub fn missing(&self) -> &[String] {
        &self.missing
    }

    #[must_use]
    pub fn optional_missing(&self) -> &[String] {
        &self.optional_missing
    }

    #[must_use]
    pub fn cycles(&self) -> &[Vec<String>] {
        &self.cycles
    }

    #[must_use]
    pub fn invalid_references(&self) -> &[String] {
        &self.invalid_references
    }

    #[must_use]
    pub fn edge_dispositions(&self) -> &[BmadResourceEdgeDisposition] {
        &self.edge_dispositions
    }

    #[must_use]
    pub fn activation_blockers(&self) -> &[BmadResourceEdgeDisposition] {
        &self.activation_blockers
    }

    #[must_use]
    pub fn activation_allowed(&self) -> bool {
        self.closed_mandatory_edges == self.required_mandatory_edges
            && self.missing.is_empty()
            && self.cycles.is_empty()
            && self.invalid_references.is_empty()
            && self.activation_blockers.is_empty()
    }

    #[must_use]
    pub fn required_mandatory_edges(&self) -> usize {
        self.required_mandatory_edges
    }

    #[must_use]
    pub fn closed_mandatory_edges(&self) -> usize {
        self.closed_mandatory_edges
    }

    #[must_use]
    #[allow(clippy::cast_precision_loss)] // Inventory cardinalities are bounded by the loaded package.
    pub fn closure_percent(&self) -> f64 {
        if self.required_mandatory_edges == 0 {
            100.0
        } else {
            (self.closed_mandatory_edges as f64 * 100.0) / self.required_mandatory_edges as f64
        }
    }
}

/// Fail-closed result for resource resolution.
#[derive(Debug, Error)]
pub enum BmadResourceResolverError {
    #[error("the pinned package root identity is invalid")]
    InvalidPackageRoot,
    #[error("the package resource path is invalid")]
    InvalidResourcePath,
    #[error("the package resource inventory contains a duplicate or case-colliding path")]
    CaseCollision,
    #[error("the sealed resource edge disposition is invalid or has drifted")]
    InvalidEdgeDisposition,
    #[error("the requested skill root is not a member of the pinned package")]
    UnknownSkillRoot,
    #[error("mandatory resource closure is incomplete")]
    IncompleteMandatoryClosure { closure: Box<BmadResourceClosure> },
}

impl BmadResourceResolverError {
    #[must_use]
    pub const fn closure(&self) -> Option<&BmadResourceClosure> {
        match self {
            Self::IncompleteMandatoryClosure { closure } => Some(closure),
            Self::InvalidPackageRoot
            | Self::InvalidResourcePath
            | Self::CaseCollision
            | Self::InvalidEdgeDisposition
            | Self::UnknownSkillRoot => None,
        }
    }
}

#[derive(Clone, Debug)]
struct CachedResource {
    references: Arc<Vec<ParsedReference>>,
    content_hash: Sha256Digest,
    media_type: &'static str,
}

/// Resolves every declared resource without granting filesystem or network authority.
#[derive(Clone, Debug)]
pub struct BmadResourceResolver {
    package_root: BmadPackageRoot,
    resources: BTreeMap<RelativeWorkspacePath, CachedResource>,
    content_cache: BTreeMap<Sha256Digest, Arc<[u8]>>,
    parsed_reference_cache: BTreeMap<(Sha256Digest, String), Arc<Vec<ParsedReference>>>,
    sealed_edge_dispositions:
        BTreeMap<(RelativeWorkspacePath, Sha256Digest, String), BmadResourceEdgeDisposition>,
}

impl BmadResourceResolver {
    /// Seals a host-supplied inventory under one pinned source/package identity.
    ///
    /// # Errors
    ///
    /// Returns [`BmadResourceResolverError::CaseCollision`] when paths collide
    /// under case-folding. This rejects platform-dependent resource resolution.
    pub fn new(
        package_root: BmadPackageRoot,
        inputs: Vec<BmadResourceInput>,
    ) -> Result<Self, BmadResourceResolverError> {
        Self::with_edge_dispositions(package_root, inputs, Vec::new())
    }

    /// Seals a host-supplied inventory and exact generated-output or rejected-authority edges.
    ///
    /// # Errors
    ///
    /// Returns [`BmadResourceResolverError::InvalidEdgeDisposition`] when a
    /// disposition does not match the exact source/package/member/hash/literal
    /// tuple or contradicts the source syntax.
    pub fn with_edge_dispositions(
        package_root: BmadPackageRoot,
        inputs: Vec<BmadResourceInput>,
        edge_dispositions: Vec<BmadResourceEdgeDisposition>,
    ) -> Result<Self, BmadResourceResolverError> {
        let mut resources = BTreeMap::new();
        let mut case_folded = BTreeSet::new();
        let mut content_cache = BTreeMap::<Sha256Digest, Arc<[u8]>>::new();
        let mut parsed_reference_cache =
            BTreeMap::<(Sha256Digest, String), Arc<Vec<ParsedReference>>>::new();
        for input in inputs {
            if !case_folded.insert(input.path.case_folded()) || resources.contains_key(&input.path)
            {
                return Err(BmadResourceResolverError::CaseCollision);
            }
            let content_hash = sha256_bytes(&input.bytes);
            let media_type = media_type_for_path(input.path.as_str());
            content_cache
                .entry(content_hash)
                .or_insert_with(|| Arc::<[u8]>::from(input.bytes.clone()));
            let references = parsed_reference_cache
                .entry((content_hash, parser_cache_key(input.path.as_str())))
                .or_insert_with(|| Arc::new(parse_references(input.path.as_str(), &input.bytes)))
                .clone();
            resources.insert(
                input.path,
                CachedResource {
                    references,
                    content_hash,
                    media_type,
                },
            );
        }
        let sealed_edge_dispositions =
            validate_edge_dispositions(&package_root, &resources, edge_dispositions)?;
        Ok(Self {
            package_root,
            resources,
            content_cache,
            parsed_reference_cache,
            sealed_edge_dispositions,
        })
    }

    /// Resolves one skill into a mandatory-closure activation decision and evidence set.
    ///
    /// # Errors
    ///
    /// Returns [`BmadResourceResolverError::IncompleteMandatoryClosure`] with
    /// deterministic diagnostics if an edge is missing, cyclic, or unsafe.
    pub fn resolve_skill(
        &self,
        skill_root: impl Into<String>,
    ) -> Result<BmadResourceClosure, BmadResourceResolverError> {
        let skill_root = RelativeWorkspacePath::new(skill_root.into())
            .map_err(|_| BmadResourceResolverError::UnknownSkillRoot)?;
        if !self.resources.contains_key(&skill_root) {
            return Err(BmadResourceResolverError::UnknownSkillRoot);
        }

        let mut state = ResolutionState::default();
        state
            .required_edges
            .insert(format!("root:{}", skill_root.as_str()));
        state
            .closed_edges
            .insert(format!("root:{}", skill_root.as_str()));
        self.visit(
            &skill_root,
            &BmadResourceProvenance {
                declared_by: None,
                reference_kind: None,
                requirement: BmadResourceRequirement::Mandatory,
            },
            &mut state,
        );
        let mut resources = state
            .resolved
            .into_iter()
            .map(|(path, provenance)| self.resolved_resource(path, &provenance))
            .collect::<Vec<_>>();
        resources.sort_by(|left, right| left.path.canonical_cmp(&right.path));
        state.missing.sort();
        state.optional_missing.sort();
        state.invalid_references.sort();
        state.cycles.sort();
        state.edge_dispositions.sort();
        state.edge_dispositions.dedup();
        state.activation_blockers.sort();
        state.activation_blockers.dedup();
        let closure = BmadResourceClosure {
            skill_root,
            resources,
            missing: state.missing,
            optional_missing: state.optional_missing,
            cycles: state.cycles,
            invalid_references: state.invalid_references,
            edge_dispositions: state.edge_dispositions,
            activation_blockers: state.activation_blockers,
            required_mandatory_edges: state.required_edges.len(),
            closed_mandatory_edges: state.closed_edges.len(),
        };
        if closure.activation_allowed() {
            Ok(closure)
        } else {
            Err(BmadResourceResolverError::IncompleteMandatoryClosure {
                closure: Box::new(closure),
            })
        }
    }

    /// Number of immutable byte-content entries, independent of resource identity.
    #[must_use]
    pub fn cached_content_count(&self) -> usize {
        self.content_cache.len()
    }

    /// Number of parsed-reference cache entries, partitioned by parser kind.
    #[must_use]
    pub fn cached_parser_entry_count(&self) -> usize {
        self.parsed_reference_cache.len()
    }

    fn visit(
        &self,
        path: &RelativeWorkspacePath,
        provenance: &BmadResourceProvenance,
        state: &mut ResolutionState,
    ) {
        match state.visit.get(path) {
            Some(VisitState::Visited) => return,
            Some(VisitState::Visiting) => {
                if let Some(index) = state.stack.iter().position(|item| item == path) {
                    let cycle = state.stack[index..]
                        .iter()
                        .map(|item| item.as_str().to_owned())
                        .collect::<Vec<_>>();
                    let key = cycle.join("\u{0}");
                    if state.cycle_keys.insert(key) {
                        state.cycles.push(cycle);
                    }
                }
                return;
            }
            None => {}
        }
        state.visit.insert(path.clone(), VisitState::Visiting);
        state
            .resolved
            .entry(path.clone())
            .or_insert_with(|| provenance.clone());
        state.stack.push(path.clone());

        let Some(resource) = self.resources.get(path) else {
            state.stack.pop();
            state.visit.insert(path.clone(), VisitState::Visited);
            return;
        };
        for reference in resource.references.iter() {
            let disposition_key = (
                path.clone(),
                resource.content_hash,
                reference.literal.clone(),
            );
            let disposition = self
                .sealed_edge_dispositions
                .get(&disposition_key)
                .cloned()
                .unwrap_or_else(|| BmadResourceEdgeDisposition {
                    source_id: self.package_root.source_id.clone(),
                    package_identity: self.package_root.package_identity.clone(),
                    declared_by: path.clone(),
                    declaring_member_hash: resource.content_hash,
                    literal: reference.literal.clone(),
                    kind: match reference.requirement {
                        BmadResourceRequirement::Mandatory => {
                            BmadResourceEdgeDispositionKind::PackageResourceMandatory
                        }
                        BmadResourceRequirement::Optional => {
                            BmadResourceEdgeDispositionKind::PackageResourceOptional
                        }
                    },
                });
            state.edge_dispositions.push(disposition.clone());
            let requirement = match disposition.kind() {
                BmadResourceEdgeDispositionKind::PackageResourceMandatory => {
                    BmadResourceRequirement::Mandatory
                }
                BmadResourceEdgeDispositionKind::PackageResourceOptional => {
                    BmadResourceRequirement::Optional
                }
                BmadResourceEdgeDispositionKind::GeneratedOutput { .. } => continue,
                BmadResourceEdgeDispositionKind::ExternalAuthorityRejected { .. } => {
                    state.activation_blockers.push(disposition);
                    continue;
                }
            };
            let edge_identity = format!("{}\u{0}{}", path.as_str(), reference.literal);
            if requirement == BmadResourceRequirement::Mandatory {
                state.required_edges.insert(edge_identity.clone());
            }
            let Ok(target) = canonicalize_reference(path, &reference.literal) else {
                state.invalid_references.push(reference.literal.clone());
                continue;
            };
            if !self.resources.contains_key(&target) {
                match requirement {
                    BmadResourceRequirement::Mandatory => state.missing.push(target.to_string()),
                    BmadResourceRequirement::Optional => {
                        state.optional_missing.push(target.to_string());
                    }
                }
                continue;
            }
            if requirement == BmadResourceRequirement::Mandatory {
                state.closed_edges.insert(edge_identity);
            }
            let child_provenance = BmadResourceProvenance {
                declared_by: Some(path.clone()),
                reference_kind: Some(reference.kind),
                requirement,
            };
            self.visit(&target, &child_provenance, state);
        }
        state.stack.pop();
        state.visit.insert(path.clone(), VisitState::Visited);
    }

    fn resolved_resource(
        &self,
        path: RelativeWorkspacePath,
        provenance: &BmadResourceProvenance,
    ) -> BmadResolvedResource {
        let Some(resource) = self.resources.get(&path) else {
            unreachable!("resolved paths are members of the sealed inventory");
        };
        BmadResolvedResource {
            path,
            source_id: self.package_root.source_id.clone(),
            package_identity: self.package_root.package_identity.clone(),
            content_hash: resource.content_hash,
            media_type: resource.media_type,
            provenance: provenance.clone(),
        }
    }
}

#[derive(Default)]
struct ResolutionState {
    visit: BTreeMap<RelativeWorkspacePath, VisitState>,
    stack: Vec<RelativeWorkspacePath>,
    resolved: BTreeMap<RelativeWorkspacePath, BmadResourceProvenance>,
    missing: Vec<String>,
    optional_missing: Vec<String>,
    cycles: Vec<Vec<String>>,
    cycle_keys: BTreeSet<String>,
    invalid_references: Vec<String>,
    edge_dispositions: Vec<BmadResourceEdgeDisposition>,
    activation_blockers: Vec<BmadResourceEdgeDisposition>,
    required_edges: BTreeSet<String>,
    closed_edges: BTreeSet<String>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum VisitState {
    Visiting,
    Visited,
}

#[derive(Clone, Debug)]
struct ParsedReference {
    literal: String,
    kind: BmadResourceReferenceKind,
    requirement: BmadResourceRequirement,
}

fn validate_edge_dispositions(
    package_root: &BmadPackageRoot,
    resources: &BTreeMap<RelativeWorkspacePath, CachedResource>,
    edge_dispositions: Vec<BmadResourceEdgeDisposition>,
) -> Result<
    BTreeMap<(RelativeWorkspacePath, Sha256Digest, String), BmadResourceEdgeDisposition>,
    BmadResourceResolverError,
> {
    let mut sealed = BTreeMap::new();
    for disposition in edge_dispositions {
        if disposition.source_id != package_root.source_id
            || disposition.package_identity != package_root.package_identity
        {
            return Err(BmadResourceResolverError::InvalidEdgeDisposition);
        }
        let resource = resources
            .get(&disposition.declared_by)
            .ok_or(BmadResourceResolverError::InvalidEdgeDisposition)?;
        if resource.content_hash != disposition.declaring_member_hash {
            return Err(BmadResourceResolverError::InvalidEdgeDisposition);
        }
        let matching_references = resource
            .references
            .iter()
            .filter(|reference| reference.literal == disposition.literal)
            .collect::<Vec<_>>();
        if matching_references.is_empty() {
            return Err(BmadResourceResolverError::InvalidEdgeDisposition);
        }
        match &disposition.kind {
            BmadResourceEdgeDispositionKind::PackageResourceMandatory => {
                if !matching_references
                    .iter()
                    .any(|reference| reference.requirement == BmadResourceRequirement::Mandatory)
                {
                    return Err(BmadResourceResolverError::InvalidEdgeDisposition);
                }
            }
            BmadResourceEdgeDispositionKind::PackageResourceOptional => {
                if !matching_references
                    .iter()
                    .any(|reference| reference.requirement == BmadResourceRequirement::Optional)
                {
                    return Err(BmadResourceResolverError::InvalidEdgeDisposition);
                }
            }
            BmadResourceEdgeDispositionKind::GeneratedOutput { .. } => {
                let target = canonicalize_reference(&disposition.declared_by, &disposition.literal)
                    .map_err(|()| BmadResourceResolverError::InvalidEdgeDisposition)?;
                if resources.contains_key(&target) {
                    return Err(BmadResourceResolverError::InvalidEdgeDisposition);
                }
            }
            BmadResourceEdgeDispositionKind::ExternalAuthorityRejected { .. } => {
                if !(disposition.literal.starts_with("http:")
                    || disposition.literal.starts_with("https:"))
                    || canonicalize_reference(&disposition.declared_by, &disposition.literal)
                        .is_ok()
                {
                    return Err(BmadResourceResolverError::InvalidEdgeDisposition);
                }
            }
        }
        let key = (
            disposition.declared_by.clone(),
            disposition.declaring_member_hash,
            disposition.literal.clone(),
        );
        if sealed.insert(key, disposition).is_some() {
            return Err(BmadResourceResolverError::InvalidEdgeDisposition);
        }
    }
    Ok(sealed)
}

fn parse_references(path: &str, bytes: &[u8]) -> Vec<ParsedReference> {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Vec::new();
    };
    let mut references = Vec::new();
    let mut seen = BTreeSet::new();
    let mut in_frontmatter = false;
    for line in text.lines() {
        if is_frontmatter_path(path) && line.trim() == "---" {
            in_frontmatter = !in_frontmatter;
            continue;
        }
        let lower = line.to_ascii_lowercase();
        let requirement = if is_explicit_optional_declaration(&lower) {
            BmadResourceRequirement::Optional
        } else {
            BmadResourceRequirement::Mandatory
        };
        for literal in markdown_targets(line) {
            push_reference(
                &mut references,
                &mut seen,
                &literal,
                BmadResourceReferenceKind::Markdown,
                requirement,
            );
        }
        let kind = reference_kind_for_line(path, &lower, in_frontmatter);
        if let Some(kind) = kind {
            for literal in quoted_values(line) {
                push_reference(&mut references, &mut seen, &literal, kind, requirement);
            }
            for literal in bare_path_values(line) {
                push_reference(&mut references, &mut seen, &literal, kind, requirement);
            }
        }
    }
    references
}

fn is_explicit_optional_declaration(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("optional:")
        || trimmed.starts_with("optional =")
        || trimmed.starts_with("optional=")
}

fn push_reference(
    references: &mut Vec<ParsedReference>,
    seen: &mut BTreeSet<(String, BmadResourceReferenceKind, BmadResourceRequirement)>,
    literal: &str,
    kind: BmadResourceReferenceKind,
    requirement: BmadResourceRequirement,
) {
    let literal = literal.trim().trim_matches('`').to_owned();
    if literal.is_empty() || literal.starts_with('#') || !looks_like_resource_reference(&literal) {
        return;
    }
    if seen.insert((literal.clone(), kind, requirement)) {
        references.push(ParsedReference {
            literal,
            kind,
            requirement,
        });
    }
}

fn markdown_targets(line: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let mut remainder = line;
    while let Some(start) = remainder.find("](") {
        let after_start = &remainder[start + 2..];
        let Some(end) = after_start.find(')') else {
            break;
        };
        targets.push(
            after_start[..end]
                .trim()
                .trim_matches('<')
                .trim_matches('>')
                .to_owned(),
        );
        remainder = &after_start[end + 1..];
    }
    targets
}

fn quoted_values(line: &str) -> Vec<String> {
    let mut values = Vec::new();
    for quote in ['\'', '"'] {
        let mut remainder = line;
        while let Some(start) = remainder.find(quote) {
            let after_start = &remainder[start + quote.len_utf8()..];
            let Some(end) = after_start.find(quote) else {
                break;
            };
            values.push(after_start[..end].to_owned());
            remainder = &after_start[end + quote.len_utf8()..];
        }
    }
    values
}

fn bare_path_values(line: &str) -> Vec<String> {
    line.split(|character: char| character.is_whitespace() || matches!(character, ',' | '=' | ':'))
        .map(|value| {
            value.trim_matches(|character| matches!(character, '[' | ']' | '{' | '}' | '(' | ')'))
        })
        .filter(|value| looks_like_resource_reference(value))
        .map(str::to_owned)
        .collect()
}

fn reference_kind_for_line(
    path: &str,
    lower_line: &str,
    in_frontmatter: bool,
) -> Option<BmadResourceReferenceKind> {
    if lower_line.trim_start().starts_with("optional") {
        return Some(BmadResourceReferenceKind::Yaml);
    }
    if lower_line.contains("{{") || lower_line.contains("include") {
        return Some(BmadResourceReferenceKind::Template);
    }
    if lower_line.contains("import ")
        || lower_line.contains("require(")
        || lower_line.starts_with("source ")
    {
        return Some(BmadResourceReferenceKind::Script);
    }
    if has_extension(path, &["csv"]) {
        return Some(BmadResourceReferenceKind::Csv);
    }
    if has_extension(path, &["toml"]) {
        return Some(BmadResourceReferenceKind::Toml);
    }
    if has_extension(path, &["yaml", "yml"]) {
        return Some(BmadResourceReferenceKind::Yaml);
    }
    if in_frontmatter && lower_line.contains(':') {
        return Some(BmadResourceReferenceKind::Frontmatter);
    }
    None
}

fn is_frontmatter_path(path: &str) -> bool {
    has_extension(path, &["md", "mdx", "html"])
}

fn looks_like_resource_reference(value: &str) -> bool {
    let value = value.trim();
    value.starts_with("./")
        || value.starts_with("../")
        || value.starts_with('/')
        || value.starts_with("\\\\")
        || is_windows_drive_path(value)
        || value.starts_with("file:")
        || value.starts_with("http:")
        || value.starts_with("https:")
        || value.contains('/') && extension_is_resource(value)
}

fn is_windows_drive_path(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\')
}

fn extension_is_resource(value: &str) -> bool {
    let value = value.split(['#', '?']).next().unwrap_or(value);
    [
        ".md",
        ".mdx",
        ".toml",
        ".yaml",
        ".yml",
        ".json",
        ".csv",
        ".ts",
        ".js",
        ".mjs",
        ".cjs",
        ".py",
        ".html",
        ".svg",
        ".txt",
        ".xml",
        ".mustache",
        ".hbs",
    ]
    .iter()
    .any(|extension| value.ends_with(extension))
}

fn canonicalize_reference(
    source: &RelativeWorkspacePath,
    literal: &str,
) -> Result<RelativeWorkspacePath, ()> {
    let literal = literal
        .split('#')
        .next()
        .unwrap_or(literal)
        .split('?')
        .next()
        .unwrap_or(literal);
    if literal.is_empty()
        || literal.starts_with('/')
        || literal.starts_with('\\')
        || literal.contains('\\')
        || literal.contains(':')
        || literal.chars().any(char::is_control)
    {
        return Err(());
    }
    let mut components = source
        .as_str()
        .split('/')
        .map(str::to_owned)
        .collect::<Vec<_>>();
    components.pop();
    for component in literal.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                if components.pop().is_none() {
                    return Err(());
                }
            }
            value => components.push(value.to_owned()),
        }
    }
    RelativeWorkspacePath::new(components.join("/")).map_err(|_| ())
}

fn media_type_for_path(path: &str) -> &'static str {
    if has_extension(path, &["md", "mdx"]) {
        "text/markdown"
    } else if has_extension(path, &["toml"]) {
        "application/toml"
    } else if has_extension(path, &["yaml", "yml"]) {
        "application/yaml"
    } else if has_extension(path, &["csv"]) {
        "text/csv"
    } else if has_extension(path, &["js", "mjs", "cjs"]) {
        "text/javascript"
    } else if has_extension(path, &["ts"]) {
        "text/typescript"
    } else if has_extension(path, &["html", "hbs", "mustache"]) {
        "text/html"
    } else if has_extension(path, &["svg"]) {
        "image/svg+xml"
    } else {
        "application/octet-stream"
    }
}

fn parser_cache_key(path: &str) -> String {
    let extension = std::path::Path::new(path)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .map_or_else(|| "none".to_owned(), str::to_ascii_lowercase);
    format!(
        "extension:{extension};frontmatter:{}",
        is_frontmatter_path(path)
    )
}

fn has_extension(path: &str, candidates: &[&str]) -> bool {
    std::path::Path::new(path)
        .extension()
        .is_some_and(|extension| {
            candidates
                .iter()
                .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        })
}

fn is_safe_identity(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn resource_path_order_and_collision_matrix_is_deterministic() {
        let hash = sha256_bytes(b"same");
        for path in ["/absolute", "../escape", "a\\b"] {
            assert!(BmadResource::new(path, hash).is_err(), "{path} must fail");
        }
        let ordered = BmadResourceSet::new(vec![
            BmadResource::new("z/last.md", hash).expect("last"),
            BmadResource::new("a/first.md", hash).expect("first"),
        ])
        .expect("ordered");
        assert_eq!(
            ordered
                .resources()
                .iter()
                .map(BmadResource::path)
                .collect::<Vec<_>>(),
            ["a/first.md", "z/last.md"]
        );
        let result = BmadResourceSet::new(vec![
            BmadResource::new("templates/Brief.md", hash).expect("first"),
            BmadResource::new("templates/brief.md", hash).expect("second"),
        ]);
        assert!(result.is_err());
    }
}
