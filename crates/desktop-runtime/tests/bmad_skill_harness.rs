#![allow(clippy::expect_used)]

//! The shared Method-skill harness, proven through the crate's public surface.
//!
//! Every test name begins with `bmad_skill_harness_` so a name-filtered test
//! command (`cargo test -p desktop-runtime bmad_skill_harness`) actually
//! selects them, matching the convention `bmad_dependencies.rs` documents.
//!
//! This file exercises [`BmadMethodSkill`] and [`BmadSkillDescriptor`] from
//! outside the crate, the way a downstream MTH adapter's own test module
//! will: only through `desktop_runtime`'s public re-exports, never through a
//! private path into `bmad::skills`.

use desktop_runtime::generated_contracts::{
    BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationDeclaredAction,
    BmadInvocationDeclaredActionName, BmadInvocationMode,
};
use desktop_runtime::{
    sha256_bytes, BmadMethodSkill, BmadSkillDescriptor, BmadSkillDescriptorInput,
    BmadSkillSupportState,
};

/// A minimal example adapter, shaped the way a real MTH-01..MTH-48 adapter
/// would be: a zero-sized type whose only method returns one already-built,
/// already-validated descriptor.
struct ExampleProductBriefSkill;

impl BmadMethodSkill for ExampleProductBriefSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        let action_name: BmadInvocationDeclaredActionName =
            "draft".try_into().expect("valid action name");
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "bmm:bmad-product-brief".to_owned(),
            source_path: "src/bmm-skills/1-analysis/bmad-product-brief/SKILL.md".to_owned(),
            source_sha256: sha256_bytes(b"product brief source"),
            declared_actions: vec![BmadInvocationDeclaredAction {
                declared_inputs: Vec::new(),
                name: action_name,
            }],
            supported_modes: vec![
                BmadInvocationMode::Interactive,
                BmadInvocationMode::Headless,
            ],
            predecessor: None,
            follower: Some("bmm:bmad-prd".to_owned()),
            output_locations: vec!["planning_artifacts".to_owned()],
            artifact_kinds: vec![BmadArtifactKind::Document],
            completion_hooks: vec![BmadArtifactCompletionHookRunHook::PersistArtifacts],
            declared_resources: Vec::new(),
            support_state: BmadSkillSupportState::Unavailable,
        })
        .expect("this fixture's fields satisfy the harness's own bounds")
    }
}

// Break caught: a per-skill adapter having to hand-roll its own descriptor
// shape instead of one typed value round-tripping through the shared
// harness -- exactly the divergence 48 independent adapters would otherwise
// invent.
#[test]
fn bmad_skill_harness_descriptor_round_trips_through_a_worked_adapter() {
    let descriptor = ExampleProductBriefSkill.descriptor();
    assert_eq!(descriptor.canonical_id(), "bmm:bmad-product-brief");
    assert_eq!(descriptor.follower(), Some("bmm:bmad-prd"));
    assert_eq!(descriptor.predecessor(), None);
    assert_eq!(descriptor.declared_actions().len(), 1);
    assert_eq!(descriptor.supported_modes().len(), 2);
    assert_eq!(
        descriptor.support_state(),
        BmadSkillSupportState::Unavailable
    );
}

// Break caught: Exact/Adapted/Shim/Unavailable collapsing to fewer states at
// the crate boundary (e.g. a `#[non_exhaustive]` or re-export dropping a
// variant), which would silently break the ledger's requirement that a
// `pending` row be representable as genuinely distinct from a classified one.
#[test]
fn bmad_skill_harness_support_states_are_distinguishable_across_the_crate_boundary() {
    let states = [
        BmadSkillSupportState::Exact,
        BmadSkillSupportState::Adapted,
        BmadSkillSupportState::Shim,
        BmadSkillSupportState::Unavailable,
    ];
    for (left_index, left) in states.iter().enumerate() {
        for (right_index, right) in states.iter().enumerate() {
            assert_eq!(left_index == right_index, left == right);
        }
    }
}

// Break caught: the harness module reaching for a filesystem, process,
// network, model, or scheduler capability. This mirrors
// `chat_bmad_capability_separation_source_has_no_bmad_module_dependency` in
// `chat_bmad_capability_separation.rs`: scan the module's own source (from
// outside, via the crate manifest dir) rather than trust a doc comment's
// claim.
#[test]
fn bmad_skill_harness_module_references_no_filesystem_process_network_or_model_symbol() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bmad/skills/harness.rs");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()));
    let module_source = source
        .split_once("#[cfg(test)]")
        .map_or(source.as_str(), |(module, _)| module);
    let forbidden = [
        "std::fs",
        "std::process",
        "std::net",
        "tokio::net",
        "reqwest",
        "model_port",
        "ModelPort",
        "chat::",
        "ChatSession",
        "Command::new",
    ];
    for (index, line) in module_source.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("");
        for token in forbidden {
            assert!(
                !code.contains(token),
                "harness.rs line {} references a forbidden capability {token:?}: {line:?}",
                index + 1,
            );
        }
    }
}
