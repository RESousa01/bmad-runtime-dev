//! Source-derived conformance tests for the canonical `bmad-spec` adapter.

#![allow(clippy::expect_used)]

use desktop_runtime::{
    generated_contracts::{
        BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationMode,
    },
    BmadMethodSkill, BmadSkillSupportState, BmadSpecSkill,
};

const SKILL_ROOT: &str = "src/bmm-skills/2-plan-workflows/bmad-spec";

// Break caught: omitting a pinned bmad-spec member (or silently changing its
// declared execution shape) leaves the generic, durable runtime with an
// incomplete source boundary before it can apply its typed ports and Airlock
// policy.
#[test]
fn bmad_spec_descriptor_closes_the_pinned_skill_and_exposes_its_execution_shape() {
    let descriptor = BmadSpecSkill.descriptor();

    assert_eq!(descriptor.canonical_id(), "bmm:bmad-spec");
    assert_eq!(
        descriptor.source_path(),
        "src/bmm-skills/2-plan-workflows/bmad-spec/SKILL.md"
    );
    assert!(descriptor.declared_actions().is_empty());
    assert_eq!(
        descriptor.supported_modes(),
        [
            BmadInvocationMode::Interactive,
            BmadInvocationMode::Headless
        ]
    );
    assert_eq!(descriptor.predecessor(), None);
    assert_eq!(descriptor.follower(), None);
    assert_eq!(descriptor.output_locations(), ["output_folder"]);
    assert_eq!(descriptor.artifact_kinds(), [BmadArtifactKind::Document]);
    assert_eq!(
        descriptor.completion_hooks(),
        [BmadArtifactCompletionHookRunHook::PersistArtifacts]
    );
    assert_eq!(descriptor.support_state(), BmadSkillSupportState::Exact);

    let resources = descriptor.declared_resources();
    assert_eq!(
        resources
            .iter()
            .map(desktop_runtime::BmadResource::path)
            .collect::<Vec<_>>(),
        [
            "src/bmm-skills/2-plan-workflows/bmad-spec/SKILL.md",
            "src/bmm-skills/2-plan-workflows/bmad-spec/assets/headless-schemas.md",
            "src/bmm-skills/2-plan-workflows/bmad-spec/assets/spec-template.md",
            "src/bmm-skills/2-plan-workflows/bmad-spec/assets/stories-schema.md",
            "src/bmm-skills/2-plan-workflows/bmad-spec/customize.toml",
        ]
    );
    assert_eq!(
        resources
            .iter()
            .map(|resource| resource.content_hash().hex_value())
            .collect::<Vec<_>>(),
        [
            "5b5680dbf01dbfc5b4a5485e70f20ea48ac21a1d6137c4ea2ff704262dcc45d2",
            "83afb3614716db2347d83586a67b9c524492dd50db5282436ccb35c99fb5f08a",
            "4b3a032c045fdd0903933446fc07f93e879997f28a0294ed4344da71192b20ee",
            "4f8137c00e2710288354a4466d1cfb90704f4b2a04bd3b63924218fd589cc6a9",
            "a89e0f53f9b6386c281dd0158a21801a6b16f122085fd68b0220cd403c18f830",
        ]
    );
}

// Break caught: direct I/O, process, network, scheduler, model, artifact, or
// action-plan dependencies in this adapter would allow source prose to escape
// the generic runtime's durable state and governed authority boundary.
#[test]
fn bmad_spec_adapter_is_a_descriptor_only_authority_boundary() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/bmad/skills/adapters/bmad_spec.rs"),
    )
    .expect("bmad-spec adapter source");
    let forbidden = [
        "std::fs",
        "std::process",
        "std::net",
        "tokio::net",
        "reqwest",
        "Command::new",
        "ModelAccessPort",
        "ActionPlanPort",
        "ArtifactPort",
        "BmadRuntimePort",
    ];
    for (index, line) in source.lines().enumerate() {
        let code = line.split("//").next().unwrap_or_default();
        for token in forbidden {
            assert!(
                !code.contains(token),
                "bmad_spec.rs line {} references forbidden capability {token:?}: {line:?}",
                index + 1,
            );
        }
    }
    assert!(source.contains(SKILL_ROOT));
}

// Break caught: a descriptor-only registration can make `bmm:bmad-spec`
// visible to catalog and Help while leaving its source-specific workflow
// unreachable from the actual runtime.  The executable binding belongs in the
// shared skill harness/registry, not in this authority-free adapter.
#[test]
fn bmad_spec_registers_an_executable_governed_runtime_binding() {
    let harness = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bmad/skills/harness.rs"),
    )
    .expect("Method skill harness source");

    assert!(
        harness.contains("fn execution_binding("),
        "BmadMethodSkill exposes descriptor() only; bmad-spec cannot register executable behavior"
    );
}
