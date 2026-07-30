# bmad-check-implementation-readiness

- **rowId**: `method:src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/SKILL.md`
- **canonicalId**: `bmm:bmad-check-implementation-readiness`
- **pinned source**: `packages/bmad-foundation/source/method/6.10.0/src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/SKILL.md`

## What it does

A micro-file, step-sequenced workflow validating that PRD, UX, Architecture,
Epics, and Stories are complete and aligned before Phase 4 implementation
starts. Activation resolves a `workflow` customization block, greets the
user, then hands off to six just-in-time step files
(`step-01-document-discovery.md` through `step-06-final-assessment.md`),
halting at every menu for user input. Step 6 appends a final
"Summary and Recommendations" section (overall readiness status, critical
issues, recommended next steps) and saves the completed report.

## Declared actions and modes

- **Actions**: none declared as a fixed, separately-invocable action --
  a single linear step-file progression. Catalog row records
  `action: null`.
- **Modes**: `interactive`, `headless` (per capability-catalog.json). The
  pinned source itself only defines the interactive menu path. Forge's
  headless mode is an explicit adaptation: the caller supplies reviewed,
  bounded answers for every one of the same six checkpoints. It cannot skip a
  checkpoint or let the adapter invent an answer.
- **Output location**: `{planning_artifacts}` (explicitly resolved in
  activation Step 4, "for output location and artifact scanning").
- **Artifact kind**: `document`. Step 6's own frontmatter fixes the output
  path: `outputFile: '{planning_artifacts}/implementation-readiness-report-
  {{date}}.md'`, saved as the final step -- `PersistArtifacts` completion
  hook.

## Semantic diff

| Field | Pinned Method 6.10.0 | Forge 0.1.x | Policy |
| --- | --- | --- | --- |
| Invocation mode | Interactive menus only | Interactive plus bounded headless | Headless must supply one reviewed answer per source checkpoint. |
| Menu pauses | Each step halts for user input | Interactive preserves pauses; headless receives the equivalent answers in one sealed host envelope | Missing/empty answers fail closed. |
| Output | Six accumulated sections ending in Summary and Recommendations | The same six headings, order, Markdown body, output location, and dated filename | Interactive and headless output equivalence is executable-tested. |
| Effects | Source prose requests reads, writes, Python resolution, and `bmad-help` | Typed read/write/command/artifact plans and a generated `bmm:bmad-help` follower edge | Source text grants no runtime authority. |
| Recovery | Step files and output frontmatter imply resumable progression | Durable Method checkpoints bind stage, answers, artifact expectations, source hashes, model binding, and approvals | Cancel/fail/retry/rebind/restart/resume/stale-source branches are persisted-test obligations. |

## Compatibility decision

**adapt / adapted.**

- **Owner:** Forge BMAD Runtime / MTH-13.
- **Supported versions:** `bmad-method@6.10.0`; `forge@0.1.x`.
- **Migration policy:** re-review and regenerate the adapter, fixtures, Help
  topology, persisted exact binding, and this field-level diff whenever the
  pinned Method version, six-step workflow, mode contract, or Forge 0.1
  execution contract changes.
