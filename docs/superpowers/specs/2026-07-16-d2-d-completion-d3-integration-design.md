# D2-D Completion and D3 Integration Design

**Status:** Approved design checkpoint
**Date:** 2026-07-16
**D2 worktree:** `C:\tmp\d2`
**D2 branch:** `codex/d2-ai-request` at committed base `3108bf37`
**Integration baseline:** `codex/bmad-00-foundation` at `6365c8a6`

## 1. Decision

Finish the existing D2-D BMAD Help desktop-composition slice in its isolated
worktree before integrating the committed D3 governed-edits and clean-checkout
build baseline. Preserve the current uncommitted D2-D work, qualify it in
bounded checkpoints, complete the remaining coordinator, IPC, renderer, and
smoke-test work, and only then merge `6365c8a6` into the D2 branch.

Production D2-E remains a separate milestone. D2-D proves one honest local
reviewed Help vertical through explicit deterministic development composition;
default and production composition remain fail-closed and offline.

## 2. Scope

### In scope

- Inventory and preserve the inherited D2-D changes already present in
  `C:\tmp\d2`.
- Complete the approved D2-D checkpoints for exact response bytes, bounded
  consent evidence, the D2-to-Method bridge, durable completed projections, the
  one-shot coordinator, validated IPC, renderer review/result UX, documentation,
  and native deterministic smoke.
- Commit coherent D2-D checkpoints only after their focused proof passes.
- Merge the committed D3/build baseline `6365c8a6` after D2-D is clean.
- Resolve shared desktop-app and renderer conflicts while preserving the D2 Help
  and D3 governed-edits authority boundaries.
- Run full integrated Rust, renderer, TypeScript, architecture, production-build,
  .NET, and cross-language gates.

### Protected and out of scope

- Do not edit, stage, relocate, or delete the dirty Desktop Support API,
  infrastructure, fixture, or CI changes in the primary checkout.
- Do not implement production signed consent verification, managed-identity
  model brokerage, production receipt issuance, tenant deployment, installer
  signing, or clean-machine lifecycle qualification in D2-D.
- Do not make deterministic adapters available as a production fallback.
- Do not combine model authority with D3 file-effect approval authority.
- Do not reset, stash, or discard inherited work to obtain a cleaner-looking
  history.

## 3. Isolation and provenance

`C:\tmp\d2` is authoritative for D2-A through D2-D. The primary checkout remains
the authoritative source of the committed D3/build baseline and the separate
uncommitted support-plane lane. D2-D work occurs only in the D2 worktree until
the D2-D branch is clean and ready for integration.

The inherited D2-D diff predates this design checkpoint. Its tests and behavior
must be inspected and verified, but the implementation record must not claim
that this run observed their original RED signals. Every new correction or
observable behavior added after this checkpoint follows a fresh RED-GREEN-
REFACTOR cycle.

Commits stage exact files for one logical checkpoint. Unrelated dirty files are
never included. A merge commit brings `6365c8a6` into `codex/d2-ai-request`
after D2-D completion; the primary checkout itself is not mutated by that merge.

## 4. Component boundaries

### `desktop-egress`

Owns exact outbound context preparation, renderer-safe review manifests,
single-use consent decisions, bounded ledger retention, and opaque host evidence
for a still-live pending decision. It performs no network, process, filesystem,
database, Tauri, Method, or renderer work.

### `desktop-cloud`

Owns cloud-session epochs, identity and entitlement ports, sealed authorized
model requests, offline/deterministic transport composition, exact response-byte
retention, schema validation, receipt binding, receipt proof/freshness/replay
verification, and safe cloud error codes. It cannot prepare or approve consent.

### `desktop-app::bmad_model`

Owns the one-shot Help coordinator and the host-only D2-to-Method bridge. It
compiles the sealed Help invocation, prepares the closed set of outbound items,
revalidates renderer/workspace/run/session state at every transition, consumes
consent, persists `Advancing`, dispatches once, verifies the response, invokes
Method materialization, and asks `desktop-store` to finalize atomically. It
cannot invent consent, model output, receipts, or Method authority.

### `desktop-store`

Owns transactional durability. Successful Help finalization writes the raw
verified proposal, canonical recommendation, completed aggregate, checkpoint,
evidence, outbox lineage, and bounded renderer-safe completed projection in one
transaction. Restart recovery authenticates and returns only the safe projection.

### `desktop-ipc`

Owns strict envelopes and renderer projections for authentication status, Help
prepare, approve, cancel, submit, and latest/completed state. Payloads cannot
select provider, model, deployment, region, schema, package, context bytes,
receipt, result, or authority fields.

### Renderer

Owns presentation and user gestures only. It shows exact inert outbound text,
destination disclosure, expiry, lifecycle state, the canonical recommendation,
and a metadata-only receipt summary. It never receives token material, raw
provider errors, receipt proof, raw model output, or host authority hashes.

## 5. State and data flow

The coordinator exposes one active Help review for the single bound renderer and
workspace:

```text
CreatedUnbound
  -> Bound
  -> ContextReviewRequired
  -> Ready
  -> Advancing
  -> Completed
```

Terminal safe outcomes include cancelled, offline, invalid output, invalid
receipt, context or authority drift, and persistence failure. A terminal or
consumed run is never silently resumed.

1. The host creates an inert `created_unbound` Help run.
2. The coordinator compiles the exact sealed Help invocation and prepares only
   the approved instruction, current intent, catalog candidate, and bounded
   evidence facts as outbound context items.
3. `desktop-egress` seals the exact redacted bytes and returns a review
   projection. Preparing a replacement invalidates the prior review.
4. Approval records one short-lived pending decision for the displayed manifest
   and exact invocation binding. Approval performs no transport.
5. Submit revalidates renderer generation, workspace ID, grant epoch, run,
   session, manifest, binding, decision, and expiry.
6. Consent is atomically consumed once. The host bridge maps the exact D2 evidence
   into `MethodContextDecision` while retaining the D2 binding separately for
   Method/model lineage.
7. The coordinator persists `Advancing` before dispatch.
8. Default composition returns offline without exposing context. The explicit
   deterministic feature dispatches one fixture through the normal sealed
   request and response-verification path.
9. `desktop-cloud` retains the exact original UTF-8 payload bytes, validates the
   registered schema, checks every request/manifest/consent/profile/model/
   deployment/region/retention binding, and verifies receipt policy.
10. Method validates the D2/Method bridge and materializes the canonical
    recommendation from the exact verified bytes.
11. `desktop-store` finalizes all authoritative records and the renderer-safe
    completion projection atomically.
12. `bmad.help.latest` returns current safe lifecycle state or the authenticated
    durable completed projection after restart.

## 6. Consent, retention, and secrecy invariants

- One approval authorizes one invocation. Identical retries require a new review
  and decision.
- Sign-out, restart, renderer rebind, workspace/grant change, run change,
  manifest change, session change, or expiry invalidates pending authority.
- A user cancellation before submit terminally invalidates the pending decision
  without dispatch. Cancellation after submit, offline response, timeout,
  invalid output, invalid receipt, or persistence failure never resurrects
  already consumed consent.
- The consent ledger is capacity-bounded and time-pruned without permitting
  identifier replay.
- Context and raw model bytes never enter the general reply cache.
- Verified payload and receipt proof remain opaque trusted-host data.
- Only closed canonical recommendation fields and metadata-only receipt fields
  cross IPC.
- Raw broker, HTTP, JWT, provider, schema, receipt, token, and model errors are
  mapped to stable safe codes before projection or ordinary logging.

## 7. Failure behavior

Failures before consumption leave no invocation authority. Failures after
consumption are terminal and never recreate or reuse the decision. When the run
has crossed the durable `Advancing` boundary, recovery reports a sanitized
non-resumable or terminal outcome rather than replaying transport.

Successful finalization is all-or-nothing. A failed transaction leaves no
authoritative completed projection, canonical recommendation, checkpoint,
evidence, or outbox partial state. Existing encrypted content staging may leave
an unreferenced orphan, but it is never authoritative without the relational
commit.

## 8. Delivery sequence

1. Inventory the inherited work against D2-D checkpoints 1 through 5 and record
   which tests and invariants already exist.
2. Qualify and commit the build assets, exact verified response bytes, bounded
   consent/bridge evidence, and durable completion projection as separate
   coherent checkpoints.
3. Complete and commit the one-shot BMAD model coordinator.
4. Complete and commit validated IPC commands and projections.
5. Complete and commit the renderer review/result experience.
6. Document default-offline versus explicit deterministic behavior and run the
   native deterministic/default-offline smoke.
7. Merge `6365c8a6` into the clean D2 branch and resolve D3/build conflicts.
8. Run integrated proof and record the D2-D/D3 green checkpoint.
9. Begin a separate D2-E production consent/model/receipt design from that
   integrated baseline.

## 9. Verification

### Focused proof

- `desktop-cloud`: exact payload bytes, opaque debug/serialization boundaries,
  deterministic/default composition, full response and receipt substitution.
- `desktop-egress`: bounded/pruned ledger, exact pending evidence, replay and
  drift rejection.
- `desktop-app`: bridge substitutions and the complete deterministic coordinator
  state machine.
- `desktop-store`: atomic finalization, rollback, tamper rejection, and restart
  recovery.
- `desktop-ipc`: strict command shapes, unknown-field rejection, capability and
  mutability classification, safe projection closures, and reply-cache rules.
- renderer: exact command order, one-shot dispatch, authority invalidation,
  accessibility, inert outbound text, completed result, and proof omission.

### Native smoke

The explicit deterministic desktop entry point must prove create, review,
approve, one send, verified result, completed projection, restart recovery, and
mandatory fresh review for a second run. The default entry point must remain
offline and must never fall back to deterministic output.

### Integrated regression proof

- Rust formatting, strict all-target/all-feature Clippy, and locked all-feature
  workspace tests.
- Exact pinned Node/pnpm source verification, renderer tests, TypeScript
  typecheck, lint, secret scan, architecture boundaries, and production Vite
  build.
- Desktop Support API and .NET contract-conformance tests without modifying the
  primary checkout's in-flight support-plane work.
- Existing cross-language contract qualification and `git diff --check`; new
  production request/receipt reconciliation remains D2-E work.
- D3 governed changes: enablement, proposal, exact review, apply, undo, conflict,
  history, and recovery regressions remain green.

## 10. Acceptance criteria

D2-D is complete only when:

- the inherited diff is classified and committed in bounded passing checkpoints;
- deterministic Help performs the complete reviewed one-shot flow;
- default composition remains offline;
- restart returns the authenticated completed renderer projection;
- every authority drift and replay case fails closed;
- no token, proof, raw output, absolute path, or authority object crosses IPC;
- native deterministic and default-offline smoke pass;
- `6365c8a6` is integrated without regressing D3 or clean-checkout build assets;
- full integrated gates pass or any environment-limited gap is explicitly
  recorded with reproducible evidence.

## 11. D2-E handoff

The next design begins only after the integrated D2-D/D3 checkpoint. It will
reconcile the richer Rust authorized-request and receipt contracts with the
Desktop Support API, add signed single-use consent verification, managed-
identity provider brokerage, signed receipt issuance and replay policy, concrete
tenant deployment inputs, and production desktop composition. The in-flight
support-plane changes in the primary checkout are reviewed and classified at
that boundary rather than being absorbed implicitly into D2-D.
