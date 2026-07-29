//! The `bmad-product-brief` skill adapter.
//!
//! The pinned source (`packages/bmad-foundation/source/method/6.10.0/src/
//! bmm-skills/1-analysis/bmad-product-brief/SKILL.md`) names three "Intent
//! Operating Modes" in prose (Create, Update, Validate) and writes
//! `brief.md` as its persisted artifact: "Persistence is real-time. Once
//! Create intent is confirmed, the workspace ... exists on disk". That
//! artifact is `BmadArtifactKind::Document`, persisted, matching
//! `BmadArtifactCompletionHookRunHook::PersistArtifacts`.
//!
//! `declared_actions` stays empty even though Create/Update/Validate are
//! named: the catalog row for this rowId records `action: null`, and intent
//! here is inferred conversationally rather than selected as a fixed,
//! separately-invocable action the way [`BmadInvocationDeclaredAction`]
//! models one. Treating "Create" as a declared action would assert a fixed
//! entry point the source does not actually offer -- the skill has exactly
//! one entry point, intent detection happens inside it. This is reported as
//! a gap: see the parent task's "what couldn't be represented" note.
//!
//! [`BmadInvocationDeclaredAction`]: crate::generated_contracts::BmadInvocationDeclaredAction

use crate::generated_contracts::{
    BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationMode,
};
use crate::{
    BmadMethodSkill, BmadSkillDescriptor, BmadSkillDescriptorInput, BmadSkillSupportState,
    Sha256Digest,
};

/// Adapter for the pinned `bmad-product-brief` Method skill (MTH-05).
pub struct BmadProductBriefSkill;

impl BmadMethodSkill for BmadProductBriefSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "bmm:bmad-product-brief".to_owned(),
            source_path: "src/bmm-skills/1-analysis/bmad-product-brief/SKILL.md".to_owned(),
            // Matches capability-catalog.json's row for this rowId; see
            // `bmad_method_analysis_skills.rs`, which binds against the
            // catalog directly rather than trusting this literal alone.
            // `from_bytes` is infallible (a `const fn` over a fixed-size
            // array), so no fallible `parse` + panic-on-error is needed here.
            source_sha256: Sha256Digest::from_bytes([
                0x4c, 0x15, 0xa8, 0xec, 0x68, 0x8c, 0x99, 0x8e, 0xeb, 0x71, 0x95, 0x7f, 0x20, 0x8b,
                0x59, 0x53, 0xb1, 0xac, 0x7a, 0x56, 0x41, 0xb0, 0x08, 0xb3, 0x50, 0x9f, 0x1f, 0x4f,
                0xcd, 0xc8, 0xe6, 0xcd,
            ]),
            declared_actions: Vec::new(),
            supported_modes: vec![
                BmadInvocationMode::Interactive,
                BmadInvocationMode::Headless,
            ],
            predecessor: None,
            follower: None,
            // Activation step 5 explicitly resolves `{planning_artifacts}`
            // alongside `{project_name}`, `{user_name}`, etc.
            output_locations: vec!["planning_artifacts".to_owned()],
            // "write `brief.md` there with YAML frontmatter" -- the working
            // artifact is a single markdown document.
            artifact_kinds: vec![BmadArtifactKind::Document],
            completion_hooks: vec![BmadArtifactCompletionHookRunHook::PersistArtifacts],
            declared_resources: Vec::new(),
            support_state: BmadSkillSupportState::Exact,
        })
        .unwrap_or_else(|_| unreachable!("this adapter's fields satisfy the harness's own bounds"))
    }
}
