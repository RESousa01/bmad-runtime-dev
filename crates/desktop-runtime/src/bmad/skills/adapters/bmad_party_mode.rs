//! The `bmad-party-mode` skill adapter.
//!
//! The pinned source (`packages/bmad-foundation/source/method/6.10.0/src/
//! core-skills/bmad-party-mode/SKILL.md`) keeps "a party's memory ... the
//! per-party memlog at `{workflow.memory_dir}/<party>/.memlog.md`" for the
//! whole run when memory is enabled, and at Wrap-Up offers "a single self-
//! contained ... HTML of the session ... written as a `{date}`-stamped
//! `.html` into `{workflow.output_dir}/`". Both are markdown/HTML documents,
//! so `artifact_kinds` is `[Document]`; both writes are persisted as they
//! happen, matching `PersistArtifacts`.
//!
//! `declared_actions` stays empty: the catalog row for this rowId records
//! `action: null`. The four run modes (`session`/`auto`/`subagent`/
//! `agent-team`) are resolved from `{workflow.party_mode}` or a runtime
//! `--mode` flag, not fixed, separately-invocable actions.
//!
//! `predecessor`/`follower` stay `None`: the source names no fixed
//! upstream or downstream skill -- it is invoked directly ("Use when the
//! user requests party mode...") and hands control back to "normal mode" at
//! Wrap-Up rather than routing to a named successor.

use crate::generated_contracts::{
    BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationMode,
};
use crate::{
    BmadMethodSkill, BmadSkillDescriptor, BmadSkillDescriptorInput, BmadSkillSupportState,
    Sha256Digest,
};

/// Adapter for the pinned `bmad-party-mode` Method skill.
pub struct BmadPartyModeSkill;

impl BmadMethodSkill for BmadPartyModeSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "core:bmad-party-mode".to_owned(),
            source_path: "src/core-skills/bmad-party-mode/SKILL.md".to_owned(),
            // Matches capability-catalog.json's row for this rowId.
            source_sha256: Sha256Digest::from_bytes([
                0xde, 0x03, 0x5b, 0x4e, 0x08, 0x1f, 0xdd, 0x62, 0x6a, 0x20, 0x11, 0x41, 0x78, 0xf7,
                0x06, 0xac, 0x22, 0x2f, 0x51, 0x91, 0x71, 0xd8, 0x2b, 0x02, 0x13, 0x9a, 0x7c, 0xec,
                0x56, 0x1a, 0x34, 0x87,
            ]),
            declared_actions: Vec::new(),
            supported_modes: vec![
                BmadInvocationMode::Interactive,
                BmadInvocationMode::Headless,
            ],
            predecessor: None,
            follower: None,
            // The per-party memlog root and the Wrap-Up keepsake root.
            output_locations: vec!["memory_dir".to_owned(), "output_dir".to_owned()],
            artifact_kinds: vec![BmadArtifactKind::Document],
            completion_hooks: vec![BmadArtifactCompletionHookRunHook::PersistArtifacts],
            declared_resources: Vec::new(),
            support_state: BmadSkillSupportState::Exact,
        })
        .unwrap_or_else(|_| unreachable!("this adapter's fields satisfy the harness's own bounds"))
    }
}
