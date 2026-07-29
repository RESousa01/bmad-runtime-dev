//! The `bmad-agent-tech-writer` (Paige, Technical Writer) skill adapter.
//!
//! Mirrors `bmad_agent_analyst.rs`'s reasoning: the pinned source
//! (`packages/bmad-foundation/source/method/6.10.0/src/bmm-skills/
//! 1-analysis/bmad-agent-tech-writer/SKILL.md`) is the same persona-adoption
//! shape, differing only in identity and voice. Dispatch is
//! `{agent.menu}`-driven from `customize.toml`, not a fixed action this file
//! can read, so `declared_actions` stays empty -- matching the catalog row's
//! `action: null`.

use crate::generated_contracts::BmadInvocationMode;
use crate::{
    BmadMethodSkill, BmadSkillDescriptor, BmadSkillDescriptorInput, BmadSkillSupportState,
    Sha256Digest,
};

/// Adapter for the pinned `bmad-agent-tech-writer` Method skill (MTH-02).
pub struct BmadAgentTechWriterSkill;

impl BmadMethodSkill for BmadAgentTechWriterSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "bmm:bmad-agent-tech-writer".to_owned(),
            source_path: "src/bmm-skills/1-analysis/bmad-agent-tech-writer/SKILL.md".to_owned(),
            // Matches capability-catalog.json's row for this rowId; see
            // `bmad_method_analysis_skills.rs`, which binds against the
            // catalog directly rather than trusting this literal alone.
            // `from_bytes` is infallible (a `const fn` over a fixed-size
            // array), so no fallible `parse` + panic-on-error is needed here.
            source_sha256: Sha256Digest::from_bytes([
                0x75, 0x44, 0x24, 0x2f, 0xf6, 0x8d, 0x0d, 0xdf, 0xd4, 0x66, 0x27, 0x4f, 0x40, 0xae,
                0x9f, 0x3b, 0x0e, 0xa0, 0x6e, 0xe5, 0xf5, 0xf9, 0x38, 0xf6, 0x8f, 0x95, 0x3e, 0x19,
                0xb3, 0x59, 0x8c, 0xd7,
            ]),
            declared_actions: Vec::new(),
            supported_modes: vec![
                BmadInvocationMode::Interactive,
                BmadInvocationMode::Headless,
            ],
            predecessor: None,
            follower: None,
            // Step 5 ("Load Config") explicitly resolves `{planning_artifacts}`
            // "for output location and artifact scanning".
            output_locations: vec!["planning_artifacts".to_owned()],
            artifact_kinds: Vec::new(),
            completion_hooks: Vec::new(),
            declared_resources: Vec::new(),
            support_state: BmadSkillSupportState::Exact,
        })
        .unwrap_or_else(|_| unreachable!("this adapter's fields satisfy the harness's own bounds"))
    }
}
