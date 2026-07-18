# Implementation packet: P2 — reviewed D3 restart recovery

## Authority and intent

- Owning authority: the native `desktop-app` composition root, authenticated
  desktop store, and exact governed-workspace grant. The renderer presents
  bounded projections and retains only process-local one-shot approval state.
- User-visible outcome: after an interrupted governed edit, restart never
  mutates files automatically. The user can inspect a bounded recovery review
  and restore the checkpoint only after re-enabling governed edits for the
  exact workspace and approving a fresh, short-lived, recovery-only decision.
- Design authority:
  `docs/superpowers/specs/2026-07-18-p2-d3-recovery-authorization-design.md`.
- Non-goals: discarding a journal, assuming an effect recovered, retrying an
  interrupted restore, model-originated proposals, live cloud integration, or
  weakening the existing update blocker.

## Implemented behavior

- Boot quarantines interrupted effects as `recovery_required` without touching
  workspace files. An interrupted `restoring` journal becomes terminal
  `manual_review`; neither state permits update installation.
- Recovery planning authenticates the journal and checkpoint, re-observes the
  exact bounded relative paths, and binds historical journal provenance
  separately from the current governed-workspace authority.
- `changes.recovery.prepare` returns only a read-only, renderer-safe review.
  `changes.recovery.decide` consumes a fresh, short-lived, single-use approval
  before any restore attempt. Workspace, installation, renderer session,
  journal, execution, review hash, current grant epoch, and historical journal
  epoch are all bound and revalidated.
- Restore writes the exact checkpoint bytes through governed I/O, verifies the
  restored post-state, finalizes the journal durably, and cannot be replayed.
  Any interrupted or inconsistent restore fails closed to `manual_review` and
  remains update-blocking.
- Changes and Activity use one shared accessible review component. Host-owned
  closed availability/reason codes control entry points; approvals are never
  persisted and are invalidated on authority, history, bootstrap, expiry, or
  lifecycle drift.

## Tests first and defect closure

- The integrated restart fixture was first run red. A legitimate restart under
  a fresh governed epoch was rejected with `Recovery belongs to a different
  workspace authority.`
- Root cause: the retained journal's historical grant epoch was incorrectly
  required to equal the fresh post-restart governed epoch.
- Fix: durable journal provenance remains authenticated and is included in the
  recovery-plan binding, while current governed I/O and approval authority use
  the exact fresh epoch. A pending approval binds both epochs. Historical
  epochs may not be substituted, and a journal claiming a future epoch is
  quarantined.
- The final integrated fixture proves: restart is non-mutating and
  update-blocking; stale approval is unusable; fresh review restores exact
  checkpoint bytes exactly once; the recovered journal disappears from the
  open set; and interruption after durable `restoring` becomes terminal
  `manual_review` and remains update-blocking.
- A separate regression rejects stored historical-epoch substitution, a
  future current-epoch request, and cross-workspace drift before mutation.

## Exact-revision qualification

The executable code revision qualified here is
`0eea58a4f940c65afca5ad43da37e7f8959eac02`. Later packet/README/BigBrain edits
are documentation-only and do not change the qualified product tree.

Main checkout proof under Node 24.18.0, pnpm 11.12.0, Rust/Cargo 1.97.0:

- `cargo fmt --all -- --check` — pass.
- `cargo clippy --workspace --all-features --all-targets -- -D warnings` — pass.
- `cargo test --workspace --all-features --locked` — 481 counted tests,
  including 15 compile-fail doctests; zero failures or ignored tests.
- `pnpm --filter @sapphirus/desktop-ui test --run` — 24 files, 322/322 pass.
- `pnpm verify:source` — pass: BMAD foundation 59/59, TypeScript contracts 85
  pass plus one documented Windows file-link `EPERM` skip, release regressions
  23/23, renderer 322/322, typecheck/lint, boundaries, secret scan, and the
  3,089-module production build.
- `pnpm contracts:verify:cross-language` — 104 pass, one documented Windows
  file-link `EPERM` skip, zero failures; 90 qualification files and 2,777
  generated files checked. The plan's older `qualify:cross-language` spelling
  is not a repository script; the named authoritative command above was run.

Independent clean-worktree proof used detached `C:\tmp\p2q` at that exact full
revision:

- `pnpm install --offline --frozen-lockfile` reused 176/176 packages and
  downloaded zero.
- `pnpm verify:source` passed with the same functional counts; its secret scan
  examined 3,329 active clean-checkout files.
- Full all-feature workspace Rust tests passed with the same 481 counted tests
  and no failures/ignored tests.
- Renderer tests passed 322/322 across 24 files.
- The integrated native restart fixture passed 1/1, and
  `tools/check-boundaries.test.mjs` passed 2/2.
- `git status --short` was empty and `git rev-parse HEAD` returned the exact
  revision above.

The clean-worktree install, source verification, Rust tests, renderer tests,
and focused native boundary needed exact retries outside the restricted runner
after Windows returned `EPERM`/access-denied errors for generated or target
paths. Those were runner restrictions, not product skips. The only test skip is
the documented Windows file-link probe reported above. Existing non-failing
messages remain the axe/jsdom canvas notice and Vite large-chunk warning.

## Review ledger and exit posture

- Implementer review: complete for the integrated restart fixture, historical
  versus current grant-epoch correction, strict regression, and exact-revision
  qualification. Broad Clippy also exposed a test-only needless raw-string
  delimiter in `desktop-store`; the mechanical correction was focused-tested
  before the exact revision was qualified.
- Independent adversarial review of the complete P2 delta: **pending**. No
  claim is made that P2 is release- or merge-complete until that review closes
  every P0/P1 finding and its verdict is recorded here.
- P0/P1 carry-forward review: **pending as part of the same independent
  whole-delta review**. Local green qualification is not a substitute for it.
- Remaining operational path: there is deliberately no discard or automatic
  retry for `manual_review`; operator resolution remains a future separately
  designed authority. Production signing and release evidence remain P1
  concerns, not evidence supplied by this packet.

No readiness percentage was changed by this packet. The measured evidence
closes the local P2 implementation/qualification work only; aggregate product
readiness must be reassessed separately after independent review.
