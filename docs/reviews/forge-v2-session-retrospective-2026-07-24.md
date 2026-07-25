# Forge v2 — retrospective audit of the 2026-07-24 remediation session

**Scope:** every fix shipped in this session, re-verified from source, plus the *interactions*
between fixes and the stale-closure risk introduced by the new hooks. Companion documents:
[remediation audit](forge-v2-remediation-audit-2026-07-24.md),
[frontend audit](forge-v2-frontend-audit-2026-07-24.md),
[fidelity matrix](bmad-capability-fidelity-matrix.md).

**Verdict:** all shipped fixes verified correct. Two latent issues were found in this pass — both
introduced by the session's own work — and both hardened. No defects remain open in the shipped
surface.

## Fix-by-fix verification

| # | Fix | What it addresses | Verified | Coverage |
|---|---|---|---|---|
| 1 | Coordinator dispatch split (`begin`/`finalize`/`abort` + `Dispatching`) | Mutex was held across the model round trip, blocking sign-out/revocation | Call site re-read: authority+store held only for the finalize write; network runs unlocked | 2 tests |
| 2 | **C1** dispatch nonce (`dispatch_id`) | Lock release let a superseded dispatch finalize/abort a *newer* run | Guarded in both `finalize_dispatch` and `abort_dispatch`; all callers consistent | 1 dedicated test (fails if guard removed) |
| 3 | Per-task capability store + hook (`taskSessions.ts`) | Completions landed on the visible tab, not the origin task | Pure store + ref-mirror hook; StrictMode-safe | 9 store/hook tests + App-level test |
| 4 | **C2** run-id result guard | Capability-keyed "latest" query could return another task's payload | Extracted to `latestResultForRun` | 3 tests incl. mismatch case |
| 5 | BMAD fidelity matrix + badge | Runnable capabilities presented as full upstream workflows | Classifier defaults are conservative (never a false "faithful") | 4 tests + 1 render test |
| 6 | **F1** context in-flight reset | `browseAndAttach` bumped a shared generation, stranding auto-select | Corroborated: `invalidateWorkspaceBoundUi` already resets this ref unconditionally | Existing suite |
| 7 | **W5** capability store cap | One unpruned entry per task created | `MAX_LANE_SESSIONS`, evicts oldest, never the newest | 1 test |
| 8 | Per-task `changesFlow` | Slow change-set reviews landed on the visible tab | Origin captured per op; shared `changesGenerationRef` preserved | Existing changes tests |
| 9 | App-level cross-tab test | No end-to-end proof of task isolation | Drives the real App: run on A → absent on B → restored on A | 1 test |
| 10 | `openBuilderCapabilityRun` dedup | Byte-identical dead duplication | Both panel props share one handler | Existing suite |

**Reverted, documented:** the bmadHelp render-gate. `bmadHelpState` reaches `TaskWorkspace` as
`methodGuidanceState`, which drives the whole lifecycle (composer + in-flight UI + result), so
gating it broke the composer and five tests. Correct fix = true per-task state through all 27
`applyBmadEvent` sites — a dedicated milestone.

## Findings from this pass (both hardened)

### R1 — Latent stale-closure trap around the capability hook

`invalidateHostBoundUi` is a `useCallback` that called `capabilitySessions.clearAllCapabilityRuns()`
without listing `capabilitySessions` in its dependencies, capturing a **stale hook object**.

*Not a live defect:* `clearAllCapabilityRuns` is session-independent and referentially stable
(`[commit]` → `[]`), and reads `storeRef.current`, which is always fresh.

*The trap:* the same stale object also carries `openActiveCapabilityRun`, `captureActive`, and
`setActiveCapabilityRun`, which are bound to `activeSessionId` (`[activeSessionId, commit]`). Any
future edit reaching for one of those inside that callback would write to the **wrong task** — the
exact bug class this session fixed (C1/C2/F1: shared/stale coordination state).

*Hardening:* the stable member is now destructured (`const { clearAllCapabilityRuns } = …`) and
listed in the dependency array, with a comment stating the invariant. Identity stays stable, so
nothing ripples into `adoptHostRuntime` or the projection-poll effect; a future author reaching for
a session-bound member must now confront the dependencies.

### R2 — Changes map repeated the growth pattern W5 fixed

The per-task `changesFlowByTask` map collapses idle/null entries, but **terminal** flows
(`applied` / `discarded` / `undo_unavailable`) persist and were never pruned when a task fell out
of the 20-item recents list — the same unbounded growth fixed for the capability store in W5, and
introduced by this session's own later work.

*Hardening:* the same `MAX_LANE_SESSIONS` bound now applies, evicting oldest-first and never the
entry just written.

## Verified interactions between fixes

- **Capability store ↔ changes map:** both key off `selectedSessionId`; no divergence.
- **W5 cap ↔ routing:** a result whose task was evicted drops naturally (its generation is gone) —
  the eviction path and the drop path agree.
- **C1 nonce ↔ C2 guard:** independent layers (host run identity vs. renderer result attribution);
  no overlap or conflict.
- **F1 ↔ `clearContext`/invalidation:** the unconditional reset matches the pre-existing
  unconditional reset in `invalidateWorkspaceBoundUi`.
- **Idle-collapse ↔ `changesState`:** absent and `{kind:"idle"}` are equivalent by derivation, so
  collapsing is UI-transparent.

## Verification record

| Check | Result |
|---|---|
| `cargo test -p desktop-app --all-features --jobs 1` | **120 passed** |
| `cargo clippy -p desktop-app --all-targets --all-features -- -D warnings` | Clean |
| `cargo fmt --check` | Clean |
| Renderer `vitest run` | **415 passed** / 39 files |
| Renderer `typecheck` (lint gate) | Clean |
| Full `pnpm verify:source` | Not run — env is Node 24.11.1 vs pinned 24.18.0 (`toolchain:check` fails first); later steps need Docker |

## Open items (unchanged, honestly scoped)

1. **bmadHelp per-task migration** — the largest lane; render-gate proven insufficient.
2. **context lane** — still global (entangled with selection mode, provenance, workspace authority).
3. **External infrastructure** — live Azure/Entra, deployed migration job, trusted image publisher,
   signed installer, pilot evidence. Not closable from a coding session.
