//! The `bmad-advanced-elicitation` skill adapter.
//!
//! The pinned source at `src/core-skills/bmad-advanced-elicitation/SKILL.md`
//! is the method's shared
//! refinement checkpoint: it operates on "the most recent output in the
//! conversation" and, on exit ("x"), "hand[s] back the improved version ...
//! as the replacement for what it had" -- entirely in-conversation. No step
//! names a persisted file, a `{doc_workspace}`, or a memlog; the whole run
//! lives in the calling session's turns. `output_locations`,
//! `artifact_kinds`, and `completion_hooks` therefore stay empty: inventing
//! a persisted artifact this skill never writes would misrepresent it, the
//! same discipline `declared_actions` already follows for an action the
//! source never named.
//!
//! `declared_actions` stays empty: the catalog row for this rowId records
//! `action: null`, and the menu (`1-5`, `r`, `a`, `x`) is served dynamically
//! from `scripts/pick_methods.py`'s catalog rather than a fixed,
//! separately-invocable action [`BmadInvocationDeclaredAction`] could model.
//!
//! [`BmadInvocationDeclaredAction`]: crate::generated_contracts::BmadInvocationDeclaredAction

use crate::generated_contracts::BmadInvocationMode;
use crate::{
    BmadMethodSkill, BmadSkillDescriptor, BmadSkillDescriptorInput, BmadSkillSupportState,
    Sha256Digest,
};

/// Adapter for the pinned `bmad-advanced-elicitation` Method skill.
pub struct BmadAdvancedElicitationSkill;

impl BmadMethodSkill for BmadAdvancedElicitationSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "core:bmad-advanced-elicitation".to_owned(),
            source_path: "src/core-skills/bmad-advanced-elicitation/SKILL.md".to_owned(),
            // Matches capability-catalog.json's row for this rowId; the test
            // suite binds against the catalog directly rather than trusting
            // this literal alone. `from_bytes` is infallible (a `const fn`
            // over a fixed-size array), so no fallible `parse` + panic-on-
            // error is needed here.
            source_sha256: Sha256Digest::from_bytes([
                0x23, 0xa1, 0xf2, 0x90, 0x3f, 0x6a, 0x19, 0x25, 0x5e, 0x3c, 0xea, 0xe7, 0x98, 0xaf,
                0xe4, 0x8f, 0x63, 0x94, 0x69, 0x47, 0x69, 0xb3, 0xb0, 0x34, 0x50, 0x2d, 0x17, 0x4d,
                0x68, 0x36, 0x12, 0xb2,
            ]),
            declared_actions: Vec::new(),
            supported_modes: vec![
                BmadInvocationMode::Interactive,
                BmadInvocationMode::Headless,
            ],
            predecessor: None,
            follower: None,
            // No `{doc_workspace}`, output path, or persisted file is named
            // anywhere in the source -- refinement happens on the in-context
            // target and the improved version is handed back conversationally.
            output_locations: Vec::new(),
            artifact_kinds: Vec::new(),
            completion_hooks: Vec::new(),
            declared_resources: Vec::new(),
            support_state: BmadSkillSupportState::Exact,
        })
        .unwrap_or_else(|_| unreachable!("this adapter's fields satisfy the harness's own bounds"))
    }
}
