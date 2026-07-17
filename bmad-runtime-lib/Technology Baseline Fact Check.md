---
title: "Technology Baseline Fact Check"
aliases:
  - "Research Result"
  - "Fact Check - Technology Baseline"
tags:
  - bmad-runtime
  - vault/source-and-research
  - vault/audit-and-validation
section: "Source and Research"
order: 2
vault_role: "research-summary"
project: Sapphirus BMAD Runtime
status: legacy-reference
validated_on: 2026-07-09
source_file: "C:/Users/rodrigocsousa/Downloads/compass_artifact_wf-8c0d551b-935b-4042-94c5-151ddf638398_text_markdown.md"
---

# Technology Baseline Fact Check

This note summarizes the attached research result and records the changes applied to the vault.

## V6.17 dual-delivery validation addendum

The original fact check remains the web/toolchain baseline. The Windows architecture adds a separate, source-validated technology surface: Tauri 2 runtime authority/capabilities, React/TypeScript in WebView2, stable Rust native host, Windows public-client Entra authentication using WAM or system-browser authorization code + PKCE subject to D0 proof, user-scoped DPAPI key protection, SQLite/encrypted local payloads, signed MSI/NSIS/updater delivery, and Win32 structured process launch.

Two claims are explicitly bounded by evidence rather than documentation alone: Job Objects manage process trees/resources but do not confine filesystem or network access; and arbitrary developer-tool confinement to selected folders requires an empirically compatible AppContainer/restricted-token/broker design. Files [[60 - External Platform References and Verification Sources]], [[73 - Verification Register]], [[77 - Platform Revalidation Register]], and [[97 - Windows Desktop Security and Trust Model]] track the proof obligations.

## Research Verdict

The research checked 27 technology/platform claims in the Sapphirus BMAD Runtime baseline:

- 24 claims were accurate.
- 2 claims were outdated on the document date.
- 1 claim was accurate on 2026-07-09 but scheduled to change within months.

The counts above describe the attached research result. A second official-source architecture validation on the same date refined several implementation decisions where a fact was technically current but would create dual contracts, floating toolchains, provider-owned state, or an incomplete security posture.

## Corrections Applied

| Area | Previous guidance | Corrected vault guidance |
|---|---|---|
| TypeScript | TypeScript 6.x was treated as current and TypeScript 7.0 as RC/not GA. | TypeScript 7.0.0 is a conditional application CLI baseline. It reached GA on 2026-07-08 but 7.0 has no public programmatic API; generated clients, lint, Vite, declarations, CI, and editor/LSP must pass. Keep TS 6 only as an exact, isolated tooling sidecar with an owner and retirement condition. |
| OpenAPI | OpenAPI 3.1.1 was treated as the newest/current spec, then 3.2 was proposed as canonical despite .NET 10 generating 3.1. | OpenAPI 3.1.2 is the one canonical v1 contract. OpenAPI 3.2 remains a published-current standard and a .NET 11/generator watch item, not a parallel source of truth. |
| Node.js 26 | Node 26 was described as current but not yet LTS. | Pin Node 24.18.0 LTS for v1. Node 26 is scheduled for Active LTS on 2026-10-28 and still needs dependency audit before adoption. |
| .NET 8/9 | Some wording could imply they had already ended support. | They remain supported until 2026-11-10, but the Runtime API baseline is .NET 10 LTS, currently runtime 10.0.9/SDK 10.0.301 and supported through 2028-11-14. |
| Reproducible pins | Several files used floating `10.x`, `24.x`, `pnpm@11.x`, `3.14.x`, or unversioned uv/AVM guidance. | Pin the validated toolchain: .NET SDK 10.0.301, Node 24.18.0, pnpm 11.4.0, React 19.2.7, TypeScript 7.0.0 after its gate, Python 3.14.6, and uv 0.11.21. Pin images by digest and AVM modules by exact semantic version. |
| React delivery gap | React was selected without a bundler/router or a demonstrated SSR/BFF requirement. | Lock a React 19.2.7 client SPA using Vite 8.1.0 and React Router 8.0.1 behind compatibility/build gates. Add Next.js or another server framework only if a new ADR proves SSR/BFF needs. |
| Responses API | Responses was treated as a generally stateful/tool-capable API without an explicit retention and hosted-tool default. | Use `store=false`, application-owned conversation/run state, and capability profiles. Defer background mode and disable provider-hosted tools unless they pass Tool Registry, Airlock, isolation, approval, and evidence gates. |
| Structured outputs | Provider schema following could be read as sufficient validation. | Compile a provider-supported schema projection, record both schema hashes, and validate against the full canonical schema plus semantic and policy invariants. |
| ACA Jobs | Finite-job fit was validated without accounting for start-time template replacement. | Use fixed digest-pinned templates, a custom start-only dispatcher identity, and negative tests that reject image, command, environment, and secret-binding overrides. |
| Observability | OpenTelemetry wording covered browser and server uniformly. | Use OpenTelemetry/Azure Monitor for API and workers, the Application Insights JavaScript SDK for the browser, and SQL/Blob rather than sampled telemetry for audit/evidence. |
| Azure foundation | Bicep/AVM and managed Azure services were selected but first provisioning could occur after real provider/job integration. | Provision Entra scopes, managed identities, Key Vault RBAC, ACR role separation, SQL/Blob identity access, Foundry binding, and Application Insights through reviewed Bicep before real model/job integration. |
| Hardware/development mismatch | Earlier local guidance could imply devcontainers, Docker, storage/database emulators, or local model serving. | The supported path requires none of them: use direct pinned toolchains and deterministic sealed fakes locally, ACR Tasks (`az acr build`) or hosted CI for remote immutable image builds, and fixed ACA Jobs for the first real isolated execution. |

## Notes Updated

- [[02 - Locked Architecture Decisions|Locked Architecture Decisions]]
- [[11 - Runtime API Control Plane|Runtime API Control Plane]]
- [[25 - OpenAPI, Schemas, and Generated Clients|OpenAPI, Schemas, and Generated Clients]]
- [[26 - Frontend Design System|Frontend Design System]]
- [[58 - Risk Register and Mitigation Plan|Risk Register and Mitigation Plan]]
- [[60 - External Platform References and Verification Sources|External Platform References and Verification Sources]]
- [[66 - Frontend Component Specification|Frontend Component Specification]]
- [[73 - Verification Register|Verification Register]]
- [[76 - Current Stack Baseline|Current Stack Baseline]]
- [[77 - Platform Revalidation Register|Platform Revalidation Register]]
- [[78 - Deprecation and Preview Watchlist|Deprecation and Preview Watchlist]]
- [[79 - Corrections Applied|Corrections Applied]]
- [[80 - Modern Engineering Methods|Modern Engineering Methods]]
- [[81 - Modernization Spike Backlog|Modernization Spike Backlog]]
- [[82 - Current Technology Decision Summary|Current Technology Decision Summary]]
- [[Library Quality Report|Library Quality Report]]

## Source Links

- TypeScript 7.0 GA: https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/
- React versions: https://react.dev/versions
- Vite 8.1: https://vite.dev/blog/announcing-vite8-1
- React Router 8 changelog: https://reactrouter.com/changelog
- OpenAPI 3.1.2 specification: https://spec.openapis.org/oas/v3.1.2.html
- OpenAPI 3.2.0 specification: https://spec.openapis.org/oas/v3.2.0.html
- ASP.NET Core 10 OpenAPI generation: https://learn.microsoft.com/en-us/aspnet/core/fundamentals/openapi/aspnetcore-openapi?view=aspnetcore-10.0
- Node.js release schedule: https://github.com/nodejs/release
- Node.js 24.18.0 release: https://nodejs.org/en/blog/release/v24.18.0
- .NET 10 downloads and SDK/runtime versions: https://dotnet.microsoft.com/en-us/download/dotnet/10.0
- Python 3.14.6: https://www.python.org/downloads/release/python-3146/
- uv locked projects: https://docs.astral.sh/uv/concepts/projects/sync/
- Azure Responses API: https://learn.microsoft.com/en-us/azure/foundry/openai/how-to/responses
- Azure structured outputs: https://learn.microsoft.com/en-us/azure/foundry/openai/how-to/structured-outputs
- Azure Container Apps Jobs: https://learn.microsoft.com/en-us/azure/container-apps/jobs
- Azure Monitor OpenTelemetry: https://learn.microsoft.com/en-us/azure/azure-monitor/app/opentelemetry-enable
- Application Insights JavaScript SDK: https://learn.microsoft.com/en-us/azure/azure-monitor/app/javascript-sdk
- Key Vault RBAC: https://learn.microsoft.com/en-us/azure/key-vault/general/rbac-guide
- ACR managed identity: https://learn.microsoft.com/en-us/azure/container-registry/container-registry-authentication-managed-identity
- ACR Tasks remote build: https://learn.microsoft.com/en-us/azure/container-registry/container-registry-tasks-overview
- Bicep modules and AVM: https://learn.microsoft.com/en-us/azure/azure-resource-manager/bicep/modules

## Follow-Up Rule

When a platform fact changes, update this note and the linked baseline notes together. Do not leave a newer claim in one note and an older claim in another. Exact point-version examples are the validated pins for 2026-07-09; dependency automation may advance them only through a reviewed lockfile/digest change with compatibility and security evidence.
