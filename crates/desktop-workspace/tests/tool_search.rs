//! `glob` and `grep` — the two tools ADR-0009's `default_rules()` already
//! grants `Allow` but which did not exist until now.
//!
//! The load-bearing tests here are the ones asserting that a pattern cannot
//! reach a secret. `default_rules()` allows these tools unconditionally and
//! with no secret exclusion of its own, so the *only* thing standing between a
//! model and `.env` is the broker applying `is_sensitive_relative_path`
//! structurally. If these tests pass while the rest fail, the security property
//! still holds; if these fail, the permission defaults are actively unsafe.

use desktop_workspace::{GrepRequest, WorkspaceBroker, WorkspaceError};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// A workspace holding ordinary source, plus two files a model must never see.
fn seeded_workspace() -> TestResult<tempfile::TempDir> {
    let temp = tempfile::tempdir()?;
    std::fs::create_dir_all(temp.path().join("src"))?;
    std::fs::create_dir_all(temp.path().join("docs"))?;
    std::fs::create_dir_all(temp.path().join(".ssh"))?;

    std::fs::write(temp.path().join("README.md"), "# Title\nplain prose\n")?;
    std::fs::write(temp.path().join("docs/guide.md"), "nested prose\n")?;
    std::fs::write(
        temp.path().join("src/main.rs"),
        "fn main() {\n    let token = 1;\n}\n",
    )?;
    std::fs::write(temp.path().join("src/util.rs"), "fn helper() {}\n")?;

    // The bait: both hold a marker the tests search for by name. It is
    // deliberately not shaped like a real credential assignment — the path is
    // what drives the exclusion, and a realistic-looking secret here would only
    // trip the repository's own secret scanner.
    std::fs::write(temp.path().join(".env"), "hunter2-sentinel\n")?;
    std::fs::write(temp.path().join(".ssh/id_rsa"), "hunter2-sentinel\n")?;
    Ok(temp)
}

#[test]
fn glob_matches_paths_and_keeps_star_inside_one_segment() -> TestResult {
    let temp = seeded_workspace()?;
    let broker = WorkspaceBroker::new();
    let projection = broker.grant("project_forge_glob", temp.path())?;

    let top_level = broker.glob(&projection.workspace_id, "*.md", 100)?;
    assert_eq!(
        top_level,
        vec!["README.md".to_owned()],
        "a single star must not cross a path separator"
    );

    let mut recursive = broker.glob(&projection.workspace_id, "**/*.md", 100)?;
    recursive.sort();
    assert_eq!(
        recursive,
        vec!["README.md".to_owned(), "docs/guide.md".to_owned()],
        "a double star must cross separators"
    );

    let mut rust = broker.glob(&projection.workspace_id, "src/*.rs", 100)?;
    rust.sort();
    assert_eq!(
        rust,
        vec!["src/main.rs".to_owned(), "src/util.rs".to_owned()]
    );
    Ok(())
}

#[test]
fn glob_cannot_enumerate_secrets_even_when_asked_directly() -> TestResult {
    let temp = seeded_workspace()?;
    let broker = WorkspaceBroker::new();
    let projection = broker.grant("project_forge_glob_secrets", temp.path())?;

    // Exclusion is the broker's, not the pattern's: naming the secret
    // explicitly must still return nothing.
    for pattern in ["**/.env", ".env", "**/id_rsa", ".ssh/*", "**/*"] {
        let found = broker.glob(&projection.workspace_id, pattern, 1_000)?;
        assert!(
            !found.iter().any(|path| path.contains(".env")),
            "pattern {pattern:?} leaked .env: {found:?}"
        );
        assert!(
            !found.iter().any(|path| path.contains("id_rsa")),
            "pattern {pattern:?} leaked id_rsa: {found:?}"
        );
    }

    // The catch-all still returns the ordinary files, so the assertion above is
    // meaningful rather than vacuously true on an empty result.
    let everything = broker.glob(&projection.workspace_id, "**/*", 1_000)?;
    assert!(everything.contains(&"README.md".to_owned()));
    assert!(everything.contains(&"src/main.rs".to_owned()));
    Ok(())
}

#[test]
fn grep_matches_a_real_expression_not_a_substring() -> TestResult {
    let temp = seeded_workspace()?;
    let broker = WorkspaceBroker::new();
    let projection = broker.grant("project_forge_grep", temp.path())?;

    let hits = broker.grep(&GrepRequest {
        workspace_id: &projection.workspace_id,
        pattern: r"^fn \w+\(\)",
        path_scope: None,
        case_sensitive: true,
        max_results: 100,
    })?;
    let mut paths: Vec<_> = hits.iter().map(|hit| hit.relative_path.clone()).collect();
    paths.sort();
    paths.dedup();
    assert_eq!(
        paths,
        vec!["src/main.rs".to_owned(), "src/util.rs".to_owned()],
        "an anchored expression must match only the function definitions"
    );
    Ok(())
}

#[test]
fn grep_cannot_read_secrets_even_when_the_content_matches() -> TestResult {
    let temp = seeded_workspace()?;
    let broker = WorkspaceBroker::new();
    let projection = broker.grant("project_forge_grep_secrets", temp.path())?;

    // "hunter2-sentinel" appears only inside .env and .ssh/id_rsa.
    let hits = broker.grep(&GrepRequest {
        workspace_id: &projection.workspace_id,
        pattern: "hunter2-sentinel",
        path_scope: None,
        case_sensitive: true,
        max_results: 1_000,
    })?;
    assert!(
        hits.is_empty(),
        "grep reached a sensitive file and returned its content: {hits:?}"
    );
    Ok(())
}

#[test]
fn grep_path_scope_narrows_and_never_widens() -> TestResult {
    let temp = seeded_workspace()?;
    let broker = WorkspaceBroker::new();
    let projection = broker.grant("project_forge_grep_scope", temp.path())?;

    let scoped = broker.grep(&GrepRequest {
        workspace_id: &projection.workspace_id,
        pattern: "prose",
        path_scope: Some("docs/**"),
        case_sensitive: true,
        max_results: 100,
    })?;
    let paths: Vec<_> = scoped.iter().map(|hit| hit.relative_path.clone()).collect();
    assert_eq!(
        paths,
        vec!["docs/guide.md".to_owned()],
        "the scope must exclude README.md, which also contains the word"
    );

    // A scope pointing straight at a secret narrows to nothing; it cannot be
    // used to re-admit what the structural policy excludes.
    let scoped_at_secret = broker.grep(&GrepRequest {
        workspace_id: &projection.workspace_id,
        pattern: "hunter2-sentinel",
        path_scope: Some(".ssh/**"),
        case_sensitive: true,
        max_results: 100,
    })?;
    assert!(scoped_at_secret.is_empty());
    Ok(())
}

#[test]
fn grep_honours_case_sensitivity_in_both_directions() -> TestResult {
    let temp = seeded_workspace()?;
    let broker = WorkspaceBroker::new();
    let projection = broker.grant("project_forge_grep_case", temp.path())?;

    let sensitive = broker.grep(&GrepRequest {
        workspace_id: &projection.workspace_id,
        pattern: "PLAIN",
        path_scope: None,
        case_sensitive: true,
        max_results: 100,
    })?;
    assert!(sensitive.is_empty(), "the file says 'plain', not 'PLAIN'");

    let insensitive = broker.grep(&GrepRequest {
        workspace_id: &projection.workspace_id,
        pattern: "PLAIN",
        path_scope: None,
        case_sensitive: false,
        max_results: 100,
    })?;
    assert_eq!(insensitive.len(), 1);
    Ok(())
}

#[test]
fn patterns_are_bounded_and_invalid_ones_are_refused() -> TestResult {
    let temp = seeded_workspace()?;
    let broker = WorkspaceBroker::new();
    let projection = broker.grant("project_forge_pattern_bounds", temp.path())?;

    let oversized = "a".repeat(257);
    for pattern in ["", "   ", "(unclosed", oversized.as_str()] {
        assert!(
            matches!(
                broker.grep(&GrepRequest {
                    workspace_id: &projection.workspace_id,
                    pattern,
                    path_scope: None,
                    case_sensitive: true,
                    max_results: 10,
                }),
                Err(WorkspaceError::InvalidPattern)
            ),
            "grep must refuse pattern {pattern:?}"
        );
    }

    for pattern in ["", "   ", "a[", oversized.as_str()] {
        assert!(
            matches!(
                broker.glob(&projection.workspace_id, pattern, 10),
                Err(WorkspaceError::InvalidPattern)
            ),
            "glob must refuse pattern {pattern:?}"
        );
    }

    // A valid pattern is still refused as a *scope* when malformed.
    assert!(matches!(
        broker.grep(&GrepRequest {
            workspace_id: &projection.workspace_id,
            pattern: "prose",
            path_scope: Some("a["),
            case_sensitive: true,
            max_results: 10,
        }),
        Err(WorkspaceError::InvalidPattern)
    ));
    Ok(())
}

#[test]
fn both_tools_stop_at_a_revoked_grant() -> TestResult {
    let temp = seeded_workspace()?;
    let broker = WorkspaceBroker::new();
    let projection = broker.grant("project_forge_revoked", temp.path())?;
    broker.revoke(&projection.workspace_id)?;

    assert!(matches!(
        broker.glob(&projection.workspace_id, "**/*", 10),
        Err(WorkspaceError::GrantUnavailable)
    ));
    assert!(matches!(
        broker.grep(&GrepRequest {
            workspace_id: &projection.workspace_id,
            pattern: "prose",
            path_scope: None,
            case_sensitive: true,
            max_results: 10,
        }),
        Err(WorkspaceError::GrantUnavailable)
    ));
    Ok(())
}
