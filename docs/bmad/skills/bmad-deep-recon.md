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

- **Actions**: `draft`, `process`, `run`, `refresh`, `deepen`. The catalog
  retains `action: null` because these are branches of one canonical skill,
  while the runtime binds the selected action to a distinct source-derived
  step table.
- **Modes**: `interactive`, `headless` (per capability-catalog.json and the
  source's own `## Headless Mode` section).
- **Output location**: `research_output_path`
  (`{workflow.research_output_path}`).
- **Artifact kind**: `document` and `file_set`. The run-folder workspace
  mixes single markdown documents (`brief.md`, `research.md`, `.memlog.md`)
  with directories of multiple files (`imports/`, `digests/`); persistence
  is immediate and per-write -- `PersistArtifacts` completion hook.
- **Follower**: `core:bmad-help`, matching the source finalize instruction
  that asks Help to suggest the next step.

## Compatibility decision

**adapt / exact.** The pinned research semantics, five services, topology,
and recovery intent are preserved exactly. Source-directed filesystem,
process, web, browser, subagent, and external-handoff operations are
translated into typed host-governed plans; the source text itself grants no
runtime authority.
