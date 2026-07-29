//! Source-derived conformance tests for the canonical `bmad-deep-recon` adapter.

#![allow(clippy::expect_used, clippy::panic)]

use desktop_runtime::{
    generated_contracts::{
        BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationMode,
    },
    sha256_bytes, BmadDeepReconSkill, BmadMethodSkill, BmadSkillSupportState, BmadSkillToolIntent,
};

const SKILL_ROOT: &str = "src/core-skills/bmad-deep-recon";

// Break caught: omitting any pinned Deep Recon member leaves the generic
// runtime unable to bind the source-faithful workflow before execution.
#[test]
fn bmad_deep_recon_descriptor_closes_every_pinned_member() {
    let descriptor = BmadDeepReconSkill.descriptor();

    assert_eq!(descriptor.canonical_id(), "core:bmad-deep-recon");
    assert_eq!(descriptor.source_path(), format!("{SKILL_ROOT}/SKILL.md"));
    assert_eq!(
        descriptor.supported_modes(),
        [
            BmadInvocationMode::Interactive,
            BmadInvocationMode::Headless
        ]
    );
    assert_eq!(
        descriptor.artifact_kinds(),
        [BmadArtifactKind::Document, BmadArtifactKind::FileSet]
    );
    assert_eq!(
        descriptor.completion_hooks(),
        [BmadArtifactCompletionHookRunHook::PersistArtifacts]
    );
    assert_eq!(descriptor.support_state(), BmadSkillSupportState::Exact);
    assert_eq!(descriptor.declared_resources().len(), 20);

    for resource in descriptor.declared_resources() {
        let resource_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/bmad-foundation/source/method/6.10.0")
            .join(resource.path());
        let bytes = std::fs::read(&resource_path).unwrap_or_else(|error| {
            panic!(
                "could not read pinned resource {}: {error}",
                resource_path.display()
            )
        });
        assert_eq!(
            resource.content_hash(),
            sha256_bytes(&bytes),
            "{} must retain its pinned SHA-256 identity",
            resource.path()
        );
    }
}

// Break caught: a descriptor-only Deep Recon row can be visible in Help while
// remaining impossible to prepare through the shared Method compiler.
#[test]
fn bmad_deep_recon_exposes_the_generic_execution_contract() {
    let contract = BmadDeepReconSkill
        .execution_contract()
        .expect("Deep Recon must opt into generic Method execution");

    assert_eq!(contract.package_name(), "bmad-method");
    assert_eq!(contract.package_version(), "6.10.0");
}

// Break caught: collapsing the five source services to an availability flag
// makes Refresh/Deepen non-resumable and silently routes Draft/Process through
// Run even though the source declares different transition sequences.
#[test]
fn bmad_deep_recon_compiles_every_source_action_and_distinct_authority_intent() {
    let expected = [
        ("draft", vec!["select", "draft_brief", "handoff"]),
        (
            "process",
            vec!["select", "process_imports", "synthesize", "finalize"],
        ),
        (
            "run",
            vec!["select", "research", "verify", "synthesize", "finalize"],
        ),
        (
            "refresh",
            vec![
                "select",
                "resume",
                "research",
                "verify",
                "synthesize",
                "finalize",
            ],
        ),
        (
            "deepen",
            vec![
                "select",
                "resume",
                "research",
                "verify",
                "synthesize",
                "finalize",
            ],
        ),
    ];
    for (action, steps) in expected {
        let contract = BmadDeepReconSkill
            .execution_contract_for_action(Some(action))
            .expect("source action compiles");
        assert_eq!(
            contract
                .workflow()
                .steps
                .iter()
                .map(|step| step.step_key.as_str())
                .collect::<Vec<_>>(),
            steps
        );
    }
    assert_eq!(
        BmadDeepReconSkill
            .execution_contract()
            .expect("default Run contract")
            .tool_intents(),
        [
            BmadSkillToolIntent::FileRead,
            BmadSkillToolIntent::FileWrite,
            BmadSkillToolIntent::Process,
            BmadSkillToolIntent::Web,
            BmadSkillToolIntent::Subagent,
            BmadSkillToolIntent::Browser,
            BmadSkillToolIntent::ExternalHandoff,
        ]
    );
}
