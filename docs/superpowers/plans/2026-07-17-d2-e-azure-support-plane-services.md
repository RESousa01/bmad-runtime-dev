# D2-E Azure Support Plane Services Implementation Plan

**Status:** Proposed implementation plan  
**Date:** 2026-07-17  
**Milestone:** D2-E - production support-plane reconciliation  
**Primary authority reference:** `bmad-runtime-lib/98 - Azure Support Plane for Windows Desktop.md`

**Goal:** Replace the fail-closed D2 development seams with production Azure adapters for durable
device authority, signed policy and entitlement issuance, installation-key consent verification,
managed-identity model brokerage, signed receipts, privacy-safe observability, and production
desktop composition without moving local workspace authority into Azure.

**Architecture:** Keep `desktop-app` as the sole local composition root and the Desktop Support API
as a narrow, single-tenant support plane. Reconcile the richer canonical D2 contracts before
implementing infrastructure adapters. The API uses Microsoft Entra authentication and separate
user-assigned managed identities to access Azure SQL, App Configuration, Key Vault, and the exact
Azure OpenAI deployment. Azure stores registration, replay, receipt, policy, and usage metadata
only; request context and model output remain transient. Every production dependency fails closed,
and no development adapter can be selected in a production environment.

**Tech stack:** Rust 1.97, .NET 10 ASP.NET Core, Microsoft Entra ID, Azure Container Apps, Azure SQL,
Azure App Configuration, Azure Key Vault ES256 keys, Azure OpenAI, Azure Monitor/Application
Insights with OpenTelemetry, Bicep, existing JSON Schema 2020-12 contracts, xUnit v3, Cargo tests,
and the repository's existing Node contract qualification.

## Authority and intent

- **Owning authority:** `desktop-app` and the Rust domain remain the only owners of local context
  selection, consent gestures, Method state, proposals, approvals, specs, execution, checkpoints,
  rollback, and evidence. The Desktop Support API owns only cloud registration, entitlement,
  admission, replay prevention, provider brokerage, receipt issuance, and support-plane audit
  metadata.
- **User-visible outcome:** an approved employee can sign in, register one installation, receive a
  verifiable policy and entitlement lease, review and sign one exact context disclosure, invoke the
  configured model once, verify the signed response receipt locally, and continue local work when
  the support plane is unavailable according to the signed lease policy.
- **Contracts read:** `model-context-consent`, `model-access-receipt`, entitlement lease, desktop
  policy, D2 egress manifest and invocation binding, host dispatch envelopes, and the D2-D completed
  Help projection.
- **Non-goals:** public or multi-tenant onboarding; Azure ownership of local project/run/effect
  state; durable prompt, response, source, path, or transient label storage; arbitrary provider
  endpoints; provider-hosted tools; background model work; sync, collaboration, remote jobs,
  diagnostics upload, package publication, or release publication in this milestone.
- **Stop conditions:** any contract permits an absolute/local path or local authority field; any
  production route can reach a development signer/model/store; raw context or output reaches SQL,
  logs, traces, metrics, queues, or exception details; a caller can select endpoint/deployment,
  region, retention, credentials, or tools; consent can be replayed; a receipt can be committed
  after registration revocation; or the desktop accepts unsigned/untrusted policy, lease, or model
  output.

## Current-state inventory and gaps

The repository already contains substantial D2 foundations. Implementation must extend these
surfaces rather than create a parallel cloud stack.

| Existing surface | Current state | D2-E gap |
|---|---|---|
| `services/desktop-support-api` | Strict single-tenant JWT validation, route scopes, bounded JSON, registration, revocation, in-memory idempotency, model admission, and development-only seams | No production dependency composition or Azure SDK adapters |
| `services/desktop-support-api.Tests` | Security, cancellation, revocation, idempotency, consent, and cross-language fixture tests | No SQL concurrency, Key Vault, managed-model, privacy export, or deployed smoke coverage |
| `packages/contracts` | Canonical consent and receipt schemas plus generated Rust/TypeScript bindings | Rust `desktop-cloud` transport types are older and do not serialize the API's canonical request/receipt envelope |
| `crates/desktop-cloud` | Session epochs, entitlement checks, fixed-origin HTTPS, WAM helper protocol, and replay-safe response verification | No installation signature, production policy/lease verifier, canonical support API adapter, or production receipt proof verifier |
| `crates/desktop-app::bmad_model` | Offline default and explicit deterministic composition | No production mode, bootstrap/registration lifecycle, cloud policy, entitlement refresh, or async HTTPS coordinator |
| `infra/desktop-support` | Container Apps, ACR, Key Vault, App Configuration, Azure SQL, Azure OpenAI, private endpoints, and Monitor scaffold | One runtime identity is over-broad; no schema deployment, alerts, budget, environment modules, or production adapter settings |

Two contract issues must be resolved before Azure implementation:

1. `AuthorizedModelRequest` is an internal local-authority object and must not be sent as if it were
   the canonical `ModelAccessRequest`. Add an explicit one-way transport projection that combines
   the consumed local request with registration, lease, tenant policy, and installation signature.
2. Device registration currently sends only `installationPublicKeyHash`. The service cannot verify
   an installation signature from a hash. Registration must carry a bounded canonical P-256 public
   key representation; the service recomputes and stores its hash. The public contract continues to
   expose only the hash.

## Target production component map

| Module | Production implementation | Azure dependency | Durable data |
|---|---|---|---|
| API authentication/admission | Existing ASP.NET Core host plus hardened options | Entra ID | None |
| Device and receipt authority | SQL-backed registry with optimistic epochs | Azure SQL | Registration/revocation and receipt metadata |
| Idempotency/replay | Transactional SQL stores | Azure SQL | Fingerprints, state, safe completion markers |
| Consent verification | Registered installation-key verifier | Azure SQL public key record | No context body |
| Policy and leases | App Configuration policy loader plus Key Vault signer | App Configuration, Key Vault | Signed hashes and issuance audit |
| Model access | Fixed-profile managed-identity broker | Azure OpenAI | No prompt/output |
| Receipt signing | Separate canonical receipt builder and Key Vault signer | Key Vault | Receipt/hash/usage metadata |
| Observability | Allowlisted OpenTelemetry instrumentation | Application Insights/Log Analytics | Low-cardinality safe metadata only |
| Desktop production client | Canonical transport adapter and proof verifiers | Support API only | Existing encrypted local authority |

## Non-negotiable implementation invariants

1. The desktop requests only the support API audience; it never obtains Azure SQL, Key Vault,
   App Configuration, Azure OpenAI, or Azure management tokens.
2. Production Azure clients use explicit user-assigned managed identity client IDs. No connection
   password, provider key, ACR credential, signing material, or client secret is accepted.
3. Production composition is selected only after all required options validate and all production
   adapters are registered. Missing dependencies keep the service unhealthy and model access
   unavailable; they never select a development fallback.
4. SQL queries are tenant/subject partitioned and parameterized. Final lease/receipt commits verify
   the registration row's active state and concurrency epoch in the same transaction.
5. Consent consumption has a unique authority key over subject partition, registration, and
   consumption hash. A failed provider call does not make consent reusable.
6. Idempotency keys are subject partitioned and bound to an exact request fingerprint. Reusing a key
   with another fingerprint is a conflict. A completed model call returns only the existing safe
   marker, never stored raw output.
7. Key Vault signs a domain-separated canonical hash. The proof records the immutable key version,
   algorithm, issuer, audience, and exact signed payload hash.
8. The model profile fixes endpoint, deployment, region, API behavior, schema projection,
   retention, content-filter policy, timeout, retry boundary, and credential identity. Request data
   cannot override any profile field.
9. Provider requests disable persistence/background work and hosted tools. Retries are bounded to
   explicitly transient failures and do not change context, region, deployment, or profile.
10. Request/response bodies, authorization headers, signatures, source, labels, prompt content, and
    model output are excluded from logs, traces, metrics, SQL, and operator error responses.
11. Azure never creates or approves a local proposal, spec, effect, execution, checkpoint, rollback,
    or Method transition.

## Tests first

- **Success fixture:** one registered installation obtains signed policy and lease, signs one exact
  consent envelope, performs one model request through a fixed deployment, receives a Key
  Vault-signed receipt, verifies it in Rust, and completes the existing local Method flow.
- **Negative/bypass fixture:** reject wrong tenant/client/scope, unregistered or revoked device,
  public-key/hash mismatch, malformed signature, unknown key, stale policy/lease, substituted
  request/manifest/profile/deployment/schema/region/retention, reused consent, caller-selected model
  settings, and receipt mismatch.
- **Failure/recovery fixture:** SQL, Key Vault, App Configuration, identity, DNS, and model failures
  return stable errors; consumed consent remains consumed; uncertain model completion cannot be
  retried as a fresh request; revocation wins before receipt publication; local inspect/history/
  recovery remains available.
- **Compatibility/migration fixture:** canonical JSON round-trips identically in C#, Rust, and
  TypeScript; the SQL migration is forward-only and idempotent at deployment level; existing
  deterministic development tests remain explicit and default production builds remain offline
  until complete configuration is present.
- **Privacy fixture:** seeded Windows path, UNC path, username, source marker, prompt marker, model
  marker, token marker, and secret marker are absent from SQL rows, structured logs, traces,
  metrics, exception output, test exports, and Container Apps console output.

## Task 0: Freeze the baseline and classify existing D2-E work

**Files:**

- Inspect `services/desktop-support-api/**`
- Inspect `services/desktop-support-api.Tests/**`
- Inspect `packages/contracts/**`
- Inspect `crates/desktop-cloud/**`
- Inspect `crates/desktop-app/src/bmad_model/**`
- Inspect `infra/desktop-support/**`
- Modify no implementation files in this task

**Red tests / evidence:**

- Record the existing API, contract, Rust, and Bicep verification results before edits.
- Record the dirty worktree inventory and do not absorb unrelated desktop updater/UI changes.
- Confirm all development adapters fail closed when disabled and production mode is not currently
  reachable.

**Implementation:**

- Create a D2-E change ledger grouped by contracts, support API, desktop, infrastructure, and CI.
- Identify generated files versus sources; edit schemas/generators, never generated bindings by
  hand.
- Capture current package locks and tool versions before adding Azure dependencies.

**Proof:**

```powershell
pnpm contracts:verify:cross-language
dotnet test services/desktop-support-api.Tests/Sapphirus.DesktopSupportApi.Tests.csproj --locked-mode
cargo test -p desktop-cloud --all-features --locked
az bicep build --file infra/desktop-support/main.bicep
git diff --check
```

**Checkpoint:** baseline evidence recorded; no production behavior changed.

## Task 1: Reconcile canonical D2 transport contracts

**Files:**

- Modify `packages/contracts/schemas/model-context-consent.schema.json`
- Modify or create the canonical device-registration, entitlement-lease, desktop-policy, and
  model-access-request schemas under `packages/contracts/schemas/`
- Modify `packages/contracts/fixtures/catalog.json`
- Add valid and invalid cross-language fixtures under `packages/contracts/fixtures/`
- Modify `packages/contracts/scripts/generate.mjs` only if a source schema cannot express the
  required generated shape
- Modify `services/desktop-support-api/Contracts.cs` to consume or mirror the finalized canonical
  shapes without aliases
- Add `crates/desktop-cloud/src/support_contract.rs`
- Modify `crates/desktop-cloud/src/lib.rs`
- Add focused C#, Rust, and Node conformance tests

**Red tests:**

- Prove byte-for-byte canonical fixture agreement for registration, policy, lease, consent,
  request, result, receipt, and safe replay marker.
- Prove unknown/duplicate fields, case aliases, non-UTC timestamps, invalid IDs, invalid base64url,
  oversized public keys/signatures, and unsafe relative labels fail.
- Prove the transport projection cannot be constructed without a consumed local request, current
  registration, verified lease/policy, and installation signer.
- Prove local-only fields (`projectRef`, `runRef`, decision internals, token estimates, redaction
  details) are either intentionally bound into hashes or intentionally omitted from the public
  envelope; no accidental serialization is allowed.

**Implementation:**

- Keep `AuthorizedModelRequest` internal and non-cloneable.
- Add a single consuming projection into canonical `ModelAccessRequest`.
- Extend registration intake with canonical P-256 SubjectPublicKeyInfo bytes encoded as base64url.
  The server computes `installationPublicKeyHash`; caller-provided hashes are comparison evidence,
  not authority.
- Use RFC 3339 UTC instants in public support-plane contracts and convert explicitly at the Rust
  boundary.
- Adopt the richer canonical receipt shape already present in `packages/contracts`; remove the
  older reduced Rust receipt wire shape rather than maintaining two production receipt formats.

**Proof:**

```powershell
pnpm contracts:generate:cross-language
pnpm contracts:verify:cross-language
dotnet test services/desktop-support-api.Tests/Sapphirus.DesktopSupportApi.Tests.csproj --locked-mode
cargo test -p desktop-cloud --all-features --locked
```

**Checkpoint:** `feat(d2): reconcile support-plane contracts`

## Task 2: Add validated production configuration and credential composition

**Files:**

- Split `services/desktop-support-api/Services.cs` into bounded modules without changing behavior
- Add `services/desktop-support-api/Configuration/ProductionOptions.cs`
- Add `services/desktop-support-api/Configuration/AzureClientRegistration.cs`
- Add `services/desktop-support-api/Configuration/ProductionComposition.cs`
- Modify `services/desktop-support-api/Program.cs`
- Modify `services/desktop-support-api/appsettings.json`
- Modify `services/desktop-support-api/Sapphirus.DesktopSupportApi.csproj`
- Update both NuGet lock files through `dotnet` tooling
- Add focused production-composition tests

**Red tests:**

- Production rejects missing/invalid managed identity IDs, SQL server/database, App Configuration
  endpoint, Key Vault URI/key name, model endpoint/deployment, issuer, audience, region, and
  canonical profile hashes.
- Production rejects all development flags and development file stores.
- Azure endpoint options reject credentials, query, fragment, non-HTTPS, unexpected hosts, and
  request-time overrides.
- Each Azure client receives only its designated credential identity.

**Implementation:**

- Add compatible stable packages using `dotnet add package`, then lock restore:
  `Azure.Identity`, `Azure.Data.AppConfiguration`, `Azure.Security.KeyVault.Keys`,
  `Microsoft.Data.SqlClient`, the supported Azure OpenAI client, and
  `Azure.Monitor.OpenTelemetry.AspNetCore`.
- Use explicit `ManagedIdentityCredential(clientId)` in production. Keep local Azure developer
  credentials behind a separate Development-only composition path; do not place a broad
  `DefaultAzureCredential` chain in production.
- Validate all options once at startup. Represent endpoints and profile identifiers as validated
  value objects rather than passing raw strings through request handlers.
- Make the production service fail startup when a required adapter is absent. Health endpoints may
  report dependency state but must not disclose endpoints, tenant IDs, SQL names, or exception
  text.

**Proof:**

```powershell
dotnet restore services/desktop-support-api.Tests/Sapphirus.DesktopSupportApi.Tests.csproj --locked-mode
dotnet test services/desktop-support-api.Tests/Sapphirus.DesktopSupportApi.Tests.csproj --no-restore
dotnet publish services/desktop-support-api/Sapphirus.DesktopSupportApi.csproj -c Release --no-restore
```

**Checkpoint:** `feat(cloud): add fail-closed Azure composition`

## Task 3: Create the Azure SQL authority schema and migration runner

**Files:**

- Add `services/desktop-support-api/Sql/Migrations/0001_support_authority.sql`
- Add `services/desktop-support-api/Sql/SqlConnectionFactory.cs`
- Add `services/desktop-support-api/Sql/SqlMigrationRunner.cs`
- Add `services/desktop-support-api/Sql/SqlDeviceRegistry.cs`
- Add `services/desktop-support-api/Sql/SqlIdempotencyStore.cs`
- Add `services/desktop-support-api/Sql/SqlModelCallIdempotencyStore.cs`
- Add `services/desktop-support-api/Sql/SqlConsentConsumptionStore.cs`
- Add SQL integration tests under `services/desktop-support-api.Tests/Sql/`
- Add reviewed data-plane grant/migration instructions under `infra/desktop-support/`

**Schema minimum:**

- `desktop_schema_migrations`
- `desktop_device_registrations`
- `desktop_entitlement_lease_audit`
- `desktop_context_consent_consumptions`
- `desktop_request_idempotency`
- `desktop_model_call_idempotency`
- `desktop_model_access_receipts`
- `desktop_security_audit`

No table may contain prompt/output content, context labels, local paths, source text, authorization
tokens, signatures beyond required proof/audit material, or provider credentials.

**Red tests:**

- Register/revoke/find are subject partitioned and registration IDs cannot cross subjects.
- Concurrent revocation and lease/model completion always produce either a committed pre-revocation
  result or a revoked failure; no post-revocation receipt can publish.
- Duplicate consent consumption across API replicas yields exactly one success.
- Same idempotency key and fingerprint converges; another fingerprint conflicts.
- Interrupted/in-progress model calls do not return success-shaped results and cannot broaden retry
  authority.
- All SQL commands are parameterized and cancellation-aware.
- Privacy canaries are absent from every table after success and every failure path.

**Implementation:**

- Use Entra-only `Microsoft.Data.SqlClient` authentication with the designated user-assigned managed
  identity. Keep pooling enabled and use bounded connect/command timeouts.
- Replace in-memory cancellation-token authority with a durable `DeviceOperationLease` containing
  subject, registration, and registration epoch/row version. Local cancellation is an optimization;
  the final transactional active/epoch check is authority.
- Use unique indexes for consent authority and subject/idempotency keys.
- Store model-call state as `started`, `completed`, or terminal uncertainty with request/result
  hashes and safe receipt marker only. Never persist transient provider payloads.
- Run schema migration as an explicit deployment step using a migration identity/role. The runtime
  identity receives only runtime DML/execute permissions and cannot alter schema.

**Proof:**

```powershell
dotnet test services/desktop-support-api.Tests/Sapphirus.DesktopSupportApi.Tests.csproj --filter Sql
dotnet test services/desktop-support-api.Tests/Sapphirus.DesktopSupportApi.Tests.csproj
```

**Checkpoint:** `feat(cloud): add durable support authority stores`

## Task 4: Implement installation-key registration and consent verification

**Files:**

- Add `services/desktop-support-api/Security/InstallationPublicKey.cs`
- Add `services/desktop-support-api/Security/InstallationConsentVerifier.cs`
- Modify registration validation and SQL registration storage
- Add consent verification tests in `services/desktop-support-api.Tests/Security/`
- Add `crates/desktop-cloud/src/installation_identity.rs`
- Add Windows implementation and tests under `crates/desktop-cloud/`
- Modify the D2 request preparation/composition seam only where needed to attach the signed
  canonical consent envelope

**Red tests:**

- Only P-256 SPKI with the required algorithm/curve/point shape is accepted.
- Server-computed public-key hash must match the registration/consent binding.
- ES256 verifies the exact domain-separated canonical consent-envelope hash.
- Wrong key ID, algorithm, signature encoding, subject, registration, request, manifest,
  invocation, disclosure, lease, policy, profile, schema, region, retention, nonce, or time window
  is rejected.
- One installation key cannot authorize another registration.
- Exported logs/debug values redact key and signature material.

**Implementation:**

- Generate a non-exportable installation P-256 key through a Windows platform key provider. Persist
  only its opaque local key reference and public material under the existing host-owned encrypted
  local identity boundary.
- Define one documented ES256 signature encoding and canonical payload algorithm shared by Rust and
  C# golden vectors.
- Registration stores the bounded public key and computed hash. Read APIs project only the hash.
- Verify signature before consuming consent or contacting the model provider.
- Treat cryptographic parse/verification failures as stable rejection codes; do not return detailed
  cryptographic diagnostics to clients.

**Proof:**

```powershell
cargo test -p desktop-cloud --all-features --locked
dotnet test services/desktop-support-api.Tests/Sapphirus.DesktopSupportApi.Tests.csproj --filter Consent
pnpm contracts:verify:cross-language
```

**Checkpoint:** `feat(d2): bind consent to installation identity`

## Task 5: Implement App Configuration policy and Key Vault signing

**Files:**

- Add `services/desktop-support-api/Policy/AppConfigurationPolicyProvider.cs`
- Add `services/desktop-support-api/Policy/CanonicalPolicyProjector.cs`
- Add `services/desktop-support-api/Signing/KeyVaultHashSigner.cs`
- Add `services/desktop-support-api/Signing/AzureSignedPolicyService.cs`
- Add `services/desktop-support-api/Signing/AzureModelReceiptSigner.cs`
- Add signing/policy tests and fixed cryptographic vectors
- Modify production dependency registration

**Red tests:**

- Policy snapshots reject unknown fields, invalid region/profile/schema hashes, downgrade, stale
  refresh, over-limit context settings, unsupported retention, and unapproved model deployments.
- Lease and receipt proofs bind exact canonical payload hashes, immutable key versions, issuer, and
  audience.
- Key rotation accepts an explicit active/verification overlap and rejects unknown, disabled, or
  retired keys outside policy.
- Key Vault timeout/throttle/unavailability returns no unsigned policy, lease, or receipt.
- Signing is invoked only after all semantic validation succeeds.

**Implementation:**

- Load allowlisted configuration keys into an immutable policy snapshot; do not dynamically merge
  arbitrary labels into request behavior.
- Cache only verified non-secret policy snapshots with bounded refresh and last-known-valid expiry.
- Use `CryptographyClient` against a non-exportable P-256 Key Vault key. Sign the canonical SHA-256
  digest and emit base64url ES256 proof with the exact key version as `keyId`.
- Keep policy/lease construction separate from receipt construction even if they initially use one
  vault. The plan allows separate keys without changing contracts.
- Record only safe issuance audit metadata and hashes.

**Proof:**

```powershell
dotnet test services/desktop-support-api.Tests/Sapphirus.DesktopSupportApi.Tests.csproj --filter "Policy|Signing|Lease|Receipt"
```

**Checkpoint:** `feat(cloud): sign policy leases and receipts`

## Task 6: Implement the fixed-profile managed-identity model broker

**Files:**

- Add `services/desktop-support-api/Model/ModelAccessProfile.cs`
- Add `services/desktop-support-api/Model/CanonicalPromptProjector.cs`
- Add `services/desktop-support-api/Model/AzureOpenAiModelAccessBroker.cs`
- Add `services/desktop-support-api/Model/CanonicalModelOutputValidator.cs`
- Add model broker tests with an injected provider executor
- Modify production dependency registration

**Red tests:**

- Request data cannot choose or alter endpoint, API version, deployment, credential, region,
  retention, tools, background behavior, schema, or fallback.
- Absolute paths, UNC paths, drive prefixes, traversal, username-bearing labels, oversized context,
  unsupported classification, unapproved purpose/role/schema, and whole-repository patterns fail
  before provider egress.
- Provider refusal, incomplete output, malformed JSON, schema-invalid output, timeout, rate limit,
  cancellation, content filter, and quota exhaustion map to explicit safe outcomes.
- Retries are bounded and limited to policy-approved transient failures; no retry changes request
  bytes or profile.
- Provider request and response bodies never reach logs, metrics, traces, SQL, exception details, or
  idempotency stores.

**Implementation:**

- Resolve one immutable model profile from the verified policy, not from caller input.
- Authenticate with the model-specific user-assigned managed identity and the narrow Azure OpenAI
  user role.
- Build the minimum prompt from reviewed items in memory. Disable provider storage, hosted tools,
  background execution, and arbitrary URL access.
- Request the exact canonical output shape supported by the selected API. Recompute and validate the
  returned payload hash and canonical schema before receipt signing.
- Capture safe provider request ID, usage, latency, retry count, and terminal status. Compute cost
  from a versioned server-side price profile; never trust caller or model-reported cost.
- Dispose transient request/output buffers as soon as response verification and signing complete.

**Proof:**

```powershell
dotnet test services/desktop-support-api.Tests/Sapphirus.DesktopSupportApi.Tests.csproj --filter Model
dotnet test services/desktop-support-api.Tests/Sapphirus.DesktopSupportApi.Tests.csproj
```

**Checkpoint:** `feat(cloud): broker transient Azure model access`

## Task 7: Make model admission, consumption, brokerage, and receipt commit coherent

**Files:**

- Refactor `services/desktop-support-api/Program.cs` route body into
  `Model/ModelAccessCoordinator.cs`
- Modify SQL idempotency, consent, registry, and receipt adapters
- Add coordinator concurrency/recovery tests
- Keep public route and error contracts stable unless Task 1 deliberately versions them

**Red tests:**

- Consent is consumed before provider egress and cannot be restored by provider failure.
- A completed provider result is never returned until receipt signing and active-registration
  commit succeed.
- Key Vault failure after provider completion creates a terminal uncertain/failure marker and does
  not allow a new provider call with the same request authority.
- Registration revocation between admission and commit prevents receipt publication.
- Concurrent identical requests converge to one provider call; alternate idempotency keys cannot
  replay one consent.
- Safe replay exposes only receipt ID, request hash, result hash, and typed completion status.

**Implementation:**

- Introduce a coordinator with explicit stages: authenticate -> validate -> load active
  registration -> verify policy/lease/consent -> reserve idempotency -> consume consent -> broker ->
  validate -> sign -> transactional commit.
- Persist stage metadata only when needed for replay/recovery and never persist provider payload.
- Define terminal uncertainty behavior explicitly. Do not automatically retry after provider
  acceptance when completion is unknown.
- Preserve server-owned deadlines and cancellation, but rely on final SQL concurrency checks for
  cross-replica authority.

**Proof:**

```powershell
dotnet test services/desktop-support-api.Tests/Sapphirus.DesktopSupportApi.Tests.csproj --filter "Coordinator|Concurrency|Recovery|Revocation|Idempotency"
```

**Checkpoint:** `feat(cloud): make model access single-use and durable`

## Task 8: Add privacy-safe observability and operational health

**Files:**

- Add `services/desktop-support-api/Observability/SupportPlaneTelemetry.cs`
- Add `services/desktop-support-api/Observability/PrivacyRedactionProcessor.cs`
- Add `services/desktop-support-api/Health/AzureDependencyHealthChecks.cs`
- Modify `Program.cs` and production composition
- Add privacy canary and telemetry tests
- Add Azure Monitor queries/alerts as Bicep modules under `infra/desktop-support/modules/`

**Red tests:**

- Seeded source, output, path, username, token, signature, and secret canaries are absent from all
  exported spans, logs, metrics, baggage, exception events, and health responses.
- Route templates, not raw paths, are recorded.
- Subject, registration, request, receipt, and content hashes are not metric dimensions.
- Health endpoints disclose only healthy/degraded/unhealthy plus safe dependency classes.

**Implementation:**

- Instrument authentication outcome, admission denial class, SQL/Key Vault/model dependency
  latency, provider status class, schema outcome, retry count, token/cost aggregates, receipt
  issuance, and replay/revocation events.
- Use allowlisted low-cardinality dimensions: route template, region, environment, release, safe
  error code, model role/profile, and coarse budget class.
- Disable ASP.NET request-body logging and Azure SDK content logging.
- Add alerts for authentication spikes, consent replays, receipt-signing failures, SQL saturation,
  model throttling, privacy canary detection, and health/SLO burn.

**Proof:**

```powershell
dotnet test services/desktop-support-api.Tests/Sapphirus.DesktopSupportApi.Tests.csproj --filter "Privacy|Telemetry|Health"
node tools/check-secrets.mjs
```

**Checkpoint:** `feat(cloud): add privacy-safe support telemetry`

## Task 9: Compose the production desktop support client

**Files:**

- Add production-only modules under `crates/desktop-cloud/src/` for bootstrap, registration,
  policy, lease, transport projection, and ES256 proof verification
- Modify `crates/desktop-app/src/bmad_model/config.rs`
- Modify `crates/desktop-app/src/bmad_model/transport.rs`
- Modify `crates/desktop-app/src/bmad_model/verification.rs`
- Modify `crates/desktop-app/src/bmad_model/coordinator.rs`
- Add host-only lifecycle tests and bounded IPC projections
- Modify build-time configuration only through signed/package-controlled values

**Red tests:**

- Production mode cannot start without exact tenant, API client, scope, origin, policy trust root,
  and receipt trust configuration.
- Token, installation private key, public key bytes, lease proof, receipt proof, raw output, and
  absolute paths never cross IPC.
- Policy/lease/receipt unknown key, bad signature, issuer/audience mismatch, expiry, downgrade,
  registration mismatch, profile mismatch, or replay fails closed.
- Sign-out invalidates cloud session epoch and registration access without deleting local work.
- Offline/default and deterministic development modes remain behaviorally unchanged and explicit.

**Implementation:**

- Add a production feature/configuration path; do not overload `deterministic-help`.
- Reuse the existing Windows identity broker and fixed-origin HTTPS executor.
- Add bootstrap -> registration recovery -> policy -> lease sequencing with bounded refresh and
  last-known-valid signed cache under host-owned encrypted local storage.
- Sign the exact canonical consent envelope immediately before the one-shot transport projection.
- Verify the full canonical receipt and import only verified result/hash evidence into the existing
  local Method completion path.
- Keep cloud operations asynchronous and cancellable without giving renderer callbacks cloud
  authority.

**Proof:**

```powershell
cargo fmt --all -- --check
cargo test -p desktop-cloud -p desktop-app --all-features --locked
cargo clippy -p desktop-cloud -p desktop-app --all-targets --all-features --locked -- -D warnings
pnpm verify:boundaries
```

**Checkpoint:** `feat(d2): compose production support plane`

## Task 10: Harden and modularize the Azure deployment

**Files:**

- Refactor `infra/desktop-support/main.bicep`
- Add resource modules under `infra/desktop-support/modules/`
- Add environment-safe example parameter files only
- Add SQL migration deployment identity/role outputs
- Add alert, action-group input, budget, diagnostic settings, and deployment outputs
- Update `infra/desktop-support/README.md`
- Add or update a deployment workflow only after repository environment/approval conventions are
  confirmed

**Red validation / policy checks:**

- Bicep build and resource-group validation pass.
- Azure policy assignments are inspected for the target subscription before an actual deployment.
- What-if contains no local-auth enablement, public data-plane endpoint, plaintext secret, mutable
  image tag, overly broad role, or caller-controlled model endpoint.
- Production image is digest pinned and API deployment waits for RBAC/private DNS readiness.

**Implementation:**

- Split identities at minimum into image pull, support data/config, signing, and model access.
  Because identities attached to one process are not a hard process-isolation boundary, document
  this first deployment as logical least privilege and preserve module seams for independently
  deployed Model Access API/signing workloads if the threat review requires it.
- Scope RBAC to exact resources and roles. Keep schema migration identity separate from runtime SQL
  permissions.
- Keep Azure SQL, Key Vault, App Configuration, ACR, and Azure OpenAI private with private DNS.
- Keep public TLS ingress only for employee desktop access; Entra tenant/audience/client/scope checks
  remain mandatory.
- Add Container Apps readiness/liveness probes, revision settings, bounded scale, zone/DR decisions,
  diagnostic settings, budget alerts, and required tags.
- Preserve two-stage deployment: infrastructure first, immutable image publication second, API
  revision activation third, smoke/promotion last.

**Proof:**

```powershell
az bicep build --file infra/desktop-support/main.bicep
az deployment group validate --resource-group <resource-group> --parameters infra/desktop-support/main.example.bicepparam
az deployment group what-if --resource-group <resource-group> --parameters infra/desktop-support/main.example.bicepparam
```

**Checkpoint:** `feat(cloud): harden Azure support infrastructure`

## Task 11: Add CI, deployed smoke, privacy, and recovery gates

**Files:**

- Add focused support-plane jobs to `.github/workflows/source.yml` or a dedicated approved workflow
- Add deployment validation to the existing native/manual lane conventions
- Add deployment smoke scripts under `tools/` without embedded tenant/subscription values
- Add privacy scan and SQL inspection helpers
- Update root verification scripts only when they remain deterministic for contributors without
  Azure access

**Gates:**

1. Offline source gate: contract generation, C# tests, Rust tests, secret scan, boundary scan, Bicep
   build, container build, and SBOM/vulnerability scan.
2. Azure integration gate: managed-identity access to each exact dependency, SQL migration/runtime
   permission separation, Key Vault sign/verify, App Configuration policy load, private DNS, and
   fixed model deployment.
3. End-to-end gate: Entra-authenticated test installation, registration, lease, signed consent,
   one no-store model call, signed receipt, Rust verification, and local Method completion.
4. Privacy gate: canary request followed by SQL/log/trace/metric/export scans proving transient
   payload absence.
5. Recovery gate: revocation race, SQL transient failure, Key Vault throttle, model timeout,
   Container Apps revision rollback, database point-in-time restore rehearsal, and signing-key
   rotation.

Azure integration and deployment jobs use GitHub OIDC/workload identity federation and protected
environments. They do not use long-lived client secrets.

**Proof:**

```powershell
pnpm verify:deferred-full
dotnet publish services/desktop-support-api/Sapphirus.DesktopSupportApi.csproj -c Release --no-restore
docker build --file services/desktop-support-api/Dockerfile services/desktop-support-api
git diff --check
```

**Checkpoint:** `test(cloud): qualify production support plane`

## Task 12: Roll out behind explicit production gates

**Stages:**

1. Deploy infrastructure with `deployApi=false`.
2. Apply reviewed SQL migration and least-privilege data-plane grants.
3. Publish signed/SBOM-attested API image by immutable digest.
4. Deploy one non-production revision with model access disabled.
5. Verify identity, policy, lease, signing, SQL, telemetry, and privacy gates.
6. Enable model access for a dedicated test tenant/group and fixed budget.
7. Run end-to-end and failure-injection qualification.
8. Promote the same digest through approved environments; do not rebuild per environment.
9. Enable production desktop composition only after trust roots, helper signing, API endpoint, and
   tenant/client/scope values are package-controlled and verified.
10. Retain an operator kill switch that blocks new model admission without affecting local history,
    inspect, export, rollback, or recovery.

**Rollback:**

- Route traffic back to the prior Container Apps revision.
- Disable model admission in the signed policy/App Configuration.
- Revoke the affected signing key version or model identity assignment only through an audited
  incident procedure.
- Never roll back a SQL migration destructively; use compatible forward repair.
- Desktop falls back to explicit offline behavior, not deterministic or unsigned cloud behavior.

## Change and rollback

- **Files/surfaces allowed:** canonical contract schemas/generation/fixtures; `desktop-cloud`;
  production `desktop-app::bmad_model` composition; Desktop Support API and tests; Azure SQL
  migrations; `infra/desktop-support`; directly related CI, boundary checks, and documentation.
- **Files/surfaces excluded:** renderer authority expansion; generic filesystem/process routes;
  web-managed workspace lifecycle; sync/collaboration/remote-job/package publication; unrelated
  updater/UI work already present in the worktree.
- **Disable path:** signed policy disables new connected model access and Container Apps rolls back
  to the prior digest. The desktop remains locally usable and reports support-plane unavailability.
- **Observability/evidence:** safe route outcomes, dependency latency/status, admission reason,
  usage/cost aggregates, replay/revocation/signing events, immutable image digest, Bicep what-if,
  SQL migration version, RBAC evidence, privacy scan, and end-to-end receipt verification.

## Review ledger

- **Implementer full-diff review:** required after each checkpoint; verify authority boundaries,
  production/development composition separation, SQL privacy, canonical signing, endpoint fixation,
  and renderer projections.
- **Independent bug/security review:** required for Tasks 1, 3, 4, 5, 6, 7, 9, and 10 before
  production deployment.
- **Commands executed:** populate from each task's proof section during implementation.
- **Checks skipped and reason:** no production promotion may skip contract, privacy, cryptographic,
  SQL concurrency, Bicep, or deployed end-to-end gates.
- **Remaining risks after D2-E:** Authenticode/helper publisher verification and clean-machine release
  qualification remain release gates. Signed package publication, optional sync/collaboration,
  telemetry intake from desktops, and remote jobs remain separate milestones C3-C6.

## Acceptance criteria

D2-E is complete only when all of the following are true:

- C#, Rust, and TypeScript use one canonical support-plane request/receipt contract.
- Production service startup contains no in-memory or development signing/model/consent adapter.
- Azure SQL enforces durable subject partitioning, replay prevention, revocation epochs, and safe
  receipt metadata without transient payloads.
- Installation signatures, policy/lease proofs, and receipt proofs verify against exact canonical
  hashes and documented key rotation policy.
- The model broker uses only managed identity and a fixed approved Azure deployment with no-store,
  no hosted tools, no background work, bounded retries, and canonical output validation.
- The desktop production path verifies identity, policy, lease, consent, response, and receipt
  bindings while keeping every local effect under the existing Rust authority.
- Privacy canaries are absent from SQL, logs, traces, metrics, queues, diagnostics, and operator
  projections.
- Bicep validation/what-if, source gates, Azure integration tests, end-to-end tests, failure
  injection, rollback, and recovery evidence pass.
- Disabling or losing Azure blocks only connected capabilities; local inspect, history, export,
  rollback, and recovery continue according to existing local policy.

## Azure implementation references

- Repository authority:
  - `bmad-runtime-lib/98 - Azure Support Plane for Windows Desktop.md`
  - `bmad-runtime-lib/93 - Split Web and Windows Desktop Architecture Plans.md`
  - `docs/superpowers/specs/2026-07-15-d2-real-ai-request-path-design.md`
  - `docs/superpowers/specs/2026-07-16-d2-d-completion-d3-integration-design.md`
- Microsoft guidance:
  - Managed identities in Azure Container Apps:
    <https://learn.microsoft.com/azure/container-apps/managed-identity>
  - Azure Container Apps security:
    <https://learn.microsoft.com/azure/container-apps/security>
  - Managed identity with App Configuration:
    <https://learn.microsoft.com/azure/azure-app-configuration/howto-integrate-azure-managed-service-identity>
  - Azure-hosted app authentication to Azure OpenAI:
    <https://learn.microsoft.com/dotnet/ai/how-to/app-service-aoai-auth>
  - Azure OpenAI secure .NET application guidance:
    <https://learn.microsoft.com/azure/developer/ai/get-started-securing-your-ai-app>

