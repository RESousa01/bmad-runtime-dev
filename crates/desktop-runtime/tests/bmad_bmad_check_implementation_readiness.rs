//! Source-derived conformance tests for `bmad-check-implementation-readiness`.

#![allow(clippy::expect_used, clippy::panic)]

use std::{collections::BTreeMap, path::Path};

use desktop_runtime::{
    generated_contracts::{
        BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationMode,
    },
    sha256_bytes, BmadCheckImplementationReadinessSkill, BmadMethodSkill, BmadSkillSupportState,
};

const SKILL_ROOT: &str = "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness";

fn expected_pinned_subtree() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        (
            "SKILL.md",
            "a7e12eae2bdb5a95cc0534e6d5bf58e7ce4b947327a494d475ef98e67f03718a",
        ),
        (
            "customize.toml",
            "571d81087b7a6d914cf7c2f21aa26354eac385bb2f5cfeabc591892cbb98d036",
        ),
        (
            "steps/step-01-document-discovery.md",
            "c763b512d55906122433cb65c1bcd5b5b283e45eacdc07281c8ec7596b6b3980",
        ),
        (
            "steps/step-02-prd-analysis.md",
            "38be2bf4b924c0b5b395b57d68f685d790ade7b1a6c10993d3c550675f87d954",
        ),
        (
            "steps/step-03-epic-coverage-validation.md",
            "7b187f03a47cba0325fcfd10240410db9c59d93768342fc2dd3de2a01ec23356",
        ),
        (
            "steps/step-04-ux-alignment.md",
            "f71e5f0d77615e885ae40fdee6b04c1dd6e472c871f87b515fe869cb5f6966fb",
        ),
        (
            "steps/step-05-epic-quality-review.md",
            "d8a84e57f4e3a321734b5b5d093458ceb1e338744f18954c5a204f5ce3576185",
        ),
        (
            "steps/step-06-final-assessment.md",
            "1b9c8c2a21950f85e052e05309bfcb3bfe38157ba6fca402ccf37444e7f1cee3",
        ),
        (
            "templates/readiness-report-template.md",
            "0da97ab1e38818e642f36dc0ef24d2dae69fc6e0be59924dc2dbf44329738ff6",
        ),
    ])
}

// Break caught: omitting any entrypoint, configuration, template, or required
// sequential step makes the generic runtime unable to establish an immutable
// source boundary for this micro-file workflow.
#[test]
fn bmad_check_implementation_readiness_closes_the_exact_pinned_subtree() {
    let descriptor = BmadCheckImplementationReadinessSkill.descriptor();
    let actual = descriptor
        .declared_resources()
        .iter()
        .map(|resource| {
            let relative = resource
                .path()
                .strip_prefix(&format!("{SKILL_ROOT}/"))
                .expect("each declared resource remains under the pinned skill root");
            (relative.to_owned(), resource.content_hash().hex_value())
        })
        .collect::<BTreeMap<_, _>>();
    let expected = expected_pinned_subtree()
        .into_iter()
        .map(|(path, hash)| (path.to_owned(), hash.to_owned()))
        .collect::<BTreeMap<_, _>>();

    assert_eq!(
        actual, expected,
        "the descriptor must declare every pinned resource once"
    );

    for resource in descriptor.declared_resources() {
        let source_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/bmad-foundation/source/method/6.10.0")
            .join(resource.path());
        let bytes = std::fs::read(&source_path)
            .unwrap_or_else(|error| panic!("could not read {}: {error}", source_path.display()));
        assert_eq!(
            resource.content_hash(),
            sha256_bytes(&bytes),
            "{} must retain its pinned SHA-256 identity",
            resource.path()
        );
    }
}

// Break caught: changing the declared surface would silently erase the source
// workflow's user-gated, persisted readiness-report outcome.
#[test]
fn bmad_check_implementation_readiness_preserves_its_source_derived_contract() {
    let descriptor = BmadCheckImplementationReadinessSkill.descriptor();

    assert_eq!(
        descriptor.canonical_id(),
        "bmm:bmad-check-implementation-readiness"
    );
    assert_eq!(descriptor.source_path(), format!("{SKILL_ROOT}/SKILL.md"));
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
    assert_eq!(descriptor.output_locations(), ["planning_artifacts"]);
    assert_eq!(descriptor.artifact_kinds(), [BmadArtifactKind::Document]);
    assert_eq!(
        descriptor.completion_hooks(),
        [BmadArtifactCompletionHookRunHook::PersistArtifacts]
    );
    assert_eq!(descriptor.support_state(), BmadSkillSupportState::Exact);
}

// Break caught: direct file, process, network, model, action-plan, or artifact
// authority in this adapter would let source prose bypass the generic runtime's
// typed execution ports and durable governance boundary.
#[test]
fn bmad_check_implementation_readiness_adapter_is_descriptor_only() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/bmad/skills/adapters/bmad_check_implementation_readiness.rs"),
    )
    .expect("readiness adapter source");
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
                "adapter line {} references forbidden capability {token:?}: {line:?}",
                index + 1,
            );
        }
    }
}

// RED reachability check, intentionally left failing until the shared
// Method-session bridge lands. The generic runtime already owns durable
// ordering, recovery, and authority validation in `MethodSession`, but this
// adapter contract currently exposes only `descriptor()`. Consequently no
// sealed source-specific `MethodStepTable`/`MethodExactBinding` can be
// prepared for the six source steps through a public execution boundary.
//
// The bridge must replace this source-level assertion with an executable
// source-derived 01..06 session test; do not paper over the absence by
// constructing an unrelated test-only binding here.
#[test]
fn bmad_check_implementation_readiness_reaches_the_generic_method_session_bridge() {
    let harness = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bmad/skills/harness.rs"),
    )
    .expect("method-skill harness source");
    assert!(
        harness.contains("MethodStepTable"),
        "blocked: BmadMethodSkill exposes only descriptor(); add the sealed generic prepare boundary that binds this source workflow to MethodExactBinding + MethodStepTable + MethodSession"
    );
}
