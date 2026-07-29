//! The `bmad-spec` (SPEC kernel distiller) skill adapter.
//!
//! The pinned source (`packages/bmad-foundation/source/method/6.10.0/src/
//! bmm-skills/2-plan-workflows/bmad-spec/SKILL.md`) distills any intent
//! input into one canonical working document, `SPEC.md`, plus optional
//! companion files: "produces **SPEC.md** carrying the five-field kernel ...
//! plus companion files for load-bearing content". `SPEC.md` is
//! `BmadArtifactKind::Document`, and it is re-derived and written on every
//! create or update run ("`SPEC.md` and every spec-authored companion are
//! **derived on each run**"), matching
//! `BmadArtifactCompletionHookRunHook::PersistArtifacts`.
//!
//! `declared_actions` stays empty, matching the catalog row's `action:
//! null`: the source names no fixed action -- it detects create-vs-update by
//! whether a prior `.memlog.md` exists at the target folder, not by a
//! caller-selected action.
//!
//! Unlike `bmad-prd` and `bmad-ux`, this skill's own activation step 3
//! resolves `{output_folder}` (under the merged `core` config), not
//! `{planning_artifacts}`: "resolve `{user_name}`, ..., `{output_folder}`
//! (under `core`), and `{date}`", and the spec folder itself defaults to
//! `{output_folder}/specs/spec-{slug}/`. `output_locations` reflects that
//! divergence rather than reusing the sibling skills' `planning_artifacts`
//! literal.
//!
//! `predecessor`/`follower` stay `None`: the source explicitly describes
//! itself as callable from many directions ("Multiple skills may call to
//! update the same spec over time") with no single fixed upstream or
//! downstream skill named.

use crate::generated_contracts::{
    BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationMode,
};
use crate::{
    BmadMethodSkill, BmadSkillDescriptor, BmadSkillDescriptorInput, BmadSkillSupportState,
    Sha256Digest,
};

/// Adapter for the pinned `bmad-spec` Method skill (MTH-09).
pub struct BmadSpecSkill;

impl BmadMethodSkill for BmadSpecSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "bmm:bmad-spec".to_owned(),
            source_path: "src/bmm-skills/2-plan-workflows/bmad-spec/SKILL.md".to_owned(),
            // Matches capability-catalog.json's row for this rowId; see
            // `bmad_method_plan_skills.rs`, which binds against the catalog
            // directly rather than trusting this literal alone. `from_bytes`
            // is infallible (a `const fn` over a fixed-size array), so no
            // fallible `parse` + panic-on-error is needed here.
            source_sha256: Sha256Digest::from_bytes([
                0x5b, 0x56, 0x80, 0xdb, 0xf0, 0x1d, 0xbf, 0xc5, 0xb4, 0xa5, 0x48, 0x5e, 0x70, 0xf2,
                0x0e, 0xa4, 0x8a, 0xc2, 0x1a, 0x1d, 0x61, 0x37, 0xc4, 0xea, 0x2f, 0xf7, 0x04, 0x26,
                0x2d, 0xcc, 0x45, 0xd2,
            ]),
            declared_actions: Vec::new(),
            supported_modes: vec![
                BmadInvocationMode::Interactive,
                BmadInvocationMode::Headless,
            ],
            predecessor: None,
            follower: None,
            // Step 3 resolves `{output_folder}` (under `core`), the field
            // the spec workspace is actually bound under -- see module doc.
            output_locations: vec!["output_folder".to_owned()],
            // "produces **SPEC.md**" -- the canonical working artifact is a
            // single markdown document; companions are optional overflow of
            // the same kind, not a distinct declared kind.
            artifact_kinds: vec![BmadArtifactKind::Document],
            completion_hooks: vec![BmadArtifactCompletionHookRunHook::PersistArtifacts],
            declared_resources: Vec::new(),
            support_state: BmadSkillSupportState::Exact,
        })
        .unwrap_or_else(|_| unreachable!("this adapter's fields satisfy the harness's own bounds"))
    }
}
