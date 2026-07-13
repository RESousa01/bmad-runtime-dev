---
title: "Technology, Language, Method, and LLM Implementation Review"
aliases:
  - "Technology and LLM Review"
  - "V6.16 Architecture Review"
tags:
  - bmad-runtime
  - vault/architecture-review
  - azure
  - llm
section: "Review and Decisions"
order: 91
status: v6.16-reviewed
reviewed_on: 2026-07-09
project: Sapphirus BMAD Runtime
---

# Technology, Language, Method, and LLM Implementation Review

## V6.18 BMAD foundation scope correction

The stack verdict remains unchanged. [[100 - BMAD Method and Builder Deep Comprehension Audit]] supersedes this note only for Method/Builder semantic depth: BMAD Method is prompt-native rather than one executable DSL; Builder contracts and inactive drafts are early foundation work; execution/evaluation/promotion remain gated; Convert is not a current upstream Builder capability; memory/autonomous agents require separate storage/scheduler/containment contracts.

## V6.17 scope correction

This V6.16 review remains the technology authority for `web_managed`: React/TypeScript, .NET/ASP.NET Core, Azure, hosted model access, cloud workspaces, remote builds, and fixed isolated executors. It is superseded only where it previously implied one delivery model for the whole product.

The separate Windows decision is Tauri 2 + React/TypeScript + Rust, selected-folder authority, SQLite/encrypted local payloads, and local approved effects, with Azure limited to support-plane capabilities. Shared model/BMAD/schema principles remain intact. See [[93 - Split Web and Windows Desktop Architecture Plans]] through [[99 - Dual-Delivery Contract and Conformance Specification]].

## Executive verdict

The architecture is viable after four corrections:

1. BMAD Method and BMAD Builder are the foundation for product method, artifacts, help, authoring, and evaluation—not for identity, authorization, execution, or durable runtime state.
2. The product is cloud-first. It requires no local Docker, Kubernetes, infrastructure emulator, or local model server. Local work proves contracts with trusted deterministic fakes; the first real isolated effect is an Azure Container Apps Job.
3. The LLM layer is a typed, evaluated Model Gateway, not an autonomous provider SDK embedded throughout the product. Sapphirus owns state, schemas, policy, and evidence.
4. Durable evidence and work recovery are domain data. Telemetry, model response chains, in-memory event buffers, and worker claims are projections or inputs—not authority.

The selected stack is coherent for Azure and the stated hardware constraint. The remaining risk is implementation discipline, not a missing framework.

## Review inputs and confidence

| Input | Use in this review | Confidence boundary |
|---|---|---|
| BMAD Method 6.10.0 and BMAD Builder package 2.1.0/module 1.0.0 snapshots | Selected foundation inputs; detailed semantic authority is [[100 - BMAD Method and Builder Deep Comprehension Audit]] | Archive hashes are known; upstream immutable ref is still required before redistribution/release promotion. |
| Complete OpenClaw archive | Patterns for approval binding, task recovery, plugin contracts, audit/release practice | Research input only; archive has no Git commit/tag identity and bundled licenses are component-scoped. |
| Complete Hermes archive | Patterns and failure cases for turn lifecycle, providers, memory, skills, automation, and budgets | Research input only; archive has no Git commit/tag identity and one bundled PowerPoint skill has restrictive terms and is excluded. |
| Available Odysseus source | Product-shell and workflow requirements | Broad but not provenance-complete; AGPL-3.0-or-later means clean-room pattern adoption by default. |
| Context7 library context | Cross-check of TypeScript, React, ASP.NET Core, Vite, and React Router APIs/current guidance | Context7 metadata lagged the newest official TypeScript and React Router announcements; official primary sources win when they conflict. |
| Official vendor documentation | Current platform/lifecycle/API validation | Revalidate exact versions, Azure regional availability, quota, and preview status at each release gate. |

Context7 library ids consulted: `/microsoft/typescript`, `/react/react/v19.2.7`, `/dotnet/aspnetcore/v10.0.1`, `/vitejs/vite/v8.0.10`, and `/remix-run/react-router/react-router_7.9.4`.

## Authority model

| Layer | Owns | Must not own |
|---|---|---|
| BMAD Method | Workflow, skill, artifact, help/advice, configuration layering, and method lineage | User authorization, policy decisions, secrets, cloud dispatch, or lifecycle SQL |
| BMAD Builder | Governed agent/workflow/module/skill authoring and evaluation inputs | Direct activation or self-approval |
| Runtime API | Authoritative domain lifecycle, owner scope, idempotency, transactions, outbox, Evidence Ledger, and projections | Provider-specific response objects or worker execution internals |
| Model Gateway | Provider/deployment resolution, schema projection, model profiles, budgets, redaction, typed failures, and evaluation-gated routing | Proposal ownership, Airlock decisions, or durable conversation authority at the provider |
| Airlock | Exact candidate policy evaluation and single-use execution authority after any required human approval | Command execution or ordinary application CRUD |
| Azure worker lane | Execute one approved finite attempt and emit bounded result/log/artifact claims | Lifecycle SQL mutation, policy reinterpretation, or dynamic image/entrypoint selection |

## Technology and language validation

| Area | Decision | Why it fits | Required gate |
|---|---|---|---|
| API/control plane | .NET 10 LTS and ASP.NET Core 10 modular monolith | Strong Entra/managed identity, SQL transactions, OpenAPI, policy, observability, and Azure operations fit | Pin an exact supported SDK feature band/runtime patch and test Azure/library compatibility. [.NET support policy](https://dotnet.microsoft.com/en-us/platform/support/policy/dotnet-core) |
| Frontend | React 19.2.7, Vite 8.1, React Router 8 SPA | The ASP.NET control plane is already authoritative; no proven SSR/BFF/SEO need justifies another server | Generated client, route/data, typecheck, accessibility, reconnect, and bundle gates. [React 19.2](https://react.dev/blog/2025/10/01/react-19-2), [Vite 8](https://vite.dev/blog/announcing-vite8), [React Router changelog](https://reactrouter.com/changelog) |
| Web language | TypeScript 7 application compiler | Current high-performance compiler and correct long-term target | TypeScript 7 currently lacks the established public compiler API used by some tools; keep a clearly isolated TypeScript 6 compatibility package only for those tools until the TS7 API/toolchain gate passes. [TypeScript 7 announcement](https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/) |
| JS runtime/packages | Node 24 LTS and pnpm 11 | Supported build runtime with deterministic workspace/lockfile behavior | Exact `engines`/`packageManager`, frozen install, registry policy, provenance, and hosted-CI clean install. [Node releases](https://nodejs.org/en/about/previous-releases) |
| Worker/import tools | Python per image; Python 3.14 preferred after compatibility proof; uv locked | Fits BMAD utilities, document/render tooling, and finite workers without moving domain authority out of .NET | Each worker declares its own runtime/dependencies, lock hash, licenses, SBOM, tests, and fallback; no global Python assumption. [Python 3.14](https://docs.python.org/3.14/whatsnew/3.14.html), [uv projects](https://docs.astral.sh/uv/guides/projects/) |
| API/schema | OpenAPI 3.1.2 and JSON Schema 2020-12 | Matches ASP.NET Core 10's supported first-party generation path and Structured Output contract needs | One canonical schema; OpenAPI 3.2 is a future .NET/tooling migration gate. [ASP.NET Core OpenAPI](https://learn.microsoft.com/en-us/aspnet/core/fundamentals/openapi/aspnetcore-openapi?view=aspnetcore-10.0), [OpenAPI specifications](https://spec.openapis.org/oas/latest.html) |
| Database/storage | Azure SQL for compact lifecycle authority; Blob for immutable/large payloads | Clear transactions/outbox plus scalable artifact/evidence payload storage | Owner scope, schema migrations, content hashes, retention/deletion, backup/restore, and reconciliation tests |
| IaC | Bicep plus pinned Azure Verified Modules where useful | Native Azure what-if/deployment and readable environment boundaries | Exact module versions, reviewed defaults, role-assignment tests, policy checks, drift/rollback evidence |
| Observability | Server OpenTelemetry plus Azure Monitor; browser Application Insights JS instrumentation | Open standards on the server and appropriate browser telemetry | Telemetry is redacted and non-authoritative; sampling/drop cannot change Evidence Ledger truth |

Supporting files such as SQL, Bicep, YAML, JSON, JSON Schema, TOML, Markdown, and PowerShell are configuration, contracts, documentation, or operations assets. They are not additional business-logic runtimes. PowerShell may bootstrap or diagnose Windows/Azure workflows but must not become a second orchestration engine.

## Cloud-first development profile

```text
Pinned local .NET/Node/Python tools
  -> unit + contract + schema + policy + deterministic replay tests
  -> trusted fake model/worker and temporary sealed fixture (no isolation claim)
  -> hosted CI or ACR Tasks remote image build
  -> immutable ACR digest + scan/license/SBOM/provenance evidence
  -> fixed-template ACA Job in the Azure dev environment
  -> WebWorkerResultManifest import
  -> atomic domain transition + EvidenceLedgerEvent + outbox
  -> user/operator EvidenceBundle
```

Rules:

- `az acr build` or a hosted runner builds images; no local daemon is required. [ACR Tasks overview](https://learn.microsoft.com/en-us/azure/container-registry/container-registry-tasks-overview)
- `sealed_test_fake` has no shell, network, dependency restore, package import, or arbitrary command surface.
- Azure foundations arrive before their consumers: Entra scopes/identities, Key Vault, SQL, Blob, ACR, monitoring, Container Apps environment, and a disabled/fixed job template.
- ACA Jobs own finite real execution. Dynamic Sessions and Container Apps Sandboxes remain measured future spikes, not baseline dependencies. [Container Apps Jobs](https://learn.microsoft.com/en-us/azure/container-apps/jobs), [Dynamic Sessions](https://learn.microsoft.com/en-us/azure/container-apps/sessions), [Container Apps Sandboxes](https://learn.microsoft.com/en-us/azure/container-apps/sandboxes-overview)
- Runtime requests cannot override the job image, entrypoint, identity, secrets, network profile, or arbitrary environment. The dispatcher starts an allowlisted digest-pinned template.

## LLM implementation plan

### Call contract

Every call carries:

- call type, owner/run/turn, purpose, data/retention class, budget, timeout, and idempotency key;
- versioned `ModelProfile` role alias mapped to an exact Azure deployment/model snapshot and settings;
- exact `ProviderCapabilities` for deployment, region, API, schema/tool/retention support, and credential class;
- trusted instruction hashes, untrusted context-pack hash/refs, tool-availability hash, canonical schema hash, projected provider schema hash, and redaction summary;
- typed outcome: success, refusal, incomplete, content-policy block, rate limit, timeout, capability mismatch, schema failure, budget block, provider failure, or cancellation.

Provider SDK objects and response-history state stop at the adapter. The Orchestrator receives typed values only.

### Responses API policy

- Use the Azure/OpenAI v1 Responses shape where it fits, but keep Sapphirus run/conversation state authoritative.
- Send `store=false` by default. Provider-side storage/background execution is not the recovery path and is incompatible with some zero-data-retention requirements. [OpenAI Responses migration guide](https://developers.openai.com/api/docs/guides/migrate-to-responses), [Azure Responses API](https://learn.microsoft.com/en-us/azure/foundry/openai/how-to/responses)
- Disable provider-hosted tools in v1. Web/file/computer/code tools require separate Airlock, egress, credential, evidence, and approval adapters before adoption.
- Treat Structured Outputs as provider assistance, not final validation. Project into the supported subset, retain both hashes, handle refusals/incomplete output, and validate the final value against the canonical server schema. [OpenAI Structured Outputs](https://developers.openai.com/api/docs/guides/structured-outputs)
- Prompt caching is an optimization. Cache miss/compaction/truncation cannot alter policy, state, tool availability, or schema semantics.

### Model profiles and promotion

Application code uses roles such as `planner`, `schema_repair`, `context_compressor`, and `artifact_reviewer`; it never hard-codes a marketing “latest” alias.

Promotion is:

`candidate -> offline_evaluated -> policy_approved -> canary -> active -> rolled_back|retired`

An immutable evaluation bundle contains:

| Lane | Minimum evidence |
|---|---|
| Contract | Schema success, refusal/incomplete handling, canonical validation, deterministic retry/repair cap |
| Task quality | BMAD artifact completeness, plan correctness, grounded evidence, patch/review quality, reviewer independence |
| Safety/privacy | Prompt injection, tool-intent abuse, PII/secret leakage, owner isolation, retention and credential policy |
| Operations | Latency, cost, quota/rate limit, cache behavior, regional availability, replay, canary and rollback |

A critical lane cannot be hidden by averaging. The candidate model cannot approve itself. Fallback must be evaluated in advance and stops when it would cross provider, credential, residency, retention, hosted-tool, schema, or material quality boundaries.

### Context and memory

- Build deterministic `ContextPack` objects from immutable workspace/BMAD artifact references with owner scope, source/evidence refs, trust labels, redaction, token budget, and freshness hashes.
- Start with lexical/metadata retrieval and measured truncation. Add embeddings/vector search only after a relevance, privacy, deletion, cost, and re-indexing evaluation proves value.
- Files, package text, web content, tool output, model summaries, and recalled memory remain untrusted context.
- Promote memory only from accepted, finalized BMAD/runtime state through a durable proposal/outbox; failed turns and model-written summaries cannot silently become authoritative memory.

## Corrected implementation sequence

| Phase | Outcome | Release boundary |
|---|---|---|
| -1 Governance | One bounded work packet, owner/non-owner, risks, stop/rollback path, tests, context ledger | No code before ownership and proof plan are explicit |
| 0 Contracts/foundation | Source/license intake, both BMAD profiles, canonical schemas, owner/trust, durable work/evidence, provider/eval seams, exact toolchains | Compiles and deterministic fixtures pass |
| 1 Trusted local proof | BMAD-native UI/state/approval/result/evidence replay through fakes only | Clearly labeled simulated; no untrusted execution or isolation claim |
| 2 Security + Azure readiness | Owner scope, principals, egress, secrets, Entra/MI, SQL/Blob/Key Vault/ACR/monitoring/ACA foundation, remote build | Reproducible cost-capped dev environment and fixed disabled job template |
| 3 Real model | Azure Responses adapter, schema projection, exact capabilities/credentials, evaluation/canary/rollback | Typed proposal candidates; still no real effect |
| 4 Real finite execution | Fixed ACA Job, WebWorkerResultManifest, attempt/outbox recovery, Evidence Ledger | First real governed isolated side effect; internal alpha boundary |
| 5 Arbitrary BMAD packages | Component license/trust, static scan, isolated install/invocation rehearsal, reversible activation | Package code never trusted by discovery alone |
| 6A Artifact adapter | Existing presentation/artifact behavior through core contracts | No parallel runtime |
| 6B Builder | Governed Builder proposals, variants, evaluation, exact rehearsal/approval/activation | Generated content inactive until every gate passes |
| 7 Operations | Kill switches, degraded-state views, provider/eval/package/job/release evidence | Operators can diagnose and disable without redeploy |
| 8 Release | IaC hardening, migration/backup/restore, security/load/cost/rollback, fresh-cloud smoke | Evidence-backed candidate |

## Source-derived adoption decision

| Source | Adopt | Do not inherit |
|---|---|---|
| BMAD Method | Method workflows, artifacts, help, configuration and lineage semantics | Runtime authorization/execution assumptions not present in BMAD |
| BMAD Builder | Authoring, validation and evaluation workflow concepts | Direct activation, “clean directory equals sandbox,” or unpinned script/runtime assumptions |
| OpenClaw | Exact-argument approval-binding ideas, plugin contracts, task recovery shapes, context lifecycle, release evidence patterns | Hosted-unsafe defaults, in-process untrusted plugins, generic approval-as-enforcement, in-memory replay authority, whole-snapshot task persistence |
| Hermes | Cache-aware provider adaptation, narrow-core extension thinking, lifecycle/automation test cases | Partial turn commits, URL-substring credential routing, raw env secret fallbacks, mutable plugin installs, failed-turn memory promotion, non-aggregate budgets |
| Odysseus | Product shell, plans, artifacts, admin/setup and workflow UX requirements | Source reuse without AGPL approval, unsandboxed shell, mutable artifact versions, non-durable replay, permissive CI |

## Go/no-go gates

Do not begin real provider integration until source/license, owner/trust, schema, durable work/evidence, identity, retention, and Azure foundation gates pass.

Do not begin real execution until remote-build provenance, fixed-job integrity, Airlock exact-candidate semantics, attempt/outbox recovery, manifest binding, and least-privilege identities pass.

Do not import arbitrary BMAD/Builder packages until the Phase-4 Azure lane can scan and rehearse the exact digest/lock without giving package code control-plane credentials.

Do not call the product production-ready while model/profile fallback, package deactivation, worker/job kill switches, backup/restore, deletion/retention, and no-Docker clean-machine deployment remain untested.

## Related implementation notes

- [[02 - Locked Architecture Decisions]]
- [[18 - Model Gateway and Microsoft Foundry]]
- [[36 - Local Development and DevEx]]
- [[37 - Azure Environments and Deployment Runbooks]]
- [[89 - Consolidated AI Workspace Source Review and Architecture Improvements]]
- [[90 - LLM-Tailored Development Plan and Agent Workflow]]
- [[92 - Source Snapshot Verification and Adoption Ledger]]
