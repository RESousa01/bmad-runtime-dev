# bmad-prd

- **rowId**: `method:src/bmm-skills/2-plan-workflows/bmad-prd/SKILL.md`
- **canonicalId**: `bmm:bmad-prd`
- **pinned source**: `packages/bmad-foundation/source/method/6.10.0/src/bmm-skills/2-plan-workflows/bmad-prd/SKILL.md`

## What it does

Facilitates creating, updating, or validating a PRD. Detects intent
(Create / Update / Validate) at activation, or honors it verbatim when
forwarded by a caller (e.g. `bmad-create-prd` / `bmad-edit-prd` /
`bmad-validate-prd` shims). Create walks Discovery (brain dump, stakes
calibration, working mode) then drafts `prd.md` with a `.memlog.md` audit
trail; Update reconciles the PRD against a change signal; Validate loads
`references/validate.md` for an inline critique. Finalize runs a reviewer
gate, triages open items, polishes, and marks `status: final`.

## Declared actions and modes

- **Actions**: none declared as a fixed, separately-invocable action.
  Catalog row records `action: null`. The source names three intent modes
  in prose (Create, Update, Validate) detected conversationally within one
  entry point, the same representational limit `bmad-product-brief`'s
  adapter documents for its own three modes. Reported as a fidelity gap.
- **Modes**: `interactive`, `headless` (per capability-catalog.json and
  `references/headless.md`).
- **Output location**: `{planning_artifacts}` (resolved in activation
  Step 3).
- **Artifact kind**: `document`. The source writes `prd.md` with YAML
  frontmatter, persisted as it is written and finalized in place --
  `PersistArtifacts` completion hook.
- **Predecessor/follower**: neither is declared. The only upstream
  reference is a misroute suggestion ("one-pager → `bmad-product-brief`"),
  and Finalize names three legitimate next skills side by side
  (`bmad-ux`, `bmad-architecture`, `bmad-create-epics-and-stories`) plus a
  router (`bmad-help`) rather than one fixed follower. Reported as a
  fidelity gap: the harness's single-`follower` field cannot represent an
  "any of these" set.

## Compatibility decision

**adopt / exact.** No semantic divergence was found between the pinned
source and the harness descriptor; the action-naming and follower-set gaps
above are representational limits of the harness's single-value fields,
not behavioral mismatches.
