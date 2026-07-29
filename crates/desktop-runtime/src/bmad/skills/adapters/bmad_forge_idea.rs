//! The `bmad-forge-idea` skill adapter.
//!
//! The pinned source (`packages/bmad-foundation/source/method/6.10.0/src/
//! core-skills/bmad-forge-idea/SKILL.md`) binds `{workspace} =
//! {workflow.forge_output_path}/{workflow.run_folder_pattern}`, keeps a
//! memlog there for the whole session, and on a Hardened exit "Distill the
//! memlog into `{workspace}/forged-idea.md`". Every exit additionally
//! "Always render `{workspace}/forge-report.html`". Both are documents
//! (markdown and self-contained HTML), so `artifact_kinds` is `[Document]`;
//! writes are persisted as they happen, matching `PersistArtifacts`.
//!
//! `declared_actions` stays empty: the catalog row for this rowId records
//! `action: null`. "attack this" / "defend this" / "switch roles" are
//! in-conversation stance changes, not fixed, separately-invocable actions.
//!
//! `follower` stays `None`: the Hardened exit names four legitimate next
//! skills side by side ("`bmad-spec`, `bmad-prd`, `bmad-prfaq`,
//! `bmad-quick-dev`"), not one fixed follower -- the same "any of these"
//! framing `bmad-prd`'s adapter already documents for its own Finalize step.

use crate::generated_contracts::{
    BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationMode,
};
use crate::{
    BmadMethodSkill, BmadSkillDescriptor, BmadSkillDescriptorInput, BmadSkillSupportState,
    Sha256Digest,
};

/// Adapter for the pinned `bmad-forge-idea` Method skill.
pub struct BmadForgeIdeaSkill;

impl BmadMethodSkill for BmadForgeIdeaSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "core:bmad-forge-idea".to_owned(),
            source_path: "src/core-skills/bmad-forge-idea/SKILL.md".to_owned(),
            // Matches capability-catalog.json's row for this rowId.
            source_sha256: Sha256Digest::from_bytes([
                0xed, 0x37, 0x77, 0xd2, 0xb3, 0x08, 0x57, 0xc6, 0x12, 0xca, 0x32, 0x60, 0x10, 0x1d,
                0x0c, 0x67, 0x23, 0x30, 0x63, 0x56, 0xa1, 0xa4, 0xc2, 0x3e, 0x6a, 0x2f, 0x2a, 0x58,
                0x83, 0xeb, 0x1e, 0x54,
            ]),
            declared_actions: Vec::new(),
            supported_modes: vec![
                BmadInvocationMode::Interactive,
                BmadInvocationMode::Headless,
            ],
            predecessor: None,
            follower: None,
            // `{workflow.forge_output_path}` is the resolved root the
            // session workspace binds under.
            output_locations: vec!["forge_output_path".to_owned()],
            artifact_kinds: vec![BmadArtifactKind::Document],
            completion_hooks: vec![BmadArtifactCompletionHookRunHook::PersistArtifacts],
            declared_resources: Vec::new(),
            support_state: BmadSkillSupportState::Exact,
        })
        .unwrap_or_else(|_| unreachable!("this adapter's fields satisfy the harness's own bounds"))
    }
}
