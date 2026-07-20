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

- (filled during execution)
