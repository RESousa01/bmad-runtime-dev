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
- **Modes**: `interactive`, `headless` (per capability-catalog.json). **Gap:**
  neither `SKILL.md` nor any of its six step files names a headless path
  anywhere -- every step instead halts at a menu and states content is
  never generated without user input. `supported_modes` still declares both
  because the harness must "declare its catalog-declared modes", but this is
  reported as a fidelity gap the descriptor cannot otherwise represent.
- **Output location**: `{planning_artifacts}` (explicitly resolved in
  activation Step 4, "for output location and artifact scanning").
- **Artifact kind**: `document`. Step 6's own frontmatter fixes the output
  path: `outputFile: '{planning_artifacts}/implementation-readiness-report-
  {{date}}.md'`, saved as the final step -- `PersistArtifacts` completion
  hook.

## Compatibility decision

**adopt / exact.** No semantic divergence was found between the pinned
source and the harness descriptor; the headless-mode fidelity gap above is
a representational limit (catalog declares a mode the source text never
describes), not a behavioral mismatch this adapter introduced.
