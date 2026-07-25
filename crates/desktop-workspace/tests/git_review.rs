#[path = "../src/git.rs"]
mod git_review;

use desktop_workspace::{GitComparison as BrokerComparison, WorkspaceBroker, WorkspaceError};
use git_review::{read_git_diff, read_git_status, GitComparison, GitFileState, GitReadError};
use gix::bstr::ByteSlice as _;
use std::path::Path;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn committed_workspace() -> TestResult<tempfile::TempDir> {
    let temp = tempfile::tempdir()?;
    std::fs::write(temp.path().join("tracked.txt"), "base\n")?;
    let repository = gix::init(temp.path())?;
    let blob_id = repository.write_blob(b"base\n")?.detach();
    let tree = gix::objs::Tree {
        entries: vec![gix::objs::tree::Entry {
            mode: gix::objs::tree::EntryKind::Blob.into(),
            filename: "tracked.txt".into(),
            oid: blob_id,
        }],
    };
    let tree_id = repository.write_object(&tree)?.detach();
    let head_name = repository
        .head_name()?
        .ok_or_else(|| std::io::Error::other("the fixture has no unborn branch"))?
        .clone();
    let signature = gix::actor::SignatureRef {
        name: b"Forge Tests".as_bstr(),
        email: b"forge-tests@example.invalid".as_bstr(),
        time: "1 +0000",
    };
    repository.commit_as(
        signature,
        signature,
        head_name,
        "base",
        tree_id,
        std::iter::empty::<gix::ObjectId>(),
    )?;
    let mut index = repository.index_from_tree(&tree_id)?;
    index.write(gix::index::write::Options::default())?;
    Ok(temp)
}

fn stage_text(root: &Path, relative_path: &str, content: &str) -> TestResult {
    std::fs::write(root.join(relative_path), content)?;
    let repository = gix::open(root)?;
    let blob_id = repository.write_blob(content.as_bytes())?.detach();
    let mut index = repository.open_index()?;
    let entry = index
        .entry_mut_by_path_and_stage(
            relative_path.as_bytes().as_bstr(),
            gix::index::entry::Stage::Unconflicted,
        )
        .ok_or_else(|| std::io::Error::other("the fixture index has no tracked entry"))?;
    entry.id = blob_id;
    index.write(gix::index::write::Options::default())?;
    Ok(())
}

#[test]
fn review_diff_rejects_paths_outside_the_workspace() -> TestResult {
    let temp = tempfile::tempdir()?;
    let root = temp.path().join("workspace");
    std::fs::create_dir(&root)?;
    std::fs::write(temp.path().join("outside.txt"), "secret\n")?;

    gix::init(&root)?;

    let result = read_git_diff(&root, "../outside.txt", GitComparison::HeadToWorktree);
    assert!(matches!(result, Err(GitReadError::PathUnavailable)));
    Ok(())
}

#[test]
fn review_does_not_borrow_a_repository_from_a_parent_workspace() -> TestResult {
    let temp = committed_workspace()?;
    let nested_workspace = temp.path().join("nested-selection");
    std::fs::create_dir(&nested_workspace)?;
    std::fs::write(nested_workspace.join("local.txt"), "local\n")?;

    assert!(matches!(
        read_git_status(&nested_workspace),
        Err(GitReadError::NotARepository)
    ));
    assert!(matches!(
        read_git_diff(
            &nested_workspace,
            "local.txt",
            GitComparison::HeadToWorktree,
        ),
        Err(GitReadError::NotARepository)
    ));
    Ok(())
}

#[test]
fn review_status_merges_staged_and_unstaged_changes_per_file() -> TestResult {
    let temp = committed_workspace()?;
    stage_text(temp.path(), "tracked.txt", "staged\n")?;
    std::fs::write(temp.path().join("tracked.txt"), "working\n")?;
    std::fs::write(temp.path().join("untracked.txt"), "new\n")?;

    let status = read_git_status(temp.path())?;

    assert_eq!(status.entries.len(), 2);
    let tracked = status
        .entries
        .iter()
        .find(|entry| entry.relative_path == "tracked.txt")
        .ok_or_else(|| std::io::Error::other("tracked status entry is missing"))?;
    assert_eq!(tracked.index_state, Some(GitFileState::Modified));
    assert_eq!(tracked.worktree_state, Some(GitFileState::Modified));

    let untracked = status
        .entries
        .iter()
        .find(|entry| entry.relative_path == "untracked.txt")
        .ok_or_else(|| std::io::Error::other("untracked status entry is missing"))?;
    assert_eq!(untracked.index_state, None);
    assert_eq!(untracked.worktree_state, Some(GitFileState::Untracked));
    Ok(())
}

#[test]
fn review_diff_supports_default_staged_and_unstaged_baselines() -> TestResult {
    let temp = committed_workspace()?;
    stage_text(temp.path(), "tracked.txt", "staged\n")?;
    std::fs::write(temp.path().join("tracked.txt"), "working\n")?;

    let full = read_git_diff(temp.path(), "tracked.txt", GitComparison::HeadToWorktree)?;
    assert_eq!(full.before_content.as_deref(), Some("base\n"));
    assert_eq!(full.after_content.as_deref(), Some("working\n"));

    let staged = read_git_diff(temp.path(), "tracked.txt", GitComparison::HeadToIndex)?;
    assert_eq!(staged.before_content.as_deref(), Some("base\n"));
    assert_eq!(staged.after_content.as_deref(), Some("staged\n"));

    let unstaged = read_git_diff(temp.path(), "tracked.txt", GitComparison::IndexToWorktree)?;
    assert_eq!(unstaged.before_content.as_deref(), Some("staged\n"));
    assert_eq!(unstaged.after_content.as_deref(), Some("working\n"));
    Ok(())
}

#[test]
fn broker_keeps_git_reads_inside_an_active_workspace_grant() -> TestResult {
    let temp = committed_workspace()?;
    std::fs::write(temp.path().join("tracked.txt"), "working\n")?;

    let broker = WorkspaceBroker::new();
    let projection = broker.grant("project_forge_review", temp.path())?;
    let status = broker.git_status(&projection.workspace_id)?;
    assert_eq!(status.entries.len(), 1);
    let comparison = broker.git_diff(
        &projection.workspace_id,
        "tracked.txt",
        BrokerComparison::HeadToWorktree,
    )?;
    assert_eq!(comparison.after_content.as_deref(), Some("working\n"));

    broker.revoke(&projection.workspace_id)?;
    assert!(matches!(
        broker.git_status(&projection.workspace_id),
        Err(WorkspaceError::GrantUnavailable)
    ));
    Ok(())
}
