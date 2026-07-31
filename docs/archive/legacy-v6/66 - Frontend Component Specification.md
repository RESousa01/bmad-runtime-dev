---
title: "Frontend Component Specification"
aliases:
  - "66 - Frontend Component Specification"
tags:
  - bmad-runtime
  - vault/implementation-assets
section: "Implementation Assets"
order: 66
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: frontend-component-spec
status: v6-modernized-implementation-guide
validated_on: 2026-07-09
---



# Frontend Component Specification

## V6.17 component host contracts

Reusable components accept delivery-neutral view models plus an explicit delivery badge and capability set. `WebRuntimeFacade` supplies cloud project/import/upload/snapshot/remote-execution operations. `DesktopRuntimeFacade` supplies selected-folder/grant, local context, local diff/command approval, checkpoint/rollback, evidence, egress, sync, and remote-handoff operations through typed IPC/cloud support clients.

Desktop components cannot accept arbitrary path or command strings as privileged operations. Approval views show executable/script identity, argv, cwd, declared writes/network, containment tier, checkpoint, expiry, and candidate hash. Cloud result views disable Apply until imported as a fresh local proposal.

## 1. V6 frontend stack

- React 19.2.
- Vite 8 + React Router 8 SPA mode.
- TypeScript 7 application compiler; isolated TypeScript 6 compatibility package only for tools that import the compiler API until their TS7 gate passes.
- Node.js 24 LTS.
- pnpm 11.x.
- Generated API clients from the single OpenAPI 3.1.2/JSON Schema 2020-12 contract; OpenAPI 3.2 remains a .NET 11/tooling watch item.

### 1.1 UI dependency baseline

The visual and interaction stack is locked in [[26 - Frontend Design System]]. The minimum first-slice package set is:

| Concern | Package / approach | Rule |
|---|---|---|
| Accessible behavior | `react-aria-components` behind `packages/ui` wrappers | No route-level raw primitive imports or bespoke dialog/menu/focus implementations. |
| Styling | Tailwind CSS 4 + semantic CSS custom properties + CVA | Tokens are the source of truth; no raw status colors in component code. |
| Icons | `lucide-react` named imports through the icon wrapper | One icon family and one optical-size/stroke policy. |
| Motion | CSS for microstates; `motion/react` for panel/capsule enter, exit, and continuity | `LazyMotion`, global reduced motion, no perpetual decorative animation. |
| Panels | `react-resizable-panels` | Keyboard-operable separators, minimum hit areas, persisted user preference only. |
| Virtualization | `@tanstack/react-virtual` | Required for large logs, trees, search results, and long timelines. |
| Diff | `@pierre/diffs` after the Phase-0 gate | Read-only split/stacked review; experimental mutation and worker APIs are excluded. |
| Markdown | `react-markdown` + `remark-gfm` without raw HTML | Strict component and URL allowlists; rich HTML is sandboxed separately. |
| Component QA | Storybook + Vitest + React Testing Library + axe | Stories and tests cover state, theme, density, keyboard, and reduced motion. |
| Flow QA | Playwright + `@axe-core/playwright` | E2E, responsive, screenshot, reconnect, expiry, and recovery coverage. |

Do not add an alternative primitive system, generic enterprise component suite, global state library, animation engine, or editor framework until a measured requirement and ADR justify it.

## 2. Core components

### 2.1 Shell and navigation components

| Component | Props/input | Critical behavior | Responsive behavior |
|---|---|---|---|
| `AppShell` | delivery model, capabilities, route state | Owns landmarks, skip links, theme/density, command palette, and delivery identity. | Switches from four-region workbench to single-workspace routes without duplicating navigation. |
| `GlobalRail` | current project, enabled destinations | Shows Project, Chat, Files, Artifacts, History; Builder/Operator only when gated. | `56 px` rail on desktop; bottom route bar on mobile review companion. |
| `ContextRail` | route-specific list/tree model | Threads, files, artifacts, or history; selection survives collapse. | Persistent at `≥1440 px`, mutually exclusive with inspector at mid-width, sheet below `1200 px`. |
| `ProjectHeader` | project, delivery badge, checkpoint/branch, active run | Keeps `Cloud workspace` or `Local folder` text visible and shows only action-relevant health. | Reduces metadata before hiding the delivery label. |
| `Workbench` | main content, context region, inspector region | Maintains independent scroll roots, panel sizes, and focus return. | Follows the canonical breakpoints in file 43. |
| `Inspector` | selected tab and selected object | Stable Context/Changes/Command/Logs/Evidence surface; never creates alternate authority. | Resizable region on wide screens; focus-managed non-modal sheet on narrow screens. |
| `Composer` | draft, attachments, active BMAD/profile context | Persistent, multiline, keyboard submit, upload state, and clear disabled reason. | Stays in the conversation route; does not span or cover the inspector. |

### 2.2 Run and decision components

The timeline uses one `RunCapsule` per run rather than a large card for every backend event.

| Component | Required anatomy | Critical behavior |
|---|---|---|
| `RunCapsule` | request summary, delivery target, stage rail, active-stage body, elapsed state, inspector links | Groups Understand → Plan → Review → Execute → Evidence; completed stages collapse without losing evidence. |
| `StageRail` | five named stages, current/complete/blocked status | Uses text/icon/state, not color alone; unknown event kinds remain inside technical detail. |
| `PlanSummary` | outcome, ordered steps, affected areas, risks, validation | Never implies approval; exposes `Revise plan` and `Continue to review`. |
| `ProposalSummary` | file/command/network/risk/rollback counts | Primary action is `Review changes`, not inline blind approval. |
| `ApprovalReview` | outcome, impact strip, review tabs, sticky decision footer | Exact hash remains inspectable; action label states local/cloud/export/activation boundary. |
| `ExecutionProgress` | real phase, elapsed time, safe-stop state, latest meaningful output | Does not invent percentages; logs remain in the inspector. |
| `PartialFailureDecision` | succeeded work, failed work, current workspace state, valid recovery actions | Keeps the safest reversible path prominent without auto-triggering it. |
| `RunOutcome` | validation, changed files, execution location, evidence, rollback | Uses `Validated`, `Validation failed`, or another literal terminal state; never `Done`. |

### 2.3 Operational detail components

| Component | Props/input | Data source | Critical behavior | Failure behavior |
|---|---|---|---|---|
| `RunEventCard` | event envelope | event stream | Unknown event renders safe diagnostic. | Does not crash stream. |
| `PlanCard` | plan summary | run event payload | Shows affected files, risks, validation strategy. | Missing fields show contract warning. |
| `ProposalCard` | proposal | API + event | Clearly marks proposed/not executed. | Void/stale proposal cannot approve. |
| `ApprovalTechnicalDetails` | approval requirement | Airlock API | Supplies hashes, spec fields, policy details, and expiration to `ApprovalReview`. | Expired/stale approval keeps review visible and disables the decision with a reason. |
| `DiffPanel` | diff refs | Blob/API | Shows preimage status and risk labels. | Missing diff ref shows retriable error. |
| `CommandSpecPanel` | command spec | proposal/spec | Shows `argv[]`, cwd, network mode, timeout. | Raw shell string gets danger rendering. |
| `LogPanel` | execution log refs | stream/Blob | Read-only, virtualized, redacted. | Falls back to Blob log chunks. |
| `EvidencePanel` | evidence bundle | evidence API | Links every artifact and side-effect proof. | Missing proof blocks “complete” badge. |
| `ContextPanel` | context pack | context API | Shows file hashes, line ranges, redaction status. | Stale context warning if checkpoint changed. |
| `ArtifactPreview` | artifact version | artifact API | Shows preview, provenance, export status. | Unsafe preview rendered as download/text only. |
| `OperatorPolicyPanel` | policy summary | operator API | Shows current policy version and recent denials. | Non-operator receives route denial. |

## 3. UI anti-confusion rules

- Use separate state labels for `proposed`, `approved`, `running`, `applied`, `validated`, `failed`, `rolled_back`, `voided`, and `expired`.
- Do not render “done” from client optimism.
- Approval button is disabled when expired, stale, policy refresh required, or user lacks permission.
- A rejected proposal remains inspectable but cannot execute.
- A voided proposal must explain which checkpoint, preimage drift, policy version, or expiry made it invalid.
- Job logs are evidence streams, not terminal input. Do not provide raw shell prompt UI in v1.
- Approval-required cards open the complete `ApprovalReview`; they do not place a high-consequence primary approval button beside a collapsed diff summary.
- Every action label names its authority boundary: `Approve & run locally`, `Approve & run in cloud`, `Approve export`, or `Approve package activation`.
- Completed run detail collapses into its parent `RunCapsule`; the conversation must not grow by one permanent large card per server event.
- Toasts confirm transient actions only. Task-changing failures remain inline in the owning run, review, or inspector surface.

## 4. Modern React guidance

- Use React transitions for non-blocking panel refresh and stream projections.
- Use optimistic UI only for local display states that can be reconciled safely.
- Do not use client cache as authority for lifecycle state.
- Use discriminated unions for event cards and run states.
- Keep generated API client types separate from view models.
- Sanitize markdown/artifact previews and isolate HTML previews with strict CSP/sandboxing.
- Prefer React Router loaders/actions and generated facade clients for v1 server state. Streaming events reduce into an idempotent projection and reconcile from the authority snapshot after reconnect.
- Lazy-load diff rendering, syntax themes, Builder, Operator, and rich artifact preview routes. Do not load those packages in the default project shell chunk.
- Keep `AppShell`, route composition, and event projection separate from presentation components. `packages/ui` accepts delivery-neutral view models and capability flags only.
- Use stable keys from durable event/object IDs; never key streaming rows by array position.

## 5. Accessibility implementation notes

- Approval decisions require keyboard operation.
- High-risk approvals require confirmation and visible risk reason.
- Log updates use controlled live regions; do not announce every line.
- Diff navigation supports keyboard next/previous hunk.
- Panels preserve focus when resized/collapsed.
- Reduced-motion setting disables nonessential animation.
- Color is never the only risk indicator.
- Opening and closing context/inspector regions restores focus to the exact trigger; route transitions focus the screen heading only when navigation changes the user's task context.
- Resize separators support keyboard input, visible focus, current-value announcement, and a generous invisible pointer hit area.
- `200%` zoom preserves all decision actions without page-level horizontal scrolling; code/diff regions may scroll horizontally when wrapping is disabled.
- Forced-colors mode preserves selection, focus, diff additions/removals, risk, and delivery-boundary identification.
- Compact density changes spacing, never root font size or minimum target size.

## 5.1 Motion implementation contract

- Use the centralized `80/120/180/220 ms` duration tokens from file 26; arbitrary component-local timings fail review.
- CSS transitions handle hover, focus, press, border, and color. Motion handles inspector/sheet entrance, capsule entrance, and approval-to-execution continuity only.
- Animate `opacity` and a maximum `8 px` translation by default. No parallax, background particles, shaking errors, bouncing approvals, flashing logs, or pulsing warning states.
- Configure `MotionConfig reducedMotion="user"`. Reduced motion removes transform/layout animation but keeps immediate opacity/color state feedback.
- Streaming text, log chunks, progress elapsed time, and diff rows do not replay entrance animation.
- Animation cannot delay navigation, approval, stop, rollback, or focus placement.

## 6. Playwright scenarios

1. Create thread and send message.
2. Receive plan card.
3. Receive proposal card and open diff.
4. Approve patch.
5. Observe execution log stream.
6. See validation failure and repair proposal.
7. Complete evidence report.
8. Try approving expired approval and expect block.
9. Attempt operator route as non-operator and expect denial.
10. Receive unknown event type and verify safe fallback.
11. Receive stale proposal after checkpoint and verify approval block.
12. Render artifact preview containing unsafe HTML and verify sanitization/sandbox behavior.
13. Complete the first slice at `1280×720` and `1440×900` without clipped composer, inspector, or approval actions.
14. Complete proposal review and rejection keyboard-only; verify focus return after closing the inspector.
15. Enable reduced motion and verify that panel/capsule transforms stop while state changes remain clear.
16. Switch to compact density and `200%` zoom; verify readable controls and no page-level horizontal overflow.
17. Collapse a completed `RunCapsule`, reopen it, and verify evidence/rollback links and scroll position remain stable.
18. Disconnect during approval, reconnect after expiry, and verify the decision stays disabled until authority state refreshes.
19. Render a very large log and diff fixture; verify virtualization, search, selection, and keyboard navigation stay responsive.
20. Verify light, dark, and forced-colors states with screenshot and axe gates for the workbench, approval review, failure decision, and evidence outcome.
