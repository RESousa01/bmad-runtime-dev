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

**adapt / adapted.** Owner: Forge BMAD Runtime. Supported Forge versions:
`0.1.x`. The adaptation is re-evaluated against every pinned Method upgrade.

## Semantic diff

| Pinned 6.10.0 behavior | Forge execution | Semantic effect |
| --- | --- | --- |
| Detect `create`, `update`, or `validate` from the supplied intent and artifacts. | The sealed source entrypoint remains the workflow authority; the generic runtime binds the selected intent to one exact Method session and rejects undeclared fixed actions. Golden cases exercise all three source intents. | Preserved, with conversational intent kept distinct from a catalog action. |
| Interactive Coaching/Fast activation and non-interactive headless execution. | Both invocation modes enter the same source-bound execution contract, resource set, artifact expectation, and durable Method checkpoint lifecycle. | Preserved; required output identity is equivalent across modes. |
| Read and append `.memlog.md`, then distill or inspect `ARCHITECTURE-SPINE.md`. | Reads and writes become typed `Read` and `WriteProposal` plans; artifacts remain app-local and require authoritative evidence before completion. | Authority adapted; output semantics preserved. |
| Invoke `memlog.py`, `lint_spine.py`, and customization/config scripts. | Process requests become `CommandProposal` plans. The pinned Python and its tests remain hash-bound resources, never ambient executable authority. | Authority adapted; deterministic validation remains required. |
| Verify current technology on the web. | Browser/network lookup is represented as a distinct `Read` plan and remains unavailable until the host grants the relevant network/browser authority. It is never conflated with model access. | Fail-closed authority adaptation. |
| Dispatch reviewer subagents and model work. | Source subagent requests become typed `ModelCall` plans; reviewer output must return through the same evidence and validation boundary. | Authority adapted; reviewer gate preserved. |
| Run external handoffs at Finalize. | Handoffs become typed `ArtifactPlan` requests and stay unavailable when the destination connector is absent. | Fail-closed authority adaptation. |
| Resume from the append-only memlog after interruption. | `MethodCheckpoint` persists stage, exact binding, context decision, model lineage, artifact references, failure/cancellation state, and restart recovery. Source/resource/config drift requires an explicit rebind and fresh review. | Durable lifecycle strengthened without changing source-visible results. |

The adapter contains no filesystem, process, network, scheduler, model, or
artifact port. Pinned prose can request those effects only by causing the host
to construct the typed plan above; it cannot grant authority itself.
