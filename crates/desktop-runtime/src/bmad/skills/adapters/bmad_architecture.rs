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
    BmadMethodSkill, BmadResource, BmadSkillDescriptor, BmadSkillDescriptorInput,
    BmadSkillSupportState, Sha256Digest,
};

/// Adapter for the pinned `bmad-architecture` Method skill (MTH-12).
pub struct BmadArchitectureSkill;

fn source_resource(path: &str, content_hash: [u8; 32]) -> BmadResource {
    BmadResource::new(path, Sha256Digest::from_bytes(content_hash)).unwrap_or_else(|_| {
        unreachable!("all pinned bmad-architecture resource paths are canonical")
    })
}

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
            // The complete recursively owned source subtree is immutable
            // package data. The generic resolver receives identities only;
            // this descriptor never opens, executes, or grants authority to
            // a pinned source member.
            declared_resources: vec![
                source_resource(
                    "src/bmm-skills/3-solutioning/bmad-architecture/SKILL.md",
                    [
                        0x89, 0xa3, 0x16, 0xfd, 0x74, 0xd8, 0x49, 0x0d, 0x1c, 0xeb, 0x2e, 0xc7,
                        0xc4, 0x0e, 0xa5, 0x03, 0x50, 0x4f, 0xd4, 0xc7, 0xa2, 0xc1, 0xe3, 0x10,
                        0x89, 0xbf, 0xc6, 0x28, 0xc8, 0x6c, 0x45, 0xbf,
                    ],
                ),
                source_resource(
                    "src/bmm-skills/3-solutioning/bmad-architecture/assets/spine-template.md",
                    [
                        0xef, 0x4f, 0xf7, 0x95, 0x62, 0x4e, 0xb5, 0x43, 0x9f, 0xae, 0x54, 0xa0,
                        0x6e, 0xdb, 0x38, 0x9f, 0xeb, 0x5a, 0x0c, 0xf7, 0x9c, 0xb0, 0x1a, 0xe0,
                        0x07, 0xaf, 0x51, 0x10, 0x93, 0x35, 0xd1, 0x98,
                    ],
                ),
                source_resource(
                    "src/bmm-skills/3-solutioning/bmad-architecture/customize.toml",
                    [
                        0xa6, 0xbb, 0x60, 0xa3, 0xad, 0xf9, 0x76, 0x03, 0x1f, 0x28, 0x70, 0x20,
                        0x32, 0x85, 0xb1, 0xfc, 0x7a, 0x56, 0xe7, 0x1e, 0x21, 0x23, 0x2c, 0xd2,
                        0x3b, 0xba, 0x54, 0x4b, 0x17, 0x8b, 0x5f, 0x15,
                    ],
                ),
                source_resource(
                    "src/bmm-skills/3-solutioning/bmad-architecture/references/headless.md",
                    [
                        0x4d, 0xb2, 0x70, 0x8f, 0x33, 0x5c, 0x94, 0xcb, 0x02, 0x4b, 0x5e, 0xba,
                        0x02, 0x2b, 0x1e, 0x3d, 0x88, 0xb1, 0xec, 0x61, 0xc9, 0x4c, 0x47, 0x16,
                        0x62, 0x6d, 0x9b, 0x89, 0x68, 0x4f, 0x58, 0x33,
                    ],
                ),
                source_resource(
                    "src/bmm-skills/3-solutioning/bmad-architecture/references/reviewer-gate.md",
                    [
                        0xd3, 0x2e, 0x32, 0xa3, 0xc1, 0xd5, 0x9b, 0x56, 0x12, 0xb9, 0x47, 0x00,
                        0x4f, 0x3f, 0x6f, 0xef, 0x11, 0x17, 0xa1, 0x3c, 0xe9, 0xf1, 0xff, 0xa4,
                        0x28, 0xd6, 0x16, 0xe0, 0xb5, 0xd4, 0xdb, 0x69,
                    ],
                ),
                source_resource(
                    "src/bmm-skills/3-solutioning/bmad-architecture/scripts/lint_spine.py",
                    [
                        0x04, 0x0e, 0x0f, 0x1c, 0x13, 0xd3, 0x1e, 0x6a, 0x21, 0xc5, 0x18, 0xdf,
                        0x94, 0x3d, 0x1d, 0x7d, 0xe7, 0xb7, 0x5d, 0x8d, 0x98, 0xa0, 0x76, 0x03,
                        0xb0, 0x98, 0x3f, 0x3f, 0x3f, 0xb5, 0x00, 0xdb,
                    ],
                ),
                source_resource(
                    "src/bmm-skills/3-solutioning/bmad-architecture/scripts/tests/test_lint_spine.py",
                    [
                        0xe8, 0x6b, 0xb9, 0xa6, 0x2c, 0x5c, 0xca, 0x74, 0xdd, 0xc8, 0x9c, 0x84,
                        0x33, 0x86, 0xf2, 0xb9, 0x92, 0x61, 0x0f, 0xcc, 0x17, 0x92, 0xd4, 0x0e,
                        0x03, 0xbc, 0x9c, 0xd4, 0x45, 0xb7, 0x76, 0x81,
                    ],
                ),
            ],
            // The source's create progression is governed here, but Update
            // and Validate need shared intent/session/report contracts that
            // this descriptor-only adapter deliberately cannot invent.
            support_state: BmadSkillSupportState::Adapted,
        })
        .unwrap_or_else(|_| unreachable!("this adapter's fields satisfy the harness's own bounds"))
    }
}
