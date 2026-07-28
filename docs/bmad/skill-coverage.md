# BMAD skill coverage — BM-02

**Pinned upstream:** `bmad-method` 6.10.0
**Denominator:** 48 upstream directories containing a real `SKILL.md`
**Machine-readable:** `docs/bmad/skill-coverage.json`
**Checked by:** `node tools/check-bmad-coverage.mjs --check`, wired into `verify:source`

## Why this exists

Audit finding **F-19**: the embedded snapshot holds 228 files against 597
upstream, so BMAD 6.10.0 parity cannot be claimed from a version string, a
folder name, or a green seal. Finding **F-20** sharpens it — the foundation seal
verifies the project's own managed corpus, not completeness against the pinned
distribution.

**F-21** is the one this file answers: without a written, checked matrix, the UI
can present a capability as available when it is a shim to a legacy name, or
count a normalized package that never reached capability closure.

## Coverage today

| Runtime state | Count | What it means |
|---|---:|---|
| `active_projection` | 19 | Forge exposes it, and the capability is active in the closure ledger |
| `compatibility_shim` | 12 | a legacy or Forge name routes to a canonical capability |
| `normalized_not_in_closure` | 7 | normalized into the package but not reachable as a capability |
| `not_projected` | 7 | present upstream, absent from Forge |
| `legacy_split_active` | 2 | upstream behaviour split across two active Forge paths |
| `upstream_shim_unprojected` | 1 | upstream itself is a shim, and Forge does not project it |
| **Total** | **48** | |

| Snapshot state | Count |
|---|---:|
| `identical` | 23 |
| `relocated_or_legacy` | 13 |
| `modified` | 9 |
| `missing` | 3 |

**19 of 48 are active projections.** That is the honest coverage number, and it
is the reason this file is not a summary sentence: "supports BMAD 6.10.0" would
be false, and "supports 40%" would hide that 12 of the remainder are shims with
different semantics rather than gaps.

## What a state does and does not assert

`snapshotState` describes the source path and hash relationship to the pinned
upstream. `runtimeState` describes what the current normalization and capability
closure expose. **Neither is a compatibility decision.**

The governed decision — one of `adopt`, `adapt`, `shim`, `defer`, `reject`, with
a named owner and semantic tests — is **BM-03**, which has not been recorded.
Every row therefore carries `decision: null`, and the checker treats null as a
state rather than an omission. A row claiming a decision before BM-03 exists
would assert a compatibility position nobody has taken.

## What the check enforces

- the denominator is 48, the pinned upstream discovery result, not however many
  rows someone wrote;
- every skill appears once, with one `snapshotState` and one `runtimeState` from
  the closed vocabularies;
- every capability a row names exists in
  `packages/bmad-foundation/capability-closure-ledger.json`;
- every `active_projection` names a capability that is **active** there — this
  is the check that stops the UI overstating availability;
- a `not_projected` row cannot name a capability;
- the recorded totals match the rows, so a stale summary fails rather than
  reassures.

## Placement, and one deviation from the plan

BM-02 lists `packages/bmad-foundation/scripts/coverage.mjs`,
`packages/bmad-foundation/skill-coverage.json` and
`packages/bmad-foundation/tests/coverage.test.mjs`. Those three are **not** at
those paths.

The foundation package is sealed: `foundation.test.mjs` asserts its exact root
entry list and that `package.json.files` deep-equals a reviewed constant, so
adding any file to it fails by design. Trying it failed nine tests. That seal is
a real invariant protecting a hash-locked corpus, and breaking it to satisfy a
path would trade something that matters for something cosmetic — so the checker
is at `tools/check-bmad-coverage.mjs`, the data beside this file, and the tests
at `tools/check-bmad-coverage.test.mjs`. This document is at the path BM-02 does
name.

## Next

**BM-03** turns each of these 48 rows into a governed decision. The six rows the
audit marks **P0** are where it starts: `bmad-prd`, `bmad-spec`, `bmad-ux`, and
the three others the matrix flags — each needs a named owner, a recorded reason,
and semantic tests before its state can be called a decision.
