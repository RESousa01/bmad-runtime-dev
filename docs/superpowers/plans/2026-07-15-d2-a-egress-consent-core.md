# D2-A Egress and Consent Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a pure Rust crate that prepares exact outbound context manifests, exposes sanitized review projections, and authorizes one model invocation through an atomic single-use consent decision.

**Architecture:** `desktop-egress` depends only on authority-owned primitives from `desktop-runtime`, serialization/hashing support, and an in-memory synchronization primitive. `manifest` owns sealed values, `preparation` owns deny rules and deterministic redaction, and `consent` owns exact invocation binding plus atomic pending-to-consumed state transitions.

**Tech Stack:** Rust 2021, `desktop-runtime`, Serde, `thiserror`, `parking_lot`, Cargo tests.

## Global Constraints

- Do not modify BMAD contracts, BMAD runtime services, BMAD IPC, Method-library UI, or the dirty Desktop Support API files.
- `desktop-egress` has no network, process, filesystem, Tauri, or database dependency.
- `transient_no_store` is the only retention mode.
- Absolute paths, traversal labels, denied secret-bearing filenames, and mismatched bytes/hashes fail closed.
- Every invocation consumes one exact decision; cancellation, failure, or byte-identical retry never resurrects it.
- Renderer projections contain relative labels and exact redacted outbound bytes but no absolute paths or authority objects.
- All implementation proceeds test-first and each task receives its own focused commit.

## File map

- `Cargo.toml` — adds `crates/desktop-egress` to the workspace.
- `crates/desktop-egress/Cargo.toml` — declares the isolated crate and approved dependencies.
- `crates/desktop-egress/src/lib.rs` — crate policy and public exports only.
- `crates/desktop-egress/src/manifest.rs` — context item, manifest, review projection, limits, and canonical validation.
- `crates/desktop-egress/src/preparation.rs` — input candidates, deny classifier, secret scanner, deterministic redaction, and preparation orchestration.
- `crates/desktop-egress/src/consent.rs` — model invocation binding, pending decision, consumption record, ledger port, in-memory ledger, and consent service.
- `crates/desktop-egress/tests/manifest.rs` — sealed-manifest and renderer-projection behavior.
- `crates/desktop-egress/tests/preparation.rs` — exclusion, redaction, and budget behavior.
- `crates/desktop-egress/tests/consent.rs` — exact binding, expiry, drift, and replay behavior.

---

### Task 1: Sealed context manifest and sanitized review projection

**Files:**
- Create: `crates/desktop-egress/Cargo.toml`
- Create: `crates/desktop-egress/src/lib.rs`
- Create: `crates/desktop-egress/src/manifest.rs`
- Create: `crates/desktop-egress/tests/manifest.rs`
- Modify: `Cargo.toml`

**Interfaces:**
- Consumes: `desktop_runtime::{canonical_hash, sha256_bytes, ContractId, Sha256Digest, UnixMillis}`.
- Produces: `ContextClassification`, `RetentionMode`, `RedactionRecord`, `SecretFinding`, `PreparedContextItem`, `ContextEgressManifestDraft`, `ContextEgressManifest`, `ContextReviewProjection`, `EgressLimits`, and `EgressError`.

- [ ] **Step 1: Add the crate scaffold and write failing manifest tests**

Create the crate manifest with `desktop-runtime`, `parking_lot`, `serde`, and `thiserror` workspace dependencies; add the crate to workspace members. Add tests which call the wished-for API:

```rust
#[test]
fn manifest_seals_exact_outbound_bytes_and_projects_them_for_review() {
    let item = fixture_item("src/lib.rs", "fn main() {}\n");
    let manifest = fixture_draft(vec![item]).seal().expect("valid manifest");
    manifest.verify().expect("sealed manifest");
    let review = manifest.review_projection();
    assert_eq!(review.items[0].relative_label.as_str(), "src/lib.rs");
    assert_eq!(review.items[0].outbound_content, "fn main() {}\n");
    assert_eq!(review.manifest_hash, manifest.manifest_hash);
}

#[test]
fn manifest_rejects_an_outbound_hash_that_does_not_match_the_bytes() {
    let mut item = fixture_item("src/lib.rs", "safe\n");
    item.outbound_content_hash = sha256_bytes(b"different");
    let error = fixture_draft(vec![item]).seal().expect_err("hash drift");
    assert_eq!(error, EgressError::ContextDrift);
}

#[test]
fn manifest_hash_changes_when_item_order_changes() {
    let first = fixture_item("src/a.rs", "a");
    let second = fixture_item("src/b.rs", "b");
    let left = fixture_draft(vec![first.clone(), second.clone()]).seal().unwrap();
    let right = fixture_draft(vec![second, first]).seal().unwrap();
    assert_ne!(left.manifest_hash, right.manifest_hash);
}
```

- [ ] **Step 2: Run the manifest test and verify RED**

Run: `cargo test -p desktop-egress --test manifest`

Expected: compilation fails because the manifest types and methods do not yet exist.

- [ ] **Step 3: Implement the minimal sealed manifest**

Implement:

```rust
impl ContextEgressManifestDraft {
    pub fn seal(self) -> Result<ContextEgressManifest, EgressError> {
        validate_manifest(&self)?;
        let manifest_hash = canonical_hash("context-egress-manifest", 1, &self)?;
        Ok(ContextEgressManifest { draft: self, manifest_hash })
    }
}

impl ContextEgressManifest {
    pub fn verify(&self) -> Result<(), EgressError> {
        validate_manifest(&self.draft)?;
        let actual = canonical_hash("context-egress-manifest", 1, &self.draft)?;
        if actual != self.manifest_hash {
            return Err(EgressError::ManifestIntegrity);
        }
        Ok(())
    }

    #[must_use]
    pub fn review_projection(&self) -> ContextReviewProjection {
        ContextReviewProjection::from(self)
    }
}
```

`validate_manifest` must check the fixed schema version, non-empty ordered items, unique item IDs,
creation before expiry, exact byte counts and hashes, total counts, the configured hard limits,
and `RetentionMode::TransientNoStore`. `PreparedContextItem.relative_label` uses
`RelativeWorkspacePath`, so absolute, colon-bearing, backslash, and traversal labels cannot be
constructed.

- [ ] **Step 4: Run focused and crate tests and verify GREEN**

Run: `cargo test -p desktop-egress --test manifest`

Expected: all manifest tests pass.

Run: `cargo clippy -p desktop-egress --all-targets -- -D warnings`

Expected: exits zero without warnings.

- [ ] **Step 5: Commit Task 1**

```powershell
git add -- Cargo.toml crates/desktop-egress/Cargo.toml crates/desktop-egress/src/lib.rs crates/desktop-egress/src/manifest.rs crates/desktop-egress/tests/manifest.rs
git commit -m "feat(d2): seal context egress manifests"
```

---

### Task 2: Deny classification, deterministic secret redaction, and preparation

**Files:**
- Create: `crates/desktop-egress/src/preparation.rs`
- Create: `crates/desktop-egress/tests/preparation.rs`
- Modify: `crates/desktop-egress/src/lib.rs`
- Modify: `crates/desktop-egress/src/manifest.rs`

**Interfaces:**
- Consumes: Task 1 manifest types and `sha256_bytes`.
- Produces: `ContextCandidate`, `PrepareContextInput`, `ContextPreparer`, `SecretScanner`, `PatternSecretScanner`, and `ContextExclusion`.

- [ ] **Step 1: Write failing preparation tests**

```rust
#[test]
fn preparation_rejects_dotenv_before_scanning() {
    let dotenv_content = ["API", "_KEY=test-only"].concat();
    let input = fixture_input(vec![candidate(".env", &dotenv_content)]);
    let error = ContextPreparer::new(PatternSecretScanner::default())
        .prepare(input)
        .expect_err("dotenv is denied");
    assert_eq!(error, EgressError::DeniedContextLabel);
}

#[test]
fn preparation_redacts_private_key_material_and_records_a_finding() {
    let private_key_marker = ["BEGIN ", "PRIVATE KEY"].concat();
    let source = format!("prefix -----{private_key_marker}----- value");
    let manifest = ContextPreparer::new(PatternSecretScanner::default())
        .prepare(fixture_input(vec![candidate("notes.txt", &source)]))
        .expect("redacted manifest");
    let item = &manifest.draft.items[0];
    assert!(!item.outbound_content.contains(&private_key_marker));
    assert_eq!(item.redactions[0].kind, "private_key");
    assert_eq!(manifest.draft.secret_findings[0].kind, "private_key");
}

#[test]
fn preparation_enforces_the_outbound_byte_budget_after_redaction() {
    let mut input = fixture_input(vec![candidate("notes.txt", "12345")]);
    input.limits.maximum_context_bytes = 4;
    assert_eq!(
        ContextPreparer::new(PatternSecretScanner::default()).prepare(input),
        Err(EgressError::ContextBudgetExceeded)
    );
}
```

- [ ] **Step 2: Run the preparation test and verify RED**

Run: `cargo test -p desktop-egress --test preparation`

Expected: compilation fails because preparation interfaces do not exist.

- [ ] **Step 3: Implement deterministic preparation**

Define the scanner port and result:

```rust
pub trait SecretScanner: Send + Sync {
    fn scan(&self, content: &str) -> ScanResult;
}

pub struct ScanResult {
    pub outbound_content: String,
    pub findings: Vec<SecretFinding>,
    pub redactions: Vec<RedactionRecord>,
}
```

`PatternSecretScanner` replaces fixed markers for PEM private keys, `ghp_`, `sk-`, and common
credential assignments with `[REDACTED:<kind>]`, recording only kind/count metadata. It never
retains the matched secret. `ContextPreparer::prepare` rejects `.env`, `.env.*`, `.npmrc`,
`id_rsa`, `id_ed25519`, and `credentials` case-insensitively before scanning; computes original and
outbound hashes/counts; seals the manifest; and enforces item/byte/token limits after redaction.

- [ ] **Step 4: Run focused, crate, and clippy tests and verify GREEN**

Run: `cargo test -p desktop-egress --test preparation`

Expected: all preparation tests pass.

Run: `cargo test -p desktop-egress`

Expected: all crate tests pass.

Run: `cargo clippy -p desktop-egress --all-targets -- -D warnings`

Expected: exits zero without warnings.

- [ ] **Step 5: Commit Task 2**

```powershell
git add -- crates/desktop-egress/src/lib.rs crates/desktop-egress/src/manifest.rs crates/desktop-egress/src/preparation.rs crates/desktop-egress/tests/preparation.rs
git commit -m "feat(d2): prepare reviewable outbound context"
```

---

### Task 3: Exact invocation binding and atomic single-use consent

**Files:**
- Create: `crates/desktop-egress/src/consent.rs`
- Create: `crates/desktop-egress/tests/consent.rs`
- Modify: `crates/desktop-egress/src/lib.rs`

**Interfaces:**
- Consumes: sealed `ContextEgressManifest`, `ContractId`, `Sha256Digest`, `UnixMillis`, and canonical hashing.
- Produces: `ModelInvocationBindingDraft`, `ModelInvocationBinding`, `PendingContextDecision`, `DecisionConsumption`, `DecisionLedger`, `MemoryDecisionLedger`, and `ConsentService`.

- [ ] **Step 1: Write failing consent tests**

```rust
#[test]
fn one_decision_authorizes_one_exact_invocation() {
    let manifest = fixture_manifest();
    let binding = fixture_binding(&manifest).seal().unwrap();
    let ledger = MemoryDecisionLedger::default();
    let service = ConsentService::new(&ledger);
    let decision = service.approve(fixture_approval(&manifest, &binding)).unwrap();

    let consumed = service.consume(fixture_consumption(&decision, &binding)).unwrap();
    assert_eq!(consumed.decision_id, decision.decision_id);

    let replay = service.consume(fixture_consumption(&decision, &binding));
    assert_eq!(replay, Err(EgressError::DecisionAlreadyConsumed));
}

#[test]
fn byte_identical_retry_still_requires_a_new_decision() {
    let manifest = fixture_manifest();
    let binding = fixture_binding(&manifest).seal().unwrap();
    let ledger = MemoryDecisionLedger::default();
    let service = ConsentService::new(&ledger);
    let decision = service.approve(fixture_approval(&manifest, &binding)).unwrap();
    service.consume(fixture_consumption(&decision, &binding)).unwrap();

    let mut retry = fixture_consumption(&decision, &binding);
    retry.invocation_id = ContractId::new("invocation_retry").unwrap();
    assert_eq!(service.consume(retry), Err(EgressError::DecisionAlreadyConsumed));
}

#[test]
fn drifted_region_is_rejected_without_consuming_the_decision() {
    let manifest = fixture_manifest();
    let binding = fixture_binding(&manifest).seal().unwrap();
    let ledger = MemoryDecisionLedger::default();
    let service = ConsentService::new(&ledger);
    let decision = service.approve(fixture_approval(&manifest, &binding)).unwrap();

    let mut drifted = fixture_binding(&manifest);
    drifted.region = "westus".to_owned();
    let drifted = drifted.seal().unwrap();
    assert_eq!(
        service.consume(fixture_consumption(&decision, &drifted)),
        Err(EgressError::DecisionBindingMismatch)
    );
    assert!(service.consume(fixture_consumption(&decision, &binding)).is_ok());
}

#[test]
fn expired_decision_is_rejected_and_never_resurrected() {
    let manifest = fixture_manifest();
    let binding = fixture_binding(&manifest).seal().unwrap();
    let ledger = MemoryDecisionLedger::default();
    let service = ConsentService::new(&ledger);
    let decision = service.approve(fixture_approval(&manifest, &binding)).unwrap();
    let mut input = fixture_consumption(&decision, &binding);
    input.consumed_at = UnixMillis(decision.expires_at.0 + 1);
    assert_eq!(service.consume(input), Err(EgressError::DecisionExpired));
}
```

- [ ] **Step 2: Run the consent test and verify RED**

Run: `cargo test -p desktop-egress --test consent`

Expected: compilation fails because consent interfaces do not exist.

- [ ] **Step 3: Implement exact binding and atomic consumption**

Implement sealing and ledger operations:

```rust
impl ModelInvocationBindingDraft {
    pub fn seal(self) -> Result<ModelInvocationBinding, EgressError> {
        validate_binding(&self)?;
        let binding_hash = canonical_hash("model-invocation-binding", 1, &self)?;
        Ok(ModelInvocationBinding { draft: self, binding_hash })
    }
}

pub trait DecisionLedger: Send + Sync {
    fn insert_pending(&self, decision: PendingContextDecision) -> Result<(), EgressError>;
    fn consume_if_pending(
        &self,
        input: &ConsumeDecisionInput,
    ) -> Result<DecisionConsumption, EgressError>;
}
```

`MemoryDecisionLedger` uses `parking_lot::Mutex<HashMap<ContractId, DecisionState>>`. It validates
all manifest, binding, consent, policy, authority, session, and expiry fields before changing state.
A mismatch leaves the decision pending. Expiry changes it to a terminal expired state. Successful
consumption changes it to a terminal consumed state inside the same lock and stores only hashes and
identifiers in `DecisionConsumption`.

- [ ] **Step 4: Run focused, crate, and workspace-adjacent tests and verify GREEN**

Run: `cargo test -p desktop-egress --test consent`

Expected: all consent tests pass.

Run: `cargo test -p desktop-egress`

Expected: all D2-A tests pass.

Run: `cargo clippy -p desktop-egress --all-targets -- -D warnings`

Expected: exits zero without warnings.

Run: `cargo test -p desktop-airlock -p desktop-cloud`

Expected: existing adjacent boundary tests pass.

- [ ] **Step 5: Run boundary and formatting checks**

Run: `cargo fmt --all -- --check`

Expected: exits zero.

Run: `rg -n "reqwest|hyper|tauri|rusqlite|std::fs|std::process|TcpStream" crates/desktop-egress`

Expected: no matches outside this plan text; the crate has no forbidden adapter dependency.

Run: `git diff --check`

Expected: exits zero.

- [ ] **Step 6: Commit Task 3**

```powershell
git add -- crates/desktop-egress/src/lib.rs crates/desktop-egress/src/consent.rs crates/desktop-egress/tests/consent.rs Cargo.lock
git commit -m "feat(d2): consume context consent exactly once"
```

---

## D2-A completion gate

- Every new public function has a test that was observed failing before implementation.
- Manifest integrity binds exact outbound bytes and ordered item metadata.
- Review projections contain exact outbound redacted bytes and no absolute paths.
- Denied secret-bearing labels fail before scanning.
- Secret findings store kind/count metadata, not matched secret text.
- All exact invocation fields participate in the binding hash.
- Drift does not consume a pending decision.
- Expiry and successful consumption are terminal.
- Byte-identical replay fails with `DecisionAlreadyConsumed`.
- D2-A, adjacent crate, clippy, format, boundary, and diff checks pass.
