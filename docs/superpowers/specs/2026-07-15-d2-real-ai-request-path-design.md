# D2 Real AI Request Path Design

**Status:** Approved direction, design checkpoint
**Date:** 2026-07-15
**Scope:** Non-BMAD desktop identity, context-egress consent, and transient model access

## 1. Outcome

The initial prototype will support one honest, bounded AI request path:

1. the Windows desktop authenticates the user through an organization-owned Entra public-client flow;
2. the Rust host prepares the exact context that would leave the device;
3. the user reviews the prepared, redacted bytes and their destination/purpose;
4. the host issues and atomically consumes a short-lived, single-use consent decision;
5. the desktop sends the request to the tenant-owned Desktop Support API with an in-memory access token;
6. the host verifies the typed result and metadata-only receipt before exposing the result to the renderer.

This path must remain useful in deterministic development mode and fail closed when authentication,
entitlement, consent, transport, or response validation is unavailable.

## 2. Repository and ownership boundaries

The parallel BMAD implementation owns BMAD contracts, Method/Builder services, BMAD desktop IPC,
and Method-library UI. D2 work must not modify those files while that lane is active.

The D2 implementation owns:

- a new pure Rust `desktop-egress` crate;
- non-BMAD additions to `desktop-cloud`;
- the Windows authentication broker integration boundary;
- D2-only tests and documentation;
- later, narrowly scoped identity/consent/model-access IPC and UI composition after the shared
  desktop-app and renderer files are released by the BMAD lane.

The currently modified Desktop Support API files are treated as an external in-progress contract.
D2 client work may read and test against their current public request/response shape but must not
overwrite or reformat those changes. Service changes require a separate clean checkpoint.

## 3. Locked trust decisions

- The renderer never receives an access token, refresh token, provider credential, entitlement
  signature key, or raw broker error.
- The desktop contains no Azure OpenAI or Foundry provider key.
- Provider access occurs only behind the tenant-owned Desktop Support API.
- The endpoint, tenant, client identifier, scopes, region allowlist, and provider profile are host
  configuration or signed-policy inputs. Workspace, package, model, and renderer content cannot set
  them.
- Every model invocation requires a fresh consent decision. A decision authorizes exactly one
  invocation, including when a later request is byte-identical.
- Context, package text, and model output are untrusted data. They cannot author identity, policy,
  entitlement, consent, capabilities, tool authority, approvals, or local effects.
- `transient_no_store` is the only prototype retention mode.
- Sign-out invalidates the local cloud session and blocks new calls without deleting local source,
  evidence, or prior metadata-only receipts.
- Development fakes are explicit build/test composition. Production composition cannot silently
  fall back to them.

## 4. Architecture

```text
Renderer review UI
       |
       | bounded commands and sanitized projections
       v
Desktop host composition
       |
       +--> desktop-egress
       |      prepare -> review projection -> approve -> consume once
       |
       +--> desktop-cloud
       |      identity session -> entitlement -> HTTP transport -> response verification
       |
       +--> signed WAM helper
       |      broker/cache operations only; token returned in memory
       |
       `--> tenant Desktop Support API
              validates Entra token and device -> transient provider call -> signed receipt
```

`desktop-egress` has no network, process, filesystem, Tauri, or database implementation. It owns
pure preparation and authorization rules. Storage and scanning are injected ports.

`desktop-cloud` owns identity and remote-call orchestration, but it cannot create or approve an
egress decision. It accepts only a consumed invocation authorization produced by `desktop-egress`.

## 5. Context preparation and review

### 5.1 Prepared context

The host constructs a `PreparedContext` from an explicit set of already-authorized workspace
reads. Each item contains:

- an opaque client item identifier;
- a relative or privacy-preserving label, never an absolute path;
- semantic role and optional language;
- original content hash;
- exact outbound content hash;
- original and outbound byte counts;
- bounded token estimate;
- classification;
- ordered redaction records;
- exact outbound content.

Preparation rejects absolute labels, traversal, empty or duplicate identifiers, unsupported
classifications, oversized context, and content/length/hash mismatches.

### 5.2 Exclusions and scanning

The first prototype uses two explicit controls:

1. a deny classifier for `.env` files, credential stores, private keys, token caches, app-local
   authority state, and known secret-bearing filenames;
2. an injected secret scanner that returns findings and deterministic redaction transformations.

The scanner is a risk-reduction control, not proof that context is secret-free. The review
projection states this limitation.

### 5.3 Manifest

`ContextEgressManifest` seals:

- manifest and contract version;
- tenant/project/run references as opaque hashes or identifiers;
- purpose and model role;
- canonical output schema identifier and hash;
- provider/model/deployment/profile hashes;
- region and retention mode;
- creation and expiry times;
- ordered prepared-item metadata;
- exclusions, secret findings, redactions, and transformations;
- total item, byte, and token counts;
- policy decision and policy hash;
- a canonical manifest hash.

The hash binds outbound bytes, not only display metadata. Reordering items changes the hash.

### 5.4 Renderer projection

The renderer receives a bounded `ContextReviewProjection` containing the exact redacted outbound
text, relative labels, classifications, findings, counts, destination/profile disclosure,
retention, purpose, expiry, and manifest hash. It does not receive absolute paths or authority
objects.

## 6. Consent and single-use consumption

`ConsentService` has three state transitions:

```text
prepared(manifest)
    -> approved(decision, binding)
    -> consumed(consumption, invocation)
```

An approved `ContextDecision` binds:

- a host-generated random decision identifier;
- manifest hash;
- exact `ModelInvocationBinding` hash;
- consent disclosure/version hash;
- policy hash;
- issuance and expiry;
- installation/session authority;
- state `pending`.

`ModelInvocationBinding` includes request identifier, purpose, model role, canonical output schema,
provider profile, deployment, region, retention, manifest hash, and consent hash.

`DecisionLedger::consume_if_pending` is an atomic compare-and-set operation. Successful consumption
records the decision identifier, invocation identifier, manifest/binding/consent hashes, and time,
then permanently changes the decision to `consumed`. Expired, unknown, drifted, cancelled, and
already consumed decisions fail closed.

The prototype in-memory ledger deliberately loses pending decisions on restart. That behavior is
safe because no decision can be recovered or replayed after a restart. A durable ledger and
metadata-only audit adapter are follow-up production-hardening work.

## 7. Identity boundary

### 7.1 Interface

`IdentityBroker` exposes only:

- current sanitized authentication status;
- acquire token for the fixed support-API scope;
- sign out;
- account and tenant change notification using opaque identifiers.

The returned access token is held in a zero-retention in-memory session object and passed directly
to the HTTP transport. Debug formatting and serialization of token-bearing types are prohibited.

### 7.2 WAM helper

The existing .NET helper remains the broker/cache owner. The Rust adapter:

- generates a cryptographically random current-user pipe name;
- creates the pipe before launching the helper;
- launches the helper with only the pipe name and fixed protocol version;
- validates framed messages, request correlation, tenant/account binding, response size, and expiry;
- enforces a bounded handshake and operation timeout;
- clears token memory after use;
- maps broker failures to stable, non-sensitive error codes.

Release composition additionally requires Authenticode publisher verification and packaged helper
identity verification. Development composition may use an explicitly enabled deterministic
identity adapter; it may not pretend to be production WAM.

System-browser PKCE fallback remains disabled unless signed tenant policy explicitly permits it.

## 8. Cloud client and transport

`desktop-cloud` is split into four responsibilities:

- `CloudSession`: connectivity, sanitized auth status, account/tenant epoch, and sign-out
  invalidation;
- `EntitlementVerifier`: validates audience-bound lease signature, time window, registration,
  tenant policy, minimum version, and feature availability;
- `SupportApiTransport`: sends bounded JSON over HTTPS to a fixed configured origin;
- `ModelResponseVerifier`: validates request, schema, manifest, consent, profile, region, retention,
  payload, and receipt bindings.

The request is created only after decision consumption. Cancellation or transport failure never
resurrects the consumed decision; retry requires a newly reviewed decision and invocation.

The response is accepted only when:

- its request identifier matches;
- output schema identifier matches the requested canonical schema;
- payload JSON parses and passes the registered schema validator;
- payload hash matches the received payload;
- receipt request/result/manifest/consent/profile hashes match local values;
- receipt region and retention match the approved binding;
- receipt status is successful and its signature/verifier policy is valid.

Only the typed payload and a metadata-only verified receipt projection cross to the renderer.

## 9. Error model

Errors have stable codes and optional safe recovery guidance. Initial codes include:

- `identity_unavailable`
- `authentication_required`
- `reauthentication_required`
- `tenant_mismatch`
- `entitlement_unavailable`
- `feature_disabled`
- `context_rejected`
- `context_drift`
- `consent_required`
- `consent_expired`
- `consent_binding_mismatch`
- `consent_already_consumed`
- `support_plane_offline`
- `transport_failed`
- `response_binding_mismatch`
- `invalid_model_output`
- `receipt_invalid`

Raw MSAL, HTTP, JWT, provider, or validation errors are not returned to the renderer or written to
ordinary logs.

## 10. Prototype UI and IPC

After the shared composition files are available, the desktop receives narrow commands for:

- authentication status, sign-in, and sign-out;
- prepare context review;
- approve or cancel one review;
- submit exactly one approved invocation;
- inspect a sanitized model result and verified receipt summary.

The UI shows sign-in state, destination/region/retention, exact outbound context, redactions,
context size, purpose, and decision expiry. Submit remains disabled until the currently displayed
manifest is approved. Any selected-context or model-binding change discards the approval and
requires a new review.

## 11. Verification

### Pure policy tests

- canonical hashing and deterministic ordering;
- absolute/traversal label rejection;
- excluded filename and secret-fixture behavior;
- outbound byte/hash/count mismatch rejection;
- approval of exact binding;
- identical replay rejection;
- drift in bytes, order, purpose, model, deployment, profile, region, schema, retention, consent,
  policy, session, or expiry rejection;
- cancellation and restart fail closed.

### Identity and transport tests

- framed protocol limits and request correlation;
- helper timeout, malformed response, wrong tenant/account, and expired token;
- token types cannot serialize or expose debug content;
- sign-out/account change invalidates active session epoch;
- offline behavior never falls back to a fake;
- support origin cannot be supplied by renderer/context/model data;
- cancellation consumes no additional authority and never reuses consent.

### Response tests

- request/schema/payload/manifest/consent/profile/region/retention substitutions;
- malformed or oversized JSON;
- forged, missing, expired, duplicate, or mismatched receipt;
- model text attempting to set authority fields remains inert data;
- successful deterministic end-to-end request produces one consumption and one verified receipt.

### Boundary checks

Static boundary checks reject provider SDKs/keys in the desktop, token fields in renderer contracts,
network dependencies in `desktop-egress`, and imports from D2 crates into the BMAD foundation layer.

## 12. Delivery checkpoints

1. **D2-A — contracts and pure egress core:** crate, domain types, canonical hashing, preparation,
   review projection, single-use ledger, and exhaustive pure tests.
2. **D2-B — cloud session and verification:** identity/session abstractions, entitlement and
   response verification, deterministic/offline adapters, and integration tests.
3. **D2-C — WAM and HTTPS adapters:** Rust helper protocol client, fixed-origin transport, timeout
   and cancellation behavior, and Windows-focused tests.
4. **D2-D — desktop composition:** IPC, UI review flow, sanitized status/result projections, and a
   deterministic end-to-end desktop smoke test after BMAD releases shared files.
5. **D2-E — support-plane reconciliation:** align canonical contracts with the existing service,
   verify signed consent/receipt handling, and run cross-language contract tests without replacing
   unrelated in-progress service changes.

Each checkpoint is independently tested and committed with only its owned files.

## 13. Non-goals and release blockers

The initial prototype does not claim:

- production Azure deployment or managed-identity provider configuration;
- production certificate/HSM signing;
- signed installer or helper publisher verification completion;
- multi-tenant identity;
- provider-side persistent state or background processing;
- durable consent/audit persistence across restarts;
- governed local edit/apply/undo authority;
- remote jobs, synchronization, or collaboration.

Those remain explicit release blockers or later milestones. The prototype may demonstrate the
complete request path with deterministic development adapters, but production mode stays disabled
until concrete tenant/client/API identifiers, signed policy keys, helper packaging/signing, and the
support-plane deployment are configured and verified.
