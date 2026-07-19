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

READY commands first went 24 → 28 (`app.preferences.get`,
`app.preferences.set`, `app.about`, `workspace.pick_files`) and reviewed
recovery then extended the exact catalog to 30 with
`changes.recovery.prepare` and `changes.recovery.decide`. The commands remain
in identical order in:
`crates/desktop-app/src/commands.rs`, `crates/desktop-ipc/src/envelope.rs`,
`apps/desktop-ui/src/lib/hostClient/contracts.ts`, and
`tools/check-boundaries.mjs`. A renderer tripwire test
(`lib/hostClient/commandCatalog.test.ts`) pins the exact ordered list.
Recovery mode still exposes only `app.get_boot_state` and `workspace.list`.

## Reviewed restart recovery

- Changes and Activity now share one `RecoveryReview` surface for a native
  `recovery_required` journal. Entry availability is a closed host projection;
  the renderer does not infer authority from journal state.
- Preparation is filesystem-read-only and request-ID tracked. An identical
  replay short-circuits before host observation or authority creation, while a
  changed request under the same ID conflicts. It shows bounded relative paths
  and fixed, renderer-owned operation explanations without exposing checkpoint
  bytes, absolute paths, hashes, approval identifiers, or native diagnostic
  text.
- Restore requires governed edits to be re-enabled for the exact workspace and
  a fresh process-local approval. The client consumes its one-shot authority
  before dispatch and invalidates it on expiry or relevant authority/history
  drift. Refresh and duplicate actions stay disabled while a decision is in
  flight, including across the expiry boundary.
- Success is projected only after the host durably verifies and finalizes the
  restore. Failed/interrupted restoration becomes non-actionable
  `manual_review`; the UI offers neither discard nor automatic retry.
- Renderer qualification is 322/322 tests across 24 files at executable code
  revision `23d9add3fef372243d11460c4cf04a2a6881d714`, in both the main checkout
  and detached clean proof. Native-host copy no longer implies signing. The
  first independent whole-P2 review's two Important and five Minor findings
  are fixed; independent re-review approved the final revision with zero
  findings and no P0/P1 carry-forward issue.
