//! The `bmad-prd` (PRD create/update/validate) skill adapter.
//!
//! The pinned source (`packages/bmad-foundation/source/method/6.10.0/src/
//! bmm-skills/2-plan-workflows/bmad-prd/SKILL.md`) is a workflow skill that
//! detects intent -- **Create**, **Update**, or **Validate** -- at
//! activation step 5 ("Detect intent") and drives a single working document,
//! `prd.md`, through Discovery and Finalize: "Bind `{doc_workspace}` ... .
//! Write `prd.md` with YAML frontmatter". That document is
//! `BmadArtifactKind::Document`, persisted as it is written (Finalize step 7
//! sets `status: final` in place), matching
//! `BmadArtifactCompletionHookRunHook::PersistArtifacts`.
//!
//! `declared_actions` stays empty even though Create/Update/Validate are
//! named, matching the catalog row's `action: null`: intent is detected
//! conversationally within one entry point (or forwarded verbatim by a
//! caller, per "Forwarded activation"), not selected as a fixed,
//! separately-invocable action the way [`BmadInvocationDeclaredAction`]
//! models one -- the same representational limit `bmad-product-brief`'s
//! adapter already documents for its own Create/Update/Validate modes.
//!
//! `follower` stays `None`: Finalize step 7 names three legitimate next
//! skills side by side ("Common next: `bmad-ux`, `bmad-architecture`,
//! `bmad-create-epics-and-stories`") plus a router (`bmad-help`), not one
//! fixed follower -- asserting any single one of the three would misrepresent
//! the source's own "any of these" framing. This is reported as a gap: see
//! the parent task's "what couldn't be represented" note.
//!
//! [`BmadInvocationDeclaredAction`]: crate::generated_contracts::BmadInvocationDeclaredAction

use crate::generated_contracts::{
    BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationMode,
};
use crate::{
    BmadMethodSkill, BmadSkillDescriptor, BmadSkillDescriptorInput, BmadSkillSupportState,
    Sha256Digest,
};

/// Adapter for the pinned `bmad-prd` Method skill (MTH-08).
pub struct BmadPrdSkill;

impl BmadMethodSkill for BmadPrdSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "bmm:bmad-prd".to_owned(),
            source_path: "src/bmm-skills/2-plan-workflows/bmad-prd/SKILL.md".to_owned(),
            // Matches capability-catalog.json's row for this rowId; see
            // `bmad_method_plan_skills.rs`, which binds against the catalog
            // directly rather than trusting this literal alone. `from_bytes`
            // is infallible (a `const fn` over a fixed-size array), so no
            // fallible `parse` + panic-on-error is needed here.
            source_sha256: Sha256Digest::from_bytes([
                0x5d, 0x7b, 0x47, 0x19, 0xa1, 0xfa, 0x9e, 0x4f, 0x7d, 0x4c, 0xbe, 0xeb, 0x4a, 0x41,
                0x2c, 0x79, 0xe1, 0x41, 0xbc, 0x77, 0xe8, 0xe2, 0x5b, 0xa4, 0x42, 0xb8, 0x5e, 0x1c,
                0x5c, 0x58, 0x7f, 0x3f,
            ]),
            declared_actions: Vec::new(),
            supported_modes: vec![
                BmadInvocationMode::Interactive,
                BmadInvocationMode::Headless,
            ],
            // No single predecessor is named: the only upstream reference is
            // a misroute suggestion ("one-pager → `bmad-product-brief`"),
            // which redirects away from bmad-prd rather than naming what
            // normally precedes it.
            predecessor: None,
            follower: None,
            // Step 3 ("Load config") explicitly resolves `{planning_artifacts}`.
            output_locations: vec!["planning_artifacts".to_owned()],
            // "Write `prd.md` with YAML frontmatter" -- the working artifact
            // is a single markdown document at a `{doc_workspace}` path.
            artifact_kinds: vec![BmadArtifactKind::Document],
            completion_hooks: vec![BmadArtifactCompletionHookRunHook::PersistArtifacts],
            declared_resources: Vec::new(),
            support_state: BmadSkillSupportState::Exact,
        })
        .unwrap_or_else(|_| unreachable!("this adapter's fields satisfy the harness's own bounds"))
    }
}
