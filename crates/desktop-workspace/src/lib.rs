#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

mod bmad_snapshot;
mod governed_io;

pub use bmad_snapshot::{read_bmad_source_snapshot, BmadSnapshotError};
pub use governed_io::{GovernedRecoveryTransaction, PreimageObservation, MAX_GOVERNED_FILE_BYTES};

use ignore::WalkBuilder;
use parking_lot::{RwLock, RwLockReadGuard};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;
use ulid::Ulid;

const DEFAULT_MAX_TEXT_BYTES: u64 = 512 * 1024;
const DEFAULT_MAX_RESULTS: usize = 1_000;
const DEFAULT_MAX_SCAN_BYTES: u64 = 4 * 1024 * 1024;
const MAX_DIRECTORY_ENTRIES: usize = 10_000;
const MAX_WALK_ENTRIES: usize = 10_000;
const MAX_RELATIVE_PATH_BYTES: usize = 1_024;
const MAX_SEGMENT_UTF16_UNITS: usize = 255;
const CONFIG_CREDENTIAL_STORES: &[&str] = &[
    "1password",
    "aws",
    "azure",
    "doctl",
    "gcloud",
    "gh",
    "hub",
    "op",
    "pulumi",
];
const APPDATA_CREDENTIAL_STORES: &[&str] = &[
    "1password",
    "aws",
    "azure",
    "docker",
    "gcloud",
    "github cli",
    "gnupg",
    "google",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceProjection {
    pub workspace_id: String,
    pub project_id: String,
    pub display_name: String,
    pub grant_epoch: u64,
    pub permissions: WorkspacePermissions,
    /// D2 context-read authority version (ADR-0002). Independent from the
    /// governed-edit epoch; advanced when context-read authority is
    /// withdrawn (for example on model sign-out).
    #[serde(default = "default_vertical_epoch")]
    pub context_read_epoch: u64,
    /// D3 governed-edit authority version (ADR-0002). Advanced whenever
    /// edit authority changes; never advanced by context-read changes.
    #[serde(default = "default_vertical_epoch")]
    pub governed_edit_epoch: u64,
}

const fn default_vertical_epoch() -> u64 {
    1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspacePermissions {
    ReadOnly,
    GovernedEdits,
}

/// Opaque authority values for binding host-side work. The selected absolute
/// root is intentionally absent and never enters renderer-facing projections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceAuthorityBinding {
    pub workspace_id: String,
    pub grant_epoch: u64,
    pub context_read_epoch: u64,
    pub governed_edit_epoch: u64,
    pub root_identity_hash: String,
}

/// A host-only, exact-epoch workspace scope authorization.
///
/// The guard intentionally exposes no selected-root path. Holding it prevents
/// in-process broker mutation and revocation until the guard is dropped. A
/// caller must not invoke another [`WorkspaceBroker`] method while holding this
/// guard because those methods acquire the same task-fair barrier. Durable
/// revocation must be ordered with guarded operations by the host.
#[must_use = "workspace scope authority must remain held while the authorized operation runs"]
pub struct WorkspaceScopeAuthorityGuard<'a> {
    _revocation_barrier: RwLockReadGuard<'a, ()>,
    projection: WorkspaceProjection,
    authority_binding: WorkspaceAuthorityBinding,
}

impl WorkspaceScopeAuthorityGuard<'_> {
    /// Returns a detached renderer-safe workspace projection.
    #[must_use]
    pub fn projection(&self) -> WorkspaceProjection {
        self.projection.clone()
    }

    /// Returns a detached opaque authority binding without the selected root.
    #[must_use]
    pub fn authority_binding(&self) -> WorkspaceAuthorityBinding {
        self.authority_binding.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceEntry {
    pub relative_path: String,
    pub kind: EntryKind,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceEntryPage {
    pub entries: Vec<WorkspaceEntry>,
    pub next_after: Option<String>,
}

/// Result of mapping host-picked absolute files to workspace-relative paths.
/// Only relative wire paths and rejection counts leave the broker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickedFiles {
    pub relative_paths: Vec<String>,
    pub rejected_outside_root: u64,
    pub rejected_unreadable: u64,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    Directory,
    TextFile,
    BinaryFile,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextPreview {
    pub relative_path: String,
    pub content: String,
    pub content_hash: String,
    pub byte_count: u64,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchMatch {
    pub relative_path: String,
    pub line: usize,
    pub preview: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BmadScanProjection {
    pub status: BmadStatus,
    pub assets: Vec<BmadAssetProjection>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadStatus {
    NotDetected,
    MethodDetected,
    BuilderDraftsDetected,
    MethodAndBuilderDraftsDetected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BmadAssetProjection {
    pub relative_path: String,
    pub asset_kind: BmadAssetKind,
    pub activation: BmadActivation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadAssetKind {
    MethodConfiguration,
    Agent,
    Workflow,
    BuilderBuildDraft,
    BuilderEditDraft,
    BuilderAnalyzeDraft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadActivation {
    ReadOnly,
    InactiveDraft,
}

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("the workspace root must be a fixed local NTFS directory")]
    UnsupportedRoot,
    #[error("the requested relative path is invalid")]
    InvalidRelativePath,
    #[error("the requested path is outside the selected workspace")]
    OutsideWorkspace,
    #[error("the workspace grant is not visible or has been revoked")]
    GrantUnavailable,
    #[error("the workspace identity changed; select the folder again")]
    RootIdentityChanged,
    #[error("the path is blocked by workspace policy")]
    PathBlocked,
    #[error("the file is not supported as bounded UTF-8 text")]
    UnsupportedText,
    #[error("the requested operation exceeded its configured limit")]
    LimitExceeded,
    #[error("governed edits are not enabled for this workspace grant")]
    EditsNotEnabled,
    #[error("the observed file no longer matches the expected preimage")]
    StalePreimage,
    #[error("the target file already exists")]
    AlreadyExists,
    #[error("workspace I/O failed")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
struct WorkspaceGrant {
    projection: WorkspaceProjection,
    root: PathBuf,
    root_identity_hash: String,
    revoked: bool,
}

#[derive(Debug, Default)]
struct WalkBudget {
    visited: usize,
}

impl WalkBudget {
    fn visit(&mut self) -> Result<(), WorkspaceError> {
        if self.visited >= MAX_WALK_ENTRIES {
            return Err(WorkspaceError::LimitExceeded);
        }
        self.visited = self.visited.saturating_add(1);
        Ok(())
    }
}

/// Selected-workspace broker for bounded reads and governed durable edits.
///
/// The path checks in this crate are defense in depth for D1 reads. Governed
/// writes live in the `governed_io` module: they additionally require an
/// explicit `GovernedEdits` grant at an exact epoch and verify each preimage
/// through an open handle that denies concurrent writers. Handle-relative
/// selected-root containment across a check/use race remains a documented
/// residual limitation until a pinned root-handle design lands.
#[derive(Debug, Default)]
pub struct WorkspaceBroker {
    /// Prevents a grant mutation or revocation from crossing a read operation.
    /// Lock order is always `revocation_barrier` before `grants`.
    revocation_barrier: RwLock<()>,
    grants: RwLock<HashMap<String, WorkspaceGrant>>,
}

impl WorkspaceBroker {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a read-only grant for a selected fixed local NTFS directory.
    ///
    /// # Errors
    ///
    /// Returns an error when the selected root is unsupported or inaccessible,
    /// or when its stable host identity cannot be read.
    pub fn grant(
        &self,
        project_id: impl Into<String>,
        selected_root: impl AsRef<Path>,
    ) -> Result<WorkspaceProjection, WorkspaceError> {
        let _authority = self.revocation_barrier.write();
        let root = validate_root(selected_root.as_ref())?;
        let metadata = fs::metadata(&root)?;
        let root_identity_hash = root_identity(&root, &metadata)?;
        let display_name = root
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("Local workspace")
            .to_owned();
        let workspace_id = format!("workspace_{}", Ulid::new());
        let projection = WorkspaceProjection {
            workspace_id: workspace_id.clone(),
            project_id: project_id.into(),
            display_name,
            grant_epoch: 1,
            permissions: WorkspacePermissions::ReadOnly,
            context_read_epoch: 1,
            governed_edit_epoch: 1,
        };
        self.grants.write().insert(
            workspace_id,
            WorkspaceGrant {
                projection: projection.clone(),
                root,
                root_identity_hash,
                revoked: false,
            },
        );
        Ok(projection)
    }

    /// Restores a persisted read-only grant after revalidating its root identity.
    ///
    /// # Errors
    ///
    /// Returns an error when the projection is invalid, the selected root is
    /// unsupported or inaccessible, the root identity changed, or the grant is
    /// already present.
    pub fn restore_grant(
        &self,
        mut projection: WorkspaceProjection,
        selected_root: impl AsRef<Path>,
        expected_root_identity_hash: &str,
    ) -> Result<(), WorkspaceError> {
        let _authority = self.revocation_barrier.write();
        if projection.workspace_id.is_empty()
            || projection.project_id.is_empty()
            || projection.display_name.is_empty()
            || projection.grant_epoch == 0
            || projection.context_read_epoch == 0
            || projection.governed_edit_epoch == 0
            || !is_sha256(expected_root_identity_hash)
        {
            return Err(WorkspaceError::GrantUnavailable);
        }
        let root = validate_root(selected_root.as_ref())?;
        let metadata = fs::metadata(&root)?;
        let root_identity_hash = root_identity(&root, &metadata)?;
        if root_identity_hash != expected_root_identity_hash {
            return Err(WorkspaceError::RootIdentityChanged);
        }
        projection.permissions = WorkspacePermissions::ReadOnly;
        let workspace_id = projection.workspace_id.clone();
        let mut grants = self.grants.write();
        if grants.contains_key(&workspace_id) {
            return Err(WorkspaceError::GrantUnavailable);
        }
        grants.insert(
            workspace_id,
            WorkspaceGrant {
                projection,
                root,
                root_identity_hash,
                revoked: false,
            },
        );
        Ok(())
    }

    #[must_use]
    pub fn list(&self) -> Vec<WorkspaceProjection> {
        let _authority = self.revocation_barrier.read();
        let mut projections = self
            .grants
            .read()
            .values()
            .filter(|grant| !grant.revoked)
            .map(|grant| grant.projection.clone())
            .collect::<Vec<_>>();
        projections.sort_by(|left, right| left.workspace_id.cmp(&right.workspace_id));
        projections
    }

    /// Advances one workspace's D2 context-read epoch, invalidating every
    /// Help context binding issued against the previous epoch. Governed-edit
    /// authority and D1 reads are untouched (ADR-0002).
    ///
    /// # Errors
    ///
    /// Returns an error when the grant is absent or revoked; an exhausted
    /// epoch revokes the grant fail-closed.
    pub fn advance_context_read_epoch(
        &self,
        workspace_id: &str,
    ) -> Result<WorkspaceProjection, WorkspaceError> {
        let _authority = self.revocation_barrier.write();
        let mut grants = self.grants.write();
        let grant = grants
            .get_mut(workspace_id)
            .filter(|grant| !grant.revoked)
            .ok_or(WorkspaceError::GrantUnavailable)?;
        let Some(next) = grant.projection.context_read_epoch.checked_add(1) else {
            grant.revoked = true;
            return Err(WorkspaceError::GrantUnavailable);
        };
        grant.projection.context_read_epoch = next;
        Ok(grant.projection.clone())
    }

    /// Advances the context-read epoch of every active grant (model
    /// sign-out): D2 authority is withdrawn everywhere while D3 proposals
    /// and local work remain valid.
    pub fn advance_all_context_read_epochs(&self) {
        let _authority = self.revocation_barrier.write();
        let mut grants = self.grants.write();
        for grant in grants.values_mut().filter(|grant| !grant.revoked) {
            match grant.projection.context_read_epoch.checked_add(1) {
                Some(next) => grant.projection.context_read_epoch = next,
                None => grant.revoked = true,
            }
        }
    }

    /// Revokes an active workspace grant and advances its epoch.
    ///
    /// # Errors
    ///
    /// Returns an error when the grant is absent, already revoked, or its epoch
    /// cannot be advanced without wrapping.
    pub fn revoke(&self, workspace_id: &str) -> Result<WorkspaceProjection, WorkspaceError> {
        let _authority = self.revocation_barrier.write();
        let mut grants = self.grants.write();
        let grant = grants
            .get_mut(workspace_id)
            .ok_or(WorkspaceError::GrantUnavailable)?;
        if grant.revoked {
            return Err(WorkspaceError::GrantUnavailable);
        }
        let Some(next_epoch) = grant.projection.grant_epoch.checked_add(1) else {
            // Revocation must fail closed even for a corrupted, exhausted epoch.
            grant.revoked = true;
            return Err(WorkspaceError::GrantUnavailable);
        };
        grant.revoked = true;
        grant.projection.grant_epoch = next_epoch;
        Ok(grant.projection.clone())
    }

    /// Lists a bounded number of direct children in a workspace directory.
    ///
    /// # Errors
    ///
    /// Returns an error when the grant is unavailable, the root identity or
    /// requested path is invalid, a policy limit is exceeded, or an I/O
    /// operation fails.
    pub fn list_entries(
        &self,
        workspace_id: &str,
        relative_directory: &str,
        max_results: usize,
    ) -> Result<Vec<WorkspaceEntry>, WorkspaceError> {
        let authority = self.revocation_barrier.read();
        let grant = self.active_grant(&authority, workspace_id)?;
        revalidate_root(&grant)?;
        let directory = resolve_existing(&grant.root, relative_directory)?;
        if !directory.is_dir() {
            return Err(WorkspaceError::InvalidRelativePath);
        }

        let limit = max_results.clamp(1, DEFAULT_MAX_RESULTS);
        let mut entries = Vec::new();
        for item in fs::read_dir(directory)? {
            if entries.len() >= limit {
                break;
            }
            let item = item?;
            let path = item.path();
            let relative = to_relative_wire_path(&grant.root, &path)?;
            let metadata = fs::symlink_metadata(&path)?;
            let kind = classify_entry(&path, &relative, &metadata);
            entries.push(WorkspaceEntry {
                relative_path: relative,
                kind,
                size_bytes: metadata.len(),
            });
        }
        entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        self.ensure_grant_current(&authority, &grant)?;
        Ok(entries)
    }

    /// Returns a stable bounded page for a directory. `after` is an internal
    /// host cursor value obtained from the previous page; it is never accepted
    /// directly from a renderer path field.
    ///
    /// # Errors
    ///
    /// Returns an error when the grant is unavailable, the root identity,
    /// directory, or cursor is invalid, the bounded scan limit is exceeded, or
    /// an I/O operation fails.
    pub fn list_entries_page(
        &self,
        workspace_id: &str,
        relative_directory: &str,
        after: Option<&str>,
        page_size: usize,
    ) -> Result<WorkspaceEntryPage, WorkspaceError> {
        let authority = self.revocation_barrier.read();
        let grant = self.active_grant(&authority, workspace_id)?;
        revalidate_root(&grant)?;
        let directory = resolve_existing(&grant.root, relative_directory)?;
        if !directory.is_dir() {
            return Err(WorkspaceError::InvalidRelativePath);
        }

        let page_size = page_size.clamp(1, DEFAULT_MAX_RESULTS);
        let mut entries = Vec::new();
        for item in fs::read_dir(directory)? {
            if entries.len() >= MAX_DIRECTORY_ENTRIES {
                return Err(WorkspaceError::LimitExceeded);
            }
            let item = item?;
            let path = item.path();
            let relative_path = to_relative_wire_path(&grant.root, &path)?;
            let metadata = fs::symlink_metadata(&path)?;
            let kind = classify_entry(&path, &relative_path, &metadata);
            entries.push(WorkspaceEntry {
                relative_path,
                kind,
                size_bytes: metadata.len(),
            });
        }
        entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

        let start = match after {
            Some(previous) => entries
                .iter()
                .position(|entry| entry.relative_path == previous)
                .map(|index| index.saturating_add(1))
                .ok_or(WorkspaceError::InvalidRelativePath)?,
            None => 0,
        };
        let available = entries.len().saturating_sub(start);
        let take = available.min(page_size);
        let page = entries
            .into_iter()
            .skip(start)
            .take(take)
            .collect::<Vec<_>>();
        let next_after = if available > take {
            page.last().map(|entry| entry.relative_path.clone())
        } else {
            None
        };
        self.ensure_grant_current(&authority, &grant)?;

        Ok(WorkspaceEntryPage {
            entries: page,
            next_after,
        })
    }

    /// Maps host-picked absolute file paths to validated workspace-relative
    /// wire paths, discarding anything outside the granted root.
    ///
    /// The absolute inputs come only from the host's own file dialog, never from
    /// the renderer. Each candidate is canonicalized and required to be a regular
    /// text-eligible file strictly inside the canonical root with no reparse
    /// component; survivors are deduplicated and sorted. Counts of rejected and
    /// unreadable candidates are returned so the host can report them without
    /// disclosing any absolute path.
    ///
    /// # Errors
    ///
    /// Returns an error when the grant is unavailable or its root identity
    /// changed since selection.
    pub fn resolve_picked_files(
        &self,
        workspace_id: &str,
        candidates: &[PathBuf],
        limit: usize,
    ) -> Result<PickedFiles, WorkspaceError> {
        let authority = self.revocation_barrier.read();
        let grant = self.active_grant(&authority, workspace_id)?;
        revalidate_root(&grant)?;
        let root = grant.root.clone();

        let mut relative_paths = BTreeSet::new();
        let mut rejected_outside_root = 0_u64;
        let mut rejected_unreadable = 0_u64;
        for candidate in candidates {
            match resolve_picked_file(&root, candidate) {
                Ok(relative) => {
                    relative_paths.insert(relative);
                }
                Err(
                    WorkspaceError::OutsideWorkspace
                    | WorkspaceError::PathBlocked
                    | WorkspaceError::InvalidRelativePath
                    | WorkspaceError::UnsupportedText,
                ) => {
                    rejected_outside_root = rejected_outside_root.saturating_add(1);
                }
                Err(_) => {
                    rejected_unreadable = rejected_unreadable.saturating_add(1);
                }
            }
        }
        self.ensure_grant_current(&authority, &grant)?;

        let total = relative_paths.len();
        let bounded_limit = limit.max(1);
        let truncated = total > bounded_limit;
        let relative_paths = relative_paths
            .into_iter()
            .take(bounded_limit)
            .collect::<Vec<_>>();
        Ok(PickedFiles {
            relative_paths,
            rejected_outside_root,
            rejected_unreadable,
            truncated,
        })
    }

    /// Reads a bounded UTF-8 preview from a policy-approved workspace file.
    ///
    /// # Errors
    ///
    /// Returns an error when authority or root identity validation fails, the
    /// path is invalid or blocked, the file is unsupported text, a limit cannot
    /// be represented safely, or an I/O operation fails.
    pub fn read_text(
        &self,
        workspace_id: &str,
        relative_path: &str,
        max_bytes: u64,
    ) -> Result<TextPreview, WorkspaceError> {
        let authority = self.revocation_barrier.read();
        let grant = self.active_grant(&authority, workspace_id)?;
        revalidate_root(&grant)?;
        if is_sensitive_relative_path(relative_path) {
            return Err(WorkspaceError::PathBlocked);
        }
        let path = resolve_existing(&grant.root, relative_path)?;
        let metadata = fs::metadata(&path)?;
        if !metadata.is_file()
            || is_reparse_point(&metadata)
            || is_cloud_placeholder(&metadata)
            || has_unexpected_hardlinks(&path, &metadata)
        {
            return Err(WorkspaceError::PathBlocked);
        }

        let limit = max_bytes.clamp(1, DEFAULT_MAX_TEXT_BYTES);
        let limit_usize = usize::try_from(limit).map_err(|_| WorkspaceError::LimitExceeded)?;
        let initial_capacity = usize::try_from(metadata.len().min(limit))
            .map_err(|_| WorkspaceError::LimitExceeded)?
            .saturating_add(1);
        let mut file = File::open(&path)?;
        let mut bytes = Vec::with_capacity(initial_capacity);
        file.by_ref()
            .take(limit.saturating_add(1))
            .read_to_end(&mut bytes)?;
        let truncated = bytes.len() > limit_usize;
        if truncated {
            bytes.truncate(limit_usize);
        }
        if bytes.contains(&0) {
            return Err(WorkspaceError::UnsupportedText);
        }
        let content = String::from_utf8(bytes).map_err(|_| WorkspaceError::UnsupportedText)?;
        let content_hash = hash_bytes(content.as_bytes());
        self.ensure_grant_current(&authority, &grant)?;
        Ok(TextPreview {
            relative_path: normalize_relative_path(relative_path)?,
            content,
            content_hash,
            byte_count: metadata.len(),
            truncated,
        })
    }

    /// Searches bounded, policy-approved UTF-8 workspace content.
    ///
    /// # Errors
    ///
    /// Returns an error when the query or workspace authority is invalid, the
    /// root identity changes, the walk budget is exhausted, or an I/O operation
    /// fails.
    pub fn search(
        &self,
        workspace_id: &str,
        query: &str,
        max_results: usize,
    ) -> Result<Vec<SearchMatch>, WorkspaceError> {
        if query.trim().is_empty() || query.len() > 256 {
            return Err(WorkspaceError::InvalidRelativePath);
        }
        let authority = self.revocation_barrier.read();
        let grant = self.active_grant(&authority, workspace_id)?;
        revalidate_root(&grant)?;
        let limit = max_results.clamp(1, DEFAULT_MAX_RESULTS);
        let mut inspected_bytes = 0_u64;
        let mut matches = Vec::new();
        let mut walk_budget = WalkBudget::default();
        let query_lower = query.to_lowercase();

        let walker = WalkBuilder::new(&grant.root)
            .hidden(false)
            .follow_links(false)
            .git_ignore(true)
            .git_global(false)
            .git_exclude(true)
            .build();

        for entry in walker {
            self.ensure_grant_current(&authority, &grant)?;
            if matches.len() >= limit || inspected_bytes >= DEFAULT_MAX_SCAN_BYTES {
                break;
            }
            walk_budget.visit()?;
            let entry = entry
                .map_err(|error| WorkspaceError::Io(std::io::Error::other(error.to_string())))?;
            let path = entry.path();
            if !entry
                .file_type()
                .is_some_and(|file_type| file_type.is_file())
            {
                continue;
            }
            let relative = to_relative_wire_path(&grant.root, path)?;
            if is_sensitive_relative_path(&relative) {
                continue;
            }
            let metadata = entry
                .metadata()
                .map_err(|error| WorkspaceError::Io(std::io::Error::other(error.to_string())))?;
            if metadata.len() > DEFAULT_MAX_TEXT_BYTES
                || is_reparse_point(&metadata)
                || is_cloud_placeholder(&metadata)
                || has_unexpected_hardlinks(path, &metadata)
            {
                continue;
            }
            let Some(bytes) = read_bounded_file(path, DEFAULT_MAX_TEXT_BYTES)? else {
                continue;
            };
            inspected_bytes = inspected_bytes.saturating_add(bytes.len() as u64);
            if bytes.contains(&0) {
                continue;
            }
            let Ok(text) = String::from_utf8(bytes) else {
                continue;
            };
            for (index, line) in text.lines().enumerate() {
                if line.to_lowercase().contains(&query_lower) {
                    matches.push(SearchMatch {
                        relative_path: relative.clone(),
                        line: index.saturating_add(1),
                        preview: truncate_chars(line.trim(), 240),
                    });
                    if matches.len() >= limit {
                        break;
                    }
                }
            }
        }
        self.ensure_grant_current(&authority, &grant)?;
        Ok(matches)
    }

    /// Detects bounded BMAD Method assets and inactive Builder drafts.
    ///
    /// # Errors
    ///
    /// Returns an error when workspace authority or root identity validation
    /// fails, a path cannot be normalized, or a directory walk operation fails.
    pub fn scan_bmad(
        &self,
        workspace_id: &str,
        max_assets: usize,
    ) -> Result<BmadScanProjection, WorkspaceError> {
        let authority = self.revocation_barrier.read();
        let grant = self.active_grant(&authority, workspace_id)?;
        revalidate_root(&grant)?;
        let limit = max_assets.clamp(1, 256);
        let mut assets = Vec::new();
        let mut truncated = false;
        let mut walk_budget = WalkBudget::default();
        let walker = WalkBuilder::new(&grant.root)
            .hidden(false)
            .follow_links(false)
            .git_ignore(false)
            .git_global(false)
            .git_exclude(false)
            .max_depth(Some(6))
            .build();

        for entry in walker {
            self.ensure_grant_current(&authority, &grant)?;
            if walk_budget.visit().is_err() {
                truncated = true;
                break;
            }
            let entry = entry
                .map_err(|error| WorkspaceError::Io(std::io::Error::other(error.to_string())))?;
            if !entry
                .file_type()
                .is_some_and(|file_type| file_type.is_file())
            {
                continue;
            }
            let relative_path = to_relative_wire_path(&grant.root, entry.path())?;
            if is_sensitive_relative_path(&relative_path) {
                continue;
            }
            let Some((asset_kind, activation)) = classify_bmad_asset(&relative_path) else {
                continue;
            };
            if assets.len() == limit {
                truncated = true;
                break;
            }
            assets.push(BmadAssetProjection {
                relative_path,
                asset_kind,
                activation,
            });
        }
        assets.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        let method_detected = assets
            .iter()
            .any(|asset| asset.activation == BmadActivation::ReadOnly);
        let builder_detected = assets
            .iter()
            .any(|asset| asset.activation == BmadActivation::InactiveDraft);
        let status = match (method_detected, builder_detected) {
            (true, true) => BmadStatus::MethodAndBuilderDraftsDetected,
            (true, false) => BmadStatus::MethodDetected,
            (false, true) => BmadStatus::BuilderDraftsDetected,
            (false, false) => BmadStatus::NotDetected,
        };
        self.ensure_grant_current(&authority, &grant)?;
        Ok(BmadScanProjection {
            status,
            assets,
            truncated,
        })
    }

    /// Returns the opaque host authority binding for an active grant.
    ///
    /// # Errors
    ///
    /// Returns an error when the grant is unavailable, the selected root
    /// identity changed, or root revalidation encounters an I/O failure.
    pub fn authority_binding(
        &self,
        workspace_id: &str,
    ) -> Result<WorkspaceAuthorityBinding, WorkspaceError> {
        let authority = self.revocation_barrier.read();
        let guard = self.validated_scope_guard(authority, workspace_id, None)?;
        Ok(guard.authority_binding())
    }

    /// Authorizes a workspace scope at an exact active grant epoch.
    ///
    /// The returned guard holds the broker's in-process revocation barrier for
    /// its entire lifetime. Callers receive only cloned projection and opaque
    /// binding facts; the selected absolute root remains broker-private. The
    /// caller must not re-enter this broker while the guard is held and must
    /// use a host-level ordering boundary for durable revocation.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError::GrantUnavailable`] when the grant is absent,
    /// revoked, zero-epoch, stale, or does not match `expected_grant_epoch`.
    /// Root identity drift and root revalidation failures remain fail-closed.
    pub fn authorize_scope(
        &self,
        workspace_id: &str,
        expected_grant_epoch: u64,
    ) -> Result<WorkspaceScopeAuthorityGuard<'_>, WorkspaceError> {
        let authority = self.revocation_barrier.read();
        self.validated_scope_guard(authority, workspace_id, Some(expected_grant_epoch))
    }

    fn validated_scope_guard<'a>(
        &'a self,
        authority: RwLockReadGuard<'a, ()>,
        workspace_id: &str,
        expected_grant_epoch: Option<u64>,
    ) -> Result<WorkspaceScopeAuthorityGuard<'a>, WorkspaceError> {
        let grant = self.active_grant(&authority, workspace_id)?;
        let grant_epoch = grant.projection.grant_epoch;
        if grant_epoch == 0
            || expected_grant_epoch.is_some_and(|expected| expected == 0 || expected != grant_epoch)
        {
            return Err(WorkspaceError::GrantUnavailable);
        }
        revalidate_root(&grant)?;
        self.ensure_grant_current(&authority, &grant)?;

        let projection = grant.projection;
        let authority_binding = WorkspaceAuthorityBinding {
            workspace_id: projection.workspace_id.clone(),
            grant_epoch,
            context_read_epoch: projection.context_read_epoch,
            governed_edit_epoch: projection.governed_edit_epoch,
            root_identity_hash: grant.root_identity_hash,
        };
        Ok(WorkspaceScopeAuthorityGuard {
            _revocation_barrier: authority,
            projection,
            authority_binding,
        })
    }

    fn active_grant(
        &self,
        _authority: &RwLockReadGuard<'_, ()>,
        workspace_id: &str,
    ) -> Result<WorkspaceGrant, WorkspaceError> {
        self.grants
            .read()
            .get(workspace_id)
            .filter(|grant| !grant.revoked)
            .cloned()
            .ok_or(WorkspaceError::GrantUnavailable)
    }

    fn ensure_grant_current(
        &self,
        _authority: &RwLockReadGuard<'_, ()>,
        grant: &WorkspaceGrant,
    ) -> Result<(), WorkspaceError> {
        let grants = self.grants.read();
        let current = grants
            .get(&grant.projection.workspace_id)
            .ok_or(WorkspaceError::GrantUnavailable)?;
        if current.revoked
            || current.projection.grant_epoch != grant.projection.grant_epoch
            || current.root_identity_hash != grant.root_identity_hash
            || current.root != grant.root
        {
            return Err(WorkspaceError::GrantUnavailable);
        }
        Ok(())
    }
}

fn validate_root(selected_root: &Path) -> Result<PathBuf, WorkspaceError> {
    if !selected_root.is_absolute() || has_unsupported_prefix(selected_root) {
        return Err(WorkspaceError::UnsupportedRoot);
    }
    let selected_metadata = fs::symlink_metadata(selected_root)?;
    if selected_metadata.file_type().is_symlink()
        || is_reparse_point(&selected_metadata)
        || is_cloud_placeholder(&selected_metadata)
    {
        return Err(WorkspaceError::UnsupportedRoot);
    }
    let canonical = fs::canonicalize(selected_root)?;
    let metadata = fs::symlink_metadata(&canonical)?;
    if !metadata.is_dir()
        || is_reparse_point(&metadata)
        || is_cloud_placeholder(&metadata)
        || !is_supported_volume(&canonical)
    {
        return Err(WorkspaceError::UnsupportedRoot);
    }
    Ok(canonical)
}

fn revalidate_root(grant: &WorkspaceGrant) -> Result<(), WorkspaceError> {
    let canonical = fs::canonicalize(&grant.root)?;
    let metadata = fs::metadata(&canonical)?;
    if canonical != grant.root || root_identity(&canonical, &metadata)? != grant.root_identity_hash
    {
        return Err(WorkspaceError::RootIdentityChanged);
    }
    Ok(())
}

fn normalize_relative_path(relative_path: &str) -> Result<String, WorkspaceError> {
    if relative_path.is_empty()
        || relative_path.len() > MAX_RELATIVE_PATH_BYTES
        || relative_path.starts_with('/')
        || relative_path.ends_with('/')
        || relative_path.contains("//")
        || relative_path.contains('\\')
        || relative_path.chars().any(char::is_control)
    {
        return Err(WorkspaceError::InvalidRelativePath);
    }
    if relative_path == "." {
        return Ok(".".to_owned());
    }
    let mut segments = Vec::new();
    for segment in relative_path.split('/') {
        if matches!(segment, "" | "." | "..") {
            return Err(WorkspaceError::InvalidRelativePath);
        }
        validate_segment(segment)?;
        segments.push(segment);
    }
    if segments.is_empty() {
        return Err(WorkspaceError::InvalidRelativePath);
    }
    Ok(segments.join("/"))
}

fn validate_segment(segment: &str) -> Result<(), WorkspaceError> {
    if segment.is_empty()
        || segment.encode_utf16().count() > MAX_SEGMENT_UTF16_UNITS
        || segment.ends_with('.')
        || segment.ends_with(' ')
        || segment.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
        })
    {
        return Err(WorkspaceError::InvalidRelativePath);
    }
    let stem = segment
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(['.', ' '])
        .to_ascii_uppercase();
    if is_reserved_windows_device_name(&stem) {
        return Err(WorkspaceError::InvalidRelativePath);
    }
    Ok(())
}

fn is_reserved_windows_device_name(stem: &str) -> bool {
    if matches!(
        stem,
        "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$" | "CONIN$" | "CONOUT$"
    ) {
        return true;
    }
    ["COM", "LPT"].into_iter().any(|prefix| {
        stem.strip_prefix(prefix).is_some_and(|suffix| {
            matches!(
                suffix,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    })
}

fn resolve_existing(root: &Path, relative_path: &str) -> Result<PathBuf, WorkspaceError> {
    // Residual security blocker: this path-based canonicalize/reparse sequence
    // does not pin the selected root or descendant identity across later opens.
    // It is used only by the read-only broker and is not a governed-write proof.
    let normalized = normalize_relative_path(relative_path)?;
    if normalized == "." {
        return Ok(root.to_path_buf());
    }
    let candidate = root.join(normalized.replace('/', std::path::MAIN_SEPARATOR_STR));
    let canonical = fs::canonicalize(candidate)?;
    ensure_descendant(root, &canonical)?;
    reject_reparse_components(root, &canonical)?;
    Ok(canonical)
}

fn resolve_picked_file(root: &Path, candidate: &Path) -> Result<String, WorkspaceError> {
    if !candidate.is_absolute() {
        return Err(WorkspaceError::InvalidRelativePath);
    }
    let canonical = fs::canonicalize(candidate)?;
    let metadata = fs::symlink_metadata(&canonical)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || is_reparse_point(&metadata)
        || is_cloud_placeholder(&metadata)
    {
        return Err(WorkspaceError::PathBlocked);
    }
    // Component-wise containment: strip_prefix rejects the sibling-prefix trap
    // (`C:\root` vs `C:\rootx`) that a raw string prefix check would accept.
    ensure_descendant(root, &canonical)?;
    if canonical == root {
        return Err(WorkspaceError::InvalidRelativePath);
    }
    reject_reparse_components(root, &canonical)?;
    let relative = to_relative_wire_path(root, &canonical)?;
    if is_sensitive_relative_path(&relative) {
        return Err(WorkspaceError::PathBlocked);
    }
    Ok(relative)
}

fn ensure_descendant(root: &Path, candidate: &Path) -> Result<(), WorkspaceError> {
    if candidate == root || candidate.starts_with(root) {
        Ok(())
    } else {
        Err(WorkspaceError::OutsideWorkspace)
    }
}

fn reject_reparse_components(root: &Path, candidate: &Path) -> Result<(), WorkspaceError> {
    let relative = candidate
        .strip_prefix(root)
        .map_err(|_| WorkspaceError::OutsideWorkspace)?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        let metadata = fs::symlink_metadata(&current)?;
        if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
            return Err(WorkspaceError::PathBlocked);
        }
    }
    Ok(())
}

fn to_relative_wire_path(root: &Path, path: &Path) -> Result<String, WorkspaceError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| WorkspaceError::OutsideWorkspace)?;
    let value = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    normalize_relative_path(&value)
}

fn classify_entry(path: &Path, relative_path: &str, metadata: &fs::Metadata) -> EntryKind {
    if metadata.file_type().is_symlink()
        || is_reparse_point(metadata)
        || is_cloud_placeholder(metadata)
        || has_unexpected_hardlinks(path, metadata)
        || is_sensitive_relative_path(relative_path)
    {
        return EntryKind::Blocked;
    }
    if metadata.is_dir() {
        return EntryKind::Directory;
    }
    if !metadata.is_file() || metadata.len() > DEFAULT_MAX_TEXT_BYTES {
        return EntryKind::BinaryFile;
    }
    match File::open(path).and_then(|file| {
        let mut sample = Vec::new();
        file.take(8 * 1024).read_to_end(&mut sample)?;
        Ok(sample)
    }) {
        Ok(sample) if !sample.contains(&0) && std::str::from_utf8(&sample).is_ok() => {
            EntryKind::TextFile
        }
        _ => EntryKind::BinaryFile,
    }
}

fn read_bounded_file(path: &Path, limit: u64) -> Result<Option<Vec<u8>>, WorkspaceError> {
    let mut file = File::open(path)?;
    let capacity = usize::try_from(limit)
        .map_err(|_| WorkspaceError::LimitExceeded)?
        .checked_add(1)
        .ok_or(WorkspaceError::LimitExceeded)?;
    let mut bytes = Vec::with_capacity(capacity);
    file.by_ref()
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Ok(None);
    }
    Ok(Some(bytes))
}

fn classify_bmad_asset(relative_path: &str) -> Option<(BmadAssetKind, BmadActivation)> {
    let normalized = relative_path.to_ascii_lowercase();
    let (parent_path, file_name) = normalized.rsplit_once('/')?;
    let under_bmad = normalized.starts_with("_bmad/") || normalized.starts_with("bmad/");
    if !under_bmad {
        return None;
    }
    let under_builder = parent_path.split('/').any(|segment| {
        matches!(segment, "bmb" | "builder" | "builders") || segment.ends_with("-builder")
    });
    if under_builder {
        if normalized.contains("/build/") || file_name.starts_with("build-") {
            return Some((
                BmadAssetKind::BuilderBuildDraft,
                BmadActivation::InactiveDraft,
            ));
        }
        if normalized.contains("/edit/") || file_name.starts_with("edit-") {
            return Some((
                BmadAssetKind::BuilderEditDraft,
                BmadActivation::InactiveDraft,
            ));
        }
        if normalized.contains("/analyze/") || file_name.starts_with("analyze-") {
            return Some((
                BmadAssetKind::BuilderAnalyzeDraft,
                BmadActivation::InactiveDraft,
            ));
        }
    }
    let activation = if under_builder {
        BmadActivation::InactiveDraft
    } else {
        BmadActivation::ReadOnly
    };
    if file_name == "skill.md" || normalized.contains("/agents/") {
        return Some((BmadAssetKind::Agent, activation));
    }
    if normalized.contains("/workflows/")
        && matches!(
            Path::new(file_name)
                .extension()
                .and_then(|value| value.to_str()),
            Some("md" | "yaml" | "yml")
        )
    {
        return Some((BmadAssetKind::Workflow, activation));
    }
    if matches!(
        file_name,
        "config.yaml" | "config.yml" | "manifest.yaml" | "manifest.yml"
    ) {
        return Some((BmadAssetKind::MethodConfiguration, activation));
    }
    None
}

fn is_sensitive_relative_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    let segments = normalized
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.iter().copied().any(is_sensitive_path_segment) {
        return true;
    }

    segments
        .windows(2)
        .any(|pair| pair[0] == ".config" && CONFIG_CREDENTIAL_STORES.contains(&pair[1]))
        || segments.windows(3).any(|triple| {
            triple[0] == "appdata"
                && matches!(triple[1], "local" | "roaming")
                && APPDATA_CREDENTIAL_STORES.contains(&triple[2])
        })
}

fn is_sensitive_path_segment(segment: &str) -> bool {
    const SENSITIVE_DIRECTORIES: &[&str] = &[
        ".aws",
        ".azure",
        ".docker",
        ".git",
        ".gnupg",
        ".hg",
        ".kube",
        ".pulumi",
        ".ssh",
        ".svn",
        ".terraform.d",
        "node_modules",
        "target",
    ];
    const SENSITIVE_FILES: &[&str] = &[
        ".env",
        ".git-credentials",
        ".netrc",
        ".npmrc",
        ".pypirc",
        ".terraformrc",
        ".yarnrc",
        ".yarnrc.yml",
        "_netrc",
        "access_token",
        "access_token.json",
        "access_tokens.db",
        "application_default_credentials.json",
        "auth.json",
        "credentials",
        "credentials.json",
        "credentials.tfrc.json",
        "id_dsa",
        "id_ecdsa",
        "id_ed25519",
        "id_rsa",
        "msal_token_cache.bin",
        "refresh_token",
        "refresh_token.json",
        "terraform.rc",
        "token",
        "token.json",
        "tokens.json",
    ];
    const SENSITIVE_EXTENSIONS: &[&str] = &[
        ".jks",
        ".kdbx",
        ".key",
        ".keystore",
        ".p12",
        ".pem",
        ".pfx",
        ".ppk",
    ];

    SENSITIVE_DIRECTORIES.contains(&segment)
        || SENSITIVE_FILES.contains(&segment)
        || segment.starts_with(".env.")
        || SENSITIVE_EXTENSIONS
            .iter()
            .copied()
            .any(|extension| segment.ends_with(extension))
        || has_sensitive_name_marker(segment, "secret")
        || has_sensitive_name_marker(segment, "secrets")
        || has_sensitive_name_marker(segment, "token")
        || has_sensitive_name_marker(segment, "tokens")
        || segment.starts_with("service-account")
        || segment.starts_with("service_account")
        || segment.starts_with("private-key")
        || segment.starts_with("private_key")
}

fn has_sensitive_name_marker(value: &str, marker: &str) -> bool {
    value.split(['.', '-', '_']).any(|part| part == marker)
}

fn has_unsupported_prefix(path: &Path) -> bool {
    matches!(
        path.components().next(),
        Some(Component::Prefix(prefix))
            if !matches!(prefix.kind(), std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_))
    )
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

fn hash_bytes(value: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(value))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value
            .as_bytes()
            .iter()
            .skip(7)
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

#[cfg(windows)]
fn root_identity(path: &Path, metadata: &fs::Metadata) -> Result<String, WorkspaceError> {
    let information = file_information(path, metadata.is_dir())?;
    Ok(identity_hash_from_information(&information))
}

#[cfg(windows)]
fn identity_hash_from_information(
    information: &windows::Win32::Storage::FileSystem::BY_HANDLE_FILE_INFORMATION,
) -> String {
    let identity = format!(
        "{}:{:08x}{:08x}",
        information.dwVolumeSerialNumber, information.nFileIndexHigh, information.nFileIndexLow
    );
    hash_bytes(identity.as_bytes())
}

#[cfg(not(windows))]
fn root_identity(path: &Path, metadata: &fs::Metadata) -> Result<String, WorkspaceError> {
    let _ = path;
    let identity = format!("unsupported-platform:{}", metadata.len());
    Ok(hash_bytes(identity.as_bytes()))
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(windows)]
fn is_cloud_placeholder(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_OFFLINE: u32 = 0x0000_1000;
    const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x0004_0000;
    const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x0040_0000;
    metadata.file_attributes()
        & (FILE_ATTRIBUTE_OFFLINE
            | FILE_ATTRIBUTE_RECALL_ON_OPEN
            | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS)
        != 0
}

#[cfg(not(windows))]
fn is_cloud_placeholder(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(windows)]
fn has_unexpected_hardlinks(path: &Path, metadata: &fs::Metadata) -> bool {
    metadata.is_file()
        && file_information(path, false).map_or(true, |information| information.nNumberOfLinks > 1)
}

#[cfg(not(windows))]
fn has_unexpected_hardlinks(_path: &Path, _metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(windows)]
#[expect(
    unsafe_code,
    reason = "GetFileInformationByHandle requires a Win32 FFI call using a live owned file handle and a writable output structure"
)]
fn file_information(
    path: &Path,
    is_directory: bool,
) -> Result<windows::Win32::Storage::FileSystem::BY_HANDLE_FILE_INFORMATION, WorkspaceError> {
    use std::fs::OpenOptions;
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };

    let mut options = OpenOptions::new();
    options
        .read(true)
        .share_mode(FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0 | FILE_SHARE_DELETE.0);
    if is_directory {
        options.custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0);
    }
    let file = options.open(path)?;
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: the handle is borrowed from `file`, which stays alive through the call, and the
    // output pointer refers to an initialized, writable structure of the expected size.
    unsafe {
        GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &raw mut information)
            .map_err(|error| WorkspaceError::Io(std::io::Error::other(error)))?;
    }
    Ok(information)
}

#[cfg(windows)]
#[expect(
    unsafe_code,
    reason = "GetFileInformationByHandle requires a Win32 FFI call using a live owned file handle and a writable output structure"
)]
fn handle_information(
    file: &File,
) -> Result<windows::Win32::Storage::FileSystem::BY_HANDLE_FILE_INFORMATION, WorkspaceError> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };

    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: the handle is borrowed from `file`, which stays alive through the call, and the
    // output pointer refers to an initialized, writable structure of the expected size.
    unsafe {
        GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &raw mut information)
            .map_err(|error| WorkspaceError::Io(std::io::Error::other(error)))?;
    }
    Ok(information)
}

#[cfg(windows)]
#[expect(
    unsafe_code,
    reason = "GetDriveTypeW and GetVolumeInformationW require Win32 FFI calls using a validated NUL-terminated drive root and bounded writable buffer"
)]
fn is_supported_volume(path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{GetDriveTypeW, GetVolumeInformationW};
    use windows::Win32::System::WindowsProgramming::DRIVE_FIXED;

    let Some(prefix) = path.components().next() else {
        return false;
    };
    let Component::Prefix(prefix) = prefix else {
        return false;
    };
    let (std::path::Prefix::Disk(letter) | std::path::Prefix::VerbatimDisk(letter)) = prefix.kind()
    else {
        return false;
    };
    let root = format!("{}:\\", char::from(letter));
    let wide: Vec<u16> = std::ffi::OsStr::new(&root)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: `wide` is a valid, NUL-terminated UTF-16 drive root for both calls, and the
    // filesystem name buffer remains writable for the duration of `GetVolumeInformationW`.
    unsafe {
        if GetDriveTypeW(PCWSTR(wide.as_ptr())) != DRIVE_FIXED {
            return false;
        }
        let mut filesystem_name = [0_u16; 16];
        if GetVolumeInformationW(
            PCWSTR(wide.as_ptr()),
            None,
            None,
            None,
            None,
            Some(&mut filesystem_name),
        )
        .is_err()
        {
            return false;
        }
        let length = filesystem_name
            .iter()
            .position(|value| *value == 0)
            .unwrap_or(filesystem_name.len());
        String::from_utf16_lossy(&filesystem_name[..length]).eq_ignore_ascii_case("NTFS")
    }
}

#[cfg(not(windows))]
fn is_supported_volume(_path: &Path) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::{self, RecvTimeoutError};
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::Duration;

    use super::*;

    #[test]
    fn rejects_windows_path_escape_and_device_forms() {
        for value in [
            "../outside",
            "./inside",
            "inside/../outside",
            "/absolute",
            "//server/share",
            "C:/drive",
            "C:drive-relative",
            "folder\\file",
            r"\\?\C:\device",
            r"\\.\PhysicalDrive0",
            "folder/file:stream",
            "folder//file",
            "folder/",
            "folder/line\nbreak",
            "folder/control\u{0001}name",
            "folder/name<bad",
            "folder/name>bad",
            "folder/name\"bad",
            "folder/name|bad",
            "folder/name?bad",
            "folder/name*bad",
        ] {
            assert!(
                normalize_relative_path(value).is_err(),
                "expected {value:?} to be rejected"
            );
        }
    }

    #[test]
    fn rejects_all_windows_reserved_device_stems() {
        for value in [
            "CON",
            "folder/CON.txt",
            "folder/CON .txt",
            "folder/prn.log",
            "folder/AUX",
            "folder/nul.data",
            "folder/CLOCK$",
            "folder/conin$.txt",
            "folder/CONOUT$",
            "folder/COM1",
            "folder/com9.txt",
            "folder/LPT1",
            "folder/lpt9.txt",
            "folder/COM¹.txt",
            "folder/com²",
            "folder/COM³.log",
            "folder/LPT¹.txt",
            "folder/lpt²",
            "folder/LPT³.log",
            "folder/trailing.",
            "folder/trailing ",
        ] {
            assert!(
                normalize_relative_path(value).is_err(),
                "expected {value:?} to be rejected"
            );
        }
    }

    #[test]
    fn normalizes_wire_paths() -> Result<(), WorkspaceError> {
        assert_eq!(normalize_relative_path("src/main.rs")?, "src/main.rs");
        assert_eq!(normalize_relative_path("src/COM10.txt")?, "src/COM10.txt");
        assert_eq!(normalize_relative_path("src/LPT0.txt")?, "src/LPT0.txt");
        assert_eq!(normalize_relative_path(".")?, ".");
        Ok(())
    }

    #[test]
    fn enforces_bounded_relative_paths_and_segments() {
        let overlong_segment = "a".repeat(MAX_SEGMENT_UTF16_UNITS + 1);
        let overlong_path = "a".repeat(MAX_RELATIVE_PATH_BYTES + 1);
        assert!(normalize_relative_path(&overlong_segment).is_err());
        assert!(normalize_relative_path(&overlong_path).is_err());
        assert!(normalize_relative_path(&"a".repeat(MAX_SEGMENT_UTF16_UNITS)).is_ok());
    }

    #[test]
    fn blocks_common_secret_files_and_credential_stores() {
        for value in [
            ".env.local",
            ".npmrc",
            "nested/.netrc",
            "nested/_netrc",
            ".ssh/id_ed25519",
            ".aws/credentials",
            ".azure/msal_token_cache.bin",
            ".config/gcloud/application_default_credentials.json",
            ".config/gh/hosts.yml",
            ".kube/config",
            ".docker/config.json",
            ".gnupg/private-keys-v1.d/key",
            ".terraform.d/credentials.tfrc.json",
            "credentials.json",
            "token.json",
            "oauth-token.json",
            "access_tokens.db",
            "client_secret.json",
            "service_account_key.json",
            "id_rsa",
            "certificates/signing.pfx",
            "AppData/Roaming/gcloud/credentials.db",
        ] {
            assert!(
                is_sensitive_relative_path(value),
                "expected {value:?} to be blocked"
            );
        }
    }

    #[test]
    fn does_not_block_similarly_named_source_files() {
        for value in [
            "src/tokenizer.ts",
            "src/secretary.ts",
            "docs/credentials-guide.md",
            "examples/aws-client.rs",
        ] {
            assert!(
                !is_sensitive_relative_path(value),
                "expected {value:?} to remain visible"
            );
        }
    }

    #[test]
    fn revoke_advances_epoch_and_invalidates_snapshots() -> Result<(), WorkspaceError> {
        let broker = broker_with_grant(1);
        let authority = broker.revocation_barrier.read();
        let stale = broker.active_grant(&authority, "workspace_test")?;
        drop(authority);
        let revoked = broker.revoke("workspace_test")?;
        assert_eq!(revoked.grant_epoch, 2);
        let authority = broker.revocation_barrier.read();
        assert!(matches!(
            broker.active_grant(&authority, "workspace_test"),
            Err(WorkspaceError::GrantUnavailable)
        ));
        assert!(matches!(
            broker.ensure_grant_current(&authority, &stale),
            Err(WorkspaceError::GrantUnavailable)
        ));
        drop(authority);
        assert!(matches!(
            broker.revoke("workspace_test"),
            Err(WorkspaceError::GrantUnavailable)
        ));
        Ok(())
    }

    #[test]
    fn revoke_fails_closed_instead_of_reusing_max_epoch() {
        let broker = broker_with_grant(u64::MAX);
        assert!(matches!(
            broker.revoke("workspace_test"),
            Err(WorkspaceError::GrantUnavailable)
        ));
        let authority = broker.revocation_barrier.read();
        assert!(matches!(
            broker.active_grant(&authority, "workspace_test"),
            Err(WorkspaceError::GrantUnavailable)
        ));
    }

    #[test]
    fn read_authority_blocks_revocation_until_release() -> Result<(), String> {
        let broker = Arc::new(broker_with_grant(1));
        let authority = broker.revocation_barrier.read();
        assert!(broker.revocation_barrier.try_write().is_none());

        let start = Arc::new(Barrier::new(2));
        let (attempted_tx, attempted_rx) = mpsc::channel();
        let (completed_tx, completed_rx) = mpsc::channel();
        let worker_broker = Arc::clone(&broker);
        let worker_start = Arc::clone(&start);
        let worker = thread::spawn(move || {
            worker_start.wait();
            let _ = attempted_tx.send(());
            let result = worker_broker.revoke("workspace_test");
            let _ = completed_tx.send(result);
        });

        start.wait();
        attempted_rx
            .recv_timeout(Duration::from_secs(2))
            .map_err(|error| format!("revocation worker did not start: {error}"))?;
        assert!(matches!(
            completed_rx.recv_timeout(Duration::from_millis(100)),
            Err(RecvTimeoutError::Timeout)
        ));

        drop(authority);
        let revoked = completed_rx
            .recv_timeout(Duration::from_secs(2))
            .map_err(|error| format!("revocation did not complete after read release: {error}"))?
            .map_err(|error| error.to_string())?;
        worker
            .join()
            .map_err(|_| "revocation worker panicked".to_owned())?;

        assert_eq!(revoked.grant_epoch, 2);
        let authority = broker.revocation_barrier.read();
        assert!(matches!(
            broker.active_grant(&authority, "workspace_test"),
            Err(WorkspaceError::GrantUnavailable)
        ));
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn scope_authority_guard_exposes_exact_cloned_scope_facts(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let selected_root = tempfile::tempdir()?;
        let broker = WorkspaceBroker::new();
        let projection = broker.grant("project_test", selected_root.path())?;
        let expected_root_identity_hash = broker
            .grants
            .read()
            .get(&projection.workspace_id)
            .ok_or(WorkspaceError::GrantUnavailable)?
            .root_identity_hash
            .clone();

        let guard = broker.authorize_scope(&projection.workspace_id, projection.grant_epoch)?;
        assert_eq!(guard.projection(), projection);
        assert_eq!(guard.projection().project_id, "project_test");
        assert_eq!(
            guard.authority_binding(),
            WorkspaceAuthorityBinding {
                workspace_id: projection.workspace_id.clone(),
                grant_epoch: projection.grant_epoch,
                context_read_epoch: projection.context_read_epoch,
                governed_edit_epoch: projection.governed_edit_epoch,
                root_identity_hash: expected_root_identity_hash,
            }
        );

        let mut detached_projection = guard.projection();
        detached_projection.project_id = "untrusted_change".to_owned();
        let mut detached_binding = guard.authority_binding();
        detached_binding.root_identity_hash = format!("sha256:{}", "f".repeat(64));
        assert_eq!(guard.projection(), projection);
        assert_ne!(
            guard.authority_binding().root_identity_hash,
            detached_binding.root_identity_hash
        );
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn resolve_picked_files_keeps_only_files_inside_the_granted_root(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        fs::write(root.path().join("keep.txt"), b"inside")?;
        fs::create_dir(root.path().join("nested"))?;
        fs::write(root.path().join("nested").join("child.md"), b"nested")?;

        // A sibling directory whose absolute path shares the root's string prefix
        // (`root` vs `rootx`) — the classic containment trap.
        let sibling = tempfile::Builder::new().prefix("workspacex").tempdir()?;
        fs::write(sibling.path().join("outside.txt"), b"outside")?;

        let broker = WorkspaceBroker::new();
        let projection = broker.grant("project_test", root.path())?;

        let picked = broker.resolve_picked_files(
            &projection.workspace_id,
            &[
                root.path().join("keep.txt"),
                root.path().join("nested").join("child.md"),
                sibling.path().join("outside.txt"),
                root.path().join("missing.txt"),
                root.path().to_path_buf(),
            ],
            100,
        )?;

        assert_eq!(picked.relative_paths, vec!["keep.txt", "nested/child.md"]);
        // The sibling file and the root directory itself are outside-root
        // rejections; the missing file is an unreadable rejection.
        assert_eq!(picked.rejected_outside_root, 2);
        assert_eq!(picked.rejected_unreadable, 1);
        assert!(!picked.truncated);
        Ok(())
    }

    #[test]
    fn resolve_picked_files_dedups_sorts_and_reports_truncation(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        for name in ["b.txt", "a.txt", "c.txt"] {
            fs::write(root.path().join(name), b"x")?;
        }
        let broker = WorkspaceBroker::new();
        let projection = broker.grant("project_test", root.path())?;

        let picked = broker.resolve_picked_files(
            &projection.workspace_id,
            &[
                root.path().join("b.txt"),
                root.path().join("b.txt"),
                root.path().join("a.txt"),
                root.path().join("c.txt"),
            ],
            2,
        )?;
        assert_eq!(picked.relative_paths, vec!["a.txt", "b.txt"]);
        assert!(picked.truncated);
        assert_eq!(picked.rejected_outside_root, 0);
        Ok(())
    }

    #[test]
    fn scope_authority_guard_rejects_zero_stale_and_mismatched_epochs(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let selected_root = tempfile::tempdir()?;
        let broker = WorkspaceBroker::new();
        let projection = broker.grant("project_test", selected_root.path())?;
        broker
            .grants
            .write()
            .get_mut(&projection.workspace_id)
            .ok_or(WorkspaceError::GrantUnavailable)?
            .projection
            .grant_epoch = 2;

        for expected_epoch in [0, 1, 3] {
            assert!(matches!(
                broker.authorize_scope(&projection.workspace_id, expected_epoch),
                Err(WorkspaceError::GrantUnavailable)
            ));
        }
        assert!(broker.authorize_scope(&projection.workspace_id, 2).is_ok());
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn scope_authority_guard_blocks_revoke_until_dropped() -> Result<(), String> {
        let selected_root = tempfile::tempdir().map_err(|error| error.to_string())?;
        let broker = Arc::new(WorkspaceBroker::new());
        let projection = broker
            .grant("project_test", selected_root.path())
            .map_err(|error| error.to_string())?;
        let guard = broker
            .authorize_scope(&projection.workspace_id, projection.grant_epoch)
            .map_err(|error| error.to_string())?;

        let (attempted_tx, attempted_rx) = mpsc::channel();
        let (completed_tx, completed_rx) = mpsc::channel();
        let worker_broker = Arc::clone(&broker);
        let workspace_id = projection.workspace_id.clone();
        let worker = thread::spawn(move || {
            let _ = attempted_tx.send(());
            let result = worker_broker.revoke(&workspace_id);
            let _ = completed_tx.send(result);
        });

        attempted_rx
            .recv_timeout(Duration::from_secs(2))
            .map_err(|error| format!("revocation worker did not start: {error}"))?;
        assert!(matches!(
            completed_rx.recv_timeout(Duration::from_millis(100)),
            Err(RecvTimeoutError::Timeout)
        ));

        drop(guard);
        let revoked = completed_rx
            .recv_timeout(Duration::from_secs(2))
            .map_err(|error| format!("revocation did not complete after guard drop: {error}"))?
            .map_err(|error| error.to_string())?;
        worker
            .join()
            .map_err(|_| "revocation worker panicked".to_owned())?;
        assert_eq!(revoked.grant_epoch, projection.grant_epoch + 1);
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn scope_authority_guard_fails_closed_after_revoke_or_root_drift(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let revoked_root = tempfile::tempdir()?;
        let revoked_broker = WorkspaceBroker::new();
        let revoked_projection = revoked_broker.grant("project_revoked", revoked_root.path())?;
        let revoked = revoked_broker.revoke(&revoked_projection.workspace_id)?;
        assert!(matches!(
            revoked_broker.authorize_scope(&revoked.workspace_id, revoked.grant_epoch),
            Err(WorkspaceError::GrantUnavailable)
        ));

        let drifted_root = tempfile::tempdir()?;
        let drifted_broker = WorkspaceBroker::new();
        let drifted_projection = drifted_broker.grant("project_drifted", drifted_root.path())?;
        drifted_broker
            .grants
            .write()
            .get_mut(&drifted_projection.workspace_id)
            .ok_or(WorkspaceError::GrantUnavailable)?
            .root_identity_hash = format!("sha256:{}", "0".repeat(64));
        assert!(matches!(
            drifted_broker.authorize_scope(
                &drifted_projection.workspace_id,
                drifted_projection.grant_epoch
            ),
            Err(WorkspaceError::RootIdentityChanged)
        ));
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn new_and_restored_grants_project_read_only() -> Result<(), Box<dyn std::error::Error>> {
        let selected_root = tempfile::tempdir()?;
        let broker = WorkspaceBroker::new();
        let mut projection = broker.grant("project_test", selected_root.path())?;
        assert_eq!(projection.permissions, WorkspacePermissions::ReadOnly);
        let binding = broker.authority_binding(&projection.workspace_id)?;

        projection.permissions = WorkspacePermissions::GovernedEdits;
        let restored = WorkspaceBroker::new();
        restored.restore_grant(
            projection,
            selected_root.path(),
            &binding.root_identity_hash,
        )?;
        assert_eq!(
            restored.list().first().map(|item| item.permissions),
            Some(WorkspacePermissions::ReadOnly)
        );
        Ok(())
    }

    #[test]
    fn persisted_projections_without_vertical_epochs_default_to_one(
    ) -> Result<(), serde_json::Error> {
        let legacy = serde_json::json!({
            "workspaceId": "workspace_test",
            "projectId": "project_test",
            "displayName": "Workspace",
            "grantEpoch": 3,
            "permissions": "read_only",
        });
        let projection: WorkspaceProjection = serde_json::from_value(legacy)?;
        assert_eq!(projection.grant_epoch, 3);
        assert_eq!(projection.context_read_epoch, 1);
        assert_eq!(projection.governed_edit_epoch, 1);
        Ok(())
    }

    fn broker_with_grant(grant_epoch: u64) -> WorkspaceBroker {
        let broker = WorkspaceBroker::new();
        broker.grants.write().insert(
            "workspace_test".to_owned(),
            WorkspaceGrant {
                projection: WorkspaceProjection {
                    workspace_id: "workspace_test".to_owned(),
                    project_id: "project_test".to_owned(),
                    display_name: "Workspace".to_owned(),
                    grant_epoch,
                    permissions: WorkspacePermissions::ReadOnly,
                    context_read_epoch: 1,
                    governed_edit_epoch: 1,
                },
                root: PathBuf::from(r"C:\workspace"),
                root_identity_hash: format!("sha256:{}", "0".repeat(64)),
                revoked: false,
            },
        );
        broker
    }

    #[test]
    fn truncates_by_unicode_scalar_not_byte() {
        assert_eq!(truncate_chars("alpha βeta", 7), "alpha β…");
    }

    #[test]
    fn classifies_builder_outputs_as_inactive_drafts() {
        assert_eq!(
            classify_bmad_asset("_bmad/builder/build/build-agent.yaml"),
            Some((
                BmadAssetKind::BuilderBuildDraft,
                BmadActivation::InactiveDraft,
            ))
        );
        assert_eq!(
            classify_bmad_asset("_bmad/builder/edit/edit-workflow.md"),
            Some((
                BmadAssetKind::BuilderEditDraft,
                BmadActivation::InactiveDraft,
            ))
        );
        assert_eq!(
            classify_bmad_asset("_bmad/builder/analyze/analyze-agent.md"),
            Some((
                BmadAssetKind::BuilderAnalyzeDraft,
                BmadActivation::InactiveDraft,
            ))
        );
        assert_eq!(classify_bmad_asset("_bmad/builder/convert/output.md"), None);
        assert_eq!(
            classify_bmad_asset("_bmad/builder/agents/example/SKILL.md"),
            Some((BmadAssetKind::Agent, BmadActivation::InactiveDraft))
        );
        assert_eq!(
            classify_bmad_asset("_bmad/bmb/agents/example/SKILL.md"),
            Some((BmadAssetKind::Agent, BmadActivation::InactiveDraft))
        );
        assert_eq!(
            classify_bmad_asset("_bmad/notbuilder/agents/example/SKILL.md"),
            Some((BmadAssetKind::Agent, BmadActivation::ReadOnly))
        );
    }

    #[test]
    fn bounded_file_read_rejects_content_past_the_limit() -> Result<(), WorkspaceError> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("growing.txt");
        fs::write(&path, vec![b'a'; 33])?;

        assert!(read_bounded_file(&path, 32)?.is_none());
        assert_eq!(
            read_bounded_file(&path, 33)?.map(|bytes| bytes.len()),
            Some(33)
        );
        Ok(())
    }

    #[test]
    fn walk_budget_fails_closed_at_the_shared_ceiling() {
        let mut budget = WalkBudget {
            visited: MAX_WALK_ENTRIES.saturating_sub(1),
        };
        assert!(budget.visit().is_ok());
        assert!(matches!(budget.visit(), Err(WorkspaceError::LimitExceeded)));
    }

    #[test]
    fn entry_policy_uses_workspace_relative_path() -> Result<(), WorkspaceError> {
        let directory = tempfile::tempdir()?;
        let workspace = directory.path().join("target").join("workspace");
        let path = workspace.join("src").join("main.rs");
        fs::create_dir_all(path.parent().ok_or(WorkspaceError::InvalidRelativePath)?)?;
        fs::write(&path, b"fn main() {}\n")?;
        let metadata = fs::metadata(&path)?;

        assert_eq!(
            classify_entry(&path, "src/main.rs", &metadata),
            EntryKind::TextFile
        );
        Ok(())
    }
}
