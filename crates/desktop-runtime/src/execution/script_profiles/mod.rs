//! Per-script execution profiles and approval binding.
//!
//! Authority to run a script comes from an Airlock-approved [`ScriptPlan`]
//! (`packages/contracts/schemas/bmad-script-plan.schema.json`) plus a
//! [`ScriptProfile`] declared and hashed alongside it. Without this module the
//! sandbox would have nothing to bind approval to: a script could be swapped,
//! its interpreter re-pointed, or its declared mounts widened after review
//! without invalidating the human's "yes". `ApprovalBinding` makes each of
//! those three inputs independently load-bearing in the resulting hash.

use std::collections::BTreeSet;

use serde::Serialize;
use thiserror::Error;

use crate::{canonical_hash, CanonicalHashError, Sha256Digest};

const HASH_PURPOSE_PROFILE: &str = "bmad-script-profile";
const HASH_PURPOSE_APPROVAL: &str = "bmad-script-approval";
const HASH_SCHEMA_MAJOR: u32 = 1;

/// A filesystem mount granted to the sandboxed script, expressed as a path
/// relative to the run's content-addressed root. Absolute paths and `..`
/// segments are rejected by the resolver in `sandbox::staging`, never here:
/// this type only records the declaration that resolver checks against.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MountDeclaration {
    /// Relative mount point inside the sandbox root, e.g. `"in/spec.json"`.
    pub mount_path: String,
    /// Content hash the staged input must match before the script may read
    /// it. `None` for read-write output mounts, which have no input content.
    pub expected_content_hash: Option<Sha256Digest>,
}

/// Network reachability granted to the script. Absent unless declared:
/// there is no "default allow" variant.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NetworkPolicy {
    /// No network access of any kind, including DNS.
    Denied,
    /// Only these exact hostnames may be contacted. The cloud metadata
    /// address is filtered out at the checker regardless of what a profile
    /// author lists here; see `sandbox::enforcement::check_network`.
    AllowlistedHosts(BTreeSet<String>),
}

/// Command shape the script is permitted to invoke. The profile is not a
/// free-form shell: `argument_pattern` is an anchored regex the fully
/// rendered argument vector must match.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CommandSchema {
    pub program: String,
    pub argument_pattern: String,
}

/// Preflight resource limits. These are *declared* quotas checked by the
/// decision layer before and during staging; whether the OS can *enforce*
/// the CPU/memory/process ones on this host is reported separately by
/// `sandbox::enforcement::ControlState` and must never be conflated with
/// this declaration being present.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ResourceQuotas {
    pub timeout_ms: u64,
    pub cpu_time_ms: u64,
    pub memory_bytes: u64,
    pub max_processes: u32,
    pub max_open_files: u32,
    pub max_output_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanupPolicy {
    /// Remove the run directory whether the script succeeded or not.
    Always,
    /// Keep the run directory only when the script failed, for diagnosis.
    RetainOnFailure,
}

/// The full per-script isolation contract. Everything the sandbox is willing
/// to grant is enumerated here; nothing outside this struct is reachable.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ScriptProfile {
    pub profile_id: String,
    pub script_hash: Sha256Digest,
    pub interpreter_hash: Sha256Digest,
    pub read_only_mounts: Vec<MountDeclaration>,
    pub read_write_mounts: Vec<MountDeclaration>,
    pub environment_allowlist: BTreeSet<String>,
    pub network_policy: NetworkPolicy,
    pub command_schema: CommandSchema,
    pub quotas: ResourceQuotas,
    pub expected_outputs: Vec<String>,
    pub cleanup_policy: CleanupPolicy,
}

/// The three canonical hash components an approval binds to, computed once
/// so `changed_component` can compare them independently without
/// re-serializing the whole profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[allow(clippy::struct_field_names)]
struct ApprovalComponents {
    script_hash: Sha256Digest,
    interpreter_hash: Sha256Digest,
    profile_hash: Sha256Digest,
}

/// Binds an approval decision to the exact script, interpreter, and profile
/// it was granted for. Any drift in any one component yields a different
/// [`Self::binding_hash`], so a stale approval can never be replayed against
/// a changed script, a swapped interpreter, or a widened profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ApprovalBinding {
    components: ApprovalComponents,
    binding_hash: Sha256Digest,
}

#[derive(Debug, Error)]
pub enum ScriptProfileError {
    #[error("failed to hash the script profile: {0}")]
    Hashing(#[from] CanonicalHashError),
}

impl ApprovalBinding {
    /// Computes the approval binding for a script hash, interpreter hash,
    /// and profile. The profile itself is hashed first so the binding
    /// changes if any declared mount, quota, or policy field changes.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptProfileError::Hashing`] if canonical serialization of
    /// the profile or the composed components fails.
    pub fn compute(profile: &ScriptProfile) -> Result<Self, ScriptProfileError> {
        let profile_hash = canonical_hash(HASH_PURPOSE_PROFILE, HASH_SCHEMA_MAJOR, profile)?;
        let components = ApprovalComponents {
            script_hash: profile.script_hash,
            interpreter_hash: profile.interpreter_hash,
            profile_hash,
        };
        let binding_hash = canonical_hash(HASH_PURPOSE_APPROVAL, HASH_SCHEMA_MAJOR, &components)?;
        Ok(Self {
            components,
            binding_hash,
        })
    }

    #[must_use]
    pub const fn binding_hash(&self) -> Sha256Digest {
        self.binding_hash
    }

    /// Reports which of the three bound components changed between an
    /// already-granted binding and the profile about to run, so callers can
    /// prove independent invalidation (acceptance criterion 3) rather than
    /// only an overall hash mismatch.
    #[must_use]
    pub fn changed_components(&self, current: &Self) -> ApprovalDrift {
        ApprovalDrift {
            script_changed: self.components.script_hash != current.components.script_hash,
            interpreter_changed: self.components.interpreter_hash
                != current.components.interpreter_hash,
            profile_changed: self.components.profile_hash != current.components.profile_hash,
        }
    }
}

/// Which parts of a previously granted [`ApprovalBinding`] no longer match.
/// An empty (all-false) drift means the approval is still valid.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ApprovalDrift {
    pub script_changed: bool,
    pub interpreter_changed: bool,
    pub profile_changed: bool,
}

impl ApprovalDrift {
    #[must_use]
    pub const fn is_invalidated(&self) -> bool {
        self.script_changed || self.interpreter_changed || self.profile_changed
    }
}
