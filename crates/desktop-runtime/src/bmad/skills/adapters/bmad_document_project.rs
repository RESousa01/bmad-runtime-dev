//! The `bmad-document-project` skill adapter.
//!
//! The pinned source (`packages/bmad-foundation/source/method/6.10.0/src/
//! bmm-skills/1-analysis/bmad-document-project/SKILL.md`) is a thin
//! activation shell: it resolves a `workflow` block, then defers its entire
//! execution to `./instructions.md` ("Read fully and follow: `./instructions.md`").
//! `SKILL.md` itself names no fixed action and no artifact kind -- both
//! live in `instructions.md`, which is outside this adapter's declared
//! source (only the pinned `SKILL.md` is parsed here), so both are left
//! empty rather than guessed. This is reported as a gap: see the parent
//! task's "what couldn't be represented" note.

use crate::generated_contracts::BmadInvocationMode;
use crate::{
    BmadMethodSkill, BmadSkillDescriptor, BmadSkillDescriptorInput, BmadSkillSupportState,
    Sha256Digest,
};

/// Adapter for the pinned `bmad-document-project` Method skill (MTH-03).
pub struct BmadDocumentProjectSkill;

impl BmadMethodSkill for BmadDocumentProjectSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "bmm:bmad-document-project".to_owned(),
            source_path: "src/bmm-skills/1-analysis/bmad-document-project/SKILL.md".to_owned(),
            // Matches capability-catalog.json's row for this rowId; see
            // `bmad_method_analysis_skills.rs`, which binds against the
            // catalog directly rather than trusting this literal alone.
            // `from_bytes` is infallible (a `const fn` over a fixed-size
            // array), so no fallible `parse` + panic-on-error is needed here.
            source_sha256: Sha256Digest::from_bytes([
                0x80, 0x6d, 0x73, 0xf6, 0x6a, 0x8f, 0x3f, 0x3e, 0x7b, 0x3c, 0xbd, 0x3b, 0x9a, 0x87,
                0xad, 0x4d, 0x8b, 0x9a, 0xab, 0xd3, 0xff, 0x91, 0xf3, 0x7c, 0x96, 0xed, 0xef, 0x36,
                0xa1, 0x7a, 0xa2, 0xb3,
            ]),
            declared_actions: Vec::new(),
            supported_modes: vec![
                BmadInvocationMode::Interactive,
                BmadInvocationMode::Headless,
            ],
            predecessor: None,
            follower: None,
            // Step 4 ("Load Config") explicitly resolves `{planning_artifacts}`
            // "for output location and artifact scanning".
            output_locations: vec!["planning_artifacts".to_owned()],
            // Artifact kind and completion hook live in instructions.md, not
            // in the pinned SKILL.md this adapter parses. See module doc.
            artifact_kinds: Vec::new(),
            completion_hooks: Vec::new(),
            declared_resources: Vec::new(),
            support_state: BmadSkillSupportState::Exact,
        })
        .unwrap_or_else(|_| unreachable!("this adapter's fields satisfy the harness's own bounds"))
    }
}
