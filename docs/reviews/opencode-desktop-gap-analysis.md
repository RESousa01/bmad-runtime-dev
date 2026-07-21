# OpenCode desktop review — what Sapphirus is missing

Reviewed 2026-07-21 from `anomalyco/opencode` (`packages/desktop` Electron shell,
`packages/app` + `packages/ui` + `packages/session-ui` renderer). Design
reference only: every pattern is reimplemented in Sapphirus, never copied,
per provenance governance.

## Already ported

- Unified diff review with per-file counts, collapsed unchanged runs
  (`session-review`, `MAX_DIFF_CHANGED_LINES` bounding).
- Five-block change-magnitude meter (`diff-changes`).
- Dim-directory/bright-filename paths, changed-files index, sticky file headers.
- Chat-first changes flow (their review tab has no manual composer either).
- Warm-graphite + peach dark theme in the OpenCode style (`tokens.css`).
- Turn-timeline chat cards.

## Missing — renderer, feasible now

| Feature | OpenCode source | Notes for Sapphirus |
|---|---|---|
| Command palette (Ctrl+K) | `components/dialog-command-palette-v2.tsx`, `command-palette.ts` | Actions: new task, open workspace, drawer tabs, settings, per-agent capability launch. |
| Keyboard shortcut map + tooltips | `command-tooltip-keybind.ts` | Central keybind registry; show binds in tooltips. |
| Session tabs / multi-session | `session/session-sortable-tab-v2.tsx`, `pages/session-layout.ts` | Host has `run.create`; renderer keeps one implicit task today. |
| File tabs + viewer | `pages/session/file-tabs.tsx` | Workspace explorer exists; no persistent open-file tabs. |
| Split diff style toggle | `review-tab.tsx` (`unified \| split`) | ChangeDiff currently unified-only. |
| Theme picker | `ui/src/theme/themes/*.json` (30+ themes) | Sapphirus pins dark; `app.preferences.*` commands exist for persistence. |
| Command/agent mentions in composer | `components/prompt-input` | Attachment + mention affordances. |
| Release notes dialog | `dialog-release-notes.tsx` | Post-update "what changed" surface. |
| Error page with recovery actions | `pages/error.tsx`, `error-description.ts` | Sapphirus shows bare fail-closed strings. |

## Missing — host/shell

| Feature | OpenCode source | Notes |
|---|---|---|
| Window state persistence | `main/window-state.ts` (electron-window-state) | Tauri equivalent: persist size/pos in app data; restore in setup. |
| Single-instance guard | Electron default + `window-registry.ts` | Confirmed gap: second Sapphirus launch shows a broken store-locked window. |
| Native menu + accelerators | `main/menu.ts`, `desktop-menu-actions.ts` | Tauri menu API; at minimum Edit-menu clipboard actions. |
| Auto-update UX | `main/updater-controller.ts` + subscriptions | Blocked on org signing (policy), but the renderer surface can be staged. |
| Onboarding flow | `main/onboarding.ts` | First-run explanation of governance model would soften the fail-closed UX. |
| Crash/unresponsive handling | `main/unresponsive.ts` | Detect hung webview, offer reload. |
| Structured logging | `main/logging.ts` (electron-log) | Sapphirus host logs nothing by policy; a bounded local ring buffer would have saved hours of this session's debugging. |
| External app handoff | `main/apps.ts`, `open-in-app-v2.tsx` | "Open in editor/explorer" for workspace files. |

## Deliberately not adopted

- Electron itself (Sapphirus is Tauri by design; smaller, org-policy friendly).
- Terminal/PTY integration (`@lydell/node-pty`) — outside Sapphirus's
  read-only + governed-edit authority model.
- Cloud sync/telemetry (Sentry) — conflicts with the privacy posture.
- SolidJS migration — architecture, not framework, is what matters here.
