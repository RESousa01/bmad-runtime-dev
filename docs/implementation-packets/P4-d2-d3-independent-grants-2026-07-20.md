# Implementation packet: P4 — independent D2 context-read and D3 governed-edit grants

## Authority and intent

- Owning authority: repository maintainer (RodrigoSousa0); executes the P0
  open decision "Representation of independent D2 (context-read) vs D3
  (governed-edit) grants", locked in ADR-0002.
- User-visible outcome: enabling governed edits no longer invalidates an
  in-review BMAD Help request, and signing out of model access withdraws
  context-read authority (Help lifecycle fails closed) without destroying
  governed-edit proposals or any local work. Whole-workspace revocation
  still invalidates everything.
- Contracts read: pinned `host_dispatch` command payloads (unchanged),
  `crates/desktop-ipc` envelope shapes (unchanged), persisted workspace
  grant projections (additive with serde defaults).
- Non-goals: renderer UX for per-vertical grant display (deliberate future
  catalog change); production support-plane behavior (P3, complete);
  BMAD breadth (P5); any new Tauri command, permission, or IPC field.
- Stop conditions: any change would require altering a pinned renderer
  payload or catalog hash; independence would weaken an existing
  fail-closed invariant (for example whole-grant revocation no longer
  invalidating a vertical).

## Tests first

- Success fixtures:
  - desktop-workspace: `enable_governed_edits` bumps only
    `governed_edit_epoch`; snapshots/read bindings issued before the bump
    remain valid; governed mutations validate the exact edit epoch and
    reject a stale one; `advance_context_read_epoch` bumps only the
    context-read epoch; persisted projections without the new fields
    deserialize with both epochs = 1.
  - desktop-app D3: authority captured before a context-read epoch advance
    still applies its proposal afterward; authority captured before
    `enable_governed_edits` is rejected afterward with the existing
    conflict error.
  - desktop-app D2: a Help lifecycle prepared before `enable_governed_edits`
    still approves and submits afterward; a Help lifecycle prepared before
    `model.auth.sign_out` is rejected at approve/submit afterward while a
    D3 proposal from before the sign-out still applies.
- Negative/bypass fixtures: a stale `governed_edit_epoch` cannot reach any
  governed mutation; a stale `context_read_epoch` cannot reach consent
  creation or submission; whole-grant `revoke` invalidates both verticals.
- Compatibility fixture: previously persisted grant JSON (no new fields)
  restores and operates; all pinned renderer payloads and the 28-command
  catalog are byte-identical.

## Change and rollback

- Files/surfaces allowed (one commit per lane):
  1. Decision/docs: `docs/adr/ADR-0002-*`, this packet.
  2. Workspace authority: `crates/desktop-workspace/src/lib.rs`,
     `crates/desktop-workspace/src/governed_io.rs`.
  3. Desktop composition: `crates/desktop-app/src/edits.rs`,
     `crates/desktop-app/src/recovery.rs`,
     `crates/desktop-app/src/state.rs`,
     `crates/desktop-app/src/commands.rs`,
     `crates/desktop-app/src/bmad_model/**` (host-only).
- Rollback: revert the desktop-app lane, then the workspace lane; the doc
  lane is independent. Persisted grants are forward/backward compatible
  (additive fields with defaults), so rollback needs no data migration.
- Observability/evidence: workspace-wide fmt/clippy/tests, renderer suite
  unchanged, `verify:deferred-full`, evidence recorded below.

## Exit gate

- `cargo fmt --check`, workspace clippy `-D warnings`, workspace tests all
  green.
- Renderer full suite green with zero renderer/catalog diffs.
- `pnpm verify:deferred-full` green.
- Clean `git status`; lanes committed separately.

## Review ledger

- (filled during execution)
