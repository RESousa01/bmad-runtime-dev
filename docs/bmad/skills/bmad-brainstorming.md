# bmad-brainstorming

- **rowId**: `method:src/core-skills/bmad-brainstorming/SKILL.md`
- **canonicalId**: `core:bmad-brainstorming`
- **pinned source**: `packages/bmad-foundation/source/method/6.10.0/src/core-skills/bmad-brainstorming/SKILL.md`

## What it does

Runs a brainstorming session in one of three stances (Facilitator, Creative
Partner, Ideate for me). Interactive sessions can compose the stance and
technique batch via a generated `brain-selector.html` page or in chat;
headless runs generate ideas autonomously per `references/headless.md`.
Every idea, decision, question, and direction is logged one line at a time
to an append-only memlog, the session's single source of truth. Wrap-up
loads `references/finalize.md` to synthesize and persist artifacts.

## Declared actions and modes

- **Actions**: none declared as a fixed, separately-invocable action.
  Catalog row records `action: null`. Stance and technique batch are chosen
  conversationally or via the composer page, not a fixed action.
- **Modes**: `interactive`, `headless` (per capability-catalog.json and the
  source's own `## On Activation` step 4/5 split).
- **Output location**: `{workflow.output_dir}` (the root `{doc_workspace}`
  binds under).
- **Artifact kind**: `document`. The memlog (`.memlog.md`) and Wrap-Up
  synthesis are both markdown, persisted as they are written --
  `PersistArtifacts` completion hook.

## Compatibility decision

**adopt / exact.** No semantic divergence was found between the pinned
source and the harness descriptor.
