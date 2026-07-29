# bmad-create-epics-and-stories

- **rowId**: `method:src/bmm-skills/3-solutioning/bmad-create-epics-and-stories/SKILL.md`
- **canonicalId**: `bmm:bmad-create-epics-and-stories`
- **pinned source**: `packages/bmad-foundation/source/method/6.10.0/src/bmm-skills/3-solutioning/bmad-create-epics-and-stories/SKILL.md`

## What it does

A micro-file, step-sequenced workflow transforming PRD requirements and
architecture decisions into epics and stories organized by user value, with
complete acceptance criteria for the Developer agent. Activation resolves a
`workflow` customization block, greets the user, then hands off to four
just-in-time step files (`step-01-validate-prerequisites.md` through
`step-04-final-validation.md`), halting at every menu for user input. Step 4
validates FR coverage, architecture/starter-template compliance, story
quality, epic structure, and epic/story dependency direction, then saves the
final `epics.md`.

## Declared actions and modes

- **Actions**: none declared as a fixed, separately-invocable action --
  a single linear step-file progression. Catalog row records
  `action: null`.
- **Modes**: `interactive`, `headless` (per capability-catalog.json). **Gap:**
  same as `bmad-check-implementation-readiness` (MTH-13) -- neither
  `SKILL.md` nor any of its four step files names a headless path anywhere.
  `supported_modes` still declares both per the catalog-declared-modes
  requirement; reported as a fidelity gap.
- **Output location**: `{planning_artifacts}` (explicitly resolved in
  activation Step 4, "for output location and artifact scanning").
- **Artifact kind**: `document`. Step 4 ("Complete and Save") explicitly
  saves "the final epics.md" once all validations pass --
  `PersistArtifacts` completion hook.

## Compatibility decision

**adopt / exact.** No semantic divergence was found between the pinned
source and the harness descriptor; the headless-mode fidelity gap above is
a representational limit, not a behavioral mismatch this adapter
introduced.
