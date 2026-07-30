# BMAD 100% frozen-plan resume guide

Date frozen: 2026-07-30

## Freeze policy

Implementation is stopped after the already-started Method branches are
preserved. No untouched backlog task should be started until the owner
explicitly resumes this plan.

Do not delete or rewrite the following worktrees until their changes are
confirmed in the integration history:

- `C:\tmp\bmad100-mth08-final`
- `C:\tmp\bmad100-mth09-final`
- `C:\tmp\bmad100-mth10`
- `C:\tmp\bmad100-mth12`
- `C:\tmp\bmad100-mth13`
- `C:\tmp\bmad100-mth38`
- `C:\tmp\bmad100-mth40`
- `C:\tmp\bmad100-mth42`

Do not push, deploy, sign, publish, or release solely because these branches
have been merged locally.

## Authoritative references

- Plan: `.tmp/plan/sapphirus-bmad-100-completion-plan-v1/15-full-plan.md`
- Backlog:
  `.tmp/plan/sapphirus-bmad-100-completion-plan-v1/12-implementation-backlog.json`
- Execution manifest:
  `.tmp/plan/sapphirus-bmad-100-completion-plan-v1/agent/execution-manifest.json`
- Campaign state: `.bmad100/run-state.json`
- Progress ledger: `.superpowers/sdd/15-full-plan/progress.md`
- Project context:
  `C:\Users\rodri\source\BigBrain\03-projects\sapphirus-bmad-runtime.md`

## Resume sequence

Resume one task at a time. Do not fan out from an unreviewed shared runtime
checkpoint.

### 1. Reconcile authoritative state

1. Preserve the `ENG-06` baseline tree:
   `c055413d52514e6ca13b99a1d43c7a72d18f5d45`.
2. Validate the accepted MTH-08 receipt with
   `tools/bmad100/update-run-state.mjs`.
3. Transition MTH-08 to passed and remove its active lease only after the
   repository evidence tree matches the receipt.
4. Record the resulting next-ready set.

### 2. Finish the partial Method tasks

Recommended order:

1. MTH-09 — correct source-faithful Spec outputs, provenance, and receipt.
2. MTH-10 — finish UX gates, receipt, commit, and review.
3. MTH-12 — wire production Architecture dispatch and executable actions,
   then regenerate the receipt.
4. MTH-13 — finish the existing production/lifecycle repair, gates, receipt,
   and review.
5. MTH-38 — finish Deep Recon production and lifecycle behavior.
6. MTH-40 — validate the clean candidate with an independent review and
   canonical updater receipt.
7. MTH-42 — finish Review production/lifecycle behavior and evidence.

For every task:

- begin from the latest independently accepted branch;
- prefer verbatim, hash-locked upstream BMAD content when the plan permits;
- add only the minimum Sapphirus persistence, authority, and projection
  adapter;
- run the task's exact five commands;
- run affected full regressions;
- bind command evidence to the implementation tree;
- obtain independent specification and quality approval;
- update run-state only after approval.

### 3. Resume untouched work

After all seven partial Method tasks pass:

1. complete `ENG-17` if its dependency position requires it;
2. continue the remaining Method tasks in topological order;
3. continue Builder tasks;
4. continue production UI tasks;
5. continue security/operations tasks;
6. continue qualification and release tasks.

Use
`.tmp/plan/sapphirus-bmad-100-completion-plan-v1/evidence/topological-order.txt`
as the ordering authority.

## Known failure patterns to avoid

- Do not treat a descriptor, catalog row, or availability predicate as an
  executable capability.
- Do not keep Method sessions as prepare-time sidecars while approve,
  cancel, and submit use a legacy lifecycle.
- Do not infer lifecycle completion from static golden JSON.
- Do not use a custom receipt shape ignored by
  `update-run-state.mjs`.
- Do not run clean-tree gates against ignored or untracked source bytes.
- Do not let Help, UI, runtime descriptors, catalogs, or ledgers describe
  different output topology.
- Do not copy executable upstream behavior across Sapphirus authority
  boundaries without explicit governed plans.
- Do not use a shared Cargo target across concurrent worktrees.
- Pin Node `24.18.0` and pnpm `11.15.1` before trusting foundation results.

## External and host constraints

- Windows/Tauri builds intermittently failed with `Access is denied`.
- Parallel Cargo builds exhausted the C: drive; disposable caches were
  cleaned, but future work should use task-local targets and
  `CARGO_INCREMENTAL=0`.
- Real Azure, installer, signing, release, and pilot evidence remain
  separate from local implementation evidence.

## Definition of a safe restart

A restart is safe when:

- the integration branch and all preserved worktrees are clean or their WIP
  snapshots are committed;
- `.bmad100/run-state.json` validates;
- the latest accepted task has a clean independent review;
- the next task starts from that accepted tree;
- no unrelated Foundry changes are included in the BMAD task diff.

