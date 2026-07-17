# Sapphirus Task Shell Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:test-driven-development for each behavior slice. The pack explicitly requires parallel, non-overlapping builders; strict file ownership replaces per-worker worktrees in this already-dirty shared checkout.

**Goal:** Deliver a task-first desktop shell with a unified Sidebar, optional four-surface context drawer, transient modals, and unchanged native safety boundaries.

**Architecture:** `App` retains all host orchestration and owns small UI states for the task route, context drawer, modal, and a retained narrow-width fallback. New callback-only shell components compose the existing Files, Skills and agents, and governed-changes feature panels. No domain call, protocol, generated artifact, or native authority moves into visual components.

**Tech Stack:** React 19, TypeScript 7, React Aria / `@sapphirus/ui`, lucide-react, CSS, Vitest, Testing Library, axe-core, Vite 8.

## Global Constraints

- Preserve the exact four Tauri commands and existing 24-command ready / 2-command recovery catalogs.
- No new direct domain `invoke`; all domain work remains behind `DesktopHostClient`.
- Do not edit native crates, generated contracts, BMAD state machines, host-client protocols, package configuration, lockfiles, Tauri configuration, permissions, capabilities, or `bmad-runtime-lib`.
- Treat pre-existing dirty files as protected baseline; do not reset or overwrite unrelated changes.
- Use Task, New task, Workspace, Files, Attach files, Attached context, Changes, Run details, Skills and agents, and Browser preview in normal copy. Keep the internal `methods` drawer key and wire vocabulary unchanged.
- Preserve exact context review/consent, workspace/grant authority, stale-result guards, recovery restrictions, governed approval bindings, history, and rollback.
- Use test-first red/green cycles and run with Node 24.18.0 / pnpm 11.12.0.

---

### Task 1: Shell and transient-state primitives

**Files:**
- Create: `apps/desktop-ui/src/components/redesign/AppShellLayout.tsx`
- Create: `apps/desktop-ui/src/components/redesign/AppShellLayout.test.tsx`
- Create: `apps/desktop-ui/src/components/redesign/app-shell.css`

**Interfaces:**
- Consumes slots for `sidebar`, `main`, optional `drawer`, optional `modal`; `mobileSidebarOpen`; close callbacks.
- Produces landmark layout with desktop columns and named narrow overlays; owns no product state.

- [x] Write tests that fail because the shell is absent: main expands without a drawer, task remains present with a drawer, narrow overlays have dialog names/close controls, and modal content is separate.
- [x] Run the focused test and confirm missing component/behavior failures.
- [x] Implement the minimal slot layout and overlay semantics using existing focus helpers.
- [x] Rerun the focused test until green; refactor only names/duplication.

### Task 2: Unified Sidebar and focused no-workspace state

**Files:**
- Create: `apps/desktop-ui/src/components/redesign/AppSidebar.tsx`
- Create: `apps/desktop-ui/src/components/redesign/NoWorkspaceState.tsx`
- Create: `apps/desktop-ui/src/components/redesign/AppSidebar.test.tsx`
- Create: `apps/desktop-ui/src/components/redesign/sidebar.css`

**Interfaces:**
- Sidebar receives workspace label/status, tasks, selected task ID, and callbacks for workspace manager, new/select task, Settings, Account, and fallback-overlay close.
- NoWorkspaceState receives mode/copy, `onOpenWorkspace`, and optional `onTryDemo`; the primary callback fires once per activation.

- [x] Write failing tests for canonical copy, selected task, named Settings/Account at narrow width, fallback close, and Open workspace once.
- [x] Run the test and confirm behavior failures.
- [x] Implement callback-only components with no host imports.
- [x] Rerun to green and remove redundant presentation.

### Task 3: Task surface and authenticated attachments

**Files:**
- Modify: `apps/desktop-ui/src/components/TaskWorkspace.tsx`
- Modify: `apps/desktop-ui/src/components/TaskWorkspace.test.tsx`
- Create: `apps/desktop-ui/src/components/redesign/task-surface.css`

**Interfaces:**
- Add callbacks for Attach files, Changes, Skills and agents, Run details, and authenticated `contextPreview` data.
- Preserve `onReviewRequest(intent)` as the only submission entry.

- [x] Add failing tests for Task/New task copy, exactly one composer, Attach files opening Files, chips derived from `ContextPreviewProjection`, no premature Review context, Skills and agents/Run details callbacks, and exact review submission.
- [x] Run focused tests and confirm the old surface fails those expectations.
- [x] Implement minimal timeline/header/composer changes without host imports or projection mutation.
- [x] Rerun focused tests; refactor presentation while green.

### Task 4: Four-surface Context drawer

**Files:**
- Create: `apps/desktop-ui/src/components/redesign/ContextDrawer.tsx`
- Create: `apps/desktop-ui/src/components/redesign/ContextDrawer.test.tsx`
- Create: `apps/desktop-ui/src/components/redesign/context-drawer.css`

**Interfaces:**
- `kind` is exactly `"files" | "changes" | "run-details" | "methods"`.
- Receives already-wired React content or existing feature props; opening/closing does not call the host.

- [x] Write failing tests for the four titles, close behavior, overlay dialog semantics, no tabbed Inspector labels, and no fifth/context destination.
- [x] Run the focused test and confirm failure.
- [x] Implement the minimal titled composer for WorkspaceExplorer, GovernedChangesPanel, run details, and Skills and agents content.
- [x] Rerun to green and keep feature callbacks unchanged.

### Task 5: App integration and authority invalidation

**Files:**
- Modify: `apps/desktop-ui/src/App.tsx`
- Modify: `apps/desktop-ui/src/App.test.tsx`
- Create: `apps/desktop-ui/src/App.shell.integration.test.tsx`

**Interfaces:**
- Replace `activeView`/Inspector route authority with `PrimaryRoute`, `ContextDrawerKind`, `AppModalKind`, and `mobileSidebarOpen`.
- Continue calling existing App handlers and the same stable host runtime/client.

- [x] Add failing tests for modal/drawer separation, no permanent Inspector, ready/no-workspace Open workspace once, Files preserving task route, task selection closing stale contextual UI, recovery mutation restrictions, and no host mutation from transient UI.
- [x] Run the focused tests and confirm the current shell fails for the intended reasons.
- [x] Wire new shell components, mapping Files/Changes/Skills and agents/Run details to existing callbacks and panels.
- [x] On workspace switch/revoke/recovery, close drawer and clear context/change/BMAD state using generation and authority-key guards.
- [x] Run focused tests green before the final full renderer suite.

### Task 6: Proof and review

**Files:**
- Modify: `docs/redesign/integration-ledger.md`
- Update BigBrain only with verified durable results.

- [ ] Run pinned frontend typecheck, full tests, build, `git diff --check`, boundary validation, direct-invoke scan, and visible-copy scan.
- [ ] Audit only the new task delta for protected-path changes and action-to-client traceability.
- [ ] Render and inspect the desktop target at 1440x900 and 1100x700; check console, focus, overlays, clipping, and primary interactions.
- [ ] Run package/native checks concurrently when time remains; label exact skips and pre-existing failures.
- [ ] Perform independent change review, resolve P0/P1 findings, and record residual risks.
