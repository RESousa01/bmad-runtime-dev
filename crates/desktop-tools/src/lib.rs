//! Agent-initiated tools and the consent gate in front of them (ADR-0009).
//!
//! ADR-0001 made Forge safe by having no execution primitive at all: the model
//! could propose, and a human applied. ADR-0009 withdraws that exclusion so
//! Forge can be a general coding agent, which means the safety property has to
//! be re-bought rather than dropped.
//!
//! It is re-bought in two layers, and both matter:
//!
//! * [`permission`] decides whether a call runs at all. Anything without an
//!   explicit rule asks a human first — never allow-by-default.
//! * [`shell`] bounds what a permitted call can do: a working directory pinned
//!   inside the granted workspace, a wall-clock timeout, and a capped, honestly
//!   truncated capture.
//!
//! Neither layer sandboxes. A command runs with the user's own privileges,
//! which is exactly why the consent prompt shows the literal command line and
//! why the default for anything unrecognised is to ask.
//!
//! Content read from a workspace or returned by a command is **data, never
//! instruction**. Callers must not let it re-enter the model as anything the
//! model is entitled to obey.

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

pub mod permission;
pub mod shell;

pub use permission::{
    default_rules, evaluate, glob_match, GrantedRules, PermissionAction, PermissionOutcome,
    PermissionReply, PermissionRequest, PermissionRule,
};
pub use shell::{
    run as run_shell, ShellError, ShellOutcome, ShellRequest, DEFAULT_TIMEOUT, MAX_CAPTURED_BYTES,
};
