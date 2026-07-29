# bmad-architecture

- **rowId**: `method:src/bmm-skills/3-solutioning/bmad-architecture/SKILL.md`
- **canonicalId**: `bmm:bmad-architecture`
- **pinned source**: `packages/bmad-foundation/source/method/6.10.0/src/bmm-skills/3-solutioning/bmad-architecture/SKILL.md`

## What it does

Coaches (default) or, on the Fast path, drafts an "architecture spine": a
lean consistency contract fixing only the invariants a future builder can't
read off compliant code, distilled from an append-only `.memlog.md` run log
into `ARCHITECTURE-SPINE.md`. Detects create/update/validate intent from
conversation and input; for a new spine, binds `{doc_workspace}` under
`{workflow.spine_output_path}` and seeds the spine from
`{workflow.spine_template}`. Runs a Reviewer Gate before handoff and
recommends `bmad-spec` and `bmad-create-epics-and-stories` as next steps in
prose at Finalize.

## Declared actions and modes

- **Actions**: none declared as a fixed, separately-invocable action.
  Catalog row records `action: null`. Create/Update/Validate intent is
  detected conversationally, matching the same representational limit
  `bmad-product-brief` (MTH-05) already documents.
- **Modes**: `interactive`, `headless` -- and unlike the other four skills
  in this batch, headless is explicitly named in the source itself:
  Activation Step 3 forks on "Headless (no interactive user) -> follow
  `references/headless.md` for the whole run."
- **Output location**: `{planning_artifacts}` (the same config surface
  every sibling skill resolves `{workflow.spine_output_path}` under).
- **Artifact kind**: `document`. The source explicitly seeds and later
  distills `ARCHITECTURE-SPINE.md`, persisted via `memlog.py append` writes
  through the run and saved at Finalize/Update/Close --
  `PersistArtifacts` completion hook.

## Runtime relationship to `crate::bmad::architecture`

This crate already carries a host-owned state machine
(`crates/desktop-runtime/src/bmad/architecture.rs`) that validates the
*ordered progression* of a sealed architecture-creation guidance flow
(`SystemContext`, `Components`, ..., `Deferred`) and distills accepted steps
into a `BmadDocumentArtifact`. That module operates at a different altitude
than this descriptor: it enforces fine-grained step ordering for one
specific guidance flow and documents its own step list as "Forge's own
reading" of the source's "sweep the breadth the altitude owns" instruction,
not sealed BMAD source. This descriptor only asserts the skill's
catalog-facing shape (modes, output location, one persisted document
artifact) and does not duplicate or contradict `architecture.rs`'s step
ordering.

## Compatibility decision

**adopt / exact.** No semantic divergence was found between the pinned
source and the harness descriptor.
