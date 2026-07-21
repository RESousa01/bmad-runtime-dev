# Sapphirus shell rework — OpenCode desktop anatomy, adapted

Target anatomy observed from the OpenCode desktop app (v1.18.x), re-expressed
for Sapphirus's governed model. All components are original Sapphirus
implementations and branding; OpenCode is layout/interaction reference only.

## 1. Title bar = app strip (replaces sidebar-first navigation)

One slim bar owning navigation, left to right:
- **Menu button** (hamburger): opens an app menu (File / Edit / View / Go /
  Window / Help equivalents). Until native menus land, it opens the command
  palette scoped to those groups.
- **Home button** (grid icon): shows the tasks overview.
- **Task tabs**: one tab per open task ("New task" default), close X on each,
  trailing **+** creates a task. Map to the existing single-session model
  first (one tab + "+" = startNewSession), multi-session later over
  `run.create`.
- Spacer, then the existing window controls (min/max/close).

Sapphirus adaptations: keep the drag region; keep `Local host ready` chip in
the workspace header (not the strip); sidebar collapses away — workspace
picker moves into the home view breadcrumb.

## 2. Home view (new-task state)

Centered column, generous whitespace:
- Sapphirus wordmark, large and dim (brand mark + name, not ASCII art).
- **Large composer card**: placeholder "Describe your intent — / for
  commands, @ for context…", attach (+) on the left, primary send at right.
- Under the composer input, inline chips: **Agent selector** (BMAD Help /
  capability agents) — the analog of OpenCode's model chip.
- Below the card: breadcrumb line — `⌂ <workspace-name> ▾ / <permissions>`
  (workspace switcher dropdown; "Read only" / "Governed edits" instead of
  git status).
- Footer hint line: what the governed flow does (nothing sent until
  approval), replacing the provider-count marketing line.

## 3. Tasks overview (home button)

- Top: **Search tasks** input, full width.
- Left rail: Workspaces list (analog of Projects), then Settings, Help.
- Main: task list; empty state — "Nothing here yet / Create a task to get
  started" + primary **New task**.

## 4. Settings dialog

Two-pane layout: left nav grouped (**Desktop**: General, Shortcuts;
**Governance**: Workspace, Model access, Offboarding), right pane as titled
sections of row items (label + description + trailing control). Version
footer bottom-left. Sapphirus already has SettingsDialog sections — regroup
into this layout.

## 5. Slice order

1. Title strip with task tab + home + menu buttons (this commit).
2. Home view: centered wordmark + big composer + breadcrumb.
3. Tasks overview page behind the home button.
4. Settings regroup into two-pane rows.
5. App menu groups (File/Edit/View/Go/Window/Help) via palette scopes.
6. Multi-task tabs over `run.create`.
