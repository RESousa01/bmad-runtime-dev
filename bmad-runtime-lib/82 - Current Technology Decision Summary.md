---
title: "Current Technology Decision Summary"
aliases:
  - "82 - Current Technology Decision Summary"
tags:
  - bmad-runtime
  - vault/audit-and-validation
section: "Audit and Validation"
order: 82
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: v6-current-tech-decision-summary
status: legacy-reference
validated_on: 2026-07-09
---



# Current Technology Decision Summary

## V6.17 architecture verdict

Build both delivery models, but as two products over a shared governed contract foundation. The web product remains React/TypeScript + ASP.NET Core/.NET + Entra + Azure AI Foundry/OpenAI + Azure SQL/Blob/Key Vault/Application Insights + optional/fixed Container Apps Jobs with cloud-managed workspaces and isolated remote execution.

The Windows product is React/TypeScript in Tauri/WebView2 with a Rust native authority, user-selected folders, SQLite/encrypted local payloads, local checkpoint/journal/rollback, and approval-gated local patch/command execution. Azure supplies identity, licensing, Model Access, packages, optional sync/collaboration, telemetry, secrets/service configuration, and explicit remote jobs—not ordinary local file edits.

The shared layer is BMAD semantics, Airlock vocabulary/rules, canonical JSON schemas, model profiles, UI primitives, fixtures, and conformance. Do not share workspace/state/spec/executor authority. No local Docker, Kubernetes, self-hosted server, local model, or GPU is required. DESK-01 remains the explicit decision about how strongly arbitrary child tools can be confined.

## Build with this now

| Layer | Use | Do not use as default |
|---|---|---|
| API | .NET 10 LTS with validated runtime 10.0.9/SDK 10.0.301, ASP.NET Core, compatible EF Core, OpenAPI 3.1.2 as the single canonical contract | .NET 8/9 for new API code, dual OpenAPI 3.2/3.1 truth |
| Web | React 19.2.7, Vite 8.1.0, React Router 8.0.1 SPA, Node 24.18.0, pnpm 11.4.0; TypeScript 7.0.0 application compiler after the gate with isolated TS6 compiler-API tooling only where required | React 18, floating tools, unconditional TS7 ecosystem assumptions, or a server React framework without an SSR/BFF ADR |
| Workers | Per-image Python/uv profile; Python 3.14.6 + uv 0.11.21 preferred after workload compatibility; locked dependencies and remotely built digest-pinned images | Raw pip drift, automatic global fallback, unpinned base/uv, or a single assumed Python version for every BMAD/worker script |
| Development/build | Direct local toolchains and trusted deterministic fakes; ACR Tasks/hosted CI remote image builds | Local Docker/Kubernetes/emulators/model server, untrusted local execution, or developer-local image tags |
| Azure compute | ACA Runtime API + fixed-template ACA Jobs workers; App Service or static hosting chosen by the SPA auth/hosting ADR | AKS without an ADR, request-time ACA image/command/environment overrides |
| Fast execution | Benchmark Dynamic Sessions after ACA Jobs baseline | Dynamic Sessions as untested default |
| AI | Microsoft Foundry/OpenAI v1 and Responses behind Model Gateway; `store=false`, app-owned state, provider-schema projection, full canonical validation, hosted tools off | JSON mode alone, provider-stored state as authority, ungoverned hosted tools, provider objects leaking into orchestrator |
| Observability | OpenTelemetry + Azure Monitor for API/workers; Application Insights JavaScript SDK for browser; SQL/Blob for audit/evidence | Browser OTel as the Azure-supported path, raw prompt/tool data in spans, sampled telemetry as evidence |
| IaC | Pin Bicep, use exact AVM versions behind wrappers, use azd only for developer workflow; provision the minimum Azure foundation before real provider/jobs | Floating AVM tags, portal-click deployments, Aspire as unreviewed production IaC owner, real integration built on temporary secrets |
| Identity/secrets/images | Managed identities, Key Vault RBAC, separate migration and CI-push identities, ACR pull-only runtime roles, digest-pinned deployments | SQL connection-string secrets by default, ACR admin credentials, vault-wide worker access |
| Security | OWASP LLM Top 10, Airlock, command DSL, SLSA provenance, CycloneDX SBOM, image signing/digest pinning | Trusting model output or worker logs without validation |

## The central V6 architecture remains unchanged

The modernization pass does **not** change the core product design:

```text
BMAD Method / chat / typed model output
→ Runtime Proposal + immutable ExecutionSpecCandidate
→ policy + exact candidate-hash approval when required
→ audience-bound expiring single-use ApprovedExecutionSpec
→ fixed ACA worker/job for real effects
→ WebWorkerResultManifest/log/artifact output
→ Runtime atomically imports completion/lifecycle/Evidence Ledger/outbox
→ EvidenceBundle and rollback point
```

The update only makes the implementation stack more current and removes ambiguous older defaults.

## Consolidated Source-Review Technology Corrections

Source: [[89 - Consolidated AI Workspace Source Review and Architecture Improvements]].

The technology plan remains current, but these corrections are now part of the decision:

| Area | Correction |
|---|---|
| TypeScript 7 | Conditional application CLI target: release requires generated-client, lint, declaration, Vite/React Router, CI, and editor/LSP evidence because 7.0 has no public programmatic API. Keep TS 6 only as an exact isolated tooling sidecar. |
| React 19.2 | Approved with Vite 8 + React Router 8 SPA mode; runtime state, approvals, and evidence remain server-authoritative. |
| ACA Dynamic Sessions | Strong candidate for interactive execution, but not v1 default until benchmark and security evidence pass. |
| OpenAI/Foundry APIs | Responses/v1 features are capability-profile inputs, not orchestrator contracts. `store=false`, app-owned state, schema projection, and hosted tools off are defaults; background mode is deferred. |
| OpenAPI | OpenAPI 3.1.2 is canonical for v1; 3.2 is re-evaluated with .NET 11 and complete generator/validator/client evidence. |
| ACA Jobs | Fixed digest-pinned templates and a start-only dispatcher identity are mandatory; request-time image, command, environment, and secret-binding overrides are denied. |
| AVM | Use exact versions behind wrappers; module defaults and restored artifacts must be reviewed against identity, egress, private networking, observability, and policy requirements. |
| Azure foundation order | Entra scopes, managed identities, Key Vault RBAC, ACR roles, SQL/Blob access, Foundry binding, and Application Insights are provisioned through Bicep before real model or job integration. |
| Hardware/development profile | No local Docker/Kubernetes/emulator/model server is required. `sealed_test_fake` is non-isolating; ACR remote builds and fixed ACA Jobs supply web packaging/isolation, while the signed Rust host supplies approved desktop-local execution. |
