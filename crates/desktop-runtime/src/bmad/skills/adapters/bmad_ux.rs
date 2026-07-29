//! The `bmad-ux` (DESIGN.md / EXPERIENCE.md UX spines) skill adapter.
//!
//! The pinned source (`packages/bmad-foundation/source/method/6.10.0/src/
//! bmm-skills/2-plan-workflows/bmad-ux/SKILL.md`) produces two peer
//! documents: "Produce two peer contracts: **`DESIGN.md`** ... and
//! **`EXPERIENCE.md`**". Both are markdown documents persisted as the run
//! proceeds (Finalize: "Set both files' `status: final`"), so
//! `artifact_kinds` carries one `BmadArtifactKind::Document` entry -- the
//! field records the *kind* of artifact produced, and both files share that
//! one kind, matching `bmad-prd`'s and `bmad-spec`'s single-`Document`
//! declarations rather than double-counting per file.
//! `BmadArtifactCompletionHookRunHook::PersistArtifacts` follows for the
//! same reason those two skills declare it.
//!
//! `declared_actions` stays empty, matching the catalog row's `action:
//! null`: activation step 5 ("Detect intent: **Create**, **Update**,
//! **Validate**") is conversational intent detection at one entry point,
//! the same representational limit `bmad-prd`'s adapter documents for its
//! own Create/Update/Validate modes.
//!
//! `predecessor`/`follower` stay `None`: the source explicitly states "UX
//! may lead, follow, or stand alone" and Finalize names three legitimate
//! next skills side by side ("Common next: `bmad-architecture`,
//! `bmad-create-epics-and-stories`, `bmad-dev-story`") -- no single fixed
//! predecessor or follower, the same non-fixed-ordering shape `bmad-prd`'s
//! adapter documents.

use crate::generated_contracts::{
    BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationMode,
};
use crate::{
    BmadMethodSkill, BmadSkillDescriptor, BmadSkillDescriptorInput, BmadSkillSupportState,
    Sha256Digest,
};

/// Adapter for the pinned `bmad-ux` Method skill (MTH-10).
pub struct BmadUxSkill;

impl BmadMethodSkill for BmadUxSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "bmm:bmad-ux".to_owned(),
            source_path: "src/bmm-skills/2-plan-workflows/bmad-ux/SKILL.md".to_owned(),
            // Matches capability-catalog.json's row for this rowId; see
            // `bmad_method_plan_skills.rs`, which binds against the catalog
            // directly rather than trusting this literal alone. `from_bytes`
            // is infallible (a `const fn` over a fixed-size array), so no
            // fallible `parse` + panic-on-error is needed here.
            source_sha256: Sha256Digest::from_bytes([
                0x25, 0x0d, 0x27, 0x94, 0xf2, 0xb8, 0xe3, 0x16, 0xbb, 0x1b, 0xa4, 0x66, 0x6f, 0x07,
                0x28, 0x97, 0x0a, 0xfd, 0x8a, 0x5d, 0x27, 0x07, 0xac, 0x1c, 0xce, 0xac, 0x6c, 0x21,
                0xe6, 0x37, 0x30, 0xb6,
            ]),
            declared_actions: Vec::new(),
            supported_modes: vec![
                BmadInvocationMode::Interactive,
                BmadInvocationMode::Headless,
            ],
            predecessor: None,
            follower: None,
            // Step 3 ("Load config") explicitly resolves `{planning_artifacts}`.
            output_locations: vec!["planning_artifacts".to_owned()],
            // Both DESIGN.md and EXPERIENCE.md are markdown documents; see
            // module doc for why this stays a single `Document` entry.
            artifact_kinds: vec![BmadArtifactKind::Document],
            completion_hooks: vec![BmadArtifactCompletionHookRunHook::PersistArtifacts],
            declared_resources: Vec::new(),
            support_state: BmadSkillSupportState::Exact,
        })
        .unwrap_or_else(|_| unreachable!("this adapter's fields satisfy the harness's own bounds"))
    }
}
