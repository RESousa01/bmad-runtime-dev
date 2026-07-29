//! The `bmad-prfaq` (Working Backwards PRFAQ challenge) skill adapter.
//!
//! Unlike the two agent-persona skills, the pinned source
//! (`packages/bmad-foundation/source/method/6.10.0/src/bmm-skills/
//! 1-analysis/bmad-prfaq/SKILL.md`) explicitly names one produced artifact:
//! "Create the output document at `{planning_artifacts}/prfaq-{project_name}.md`
//! ... This document is the working artifact". That single named document is
//! `BmadArtifactKind::Document`, persisted as it is written -- matching
//! `BmadArtifactCompletionHookRunHook::PersistArtifacts`. The source names no
//! fixed action (only a `--headless`/`-H` mode flag, already captured by
//! `supported_modes`), so `declared_actions` stays empty, matching the
//! catalog row's `action: null`.

use crate::generated_contracts::{
    BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationMode,
};
use crate::{
    BmadMethodSkill, BmadSkillDescriptor, BmadSkillDescriptorInput, BmadSkillSupportState,
    Sha256Digest,
};

/// Adapter for the pinned `bmad-prfaq` Method skill (MTH-04).
pub struct BmadPrfaqSkill;

impl BmadMethodSkill for BmadPrfaqSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "bmm:bmad-prfaq".to_owned(),
            source_path: "src/bmm-skills/1-analysis/bmad-prfaq/SKILL.md".to_owned(),
            // Matches capability-catalog.json's row for this rowId; see
            // `bmad_method_analysis_skills.rs`, which binds against the
            // catalog directly rather than trusting this literal alone.
            // `from_bytes` is infallible (a `const fn` over a fixed-size
            // array), so no fallible `parse` + panic-on-error is needed here.
            source_sha256: Sha256Digest::from_bytes([
                0x35, 0x3d, 0xa1, 0xcc, 0x88, 0x37, 0xe8, 0x80, 0xc7, 0xf4, 0x1b, 0x63, 0x6a, 0x1b,
                0xd3, 0x80, 0x86, 0xc5, 0x6a, 0x91, 0xd5, 0x97, 0x98, 0xb1, 0x3f, 0x5d, 0x61, 0x68,
                0xf0, 0x02, 0x1d, 0xe4,
            ]),
            declared_actions: Vec::new(),
            supported_modes: vec![
                BmadInvocationMode::Interactive,
                BmadInvocationMode::Headless,
            ],
            predecessor: None,
            follower: None,
            // Step 4 ("Load Config") explicitly resolves `{planning_artifacts}`
            // "for output location and artifact scanning", and the document
            // is written under that same location.
            output_locations: vec!["planning_artifacts".to_owned()],
            // "Output: A complete PRFAQ document ..."; the working artifact
            // is a single markdown document at a `{planning_artifacts}` path.
            artifact_kinds: vec![BmadArtifactKind::Document],
            completion_hooks: vec![BmadArtifactCompletionHookRunHook::PersistArtifacts],
            declared_resources: Vec::new(),
            support_state: BmadSkillSupportState::Exact,
        })
        .unwrap_or_else(|_| unreachable!("this adapter's fields satisfy the harness's own bounds"))
    }
}
