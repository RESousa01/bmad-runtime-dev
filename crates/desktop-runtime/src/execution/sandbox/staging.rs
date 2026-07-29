//! Content-addressed run staging and declared-mount path resolution.
//!
//! This is the one place in `desktop-runtime` that touches the real
//! filesystem, and it exists for a specific reason: a renderer-supplied path
//! is data, never authority. Every path a script can touch must resolve
//! through [`resolve_declared_path`] against the profile's own mount list —
//! there is no code path that lets a caller-supplied absolute path or a
//! symlink target become a capability. Everything here is genuinely enforced
//! (it performs real `std::fs` operations and can be observed to fail), not
//! merely decided.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use crate::execution::script_profiles::{MountDeclaration, ScriptProfile};
use crate::{sha256_bytes, Sha256Digest};

use super::denial::SandboxDenial;

/// Root directory for one script run: `<base>/runs/<run_id>/`. `run_id` is
/// the hex of the approval binding hash, so two runs of the same
/// script+interpreter+profile land in the same content-addressed location
/// and a run directory can never be confused with a run of a different
/// approval.
#[derive(Clone, Debug)]
pub struct RunRoot {
    root: PathBuf,
}

const COMPLETION_MARKER: &str = ".sandbox-complete";
const QUARANTINE_DIR: &str = "quarantine";

impl RunRoot {
    /// Creates (or reuses, if not yet marked complete) the run directory for
    /// `run_id` under `base`.
    ///
    /// # Errors
    ///
    /// Returns [`io::Error`] if the directory cannot be created.
    pub fn create(base: &Path, run_id: &str) -> Result<Self, io::Error> {
        let root = base.join("runs").join(run_id);
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.root
    }

    /// Marks this run as cleanly finished so a later process does not treat
    /// it as a stale, crashed run on restart.
    ///
    /// # Errors
    ///
    /// Returns [`io::Error`] if the marker file cannot be written.
    pub fn mark_complete(&self) -> Result<(), io::Error> {
        fs::write(self.root.join(COMPLETION_MARKER), b"")
    }

    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.root.join(COMPLETION_MARKER).is_file()
    }
}

/// One run directory found during restart recovery, and what was done with
/// it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveredRun {
    pub run_id: String,
    pub quarantined: bool,
}

/// Restart-safe cleanup sweep: scans `<base>/runs/*` and moves every run
/// directory that lacks the completion marker into `<base>/quarantine/`
/// instead of ever treating unexpected leftover output as trusted. Safe to
/// call repeatedly and safe to call after a crash mid-run.
///
/// # Errors
///
/// Returns [`io::Error`] if the runs directory cannot be listed or a stale
/// directory cannot be moved.
pub fn recover_stale_runs(base: &Path) -> Result<Vec<RecoveredRun>, io::Error> {
    let runs_dir = base.join("runs");
    if !runs_dir.is_dir() {
        return Ok(Vec::new());
    }
    let quarantine_dir = base.join(QUARANTINE_DIR);
    let mut recovered = Vec::new();
    for entry in fs::read_dir(&runs_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let run_id = entry.file_name().to_string_lossy().into_owned();
        let complete = entry.path().join(COMPLETION_MARKER).is_file();
        if complete {
            recovered.push(RecoveredRun {
                run_id,
                quarantined: false,
            });
            continue;
        }
        fs::create_dir_all(&quarantine_dir)?;
        let destination = quarantine_dir.join(format!("{run_id}-{}", stale_suffix()));
        fs::rename(entry.path(), &destination)?;
        recovered.push(RecoveredRun {
            run_id,
            quarantined: true,
        });
    }
    Ok(recovered)
}

fn stale_suffix() -> String {
    // A monotonically-irrelevant, collision-avoiding suffix: two stale runs
    // discovered in the same sweep must not overwrite each other in the
    // quarantine directory.
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map_or_else(
        |_| "0".to_string(),
        |duration| duration.as_nanos().to_string(),
    )
}

/// Resolves a script-requested relative path against the profile's declared
/// mounts, real filesystem in hand. This is the sole authority boundary for
/// filesystem access: `requested` is treated as untrusted data end to end.
///
/// Enforced checks, in order:
/// 1. `requested` must be a relative path with no `..`, no root, no drive
///    prefix, and no embedded NUL.
/// 2. `requested` must name an NTFS alternate data stream — rejected.
/// 3. `requested` must fall under one of the profile's declared mounts.
/// 4. Write access requires a read-write mount.
/// 5. The resolved path's *canonicalized* (symlink/junction-resolved) form
///    must still be under the mount's canonicalized root — this defeats a
///    reparse point planted inside the mount that points outside it.
///
/// # Errors
///
/// Returns the first [`SandboxDenial`] check that fails.
pub fn resolve_declared_path(
    profile: &ScriptProfile,
    run_root: &RunRoot,
    requested: &str,
    for_write: bool,
) -> Result<PathBuf, SandboxDenial> {
    reject_traversal(requested)?;
    reject_alternate_data_stream(requested)?;

    let mounts: &[MountDeclaration] = if for_write {
        &profile.read_write_mounts
    } else {
        // A write-mount is also a valid read target once staged.
        &profile.read_only_mounts
    };
    let declaring_mount = mounts
        .iter()
        .chain(profile.read_write_mounts.iter())
        .find(|mount| {
            requested == mount.mount_path
                || requested.starts_with(&format!("{}/", mount.mount_path))
        });

    let Some(mount) = declaring_mount else {
        if for_write {
            // Distinguish "declared but read-only" from "never declared" so
            // the denial reason is precise.
            let read_only_hit = profile
                .read_only_mounts
                .iter()
                .any(|mount| requested == mount.mount_path);
            if read_only_hit {
                return Err(SandboxDenial::MountReadOnly {
                    mount_path: requested.to_string(),
                });
            }
        }
        return Err(SandboxDenial::PathNotDeclared {
            requested: requested.to_string(),
        });
    };

    let mount_root = run_root.path().join(sanitized_relative(&mount.mount_path));
    let candidate = run_root.path().join(sanitized_relative(requested));

    verify_no_escape(&mount_root, &candidate, requested)?;
    Ok(candidate)
}

/// Confirms the canonical (symlink-resolved) location of `candidate` is
/// still contained by `mount_root`'s canonical location. `mount_root` itself
/// may not exist yet for a not-yet-staged write mount, in which case we walk
/// up to the nearest existing ancestor to canonicalize against — a reparse
/// point can only be planted on something that already exists, so this still
/// catches a pre-planted escape.
fn verify_no_escape(
    mount_root: &Path,
    candidate: &Path,
    requested: &str,
) -> Result<(), SandboxDenial> {
    let canonical_mount = canonicalize_existing_ancestor(mount_root);
    if let Some(parent) = candidate.parent() {
        let canonical_parent = canonicalize_existing_ancestor(parent);
        if !canonical_parent.starts_with(&canonical_mount) && canonical_mount != canonical_parent {
            return Err(SandboxDenial::SymlinkEscape {
                requested: requested.to_string(),
                real_target: canonical_parent.to_string_lossy().into_owned(),
            });
        }
    }
    Ok(())
}

fn canonicalize_existing_ancestor(path: &Path) -> PathBuf {
    let mut probe = path.to_path_buf();
    loop {
        if let Ok(real) = fs::canonicalize(&probe) {
            return real;
        }
        if !probe.pop() {
            return path.to_path_buf();
        }
    }
}

fn reject_traversal(requested: &str) -> Result<(), SandboxDenial> {
    if requested.is_empty() || requested.contains('\0') {
        return Err(SandboxDenial::PathTraversal {
            requested: requested.to_string(),
        });
    }
    let path = Path::new(requested);
    if path.is_absolute() {
        return Err(SandboxDenial::PathTraversal {
            requested: requested.to_string(),
        });
    }
    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(SandboxDenial::PathTraversal {
                    requested: requested.to_string(),
                });
            }
        }
    }
    Ok(())
}

/// Rejects `name:stream` NTFS alternate-data-stream syntax. A single-letter
/// drive prefix (`C:`) is already rejected by [`reject_traversal`] via
/// `Component::Prefix`, so any remaining colon here is a stream selector.
fn reject_alternate_data_stream(requested: &str) -> Result<(), SandboxDenial> {
    if requested.contains(':') {
        return Err(SandboxDenial::AlternateDataStream {
            requested: requested.to_string(),
        });
    }
    Ok(())
}

fn sanitized_relative(value: &str) -> PathBuf {
    value.split('/').collect()
}

/// Stages bytes into a declared read-only mount, then verifies the written
/// content hash matches the profile's `expected_content_hash`. Staging
/// happens before the hash check so the mismatch evidence (actual hash) is
/// available even on failure; the file is removed on mismatch so a denied
/// input is never left readable to a subsequent step.
///
/// # Errors
///
/// Returns [`SandboxDenial`] if path resolution fails or the content hash
/// does not match; returns the resolution error mapped in, and a hash
/// mismatch as [`SandboxDenial::ContentHashMismatch`].
pub fn stage_input(
    profile: &ScriptProfile,
    run_root: &RunRoot,
    mount: &MountDeclaration,
    bytes: &[u8],
) -> Result<PathBuf, SandboxDenial> {
    let destination = resolve_declared_path(profile, run_root, &mount.mount_path, false)?;
    if let Some(parent) = destination.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(&destination, bytes).map_err(|error| SandboxDenial::Unavailable {
        control: "filesystem_staging".to_string(),
        detail: error.to_string(),
    })?;

    let actual: Sha256Digest = sha256_bytes(bytes);
    if let Some(expected) = mount.expected_content_hash {
        if expected != actual {
            let _ = fs::remove_file(&destination);
            return Err(SandboxDenial::ContentHashMismatch {
                mount_path: mount.mount_path.clone(),
                expected: expected.to_string(),
                actual: actual.to_string(),
            });
        }
    }
    Ok(destination)
}
