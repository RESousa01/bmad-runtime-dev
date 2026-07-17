# Sapphirus task-shell redesign coordination

Date: 2026-07-17

## Frozen contracts

```ts
export type PrimaryRoute = { kind: "task"; taskId: string | null };
export type ContextDrawerKind = null | "files" | "changes" | "run-details" | "methods";
export type AppModalKind = null | "workspace-manager" | "settings" | "account";
```

`App` owns these states plus the retained narrow-width fallback state. Child components are presentation-only and receive callbacks. The renderer continues to use the existing stable `DesktopHostClient` facade and its closed 24-command ready catalog / 2-command recovery catalog.

## Baseline adaptation

The checkout began dirty on branch `codex/bmad-00-foundation`, ahead of origin, with overlapping uncommitted frontend, host-client, native, generated-contract, package, and Tauri changes. Those files are user/in-flight baseline state. Do not reset, revert, stage, commit, or rewrite them wholesale. Review only changes introduced after this coordination record and preserve the stable `src/lib/hostClient.ts` facade.

## File ownership

- Coordinator: `App.tsx`, integration wiring, final stylesheet import, ledgers.
- Sidebar slice: new Sidebar and no-workspace component/test/style files only.
- Task slice: `TaskWorkspace.tsx`, its tests, and task-shell style file only.
- Drawer slice: new ContextDrawer component/test/style files only; existing feature panels are composed, not copied.
- Shell slice: new AppShellLayout component/test/style files only.
- Integration tests: new test/fixture files only.

No worker edits `crates/**`, generated contracts, `packages/ui`, BMAD projection modules, `src/lib/hostClient/**`, package configuration, lockfiles, Tauri config/capabilities/permissions, `bmad-runtime-lib`, or another slice's files.

## Integration invariants

- Opening/closing chrome makes no mutation. Files and Skills and agents may load their existing bounded read-only projections when mounted.
- Files uses existing workspace source and authenticated preview callback.
- Attached context is display derived from the authenticated projection and is not consent.
- Skills and agents is presentation copy; the internal `methods` drawer key and `bmad.*` protocol identifiers stay unchanged.
- Changes never synthesizes approval IDs or hashes.
- Recovery and unavailable modes expose no mutation.
- Workspace/binding changes close contextual UI and invalidate stale workspace-bound state.

## Required checks

Pinned Node 24.18.0 and pnpm 11.12.0; frontend typecheck, Vitest, build, boundary scan, direct-invoke scan, copy scan, diff check, and browser viewport review. Native/package checks run in parallel when time permits and may not be replaced by Vite evidence.
