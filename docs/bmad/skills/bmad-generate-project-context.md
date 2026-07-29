# bmad-generate-project-context

- **rowId**: `method:src/bmm-skills/3-solutioning/bmad-generate-project-context/SKILL.md`
- **canonicalId**: `bmm:bmad-generate-project-context`
- **pinned source**: `packages/bmad-foundation/source/method/6.10.0/src/bmm-skills/3-solutioning/bmad-generate-project-context/SKILL.md`

## What it does

A micro-file, step-sequenced workflow producing a concise, LLM-optimized
`project-context.md` capturing critical implementation rules, patterns, and
guidelines for AI agents. Activation resolves a `workflow` customization
block, greets the user, then hands off to three just-in-time step files
(`step-01-discover.md` through `step-03-complete.md`), never proceeding to
the next step until the user approves continuation. The `## Paths` section
fixes the output file: `output_file = {output_folder}/project-context.md`.

## Declared actions and modes

- **Actions**: none declared as a fixed, separately-invocable action --
  a single linear step-file progression. Catalog row records
  `action: null`.
- **Modes**: `interactive`, `headless` (per capability-catalog.json). **Gap:**
  same as `bmad-check-implementation-readiness` (MTH-13) and
  `bmad-create-epics-and-stories` (MTH-14) -- neither `SKILL.md` nor any of
  its three step files names a headless path anywhere. `supported_modes`
  still declares both per the catalog-declared-modes requirement; reported
  as a fidelity gap.
- **Output location**: `{planning_artifacts}` (explicitly resolved in
  activation Step 4, "for output location and artifact scanning"). **Gap:**
  the concrete write target the source names, `{output_folder}/
  project-context.md`, is a distinct config variable from
  `{planning_artifacts}` -- the only variable `output_locations` can
  record. Both are reported here rather than silently picking one.
- **Artifact kind**: `document`. Step 3 ("Context Completion &
  Finalization") reviews, optimizes, and finishes the context file --
  `PersistArtifacts` completion hook.

## Compatibility decision

**adopt / exact.** No semantic divergence was found between the pinned
source and the harness descriptor; the two fidelity gaps above are
representational limits of the harness's single-field shape, not behavioral
mismatches this adapter introduced.
