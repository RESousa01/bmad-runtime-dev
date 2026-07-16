# D2-C WAM and HTTPS Adapters Implementation Plan

## Completion checkpoint — 2026-07-15

D2-C is implemented and independently reviewed. The fixed packaged-helper launch contract, actual-current-user named-pipe ACL, child-PID correlation, bounded protocol, zeroizing broker decode, fixed-origin HTTPS dispatch, entitlement-before-egress enforcement, and offline-by-default composition all pass the focused default/all-feature suites, strict Clippy, formatting, and the Rust workspace gate excluding the separately blocked Tauri packaging crate. Authenticode signing and desktop command/UI wiring remain later packaging and D2-D integration work. The task checklist below is retained as the historical implementation script.

**Goal:** Connect the verified D2 identity and model boundaries to a bounded Windows authentication helper protocol and a fixed-origin HTTPS support API transport without exposing secrets or allowing request-controlled destinations.

**Architecture:** `desktop-cloud::broker_protocol` owns the strict framed JSON contract and response validation independently of Windows I/O. A Windows-only adapter creates a current-user named pipe before launching the fixed helper and implements `IdentityBroker`. `desktop-cloud::transport` owns one constructor-validated support API origin, bounded request/response serialization, bearer-token borrowing, timeout handling, and untrusted response decoding. Model response trust remains exclusively in `verify_model_response`.

**Tech Stack:** Rust 1.97, Tokio named pipes/process/time, Reqwest with rustls, Serde, Rand, Zeroize, existing desktop-runtime contract primitives.

## Ownership and invariants

- Modify only D2-owned `desktop-cloud`, workspace dependency metadata, and D2 docs/tests.
- No access token implements `Clone`, `Serialize`, or `Deserialize`; raw broker/HTTP errors never cross the public boundary.
- The helper receives only a random pipe name as a process argument.
- Broker frames are big-endian length-prefixed and capped at 64 KiB.
- Unknown/duplicate JSON fields, correlation drift, tenant/account substitution, missing token fields, and expired tokens fail closed.
- The support API origin is immutable after construction, HTTPS-only, has no user info/query/fragment, and may not be overridden by request/context/model data.
- Transport cancellation or failure does not recreate consent authority.

## Task 1: Strict helper protocol and Windows adapter

**Files:**

- Create `crates/desktop-cloud/src/broker_protocol.rs`
- Create `crates/desktop-cloud/src/windows_broker.rs`
- Create `crates/desktop-cloud/tests/broker_protocol.rs`
- Modify `crates/desktop-cloud/src/lib.rs`
- Modify `crates/desktop-cloud/Cargo.toml`
- Modify `Cargo.toml`
- Modify `Cargo.lock`

**Red tests:** prove frame limits, protocol/correlation/tenant/account/expiry validation, stable error mapping, token redaction, and deterministic acquire/sign-out request shapes.

**Implementation:** add private wire types, strict duplicate-key rejection, request builder and response validator. Add the Windows named-pipe/process adapter with current-user-only pipe creation, bounded connect/read/write/process timeouts, inherited-output suppression, and stable `CloudError` mapping. Non-Windows builds expose only the protocol core.

**Proof:**

```powershell
cargo test -p desktop-cloud --test broker_protocol
cargo clippy -p desktop-cloud --all-targets -- -D warnings
```

**Checkpoint:** `feat(d2): integrate windows auth broker protocol`

## Task 2: Fixed-origin HTTPS model transport

**Files:**

- Create `crates/desktop-cloud/src/transport.rs`
- Create `crates/desktop-cloud/tests/support_transport.rs`
- Modify `crates/desktop-cloud/src/lib.rs`
- Modify `crates/desktop-cloud/Cargo.toml`
- Modify `Cargo.toml`
- Modify `Cargo.lock`

**Red tests:** prove HTTPS-only origin validation, fixed path joining, request-size limits, no redirect following, bearer-header redaction boundaries, status/content-length/response-size enforcement, malformed JSON handling, and stable timeout/transport errors.

**Implementation:** add `SupportApiOrigin`, `SupportApiTransport`, and an injected minimal HTTP execution port for deterministic tests. Add the Reqwest production executor configured with rustls, redirects disabled, bounded connect/operation timeouts, and no proxy discovery. The public send path accepts only `AuthorizedModelRequest` plus `CloudAccess`; response verification remains a separate mandatory call.

**Proof:**

```powershell
cargo test -p desktop-cloud --test support_transport
cargo test --workspace --exclude desktop-app
cargo clippy -p desktop-egress -p desktop-cloud --all-targets -- -D warnings
cargo fmt --package desktop-egress --package desktop-cloud -- --check
git diff --check
```

**Checkpoint:** `feat(d2): add fixed support api transport`

## Deferred release gates

- Authenticode publisher and packaged-helper identity verification requires the signed installer/package lane.
- Concrete tenant/client/scope/origin values remain signed host configuration inputs.
- Full `cargo test --workspace` remains separately blocked until the desktop app icon asset is committed by its owner.
