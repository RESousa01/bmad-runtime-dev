# D2-D Completion and D3 Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the reviewed, deterministic-development BMAD Help request vertical in `C:\tmp\d2`, prove that the default desktop remains offline, then integrate committed D3 governed edits at `6365c8a6` without mixing model authority with file-effect authority.

**Architecture:** `desktop-egress` owns exact review and single-use consent, `desktop-cloud` owns one-shot request/response verification, `desktop-app::bmad_model` composes the host-only state machine and D2-to-Method bridge, `desktop-store` owns atomic completion durability, `desktop-ipc` owns strict renderer shapes, and the renderer owns presentation and gestures only. D2-D is completed and committed before a no-fast-forward merge of the D3 baseline. Production consent, managed identity, provider brokerage, and signed receipts remain D2-E.

**Tech Stack:** Rust 1.97.0, Tauri 2.11.4, SQLite/DPAPI, React 19, TypeScript 7.0.2, Vitest, axe-core 4.12.1, Node 24.18.0, pnpm 11.12.0, .NET contract/support tests.

## Global Constraints

- Execute only in `C:\tmp\d2` until the D2-D branch is clean and the plan reaches the explicit D3 merge task.
- Treat `C:\Users\rodri\source\bmad-runtime-dev` as read-only reference context. Do not edit, stage, stash, reset, relocate, or delete its in-flight Desktop Support API, infrastructure, fixture, deployment, or CI work.
- Preserve the inherited D2-D diff. Do not reset or rewrite it to manufacture a cleaner implementation history.
- The inherited tests may be rerun and cited as qualification evidence, but do not claim this execution observed their original RED phase. Every correction or behavior added while executing this plan starts with a newly failing test.
- Treat `bmad-runtime-lib` as a reference vault. Never import it into product code or edit it as part of D2-D.
- Keep `model.auth.*` and `bmad.help.*` behind the existing `host_dispatch` Tauri command. Add no Tauri permission, network primitive, filesystem primitive, generic execution primitive, or renderer storage.
- Never send a model request during approval. `bmad.help.submit` is the only command allowed to consume consent or dispatch.
- Never put exact outbound context, raw model bytes, receipt proof, tokens, absolute paths, or authority hashes in the general host reply cache, renderer state persistence, ordinary logs, or debug output.
- Keep the default build fail-closed. Its `prepare` preflight returns `support_plane_offline` before compiling or projecting outbound context, creating consent, or advancing Method state. `OfflineModelTransport` remains a defense-in-depth seam. The deterministic transport exists only with `desktop-app/deterministic-help`, is visibly labeled development-only, and is never a fallback.
- Keep D2 Help approval and D3 file-effect approval disjoint. A decision from one state machine must be rejected by the other.
- Use `apply_patch` for source and documentation edits. Use formatters only for mechanical formatting.
- Stage exact files for each checkpoint and inspect `git diff --cached --name-only` before every commit.
- Stop and investigate any integrity, authority, schema, replay, or boundary failure. Do not weaken a test or gate to make it green.

## Locked public command contract

All eight model/Help commands remain inside `host_dispatch`:

1. `model.auth.status`
2. `model.auth.sign_in`
3. `model.auth.sign_out`
4. `bmad.help.prepare`
5. `bmad.help.approve`
6. `bmad.help.cancel`
7. `bmad.help.submit`
8. existing `bmad.help.latest`

The exact renderer payloads are:

```text
model.auth.status    {}
model.auth.sign_in   {}
model.auth.sign_out  {}
bmad.help.prepare    { workspaceId, workspaceGrantEpoch }
bmad.help.approve    { workspaceId, workspaceGrantEpoch, manifestHash }
bmad.help.cancel     { workspaceId, workspaceGrantEpoch, manifestHash, decisionId }
bmad.help.submit     { workspaceId, workspaceGrantEpoch, manifestHash, decisionId }
bmad.help.latest     { workspaceId, workspaceGrantEpoch }
```

No payload may contain provider, model, deployment, region, retention, schema, package, capability, context bytes, receipt, result, token, proof, or authority fields.

## Locked lifecycle and lock order

The durable Method lifecycle is:

```text
Created(v1)
  -> CapabilityBound(v2)
  -> ContextReviewRequired(v3)
  -> Ready(v4)
  -> Advancing(v5)
  -> Completed(v6)
```

Approval creates a live D2 decision but performs no Method transition beyond `ContextReviewRequired`. Submit borrows pending evidence, persists `Ready`, consumes the D2 decision once, persists `Advancing`, dispatches once, verifies, materializes, and atomically finalizes `Completed`.

The v4 `Ready` write is an exact bridge-validation checkpoint inside submit, not invocation authority and not a side effect of approval. If D2 consumption fails after that checkpoint, the run becomes terminal/interrupted and cannot resume or redispatch after restart; a fresh run and review are required.

When multiple authorities are needed, acquire them in this order:

```text
HostState.workspace_commits
  -> HostState Ready read guard
  -> WorkspaceScopeAuthorityGuard
  -> HostState.bmad_model mutex
  -> LocalStore transaction
```

Never acquire a workspace or coordinator lock from inside a store transaction.

---

## Task 1: Freeze the inherited inventory and restore a buildable desktop package

**Files:**

- Preserve: every currently modified/untracked D2-D path
- Generate from: `crates/desktop-app/icons/app-icon.svg`
- Create: `crates/desktop-app/icons/32x32.png`
- Create: `crates/desktop-app/icons/128x128.png`
- Create: `crates/desktop-app/icons/128x128@2x.png`
- Create: `crates/desktop-app/icons/icon.icns`
- Create: `crates/desktop-app/icons/icon.ico`
- Create: the Windows Store logo PNGs emitted by the pinned Tauri generator

- [ ] **Step 1: Confirm the isolated branch and preserve the exact inventory**

Run:

```powershell
Set-Location C:\tmp\d2
git branch --show-current
git status --short
git diff --cached --name-only
git diff --check
```

Expected: branch `codex/d2-ai-request`; the inherited D2-D paths are present; the staged list is empty; whitespace validation passes. Save the status output in the execution notes before changing files.

- [ ] **Step 2: Reproduce the known build failure**

Run:

```powershell
cargo test -p desktop-app --lib --locked
```

Expected: Tauri build fails because `crates/desktop-app/icons/icon.ico` is absent. If it fails earlier for a different reason, diagnose that failure before generating assets.

- [ ] **Step 3: Generate all bundle assets from the tracked SVG source**

Run:

```powershell
pnpm exec tauri icon crates/desktop-app/icons/app-icon.svg --output crates/desktop-app/icons
Get-ChildItem crates/desktop-app/icons -File | Sort-Object Name | Select-Object Name,Length
```

Expected: non-empty ICO, ICNS, `32x32.png`, `128x128.png`, `128x128@2x.png`, and Windows Store logo PNGs. Do not copy either icon from the primary checkout.

- [ ] **Step 4: Prove the native package can compile**

Run:

```powershell
cargo test -p desktop-app --lib --locked
cargo check -p desktop-app --all-features --locked
```

Expected: the icon error is gone. A new Rust compilation failure is evidence for the next incomplete D2-D checkpoint; record it, but do not stage unrelated Rust corrections in this asset commit.

- [ ] **Step 5: Commit only the qualified icon inventory**

Run:

```powershell
git add crates/desktop-app/icons
git diff --cached --name-only
git diff --cached --check
git commit -m "build(desktop): track qualified application icons"
```

Expected staged paths: only `crates/desktop-app/icons/*`.

---

## Task 2: Qualify and commit exact verified model bytes

**Files:**

- Modify: `crates/desktop-cloud/src/model.rs`
- Modify: `crates/desktop-cloud/src/composition.rs`
- Modify: `crates/desktop-cloud/tests/model_verification.rs`
- Create: `crates/desktop-cloud/tests/composition_modes.rs`

- [ ] **Step 1: Inspect the inherited slice for its four required invariants**

Confirm in the diff that:

- `VerifiedModelOutput` retains `Arc<[u8]>` from the original UTF-8 response string;
- `payload_bytes()` returns those exact bytes without canonical reserialization;
- `Debug` redacts payload and receipt proof and the type has no Serde implementation;
- deterministic fixture dispatch and default-offline composition both use the normal bounded request/receipt contracts.

- [ ] **Step 2: Run the inherited qualification tests**

Run:

```powershell
cargo test -p desktop-cloud --all-features --locked
cargo test -p desktop-cloud --locked
```

Expected: model substitution, exact-byte, debug-redaction, deterministic-fixture, and default-offline tests pass in both feature modes.

- [ ] **Step 3: Add a fresh failing test only if qualification exposes a gap**

For each newly discovered gap, add one narrowly named test to `model_verification.rs` or `composition_modes.rs`, run that exact test to observe failure, make the smallest correction, and rerun it. Do not restyle or rewrite the inherited implementation when it is already correct.

- [ ] **Step 4: Commit the exact-byte checkpoint**

Run:

```powershell
git add crates/desktop-cloud/src/model.rs crates/desktop-cloud/src/composition.rs crates/desktop-cloud/tests/model_verification.rs crates/desktop-cloud/tests/composition_modes.rs
git diff --cached --name-only
git diff --cached --check
git commit -m "feat(cloud): retain exact verified model bytes"
```

Expected staged paths: exactly the four files listed above.

---

## Task 3: Qualify bounded consent evidence and the D2-to-Method bridge

**Files:**

- Modify: `crates/desktop-egress/src/consent.rs`
- Modify: `crates/desktop-egress/src/lib.rs`
- Modify: `crates/desktop-egress/tests/consent.rs`
- Modify: `crates/desktop-app/Cargo.toml`
- Modify: `crates/desktop-app/src/lib.rs`
- Create: `crates/desktop-app/src/bmad_model.rs`
- Create: `crates/desktop-app/src/bmad_model/bridge.rs`
- Create: `crates/desktop-app/src/bmad_model/bridge_tests.rs`
- Modify: `Cargo.lock`

- [ ] **Step 1: Qualify the inherited bounded ledger**

Run:

```powershell
cargo test -p desktop-egress --test consent --locked
cargo test -p desktop-egress --doc --locked
```

Expected: bounded live decisions, bounded replay tombstones, expiry pruning, terminal-state rejection, evidence borrowing, and compile-fail opacity tests pass.

- [ ] **Step 2: Qualify the inherited bridge after the icon repair**

Run:

```powershell
cargo test -p desktop-app --lib bmad_model::bridge --all-features --locked
cargo test -p desktop-app --lib bmad_model::bridge --locked
```

Expected: every substitution of decision, manifest, D2 binding, session authority, Method binding, item order, item hash, byte count, issuance time, or expiry is rejected.

- [ ] **Step 3: Verify the bridge mapping is exact**

The implementation must remain:

```text
MethodContextDecision.decision_id   = D2 decision_id
MethodContextDecision.manifest_hash = D2 manifest_hash
MethodContextDecision.consent_hash  = D2 consent_disclosure_hash
MethodContextDecision.context_digest = canonical_hash(
  "bmad-help-reviewed-context",
  1,
  ordered item ids + outbound hashes + byte counts + manifest hash
)
MethodContextDecision.binding_hash  = MethodExactBinding.binding_hash()
MethodContextDecision.reviewed_at   = D2 issued_at
```

The D2 invocation binding hash stays separate and later enters `MethodAdvanceRequest`; it must never replace the Method binding hash.

- [ ] **Step 4: Commit only consent and bridge files**

Run:

```powershell
git add Cargo.lock crates/desktop-egress/src/consent.rs crates/desktop-egress/src/lib.rs crates/desktop-egress/tests/consent.rs crates/desktop-app/Cargo.toml crates/desktop-app/src/lib.rs crates/desktop-app/src/bmad_model.rs crates/desktop-app/src/bmad_model/bridge.rs crates/desktop-app/src/bmad_model/bridge_tests.rs
git diff --cached --name-only
git diff --cached --check
git commit -m "feat(bmad): bind consent evidence to Method review"
```

Expected: no cloud, store, IPC, command, or renderer path is staged.

---

## Task 4: Finish durable completion projection and restart decoding

**Files:**

- Modify: `crates/desktop-store/src/bmad_method.rs`
- Modify: `crates/desktop-store/src/lib.rs`
- Modify: `crates/desktop-store/tests/bmad_help_run_store.rs`
- Modify: `crates/desktop-store/tests/bmad_method_store.rs`
- Create: `crates/desktop-ipc/src/bmad_completion.rs`
- Modify: `crates/desktop-ipc/src/bmad_run.rs`
- Modify: `crates/desktop-ipc/src/lib.rs`
- Create: `crates/desktop-ipc/tests/bmad_completion.rs`
- Modify: `crates/desktop-ipc/tests/bmad_run.rs`
- Modify: `crates/desktop-app/src/commands.rs`
- Modify: `crates/desktop-app/src/wire.rs`
- Modify: tests in `crates/desktop-app/src/commands.rs`

- [ ] **Step 1: Run the inherited store and IPC proof**

Run:

```powershell
cargo test -p desktop-store -p desktop-ipc --locked
```

Expected: atomic finalization, rollback, tamper rejection, completed projection decoding, and current-intent retention tests pass.

- [ ] **Step 2: Write failing host tests for the new latest-run variants**

Add tests proving:

1. `BmadHelpRunLatest::Interrupted` returns a `bmad_help_run_interrupted` host-data variant bound to the authenticated workspace/run/session creation receipt;
2. `BmadHelpRunLatest::Completed` strictly decodes `bmad-help-completed.v1` and returns `bmad_help_run_completed`;
3. completion bytes with a substituted workspace, run, session, unknown field, duplicate field, or oversized body enter recovery instead of crossing IPC;
4. `currentIntent` survives create and restart projection exactly.

Run the new exact tests and confirm they fail because `latest_bmad_help_run` handles only `None`, `LegacyProjectionUnavailable`, and `Retained`.

- [ ] **Step 3: Implement the two host-data variants**

Add to `HostCommandData`:

```rust
BmadHelpRunInterrupted(BmadHelpRunCreatedProjection),
BmadHelpRunCompleted(BmadHelpRunCompletedProjection),
```

Update `latest_bmad_help_run` to:

- decode `Retained` through `decode_retained_bmad_help_run`;
- decode `Interrupted` through the same authenticated creation decoder but project the distinct host-data kind;
- decode `Completed.creation` for identity and `Completed.renderer_projection` through `decode_retained_bmad_help_completion` with the exact workspace/run/session IDs;
- enter recovery for authenticated-store or strict-decoder failure.

Do not turn an interrupted creation projection into a completion claim.

- [ ] **Step 4: Run focused and package tests**

Run:

```powershell
cargo test -p desktop-app --lib commands --locked
cargo test -p desktop-store -p desktop-ipc --locked
cargo test -p desktop-app --lib --locked
```

Expected: all new latest-run tests and all inherited tests pass.

- [ ] **Step 5: Commit the durability checkpoint**

Run:

```powershell
git add crates/desktop-store/src/bmad_method.rs crates/desktop-store/src/lib.rs crates/desktop-store/tests/bmad_help_run_store.rs crates/desktop-store/tests/bmad_method_store.rs crates/desktop-ipc/src/bmad_completion.rs crates/desktop-ipc/src/bmad_run.rs crates/desktop-ipc/src/lib.rs crates/desktop-ipc/tests/bmad_completion.rs crates/desktop-ipc/tests/bmad_run.rs crates/desktop-app/src/commands.rs crates/desktop-app/src/wire.rs
git diff --cached --name-only
git diff --cached --check
git commit -m "feat(bmad): retain completed Help projections atomically"
```

---

## Task 5: Add the pure Help model policy and review preparation

**Files:**

- Modify: `crates/desktop-runtime/src/bmad/help_materialization.rs`
- Modify: `crates/desktop-runtime/src/bmad/mod.rs`
- Modify: `crates/desktop-runtime/src/lib.rs`
- Modify: `crates/desktop-app/Cargo.toml`
- Modify: `crates/desktop-app/src/bmad_model.rs`
- Create: `crates/desktop-app/src/bmad_model/config.rs`
- Create: `crates/desktop-app/src/bmad_model/context.rs`
- Create: `crates/desktop-app/src/bmad_model/coordinator.rs`
- Create: `crates/desktop-app/src/bmad_model/coordinator_tests.rs`
- Modify: `crates/desktop-app/src/state.rs`
- Modify: `Cargo.lock`

- [ ] **Step 1: Write failing schema-closure tests in `desktop-runtime`**

Add a public pure function with this interface:

```rust
pub fn validate_bmad_help_proposal_schema(
    value: &serde_json::Value,
) -> Result<(), BmadKernelError>;
```

Tests must require acceptance of both generated `MethodHelpProposal` branches and rejection of unknown fields, missing fields, unsupported reason codes, unsafe rationale text, duplicate evidence token IDs, malformed capability keys, and non-object values. Run the exact tests first; expected failure is the missing function.

- [ ] **Step 2: Implement schema validation by reusing generated contracts**

The function must deserialize a clone into `generated_contracts::MethodHelpProposal` and run the same semantic shape checks used by `parse_proposal`. Refactor shared checks rather than adding a looser second validator. Keep strict duplicate-key rejection on the later exact-byte `parse_proposal` path.

- [ ] **Step 3: Write failing deterministic policy tests**

In `context.rs`, define an ASCII token-boundary classifier. It returns an architecture evidence signal only for the lowercased complete tokens `architecture`, `architect`, or `readiness`. Tests must reject substring-only inputs such as `microarchitecture` and unrelated intents.

Define the exact outcome policy:

- explicit signal: one `UserAsserted` evidence token bound to the exact non-`_meta` architecture catalog action;
- no explicit signal: empty evidence allowlist and deterministic proposal `{"proposalKind":"no_recommendation","reasonCode":"catalog_evidence_absent"}`.

Run the classifier/policy tests first and observe failure.

- [ ] **Step 4: Implement host-owned deterministic configuration**

Add the Cargo feature:

```toml
[features]
default = []
deterministic-help = ["desktop-cloud/deterministic-fake"]
```

`config.rs` must construct `BmadTrustedHelpModelProfileData` from host constants only. Derive its request-schema and provider/profile/deployment/policy hashes with named canonical domains; do not hard-code anonymous digest bytes. Keep `BmadTrustedHelpModelProfile::from_host_assertion` as the sole binder of the generated response schema. After compilation, populate the egress manifest with `sapphirus.bmad-method-help-proposal.v1` as the canonical output schema ID and `compiled.proposal_schema_closure_hash()` as its schema hash. Use lowercase `localdev` as the deterministic region and `TransientNoStore` retention.

The default mode is `Offline`. The deterministic mode is compiled only by `deterministic-help` and projects the label `Deterministic local model — development only`.

D2-D does not instantiate a production `CloudSession`, Windows broker, token cache, tenant, or entitlement. Lock these command semantics in tests:

- `model.auth.status` returns `status = unavailable`, `mode = offline` in the default build and `status = development_ready`, `mode = deterministic_development` in the feature build;
- `model.auth.sign_in` returns `identity_unavailable` in both D2-D modes because deterministic access is not identity;
- `model.auth.sign_out` invalidates pending Help authority, increments the local auth epoch, and returns the current safe status without invoking a broker; it does not disable the compile-time deterministic mode.

- [ ] **Step 5: Write failing exact-context preparation tests**

`context.rs` must prepare an ordered closed set:

1. sealed Help instruction bytes;
2. the retained current intent;
3. one canonical JSON catalog candidate;
4. one canonical JSON evidence fact only when the explicit architecture signal exists.

Tests must prove exact order, IDs, privacy-preserving relative labels, roles, hashes, byte counts, scanner redaction, limits, manifest expiry, and absence of every other workspace or catalog byte. Test that item reorder or byte substitution changes the manifest hash.

- [ ] **Step 6: Implement `PreparedBmadHelpReview`**

Use this host-only shape in `coordinator.rs`:

```rust
struct PreparedBmadHelpReview {
    renderer_session_id: ContractId,
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    workspace_catalog_version: u64,
    creation: BmadHelpRunCreationReceipt,
    compiled: BmadCompiledHelpInvocation,
    manifest: ContextEgressManifest,
    invocation_binding: ModelInvocationBinding,
    deterministic_fixture: String,
}
```

The fixture is exact proposal JSON selected by the pure policy. It stays native and is never included in the review projection.

- [ ] **Step 7: Bind and persist the pre-review Method transitions**

`BmadHelpCoordinator::prepare` must:

1. authenticate the current renderer, workspace, grant epoch, catalog version, latest creation receipt, and retained intent;
2. require the compile-time deterministic-development mode; default/offline returns `support_plane_offline` here;
3. invalidate any prior in-memory review;
4. compile `BmadCompiledHelpInvocation` from the sealed foundation and host profile;
5. attach the bounded evidence allowlist;
6. call `MethodSessionService::bind_invocation(..., expected_version = 1, ...)`;
7. call `MethodSessionService::request_context_review(..., expected_version = 2)`;
8. prepare and seal the D2 manifest/binding using the v3 session authority hash;
9. retain the host-only review and return only a renderer-safe projection.

If the retained Method session is not `Created`, preparation returns a stable conflict and never attempts recovery or transport. Tests for the default build must prove that the offline preflight returns no item text or review projection and leaves the Method session, consent ledger, and coordinator review state untouched.

- [ ] **Step 8: Add coordinator ownership to `HostState`**

Add:

```rust
bmad_model: Mutex<BmadHelpCoordinator>,
```

Initialize it in both `HostState::initialize` and `HostState::recovery`. Expose a guard-scoped `method_store(&ReadyAuthorityGuard) -> Result<&LocalStore, StoreError>` for the coordinator; do not make the store public. Add invalidation calls on renderer rebind and recovery entry.

- [ ] **Step 9: Run the pure preparation proof**

Run:

```powershell
cargo test -p desktop-runtime bmad_help_proposal --locked
cargo test -p desktop-app --lib bmad_model::coordinator_tests::prepare --all-features --locked
cargo test -p desktop-app --lib bmad_model::coordinator_tests::prepare --locked
```

Expected: exact-context tests pass in both modes; the default build contains no deterministic transport type.

- [ ] **Step 10: Commit review preparation**

Run:

```powershell
git add Cargo.lock crates/desktop-runtime/src/bmad/help_materialization.rs crates/desktop-runtime/src/bmad/mod.rs crates/desktop-runtime/src/lib.rs crates/desktop-app/Cargo.toml crates/desktop-app/src/bmad_model.rs crates/desktop-app/src/bmad_model/config.rs crates/desktop-app/src/bmad_model/context.rs crates/desktop-app/src/bmad_model/coordinator.rs crates/desktop-app/src/bmad_model/coordinator_tests.rs crates/desktop-app/src/state.rs
git diff --cached --name-only
git diff --cached --check
git commit -m "feat(bmad): prepare exact reviewed Help requests"
```

---

## Task 6: Complete one-shot approval, dispatch, verification, and finalization

**Files:**

- Modify: `crates/desktop-app/src/bmad_model/coordinator.rs`
- Modify: `crates/desktop-app/src/bmad_model/coordinator_tests.rs`
- Create: `crates/desktop-app/src/bmad_model/transport.rs`
- Create: `crates/desktop-app/src/bmad_model/verification.rs`
- Modify: `crates/desktop-app/src/bmad_model.rs`
- Modify: `crates/desktop-app/src/state.rs`

- [ ] **Step 1: Define the host-only active state machine**

Use a non-serializable enum owned by the coordinator mutex:

```rust
enum ActiveBmadHelp {
    ReviewRequired(PreparedBmadHelpReview),
    Approved(ApprovedBmadHelpReview),
    Advancing(BmadHelpAdvancing),
    Completed(BmadHelpRunCompletedProjection),
    Terminal(BmadHelpTerminalState),
}
```

`ApprovedBmadHelpReview` owns the non-cloneable `PendingContextDecision`. No active variant may derive `Serialize`, and custom `Debug` must redact manifest item content, fixture bytes, decisions, and receipt material.

- [ ] **Step 2: Write failing approval/cancellation tests**

Tests must prove:

- approval requires the exact displayed manifest hash and current renderer/workspace/grant/run/session;
- approval creates one pending decision with a lifetime no longer than the manifest;
- approval performs zero transport calls and leaves durable Method state at `ContextReviewRequired`;
- replacement prepare cancels the old decision;
- cancel permanently invalidates the exact decision and performs zero transport calls;
- sign-out, renderer rebind, workspace revocation, grant change, recovery, expiry, and process restart invalidate pending authority.

- [ ] **Step 3: Implement approve, cancel, and invalidation**

Use `ConsentService::approve`, `ConsentService::cancel`, and the existing bounded `MemoryDecisionLedger`. Return stable safe states only. Never reconstruct a pending decision from renderer IDs.

- [ ] **Step 4: Add a one-shot transport seam and failing dispatch-order tests**

Define an app-internal object-safe seam that consumes `AuthorizedModelRequest`:

```rust
trait BmadHelpTransport: Send + Sync {
    fn send(
        &self,
        request: AuthorizedModelRequest,
        deterministic_fixture: &str,
        now: UnixMillis,
    ) -> Result<(DispatchedModelRequest, RawModelOutput), CloudError>;
}
```

Implement `OfflineHelpTransport` with `OfflineModelTransport`. Under `deterministic-help`, implement `DeterministicHelpTransport` with `DeterministicModelTransport::send_fixture`. A counting scripted transport in tests must prove exactly zero or one calls.

Write failing tests for this exact submit order:

```text
revalidate all bindings
  -> borrow live D2 evidence
  -> bridge and persist Method Ready(v4)
  -> consume D2 decision once
  -> construct AuthorizedModelRequest
  -> persist Method Advancing(v5)
  -> dispatch once
  -> verify exact response bytes and receipt
  -> materialize canonical Method records
  -> build safe completed projection
  -> atomically finalize Method Completed(v6)
```

- [ ] **Step 5: Implement the schema and receipt verifiers**

`verification.rs` must implement `CanonicalOutputValidator` by checking the exact contract ID/hash and calling `validate_bmad_help_proposal_schema`.

Under `deterministic-help`, accept receipt proof only when it is exactly `deterministic-fake-no-trust`, then wrap that proof verifier in `ReplaySafeReceiptVerifier<_, SystemReceiptClock>`. Keep the verifier instance in coordinator state so receipt IDs cannot replay during the process lifetime. The default path never creates this verifier.

- [ ] **Step 6: Implement `submit` through existing authority APIs**

Before dispatch, capture request ID/hash and D2 binding hash from the authorized request, then call:

```rust
store.begin_method_advance(
    &scope,
    &session_id,
    compiled.exact_binding(),
    MethodAdvanceRequest {
        invocation_id,
        idempotency_key,
        decision_id,
        decision_consumption_hash,
        model_request_id,
        model_request_hash,
        session_authority_hash,
        d2_model_invocation_binding_hash,
        model_bridge_binding_hash,
        expected_version: 4,
    },
)
```

Only after that write succeeds may the transport receive the request.

For a verified response:

1. hash the full native receipt as `bmad-model-receipt-evidence` without projecting proof;
2. wrap the exact `payload_bytes()` in `BmadVerifiedHelpProposal` with the Method advance receipt;
3. load the authoritative v5 session;
4. call `BmadHelpMaterializer::materialize` with host-generated recommendation/result IDs;
5. derive any display name from the exact compiled catalog candidate, never from model text;
6. call `project_completed_bmad_help_run` with a metadata-only receipt summary;
7. serialize it and call `LocalStore::finalize_bmad_help(..., expected_version = 5, ...)`.

- [ ] **Step 7: Write and pass the full failure matrix**

Add tests for duplicate submit, stale manifest, wrong decision, expiry, renderer/workspace/grant/run/session drift, sign-out, transport offline, transport failure, invalid JSON, schema substitution, payload hash substitution, request substitution, every receipt binding substitution, duplicate receipt ID, materialization rejection, projection rejection, store begin failure, and finalization failure.

Required outcomes:

- default/offline prepare: no context assembly or projection, no consent, no Method transition, and no transport;
- before D2 consumption: no model authority and no transport;
- after D2 consumption: terminal, non-resumable, and no consent resurrection;
- after durable v5: restart projects `interrupted`, never redispatches;
- after success: one v6 completion, one checkpoint, one raw proposal, one canonical recommendation, one completed projection, and one transport call.

- [ ] **Step 8: Prove default-offline and deterministic modes independently**

Run:

```powershell
cargo test -p desktop-app --lib bmad_model --all-features --locked
cargo test -p desktop-app --lib bmad_model --locked
cargo clippy -p desktop-app --all-targets --all-features --locked -- -D warnings
```

Expected: deterministic success exists only with all features; the default suite rejects before context review, reaches stable offline behavior without consent, and never returns a fixture.

- [ ] **Step 9: Commit the one-shot coordinator**

Run:

```powershell
git add crates/desktop-app/src/bmad_model.rs crates/desktop-app/src/bmad_model/coordinator.rs crates/desktop-app/src/bmad_model/coordinator_tests.rs crates/desktop-app/src/bmad_model/transport.rs crates/desktop-app/src/bmad_model/verification.rs crates/desktop-app/src/state.rs
git diff --cached --name-only
git diff --cached --check
git commit -m "feat(bmad): compose one-shot verified Help requests"
```

---

## Task 7: Expose the eight-command contract through strict IPC

**Files:**

- Modify: `crates/desktop-runtime/src/command.rs`
- Modify: `crates/desktop-runtime/src/error.rs`
- Create: `crates/desktop-ipc/src/bmad_model.rs`
- Modify: `crates/desktop-ipc/src/lib.rs`
- Modify: `crates/desktop-ipc/src/envelope.rs`
- Create: `crates/desktop-ipc/tests/bmad_help_model_access.rs`
- Modify: `crates/desktop-app/src/commands.rs`
- Modify: `crates/desktop-app/src/wire.rs`
- Modify: tests in `crates/desktop-app/src/commands.rs`
- Modify: `tools/check-boundaries.mjs`

- [ ] **Step 1: Write failing `LocalCommand` catalog tests**

Add these exact variants:

```rust
ModelAuthStatus,
ModelAuthSignIn,
ModelAuthSignOut,
PrepareBmadHelp { workspace_id, workspace_grant_epoch },
ApproveBmadHelp { workspace_id, workspace_grant_epoch, manifest_hash },
CancelBmadHelp { workspace_id, workspace_grant_epoch, manifest_hash, decision_id },
SubmitBmadHelp { workspace_id, workspace_grant_epoch, manifest_hash, decision_id },
```

Tests must require `ModelAuthStatus` and `LatestBmadHelpRun` to be read-only and every other new command to be mutating.

- [ ] **Step 2: Add stable safe error codes**

Extend `LocalErrorCode` with the approved D2 codes:

```text
identity_unavailable
authentication_required
reauthentication_required
tenant_mismatch
entitlement_unavailable
feature_disabled
context_rejected
context_drift
consent_required
consent_expired
consent_binding_mismatch
consent_already_consumed
support_plane_offline
transport_failed
response_binding_mismatch
invalid_model_output
receipt_invalid
```

Map raw egress/cloud/Method/store causes to these or existing recovery/conflict codes in `desktop-app`; never include source error text.

- [ ] **Step 3: Write strict envelope tests before parsers**

`bmad_help_model_access.rs` must test all eight exact command shapes, missing fields, unknown fields, duplicate keys, invalid IDs/hashes/epochs, expired envelope timestamps, capability absence, mutation classification, and oversize bodies.

For every forbidden renderer-controlled field listed in the locked contract, add a table-driven rejection case.

- [ ] **Step 4: Implement envelope parsing**

Keep all payload structs private with `deny_unknown_fields`. Empty auth payloads must reject even one unknown field. Reuse `Sha256Digest` and `ContractId` validation rather than accepting strings for later parsing.

- [ ] **Step 5: Add renderer-safe projection types**

`bmad_model.rs` owns:

- `ModelAuthStatusProjection` with closed `status` (`unavailable` or `development_ready`), monotonically changing `epoch`, and `mode` (`offline` or `deterministic_development`);
- `BmadHelpContextReviewProjection` with workspace/run/session, exact manifest hash, purpose, destination label, region, retention, expiry, exact inert item text/metadata, exclusions/findings, totals, disclosure text, and development-only marker;
- `BmadHelpApprovedProjection` with only manifest hash, opaque decision ID, expiry, and send eligibility;
- `BmadHelpTerminalProjection` with a closed reason code and no native error detail;
- the existing strict completed projection.

Keep the projection constructors IPC-owned and dependency-neutral: accept plain IPC input structs, then convert native `desktop-egress` and coordinator values inside `desktop-app`. Do not add a `desktop-ipc -> desktop-egress` dependency.

Do not serialize provider/profile/deployment hashes, D2/Method binding hashes, policy hashes, session authority hashes, raw proposal, receipt proof, token material, or absolute paths.

- [ ] **Step 6: Write failing host dispatch tests**

Tests must prove:

- command order is `create -> prepare -> approve -> submit -> latest/completed`;
- default/offline `prepare` returns `support_plane_offline` before context preparation and never emits a review projection;
- D2-D sign-in returns `identity_unavailable`, while sign-out increments the auth epoch and invalidates pending authority without broker activity;
- approval causes no transport;
- duplicate mutation request IDs cannot replay when no safe cache entry exists;
- prepare and submit replies never enter `ReplyCache`;
- `bmad.help.latest` prefers the exact matching in-memory safe lifecycle and otherwise authenticates the durable store projection;
- a renderer/workspace/grant mismatch is rejected before coordinator mutation;
- `context.preview` remains model-target-free.

- [ ] **Step 7: Wire commands without adding Tauri surface**

Add the seven new names to `READY_COMMANDS`; keep recovery commands exactly `app.get_boot_state` and `workspace.list`. Route all variants in `execute_command` to `HostState.bmad_model` while holding the locked authority order.

Change reply-cache eligibility so every D2 mutation (`run.create`, auth sign-in/out, prepare, approve, cancel, submit) is excluded. On gate replay with no safe cache entry, return the existing sanitized conflict; do not rerun the command.

- [ ] **Step 8: Extend the boundary checker**

Update the exact command catalogs in host, renderer expectation, IPC known-command parser, and `tools/check-boundaries.mjs`. Add checks that:

- `desktop-egress` has no network, process, filesystem, Tauri, or database dependency;
- `desktop-cloud` cannot depend on `desktop-egress` consent construction through app-only code paths beyond its authorized request types;
- renderer sources contain none of `accessToken`, `receiptProof`, `rawModelOutput`, `sessionAuthorityHash`, `modelBridgeBindingHash`, or absolute-path context fields;
- default package scripts do not enable `deterministic-help`.

- [ ] **Step 9: Run focused IPC and host proof**

Run:

```powershell
cargo test -p desktop-runtime --locked
cargo test -p desktop-ipc --test bmad_help_model_access --locked
cargo test -p desktop-app --lib commands --all-features --locked
node tools/check-boundaries.mjs
```

Expected: exact envelope, command, cache, projection, and static-boundary tests pass.

- [ ] **Step 10: Commit validated IPC**

Run:

```powershell
git add crates/desktop-runtime/src/command.rs crates/desktop-runtime/src/error.rs crates/desktop-ipc/src/bmad_model.rs crates/desktop-ipc/src/lib.rs crates/desktop-ipc/src/envelope.rs crates/desktop-ipc/tests/bmad_help_model_access.rs crates/desktop-app/src/commands.rs crates/desktop-app/src/wire.rs tools/check-boundaries.mjs
git diff --cached --name-only
git diff --cached --check
git commit -m "feat(ipc): expose reviewed BMAD Help activation"
```

---

## Task 8: Build the renderer review, consent, and result experience

**Files:**

- Modify: `apps/desktop-ui/src/lib/hostClient.ts`
- Modify: `apps/desktop-ui/src/lib/hostClient.test.ts`
- Create: `apps/desktop-ui/src/lib/bmadModelProjection.ts`
- Modify: `apps/desktop-ui/src/lib/bmadProjection.ts`
- Modify: `apps/desktop-ui/src/App.tsx`
- Modify: `apps/desktop-ui/src/App.test.tsx`
- Modify: `apps/desktop-ui/src/components/TaskWorkspace.tsx`
- Modify: `apps/desktop-ui/src/components/TaskWorkspace.test.tsx`
- Create: `apps/desktop-ui/src/components/ContextEgressReview.tsx`
- Create: `apps/desktop-ui/src/components/ContextEgressReview.test.tsx`
- Create: `apps/desktop-ui/src/components/BmadHelpResultCard.tsx`
- Create: `apps/desktop-ui/src/components/BmadHelpResultCard.test.tsx`
- Modify: `apps/desktop-ui/src/components/BmadHelpCard.tsx`
- Modify: `apps/desktop-ui/src/components/BmadHelpCard.test.tsx`
- Modify: `apps/desktop-ui/src/components/Inspector.tsx`
- Modify: `apps/desktop-ui/src/styles.css`

- [ ] **Step 1: Write failing host-client contract tests**

Add strict builders/parsers for all seven new commands and all safe reply variants. Tests must reject extra/missing keys, unsafe text, impossible lifecycle transitions, identity substitutions, non-safe integers, unrecognized terminal reasons, receipt proof, raw proposal, raw provider errors, token fields, and authority hashes.

Update `BmadHelpRunCreatedProjection` to require the retained `currentIntent` and update `LatestBmadHelpRunResult` to distinguish `retained`, `interrupted`, `completed`, `terminal`, `no_run`, and `projection_unavailable`.

- [ ] **Step 2: Define one discriminated renderer state machine**

In `bmadModelProjection.ts`, define states for:

```text
idle
creating
review_required
approving
approved
submitting
completed
interrupted
terminal
unavailable
```

Each state carries only the projections needed by that view. `approved` must carry the exact displayed manifest hash and decision ID. No reducer transition may reuse a decision after cancel, submit, binding change, or error.

- [ ] **Step 3: Write failing App flow tests**

Test the exact gesture/command behavior:

1. submitting an intent creates an inert run and prepares review;
2. the initial CTA reads `Review request`;
3. exact outbound item text is visible but never in a live region;
4. `Approve context` dispatches approve only;
5. `Send request` is disabled before approval;
6. one send dispatches once even on double click;
7. `Cancel review` performs no send;
8. workspace, grant, renderer generation, auth epoch, run, expiry, recovery, and unmount clear approval;
9. completed recommendation and safe receipt render;
10. raw proposal, proof, hashes, and provider details never render;
11. restart loads durable completion; interrupted restart never offers resend.

Add a separate default/offline flow: `Review request` receives `support_plane_offline`, renders no outbound item text, and never exposes approve, cancel, or send controls. The deterministic flow must not show a production sign-in claim; it shows only the development identity label.

- [ ] **Step 4: Implement `ContextEgressReview`**

Render:

- purpose, destination label, region, retention, development marker, and `<time dateTime>` expiry;
- exact ordered items with relative label, role, classification, counts, redactions, and inert `<pre><code>` text;
- exclusions, findings, redaction limitation, and exact consent disclosure;
- `Approve context`, `Cancel review`, and one-shot `Send request` controls.

Use `role="status"` only for short lifecycle text and `role="alert"` only for safe errors. Focus the review heading once when review opens; do not announce item bodies.

- [ ] **Step 5: Implement `BmadHelpResultCard`**

For `recommended_capability`, show display name, module/skill/action, evidence class, guidance requirement, rationale, and creation time. For `no_recommendation`, show the closed reason label and creation time. Show only receipt ID, succeeded status, transient-no-store retention, region, input/output byte counts, and start/completion times.

The development label must say `Deterministic local model — development only`; never call it connected, production, or cloud-backed.

- [ ] **Step 6: Integrate with `TaskWorkspace`, `Inspector`, and `App`**

`App` owns the state machine and command sequencing. `TaskWorkspace` owns only the intent text and the `Review request` gesture. `Inspector` receives projections and callbacks. Do not store decisions in component-local state, browser storage, or URL state.

Keep existing D1 context preview and BMAD Library flows unchanged.

- [ ] **Step 7: Pass component, integration, and accessibility tests**

Run:

```powershell
pnpm --filter @sapphirus/desktop-ui exec vitest run src/lib/hostClient.test.ts src/components/ContextEgressReview.test.tsx src/components/BmadHelpResultCard.test.tsx src/components/BmadHelpCard.test.tsx src/components/TaskWorkspace.test.tsx src/App.test.tsx
pnpm --filter @sapphirus/desktop-ui typecheck
pnpm --filter @sapphirus/desktop-ui build
pnpm verify:boundaries
```

Expected: command order, invalidation, strict parsing, keyboard/focus behavior, and axe checks pass.

- [ ] **Step 8: Commit the renderer vertical**

Run:

```powershell
git add apps/desktop-ui/src/lib/hostClient.ts apps/desktop-ui/src/lib/hostClient.test.ts apps/desktop-ui/src/lib/bmadModelProjection.ts apps/desktop-ui/src/lib/bmadProjection.ts apps/desktop-ui/src/App.tsx apps/desktop-ui/src/App.test.tsx apps/desktop-ui/src/components/TaskWorkspace.tsx apps/desktop-ui/src/components/TaskWorkspace.test.tsx apps/desktop-ui/src/components/ContextEgressReview.tsx apps/desktop-ui/src/components/ContextEgressReview.test.tsx apps/desktop-ui/src/components/BmadHelpResultCard.tsx apps/desktop-ui/src/components/BmadHelpResultCard.test.tsx apps/desktop-ui/src/components/BmadHelpCard.tsx apps/desktop-ui/src/components/BmadHelpCard.test.tsx apps/desktop-ui/src/components/Inspector.tsx apps/desktop-ui/src/styles.css
git diff --cached --name-only
git diff --cached --check
git commit -m "feat(ui): complete the BMAD Help review flow"
```

---

## Task 9: Prove D2-D in isolation and document the honest development entry point

**Files:**

- Modify: `package.json`
- Modify: `README.md`
- Preserve and commit: `docs/superpowers/plans/2026-07-16-d2-d-bmad-help-desktop-composition.md`
- Create: `docs/implementation-packets/D2-D-desktop-composition-2026-07-16.md`

- [ ] **Step 1: Add an explicit deterministic development script**

Add only:

```json
"desktop:dev:deterministic": "pnpm exec tauri dev --config crates/desktop-app/tauri.conf.json --features deterministic-help"
```

Keep `desktop:dev` and `desktop:build` free of `deterministic-help`.

- [ ] **Step 2: Run the complete isolated automated proof**

Run:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo test --workspace --locked
pnpm verify:source
dotnet test services/desktop-support-api.Tests/Sapphirus.DesktopSupportApi.Tests.csproj --no-restore
dotnet test tests/conformance/dotnet/Sapphirus.Contracts.Conformance.Tests.csproj --no-restore
dotnet test tests/generator-qualification/dotnet/Sapphirus.GeneratorQualification.Tests.csproj --no-restore
git diff --check
```

Expected: all automated gates pass under the exact pinned toolchain. Record command, exit code, and test count in the implementation packet.

- [ ] **Step 3: Run the deterministic native desktop smoke**

Create an ignored local smoke workspace containing a small README with an explicit architecture-readiness request. Start `pnpm desktop:dev:deterministic` and verify in the native window:

1. the model identity is visibly development-only;
2. create and prepare show the exact four reviewed items for the architecture signal;
3. approve changes no file and sends nothing;
4. one send produces one verified recommended-capability result and metadata receipt;
5. restarting the app recovers the same completed result;
6. a second intent requires a new run, manifest, decision, and review.

Capture the observed manifest hash, decision ID prefix only, receipt ID, and restart result in the packet. Do not record outbound text or proof material in the packet.

- [ ] **Step 4: Run the default-offline native smoke**

Start `pnpm desktop:dev` without features. Create a fresh inert run and select `Review request`. Verify the safe `support_plane_offline` outcome occurs before any outbound item is projected, no approve/cancel/send control or decision ID appears, no deterministic label/result exists, and no fallback dispatch or Method transition occurs.

- [ ] **Step 5: Document exact claims and remaining blockers**

Update README and the implementation packet to state:

- D2-D is a deterministic development vertical, not production AI;
- the default build is offline;
- D3 is not yet merged at this checkpoint;
- D2-E still owns production identity, consent verification, managed brokerage, and signed receipt reconciliation;
- release signing and clean-machine packaging remain separate blockers.

- [ ] **Step 6: Commit the isolated D2-D closure**

Run:

```powershell
git add package.json README.md docs/superpowers/plans/2026-07-16-d2-d-bmad-help-desktop-composition.md docs/implementation-packets/D2-D-desktop-composition-2026-07-16.md
git diff --cached --name-only
git diff --cached --check
git commit -m "docs(bmad): close the D2-D desktop activation gate"
git status --short
```

Expected: the D2 branch is clean before integration.

---

## Task 10: Merge the committed D3 baseline and preserve both authority systems

**Files likely to conflict:**

- `crates/desktop-app/icons/icon.ico`
- `crates/desktop-app/src/commands.rs`
- `crates/desktop-app/src/lib.rs`
- `crates/desktop-app/src/state.rs`
- `crates/desktop-app/src/wire.rs`
- `crates/desktop-runtime/src/command.rs`
- `crates/desktop-ipc/src/envelope.rs`
- `apps/desktop-ui/src/App.tsx`
- `apps/desktop-ui/src/App.test.tsx`
- `apps/desktop-ui/src/components/Inspector.tsx`
- `apps/desktop-ui/src/lib/hostClient.ts`
- `apps/desktop-ui/src/lib/hostClient.test.ts`
- `apps/desktop-ui/src/styles.css`
- `README.md`
- `package.json`
- `tools/check-boundaries.mjs`

- [ ] **Step 1: Verify clean D2 and snapshot the untouched primary checkout**

Run:

```powershell
git -C C:\tmp\d2 status --short
git -C C:\tmp\d2 rev-parse HEAD
git -C C:\Users\rodri\source\bmad-runtime-dev rev-parse HEAD
git -C C:\Users\rodri\source\bmad-runtime-dev status --short
```

Expected: D2 is clean; the primary HEAD is `6365c8a6`; its existing dirty support-plane inventory is recorded and remains untouched.

- [ ] **Step 2: Start the no-fast-forward merge without auto-commit**

Run in `C:\tmp\d2`:

```powershell
git merge --no-ff --no-commit 6365c8a6
git status --short
```

Expected: a bounded set of shared desktop host/renderer conflicts. D2-only `desktop-cloud`, `desktop-egress`, bridge, coordinator, store-completion, and design files remain present.

- [ ] **Step 3: Resolve native host conflicts as a union**

Preserve all of the following:

- D3 `edits` module, pending proposals, journal reconciliation, edit commands, projections, and tests;
- D2 `bmad_model` module, coordinator mutex, invalidation hooks, eight commands, projections, and tests;
- one shared `READY_COMMANDS` array containing both catalogs;
- D2 no-cache rules for sensitive Help mutations;
- D3 replay/checkpoint rules for governed edits;
- the locked authority order.

For icons, keep `app-icon.svg` as the canonical source, regenerate the D2 asset inventory, and resolve `icon.ico` to those generated bytes. Remove the competing D3 `icon.svg` source so the tree has one documented icon source.

- [ ] **Step 4: Resolve renderer conflicts on the D3 presentation baseline**

Keep D3’s `GovernedChangesPanel`, shell layout, visual contract, and edit callbacks. Insert the D2 review/result state machine into the Method section of that layout. Preserve both test suites and exact host-client command parsers.

Do not feed a completed Help proposal into `changes.propose`; D3 proposal authoring remains explicit renderer input in this milestone.

- [ ] **Step 5: Resolve scripts, docs, and boundary checks**

Use the D3 versions as the baseline for `package.json`, README, and `tools/check-boundaries.mjs`, then add the D2 deterministic script, honest D2 claims, exact union command catalog, and D2 secrecy/dependency checks.

Keep `desktop:dev` default-offline and keep D3 production build scripts unchanged.

- [ ] **Step 6: Write failing cross-authority integration tests before completing the merge**

Add tests proving:

1. a BMAD Help decision ID is rejected by `approval.decide`;
2. a D3 changes approval ID is rejected by `bmad.help.submit`;
3. enabling governed edits increments the workspace grant and invalidates a pending Help review;
4. Help completion performs no file write, checkpointed edit execution, or rollback creation;
5. D3 apply/undo does not create, approve, consume, or dispatch a model request;
6. both panels render together without hiding or relabeling either authority.

Observe the focused failures before adding the minimum integration corrections.

- [ ] **Step 7: Run focused union proof while the merge is open**

Run:

```powershell
cargo test -p desktop-app --lib --all-features --locked
cargo test -p desktop-ipc -p desktop-runtime -p desktop-store --all-features --locked
pnpm --filter @sapphirus/desktop-ui exec vitest run
pnpm --filter @sapphirus/desktop-ui typecheck
node tools/check-boundaries.mjs
git diff --check
```

Expected: all D2 and D3 focused tests pass and no conflict markers remain.

- [ ] **Step 8: Complete the merge commit**

Run:

```powershell
git status --short
git diff --cached --name-only
git diff --cached --check
git commit -m "merge: integrate D3 governed edits baseline"
```

Expected: one merge commit with parents equal to the D2-D closure and `6365c8a6`.

- [ ] **Step 9: Confirm the primary checkout was not mutated**

Run:

```powershell
git -C C:\Users\rodri\source\bmad-runtime-dev rev-parse HEAD
git -C C:\Users\rodri\source\bmad-runtime-dev status --short
```

Expected: HEAD and the pre-merge dirty inventory match the snapshot from Step 1.

---

## Task 11: Qualify the integrated D2-D/D3 desktop behavior

**Files:**

- Modify only files implicated by a newly failing integrated test
- Update: `docs/implementation-packets/D2-D-desktop-composition-2026-07-16.md`

- [ ] **Step 1: Run the integrated native host regression matrix**

Run:

```powershell
cargo test -p desktop-app --lib --all-features --locked
cargo test -p desktop-app --lib --locked
cargo test -p desktop-workspace -p desktop-execution -p desktop-store --all-features --locked
```

Expected: D3 enable/propose/review/apply/undo/conflict/history/recovery tests and D2 create/prepare/approve/submit/completion/restart tests all pass.

- [ ] **Step 2: Run the integrated renderer regression matrix**

Run:

```powershell
pnpm --filter @sapphirus/desktop-ui exec vitest run
pnpm --filter @sapphirus/desktop-ui typecheck
pnpm --filter @sapphirus/desktop-ui lint
pnpm --filter @sapphirus/desktop-ui build
```

Expected: governed changes, BMAD Help, host client, App, visual-contract, and accessibility tests pass together.

- [ ] **Step 3: Correct only observed integration defects with fresh RED-GREEN cycles**

For each failure, add or tighten one focused regression test, rerun it to observe failure, apply the smallest fix, then rerun the owning package. Do not alter the D2/D3 authority split.

- [ ] **Step 4: Repeat both native smoke paths on the merged UI**

Repeat Task 9’s deterministic and default-offline native flows. Additionally create/apply/undo one D3 manual proposal before and after the Help flow. Verify that Help neither enables edits nor authors a file proposal and that edit approval never enables model submit.

- [ ] **Step 5: Commit post-merge corrections only if files changed**

If corrections were necessary, stage only their exact files plus updated tests and run:

```powershell
git diff --cached --name-only
git diff --cached --check
git commit -m "fix(desktop): preserve D2 and D3 authority isolation"
```

If no correction was necessary, leave the merge commit as the integrated checkpoint and only update evidence in the final task.

---

## Task 12: Run full integrated gates and record the D2-E handoff

**Files:**

- Modify: `README.md` only if gate evidence or final commit IDs changed after the merge
- Modify: `docs/implementation-packets/D2-D-desktop-composition-2026-07-16.md`
- Update outside repo when permitted: `C:\Users\rodri\source\BigBrain\03-projects\sapphirus-bmad-runtime.md`
- Update outside repo when permitted: `C:\Users\rodri\source\BigBrain\00-meta\active-focus.md`

- [ ] **Step 1: Verify the exact pinned JavaScript toolchain**

Run:

```powershell
node --version
pnpm --version
```

Expected: `v24.18.0` and `11.12.0`. Stop if either differs.

- [ ] **Step 2: Run all Rust gates**

Run:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo test --workspace --locked
```

Expected: formatting, strict Clippy, all-feature tests, default-offline tests, and compile-fail doctests pass.

- [ ] **Step 3: Run all source and renderer gates**

Run:

```powershell
pnpm verify:source
pnpm contracts:verify:cross-language
pnpm build
```

Expected: vault/foundation checks, TypeScript contracts, secret scan, typecheck, lint, all renderer tests, architecture boundaries, and production Vite build pass.

- [ ] **Step 4: Run .NET support and contract gates from the isolated integrated tree**

Run:

```powershell
dotnet test services/desktop-support-api.Tests/Sapphirus.DesktopSupportApi.Tests.csproj --no-restore
dotnet test tests/conformance/dotnet/Sapphirus.Contracts.Conformance.Tests.csproj --no-restore
dotnet test tests/generator-qualification/dotnet/Sapphirus.GeneratorQualification.Tests.csproj --no-restore
```

Expected: committed support API and both contract suites pass without reading or modifying the primary checkout’s uncommitted support-plane lane.

- [ ] **Step 5: Run final repository integrity checks**

Run:

```powershell
git diff --check
git status --short
git log --oneline --decorate -12
git merge-base --is-ancestor 6365c8a6 HEAD
```

Expected: no whitespace errors, only deliberate evidence-note changes before the final documentation commit, visible bounded D2-D commits, and successful D3 ancestry.

- [ ] **Step 6: Record durable evidence without overstating production readiness**

Update the implementation packet with exact commit IDs, commands, exit codes, test counts, deterministic/default native smoke evidence, and any environment-limited gap.

Update BigBrain with factual closure and the next boundary:

- D2-D deterministic reviewed Help vertical complete and integrated with D3;
- default composition still offline;
- D3 governed edits remain manual/renderer-authored and authority-separated;
- D2-E production identity, consent, managed model brokerage, and signed receipt bridge is next;
- packaging/signing, AJV requalification, distribution-root classification, clean-machine smoke, and richer recovery/history remain separate open work.

Do not invent a new readiness percentage in this implementation task; a later audit may recalculate it from the verified evidence.

- [ ] **Step 7: Commit final evidence**

Run:

```powershell
git add README.md docs/implementation-packets/D2-D-desktop-composition-2026-07-16.md
git diff --cached --name-only
git diff --cached --check
git commit -m "docs(bmad): record integrated D2-D and D3 proof"
git status --short
```

If README did not change, stage only the implementation packet. Expected final D2 worktree status: clean.

## Completion criteria

The plan is complete only when all of the following are true:

- inherited D2-D changes are classified and committed in bounded checkpoints;
- the deterministic build performs create, exact review, approve, one send, verification, canonical materialization, atomic completion, and restart recovery;
- the default build returns offline before context review or consent and never falls back to deterministic output;
- duplicate, stale, expired, rebound, restarted, or cross-authority decisions fail closed;
- no token, proof, raw output, absolute path, or native authority object crosses IPC;
- native deterministic and default-offline smoke both pass;
- `6365c8a6` is an ancestor of the integrated D2 branch;
- D3 enable/propose/review/apply/undo/conflict/history/recovery remains green;
- Help approval cannot authorize edits and edit approval cannot authorize Help submit;
- full Rust, source, renderer, boundary, build, .NET, and repository-integrity gates pass;
- the branch and primary checkout states are both accounted for, and the primary dirty lane is unchanged;
- durable documentation names D2-E and release qualification as remaining blockers.
