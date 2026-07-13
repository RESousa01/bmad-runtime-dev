---
title: "Modern Engineering Methods"
aliases:
  - "80 - Modern Engineering Methods"
tags:
  - bmad-runtime
  - vault/audit-and-validation
section: "Audit and Validation"
order: 80
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: v6-modern-engineering-methods
status: current
validated_on: 2026-07-09
---



# Modern Engineering Methods

## V6.17 engineering method by authority

Use contract-first/domain-driven/TDD/replay methods across both products, then tailor reliability and security evidence to the authority. Shared code is limited to pure schemas, canonicalization utilities where behavior is demonstrably identical, BMAD fixtures, rule data, and UI primitives. Prefer ports at delivery boundaries, but do not abstract two effect authorities behind a fallback-capable “universal executor.”

Web uses transactional outbox, immutable object payloads, lease/idempotent import, IaC/canary, and isolated-worker tests. Desktop uses capability-based local APIs, typestate/closed enums, property-based path/canonicalization tests, crash/fault injection, journal recovery, signed supply chain, privacy-by-default payload inspection, and supported-matrix containment testing.

This file defines implementation methods that should be used across the project. These are not line-count additions; they are operating rules for the coding agents and developers implementing the runtime.

## 1. Architecture methods

| Method | How to apply it here | Anti-pattern it prevents |
|---|---|---|
| Modular monolith with hard ports | Keep modules in one ASP.NET Core process but force interactions through interfaces like `IRunStateStore`, `IAirlockPolicy`, `IExecutionDispatcher`, `ITraceWriter`. | God service with hidden cross-module writes. |
| Transactional state machine | Every run/proposal/approval/job transition has a legal previous state, actor, event, and idempotency key. | Partial failures corrupting lifecycle state. |
| Outbox-style event publication | SQL transaction records lifecycle change; event streamer publishes compact event from committed state. | UI showing events for uncommitted state. |
| Manifest import pattern | Worker writes immutable Blob manifest; Runtime API validates/imports it. | Worker directly mutating SQL lifecycle state. |
| Policy object capability | Side-effect APIs accept only `ApprovedExecutionSpec`, never raw proposal. | Accidental Airlock bypass. |
| Contract-first API | OpenAPI 3.1.2 is the one canonical v1 contract; generated clients and contract tests guard all durable APIs. OpenAPI 3.2 adoption waits for .NET 11 and full toolchain evidence. | Frontend/API drift and dual contract truth. |
| React client-SPA ADR | Recommend Vite 8 + React Router 8 for the authenticated workbench; introduce a server React framework only for a documented SSR/BFF requirement. | Accidental framework and hosting complexity. |
| Schema-first AI output | Model output is typed. Model Gateway compiles a provider-supported schema projection, records canonical/projection hashes, and the server validates the full canonical schema and semantic invariants before Agent Kernel creates proposals. | Provider schema limits weakening domain contracts or Model Gateway becoming policy/orchestration layer. |

## 2. Security methods

- Treat workspace files, BMAD packages, generated artifacts, command output, and logs as untrusted.
- Redact before model calls and before trace persistence.
- Canonicalize all paths and reject symlink escape attempts.
- Use `argv[]` command specs; disallow `sh -c` unless a policy exception exists.
- Pin worker image digests in `ApprovedExecutionSpec`.
- Start ACA Jobs only through a custom start-only dispatcher identity and fixed digest-pinned template; never accept request-time image, command, environment, or secret-binding overrides.
- Store secrets in an RBAC-mode Key Vault per app/environment; workers receive only scoped, time-bound credentials and no broad vault-reader role.
- Use user delegation SAS or managed identity where possible instead of account-key style access.
- Use managed identity for production SQL, Blob, ACR pull, Key Vault, and Foundry access. Keep database migration and CI image-push identities separate from runtime identities.
- Use Responses with `store=false` and application-owned context/run state by default.
- Keep provider-hosted web, code, computer, and MCP tools disabled unless each capability is registered, Airlock-governed, isolated, and evidenced like an internal tool.
- Record policy hash and approval hash in every side-effect manifest.

## 3. Observability methods

- Instrument the .NET API and workers with OpenTelemetry and export through Azure Monitor/Application Insights.
- Instrument the React browser with the Application Insights JavaScript SDK; do not claim browser OpenTelemetry as the supported Azure path.
- Define stable internal `sapphirus.*` model/run/tool attributes and map to a version-pinned generative-AI semantic convention only where useful.
- Never put raw prompts, model outputs, retrieved documents, secrets, or tool arguments in span attributes.
- Configure sampling deliberately. Audit events, approvals, manifests, policy decisions, and evidence remain authoritative in SQL/Blob and must survive telemetry sampling or outage.

## 4. Testing methods

| Layer | Minimum required tests |
|---|---|
| Runtime API | state-machine tests, idempotency tests, authorization tests, contract tests. |
| Airlock | allow/deny fixtures, bypass tests, policy hash tests, approval expiry tests. |
| Worker | manifest schema tests, no-SQL-credential test, command DSL fixtures, output redaction tests. |
| Workspace | preimage drift, stale proposal voiding, rollback, path/symlink attacks. |
| Model Gateway | provider-schema projection, canonical validation failures, refusal/incomplete outcomes, `store=false`, app-owned state, hosted-tools-off, retry/backoff, capability-safe fallback, token/cost accounting. |
| ACA dispatcher | custom-role authorization, fixed-template dispatch, and negative tests for image/command/environment/secret override. |
| Frontend | approval UX, diff rendering, log streaming, optimistic UI rollback, accessibility. |
| Replay | golden vertical-slice fixture, failure fixture, repair fixture, presentation adapter fixture. |

## 5. Infrastructure and delivery methods

1. Build the smallest trusted simulated vertical slice first, then cross the fixed ACA boundary before calling it a real executable/internal-alpha slice.
2. Before real Foundry or ACA integration, provision the minimum dev foundation through reviewed Bicep: Entra registrations/scopes, managed identities, Key Vault RBAC, ACR role separation, SQL/Blob identity access, Foundry profile binding, and Application Insights.
3. Keep the supported developer path container-free: direct pinned toolchains and deterministic `sealed_test_fake` adapters, ACR Tasks/hosted CI for immutable remote builds, and fixed ACA Jobs for real web effects. `sealed_test_fake` never proves isolation.
4. Pin the Bicep CLI and every AVM module to an exact version; keep AVM behind project wrapper modules and review defaults, policy, identity, networking, diagnostics, and restored artifact provenance.
5. Treat `what-if` as review evidence, not proof of correctness; run Bicep build/lint, policy checks, deployment smoke tests, and identity negative tests.
6. Every story ships with one API test, one policy/state assertion, and one evidence artifact check.
7. Do not add a new UI mode unless it produces or consumes a real Runtime API contract.
8. Do not add a new worker capability unless it has a manifest schema and Airlock rule.
9. Do not add a new model call type unless it has canonical and provider-projection schemas, capability/retention settings, and retry/error policy.
10. Every spike ends in either a locked decision, deferred decision, or removed option.
