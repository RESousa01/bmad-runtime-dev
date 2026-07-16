# Implementation packet: D2-D reviewed BMAD Help desktop composition

## Authority and intent

- Owning authority: `desktop-egress` owns exact context review and single-use consent;
  `desktop-cloud` owns authorized request/verified-response transport contracts;
  `desktop-app::bmad_model` alone composes them with the durable Method session.
- User-visible outcome: an explicit deterministic-development build can review the exact bounded
  Help context, approve without sending, submit once, verify the response and receipt, and retain a
  renderer-safe canonical result. The default build fails offline before context projection or
  Method mutation.
- Contracts read: the sealed BMAD Help binding, Method session v1 lifecycle, D2 context manifest,
  model invocation binding, model access receipt, and strict host-dispatch envelopes.
- Non-goals: production identity, Azure deployment, managed model brokerage, production receipt
  signing, installer signing, or allowing model output to authorize file effects.
- Stop conditions: any authority substitution, consent replay, context drift, response/receipt
  mismatch, unsafe IPC field, default deterministic fallback, or D2/D3 approval crossover.

## Tests first

- Success fixture: explicit `deterministic-help` composition follows
  `Created(v1) -> CapabilityBound(v2) -> ContextReviewRequired(v3) -> Ready(v4) -> Advancing(v5)
  -> Completed(v6)` and dispatches exactly once.
- Negative/bypass fixture: stale manifest/decision, expiry, renderer/workspace drift, duplicate
  submit, invalid schema, invalid receipt, and transport failure all fail closed.
- Failure/recovery fixture: post-consumption failures retain the interrupted v5 Method record and
  cannot resume or redispatch; completed projections are retained atomically for restart recovery.
- Compatibility fixture: the default build reports `support_plane_offline` before reviewed context
  or consent exists; existing local context preview remains model-target-free.

## Change and rollback

- Files/surfaces allowed: D2/Method Rust crates, the single `host_dispatch` envelope, bounded
  renderer projections, the existing desktop UI, implementation evidence, and an explicit
  deterministic-only developer script.
- Disable or rollback path: omit `desktop-app/deterministic-help`; the normal `desktop:dev` and
  `desktop:build` scripts remain default-offline and contain no deterministic fallback.
- Observability/evidence: stable sanitized lifecycle/error projections, canonical receipt metadata
  without proof material, durable completed-run projection, and repository gate results below.

## Review ledger

- Trusted bridge commit: `92f890a3` (`feat(d2): seal the Method consent bridge`).
- Atomic completion commit: `24b9dc81` (`feat(bmad): retain completed Help projections atomically`).
- Exact review preparation commit: `e600cbe5` (`feat(bmad): prepare exact reviewed Help requests`).
- One-shot coordinator commit: `f9e5456f` (`feat(bmad): compose one-shot verified Help requests`).
- Paused checkpoint (2026-07-16): implementation was explicitly stopped before D3 integration,
  commit, staging, or deployment. The following D2-D changes are intentionally still uncommitted in
  the isolated `C:\tmp\d2` worktree and must be re-reviewed as one diff before resuming.
- Implementer full-diff review: two lock/invalidation findings were corrected during the checkpoint;
  a final stable-diff review remains required after the UI wiring is completed.
- Independent bug/security review: complete for the coordinator/state checkpoint; repeat after D3
  merge and after the final UI diff stabilizes.
- Verified commands:
  - `cargo test -p desktop-app --lib --all-features --locked` — 50 passed.
  - `cargo test -p desktop-app --lib state::tests --all-features --locked` — 7 passed.
  - `cargo test -p desktop-ipc --locked` — 50 passed across unit, projection, completion, and
    strict model-access contract tests.
  - Earlier focused baseline: `cargo test -p desktop-app --lib bmad_model --all-features --locked`
    — 21 passed; default variant — 14 passed; `cargo clippy -p desktop-app --all-targets
    --all-features --locked -- -D warnings` — passed.
- Checkpoint corrections verified:
  - recovery now obtains the Ready write authority before the Help coordinator, with a regression
    test proving the coordinator remains available while recovery waits for Ready readers;
  - model sign-out invalidates pending Help before advancing a renderer-safe identity epoch and
    fails closed at the JavaScript-safe epoch bound;
  - workspace revocation test coverage verifies active Help authority is cleared.
- Work still pending at stop: complete/re-run focused UI tests and rendered smoke, final IPC handler
  race review, D3 merge and cross-authority tests, full workspace/source/.NET gates, native
  deterministic/default-offline smoke, commit/review, and Azure preparation/deployment (last).
- Remaining risks: production D2-E identity, entitlement, consent reconciliation, managed broker,
  signed receipt verification, Azure deployment, and release signing are not closed by D2-D.
