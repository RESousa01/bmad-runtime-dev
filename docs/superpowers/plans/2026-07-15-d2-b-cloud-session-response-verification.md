# D2-B Cloud Session and Response Verification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a secret-safe identity session and a typed model-call boundary that accepts only consumed D2 consent and verifies every response/receipt binding before exposing model output.

**Architecture:** `desktop-cloud` gains two focused modules. `identity` owns transient access-token lifecycle and session epochs; `model` maps sealed `desktop-egress` authority into an authorized request and validates untrusted transport responses through explicit schema and receipt-verification ports. Existing offline behavior remains fail-closed and production never falls back to deterministic adapters.

**Tech Stack:** Rust 2021, async-trait, desktop-runtime, desktop-egress, parking_lot, serde/serde_json, sha2, thiserror, tokio, zeroize.

## Global Constraints

- Work only in the isolated `codex/d2-ai-request` worktree.
- Do not modify BMAD contracts/services, Tauri composition, desktop UI, icon assets, or Desktop Support API files.
- Tokens are neither serializable nor exposed by `Debug`; they are zeroized on drop.
- Sign-out invalidates every previously issued session grant even if broker sign-out fails.
- Account or tenant mismatch fails closed and never returns a token-bearing grant.
- A model request requires a verified `DecisionConsumption`; request construction cannot invent consent.
- Every response field is untrusted until request, schema, payload, manifest, consent, profile, deployment, region, retention, and receipt bindings pass.
- `transient_no_store` remains the only accepted retention mode.
- Full-workspace testing has a known pre-existing blocker: committed Tauri configuration expects untracked `crates/desktop-app/icons/icon.ico`. D2 proof uses the full workspace excluding `desktop-app`, plus focused D2 gates.

## File map

- `crates/desktop-cloud/Cargo.toml` — adds D2 authority, synchronization, and token-zeroization dependencies.
- `crates/desktop-cloud/src/lib.rs` — preserves existing public API and re-exports focused modules.
- `crates/desktop-cloud/src/identity.rs` — broker port, secret token wrapper, session status/epoch, authorization, and sign-out invalidation.
- `crates/desktop-cloud/src/model.rs` — authorized request, raw receipt/output, validator ports, response verifier, and deterministic transport seam.
- `crates/desktop-cloud/tests/identity_session.rs` — identity/session behavior.
- `crates/desktop-cloud/tests/model_verification.rs` — exact request and response binding behavior.
- `crates/desktop-egress/src/consent.rs` — adds public verification for the already sealed consumption aggregate.
- `crates/desktop-egress/tests/consent.rs` — proves consumption tamper is detected.

---

### Task 1: Secret-safe identity session and epoch invalidation

**Files:**
- Modify: `crates/desktop-cloud/Cargo.toml`
- Modify: `crates/desktop-cloud/src/lib.rs`
- Create: `crates/desktop-cloud/src/identity.rs`
- Create: `crates/desktop-cloud/tests/identity_session.rs`

**Interfaces:**
- Produces: `IdentityBroker`, `BrokerToken`, `CloudSession`, `CloudAccess`, `SessionSnapshot`, and stable `CloudError` identity variants.
- `IdentityBroker::acquire_token(&self) -> Result<BrokerToken, CloudError>` and `IdentityBroker::sign_out(&self) -> Result<(), CloudError>` are the only identity side effects.

- [ ] **Step 1: Write failing identity tests**

```rust
#[tokio::test]
async fn access_token_debug_is_redacted_and_sign_out_invalidates_the_epoch() {
    let broker = StaticBroker::successful("secret-token", "tenant_ref", "account_ref");
    let session = CloudSession::new(broker, id("tenant_ref"));
    let access = session.acquire_access().await.expect("access");
    assert!(!format!("{access:?}").contains("secret-token"));
    assert!(access.is_current(&session));
    session.sign_out().await.expect("sign out");
    assert!(!access.is_current(&session));
    assert_eq!(session.snapshot().status, AuthStatus::SignedOut);
}

#[tokio::test]
async fn local_sign_out_remains_terminal_when_broker_cleanup_fails() {
    let broker = StaticBroker::sign_out_failure();
    let session = CloudSession::new(broker, id("tenant_ref"));
    let before = session.snapshot().epoch;
    assert_eq!(session.sign_out().await, Err(CloudError::IdentityUnavailable));
    assert_eq!(session.snapshot().epoch, before + 1);
    assert_eq!(session.snapshot().status, AuthStatus::SignedOut);
}

#[tokio::test]
async fn tenant_substitution_never_returns_an_access_grant() {
    let broker = StaticBroker::successful("secret-token", "other_tenant", "account_ref");
    let session = CloudSession::new(broker, id("tenant_ref"));
    assert_eq!(session.acquire_access().await, Err(CloudError::TenantMismatch));
    assert_eq!(session.snapshot().status, AuthStatus::ReauthenticationRequired);
}
```

- [ ] **Step 2: Run identity tests and verify RED**

Run: `cargo test -p desktop-cloud --test identity_session`

Expected: compilation fails because identity session types do not exist.

- [ ] **Step 3: Implement minimal identity lifecycle**

```rust
#[async_trait]
pub trait IdentityBroker: Send + Sync {
    async fn acquire_token(&self) -> Result<BrokerToken, CloudError>;
    async fn sign_out(&self) -> Result<(), CloudError>;
}

impl<B: IdentityBroker> CloudSession<B> {
    pub async fn acquire_access(&self) -> Result<CloudAccess, CloudError> {
        let starting_epoch = self.snapshot().epoch;
        let token = self.broker.acquire_token().await?;
        let mut state = self.state.lock();
        if state.epoch != starting_epoch {
            return Err(CloudError::SessionInvalidated);
        }
        if token.tenant_ref != self.expected_tenant_ref {
            state.status = AuthStatus::ReauthenticationRequired;
            return Err(CloudError::TenantMismatch);
        }
        state.status = AuthStatus::SignedIn;
        Ok(CloudAccess::new(token, state.epoch))
    }

    pub async fn sign_out(&self) -> Result<(), CloudError> {
        {
            let mut state = self.state.lock();
            state.epoch = state.epoch.checked_add(1).ok_or(CloudError::SessionInvalidated)?;
            state.status = AuthStatus::SignedOut;
        }
        self.broker.sign_out().await
    }
}
```

`BrokerToken` and `CloudAccess` own `Zeroizing<String>`. Their `Debug` implementations print only
metadata plus `access_token: "[REDACTED]"`. Neither type implements `Serialize`, `Deserialize`, or
`Clone`. `CloudAccess::with_bearer` provides a short-lived borrow to the transport adapter.

- [ ] **Step 4: Run focused and regression gates**

Run: `cargo test -p desktop-cloud --test identity_session`

Expected: identity tests pass.

Run: `cargo test -p desktop-cloud && cargo clippy -p desktop-cloud --all-targets -- -D warnings`

Expected: existing offline test plus identity tests pass; clippy exits zero.

- [ ] **Step 5: Commit Task 1**

```powershell
git add -- Cargo.lock crates/desktop-cloud/Cargo.toml crates/desktop-cloud/src/lib.rs crates/desktop-cloud/src/identity.rs crates/desktop-cloud/tests/identity_session.rs
git commit -m "feat(d2): add secret-safe cloud sessions"
```

---

### Task 2: Authorized model request and exact response verification

**Files:**
- Modify: `crates/desktop-cloud/src/lib.rs`
- Create: `crates/desktop-cloud/src/model.rs`
- Create: `crates/desktop-cloud/tests/model_verification.rs`
- Modify: `crates/desktop-egress/src/consent.rs`
- Modify: `crates/desktop-egress/tests/consent.rs`

**Interfaces:**
- Consumes: `ContextEgressManifest`, `ModelInvocationBinding`, and `DecisionConsumption` from `desktop-egress`.
- Produces: `AuthorizedModelRequest`, `RawModelOutput`, `ModelAccessReceipt`, `VerifiedModelOutput`, `CanonicalOutputValidator`, `ReceiptVerifier`, and `verify_model_response`.

- [ ] **Step 1: Write failing consumption-integrity and model-verification tests**

```rust
#[test]
fn consumption_tamper_is_detected() {
    let mut consumption = fixture_consumption_record();
    consumption.invocation_id = ContractId::new("tampered_invocation").unwrap();
    assert_eq!(consumption.verify(), Err(EgressError::DecisionIntegrity));
}

#[test]
fn authorized_request_requires_the_exact_consumed_binding() {
    let (manifest, binding, consumption) = consumed_fixture();
    let request = AuthorizedModelRequest::new(&manifest, &binding, &consumption)
        .expect("authorized request");
    assert_eq!(request.request_id, binding.draft.request_id);
    assert_eq!(request.consumption_hash, consumption.consumption_hash);
    assert_eq!(request.items[0].content, manifest.draft.items[0].outbound_content);
}

#[test]
fn response_verifier_rejects_every_authority_substitution() {
    let request = authorized_fixture();
    let valid = raw_response_for(&request);
    for mutate in authority_mutations() {
        let mut response = valid.clone();
        mutate(&mut response);
        assert!(matches!(
            verify_model_response(&request, response, &KnownSchema, &KnownReceipt),
            Err(CloudError::ResponseBindingMismatch | CloudError::InvalidModelOutput | CloudError::ReceiptInvalid)
        ));
    }
}
```

- [ ] **Step 2: Run both tests and verify RED**

Run: `cargo test -p desktop-egress --test consent consumption_tamper_is_detected`

Expected: compilation fails because `DecisionConsumption::verify` does not exist.

Run: `cargo test -p desktop-cloud --test model_verification`

Expected: compilation fails because the authorized request and response APIs do not exist.

- [ ] **Step 3: Add sealed consumption verification**

Reconstruct the existing private `DecisionConsumptionDraft`, validate the fixed schema version, and
recompute `canonical_hash("decision-consumption", 1, &draft)`. Return
`EgressError::DecisionIntegrity` on schema or hash mismatch.

- [ ] **Step 4: Implement authorized request construction**

```rust
impl AuthorizedModelRequest {
    pub fn new(
        manifest: &ContextEgressManifest,
        binding: &ModelInvocationBinding,
        consumption: &DecisionConsumption,
    ) -> Result<Self, CloudError> {
        binding.verify_for(manifest).map_err(CloudError::from_egress)?;
        consumption.verify().map_err(CloudError::from_egress)?;
        if consumption.manifest_hash != manifest.manifest_hash
            || consumption.binding_hash != binding.binding_hash
            || consumption.consent_disclosure_hash != binding.draft.consent_disclosure_hash
            || consumption.policy_hash != binding.draft.policy_hash
            || consumption.installation_id != binding.draft.installation_id
            || consumption.session_authority_hash != binding.draft.session_authority_hash
        {
            return Err(CloudError::ConsentBindingMismatch);
        }
        Self::seal_from(manifest, binding, consumption)
    }
}
```

The sealed request contains no absolute paths, tokens, or provider secrets. It maps only the exact
outbound bytes and reviewed relative metadata from the manifest.

- [ ] **Step 5: Implement response and receipt verification**

`verify_model_response` parses payload JSON, recomputes payload hash, checks the request/schema and
all receipt bindings, invokes `CanonicalOutputValidator` for the exact schema ID/hash, invokes
`ReceiptVerifier` for signature/trust policy, and returns a non-deserializable
`VerifiedModelOutput`. Validation failure returns a stable `CloudError` without raw provider text.

- [ ] **Step 6: Run focused, D2, and quality gates**

Run: `cargo test -p desktop-egress --test consent && cargo test -p desktop-cloud --test model_verification`

Expected: focused tests pass.

Run: `cargo test -p desktop-egress -p desktop-cloud`

Expected: all D2 tests pass.

Run: `cargo clippy -p desktop-egress -p desktop-cloud --all-targets -- -D warnings`

Expected: exits zero without warnings.

Run: `cargo fmt --package desktop-egress --package desktop-cloud -- --check && git diff --check`

Expected: exits zero.

- [ ] **Step 7: Commit Task 2**

```powershell
git add -- crates/desktop-egress/src/consent.rs crates/desktop-egress/tests/consent.rs crates/desktop-cloud/src/lib.rs crates/desktop-cloud/src/model.rs crates/desktop-cloud/tests/model_verification.rs
git commit -m "feat(d2): verify model response bindings"
```

---

## D2-B completion gate

- Token-bearing types are zeroized and redact `Debug` output.
- Sign-out increments the local epoch before broker cleanup and remains locally terminal on cleanup failure.
- Tenant substitution and sign-out races cannot produce a current access grant.
- Authorized requests are derived only from sealed manifest, binding, and consumed decision authority.
- Consumption integrity is independently revalidated at the cloud boundary.
- Payload and receipt substitution tests cover request, schema, manifest, consent, profile, deployment, region, and retention.
- Existing offline behavior remains fail-closed.
- All D2 tests, clippy, rustfmt, forbidden-boundary scan, and non-app workspace tests pass.
