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

- **Actions**: none declared as a fixed, separately-invocable action.
  Catalog row records `action: null`. "What to do next" is derived by
  ranking the catalog against the user's query, not a fixed action.
- **Modes**: `interactive`, `headless` (per capability-catalog.json).
- **Output location**: none. The deliverable is the in-chat orientation
  response; the CSV catalog, resolved config, and project artifacts are
  inputs this skill reads, not outputs it writes.
- **Artifact kind**: none.

## Compatibility decision

**adopt / adapted.** `crates/desktop-runtime/src/bmad/help.rs` already
implements a runtime Help engine, and it deliberately narrows the pinned
source's surface:

- It is model-free by construction (a structural scan forbids any
  model/chat reference), while the pinned source's "fetch and use \[module
  docs\] to answer general questions" and open-ended-question handling are
  explicitly model-driven.
- It answers a fixed set of typed queries (availability, next-steps, alias
  resolution, capability projection, catalog counts) rather than arbitrary
  free-form questions.
- Its own `BmadHelpSupportState` (Exact/Adapted/Shim) is a narrower-scoped,
  deliberately distinct type from this harness's `BmadSkillSupportState`
  (Exact/Adapted/Shim/Unavailable); see `help.rs`'s doc comment on
  `BmadSkillSupportState` for why the two were not unified.

The deterministic subset `help.rs` covers matches the pinned source
faithfully; the remote-doc-fetch and general-question-answering outcomes
are genuinely out of scope for it, which is why this row reports `adapted`
rather than `exact`.
