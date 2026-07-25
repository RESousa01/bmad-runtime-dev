# Forge v2 remediation self-audit (2026-07-24)

**Scope:** a deep-dive review of the 2026-07-24 remediation pass that implemented three
audit scopes — host concurrency/durability (coordinator lock), task/session identity
(first slice), and BMAD honesty (fidelity matrix + labels). This audits the *new work*,
not the whole candidate. Baseline: [`forge-v2-comprehensive-audit-2026-07-23.md`](forge-v2-comprehensive-audit-2026-07-23.md).

**Verdict:** the remediation holds up, **after one real concurrency defect was found and
fixed** (below). The renderer isolation and the BMAD honesty work are correct within their
declared first-slice scope. Several honest gaps remain and are named, not hidden.

## Critical — found and fixed

### C1. Dispatch misattribution race introduced by the lock release

**Where:** `crates/desktop-app/src/bmad_model/capability_coordinator.rs`.

**Defect:** Scope 2 releases the coordinator mutex during `transport.send`. That is correct
for revocation liveness, but it opened a window the old lock-held-throughout code could not
hit. `invalidate()` flips `Dispatching → Terminal(Invalidated)`, which lets a *fresh*
`prepare → approve → begin_dispatch` reach `Dispatching` before the first dispatch's caller
returns. Both `finalize_dispatch` and `abort_dispatch` then acted on **whatever** `Dispatching`
was present:

- a stale `finalize_dispatch` would verify the first dispatch's output against, and record it
  under, the **second** run's metadata (wrong-run attribution), or
- a stale `abort_dispatch` would terminate the **second**, still-live dispatch.

**Fix:** each dispatch is stamped with a monotonic `dispatch_id` (`begin_dispatch` returns it;
the caller quotes it back). `finalize_dispatch(dispatch_id, …)` and
`abort_dispatch(dispatch_id, …)` now act only when the current `Dispatching` carries the same
id; a superseded dispatch is dropped as `Conflict` and leaves the newer dispatch intact.

**Test:** `a_superseded_dispatch_cannot_finalize_or_abort_a_newer_run` reproduces the
interleaving and asserts the stale finalize is dropped, the stale abort is a no-op, and only
the second run is recorded. It fails if the guard is removed.

## Watch — real gaps, not fixed this pass

- **W1. Stuck `Dispatching` on caller death.** If the caller panics or drops between
  `begin_dispatch` and finalize/abort, the run stays `Dispatching` and blocks new capability
  ops until `invalidate()`. Acceptable for the current **synchronous** transport (no async
  cancellation), but it is a durability edge to revisit if the transport becomes async.
- **W2. Renderer isolation is capability-lane only.** Only `capabilityRun` is task-owned.
  `bmadHelpState`, `changesFlow`, `contextPreview`, and `currentBmadIntent` remain app-global,
  so a slow BMAD Help / changes completion can still surface under the wrong tab. This is the
  declared first-slice boundary; the same per-task pattern extends to those lanes next.
- **W3. App-level cross-tab test — added (isolation).** `App.test.tsx` now drives a real
  capability run on task A through the full App (open the skills drawer, "Start reviewed run"),
  creates a fresh task B and asserts the run does **not** appear there with the drawer open, then
  returns to task A and asserts the run is restored — proving per-task isolation through the
  actual App wiring (`runnableCapability` fixture option on `bmadHelpRuntime`). Still open: an
  App-level drive of the full async completion (prepare → approve → submit → latest with a
  mismatched run id) to exercise the C2 guard end-to-end; that path is covered by the hook-level
  routing test and the store tests, and the full suite is 412 green.
- **W4. Help/Architecture conflict is documented, not fixed.** The sealed Help binding still
  recommends Architecture, which is fail-closed. Only
  [`bmad-capability-fidelity-matrix.md`](bmad-capability-fidelity-matrix.md) reconciles it; the
  binding is hash-verified foundation data and is deferred to the Help milestone.
  > **Resolved 2026-07-25:** Architecture is no longer fail-closed — it runs as an
  > ordered, host-validated progression
  > ([P9](../implementation-packets/P9-architecture-state-machine-2026-07-25.md)), so the
  > Help recommendation is actionable. The *catalog-breadth* half of the Help gap (two
  > actions against 24 capabilities) is unchanged and still deferred.

## Info — observations, low severity

- **I1. Fidelity map is a renderer-local hardcode** (`bmadFidelity.ts`), not cross-checked
  against `capability-closure-ledger.json`. Drift can only *miss a specific note*: the default
  is conservative (`adapted_one_shot`), never a false `faithful`, so it cannot produce a
  dishonest overclaim.
- **I2. The `Unavailable`/`Inert draft` badge branches are production-dead.** The badge renders
  only for `available` menus; unavailable capabilities show the existing `AvailabilityChip`
  instead. Those classifications exist for the matrix doc, not the runtime badge.
- **I3. `cargo fmt` reformatted ~27 unrelated Rust files** carrying the audit's uncommitted
  in-flight edits. Semantically safe, canonical under the pinned rustfmt 1.9.0 (rustc 1.97.0),
  and required for the fmt gate, but it widened the diff beyond the functional changes.

## Verification record (post-fix)

| Check | Result |
|---|---|
| `desktop-app` tests `--all-features --jobs 1` | Pass — **120** (adds `a_superseded_dispatch_…`) |
| `desktop-app` clippy `--all-targets --all-features -D warnings` | Pass |
| `desktop-ipc` tests | Pass |
| `cargo fmt --check` | Clean |
| Renderer `test` | Pass — 410 / 39 files (unchanged this round) |
| Renderer `typecheck` (lint) | Pass |
| Foundation verify | Pass — 116 tests, 114 members |
| Full `pnpm verify:source` | Not run — environment is Node 24.11.1 vs pinned 24.18.0; `toolchain:check` fails first, later steps need Docker |

## Second pass (deeper)

A second, more adversarial review found one more real defect (fixed), a low-severity leak, and
confirmed one design choice is sound.

### C2. Cross-task result-*content* leak via capability-keyed fetch — found and fixed

**Where:** `apps/desktop-ui/src/App.tsx` `submitCapabilityRun`; root cause in
`crates/desktop-store/src/bmad_capability.rs` `latest_bmad_capability_run`.

**Defect:** after submitting, the renderer fetched the payload with
`latestCapabilityRun(capabilityId)`, whose store query is
`… WHERE workspace_id=?1 AND capability_id=?2 ORDER BY created_at_ms DESC LIMIT 1` — keyed by
**capability**, newest run wins. Before the Scope 1 change only one global capability run could
exist, so this was unambiguous; **per-task isolation newly makes two concurrent runs of the same
capability reachable**, so task A's completion could display task B's result content (right tab,
wrong payload). The routing put the completion on the correct task, but the content came from a
capability-scoped "latest" query.

**Fix:** both projections already carry `runId`. The guard is now an extracted, unit-tested pure
helper `latestResultForRun(latest, submittedRunId)` (`capabilityProtocol.ts`) — used by
`submitCapabilityRun` and covered by three tests including the mismatched-concurrent-run case, so
the fix has regression protection rather than living as an untested inline conditional. A
run-keyed result command would be a cleaner host-side follow-up.

### W5. Per-task store grew unbounded — fixed

The `runs`/`generations` maps gained one entry per task created and were never pruned (the
`closeCapabilitySession` hook method was unwired). **Fixed** by a self-contained cap in `openRun`:
`MAX_LANE_SESSIONS = 64`, evicting the oldest sessions (Map insertion order, never the newest) —
far above the ~20 visible tasks, so evicted entries are long gone from view, and a late result
for an evicted session drops naturally. Test: `caps retained sessions, evicting the oldest…`.

### Confirmed sound — no resurrection after cancel/clear

Every route updater guards `current === null ? null : …`, and generation bumps only on
`openActiveCapabilityRun`. So after a cancel or `clearAll` sets a run to null, a late completion
for the same generation is dropped by the null-guard (it does not resurrect the cleared run), and
a new run started on the same task advances the generation so the stale completion drops. The
generation + null-guard pair is robust; this is intentional, not accidental.

### I4. Fidelity badge tooltip is not keyboard/SR-accessible

The badge is a `<span title="…">`; the `title` note is not reachable by keyboard or reliably by
screen readers. The visible label text ("Adapted") *is* readable, so the honesty signal survives,
but the explanatory note is mouse-hover-only. Consider an accessible tooltip if this feeds the
accessibility gate.

### W2 — changes lane now per-task (context + BMAD-Help remain)

The **changes lane is now per-task.** `changesFlow` is stored per task
(`changesFlowByTask`), and `proposeCapabilityChangeSet` / `decideGovernedChange` /
`undoGovernedChange` capture the originating task and route their result to it, so a slow
change-set review lands on the task it was started from rather than the visible tab. The shared
`changesGenerationRef` is deliberately kept as the changes-subsystem coordinator (history, undo,
enable), so concurrent *cross-task* changes ops **over-drop rather than mis-display** — strictly
safer than the previous single global flow, at the cost of dropping a rare concurrent op. Idle/null
entries are collapsed out of the map to keep it bounded. Global resets (workspace switch, enable,
host invalidation) clear all tasks; `onStartNewProposal` resets only the active task. 411 renderer
tests stay green.

**Still global:** `contextPreview` and `bmadHelpState`.

**bmadHelp lane — attempted, reverted (documented negative result).** A low-risk *render-gate*
shortcut was tried: bind the single in-flight flow to its origin task and show its state only
there. It fails because `bmadHelpState` is handed to `TaskWorkspace` as `methodGuidanceState`,
which drives the **entire** method-guidance lifecycle — the composer, its in-flight/availability
UI, *and* the result — not just the result display. Gating it wholesale broke the composer on
tasks whose bound-task binding is ambiguous (idle / recovery / unavailable states), failing five
existing tests. A correct fix requires **true per-task `bmadHelpState`** (a per-task map plus its
synchronous mirror) threaded through all 27 `applyBmadEvent` sites, recovery, and the projection
poll — a dedicated milestone, not a render-layer patch. The attempt was reverted; the tree is
412-green.

The capability and changes cross-tab routing is verified by the hook/store tests and, for
capability, an App-level isolation drive; the changes lane still lacks an App-level cross-tab
test.

## Verification record (second pass)

| Check | Result |
|---|---|
| `desktop-app` tests / clippy / fmt | Pass — 120 tests (C1 unchanged) |
| Renderer `test` (post run-id guard) | Pass — 410 / 39 files |
| Renderer `typecheck` | Pass |

## Decision

Sound after C1 and C2. The highest-value next steps: extend per-task state to the remaining
renderer lanes (W2) with an App-level cross-tab test (W3, which would also regression-test the
C2 guard), wire `closeCapabilitySession` on eviction (W5), and — separately — the deferred
host-issued session identity milestone (the Agent-session command lane remains a deliberate stub).
