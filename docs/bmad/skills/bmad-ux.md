# bmad-ux

- **rowId**: `method:src/bmm-skills/2-plan-workflows/bmad-ux/SKILL.md`
- **canonicalId**: `bmm:bmad-ux`
- **pinned source**: `packages/bmad-foundation/source/method/6.10.0/src/bmm-skills/2-plan-workflows/bmad-ux/SKILL.md`

## What it does

Facilitates UX design, producing two peer contracts: `DESIGN.md` (visual
identity, per the Google Labs design.md spec) and `EXPERIENCE.md`
(information architecture, behavior, states, interactions, accessibility,
journeys). Detects Create / Update / Validate intent at activation. Create
walks Discovery (source scan, brain dump, working mode, journeys, surface
closure) then distills both spines at Finalize; Update reconciles against
the memlog; Validate loads `references/validate.md`. Finalize also renders
key-screen mocks, runs an opt-in reviewer gate, and marks both files
`status: final`.

## Declared actions and modes

- **Actions**: none declared as a fixed, separately-invocable action.
  Catalog row records `action: null`. Activation Step 5 ("Detect intent:
  Create, Update, Validate") is conversational intent detection at one
  entry point, the same representational limit `bmad-prd`'s adapter
  documents for its own three modes.
- **Modes**: `interactive`, `headless` (per capability-catalog.json and
  `references/headless.md`).
- **Output location**: `{planning_artifacts}` (resolved in activation
  Step 3).
- **Artifact kind**: `document`. Both `DESIGN.md` and `EXPERIENCE.md` are
  markdown documents persisted as the run proceeds -- `PersistArtifacts`
  completion hook. Both share the one declared `document` kind rather than
  being double-counted as two kinds.
- **Predecessor/follower**: neither is declared. The source states "UX may
  lead, follow, or stand alone", and Finalize names three legitimate next
  skills side by side (`bmad-architecture`, `bmad-create-epics-and-stories`,
  `bmad-dev-story`) -- the same non-fixed-ordering shape `bmad-prd`'s
  adapter documents.

## Compatibility decision

**adopt / exact.** No semantic divergence was found between the pinned
source and the harness descriptor.
