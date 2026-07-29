//! The BMAD script sandbox: content-bound staging, declared-mount path
//! resolution, and the decision layer around process/network/environment
//! isolation for approved script execution.
//!
//! See `denial` for the shared vocabulary every negative test asserts
//! against, `staging` for the real filesystem enforcement boundary, and
//! `enforcement` for the data-only decision checks plus the typed port to
//! OS-level isolation this crate does not itself perform.

mod denial;
mod enforcement;
mod result;
mod staging;

pub use denial::SandboxDenial;
pub use enforcement::{
    check_network, check_quota, resolve_environment, verify_approval_current, ControlState,
    ProcessIsolationPort, UnavailableProcessIsolation,
};
pub use result::{
    BoundedStream, CleanupStatus, ExitReason, NetworkAttempt, ResourceReading, ResourceUsageReport,
    ScriptResult, TouchedFile,
};
pub use staging::{recover_stale_runs, resolve_declared_path, stage_input, RecoveredRun, RunRoot};
