//! Deterministic BMAD workflow state.
//!
//! Upstream's `bmad-help` skill resolves the method's config, scans the output
//! locations named in `module-help.csv`, fuzzy-matches the files it finds
//! against each row's `outputs`, and asks a model to infer where the user is.
//! It has to: it is markdown running inside a chat agent, with no code.
//!
//! Forge has code and a governed view of the workspace, so it computes the
//! answer instead. No model call, no receipt, nothing to hallucinate — and the
//! rule is written down here rather than left to a model's judgement.
//!
//! Everything this module reads goes through the same governed workspace
//! broker as any other read; it writes nothing.

use std::collections::{BTreeMap, BTreeSet};

use desktop_runtime::{
    parse_toml_subset, BmadConfigGraph, BmadConfigGraphKind, BmadConfigLayer, BmadConfigResolver,
    BmadWorkflowGraph, BmadWorkflowState, BmadWorkflowStep,
};
use desktop_workspace::WorkspaceBroker;
use serde::Serialize;
use serde_json::Value;

/// The method's config root inside a project, per upstream install layout.
const CONFIG_ROOT: &str = "_bmad";
/// Upstream's four central-config layers, in merge order (lowest first).
const CENTRAL_LAYERS: [(&str, &str); 4] = [
    ("_bmad/config.toml", "installer_team"),
    ("_bmad/config.user.toml", "installer_user"),
    ("_bmad/custom/config.toml", "custom_team"),
    ("_bmad/custom/config.user.toml", "custom_user"),
];
const MAX_CONFIG_BYTES: u64 = 256 * 1024;
/// Directory listing ceiling per scanned output location.
const MAX_SCAN_ENTRIES: usize = 500;
/// Tokens too generic to identify an artifact.
const STOP_WORDS: [&str; 8] = ["and", "the", "for", "with", "a", "an", "of", "to"];

/// What the renderer is told about a project's position in the method.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BmadWorkflowStateProjection {
    pub schema_version: &'static str,
    /// Whether a `_bmad` install was found. Without one there is no method
    /// state to report, and the panel must say so rather than invent a phase.
    pub method_installed: bool,
    pub phase_code: &'static str,
    pub phase_title: &'static str,
    pub steps: Vec<BmadWorkflowStepProjection>,
    pub next_required: Option<String>,
    /// Config variables that a step needs but the resolved config does not
    /// define, so the reader can see *why* a step's state is unknown.
    pub unresolved_locations: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BmadWorkflowStepProjection {
    pub key: String,
    pub display_name: String,
    pub menu_code: String,
    pub phase_code: &'static str,
    pub required: bool,
    pub completed: bool,
    /// Ordering predecessors are satisfied and the step is not complete.
    pub available: bool,
    /// The artifact that evidenced completion, when one did.
    pub evidence_path: Option<String>,
}

/// Resolved method configuration for one workspace.
pub(crate) struct BmadProjectConfig {
    pub installed: bool,
    pub values: BTreeMap<String, String>,
}

impl BmadProjectConfig {
    /// Resolves a config variable to a workspace-relative directory.
    ///
    /// Upstream values often carry `{project-root}`; that prefix is stripped
    /// because every path Forge uses is already workspace-relative.
    fn location(&self, name: &str) -> Option<String> {
        // The CSV uses both spellings for the same idea.
        let candidates = [
            name.to_owned(),
            name.replace('-', "_"),
            name.replace('_', "-"),
        ];
        let raw = candidates
            .iter()
            .find_map(|candidate| self.values.get(candidate))?;
        let trimmed = raw
            .trim()
            .trim_start_matches("{project-root}")
            .trim_start_matches('/')
            .trim_end_matches('/');
        (!trimmed.is_empty()).then(|| trimmed.to_owned())
    }
}

/// Reads and merges the four central config layers through the reviewed
/// resolver. A layer that fails to parse is recorded as *invalid* rather than
/// partially accepted, which is what makes a malformed override fail closed.
pub(crate) fn resolve_project_config(
    workspace: &WorkspaceBroker,
    workspace_id: &str,
) -> BmadProjectConfig {
    let mut layers = Vec::new();
    let mut installed = false;
    for (ordinal, (path, layer_kind)) in CENTRAL_LAYERS.into_iter().enumerate() {
        let ordinal = u8::try_from(ordinal).unwrap_or(0);
        let Ok(preview) = workspace.read_text(workspace_id, path, MAX_CONFIG_BYTES) else {
            continue;
        };
        installed = true;
        let layer = match parse_toml_subset(&preview.content) {
            Ok(value) => BmadConfigLayer::valid(
                BmadConfigGraphKind::MethodCentralToml,
                layer_kind,
                ordinal,
                false,
                value,
            ),
            Err(_) => BmadConfigLayer::invalid(
                BmadConfigGraphKind::MethodCentralToml,
                layer_kind,
                ordinal,
                false,
                "config layer is not within the supported TOML subset",
            ),
        };
        if let Ok(layer) = layer {
            layers.push(layer);
        }
    }
    if !installed {
        // A `_bmad` directory alone still signals an install, even with no
        // config layers written yet.
        installed = workspace
            .list_entries(workspace_id, CONFIG_ROOT, 1)
            .is_ok_and(|entries| !entries.is_empty());
    }

    let values = BmadConfigGraph::new(BmadConfigGraphKind::MethodCentralToml, layers)
        .ok()
        .and_then(|graph| BmadConfigResolver::resolve_graph(&graph).ok())
        .map(|resolution| flatten_scalars(&resolution.value))
        .unwrap_or_default();
    BmadProjectConfig { installed, values }
}

/// Flattens the merged config to `name -> scalar`, keeping both the leaf name
/// and its dotted path so `modules.bmm.planning_artifacts` is reachable as
/// `planning_artifacts` too — which is how the CSV names it.
fn flatten_scalars(value: &Value) -> BTreeMap<String, String> {
    let mut flat = BTreeMap::new();
    flatten_into("", value, &mut flat);
    flat
}

fn flatten_into(prefix: &str, value: &Value, flat: &mut BTreeMap<String, String>) {
    match value {
        Value::Object(entries) => {
            for (key, child) in entries {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten_into(&path, child, flat);
            }
        }
        Value::String(text) => {
            let leaf = prefix.rsplit('.').next().unwrap_or(prefix).to_owned();
            flat.entry(leaf).or_insert_with(|| text.clone());
            flat.insert(prefix.to_owned(), text.clone());
        }
        _ => {}
    }
}

/// Normalizes a name for matching: lowercase, non-alphanumeric collapsed to a
/// single separator.
fn normalize(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut separated = true;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            out.push(character.to_ascii_lowercase());
            separated = false;
        } else if !separated {
            out.push('-');
            separated = true;
        }
    }
    out.trim_matches('-').to_owned()
}

/// The tokens a file name must contain to evidence a step. Derived from the
/// row's `outputs` description; stop-words and very short tokens are dropped
/// because they identify nothing.
fn evidence_tokens(step: &BmadWorkflowStep) -> Vec<String> {
    let source = step.outputs.as_deref().unwrap_or_default();
    normalize(source)
        .split('-')
        .filter(|token| token.len() >= 3 && !STOP_WORDS.contains(token))
        .map(str::to_owned)
        .collect()
}

/// Scans each distinct output location once and returns the normalized file
/// names found there.
/// One scanned output location: `(normalized file name, workspace path)`.
type ScannedFile = (String, String);

struct ScannedLocations {
    files: BTreeMap<String, Vec<ScannedFile>>,
    unresolved: Vec<String>,
}

fn scan_locations(
    workspace: &WorkspaceBroker,
    workspace_id: &str,
    graph: &BmadWorkflowGraph,
    config: &BmadProjectConfig,
) -> ScannedLocations {
    let mut scanned: BTreeMap<String, Vec<ScannedFile>> = BTreeMap::new();
    let mut unresolved = BTreeSet::new();
    for name in graph.steps_by_output_location().keys() {
        let Some(directory) = config.location(name) else {
            unresolved.insert(name.clone());
            continue;
        };
        if scanned.contains_key(name) {
            continue;
        }
        let entries = workspace
            .list_entries(workspace_id, &directory, MAX_SCAN_ENTRIES)
            .unwrap_or_default();
        let files = entries
            .into_iter()
            .map(|entry| {
                // The broker returns workspace-relative paths; match on the
                // final segment so a directory name cannot satisfy a token.
                let leaf = entry
                    .relative_path
                    .rsplit('/')
                    .next()
                    .unwrap_or(&entry.relative_path)
                    .to_owned();
                (normalize(&leaf), entry.relative_path)
            })
            .collect::<Vec<_>>();
        scanned.insert(name.clone(), files);
    }
    ScannedLocations {
        files: scanned,
        unresolved: unresolved.into_iter().collect(),
    }
}

/// Computes the project's position in the method.
pub(crate) fn compute_workflow_state(
    workspace: &WorkspaceBroker,
    workspace_id: &str,
    graph: &BmadWorkflowGraph,
) -> BmadWorkflowStateProjection {
    let config = resolve_project_config(workspace, workspace_id);
    let scanned = scan_locations(workspace, workspace_id, graph, &config);

    let mut evidence: BTreeMap<String, String> = BTreeMap::new();
    for step in graph.steps() {
        let tokens = evidence_tokens(step);
        if tokens.is_empty() {
            // Without an artifact description there is nothing to detect, so
            // the step stays incomplete rather than being guessed complete.
            continue;
        }
        for location in &step.output_location {
            let Some(files) = scanned.files.get(location) else {
                continue;
            };
            if let Some((_, path)) = files
                .iter()
                .find(|(name, _)| tokens.iter().all(|token| name.contains(token.as_str())))
            {
                evidence.insert(step.key.clone(), path.clone());
                break;
            }
        }
    }

    let completed = evidence.keys().cloned().collect::<BTreeSet<_>>();
    let state: BmadWorkflowState = graph.state(&completed);
    let available = state.available.iter().cloned().collect::<BTreeSet<_>>();

    BmadWorkflowStateProjection {
        schema_version: "sapphirus.bmad-workflow-state.v1",
        method_installed: config.installed,
        phase_code: state.phase.code(),
        phase_title: state.phase.title(),
        steps: graph
            .steps()
            .iter()
            .map(|step| BmadWorkflowStepProjection {
                key: step.key.clone(),
                display_name: step.display_name.clone(),
                menu_code: step.menu_code.clone(),
                phase_code: step.phase.code(),
                required: step.required,
                completed: completed.contains(&step.key),
                available: available.contains(&step.key),
                evidence_path: evidence.get(&step.key).cloned(),
            })
            .collect(),
        next_required: state.next_required,
        unresolved_locations: scanned.unresolved,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{evidence_tokens, flatten_scalars, normalize, BmadProjectConfig};
    use desktop_runtime::BmadWorkflowGraph;
    use std::collections::BTreeMap;

    const GRAPH: &str = include_str!(
        "../../../packages/bmad-foundation/source/method/6.10.0/src/bmm-skills/module-help.csv"
    );

    fn graph() -> BmadWorkflowGraph {
        BmadWorkflowGraph::parse(GRAPH).expect("sealed method graph")
    }

    #[test]
    fn artifact_names_normalize_for_matching() {
        assert_eq!(normalize("ARCHITECTURE-SPINE.md"), "architecture-spine-md");
        assert_eq!(
            normalize("prd-foo-bar-2026-05-23"),
            "prd-foo-bar-2026-05-23"
        );
        assert_eq!(normalize("  Epics & Stories  "), "epics-stories");
    }

    #[test]
    fn evidence_tokens_identify_the_artifact_not_the_english() {
        let graph = graph();
        let tokens = |key: &str| evidence_tokens(graph.step(key).expect("step"));

        assert_eq!(tokens("bmad-architecture"), ["architecture"]);
        assert_eq!(tokens("bmad-prd"), ["prd"]);
        // Stop-words are dropped; the identifying nouns remain.
        assert_eq!(
            tokens("bmad-create-epics-and-stories"),
            ["epics", "stories"]
        );
        // A row with no declared artifact yields nothing to detect.
        assert!(tokens("bmad-dev-story").is_empty());
    }

    #[test]
    fn a_real_artifact_name_matches_its_step_tokens() {
        let graph = graph();
        let matches = |key: &str, file: &str| {
            let name = normalize(file);
            evidence_tokens(graph.step(key).expect("step"))
                .iter()
                .all(|token| name.contains(token.as_str()))
        };

        assert!(matches("bmad-architecture", "ARCHITECTURE-SPINE.md"));
        assert!(matches("bmad-prd", "prd-checkout-2026-05-23.md"));
        assert!(matches(
            "bmad-create-epics-and-stories",
            "epics-and-stories.md"
        ));
        // A near-miss must not count: one token missing is not evidence.
        assert!(!matches("bmad-create-epics-and-stories", "epics.md"));
        assert!(!matches("bmad-architecture", "README.md"));
    }

    #[test]
    fn config_locations_resolve_through_both_spellings_and_strip_the_project_root() {
        let config = BmadProjectConfig {
            installed: true,
            values: BTreeMap::from([
                (
                    "planning_artifacts".to_owned(),
                    "{project-root}/docs/plan/".to_owned(),
                ),
                ("project_knowledge".to_owned(), "docs/knowledge".to_owned()),
            ]),
        };

        assert_eq!(
            config.location("planning_artifacts").as_deref(),
            Some("docs/plan")
        );
        // The CSV writes `project-knowledge` for the same variable.
        assert_eq!(
            config.location("project-knowledge").as_deref(),
            Some("docs/knowledge")
        );
        assert_eq!(config.location("implementation_artifacts"), None);
    }

    #[test]
    fn nested_config_is_reachable_by_leaf_name_and_by_path() {
        let value = serde_json::json!({
            "core": { "communication_language": "en" },
            "modules": { "bmm": { "planning_artifacts": "docs/plan" } },
        });

        let flat = flatten_scalars(&value);

        assert_eq!(
            flat.get("planning_artifacts").map(String::as_str),
            Some("docs/plan")
        );
        assert_eq!(
            flat.get("modules.bmm.planning_artifacts")
                .map(String::as_str),
            Some("docs/plan")
        );
        assert_eq!(
            flat.get("core.communication_language").map(String::as_str),
            Some("en")
        );
    }
}
