//! The `bmad-agent-analyst` (Mary, Business Analyst) skill adapter.
//!
//! Without this module, MTH-01 has no typed descriptor to hand the runtime,
//! Help, or the compatibility ledger -- they would each have to re-derive
//! this skill's shape from the pinned `SKILL.md` prose themselves.
//!
//! The pinned source (`packages/bmad-foundation/source/method/6.10.0/src/
//! bmm-skills/1-analysis/bmad-agent-analyst/SKILL.md`) is a persona-adoption
//! agent: it resolves a customization-layered `agent` block, greets the
//! user, and dispatches on `{agent.menu}` -- a table resolved at runtime from
//! `customize.toml` and its overlays, not a fixed list this file can read.
//! That is why `declared_actions` is empty here: the source names no fixed
//! action, matching the catalog row's `action: null`. Inventing a menu item
//! name (e.g. "brainstorm") would assert a fixed action the source does not
//! actually fix.

use crate::generated_contracts::BmadInvocationMode;
use crate::{
    BmadMethodSkill, BmadSkillDescriptor, BmadSkillDescriptorInput, BmadSkillSupportState,
    Sha256Digest,
};

/// Adapter for the pinned `bmad-agent-analyst` Method skill (MTH-01).
pub struct BmadAgentAnalystSkill;

impl BmadMethodSkill for BmadAgentAnalystSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "bmm:bmad-agent-analyst".to_owned(),
            source_path: "src/bmm-skills/1-analysis/bmad-agent-analyst/SKILL.md".to_owned(),
            // Matches capability-catalog.json's row for this rowId; see
            // `bmad_method_analysis_skills.rs`, which binds against the
            // catalog directly rather than trusting this literal alone.
            // `from_bytes` is infallible (a `const fn` over a fixed-size
            // array), so no fallible `parse` + panic-on-error is needed here.
            source_sha256: Sha256Digest::from_bytes([
                0xfc, 0xa3, 0x8a, 0x40, 0x4e, 0x79, 0x50, 0x8e, 0xbc, 0x39, 0x23, 0x5d, 0x01, 0x62,
                0xbc, 0xb4, 0x75, 0xb0, 0x24, 0x07, 0xa9, 0x13, 0xe3, 0xe5, 0xbd, 0x8c, 0x55, 0x26,
                0xb2, 0xb2, 0x61, 0xa6,
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
            // The skill itself produces no artifact -- it adopts a persona
            // and dispatches to whatever menu item the user picks.
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
