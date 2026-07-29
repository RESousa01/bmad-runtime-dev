//! The single vocabulary of sandbox denial reasons.
//!
//! Every negative test in `tests/bmad_script_sandbox.rs` asserts on a
//! specific variant here, never on "it returned an error". Without a shared,
//! matchable reason type a caller (or a lazy test) can't tell "the sandbox
//! blocked this" apart from "the path happened not to exist".

use serde::Serialize;

/// Why a requested operation, path, environment variable, network target, or
/// approval was refused. `Unavailable` is distinct from every other variant:
/// it means this environment cannot enforce the control at all, and must
/// never be produced by a code path that silently allowed the operation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum SandboxDenial {
    /// The requested path was absolute, contained a `..` segment, or
    /// otherwise did not resolve to a location under the declared mount.
    PathTraversal { requested: String },
    /// The requested path is not one of the profile's declared mounts.
    PathNotDeclared { requested: String },
    /// The path resolved inside the mount textually, but its real
    /// (symlink/junction/reparse-point-resolved) target lies outside the
    /// mount's canonical root.
    SymlinkEscape {
        requested: String,
        real_target: String,
    },
    /// The path names an NTFS alternate data stream (`name:stream`).
    AlternateDataStream { requested: String },
    /// The mount is read-only; the operation required write access.
    MountReadOnly { mount_path: String },
    /// The staged input's content hash did not match the profile's
    /// declared `expected_content_hash` — the script bytes (or another
    /// declared input) were tampered with after approval.
    ContentHashMismatch {
        mount_path: String,
        expected: String,
        actual: String,
    },
    /// The environment variable is not on the profile's allowlist.
    EnvNotAllowlisted { variable: String },
    /// The environment variable is a hard-denied secret-shaped name and is
    /// refused even if a profile author allowlisted it.
    EnvHardDenied { variable: String },
    /// The network policy denies all access and a connection was requested.
    NetworkDenied { host: String },
    /// The host is not on the profile's network allowlist.
    HostNotAllowlisted { host: String },
    /// The target is a cloud instance-metadata endpoint. Denied
    /// unconditionally; no profile may allowlist it.
    CloudMetadataEndpoint { host: String },
    /// The previously granted approval no longer matches the script,
    /// interpreter, or profile about to run.
    ApprovalInvalidated { detail: String },
    /// A declared resource quota was exceeded during preflight or run.
    QuotaExceeded {
        quota: String,
        limit: u64,
        observed: u64,
    },
    /// A stale run directory from a previous process was found without a
    /// completion marker; its contents are quarantined rather than reused.
    QuarantinedRun { run_id: String },
    /// This control is not enforceable on the current host/environment.
    /// Carries the exact reason so callers can distinguish "denied" from
    /// "we cannot tell". Never emitted by a path that reports success.
    Unavailable { control: String, detail: String },
}

impl SandboxDenial {
    /// Stable machine-readable tag for logging/evidence records, independent
    /// of the serde field layout.
    #[must_use]
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::PathTraversal { .. } => "path_traversal",
            Self::PathNotDeclared { .. } => "path_not_declared",
            Self::SymlinkEscape { .. } => "symlink_escape",
            Self::AlternateDataStream { .. } => "alternate_data_stream",
            Self::MountReadOnly { .. } => "mount_read_only",
            Self::ContentHashMismatch { .. } => "content_hash_mismatch",
            Self::EnvNotAllowlisted { .. } => "env_not_allowlisted",
            Self::EnvHardDenied { .. } => "env_hard_denied",
            Self::NetworkDenied { .. } => "network_denied",
            Self::HostNotAllowlisted { .. } => "host_not_allowlisted",
            Self::CloudMetadataEndpoint { .. } => "cloud_metadata_endpoint",
            Self::ApprovalInvalidated { .. } => "approval_invalidated",
            Self::QuotaExceeded { .. } => "quota_exceeded",
            Self::QuarantinedRun { .. } => "quarantined_run",
            Self::Unavailable { .. } => "unavailable",
        }
    }
}
