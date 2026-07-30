//! The `bmad-check-implementation-readiness` skill adapter.
//!
//! Without this module, MTH-13 has no typed descriptor to hand the runtime,
//! Help, or the compatibility ledger -- they would each have to re-derive
//! this skill's shape from the pinned `SKILL.md` prose themselves.
//!
//! The pinned source (`packages/bmad-foundation/source/method/6.10.0/src/
//! bmm-skills/3-solutioning/bmad-check-implementation-readiness/SKILL.md`)
//! is a micro-file, step-sequenced workflow: activation resolves a
//! `workflow` customization block, then execution hands off to
//! `steps/step-01-document-discovery.md` through
//! `steps/step-06-final-assessment.md`, each step self-contained and loaded
//! just-in-time. Step 6's own frontmatter fixes the output path:
//! `outputFile: '{planning_artifacts}/implementation-readiness-report-
//! {{date}}.md'`.
//!
//! `declared_actions` stays empty: the source names no fixed,
//! separately-invocable action -- the workflow is a single linear
//! progression through its step files -- matching the catalog row's
//! `action: null`.
//!
//! **Modes gap**: `capability-catalog.json` declares both `interactive` and
//! `headless` for this rowId, but neither the `SKILL.md` nor any of its six
//! step files names a headless path anywhere (unlike `bmad-architecture`,
//! which explicitly forks on "Headless (no interactive user)", or
//! `bmad-prfaq`'s documented `--headless` flag). Every step file instead
//! repeats "HALT — wait for user input" and "NEVER generate content without
//! user input". `supported_modes` still declares both, per the non-negotiable
//! rule that a descriptor "declares its catalog-declared modes" -- but this
//! is reported as a fidelity gap: the harness cannot represent "catalog says
//! headless, but the source text never describes what headless does here."

use crate::generated_contracts::{
    BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationMode,
};
use crate::{
    BmadMethodSkill, BmadResource, BmadSkillDescriptor, BmadSkillDescriptorInput,
    BmadSkillSupportState, Sha256Digest,
};

/// Adapter for the pinned `bmad-check-implementation-readiness` Method skill
/// (MTH-13).
pub struct BmadCheckImplementationReadinessSkill;

impl BmadMethodSkill for BmadCheckImplementationReadinessSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "bmm:bmad-check-implementation-readiness".to_owned(),
            source_path: "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/\
SKILL.md"
                .to_owned(),
            // Matches capability-catalog.json's row for this rowId; see
            // `bmad_method_solutioning_skills.rs`, which binds against the
            // catalog directly rather than trusting this literal alone.
            // `from_bytes` is infallible (a `const fn` over a fixed-size
            // array), so no fallible `parse` + panic-on-error is needed here.
            source_sha256: Sha256Digest::from_bytes([
                0xa7, 0xe1, 0x2e, 0xae, 0x2b, 0xdb, 0x5a, 0x95, 0xcc, 0x05, 0x34, 0xe6, 0xd5, 0xbf,
                0x58, 0xe7, 0xce, 0x4b, 0x94, 0x73, 0x27, 0xa4, 0x94, 0xd4, 0x75, 0xef, 0x98, 0xe6,
                0x7f, 0x03, 0x71, 0x8a,
            ]),
            // The source names no fixed action -- a single linear step-file
            // progression. Catalog row records `action: null`. See module
            // doc.
            declared_actions: Vec::new(),
            supported_modes: vec![
                BmadInvocationMode::Interactive,
                BmadInvocationMode::Headless,
            ],
            // No predecessor/follower is named as a fixed edge in the
            // source text.
            predecessor: None,
            follower: None,
            // Step 4 ("Load Config") explicitly resolves
            // `{planning_artifacts}` "for output location and artifact
            // scanning", and Step 6's frontmatter writes the report there.
            output_locations: vec!["planning_artifacts".to_owned()],
            // Step 6's `outputFile` is a single markdown report document.
            artifact_kinds: vec![BmadArtifactKind::Document],
            // "Save the final report" / "Save state" appears throughout the
            // step files, and Step 6 explicitly saves the completed report.
            completion_hooks: vec![BmadArtifactCompletionHookRunHook::PersistArtifacts],
            // The full recursively owned source directory is immutable
            // package data. Declaring its exact identities lets the generic
            // resolver establish source closure without this adapter reading,
            // interpreting, or granting authority to the pinned prose.
            declared_resources: vec![
                source_resource(
                    "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/SKILL.md",
                    [
                        0xa7, 0xe1, 0x2e, 0xae, 0x2b, 0xdb, 0x5a, 0x95, 0xcc, 0x05, 0x34, 0xe6,
                        0xd5, 0xbf, 0x58, 0xe7, 0xce, 0x4b, 0x94, 0x73, 0x27, 0xa4, 0x94, 0xd4,
                        0x75, 0xef, 0x98, 0xe6, 0x7f, 0x03, 0x71, 0x8a,
                    ],
                ),
                source_resource(
                    "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/customize.toml",
                    [
                        0x57, 0x1d, 0x81, 0x08, 0x7b, 0x7a, 0x6d, 0x91, 0x4c, 0xf7, 0xc2, 0xf2,
                        0x1a, 0xa2, 0x63, 0x54, 0xea, 0xc3, 0x85, 0xbb, 0x2f, 0x5c, 0xfe, 0xab,
                        0xc5, 0x91, 0x89, 0x2c, 0xbb, 0x98, 0xd0, 0x36,
                    ],
                ),
                source_resource(
                    "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/steps/step-01-document-discovery.md",
                    [
                        0xc7, 0x63, 0xb5, 0x12, 0xd5, 0x59, 0x06, 0x12, 0x24, 0x33, 0xcb, 0x65,
                        0xc1, 0xbc, 0xd5, 0xb5, 0xb2, 0x83, 0xe4, 0x5e, 0xac, 0xdc, 0x07, 0x28,
                        0x1c, 0x8e, 0xc7, 0x59, 0x6b, 0x6b, 0x39, 0x80,
                    ],
                ),
                source_resource(
                    "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/steps/step-02-prd-analysis.md",
                    [
                        0x38, 0xbe, 0x2b, 0xf4, 0xb9, 0x24, 0xc0, 0xb5, 0xb3, 0x95, 0xb5, 0x7d,
                        0x68, 0xf6, 0x85, 0xd7, 0x90, 0xad, 0xe7, 0xb1, 0xa6, 0xc1, 0x09, 0x93,
                        0xd3, 0xc5, 0x50, 0x67, 0x5f, 0x87, 0xd9, 0x54,
                    ],
                ),
                source_resource(
                    "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/steps/step-03-epic-coverage-validation.md",
                    [
                        0x7b, 0x18, 0x7f, 0x03, 0xa4, 0x7c, 0xba, 0x03, 0x25, 0xfc, 0xfd, 0x10,
                        0x24, 0x04, 0x10, 0xdb, 0x9c, 0x59, 0xd9, 0x37, 0x68, 0x34, 0x2f, 0xc2,
                        0xdd, 0x3d, 0xe2, 0xa0, 0x1e, 0xc2, 0x33, 0x56,
                    ],
                ),
                source_resource(
                    "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/steps/step-04-ux-alignment.md",
                    [
                        0xf7, 0x1e, 0x5f, 0x0d, 0x77, 0x61, 0x5e, 0x88, 0x5a, 0xe4, 0x0f, 0xde,
                        0xe6, 0xb0, 0x4c, 0x1d, 0xd6, 0xe4, 0x72, 0xc8, 0x71, 0xf8, 0x7b, 0x51,
                        0x5f, 0xe8, 0x69, 0xcb, 0x5f, 0x69, 0x66, 0xfb,
                    ],
                ),
                source_resource(
                    "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/steps/step-05-epic-quality-review.md",
                    [
                        0xd8, 0xa8, 0x4e, 0x57, 0xf4, 0xe3, 0xa3, 0x21, 0x73, 0x4b, 0x5b, 0x5d,
                        0x09, 0x34, 0x58, 0xce, 0xb1, 0xe3, 0x38, 0x74, 0x4f, 0x18, 0x95, 0x4c,
                        0x5a, 0x20, 0x4f, 0x5c, 0xe3, 0x57, 0x61, 0x85,
                    ],
                ),
                source_resource(
                    "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/steps/step-06-final-assessment.md",
                    [
                        0x1b, 0x9c, 0x8c, 0x2a, 0x21, 0x95, 0x0f, 0x85, 0xe0, 0x52, 0xe0, 0x53,
                        0x09, 0xbf, 0xcb, 0x3b, 0xfe, 0x38, 0x15, 0x7b, 0xa6, 0xfc, 0xa4, 0x02,
                        0xcc, 0xf3, 0x74, 0x44, 0xe7, 0xf1, 0xce, 0xe3,
                    ],
                ),
                source_resource(
                    "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/templates/readiness-report-template.md",
                    [
                        0x0d, 0xa9, 0x7a, 0xb1, 0xe3, 0x88, 0x18, 0xe6, 0x42, 0xf3, 0x6d, 0xc0,
                        0xef, 0x24, 0xd2, 0xda, 0xe6, 0x9f, 0xc6, 0xe0, 0xbe, 0x59, 0x92, 0x4d,
                        0xc2, 0xdb, 0xf4, 0x43, 0x29, 0x73, 0x8f, 0xf6,
                    ],
                ),
            ],
            support_state: BmadSkillSupportState::Exact,
        })
        .unwrap_or_else(|_| unreachable!("this adapter's fields satisfy the harness's own bounds"))
    }
}

fn source_resource(path: &str, content_hash: [u8; 32]) -> BmadResource {
    BmadResource::new(path, Sha256Digest::from_bytes(content_hash)).unwrap_or_else(|_| {
        unreachable!("all pinned implementation-readiness resource paths are canonical")
    })
}
