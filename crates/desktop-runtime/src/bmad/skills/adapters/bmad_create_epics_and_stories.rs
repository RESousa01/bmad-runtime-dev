//! The `bmad-create-epics-and-stories` skill adapter.
//!
//! Without this module, MTH-14 has no typed descriptor to hand the runtime,
//! Help, or the compatibility ledger -- they would each have to re-derive
//! this skill's shape from the pinned `SKILL.md` prose themselves.
//!
//! The pinned source (`packages/bmad-foundation/source/method/6.10.0/src/
//! bmm-skills/3-solutioning/bmad-create-epics-and-stories/SKILL.md`) is a
//! micro-file, step-sequenced workflow transforming PRD requirements and
//! architecture decisions into epics and stories: activation resolves a
//! `workflow` customization block, then execution hands off to
//! `steps/step-01-validate-prerequisites.md` through
//! `steps/step-04-final-validation.md`. Step 4 saves "the final epics.md" as
//! the working artifact and, on completion, tells the user "Invoke the
//! `bmad-help` skill".
//!
//! `declared_actions` stays empty: the source names no fixed,
//! separately-invocable action -- a single linear step-file progression --
//! matching the catalog row's `action: null`.
//!
//! **Modes gap**: `capability-catalog.json` declares both `interactive` and
//! `headless` for this rowId, but neither the `SKILL.md` nor any of its four
//! step files names a headless path anywhere. Every step file instead halts
//! at menus and repeats "NEVER generate content without user input". Same
//! gap `bmad-check-implementation-readiness` (MTH-13) reports:
//! `supported_modes` still declares both, per the non-negotiable rule that a
//! descriptor "declares its catalog-declared modes", but this is a fidelity
//! gap the harness cannot otherwise represent.

use crate::generated_contracts::{
    BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationMode,
};
use crate::{
    BmadMethodSkill, BmadSkillDescriptor, BmadSkillDescriptorInput, BmadSkillSupportState,
    Sha256Digest,
};

/// Adapter for the pinned `bmad-create-epics-and-stories` Method skill
/// (MTH-14).
pub struct BmadCreateEpicsAndStoriesSkill;

impl BmadMethodSkill for BmadCreateEpicsAndStoriesSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "bmm:bmad-create-epics-and-stories".to_owned(),
            source_path: "src/bmm-skills/3-solutioning/bmad-create-epics-and-stories/SKILL.md"
                .to_owned(),
            // Matches capability-catalog.json's row for this rowId; see
            // `bmad_method_solutioning_skills.rs`, which binds against the
            // catalog directly rather than trusting this literal alone.
            // `from_bytes` is infallible (a `const fn` over a fixed-size
            // array), so no fallible `parse` + panic-on-error is needed here.
            source_sha256: Sha256Digest::from_bytes([
                0x4e, 0xc9, 0xec, 0x25, 0x5c, 0x2c, 0x0e, 0x97, 0xae, 0x91, 0x23, 0x42, 0xae, 0x14,
                0x45, 0x31, 0x8e, 0x67, 0x2a, 0xa1, 0xc2, 0xa9, 0xe4, 0x12, 0x80, 0x6b, 0x7d, 0x7e,
                0xf7, 0xd5, 0xa6, 0xa7,
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
            // source text -- completion only says "Invoke the `bmad-help`
            // skill", which is generic routing, not a declared edge.
            predecessor: None,
            follower: None,
            // Step 4 ("Load Config") explicitly resolves
            // `{planning_artifacts}` "for output location and artifact
            // scanning".
            output_locations: vec!["planning_artifacts".to_owned()],
            // Step 4 ("Complete and Save") saves "the final epics.md" -- a
            // single markdown document.
            artifact_kinds: vec![BmadArtifactKind::Document],
            // "Save the final epics.md" is explicit persistence once all
            // validations pass.
            completion_hooks: vec![BmadArtifactCompletionHookRunHook::PersistArtifacts],
            // See module doc: resource closure is out of this adapter's
            // scope for now (tracked as a reported gap, not invented data).
            declared_resources: Vec::new(),
            support_state: BmadSkillSupportState::Exact,
        })
        .unwrap_or_else(|_| unreachable!("this adapter's fields satisfy the harness's own bounds"))
    }
}
