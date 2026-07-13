---
title: "Modernization Spike Backlog"
aliases:
  - "81 - Modernization Spike Backlog"
tags:
  - bmad-runtime
  - vault/audit-and-validation
section: "Audit and Validation"
order: 81
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: v6-modernization-spike-backlog
status: current
validated_on: 2026-07-09
---



# Modernization Spike Backlog

## V6.17 Windows D0 spike set

| ID | Spike | Exit evidence |
|---|---|---|
| DESK-01 | Child-process filesystem/network containment | Supported-toolchain matrix for local-user, restricted-token, AppContainer/broker tiers; product claim/go-no-go |
| DESK-02 | Entra public-client authentication | WAM versus system-browser PKCE evidence under Conditional Access/offline/sign-out scenarios |
| DESK-03 | Installer/update channel | MSI/WiX, NSIS, WebView2, update key rotation, rings, rollback, Intune/winget proof |
| DESK-04 | Local database encryption/recovery | DPAPI/key hierarchy, SQLite/encrypted-CAS or SQLCipher decision, corruption/key-loss/export/uninstall tests |
| DESK-05 | Executable identity binding | Path/file-ID/hash/signature performance and invalidation behavior across supported toolchain updates |
| DESK-06 | Model context egress consent | Per-call versus time-bounded grant, preview/redaction/size/retention and server payload inspection |
| DESK-07 | Sync scope/encryption | Metadata-only versus selected evidence/artifacts, conflict policy, encryption and source-off proof |
| DESK-08 | Offline entitlement | Lease/grace, revocation, clock-tamper and permitted deterministic offline operation matrix |
| DESK-09 | Workspace metadata location | App-local default versus explicit `.sapphirus/` opt-in, ignore and migration behavior |
| DESK-10 | Git integration | Read-only MVP versus later commit creation; auto-push remains excluded |
| D0-FS-01 | Selected-folder NTFS authority | File-ID/root identity/reparse/hardlink/device/ADS/race suite and support-tier policy |
| D0-JOURNAL-01 | Journaled patch/checkpoint durability | Power-loss/crash injection at every ordering point with deterministic recovery disposition |
| D0-IPC-01 | Tauri IPC least privilege | Generated allowlist/capability audit and renderer-compromise negative tests |
| D0-PKG-01 | Package compatibility/cache | Signature/revocation/offline grace/cross-language fixture proof |
| D0-HANDOFF-01 | Remote job handoff | Exact upload, separate cloud work, non-applicable output, fresh local approval proof |

These spikes decide whether modern capabilities should be adopted now, later, or not at all.

## 1. Execution latency spike

**Question:** Are ACA Jobs good enough for interactive patch/test loops, or do Dynamic Sessions materially improve UX?

This is a post-Phase-4 optimization spike. The fixed ACA Job lane and no-container remote-build workflow must already pass; Dynamic Sessions cannot be used to postpone the first real execution boundary.

**Protocol:**

1. Build one worker image through ACR Tasks/hosted CI with immutable source/lock/license/scan/SBOM/provenance/digest evidence; do not require local Docker.
2. Run 50 cold ACA Job executions.
3. Run 50 warm/repeated executions where platform cache may help.
4. Run equivalent Dynamic Session requests if available in target region.
5. Measure dispatch-to-first-log, total runtime, failure rate, setup complexity, cost.
6. Dispatch ACA Jobs through a custom start-only identity and a fixed digest-pinned template.
7. Attempt to override the image, command, environment, and secret bindings from the Runtime API request; all attempts must fail before platform dispatch.

**Decision:**

- keep ACA Jobs if p95 first-log latency is acceptable for approved jobs and the fixed-template/least-privilege negative tests pass;
- add Dynamic Sessions only for low-latency read/plan/quick-validate operations if isolation and cost are acceptable.

## 2. Structured-output schema limits spike

**Question:** How large/complex can patch, command, and plan schemas be before model/API limits degrade reliability?

**Protocol:**

1. Test minimal proposal schema.
2. Test full patch proposal schema.
3. Test multi-file patch schema.
4. Compile each canonical schema into the deployed provider's supported subset, including required fields, closed objects, supported keywords, property count, and nesting depth.
5. Record canonical and provider-projection hashes, then validate model output against the full canonical schema and semantic/domain invariants.
6. Test refusal, incomplete output, output-limit, unsupported schema, and invalid model output branches.
7. Record latency, failure mode, retry behavior, repair strategy, and any staged-call boundary.

**Decision:**

- keep structured outputs as a generation constraint for supported model proposal types, never as the sole validator or policy boundary;
- split schemas into staged model calls when provider limits are exceeded rather than weakening canonical contracts.

## 3. Model Router spike

**Question:** Can Foundry Model Router reduce cost/latency without hurting plan/patch quality?

**Protocol:**

1. Compare fixed model profile vs router profile on 20 replay tasks.
2. Score schema validity, correctness, token cost, latency, repair rate.
3. Confirm fallback behavior and observability metadata.

**Decision:**

- allow router only behind Model Gateway profile, never inside Agent Kernel.

## 4. TypeScript 7 compatibility benchmark

**Question:** TypeScript 7.0 is GA but ships without a public programmatic API; which generated-client, lint, editor, and CI tools still require an isolated TS 6 compatibility toolchain?

**Protocol:**

1. Pin Node 24.18.0, pnpm 11.4.0, React 19.2.7, and `typescript@7.0.0`; run `pnpm install --frozen-lockfile` and `pnpm typecheck`.
2. Run OpenAPI-generated client, `tsc --build`, lint, test, declaration-output, Vite, React Router, and editor language-service checks.
3. Verify generated clients compile from the single OpenAPI 3.1.2 canonical contract and match the .NET 10 runtime contract tests.
4. If a tool requires the TypeScript programmatic API, isolate an exact TS 6 compatibility alias so application compilation remains on TS 7; record its owner and retirement condition.
5. Check Windows/Linux CI and editor compatibility without relying on a globally installed compiler.

**Decision:**

- adopt TypeScript 7 as the application CLI baseline only if generated clients, lint, build, declarations, Vite/React Router, and IDE tooling pass without workaround debt;
- retain TS 6 only as a documented, exact, tool-local sidecar until the dependent tool supports the TypeScript 7 API.

## 5. Foundry Agent Service spike

**Question:** Can managed hosted agents help later without weakening custom Airlock/run-state/evidence semantics?

**Protocol:**

1. Build a toy hosted agent that calls Model Gateway-compatible code.
2. Verify whether custom approval/evidence/workspace policies remain app-owned.
3. Compare with custom Run Orchestrator complexity.

**Decision:**

- defer unless it reduces ops burden while preserving all governance semantics.

## 6. React SPA delivery ADR spike

**Question:** Does the authenticated workbench need SSR, React Server Components, or a backend-for-frontend, or is a client SPA the smaller and safer v1 delivery model?

**Recommended candidate:** React 19.2.7 + Vite 8.1.0 + React Router 8.0.1 on Node 24.18.0 and pnpm 11.4.0.

**Protocol:**

1. Build the chat, run timeline, approval, diff, and artifact routes as a generated-client-only SPA.
2. Validate Entra sign-in, API scope/audience handling, deep-link refresh, route-level authorization UX, CSP, and static asset hosting.
3. Verify TypeScript 7, accessibility, browser tests, production build, source-map policy, and Application Insights JavaScript SDK integration.
4. Compare App Service same-origin/Easy Auth and static-host-plus-JWT postures.
5. Document any concrete SSR, SEO, BFF, server action, or edge-rendering requirement that would justify Next.js or another server framework.

**Decision:**

- retain the locked Vite 8 + React Router 8 client SPA unless a measured SSR/BFF requirement produces a superseding ADR;
- keep runtime lifecycle, approvals, authorization, and evidence server-authoritative regardless of frontend hosting.

## 7. Responses retention and capability spike

**Question:** Which Responses/v1 fields and modes are supported by each approved Foundry deployment without giving the provider ownership of Sapphirus state or tools?

**Protocol:**

1. Run calls with `store=false` and prove application-owned transcript, context pack, run events, and replay work without provider state.
2. Record a capability profile per deployment for streaming, structured outputs, prompt caching, truncation, parallel tool calls, and output limits; do not infer parity from another provider/model.
3. Exercise refusal, incomplete output, disconnect/resume, timeout, and retry branches.
4. Verify provider response IDs and opaque compaction are correlation metadata only, never lifecycle or audit authority.
5. Verify provider-hosted web, code, computer, and MCP tools are disabled. Any exception must enter Tool Registry and pass Airlock, approval, isolation, egress, and evidence tests.
6. Evaluate background mode separately because it requires provider-stored state and an explicit retention classification.

**Decision:**

- keep Responses as the default inference adapter with `store=false` and app-owned state;
- defer background mode and every hosted tool until its retention/governance spike passes.

## 8. Azure integration foundation spike

**Question:** Can the first real Foundry and ACA integration run entirely through reviewed Bicep and managed identities, without production-shaped connection-string, registry-admin, or vault-wide credentials?

**Protocol:**

1. Pin the Bicep CLI and exact AVM versions; wrap modules with required naming, identity, private-network, diagnostic, and policy parameters.
2. Provision environment-specific Entra registrations/scopes, runtime and dispatcher identities, RBAC-mode Key Vault, ACR, SQL, Blob, Foundry profile binding, and Application Insights before enabling real model/job calls.
3. Give Runtime API least-privilege SQL/Blob/Foundry access, the dispatcher only the custom job-start action, workers no SQL role, ACR pull only to runtimes, and ACR push only to CI.
4. Deploy images by digest and verify provenance/signature/SBOM gates.
5. Run Bicep build/lint/what-if, deployment smoke tests, and negative identity tests. Treat `what-if` as review evidence rather than proof.
6. Decide ACR Premium/private endpoint and CI network access explicitly before disabling public access.

**Decision:**

- real provider/job phases cannot start until this minimum dev foundation passes;
- later infrastructure work becomes production networking, scale, restore, resilience, and disaster-recovery hardening.

## 9. Browser/server observability split spike

**Question:** Can browser and server telemetry correlate a run without leaking model/workspace content or treating sampled telemetry as evidence?

**Protocol:**

1. Instrument .NET and workers with OpenTelemetry/Azure Monitor and the React browser with the Application Insights JavaScript SDK.
2. Propagate `traceparent` and an allowlisted correlation context; use stable `sapphirus.*` attributes and a version-pinned GenAI mapping only where useful.
3. Verify raw prompts, outputs, retrieved content, tool arguments, and secrets never appear in span attributes.
4. Enable sampling and a simulated telemetry outage; SQL/Blob approvals, manifests, audit events, and evidence must remain complete.

**Decision:**

- lock server OTel plus browser Application Insights JavaScript SDK;
- keep observability diagnostic and keep evidence authoritative in SQL/Blob.
