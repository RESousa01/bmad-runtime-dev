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

impl WorkspaceBroker {
    /// Enables governed edits for an active grant and advances its epoch.
    ///
    /// The epoch bump invalidates every proposal, spec, and authority binding
    /// issued against the previous read-only epoch.
    ///
    /// # Errors
    ///
    /// Returns an error when the grant is absent or revoked, the root identity
    /// changed, or the grant epoch cannot advance without wrapping.
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
            .grant_epoch
            .checked_add(1)
            .ok_or(WorkspaceError::GrantUnavailable)?;
        grant.projection.grant_epoch = next_epoch;
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
        relative_path: &str,
    ) -> Result<PreimageObservation, WorkspaceError> {
        let authority = self.revocation_barrier.read();
        let grant = self.governed_grant(&authority, workspace_id, expected_grant_epoch)?;
        let normalized = validate_effect_path(relative_path)?;
        let observation = match resolve_existing(&grant.root, &normalized) {
            Ok(path) => {
                let verified = open_verified_file(&path)?;
                let content = String::from_utf8(verified.content)
                    .map_err(|_| WorkspaceError::UnsupportedText)?;
                PreimageObservation {
                    relative_path: normalized,
                    exists: true,
                    content: Some(content),
                    content_hash: Some(verified.content_hash),
                    file_identity_hash: Some(verified.file_identity_hash),
                    metadata_hash: Some(verified.metadata_hash),
                }
            }
            Err(WorkspaceError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                resolve_existing_parent(&grant.root, &normalized)?;
                PreimageObservation {
                    relative_path: normalized,
                    exists: false,
                    content: None,
                    content_hash: None,
                    file_identity_hash: None,
                    metadata_hash: None,
                }
            }
            Err(error) => return Err(error),
        };
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
        relative_path: &str,
        expected_file_identity_hash: Option<&str>,
    ) -> Result<Option<Vec<u8>>, WorkspaceError> {
        let authority = self.revocation_barrier.read();
        let grant = self.governed_grant(&authority, workspace_id, expected_grant_epoch)?;
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
        relative_path: &str,
        content: &str,
    ) -> Result<(), WorkspaceError> {
        let authority = self.revocation_barrier.read();
        let grant = self.governed_grant(&authority, workspace_id, expected_grant_epoch)?;
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
        flush_directory(&parent)?;
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
    pub fn replace_utf8_durable(
        &self,
        workspace_id: &str,
        expected_grant_epoch: u64,
        relative_path: &str,
        expected_content_hash: &str,
        expected_file_identity_hash: &str,
        content: &str,
    ) -> Result<(), WorkspaceError> {
        let authority = self.revocation_barrier.read();
        let grant = self.governed_grant(&authority, workspace_id, expected_grant_epoch)?;
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

        // The verification handle denies concurrent writers but shares delete
        // access, so the atomic same-volume rename below can supersede the file
        // while the verified preimage is still pinned.
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
        flush_directory(&parent)?;
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
        relative_path: &str,
        expected_content_hash: &str,
        expected_file_identity_hash: &str,
    ) -> Result<(), WorkspaceError> {
        let authority = self.revocation_barrier.read();
        let grant = self.governed_grant(&authority, workspace_id, expected_grant_epoch)?;
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
        flush_directory(&parent)?;
        self.ensure_grant_current(&authority, &grant)?;
        Ok(())
    }

    fn governed_grant(
        &self,
        authority: &parking_lot::RwLockReadGuard<'_, ()>,
        workspace_id: &str,
        expected_grant_epoch: u64,
    ) -> Result<WorkspaceGrant, WorkspaceError> {
        let grant = self.active_grant(authority, workspace_id)?;
        if expected_grant_epoch == 0 || grant.projection.grant_epoch != expected_grant_epoch {
            return Err(WorkspaceError::GrantUnavailable);
        }
        if grant.projection.permissions != WorkspacePermissions::GovernedEdits {
            return Err(WorkspaceError::EditsNotEnabled);
        }
        revalidate_root(&grant)?;
        Ok(grant)
    }
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

    fn governed_fixture(
    ) -> Result<(tempfile::TempDir, WorkspaceBroker, String, u64), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        fs::write(dir.path().join("main.rs"), "fn main() {}\n")?;
        fs::create_dir(dir.path().join("src"))?;
        fs::write(dir.path().join("src").join("lib.rs"), "pub fn lib() {}\n")?;
        let broker = WorkspaceBroker::new();
        let projection = broker.grant("project_1", dir.path())?;
        let enabled = broker.enable_governed_edits(&projection.workspace_id)?;
        assert_eq!(enabled.permissions, WorkspacePermissions::GovernedEdits);
        assert_eq!(enabled.grant_epoch, projection.grant_epoch + 1);
        let epoch = enabled.grant_epoch;
        Ok((dir, broker, enabled.workspace_id, epoch))
    }

    #[test]
    fn mutation_requires_explicit_edits_enablement() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let broker = WorkspaceBroker::new();
        let projection = broker.grant("project_1", dir.path())?;
        let result = broker.create_utf8_durable(
            &projection.workspace_id,
            projection.grant_epoch,
            "new.txt",
            "content",
        );
        assert!(matches!(result, Err(WorkspaceError::EditsNotEnabled)));
        Ok(())
    }

    #[test]
    fn mutation_requires_the_exact_grant_epoch() -> Result<(), Box<dyn std::error::Error>> {
        let (_dir, broker, workspace_id, epoch) = governed_fixture()?;
        let result = broker.create_utf8_durable(&workspace_id, epoch + 1, "new.txt", "content");
        assert!(matches!(result, Err(WorkspaceError::GrantUnavailable)));
        let result = broker.create_utf8_durable(&workspace_id, 0, "new.txt", "content");
        assert!(matches!(result, Err(WorkspaceError::GrantUnavailable)));
        Ok(())
    }

    #[test]
    fn observes_existing_and_absent_preimages() -> Result<(), Box<dyn std::error::Error>> {
        let (_dir, broker, workspace_id, epoch) = governed_fixture()?;
        let existing = broker.observe_preimage(&workspace_id, epoch, "main.rs")?;
        assert!(existing.exists);
        assert_eq!(existing.content.as_deref(), Some("fn main() {}\n"));
        assert!(existing
            .content_hash
            .is_some_and(|hash| hash.starts_with("sha256:")));
        assert!(existing.file_identity_hash.is_some());
        assert!(existing.metadata_hash.is_some());

        let absent = broker.observe_preimage(&workspace_id, epoch, "src/new_module.rs")?;
        assert!(!absent.exists);
        assert!(absent.content_hash.is_none());
        Ok(())
    }

    #[test]
    fn observation_rejects_escape_and_sensitive_paths() -> Result<(), Box<dyn std::error::Error>> {
        let (_dir, broker, workspace_id, epoch) = governed_fixture()?;
        assert!(matches!(
            broker.observe_preimage(&workspace_id, epoch, "../outside.txt"),
            Err(WorkspaceError::InvalidRelativePath)
        ));
        assert!(matches!(
            broker.observe_preimage(&workspace_id, epoch, ".env"),
            Err(WorkspaceError::PathBlocked)
        ));
        assert!(matches!(
            broker.observe_preimage(&workspace_id, epoch, "missing_dir/new.txt"),
            Err(WorkspaceError::Io(_))
        ));
        Ok(())
    }

    #[test]
    fn creates_a_new_file_durably() -> Result<(), Box<dyn std::error::Error>> {
        let (dir, broker, workspace_id, epoch) = governed_fixture()?;
        broker.create_utf8_durable(&workspace_id, epoch, "src/created.rs", "pub fn f() {}\n")?;
        let written = fs::read_to_string(dir.path().join("src").join("created.rs"))?;
        assert_eq!(written, "pub fn f() {}\n");

        let duplicate = broker.create_utf8_durable(&workspace_id, epoch, "src/created.rs", "other");
        assert!(matches!(duplicate, Err(WorkspaceError::AlreadyExists)));
        Ok(())
    }

    #[test]
    fn create_rejects_unsupported_content() -> Result<(), Box<dyn std::error::Error>> {
        let (_dir, broker, workspace_id, epoch) = governed_fixture()?;
        assert!(matches!(
            broker.create_utf8_durable(&workspace_id, epoch, "bad.txt", "nul\0byte"),
            Err(WorkspaceError::UnsupportedText)
        ));
        let oversized = "a".repeat(usize::try_from(MAX_GOVERNED_FILE_BYTES)? + 1);
        assert!(matches!(
            broker.create_utf8_durable(&workspace_id, epoch, "big.txt", &oversized),
            Err(WorkspaceError::LimitExceeded)
        ));
        assert!(matches!(
            broker.create_utf8_durable(&workspace_id, epoch, "secrets/token.json", "{}"),
            Err(WorkspaceError::PathBlocked)
        ));
        Ok(())
    }

    #[test]
    fn replaces_only_the_exact_verified_preimage() -> Result<(), Box<dyn std::error::Error>> {
        let (dir, broker, workspace_id, epoch) = governed_fixture()?;
        let before = broker.observe_preimage(&workspace_id, epoch, "main.rs")?;
        let content_hash = before.content_hash.ok_or("missing content hash")?;
        let identity_hash = before.file_identity_hash.ok_or("missing identity hash")?;

        broker.replace_utf8_durable(
            &workspace_id,
            epoch,
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
        let (dir, broker, workspace_id, epoch) = governed_fixture()?;
        let before = broker.observe_preimage(&workspace_id, epoch, "main.rs")?;
        let content_hash = before.content_hash.ok_or("missing content hash")?;
        let identity_hash = before.file_identity_hash.ok_or("missing identity hash")?;

        fs::write(dir.path().join("main.rs"), "// external edit\n")?;
        let result = broker.replace_utf8_durable(
            &workspace_id,
            epoch,
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
        let (dir, broker, workspace_id, epoch) = governed_fixture()?;
        let before = broker.observe_preimage(&workspace_id, epoch, "src/lib.rs")?;
        let content_hash = before.content_hash.ok_or("missing content hash")?;
        let identity_hash = before.file_identity_hash.ok_or("missing identity hash")?;

        let wrong_hash = crate::hash_bytes(b"different");
        let stale = broker.delete_durable(
            &workspace_id,
            epoch,
            "src/lib.rs",
            &wrong_hash,
            &identity_hash,
        );
        assert!(matches!(stale, Err(WorkspaceError::StalePreimage)));
        assert!(dir.path().join("src").join("lib.rs").exists());

        broker.delete_durable(
            &workspace_id,
            epoch,
            "src/lib.rs",
            &content_hash,
            &identity_hash,
        )?;
        assert!(!dir.path().join("src").join("lib.rs").exists());
        Ok(())
    }

    #[test]
    fn read_effect_file_verifies_identity() -> Result<(), Box<dyn std::error::Error>> {
        let (_dir, broker, workspace_id, epoch) = governed_fixture()?;
        let before = broker.observe_preimage(&workspace_id, epoch, "main.rs")?;
        let identity_hash = before.file_identity_hash.ok_or("missing identity hash")?;

        let bytes =
            broker.read_effect_file(&workspace_id, epoch, "main.rs", Some(&identity_hash))?;
        assert_eq!(bytes.as_deref(), Some(b"fn main() {}\n".as_slice()));

        let wrong = crate::hash_bytes(b"other-identity");
        assert!(matches!(
            broker.read_effect_file(&workspace_id, epoch, "main.rs", Some(&wrong)),
            Err(WorkspaceError::StalePreimage)
        ));

        let absent = broker.read_effect_file(&workspace_id, epoch, "src/none.rs", None)?;
        assert!(absent.is_none());
        Ok(())
    }

    #[test]
    fn revocation_fails_governed_operations_closed() -> Result<(), Box<dyn std::error::Error>> {
        let (_dir, broker, workspace_id, epoch) = governed_fixture()?;
        broker.revoke(&workspace_id)?;
        let result = broker.create_utf8_durable(&workspace_id, epoch, "late.txt", "content");
        assert!(matches!(result, Err(WorkspaceError::GrantUnavailable)));
        Ok(())
    }
}
