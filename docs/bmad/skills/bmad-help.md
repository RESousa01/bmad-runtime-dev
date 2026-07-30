# bmad-help

- **rowId**: `method:src/core-skills/bmad-help/SKILL.md`
- **canonicalId**: `core:bmad-help`
- **pinned source**: `packages/bmad-foundation/source/method/6.10.0/src/core-skills/bmad-help/SKILL.md`

## What it does

Helps the user understand where they are in their BMad workflow and what
to do next: reads the installed skill CSV catalog, resolved config, and
project artifacts, fuzzy-matches completion, and recommends the next step
with invocation details. Also answers general questions, fetching module
documentation (`_meta` catalog rows) when the question doesn't map to a
specific skill.

## Declared actions and modes

- **Actions**: one source-derived `recommend` execution step. The public
  catalog still records `action: null` because Help is invoked directly;
  recommendation routing happens inside its canonical execution contract.
- **Modes**: `interactive`, `headless`.
- **Output location**: none. The deliverable is the in-chat orientation
  response; the CSV catalog, resolved config, and project artifacts are
  inputs this skill reads, not outputs it writes.
- **Artifact kind**: none.
- **Typed host intents**: `file_read`, `process`, `web`, and `subagent`.
  These compile to inert plans. Web/browser/external-handoff authority remains
  separately governed and is never inferred from source prose.

## Compatibility decision

**adopt / adapted, runtime available.** The canonical adapter closes the five
pinned source members (Help entrypoint, both module catalogs, and both module
metadata files) and exposes an explicit generic Method execution contract.
Production preparation compiles the contract through `prepare_skill_execution`
and binds it durably with `MethodSessionService` before context review.

`crates/desktop-runtime/src/bmad/help.rs` remains the deterministic catalog
engine for availability, next-step, alias, capability, and catalog-count
questions. The generic execution contract adds source-faithful preparation for
the open-ended recommendation path without adding a handwritten registry or a
parallel lifecycle. Generated catalog and Help graph projections are the one
availability/topology truth consumed by the desktop library and Help loader.

The row remains `adapted`, not `exact`, because mutable upstream documentation
fetches and unrestricted source-directed tools are replaced by bounded host
inputs and explicit unavailable/authority facts.

## Durable lifecycle

Both modes persist `CapabilityBound` before context review and use the existing
Method lifecycle for cancel, classified fail/retry/repair, restart/resume, and
stale-binding upgrade. No Help-specific session store or executor exists.
