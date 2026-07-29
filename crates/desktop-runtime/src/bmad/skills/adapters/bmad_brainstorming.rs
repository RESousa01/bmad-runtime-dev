//! The `bmad-brainstorming` skill adapter.
//!
//! The pinned source (`packages/bmad-foundation/source/method/6.10.0/src/
//! core-skills/bmad-brainstorming/SKILL.md`) binds `{doc_workspace} =
//! {workflow.output_dir}/{workflow.output_folder_name}/` and keeps "the
//! memlog" -- `.memlog.md` -- as "the session's memory: the single source
//! every output builds from". Wrap-Up loads `references/finalize.md` to
//! write "synthesis, `status: complete`, artifacts" into the same workspace.
//! Both are markdown documents, so `artifact_kinds` is `[Document]` and the
//! completion hook is `PersistArtifacts`.
//!
//! `declared_actions` stays empty: the catalog row for this rowId records
//! `action: null`, and the run's shape (stance, technique batch) is chosen
//! conversationally or via the composer page, not a fixed, separately-
//! invocable action.
//!
//! `predecessor`/`follower` stay `None`: the source names
//! `bmad-party-mode` and `bmad-advanced-elicitation` only as skills
//! "available any time" during a session, not as an ordering relationship.

use crate::generated_contracts::{
    BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationMode,
};
use crate::{
    BmadMethodSkill, BmadSkillDescriptor, BmadSkillDescriptorInput, BmadSkillSupportState,
    Sha256Digest,
};

/// Adapter for the pinned `bmad-brainstorming` Method skill.
pub struct BmadBrainstormingSkill;

impl BmadMethodSkill for BmadBrainstormingSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "core:bmad-brainstorming".to_owned(),
            source_path: "src/core-skills/bmad-brainstorming/SKILL.md".to_owned(),
            // Matches capability-catalog.json's row for this rowId.
            source_sha256: Sha256Digest::from_bytes([
                0xe9, 0xcb, 0x07, 0xfe, 0x86, 0xaa, 0x25, 0x4f, 0x9f, 0xb7, 0x21, 0x77, 0x20, 0xd7,
                0x7f, 0xd6, 0x93, 0xbf, 0x66, 0xa0, 0xc0, 0x1a, 0xf8, 0x11, 0x4e, 0xcd, 0xf7, 0x14,
                0x81, 0x59, 0xfd, 0xd1,
            ]),
            declared_actions: Vec::new(),
            supported_modes: vec![
                BmadInvocationMode::Interactive,
                BmadInvocationMode::Headless,
            ],
            predecessor: None,
            follower: None,
            // `{workflow.output_dir}` is the resolved root the session's
            // `{doc_workspace}` binds under.
            output_locations: vec!["output_dir".to_owned()],
            // The memlog and the Wrap-Up synthesis are both markdown.
            artifact_kinds: vec![BmadArtifactKind::Document],
            completion_hooks: vec![BmadArtifactCompletionHookRunHook::PersistArtifacts],
            declared_resources: Vec::new(),
            support_state: BmadSkillSupportState::Exact,
        })
        .unwrap_or_else(|_| unreachable!("this adapter's fields satisfy the harness's own bounds"))
    }
}
