---
title: "External Platform References and Verification Sources"
aliases:
  - "60 - External Platform References and Verification Sources"
tags:
  - bmad-runtime
  - vault/source-and-research
section: "Source and Research"
order: 60
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: v6-external-platform-references
status: current
validated_on: 2026-07-09
---



# External Platform References and Verification Sources

## V6.17 Windows and Tauri primary references

- [Tauri runtime authority](https://v2.tauri.app/security/runtime-authority/) — renderer/core authority boundary and IPC trust.
- [Tauri command scopes](https://v2.tauri.app/security/scope/) — scope validation for command arguments.
- [Tauri capabilities](https://v2.tauri.app/security/capabilities/) and [permissions](https://v2.tauri.app/security/permissions/) — webview/window capability configuration.
- [Microsoft identity platform authorization code flow with PKCE](https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow) — native/public-client sign-in baseline.
- [Windows OAuth2Manager](https://learn.microsoft.com/en-us/windows/apps/develop/security/oauth2) — WAM-backed Windows sign-in option subject to D0 proof.
- [Windows app capability declarations](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/app-capability-declarations) — packaged capability model.
- [CreateRestrictedToken](https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-createrestrictedtoken) and [launch an AppContainer process](https://learn.microsoft.com/en-us/windows/win32/secauthz/appcontainer-for-legacy-applications-) — containment alternatives requiring empirical toolchain compatibility evidence.
- [Windows Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects) — process grouping, limits, lifetime, and accounting; not filesystem/network isolation.
- [CryptProtectData](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata) — user-scoped protection for local key material.

Revalidate exact OS/API/Tauri versions, packaging identity, enterprise policy behavior, and model-service retention before each release. References inform the design; release claims require tests on the supported Windows matrix.

This file tracks platform and standards claims that influence implementation choices. It is not a marketing bibliography; every reference below exists because it affects an architecture decision, release gate, or spike.

## 1. Runtime and Azure execution

| Area | Current validated claim | Decision in this library | Source |
|---|---|---|---|
| Azure Container Apps Jobs | Jobs run containerized tasks for finite duration and then stop. A start request can replace the execution template, including image, command, and environment. | Use fixed digest-pinned templates for v1 workers. Give the dispatcher a custom start-only role and reject request-time template overrides. | https://learn.microsoft.com/en-us/azure/container-apps/jobs |
| Azure Container Registry Tasks | ACR Tasks performs cloud container builds, including on-demand `az acr build`, without a local Docker installation, and pushes results into the registry. | Make ACR Tasks or hosted CI the supported image-build path for the user's hardware. Bind immutable source/definition/locks to license/scan/SBOM/provenance/signature/digest evidence. | https://learn.microsoft.com/en-us/azure/container-registry/container-registry-tasks-overview and https://learn.microsoft.com/en-us/azure/container-registry/container-registry-quickstart-task-cli |
| Azure Container Apps Sandboxes | Sandboxes are a newer managed isolation capability and remain preview/evidence-sensitive by region and use case. | Keep as a future spike. Fixed ACA Jobs remain the first real execution lane; no preview sandbox is required for v1. | https://learn.microsoft.com/en-us/azure/container-apps/sandboxes-overview |
| ACA environments | Environment is the secure boundary around container apps and jobs. | Keep Runtime API and jobs inside controlled ACA environment; isolate workspaces through job/session policy. | https://learn.microsoft.com/en-us/azure/container-apps/environment |
| ACA Dynamic Sessions | Session pools provide prewarmed isolated sessions useful for interactive/user-generated code. | Phase-0/v1.5 spike, not v1 baseline. | https://learn.microsoft.com/en-us/azure/container-apps/sessions |
| App Service auth | App Service has built-in auth/authorization and Microsoft Entra provider support. | Use for user-facing web auth entry; Runtime API still owns project RBAC. | https://learn.microsoft.com/en-us/azure/app-service/overview-authentication-authorization |
| Azure SQL | Azure SQL Database is the managed relational platform for SQL lifecycle state. | Keep compact lifecycle state in SQL; logs/artifacts/manifests stay in Blob. | https://learn.microsoft.com/en-us/azure/azure-sql/database/ |
| Azure SQL security | Newly created Azure SQL databases are encrypted by default. | Encryption is baseline, not sufficient; add RBAC, private networking, auditing, retention. | https://learn.microsoft.com/en-us/azure/azure-sql/database/security-overview |
| Azure SQL Microsoft Entra auth | Azure SQL supports contained database principals for managed identities. | Production Runtime API uses managed identity by default; use separate least-privilege runtime and migration principals and give workers no SQL principal. | https://learn.microsoft.com/en-us/azure/azure-sql/database/authentication-aad-configure |
| Key Vault RBAC | Azure RBAC is the recommended data-plane authorization model, with roles normally assigned at vault scope. | Use one RBAC-mode vault per app/environment, managed identities, purge protection, and separate signing/key permissions from secret reads. | https://learn.microsoft.com/en-us/azure/key-vault/general/rbac-guide |
| Azure Container Registry identity | ACA can pull ACR images with managed identity; repository-scoped roles are available for ABAC-enabled registries. | Runtime identities are pull-only, CI publishing identity is separate, and releases deploy immutable image digests rather than tags. | https://learn.microsoft.com/en-us/azure/container-registry/container-registry-authentication-managed-identity |
| ACR private access | Private endpoints require the Premium tier and change hosted-CI connectivity when public access is disabled. | Resolve registry tier, private networking, and self-hosted/dedicated CI access in an ADR before disabling the public endpoint. | https://learn.microsoft.com/en-us/azure/container-registry/container-registry-private-link |
| Bicep | Bicep is Azure's declarative DSL for repeatable resource deployment. | Bicep remains IaC source of truth for v1; pin the CLI used by CI, run build/lint/what-if, and exclude experimental features without an ADR. | https://learn.microsoft.com/en-us/azure/azure-resource-manager/bicep/overview |
| Azure Verified Modules | AVM modules are Microsoft prebuilt/pretested modules aligned with best practices. | Use exact AVM module versions behind project wrapper modules; review defaults, identity, private networking, diagnostics, and restored artifact digest. | https://learn.microsoft.com/en-us/azure/azure-resource-manager/bicep/modules |
| Azure Developer CLI | `azd` accelerates provisioning/deployment workflows. | Use for developer environment orchestration if it reduces friction; do not let it bypass IaC review. | https://learn.microsoft.com/en-us/azure/developer/azure-developer-cli/overview |

## 2. Application stack

| Area | Current validated claim | Decision in this library | Source |
|---|---|---|---|
| .NET | .NET 10 is active LTS through November 14, 2028; the validated patch is runtime 10.0.9 with SDK 10.0.301 on 2026-07-09. Supported systems must remain current on patches. | Pin SDK 10.0.301 in `global.json`, pin runtime images by digest, and automate monthly patch review. | https://dotnet.microsoft.com/en-us/platform/support/policy/dotnet-core and https://dotnet.microsoft.com/en-us/download/dotnet/10.0 |
| React | React 19.2.7 is the latest listed 19.2 patch on the validation date. | Pin React and React DOM 19.2.7 for the workbench; approvals and lifecycle state remain server-authoritative. | https://react.dev/versions |
| Vite | Vite 8 is stable; Vite 8.1 is the current supported minor and uses Rolldown. | Recommend a Vite 8.1.0 client-SPA baseline in the frontend ADR; validate plugins and TypeScript 7 before locking. | https://vite.dev/blog/announcing-vite8-1 and https://vite.dev/releases |
| React Router | React Router 8.0.1 is stable and requires React 19.2.7+, Node 22.22.0+, and Vite 7+. | Recommend React Router 8.0.1 for client routing in the SPA ADR; introduce a server framework only for a proven SSR/BFF requirement. | https://reactrouter.com/changelog |
| Node.js | Node.js 24.18.0 is the validated LTS patch and the release line receives updates through April 2028. | Pin Node.js 24.18.0 for web tooling and CI; update deliberately when a newer supported patch is verified. | https://nodejs.org/en/blog/release/v24.18.0 and https://nodejs.org/en/about/previous-releases |
| pnpm | pnpm 11 requires Node.js 22+; pnpm 11.4 is current on the validation date and CI uses frozen lockfile behavior. | Pin `packageManager` to `pnpm@11.4.0`, commit the matching lockfile, and run `pnpm install --frozen-lockfile`. | https://github.com/pnpm/pnpm/releases and https://pnpm.io/continuous-integration |
| TypeScript | TypeScript 7.0 reached GA on July 8, 2026, but 7.0 does not ship a public programmatic API; compatibility packages remain necessary for some ecosystem tools. | Treat `typescript@7.0.0` as a conditional CLI baseline. Keep an isolated, exact TS 6 compatibility alias only for tools that require the old API, and remove it when the migration gate passes. | https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/ |
| Python | Python 3.14.6 is the current 3.14 maintenance release on the validation date; the series receives security updates until about Oct 2030. | Pin Python 3.14.6 per worker image after wheel/native-dependency validation. Python 3.13 is an explicit per-worker profile, not an automatic global fallback. | https://www.python.org/downloads/release/python-3146/ and https://peps.python.org/pep-0745/ |
| uv | uv provides locked project sync; 0.11.21 is the validated release. | Pin uv 0.11.21 or its image/binary digest, verify release provenance, and require `uv lock --check` plus `uv sync --locked` in CI. | https://docs.astral.sh/uv/concepts/projects/sync/ and https://docs.astral.sh/uv/guides/integration/docker/ |

## 3. AI/model platform

| Area | Current validated claim | Decision in this library | Source |
|---|---|---|---|
| Microsoft Foundry | Foundry is the Azure platform for AI operations, model builders, and app development. | Keep AI platform abstraction behind Model Gateway. | https://learn.microsoft.com/en-us/azure/foundry/what-is-foundry |
| Foundry Models | Foundry provides a broad, evolving multi-provider model catalog. | Model Gateway must resolve deployment/profile aliases and record the resolved provider, model/version, region, and API mode; never hard-code a volatile catalog count or `latest` model. | https://learn.microsoft.com/en-us/azure/foundry/concepts/foundry-models-overview |
| OpenAI v1 API in Foundry | v1 API removes dated api-version parameters and supports cross-provider model calls. | Prefer v1-compatible integration where available; keep provider abstraction. | https://learn.microsoft.com/en-us/azure/foundry/openai/api-version-lifecycle |
| Responses API | Azure OpenAI Responses supports multi-turn responses, streaming, and tools; stored response data has provider retention implications and background mode requires stored state. | Use Responses behind Model Gateway with `store=false` by default and app-owned conversation/run state. Defer background mode, and disable provider-hosted tools unless they pass Tool Registry, Airlock, and evidence controls. | https://learn.microsoft.com/en-us/azure/foundry/openai/how-to/responses and https://developers.openai.com/api/docs/guides/background |
| JSON mode | JSON mode guarantees valid JSON, not schema conformance. | Never use JSON mode alone for proposals. | https://learn.microsoft.com/en-us/azure/foundry/openai/how-to/json-mode |
| Structured outputs | Structured outputs follow a constrained JSON Schema subset with required fields, closed objects, and nesting/property limits. | Compile a provider-compatible projection, record canonical/projection hashes, and validate returned data against the full canonical schema plus domain and policy invariants. | https://learn.microsoft.com/en-us/azure/foundry/openai/how-to/structured-outputs |
| Model Router | Model Router routes prompts to a suitable LLM in real time. | Spike for cost/latency routing; not a v1 correctness dependency. | https://learn.microsoft.com/en-us/azure/foundry/openai/concepts/model-router |
| Foundry Agent Service | Managed agent hosting exists; classic agents are deprecated and new service exists. | Do not replace custom Orchestrator in v1; evaluate later for hosted-agent deployment/evals. | https://learn.microsoft.com/en-us/azure/foundry/agents/overview |

## 4. API, observability, and supply chain

| Area | Current validated claim | Decision in this library | Source |
|---|---|---|---|
| OpenAPI | OAS 3.1.2 and 3.2.0 were both published on September 19, 2025. ASP.NET Core on .NET 10 generates OpenAPI 3.1; .NET 11 moves to 3.2. | Use OpenAPI 3.1.2 as the single v1 canonical contract. Keep 3.2 on the .NET 11/generator compatibility watchlist. | https://spec.openapis.org/oas/v3.1.2.html, https://spec.openapis.org/oas/v3.2.0.html, and https://learn.microsoft.com/en-us/aspnet/core/fundamentals/openapi/aspnetcore-openapi?view=aspnetcore-10.0 |
| OpenTelemetry | Semantic conventions standardize telemetry attributes, while generative-AI conventions continue to evolve. | Use stable internal `sapphirus.*` attributes and version-pinned mappings for API/model/job/artifact correlation; never put raw prompts, outputs, or tool arguments in span attributes. | https://opentelemetry.io/docs/specs/semconv/ and https://opentelemetry.io/docs/specs/semconv/gen-ai/ |
| Azure Monitor OTel | Azure Monitor OpenTelemetry Distro supports Application Insights collection for server runtimes. | Use the Azure Monitor distro in the .NET API and workers; configure sampling explicitly and keep audit/evidence in SQL/Blob rather than sampled telemetry. | https://learn.microsoft.com/en-us/azure/azure-monitor/app/opentelemetry-enable |
| Browser monitoring | Azure's supported browser path is the Application Insights JavaScript SDK rather than OpenTelemetry. | Instrument the React SPA with the Application Insights JavaScript SDK and keep its public connection string/resource boundary separate from server authentication and audit evidence. | https://learn.microsoft.com/en-us/azure/azure-monitor/app/javascript-sdk |
| OWASP LLM Top 10 | OWASP publishes LLM-specific risk categories including prompt injection, insecure output handling, supply chain, excessive agency, and sensitive information disclosure. | Threat model every model/tool/workspace boundary against OWASP LLM risks. | https://owasp.org/www-project-top-10-for-large-language-model-applications/ |
| SLSA provenance | SLSA provenance describes where, when, and how an artifact was produced. | Require provenance for worker images and exported release artifacts. | https://slsa.dev/provenance/v1 |
| CycloneDX | CycloneDX is a full-stack Bill of Materials standard. | Use CycloneDX for SBOM where tooling supports it. | https://cyclonedx.org/ |

## 5. V6 validation rules

1. A platform claim must not be used to justify a v1 feature unless it has a source above or a new source in `76 - Current Stack Baseline.md`.
2. Preview/RC/beta features are never `LOCKED` unless the project explicitly accepts preview risk in an ADR.
3. Provider features are wrapped by internal contracts; no UI, orchestrator, or worker code should depend directly on provider response objects.
4. Any platform decision that affects security, identity, execution, storage, or cost must have a release gate and an owner.

## 6. Consolidated source-review revalidation notes

Source: [[89 - Consolidated AI Workspace Source Review and Architecture Improvements]].

Revalidated on 2026-07-09:

| Area | Resulting plan rule |
|---|---|
| .NET 10 LTS | Stays locked for new Runtime API work; monthly patch currency is part of support posture. |
| Node 24 LTS | Stays locked for the web toolchain until Node 26 enters LTS and dependency compatibility is proven. |
| TypeScript 7 | Conditional CLI baseline: requires generated-client, Vite/React Router, lint, declaration, and editor/LSP gates; isolate TS 6 only where a tool requires its API. |
| Python 3.14 | Stays target worker runtime; Python 3.13 fallback requires scoped compatibility evidence. |
| React 19.2 | Stays web baseline with Vite 8 + React Router 8 SPA mode; a new server framework requires an ADR proving SSR/BFF needs. |
| OpenAPI 3.1.2 | Becomes the single canonical v1 contract; OpenAPI 3.2 remains a .NET 11/tooling watch item. |
| ACA Jobs | Stays v1 finite worker baseline with fixed digest-pinned templates, start-only dispatcher identity, and no request-time template overrides. |
| ACA Dynamic Sessions | Remains spike until benchmarked against Sapphirus worker protocol and security needs. |
| AVM | Approved only with exact module pins, restored artifact evidence, and default/security review. |
| Responses API / Foundry v1 | Gateway-owned provider capability; `store=false`, app-owned state, provider schema projection, and hosted tools off are the v1 defaults. |
