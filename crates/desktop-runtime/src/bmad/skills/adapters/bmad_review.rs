//! The `bmad-review` skill adapter.
//!
//! The pinned source (`packages/bmad-foundation/source/method/6.10.0/src/
//! core-skills/bmad-review/SKILL.md`) always produces "one JSON array
//! holding every finding from every lens" and, "when `{workflow.report_path}`
//! is set, write[s] the report there; otherwise present it in chat" (Output
//! section). The persisted form is a findings report, so `artifact_kinds` is
//! `[Report]` rather than `Document`; the write happens on completion,
//! matching `PersistArtifacts`.
//!
//! `declared_actions` stays empty: the catalog row for this rowId records
//! `action: null`. Lens selection (adversarial/edge-case/verification-gap/
//! structure/prose, or any override) is resolved from `{workflow.lenses}`
//! and the caller's request at Execution step 3, not fixed, separately-
//! invocable actions -- and the lens set itself is explicitly "not a fixed
//! list" per the Overview.
//!
//! This rowId's catalog `aliases` list six legacy shims this one
//! consolidated skill now serves (`core:bmad-editorial-review`,
//! `core:bmad-review-adversarial-general`, etc.); those aliases are catalog
//! metadata, not part of this descriptor.

use crate::generated_contracts::{
    BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationMode,
};
use crate::{
    BmadMethodSkill, BmadSkillDescriptor, BmadSkillDescriptorInput, BmadSkillSupportState,
    Sha256Digest,
};

/// Adapter for the pinned `bmad-review` Method skill.
pub struct BmadReviewSkill;

impl BmadMethodSkill for BmadReviewSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "core:bmad-review".to_owned(),
            source_path: "src/core-skills/bmad-review/SKILL.md".to_owned(),
            // Matches capability-catalog.json's row for this rowId.
            source_sha256: Sha256Digest::from_bytes([
                0xbc, 0x46, 0xcd, 0xf6, 0x6b, 0x37, 0xb4, 0xd4, 0x9d, 0x41, 0x4b, 0xdc, 0x5a, 0xfc,
                0x54, 0x74, 0x7a, 0x9b, 0x19, 0xb3, 0xfe, 0xca, 0xe4, 0x71, 0xd4, 0x51, 0xb3, 0x69,
                0x59, 0x34, 0x3e, 0xc3,
            ]),
            declared_actions: Vec::new(),
            supported_modes: vec![
                BmadInvocationMode::Interactive,
                BmadInvocationMode::Headless,
            ],
            predecessor: None,
            follower: None,
            // "When `{workflow.report_path}` is set, write the report there".
            output_locations: vec!["report_path".to_owned()],
            artifact_kinds: vec![BmadArtifactKind::Report],
            completion_hooks: vec![BmadArtifactCompletionHookRunHook::PersistArtifacts],
            declared_resources: Vec::new(),
            support_state: BmadSkillSupportState::Exact,
        })
        .unwrap_or_else(|_| unreachable!("this adapter's fields satisfy the harness's own bounds"))
    }
}
