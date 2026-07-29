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
    BmadMethodSkill, BmadSkillDescriptor, BmadSkillDescriptorInput, BmadSkillSupportState,
    Sha256Digest,
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
            // See module doc: resource closure is out of this adapter's
            // scope for now (tracked as a reported gap, not invented data).
            declared_resources: Vec::new(),
            support_state: BmadSkillSupportState::Exact,
        })
        .unwrap_or_else(|_| unreachable!("this adapter's fields satisfy the harness's own bounds"))
    }
}
