use std::fs::File;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use desktop_runtime::{
    BmadKernelErrorCode, BmadLocationClass, BmadSourceEntry, BmadSourceKind, BmadSourceSnapshot,
};
use thiserror::Error;
use walkdir::WalkDir;

const MAX_ENTRY_BYTES: u64 = 1_048_576;
const MAX_ENTRIES: usize = 4_096;

#[derive(Debug, Error)]
pub enum BmadSnapshotError {
    #[error("the BMAD workspace root is unavailable")]
    RootUnavailable,
    #[error("the BMAD workspace snapshot exceeded its bounded-read limits")]
    LimitExceeded,
    #[error("the BMAD workspace source contains an invalid path or entry")]
    InvalidSource,
    #[error("BMAD workspace snapshot I/O failed")]
    Io(#[from] std::io::Error),
}

/// Reads the Method CLI composite roots as bounded immutable source bytes.
///
/// `_bmad` is treated as control/config data. Host-native `.agents/skills` and
/// `.claude/skills` are observed independently; no staging manifest is trusted
/// as a substitute for those final bytes.
///
/// # Errors
///
/// Returns a stable error for an unavailable root, unsafe/symlinked entries,
/// I/O failure, or per-file/aggregate source bounds.
pub fn read_bmad_source_snapshot(
    workspace_root: impl AsRef<Path>,
) -> Result<BmadSourceSnapshot, BmadSnapshotError> {
    let workspace_root = workspace_root.as_ref();
    let metadata = std::fs::symlink_metadata(workspace_root)
        .map_err(|_| BmadSnapshotError::RootUnavailable)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(BmadSnapshotError::RootUnavailable);
    }

    let roots = [
        ("_bmad", BmadLocationClass::BmadControl),
        (".agents/skills", BmadLocationClass::HostNativeAgents),
        (".claude/skills", BmadLocationClass::HostNativeClaude),
    ];
    let mut entries = Vec::new();
    for (relative_root, location) in roots {
        let discovery_root = workspace_root.join(wire_to_native(relative_root));
        if !discovery_root.exists() {
            continue;
        }
        let root_metadata = std::fs::symlink_metadata(&discovery_root)?;
        if !root_metadata.is_dir() || root_metadata.file_type().is_symlink() {
            return Err(BmadSnapshotError::InvalidSource);
        }
        for item in WalkDir::new(&discovery_root).follow_links(false) {
            let item = item.map_err(|error| {
                error
                    .into_io_error()
                    .map_or(BmadSnapshotError::InvalidSource, BmadSnapshotError::Io)
            })?;
            let file_type = item.file_type();
            if file_type.is_symlink() {
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            if entries.len() >= MAX_ENTRIES {
                return Err(BmadSnapshotError::LimitExceeded);
            }
            let relative = item
                .path()
                .strip_prefix(workspace_root)
                .map_err(|_| BmadSnapshotError::InvalidSource)?;
            let wire_path = native_to_wire(relative)?;
            if is_sensitive_path(&wire_path) {
                continue;
            }
            let bytes = read_bounded(item.path())?;
            entries.push(
                BmadSourceEntry::new(wire_path, bytes, BmadSourceKind::MethodComposite, location)
                    .map_err(|error| map_runtime_error(&error))?,
            );
        }
    }
    BmadSourceSnapshot::new(entries).map_err(|error| map_runtime_error(&error))
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, BmadSnapshotError> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > MAX_ENTRY_BYTES
    {
        return Err(BmadSnapshotError::LimitExceeded);
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(metadata.len()).map_err(|_| BmadSnapshotError::LimitExceeded)?,
    );
    File::open(path)?
        .take(MAX_ENTRY_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > usize::try_from(MAX_ENTRY_BYTES).unwrap_or(usize::MAX) {
        return Err(BmadSnapshotError::LimitExceeded);
    }
    Ok(bytes)
}

fn native_to_wire(path: &Path) -> Result<String, BmadSnapshotError> {
    let components = path
        .components()
        .map(|component| {
            component
                .as_os_str()
                .to_str()
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .ok_or(BmadSnapshotError::InvalidSource)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if components.is_empty() {
        return Err(BmadSnapshotError::InvalidSource);
    }
    Ok(components.join("/"))
}

fn wire_to_native(path: &str) -> PathBuf {
    path.split('/').collect()
}

fn is_sensitive_path(path: &str) -> bool {
    path.split('/').any(|component| {
        let lower = component.to_ascii_lowercase();
        lower == ".env"
            || lower.starts_with(".env.")
            || matches!(
                lower.as_str(),
                ".npmrc"
                    | ".netrc"
                    | "_netrc"
                    | "credentials.json"
                    | "token.json"
                    | "client_secret.json"
            )
    })
}

fn map_runtime_error(error: &desktop_runtime::BmadKernelError) -> BmadSnapshotError {
    if error.code() == BmadKernelErrorCode::SourceLimitExceeded {
        BmadSnapshotError::LimitExceeded
    } else {
        BmadSnapshotError::InvalidSource
    }
}
