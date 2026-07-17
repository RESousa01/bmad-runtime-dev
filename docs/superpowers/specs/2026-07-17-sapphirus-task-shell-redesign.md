# Sapphirus Task Shell Redesign

## Status

Approved by the user through `sapphirus-gpt-5.6-sol-ultra-integrated-agent-pack.zip`; the pack explicitly identifies itself as the approved design direction and requires implementation without another approval pause.

## Goal

Replace the overloaded IDE-like renderer shell with a task-first desktop experience while preserving every existing host, consent, workspace-authority, BMAD skill-and-agent, governed-change, recovery, and rollback boundary.

## Information architecture

- Stable location: one task route, `{ kind: "task", taskId }`.
- Persistent desktop navigation: one Sidebar with workspace scope, New task, task history, Settings, and Account.
- Contextual work: one drawer, closed by default, with exactly Files, Changes, Run details, and Skills and agents.
- Transient utilities: workspace management, Settings, and Account are modals and never routes.
- Desktop is the product target for this pass. Existing narrow-width safeguards remain best-effort compatibility, not a tablet or phone design target.

## Product language

Use Task, New task, Workspace, Files, Attach files, Attached context, Changes, Run details, Skills and agents, and Browser preview in normal presentation copy. “Methods” is not a user-facing destination. Keep `session*`, the internal drawer key `methods`, `bmad.*`, schema IDs, command names, receipts, and other protocol vocabulary unchanged internally. Availability copy must remain honest: deterministic/local Help is available, production model brokerage is not.

## Components

- `AppShellLayout`: slot-only layout and overlay mechanics; no route or host authority.
- `AppSidebar`: workspace scope, New task, task history, Settings, Account; callback-only.
- `NoWorkspaceState`: one valid Open workspace action and conditional Try demo.
- `TaskWorkspace`: one timeline and one composer; Attach files, authenticated context chips, exact review/consent progress, and contextual Changes/Skills and agents/Run details entry points.
- `ContextDrawer`: one titled pane/overlay that composes existing Files, Changes, Skills and agents, and run-detail projections without reconstructing authority data.
- Existing `WorkspacePanel` and `UtilityPanel`: retained as modal adapters.

## Data and safety

`App` retains the stable `HostRuntime`, `DesktopHostClient`, capability checks, generation refs, authority snapshots, and all domain handlers. Opening navigation or modal chrome does not mutate domain state; Files and Skills and agents may request their existing bounded, read-only projections when their drawer content mounts. Workspace switch, revocation, bootstrap binding change, or recovery closes workspace-bound UI and clears preview/change/BMAD state before stale results can render. Attached chips derive only from `ContextPreviewProjection`; submission still uses the existing exact review and explicit consent path. Changes continue through host-projected review, exact decision bindings, execution, history, and rollback.

## Accessibility and responsive behavior

Desktop uses a 260 px Sidebar and optional 420 px drawer. The task remains visible beside the drawer. Body text remains at least 14 px except metadata, and visible focus rings/reduced-motion behavior are preserved. Narrow-width behavior is retained as a safety fallback but is outside this desktop-only acceptance pass.

## Verification

Behavior tests must cover transient-state separation, no-workspace routing, Files without route changes, canonical vocabulary, runtime-mode restrictions, read-only drawer loads, and preservation of Skills and agents/Changes callbacks. Run pinned frontend typecheck, full renderer tests, build, boundary scan, direct-invoke scan, visible-copy scan, and rendered desktop checks at 1440x900 and 1100x700. Native and generated paths are protected from new task changes; because the baseline is already dirty, review the task delta rather than requiring a zero diff from HEAD's merge base.

## Scope

No native, generated-contract, host-client protocol, BMAD projection state-machine, package, lockfile, Tauri configuration, permission, or capability changes. Do not reintroduce seeded demo sessions, fabricated history, fake model availability, or model-to-edit authorization.
