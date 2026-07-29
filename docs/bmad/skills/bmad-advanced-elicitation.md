# bmad-advanced-elicitation

- **rowId**: `method:src/core-skills/bmad-advanced-elicitation/SKILL.md`
- **canonicalId**: `core:bmad-advanced-elicitation`
- **pinned source**: `packages/bmad-foundation/source/method/6.10.0/src/core-skills/bmad-advanced-elicitation/SKILL.md`

## What it does

BMad's shared refinement checkpoint. Operates on the most recent output in
the conversation (or whatever the caller/user points at), serves a numbered
menu of elicitation methods drawn from `scripts/pick_methods.py`'s catalog,
runs the chosen methods against the target, and hands the improved version
back in place so the invoking flow resumes exactly where it paused.

## Declared actions and modes

- **Actions**: none declared as a fixed, separately-invocable action.
  Catalog row records `action: null`. The menu (`1-5`, `r`, `a`, `x`) is
  served dynamically from the method catalog, not a fixed action.
- **Modes**: `interactive`, `headless` (per capability-catalog.json).
- **Output location**: none. No `{doc_workspace}` or persisted file is
  named anywhere in the source -- the improved version is handed back
  conversationally.
- **Artifact kind**: none.

## Compatibility decision

**adopt / exact.** No semantic divergence was found between the pinned
source and the harness descriptor.
