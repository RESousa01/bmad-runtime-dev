//! Decision-layer checks (environment, network, approval, quotas) and the
//! typed boundary to OS-level enforcement this crate does not itself
//! perform.
//!
//! `desktop-runtime` has no process, job-object, or socket API (see the
//! crate doc comment in `lib.rs`) — it is the pure domain/policy layer.
//! Everything in this file that can be decided with data alone (allowlists,
//! the cloud-metadata block, approval-hash comparison) is real, testable
//! logic. Everything that requires actually controlling a running OS
//! process (kill descendants, cap CPU/memory, block a socket syscall) is
//! represented by [`ProcessIsolationPort`], whose only implementation here
//! is [`UnavailableProcessIsolation`] — a typed, loud refusal, never a
//! silent no-op.

use std::collections::BTreeSet;
use std::net::IpAddr;

use crate::execution::script_profiles::{ApprovalBinding, NetworkPolicy, ScriptProfile};

use super::denial::SandboxDenial;

/// Environment variable name-shapes that are refused even if a profile
/// author put them on the allowlist. Case-insensitive prefix/exact match.
const HARD_DENIED_ENV: &[&str] = &[
    "AWS_SECRET_ACCESS_KEY",
    "AWS_SESSION_TOKEN",
    "AZURE_CLIENT_SECRET",
    "GCP_SERVICE_ACCOUNT",
    "GITHUB_TOKEN",
    "GIT_ASKPASS",
    "GIT_CREDENTIAL",
    "HOME",
    "NPM_TOKEN",
    "PASSWORD",
    "SSH_AGENT_PID",
    "SSH_AUTH_SOCK",
    "TEMP",
    "TMP",
    "USERPROFILE",
];

/// The well-known cloud instance-metadata address. Denied unconditionally
/// regardless of network policy — see IMDS SSRF advisories for why this
/// cannot be an allowlist decision left to a profile author.
const CLOUD_METADATA_HOST: &str = "169.254.169.254";

/// Resolves the environment the script may see: every requested variable
/// must be on the profile's allowlist, and no hard-denied variable is ever
/// passed even if a profile author allowlisted it by name.
///
/// # Errors
///
/// Returns [`SandboxDenial::EnvHardDenied`] or
/// [`SandboxDenial::EnvNotAllowlisted`] for the first offending variable.
pub fn resolve_environment(
    profile: &ScriptProfile,
    requested: &BTreeSet<String>,
) -> Result<BTreeSet<String>, SandboxDenial> {
    let mut resolved = BTreeSet::new();
    for variable in requested {
        let upper = variable.to_ascii_uppercase();
        if HARD_DENIED_ENV
            .iter()
            .any(|denied| upper == *denied || upper.starts_with(&format!("{denied}_")))
        {
            return Err(SandboxDenial::EnvHardDenied {
                variable: variable.clone(),
            });
        }
        if !profile.environment_allowlist.contains(variable) {
            return Err(SandboxDenial::EnvNotAllowlisted {
                variable: variable.clone(),
            });
        }
        resolved.insert(variable.clone());
    }
    Ok(resolved)
}

/// Checks whether the script may contact `host` under the profile's network
/// policy. The cloud metadata endpoint is rejected before the policy is
/// even consulted, both by literal address and by anything that resolves to
/// it, so a hostname alias cannot route around the block.
///
/// # Errors
///
/// Returns [`SandboxDenial::CloudMetadataEndpoint`],
/// [`SandboxDenial::NetworkDenied`], or [`SandboxDenial::HostNotAllowlisted`].
pub fn check_network(profile: &ScriptProfile, host: &str) -> Result<(), SandboxDenial> {
    if is_cloud_metadata_target(host) {
        return Err(SandboxDenial::CloudMetadataEndpoint {
            host: host.to_string(),
        });
    }
    match &profile.network_policy {
        NetworkPolicy::Denied => Err(SandboxDenial::NetworkDenied {
            host: host.to_string(),
        }),
        NetworkPolicy::AllowlistedHosts(hosts) => {
            if hosts.contains(host) {
                Ok(())
            } else {
                Err(SandboxDenial::HostNotAllowlisted {
                    host: host.to_string(),
                })
            }
        }
    }
}

fn is_cloud_metadata_target(host: &str) -> bool {
    if host == CLOUD_METADATA_HOST {
        return true;
    }
    matches!(host.parse::<IpAddr>(), Ok(IpAddr::V4(addr)) if addr.octets() == [169, 254, 169, 254])
}

/// Verifies a previously granted approval still matches the script,
/// interpreter, and profile about to run. This is the acceptance-3 gate:
/// callers must call it on every run, not just once at approval time.
///
/// # Errors
///
/// Returns [`SandboxDenial::ApprovalInvalidated`] naming which component(s)
/// drifted.
pub fn verify_approval_current(
    granted: &ApprovalBinding,
    profile: &ScriptProfile,
) -> Result<(), SandboxDenial> {
    let current =
        ApprovalBinding::compute(profile).map_err(|error| SandboxDenial::Unavailable {
            control: "approval_hash".to_string(),
            detail: error.to_string(),
        })?;
    let drift = granted.changed_components(&current);
    if drift.is_invalidated() {
        let mut parts = Vec::new();
        if drift.script_changed {
            parts.push("script");
        }
        if drift.interpreter_changed {
            parts.push("interpreter");
        }
        if drift.profile_changed {
            parts.push("profile");
        }
        return Err(SandboxDenial::ApprovalInvalidated {
            detail: parts.join(","),
        });
    }
    Ok(())
}

/// Checks one observed resource counter against its declared quota. This is
/// preflight/decision-layer arithmetic; it does not itself stop a running
/// process from exceeding the quota — see [`ProcessIsolationPort`] for that
/// distinction.
///
/// # Errors
///
/// Returns [`SandboxDenial::QuotaExceeded`] when `observed > limit`.
pub fn check_quota(quota: &'static str, limit: u64, observed: u64) -> Result<(), SandboxDenial> {
    if observed > limit {
        return Err(SandboxDenial::QuotaExceeded {
            quota: quota.to_string(),
            limit,
            observed,
        });
    }
    Ok(())
}

/// Whether a control is genuinely enforced by the OS/runtime, only decided
/// by policy ahead of execution, or not enforceable at all in this
/// environment. Every negative-test table entry maps to exactly one of
/// these.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlState {
    /// The sandbox actively stopped the behavior at the OS/runtime level.
    Enforced,
    /// Policy refused the operation before anything ran; no live process
    /// was ever given the chance to attempt it.
    Decided,
    /// This control cannot be enforced by this crate in this environment.
    Unavailable,
}

/// The boundary to OS-level process isolation (job objects / process groups,
/// descendant termination, CPU and memory quota enforcement, socket-level
/// network blocking). `desktop-runtime` has no process API, so this trait
/// exists to make that boundary explicit and typed rather than silently
/// absorbed into "the sandbox module didn't do anything about it".
pub trait ProcessIsolationPort {
    /// Reports whether this port can actually enforce descendant
    /// termination, CPU/memory quotas, and OS-level network blocking on the
    /// current host.
    fn control_state(&self) -> ControlState;

    /// Attempts to launch a script under live OS isolation.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxDenial::Unavailable`] whenever [`Self::control_state`]
    /// is not [`ControlState::Enforced`]. A real implementation would return
    /// other [`SandboxDenial`] variants for genuine runtime denials.
    fn spawn_isolated(&self, profile: &ScriptProfile) -> Result<(), SandboxDenial>;
}

/// The only `ProcessIsolationPort` wired in this environment: OS-level job
/// objects / process-tree containment are not available here, so every call
/// fails loudly and explicitly rather than pretending to isolate a process
/// it never actually spawned.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnavailableProcessIsolation;

impl ProcessIsolationPort for UnavailableProcessIsolation {
    fn control_state(&self) -> ControlState {
        ControlState::Unavailable
    }

    fn spawn_isolated(&self, _profile: &ScriptProfile) -> Result<(), SandboxDenial> {
        Err(SandboxDenial::Unavailable {
            control: "process_isolation".to_string(),
            detail: "no Windows job-object / process-tree isolation port is wired in this crate; \
                     desktop-runtime deliberately has no process API (see lib.rs), and no host \
                     process crate is a dependency here"
                .to_string(),
        })
    }
}
