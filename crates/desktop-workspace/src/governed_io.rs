//! Governed durable file effects for a selected workspace root.
//!
//! Every mutation in this module requires an explicit [`WorkspacePermissions::GovernedEdits`]
//! grant at an exact epoch, revalidates the root identity around the effect,
//! verifies the expected preimage through an open handle that denies concurrent
//! writers, and durably flushes both the file and its owning directory. The
//! module never accepts an absolute path and never creates parent directories.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[cfg(windows)]
use std::collections::HashMap;
#[cfg(windows)]
use std::io::{Seek, SeekFrom};
#[cfg(windows)]
use std::sync::Arc;

use ulid::Ulid;

use crate::{
    hash_bytes, is_cloud_placeholder, is_reparse_point, is_sensitive_relative_path, is_sha256,
    normalize_relative_path, resolve_existing, revalidate_root, WorkspaceBroker, WorkspaceError,
    WorkspaceGrant, WorkspacePermissions, WorkspaceProjection,
};

/// Upper bound for one governed file preimage or postimage.
pub const MAX_GOVERNED_FILE_BYTES: u64 = 1024 * 1024;

#[cfg(windows)]
const WINDOWS_FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;

/// Host-observed facts about one workspace path before a governed effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreimageObservation {
    pub relative_path: String,
    pub exists: bool,
    pub content: Option<String>,
    pub content_hash: Option<String>,
    pub file_identity_hash: Option<String>,
    pub metadata_hash: Option<String>,
}

struct VerifiedFile {
    file: File,
    content: Vec<u8>,
    content_hash: String,
    file_identity_hash: String,
    metadata_hash: String,
}

#[cfg(windows)]
struct PinnedRecoveryPath {
    directories: Vec<Arc<File>>,
    target: PathBuf,
    verified_file: Option<VerifiedFile>,
}

/// Broker-owned recovery scope which retains the exact governed grant and its
/// revocation barrier across final observation and every durable effect.
/// Recovery mutation is supported only on Windows, where native handles pin
/// the root, ancestors, parent entry, and existing target. Other platforms can
/// observe for diagnostics but fail mutation closed with `UnsupportedRoot`.
pub struct GovernedRecoveryTransaction<'a> {
    broker: &'a WorkspaceBroker,
    authority: parking_lot::RwLockReadGuard<'a, ()>,
    grant: WorkspaceGrant,
    #[cfg(windows)]
    recovery_paths: parking_lot::Mutex<HashMap<String, PinnedRecoveryPath>>,
    #[cfg(windows)]
    recovery_directories: parking_lot::Mutex<HashMap<String, Arc<File>>>,
}

impl GovernedRecoveryTransaction<'_> {
    /// Reobserves one path with its broker-owned file identity.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError`] when the path, root, or retained grant fails
    /// revalidation.
    pub fn observe_preimage(
        &self,
        relative_path: &str,
    ) -> Result<PreimageObservation, WorkspaceError> {
        revalidate_root(&self.grant)?;
        #[cfg(windows)]
        let observation = {
            let (observation, pinned) =
                pin_recovery_path(&self.grant, relative_path, &self.recovery_directories)?;
            self.recovery_paths
                .lock()
                .insert(observation.relative_path.clone(), pinned);
            observation
        };
        #[cfg(not(windows))]
        let observation = observe_preimage_for_grant(&self.grant, relative_path)?;
        self.broker
            .ensure_grant_current(&self.authority, &self.grant)?;
        Ok(observation)
    }

    /// Creates a durably flushed UTF-8 file in the retained recovery scope.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError`] when validation or the durable create fails.
    pub fn create_utf8_durable(
        &self,
        relative_path: &str,
        content: &str,
    ) -> Result<(), WorkspaceError> {
        revalidate_root(&self.grant)?;
        #[cfg(windows)]
        self.mutate_pinned(relative_path, |pinned| {
            recovery_create_pinned(&pinned, content, |_, _| Ok(()))
        })?;
        #[cfg(not(windows))]
        return Err(WorkspaceError::UnsupportedRoot);
        self.broker
            .ensure_grant_current(&self.authority, &self.grant)
    }

    /// Replaces an exact identity-bound preimage in the retained scope.
    ///
    /// Recovery intentionally rewrites the retained verified handle in place,
    /// rather than performing the ordinary adapter's atomic path replacement.
    /// The journal is already durably `restoring`; interruption therefore
    /// becomes terminal `manual_review` and is never retried automatically.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError`] when validation or replacement fails.
    pub fn replace_utf8_durable(
        &self,
        relative_path: &str,
        expected_content_hash: &str,
        expected_file_identity_hash: &str,
        content: &str,
    ) -> Result<(), WorkspaceError> {
        revalidate_root(&self.grant)?;
        #[cfg(windows)]
        self.mutate_pinned(relative_path, |pinned| {
            recovery_replace_pinned(
                pinned,
                expected_content_hash,
                expected_file_identity_hash,
                content,
                |_, _| Ok(()),
            )
        })?;
        #[cfg(not(windows))]
        return Err(WorkspaceError::UnsupportedRoot);
        self.broker
            .ensure_grant_current(&self.authority, &self.grant)
    }

    /// Deletes an exact identity-bound preimage in the retained scope.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError`] when validation or deletion fails.
    pub fn delete_durable(
        &self,
        relative_path: &str,
        expected_content_hash: &str,
        expected_file_identity_hash: &str,
    ) -> Result<(), WorkspaceError> {
        revalidate_root(&self.grant)?;
        #[cfg(windows)]
        self.mutate_pinned(relative_path, |pinned| {
            recovery_delete_pinned(
                pinned,
                expected_content_hash,
                expected_file_identity_hash,
                |_, _| Ok(()),
            )
        })?;
        #[cfg(not(windows))]
        return Err(WorkspaceError::UnsupportedRoot);
        self.broker
            .ensure_grant_current(&self.authority, &self.grant)
    }
}

#[cfg(windows)]
impl GovernedRecoveryTransaction<'_> {
    fn mutate_pinned(
        &self,
        relative_path: &str,
        mutation: impl FnOnce(PinnedRecoveryPath) -> Result<(), WorkspaceError>,
    ) -> Result<(), WorkspaceError> {
        let normalized = validate_effect_path(relative_path)?;
        let pinned = self
            .recovery_paths
            .lock()
            .remove(&normalized)
            .ok_or(WorkspaceError::StalePreimage)?;
        mutation(pinned)
    }
}

impl WorkspaceBroker {
    /// Runs final recovery validation and effects while retaining one exact
    /// governed grant and the broker's revocation barrier.
    ///
    /// The nested result keeps broker scope failures distinct from the
    /// caller's closed recovery-domain result.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError`] when the exact governed scope cannot be
    /// acquired or revalidated.
    pub fn with_governed_recovery<T, E>(
        &self,
        workspace_id: &str,
        expected_grant_epoch: u64,
        expected_governed_edit_epoch: u64,
        operation: impl FnOnce(&GovernedRecoveryTransaction<'_>) -> Result<T, E>,
    ) -> Result<Result<T, E>, WorkspaceError> {
        let authority = self.revocation_barrier.read();
        let grant = self.governed_grant(
            &authority,
            workspace_id,
            expected_grant_epoch,
            expected_governed_edit_epoch,
        )?;
        let transaction = GovernedRecoveryTransaction {
            broker: self,
            authority,
            grant,
            #[cfg(windows)]
            recovery_paths: parking_lot::Mutex::new(HashMap::new()),
            #[cfg(windows)]
            recovery_directories: parking_lot::Mutex::new(HashMap::new()),
        };
        let result = operation(&transaction);
        revalidate_root(&transaction.grant)?;
        transaction
            .broker
            .ensure_grant_current(&transaction.authority, &transaction.grant)?;
        Ok(result)
    }

    /// Enables governed edits for an active grant and advances its
    /// governed-edit epoch.
    ///
    /// The edit-epoch bump invalidates every proposal, spec, and edit
    /// authority binding issued against the previous edit epoch. The
    /// workspace binding epoch and the D2 context-read epoch are untouched
    /// (ADR-0002): escalating edit authority neither invalidates D1 reads
    /// nor an in-review Help request.
    ///
    /// # Errors
    ///
    /// Returns an error when the grant is absent or revoked, the root identity
    /// changed, or the governed-edit epoch cannot advance without wrapping.
    pub fn enable_governed_edits(
        &self,
        workspace_id: &str,
    ) -> Result<WorkspaceProjection, WorkspaceError> {
        let _authority = self.revocation_barrier.write();
        let mut grants = self.grants.write();
        let grant = grants
            .get_mut(workspace_id)
            .filter(|grant| !grant.revoked)
            .ok_or(WorkspaceError::GrantUnavailable)?;
        revalidate_root(grant)?;
        let next_epoch = grant
            .projection
            .governed_edit_epoch
            .checked_add(1)
            .ok_or(WorkspaceError::GrantUnavailable)?;
        grant.projection.governed_edit_epoch = next_epoch;
        grant.projection.permissions = WorkspacePermissions::GovernedEdits;
        Ok(grant.projection.clone())
    }

    /// Observes one workspace path for proposal preparation.
    ///
    /// Existing files are read completely through a handle that denies
    /// concurrent writers; absent paths are validated down to their existing
    /// parent directory so a later create cannot escape the root.
    ///
    /// # Errors
    ///
    /// Returns an error when governed edits are not enabled at the expected
    /// epoch, the path is invalid, blocked, oversized, or not UTF-8 text, or
    /// when root or grant revalidation fails.
    pub fn observe_preimage(
        &self,
        workspace_id: &str,
        expected_grant_epoch: u64,
        expected_governed_edit_epoch: u64,
        relative_path: &str,
    ) -> Result<PreimageObservation, WorkspaceError> {
        let authority = self.revocation_barrier.read();
        let grant = self.governed_grant(
            &authority,
            workspace_id,
            expected_grant_epoch,
            expected_governed_edit_epoch,
        )?;
        let observation = observe_preimage_for_grant(&grant, relative_path)?;
        self.ensure_grant_current(&authority, &grant)?;
        Ok(observation)
    }

    /// Reads one governed file completely for checkpoint capture.
    ///
    /// Returns `Ok(None)` when the path validates but no file exists.
    ///
    /// # Errors
    ///
    /// Returns an error when governed edits are not enabled at the expected
    /// epoch, the path is invalid or blocked, the observed file identity does
    /// not match `expected_file_identity_hash`, or an I/O operation fails.
    pub fn read_effect_file(
        &self,
        workspace_id: &str,
        expected_grant_epoch: u64,
        expected_governed_edit_epoch: u64,
        relative_path: &str,
        expected_file_identity_hash: Option<&str>,
    ) -> Result<Option<Vec<u8>>, WorkspaceError> {
        let authority = self.revocation_barrier.read();
        let grant = self.governed_grant(
            &authority,
            workspace_id,
            expected_grant_epoch,
            expected_governed_edit_epoch,
        )?;
        let normalized = validate_effect_path(relative_path)?;
        let bytes = match resolve_existing(&grant.root, &normalized) {
            Ok(path) => {
                let verified = open_verified_file(&path)?;
                if expected_file_identity_hash
                    .is_some_and(|expected| expected != verified.file_identity_hash)
                {
                    return Err(WorkspaceError::StalePreimage);
                }
                Some(verified.content)
            }
            Err(WorkspaceError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                resolve_existing_parent(&grant.root, &normalized)?;
                None
            }
            Err(error) => return Err(error),
        };
        self.ensure_grant_current(&authority, &grant)?;
        Ok(bytes)
    }

    /// Creates a new governed UTF-8 file and durably flushes it and its
    /// owning directory. The parent directory must already exist.
    ///
    /// # Errors
    ///
    /// Returns an error when governed edits are not enabled at the expected
    /// epoch, the path is invalid or blocked, the file already exists, the
    /// content is unsupported, or a durable write fails.
    pub fn create_utf8_durable(
        &self,
        workspace_id: &str,
        expected_grant_epoch: u64,
        expected_governed_edit_epoch: u64,
        relative_path: &str,
        content: &str,
    ) -> Result<(), WorkspaceError> {
        let authority = self.revocation_barrier.read();
        let grant = self.governed_grant(
            &authority,
            workspace_id,
            expected_grant_epoch,
            expected_governed_edit_epoch,
        )?;
        create_utf8_for_grant(&grant, relative_path, content)?;
        self.ensure_grant_current(&authority, &grant)?;
        Ok(())
    }

    /// Atomically replaces an existing governed file after verifying its exact
    /// preimage through a handle that denies concurrent writers, then durably
    /// flushes the replacement and its owning directory.
    ///
    /// # Errors
    ///
    /// Returns an error when governed edits are not enabled at the expected
    /// epoch, the path is invalid or blocked, the observed content or file
    /// identity does not match the expected preimage, the content is
    /// unsupported, or the durable replacement fails.
    #[expect(
        clippy::too_many_arguments,
        reason = "the exact dual-epoch authority facts are deliberate parameters"
    )]
    pub fn replace_utf8_durable(
        &self,
        workspace_id: &str,
        expected_grant_epoch: u64,
        expected_governed_edit_epoch: u64,
        relative_path: &str,
        expected_content_hash: &str,
        expected_file_identity_hash: &str,
        content: &str,
    ) -> Result<(), WorkspaceError> {
        let authority = self.revocation_barrier.read();
        let grant = self.governed_grant(
            &authority,
            workspace_id,
            expected_grant_epoch,
            expected_governed_edit_epoch,
        )?;
        replace_utf8_for_grant(
            &grant,
            relative_path,
            expected_content_hash,
            expected_file_identity_hash,
            content,
        )?;
        self.ensure_grant_current(&authority, &grant)?;
        Ok(())
    }

    /// Deletes an existing governed file after verifying its exact preimage,
    /// then durably flushes the owning directory.
    ///
    /// # Errors
    ///
    /// Returns an error when governed edits are not enabled at the expected
    /// epoch, the path is invalid or blocked, the observed content or file
    /// identity does not match the expected preimage, or the durable deletion
    /// fails.
    pub fn delete_durable(
        &self,
        workspace_id: &str,
        expected_grant_epoch: u64,
        expected_governed_edit_epoch: u64,
        relative_path: &str,
        expected_content_hash: &str,
        expected_file_identity_hash: &str,
    ) -> Result<(), WorkspaceError> {
        let authority = self.revocation_barrier.read();
        let grant = self.governed_grant(
            &authority,
            workspace_id,
            expected_grant_epoch,
            expected_governed_edit_epoch,
        )?;
        delete_for_grant(
            &grant,
            relative_path,
            expected_content_hash,
            expected_file_identity_hash,
        )?;
        self.ensure_grant_current(&authority, &grant)?;
        Ok(())
    }

    fn governed_grant(
        &self,
        authority: &parking_lot::RwLockReadGuard<'_, ()>,
        workspace_id: &str,
        expected_grant_epoch: u64,
        expected_governed_edit_epoch: u64,
    ) -> Result<WorkspaceGrant, WorkspaceError> {
        let grant = self.active_grant(authority, workspace_id)?;
        if expected_grant_epoch == 0 || grant.projection.grant_epoch != expected_grant_epoch {
            return Err(WorkspaceError::GrantUnavailable);
        }
        if expected_governed_edit_epoch == 0
            || grant.projection.governed_edit_epoch != expected_governed_edit_epoch
        {
            return Err(WorkspaceError::GrantUnavailable);
        }
        if grant.projection.permissions != WorkspacePermissions::GovernedEdits {
            return Err(WorkspaceError::EditsNotEnabled);
        }
        revalidate_root(&grant)?;
        Ok(grant)
    }
}

fn observe_preimage_for_grant(
    grant: &WorkspaceGrant,
    relative_path: &str,
) -> Result<PreimageObservation, WorkspaceError> {
    let normalized = validate_effect_path(relative_path)?;
    match resolve_existing(&grant.root, &normalized) {
        Ok(path) => {
            let verified = open_verified_file(&path)?;
            let content =
                String::from_utf8(verified.content).map_err(|_| WorkspaceError::UnsupportedText)?;
            Ok(PreimageObservation {
                relative_path: normalized,
                exists: true,
                content: Some(content),
                content_hash: Some(verified.content_hash),
                file_identity_hash: Some(verified.file_identity_hash),
                metadata_hash: Some(verified.metadata_hash),
            })
        }
        Err(WorkspaceError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            resolve_existing_parent(&grant.root, &normalized)?;
            Ok(PreimageObservation {
                relative_path: normalized,
                exists: false,
                content: None,
                content_hash: None,
                file_identity_hash: None,
                metadata_hash: None,
            })
        }
        Err(error) => Err(error),
    }
}

#[cfg(windows)]
fn pin_recovery_path(
    grant: &WorkspaceGrant,
    relative_path: &str,
    directory_cache: &parking_lot::Mutex<HashMap<String, Arc<File>>>,
) -> Result<(PreimageObservation, PinnedRecoveryPath), WorkspaceError> {
    let normalized = validate_effect_path(relative_path)?;
    let parent_relative = normalized.rsplit_once('/').map(|(parent, _)| parent);
    let directories = pin_recovery_directories(grant, parent_relative, directory_cache)?;
    let parent = parent_relative.map_or_else(
        || grant.root.clone(),
        |relative| grant.root.join(relative.replace('/', "\\")),
    );
    let target = parent.join(leaf_segment(&normalized)?);
    match open_recovery_verified_file(&target) {
        Ok(verified) => {
            let content = String::from_utf8(verified.content.clone())
                .map_err(|_| WorkspaceError::UnsupportedText)?;
            let observation = PreimageObservation {
                relative_path: normalized,
                exists: true,
                content: Some(content),
                content_hash: Some(verified.content_hash.clone()),
                file_identity_hash: Some(verified.file_identity_hash.clone()),
                metadata_hash: Some(verified.metadata_hash.clone()),
            };
            Ok((
                observation,
                PinnedRecoveryPath {
                    directories,
                    target,
                    verified_file: Some(verified),
                },
            ))
        }
        Err(WorkspaceError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => Ok((
            PreimageObservation {
                relative_path: normalized,
                exists: false,
                content: None,
                content_hash: None,
                file_identity_hash: None,
                metadata_hash: None,
            },
            PinnedRecoveryPath {
                directories,
                target,
                verified_file: None,
            },
        )),
        Err(error) => Err(error),
    }
}

#[cfg(windows)]
fn pin_recovery_directories(
    grant: &WorkspaceGrant,
    parent_relative: Option<&str>,
    directory_cache: &parking_lot::Mutex<HashMap<String, Arc<File>>>,
) -> Result<Vec<Arc<File>>, WorkspaceError> {
    let mut pinned = Vec::new();
    let root = cached_pinned_recovery_directory(directory_cache, "", &grant.root)?;
    if crate::identity_hash_from_information(&crate::handle_information(root.as_ref())?)
        != grant.root_identity_hash
    {
        return Err(WorkspaceError::RootIdentityChanged);
    }
    pinned.push(root);

    if let Some(relative) = parent_relative {
        let mut current_relative = String::new();
        for segment in relative.split('/') {
            if !current_relative.is_empty() {
                current_relative.push('/');
            }
            current_relative.push_str(segment);
            let resolved = resolve_existing(&grant.root, &current_relative)?;
            pinned.push(cached_pinned_recovery_directory(
                directory_cache,
                &current_relative,
                &resolved,
            )?);
        }
    }
    Ok(pinned)
}

#[cfg(windows)]
fn cached_pinned_recovery_directory(
    directory_cache: &parking_lot::Mutex<HashMap<String, Arc<File>>>,
    cache_key: &str,
    path: &Path,
) -> Result<Arc<File>, WorkspaceError> {
    let mut cache = directory_cache.lock();
    if let Some(directory) = cache.get(cache_key) {
        return Ok(Arc::clone(directory));
    }
    let directory = Arc::new(open_pinned_recovery_directory(path)?);
    cache.insert(cache_key.to_owned(), Arc::clone(&directory));
    Ok(directory)
}

#[cfg(windows)]
fn open_pinned_recovery_directory(path: &Path) -> Result<File, WorkspaceError> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
    };

    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err(WorkspaceError::PathBlocked);
    }
    let directory = OpenOptions::new()
        .read(true)
        .write(true)
        // Retagging a directory requires a write-capable handle. Denying both
        // write and delete sharing keeps the pinned namespace immutable until
        // every recovery effect and parent sync has completed.
        .share_mode(FILE_SHARE_READ.0)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0 | FILE_FLAG_OPEN_REPARSE_POINT.0)
        .open(path)?;
    let information = crate::handle_information(&directory)?;
    if information.dwFileAttributes & WINDOWS_FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(WorkspaceError::PathBlocked);
    }
    Ok(directory)
}

#[cfg(windows)]
fn recovery_create_pinned(
    pinned: &PinnedRecoveryPath,
    content: &str,
    after_validation: impl FnOnce(&Path, &Path) -> Result<(), WorkspaceError>,
) -> Result<(), WorkspaceError> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows::Win32::Storage::FileSystem::FILE_SHARE_READ;

    validate_governed_content(content)?;
    if pinned.verified_file.is_some() {
        return Err(WorkspaceError::AlreadyExists);
    }
    let parent = pinned
        .target
        .parent()
        .ok_or(WorkspaceError::InvalidRelativePath)?;
    after_validation(&pinned.target, parent)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .share_mode(FILE_SHARE_READ.0)
        .open(&pinned.target)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                WorkspaceError::AlreadyExists
            } else {
                WorkspaceError::Io(error)
            }
        })?;
    file.write_all(content.as_bytes())?;
    file.sync_all()?;
    drop(file);
    sync_pinned_parent(pinned)
}

#[cfg(windows)]
fn recovery_replace_pinned(
    mut pinned: PinnedRecoveryPath,
    expected_content_hash: &str,
    expected_file_identity_hash: &str,
    content: &str,
    after_validation: impl FnOnce(&Path, &Path) -> Result<(), WorkspaceError>,
) -> Result<(), WorkspaceError> {
    validate_governed_content(content)?;
    if !is_sha256(expected_content_hash) || !is_sha256(expected_file_identity_hash) {
        return Err(WorkspaceError::StalePreimage);
    }
    let mut verified = pinned
        .verified_file
        .take()
        .ok_or(WorkspaceError::StalePreimage)?;
    if verified.content_hash != expected_content_hash
        || verified.file_identity_hash != expected_file_identity_hash
    {
        return Err(WorkspaceError::StalePreimage);
    }
    let parent = pinned
        .target
        .parent()
        .ok_or(WorkspaceError::InvalidRelativePath)?;
    after_validation(&pinned.target, parent)?;
    verified.file.seek(SeekFrom::Start(0))?;
    verified.file.set_len(0)?;
    verified.file.write_all(content.as_bytes())?;
    verified.file.sync_all()?;
    drop(verified.file);
    sync_pinned_parent(&pinned)
}

#[cfg(windows)]
#[allow(
    unsafe_code,
    reason = "Windows exposes identity-preserving delete only through SetFileInformationByHandle"
)]
fn recovery_delete_pinned(
    mut pinned: PinnedRecoveryPath,
    expected_content_hash: &str,
    expected_file_identity_hash: &str,
    after_validation: impl FnOnce(&Path, &Path) -> Result<(), WorkspaceError>,
) -> Result<(), WorkspaceError> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{
        FileDispositionInfo, SetFileInformationByHandle, FILE_DISPOSITION_INFO,
    };

    if !is_sha256(expected_content_hash) || !is_sha256(expected_file_identity_hash) {
        return Err(WorkspaceError::StalePreimage);
    }
    let verified = pinned
        .verified_file
        .take()
        .ok_or(WorkspaceError::StalePreimage)?;
    if verified.content_hash != expected_content_hash
        || verified.file_identity_hash != expected_file_identity_hash
    {
        return Err(WorkspaceError::StalePreimage);
    }
    let parent = pinned
        .target
        .parent()
        .ok_or(WorkspaceError::InvalidRelativePath)?;
    after_validation(&pinned.target, parent)?;
    let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
    // SAFETY: the handle is owned and live, the information pointer and size
    // exactly describe FILE_DISPOSITION_INFO, and no alias can close the file.
    unsafe {
        SetFileInformationByHandle(
            HANDLE(verified.file.as_raw_handle()),
            FileDispositionInfo,
            std::ptr::from_ref(&disposition).cast(),
            u32::try_from(std::mem::size_of::<FILE_DISPOSITION_INFO>())
                .map_err(|_| WorkspaceError::UnsupportedRoot)?,
        )
    }
    .map_err(|error| WorkspaceError::Io(std::io::Error::other(error.to_string())))?;
    drop(verified.file);
    sync_pinned_parent(&pinned)
}

#[cfg(windows)]
fn sync_pinned_parent(pinned: &PinnedRecoveryPath) -> Result<(), WorkspaceError> {
    pinned
        .directories
        .last()
        .ok_or(WorkspaceError::InvalidRelativePath)?
        .sync_all()?;
    Ok(())
}

fn create_utf8_for_grant(
    grant: &WorkspaceGrant,
    relative_path: &str,
    content: &str,
) -> Result<(), WorkspaceError> {
    let normalized = validate_effect_path(relative_path)?;
    validate_governed_content(content)?;
    let parent = resolve_existing_parent(&grant.root, &normalized)?;
    let leaf = leaf_segment(&normalized)?;
    let target = parent.join(leaf);

    let mut file = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(WorkspaceError::AlreadyExists);
        }
        Err(error) => return Err(WorkspaceError::Io(error)),
    };
    file.write_all(content.as_bytes())?;
    file.sync_all()?;
    drop(file);
    flush_directory(&parent)
}

#[cfg(all(test, windows))]
fn recovery_create_utf8_with_seam(
    grant: &WorkspaceGrant,
    relative_path: &str,
    content: &str,
    after_validation: impl FnOnce(&Path, &Path) -> Result<(), WorkspaceError>,
) -> Result<(), WorkspaceError> {
    let directory_cache = parking_lot::Mutex::new(HashMap::new());
    let (_observation, pinned) = pin_recovery_path(grant, relative_path, &directory_cache)?;
    recovery_create_pinned(&pinned, content, after_validation)
}

fn replace_utf8_for_grant(
    grant: &WorkspaceGrant,
    relative_path: &str,
    expected_content_hash: &str,
    expected_file_identity_hash: &str,
    content: &str,
) -> Result<(), WorkspaceError> {
    let normalized = validate_effect_path(relative_path)?;
    validate_governed_content(content)?;
    if !is_sha256(expected_content_hash) || !is_sha256(expected_file_identity_hash) {
        return Err(WorkspaceError::StalePreimage);
    }
    let target = resolve_existing(&grant.root, &normalized)?;
    let parent = target
        .parent()
        .ok_or(WorkspaceError::InvalidRelativePath)?
        .to_path_buf();

    // Keep the identity-verified source handle pinned through replacement.
    let verified = open_verified_file(&target)?;
    if verified.content_hash != expected_content_hash
        || verified.file_identity_hash != expected_file_identity_hash
    {
        return Err(WorkspaceError::StalePreimage);
    }
    let temp = write_temp_sibling(&parent, content)?;
    if let Err(error) = fs::rename(&temp, &target) {
        let _ = fs::remove_file(&temp);
        return Err(WorkspaceError::Io(error));
    }
    drop(verified.file);
    flush_directory(&parent)
}

#[cfg(all(test, windows))]
fn recovery_replace_utf8_with_seam(
    grant: &WorkspaceGrant,
    relative_path: &str,
    expected_content_hash: &str,
    expected_file_identity_hash: &str,
    content: &str,
    after_validation: impl FnOnce(&Path, &Path) -> Result<(), WorkspaceError>,
) -> Result<(), WorkspaceError> {
    let directory_cache = parking_lot::Mutex::new(HashMap::new());
    let (_observation, pinned) = pin_recovery_path(grant, relative_path, &directory_cache)?;
    recovery_replace_pinned(
        pinned,
        expected_content_hash,
        expected_file_identity_hash,
        content,
        after_validation,
    )
}

fn delete_for_grant(
    grant: &WorkspaceGrant,
    relative_path: &str,
    expected_content_hash: &str,
    expected_file_identity_hash: &str,
) -> Result<(), WorkspaceError> {
    let normalized = validate_effect_path(relative_path)?;
    if !is_sha256(expected_content_hash) || !is_sha256(expected_file_identity_hash) {
        return Err(WorkspaceError::StalePreimage);
    }
    let target = resolve_existing(&grant.root, &normalized)?;
    let parent = target
        .parent()
        .ok_or(WorkspaceError::InvalidRelativePath)?
        .to_path_buf();
    let verified = open_verified_file(&target)?;
    if verified.content_hash != expected_content_hash
        || verified.file_identity_hash != expected_file_identity_hash
    {
        return Err(WorkspaceError::StalePreimage);
    }
    fs::remove_file(&target)?;
    drop(verified.file);
    flush_directory(&parent)
}

#[cfg(all(test, windows))]
fn recovery_delete_with_seam(
    grant: &WorkspaceGrant,
    relative_path: &str,
    expected_content_hash: &str,
    expected_file_identity_hash: &str,
    after_validation: impl FnOnce(&Path, &Path) -> Result<(), WorkspaceError>,
) -> Result<(), WorkspaceError> {
    let directory_cache = parking_lot::Mutex::new(HashMap::new());
    let (_observation, pinned) = pin_recovery_path(grant, relative_path, &directory_cache)?;
    recovery_delete_pinned(
        pinned,
        expected_content_hash,
        expected_file_identity_hash,
        after_validation,
    )
}

fn validate_effect_path(relative_path: &str) -> Result<String, WorkspaceError> {
    let normalized = normalize_relative_path(relative_path)?;
    if normalized == "." || is_sensitive_relative_path(&normalized) {
        return Err(WorkspaceError::PathBlocked);
    }
    Ok(normalized)
}

fn leaf_segment(normalized: &str) -> Result<&str, WorkspaceError> {
    normalized
        .rsplit('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .ok_or(WorkspaceError::InvalidRelativePath)
}

fn resolve_existing_parent(root: &Path, normalized: &str) -> Result<PathBuf, WorkspaceError> {
    match normalized.rsplit_once('/') {
        Some((parent, _leaf)) => {
            let resolved = resolve_existing(root, parent)?;
            if !resolved.is_dir() {
                return Err(WorkspaceError::InvalidRelativePath);
            }
            Ok(resolved)
        }
        None => Ok(root.to_path_buf()),
    }
}

fn validate_governed_content(content: &str) -> Result<(), WorkspaceError> {
    if content.contains('\0') {
        return Err(WorkspaceError::UnsupportedText);
    }
    if content.len() as u64 > MAX_GOVERNED_FILE_BYTES {
        return Err(WorkspaceError::LimitExceeded);
    }
    Ok(())
}

fn write_temp_sibling(parent: &Path, content: &str) -> Result<PathBuf, WorkspaceError> {
    let temp = parent.join(format!(".sapphirus.tmp.{}", Ulid::new()));
    let result = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .and_then(|mut file| {
            file.write_all(content.as_bytes())?;
            file.sync_all()
        });
    match result {
        Ok(()) => Ok(temp),
        Err(error) => {
            let _ = fs::remove_file(&temp);
            Err(WorkspaceError::Io(error))
        }
    }
}

#[cfg(windows)]
fn open_verified_file(path: &Path) -> Result<VerifiedFile, WorkspaceError> {
    use std::fs::OpenOptions;
    use std::os::windows::fs::OpenOptionsExt;
    use windows::Win32::Storage::FileSystem::{FILE_SHARE_DELETE, FILE_SHARE_READ};

    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || is_reparse_point(&metadata)
        || is_cloud_placeholder(&metadata)
    {
        return Err(WorkspaceError::PathBlocked);
    }
    let mut file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ.0 | FILE_SHARE_DELETE.0)
        .open(path)?;
    let information = crate::handle_information(&file)?;
    if information.dwFileAttributes & WINDOWS_FILE_ATTRIBUTE_REPARSE_POINT != 0
        || information.nNumberOfLinks > 1
    {
        return Err(WorkspaceError::PathBlocked);
    }
    let size = (u64::from(information.nFileSizeHigh) << 32) | u64::from(information.nFileSizeLow);
    if size > MAX_GOVERNED_FILE_BYTES {
        return Err(WorkspaceError::LimitExceeded);
    }
    let mut content = Vec::with_capacity(usize::try_from(size).unwrap_or(0).saturating_add(1));
    file.read_to_end(&mut content)?;
    if content.len() as u64 > MAX_GOVERNED_FILE_BYTES || content.contains(&0) {
        return Err(WorkspaceError::UnsupportedText);
    }
    if std::str::from_utf8(&content).is_err() {
        return Err(WorkspaceError::UnsupportedText);
    }
    let content_hash = hash_bytes(&content);
    let file_identity_hash = crate::identity_hash_from_information(&information);
    let metadata_hash = hash_bytes(
        format!(
            "{}:{}:{}",
            size,
            information.ftLastWriteTime.dwHighDateTime,
            information.ftLastWriteTime.dwLowDateTime
        )
        .as_bytes(),
    );
    Ok(VerifiedFile {
        file,
        content,
        content_hash,
        file_identity_hash,
        metadata_hash,
    })
}

#[cfg(windows)]
fn open_recovery_verified_file(path: &Path) -> Result<VerifiedFile, WorkspaceError> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE};
    use windows::Win32::Storage::FileSystem::{
        DELETE, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
    };

    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || is_reparse_point(&metadata)
        || is_cloud_placeholder(&metadata)
    {
        return Err(WorkspaceError::PathBlocked);
    }
    let mut file = OpenOptions::new()
        .access_mode(GENERIC_READ.0 | GENERIC_WRITE.0 | DELETE.0)
        .share_mode(FILE_SHARE_READ.0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0)
        .open(path)?;
    let information = crate::handle_information(&file)?;
    if information.dwFileAttributes & WINDOWS_FILE_ATTRIBUTE_REPARSE_POINT != 0
        || information.nNumberOfLinks > 1
    {
        return Err(WorkspaceError::PathBlocked);
    }
    let size = (u64::from(information.nFileSizeHigh) << 32) | u64::from(information.nFileSizeLow);
    if size > MAX_GOVERNED_FILE_BYTES {
        return Err(WorkspaceError::LimitExceeded);
    }
    let mut content = Vec::with_capacity(usize::try_from(size).unwrap_or(0).saturating_add(1));
    file.read_to_end(&mut content)?;
    if content.len() as u64 > MAX_GOVERNED_FILE_BYTES || content.contains(&0) {
        return Err(WorkspaceError::UnsupportedText);
    }
    if std::str::from_utf8(&content).is_err() {
        return Err(WorkspaceError::UnsupportedText);
    }
    let content_hash = hash_bytes(&content);
    let file_identity_hash = crate::identity_hash_from_information(&information);
    let metadata_hash = hash_bytes(
        format!(
            "{}:{}:{}",
            size,
            information.ftLastWriteTime.dwHighDateTime,
            information.ftLastWriteTime.dwLowDateTime
        )
        .as_bytes(),
    );
    Ok(VerifiedFile {
        file,
        content,
        content_hash,
        file_identity_hash,
        metadata_hash,
    })
}

#[cfg(not(windows))]
fn open_verified_file(path: &Path) -> Result<VerifiedFile, WorkspaceError> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(WorkspaceError::PathBlocked);
    }
    if metadata.len() > MAX_GOVERNED_FILE_BYTES {
        return Err(WorkspaceError::LimitExceeded);
    }
    let mut file = File::open(path)?;
    let mut content = Vec::new();
    file.read_to_end(&mut content)?;
    if content.len() as u64 > MAX_GOVERNED_FILE_BYTES || content.contains(&0) {
        return Err(WorkspaceError::UnsupportedText);
    }
    if std::str::from_utf8(&content).is_err() {
        return Err(WorkspaceError::UnsupportedText);
    }
    let content_hash = hash_bytes(&content);
    let file_identity_hash = crate::root_identity(path, &metadata)?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_nanos());
    let metadata_hash = hash_bytes(format!("{}:{modified}", metadata.len()).as_bytes());
    Ok(VerifiedFile {
        file,
        content,
        content_hash,
        file_identity_hash,
        metadata_hash,
    })
}

#[cfg(windows)]
fn flush_directory(path: &Path) -> Result<(), WorkspaceError> {
    use std::fs::OpenOptions;
    use std::os::windows::fs::OpenOptionsExt;
    use windows::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS;

    let directory = OpenOptions::new()
        .write(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0)
        .open(path)?;
    directory.sync_all()?;
    Ok(())
}

#[cfg(not(windows))]
fn flush_directory(path: &Path) -> Result<(), WorkspaceError> {
    File::open(path)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::MAX_GOVERNED_FILE_BYTES;
    use crate::{WorkspaceBroker, WorkspaceError, WorkspacePermissions};

    type GovernedFixture = (tempfile::TempDir, WorkspaceBroker, String, u64, u64);

    fn governed_fixture() -> Result<GovernedFixture, Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        fs::write(dir.path().join("main.rs"), "fn main() {}\n")?;
        fs::create_dir(dir.path().join("src"))?;
        fs::write(dir.path().join("src").join("lib.rs"), "pub fn lib() {}\n")?;
        let broker = WorkspaceBroker::new();
        let projection = broker.grant("project_1", dir.path())?;
        let enabled = broker.enable_governed_edits(&projection.workspace_id)?;
        assert_eq!(enabled.permissions, WorkspacePermissions::GovernedEdits);
        // ADR-0002: enabling edits advances only the governed-edit epoch.
        assert_eq!(enabled.grant_epoch, projection.grant_epoch);
        assert_eq!(enabled.context_read_epoch, projection.context_read_epoch);
        assert_eq!(
            enabled.governed_edit_epoch,
            projection.governed_edit_epoch + 1
        );
        Ok((
            dir,
            broker,
            enabled.workspace_id,
            enabled.grant_epoch,
            enabled.governed_edit_epoch,
        ))
    }

    #[cfg(windows)]
    #[allow(
        unsafe_code,
        reason = "the regression must attempt the real Windows reparse control operation"
    )]
    fn attempt_mount_point_retag(
        directory: &std::path::Path,
        outside: &std::path::Path,
    ) -> Result<bool, WorkspaceError> {
        use std::os::windows::ffi::OsStrExt;
        use std::os::windows::fs::OpenOptionsExt;
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::Storage::FileSystem::{
            FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
            FILE_SHARE_WRITE,
        };
        use windows::Win32::System::IO::DeviceIoControl;

        const FSCTL_SET_REPARSE_POINT: u32 = 589_988;
        const IO_REPARSE_TAG_MOUNT_POINT: u32 = 0xA000_0003;

        let handle = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0 | FILE_FLAG_OPEN_REPARSE_POINT.0)
            .open(directory)?;

        let substitute = format!(r"\??\{}", outside.display())
            .encode_utf16()
            .collect::<Vec<_>>();
        let print = outside.as_os_str().encode_wide().collect::<Vec<_>>();
        let substitute_bytes =
            u16::try_from(substitute.len() * 2).map_err(|_| WorkspaceError::LimitExceeded)?;
        let print_bytes =
            u16::try_from(print.len() * 2).map_err(|_| WorkspaceError::LimitExceeded)?;
        let path_bytes = usize::from(substitute_bytes) + 2 + usize::from(print_bytes) + 2;
        let reparse_data_length =
            u16::try_from(8 + path_bytes).map_err(|_| WorkspaceError::LimitExceeded)?;
        let mut buffer = Vec::with_capacity(8 + usize::from(reparse_data_length));
        buffer.extend_from_slice(&IO_REPARSE_TAG_MOUNT_POINT.to_le_bytes());
        buffer.extend_from_slice(&reparse_data_length.to_le_bytes());
        buffer.extend_from_slice(&0_u16.to_le_bytes());
        buffer.extend_from_slice(&0_u16.to_le_bytes());
        buffer.extend_from_slice(&substitute_bytes.to_le_bytes());
        buffer.extend_from_slice(&(substitute_bytes + 2).to_le_bytes());
        buffer.extend_from_slice(&print_bytes.to_le_bytes());
        for unit in substitute {
            buffer.extend_from_slice(&unit.to_le_bytes());
        }
        buffer.extend_from_slice(&0_u16.to_le_bytes());
        for unit in print {
            buffer.extend_from_slice(&unit.to_le_bytes());
        }
        buffer.extend_from_slice(&0_u16.to_le_bytes());

        let mut bytes_returned = 0;
        // SAFETY: the handle is live and owned; the immutable input buffer and
        // byte count describe the complete mount-point reparse payload.
        let result = unsafe {
            DeviceIoControl(
                HANDLE(handle.as_raw_handle()),
                FSCTL_SET_REPARSE_POINT,
                Some(buffer.as_ptr().cast()),
                u32::try_from(buffer.len()).map_err(|_| WorkspaceError::LimitExceeded)?,
                None,
                0,
                Some(&raw mut bytes_returned),
                None,
            )
        };
        Ok(result.is_ok())
    }

    #[cfg(windows)]
    fn remove_test_mount_point(path: &std::path::Path) -> std::io::Result<()> {
        fs::remove_dir(path)?;
        fs::create_dir(path)
    }

    #[test]
    fn mutation_requires_explicit_edits_enablement() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let broker = WorkspaceBroker::new();
        let projection = broker.grant("project_1", dir.path())?;
        let result = broker.create_utf8_durable(
            &projection.workspace_id,
            projection.grant_epoch,
            projection.governed_edit_epoch,
            "new.txt",
            "content",
        );
        assert!(matches!(result, Err(WorkspaceError::EditsNotEnabled)));
        Ok(())
    }

    #[test]
    fn mutation_requires_the_exact_grant_epoch() -> Result<(), Box<dyn std::error::Error>> {
        let (_dir, broker, workspace_id, epoch, edit_epoch) = governed_fixture()?;
        let result =
            broker.create_utf8_durable(&workspace_id, epoch + 1, edit_epoch, "new.txt", "content");
        assert!(matches!(result, Err(WorkspaceError::GrantUnavailable)));
        let result = broker.create_utf8_durable(&workspace_id, 0, edit_epoch, "new.txt", "content");
        assert!(matches!(result, Err(WorkspaceError::GrantUnavailable)));
        Ok(())
    }

    #[test]
    fn mutation_requires_the_exact_governed_edit_epoch() -> Result<(), Box<dyn std::error::Error>> {
        let (_dir, broker, workspace_id, epoch, edit_epoch) = governed_fixture()?;
        let stale =
            broker.create_utf8_durable(&workspace_id, epoch, edit_epoch + 1, "new.txt", "content");
        assert!(matches!(stale, Err(WorkspaceError::GrantUnavailable)));
        let zero = broker.create_utf8_durable(&workspace_id, epoch, 0, "new.txt", "content");
        assert!(matches!(zero, Err(WorkspaceError::GrantUnavailable)));
        Ok(())
    }

    #[test]
    fn context_read_withdrawal_leaves_governed_mutations_valid(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (dir, broker, workspace_id, epoch, edit_epoch) = governed_fixture()?;
        let before = broker.advance_context_read_epoch(&workspace_id)?;
        assert_eq!(before.grant_epoch, epoch);
        assert_eq!(before.governed_edit_epoch, edit_epoch);
        broker.create_utf8_durable(
            &workspace_id,
            epoch,
            edit_epoch,
            "src/after_signout.rs",
            "pub fn kept() {}\n",
        )?;
        assert!(dir.path().join("src").join("after_signout.rs").exists());
        Ok(())
    }

    #[test]
    fn observes_existing_and_absent_preimages() -> Result<(), Box<dyn std::error::Error>> {
        let (_dir, broker, workspace_id, epoch, edit_epoch) = governed_fixture()?;
        let existing = broker.observe_preimage(&workspace_id, epoch, edit_epoch, "main.rs")?;
        assert!(existing.exists);
        assert_eq!(existing.content.as_deref(), Some("fn main() {}\n"));
        assert!(existing
            .content_hash
            .is_some_and(|hash| hash.starts_with("sha256:")));
        assert!(existing.file_identity_hash.is_some());
        assert!(existing.metadata_hash.is_some());

        let absent =
            broker.observe_preimage(&workspace_id, epoch, edit_epoch, "src/new_module.rs")?;
        assert!(!absent.exists);
        assert!(absent.content_hash.is_none());
        Ok(())
    }

    #[test]
    fn observation_rejects_escape_and_sensitive_paths() -> Result<(), Box<dyn std::error::Error>> {
        let (_dir, broker, workspace_id, epoch, edit_epoch) = governed_fixture()?;
        assert!(matches!(
            broker.observe_preimage(&workspace_id, epoch, edit_epoch, "../outside.txt"),
            Err(WorkspaceError::InvalidRelativePath)
        ));
        assert!(matches!(
            broker.observe_preimage(&workspace_id, epoch, edit_epoch, ".env"),
            Err(WorkspaceError::PathBlocked)
        ));
        assert!(matches!(
            broker.observe_preimage(&workspace_id, epoch, edit_epoch, "missing_dir/new.txt"),
            Err(WorkspaceError::Io(_))
        ));
        Ok(())
    }

    #[test]
    fn creates_a_new_file_durably() -> Result<(), Box<dyn std::error::Error>> {
        let (dir, broker, workspace_id, epoch, edit_epoch) = governed_fixture()?;
        broker.create_utf8_durable(
            &workspace_id,
            epoch,
            edit_epoch,
            "src/created.rs",
            "pub fn f() {}\n",
        )?;
        let written = fs::read_to_string(dir.path().join("src").join("created.rs"))?;
        assert_eq!(written, "pub fn f() {}\n");

        let duplicate =
            broker.create_utf8_durable(&workspace_id, epoch, edit_epoch, "src/created.rs", "other");
        assert!(matches!(duplicate, Err(WorkspaceError::AlreadyExists)));
        Ok(())
    }

    #[test]
    fn create_rejects_unsupported_content() -> Result<(), Box<dyn std::error::Error>> {
        let (_dir, broker, workspace_id, epoch, edit_epoch) = governed_fixture()?;
        assert!(matches!(
            broker.create_utf8_durable(&workspace_id, epoch, edit_epoch, "bad.txt", "nul\0byte"),
            Err(WorkspaceError::UnsupportedText)
        ));
        let oversized = "a".repeat(usize::try_from(MAX_GOVERNED_FILE_BYTES)? + 1);
        assert!(matches!(
            broker.create_utf8_durable(&workspace_id, epoch, edit_epoch, "big.txt", &oversized),
            Err(WorkspaceError::LimitExceeded)
        ));
        assert!(matches!(
            broker.create_utf8_durable(
                &workspace_id,
                epoch,
                edit_epoch,
                "secrets/token.json",
                "{}"
            ),
            Err(WorkspaceError::PathBlocked)
        ));
        Ok(())
    }

    #[test]
    fn replaces_only_the_exact_verified_preimage() -> Result<(), Box<dyn std::error::Error>> {
        let (dir, broker, workspace_id, epoch, edit_epoch) = governed_fixture()?;
        let before = broker.observe_preimage(&workspace_id, epoch, edit_epoch, "main.rs")?;
        let content_hash = before.content_hash.ok_or("missing content hash")?;
        let identity_hash = before.file_identity_hash.ok_or("missing identity hash")?;

        broker.replace_utf8_durable(
            &workspace_id,
            epoch,
            edit_epoch,
            "main.rs",
            &content_hash,
            &identity_hash,
            "fn main() { updated(); }\n",
        )?;
        assert_eq!(
            fs::read_to_string(dir.path().join("main.rs"))?,
            "fn main() { updated(); }\n"
        );

        // The preimage hash is now stale; a second replace must fail closed.
        let stale = broker.replace_utf8_durable(
            &workspace_id,
            epoch,
            edit_epoch,
            "main.rs",
            &content_hash,
            &identity_hash,
            "fn main() { twice(); }\n",
        );
        assert!(matches!(stale, Err(WorkspaceError::StalePreimage)));
        Ok(())
    }

    #[test]
    fn replace_detects_external_modification() -> Result<(), Box<dyn std::error::Error>> {
        let (dir, broker, workspace_id, epoch, edit_epoch) = governed_fixture()?;
        let before = broker.observe_preimage(&workspace_id, epoch, edit_epoch, "main.rs")?;
        let content_hash = before.content_hash.ok_or("missing content hash")?;
        let identity_hash = before.file_identity_hash.ok_or("missing identity hash")?;

        fs::write(dir.path().join("main.rs"), "// external edit\n")?;
        let result = broker.replace_utf8_durable(
            &workspace_id,
            epoch,
            edit_epoch,
            "main.rs",
            &content_hash,
            &identity_hash,
            "fn main() { updated(); }\n",
        );
        assert!(matches!(result, Err(WorkspaceError::StalePreimage)));
        assert_eq!(
            fs::read_to_string(dir.path().join("main.rs"))?,
            "// external edit\n"
        );
        Ok(())
    }

    #[test]
    fn deletes_only_the_exact_verified_preimage() -> Result<(), Box<dyn std::error::Error>> {
        let (dir, broker, workspace_id, epoch, edit_epoch) = governed_fixture()?;
        let before = broker.observe_preimage(&workspace_id, epoch, edit_epoch, "src/lib.rs")?;
        let content_hash = before.content_hash.ok_or("missing content hash")?;
        let identity_hash = before.file_identity_hash.ok_or("missing identity hash")?;

        let wrong_hash = crate::hash_bytes(b"different");
        let stale = broker.delete_durable(
            &workspace_id,
            epoch,
            edit_epoch,
            "src/lib.rs",
            &wrong_hash,
            &identity_hash,
        );
        assert!(matches!(stale, Err(WorkspaceError::StalePreimage)));
        assert!(dir.path().join("src").join("lib.rs").exists());

        broker.delete_durable(
            &workspace_id,
            epoch,
            edit_epoch,
            "src/lib.rs",
            &content_hash,
            &identity_hash,
        )?;
        assert!(!dir.path().join("src").join("lib.rs").exists());
        Ok(())
    }

    #[test]
    fn read_effect_file_verifies_identity() -> Result<(), Box<dyn std::error::Error>> {
        let (_dir, broker, workspace_id, epoch, edit_epoch) = governed_fixture()?;
        let before = broker.observe_preimage(&workspace_id, epoch, edit_epoch, "main.rs")?;
        let identity_hash = before.file_identity_hash.ok_or("missing identity hash")?;

        let bytes = broker.read_effect_file(
            &workspace_id,
            epoch,
            edit_epoch,
            "main.rs",
            Some(&identity_hash),
        )?;
        assert_eq!(bytes.as_deref(), Some(b"fn main() {}\n".as_slice()));

        let wrong = crate::hash_bytes(b"other-identity");
        assert!(matches!(
            broker.read_effect_file(&workspace_id, epoch, edit_epoch, "main.rs", Some(&wrong)),
            Err(WorkspaceError::StalePreimage)
        ));

        let absent =
            broker.read_effect_file(&workspace_id, epoch, edit_epoch, "src/none.rs", None)?;
        assert!(absent.is_none());
        Ok(())
    }

    #[test]
    fn revocation_fails_governed_operations_closed() -> Result<(), Box<dyn std::error::Error>> {
        let (_dir, broker, workspace_id, epoch, edit_epoch) = governed_fixture()?;
        broker.revoke(&workspace_id)?;
        let result =
            broker.create_utf8_durable(&workspace_id, epoch, edit_epoch, "late.txt", "content");
        assert!(matches!(result, Err(WorkspaceError::GrantUnavailable)));
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn recovery_replace_rejects_target_substitution_after_validation(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (dir, broker, workspace_id, epoch, edit_epoch) = governed_fixture()?;
        let before = broker.observe_preimage(&workspace_id, epoch, edit_epoch, "main.rs")?;
        let content_hash = before.content_hash.ok_or("missing content hash")?;
        let identity_hash = before.file_identity_hash.ok_or("missing identity hash")?;
        let displaced = dir.path().join("main.displaced.rs");
        let result =
            broker.with_governed_recovery(&workspace_id, epoch, edit_epoch, |transaction| {
                super::recovery_replace_utf8_with_seam(
                    &transaction.grant,
                    "main.rs",
                    &content_hash,
                    &identity_hash,
                    "restored\n",
                    |target, _parent| {
                        fs::rename(target, &displaced)?;
                        fs::write(target, "attacker\n")?;
                        Ok(())
                    },
                )
            })?;

        assert!(result.is_err());
        assert_eq!(fs::read(dir.path().join("main.rs"))?, b"fn main() {}\n");
        assert!(!displaced.exists());
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn recovery_delete_rejects_target_substitution_after_validation(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (dir, broker, workspace_id, epoch, edit_epoch) = governed_fixture()?;
        let before = broker.observe_preimage(&workspace_id, epoch, edit_epoch, "main.rs")?;
        let content_hash = before.content_hash.ok_or("missing content hash")?;
        let identity_hash = before.file_identity_hash.ok_or("missing identity hash")?;
        let displaced = dir.path().join("main.displaced.rs");
        let result =
            broker.with_governed_recovery(&workspace_id, epoch, edit_epoch, |transaction| {
                super::recovery_delete_with_seam(
                    &transaction.grant,
                    "main.rs",
                    &content_hash,
                    &identity_hash,
                    |target, _parent| {
                        fs::rename(target, &displaced)?;
                        fs::write(target, "attacker\n")?;
                        Ok(())
                    },
                )
            })?;

        assert!(result.is_err());
        assert_eq!(fs::read(dir.path().join("main.rs"))?, b"fn main() {}\n");
        assert!(!displaced.exists());
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn recovery_create_rejects_parent_substitution_after_validation(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (dir, broker, workspace_id, epoch, edit_epoch) = governed_fixture()?;
        let displaced = dir.path().join("src.displaced");
        let result =
            broker.with_governed_recovery(&workspace_id, epoch, edit_epoch, |transaction| {
                super::recovery_create_utf8_with_seam(
                    &transaction.grant,
                    "src/new.rs",
                    "restored\n",
                    |_target, parent| {
                        fs::rename(parent, &displaced)?;
                        fs::create_dir(parent)?;
                        Ok(())
                    },
                )
            })?;

        assert!(result.is_err());
        assert!(dir.path().join("src").join("lib.rs").exists());
        assert!(!dir.path().join("src").join("new.rs").exists());
        assert!(!displaced.exists());
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn recovery_create_blocks_empty_parent_reparse_retag_after_pin(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        fs::create_dir(root.path().join("empty"))?;
        let broker = WorkspaceBroker::new();
        let projection = broker.grant("project_1", root.path())?;
        let enabled = broker.enable_governed_edits(&projection.workspace_id)?;
        let parent = root.path().join("empty");
        let outside_file = outside.path().join("escaped.txt");
        let retagged = std::cell::Cell::new(false);

        let result = broker.with_governed_recovery(
            &enabled.workspace_id,
            enabled.grant_epoch,
            enabled.governed_edit_epoch,
            |transaction| {
                super::recovery_create_utf8_with_seam(
                    &transaction.grant,
                    "empty/escaped.txt",
                    "blocked\n",
                    |_target, pinned_parent| {
                        retagged.set(attempt_mount_point_retag(pinned_parent, outside.path())?);
                        Ok(())
                    },
                )
            },
        );

        let governed_file = parent.join("escaped.txt");
        let governed_changed = governed_file.exists();
        let escaped = outside_file.exists();
        if retagged.get() {
            remove_test_mount_point(&parent)?;
        }
        assert!(matches!(result, Ok(Err(_))));
        assert!(!governed_changed);
        assert!(!escaped);
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn recovery_create_blocks_empty_root_reparse_retag_after_pin(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        let broker = WorkspaceBroker::new();
        let projection = broker.grant("project_1", root.path())?;
        let enabled = broker.enable_governed_edits(&projection.workspace_id)?;
        let outside_file = outside.path().join("escaped.txt");
        let retagged = std::cell::Cell::new(false);

        let result = broker.with_governed_recovery(
            &enabled.workspace_id,
            enabled.grant_epoch,
            enabled.governed_edit_epoch,
            |transaction| {
                super::recovery_create_utf8_with_seam(
                    &transaction.grant,
                    "escaped.txt",
                    "blocked\n",
                    |_target, pinned_root| {
                        retagged.set(attempt_mount_point_retag(pinned_root, outside.path())?);
                        Ok(())
                    },
                )
            },
        )?;

        let governed_file = root.path().join("escaped.txt");
        let governed_changed = governed_file.exists();
        let escaped = outside_file.exists();
        if retagged.get() {
            remove_test_mount_point(root.path())?;
        }
        assert!(result.is_err());
        assert!(!governed_changed);
        assert!(!escaped);
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn recovery_pin_rejects_preexisting_write_capable_directory_handle(
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::os::windows::fs::OpenOptionsExt;
        use windows::Win32::Storage::FileSystem::{
            FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_READ, FILE_SHARE_WRITE,
        };

        let root = tempfile::tempdir()?;
        fs::create_dir(root.path().join("empty"))?;
        let broker = WorkspaceBroker::new();
        let projection = broker.grant("project_1", root.path())?;
        let enabled = broker.enable_governed_edits(&projection.workspace_id)?;
        let _write_capable = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0)
            .open(root.path().join("empty"))?;

        let result = broker.with_governed_recovery(
            &enabled.workspace_id,
            enabled.grant_epoch,
            enabled.governed_edit_epoch,
            |transaction| {
                super::recovery_create_utf8_with_seam(
                    &transaction.grant,
                    "empty/new.txt",
                    "blocked\n",
                    |_target, _parent| Ok(()),
                )
            },
        )?;
        assert!(result.is_err());
        assert!(!root.path().join("empty/new.txt").exists());
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn governed_recovery_transaction_uses_pinned_handles_for_all_effect_kinds(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (dir, broker, workspace_id, epoch, edit_epoch) = governed_fixture()?;
        broker.with_governed_recovery(&workspace_id, epoch, edit_epoch, |transaction| {
            let replace = transaction.observe_preimage("main.rs")?;
            let delete = transaction.observe_preimage("src/lib.rs")?;
            assert!(!transaction.observe_preimage("src/new.rs")?.exists);

            transaction.replace_utf8_durable(
                "main.rs",
                replace
                    .content_hash
                    .as_deref()
                    .ok_or(WorkspaceError::StalePreimage)?,
                replace
                    .file_identity_hash
                    .as_deref()
                    .ok_or(WorkspaceError::StalePreimage)?,
                "restored\n",
            )?;

            transaction.delete_durable(
                "src/lib.rs",
                delete
                    .content_hash
                    .as_deref()
                    .ok_or(WorkspaceError::StalePreimage)?,
                delete
                    .file_identity_hash
                    .as_deref()
                    .ok_or(WorkspaceError::StalePreimage)?,
            )?;
            transaction.create_utf8_durable("src/new.rs", "created\n")
        })??;

        assert_eq!(fs::read(dir.path().join("main.rs"))?, b"restored\n");
        assert!(!dir.path().join("src").join("lib.rs").exists());
        assert_eq!(
            fs::read(dir.path().join("src").join("new.rs"))?,
            b"created\n"
        );
        Ok(())
    }
}
