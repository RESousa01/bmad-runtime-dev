//! The `bmad-help` skill adapter.
//!
//! The pinned source (`packages/bmad-foundation/source/method/6.10.0/src/
//! core-skills/bmad-help/SKILL.md`) is a model-driven skill: it reads a CSV
//! catalog and resolved config, but also fetches remote module documentation
//! ("Module docs: ... Fetch and use these to answer general questions about
//! that module") and answers open-ended questions from "existing knowledge
//! of the skill" when the CSV description is absent. No step names a
//! persisted output file -- the deliverable is the in-chat orientation
//! response -- so `output_locations`, `artifact_kinds`, and
//! `completion_hooks` stay empty.
//!
//! `declared_actions` stays empty: the catalog row for this rowId records
//! `action: null`, and "what to do next" is derived by ranking the catalog
//! against the user's query, not a fixed, separately-invocable action.
//!
//! ## Divergence from the pinned source: `support_state: Adapted`
//!
//! `crates/desktop-runtime/src/bmad/help.rs` already implements a runtime
//! Help engine, and it deliberately narrows the pinned source's surface
//! rather than shimming or exactly replicating it:
//! - It is **model-free by construction** (`bmad_help_answers_with_no_model_
//!   or_chat_capability_present` scans for and forbids any model/chat
//!   reference). The pinned source's "Module docs ... Fetch and use these to
//!   answer general questions" and open-ended-question handling are
//!   explicitly model-driven behaviors `help.rs` does not, and structurally
//!   cannot, perform.
//! - It answers a fixed set of typed queries (availability, next-steps,
//!   alias resolution, capability projection, catalog counts) rather than
//!   arbitrary free-form questions the pinned source's "Get answers to
//!   general questions" outcome promises.
//! - Its `BmadHelpSupportState` (Exact/Adapted/Shim, `help.rs` line ~343) is
//!   a *different, narrower-scoped* type from this harness's
//!   [`BmadSkillSupportState`] (Exact/Adapted/Shim/Unavailable) -- `help.rs`'s
//!   own doc comment on [`BmadSkillSupportState`] explains why the two could
//!   not be unified, and this adapter does not attempt to bridge them.
//!
//! This is a genuine, nameable semantic divergence, not an excuse to avoid
//! documenting fidelity: the deterministic subset `help.rs` covers matches
//! the pinned source faithfully, but the remote-doc-fetch and general-
//! question-answering outcomes are out of scope for it. `Adapted` reports
//! that honestly rather than claiming `Exact`.

use crate::generated_contracts::BmadInvocationMode;
use crate::{
    BmadMethodSkill, BmadSkillDescriptor, BmadSkillDescriptorInput, BmadSkillSupportState,
    Sha256Digest,
};

/// Adapter for the pinned `bmad-help` Method skill.
///
/// See the module doc for why this reports [`BmadSkillSupportState::Adapted`]
/// rather than `Exact`, and why it does not reference
/// `crate::bmad::help::BmadHelpQuery` directly: this adapter only *describes*
/// the pinned skill's declared shape (per `harness.rs`'s "authority is
/// structural, not a promise" boundary); it never becomes the engine that
/// answers a Help query.
pub struct BmadHelpSkill;

impl BmadMethodSkill for BmadHelpSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "core:bmad-help".to_owned(),
            source_path: "src/core-skills/bmad-help/SKILL.md".to_owned(),
            // Matches capability-catalog.json's row for this rowId.
            source_sha256: Sha256Digest::from_bytes([
                0x97, 0x0c, 0x78, 0xec, 0x4e, 0xb6, 0x49, 0xc7, 0xed, 0xdd, 0x6d, 0xe0, 0xcf, 0x83,
                0xdf, 0xa9, 0xab, 0x1e, 0x63, 0xd4, 0x1f, 0x04, 0xd0, 0x24, 0x23, 0xe6, 0x28, 0x28,
                0xcb, 0x1f, 0xb5, 0xb2,
            ]),
            declared_actions: Vec::new(),
            supported_modes: vec![
                BmadInvocationMode::Interactive,
                BmadInvocationMode::Headless,
            ],
            predecessor: None,
            follower: None,
            // No persisted output is named: Help's deliverable is the
            // in-chat orientation response, and its own read paths (the CSV
            // catalog, resolved config, project artifacts) are inputs, not
            // outputs this skill writes.
            output_locations: Vec::new(),
            artifact_kinds: Vec::new(),
            completion_hooks: Vec::new(),
            declared_resources: Vec::new(),
            support_state: BmadSkillSupportState::Adapted,
        })
        .unwrap_or_else(|_| unreachable!("this adapter's fields satisfy the harness's own bounds"))
    }
}
