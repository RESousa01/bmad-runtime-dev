//! The `bmad-customize` skill adapter.
//!
//! The pinned source (`packages/bmad-foundation/source/method/6.10.0/src/
//! core-skills/bmad-customize/SKILL.md`) writes "a correctly-placed TOML
//! override file under `{project-root}/_bmad/custom/`" (Step 5/6): either
//! `{skill-name}.toml` (team) or `{skill-name}.user.toml` (user). That is a
//! single configuration file, not a narrative document, so `artifact_kinds`
//! is `[FileSet]` rather than `Document`; the write is persisted immediately
//! ("Write. Create ... if needed."), matching
//! `BmadArtifactCompletionHookRunHook::PersistArtifacts`.
//!
//! `declared_actions` stays empty: the catalog row for this rowId records
//! `action: null`, and Step 1 ("Classify intent": Directed / Exploratory /
//! Audit-iterate / Cross-cutting) is conversational routing, not a fixed,
//! separately-invocable action.

use crate::generated_contracts::{
    BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationMode,
};
use crate::{
    BmadMethodSkill, BmadSkillDescriptor, BmadSkillDescriptorInput, BmadSkillSupportState,
    Sha256Digest,
};

/// Adapter for the pinned `bmad-customize` Method skill.
pub struct BmadCustomizeSkill;

impl BmadMethodSkill for BmadCustomizeSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "core:bmad-customize".to_owned(),
            source_path: "src/core-skills/bmad-customize/SKILL.md".to_owned(),
            // Matches capability-catalog.json's row for this rowId.
            source_sha256: Sha256Digest::from_bytes([
                0xa4, 0x20, 0x86, 0xf9, 0x00, 0x24, 0x01, 0x3e, 0x26, 0x11, 0x50, 0x3e, 0xb0, 0x22,
                0x4a, 0xc0, 0xe9, 0x0b, 0x2a, 0xec, 0xac, 0x44, 0xa8, 0x86, 0xae, 0x0e, 0x38, 0xcf,
                0xdd, 0xa6, 0x95, 0x11,
            ]),
            declared_actions: Vec::new(),
            supported_modes: vec![
                BmadInvocationMode::Interactive,
                BmadInvocationMode::Headless,
            ],
            predecessor: None,
            follower: None,
            // Step 5 writes under `{project-root}/_bmad/custom/`.
            output_locations: vec!["_bmad/custom".to_owned()],
            // A TOML override file, not a narrative document.
            artifact_kinds: vec![BmadArtifactKind::FileSet],
            completion_hooks: vec![BmadArtifactCompletionHookRunHook::PersistArtifacts],
            declared_resources: Vec::new(),
            support_state: BmadSkillSupportState::Exact,
        })
        .unwrap_or_else(|_| unreachable!("this adapter's fields satisfy the harness's own bounds"))
    }
}
