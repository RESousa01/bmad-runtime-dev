//! Capability dependency closure (ENG-09).
//!
//! Without this module, "does this skill actually have everything it needs to
//! run" has no single answer: Help, the Builder, and the runtime would each
//! grow their own ad hoc walk over source references, and could disagree
//! about whether a capability is activatable. This module is the one graph
//! and the one diagnostic vocabulary all three read.
//!
//! Two properties matter more than the rest:
//!
//! - Alias resolution is a distinct phase that runs before any edge is
//!   recorded. A capability referenced two different ways (an alias and its
//!   canonical id) must collapse to one graph node, or the closure would
//!   silently double-count it and callers would disagree about identity.
//! - Only the REQUIRED subgraph must be acyclic. A skill that optionally
//!   references something that (optionally) references it back is a
//!   legitimate authoring pattern -- forbidding it would make "optional"
//!   meaningless. A cycle is only fatal when every edge on it is required.
//!
//! Persona activation is modeled as a graph edge to a persona node, never as
//! copied prompt text: this module carries ids, hashes, and edge kinds only.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use thiserror::Error;

use crate::Sha256Digest;

const MAX_NODES: usize = 10_000;
const MAX_EDGES: usize = 40_000;
const MAX_ALIASES: usize = 10_000;
const MAX_ID_BYTES: usize = 256;
const MAX_VERSION_BYTES: usize = 64;
const MAX_ALIAS_CHAIN: usize = 8;

/// The capability surface a dependency node stands for.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BmadDependencyNodeKind {
    Skill,
    Tool,
    Script,
    Persona,
    Artifact,
    Descriptor,
}

/// Whether a declared edge blocks activation, degrades gracefully, or only
/// applies when a named runtime condition holds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BmadDependencyEdgeKind {
    Required,
    Optional,
    /// Included only when its condition is in the caller-supplied active set.
    /// Excluded from cycle detection even when active: a condition is a
    /// runtime fact, not a structural guarantee the closure can lean on.
    Conditional {
        condition: String,
    },
}

/// One canonical-id node. Carries identity, kind, an optional declared
/// version, and a content hash -- never free-form prose. A persona is a node
/// like any other; its prompt text lives outside this module entirely.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadDependencyNode {
    id: String,
    kind: BmadDependencyNodeKind,
    version: Option<String>,
    content_hash: Sha256Digest,
}

impl BmadDependencyNode {
    /// Creates one canonical-id node.
    ///
    /// # Errors
    ///
    /// Returns [`BmadDependencyError::InvalidId`] for an empty, oversized, or
    /// control-character-containing id or version.
    pub fn new(
        id: impl Into<String>,
        kind: BmadDependencyNodeKind,
        version: Option<String>,
        content_hash: Sha256Digest,
    ) -> Result<Self, BmadDependencyError> {
        let id = id.into();
        if !valid_id(&id) || !valid_version(version.as_deref()) {
            return Err(BmadDependencyError::InvalidId);
        }
        Ok(Self {
            id,
            kind,
            version,
            content_hash,
        })
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub const fn kind(&self) -> BmadDependencyNodeKind {
        self.kind
    }

    #[must_use]
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    #[must_use]
    pub const fn content_hash(&self) -> Sha256Digest {
        self.content_hash
    }
}

/// One declared edge. Endpoints are references (alias or canonical id) as
/// authored; resolution to a canonical id happens in a distinct phase before
/// the graph is built, never inline in this constructor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadDependencyEdge {
    from: String,
    to: String,
    kind: BmadDependencyEdgeKind,
    demanded_version: Option<String>,
}

impl BmadDependencyEdge {
    /// Declares one edge between two references.
    ///
    /// # Errors
    ///
    /// Returns [`BmadDependencyError::InvalidId`] for an empty,
    /// oversized, or control-character-containing endpoint, condition, or
    /// demanded version.
    pub fn new(
        from: impl Into<String>,
        to: impl Into<String>,
        kind: BmadDependencyEdgeKind,
        demanded_version: Option<String>,
    ) -> Result<Self, BmadDependencyError> {
        let from = from.into();
        let to = to.into();
        let condition_valid = match &kind {
            BmadDependencyEdgeKind::Conditional { condition } => valid_id(condition),
            BmadDependencyEdgeKind::Required | BmadDependencyEdgeKind::Optional => true,
        };
        if !valid_id(&from)
            || !valid_id(&to)
            || !condition_valid
            || !valid_version(demanded_version.as_deref())
        {
            return Err(BmadDependencyError::InvalidId);
        }
        Ok(Self {
            from,
            to,
            kind,
            demanded_version,
        })
    }
}

/// Alias-to-canonical-id substitution, applied before any edge is recorded.
#[derive(Clone, Debug, Default)]
pub struct BmadDependencyAliasTable {
    aliases: BTreeMap<String, String>,
}

impl BmadDependencyAliasTable {
    /// Builds an alias table from `(alias, canonical_id)` pairs.
    ///
    /// # Errors
    ///
    /// Returns [`BmadDependencyError::BoundsViolation`] past [`MAX_ALIASES`]
    /// entries, or [`BmadDependencyError::InvalidId`] for an unsafe alias or
    /// canonical id.
    pub fn new(
        pairs: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Self, BmadDependencyError> {
        let aliases = pairs.into_iter().collect::<BTreeMap<_, _>>();
        if aliases.len() > MAX_ALIASES {
            return Err(BmadDependencyError::BoundsViolation);
        }
        for (alias, canonical) in &aliases {
            if !valid_id(alias) || !valid_id(canonical) {
                return Err(BmadDependencyError::InvalidId);
            }
        }
        Ok(Self { aliases })
    }

    /// Resolves one reference to its canonical id. A reference with no alias
    /// entry is already canonical -- it is returned unchanged, never dropped.
    fn resolve(&self, reference: &str) -> String {
        let mut current = reference.to_owned();
        for _ in 0..MAX_ALIAS_CHAIN {
            match self.aliases.get(&current) {
                Some(next) if *next != current => current = next.clone(),
                _ => break,
            }
        }
        current
    }
}

#[derive(Clone, Debug)]
struct ResolvedEdge {
    to: String,
    demanded_version: Option<String>,
}

/// The alias-resolved, canonical-id dependency graph.
#[derive(Clone, Debug)]
pub struct BmadDependencyGraph {
    nodes: BTreeMap<String, BmadDependencyNode>,
    required_out: BTreeMap<String, Vec<ResolvedEdge>>,
    optional_out: BTreeMap<String, Vec<ResolvedEdge>>,
    conditional_out: BTreeMap<String, Vec<(ResolvedEdge, String)>>,
}

impl BmadDependencyGraph {
    /// Builds the graph. Alias resolution runs first, as a distinct phase,
    /// so two aliases for one capability collapse onto the same canonical
    /// adjacency entry rather than producing two nodes.
    ///
    /// # Errors
    ///
    /// Returns [`BmadDependencyError::BoundsViolation`] past [`MAX_NODES`] or
    /// [`MAX_EDGES`], or [`BmadDependencyError::DuplicateNode`] when two
    /// nodes declare the same canonical id.
    pub fn build(
        nodes: Vec<BmadDependencyNode>,
        edges: Vec<BmadDependencyEdge>,
        aliases: &BmadDependencyAliasTable,
    ) -> Result<Self, BmadDependencyError> {
        if nodes.len() > MAX_NODES || edges.len() > MAX_EDGES {
            return Err(BmadDependencyError::BoundsViolation);
        }
        let mut node_map = BTreeMap::new();
        for node in nodes {
            if node_map.insert(node.id.clone(), node).is_some() {
                return Err(BmadDependencyError::DuplicateNode);
            }
        }
        let mut graph = Self {
            nodes: node_map,
            required_out: BTreeMap::new(),
            optional_out: BTreeMap::new(),
            conditional_out: BTreeMap::new(),
        };
        for edge in edges {
            graph.record_edge(edge, aliases);
        }
        Ok(graph)
    }

    fn record_edge(&mut self, edge: BmadDependencyEdge, aliases: &BmadDependencyAliasTable) {
        let from = aliases.resolve(&edge.from);
        let to = aliases.resolve(&edge.to);
        let resolved = ResolvedEdge {
            to,
            demanded_version: edge.demanded_version,
        };
        match edge.kind {
            BmadDependencyEdgeKind::Required => {
                self.required_out.entry(from).or_default().push(resolved);
            }
            BmadDependencyEdgeKind::Optional => {
                self.optional_out.entry(from).or_default().push(resolved);
            }
            BmadDependencyEdgeKind::Conditional { condition } => {
                self.conditional_out
                    .entry(from)
                    .or_default()
                    .push((resolved, condition));
            }
        }
    }

    #[must_use]
    pub fn node_kind(&self, id: &str) -> Option<BmadDependencyNodeKind> {
        self.nodes.get(id).map(BmadDependencyNode::kind)
    }

    /// Computes the dependency closure for one root reference.
    ///
    /// Traversal is an explicit worklist (breadth-first over a `VecDeque`),
    /// not recursion, so a deep required chain cannot blow the call stack.
    ///
    /// # Errors
    ///
    /// Returns [`BmadDependencyError::BoundsViolation`] if traversal would
    /// visit more than [`MAX_NODES`] nodes.
    pub fn closure_for(
        &self,
        root_reference: &str,
        aliases: &BmadDependencyAliasTable,
        active_conditions: &BTreeSet<String>,
    ) -> Result<BmadDependencyClosure, BmadDependencyError> {
        let root = aliases.resolve(root_reference);
        let mut state = ClosureState::new(root.clone());
        let mut queue = VecDeque::from([root.clone()]);
        while let Some(current) = queue.pop_front() {
            if state.required.len() + state.optional.len() + 1 > MAX_NODES {
                return Err(BmadDependencyError::BoundsViolation);
            }
            self.expand_required_like(&self.required_out, &current, &mut state, &mut queue);
            self.expand_conditional(&current, active_conditions, &mut state, &mut queue);
            self.expand_optional(&current, &mut state);
        }
        Ok(state.finish(&self.required_out, &root))
    }

    fn expand_required_like(
        &self,
        edges: &BTreeMap<String, Vec<ResolvedEdge>>,
        from: &str,
        state: &mut ClosureState,
        queue: &mut VecDeque<String>,
    ) {
        for edge in edges.get(from).into_iter().flatten() {
            state.record_demand(&edge.to, edge.demanded_version.as_deref());
            if self.nodes.contains_key(&edge.to) {
                if state.required.insert(edge.to.clone()) {
                    queue.push_back(edge.to.clone());
                }
            } else {
                state.missing_required.insert(edge.to.clone());
            }
        }
    }

    fn expand_conditional(
        &self,
        from: &str,
        active_conditions: &BTreeSet<String>,
        state: &mut ClosureState,
        queue: &mut VecDeque<String>,
    ) {
        for (edge, condition) in self.conditional_out.get(from).into_iter().flatten() {
            if !active_conditions.contains(condition) {
                continue;
            }
            state.record_demand(&edge.to, edge.demanded_version.as_deref());
            if self.nodes.contains_key(&edge.to) {
                if state.required.insert(edge.to.clone()) {
                    queue.push_back(edge.to.clone());
                }
            } else {
                state.missing_required.insert(edge.to.clone());
            }
        }
    }

    fn expand_optional(&self, from: &str, state: &mut ClosureState) {
        for edge in self.optional_out.get(from).into_iter().flatten() {
            state.record_demand(&edge.to, edge.demanded_version.as_deref());
            if self.nodes.contains_key(&edge.to) {
                state.optional.insert(edge.to.clone());
            } else {
                state.missing_optional.insert(edge.to.clone());
            }
        }
    }
}

struct ClosureState {
    root: String,
    required: BTreeSet<String>,
    optional: BTreeSet<String>,
    missing_required: BTreeSet<String>,
    missing_optional: BTreeSet<String>,
    demanded_versions: BTreeMap<String, BTreeSet<String>>,
}

impl ClosureState {
    fn new(root: String) -> Self {
        Self {
            root,
            required: BTreeSet::new(),
            optional: BTreeSet::new(),
            missing_required: BTreeSet::new(),
            missing_optional: BTreeSet::new(),
            demanded_versions: BTreeMap::new(),
        }
    }

    fn record_demand(&mut self, id: &str, demanded_version: Option<&str>) {
        if let Some(version) = demanded_version {
            self.demanded_versions
                .entry(id.to_owned())
                .or_default()
                .insert(version.to_owned());
        }
    }

    fn finish(
        self,
        required_out: &BTreeMap<String, Vec<ResolvedEdge>>,
        root: &str,
    ) -> BmadDependencyClosure {
        let mut diagnostics = Vec::new();
        for id in &self.missing_required {
            diagnostics.push(BmadDependencyDiagnostic::MissingRequired { id: id.clone() });
        }
        for id in &self.missing_optional {
            diagnostics.push(BmadDependencyDiagnostic::MissingOptional { id: id.clone() });
        }
        for (id, versions) in &self.demanded_versions {
            if versions.len() > 1 {
                let mut sorted = versions.iter().cloned().collect::<Vec<_>>();
                sorted.sort();
                if let (Some(first), Some(second)) = (sorted.first(), sorted.last()) {
                    diagnostics.push(BmadDependencyDiagnostic::VersionConflict {
                        id: id.clone(),
                        first_version: first.clone(),
                        second_version: second.clone(),
                    });
                }
            }
        }
        let mut universe = self.required.clone();
        universe.insert(root.to_owned());
        if let Some(cycle) = detect_required_cycle(required_out, &universe) {
            diagnostics.push(BmadDependencyDiagnostic::RequiredCycle { cycle });
        }
        diagnostics.sort_by_key(diagnostic_sort_key);
        BmadDependencyClosure {
            root: self.root,
            required: self.required,
            optional: self.optional,
            diagnostics,
        }
    }
}

/// Iterative Kahn's-algorithm cycle check over the required subgraph induced
/// by `universe`. No recursion: a queue of zero-indegree nodes is drained
/// until empty; any node left with nonzero indegree sits on a cycle.
fn detect_required_cycle(
    required_out: &BTreeMap<String, Vec<ResolvedEdge>>,
    universe: &BTreeSet<String>,
) -> Option<Vec<String>> {
    let mut indegree = universe
        .iter()
        .map(|id| (id.clone(), 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut adjacency = BTreeMap::new();
    for id in universe {
        let targets = required_out
            .get(id)
            .into_iter()
            .flatten()
            .map(|edge| edge.to.clone())
            .filter(|to| universe.contains(to))
            .collect::<Vec<_>>();
        for target in &targets {
            *indegree.entry(target.clone()).or_insert(0) += 1;
        }
        adjacency.insert(id.clone(), targets);
    }
    let mut zero = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| id.clone())
        .collect::<Vec<_>>();
    zero.sort();
    let mut queue = VecDeque::from(zero);
    let mut removed = 0usize;
    while let Some(node) = queue.pop_front() {
        removed += 1;
        let mut newly_zero = Vec::new();
        for target in adjacency.get(&node).into_iter().flatten() {
            if let Some(degree) = indegree.get_mut(target) {
                *degree -= 1;
                if *degree == 0 {
                    newly_zero.push(target.clone());
                }
            }
        }
        newly_zero.sort();
        queue.extend(newly_zero);
    }
    if removed == universe.len() {
        return None;
    }
    let mut remaining = indegree
        .into_iter()
        .filter(|(_, degree)| *degree > 0)
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    remaining.sort();
    Some(remaining)
}

fn diagnostic_sort_key(diagnostic: &BmadDependencyDiagnostic) -> (u8, String) {
    match diagnostic {
        BmadDependencyDiagnostic::MissingRequired { id } => (0, id.clone()),
        BmadDependencyDiagnostic::MissingOptional { id } => (1, id.clone()),
        BmadDependencyDiagnostic::VersionConflict { id, .. } => (2, id.clone()),
        BmadDependencyDiagnostic::RequiredCycle { cycle } => (3, cycle.join(",")),
    }
}

/// One diagnostic in the vocabulary shared by the runtime, Help, and the
/// Builder. Only [`Self::blocks_activation`] draws the activation line so all
/// three callers apply the same rule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BmadDependencyDiagnostic {
    /// A required (or currently-active conditional) reference resolved to no
    /// known node.
    MissingRequired { id: String },
    /// An optional reference resolved to no known node. Never blocks.
    MissingOptional { id: String },
    /// Two edges reaching one canonical id demanded different versions.
    VersionConflict {
        id: String,
        first_version: String,
        second_version: String,
    },
    /// A cycle made entirely of required edges, reachable from the root.
    RequiredCycle { cycle: Vec<String> },
}

impl BmadDependencyDiagnostic {
    #[must_use]
    pub const fn blocks_activation(&self) -> bool {
        !matches!(self, Self::MissingOptional { .. })
    }
}

/// The resolved closure for one root: required and optional membership plus
/// the diagnostics that decide whether activation is allowed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadDependencyClosure {
    root: String,
    required: BTreeSet<String>,
    optional: BTreeSet<String>,
    diagnostics: Vec<BmadDependencyDiagnostic>,
}

impl BmadDependencyClosure {
    #[must_use]
    pub fn root(&self) -> &str {
        &self.root
    }

    #[must_use]
    pub const fn required(&self) -> &BTreeSet<String> {
        &self.required
    }

    #[must_use]
    pub const fn optional(&self) -> &BTreeSet<String> {
        &self.optional
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[BmadDependencyDiagnostic] {
        &self.diagnostics
    }

    /// The required closure is complete: every required (or active
    /// conditional) reference resolved to a known node. A missing optional
    /// does not affect this.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        !self.diagnostics.iter().any(|diagnostic| {
            matches!(diagnostic, BmadDependencyDiagnostic::MissingRequired { .. })
        })
    }

    /// The single predicate callers use to gate activation: a missing
    /// required dependency, a required cycle, or a version conflict all
    /// block; a missing optional never does.
    #[must_use]
    pub fn activation_allowed(&self) -> bool {
        !self
            .diagnostics
            .iter()
            .any(BmadDependencyDiagnostic::blocks_activation)
    }
}

/// Fail-closed result for dependency graph construction and closure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum BmadDependencyError {
    #[error("dependency graph exceeds its bounded node, edge, or alias count")]
    BoundsViolation,
    #[error("dependency identity is empty, oversized, or contains a control character")]
    InvalidId,
    #[error("dependency graph declares the same canonical id twice")]
    DuplicateNode,
}

fn valid_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_ID_BYTES && !value.chars().any(char::is_control)
}

fn valid_version(value: Option<&str>) -> bool {
    match value {
        None => true,
        Some(version) => {
            !version.is_empty()
                && version.len() <= MAX_VERSION_BYTES
                && !version.chars().any(char::is_control)
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn hash(seed: u8) -> Sha256Digest {
        crate::sha256_bytes(&[seed])
    }

    fn node(id: &str, kind: BmadDependencyNodeKind) -> BmadDependencyNode {
        BmadDependencyNode::new(id, kind, None, hash(1)).expect("node")
    }

    fn required(from: &str, to: &str) -> BmadDependencyEdge {
        BmadDependencyEdge::new(from, to, BmadDependencyEdgeKind::Required, None).expect("edge")
    }

    fn optional(from: &str, to: &str) -> BmadDependencyEdge {
        BmadDependencyEdge::new(from, to, BmadDependencyEdgeKind::Optional, None).expect("edge")
    }

    fn conditional(from: &str, to: &str, condition: &str) -> BmadDependencyEdge {
        BmadDependencyEdge::new(
            from,
            to,
            BmadDependencyEdgeKind::Conditional {
                condition: condition.to_owned(),
            },
            None,
        )
        .expect("edge")
    }

    fn no_aliases() -> BmadDependencyAliasTable {
        BmadDependencyAliasTable::new(std::iter::empty()).expect("aliases")
    }

    // Break caught: an alias and its canonical id being treated as two
    // different graph members, which would double-count a capability in the
    // closure and let callers disagree about what "one dependency" means.
    #[test]
    fn two_aliases_for_one_capability_collapse_to_a_single_node() {
        let nodes = vec![
            node("cap_root", BmadDependencyNodeKind::Skill),
            node("skill_canonical", BmadDependencyNodeKind::Skill),
        ];
        let edges = vec![
            required("cap_root", "skill_alias_one"),
            required("cap_root", "skill_alias_two"),
        ];
        let aliases = BmadDependencyAliasTable::new([
            ("skill_alias_one".to_owned(), "skill_canonical".to_owned()),
            ("skill_alias_two".to_owned(), "skill_canonical".to_owned()),
        ])
        .expect("aliases");
        let graph = BmadDependencyGraph::build(nodes, edges, &aliases).expect("graph");
        let closure = graph
            .closure_for("cap_root", &aliases, &BTreeSet::new())
            .expect("closure");
        assert_eq!(
            closure.required(),
            &BTreeSet::from(["skill_canonical".to_owned()])
        );
        assert!(closure.activation_allowed());
    }

    // Break caught: a required-only cycle silently passing closure and
    // letting two capabilities activate each other forever.
    #[test]
    fn required_edge_cycle_is_rejected_and_blocks_activation() {
        let nodes = vec![
            node("cap_a", BmadDependencyNodeKind::Skill),
            node("cap_b", BmadDependencyNodeKind::Skill),
        ];
        let edges = vec![required("cap_a", "cap_b"), required("cap_b", "cap_a")];
        let aliases = no_aliases();
        let graph = BmadDependencyGraph::build(nodes, edges, &aliases).expect("graph");
        let closure = graph
            .closure_for("cap_a", &aliases, &BTreeSet::new())
            .expect("closure");
        assert!(closure.diagnostics().iter().any(|diagnostic| matches!(
            diagnostic,
            BmadDependencyDiagnostic::RequiredCycle { .. }
        )));
        assert!(!closure.activation_allowed());
    }

    // Break caught: an optional (or conditional) back-edge being treated the
    // same as a required cycle and blocking activation for a legitimate
    // authoring pattern.
    #[test]
    fn optional_and_conditional_cycles_are_tolerated() {
        let nodes = vec![
            node("cap_a", BmadDependencyNodeKind::Skill),
            node("cap_b", BmadDependencyNodeKind::Skill),
        ];
        let edges = vec![
            required("cap_a", "cap_b"),
            optional("cap_b", "cap_a"),
            conditional("cap_b", "cap_a", "cond_loop"),
        ];
        let aliases = no_aliases();
        let graph = BmadDependencyGraph::build(nodes, edges, &aliases).expect("graph");
        let active = BTreeSet::from(["cond_loop".to_owned()]);
        let closure = graph
            .closure_for("cap_a", &aliases, &active)
            .expect("closure");
        assert!(!closure.diagnostics().iter().any(|diagnostic| matches!(
            diagnostic,
            BmadDependencyDiagnostic::RequiredCycle { .. }
        )));
        assert!(closure.activation_allowed());
    }

    // Break caught: a required dependency that resolves to nothing being
    // dropped instead of surfaced, leaving the closure silently incomplete.
    #[test]
    fn unresolved_required_dependency_yields_incomplete_closure_naming_the_id() {
        let nodes = vec![node("cap_root", BmadDependencyNodeKind::Skill)];
        let edges = vec![required("cap_root", "skill_missing")];
        let aliases = no_aliases();
        let graph = BmadDependencyGraph::build(nodes, edges, &aliases).expect("graph");
        let closure = graph
            .closure_for("cap_root", &aliases, &BTreeSet::new())
            .expect("closure");
        assert!(!closure.is_complete());
        assert!(!closure.activation_allowed());
        assert!(closure
            .diagnostics()
            .contains(&BmadDependencyDiagnostic::MissingRequired {
                id: "skill_missing".to_owned(),
            }));
    }

    // Break caught: a missing optional dependency incorrectly blocking
    // activation instead of degrading the reported set.
    #[test]
    fn missing_optional_dependency_leaves_closure_complete_and_activation_allowed() {
        let nodes = vec![node("cap_root", BmadDependencyNodeKind::Skill)];
        let edges = vec![optional("cap_root", "tool_missing")];
        let aliases = no_aliases();
        let graph = BmadDependencyGraph::build(nodes, edges, &aliases).expect("graph");
        let closure = graph
            .closure_for("cap_root", &aliases, &BTreeSet::new())
            .expect("closure");
        assert!(closure.is_complete());
        assert!(closure.activation_allowed());
        assert!(closure.optional().is_empty());
        assert!(closure
            .diagnostics()
            .contains(&BmadDependencyDiagnostic::MissingOptional {
                id: "tool_missing".to_owned(),
            }));
    }

    // Break caught: two edges demanding incompatible versions of one
    // canonical id being silently reconciled instead of blocking, which
    // would let a runtime pick an arbitrary winner.
    #[test]
    fn incompatible_version_demands_block_activation_and_name_both_versions() {
        let nodes = vec![
            node("cap_root", BmadDependencyNodeKind::Skill),
            node("tool_shared", BmadDependencyNodeKind::Tool),
        ];
        let edges = vec![
            BmadDependencyEdge::new(
                "cap_root",
                "tool_shared",
                BmadDependencyEdgeKind::Required,
                Some("1.0.0".to_owned()),
            )
            .expect("edge"),
            BmadDependencyEdge::new(
                "cap_root",
                "tool_shared",
                BmadDependencyEdgeKind::Optional,
                Some("2.0.0".to_owned()),
            )
            .expect("edge"),
        ];
        let aliases = no_aliases();
        let graph = BmadDependencyGraph::build(nodes, edges, &aliases).expect("graph");
        let closure = graph
            .closure_for("cap_root", &aliases, &BTreeSet::new())
            .expect("closure");
        assert!(!closure.activation_allowed());
        assert!(closure
            .diagnostics()
            .contains(&BmadDependencyDiagnostic::VersionConflict {
                id: "tool_shared".to_owned(),
                first_version: "1.0.0".to_owned(),
                second_version: "2.0.0".to_owned(),
            }));
    }

    // Break caught: a conditional dependency being pulled into the closure
    // regardless of whether its condition actually holds at runtime.
    #[test]
    fn conditional_edges_are_only_included_when_their_condition_holds() {
        let nodes = vec![
            node("cap_root", BmadDependencyNodeKind::Skill),
            node("skill_conditional", BmadDependencyNodeKind::Skill),
        ];
        let edges = vec![conditional("cap_root", "skill_conditional", "cond_beta")];
        let aliases = no_aliases();
        let graph = BmadDependencyGraph::build(nodes, edges, &aliases).expect("graph");

        let inactive = graph
            .closure_for("cap_root", &aliases, &BTreeSet::new())
            .expect("closure");
        assert!(inactive.required().is_empty());
        assert!(inactive.diagnostics().is_empty());

        let active = BTreeSet::from(["cond_beta".to_owned()]);
        let active_closure = graph
            .closure_for("cap_root", &aliases, &active)
            .expect("closure");
        assert_eq!(
            active_closure.required(),
            &BTreeSet::from(["skill_conditional".to_owned()])
        );
    }

    // Break caught: a persona's prompt text being copied into the dependency
    // graph instead of the persona participating purely as an id/edge, which
    // would leak untrusted prose into a structure Help and the Builder trust
    // as inert.
    #[test]
    fn persona_participates_as_a_node_and_edge_with_no_copied_prompt_text() {
        let nodes = vec![
            node("cap_root", BmadDependencyNodeKind::Skill),
            node("persona_analyst", BmadDependencyNodeKind::Persona),
        ];
        let edges = vec![required("cap_root", "persona_analyst")];
        let aliases = no_aliases();
        let graph = BmadDependencyGraph::build(nodes, edges, &aliases).expect("graph");
        let closure = graph
            .closure_for("cap_root", &aliases, &BTreeSet::new())
            .expect("closure");
        assert!(closure.required().contains("persona_analyst"));
        assert_eq!(
            graph.node_kind("persona_analyst"),
            Some(BmadDependencyNodeKind::Persona)
        );
        // BmadDependencyNode has exactly id/kind/version/content_hash --
        // there is no field here capable of holding free-form prose.
    }

    // Break caught: recursive traversal blowing the call stack on a deep
    // required chain; this proves the worklist handles depth a call-stack
    // walk could not.
    #[test]
    fn deep_required_chain_resolves_without_stack_overflow() {
        const DEPTH: usize = 5_000;
        let mut nodes = Vec::with_capacity(DEPTH);
        let mut edges = Vec::with_capacity(DEPTH - 1);
        for index in 0..DEPTH {
            nodes.push(node(
                &format!("cap_{index:05}"),
                BmadDependencyNodeKind::Skill,
            ));
            if index > 0 {
                edges.push(required(
                    &format!("cap_{:05}", index - 1),
                    &format!("cap_{index:05}"),
                ));
            }
        }
        let aliases = no_aliases();
        let graph = BmadDependencyGraph::build(nodes, edges, &aliases).expect("graph");
        let closure = graph
            .closure_for("cap_00000", &aliases, &BTreeSet::new())
            .expect("closure");
        assert_eq!(closure.required().len(), DEPTH - 1);
        assert!(closure.activation_allowed());
    }

    #[test]
    fn invalid_identities_are_rejected() {
        assert!(BmadDependencyNode::new("", BmadDependencyNodeKind::Skill, None, hash(1)).is_err());
        assert!(BmadDependencyEdge::new(
            "cap_a",
            "cap_b",
            BmadDependencyEdgeKind::Required,
            Some(String::new()),
        )
        .is_err());
    }

    #[test]
    fn duplicate_node_id_is_rejected() {
        let nodes = vec![
            node("cap_dup", BmadDependencyNodeKind::Skill),
            node("cap_dup", BmadDependencyNodeKind::Skill),
        ];
        let aliases = no_aliases();
        assert!(matches!(
            BmadDependencyGraph::build(nodes, Vec::new(), &aliases),
            Err(BmadDependencyError::DuplicateNode)
        ));
    }
}
