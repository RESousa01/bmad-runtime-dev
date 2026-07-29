//! The one adapter contract every Method skill (MTH-01..MTH-48) implements.
//!
//! Without this module, 48 downstream tasks would each invent their own shape
//! for "what does this skill declare" -- one skill's adapter might carry a
//! `Vec<String>` of action names, another a `HashMap`, a third free-form
//! prose describing its resources. Help, the runtime, and the compatibility
//! ledger would then have no single vocabulary to agree on, and nothing
//! would stop a 49th reviewer from having to re-derive what "adapted" versus
//! "shim" means for the third time. [`BmadMethodSkill`] fixes the shape once:
//! implement one method, return one typed [`BmadSkillDescriptor`], and every
//! MTH acceptance criterion about declared actions, invocation modes,
//! predecessor/follower ordering, and support-state parity is satisfied by
//! construction rather than by convention.
//!
//! **Authority is structural, not a promise.** [`BmadMethodSkill`] exposes no
//! filesystem, process, network, model, or scheduler capability -- an
//! implementation can only *describe* a skill's declared shape through
//! already-validated data. It cannot open a file, spawn a process, make a
//! network call, invoke a model, or schedule anything, because no method on
//! this trait or [`BmadSkillDescriptor`] accepts or returns a value capable
//! of doing so. `bmad_skill_harness_module_grants_no_capability` in
//! `tests/bmad_skill_harness.rs` proves this by scanning this file's source
//! for such a reference, the same technique
//! `chat_bmad_capability_separation.rs` already uses for `chat.rs`.
//!
//! Every compound value here is reused from the engine modules that already
//! own its vocabulary -- [`BmadInvocationDeclaredAction`] and
//! [`BmadInvocationMode`] from `invocation.rs`, [`BmadArtifactKind`] and
//! [`BmadArtifactCompletionHookRunHook`] from `artifacts.rs`'s generated
//! contract, and [`BmadResource`] from `resources.rs`. The one type this
//! module does introduce, [`BmadSkillSupportState`], has no existing
//! home: `help.rs`'s `BmadHelpSupportState` is a three-way Exact/Adapted/Shim
//! answer to "how faithfully does the shipped implementation match its
//! source," scoped to capabilities Help already knows how to run. The MTH
//! acceptance criteria need a fourth state -- `Unavailable` -- for a skill
//! whose ledger row has not been decided at all yet, which is not a support
//! *quality* Help can report on; it is the absence of a decision. Widening
//! `BmadHelpSupportState` itself would force every existing Help caller to
//! handle a state Help can never actually observe, so this module defines its
//! own four-way enum and is the one place `Unavailable` is representable.

use crate::bmad::BmadResource;
use crate::generated_contracts::{
    BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationDeclaredAction,
    BmadInvocationMode,
};
use crate::Sha256Digest;

use thiserror::Error;

/// Schema tag for the descriptor shape this module produces, following the
/// same `sapphirus.<name>.v1` convention as the generated contracts it wraps.
pub const BMAD_SKILL_HARNESS_SCHEMA: &str = "sapphirus.bmad-skill-harness.v1";

const MAX_ACTIONS: usize = 64;
const MAX_OUTPUT_LOCATIONS: usize = 64;
const MAX_ARTIFACT_KINDS: usize = 16;
const MAX_COMPLETION_HOOKS: usize = 16;
const MAX_RESOURCES: usize = 512;
const MAX_MODES: usize = 2;
const MAX_ID_BYTES: usize = 256;

/// How faithfully a skill's shipped adapter matches its reviewed source.
///
/// This is the single source of truth the MTH acceptance criteria require:
/// Help, the desktop UI, the runtime, and `compatibility-ledger.json` must
/// all report the *same* state for a given skill, so exactly one enum may
/// produce it. `Unavailable` is the ledger's honest default for a `pending`
/// row -- a skill nobody has classified yet -- and is deliberately distinct
/// from `Shim` (classified, and known to be a stand-in).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadSkillSupportState {
    Exact,
    Adapted,
    Shim,
    Unavailable,
}

/// Bounded, typed data describing one Method skill's declared shape.
///
/// Every field is already-validated data; there is no field capable of
/// carrying prose that could be mistaken for authority (no free-form "what
/// this skill is allowed to do" string). Fields are private and reached only
/// through accessors so a caller can inspect but never hand-assemble a
/// descriptor that skipped [`BmadSkillDescriptor::new`]'s bounds checks.
///
/// Deliberately does not derive `PartialEq`: [`BmadInvocationDeclaredAction`]
/// (a generated contract type owned outside this lease) does not implement
/// it, so callers compare individual accessor results instead of the whole
/// descriptor.
#[derive(Clone, Debug)]
pub struct BmadSkillDescriptor {
    canonical_id: String,
    source_path: String,
    source_sha256: Sha256Digest,
    declared_actions: Vec<BmadInvocationDeclaredAction>,
    supported_modes: Vec<BmadInvocationMode>,
    predecessor: Option<String>,
    follower: Option<String>,
    output_locations: Vec<String>,
    artifact_kinds: Vec<BmadArtifactKind>,
    completion_hooks: Vec<BmadArtifactCompletionHookRunHook>,
    declared_resources: Vec<BmadResource>,
    support_state: BmadSkillSupportState,
}

/// The unvalidated fields a per-skill adapter assembles before calling
/// [`BmadSkillDescriptor::new`]. Kept as a distinct, all-public input type --
/// matching `BmadInvocationRequest`'s shape in `invocation.rs` -- so a twelve
/// -field constructor call reads by name at the call site instead of by
/// position.
#[derive(Clone, Debug)]
pub struct BmadSkillDescriptorInput {
    pub canonical_id: String,
    pub source_path: String,
    pub source_sha256: Sha256Digest,
    pub declared_actions: Vec<BmadInvocationDeclaredAction>,
    pub supported_modes: Vec<BmadInvocationMode>,
    pub predecessor: Option<String>,
    pub follower: Option<String>,
    pub output_locations: Vec<String>,
    pub artifact_kinds: Vec<BmadArtifactKind>,
    pub completion_hooks: Vec<BmadArtifactCompletionHookRunHook>,
    pub declared_resources: Vec<BmadResource>,
    pub support_state: BmadSkillSupportState,
}

/// Fail-closed result for descriptor construction.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum BmadSkillHarnessError {
    #[error("skill descriptor identity is empty, oversized, or contains a control character")]
    InvalidId,
    #[error("skill descriptor exceeds a bounded collection count")]
    BoundsViolation,
    #[error("skill descriptor declares no supported invocation mode")]
    NoSupportedModes,
}

impl BmadSkillDescriptor {
    /// Validates and builds one skill descriptor.
    ///
    /// # Errors
    ///
    /// Returns [`BmadSkillHarnessError::InvalidId`] when the canonical id,
    /// source path, predecessor, or follower is empty, oversized, or
    /// contains a control character; [`BmadSkillHarnessError::BoundsViolation`]
    /// when any declared collection exceeds its bounded count; and
    /// [`BmadSkillHarnessError::NoSupportedModes`] when no invocation mode is
    /// declared, which would make the skill uninvocable in either mode.
    pub fn new(input: BmadSkillDescriptorInput) -> Result<Self, BmadSkillHarnessError> {
        if !valid_id(&input.canonical_id) || !valid_id(&input.source_path) {
            return Err(BmadSkillHarnessError::InvalidId);
        }
        for reference in [input.predecessor.as_deref(), input.follower.as_deref()]
            .into_iter()
            .flatten()
        {
            if !valid_id(reference) {
                return Err(BmadSkillHarnessError::InvalidId);
            }
        }
        if input.declared_actions.len() > MAX_ACTIONS
            || input.output_locations.len() > MAX_OUTPUT_LOCATIONS
            || input.artifact_kinds.len() > MAX_ARTIFACT_KINDS
            || input.completion_hooks.len() > MAX_COMPLETION_HOOKS
            || input.declared_resources.len() > MAX_RESOURCES
            || input.supported_modes.len() > MAX_MODES
        {
            return Err(BmadSkillHarnessError::BoundsViolation);
        }
        if input.supported_modes.is_empty() {
            return Err(BmadSkillHarnessError::NoSupportedModes);
        }
        Ok(Self {
            canonical_id: input.canonical_id,
            source_path: input.source_path,
            source_sha256: input.source_sha256,
            declared_actions: input.declared_actions,
            supported_modes: input.supported_modes,
            predecessor: input.predecessor,
            follower: input.follower,
            output_locations: input.output_locations,
            artifact_kinds: input.artifact_kinds,
            completion_hooks: input.completion_hooks,
            declared_resources: input.declared_resources,
            support_state: input.support_state,
        })
    }

    #[must_use]
    pub fn canonical_id(&self) -> &str {
        &self.canonical_id
    }

    #[must_use]
    pub fn source_path(&self) -> &str {
        &self.source_path
    }

    #[must_use]
    pub const fn source_sha256(&self) -> Sha256Digest {
        self.source_sha256
    }

    #[must_use]
    pub fn declared_actions(&self) -> &[BmadInvocationDeclaredAction] {
        &self.declared_actions
    }

    #[must_use]
    pub fn supported_modes(&self) -> &[BmadInvocationMode] {
        &self.supported_modes
    }

    #[must_use]
    pub fn predecessor(&self) -> Option<&str> {
        self.predecessor.as_deref()
    }

    #[must_use]
    pub fn follower(&self) -> Option<&str> {
        self.follower.as_deref()
    }

    #[must_use]
    pub fn output_locations(&self) -> &[String] {
        &self.output_locations
    }

    #[must_use]
    pub fn artifact_kinds(&self) -> &[BmadArtifactKind] {
        &self.artifact_kinds
    }

    #[must_use]
    pub fn completion_hooks(&self) -> &[BmadArtifactCompletionHookRunHook] {
        &self.completion_hooks
    }

    #[must_use]
    pub fn declared_resources(&self) -> &[BmadResource] {
        &self.declared_resources
    }

    #[must_use]
    pub const fn support_state(&self) -> BmadSkillSupportState {
        self.support_state
    }
}

fn valid_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_ID_BYTES && !value.chars().any(char::is_control)
}

/// The contract every per-skill adapter implements.
///
/// One method, one already-validated return value. Nothing on this trait
/// accepts a workspace path, a process handle, a network client, a model
/// port, or a scheduler -- an implementor can describe a skill's declared
/// shape and nothing else. Running the skill, resolving its invocation,
/// advancing its session, and writing its artifact all remain the engine's
/// job (`invocation.rs`, `state_machine.rs`, `artifacts.rs`); this trait
/// never gains a method that would let an adapter reach for one of those
/// itself.
pub trait BmadMethodSkill {
    /// Returns this skill's declared descriptor.
    fn descriptor(&self) -> BmadSkillDescriptor;
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn minimal_input() -> BmadSkillDescriptorInput {
        BmadSkillDescriptorInput {
            canonical_id: "bmm:bmad-product-brief".to_owned(),
            source_path: "src/bmm-skills/1-analysis/bmad-product-brief/SKILL.md".to_owned(),
            source_sha256: crate::sha256_bytes(b"skill source"),
            declared_actions: Vec::new(),
            supported_modes: vec![
                BmadInvocationMode::Interactive,
                BmadInvocationMode::Headless,
            ],
            predecessor: None,
            follower: Some("bmm:bmad-prd".to_owned()),
            output_locations: vec!["planning_artifacts".to_owned()],
            artifact_kinds: vec![BmadArtifactKind::Document],
            completion_hooks: vec![BmadArtifactCompletionHookRunHook::PersistArtifacts],
            declared_resources: Vec::new(),
            support_state: BmadSkillSupportState::Unavailable,
        }
    }

    // Break caught: a descriptor silently accepting an id built from raw,
    // unvalidated strings, letting a malformed canonical id or source path
    // reach the compatibility ledger or Help projection unnoticed.
    #[test]
    fn bmad_skill_harness_descriptor_round_trips_every_declared_field() {
        let descriptor =
            BmadSkillDescriptor::new(minimal_input()).expect("valid descriptor constructs");
        assert_eq!(descriptor.canonical_id(), "bmm:bmad-product-brief");
        assert_eq!(
            descriptor.source_path(),
            "src/bmm-skills/1-analysis/bmad-product-brief/SKILL.md"
        );
        assert_eq!(descriptor.predecessor(), None);
        assert_eq!(descriptor.follower(), Some("bmm:bmad-prd"));
        assert_eq!(
            descriptor.output_locations().to_vec(),
            vec!["planning_artifacts".to_owned()]
        );
        assert_eq!(
            descriptor.artifact_kinds().to_vec(),
            vec![BmadArtifactKind::Document]
        );
        assert_eq!(
            descriptor.completion_hooks().to_vec(),
            vec![BmadArtifactCompletionHookRunHook::PersistArtifacts]
        );
        assert_eq!(
            descriptor.support_state(),
            BmadSkillSupportState::Unavailable
        );
    }

    // Break caught: collapsing Exact/Adapted/Shim/Unavailable into fewer
    // states, which would make it impossible for the ledger to distinguish
    // "classified as a shim" from "not classified at all".
    #[test]
    fn bmad_skill_harness_support_states_are_all_distinguishable() {
        let states = [
            BmadSkillSupportState::Exact,
            BmadSkillSupportState::Adapted,
            BmadSkillSupportState::Shim,
            BmadSkillSupportState::Unavailable,
        ];
        for (left_index, left) in states.iter().enumerate() {
            for (right_index, right) in states.iter().enumerate() {
                assert_eq!(left_index == right_index, left == right);
            }
        }
    }

    #[test]
    fn bmad_skill_harness_rejects_a_descriptor_with_no_supported_mode() {
        let mut input = minimal_input();
        input.supported_modes = Vec::new();
        assert!(matches!(
            BmadSkillDescriptor::new(input),
            Err(BmadSkillHarnessError::NoSupportedModes)
        ));
    }

    #[test]
    fn bmad_skill_harness_rejects_an_empty_canonical_id() {
        let mut input = minimal_input();
        input.canonical_id = String::new();
        assert!(matches!(
            BmadSkillDescriptor::new(input),
            Err(BmadSkillHarnessError::InvalidId)
        ));
    }

    // Break caught: the harness module reaching for a filesystem, process,
    // network, or model capability -- which would turn a "describe this
    // skill" adapter into something that could act on its own description.
    // Comment lines (including this module's own `//!` doc, which explains
    // the rule in prose) are stripped before scanning so the rule can name
    // its own forbidden tokens without tripping itself.
    #[test]
    fn bmad_skill_harness_module_grants_no_capability() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bmad/skills/harness.rs"),
        )
        .expect("harness.rs source");
        let module_source = source
            .split_once("#[cfg(test)]")
            .map_or(source.as_str(), |(module, _)| module);
        let forbidden = [
            "std::fs",
            "std::process",
            "std::net",
            "tokio::net",
            "reqwest",
            "model_port",
            "ModelPort",
            "chat::",
            "ChatSession",
            "Command::new",
        ];
        for (index, line) in module_source.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            for token in forbidden {
                assert!(
                    !code.contains(token),
                    "harness.rs line {} references a forbidden capability {token:?}: {line:?}",
                    index + 1,
                );
            }
        }
    }
}
