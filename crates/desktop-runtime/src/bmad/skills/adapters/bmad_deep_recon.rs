//! The `bmad-deep-recon` skill adapter.
//!
//! The pinned source (`packages/bmad-foundation/source/method/6.10.0/src/
//! core-skills/bmad-deep-recon/SKILL.md`) states "every intent shares the
//! run-folder workspace shape -- `brief.md`, `imports/`, `digests/`,
//! `research.md`, `.memlog.md`" and "nothing exists until it is a file:
//! every digest, import extraction, and report section is written to the
//! run folder the moment it lands". That workspace mixes single markdown
//! documents (`brief.md`, `research.md`, `.memlog.md`) with directories of
//! multiple files (`imports/`, `digests/`), so `artifact_kinds` carries both
//! `Document` and `FileSet`; persistence is immediate and per-write, matching
//! `PersistArtifacts`.
//!
//! `declared_actions` stays empty: the catalog row for this rowId records
//! `action: null`. The four intents (Draft/Process/Run/Refresh-Deepen) are
//! detected from the user's ask (`## On Activation`, step 4), not fixed,
//! separately-invocable actions.
//!
//! This rowId's catalog `aliases` list three legacy shims
//! (`bmm:bmad-domain-research`, `bmm:bmad-market-research`,
//! `bmm:bmad-technical-research`) that this one consolidated skill now
//! serves; those aliases are catalog metadata, not part of this descriptor.

use crate::generated_contracts::{
    BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationMode,
};
use crate::{
    BmadMethodSkill, BmadSkillDescriptor, BmadSkillDescriptorInput, BmadSkillSupportState,
    Sha256Digest,
};

/// Adapter for the pinned `bmad-deep-recon` Method skill.
pub struct BmadDeepReconSkill;

impl BmadMethodSkill for BmadDeepReconSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "core:bmad-deep-recon".to_owned(),
            source_path: "src/core-skills/bmad-deep-recon/SKILL.md".to_owned(),
            // Matches capability-catalog.json's row for this rowId.
            source_sha256: Sha256Digest::from_bytes([
                0xdc, 0xe5, 0xf7, 0xdb, 0x03, 0x09, 0x4a, 0x61, 0x7c, 0xc3, 0xfc, 0xae, 0x17, 0x11,
                0xb1, 0xdc, 0x7c, 0xce, 0xbe, 0x37, 0x93, 0x76, 0xe6, 0x72, 0x16, 0x54, 0x49, 0x56,
                0xd3, 0x75, 0xf0, 0xd2,
            ]),
            declared_actions: Vec::new(),
            supported_modes: vec![
                BmadInvocationMode::Interactive,
                BmadInvocationMode::Headless,
            ],
            predecessor: None,
            follower: None,
            // Run-folder root: `{workflow.research_output_path}` (referenced
            // when checking for an existing run folder to resume/extend).
            output_locations: vec!["research_output_path".to_owned()],
            artifact_kinds: vec![BmadArtifactKind::Document, BmadArtifactKind::FileSet],
            completion_hooks: vec![BmadArtifactCompletionHookRunHook::PersistArtifacts],
            declared_resources: Vec::new(),
            support_state: BmadSkillSupportState::Exact,
        })
        .unwrap_or_else(|_| unreachable!("this adapter's fields satisfy the harness's own bounds"))
    }
}
