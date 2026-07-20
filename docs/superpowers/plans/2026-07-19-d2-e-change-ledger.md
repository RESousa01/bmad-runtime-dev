# D2-E change ledger

**Started:** 2026-07-19 · branch `p0-baseline-consolidation` (P0 exit gate met locally; CI pending push)
**Plan:** [2026-07-17-d2-e-azure-support-plane-services.md](./2026-07-17-d2-e-azure-support-plane-services.md)

## Task 0 baseline evidence (2026-07-19)

- `pnpm contracts:verify:cross-language` — green twice on 2026-07-18 (104 pass, 1 environment skip); unchanged since.
- `dotnet restore … --locked-mode` then `dotnet test --no-restore` — 62/62 passed. Note: `--locked-mode` is not a valid xUnit v3/MTP test flag; the plan's single-command form exits 5 with zero tests run. Restore and test must be separate steps.
- `cargo test -p desktop-cloud --all-features --locked` — 29 tests green.
- `az bicep build --file infra/desktop-support/main.bicep` — exit 0 with 4 warnings (BCP187 `sku`, BCP037 `infrastructureSubnetId`, hardcoded `database.windows.net`, BCP318 possible-null access). To fix in the infrastructure task.
- `git diff --check` clean; working tree clean at start.
- Worktree inventory: no unrelated dirty changes to absorb (P0 consolidation already committed by lane).

## Task 1 evidence (2026-07-19)

- Added `crates/desktop-cloud/src/support_contract.rs`: canonical `ModelAccessRequest` projection. `AuthorizedModelRequest` stays local-only; the sole wire path is `project_model_access_request(request, &CanonicalProjectionInputs { registration, lease, policy, subject, window, profile, signer })`, which consumes the request, fail-closes on policy-hash mismatch, missing context language, or non-transient retention, and emits only the public canonical shape (no local refs, decision internals, token estimates, or redaction details).
- Consent envelope hash: `ConsentEnvelopeDraft` serialized camelCase minus `consentEnvelopeHash`/`proof`, hashed via `canonical_hash`, signed ES256 by the `InstallationConsentSigner`.
- `crates/desktop-cloud/tests/support_projection.rs` — 6 tests: happy-path field parity, policy-hash mismatch, missing language, inverted window, retention mismatch, signer failure.
- Gates: `cargo clippy -p desktop-cloud -p desktop-runtime --all-features --all-targets -- -D warnings` clean; `cargo test` (both crates, all features) all green; `pnpm contracts:verify:cross-language` 104 pass / 0 fail / 1 environment skip.
- Environment notes: `cargo clean` reclaimed 98 GiB from a full disk (C: was at 100%); that also deleted the repo-local pinned `target/contract-tools/bin/cargo-typify.exe` — restored with `node tools/contract-codegen/native-generator.mjs restore`. The restore left a duplicate reviewed archive in a second Cargo cache index dir; the lock gate refuses ambiguity, so the stale identical copy (`index.crates.io-1949cf8c…`) was removed.

## Task 2 evidence (2026-07-19)

- Prior lane (`1cfc6596`) had already landed the Configuration scaffolding (`ProductionOptions`, `AzureClientRegistration`, `ProductionComposition`), package references, and 3 composition tests; this task closed the remaining plan gaps.
- Split `Services.cs` (1136 lines, 24 types) into bounded modules with no behavior change: `SupportPlaneOptions.cs`, `DeviceRegistry.cs`, `CancellableOperation.cs`, `Idempotency.cs`, `ModelAuthority.cs`, `DevelopmentAdapters.cs`.
- Added missing red tests: Key Vault and model endpoint rejection theories (non-HTTPS, userinfo, path, query, unexpected host), production rejection of development flags and development consent file store, and per-client credential identity (three distinct `ManagedIdentityCredential` instances via new internal `CreateManagedIdentityCredentials` seam).
- Proof: locked restore clean; `dotnet test` 76/76 passed; `dotnet publish -c Release --no-restore` succeeded.
- Note: `ProductionComposition.AddProductionComposition` still ends by throwing "Production authority adapters are not configured" — intentional fail-closed placeholder until Tasks 3–6 supply the real adapters.

## Task 3 evidence (2026-07-20)

- Added `services/desktop-support-api/Sql/`: embedded migration `0001_support_authority.sql` (all 8 planned tables, state CHECK constraints, no content columns), `SqlConnectionFactory` (Entra-only managed-identity auth, mandatory encryption, pooled, bounded connect/command timeouts; raw-string ctor is internal and test-only), `SqlMigrationRunner` (once-only, name-ordered, per-migration transaction, refuses re-apply on hash drift), `SqlDeviceRegistry`, `SqlIdempotencyStore`, `SqlModelCallIdempotencyStore`, `SqlConsentConsumptionStore`.
- Durable revocation authority: every lease/receipt commit re-checks active state + registration epoch under `UPDLOCK, HOLDLOCK` inside the commit transaction; the in-process token is only an optimization. Revocation is an UPDATE that flips state and increments epoch.
- Model-call uncertainty: a started claim that fails during commit is never released — later calls throw `ModelCallIdempotencyUncertainException` instead of returning success-shaped results; only the completion marker (receipt id + hashes) is persisted, never payloads. `SqlIdempotencyStore` refuses `ModelAccessResult` responses outright.
- Consent single-use authority is the primary key over (subject hash, registration, consumption hash); duplicate insert (2627/2601) maps to `AlreadyConsumed`.
- Tests: `services/desktop-support-api.Tests/Sql/` — LocalDB-backed fixture (throwaway DB per class, real migration run, `Assert.Skip` when LocalDB is absent), 10 tests covering subject partitioning, revocation vs lease/receipt commits, replica-concurrent consent duplication, idempotency convergence/conflict/claim-release, model-call uncertainty and marker replay, cancellation awareness, hostile-subject parameterization, and a privacy-canary scan of every text column after success and failure paths.
- Grants doc: `infra/desktop-support/sql-grants.md` — migration identity (db_ddladmin) vs runtime identity (DML only, no schema alteration).
- Proof: SQL-filtered run 10/10 (note: xUnit v3/MTP needs `-- --filter-namespace …`, the plan's `--filter Sql` form matches nothing); full suite 86/86; Release publish clean.

## Task 4 evidence (2026-07-20)

- Shared ES256 signature spec documented in `crates/desktop-cloud/src/installation_identity.rs`: payload = ASCII of the envelope-hash string `sha256:<hex>`; ECDSA P-256 + SHA-256; raw 64-byte `r||s` base64url (JOSE); proof key id = SPKI hash (same value as the registration's installation-public-key hash).
- Rust: `WindowsInstallationIdentity` — non-exportable persisted P-256 key via NCrypt (TPM platform provider first, software KSP fallback), SPKI export, `InstallationConsentSigner` impl; `Debug` redacts key material. Windows integration test creates a real key, signs, verifies via `BCryptVerifySignature`, reopens, and deletes it.
- Golden vector: `crates/desktop-cloud/tests/consent_envelope_vector.rs` pins the canonical envelope hash `sha256:9789b78a…` for a fixed draft; the C# `InstallationConsentVerifierTests` asserts the identical constant, proving the two `canonical_hash` implementations agree byte-for-byte (purpose preimage `sapphirus:model-context-consent:v1\n` + RFC 8785 JSON).
- C#: `Security/InstallationPublicKey.cs` (strict P-256 SPKI value object, redacting `ToString`; `RequestGuards.TryGetInstallationPublicKeyHash` now delegates to it) and `Security/InstallationConsentVerifier.cs` (recomputes the envelope hash from the received consent, enforces key-id/registration/manifest/time-window bindings, verifies the signature with the registered device's key; every failure collapses to `Rejected`). Wired as the app-default `IContextConsentVerifier`.
- Behavior change: an unverifiable consent now yields 403 (Rejected) instead of 503 (Unavailable) — `Opaque_consent_hash_is_not_treated_as_verified` updated accordingly.
- Proof: `cargo test -p desktop-cloud --all-features --locked` all green (incl. live NCrypt test); clippy `-D warnings` clean; C# 108/108 (Security namespace 22/22); cross-language 104 pass.

## Task 5 evidence (2026-07-20)

- `Policy/AppConfigurationPolicyProvider.cs`: allowlisted-key snapshot loading (`desktop-support:policy:*` only, unknown fields reject), full value validation (policy id shape, version ≥ 1, context limits ≤ 512 KiB/64 items, non-empty regions/deployments, retention pinned to `transient_no_store`), bounded refresh interval, downgrade rejection, and last-known-valid fallback that applies only to *load* failures within its lifetime — validation failures are always hard.
- `Policy/CanonicalPolicyProjector.cs`: shared `CanonicalProof` canonical-JSON digester (same RFC 8785 + purpose-preimage scheme as Rust `canonical_hash`; purposes `desktop-policy`, `entitlement-lease`, `model-access-receipt`) and policy/lease projection.
- `Signing/KeyVaultHashSigner.cs`: `IHashSigner` seam + production `CryptographyClient` ES256 signer (raw r||s base64url, versioned key id, vault failures propagate — no unsigned artifacts) + `SigningKeyRing` rotation policy (one active key, explicit verification-only overlap, everything else rejected).
- `Signing/AzureSignedPolicyService.cs` / `Signing/AzureModelReceiptSigner.cs`: vault-signed policies, leases, receipts; canonical digests recomputed from full field sets; proofs bind digest + key version + issuer (`Authority`) + audience; semantic validation strictly precedes signing (recording-signer tests prove zero sign calls on invalid input; non-`succeeded` results are ineligible for receipts).
- Production DI: policy source/provider, Key Vault signer, signed-policy service, receipt signer registered in `ProductionComposition`; startup still fails closed pending the Task 6 model broker.
- Proof: Signing namespace 18/18; full suite 126/126; Release publish clean. (MTP filter form: `-- --filter-namespace …Tests.Signing`.)

## Task 6 evidence (2026-07-20)

- `Model/ModelAccessProfile.cs`: one immutable profile resolved from verified policy + validated production config (deployment must be policy-approved, region policy-allowed); fixed purpose/role/schema; versioned server-side price profile (`desktop-price-profile.2026-07`) computes cost — caller/model-reported cost is never trusted.
- `Model/CanonicalPromptProjector.cs`: minimum in-memory prompt; pre-egress guards reject absolute/UNC/drive/traversal paths, username-bearing labels (`users/`, `home/`, …), wildcard/whole-repo patterns, oversized context, and unsupported classifications (only `public`/`internal`/`source`) — all before any provider call (recording-executor tests prove zero egress).
- `Model/CanonicalModelOutputValidator.cs`: closed-schema validation (`sapphirus.bmad-method-help-proposal.v1`, `additionalProperties:false`, bounded sizes/depth) with deterministic canonical re-serialization and recomputed payload hash; the strict provider JSON schema constant is hashed as the schema projection hash.
- `Model/AzureOpenAiModelAccessBroker.cs`: fixed-profile broker over an `IModelProviderExecutor` seam. Request data cannot alter profile fields (`profile_mismatch`). Safe outcome codes: `context_rejected`, `provider_refusal`, `content_filtered`, `incomplete_output`, `malformed_output`, `schema_invalid`, `timeout`, `rate_limited`, `quota_exhausted`, `provider_unavailable`; cancellation propagates as cancellation. Retries bounded by the profile (3 attempts), transient outcomes only, identical request record reused (`Assert.Single(Calls.Distinct())`). Production executor disables provider storage (`StoredOutputEnabled=false`), tools, and background behavior, and maps SDK failures by status with no body material in any exception surface (canary-checked).
- Production DI registers the executor and broker; startup still fails closed pending Task 7's SQL authority binding.
- Proof: Model namespace 30/30; full suite 156/156; Release publish clean.

## Task 7 evidence (2026-07-20)

- `Model/ModelAccessCoordinator.cs`: the model-access route body extracted into an explicit staged coordinator (validate → load active registration → verify consent → reserve idempotency → consume consent → broker → sign (inside broker) → transactional commit → render). Route and error contracts unchanged; the route is now a thin adapter returning `ModelAccessCoordinationResult`.
- New failure mappings the old route lacked: `ModelCallIdempotencyUncertainException` → 409 `model_call_uncertain` (terminal uncertainty; the same request authority can never fund a new provider call), and `ModelAccessFailedException` outcomes → stable statuses (`context_rejected`/`profile_mismatch` 400, `rate_limited` 429, `timeout` 504, `quota_exhausted`/`provider_unavailable` 503, output failures 502).
- Production boot unlocked: `ProductionComposition` now binds `SqlConnectionFactory`, `SqlDeviceRegistry`, `SqlIdempotencyStore`, `SqlModelCallIdempotencyStore`, `SqlConsentConsumptionStore`, and the installation consent verifier, and the fail-closed "adapters not configured" throw is removed — every authority adapter now exists.
- Coordinator tests (6): consumption strictly precedes egress and provider failure does not restore consent; revocation between admission and commit blocks receipt publication (no receipt readable afterward); terminal uncertainty yields 409 with zero broker/consumption calls; concurrent identical requests converge to one provider call and one consumption; alternate idempotency keys cannot replay one consent; safe replay exposes only receipt id/request hash/result hash.
- Proof: coordinator class 6/6 (MTP `--filter-class`); full suite 162/162; Release publish clean. Cross-replica equivalents of the concurrency/recovery invariants are covered by the Task 3 LocalDB store tests.

## Task 8 evidence (2026-07-20)

- `Observability/SupportPlaneTelemetry.cs`: one metric surface, fixed allowlisted tag keys (unknown key throws), values that look like hashes/tokens/paths/opaque blobs degrade to `invalid_dimension`; instruments auth outcomes, admission denials, dependency latency, provider status classes, schema outcomes, retries, token/cost aggregates, receipt issuance, replays, revocations. Coordinator emits denial/replay/receipt/usage/revocation events (optional telemetry param — tests unchanged).
- `Observability/PrivacyRedactionProcessor.cs`: exported spans keep only allowlisted tags, redact unsafe values, drop all events (exception messages/stack traces never export), and force `DisplayName` to the route template.
- `Health/AzureDependencyHealthChecks.cs`: `/healthz/live` + `/healthz/ready` with a safe writer that renders only status + dependency class (`sql`, `configuration`, `signing`, `model`); SQL/policy probes swallow exception detail; signing/model checks are composition-state only (live probes belong to the deployed smoke gate).
- Production composition: health checks + Azure Monitor OpenTelemetry export gated on `APPLICATIONINSIGHTS_CONNECTION_STRING`, with the redaction processor in the pipeline.
- `infra/desktop-support/modules/monitor-alerts.bicep`: seven scheduled-query alerts (auth spike, consent replays, receipt-signing failures, SQL saturation, model throttling, privacy canary sev-0, SLO fast burn) — compiles clean.
- Proof: Observability tests 5/5 (canaries absent from spans/metrics/health bodies); full suite 167/167; `node tools/check-secrets.mjs` clean (3378 files); `az bicep build` on the module exit 0.

## Task 9 evidence (2026-07-20)

- `crates/desktop-cloud/src/production.rs`: fail-closed `ProductionSupportConfig` (tenant/client GUIDs, `api://` scope, HTTPS origin via `SupportApiOrigin`, region, issuer/audience, pinned policy + receipt `ProofKeyRing`s), ES256 verification over raw canonical digests via BCrypt (matching `KeyVaultHashSigner`, which signs the digest directly — an early draft that re-hashed the payload was caught and fixed in review), strict `deny_unknown_fields` policy/lease documents, downgrade + expiry + registration + issuer/audience + replay fail-closure, `SignedStateStore` last-known-valid cache that re-verifies (a tampered cache is discarded, never trusted), and sign-out session-epoch invalidation that leaves local work untouched.
- Correctness fix found in review: lease wire instants are parsed and re-rendered canonically (`…fffZ`) before digest recomputation, because the wire encoding (`+00:00`) differs from the rendering the service hashed; the test signs with canonical instants and serves offset-format instants to prove normalization.
- `crates/desktop-cloud/tests/production_lifecycle.rs` (4 tests, Windows signed-document matrix uses a real NCrypt key as the vault analog via `sign_digest`): configuration fail-closure, bounded session projection + epoch invalidation, full policy/lease/receipt tamper matrix, cache re-verification.
- desktop-app: new `production-support` feature (deterministic-help not overloaded); production activates only when the complete package-controlled value set (`SAPPHIRUS_SUPPORT_*` build-time values) exists — otherwise explicit development/offline behavior is unchanged, and composing production without exact config still fails closed in desktop-cloud. `ProductionHelpTransport` fails closed (`Offline`) until the gated rollout activates the deployed round-trip; the pinned IPC catalog is untouched (production projects truthfully as unavailable/offline until rollout).
- `map_cloud_error` covers the two new `CloudError` variants.
- Proof: `cargo fmt --check` clean; `cargo test -p desktop-cloud -p desktop-app --all-features --locked` all green (15 suites); clippy `-D warnings` clean; `pnpm verify:boundaries` green (test key names switched from `std::process::id()` to `rand` to satisfy the child-process boundary scan).

## Task 10 evidence (2026-07-20) — committed `ed92395b`

- `main.bicep` hardening: image-pull identity split from the runtime identity (registry pull now uses `imagePullIdentity`; documented as logical least privilege with module seams for later hard isolation), dedicated never-attached SQL migration identity with client/object-id outputs, startup/liveness/readiness probes on `/healthz/*`, Key Vault audit + SQL metric diagnostic settings, alert + budget modules behind `deployAlerts`, and new outputs (signing key URI, App Configuration endpoint, model endpoint).
- Config-bug fixes found in review: the container env set `Sapphirus__SigningKeyName` but the options bind `ReceiptSigningKeyName`, and the four required canonical profile hashes were entirely absent — production boot would have failed options validation. Both fixed; example bicepparam updated.
- Existing digest-pin assert retained; `az bicep build` exit 0 with only the four pre-existing Task-0 baseline warnings. `az deployment group validate`/`what-if` are operator steps, deliberately deferred with the deployment itself.

## Task 11 evidence (2026-07-20)

- `.github/workflows/desktop-support.yml`: offline gate (`support-api` job — locked restore, full C# suite incl. LocalDB stores on `windows-2025`, Release publish, Bicep build) + container gate (`support-container` — docker build, SBOM, vulnerability scan, all gated on reviewed base-image digests being pinned; the Dockerfile intentionally carries `REPLACE_WITH_REVIEWED_*` placeholders until rollout stage 3) + `azure-gates` job (workflow_dispatch only, protected environment, GitHub OIDC federation, zero embedded tenant/subscription values).
- `tools/support-smoke/deployed-smoke.ps1` (parameterized deployed smoke: token, bounded health shape, bootstrap, signed policy shape, fail-closed lease) and `tools/support-smoke/privacy-sql-inspect.sql` (scans every text column of every `desktop_*` table for canary/token/path markers; empty set = pass).
- Proof: `pnpm verify:deferred-full` green (see below); `dotnet publish -c Release` clean; local `docker build` correctly fails closed on the unpinned reviewed-digest placeholders (the CI gate mirrors this); `git diff --check` clean.

## Task 12 preparation (2026-07-20) — execution deferred

- `docs/superpowers/plans/2026-07-20-d2-e-rollout-runbook.md`: the ten rollout stages and the rollback procedure mapped onto the concrete artifacts from Tasks 1–11 (migration identity outputs, kill switch via `approvedModelDeployments`, key-rotation contract via `SigningKeyRing`/`ProofKeyRing`, desktop enablement via the `production-support` package values). All stages are operator actions against real Azure with human approval; none run from local gates, per instruction.

## Change groups

- Contracts: (Task 1) — no schema changes; Rust consumes existing canonical `ModelAccessRequest`/`ModelContextConsent` bindings.
- Support API: (Tasks 1–2+) —
- Desktop (Rust): (Task 1+) — `desktop-cloud`: new `support_contract` module + exports (`CanonicalProjectionInputs`, bindings, signer trait); `model.rs` gained `into_transport_parts`; `desktop-runtime` re-exports unchanged surface.
- Infrastructure: —
- CI: —
