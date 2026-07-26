//! Executing the read-only tools a chat turn may ask for (ADR-0009, Phase 1).
//!
//! This is the point where a model's *request* becomes a real filesystem read,
//! so it is where the two layers ADR-0009 relies on meet:
//!
//! * [`desktop_tools::evaluate`] decides whether the call runs at all. Anything
//!   without an explicit `Allow` asks a human — and because Phase 1 has no
//!   consent surface yet, "asks a human" is implemented as *refuse*. Failing
//!   closed while the dock is missing is the only honest option; the
//!   alternative is a prompt nobody can answer, or worse, a silent allow.
//! * [`WorkspaceBroker`] bounds what an allowed call can reach. Every tool here
//!   goes through it, so `is_sensitive_relative_path` applies structurally and
//!   a granted `glob` still cannot enumerate `.env`.
//!
//! Everything returned is **data, never instruction**. The rendering below is
//! deliberately plain and carries no framing a model could mistake for a
//! directive; the provenance fence around reviewed context does the rest.

use desktop_tools::{default_rules, evaluate, GrantedRules, PermissionAction};
use desktop_workspace::{GrepRequest, WorkspaceBroker, WorkspaceError};

/// The tools a chat turn may ask for in this phase.
///
/// The same four names `run_tool` implements below. One list, so the set a
/// turn is *told about* and the set the host can actually run stay the same
/// set.
const CHAT_TURN_TOOLS: [&str; 4] = ["read", "list", "glob", "grep"];

/// What the grant is evaluated against. The question is whether a tool is
/// reachable at all, not whether one particular path is — every tool above
/// takes a workspace-relative path or a pattern, and an ordinary relative path
/// is representative of both without matching the secret-path rules that
/// narrow `read`.
const GRANT_PROBE_SUBJECT: &str = "README.md";

/// Whether this turn is granted tools, and therefore whether it is sealed
/// under — and prompted with — the tool-bearing reply schema.
///
/// Evaluated against the rules actually in force rather than assumed. All four
/// tools are `Allow` in [`default_rules`], so today this is always true; the
/// point of computing it is that a deployment which withdraws them stops
/// advertising them in the same breath. The v1 system prompt promises the model
/// *"you cannot read, write, or execute anything"*, and that sentence has to
/// stay true whenever it is the one sent.
pub(crate) fn chat_turn_tools_granted(granted: &GrantedRules) -> bool {
    let defaults = default_rules();
    CHAT_TURN_TOOLS.iter().any(|tool| {
        evaluate(tool, GRANT_PROBE_SUBJECT, &[&defaults, granted.rules()])
            == PermissionAction::Allow
    })
}

/// Ceiling on one tool result entering the transcript.
///
/// Smaller than the broker's own read ceiling on purpose: a result is re-sent
/// on every later iteration of the loop, so an oversized one is paid for many
/// times over.
const MAX_TOOL_RESULT_BYTES: usize = 8 * 1024;
/// Results per listing tool. Enough to be useful, small enough to stay legible.
const MAX_TOOL_RESULT_ROWS: usize = 100;

// Not `Clone`/`Eq`: `WorkspaceError` wraps `std::io::Error`, which is neither.
#[derive(Debug)]
pub(crate) enum ToolRunError {
    /// No rule allows this call, so a human would have to be asked — and in
    /// Phase 1 there is nobody to ask.
    NotPermitted,
    /// The tool name is not one this phase implements.
    UnknownTool,
    /// The broker refused: bad path, blocked path, revoked grant, bad pattern.
    /// The inner error is carried for the `Debug` rendering the model is shown.
    Workspace(#[allow(dead_code)] WorkspaceError),
}

impl From<WorkspaceError> for ToolRunError {
    fn from(error: WorkspaceError) -> Self {
        Self::Workspace(error)
    }
}

/// Runs one tool call and renders its result for the transcript.
///
/// The permission gate is consulted *before* the broker, so a refused call
/// never touches the filesystem at all.
pub(crate) fn run_tool(
    broker: &WorkspaceBroker,
    granted: &GrantedRules,
    workspace_id: &str,
    tool: &str,
    subject: &str,
) -> Result<String, ToolRunError> {
    let defaults = default_rules();
    if evaluate(tool, subject, &[&defaults, granted.rules()]) != PermissionAction::Allow {
        return Err(ToolRunError::NotPermitted);
    }
    let rendered = match tool {
        "read" => {
            let preview = broker.read_text(
                workspace_id,
                subject,
                MAX_TOOL_RESULT_BYTES.try_into().unwrap_or(u64::MAX),
            )?;
            // The broker already tells us when it cut the file short. Passing
            // that through is the difference between the model knowing it saw
            // part of a file and believing it saw all of one.
            if preview.truncated {
                format!("{}\n… (truncated)", preview.content)
            } else {
                preview.content
            }
        }
        "list" => {
            let mut rows: Vec<String> = broker
                .list_entries(workspace_id, subject, MAX_TOOL_RESULT_ROWS)?
                .iter()
                .map(|entry| entry.relative_path.clone())
                .collect();
            rows.sort();
            rows.join("\n")
        }
        "glob" => broker
            .glob(workspace_id, subject, MAX_TOOL_RESULT_ROWS)?
            .join("\n"),
        "grep" => broker
            .grep(&GrepRequest {
                workspace_id,
                pattern: subject,
                path_scope: None,
                case_sensitive: false,
                max_results: MAX_TOOL_RESULT_ROWS,
            })?
            .iter()
            .map(|hit| format!("{}:{}: {}", hit.relative_path, hit.line, hit.preview))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => return Err(ToolRunError::UnknownTool),
    };
    Ok(bound_result(rendered))
}

/// Truncates honestly: a silently shortened result would have the model reason
/// about a file it believes it read in full.
fn bound_result(mut rendered: String) -> String {
    if rendered.trim().is_empty() {
        return "(no matches)".to_owned();
    }
    if rendered.len() > MAX_TOOL_RESULT_BYTES {
        let mut cut = MAX_TOOL_RESULT_BYTES;
        while cut > 0 && !rendered.is_char_boundary(cut) {
            cut -= 1;
        }
        rendered.truncate(cut);
        rendered.push_str("\n… (truncated)");
    }
    rendered
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{
        chat_turn_tools_granted, run_tool, ToolRunError, CHAT_TURN_TOOLS, MAX_TOOL_RESULT_BYTES,
    };
    use desktop_tools::GrantedRules;
    use desktop_workspace::WorkspaceBroker;

    fn workspace() -> (tempfile::TempDir, WorkspaceBroker, String) {
        let temp = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(temp.path().join("src")).expect("src");
        std::fs::write(temp.path().join("src/main.rs"), "fn main() {}\n").expect("main");
        std::fs::write(temp.path().join("README.md"), "hello prose\n").expect("readme");
        // Not credential-shaped on purpose: the path is what the broker
        // excludes on, and a realistic secret would trip the repo's scanner.
        std::fs::write(temp.path().join(".env"), "sentinel\n").expect("env");
        let broker = WorkspaceBroker::new();
        let projection = broker.grant("project_tools", temp.path()).expect("grant");
        let id = projection.workspace_id;
        (temp, broker, id)
    }

    #[test]
    fn the_four_read_only_tools_run_without_asking_anyone() {
        let (_temp, broker, id) = workspace();
        let granted = GrantedRules::new();

        let read = run_tool(&broker, &granted, &id, "read", "src/main.rs").expect("read");
        assert!(read.contains("fn main()"));

        let globbed = run_tool(&broker, &granted, &id, "glob", "**/*.rs").expect("glob");
        assert!(globbed.contains("src/main.rs"));

        let grepped = run_tool(&broker, &granted, &id, "grep", "prose").expect("grep");
        assert!(grepped.contains("README.md"));
    }

    /// The grant decides the reply schema, which decides the system prompt.
    /// Every tool this phase implements must be one the default rules allow,
    /// or a turn would be prompted with a tool it cannot run.
    #[test]
    fn the_phase_tool_set_is_granted_by_the_default_rules() {
        assert!(chat_turn_tools_granted(&GrantedRules::new()));
        let (_temp, broker, id) = workspace();
        for tool in CHAT_TURN_TOOLS {
            let subject = if tool == "list" { "." } else { "README.md" };
            assert!(
                run_tool(&broker, &GrantedRules::new(), &id, tool, subject).is_ok(),
                "the turn is told about {tool}, so {tool} must actually run",
            );
        }
    }

    #[test]
    fn a_tool_with_no_allow_rule_is_refused_rather_than_silently_run() {
        let (_temp, broker, id) = workspace();
        let granted = GrantedRules::new();
        // `shell` has no Allow rule, so it resolves to Ask — and Phase 1 has no
        // consent surface, so Ask must mean refuse.
        assert!(matches!(
            run_tool(&broker, &granted, &id, "shell", "dir"),
            Err(ToolRunError::NotPermitted)
        ));
        assert!(matches!(
            run_tool(&broker, &granted, &id, "write", "src/main.rs"),
            Err(ToolRunError::NotPermitted)
        ));
    }

    #[test]
    fn reading_a_secret_is_refused_by_the_permission_gate_before_the_broker() {
        let (_temp, broker, id) = workspace();
        let granted = GrantedRules::new();
        // `read` is Allow, but `*.env` is Ask — so this never reaches the disk.
        assert!(matches!(
            run_tool(&broker, &granted, &id, "read", ".env"),
            Err(ToolRunError::NotPermitted)
        ));
    }

    #[test]
    fn a_permitted_pattern_still_cannot_enumerate_secrets() {
        let (_temp, broker, id) = workspace();
        let granted = GrantedRules::new();
        // `glob` is Allow with no secret exclusion of its own; the broker's
        // structural policy is what keeps `.env` out of the answer.
        let globbed = run_tool(&broker, &granted, &id, "glob", "**/*").expect("glob");
        assert!(!globbed.contains(".env"), "glob leaked a secret: {globbed}");

        let grepped = run_tool(&broker, &granted, &id, "grep", "sentinel").expect("grep");
        assert_eq!(grepped, "(no matches)");
    }

    #[test]
    fn an_oversized_result_says_that_it_was_truncated() {
        let temp = tempfile::tempdir().expect("temp dir");
        let big = "x".repeat(MAX_TOOL_RESULT_BYTES * 2);
        std::fs::write(temp.path().join("big.txt"), &big).expect("big");
        let broker = WorkspaceBroker::new();
        let id = broker
            .grant("project_big", temp.path())
            .expect("grant")
            .workspace_id;

        // The broker refuses beyond its own ceiling, which is itself a bound.
        // Either outcome is acceptable; silently returning a partial file as
        // though whole is not.
        let outcome = run_tool(&broker, &GrantedRules::new(), &id, "read", "big.txt");
        let honest = match &outcome {
            Ok(rendered) => rendered.ends_with("… (truncated)"),
            Err(ToolRunError::Workspace(_)) => true,
            Err(_) => false,
        };
        assert!(
            honest,
            "a partial file must not be presented as whole: {outcome:?}"
        );
    }
}
