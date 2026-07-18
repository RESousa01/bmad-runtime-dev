# Task 3 Report: Single-Use Reviewed Recovery Authority

Date: 2026-07-18
Status: COMPLETE
Implementation commit: `6a00b2f`

## Delivered

- Added host-only, bounded, in-memory recovery approvals with consume-first decisions and invalidation on renderer, workspace, grant, recovery-mode, and restart boundaries.
- Added strict authenticated journal/checkpoint loading, observation-only recovery preparation, closed recovery-plan mapping, and renderer-safe bounded projections.
- Added broker-scoped checkpoint restoration through the Task 2 governed transaction seam. A durable `restoring` transition precedes mutation; failed restore paths are terminalized as `manual_review`.
- Kept runtime command names, IPC parsing, renderer clients, and UI out of Task 3.

## TDD proof

Initial red command:

```text
cargo test -p desktop-app recovery --locked
```

Initial red result: exit 1, `error[E0432]: unresolved import super::prepare_recovery` at `crates/desktop-app/src/recovery.rs:7:9`.

Final proof:

```text
cargo fmt --all -- --check
PASS

cargo clippy -p desktop-app --all-features --all-targets -- -D warnings
PASS

cargo test -p desktop-app recovery --locked
PASS: 18 passed, 0 failed

cargo test -p desktop-app governed_changes --locked
PASS: 0 selected, 0 failed, 63 filtered out

cargo test -p desktop-app --all-features --locked
PASS: 72 passed, 0 failed

git diff --check
PASS
```

The exact `governed_changes` filter currently selects no test names; the full all-features package run exercises the existing governed-edit tests plus all new recovery tests.

## Self-review

- Scope matches the five files named by the brief.
- Recovery approval IDs remain opaque, bounded, short-lived, single-use, and cross-domain isolated from ordinary edit approvals.
- Preparation does not write `restoring`; only direct no-effect reconciliation writes `recovered`, while structurally unsafe durable data writes `manual_review`.
- Restore reauthenticates durable bindings and fresh workspace observations before mutation, then uses constant-time digest comparisons.
- Lock order remains workspace commit/Ready authority before workspace scope/store, with the pending-recovery mutex acquired only in narrow insert/take/invalidate operations and released before restore reacquires the commit barrier.
- Wire serialization exposes only relative paths, stable operation labels/explanations, public review bindings, and stable safe reasons.

## Concerns / follow-up

- Task 4 must add the runtime command and strict IPC boundary; Task 3 intentionally leaves the new host functions and projections unrouted.
- BigBrain was consulted at `03-projects/sapphirus-bmad-runtime.md`. The durable project checkpoint should be updated by the parent integration task after Tasks 3-5 are assembled, so it records the final P2 boundary rather than this isolated subtask alone.
