# bmad-deep-recon

- **rowId**: `method:src/core-skills/bmad-deep-recon/SKILL.md`
- **canonicalId**: `core:bmad-deep-recon`
- **pinned source**: `packages/bmad-foundation/source/method/6.10.0/src/core-skills/bmad-deep-recon/SKILL.md`

## What it does

Decision-grade research director offering three freely-combined services:
Draft a deep-research prompt for the user's own tool, Process a finished
report into a cited summary, or Run research here through parallel web
fan-out. Every engagement shares a run-folder workspace shape (`brief.md`,
`imports/`, `digests/`, `research.md`, `.memlog.md`) and two standing
epistemic rules: never conclude from training data alone, and project
context shapes queries but is never admissible evidence.

This rowId's catalog `aliases` list three legacy shims this consolidated
skill now serves: `bmm:bmad-domain-research`, `bmm:bmad-market-research`,
`bmm:bmad-technical-research`.

## Declared actions and modes

- **Actions**: none declared as a fixed, separately-invocable action.
  Catalog row records `action: null`. The four intents (Draft / Process /
  Run / Refresh-Deepen) are detected from the user's ask, not fixed actions.
- **Modes**: `interactive`, `headless` (per capability-catalog.json and the
  source's own `## Headless Mode` section).
- **Output location**: `research_output_path`
  (`{workflow.research_output_path}`).
- **Artifact kind**: `document` and `file_set`. The run-folder workspace
  mixes single markdown documents (`brief.md`, `research.md`, `.memlog.md`)
  with directories of multiple files (`imports/`, `digests/`); persistence
  is immediate and per-write -- `PersistArtifacts` completion hook.

## Compatibility decision

**adopt / exact.** No semantic divergence was found between the pinned
source and the harness descriptor.
