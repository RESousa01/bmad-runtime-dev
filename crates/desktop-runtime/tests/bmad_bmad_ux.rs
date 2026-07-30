#![allow(clippy::expect_used)]

//! MTH-10 source-closure and runtime-reachability characterization.

use std::collections::BTreeMap;

use desktop_runtime::{BmadMethodSkill, BmadUxSkill};

const ROW_ID: &str = "method:src/bmm-skills/2-plan-workflows/bmad-ux/SKILL.md";
const PINNED_PREFIX: &str = "src/bmm-skills/2-plan-workflows/bmad-ux/";

// Break caught: a source resource disappearing from the thin adapter means
// an execution host could prepare a UX run without a source-pinned artifact.
// The expected members come from the live sealed source manifest, avoiding a
// second independently-maintained inventory in this test.
#[test]
fn bmad_ux_descriptor_closes_the_entire_pinned_skill_subtree() {
    let manifest_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/bmad-foundation/source-manifest.json");
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&manifest_path)
            .unwrap_or_else(|error| panic!("could not read {}: {error}", manifest_path.display())),
    )
    .expect("sealed source manifest is JSON");
    let expected = manifest["members"]
        .as_array()
        .expect("sealed source manifest has members")
        .iter()
        .filter(|member| member["source_id"] == "method")
        .filter_map(|member| {
            let path = member["path"].as_str()?;
            let relative = path.strip_prefix(PINNED_PREFIX)?;
            Some((relative, member["sha256"].as_str()?.to_owned()))
        })
        .collect::<BTreeMap<_, _>>();
    let descriptor = BmadUxSkill.descriptor();
    let actual = descriptor
        .declared_resources()
        .iter()
        .map(|resource| {
            let relative = resource
                .path()
                .strip_prefix(PINNED_PREFIX)
                .expect("each UX resource remains under its pinned skill root");
            (relative, resource.content_hash().hex_value())
        })
        .collect::<BTreeMap<_, _>>();

    assert_eq!(
        actual, expected,
        "{ROW_ID} must declare every sealed source resource exactly once"
    );
}

// Break caught: direct I/O, process, network, model, or action-plan access
// in the adapter would let source prose bypass the generic runtime's ports.
#[test]
fn bmad_ux_adapter_is_a_descriptor_only_authority_boundary() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/bmad/skills/adapters/bmad_ux.rs");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()));
    for (index, line) in source.lines().enumerate() {
        let code = line.split("//").next().unwrap_or_default();
        for token in [
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
        ] {
            assert!(
                !code.contains(token),
                "bmad_ux.rs line {} grants {token:?}",
                index + 1
            );
        }
    }
}

// Break caught: one mode producing only DESIGN.md while the other produced a
// spine pair would make mode selection change the UX contract. These are
// source-derived conformance fixtures, consumed here rather than merely
// existing as directory markers.
#[test]
fn bmad_ux_interactive_and_headless_fixtures_require_the_same_spine_pair() {
    let fixture_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../packages/bmad-foundation/tests/fixtures/method__src__bmm-skills__2-plan-workflows__bmad-ux__SKILL.md",
    );
    let artifacts_for = |mode: &str| {
        let path = fixture_root.join(mode).join("run.json");
        let value: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display())),
        )
        .expect("UX fixture is JSON");
        value["outputs"]["artifacts"]
            .as_array()
            .expect("fixture declares every required UX artifact")
            .iter()
            .map(|artifact| {
                artifact["path"]
                    .as_str()
                    .expect("artifact has path")
                    .to_owned()
            })
            .collect::<Vec<_>>()
    };
    let expected = vec![
        "{doc_workspace}/DESIGN.md".to_owned(),
        "{doc_workspace}/EXPERIENCE.md".to_owned(),
    ];
    assert_eq!(artifacts_for("interactive"), expected);
    assert_eq!(artifacts_for("headless"), expected);
}

// Break caught: treating the descriptor as an executable Create/Update/
// Validate UX workflow when the host port cannot accept a descriptor plus
// source closure, resolved config/customization, and a mode-specific run
// request. That missing binding prevents governed artifact persistence,
// reviewer/finalization plans, and durable recovery from being real behavior.
#[test]
fn bmad_ux_requires_a_descriptor_bound_generic_run_interface() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bmad/mod.rs");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()));
    let trait_source = source
        .split_once("pub trait BmadRuntimePort")
        .and_then(|(_, tail)| tail.split_once("pub struct BmadDomainRuntime"))
        .map_or("", |(trait_source, _)| trait_source);

    assert!(
        trait_source.contains("BmadSkillDescriptor"),
        "MTH-10 BLOCKED: BmadRuntimePort exposes only prebuilt session/artifact intents. Add a descriptor-bound generic run request carrying BmadSkillDescriptor, resolved source/config/customization identities, mode and intent, typed ports/Airlock, and durable recovery context."
    );
}
