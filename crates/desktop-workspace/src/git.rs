//! Read-only git observation for the Review surface. Uses the pure-Rust
//! `gix` crate (never a subprocess) and reports only workspace-relative
//! paths with bounded content, matching the renderer's fail-closed diff
//! contract. No mutating git operation exists here by design.

pub use desktop_runtime::GitComparison;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

/// One bounded side of a file comparison (64 KiB, the governed-edit bound).
pub const MAX_GIT_DIFF_BYTES: usize = 64 * 1024;
/// Status entries beyond this count set `truncated` instead of growing.
pub const MAX_GIT_STATUS_ENTRIES: usize = 500;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GitFileState {
    Modified,
    Added,
    Deleted,
    Untracked,
    Renamed,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatusEntry {
    pub relative_path: String,
    pub index_state: Option<GitFileState>,
    pub worktree_state: Option<GitFileState>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatusSummary {
    pub branch: Option<String>,
    pub entries: Vec<GitStatusEntry>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitFileComparison {
    pub relative_path: String,
    pub comparison: GitComparison,
    pub before_content: Option<String>,
    pub after_content: Option<String>,
    pub truncated: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum GitReadError {
    #[error("the workspace is not a git repository")]
    NotARepository,
    #[error("the git state could not be read")]
    Unreadable,
    #[error("the requested path is not tracked or present")]
    PathUnavailable,
}

fn workspace_relative_path(relative_path: &str) -> Result<PathBuf, GitReadError> {
    let path = Path::new(relative_path);
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(GitReadError::PathUnavailable);
    }

    let mut bounded = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => bounded.push(part),
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => return Err(GitReadError::PathUnavailable),
        }
    }
    if bounded.as_os_str().is_empty() {
        return Err(GitReadError::PathUnavailable);
    }
    Ok(bounded)
}

fn bounded_worktree_path(root: &Path, relative_path: &Path) -> Result<PathBuf, GitReadError> {
    let candidate = root.join(relative_path);
    if !candidate.exists() {
        return Ok(candidate);
    }

    let canonical_root = std::fs::canonicalize(root).map_err(|_| GitReadError::Unreadable)?;
    let canonical_candidate =
        std::fs::canonicalize(&candidate).map_err(|_| GitReadError::PathUnavailable)?;
    if !canonical_candidate.starts_with(canonical_root) {
        return Err(GitReadError::PathUnavailable);
    }
    Ok(canonical_candidate)
}

fn open_repository(root: &Path) -> Result<gix::Repository, GitReadError> {
    match gix::open(root) {
        Ok(repository) => Ok(repository),
        Err(gix::open::Error::NotARepository { .. }) => Err(GitReadError::NotARepository),
        Err(_) => Err(GitReadError::Unreadable),
    }
}

fn branch_name(repository: &gix::Repository) -> Option<String> {
    let head = repository.head_name().ok()??;
    Some(head.shorten().to_string())
}

/// Reads the working-tree status against the index, bounded and sorted.
/// Returns `NotARepository` when the workspace has no git metadata.
pub fn read_git_status(root: &Path) -> Result<GitStatusSummary, GitReadError> {
    let repository = open_repository(root)?;
    let branch = branch_name(&repository);
    let platform = repository
        .status(gix::progress::Discard)
        .map_err(|_| GitReadError::Unreadable)?;
    let iter = platform
        .into_iter(None)
        .map_err(|_| GitReadError::Unreadable)?;

    let mut entries = BTreeMap::<String, GitStatusEntry>::new();
    let mut truncated = false;
    for item in iter {
        let item = item.map_err(|_| GitReadError::Unreadable)?;
        if let Some((relative_path, side, state)) = classify_item(&item) {
            if !entries.contains_key(&relative_path) && entries.len() >= MAX_GIT_STATUS_ENTRIES {
                truncated = true;
                continue;
            }
            let entry = entries
                .entry(relative_path.clone())
                .or_insert_with(|| GitStatusEntry {
                    relative_path,
                    index_state: None,
                    worktree_state: None,
                });
            match side {
                GitStatusSide::Index => entry.index_state = Some(state),
                GitStatusSide::Worktree => entry.worktree_state = Some(state),
            }
        }
    }
    Ok(GitStatusSummary {
        branch,
        entries: entries.into_values().collect(),
        truncated,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GitStatusSide {
    Index,
    Worktree,
}

fn classify_item(item: &gix::status::Item) -> Option<(String, GitStatusSide, GitFileState)> {
    use gix::diff::index::ChangeRef;
    use gix::status::index_worktree::iter::Summary;

    let (side, state) = match item {
        gix::status::Item::IndexWorktree(change) => (
            GitStatusSide::Worktree,
            match change.summary()? {
                Summary::Added | Summary::IntentToAdd => GitFileState::Untracked,
                Summary::Removed => GitFileState::Deleted,
                Summary::Modified | Summary::TypeChange | Summary::Conflict => {
                    GitFileState::Modified
                }
                Summary::Renamed | Summary::Copied => GitFileState::Renamed,
            },
        ),
        gix::status::Item::TreeIndex(change) => (
            GitStatusSide::Index,
            match change {
                ChangeRef::Addition { .. } => GitFileState::Added,
                ChangeRef::Deletion { .. } => GitFileState::Deleted,
                ChangeRef::Modification { .. } => GitFileState::Modified,
                ChangeRef::Rewrite { .. } => GitFileState::Renamed,
            },
        ),
    };
    let relative_path = item.location().to_string();
    Some((relative_path, side, state))
}

fn bounded_utf8(bytes: &[u8]) -> (Option<String>, bool) {
    let truncated = bytes.len() > MAX_GIT_DIFF_BYTES;
    let bounded = &bytes[..bytes.len().min(MAX_GIT_DIFF_BYTES)];
    match std::str::from_utf8(bounded) {
        Ok(text) => (Some(text.to_owned()), truncated),
        // Binary content is reported as unavailable rather than mangled.
        Err(_) => (None, truncated),
    }
}

/// Reads one file's HEAD blob and current working-tree content, both
/// bounded, so the renderer can diff the exact observed bytes client-side.
pub fn read_git_diff(
    root: &Path,
    relative_path: &str,
    comparison: GitComparison,
) -> Result<GitFileComparison, GitReadError> {
    let repository = open_repository(root)?;

    let bounded_relative_path = workspace_relative_path(relative_path)?;
    let repository_relative_path = bounded_relative_path.to_string_lossy().replace('\\', "/");

    let head = head_blob(&repository, &repository_relative_path);
    let index = index_blob(&repository, &repository_relative_path);
    let worktree_path = bounded_worktree_path(root, &bounded_relative_path)?;
    let worktree = std::fs::read(&worktree_path).ok();
    let (before, after) = match comparison {
        GitComparison::HeadToWorktree => (head, worktree),
        GitComparison::HeadToIndex => (head, index),
        GitComparison::IndexToWorktree => (index, worktree),
    };

    if before.is_none() && after.is_none() {
        return Err(GitReadError::PathUnavailable);
    }

    let (before_content, before_truncated) = match &before {
        Some(bytes) => bounded_utf8(bytes),
        None => (None, false),
    };
    let (after_content, after_truncated) = match &after {
        Some(bytes) => bounded_utf8(bytes),
        None => (None, false),
    };

    Ok(GitFileComparison {
        relative_path: repository_relative_path,
        comparison,
        before_content,
        after_content,
        truncated: before_truncated || after_truncated,
    })
}

fn head_blob(repository: &gix::Repository, relative_path: &str) -> Option<Vec<u8>> {
    let commit = repository.head_commit().ok()?;
    let tree = commit.tree().ok()?;
    let entry = tree.lookup_entry_by_path(relative_path).ok()??;
    let object = entry.object().ok()?;
    Some(object.data.clone())
}

fn index_blob(repository: &gix::Repository, relative_path: &str) -> Option<Vec<u8>> {
    let index = repository.index().ok()?;
    let entry = index.entry_by_path(relative_path.as_bytes().into())?;
    let object = repository.find_object(entry.id).ok()?;
    Some(object.data.clone())
}
