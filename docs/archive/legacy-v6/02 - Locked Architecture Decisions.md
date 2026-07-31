---
title: "Locked Architecture Decisions"
aliases:
  - "02 - Locked Architecture Decisions"
tags:
  - bmad-runtime
  - vault/foundation
section: "Foundation"
order: 2
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: implementation-plan-library
status: v6-modernized-validated-implementation-guide
generated_on: 2026-07-09
review_pass: v6-modernization-and-platform-validation
architecture_rule: governed-chat-first-agentic-runtime
---



# Locked Architecture Decisions

## 0. Delivery-model applicability (V6.17)

Every decision now has an implicit applicability of `shared`, `web_managed`, or `windows_local`. Existing ASP.NET Core, Azure SQL/Blob, Python worker, and Container Apps decisions are locked for `web_managed`; they do not prescribe the desktop authority.

| Decision | Applicability | Status | Consequence |
|---|---|---|---|
| `Project.deliveryModel` is `web_managed` or `windows_local` and immutable | shared | `LOCKED` | Transfer creates a linked project/handoff; no run changes authority in place. |
| Tauri 2 + React/TypeScript + Rust is the Windows host | `windows_local` | `LOCKED` | Renderer has narrow capability-checked IPC; no broad filesystem/shell bridge. |
| SQLite plus encrypted local CAS is desktop lifecycle/evidence authority | `windows_local` | `LOCKED` | Azure sync is a replica/support plane, never a local write authority. |
| Local changes use checkpoints plus a journaled batch | `windows_local` | `LOCKED` | Per-file atomic replacement may be used; multi-file atomicity is not claimed. |
| Job Objects are process/resource controls, not filesystem/network confinement | `windows_local` | `LOCKED` | DESK-01 is a release gate if strong child-tool confinement is a product promise. |
| Azure Model Access API holds provider authority | `windows_local` | `LOCKED` | No Azure OpenAI/provider key is stored on the device. |
| Remote desktop work is a separate `web_managed` record | shared | `LOCKED` | Exact opt-in upload; returned patch becomes a fresh local proposal and is reapproved. |

## V6.18 BMAD foundation decisions

Source: [[100 - BMAD Method and Builder Deep Comprehension Audit]]. These decisions refine older Builder cut-line wording without merging the two delivery authorities.

| Decision | Applicability | Status | Consequence |
|---|---|---|---|
| Freeze Method/Builder semantics and select explicit source/install/validation/runtime profiles first. | shared | `LOCKED` | General contracts and fixtures cite exact source archetypes; one flattened invented BMAD schema is forbidden. |
| Pair the real sealed Method proof with early inactive Builder `Build`/`Edit`/`Analyze` drafts for one stateless agent and one simple workflow. | shared | `LOCKED` | Builder is foundational from the first proof, but draft creation grants no runtime capability. |
| Real conversational authoring follows Model Gateway. | shared | `LOCKED` | Model output is a versioned draft/proposal with source, prompt, config, model, and content hashes. |
| Builder isolated evaluation and install/invocation rehearsal follow governed execution. | delivery-specific | `LOCKED` | Web uses its isolated worker boundary; desktop uses only a proven local containment profile or an explicit remote rehearsal handoff. |
| Promotion, signing, publication, and activation follow complete evidence. | delivery-specific | `LOCKED` | Only the exact evaluated/rehearsed digest can advance; activation and rollback evidence are authority-local. |
| Memory and autonomous agents require additional substrate. | shared plus delivery-specific realization | `LOCKED` | Memory waits for owner-scoped durable storage; autonomy also waits for scheduler, quiet-hours, lifecycle, and containment gates. |
| `Convert` is not an upstream Builder capability in the pinned snapshot. | shared | `LOCKED` | A Sapphirus conversion flow may adapt `Build`/`Edit`, but UI, APIs, docs, and evidence must label it as a Sapphirus adapter rather than a native Builder command. |

## 1. Decision Status Vocabulary

| Status | Meaning |
|---|---|
| `LOCKED` | Implement now unless a formal ADR reverses it. |
| `TEMPORARY` | Implement for v1, but deliberately easy to replace. |
| `PHASE-0 SPIKE` | Must be tested before the implementation hardens. |
| `DEFERRED` | Do not build in v1 unless an ADR escalates it. |

## 2. Locked Decisions

| Decision | Status | Rationale | Consequence |
|---|---|---|---|
| Product shell is chat-first. | `LOCKED` | User intent and agentic execution must live in one auditable thread. | All blocks emit run cards/events. |
| BMAD is canonical runtime model. | `LOCKED` | Product is BMAD Runtime, not Cortex Runtime. | Cortex names are lineage only. |
| Runtime API uses ASP.NET Core modular monolith for `web_managed` and desktop cloud support APIs. | `LOCKED` | Strong Azure identity, policy, SQL, streaming, observability. | It never becomes desktop local lifecycle authority. |
| React + TypeScript for both UI surfaces. | `LOCKED` | Shared interaction language for chat, panels, diffs, approvals, and evidence. | Web uses generated OpenAPI clients; desktop uses generated narrow IPC clients. |
| Web shell is a client SPA built with Vite and React Router. | `LOCKED_WITH_GATE` | The product has a separate ASP.NET Core control plane and no established SSR/BFF or public-SEO requirement. | Use Vite 8 + React Router 8 SPA mode; adding Next.js or a Node web server requires an ADR with a concrete server-rendering need. |
| Python worker images for web/remote execution lanes. | `LOCKED` | Better repo tooling, validation scripts, artifact utilities. | Workers are stateless and digest-pinned; ordinary desktop tools run locally. |
| Azure Container Apps Jobs for finite web/explicit-remote side effects. | `LOCKED` | Patch/test/build/export are finite tasks. | Result manifests to Blob; API imports web state only. |
| Azure SQL stores compact web/support-plane lifecycle state. | `LOCKED` | Relational state fits web runs, entitlements, packages, sync metadata, and remote jobs. | It does not store authoritative ordinary desktop lifecycle state. |
| Blob stores web payloads and explicitly uploaded desktop payloads. | `LOCKED` | Object storage fits cloud snapshots, logs, diffs, bundles, packages, and consented sync. | No automatic local source upload. |
| Airlock creates `ApprovedExecutionSpec`. | `LOCKED` | Mechanical side-effect gate. | Executor rejects anything else. |
| Commands are `argv[]`, not shell strings. | `LOCKED` | Reduces shell injection and ambiguity. | No implicit shell expansion. |
| Delivery-specific authority owns state transitions. | `LOCKED` | Prevents worker, UI, model, or sync mutation ambiguity. | Web API imports worker manifests; desktop Rust host imports local results. |
| No auto-push in v1. | `LOCKED` | Keeps first version governed and reviewable. | Export patch/commit bundle only. |
| OpenAPI-first contracts. | `LOCKED` | Stabilizes web/API/test/CLI seams. | Contract tests required. |
| Production traces are redacted-by-default. | `LOCKED` | Privacy and security. | Use summaries/hashes/privileged raw refs. |

## 3. Temporary v1 Decisions

| Decision | Status | Replacement Trigger |
|---|---|---|
| Airlock lives in Runtime API process as pure policy kernel. | `TEMPORARY` | Separate deployment/security ownership required. |
| Workspace Intelligence starts as API module + async worker jobs. | `TEMPORARY` | Indexing workload impacts API reliability. |
| Operator Console shares React deployment. | `TEMPORARY` | Admin threat model requires separate app/bundle. |
| SSE is the v1 live projection over the durable event stream. | `TEMPORARY` | Adopt SignalR only when measured scale or a real bidirectional protocol need exceeds HTTP commands plus SSE. |

## 4. Phase-0 Spikes

| Spike | Question | Measurement |
|---|---|---|
| ACA Job startup latency | Is job startup acceptable for patch/test loop? | p50/p95 dispatch-to-first-log. |
| Dynamic sessions | Do prewarmed sessions materially improve interactive loops? | p95 first command latency and isolation fit. |
| Workspace snapshot size | Can Blob manifest/checkouts handle realistic repos? | snapshot time, storage, restore time. |
| Structural indexing | Should tree-sitter run async from day one? | scan time on medium repo. |
| SSE durability/scale | Does HTTP command + resumable SSE meet the first internal-alpha load target? | reconnect/restart replay, gap recovery, proxy behavior, p95 event latency, and connection count. |
| Azure OpenAI structured outputs | Which schema limits affect proposals? | schema adherence, latency, retries. |

## 5. Deferred Decisions

| Decision | Status | Reason |
|---|---|---|
| AKS | `DEFERRED` | ACA is sufficient until proven otherwise. |
| Full durable orchestration engine | `DEFERRED` | First prove vertical slice. |
| Full SkillOps release registry | `DEFERRED` | Builder must first validate/import packages. |
| Public SaaS tenancy | `DEFERRED` | Internal app target. |
| Git push/publish automation | `DEFERRED` | Higher risk side effect. |
| Broad tool/MCP marketplace | `DEFERRED` | Tool sprawl before core safety is dangerous. |
| Autonomous scheduled coding loops | `DEFERRED` | v1 requires explicit user approval. |

---

## v2 Review Improvements

### 1. Decision Ledger Format

Every decision must carry:

```yaml
id: ADR-000
status: LOCKED | TEMPORARY | PHASE-0-SPIKE | DEFERRED | SUPERSEDED
owner: architecture
applies_to: [runtime-api, web, worker, data, security]
valid_until: null | 2026-08-31
supersedes: []
validation_required: [test, spike, benchmark, threat-model]
```

### 2. Decision Conflict Resolution

If two files conflict, resolve in this order:

1. security boundary decisions;
2. state ownership decisions;
3. `00 - Common Rules and Product Shape.md`;
4. this file;
5. block-specific implementation file;
6. backlog file;
7. older context documents.

### 3. Decisions Now Locked By Review

| Decision | Status | Consequence |
|---|---|---|
| MVP starts with executable vertical slice. | LOCKED | Builder and Artifact Creator wait until substrate exists. |
| Runtime API is modular monolith with hard internal ports. | LOCKED | In-process module is allowed; bypassing ports is not. |
| Airlock mints `ApprovedExecutionSpec`. | LOCKED | Executor cannot accept raw proposals. |
| Workers do not write authoritative lifecycle state to SQL. | LOCKED | Worker writes Blob manifest; API imports. |
| Commands use `argv[]`. | LOCKED | Shell-string commands are invalid by default. |
| Workspace concurrency is single-writer/multi-reader. | LOCKED | Newer checkpoint voids stale proposals. |
| BMAD Kernel is not general orchestrator. | LOCKED | Orchestrator owns routing. |
| Model Gateway returns typed model outputs only. | LOCKED | Gateway does not create proposals or policy decisions. |
| Builder v1 is import/convert/validate one package. | LOCKED | Full Builder authoring moves later. |

### 4. Phase-0 Spikes With Required Evidence

| Spike | Max Duration | Output |
|---|---:|---|
| ACA Job cold-start benchmark | 2 days | Latency distribution, cost, failure modes, Dynamic Sessions comparison. |
| SSE durability/scale | 1 day | Reconnect/restart replay, gap recovery, proxy compatibility, and working event stream under job-log load. |
| OpenAPI generator | 1 day | Generated TypeScript client and C# DTO compile in CI. |
| Python worker base image | 2 days | Digest-pinned image, SBOM, patch/test fixture pass. |
| Secret filter | 2 days | Fixture suite with `.env`, token-like strings, certificates, prompt-injection text. |
| Structured-output limits | 2 days | Maximum patch/schema size before schema simplification required. |

### 5. Temporary Decisions Need Sunset

Temporary decisions must include a sunset date or objective trigger. Example:

```text
TEMPORARY: Use lexical/metadata-only context retrieval in the first slice.
Sunset: After the retrieval evaluation dataset and privacy/deletion gates exist.
Trigger: A measured vector-retrieval candidate materially improves grounded task quality.
```

A temporary decision without a sunset becomes architecture debt and must be listed in `31 - Architecture Decision Records.md`.



## 6. V6 Locked Modern Toolchain Baseline

These are implementation baselines as of 2026-07-09. They are locked for new code unless a formal ADR downgrades them for compatibility.

| Area | V6 baseline | Decision class | Implementation consequence |
|---|---|---|---|
| Runtime API | ASP.NET Core on **.NET 10 LTS** | `LOCKED` | Do not start new API code on .NET 8/9. Use .NET 10 SDK/runtime images and patch monthly. |
| Frontend runtime | **Node.js 24 LTS** | `LOCKED` | Pin with `.nvmrc`/Volta, hosted CI, and `engines.node`; a devcontainer is optional and never required. Avoid Node 26 until it enters Active LTS on 2026-10-28 and dependency compatibility is proven. |
| Frontend framework | **React 19.2 + Vite 8 + React Router 8 SPA mode** | `LOCKED_WITH_GATE` | Keep Runtime API state authoritative. Route loaders/actions call generated API clients; no second server-side mutation authority is introduced. |
| TypeScript | **TypeScript 7.0 application compiler** | `LOCKED_WITH_GATE` | TS 7.0 is GA but ships without a public compiler API. Keep `strict`, explicit `rootDir`/`types`, generated-client type tests, and a pinned TS 6 compatibility package only for tooling that imports the compiler API until TS 7.1+ support is proven. |
| Package manager | **pnpm 11.x** | `LOCKED` | Pin `packageManager`, use frozen lockfiles in CI, use workspace config in `pnpm-workspace.yaml`, and review pnpm trust-policy settings. |
| Python workers | **Per-worker locked profile; Python 3.14 preferred** | `LOCKED_WITH_GATE` | Each worker/package operation declares its runtime and passes wheel/native-image compatibility. An alternative version is worker-specific, evidence-backed, and never a silent global downgrade. |
| Python package manager | **uv** | `LOCKED` | Use `uv.lock`, `uv sync --locked`, and digest-pinned worker images. |
| API contract | **OpenAPI 3.1 + JSON Schema 2020-12** | `LOCKED` | One v1 contract must match ASP.NET Core 10 generation and client tooling. OpenAPI 3.2 is a .NET 11/tooling upgrade gate, not a second truth. |
| Model API | Microsoft Foundry / Azure OpenAI **v1 API + Responses API where fit** | `LOCKED` + `SPIKE_REQUIRED` | Model Gateway owns provider differences; do not couple agent kernel to provider SDK objects. |
| Low-latency execution | ACA Dynamic Sessions | `PHASE-0 SPIKE` | Verified capability, not v1 baseline until latency/isolation/cost spike passes. |
| Agent hosting | Foundry Agent Service | `DEFERRED/SPIKE` | Do not replace the custom Run Orchestrator in v1; assess later for hosted-agent deployment or eval/trace features. |
| Model state and hosted tools | Application-owned state; Responses `store=false`; provider-hosted tools disabled by default. | `LOCKED` | Provider response IDs are correlation only. Any hosted MCP/web/code/shell/computer tool must enter the Sapphirus registry, Airlock, owner-scope, and evidence path before enablement. |
| TypeScript tooling compatibility | Pinned TS 6 compiler-API sidecar where required. | `TEMPORARY` | Remove after lint/generator/editor tooling supports the TypeScript 7 public API; never use the sidecar as the application compiler by accident. |

### Baseline downgrade rule

A downgrade from this baseline is allowed only when all conditions are met:

1. a dependency or Azure runtime incompatibility is reproduced;
2. the downgrade is scoped to the affected component;
3. the ADR records the incompatibility, duration, and exit condition;
4. CI still proves security, contract, and worker-manifest gates.

---

## Historical Revision Notes (V3 -> V4)
## Review finding

`02 - Locked Architecture Decisions.md` is part of the implementation library support layer. In v3, support files were useful but not always testable. In v4, every support file must provide either a decision, reference contract, release gate, mapping, runbook, or checklist that can be executed by a developer or coding agent.

## Required usage

1. Read this file before changing the related implementation area.
2. Cross-check it against `07 - Source Coverage Matrix.md` and `50 - V4 Full Library Audit.md`.
3. When implementing a task, copy the relevant checklist items into the issue/story.
4. When a decision changes, update this file and `31 - Architecture Decision Records.md` in the same PR.
5. When a contract changes, update `25 - OpenAPI, Schemas, and Generated Clients.md`, `46 - API Route Catalog.md`, and generated clients.

## V4 quality rules for this file

- It must not contradict locked architecture decisions.
- It must not reintroduce a broad v1 scope that competes with the executable vertical slice.
- It must preserve BMAD source contracts and the existing presentation workflow adapter decision.
- It must reflect the Runtime API as lifecycle state owner and the worker as manifest/log producer only.
- It must identify whether guidance is `LOCKED`, `TEMPORARY`, `PHASE-0 SPIKE`, `V1`, `V1.5`, or `V2`.

## Implementation checklist linkages

| Related guide | What to cross-check |
|---|---|
| `01 - First Build - Executable Vertical Slice.md` | Does this file support or distract from the first slice? |
| `29 - Concurrency, Transactions, and Failures.md` | Are state and partial failure semantics compatible? |
| `32 - Integration Contract Map.md` | Are producer/consumer boundaries clear? |
| `33 - Release Gates and Acceptance Matrix.md` | Is there a release gate for this guidance? |
| `49 - Detailed Component Build Checklists.md` | Are implementation tasks represented as checklist items? |

---

## V6 validation addendum

The following wording is now enforced across the library:

- `LOCKED` means project architecture decision, not necessarily an external platform fact.
- `TEMPORARY` means buildable for v1 but intentionally replaceable.
- `PHASE-0 SPIKE` means external capability may exist, but product fit is not verified.
- `DEFERRED` means do not build without ADR escalation.

Validated examples:

- ACA Jobs are externally verified as finite containerized tasks and remain `LOCKED` for v1 execution.
- Dynamic Sessions are externally verified as an isolated low-latency capability but remain `PHASE-0 SPIKE` / v1.5 candidate.
- SSE is the `TEMPORARY` v1 live transport under ADR-021; SignalR remains an upgrade option for measured bidirectional or scale requirements.
- Structured Outputs are externally verified for schema-shaped model output, but local server-side validation remains `LOCKED`.

## Consolidated Source-Review Locked Decisions

Source: [[89 - Consolidated AI Workspace Source Review and Architecture Improvements]].

The combined BMAD, OpenClaw, Hermes, and Odysseus review locks these additional architecture rules:

| Decision | Status | Consequence |
|---|---|---|
| Tool availability is computed from package activation, actor scope, policy, dependency health, and run state. | `LOCKED` | Do not treat a static model tool schema as the source of truth. |
| Owner scope is mandatory for user data and machine-token actions. | `LOCKED` | Sessions, uploads, documents, tasks, memory, provider endpoints, jobs, and tokens require `OwnerScope`. |
| Internal loopback is a principal, not ambient trust. | `LOCKED` | Internal tool dispatch uses `InternalLoopbackPrincipal` and still checks owner/admin privilege. |
| Packages and skills are staged until validated and activated. | `LOCKED` | Builder, SkillOps, and background improvements create proposals and evidence, not direct active writes. |
| BMAD Method and Builder are the product foundation. | `LOCKED` | Method owns workflow/artifact/help semantics; Builder owns authoring/quality/eval semantics; neither owns runtime authorization or execution authority. |
| The first executable slice is BMAD-native. | `LOCKED` | Phase 1 uses one sealed, pinned Method-compatible skill/workflow fixture and persists package, skill, step, config, and artifact lineage even though arbitrary package import remains later. |
| BMAD installation shape is explicit. | `LOCKED` | Import declares `BmadInstallProfile` (`MethodCliV6` or `StandaloneBuilderSetupV2`) and normalizes into canonical package/config contracts; ambiguous mixed layouts fail review. |
| Comparable runtimes are pattern sources, not dependencies. | `LOCKED` | OpenClaw, Hermes, and Odysseus do not become the Sapphirus domain kernel or required runtime packages. Adopt contracts selectively behind Sapphirus ports. |
| Source snapshots require provenance before promotion. | `LOCKED` | Record upstream URL, tag/commit, archive hash, license/notice hash, extraction completeness, fixture hashes, and verification status before source-derived code or fixtures enter a release. |
| License decisions are component-scoped. | `LOCKED` | Root SPDX metadata cannot authorize bundled skills/plugins/assets with different or restrictive terms. Source Intake inventories every notice/license and blocks redistribution without an explicit `LicenseDecision`. |
| Odysseus implementation reuse is clean-room by default. | `LOCKED` | Its AGPL-3.0-or-later code is not copied or linked into a differently licensed Sapphirus product without explicit legal/license approval; requirements and patterns may be independently reimplemented with provenance. |
| Outbound network access is policy-governed. | `LOCKED` | Webhooks, fetched URLs, search content, provider probes, and chat-supplied endpoints use `OutboundUrlPolicy`. |
| TypeScript 7 is target baseline with a migration gate. | `LOCKED_WITH_GATE` | Generated clients, declaration output, bundler, editor/LSP, and CI typecheck must pass before release. |
| Dynamic Sessions are not promoted by availability alone. | `PHASE-0 SPIKE` | Adoption requires evidence for latency, isolation, cost, network controls, region availability, and manifest integration. |
| BMAD upstream tooling runs outside the .NET control plane. | `LOCKED` | A digest-pinned Node/Python import-rehearsal worker executes Method/Builder tooling and emits neutral `BmadPackageDescriptor`/validation evidence; the .NET BMAD Kernel consumes normalized data and never imports upstream executable code in-process. |
| Minimum identity/trust contracts precede the first project. | `LOCKED` | `OwnerScope`, dev/user principal, `UntrustedContextEnvelope`, safe archive report, redaction, and deterministic context budget exist in Phase 0/1; later hardening expands coverage. |
| Evidence is durable authority, telemetry is a projection. | `LOCKED` | A domain transaction persists lifecycle state, `EvidenceLedgerEvent`, and outbox atomically. Audit, OTEL, browser telemetry, dashboards, and support bundles may sample or fail without becoming authority. |
| Cloud execution uses fixed templates. | `LOCKED` | ACA Jobs use digest-pinned images and fixed entrypoints; request-time image/command/environment/secret overrides are rejected even when the caller may start a job. |
| Cloud prerequisites arrive before cloud consumers. | `LOCKED` | Minimal dev/staging Bicep, Entra scopes, managed identities, Key Vault, SQL, Blob, ACR, and monitoring precede real Foundry and ACA integration; Phase 8 hardens rather than first-provisions them. |
| Development is cloud-first and must not require local Docker, Kubernetes, or local model serving. | `LOCKED` | Local work uses the .NET/Node/Python toolchains, deterministic in-process fakes, temporary test workspaces, and in-memory/fake stores. Container images are built by ACR Tasks (`az acr build`) or hosted CI runners. |
| The `sealed_test_fake` executor is a test double, never an isolation boundary. | `LOCKED` | It may exercise only sealed trusted fixtures and must not run imported packages, generated shell, arbitrary dependency restores, or untrusted workspace code. The first real web execution boundary is a fixed-template Azure Container Apps Job. |
