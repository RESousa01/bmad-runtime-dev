#![allow(clippy::expect_used)]

//! Characterization for MTH-11..MTH-15: the five `3-solutioning` Method-skill
//! adapters (`bmad-agent-architect`, `bmad-architecture`,
//! `bmad-check-implementation-readiness`, `bmad-create-epics-and-stories`,
//! `bmad-generate-project-context`).
//!
//! Every test name begins with `bmad_method_` so a name-filtered test command
//! (`cargo test -p desktop-runtime --test bmad_method_solutioning_skills`)
//! selects them all, matching the convention `bmad_skill_harness.rs` and
//! `bmad_method_analysis_skills.rs` already document.
//!
//! These adapters are not wired into `desktop_runtime`'s public surface yet
//! (`crates/desktop-runtime/src/bmad/skills/mod.rs` and
//! `crates/desktop-runtime/src/bmad/mod.rs` are outside this task's file
//! lease), so the `use desktop_runtime::{BmadAgentArchitectSkill, ...}` below
//! does not resolve until the owning maintainer adds the `mod adapters;` +
//! `pub use adapters::{ ... }` lines reported alongside this task. Until
//! then, this file does not compile; that is the expected, documented state.

use std::{collections::BTreeMap, fs, path::Path};

use desktop_runtime::generated_contracts::BmadInvocationMode;
use desktop_runtime::{BmadMethodSkill, BmadSkillSupportState};

// NOTE: this `use` path assumes the owning maintainer wires
// `crates/desktop-runtime/src/bmad/skills/mod.rs` with:
//   mod adapters;
//   pub use adapters::{
//       BmadAgentArchitectSkill, BmadArchitectureSkill,
//       BmadCheckImplementationReadinessSkill, BmadCreateEpicsAndStoriesSkill,
//       BmadGenerateProjectContextSkill,
//   };
// and that `crates/desktop-runtime/src/bmad/mod.rs`'s existing
// `pub use skills::{ ... }` block is extended to re-export the same five
// names, the same way it already re-exports `BmadMethodSkill` et al. Once
// wired, this import becomes `desktop_runtime::{BmadAgentArchitectSkill, ...}`.
use desktop_runtime::{
    BmadAgentArchitectSkill, BmadArchitectureSkill, BmadCheckImplementationReadinessSkill,
    BmadCreateEpicsAndStoriesSkill, BmadGenerateProjectContextSkill,
};

/// One catalog row's identity fields, read fresh per test run rather than
/// duplicated as a literal -- so a catalog edit that changes a source hash
/// is caught here instead of two independently-drifting copies agreeing by
/// accident.
struct CatalogRow {
    source_sha256_hex: String,
}

fn load_catalog_rows() -> BTreeMap<String, CatalogRow> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/bmad-foundation/capability-catalog.json");
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()));
    let value: serde_json::Value =
        serde_json::from_str(&raw).expect("capability-catalog.json is valid JSON");
    let capabilities = value
        .get("capabilities")
        .and_then(serde_json::Value::as_array)
        .expect("capability-catalog.json has a capabilities array");
    let mut rows = BTreeMap::new();
    for capability in capabilities {
        let row_id = capability
            .get("rowId")
            .and_then(serde_json::Value::as_str)
            .expect("each capability row has a rowId")
            .to_owned();
        let sha256_hex = capability
            .get("source")
            .and_then(|source| source.get("sha256"))
            .and_then(serde_json::Value::as_str)
            .expect("each capability row's source has a sha256")
            .to_owned();
        rows.insert(
            row_id,
            CatalogRow {
                source_sha256_hex: sha256_hex,
            },
        );
    }
    rows
}

fn load_ledger_decisions() -> BTreeMap<String, String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/bmad-foundation/ledger-fragments");
    let mut decisions = BTreeMap::new();
    for entry in fs::read_dir(&path)
        .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()))
    {
        let entry = entry.expect("directory entry reads");
        let path = entry.path();
        if path.extension().and_then(std::ffi::OsStr::to_str) != Some("json") {
            continue;
        }
        let raw = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()));
        let value: serde_json::Value = serde_json::from_str(&raw)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let row_id = value
            .get("rowId")
            .and_then(serde_json::Value::as_str)
            .expect("fragment has rowId")
            .to_owned();
        let support_state = value
            .get("supportState")
            .and_then(serde_json::Value::as_str)
            .expect("fragment has supportState")
            .to_owned();
        decisions.insert(row_id, support_state);
    }
    decisions
}

const ROW_IDS: [&str; 5] = [
    "method:src/bmm-skills/3-solutioning/bmad-agent-architect/SKILL.md",
    "method:src/bmm-skills/3-solutioning/bmad-architecture/SKILL.md",
    "method:src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/SKILL.md",
    "method:src/bmm-skills/3-solutioning/bmad-create-epics-and-stories/SKILL.md",
    "method:src/bmm-skills/3-solutioning/bmad-generate-project-context/SKILL.md",
];

// Break caught: an adapter's `source_sha256` drifting from the pinned
// source's catalog-recorded hash -- silent because both values are typed
// digests that compare equal-looking but were never checked against the one
// authoritative source: capability-catalog.json.
#[test]
fn bmad_method_solutioning_skill_descriptors_match_catalog_source_sha256() {
    let catalog = load_catalog_rows();
    let checks: [(&str, String); 5] = [
        (
            ROW_IDS[0],
            BmadAgentArchitectSkill
                .descriptor()
                .source_sha256()
                .hex_value(),
        ),
        (
            ROW_IDS[1],
            BmadArchitectureSkill
                .descriptor()
                .source_sha256()
                .hex_value(),
        ),
        (
            ROW_IDS[2],
            BmadCheckImplementationReadinessSkill
                .descriptor()
                .source_sha256()
                .hex_value(),
        ),
        (
            ROW_IDS[3],
            BmadCreateEpicsAndStoriesSkill
                .descriptor()
                .source_sha256()
                .hex_value(),
        ),
        (
            ROW_IDS[4],
            BmadGenerateProjectContextSkill
                .descriptor()
                .source_sha256()
                .hex_value(),
        ),
    ];
    for (row_id, descriptor_hex) in checks {
        let catalog_row = catalog
            .get(row_id)
            .unwrap_or_else(|| panic!("{row_id} is present in capability-catalog.json"));
        assert_eq!(
            descriptor_hex, catalog_row.source_sha256_hex,
            "{row_id} descriptor source_sha256 must match the catalog row"
        );
    }
}

// Break caught: a descriptor declaring fewer than both catalog-declared
// invocation modes, which would make the skill uninvocable in the mode Help
// or the runtime expects it to support.
#[test]
fn bmad_method_solutioning_skill_descriptors_declare_both_invocation_modes() {
    let descriptors: [desktop_runtime::BmadSkillDescriptor; 5] = [
        BmadAgentArchitectSkill.descriptor(),
        BmadArchitectureSkill.descriptor(),
        BmadCheckImplementationReadinessSkill.descriptor(),
        BmadCreateEpicsAndStoriesSkill.descriptor(),
        BmadGenerateProjectContextSkill.descriptor(),
    ];
    for descriptor in &descriptors {
        assert!(
            descriptor
                .supported_modes()
                .contains(&BmadInvocationMode::Interactive),
            "{} must declare interactive mode",
            descriptor.canonical_id()
        );
        assert!(
            descriptor
                .supported_modes()
                .contains(&BmadInvocationMode::Headless),
            "{} must declare headless mode",
            descriptor.canonical_id()
        );
    }
}

// Break caught: an adapter's declared_actions silently growing an invented
// action name the pinned source never named. All five 3-solutioning rows
// carry `action: null` in capability-catalog.json (dispatch is menu- or
// step-file-driven, not a fixed enumerable action), so every descriptor here
// must report zero declared actions -- this is the fidelity gap reported to
// the task owner, not weakened by asserting "at least one".
#[test]
fn bmad_method_solutioning_skill_descriptors_declare_no_invented_action() {
    let descriptors: [desktop_runtime::BmadSkillDescriptor; 5] = [
        BmadAgentArchitectSkill.descriptor(),
        BmadArchitectureSkill.descriptor(),
        BmadCheckImplementationReadinessSkill.descriptor(),
        BmadCreateEpicsAndStoriesSkill.descriptor(),
        BmadGenerateProjectContextSkill.descriptor(),
    ];
    for descriptor in &descriptors {
        assert!(
            descriptor.declared_actions().is_empty(),
            "{} must not declare an action the pinned source never named",
            descriptor.canonical_id()
        );
    }
}

// Break caught: a descriptor's support_state disagreeing with the decision
// its own ledger fragment recorded, which would let Help, the runtime, and
// compatibility-ledger.json each report a different answer for "how
// faithful is this adapter" for the same row.
#[test]
fn bmad_method_solutioning_skill_support_state_matches_ledger_decision() {
    let ledger = load_ledger_decisions();
    let checks: [(&str, BmadSkillSupportState); 5] = [
        (
            ROW_IDS[0],
            BmadAgentArchitectSkill.descriptor().support_state(),
        ),
        (
            ROW_IDS[1],
            BmadArchitectureSkill.descriptor().support_state(),
        ),
        (
            ROW_IDS[2],
            BmadCheckImplementationReadinessSkill
                .descriptor()
                .support_state(),
        ),
        (
            ROW_IDS[3],
            BmadCreateEpicsAndStoriesSkill.descriptor().support_state(),
        ),
        (
            ROW_IDS[4],
            BmadGenerateProjectContextSkill.descriptor().support_state(),
        ),
    ];
    for (row_id, support_state) in checks {
        let ledger_support_state = ledger
            .get(row_id)
            .unwrap_or_else(|| panic!("{row_id} has a ledger fragment"));
        let expected = match ledger_support_state.as_str() {
            "exact" => BmadSkillSupportState::Exact,
            "adapted" => BmadSkillSupportState::Adapted,
            "shim" => BmadSkillSupportState::Shim,
            "unavailable" => BmadSkillSupportState::Unavailable,
            other => panic!("{row_id} ledger fragment has an unknown supportState {other:?}"),
        };
        assert!(
            matches!(support_state, state if state == expected),
            "{row_id} descriptor support_state must match its ledger fragment's supportState"
        );
    }
}

/// Reads this task's own adapters directory source, concatenated, for the
/// capability scan below. Mirrors `chat_bmad_capability_separation.rs`,
/// `bmad_skill_harness.rs`, and `bmad_method_analysis_skills.rs`'s own scans.
fn adapters_source() -> String {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bmad/skills/adapters");
    let mut combined = String::new();
    let mut entries: Vec<_> = fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("could not read {}: {error}", dir.display()))
        .map(|entry| entry.expect("directory entry reads").path())
        .filter(|path| path.extension().and_then(std::ffi::OsStr::to_str) == Some("rs"))
        .collect();
    entries.sort();
    for path in entries {
        combined.push_str(&fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!("could not read {}: {error}", path.display());
        }));
        combined.push('\n');
    }
    combined
}

// Break caught: an MTH adapter reaching for a filesystem, process, network,
// or model capability -- turning a "describe this skill" adapter into
// something that could act on its own description, exactly the authority
// boundary `harness.rs`'s own doc comment establishes.
#[test]
fn bmad_method_solutioning_skill_adapters_grant_no_capability() {
    let source = adapters_source();
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
    for (index, line) in source.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("");
        for token in forbidden {
            assert!(
                !code.contains(token),
                "adapters source line {} references a forbidden capability {token:?}: {line:?}",
                index + 1,
            );
        }
    }
}
