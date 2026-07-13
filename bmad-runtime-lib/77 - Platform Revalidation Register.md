---
title: "Platform Revalidation Register"
aliases:
  - "77 - Platform Revalidation Register"
tags:
  - bmad-runtime
  - vault/audit-and-validation
section: "Audit and Validation"
order: 77
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: v6-platform-revalidation-register
status: current
validated_on: 2026-07-09
---



# Platform Revalidation Register

## V6.17 additional revalidation subjects

Revalidate Tauri runtime authority/capability semantics, Windows/WebView2 support matrix, MSI/NSIS/update signing, WAM/OAuth public-client behavior, DPAPI/profile roaming/key loss, SQLite/filesystem durability, NTFS file-ID/reparse/hardlink behavior, AppContainer/restricted-token/broker compatibility with supported toolchains, Job Object limits, Entra Conditional Access, Model Access retention/residency, Application Insights privacy controls, and enterprise deployment channels.

Record tested OS build, packaging identity, filesystem/toolchain, Tauri/Rust/WebView2 versions, policy/profile version, date, evidence link, and architecture impact. A documentation claim alone cannot close DESK-01.

This register records what changed or was newly verified after V5.

## 1. Changes from V5

| Area | V5 position | V6 correction | Reason |
|---|---|---|---|
| .NET runtime | ASP.NET Core stated, no locked current LTS version. | Lock new API code to .NET 10 LTS. | .NET 10 is active LTS; .NET 8/9 remain supported until Nov 10, 2026 but are too short-lived for new API code. |
| Frontend runtime | React/TypeScript stated, no current version lock or delivery framework decision. | Pin React 19.2.7, Vite 8.1.0, React Router 8.0.1 SPA, Node 24.18.0, pnpm 11.4.0; make TypeScript 7.0 the gated app compiler with isolated TS6 compiler-API tooling only where required. | TypeScript 7.0 is GA but ships without a public programmatic API. The product has no established SSR/BFF requirement, so a client SPA is the locked narrow fit. |
| Python workers | Python workers stated, no current version lock. | Pin Python 3.14.6 and uv 0.11.21 after per-image dependency validation; fallback 3.13 only through a scoped worker profile. | Python 3.14 is stable, but wheel/native compatibility remains workload-specific. |
| Azure OpenAI integration | Structured outputs validated, but API generation, retention, and hosted-tool posture were not explicit enough. | Prefer Foundry/OpenAI v1 + Responses behind Model Gateway with `store=false`, app-owned state, provider schema projection, and hosted tools off by default. | Responses and structured outputs are useful generation capabilities, not lifecycle, authorization, or tool-governance boundaries. |
| Agent hosting | External agent services not clearly positioned. | Foundry Agent Service remains deferred/spike, not v1 orchestrator. | Product requires custom Airlock, workspace, execution, and evidence semantics. |
| IaC | Bicep stated, no AVM posture or delivery ordering. | Pin Bicep in CI and exact AVM module versions behind wrappers; provision the minimum Azure identity/storage/telemetry foundation before real provider and ACA integration. | AVM is useful, but exact versions, restored artifacts, defaults, identity, network, and diagnostic controls remain project responsibilities. |
| Aspire | Not clearly positioned. | Local orchestration candidate only; not production IaC source of truth by default. | Good DevEx tool, but production deployment semantics must remain explicit. |

## 2. Validation status by claim type

| Claim type | V6 handling |
|---|---|
| Official current version | Cite official source, lock only stable/LTS/GA versions. |
| Preview/RC/beta feature | Mark `SPIKE_REQUIRED` or `DEFERRED`; never use as release gate dependency. |
| Project architecture rule | Mark `ARCHITECTURE_DECISION`; no external citation required, but ADR required to reverse. |
| Source-derived BMAD rule | Mark `PROJECT_SOURCE`; preserve original source context. |
| Implementation rule | Mark `IMPLEMENTATION_SPEC`; needs tests. |

## 3. V6 release blocker checks

Before first implementation sprint starts, confirm:

- `.NET 10` SDK installs and builds the Runtime API skeleton in CI.
- `Node 24.18.0 + pnpm 11.4.0 + React 19.2.7 + Vite 8.1.0 + React Router 8.0.1` builds the workbench skeleton; any SSR/BFF server addition requires a new ADR.
- `TypeScript 7.0.0` passes generated-client, `tsc --build`, lint, test, declaration, Vite, and editor/LSP gates; any TS 6 programmatic-API sidecar has an exact pin, owner, and retirement condition.
- `Python 3.14.6 + uv 0.11.21` can build each base worker image, resolve required wheels/native libraries, and run the command manifest fixture with locked sync.
- Azure Container Apps Jobs can be started through a custom start-only dispatcher identity using a fixed digest-pinned template; attempts to override image, command, environment, or secret bindings fail.
- Dynamic Sessions are benchmarked only after the job baseline exists.
- OpenAPI 3.1.2 is the one canonical contract and produces TypeScript and C# clients with compile-time and compatibility tests; OpenAPI 3.2 is a .NET 11/tooling watch item.
- Azure OpenAI structured-output limits are tested with realistic patch/proposal schemas; provider projections and canonical schemas have separate hashes and full server validation.
- Responses calls default to `store=false`, provider response IDs are not lifecycle authority, and provider-hosted tools cannot execute outside Tool Registry, Airlock, approval, and evidence controls.
- The Model Gateway can switch between direct model, model router candidate, and fallback profiles without changing Agent Kernel code.
- The reviewed Bicep foundation creates Entra registrations/scopes, managed identities, Key Vault RBAC, ACR pull/push separation, SQL/Blob identity access, Foundry profile bindings, and Application Insights before real provider/job integration.
- Server telemetry reaches Azure Monitor through OpenTelemetry while browser telemetry uses the Application Insights JavaScript SDK; SQL/Blob evidence remains complete under trace sampling.

## 4. Consolidated source-review revalidation additions

Source: [[89 - Consolidated AI Workspace Source Review and Architecture Improvements]].

Add these checks to the first implementation readiness review:

| Check | Why it matters |
|---|---|
| TypeScript 7 dual-toolchain evidence | The compiler line changed substantially and 7.0 has no public programmatic API; application compilation and any isolated TS 6 tooling sidecar must be independently reproducible. |
| React SPA boundary | Vite 8 + React Router 8 SPA is locked with compatibility gates; revalidate Node/React/Vite/router/auth/hosting/testing support, and require a new ADR only if SSR/BFF needs emerge. |
| Dynamic Sessions benchmark | Platform availability is not enough; Sapphirus needs manifest protocol, network controls, isolation, latency, cost, and region evidence. |
| Responses/Foundry provider adapter fixture | Prove `store=false`, app-owned context, capability probing, refusal/incomplete handling, schema projection, and hosted-tools-off behavior without leaking SDK objects. |
| ACA fixed-template negative test | Starting a job can replace its template; the application must prove that untrusted requests cannot change image, command, environment, or secret context. |
| AVM module review | Exact module versions and restored artifacts are required; hidden defaults can still conflict with identity, egress, private networking, and observability requirements. |
| Azure foundation ordering | Real Foundry and ACA integration must use managed identities, Key Vault, ACR, storage, and telemetry provisioned by reviewed Bicep rather than temporary production-shaped secrets. |
| No-container workflow | Direct local toolchains/fakes plus ACR Tasks or hosted CI and fixed ACA Jobs must pass from clean Windows hardware without Docker/Kubernetes/emulators/local model serving. |
| Self-hosted/local profile review | Local/private deployment does not relax auth, owner scope, Airlock, egress policy, or worker isolation. |
