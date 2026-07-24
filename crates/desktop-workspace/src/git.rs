//! Read-only git observation for the Review surface. Uses the pure-Rust
//! `gix` crate (never a subprocess) and reports only workspace-relative
//! paths with bounded content, matching the renderer's fail-closed diff
//! contract. No mutating git operation exists here by design.

use serde::Serialize;
use std::path::Path;

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
    pub state: GitFileState,
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
    pub before_content: Option<String>,
    pub after_content: Option<String>,
    pub truncated: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum GitReadError {
    #[error("the workspace is not a git repository")]
    NotARepository,
    #[error("the git state could not be read")]
    Unreadable,
    #[error("the requested path is not tracked or present")]
    PathUnavailable,
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

    let mut entries = Vec::new();
    let mut truncated = false;
    for item in iter {
        let item = item.map_err(|_| GitReadError::Unreadable)?;
        if entries.len() >= MAX_GIT_STATUS_ENTRIES {
            truncated = true;
            break;
        }
        if let Some(entry) = classify_item(&item) {
            entries.push(entry);
        }
    }
    entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(GitStatusSummary {
        branch,
        entries,
        truncated,
    })
}

fn classify_item(item: &gix::status::Item) -> Option<GitStatusEntry> {
    use gix::status::index_worktree::iter::Summary;
    let summary = item.summary()?;
    let state = match summary {
        Summary::Added | Summary::IntentToAdd => GitFileState::Untracked,
        Summary::Removed => GitFileState::Deleted,
        Summary::Modified | Summary::TypeChange => GitFileState::Modified,
        Summary::Renamed | Summary::Copied => GitFileState::Renamed,
        Summary::Conflict => GitFileState::Modified,
    };
    let relative_path = item.location().to_string();
    Some(GitStatusEntry {
        relative_path,
        state,
    })
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
) -> Result<GitFileComparison, GitReadError> {
    let repository = open_repository(root)?;

    let before = head_blob(&repository, relative_path);
    let worktree_path = root.join(relative_path);
    let after_bytes = std::fs::read(&worktree_path).ok();

    if before.is_none() && after_bytes.is_none() {
        return Err(GitReadError::PathUnavailable);
    }

    let (before_content, before_truncated) = match &before {
        Some(bytes) => bounded_utf8(bytes),
        None => (None, false),
    };
    let (after_content, after_truncated) = match &after_bytes {
        Some(bytes) => bounded_utf8(bytes),
        None => (None, false),
    };

    Ok(GitFileComparison {
        relative_path: relative_path.to_owned(),
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
