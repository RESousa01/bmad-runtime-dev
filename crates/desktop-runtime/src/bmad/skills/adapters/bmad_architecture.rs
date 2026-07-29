//! The `bmad-architecture` skill adapter.
//!
//! Without this module, MTH-12 has no typed descriptor to hand the runtime,
//! Help, or the compatibility ledger -- they would each have to re-derive
//! this skill's shape from the pinned `SKILL.md` prose themselves.
//!
//! The pinned source (`packages/bmad-foundation/source/method/6.10.0/src/
//! bmm-skills/3-solutioning/bmad-architecture/SKILL.md`) coaches (or, on the
//! Fast path, drafts) an "architecture spine" -- a lean invariants document
//! distilled from an append-only `.memlog.md` and written to
//! `ARCHITECTURE-SPINE.md` under `{workflow.spine_output_path}`. Activation
//! step 3 explicitly names both operating modes: "Headless (no interactive
//! user) -> follow `references/headless.md` for the whole run. Otherwise
//! greet `{user_name}`" -- so, unlike some sibling skills in this batch,
//! headless is source-declared here, not only catalog-declared.
//!
//! `declared_actions` stays empty: the source detects intent (create /
//! update / validate) from conversation and input rather than exposing it as
//! a fixed, separately-invocable action, and the catalog row for this rowId
//! records `action: null`.
//!
//! This crate also carries `crate::bmad::architecture` -- a host-owned state
//! machine that validates the *ordered progression* of a sealed
//! architecture-creation guidance flow (`SystemContext`, `Components`, ...,
//! `Deferred`) and distills accepted steps into a
//! [`crate::bmad::capability::BmadDocumentArtifact`]. That module is a
//! runtime *behavior* validator for one specific guidance flow; this
//! descriptor is a *declared-shape* description of the whole skill. They do
//! not contradict each other -- `architecture.rs`'s step breadth is Forge's
//! own reading of "sweep the breadth the altitude owns" and is documented
//! there as "not sealed BMAD source", while this descriptor's fields (modes,
//! output location, artifact kind) come straight from `SKILL.md` prose. Read
//! together: the fine-grained step ordering `architecture.rs` enforces is
//! invisible at this descriptor's altitude, which only asserts that the
//! skill is invocable in both modes and produces one persisted document.

use crate::generated_contracts::{
    BmadArtifactCompletionHookRunHook, BmadArtifactKind, BmadInvocationMode,
};
use crate::{
    BmadMethodSkill, BmadSkillDescriptor, BmadSkillDescriptorInput, BmadSkillSupportState,
    Sha256Digest,
};

/// Adapter for the pinned `bmad-architecture` Method skill (MTH-12).
pub struct BmadArchitectureSkill;

impl BmadMethodSkill for BmadArchitectureSkill {
    fn descriptor(&self) -> BmadSkillDescriptor {
        BmadSkillDescriptor::new(BmadSkillDescriptorInput {
            canonical_id: "bmm:bmad-architecture".to_owned(),
            source_path: "src/bmm-skills/3-solutioning/bmad-architecture/SKILL.md".to_owned(),
            // Matches capability-catalog.json's row for this rowId; see
            // `bmad_method_solutioning_skills.rs`, which binds against the
            // catalog directly rather than trusting this literal alone.
            // `from_bytes` is infallible (a `const fn` over a fixed-size
            // array), so no fallible `parse` + panic-on-error is needed here.
            source_sha256: Sha256Digest::from_bytes([
                0x89, 0xa3, 0x16, 0xfd, 0x74, 0xd8, 0x49, 0x0d, 0x1c, 0xeb, 0x2e, 0xc7, 0xc4, 0x0e,
                0xa5, 0x03, 0x50, 0x4f, 0xd4, 0xc7, 0xa2, 0xc1, 0xe3, 0x10, 0x89, 0xbf, 0xc6, 0x28,
                0xc8, 0x6c, 0x45, 0xbf,
            ]),
            // Create/Update/Validate intent is detected conversationally,
            // not exposed as a fixed action; catalog row records
            // `action: null`. See module doc.
            declared_actions: Vec::new(),
            supported_modes: vec![
                BmadInvocationMode::Interactive,
                BmadInvocationMode::Headless,
            ],
            // No predecessor/follower is named as a fixed edge in the
            // source text -- Finalize step 7 *recommends* `bmad-spec` and
            // `bmad-create-epics-and-stories` as next steps in prose, but
            // that is conversational routing, not a declared, fixed
            // predecessor/follower edge (matching the conservative reading
            // every other adapter in this batch and the prior one applies).
            predecessor: None,
            follower: None,
            // "bind `{doc_workspace}` to `{workflow.spine_output_path}/
            // {workflow.run_folder_pattern}/`" -- resolved under the same
            // `{planning_artifacts}` config surface every sibling skill in
            // this catalog resolves from `_bmad/bmm/config.yaml`.
            output_locations: vec!["planning_artifacts".to_owned()],
            // "seed `ARCHITECTURE-SPINE.md` from `{workflow.spine_template}`"
            // -- a single markdown document.
            artifact_kinds: vec![BmadArtifactKind::Document],
            // The memlog is written via `memlog.py append` as the run
            // progresses, and the spine itself is distilled and saved at
            // Finalize/Update/Close -- persisted, not merely proposed.
            completion_hooks: vec![BmadArtifactCompletionHookRunHook::PersistArtifacts],
            // See module doc: resource closure is out of this adapter's
            // scope for now (tracked as a reported gap, not invented data).
            declared_resources: Vec::new(),
            support_state: BmadSkillSupportState::Exact,
        })
        .unwrap_or_else(|_| unreachable!("this adapter's fields satisfy the harness's own bounds"))
    }
}
