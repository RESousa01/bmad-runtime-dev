# BMAD-06A Verified Result Lineage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prevent renderer/model JSON or an unbound Rust result from advancing a BMAD Method session by requiring exact pre-call D2/Method bridge lineage and a trusted-host-only, non-deserializable result envelope whose distinct raw-response, accepted-result, and receipt-evidence hashes survive durable checkpoint persistence and restart.

**Architecture:** `desktop-runtime` remains the BMAD authority. `begin_advance` captures the already-authorized D2 decision-consumption, request, invocation-binding, and BMAD-derived session-authority hashes before the call, then seals an explicit D2/Method bridge hash. A trusted host adapter may construct a non-deserializable `MethodVerifiedAdvanceResult` after D2 verification; BMAD independently checks every pre-call Method/model/schema/request/bridge field and the canonical accepted result before transitioning. This is a Rust trust boundary and durable anti-drift proof, not yet cryptographic evidence that D2 verification ran: actual Help composition stays disabled until D2 `VerifiedModelOutput` is opaque and a production receipt verifier supplies canonical receipt evidence. `desktop-store` atomically links the aggregate projection, checkpoint index, evidence, and outbox through its existing SQLite CAS transaction. The encrypted content-addressed payload is staged first and may leave an unreferenced orphan after rollback, but it never becomes authoritative without that relational commit. No network, cloud, renderer, or schema-table expansion is introduced.

**Tech Stack:** Rust 2021, Serde, canonical SHA-256 hashing, `desktop-runtime`, `desktop-store`, SQLite/rusqlite, Cargo tests.

## Global Constraints

- Do not modify the active D2 worktree or D2-owned `desktop-cloud` / `desktop-egress` implementation.
- Do not add a dependency from BMAD runtime code to cloud, egress, Tauri, support API, renderer, or IPC crates.
- Do not connect the Help run or advertise it as runnable; this is a trust-boundary foundation checkpoint.
- Do not change the handwritten step-table authority or let model content choose transitions, persistence, artifacts, tools, paths, or commands.
- No public `Deserialize` implementation may exist for the verified-result envelope; renderer or model JSON must not mint authority evidence.
- Do not claim that a public trusted-host Rust factory cryptographically proves D2 verification. D2 output sealing and production receipt verification are hard prerequisites for enabling the Help composition path.
- A failed proof must leave version, state, active invocation, current step, and checkpoints unchanged.
- Canonical accepted-result hashing is independent of provider transport. The later D2 adapter will map its verified output and receipt into the opaque hashes defined here.
- Existing Created/unbound persisted Method sessions must still restore without a store migration. Add no migration unless a failing compatibility test proves one is necessary.
- Compatibility is deliberately limited to released Created/unbound v1 Help state. Pre-integration v1 Advancing/Completed test state without the new nested lineage fails closed and is not silently upgraded.
- Preserve all unrelated dirty worktree files and stage only the files named by each task.
- Work test-first, run strict Clippy, and create one focused commit per task.

## File Map

- `crates/desktop-runtime/src/bmad/method.rs` — request/receipt lineage, sealed verified-result boundary, transition validation, checkpoint hashing, and restart validation.
- `crates/desktop-runtime/src/bmad/ports.rs` — model port returns verified BMAD content rather than a raw result.
- `crates/desktop-runtime/src/bmad/mod.rs` and `crates/desktop-runtime/src/lib.rs` — export the new transport-neutral verification types.
- `crates/desktop-runtime/tests/bmad_method_session.rs` — domain red/green tests for exact proof matching, non-mutation, checkpoint lineage, and restore.
- `crates/desktop-runtime/src/bmad/service.rs` — service accepts the sealed result and validates artifact references from its inner result.
- `crates/desktop-store/tests/bmad_method_store.rs` — atomic persistence, restart, tamper, rollback, and Created/unbound compatibility tests.

---

### Task 1: Seal exact Method result lineage in the runtime authority

**Files:**
- Modify: `crates/desktop-runtime/src/bmad/method.rs`
- Modify: `crates/desktop-runtime/src/bmad/ports.rs`
- Modify: `crates/desktop-runtime/src/bmad/service.rs`
- Modify: `crates/desktop-runtime/src/bmad/mod.rs`
- Modify: `crates/desktop-runtime/src/lib.rs`
- Modify: `crates/desktop-runtime/tests/bmad_method_session.rs`

**Interfaces:**
- Extend `MethodAdvanceRequest` and `MethodAdvanceReceipt` with `decision_consumption_hash: Sha256Digest`, `model_request_id: ContractId`, `model_request_hash: Sha256Digest`, `session_authority_hash: Sha256Digest`, `d2_model_invocation_binding_hash: Sha256Digest`, and `model_bridge_binding_hash: Sha256Digest`.
- Add `MethodVerifiedResultBindingData` containing the receipt/request fields plus `method_binding_hash`, `model_binding_hash`, `response_schema_hash`, `session_authority_hash`, `d2_model_invocation_binding_hash`, `model_bridge_binding_hash`, `model_response_payload_hash`, `accepted_method_result_hash`, and `model_receipt_evidence_hash`.
- Add a sealed `MethodVerifiedAdvanceResult` with private `result`, `binding`, and `verification_hash` fields, no `Deserialize`, a validating constructor, and read-only getters.
- Change `MethodModelPort::advance` to return `MethodVerifiedAdvanceResult`.
- Change `MethodSession::accept_result` to accept the sealed result instead of a raw `MethodAdvanceResult` plus separately trusted invocation ID.
- Change `MethodSessionService::accept_result` in the same task so the `desktop-runtime` crate has no interim raw-result path and remains compilable.

- [ ] **Step 1: Write failing exact-lineage tests**

Add helpers that construct a valid request and a verified result from the fixture binding. Add tests proving:

1. A raw `MethodAdvanceResult` is no longer accepted by the transition API.
2. The sealed constructor rejects a claimed `accepted_method_result_hash` that differs from `canonical_hash("bmad-method-advance-result", 1, &result)` with `MethodResultInvalid`.
3. Acceptance rejects, one field at a time, mismatched invocation ID, decision ID, decision-consumption hash, model request ID/hash, session authority, D2 invocation binding, D2/Method bridge binding, exact Method binding, model binding, response schema, accepted-result hash, and verification hash.
4. Every rejection leaves a cloned pre-call session exactly unchanged.
5. A valid proof advances according to the handwritten step table and writes every lineage field to the returned checkpoint.
6. `to_persisted_json` / `from_persisted_json` preserves and revalidates the proof-bound checkpoint.

`model_response_payload_hash` is D2's exact raw JSON byte hash. `accepted_method_result_hash` is BMAD's canonical parsed projection hash; they are intentionally distinct. `model_receipt_evidence_hash` is post-call trusted-host evidence defined for later D2 composition as `canonical_hash("model-access-receipt-evidence", 1, &complete_verified_receipt)`, so BMAD cannot compare it to a pre-call expected value in this isolated crate. Bind all three into the sealed verification hash and durable checkpoint, then prove post-acceptance tampering fails restore. For tampering tests, serialize the accepted session to JSON, mutate one stored checkpoint lineage field without recomputing its checkpoint hash, and assert restore returns `MethodStoreRecoveryRequired`. Test private verification-hash tampering in a module test if integration visibility cannot represent the invalid sealed value.

- [ ] **Step 2: Run the runtime test and verify RED**

Run:

```powershell
& 'C:\Users\rodri\.cargo\bin\cargo.exe' test -p desktop-runtime --test bmad_method_session
```

Expected: compilation fails because the request lineage and verified-result boundary do not yet exist.

- [ ] **Step 3: Implement pre-call request lineage**

Add the six exact pre-call lineage fields to both request and receipt. Include them in:

- idempotent replay equality checks;
- the `bmad-context-decision-consumption-id` canonical input;
- the active receipt persisted in `consumed_decisions` and `idempotent_advances`;
- restored consumption validation.

At `begin_advance`, compute `session_authority_hash = canonical_hash("bmad-method-session-authority", 1, &{ sessionId, scope, methodBindingHash })` using an explicitly named camelCase projection and reject a different request value. Compute `model_bridge_binding_hash = canonical_hash("bmad-method-d2-bridge-binding", 1, &{ sessionAuthorityHash, d2ModelInvocationBindingHash, methodBindingHash, modelBindingHash, responseSchemaHash })` with the same named-projection discipline and reject a different request value. An idempotency-key replay is valid only when invocation, decision, decision-consumption, request ID/hash, session authority, D2 invocation binding, bridge binding, and expected aggregate version all match. Any drift returns the existing stable conflict/stale error without mutation.

- [ ] **Step 4: Implement the sealed verified-result boundary**

Implement a transport-neutral shape equivalent to:

```rust
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MethodVerifiedResultBindingData {
    pub invocation_id: ContractId,
    pub decision_id: ContractId,
    pub decision_consumption_hash: Sha256Digest,
    pub model_request_id: ContractId,
    pub model_request_hash: Sha256Digest,
    pub session_authority_hash: Sha256Digest,
    pub d2_model_invocation_binding_hash: Sha256Digest,
    pub model_bridge_binding_hash: Sha256Digest,
    pub method_binding_hash: Sha256Digest,
    pub model_binding_hash: Sha256Digest,
    pub response_schema_hash: Sha256Digest,
    pub model_response_payload_hash: Sha256Digest,
    pub accepted_method_result_hash: Sha256Digest,
    pub model_receipt_evidence_hash: Sha256Digest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodVerifiedAdvanceResult {
    result: MethodAdvanceResult,
    binding: MethodVerifiedResultBindingData,
    verification_hash: Sha256Digest,
}
```

The constructor must:

- canonical-hash the accepted result with purpose `bmad-method-advance-result`, version `1`;
- reject a different claimed `accepted_method_result_hash`;
- canonical-hash `MethodVerifiedResultBindingData` with purpose `bmad-method-verified-result-binding`, version `1`;
- retain private fields and expose only immutable getters.

Provide an internal verification method that recomputes both hashes. Do not derive or manually implement `Deserialize` for the envelope.

Name the factory to expose its trust boundary (for example `from_trusted_host_evidence`) and document that it provides internal consistency/anti-drift, not proof that D2 verification ran. D2 independently verifies the raw payload and service receipt before the later adapter creates this type. Actual composition must accept only an opaque D2 verified output and a production-verified receipt.

- [ ] **Step 5: Bind transition and checkpoint authority to the proof**

In `MethodSession::accept_result`, verify before any mutation that:

- the session is at the expected version in `Advancing`;
- the active receipt matches the proof's invocation, decision, decision-consumption, model request ID, and request hash;
- the active receipt matches session authority, D2 invocation binding, and D2/Method bridge binding;
- `MethodExactBinding::binding_hash()` matches `method_binding_hash`;
- the exact binding's `model_binding_hash` and `model_binding.data.response_schema_hash` match the proof;
- accepted-result and verified-binding hashes recompute exactly;
- the handwritten step table accepts the inner result.

Extend `MethodCheckpoint` and `CheckpointHashInput` with:

- `method_binding_hash`;
- `session_authority_hash`, `d2_model_invocation_binding_hash`, and `model_bridge_binding_hash`;
- `decision_consumption_hash`;
- `model_request_id` and `model_request_hash`;
- `response_schema_hash`;
- `model_response_payload_hash`;
- `accepted_method_result_hash`;
- `model_receipt_evidence_hash`;
- `verified_result_binding_hash`.

Also retain `advance_disposition` in the checkpoint/hash. On restore, reconstruct the exact `MethodAdvanceResult` from disposition, current/next step, and working-artifact refs, then require its canonical hash to equal `accepted_method_result_hash`; the hash must not be treated as self-authenticating metadata.

Include every field in `bmad-method-checkpoint` hashing and checkpoint construction. On restore, resolve the checkpoint's binding revision and consumed decision, then revalidate every checkpoint field against the matching revision/receipt plus its canonical checkpoint hash. Preserve exact historical binding semantics after capability rebinds.

- [ ] **Step 6: Update the model/service ports, exports, and existing runtime fixtures**

Return `MethodVerifiedAdvanceResult` from `MethodModelPort::advance`. Change `MethodSessionService::accept_result` to receive the sealed result, derive provenance/disposition/artifact refs through immutable getters, validate artifacts first, and then let the aggregate independently validate the proof. Re-export the new types from `bmad/mod.rs` and crate `lib.rs`. Update every runtime Method request/result fixture to use valid exact lineage and the sealed envelope; do not weaken existing invented-step, replay, rebind, recovery, or artifact-reference tests.

- [ ] **Step 7: Run focused and strict runtime gates**

Run:

```powershell
& 'C:\Users\rodri\.cargo\bin\cargo.exe' fmt --all -- --check
& 'C:\Users\rodri\.cargo\bin\cargo.exe' test -p desktop-runtime --test bmad_method_session
& 'C:\Users\rodri\.cargo\bin\cargo.exe' test -p desktop-runtime --locked
& 'C:\Users\rodri\.cargo\bin\cargo.exe' clippy -p desktop-runtime --all-targets --locked -- -D warnings
```

Expected: all commands exit zero.

- [ ] **Step 8: Commit Task 1**

```powershell
git add -- crates/desktop-runtime/src/bmad/method.rs crates/desktop-runtime/src/bmad/ports.rs crates/desktop-runtime/src/bmad/service.rs crates/desktop-runtime/src/bmad/mod.rs crates/desktop-runtime/src/lib.rs crates/desktop-runtime/tests/bmad_method_session.rs docs/superpowers/plans/2026-07-15-bmad-06-verified-result-lineage.md
git commit -m "feat(bmad): verify exact model result lineage"
```

---

### Task 2: Persist and recover verified Method checkpoints atomically

**Files:**
- Modify: `crates/desktop-store/tests/bmad_method_store.rs`
- Modify: `crates/desktop-store/src/bmad_method.rs`

**Interfaces:**
- Consume the Task 1 `MethodSessionService::accept_result` verified-result boundary from store integration tests.
- Reuse `MethodSessionRepository::persist_method_transition` and its existing aggregate-version CAS transaction.
- Do not add a v10 schema migration when the existing encrypted aggregate plus checkpoint evidence rows prove sufficient.

- [ ] **Step 1: Write failing service/store tests**

Update existing fixtures for request lineage and sealed results, then add tests proving:

1. A service-accepted verified result atomically persists aggregate state, one checkpoint row, state evidence, and outbox evidence.
2. Closing and reopening the store restores every decision-consumption, request, Method binding, model binding, response schema, result, receipt, and verified-binding hash from the checkpoint.
3. A failed artifact-validation trigger or injected repository failure rolls back every authoritative relational change: the durable session remains `Advancing`, retains its active invocation, keeps the same version, and has no checkpoint/evidence/outbox residue. Do not assert that append-only orphan CAS staging is rolled back.
4. Direct SQL tampering of the checkpoint index hash is detected by `verify_integrity`; repointing the session projection to a registered but stale payload or corrupting the referenced encrypted CAS file is detected by load/`verify_integrity`. Normal `LocalStore::open` alone is not an integrity gate.
5. A frozen literal pre-BMAD-06 Created/unbound v1 JSON document still restores, proving no migration is required for sessions with no invocation/checkpoint lineage.

- [ ] **Step 2: Run the store test and verify RED**

Run:

```powershell
& 'C:\Users\rodri\.cargo\bin\cargo.exe' test -p desktop-store --test bmad_method_store
```

Expected: compilation fails because store idempotency checks and fixtures lack the new request/result/bridge lineage.

- [ ] **Step 3: Tighten the store receipt boundary**

Update store imports and `same_receipt_or_conflict` so an idempotent replay matches only when decision-consumption hash, model request ID/hash, session authority, D2 invocation binding, and bridge binding also match the stored receipt. Keep canonical receipt JSON/hash verification unchanged so the new fields are authenticated without adding SQL columns.

Keep ordering fail-closed:

The Task 1 service ordering remains fail-closed: load authority session, derive provenance, validate artifacts, validate proof/transition, then persist through the existing CAS transaction.

- [ ] **Step 4: Complete durable lineage and rollback tests**

Update store fixtures and assertions. Add durable replay tests that vary each of the six new pre-call request/bridge-lineage fields under the same idempotency key, exact success counts for checkpoint/result-evidence/outbox rows, full restart-lineage assertions (including distinct raw-response, accepted-result, and receipt-evidence hashes), and durable rollback assertions after artifact and evidence failures. Keep the production change limited to exact replay comparison unless a focused test exposes another real persistence defect. The existing encrypted aggregate already contains the extended checkpoint; do not duplicate its fields into new SQL columns.

- [ ] **Step 5: Run focused and strict store gates**

Run:

```powershell
& 'C:\Users\rodri\.cargo\bin\cargo.exe' fmt --all -- --check
& 'C:\Users\rodri\.cargo\bin\cargo.exe' test -p desktop-store --test bmad_method_store
& 'C:\Users\rodri\.cargo\bin\cargo.exe' test -p desktop-store --locked
& 'C:\Users\rodri\.cargo\bin\cargo.exe' clippy -p desktop-store --all-targets --locked -- -D warnings
```

Expected: all commands exit zero.

- [ ] **Step 6: Commit Task 2**

```powershell
git add -- crates/desktop-store/src/bmad_method.rs crates/desktop-store/tests/bmad_method_store.rs
git commit -m "feat(bmad): retain verified result checkpoints"
```

---

## Final Verification

After both reviewed commits, run:

```powershell
& 'C:\Users\rodri\.cargo\bin\cargo.exe' fmt --all -- --check
& 'C:\Users\rodri\.cargo\bin\cargo.exe' clippy -p desktop-runtime -p desktop-store -p desktop-ipc -p desktop-app --all-targets --locked -- -D warnings
& 'C:\Users\rodri\.cargo\bin\cargo.exe' test --workspace --locked
& 'C:\tmp\node-v24.18.0-win-x64\node.exe' tools/check-boundaries.mjs
git diff --check
```

Then run the exact existing UI TypeScript, Vitest, and production build commands from the repository package scripts. Record gate evidence in BigBrain and request an independent final diff review before claiming the checkpoint complete.
