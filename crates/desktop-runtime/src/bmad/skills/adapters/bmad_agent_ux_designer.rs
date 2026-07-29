//! The `bmad-agent-ux-designer` (Sally, UX Designer) skill adapter.
//!
//! The pinned source (`packages/bmad-foundation/source/method/6.10.0/src/
//! bmm-skills/2-plan-workflows/bmad-agent-ux-designer/SKILL.md`) is a
//! persona-adoption agent, byte-for-byte structurally identical to
//! `bmad-agent-pm` and the `1-analysis` agent skills apart from the persona
//! name: it resolves a customization-layered `agent` block, greets the
//! user, and dispatches on `{agent.menu}` -- a table resolved at runtime
//! from `customize.toml` and its overlays, not a fixed list this file can
//! read. `declared_actions` stays empty here: the source names no fixed
//! action, matching the catalog row's `action: null`.

use crate::generated_contracts::BmadInvocationMode;
use crate::{
    BmadMethodSkill, BmadSkillDescriptor, BmadSkillDescriptorInput, BmadSkillSupportState,
    Sha256Digest,
};

/// Adapter for the pinned `bmad-agent-ux-designer` Method skill (MTH-07).
pub struct BmadAgentUxDesignerSkill;

impl BmadMethodSkill for BmadAgentUxDesignerSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "bmm:bmad-agent-ux-designer".to_owned(),
            source_path: "src/bmm-skills/2-plan-workflows/bmad-agent-ux-designer/SKILL.md"
                .to_owned(),
            // Matches capability-catalog.json's row for this rowId; see
            // `bmad_method_plan_skills.rs`, which binds against the catalog
            // directly rather than trusting this literal alone. `from_bytes`
            // is infallible (a `const fn` over a fixed-size array), so no
            // fallible `parse` + panic-on-error is needed here.
            source_sha256: Sha256Digest::from_bytes([
                0xd7, 0x80, 0xae, 0x59, 0xb9, 0xfb, 0x7e, 0x3a, 0x94, 0xc5, 0xff, 0xe4, 0xec, 0x75,
                0xf9, 0x09, 0x42, 0xd4, 0xa9, 0xa3, 0x94, 0x4a, 0xf4, 0xb3, 0x6c, 0x2d, 0x27, 0x9f,
                0xdd, 0x78, 0x01, 0xdb,
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
