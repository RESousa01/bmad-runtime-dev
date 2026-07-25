# Forge Desktop v2 Cutover Contract

**Date:** 2026-07-22  
**Status:** Implemented and locally verified in `codex/forge-v2`  
**Scope:** Public product identity, renderer cutover, Review, local context inspection, and authority visibility

## Product boundary

The public desktop product is **Forge**. The signed Windows application is
`forge.exe`, the NSIS artifact is `Forge_<version>_x64-setup.exe`, and release
metadata and the CycloneDX application component use the Forge name.

Compatibility-sensitive identities remain unchanged in this cutover:

- Tauri identifier `com.sapphirus.desktop`
- existing `@sapphirus/*` workspace package names
- existing schema IDs, hash preimages, evidence namespaces, environment variables,
  runner labels, and persisted-data contracts

Those values are protocol or deployment identities, not public product copy. A
future migration may replace them only with explicit data, policy, and release
compatibility work.

## Renderer cutover and rollback

**Superseded 2026-07-25 — the two renderer packages were merged.** The cutover
originally ran `apps/desktop-ui-v2` as a thin seven-file entry point around the
shared `apps/desktop-ui` controller, keeping the legacy surface reachable as a
rollback boundary behind an `App surface` prop. Both halves are now one package:
`apps/desktop-ui`, published as `@forge/desktop-ui`, holding the v2 entry point,
document metadata, and the whole controller and component tree. `desktop-ui-v2`
no longer exists, the `surface` prop and every `visualSystem === "legacy"`
branch are deleted, and renderer-boundary checks cover the single source root.

The complete pinned upstream checkout lives at `library/opencode`; Forge-owned
React adaptations are tracked with source-file references and MIT attribution.

The v2 shell follows OpenCode's compact neutral palette, 36-pixel title bar,
flat controls, two-column project/task overview, and source icon geometry. The
adapted source and MIT license are recorded in `THIRD_PARTY_NOTICES.md`.

Rollback is configuration-only: point Tauri's `frontendDist` and pre-build
commands back to the legacy renderer package. The native command surface,
contracts, persisted state, and compatibility identifier do not change.

## Workbench behavior

Forge uses one compact task workbench rather than separate application modes.
The task surface keeps the current session, workspace explorer, BMAD Help,
governed edit review, recovery, activity, settings, and host-status behavior.

Workspace management uses the same flat OpenCode-shaped hierarchy as the rest
of the shell: one compact header, a host-mode row, an unboxed workspace list,
and a quiet footer action. Forge still keeps the existing grant, revocation,
and path-disclosure boundaries; this is a presentation change, not a new
renderer authority path.

The former public **Changes** surface is **Review**. Review has two explicit
sources:

1. **Git changes** (default) presents read-only repository state.
2. **Agent proposals** presents the existing governed proposal, decision, and
   recovery flow.

Git changes supports these comparisons:

- All: `HEAD` to working tree
- Staged: `HEAD` to index
- Unstaged: index to working tree

Review replaces the task canvas while open. It adapts OpenCode's
`SessionReviewV2` structure: a 240-pixel filterable hierarchical file tree, file
navigation, expanded/collapsed line controls, unified/split controls, and a
full-height per-line Git difference. Split mode aligns before and after line
numbers and content in parallel panes.

The host owns repository discovery and all bytes shown in the diff. Status is
bounded to 500 paths. Each diff side is bounded to 64 KiB and reports
truncation. Requests require a live workspace grant and normalized relative
path; they do not invoke a shell, stage, discard, or commit anything. Arbitrary
Git mutation is intentionally outside this cutover.

## Context window

The composer shows a compact **Context** status row sourced from the host's exact
`context.preview` projection. Opening it replaces the task canvas with an
OpenCode-shaped session Context window: two-column provider, model, usage,
token, message, cost, and time metrics followed by expandable line-numbered raw
message JSON. Raw messages are constructed only from renderer-safe BMAD request
and completion projections; the view does not bypass the host or expose native
authority state. The meter translates the
existing 256 KiB local preview ceiling into a 64K estimated-token ceiling at
four bytes per token. It is a local preview limit, not a claim about any
provider model's context window.

The **Files** tab follows the same full-window model. It keeps the existing
bounded host traversal, search, BMAD asset scan, selection, and UTF-8 preview
contracts while presenting them as a compact file-tree sidebar and code preview
instead of a nested utility card.

Manual selection stays available. **Auto** performs deterministic, bounded
local workspace searches derived from the task draft, ranks a file at most once
per search term, selects at most eight paths, then asks the host for the same
exact context preview used by manual selection. Stale async results are ignored
when the workspace grant or bootstrap generation changes. No selected bytes are
sent until the exact outbound request review is approved.

## Authority presentation

The workbench does not expose user-selectable authority profiles. The earlier
Observe/Assist/Build presentation was removed because it looked like a policy
switch even though it never granted authority.

The renderer now presents only concrete host-projected facts and actions:
workspace access, reviewed model requests, and governed-edit availability for
the current grant epoch. Opening a panel or selecting a route never grants
access, enables edits, or changes policy. The host remains the sole authority
for workspace access, model egress, and local effects.

## Deliberate exclusions

- no PTY, terminal, or generic process bridge
- no renderer filesystem access
- no arbitrary stage, discard, commit, or branch operations
- no autonomous edit authority
- no model-window claim before provider/request binding
- no migration of persisted/schema/deployment identifiers in the branding cutover
- no claim that visual task tabs are durable or host-isolated Agent sessions;
  native projection requests remain sessionless until a dedicated task/session
  ownership milestone

## Verification contract

The cutover is complete only when all of the following pass from the pinned
toolchain:

- Forge branding and release metadata regressions
- renderer unit, integration, accessibility, typecheck, and production build
- architecture boundary checks
- Rust formatting, lint, and workspace tests
- responsive browser inspection of the active v2 entry point
- unsigned deterministic NSIS build plus artifact-name and icon inspection

The first Forge release must separately qualify upgrade behavior from the last
signed legacy-named installer. Retaining the Tauri identifier is necessary for
compatibility but is not, by itself, proof that the NSIS rename migration works.
