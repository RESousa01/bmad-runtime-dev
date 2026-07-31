---
title: "Consolidated AI Workspace Source Review and Architecture Improvements"
aliases:
  - "AI Workspace Source Synthesis"
  - "Consolidated Source Review"
  - "Architecture Improvement Synthesis"
tags:
  - bmad-runtime
  - vault/source-and-research
section: "Source and Research"
order: 89
status: source-evidence
reviewed_on: 2026-07-09
source_reviews:
  - "83 - BMAD Source Code Review - Method and Builder"
  - "84 - OpenClaw Source Review - Comparable Runtime Patterns"
  - "85 - OpenClaw Structured Code Review"
  - "86 - Hermes Source Code Review - Agent Runtime and Learning Loop"
  - "87 - Hermes Deep Review - Extension Runtime and Operational Contracts"
  - "88 - Odysseus Source Code Review - Self-Hosted AI Workspace"
---

# Consolidated AI Workspace Source Review and Architecture Improvements

## V6.17 adoption boundary

The source-derived improvements in this note are now adoption candidates for three targets: shared semantics/fixtures, `web_managed` .NET/Azure, or `windows_local` Tauri/Rust. No comparable-runtime pattern is adopted into both effect authorities merely because its interface is similar.

Current decisions supersede any single-runtime or self-hosted implication: cloud workspace/remote isolation remain the web default; selected local folders/local execution remain the desktop default; Azure is desktop support plane; and cross-delivery work is an explicit non-applying handoff.

## Purpose

This note consolidates what was learned from the reviewed AI apps, agent runtimes, and workspaces and converts it into improvements for the Sapphirus BMAD Runtime plan.

It is intentionally opinionated: the reviewed systems are useful precisely because they show where AI workspaces become fragile as they grow. The correct plan is not to copy any one system. The correct plan is to keep the Sapphirus architecture narrow, governed, contract-first, and auditable while borrowing the hard-earned operational patterns from each source.

## Reviewed Sources

| Source review | Main lesson |
|---|---|
| [[83 - BMAD Source Code Review - Method and Builder]] | BMAD package semantics, installer artifacts, manifests, module/help registries, Builder output shapes, and validation fixtures must be first-class rather than improvised. |
| [[84 - OpenClaw Source Review - Comparable Runtime Patterns]] | Exact `system.run` approval binding is worth adapting, while optional sandbox defaults, in-process plugins, bounded replay, and generic plugin approval metadata define the limits of the reference. |
| [[85 - OpenClaw Structured Code Review]] | Extension ecosystems need protocol schemas, provenance, durable work/evidence, package proposal queues, and machine-verifiable release gates; manifest breadth and maturity scores are not containment or promotion evidence. |
| [[86 - Hermes Source Code Review - Agent Runtime and Learning Loop]] | Prompt/tool/context stability, atomic turn commit, accepted-only memory promotion, exact credential binding, durable claims, aggregate budgets, and pinned extensions are core runtime contracts. |
| [[87 - Hermes Deep Review - Extension Runtime and Operational Contracts]] | Provider resolution, profile-scoped secrets, editor sessions, connector delivery, task claims, dashboard auth, drain state, and verification evidence need durable schemas. |
| [[88 - Odysseus Source Code Review - Self-Hosted AI Workspace]] | Self-hosted AI workspaces need owner scoping, internal loopback identity, SSRF/DNS-pinned egress, upload confinement, adaptive context, task chains, local provider probes, and degraded-state UX. |

## V6.16 Foundation-First Audit Verdict

BMAD Method and BMAD Builder are the product foundation, not late extensions. The comparable runtimes inform the operational substrate beneath that foundation; they do not define the Workspace's method, package, workflow, artifact, or Builder authority.

| Layer | Canonical authority | Source influence |
|---|---|---|
| Workspace experience | Chat, projects, artifacts, source/evidence views, Builder and operator surfaces. | Odysseus supplies clean-room shell/UX lessons; OpenClaw supplies protocol and diagnostic lessons. |
| BMAD product kernel | Method phases, workflows, skills, capability/help graph, artifact expectations, and project method state. | BMAD Method is authoritative. |
| Builder quality plane | Authoring, conversion, analysis, module scaffolding, eval definitions, rehearsal, and draft-to-active lifecycle. | BMAD Builder is authoritative; Sapphirus adds isolation, policy, evidence, and activation governance. |
| Agent runtime/control plane | Runs, turns, context lifecycle, model routing, tool normalization, detached work, claims, and outbox delivery. | Hermes and OpenClaw contribute selected contracts, not dependencies. |
| Governance/execution plane | Owner scope, policy, approvals, exact specs, isolation, manifests, evidence, checkpoints, rollback, and retention. | Sapphirus is authoritative; all reviewed runtimes supply cautionary patterns. |

The practical consequence is a two-depth implementation: Phase 0/1 implement a narrow sealed BMAD seam, while the full arbitrary package loader and visual Builder Studio remain later phases. This keeps the first slice small without making BMAD an add-on.

### Source Intake And Adoption Ledger

| Source snapshot | Authoritative local identity and verification | License/adoption decision |
|---|---|---|
| BMAD Method | `bmad-method` `6.10.0`; the Method ZIP digest is recorded in its dedicated review, but the archive contains no Git commit/ref/tag identity. | MIT code; the BMad trademark notice applies separately. This is the authoritative product/method foundation, subject to provenance and notice review. |
| BMAD Builder | package `2.1.0`, module schema `1.0.0`, `private: true`; the Builder ZIP digest is recorded in its dedicated review, but the archive contains no Git commit/ref/tag identity. | MIT code plus the BMad trademark notice. This is the authoritative authoring and quality foundation; generated code and evals are still staged proposals, not trusted execution. |
| OpenClaw | Package `2026.6.11`; authoritative tree `_full/o/openclaw-main`. ZIP SHA-256 `6D1F477A4C69204FB22C9480081281EB547FF2BC353592077559F02D01B4ED8E`, 73,046,632 bytes, 23,305 entries: 21,980 regular files, 1,293 directories, and 32 symlinks. Every regular entry is present and size-matched; a per-file content-hash pass was **not** completed. The archive contains no Git commit/ref/tag identity. | Pattern source only. The root is MIT with third-party notices, but licensing is component-scoped: `skills/skill-creator/license.txt` is Apache-2.0 and that skill is included in the npm package. Copying requires per-component SPDX/notice/SBOM review. |
| Hermes | Project `0.18.2`, release `2026.7.7.2`; authoritative tree `_full/h/hermes-agent-main`. ZIP SHA-256 `E5E0941C515867EC024B343E775D07F34B323B363CB0570863CF6690B9291095`, 68,120,646 bytes, 7,075 entries: 6,205 regular files and 870 directories. Every regular file was content-hash verified against the ZIP. The archive contains no Git commit/ref/tag identity. | Pattern source only. The root is MIT, but `plugins/security-guidance/LICENSE` is Apache-2.0 and `skills/productivity/powerpoint/LICENSE.txt` has restrictive Anthropic terms. The PowerPoint skill is excluded from redistribution, import, packaging, and generated fixtures unless entitlement and legal review explicitly approve it. |
| Odysseus | Runtime constant `1.0.1`; FastAPI metadata still `1.0.0`; reviewed tree `_source_review/odysseus-dev`. No archive digest or Git commit/ref/tag identity was available. | AGPL-3.0-or-later. Requirements and design patterns are clean-room inputs only by default; code reuse requires an explicit legal decision accepting AGPL obligations. |

The 32 OpenClaw symlinks are part of the snapshot rather than missing implementation. Twenty-three are `CLAUDE.md -> AGENTS.md` aliases and nine are workspace or `node_modules` links. Their targets exist in the extracted snapshot, so the implementation owners needed for the audit are present; a direct install must still recreate those links in a symlink-capable environment. The older `_source_review/openclaw-main` and `_source_review/hermes-agent-main` trees are preserved as partial historical extractions and are not evidence authorities.

`SourceSnapshot` is therefore a Phase-0 release gate. It records upstream URL, immutable ref/commit, archive and license/notice hashes, extraction manifest/completeness, reviewed paths, fixture hashes, acquisition time, and verification status. Source size or a version string is not provenance.

### Plan-Changing Corrections

| Correction | Revised plan |
|---|---|
| BMAD was scheduled too late. | The first slice now selects a sealed, pinned Method-compatible skill/workflow, renders its Help Advisor action, persists workflow-step/artifact state, and includes BMAD lineage in evidence. Arbitrary import remains later. |
| Method and Builder expose more than one installed layout. | Phase 0 captures `MethodCliV6` and `StandaloneBuilderSetupV2` golden fixtures and requires an explicit `BmadInstallProfile`; mixed/ambiguous layouts fail review. |
| Builder's clean eval directory is not containment. | Locally, only trusted deterministic fake execution is allowed. Real install, invocation, script, and eval work starts in an Airlock-approved fixed Azure Container Apps Job with bounded credentials, network, time, and output. |
| OpenClaw approvals were overgeneralized. | `system.run` has meaningful exact-request binding: canonical `argv[]`, working directory, agent/session/requester, environment hash, node, and optional mutable-file digest are revalidated. Generic plugin approval records are consent metadata, not proof that an immutable execution spec was enforced. Sapphirus adopts the stronger binding pattern and supplies its own durable approval authority. |
| OpenClaw runtime durability and isolation are narrower than the product surface suggests. | Task/flow/delivery SQLite is useful single-gateway recovery, not a distributed queue or immutable attempt ledger; audit can drop async metadata; replay is bounded and process-local; plugins execute in-process with core-equivalent trust; custom context failure can silently fall back; hosted-safe sandbox/ask/security defaults are not enabled by default. Adopt contracts and tests, not those trust assumptions. |
| Hermes skill staging was overstated. | Skill writes are staged only when `skills.write_approval` is enabled; it defaults false, scanners/gates can fail open, and staging persistence can report success inaccurately. Sapphirus makes package mutation an explicit durable proposal and never treats optional staging as a safety boundary. |
| Hermes contains hosted-boundary and durability gaps. | Introduce an atomic `TurnCommit` with outbox; promote memory only from accepted finalized BMAD work; bind Azure credentials using parsed HTTPS and exact approved hosts rather than substring matching; remove raw environment fallbacks; isolate provider plugins; pin plugin source/ref/digest/signer/SBOM; replace local cron ownership with durable leases; make compression fail closed; aggregate child budgets; and bind release verification to source, diff, toolchain, and immutable evidence. |
| Odysseus does not supply the method kernel. | Its live plan mode is disabled; use its shell/degraded-state/detached-run patterns only. Reimplement independently because of AGPL, persist events beyond process memory, and never adopt its unsandboxed shell semantics. |
| Memory and artifacts need stronger truth semantics than the references provide. | Knowledge promotion is explicit and owner/source/evidence scoped. BMAD artifact versions are append-only with optimistic concurrency, hashes, and run/step/actor/approval/validation lineage. |
| Breadth and self-scored maturity are not release evidence. | Blocking contract/security/replay scenarios and machine-verifiable event/artifact assertions promote a surface; file/test counts and human overrides do not. |

### Corrected Cloud-First Build Spine

The user hardware profile is a locked architectural input: no local Docker engine, Kubernetes cluster, or local model server is required or assumed. Local execution means deterministic in-process fakes and temporary test workspaces containing trusted fixtures only; it is never an isolation boundary and never runs untrusted packages or commands.

1. **Governance and Source Intake:** license/provenance decisions, component exclusions, owner scope, trust taxonomy, retention, and release authority.
2. **Canonical Contracts:** BMAD Method/Builder install-profile fixtures; work-item/attempt/lease/completion/outbox; durable evidence; provider capabilities; schema projection; approval candidates; and generated OpenAPI clients.
3. **Trusted Local BMAD Proof:** repository and persistence seams, deterministic fake model/fake worker, sealed BMAD action, artifact lineage, and replay tests. No container or untrusted execution is involved.
4. **Azure Security Foundation:** Entra identities, managed identities, Key Vault, SQL, Blob, ACR, network/egress policy, telemetry, and a minimal Azure development environment. Images build remotely with ACR Tasks (`az acr build`) or hosted CI runners.
5. **Real Model Lane:** Foundry/Azure OpenAI v1 Responses through exact deployment-scoped `ModelProfile` records, `store=false`, hosted tools disabled, application-owned state, schema projection, offline evals, policy promotion, canary, and rollback.
6. **Production Execution Lane:** the first real isolated execution is a fixed-template Azure Container Apps Job. No arbitrary image, command, secret, or network policy is model-selected.
7. **Arbitrary Package Quality Gate:** BMAD package import, scan, rehearsal, evaluation, approval, activation, rollback, and immutable lineage.
8. **Product Expansion:** artifact adapters first, then Builder authoring/validation/evals through the same package and execution gates.
9. **Operations and Release:** operator evidence, restart/replay, supply chain, migration rehearsal, rollback, and production release proof.

## Consolidated Architecture Verdict

The Sapphirus plan should remain a BMAD-native modular monolith with hard internal ports, not a microservice mesh and not a plugin free-for-all. BMAD Method and Builder define the product kernel; the reviewed systems show that the surrounding agent platform becomes fragile when it treats tools, memory, packages, or delivery as prompt features instead of governed runtime surfaces.

The final architecture doctrine is:

```text
BMAD Method selects the valid workflow/action and artifact contract
BMAD Builder produces staged authoring/quality/eval proposals
Chat proposes intent
Runtime normalizes intent into ExecutionSpecCandidate
Policy evaluates the candidate and any exact human approval binds its hash
Airlock mints an audience-bound, single-use ApprovedExecutionSpec
Worker executes only approved specs in an isolated lane
Worker writes manifests, logs, artifacts, and hashes
Runtime imports results and owns SQL lifecycle state
Evidence binds candidate, policy, approval, spec, attempt, result, artifact, and rollback
Packages, skills, memory, providers, and connectors enter through staged contracts
```

The critical distinction is that the model never owns side effects, lifecycle state, package activation, provider credentials, or tool availability. Those are product runtime concerns.

## What Each System Changes In The Plan

| Area | Consolidated improvement |
|---|---|
| Product shape | Keep the first vertical slice narrow but BMAD-native: authenticated chat, sealed capability/workflow step, artifact expectation, workspace context, typed proposal, approval, deterministic fake result import, method/artifact transition, checkpoint, and evidence. The identical contracts later cross the fixed ACA Job boundary for real isolation. |
| Runtime API | Add owner scope, internal loopback principal, tool availability snapshots, prompt-cache contracts, provider resolution, and task/job claim records as first-class API/domain contracts. |
| BMAD Kernel | Make it the product-method authority from Phase 0. It parses/normalizes packages, workflows, config, manifests, help catalogs, method state, and artifact expectations; it does not become the general run orchestrator or execution authority. |
| Builder quality plane | Define authoring/eval contracts in Phase 0 while deferring the broad Studio UI. Builder outputs progress only through Draft, Validated, Rehearsed, Approved, and Active states. |
| Source intake | Treat source identity, license/notices, extraction completeness, fixture hashes, and verification status as durable release inputs rather than prose footnotes. |
| Package system | Packages/skills are data until validated, scanned, rehearsed, and activated. Builder output is a proposal, not trusted runtime code. |
| Tool registry | Tool availability is generated from package capability, policy, actor scope, dependency health, and run state. Static prompt schemas are insufficient. |
| Orchestrator | Persist active run invariants: prompt hash, tool schema hash, context pack hash, model/provider, actor, approval mode, and transition reason. |
| Model Gateway | Own provider differences, prompt caching, structured outputs, retry/fallback, credential binding, and provider health. It does not create proposals. |
| Workspace | Treat every file, package, note, memory, upload, and fetched document as untrusted input until policy says otherwise. |
| Execution | Local trusted fakes prove orchestration only. Real execution uses fixed ACA Job templates, `argv[]` command specs, approved image digests, bounded environment, output caps, network policy, heartbeat, timeout, and manifest import. |
| Infrastructure | Use ACA apps for always-on API/web surfaces and ACA Jobs for finite workers in v1. Build images remotely with ACR Tasks or hosted CI. Benchmark Dynamic Sessions only as a later interactive-lane candidate. |
| Security | Add owner scope, prompt-injection envelopes, SSRF/DNS-pinned fetch, profile-scoped secrets, token principals, loopback principal, and package trust states. |
| Operations | Degraded optional services, provider probes, task/job liveness, upload quarantine, context compaction, and release evidence must be visible in Operator Console. |

## Technology And LLM Implementation Verdict

The validated baseline is Azure-first and contract-first. Versions are exact repository pins, not floating production policies; deployment-specific model capabilities are discovered and evaluated through `ModelProfile`, never inferred from a marketing model name.

| Layer | Decision | Consolidated review |
|---|---|---|
| API | ASP.NET Core on .NET 10 LTS | Keep the modular monolith and pin the SDK feature band plus runtime patch. It fits Azure identity, SQL, OpenAPI, observability, and internal port boundaries. |
| Web | React 19.2 + Vite 8 + React Router 8 SPA | Use a server-authoritative SPA. Vite builds but does not type-check; CI runs explicit type-check, generated-client, production-build, and browser tests. React Router 8 and Vite 8 impose current Node/React/ESM compatibility floors that the lockfile and CI must prove. No SSR/BFF framework is added without a concrete need. |
| TypeScript | TypeScript 7 application compiler with a TypeScript 6 compiler-API sidecar | TypeScript 7 is the application/compiler target, but its public compiler API is not yet the migration authority. Programmatic generators and analyzers remain on `@typescript/typescript6` side-by-side until declarations, diagnostics, source maps, editor tooling, and generated clients pass a recorded promotion gate. |
| Node and package manager | Node.js 24 LTS + exact pnpm 11 | Keep Node 24 for the repository/runtime and pin pnpm through Corepack/package metadata. Frozen lockfile, provenance, lifecycle-script policy, and dependency-review gates are mandatory. |
| Python workers | Python 3.14 + uv, per worker profile | Python is not a global runtime assumption. Each worker profile proves interpreter compatibility, locked dependencies, import/startup, scan, SBOM, and reproducible remote image build before activation; incompatible workloads use an explicitly approved profile rather than a silent fallback. |
| API contracts | OpenAPI 3.1 + JSON Schema 2020-12 | This is the canonical v1 wire contract because ASP.NET Core 10 first-party generation supports it. OpenAPI 3.2 is a watched upgrade, not a split canonical contract. Generated clients and runtime validators derive from the same schemas. |
| AI APIs | Foundry/Azure OpenAI v1 Responses behind Model Gateway | Send `store=false`; application SQL/Blob/event state is authoritative; hosted provider tools are disabled unless separately governed; background/provider state is deferred and disposable. Normalize refusal, incomplete output, rate limits, content filtering, schema incompatibility, and retryability into typed outcomes. |
| Model governance | Exact deployment-scoped `ModelProfile` and evaluation promotion | Profiles name roles such as planner, schema repair, context compression, and artifact review and bind endpoint, deployment, API/capability snapshot, schema projection, data policy, budgets, and fallback boundaries. Promotion is candidate -> offline eval -> policy review -> canary -> active, with immediate rollback. No silent fallback crosses provider, residency, data-use, tool, or quality boundaries. |
| Structured outputs and caching | Canonical schema plus provider projection | Persist canonical and projected schema hashes. Treat the provider schema subset, refusal, and incomplete output as explicit branches; validate the returned object again at the application boundary. Prompt caching is a cost/latency optimization only and never a correctness, identity, or replay mechanism. |
| Web execution | Fixed Azure Container Apps Jobs | This is the first real web-isolated lane. `sealed_test_fake` is a functional fixture only. Dynamic Sessions and ACA Sandboxes remain evidence-gated future spikes, not baseline dependencies. |
| Remote build | ACR Tasks or hosted CI | `az acr build`/CI builds, scans, signs, and pushes images without a local Docker daemon. Runtime deploys immutable digests, never developer-local tags. |
| IaC | Bicep plus Azure Verified Modules where useful | Keep Bicep as source of truth. Use AVM modules when they match requirements without hiding policy, identity, network, or observability details. |
| Observability | OpenTelemetry on server/workers; Azure Monitor/Application Insights | Keep application-owned correlation across model call, work attempt, job, artifact, and evidence bundle. The browser uses supported Application Insights telemetry with redaction rather than pretending server OpenTelemetry automatically covers client state. |

## Technology Verification Sources

External facts were rechecked on 2026-07-09 against Context7 library documentation and the primary project/vendor sources below. Context7 was useful for API and migration details, but its TypeScript and React Router indexes lagged the just-released current versions; current official release material wins when the two differ.

- .NET support policy and ASP.NET Core 10 OpenAPI: `https://dotnet.microsoft.com/en-us/platform/support/policy/dotnet-core` and `https://learn.microsoft.com/en-us/aspnet/core/fundamentals/openapi/aspnetcore-openapi?view=aspnetcore-10.0`
- Node.js release schedule and pnpm: `https://github.com/nodejs/Release` and `https://pnpm.io/`
- Python 3.14 and uv: `https://peps.python.org/pep-0745/` and `https://docs.astral.sh/uv/`
- React 19.2, Vite 8, and React Router 8: `https://react.dev/blog/2025/10/01/react-19-2`, `https://vite.dev/blog/announcing-vite8`, and `https://reactrouter.com/changelog`
- TypeScript 7.0 and TypeScript 6 compatibility package: `https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/`
- Canonical OpenAPI 3.1 schema dialect and current OpenAPI specification: `https://spec.openapis.org/oas/v3.1.1.html` and `https://spec.openapis.org/oas/v3.2.0.html`
- ACA Jobs, Dynamic Sessions, and Sandboxes: `https://learn.microsoft.com/en-us/azure/container-apps/jobs`, `https://learn.microsoft.com/en-us/azure/container-apps/sessions`, and `https://learn.microsoft.com/en-us/azure/container-apps/sandboxes-overview`
- ACR remote builds and Azure Verified Modules: `https://learn.microsoft.com/en-us/azure/container-registry/container-registry-tasks-overview` and `https://azure.github.io/Azure-Verified-Modules/`
- OpenAI Responses migration and Azure OpenAI v1 Responses: `https://developers.openai.com/api/docs/guides/migrate-to-responses` and `https://learn.microsoft.com/en-us/azure/foundry/openai/how-to/responses`

## Architecture Improvements To Apply

### Lock These

| Decision | Reason |
|---|---|
| Delivery-specific domain authority owns lifecycle state. | The web Runtime API owns SQL state and the desktop Rust host owns SQLite state; comparable runtimes show that worker/tool code becomes hard to govern when it can mutate authoritative state. |
| Airlock mints every side-effect spec. | Policy evaluates an `ExecutionSpecCandidate`; any human approval binds the exact candidate hash; Airlock then mints an audience-bound, single-use `ApprovedExecutionSpec`. Approval is a durable object, not a UI button or prompt convention. |
| Model Gateway is provider-only. | Provider fallback, caching, structured outputs, and credentials are gateway concerns; proposal creation is orchestrator concern. |
| Application state is model-provider independent. | Responses use `store=false`; hosted tools stay off unless governed; provider conversation/background state cannot become lifecycle authority. |
| Exact `ModelProfile` promotion controls model change. | Capabilities, schema projection, data policy, eval results, canary, fallback boundary, and rollback are deployment-scoped evidence. A provider alias or model name cannot self-promote. |
| Tool availability is computed, not static. | Tool schemas depend on package activation, actor scope, policy, provider health, and run transition state. |
| Workspace content is untrusted. | Prompt injection can arrive through files, docs, notes, memory, package text, fetched URLs, and tool output. |
| Packages are staged proposals until validated. | Builder and SkillOps cannot safely write active package state directly. |
| Execution uses isolated lanes and manifests. | Shell-like convenience cannot replace an approved spec, bounded environment, output limits, and evidence import. |
| Owner scope is required. | Tokens, sessions, uploads, documents, tasks, memory, endpoints, and jobs need consistent access rules. |
| Development is Azure-first without local containers. | Local deterministic fakes validate contracts only. Remote build and the fixed ACA Job lane supply real packaging and isolation; no Docker, Kubernetes, or model server is required on user hardware. |

### Add These Phase-0 Gates

| Gate | Required evidence |
|---|---|
| TypeScript dual-toolchain gate | TypeScript 7 application compile/build passes; the TypeScript 6 sidecar proves every compiler-API consumer; generated clients, declarations, source maps, diagnostics, bundler, tests, and editor/LSP config are recorded before the sidecar can be removed. |
| Model profile and evaluation gate | Exact deployment capabilities, canonical/projected schema hashes, data policy, refusal/incomplete/error fixtures, offline eval thresholds, canary, rollback, and forbidden cross-boundary fallback are machine-verifiable. |
| Provider routing gate | `RuntimeProviderResolution` covers Foundry/Azure OpenAI v1, declared compatible endpoints, fallback, sovereign/exact-host binding, lookalike-host rejection, port/path rules, and credential-binding failure. |
| Outbound egress gate | SSRF fixtures cover private networks, metadata endpoints, redirects, DNS failure, and DNS rebinding/pinning. |
| Package activation gate | Manifest parse, trust classification, static scan, dependency lock, rehearsal install, invocation test, capability snapshot, and approval record. |
| Trusted local proof gate | An empty checkout proves generated clients, in-memory/fake stores, fake provider, fake worker, sealed BMAD action, and replay without Docker, emulators, local models, or untrusted execution. |
| Azure foundation gate | Entra/managed identity, Key Vault, SQL, Blob, ACR, remote build, network policy, telemetry, role assignments, and the minimal development environment deploy from reviewed Bicep parameters. |
| Worker image gate | ACR Tasks or hosted CI produces SBOM, provenance, digest pin, vulnerability scan, signature, command DSL compatibility, and output truncation evidence without a local Docker engine. |
| Fixed ACA Job gate | Exact template, identity, image digest, command policy, secrets, egress, timeout, heartbeat, result manifest, evidence import, retry, cancellation, and duplicate-delivery behavior pass before real execution. |

## Infrastructure Review

The deployment should be boring, cloud-first, and compatible with hardware that cannot run local deployment infrastructure:

- Local development: deterministic in-process provider/worker/store fakes and temporary trusted workspaces only. No local Docker, Kubernetes, SQL/Blob emulator, model server, or untrusted execution is a prerequisite.
- Azure development foundation: deploy early, before real model or command execution, so identity, storage, registry, network, and telemetry assumptions are tested in their actual security boundary.
- Runtime API: ACA app or App Service depending on auth/networking posture, with managed identity and no direct worker trust.
- Web workbench: static/web app surface with generated client, no hand-written route shapes.
- Workers: a fixed ACA Job template is the first and default finite real execution lane.
- Interactive execution: Dynamic Sessions or ACA Sandboxes only behind a later spike and `ExecutionLaneSpec`; neither is required for v1.
- State: Azure SQL for compact lifecycle/index state, Blob for bulky payloads/logs/artifacts/manifests/traces.
- Secrets: Key Vault and managed identities; local/dev secret sources must produce provenance reports and never leak into model context.
- Registry and build: ACR Tasks (`az acr build`) or hosted CI builds and pushes images remotely; deployments use digest-pinned images with SBOM, provenance, scan, and signing gates.
- Observability: OpenTelemetry to Azure Monitor/Application Insights, with audit event retention and redaction.
- Network: explicit egress policy, private endpoint strategy where needed, provider endpoint binding, and no arbitrary chat-supplied private URLs.
- IaC: Bicep plus AVM modules where they improve repeatability without obscuring required security parameters.

## Risk Register Updates

| Risk | Mitigation |
|---|---|
| Tool sprawl | Capability footprint ladder, tool availability snapshots, package activation gates, and narrow v1 scope. |
| Prompt injection | Untrusted context envelopes, role separation, context-pack trust classes, package scans, and negative fixtures. |
| `sealed_test_fake` overtrust | Mark fake execution as functional testing only; reject untrusted packages/commands in the fake; require the fixed ACA Job gate for real web execution. |
| Hardware-driven deployment drift | Make no-local-Docker/Kubernetes/model-server a tested development profile; use ACR Tasks/hosted CI and deploy the minimal Azure development foundation early. |
| Provider credential leakage | ProviderCredentialBinding, RuntimeProviderResolution, profile secret scope, and fallback transition events. |
| Context bloat | Adaptive context budgets, compaction records, protected ranges, small-model lanes, and user-visible compression events. |
| Worker/state corruption | Worker has no SQL lifecycle credentials; manifest import is idempotent and hash-checked. |
| Builder overreach | Builder creates proposals and rehearsal evidence; activation requires policy and package gates. |
| Infrastructure drift | Bicep source of truth, AVM review, what-if gates, environment parameter review, and release evidence. |
| Source/license contamination | `SourceSnapshot`, SPDX/notice review, clean-room provenance for AGPL-derived requirements, SBOM attribution, and no unreviewed code transfer. |
| Snapshot incompleteness or drift | Immutable refs, archive/extraction hashes, reviewed-path inventory, executable fixture generation, and confidence labels on every source-derived claim. |
| Self-certified maturity | Blocking scenarios with machine-verifiable events/artifacts; no promotion from file counts, model-authored scores, or human override alone. |

## Second-Pass Deep Dive (Reconciled In V6.16)

The code-level pass used `_full/o/openclaw-main` and `_full/h/hermes-agent-main` as the authoritative OpenClaw and Hermes trees, and `_source_review/odysseus-dev` for Odysseus. The earlier partial OpenClaw/Hermes extractions are not cited as completeness evidence. The pass extracted runtime and process contracts that the first review had summarized only thematically:

| Finding | Source evidence | Written into |
|---|---|---|
| Detached runs: server-side drain, replay-then-live subscribe, bounded buffer retention, honest durability scope | Odysseus `src/agent_runs.py` | [[12 - Run Orchestrator and Agent Kernel]] |
| Cheap-path discipline: mechanical output transforms are code, not model calls | Odysseus `src/agent_loop.py` | [[12 - Run Orchestrator and Agent Kernel]] |
| Foreground activity gate for background scans/jobs (quiet window, bounded deferral, operator toggle) | Odysseus `src/interactive_gate.py` | [[17 - Workspace Intelligence and Context Packs]] |
| Adaptive input budget: auto sentinel, headroom scaling, hard ceiling, conservative-on-unknown-window, pure function | Odysseus `src/context_budget.py` | [[17 - Workspace Intelligence and Context Packs]] |
| Compression mechanics: reference-only summary headings, filter-safe preamble, deterministic pruning pre-pass, token-budget tail protection, iterative summaries, pre-summarizer redaction | Hermes `agent/context_compressor.py` | [[17 - Workspace Intelligence and Context Packs]] |
| Cache-aware auxiliary call routing: full replay on warm cache key, digest when routed to a different model | Hermes `agent/background_review.py` | [[18 - Model Gateway and Microsoft Foundry]] |
| Background skill learning: complexity trigger, null-biased extraction, isolated review fork with tool allowlist; Hermes writes are staged only when `skills.write_approval` is enabled | Hermes `agent/background_review.py`, Odysseus `services/memory/skill_extractor.py` | [[14 - Builder Studio and SkillOps]] |
| Creation-time guard against self-destructive scheduled work, command-shaped patterns | Hermes `cron/lifecycle_guard.py` | [[14 - Builder Studio and SkillOps]], [[38 - Worker Images and Command DSL]] |
| QA maturity register: per-surface scored YAML, scenario-driven scores, three QA lanes | OpenClaw `qa/maturity-scores.yaml`, `qa/scenarios/` | [[33 - Release Gates and Acceptance Matrix]] |
| AI-agent development conventions: root/scoped `AGENTS.md`, evidence-map reviews, premise verification, footprint ladder, behavior-contract tests, one-canonical-path storage/fallback rules, PR overlap audit | OpenClaw `AGENTS.md`, Hermes `AGENTS.md`, Odysseus `tests/TESTING_STANDARD.md`, `scripts/pr_blocker_audit.py` | [[90 - LLM-Tailored Development Plan and Agent Workflow]] §14.2 |
| Sequence impacts: detached runs in slice, register from Phase 1, policy files in Phase 0, overlap-audited dispatch, gated background jobs | all of the above | [[51 - Master Implementation Sequence]] |

Scale explains why these projects need explicit policy files and test taxonomies, but it does not prove release readiness. OpenClaw's full snapshot includes a broad QA register, 202 QA YAML files, and human overrides/model-authored scores; those are useful inventories, not independent promotion evidence. Hermes has broad tests but no Git commit/ref/tag identity, and its release evidence is not durably bound to source, diff, and toolchain. Odysseus has broad tests while relevant full-CI enforcement remains non-blocking. Sapphirus adopts useful policy/test structures while requiring blocking, machine-verifiable evidence for its own gates.

## Files Updated From This Synthesis

This review feeds:

- [[00 - Common Rules and Product Shape]]
- [[02 - Locked Architecture Decisions]]
- [[07 - Source Coverage Matrix]]
- [[08 - Phased Roadmap and Build Order]]
- [[11 - Runtime API Control Plane]]
- [[18 - Model Gateway and Microsoft Foundry]]
- [[20 - Execution Lanes and Container App Jobs]]
- [[23 - Security, Identity, and Secrets]]
- [[25 - OpenAPI, Schemas, and Generated Clients]]
- [[28 - Supply Chain, Deployment, and IaC]]
- [[30 - Implementation Epics and Backlog]]
- [[32 - Integration Contract Map]]
- [[33 - Release Gates and Acceptance Matrix]]
- [[34 - Canonical Object Model]]
- [[35 - Source Alignment Notes]]
- [[36 - Local Development and DevEx]]
- [[37 - Azure Environments and Deployment Runbooks]]
- [[51 - Master Implementation Sequence]]
- [[76 - Current Stack Baseline]]
- [[82 - Current Technology Decision Summary]]
