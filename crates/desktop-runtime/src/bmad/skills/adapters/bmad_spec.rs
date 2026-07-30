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
    BmadMethodSkill, BmadResource, BmadSkillDescriptor, BmadSkillDescriptorInput,
    BmadSkillSupportState, Sha256Digest,
};

/// Adapter for the pinned `bmad-spec` Method skill (MTH-09).
pub struct BmadSpecSkill;

fn pinned_resource(path: &str, content_hash: [u8; 32]) -> BmadResource {
    BmadResource::new(path.to_owned(), Sha256Digest::from_bytes(content_hash))
        .unwrap_or_else(|_| unreachable!("the pinned bmad-spec path is a valid package resource"))
}

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
            // The source subtree is its complete, recursively-owned closure:
            // the activation/customization surface and all three assets are
            // data supplied by the host through BmadResource resolution.  The
            // adapter only declares their immutable identities; it cannot read
            // or execute any of them itself.
            declared_resources: vec![
                pinned_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-spec/SKILL.md",
                    [
                        0x5b, 0x56, 0x80, 0xdb, 0xf0, 0x1d, 0xbf, 0xc5, 0xb4, 0xa5, 0x48, 0x5e,
                        0x70, 0xf2, 0x0e, 0xa4, 0x8a, 0xc2, 0x1a, 0x1d, 0x61, 0x37, 0xc4, 0xea,
                        0x2f, 0xf7, 0x04, 0x26, 0x2d, 0xcc, 0x45, 0xd2,
                    ],
                ),
                pinned_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-spec/assets/headless-schemas.md",
                    [
                        0x83, 0xaf, 0xb3, 0x61, 0x47, 0x16, 0xdb, 0x23, 0x47, 0xd8, 0x35, 0x86,
                        0xa6, 0x7b, 0x9c, 0x52, 0x44, 0x92, 0xdd, 0x50, 0xdb, 0x52, 0x82, 0x43,
                        0x6c, 0xcb, 0x35, 0xc9, 0x9f, 0xb5, 0xf0, 0x8a,
                    ],
                ),
                pinned_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-spec/assets/spec-template.md",
                    [
                        0x4b, 0x3a, 0x03, 0x2c, 0x04, 0x5f, 0xdd, 0x09, 0x03, 0x93, 0x34, 0x46,
                        0xfc, 0x07, 0xf9, 0x3e, 0x87, 0x99, 0x97, 0xf2, 0x8a, 0x02, 0x94, 0xed,
                        0x43, 0x44, 0xda, 0x71, 0x19, 0x2b, 0x20, 0xee,
                    ],
                ),
                pinned_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-spec/assets/stories-schema.md",
                    [
                        0x4f, 0x81, 0x37, 0xc0, 0x0e, 0x27, 0x10, 0x28, 0x83, 0x54, 0xa4, 0x46,
                        0x6d, 0x1c, 0xfb, 0x90, 0x70, 0x4f, 0x4b, 0x2a, 0x04, 0xbd, 0x3b, 0x63,
                        0x92, 0x42, 0x18, 0xfd, 0x58, 0x9c, 0xc6, 0xa9,
                    ],
                ),
                pinned_resource(
                    "src/bmm-skills/2-plan-workflows/bmad-spec/customize.toml",
                    [
                        0xa8, 0x9e, 0x0f, 0x53, 0xf9, 0xb6, 0x38, 0x6c, 0x28, 0x1d, 0xd0, 0x15,
                        0x8a, 0x21, 0x80, 0x1a, 0x6b, 0x16, 0xf1, 0x22, 0x08, 0x5f, 0xd6, 0x8b,
                        0x02, 0x20, 0xcd, 0x40, 0x3c, 0x18, 0xf8, 0x30,
                    ],
                ),
            ],
            support_state: BmadSkillSupportState::Exact,
        })
        .unwrap_or_else(|_| unreachable!("this adapter's fields satisfy the harness's own bounds"))
    }
}
