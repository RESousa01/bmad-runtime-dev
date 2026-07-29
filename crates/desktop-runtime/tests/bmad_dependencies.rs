#![allow(clippy::expect_used)]

//! Capability dependency closure, proven through the crate's public surface.
//!
//! Every test name begins with `bmad_dependencies_` so the plan's binding
//! verification command — `cargo test -p desktop-runtime bmad_dependencies`,
//! which filters on test *name* rather than target — actually selects them.
//! Named `bmad_dependency_*` they matched nothing, and the command reported
//! success while running zero tests.

use std::collections::BTreeSet;

use desktop_runtime::{
    BmadDependencyAliasTable, BmadDependencyDiagnostic, BmadDependencyEdge, BmadDependencyEdgeKind,
    BmadDependencyGraph, BmadDependencyNode, BmadDependencyNodeKind, Sha256Digest,
};

fn hash(seed: u8) -> Sha256Digest {
    Sha256Digest::from_bytes([seed; 32])
}

fn node(id: &str, kind: BmadDependencyNodeKind) -> BmadDependencyNode {
    BmadDependencyNode::new(id, kind, None, hash(1)).expect("valid node")
}

fn versioned(id: &str, version: &str) -> BmadDependencyNode {
    BmadDependencyNode::new(
        id,
        BmadDependencyNodeKind::Tool,
        Some(version.to_owned()),
        hash(2),
    )
    .expect("valid versioned node")
}

fn edge(from: &str, to: &str, kind: BmadDependencyEdgeKind) -> BmadDependencyEdge {
    BmadDependencyEdge::new(from, to, kind, None).expect("valid edge")
}

fn demanding(from: &str, to: &str, version: &str) -> BmadDependencyEdge {
    BmadDependencyEdge::new(
        from,
        to,
        BmadDependencyEdgeKind::Required,
        Some(version.to_owned()),
    )
    .expect("valid versioned edge")
}

fn no_aliases() -> BmadDependencyAliasTable {
    BmadDependencyAliasTable::new([]).expect("empty alias table")
}

fn no_conditions() -> BTreeSet<String> {
    BTreeSet::new()
}

// Break caught: acceptance 1. A required reference that resolves to nothing
// being dropped instead of reported would let a capability activate with a
// hole in its closure, the failure only surfacing mid-run.
#[test]
fn bmad_dependencies_unresolved_required_reference_yields_an_incomplete_closure_naming_it() {
    let graph = BmadDependencyGraph::build(
        vec![node("skill_root", BmadDependencyNodeKind::Skill)],
        vec![edge(
            "skill_root",
            "skill_missing",
            BmadDependencyEdgeKind::Required,
        )],
        &no_aliases(),
    )
    .expect("graph builds");

    let closure = graph
        .closure_for("skill_root", &no_aliases(), &no_conditions())
        .expect("closure resolves");

    assert!(!closure.is_complete());
    assert!(!closure.activation_allowed());
    assert!(closure.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic,
        BmadDependencyDiagnostic::MissingRequired { id } if id == "skill_missing"
    )));
}

// Break caught: the whole point of separating edge kinds. One cycle check over
// every edge would block a capability whose only cycle runs through an
// optional persona reference nobody needed.
#[test]
fn bmad_dependencies_required_cycles_block_activation_while_optional_cycles_do_not() {
    let required_cycle = BmadDependencyGraph::build(
        vec![
            node("skill_a", BmadDependencyNodeKind::Skill),
            node("skill_b", BmadDependencyNodeKind::Skill),
        ],
        vec![
            edge("skill_a", "skill_b", BmadDependencyEdgeKind::Required),
            edge("skill_b", "skill_a", BmadDependencyEdgeKind::Required),
        ],
        &no_aliases(),
    )
    .expect("graph builds");
    let blocked = required_cycle
        .closure_for("skill_a", &no_aliases(), &no_conditions())
        .expect("closure resolves");
    assert!(!blocked.activation_allowed());
    assert!(blocked
        .diagnostics()
        .iter()
        .any(|diagnostic| matches!(diagnostic, BmadDependencyDiagnostic::RequiredCycle { .. })));

    // The same back-edge shape, declared optional, is legal.
    let optional_cycle = BmadDependencyGraph::build(
        vec![
            node("skill_a", BmadDependencyNodeKind::Skill),
            node("skill_b", BmadDependencyNodeKind::Skill),
        ],
        vec![
            edge("skill_a", "skill_b", BmadDependencyEdgeKind::Required),
            edge("skill_b", "skill_a", BmadDependencyEdgeKind::Optional),
        ],
        &no_aliases(),
    )
    .expect("graph builds");
    let allowed = optional_cycle
        .closure_for("skill_a", &no_aliases(), &no_conditions())
        .expect("closure resolves");
    assert!(allowed.activation_allowed());
    assert!(allowed.is_complete());
}

// Break caught: acceptance 3. Silently preferring one version would install a
// capability against a dependency it was never validated with.
#[test]
fn bmad_dependencies_incompatible_version_demands_block_and_name_both_versions() {
    let graph = BmadDependencyGraph::build(
        vec![
            node("skill_root", BmadDependencyNodeKind::Skill),
            node("skill_mid", BmadDependencyNodeKind::Skill),
            versioned("tool_shared", "1.0.0"),
        ],
        vec![
            edge("skill_root", "skill_mid", BmadDependencyEdgeKind::Required),
            demanding("skill_root", "tool_shared", "1.0.0"),
            demanding("skill_mid", "tool_shared", "2.0.0"),
        ],
        &no_aliases(),
    )
    .expect("graph builds");

    let closure = graph
        .closure_for("skill_root", &no_aliases(), &no_conditions())
        .expect("closure resolves");

    assert!(!closure.activation_allowed());
    let conflict = closure
        .diagnostics()
        .iter()
        .find_map(|diagnostic| match diagnostic {
            BmadDependencyDiagnostic::VersionConflict {
                id,
                first_version,
                second_version,
            } if id == "tool_shared" => Some((first_version.clone(), second_version.clone())),
            _ => None,
        })
        .expect("version conflict is reported");
    // Both demands are named, so a reader can tell who needs reconciling.
    assert!(conflict.0 == "1.0.0" || conflict.1 == "1.0.0");
    assert!(conflict.0 == "2.0.0" || conflict.1 == "2.0.0");
}

// Break caught: an alias resolved after graph construction produces two nodes
// for one capability, so its closure silently splits in half.
#[test]
fn bmad_dependencies_aliases_resolve_before_construction_and_collapse_to_one_node() {
    let aliases = BmadDependencyAliasTable::new([
        ("skill_alias".to_owned(), "skill_canonical".to_owned()),
        ("skill_legacy".to_owned(), "skill_canonical".to_owned()),
    ])
    .expect("alias table");

    let graph = BmadDependencyGraph::build(
        vec![
            node("skill_root", BmadDependencyNodeKind::Skill),
            node("skill_canonical", BmadDependencyNodeKind::Skill),
        ],
        vec![
            edge(
                "skill_root",
                "skill_alias",
                BmadDependencyEdgeKind::Required,
            ),
            edge(
                "skill_root",
                "skill_legacy",
                BmadDependencyEdgeKind::Required,
            ),
        ],
        &aliases,
    )
    .expect("graph builds");

    let closure = graph
        .closure_for("skill_root", &aliases, &no_conditions())
        .expect("closure resolves");

    assert!(closure.is_complete());
    assert!(closure.required().contains("skill_canonical"));
    // One node, not two: both aliases collapsed before any edge was built.
    assert_eq!(closure.required().len(), 1);

    // Entering through an alias reaches the same closure as the canonical id.
    let via_alias = graph
        .closure_for("skill_alias", &aliases, &no_conditions())
        .expect("closure resolves");
    assert_eq!(via_alias.root(), "skill_canonical");
}

// Break caught: treating a missing optional dependency as fatal would make a
// capability refuse to run over a nicety it never required.
#[test]
fn bmad_dependencies_missing_optional_degrades_without_blocking_activation() {
    let graph = BmadDependencyGraph::build(
        vec![node("skill_root", BmadDependencyNodeKind::Skill)],
        vec![edge(
            "skill_root",
            "tool_absent",
            BmadDependencyEdgeKind::Optional,
        )],
        &no_aliases(),
    )
    .expect("graph builds");

    let closure = graph
        .closure_for("skill_root", &no_aliases(), &no_conditions())
        .expect("closure resolves");

    assert!(closure.is_complete());
    assert!(closure.activation_allowed());
    let missing = closure
        .diagnostics()
        .iter()
        .find(|diagnostic| {
            matches!(
                diagnostic,
                BmadDependencyDiagnostic::MissingOptional { id } if id == "tool_absent"
            )
        })
        .expect("the reduced set is reported rather than hidden");
    assert!(!missing.blocks_activation());
}

// Break caught: a conditional dependency pulled in unconditionally drags a
// whole subtree into every run.
#[test]
fn bmad_dependencies_conditional_edges_enter_the_closure_only_when_active() {
    let build = || {
        BmadDependencyGraph::build(
            vec![
                node("skill_root", BmadDependencyNodeKind::Skill),
                node("skill_extra", BmadDependencyNodeKind::Skill),
            ],
            vec![edge(
                "skill_root",
                "skill_extra",
                BmadDependencyEdgeKind::Conditional {
                    condition: "flag_enabled".to_owned(),
                },
            )],
            &no_aliases(),
        )
        .expect("graph builds")
    };

    let inactive = build()
        .closure_for("skill_root", &no_aliases(), &no_conditions())
        .expect("closure resolves");
    assert!(!inactive.required().contains("skill_extra"));

    let mut active = BTreeSet::new();
    active.insert("flag_enabled".to_owned());
    let included = build()
        .closure_for("skill_root", &no_aliases(), &active)
        .expect("closure resolves");
    assert!(included.required().contains("skill_extra"));
}

// Break caught: acceptance 2. Runtime, Help and Builder each growing their own
// availability rule is how the three drift apart; one graph and one diagnostic
// vocabulary is what keeps them agreeing.
#[test]
fn bmad_dependencies_persona_is_an_edge_and_the_graph_carries_no_prompt_text() {
    let graph = BmadDependencyGraph::build(
        vec![
            node("skill_root", BmadDependencyNodeKind::Skill),
            node("persona_analyst", BmadDependencyNodeKind::Persona),
        ],
        vec![edge(
            "skill_root",
            "persona_analyst",
            BmadDependencyEdgeKind::Required,
        )],
        &no_aliases(),
    )
    .expect("graph builds");

    let closure = graph
        .closure_for("skill_root", &no_aliases(), &no_conditions())
        .expect("closure resolves");

    assert!(closure.required().contains("persona_analyst"));
    assert_eq!(
        graph.node_kind("persona_analyst"),
        Some(BmadDependencyNodeKind::Persona)
    );
    // A persona participates by identity. The closure exposes ids only, so
    // there is nowhere for copied prompt text to travel.
    assert!(closure
        .required()
        .iter()
        .all(|id| !id.contains(' ') && id.len() <= 128));
}

// Break caught: a recursive walk over a deep chain overflows the stack, and a
// large installed library is exactly where that first happens.
#[test]
fn bmad_dependencies_deep_required_chain_resolves_without_recursion() {
    let depth = 5_000;
    let mut nodes = Vec::with_capacity(depth);
    let mut edges = Vec::with_capacity(depth - 1);
    for index in 0..depth {
        nodes.push(node(
            &format!("skill_{index}"),
            BmadDependencyNodeKind::Skill,
        ));
        if index + 1 < depth {
            edges.push(edge(
                &format!("skill_{index}"),
                &format!("skill_{}", index + 1),
                BmadDependencyEdgeKind::Required,
            ));
        }
    }

    let graph = BmadDependencyGraph::build(nodes, edges, &no_aliases()).expect("deep graph builds");
    let closure = graph
        .closure_for("skill_0", &no_aliases(), &no_conditions())
        .expect("deep closure resolves");

    assert!(closure.is_complete());
    assert!(closure.activation_allowed());
    assert_eq!(closure.required().len(), depth - 1);
}
