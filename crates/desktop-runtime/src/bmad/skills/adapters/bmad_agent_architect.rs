//! The `bmad-agent-architect` (Winston, System Architect) skill adapter.
//!
//! Without this module, MTH-11 has no typed descriptor to hand the runtime,
//! Help, or the compatibility ledger -- they would each have to re-derive
//! this skill's shape from the pinned `SKILL.md` prose themselves.
//!
//! The pinned source (`packages/bmad-foundation/source/method/6.10.0/src/
//! bmm-skills/3-solutioning/bmad-agent-architect/SKILL.md`) is a
//! persona-adoption agent, structurally identical to `bmad-agent-analyst`
//! (MTH-01): it resolves a customization-layered `agent` block, greets the
//! user as Winston, and dispatches on `{agent.menu}` -- a table resolved at
//! runtime from `customize.toml` and its overlays, not a fixed list this
//! file can read. That is why `declared_actions` is empty here: the source
//! names no fixed action, matching the catalog row's `action: null`.
//! Inventing a menu item name would assert a fixed action the source does
//! not actually fix.

use crate::generated_contracts::BmadInvocationMode;
use crate::{
    BmadMethodSkill, BmadSkillDescriptor, BmadSkillDescriptorInput, BmadSkillSupportState,
    Sha256Digest,
};

/// Adapter for the pinned `bmad-agent-architect` Method skill (MTH-11).
pub struct BmadAgentArchitectSkill;

impl BmadMethodSkill for BmadAgentArchitectSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "bmm:bmad-agent-architect".to_owned(),
            source_path: "src/bmm-skills/3-solutioning/bmad-agent-architect/SKILL.md".to_owned(),
            // Matches capability-catalog.json's row for this rowId; see
            // `bmad_method_solutioning_skills.rs`, which binds against the
            // catalog directly rather than trusting this literal alone.
            // `from_bytes` is infallible (a `const fn` over a fixed-size
            // array), so no fallible `parse` + panic-on-error is needed here.
            source_sha256: Sha256Digest::from_bytes([
                0x82, 0xbb, 0xf2, 0x2f, 0xf3, 0xa4, 0x57, 0x17, 0x41, 0xc9, 0x33, 0x9a, 0xe6, 0xb7,
                0xc3, 0x56, 0x76, 0xa6, 0xe2, 0x6a, 0x32, 0x37, 0xce, 0xca, 0x48, 0x7a, 0xbe, 0x85,
                0x36, 0x9e, 0x91, 0xa0,
            ]),
            // The source names no fixed action; dispatch is entirely
            // customize.toml-driven menu resolution. See module doc.
            declared_actions: Vec::new(),
            supported_modes: vec![
                BmadInvocationMode::Interactive,
                BmadInvocationMode::Headless,
            ],
            // No predecessor/follower is named in the source text.
            predecessor: None,
            follower: None,
            // Step 5 ("Load Config") explicitly resolves `{planning_artifacts}`
            // "for output location and artifact scanning".
            output_locations: vec!["planning_artifacts".to_owned()],
            // The skill itself produces no artifact -- it adopts the Winston
            // persona and dispatches to whatever menu item the user picks.
            artifact_kinds: Vec::new(),
            completion_hooks: Vec::new(),
            // See module doc: resource closure is out of this adapter's
            // scope for now (tracked as a reported gap, not invented data).
            declared_resources: Vec::new(),
            support_state: BmadSkillSupportState::Exact,
        })
        .unwrap_or_else(|_| unreachable!("this adapter's fields satisfy the harness's own bounds"))
    }
}
