# bmad-prfaq

- **rowId**: `method:src/bmm-skills/1-analysis/bmad-prfaq/SKILL.md`
- **canonicalId**: `bmm:bmad-prfaq`
- **pinned source**: `packages/bmad-foundation/source/method/6.10.0/src/bmm-skills/1-analysis/bmad-prfaq/SKILL.md`

## What it does

Runs Amazon's Working Backwards PRFAQ challenge: a five-stage coaching flow
(Ignition, Press Release, Customer FAQ, Internal FAQ, Verdict) that
stress-tests a product concept customer-first before it is built. Accepts
`--headless` / `-H` for autonomous first-draft generation from provided
context; default is full interactive coaching.

## Declared actions and modes

- **Actions**: none declared as a fixed, separately-invocable action.
  Catalog row records `action: null`; the `--headless`/`-H` flag is a mode
  selector, already captured by `supported_modes`.
- **Modes**: `interactive`, `headless` (per capability-catalog.json and
  explicitly named in the source's "Args" line).
- **Output location**: `{planning_artifacts}` (explicitly resolved in
  activation Step 4).
- **Artifact kind**: `document`. The source explicitly names the produced
  artifact -- "Create the output document at
  `{planning_artifacts}/prfaq-{project_name}.md` ... This document is the
  working artifact" -- and persists it as it is written
  (`PersistArtifacts` completion hook).

## Compatibility decision

**adopt / exact.** No semantic divergence was found between the pinned
source and the harness descriptor.
