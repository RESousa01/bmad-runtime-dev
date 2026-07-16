# Sapphirus Desktop Shell Visual Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver a polished, compact, responsive Sapphirus desktop shell based on the supplied prototype while preserving the existing palette, behavior, accessibility, and product capability boundaries.

**Architecture:** Keep `App.tsx` as the state/composition owner and refine the existing presentational components in place. Add only presentation-focused markup and CSS; do not alter host calls, BMAD projections, persistence, routes, or state ownership. Use existing UI primitives and semantic color tokens, with focused regression tests and visual QA at matching viewports.

**Tech Stack:** React 19, TypeScript 7, React Aria Components, Lucide React, CSS custom properties, Vitest, Testing Library, axe-core, Vite.

## Global Constraints

- Preserve all existing Sapphirus light and dark theme color values in `packages/ui/src/tokens.css`.
- Do not introduce the prototype's purple palette or new direct palette literals.
- Preserve all current typed IPC, host-client, BMAD, modal-focus, and capability behavior.
- Do not add routes, model behavior, edit/apply capability, fabricated evidence, or simulated completion.
- Continue using Inter Variable, JetBrains Mono, Lucide React, and existing shared UI primitives.
- Preserve accessible names, semantic roles, keyboard behavior, reduced motion, forced colors, and overlay focus containment.
- Limit production edits to `apps/desktop-ui` unless an existing shared UI primitive demonstrably requires a presentation-only fix.
- Do not touch unrelated backend, D2, D3, CI, installer, infrastructure, or dependency work.
- In this side conversation, validation checkpoints replace Git commits; do not stage or commit shared worktree changes.

---

## File map

### Create

- `apps/desktop-ui/src/styles.visual-contract.test.ts` — locks the existing palette and presentation-only color boundary.
- `design-qa.md` — records reference-versus-rendered comparison and final visual gate.

### Modify

- `apps/desktop-ui/src/styles.css` — shell sizing, spacing, hierarchy, responsive behavior, surface polish, and state treatment.
- `apps/desktop-ui/src/components/TitleBar.tsx` — stable brand and control grouping classes.
- `apps/desktop-ui/src/components/GlobalRail.tsx` — selected indicator and navigation grouping hooks without behavior changes.
- `apps/desktop-ui/src/components/SessionRail.tsx` — header/list/footer grouping hooks and empty-state readiness.
- `apps/desktop-ui/src/components/TaskWorkspace.tsx` — presentation wrappers for header metadata, message stream, and composer status.
- `apps/desktop-ui/src/components/Inspector.tsx` — stable inspector header/body/action-region hooks.
- `apps/desktop-ui/src/components/WorkspaceExplorer.tsx` — consistent primary-view header/body/footer hooks.
- `apps/desktop-ui/src/components/WorkspacePanel.tsx` — shared dialog anatomy classes.
- `apps/desktop-ui/src/components/UtilityPanel.tsx` — shared dialog anatomy classes.
- `apps/desktop-ui/src/components/BmadHelpCard.tsx` — first-class status and section hierarchy hooks.
- `apps/desktop-ui/src/components/BmadLibraryPanel.tsx` — compact catalog grouping hooks.
- `apps/desktop-ui/src/App.test.tsx` — shell landmark and overlay regression coverage.
- `apps/desktop-ui/src/components/TaskWorkspace.test.tsx` — composer/message state regression coverage.
- `apps/desktop-ui/src/components/BmadHelpCard.test.tsx` — retained Help hierarchy regression coverage.
- `apps/desktop-ui/src/components/BmadLibraryPanel.test.tsx` — Method catalog hierarchy regression coverage.

## Task 1: Lock palette and shell semantics

**Files:**
- Create: `apps/desktop-ui/src/styles.visual-contract.test.ts`
- Modify: `apps/desktop-ui/src/App.test.tsx`

**Interfaces:**
- Consumes: existing `packages/ui/src/tokens.css`, rendered `App` landmarks, and current test bootstrap.
- Produces: a regression boundary that protects the palette and shell structure during all later tasks.

- [ ] **Step 1: Add the palette contract test**

Create `styles.visual-contract.test.ts` with a table-driven assertion over the current light/dark semantic values. Read the stylesheet through `new URL("../../../packages/ui/src/tokens.css", import.meta.url)` and assert exact declarations including:

```ts
const lockedColors = [
  "--color-canvas: #f4f7fb",
  "--color-accent: #4564df",
  "--color-canvas: #06121f",
  "--color-chrome: #071522",
  "--color-surface: #0a1927",
  "--color-surface-raised: #0d1d2b",
  "--color-surface-hover: #10243a",
  "--color-surface-selected: #13283e",
  "--color-border: #26394d",
  "--color-text: #f1f4f8",
  "--color-text-muted: #98a6b8",
  "--color-accent: #6d88ff",
  "--color-accent-strong: #8da2ff",
];
```

Also assert that each listed declaration occurs exactly once in its expected theme block rather than merely appearing in comments.

This palette characterization is intentionally green before implementation. The visual-refactor red signal is the captured current shell compared with the approved reference: the current render lacks the approved compact hierarchy and surface consistency. Post-change proof requires matching-state browser captures, a written mismatch ledger, and `design-qa.md` with `final result: passed`.

- [ ] **Step 2: Run the focused test and confirm the initial contract**

Run: `pnpm --filter @sapphirus/desktop-ui test -- styles.visual-contract.test.ts`  
Expected: PASS with the current palette values.

- [ ] **Step 3: Add shell landmark assertions**

Extend the default workbench test in `App.test.tsx` to require the native title banner, Primary navigation, Sessions complementary region, main work area, and Inspector complementary region. Use accessible roles/names rather than class-only selectors:

```ts
expect(screen.getByRole("banner")).toBeInTheDocument();
expect(screen.getByRole("navigation", { name: "Primary" })).toBeInTheDocument();
expect(screen.getByLabelText("Sessions")).toBeInTheDocument();
expect(screen.getByRole("main")).toBeInTheDocument();
expect(screen.getByLabelText("Inspector")).toBeInTheDocument();
```

- [ ] **Step 4: Run shell and accessibility regressions**

Run: `pnpm --filter @sapphirus/desktop-ui test -- App.test.tsx`  
Expected: all existing App tests plus the new shell assertions pass.

- [ ] **Step 5: Checkpoint the task**

Run: `git diff -- apps/desktop-ui/src/styles.visual-contract.test.ts apps/desktop-ui/src/App.test.tsx`  
Expected: only palette/landmark tests; no production behavior changes.

## Task 2: Refine the stable shell, title bar, global navigation, and sessions

**Files:**
- Modify: `apps/desktop-ui/src/components/TitleBar.tsx`
- Modify: `apps/desktop-ui/src/components/GlobalRail.tsx`
- Modify: `apps/desktop-ui/src/components/SessionRail.tsx`
- Modify: `apps/desktop-ui/src/styles.css`
- Test: `apps/desktop-ui/src/App.test.tsx`

**Interfaces:**
- Consumes: existing `TitleBar`, `GlobalRail`, and `SessionRail` props unchanged.
- Produces: stable `.title-bar__brand`, `.title-bar__controls`, `.global-nav-item__indicator`, `.session-rail__header`, `.session-rail__content`, and `.session-rail__footer` presentation hooks.

- [ ] **Step 1: Add failing structural assertions for the new hooks**

Render `App` and assert that the selected Agent navigation item exposes `aria-current="page"`, the Sessions panel contains a heading and list region, and the pinned workspace remains after the session list. Use accessible queries first and a narrowly scoped DOM-order assertion only for the footer.

- [ ] **Step 2: Verify the focused test fails for the missing structure**

Run: `pnpm --filter @sapphirus/desktop-ui test -- App.test.tsx`  
Expected: FAIL only on the new structural grouping assertion.

- [ ] **Step 3: Add presentation-only markup hooks**

Keep every prop and handler unchanged. Add the indicator as decorative content:

```tsx
<span aria-hidden="true" className="global-nav-item__indicator" />
<Icon aria-hidden="true" className="global-nav-item__icon" size={20} strokeWidth={1.7} />
<span className="global-nav-item__label">{label}</span>
```

Wrap the session actions/list/pinned workspace with header/content/footer class hooks without changing button order, inert state, dialog role, or callbacks.

- [ ] **Step 4: Refactor the shell CSS foundation**

At the top of `styles.css`, add non-color layout variables only:

```css
.app-shell {
  --shell-titlebar-height: 42px;
  --shell-global-rail-width: 104px;
  --shell-session-rail-width: 296px;
  --shell-inspector-min-width: 390px;
  --shell-inspector-max-width: 500px;
  --shell-control-height: 34px;
  --shell-radius-sm: 6px;
  --shell-radius-md: 8px;
  --shell-space-1: 4px;
  --shell-space-2: 8px;
  --shell-space-3: 12px;
  --shell-space-4: 16px;
  --shell-space-6: 24px;
}
```

Use those values to refine the workbench grid, title bar, global navigation, session rows, selected indicator, unread marker, and pinned footer. Reference only existing `--color-*` variables for color.

- [ ] **Step 5: Preserve interaction and accessibility states**

Provide visible hover, selected, pressed, focus-visible, disabled, overlay-open, reduced-motion, and forced-colors styles. Do not suppress shared UI focus rings.

- [ ] **Step 6: Run focused regressions**

Run: `pnpm --filter @sapphirus/desktop-ui test -- App.test.tsx`  
Expected: PASS, including drawer inert/modal and accessibility tests.

- [ ] **Step 7: Checkpoint the task**

Run: `git diff --check` and `git diff -- apps/desktop-ui/src/components/TitleBar.tsx apps/desktop-ui/src/components/GlobalRail.tsx apps/desktop-ui/src/components/SessionRail.tsx apps/desktop-ui/src/styles.css`  
Expected: no whitespace errors and no business-logic changes.

## Task 3: Refine the task workspace, messages, review content, and composer

**Files:**
- Modify: `apps/desktop-ui/src/components/TaskWorkspace.tsx`
- Modify: `apps/desktop-ui/src/components/StageRail.tsx`
- Modify: `apps/desktop-ui/src/styles.css`
- Test: `apps/desktop-ui/src/components/TaskWorkspace.test.tsx`

**Interfaces:**
- Consumes: the existing `TaskWorkspaceProps` and `TaskStage` contracts unchanged.
- Produces: consistent task-header metadata, message-stream, review-summary, composer-body, composer-toolbar, and availability-note presentation regions.

- [ ] **Step 1: Add state-focused markup tests**

Assert that Method guidance preserves one main heading, the local-only notice, a labeled composer, availability text, and a disabled attach control. Add an assertion that a submitted intent creates a user message followed by the truthful unbound result without execution controls.

- [ ] **Step 2: Run the focused tests before markup changes**

Run: `pnpm --filter @sapphirus/desktop-ui test -- TaskWorkspace.test.tsx`  
Expected: existing behavior passes; any new presentation-region assertion fails until the hooks are added.

- [ ] **Step 3: Add presentation wrappers without moving state**

Keep `submitTask`, all local state, and all callbacks unchanged. Add class hooks around workspace identity, session utilities, message content, composer field, and composer meta. Preserve the `<main>`, `<header>`, `<form>`, `<textarea>`, live regions, and disabled semantics.

- [ ] **Step 4: Refactor task-workspace CSS**

Implement:

- a quieter, fixed task header with clear title hierarchy;
- a centered readable message column with deliberate wide review content;
- aligned message avatars and metadata;
- compact stage progress;
- restrained review summary and impact metrics;
- a stable composer with strong focus treatment and secondary toolbar;
- explicit preview, local-only, submitting, created-unbound, discarded, and recovery states.

Do not change any color token values or capability copy.

- [ ] **Step 5: Run focused behavior and accessibility tests**

Run: `pnpm --filter @sapphirus/desktop-ui test -- TaskWorkspace.test.tsx App.test.tsx`  
Expected: PASS with no duplicate submission, capability, recovery, or accessibility regression.

- [ ] **Step 6: Checkpoint the task**

Run: `git diff --check` and inspect only TaskWorkspace, StageRail, their tests, and related CSS sections.

## Task 4: Refine inspector, Context, Changes, Logs, Evidence, and BMAD Method surfaces

**Files:**
- Modify: `apps/desktop-ui/src/components/Inspector.tsx`
- Modify: `apps/desktop-ui/src/components/BmadHelpCard.tsx`
- Modify: `apps/desktop-ui/src/components/BmadLibraryPanel.tsx`
- Modify: `apps/desktop-ui/src/components/CodeDiff.tsx`
- Modify: `apps/desktop-ui/src/styles.css`
- Test: `apps/desktop-ui/src/components/BmadHelpCard.test.tsx`
- Test: `apps/desktop-ui/src/components/BmadLibraryPanel.test.tsx`
- Test: `apps/desktop-ui/src/App.test.tsx`

**Interfaces:**
- Consumes: existing inspector, BMAD projection, and diff data contracts unchanged.
- Produces: stable inspector chrome, scroll body, section header, technical metadata, status-chip, and action-footer presentation regions.

- [ ] **Step 1: Add hierarchy regression tests**

Assert that ready Help retains the four textual status facts, the recommendation heading, Source, Reason, Guidance, Expected artifacts, and Blockers. Assert that the Method Library retains separate Installed skills, Available actions, Method agents, and Internal identifiers sections.

- [ ] **Step 2: Verify focused tests pass before presentation changes**

Run: `pnpm --filter @sapphirus/desktop-ui test -- BmadHelpCard.test.tsx BmadLibraryPanel.test.tsx`  
Expected: PASS, establishing the content baseline.

- [ ] **Step 3: Add presentation-only inspector hooks**

Keep `Tabs`, selection callbacks, modal focus utilities, and all action callbacks unchanged. Add class hooks for the inspector header/tab strip, scroll body, technical sections, status facts, and sticky action footer. Keep all code values as text nodes and preserve current escaping.

- [ ] **Step 4: Refactor inspector and BMAD CSS**

Implement a sticky compact tab strip, consistent section headings, calm callouts, aligned metadata grids, readable diff/code scrolling, compact log columns, evidence empty state, status chips using existing semantic tokens, and first-class BMAD panels. Ensure long maximum-bound source strings wrap without clipping.

- [ ] **Step 5: Run inspector and BMAD tests**

Run: `pnpm --filter @sapphirus/desktop-ui test -- App.test.tsx BmadHelpCard.test.tsx BmadLibraryPanel.test.tsx`  
Expected: PASS, including keyboard tab changes, inert HTML-like content, maximum-bound wrapping content, and axe checks.

- [ ] **Step 6: Checkpoint the task**

Run: `git diff --check` and inspect the inspector/BMAD/diff path diff for behavior drift.

## Task 5: Refine Explorer, workspace management, settings, account, and overlays

**Files:**
- Modify: `apps/desktop-ui/src/components/WorkspaceExplorer.tsx`
- Modify: `apps/desktop-ui/src/components/WorkspacePanel.tsx`
- Modify: `apps/desktop-ui/src/components/UtilityPanel.tsx`
- Modify: `apps/desktop-ui/src/styles.css`
- Test: `apps/desktop-ui/src/App.test.tsx`

**Interfaces:**
- Consumes: existing read-only workspace source, workspace management modes, and preference props unchanged.
- Produces: one consistent primary-view header/body/footer language and one consistent modal anatomy.

- [ ] **Step 1: Extend view and dialog regression coverage**

Assert Explorer retains its Local workspace header, Files/Search/BMAD tabs, bounded preview, context selection footer, and browser-demo notice. Assert Workspaces, Settings, and Account dialogs retain their current accessible names and focus-return behavior.

- [ ] **Step 2: Run the focused tests**

Run: `pnpm --filter @sapphirus/desktop-ui test -- App.test.tsx`  
Expected: PASS before visual markup changes.

- [ ] **Step 3: Add consistent presentation hooks**

Add shared anatomy classes such as `.view-header`, `.dialog-header`, `.dialog-body`, and `.dialog-footer` alongside existing specific classes. Do not replace roles, focus effects, focus traps, or callback behavior.

- [ ] **Step 4: Refactor Explorer and modal CSS**

Align the Explorer header with the task header, refine Files/Search/BMAD tabs and tree rows, improve preview and context-selection surfaces, and normalize loading/empty/error states. Standardize workspace/settings/account dialog spacing, close controls, preference rows, action placement, and backdrop without changing palette values.

- [ ] **Step 5: Run behavior and accessibility regressions**

Run: `pnpm --filter @sapphirus/desktop-ui test -- App.test.tsx`  
Expected: PASS, including stale request suppression, workspace recovery, dialog focus, responsive drawers, and both axe scans.

- [ ] **Step 6: Checkpoint the task**

Run: `git diff --check` and inspect only Explorer, panel, utility, App-test, and corresponding CSS changes.

## Task 6: Complete responsive, density, motion, and high-contrast polish

**Files:**
- Modify: `apps/desktop-ui/src/styles.css`
- Test: `apps/desktop-ui/src/App.test.tsx`
- Test: `apps/desktop-ui/src/styles.visual-contract.test.ts`

**Interfaces:**
- Consumes: existing `useMediaQuery` breakpoints, drawer props, `data-density`, theme attributes, reduced-motion query, and forced-colors query.
- Produces: coherent wide, medium, compact, and narrow workbench layouts with unchanged modal semantics.

- [ ] **Step 1: Consolidate breakpoint responsibilities**

Keep the existing breakpoint order and assign each one a single responsibility:

- `1350px`: inspector becomes a right overlay;
- `1050px`: supporting dimensions tighten;
- `820px`: sessions become a left overlay;
- `620px`: narrow task/composer/tab/dialog layout;
- short viewport query: vertical-density protection.

Remove superseded declarations rather than appending contradictory overrides.

- [ ] **Step 2: Verify overlay behavior in tests**

Run: `pnpm --filter @sapphirus/desktop-ui test -- App.test.tsx -t "responsive drawers"`  
Expected: PASS with closed drawers inert and open drawers modal.

- [ ] **Step 3: Complete compact density, reduced motion, and forced colors**

Ensure compact density reduces spacing without shrinking interactive hit areas below the shared control minimum. Disable non-essential transitions under reduced motion. In forced colors, preserve selected indicators, focus, borders, and readable status differentiation.

- [ ] **Step 4: Re-run the palette contract**

Run: `pnpm --filter @sapphirus/desktop-ui test -- styles.visual-contract.test.ts`  
Expected: PASS; existing color values remain unchanged.

- [ ] **Step 5: Run the complete renderer gate**

Run:

```powershell
pnpm --filter @sapphirus/desktop-ui test
pnpm --filter @sapphirus/desktop-ui typecheck
pnpm --filter @sapphirus/desktop-ui lint
pnpm --filter @sapphirus/desktop-ui build
```

Expected: all UI tests pass, TypeScript exits 0, lint exits 0, and Vite produces the production bundle.

- [ ] **Step 6: Checkpoint the task**

Run: `git diff --check` and `git status --short -- apps/desktop-ui packages/ui docs/superpowers`  
Expected: no `packages/ui/src/tokens.css` change and no unrelated paths.

## Task 7: Visual QA and final verification

**Files:**
- Create: `design-qa.md`
- Modify: presentation files from Tasks 2-6 only when QA identifies a material mismatch.

**Interfaces:**
- Consumes: supplied prototype reference, current local browser render, and all automated gates.
- Produces: a `design-qa.md` result of `final result: passed` with any remaining P3 polish notes.

- [ ] **Step 1: Run the local renderer**

Run the desktop UI Vite server using the repository's pinned Node/pnpm toolchain and a fixed available port. Keep the process running for capture.

- [ ] **Step 2: Capture matching states**

Capture at least:

- 1600x992 Agent workbench matching the reference state;
- medium width with inspector overlay open;
- compact width with sessions overlay open;
- 390px narrow layout;
- Explorer populated state;
- Method inspector ready state;
- Settings and Workspaces dialogs.

- [ ] **Step 3: Compare reference and implementation together**

Evaluate panel proportions, alignment, spacing, typography, control sizing, selected states, borders, radii, overflow, composer anchoring, and inspector scrolling. Judge color only against the Sapphirus palette constraint, not the reference's purple palette.

- [ ] **Step 4: Record and fix findings**

Create `design-qa.md` with severity, surface, evidence, fix, and result. Fix all P0/P1/P2 findings, recapture the affected state, and repeat until the file states `final result: passed`. Leave only non-blocking P3 polish notes.

- [ ] **Step 5: Run final verification from a clean renderer process**

Run the complete renderer gate from Task 6 again and verify no console errors during primary navigation, panel opening, tabs, composer input, and dialogs.

- [ ] **Step 6: Final scope audit**

Run:

```powershell
git diff --check
git status --short
git diff -- apps/desktop-ui docs/superpowers design-qa.md
```

Expected: only the approved design spec, implementation plan, renderer presentation/tests, and QA report are part of this refactor; existing unrelated work remains untouched.

## Self-review result

- **Spec coverage:** shell, all visible surfaces, palette lock, state handling, responsive behavior, accessibility, BMAD panels, dialogs, testing, and visual QA are each mapped to a task.
- **Placeholder scan:** no TBD, TODO, unspecified error handling, or deferred implementation step remains.
- **Type consistency:** no existing public prop, callback, projection, host-client, or state type is renamed. New names are CSS presentation hooks only.
- **Execution choice:** inline execution is required in this side conversation because sub-agents and shared Git-state mutation are out of scope.
