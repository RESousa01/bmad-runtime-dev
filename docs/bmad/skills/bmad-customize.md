# bmad-customize

- **rowId**: `method:src/core-skills/bmad-customize/SKILL.md`
- **canonicalId**: `core:bmad-customize`
- **pinned source**: `packages/bmad-foundation/source/method/6.10.0/src/core-skills/bmad-customize/SKILL.md`

## What it does

Translates a user's stated intent into a correctly-placed TOML override
file under `{project-root}/_bmad/custom/` for a customizable agent or
workflow skill: discover, route, author, write, verify. Scope v1 covers
per-skill `[agent]` and `[workflow]` overrides; central config is out of
scope. Shows the full TOML (or a diff), waits for explicit confirmation,
writes, then verifies via the resolver script or a manual layered-merge
fallback.

## Declared actions and modes

- **Actions**: none declared as a fixed, separately-invocable action.
  Catalog row records `action: null`. Step 1's intent classification
  (Directed / Exploratory / Audit-iterate / Cross-cutting) is conversational
  routing, not a fixed action.
- **Modes**: `interactive`, `headless` (per capability-catalog.json).
- **Output location**: `_bmad/custom` (`{project-root}/_bmad/custom/`, per
  Step 5).
- **Artifact kind**: `file_set`. The written artifact is a TOML
  configuration override file, not a narrative document; the write is
  persisted immediately -- `PersistArtifacts` completion hook.

## Compatibility decision

**adopt / exact.** No semantic divergence was found between the pinned
source and the harness descriptor.
