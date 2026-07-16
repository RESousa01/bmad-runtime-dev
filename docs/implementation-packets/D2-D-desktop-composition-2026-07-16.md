# Implementation packet: D2-D reviewed BMAD Help desktop composition, integrated with D3

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
  signing, installer signing, or allowing model output to authorize file effects. D3 governed
  edits remain a separate, explicit user-proposed/approved local capability.
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
- D2-D checkpoint commit: `740c534a` (`feat-bmad-complete-deterministic-help-desktop-composition`).
- D3 baseline integrated: `6365c8a6` (governed local edits shell and flow).
- Integrated worktree: `C:\tmp\d2`, branch `codex/d2-ai-request`; the primary checkout remains
  unchanged and its unrelated dirty support-plane edits were not staged or reset.
- Final review: the merged diff was checked for authority crossover, renderer callback propagation,
  parser bounds, command-catalog parity, and redaction safety. The review path was read-only in this
  environment; no independent reviewer was spawned without explicit authorization.
- Verified commands:
  - `cargo fmt --all -- --check` — passed.
  - `cargo test --target-dir C:\tmp\codex-d2-target-20260716 -p desktop-app -p desktop-cloud
    -p desktop-egress -p desktop-ipc -p desktop-runtime -p desktop-store --all-features --locked`
    — all selected unit, integration, and doc tests passed.
  - `cargo clippy --target-dir C:\tmp\codex-d2-target-20260716 -p desktop-app --all-targets
    --all-features --locked -- -D warnings` — passed.
  - Pinned renderer TypeScript check — passed; pinned Vitest — 210 tests passed across 10 files;
    pinned production Vite build — passed.
  - BMAD foundation verification, TypeScript contract generation/schema/binding checks, and the
    contract runner — 81 passed, 1 platform-skipped link test.
  - `node tools/check-boundaries.mjs` — passed after adding the reviewed D2 command catalog and
    the single bounded Windows broker adapter exception.
  - `node tools/check-secrets.mjs` — passed for 490 active first-party files;
    `node tools/verify-reference-vault.mjs` — passed.
- D3 integration boundary: governed edits are renderer-authored proposals with separate workspace
  edit authority, exact preimage/candidate/diff hashes, one-shot approval, durable execution, and
  rollback. A Help recommendation or model output cannot create or approve an edit.
- Work deliberately not claimed: interactive native Tauri smoke, production identity/managed
  brokerage, signed receipt issuer verification, installer signing, clean-machine recovery, and
  Azure deployment.
- Remaining risks: production D2-E identity, entitlement, consent reconciliation, managed broker,
  signed receipt verification, Azure deployment, and release signing are not closed by D2-D/D3.
