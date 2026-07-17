# Desktop host client modularization

Date: 2026-07-16

## Goal

Organize the desktop renderer host boundary without changing its public import
path, wire protocol, runtime behavior, or security posture. The current
`apps/desktop-ui/src/lib/hostClient.ts` mixes public contracts, fail-closed
validation, domain codecs, command construction, stateful client behavior, and
runtime initialization in one file of more than 4,000 lines.

## Behavior invariants

- Existing consumers continue importing from `./lib/hostClient` or
  `../lib/hostClient`; no protected UI file changes.
- The exported names and their TypeScript shapes remain unchanged.
- Every command name, schema identifier, request envelope, validation limit,
  exact-key check, error class, recovery transition, sequence check, and safe
  renderer message remains behaviorally identical.
- Browser-demo and Tauri initialization behavior remain unchanged.
- No D2, D3, workspace, or projection capability is added or removed.

## Pressures addressed

- The facade currently has multiple unrelated reasons to change.
- Protocol validation for workspace, BMAD, governed changes, and projection
  events is interleaved with the stateful client.
- Public contracts and internal codec helpers are difficult to locate and
  review independently.
- The file size makes focused review and future collision avoidance harder.

## Isolation and collision matrix

The only registered worktree is the primary checkout on
`codex/bmad-00-foundation`. The working tree contains extensive concurrent D2,
contracts, support-plane, generated-binding, reference-library, and desktop UI
work. Every pre-existing modified or untracked path is user-owned and protected.

| Path class                                                | Planned edit | Ownership                 | Action                                                  |
| --------------------------------------------------------- | ------------ | ------------------------- | ------------------------------------------------------- |
| `apps/desktop-ui/src/lib/hostClient.ts`                   | yes          | clean at task start       | modularize while preserving facade                      |
| `apps/desktop-ui/src/lib/hostClient/*`                    | create       | current task              | add focused internal modules                            |
| this refactor record                                      | create       | current task              | retain scope and proof record                           |
| all pre-existing modified/untracked paths                 | no           | user/other session        | never touch, stage, format, generate, or delete         |
| user-named protected UI/config/tool files                 | no           | user/other session        | hard-protected even if a transitive edit appears useful |
| `bmad-runtime-lib/**`                                     | no           | reference-only/user-owned | consult only                                            |
| ignored `.worktrees`, `tmp`, build, and cache directories | no           | unknown                   | do not clean or delete                                  |

Hard-protected user-named paths:

- `apps/desktop-ui/src/App.tsx`
- `apps/desktop-ui/src/App.test.tsx`
- `apps/desktop-ui/src/components/TaskWorkspace.tsx`
- `apps/desktop-ui/src/components/TaskWorkspace.test.tsx`
- `apps/desktop-ui/src/components/GovernedChangesPanel.tsx`
- `apps/desktop-ui/src/components/GovernedChangesPanel.test.tsx`
- `apps/desktop-ui/src/styles.css`
- `package.json`
- `tools/check-boundaries.mjs`
- `crates/desktop-app/tauri.conf.json`

## Slices

1. Keep `hostClient.ts` as the stable public facade and stateful client/runtime
   boundary.
2. Extract public/internal protocol contracts and errors.
3. Extract scalar validation and path/text safety primitives.
4. Extract workspace/bootstrap codecs, BMAD codecs, governed-change codecs, and
   projection codecs into domain-named modules.
5. Extract command-envelope builders from response codecs.
6. Re-export only the names that were public before the refactor.

## Proof

Baseline proof before mutation:

- Pinned Node `24.18.0`.
- `vitest run src/lib/hostClient.test.ts`: 122/122 tests passed.

Required post-change proof:

- The focused host client suite passes unchanged.
- Desktop UI TypeScript typecheck passes.
- The complete desktop UI test suite passes, including protected consumers.
- Production desktop UI build passes.
- `git diff --check` passes for task-owned paths.
- Final diff and status confirm no pre-existing dirty path was changed by this
  task.

## Rollback

The change is structural and confined to one previously clean file plus new
internal modules and this record. Rollback consists of restoring the original
`hostClient.ts` and removing only the task-created module directory and this
record. No generated artifacts or shared configuration are involved.

## Review focus

- Public export compatibility and type-only import correctness.
- Accidental changes to exact validation or envelope construction.
- Module cycles that could change runtime initialization.
- Any diff outside the declared task-owned paths.
