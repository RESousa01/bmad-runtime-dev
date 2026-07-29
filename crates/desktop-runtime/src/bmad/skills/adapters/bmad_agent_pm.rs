//! The `bmad-agent-pm` (John, Product Manager) skill adapter.
//!
//! The pinned source (`packages/bmad-foundation/source/method/6.10.0/src/
//! bmm-skills/2-plan-workflows/bmad-agent-pm/SKILL.md`) is a persona-adoption
//! agent, structurally identical to the `1-analysis` agent skills: it
//! resolves a customization-layered `agent` block, greets the user, and
//! dispatches on `{agent.menu}` -- a table resolved at runtime from
//! `customize.toml` and its overlays, not a fixed list this file can read.
//! `declared_actions` stays empty here: the source names no fixed action,
//! matching the catalog row's `action: null`. Inventing a menu item name
//! (e.g. "write-prd") would assert a fixed action the source does not
//! actually fix.

use crate::generated_contracts::BmadInvocationMode;
use crate::{
    BmadMethodSkill, BmadSkillDescriptor, BmadSkillDescriptorInput, BmadSkillSupportState,
    Sha256Digest,
};

/// Adapter for the pinned `bmad-agent-pm` Method skill (MTH-06).
pub struct BmadAgentPmSkill;

impl BmadMethodSkill for BmadAgentPmSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "bmm:bmad-agent-pm".to_owned(),
            source_path: "src/bmm-skills/2-plan-workflows/bmad-agent-pm/SKILL.md".to_owned(),
            // Matches capability-catalog.json's row for this rowId; see
            // `bmad_method_plan_skills.rs`, which binds against the catalog
            // directly rather than trusting this literal alone. `from_bytes`
            // is infallible (a `const fn` over a fixed-size array), so no
            // fallible `parse` + panic-on-error is needed here.
            source_sha256: Sha256Digest::from_bytes([
                0xe2, 0xeb, 0xea, 0x0b, 0xf6, 0xee, 0x15, 0x8f, 0xcb, 0x9e, 0x62, 0x73, 0x1a, 0x18,
                0x59, 0x83, 0xf2, 0xac, 0x59, 0xf6, 0x64, 0x94, 0x0f, 0x4f, 0x43, 0x20, 0xcf, 0x10,
                0xe1, 0xbb, 0xdf, 0x92,
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
            declared_resources: Vec::new(),
            support_state: BmadSkillSupportState::Exact,
        })
        .unwrap_or_else(|_| unreachable!("this adapter's fields satisfy the harness's own bounds"))
    }
}
