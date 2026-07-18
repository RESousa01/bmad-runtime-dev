# Frontend evolution and backend completion (2026-07-18)

Record of the Claude-Code-inspired renderer evolution and the host commands that
back it. The semantic palette remains contractual and untouched.

## What changed

- **Composer** fills the workspace width (design-polish cap lifted) with an
  auto-growing textarea (`field-sizing: content`, 40vh cap).
- **Settings** is a full dialog (`components/settings/SettingsDialog`) with
  General / Appearance / Agent & model / Workspaces / Skills & agents /
  Updates / About sections. Theme and density persist host-side through
  `app.preferences.get` / `app.preferences.set` (LocalStore aggregate
  `renderer_preferences`/`local`; the boundary auditor forbids renderer
  localStorage). Updates stays **status-only** — in-app install remains
  withheld until organization signing exists. About reads `app.about`
  (version, installation id, foundation package, update posture).
- **Agent selector** (`components/composer/AgentSelector`) replaces the old
  read-only popover: BMAD Help capability plus the six Method agents from the
  library snapshot, unavailable entries disabled with blocker text, model
  access + review-before-send in the footer. The settings shortcut was removed.
- **Right panel** is one tabbed drawer — Files | Changes | Activity | Skills.
  **Activity** is the ADR-0001-compliant terminal substitute: a read-only
  timeline of governed executions (`changes.history`), open-journal attention
  banner, undo affordance, and skill-guidance receipts. No process execution
  exists or is implied; a real terminal remains gated on a future E3
  containment ADR. Bounded read-only file viewing continues through the Files
  tree (`workspace.read_text`, 1 MiB cap).
- **Attach files** offers "Browse files…" backed by the new
  `workspace.pick_files` host command: the host opens the multi-file dialog,
  canonicalizes each pick, keeps only regular non-reparse files strictly inside
  the granted root (component-wise containment; sibling-prefix trap pinned in
  tests), and returns relative paths plus rejection counts — no absolute path
  ever crosses IPC. Cap 100 files; cancel maps to `no_selection`.
- **BMAD Builder packages** (agent + workflow, bundled inactive) now appear in
  the library snapshot (`bmad-library-snapshot.v2`, `builderPackages[]`) and
  the library panel as "Builder (installed, inactive)" with a 12-hex display
  fingerprint (full digests stay out of the sealed projection). Activation
  remains a gated local decision per Note 14 — no activation path was added.
- Removed legacy components (Inspector, GlobalRail, SessionRail, StageRail,
  CodeDiff) and their dead CSS; the old UtilityPanel was replaced.

## Catalog lockstep

READY commands went 24 → 28 (`app.preferences.get`, `app.preferences.set`,
`app.about`, `workspace.pick_files`), appended in identical order in:
`crates/desktop-app/src/commands.rs`, `crates/desktop-ipc/src/envelope.rs`,
`apps/desktop-ui/src/lib/hostClient/contracts.ts`, and
`tools/check-boundaries.mjs`. A renderer tripwire test
(`lib/hostClient/commandCatalog.test.ts`) pins the exact ordered list.
Recovery mode still exposes only `app.get_boot_state` and `workspace.list`.
