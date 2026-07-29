//! Bounded, structured script results and cleanup evidence.
//!
//! A script result must never let an unbounded stream or file list become
//! the caller's problem: every collection here has a declared cap sourced
//! from the profile's [`ResourceQuotas`](crate::execution::script_profiles::ResourceQuotas),
//! and truncation is recorded rather than silently dropped.

use serde::Serialize;

use super::denial::SandboxDenial;

/// A captured output stream, bounded to the profile's declared cap.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BoundedStream {
    pub captured: Vec<u8>,
    pub total_bytes_seen: u64,
    pub truncated: bool,
}

impl BoundedStream {
    /// Appends `chunk`, capping retained bytes at `limit` and recording
    /// truncation instead of growing past it.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            captured: Vec::new(),
            total_bytes_seen: 0,
            truncated: false,
        }
    }

    pub fn push(&mut self, chunk: &[u8], limit: u64) {
        self.total_bytes_seen += chunk.len() as u64;
        let remaining = limit.saturating_sub(self.captured.len() as u64);
        if remaining == 0 {
            if !chunk.is_empty() {
                self.truncated = true;
            }
            return;
        }
        let take = usize::try_from(remaining.min(chunk.len() as u64)).unwrap_or(chunk.len());
        self.captured.extend_from_slice(&chunk[..take]);
        if take < chunk.len() {
            self.truncated = true;
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum ExitReason {
    Completed { exit_code: i32 },
    Denied(SandboxDenial),
    TimedOut,
    Killed,
    QuotaExceeded { quota: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TouchedFile {
    pub mount_path: String,
    pub write_attempted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NetworkAttempt {
    pub host: String,
    pub allowed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum CleanupStatus {
    Cleaned,
    Quarantined { reason: String },
    Pending,
}

/// A single resource usage figure with its enforcement provenance attached,
/// so a caller can never mistake "unavailable" for "zero usage".
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum ResourceReading {
    Measured(u64),
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResourceUsageReport {
    pub cpu_time_ms: ResourceReading,
    pub peak_memory_bytes: ResourceReading,
    pub process_count: ResourceReading,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ScriptResult {
    pub run_id: String,
    pub exit_reason: ExitReason,
    pub stdout: BoundedStream,
    pub stderr: BoundedStream,
    pub touched_files: Vec<TouchedFile>,
    pub network_attempts: Vec<NetworkAttempt>,
    pub resource_usage: ResourceUsageReport,
    pub cleanup_status: CleanupStatus,
}
