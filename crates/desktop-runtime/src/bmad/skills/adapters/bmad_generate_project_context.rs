//! The `bmad-generate-project-context` skill adapter.
//!
//! Without this module, MTH-15 has no typed descriptor to hand the runtime,
//! Help, or the compatibility ledger -- they would each have to re-derive
//! this skill's shape from the pinned `SKILL.md` prose themselves.
//!
//! The pinned source (`packages/bmad-foundation/source/method/6.10.0/src/
//! bmm-skills/3-solutioning/bmad-generate-project-context/SKILL.md`) is a
//! micro-file, step-sequenced workflow that produces `project-context.md`:
//! activation resolves a `workflow` customization block, then execution
//! hands off to `steps/step-01-discover.md` through `steps/step-03-complete.md`.
//! The `## Paths` section fixes the output file explicitly:
//! `output_file = {output_folder}/project-context.md`.
//!
//! `declared_actions` stays empty: the source names no fixed,
//! separately-invocable action -- a single linear step-file progression --
//! matching the catalog row's `action: null`.
//!
//! **Modes gap**: `capability-catalog.json` declares both `interactive` and
//! `headless` for this rowId, but neither the `SKILL.md` nor any of its
//! three step files names a headless path anywhere. Same gap
//! `bmad-check-implementation-readiness` (MTH-13) and
//! `bmad-create-epics-and-stories` (MTH-14) report: `supported_modes` still
//! declares both, per the non-negotiable rule that a descriptor "declares
//! its catalog-declared modes", but this is a fidelity gap the harness
//! cannot otherwise represent.
//!
//! **Output location gap**: the source's `## Paths` section resolves
//! `output_file` to `{output_folder}/project-context.md`, not
//! `{planning_artifacts}` -- a different config variable than every other
//! skill in this batch. Activation Step 4 still resolves
//! `{planning_artifacts}` generically "for output location and artifact
//! scanning" alongside `{project_knowledge}`, so `output_locations` records
//! `planning_artifacts` for consistency with what Step 4 actually names, but
//! the concrete write target (`{output_folder}`) is a distinct, undeclared
//! variable this descriptor's single `output_locations` field cannot also
//! carry without inventing a second field the harness does not have.

use crate::generated_contracts::{
    BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationMode,
};
use crate::{
    BmadMethodSkill, BmadSkillDescriptor, BmadSkillDescriptorInput, BmadSkillSupportState,
    Sha256Digest,
};

/// Adapter for the pinned `bmad-generate-project-context` Method skill
/// (MTH-15).
pub struct BmadGenerateProjectContextSkill;

impl BmadMethodSkill for BmadGenerateProjectContextSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "bmm:bmad-generate-project-context".to_owned(),
            source_path: "src/bmm-skills/3-solutioning/bmad-generate-project-context/SKILL.md"
                .to_owned(),
            // Matches capability-catalog.json's row for this rowId; see
            // `bmad_method_solutioning_skills.rs`, which binds against the
            // catalog directly rather than trusting this literal alone.
            // `from_bytes` is infallible (a `const fn` over a fixed-size
            // array), so no fallible `parse` + panic-on-error is needed here.
            source_sha256: Sha256Digest::from_bytes([
                0xf2, 0x5e, 0x05, 0xd8, 0xc3, 0x92, 0x3f, 0x33, 0x1f, 0x96, 0xa7, 0x3e, 0x6b, 0xa8,
                0xbb, 0x8b, 0x05, 0x5b, 0x5c, 0xa2, 0xaa, 0x54, 0x84, 0x02, 0xa3, 0xb6, 0x33, 0x14,
                0x1b, 0x8d, 0xea, 0x21,
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
            // scanning". See module doc for the distinct `{output_folder}`
            // gap.
            output_locations: vec!["planning_artifacts".to_owned()],
            // `output_file = {output_folder}/project-context.md` -- a
            // single markdown document.
            artifact_kinds: vec![BmadArtifactKind::Document],
            // step-03-complete.md's role is finishing and persisting
            // `project-context.md`, matching every sibling document-writing
            // skill's completion hook.
            completion_hooks: vec![BmadArtifactCompletionHookRunHook::PersistArtifacts],
            // See module doc: resource closure is out of this adapter's
            // scope for now (tracked as a reported gap, not invented data).
            declared_resources: Vec::new(),
            support_state: BmadSkillSupportState::Exact,
        })
        .unwrap_or_else(|_| unreachable!("this adapter's fields satisfy the harness's own bounds"))
    }
}
