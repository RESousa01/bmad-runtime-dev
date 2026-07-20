# Implementation packet: P6 — offboarding data retention

## Authority and intent

- Owning authority: repository maintainer (RodrigoSousa0). Executes the
  P0 open decision "Uninstall/offboarding data retention" under ADR-0004
  (application side; the installer side stays with the P1 lane).
- User-visible outcome: Settings gains an Offboarding section showing an
  honest retention manifest (categories, counts, bytes — never paths or
  content) and an explicit, typed-confirmation erase that cryptographically
  destroys all app-owned data while leaving every workspace folder and the
  installed program untouched.
- Contracts read: the reviewed 31-command catalog (grows to 33 through all
  five pin sites), `desktop-store` schema surface, IPC envelope shapes.
- Non-goals: installer changes (P1 lane), cloud-side retention (D2-E
  operator policies), data export tooling (workspace folders already hold
  all work product), any background or automatic deletion.
- Stop conditions: erase would need to touch a workspace folder; the
  manifest cannot be produced without leaking paths/content; the store
  cannot guarantee key destruction ahead of row deletion failing midway.

## Tests first

- Success: inspect returns bounded categories with plausible counts on a
  populated store; erase with the exact phrase deletes the store key,
  empties every authority table, removes content-addressed payloads,
  revokes grants, bumps the model-auth epoch, and drops the session to
  read-only recovery; workspace files on disk are byte-identical after
  erase; a fresh launch after erase initializes a new identity.
- Negative/bypass: erase without the exact phrase (wrong, empty, extra
  fields) fails closed with no deletion; inspect/erase are absent from the
  recovery command surface; the manifest serialization contains no path
  separators, hashes, or identifiers beyond bounded category labels.
- Compatibility: the prior 31 commands and their replies are byte-stable;
  renderer catalog mirrors stay in lockstep (boundary scan).

## Change and rollback

- Lanes: desktop-store erase/manifest API; desktop-runtime command +
  desktop-ipc envelope/projection; desktop-app state + handlers; renderer
  settings surface + client; docs/evidence. One commit per lane.
- Rollback: revert renderer, then host, then store lanes; no persisted
  format changes (erase only deletes).

## Exit gate

- Workspace fmt/clippy/tests green; renderer suite green;
  `pnpm verify:deferred-full` green; boundary scan green with the
  33-command catalog; clean `git status`.

## Review ledger

- 2026-07-20 — Store lane reviewed: erase destroys `store.key` first
  (crypto-erase before any fallible deletion), swaps the live connection
  to an inert in-memory handle so Windows releases the database file,
  and a fresh `LocalStore::open` after erase initializes a clean
  identity. Manifest labels are compile-time constants; counts only.
- 2026-07-20 — IPC lane reviewed: both commands admitted through the
  validated envelope with `deny_unknown_fields`; erase parses only the
  exact phrase `erase-local-authority-data` (case, whitespace, and
  extra-field variants fail closed at the boundary, before dispatch).
- 2026-07-20 — Host lane reviewed: `HostState::offboard_erase` holds the
  workspace-commit lock, signs out model authority (ADR-0002 context
  epoch withdrawal; epoch exhaustion deliberately cannot block erasure),
  revokes every grant, erases the store, then releases the Ready read
  guard before `enter_recovery` takes the write half — no lock-order
  inversion. Erase is rejected from recovery mode, so it is not
  repeatable and is absent from the recovery surface.
- 2026-07-20 — Renderer lane reviewed: manifest parser rejects any
  category label outside `^[a-z][a-z_]*$` (path separators, spaces,
  identifiers), the Settings "Local data" pane arms erase only on the
  byte-exact typed phrase, and the erased terminal state hides the
  action. Catalog pins moved 31→33 in lockstep at all five sites
  (READY_COMMANDS + count test, envelope allowlist, renderer contracts
  array + union, commandCatalog.test.ts, tools/check-boundaries.mjs).
- 2026-07-20 — Exit gate: `cargo fmt --check` clean; workspace clippy
  `-D warnings` clean; 59 workspace test suites ok (incl. 2 new envelope,
  2 new store, 2 new host tests); renderer 25 files / 336 tests ok;
  `pnpm verify:deferred-full` ok; boundary scan ok with the 33-command
  catalog.
