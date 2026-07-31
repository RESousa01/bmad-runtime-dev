---
title: "LLM-Tailored Development Plan and Agent Workflow"
aliases:
  - "LLM Development Plan"
  - "AI Agent Build Plan"
  - "Agentic Implementation Workflow"
tags:
  - bmad-runtime
  - vault/delivery-plan
section: "Delivery Plan"
order: 90
status: v6.16-cloud-first-reviewed
created_on: 2026-07-09
project: Sapphirus BMAD Runtime
---

# LLM-Tailored Development Plan and Agent Workflow

## V6.17 dual-track agent workflow

Every AI work packet is assigned to `S`, `W`, or `D`. Shared packets may edit schemas/fixtures/rule data/UI primitives only. Web packets may edit .NET/Azure/cloud-workspace/worker authority. Desktop packets may edit Tauri/Rust/SQLite/local-workspace authority. Cross-track reviews verify conformance and forbidden edges; agents do not create a universal executor/store to reduce duplication.

Desktop code packets require security invariants and fault tests before breadth: narrow IPC, selected-root enforcement, journal durability, exact command identity, measured containment, context-egress privacy, signing/update, and remote-result reapproval. The existing cloud-first sequence remains authoritative for W; it is not reused as the D execution plan.

## V6.18 mandatory BMAD/Builder work-packet overlay

Every BMAD or Builder packet must read [[100 - BMAD Method and Builder Deep Comprehension Audit]] and declare its source/install/validation/runtime profile. Agents execute this dependency order:

1. semantic source freeze and source-derived fixtures;
2. one real sealed Method proof plus inactive Builder `Build`/`Edit`/`Analyze` for one stateless agent and one simple workflow;
3. real conversational draft authoring only after Model Gateway;
4. static scans plus isolated baseline/variant/quality/trigger evals and exact install/invocation rehearsal only after the applicable governed execution boundary;
5. exact-digest promotion, signing/publication where applicable, and reversible activation only after durable evidence, policy, and rollback gates;
6. memory agents only after owner-scoped durable storage, and autonomous agents only after storage, scheduler, quiet-hours, lifecycle, and containment gates.

Shared packets own semantics, profiles, schemas, and fixtures; W or D packets own execution, activation, evidence, and rollback. No packet may describe `Convert` as an upstream Builder capability: the pinned Builder routes `Build`, `Edit`, and `Analyze`; any conversion UX is a Sapphirus adapter over `Build`/`Edit` and must be labeled and tested as such. This overlay supersedes older packet ordering that postpones all Builder authoring to Phase 6B, without allowing early drafts to execute or activate.

## 1. Purpose

This plan is the practical build manual for implementing the Sapphirus BMAD Runtime with AI coding agents.

It assumes LLMs will perform much of the implementation, review, test creation, refactoring, documentation, and release preparation. Because of that, the plan is intentionally stricter than a normal human-only roadmap. LLMs are excellent at local implementation and synthesis, but they drift when scope is blurry, contracts are implicit, or tests are delayed. This plan makes the work small, testable, contract-first, and evidence-heavy.

Use this file with:

- [[51 - Master Implementation Sequence]]
- [[71 - Backlog Story Template and Ready Rules]]
- [[44 - AI Coding Agent Handoff Prompts]]
- [[89 - Consolidated AI Workspace Source Review and Architecture Improvements]]
- [[33 - Release Gates and Acceptance Matrix]]

## 2. Core Doctrine

Every implementation agent must preserve this doctrine:

```text
BMAD Method frames the work.
BMAD Builder shapes and evaluates extensions.
Models propose.
Runtime normalizes.
Policy evaluates an exact candidate.
Humans approve the candidate hash when risk requires it.
Airlock mints a single-use execution authority.
Workers execute.
WorkerResultManifests report.
Runtime imports.
Evidence Ledger proves; Evidence Bundles explain.
Packages extend only after validation.
```

If an agent changes code in a way that skips one of these verbs, it is probably moving authority into the wrong layer.

### V6.16 authoritative boundaries

The following corrections are normative wherever older notes use broader or conflicting wording:

| Boundary | Authoritative rule |
|---|---|
| Product foundation | BMAD Method owns method/workflow/artifact/help semantics. BMAD Builder owns governed authoring and evaluation semantics. Neither owns identity, authorization, execution, or durable runtime state. |
| Approval flow | `Proposal -> ExecutionSpecCandidate -> policy evaluation -> exact candidate-hash approval when required -> audience-bound, expiring, single-use ApprovedExecutionSpec`. A prior generic approval cannot be attached to a newly generated spec. |
| Ordinary application writes | Authenticated CRUD such as creating a thread or saving a user preference uses authz, owner scope, validation, idempotency, audit, and domain transactions. It does not manufacture an execution token. |
| Governed mutations | Model/package/workspace/external mutations, exports, dependency restores, network-sensitive calls, and worker dispatch pass through Airlock policy. High-risk effects also require explicit human approval of the exact candidate hash. |
| Source intake | Archive/repository intake is a build/CI trust workflow with provenance and component-license decisions; intake alone never activates executable package content. |
| Executor result | `ExecutionResultManifest` is the closed union; `WebWorkerResultManifest` and `WindowsLocalExecutionResultManifest` are its non-interchangeable branches. `ExecutionManifest` is retired. |
| Durable proof | `EvidenceLedgerEvent` is transactional authority. `EvidenceBundle` is the canonical user/operator materialization. `TraceBundle` is a diagnostic projection and may be sampled or unavailable without changing domain truth. |
| Durable work | `WorkItem -> Attempt -> Lease -> Completion -> Outbox` is persisted with idempotency and recovery semantics. In-memory queues, event buffers, or provider response chains are never recovery authority. |
| Event identity | Durable events carry `streamId`, monotonic `sequence`, aggregate type/id, `OwnerScope`, schema version, and optional run id; consumers define cursor expiry/gap recovery, upcasters, and projection checkpoints. Replay rebuilds projections and never re-executes effects. |
| Development hardware | The baseline requires no local Docker, Kubernetes, infrastructure emulator, or local model server. Local execution is a deterministic trusted fake only; remote image builds use ACR Tasks or hosted CI, and the first real isolated execution is a fixed-template ACA Job. |

### Locked implementation baseline (revalidate exact pins at each release)

| Layer | Baseline | Plan consequence |
|---|---|---|
| Control plane | .NET 10 LTS / ASP.NET Core 10; current validation pin .NET SDK 10.0.301 and runtime 10.0.9 | Modular monolith, generated OpenAPI, Entra auth, SQL transactions/outbox, server OpenTelemetry. |
| Web | React 19.2.7 + Vite 8.1.0 + React Router 8.0.1 SPA | ASP.NET API remains state authority; no Next.js/Node server without an ADR proving SSR/BFF need. |
| Web language/tooling | TypeScript 7 application compiler; isolated pinned TypeScript 6 compatibility package only for tools importing the compiler API | Strict typecheck and generated-client gates; remove the sidecar after the toolchain proves a supported TS7 public API. |
| JavaScript runtime/package manager | Node 24 LTS (current validation pin 24.18.0) + pnpm 11 (11.4.0) | Exact engines/packageManager pins, frozen lock, hosted CI install, and supply-chain evidence. |
| Worker/import tooling | Python per-image profile, Python 3.14.6 preferred after compatibility evidence; uv 0.11.21 current validation pin | BMAD/import/render tooling stays outside the .NET process and every image has its own locked dependency/runtime matrix. |
| Contracts | OpenAPI 3.1.2 + JSON Schema 2020-12 | One canonical v1 schema set. OpenAPI 3.2 is a future .NET/tooling gate, not a competing truth. |
| Model | Microsoft Foundry/Azure OpenAI v1 Responses behind Model Gateway | `store=false`, hosted tools off, app-owned state, exact deployment profiles, schema projection plus canonical validation, and evaluated fallback. |
| Azure runtime | Bicep/AVM, Entra/managed identity, Key Vault, Azure SQL, Blob, ACR Tasks, Container Apps/API, fixed ACA Jobs, Azure Monitor | Cloud-first development, immutable remote builds, finite isolated effects, least privilege, and evidence-bound deployment. |

## 2.1 Senior Engineering Review Verdict

This plan is intentionally ambitious. The risk is not that it lacks detail; the risk is that implementation agents will treat the detail as optional once coding starts.

The senior engineering corrections are:

| Weak spot | Reinforcement |
|---|---|
| Too many parallel possibilities | Use WIP limits and one durable boundary per work packet. |
| LLMs may over-implement | Require explicit non-goals, stop conditions, and "defer instead of build" rules. |
| Tests may drift to happy path | Require one negative/bypass test for every risky path before implementation. |
| Refactors may hide behavior changes | Require characterization tests and compatibility shims before moving code. |
| Architecture may erode through convenience | Require ownership checks: who owns state, policy, side effects, evidence, and rollback. |
| BMAD may be postponed into an optional plugin | Require the sealed Method-compatible fixture, capability graph, workflow-step state, and Builder lifecycle contracts in Phase 0/1. |
| Production concerns may arrive late | Add rollback, observability, migration, retention, and operator visibility to story readiness. |
| AI context may rot | Require a context ledger: files read, assumptions made, and decisions changed. |
| Review may become style-focused | Review must lead with bugs, missing tests, contract drift, and security bypasses. |

This plan should be enforced as a build discipline. If a team member or AI agent wants to bypass it, the burden is on them to produce a written exception with owner, expiry, risk, and rollback path.

## 3. LLM Operating Rules

These rules apply to every AI-assisted implementation task.

| Rule | Meaning |
|---|---|
| Read before editing | The agent must read the component note, relevant contracts, and nearby code before patching. |
| Keep the work packet small | One story should change one behavioral surface or one compatibility-preserving refactor slice. |
| Tests before risky code | Use TDD or contract-first tests for state transitions, policy, auth, side effects, schemas, migrations, worker protocols, and security boundaries. |
| No hidden architecture changes | If a change alters ownership, hosting, security posture, package activation, or state transition, update ADR/docs in the same work packet. |
| Delivery-specific authority owns lifecycle state | Web Runtime API owns SQL state; desktop Rust host owns SQLite state. Workers, child tools, models, packages, UIs, sync, and support APIs do not mutate authoritative lifecycle state. |
| BMAD lineage is mandatory | Every BMAD-bound run records source snapshot, install profile, package, skill, workflow step, config, artifact expectations, and hashes. |
| Airlock gates side effects | No file write, command run, export, package activation, dependency restore, network-sensitive fetch, or worker dispatch without policy and approval where required. |
| Workspace content is untrusted | Source files, uploaded docs, package text, memories, notes, web pages, and tool outputs cannot override policies or system instructions. |
| Contracts are explicit | Public APIs, durable events, manifests, Blob payloads, and generated clients come from OpenAPI/JSON Schema, not hand-written drift. |
| Evidence over confidence | A task is not complete because the agent says it is complete; it is complete when tests, logs, manifests, screenshots, or replay evidence prove it. |
| Prefer boring infrastructure | Use the locked stack and simple deployment path unless a spike proves a better option. |
| Leave a context ledger | Every agent final report lists files read, files changed, assumptions, commands run, skipped checks, and remaining risks. |
| Stop on boundary confusion | If ownership or policy authority is unclear, stop and update the plan/ADR before coding. |
| Default to reversible | Prefer changes that can be rolled back, feature-flagged, disabled, or isolated by configuration. |

## 3.1 Senior Non-Negotiables

These are stop-the-line rules. If any one is violated, pause implementation and fix the process before continuing.

| Non-negotiable | Stop condition |
|---|---|
| No side-effect bypass | Any code path can write files, run commands, activate packages, call network-sensitive endpoints, or dispatch workers without the required Airlock/policy path. |
| No lifecycle SQL from workers | Worker, package, model adapter, or tool code can mutate authoritative run/proposal/approval/execution/checkpoint state. |
| No contract drift | Frontend, worker, or provider code hand-rolls public payloads that differ from OpenAPI/JSON Schema. |
| No unowned state | A table, Blob prefix, event, route, or payload lacks an owning component and retention class. |
| No owner-scope leak | A route can reveal whether another user's resource exists. |
| No unbounded output | A tool, worker, model call, or log path can emit unbounded data into UI, model context, trace, or Blob without caps and redaction. |
| No invisible degraded mode | Optional dependencies fail silently or cause partial behavior without operator/user visibility. |
| No temporary without sunset | Temporary shortcuts must have owner, expiry/trigger, rollback path, and ADR or backlog item. |

## 3.2 Work In Progress Limits

LLM-assisted teams can create too much unfinished work. Use strict WIP limits:

| Work type | Limit | Reason |
|---|---:|---|
| Active contract changes | 1 schema family | Avoid generated-client churn and cascading ambiguity. |
| Active backend feature slices | 1 vertical behavior | Keep state ownership reviewable. |
| Active UI feature slices | 1 screen or event-card family | Keep user-state transitions testable. |
| Active infrastructure changes | 1 environment/module slice | Keep rollback and what-if review manageable. |
| Active refactors | 1 module/domain move | Preserve behavior and avoid mixing migrations. |
| Active spikes | 1 decision question | Prevent research from becoming shadow implementation. |

If a story needs more, split it.

## 3.3 Context Ledger Requirement

Every AI coding agent must leave a compact context ledger in its final report or PR notes:

```md
## Context Ledger

- Vault notes read:
- Source files read:
- Contracts/schemas read:
- Assumptions made:
- Decisions changed:
- Files edited:
- Tests added/changed:
- Commands run:
- Checks skipped and why:
- Residual risks:
```

This is not bureaucracy. It is how the next agent avoids rediscovering the same terrain or trusting stale assumptions.

## 4. Agent Roles

Use these roles as prompts or thread labels. One LLM can play multiple roles, but do not mix roles inside one unchecked patch.

| Role | Primary responsibility | Should not do |
|---|---|---|
| Architecture Agent | Reads vault notes, defines component boundaries, updates ADRs and contracts. | Write production code without test and implementation agents reviewing feasibility. |
| BMAD Foundation Steward | Validates Method/Builder snapshot, install profile, package/config layering, workflow/artifact/help fidelity, and upstream compatibility evidence. | Let BMAD content bypass runtime policy or silently reinterpret upstream semantics. |
| Contract Agent | Writes OpenAPI, JSON Schema, DTOs, event schemas, examples, and generated-client tests. | Add behavior outside contracts. |
| Backend Agent | Implements Runtime API, domain state, persistence, ports, and authz. | Call provider SDKs or worker internals directly from controllers. |
| Model Gateway Agent | Implements provider adapters, structured-output validation, prompt-cache records, and fallback evidence. | Create proposals or approve actions. |
| Model Evaluation Agent | Owns frozen eval datasets, graders, promotion evidence, canaries, rollback thresholds, and model/prompt/context profile comparisons. | Promote a candidate it implemented from subjective inspection or aggregate one score that hides safety failures. |
| Workspace Agent | Implements snapshots, file manifests, uploads, context packs, redaction, preimages, and checkpoints. | Trust workspace content or bypass path policy. |
| Airlock Agent | Implements policy evaluation, approvals, approved specs, expiry, and denial evidence. | Execute commands or mutate files directly. |
| Worker Agent | Implements worker images, command DSL, execution, logs, manifests, and output limits. | Write lifecycle SQL or invent approval state. |
| Frontend Agent | Implements Chat Workbench, approval cards, event timelines, diff/evidence views, and operator UI. | Hand-roll API payloads or hide server states in client-only state. |
| Security Agent | Writes threat tests, SSRF tests, prompt-injection tests, owner-scope tests, and package safety checks. | Only review happy-path code. |
| Data and Privacy Agent | Owns retention classes, redaction, provider-storage policy, deletion/subject-right flows, data residency, and Evidence Ledger privacy review. | Treat telemetry or provider-side state as authoritative data storage. |
| Cloud and Release Provenance Agent | Owns Bicep/AVM, remote ACR/CI builds, identities, image digest/SBOM/attestations, environment promotion, and rollback evidence. | Require local Docker or promote a mutable tag/build with missing provenance. |
| Test/Replay Agent | Builds fixtures, fake providers, fake workers, golden files, contract tests, and replay packs. | Freeze incidental prompt wording or brittle snapshots. |
| Docs Agent | Updates vault notes, route catalogs, examples, runbooks, and handoff prompts. | Change architecture without cross-linking affected notes. |
| Review Agent | Performs bug-focused review after implementation, prioritizing regressions and missing tests. | Rewrite code for style without a concrete risk. |

## 4.1 Senior Review Rotation

For risky work packets, use a two-pass review:

| Pass | Reviewer stance | Required output |
|---|---|---|
| Design review before code | Senior engineer looking for scope, ownership, contracts, and missing tests. | Approve, split, or block the work packet. |
| Code review after implementation | Bug-finding reviewer looking for regressions, bypasses, contract drift, and test gaps. | Findings first, then residual risk and verification status. |

Risky work includes Airlock, execution, auth, owner scope, provider credentials, package activation, migrations, Blob retention, egress, worker images, and release gates.

## 5. Development Modes

Choose the development mode based on risk. Do not use one generic "code then test" approach.

| Mode | Use for | Required first artifact |
|---|---|---|
| Contract-first development | APIs, event streams, manifests, generated clients, package schemas. | OpenAPI/JSON Schema plus example payloads and contract tests. |
| TDD | Domain state machines, policy predicates, parsers, validators, authz checks, idempotency, ownership. | Failing unit tests for success and at least one failure case. |
| Security-test-first | Airlock bypasses, SSRF, prompt injection, path traversal, token scope, secret leakage, package activation. | Negative fixture that proves the bypass is blocked. |
| Characterization/golden testing | BMAD parser compatibility, presentation adapter, Builder output, evidence bundles. | Golden fixture from known input and expected output. |
| Replay-driven development | Full vertical slice, repair loop, worker failures, model-output failures. | Replay scenario with deterministic fake provider/worker. |
| Spike-first development | Dynamic Sessions, provider routing options, OpenAPI generator options, performance-sensitive context indexing. | Time-boxed spike report with decision, measured evidence, and exit criteria. |
| Migration-first development | SQL schema changes, Blob layout changes, versioned contracts, retention updates. | Migration fixture and rollback/forward-compatibility note. |
| UI state-machine development | Approval cards, event timeline, background jobs, operator states, degraded-service UI. | UI state table, mock payloads, and Playwright test. |
| Refactor-with-shim development | Splitting large modules, moving package/tool code, route grouping. | Compatibility shim and behavior-preserving tests before move. |

## 5.1 Mode Selection Rules

When more than one mode applies, choose the stricter mode:

| If the story includes... | Required mode |
|---|---|
| Public API, durable event, manifest, package schema, or generated client | Contract-first. |
| State transition, idempotency, owner scope, parser, validator, or policy predicate | TDD. |
| Auth, Airlock, secrets, SSRF, prompt injection, path policy, package activation, or provider credential use | Security-test-first. |
| End-to-end run behavior, repair, worker failure, or model-output failure | Replay-driven. |
| Unknown platform behavior, latency, cost, region availability, or SDK/tooling compatibility | Spike-first. |
| SQL, Blob layout, retention, or schema version changes | Migration-first. |
| Approval UI, operator UI, streaming events, background jobs, or degraded states | UI state-machine. |
| Moving code, splitting modules, renaming packages, or extracting interfaces | Refactor-with-shim. |

Never downgrade from security-test-first to ordinary TDD because the happy path is easy.

## 6. Phase Map

The phases below are optimized for AI-assisted execution. Each phase produces small, reviewable work packets.

## 6.1 Senior Phase Gates

These gates apply across the phase map:

| Phase | Senior gate |
|---|---|
| Phase -1 | No code until owner, non-owner, rollback/disable path, user/operator failure state, tests-first plan, and stop conditions are named. |
| Phase 0 | Contracts include both BMAD install-profile fixtures, invalid fixtures, generated-client compile, all source/licensing provenance, minimum identity/trust, durable work/evidence, provider capability/schema/eval seams, and pinned toolchain validation. |
| Phase 1 | A trusted local BMAD fake slice has success, denial, expiry, duplicate, crash/recovery, and lineage replay without Docker, cloud, provider credentials, shell execution, imported code, or an isolation claim. |
| Phase 2 | Security primitives and the minimal Azure development foundation exist: Entra, managed identities, Bicep, Key Vault, SQL, Blob, ACR remote build, monitoring, ACA environment, and a non-production fixed Job template. |
| Phase 3 | Exact provider/deployment capability, schema projection, `store=false`, budget, evaluated fallback, refusal/incomplete output, credential binding, and model-profile promotion evidence are visible. |
| Phase 4 | The first real isolated effect runs only in a fixed-template ACA Job; failed jobs cannot be mistaken for success and image/spec/attempt/output evidence is complete. |
| Phase 5 | Package activation is reversible/disableable and tool availability changes are explicit and tested. |
| Phase 6A | Artifact adapters use the existing governed contracts and preserve source/output provenance; they do not introduce a second runtime. |
| Phase 6B | Builder content remains proposed until static scan, Azure-isolated rehearsal, evaluation, exact approval when required, and reversible activation complete. |
| Phase 7 | Operators can disable a provider, package, toolset, worker image, or execution lane without redeploy. |
| Phase 8 | Release candidate has rollback drill, fresh-install smoke, backup/restore check, degraded-dependency check, and exception register. |
| Phase 9 | No research candidate becomes default without ADR, threat model, contract, tests, operator visibility, cost model, and rollback path. |

### Phase -1: Agent Orientation

Goal: Prevent context drift before code starts.

LLM context pack:

- [[00 - Common Rules and Product Shape]]
- [[02 - Locked Architecture Decisions]]
- [[89 - Consolidated AI Workspace Source Review and Architecture Improvements]]
- target component note
- target contracts and tests

Agent output:

- implementation brief;
- non-goals;
- files likely touched;
- contracts touched;
- test plan;
- risk list;
- verification commands.

Gate:

- No code changes until the agent can state what owns the behavior, what does not own it, and which tests will prove it.

### Phase 0: Repository, Contracts, and Toolchain Foundation

Goal: Create a compilable BMAD-native skeleton with source-pinned Method/Builder fixtures, fake operational dependencies, and contract generation.

Primary development modes:

- Contract-first development.
- TDD for state primitives.
- Spike-first for toolchain uncertainties.

Work packets:

1. Source Intake records for BMAD Method, BMAD Builder, OpenClaw, Hermes, and Odysseus: origin, immutable ref when available, archive/tree hash, extraction method/completeness, declared version, every component license/notice, verification status, and reuse decision. OpenClaw/Hermes snapshots without a Git ref remain research evidence rather than promotable dependencies.
2. Golden `MethodCliV6` and `StandaloneBuilderSetupV2` install-profile fixtures plus one sealed `BmadFoundationFixture`.
3. Canonical `BmadPackageDescriptor`, `BmadConfigLayer`, capability/help, workflow-step, artifact, source-lineage, and `Draft -> Validated -> Rehearsed -> Approved -> Active` contracts.
4. Monorepo skeleton and pinned toolchain.
5. OpenAPI contract skeleton for Runtime, Package, and Operator APIs.
6. OpenAPI 3.1 / JSON Schema 2020-12 skeletons for `Proposal`, `ExecutionSpecCandidate`, `ApprovalDecision`, `ApprovedExecutionSpec`, `SpecConsumptionRecord`, the `ExecutionResultManifest` union, `Checkpoint`, `EvidenceLedgerEvent`, `EvidenceBundle`, diagnostic `TraceProjection`, `OwnerScope`, `TurnContext`, and `ToolAvailabilitySnapshot`.
7. Generated TypeScript and .NET client compile gate.
8. SQL migration skeleton and Blob layout constants.
9. Fake auth, deterministic fake model, in-memory/fake stores, and fake worker that cannot invoke a shell or imported package code.
10. Durable `WorkItem -> Attempt -> Lease -> Completion -> Outbox` and atomic `EvidenceLedgerEvent` skeleton with stream identity, sequence, checkpoint, idempotency, and recovery contracts.
11. Hosted CI pipeline with .NET 10, Node 24, pnpm 11, the TypeScript 7 application compiler plus isolated TS 6 compiler-API compatibility gate, per-worker Python/uv locks, schema validation, source/license fixtures, and link/doc checks.
12. Minimum trust contracts before the first project: `OwnerScope`, development/user principals, `UntrustedContextEnvelope`, deterministic context budget/redaction, safe archive report, and credential-binding value objects.
13. Provider-neutral `ProviderCapabilities`, `ProviderSchemaProjection`, `ModelProfile`, `ModelEvaluationBundle`, typed failure taxonomy, retention mode, and fallback-boundary contracts.
14. A documented no-container developer profile; Docker, Kubernetes, infrastructure emulators, and local model serving are absent from the required workflow.

Tests first:

- schema examples validate;
- both BMAD install profiles normalize deterministically and malformed/mixed layouts fail with profile-specific findings;
- the sealed package/skill/capability hashes match the Source Intake record;
- generated clients compile;
- state machine rejects invalid transition;
- idempotency key returns original command result;
- fake vertical-slice command can produce a trace event.
- every component license receives an explicit include, clean-room-pattern-only, exclude, or legal-review decision;
- duplicate attempt/lease/outbox delivery and event-cursor gap fixtures recover without re-executing an effect;
- provider schema projection cannot discard a canonical required invariant;
- a clean Windows workflow runs the contract/replay suite without Docker or provider credentials.

Exit criteria:

- A fake request can select the sealed BMAD capability and create a project, thread, run, method step, proposal, exact candidate/approval requirement, fake result import, artifact transition, and evidence summary without Azure, real model calls, Docker, shell execution, or an isolation claim.

### Phase 1: Trusted Simulated Vertical Slice

Goal: Prove the complete BMAD-governed state, approval, recovery, and evidence path with a deterministic trusted test double. This phase does not claim containment and does not run untrusted code.

Primary development modes:

- TDD for lifecycle transitions and Airlock policy.
- Replay-driven development for the end-to-end path.
- UI state-machine development for approval and evidence views.

Work packets:

1. Auth/project/thread/message/run and BMAD method/artifact projection persistence.
2. Load the sealed foundation fixture and render its Help Advisor recommendation.
3. Upload sample repository zip and create immutable snapshot.
4. File tree and read-only viewer.
5. Lexical search and context pack v0.
6. Deterministic fake model BMAD-bound plan and patch output.
7. Proposal normalization and proposal hash with package/skill/step/config lineage.
8. Airlock path, schema, preimage, and command policy.
9. BMAD action/artifact, diff, and approval card plus approval decision route.
10. Build `ExecutionSpecCandidate`, evaluate policy, capture approval of its exact hash when required, then mint an audience-bound, expiring, single-use `ApprovedExecutionSpec`.
11. `sealed_test_fake` deterministically applies one predefined patch only to a sealed temporary fixture; it has no shell, network, dependency restore, package import, or arbitrary command capability.
12. Deterministic in-process validator simulating the expected validation result; real `argv[]` execution is deferred to the Azure lane.
13. `WebWorkerResultManifest`-shaped fake result, bounded log chunks, attempt/lease completion, and outbox output.
14. Idempotent result import, checkpoint, artifact/step transition, atomic Evidence Ledger append, and Evidence Bundle materialization.

Tests first:

- preimage drift blocks patch;
- expired approval cannot dispatch;
- duplicate approval submit does not double-execute;
- worker output without manifest is failure;
- worker has no lifecycle SQL credentials;
- evidence links proposal, approval, spec, manifest, checkpoint, and rollback.
- missing, invalid, or hash-mismatched BMAD capability blocks the run;
- model or worker output cannot advance BMAD step/artifact state before manifest import.
- approving candidate hash A cannot authorize regenerated candidate hash B;
- restarting between completion and import does not lose or duplicate the fake result;
- replay rebuilds projections but cannot invoke the fake executor again;
- an imported package, generated shell string, or arbitrary workspace command is rejected by the `sealed_test_fake` path.

Exit criteria:

- A user can complete the trusted simulated BMAD action through the UI with deterministic fakes and inspect package, skill, step, artifact, candidate, approval, result, recovery, and evidence lineage. The UI labels the run as simulated; no wording implies local sandboxing or production execution.

### Phase 2: Security and Azure Readiness

Goal: Complete the security substrate and minimal Azure development foundation before a real provider or real execution lane is allowed.

Primary development modes:

- Security-test-first.
- TDD for policy and owner scope.
- Contract-first for durable principals and tool snapshots.

Work packets:

1. `OwnerScope` across projects, sessions, uploads, documents, tasks, memory, provider endpoints, packages, and jobs.
2. Principal model: human session, token, worker, scheduler, connector, internal loopback.
3. `ToolAvailabilitySnapshot` service.
4. `UntrustedContextEnvelope` for workspace content and tool outputs.
5. `OutboundUrlPolicy` and SSRF/DNS-pinning adapter.
6. Upload/file path confinement and symlink rejection.
7. Secret redaction and secret provenance reports.
8. Operator audit event stream.
9. Minimal dev/staging Bicep/AVM modules, environment parameters, budget/alerts, teardown/disable path, and IaC build/what-if gates.
10. Entra application/scopes and distinct managed identities for API, remote build, image pull, job start, worker storage, SQL migration, Key Vault, and monitoring.
11. Key Vault, Azure SQL, Blob, ACR, Log Analytics/Application Insights, Container Apps environment, and a disabled/non-production fixed worker Job template.
12. Remote image-build path using ACR Tasks (`az acr build`) or a hosted CI runner—never a local Docker daemon—with lockfile, license, vulnerability, SBOM, provenance, and digest evidence.
13. Fixed job-template policy that rejects request-time image, command, entrypoint, environment, identity, network, or secret overrides.
14. Cloud migration/retention/backup skeleton and least-privilege role-assignment tests before storing production-shaped data.

Tests first:

- token cannot access another owner's resource;
- internal loopback credential cannot be replayed from browser context;
- prompt injection in file/package/memory/tool output cannot alter policy;
- SSRF fixtures deny localhost, metadata endpoints, private IPs, redirects, and DNS failure;
- upload path traversal and symlink escape are blocked.
- clean-machine onboarding and remote image submission pass without Docker, Kubernetes, Azurite, SQL Edge, or local model serving;
- ACR output binds immutable source revision/build definition to scan, SBOM/provenance, and image digest;
- IaC denies an identity that can both alter the approved image/template and start arbitrary jobs;
- fixed job configuration rejects every request-time execution override.

Exit criteria:

- Security fixtures fail closed and the cost-capped Azure development foundation can be recreated from IaC. No real provider or job execution starts until its identities, retention, evidence, and rollback gates pass.

### Phase 3: Real Model Gateway and Structured Outputs

Goal: Replace fake model output with real provider calls while preserving deterministic contract behavior.

Primary development modes:

- Contract-first development.
- TDD for provider resolution and credential binding.
- Replay-driven development with fake provider preserved.

Work packets:

1. `RuntimeProviderResolution`, exact-deployment `ProviderCapabilities`, and parsed-URI `ProviderCredentialBinding` for Azure public and approved sovereign clouds.
2. Microsoft Foundry/Azure OpenAI v1 Responses adapter behind Model Gateway with `store=false`, app-owned conversation/run state, and provider-hosted tools disabled.
3. `ProviderSchemaProjection` from canonical JSON Schema, both hashes retained, provider-subset validation followed by canonical validation, and bounded repair.
4. Prompt-cache contract and transition events.
5. Model-call evidence without raw prompt leakage by default.
6. Budget/quota windows.
7. Typed refusal, incomplete output, content-policy, rate-limit, timeout, capability mismatch, schema failure, and provider failure handling.
8. Versioned role aliases—at minimum `planner`, `schema_repair`, `context_compressor`, and `artifact_reviewer`—mapped to exact Azure deployment/snapshot profiles rather than provider “latest” names.
9. Promotion state `candidate -> offline_evaluated -> policy_approved -> canary -> active -> rolled_back|retired` with immutable evaluation bundles.
10. Separate evaluation lanes for contract adherence, BMAD task quality, safety/privacy, and operations/cost; critical-lane thresholds cannot be averaged away.
11. Explicit fallback graph evaluated before activation; crossing provider, credential, residency, retention, tool, schema, or material-quality boundaries stops rather than silently routing.

Tests first:

- schema-invalid model output cannot create proposal;
- provider fallback emits cache/cost transition;
- credential cannot be sent to unrelated custom base URL;
- a lookalike URL containing `azure.com` in a user-info, path, or attacker-controlled hostname never receives an Azure credential;
- model output with tool-like text but no structured call is treated as text;
- fake provider replay remains deterministic.
- canonical validation catches an invariant omitted by the provider projection;
- refusal and incomplete responses cannot become proposals;
- `store=false` and hosted-tools-off are asserted on every applicable baseline call;
- a model cannot promote itself and a failing critical safety/privacy lane blocks aggregate promotion;
- unevaluated fallback or a boundary-crossing fallback fails closed with visible operator/user state.

Exit criteria:

- A canary-approved exact Azure deployment can produce typed candidate output with complete capability/schema/credential/retention/evaluation evidence. Orchestrator still creates proposals, Airlock still governs effects, fake replay remains available, and rollback to the last evaluated profile is tested.

### Phase 4: Execution Lanes and ACA Jobs

Goal: Introduce the first real isolation and side-effect boundary through finite, fixed-template Azure Container Apps Jobs. There is no local-container predecessor.

Primary development modes:

- Contract-first for worker manifests.
- TDD for command DSL.
- Replay-driven worker failure tests.
- Spike-first for Dynamic Sessions only.

Work packets:

1. `ExecutionLaneSpec`.
2. Worker image skeleton with per-image uv lock/profile, built remotely by ACR Tasks or hosted CI and promoted only by digest with license/scan/SBOM/provenance evidence.
3. ACA Job dispatcher that selects an allowlisted provisioned template; it cannot accept a request-time image, entrypoint, command, identity, secret, or arbitrary environment override.
4. Command DSL and bounded environment.
5. Output limits and redaction.
6. Heartbeat, timeout, cancellation, and crash import.
7. `WebWorkerResultManifest` import idempotency and binding to spec hash, approval, audience, attempt/lease, image digest, workspace snapshot, policy, command DSL, mutable-input hashes, output hashes, and completion nonce.
8. SBOM/provenance/signing gate.
9. Dynamic Sessions benchmark harness as spike, not production lane.

Tests first:

- raw shell denied by default;
- output truncation includes hash/ref;
- timeout imports partial logs and failed state;
- manifest hash mismatch blocks success;
- worker cannot access SQL lifecycle credentials;
- ACA Job and fake worker produce same manifest shape.
- a valid approval for one audience/template/attempt cannot dispatch to another;
- a mutable source or policy change between approval and dispatch invalidates the spec;
- crash after worker completion but before import is recovered through completion/outbox without rerunning the effect;
- remote build and ACA smoke complete from a developer machine with no Docker engine.

Exit criteria:

- ACA Jobs can run approved patch/test commands from fixed templates, emit an importable `WebWorkerResultManifest`, and complete the Evidence Ledger transaction. This is the earliest internal-alpha boundary and is required before arbitrary package rehearsal or any claim of real isolated execution.

### Phase 5: Full BMAD Package Import, Capability Graph, and Activation

Goal: Expand the sealed Phase-1 foundation seam into arbitrary BMAD package import and reversible activation without making package code trusted by default.

Primary development modes:

- Characterization/golden testing.
- Contract-first for package metadata.
- Security-test-first for package activation.

Work packets:

1. Generalize the Phase-0 parsers across `SKILL.md`, `module.yaml`, `module-help.csv`, `bmad-modules.yaml`, TOML configs, `_bmad` manifests, and both `BmadInstallProfile` layouts.
2. Separate installer-managed files, per-module compatibility YAML, user config, custom overrides, and standalone Builder setup state.
3. Package trust classification.
4. Component-level license/notice inventory and explicit `ComponentLicenseDecision`; root repository license is insufficient for bundled skills/assets.
5. Static package scan and safe archive report without importing package modules into the Runtime API process.
6. Digest-pinned Node/Python BMAD import/rehearsal worker that emits neutral `BmadPackageDescriptor` and evidence; the .NET BMAD Kernel consumes data, never upstream executable code in-process.
7. Capability graph and Help Advisor derived from normalized Method semantics.
8. Package activation state machine.
9. Tool availability update after activation.

Tests first:

- valid BMAD package imports;
- malformed package is rejected with useful finding;
- duplicate menu codes and orphan help rows are caught;
- package text with prompt injection remains untrusted during review;
- package activation cannot skip scan/rehearsal/approval.
- a restrictive or incompatible component license blocks import/redistribution even when the archive root is permissively licensed;
- importer/plugin side effects cannot execute inside the Runtime API or model process;
- activation is blocked unless Azure-isolated install and invocation rehearsal uses the exact digest and lock accepted for activation.

Exit criteria:

- Valid packages become available as governed capabilities; invalid packages produce actionable findings.

### Phase 6A: Artifact Adapter

Goal: Add the existing presentation/artifact workflow through core BMAD, proposal, approval, worker, and evidence contracts without creating a parallel agent runtime.

Primary development modes:

- Golden testing for adapter compatibility.
- Contract-first for artifact provenance.
- UI state-machine testing for outline/draft/export decisions.

Work packets:

1. Inventory existing presentation workflow prompts, templates, stages, export behavior, licenses, and source assumptions.
2. Map the workflow to normalized BMAD package/skill/step/artifact records.
3. Add source, outline, draft, and export decision points according to the ordinary-CRUD/governed-mutation boundary.
4. Persist source references, transformation/model profiles, canonical artifact versions, output hashes, and export evidence.
5. Run exports only through the existing fixed ACA Job lane when a real side effect or executable renderer is involved.

Tests first:

- adapter output matches the approved golden fixture or records an intentional versioned deviation;
- export cannot run without policy and exact approval when risk requires it;
- a source/license/provenance gap blocks redistribution;
- retry cannot overwrite an immutable artifact version or duplicate an export.

Exit criteria:

- The artifact path reuses core BMAD, Airlock, worker, Evidence Ledger, and rollback contracts.

### Phase 6B: BMAD Builder Authoring and Evaluation

Goal: Make BMAD Builder the governed authoring/evaluation foundation for new agents, workflows, modules, and skills while keeping generated content inactive by default.

Primary development modes:

- Characterization/golden testing for upstream Builder behavior.
- Contract-first for proposals, variants, graders, and activation evidence.
- Security-test-first for generated content and dependency acquisition.

Work packets:

1. Recognize Builder source skills, install profiles, output shapes, script runtime requirements, and separately versioned package/module/schema metadata.
2. Emit `SkillPackageProposal`/module/agent/workflow drafts with source snapshot, prompt/config/model profile, generated-content hashes, and author/reviewer separation.
3. Run static scan first, then clean-workspace install and invocation rehearsal only in an approved Azure-isolated worker; a clean directory alone is not containment.
4. Resolve dependencies from immutable refs/digests with lock, signer/provenance, component-license, vulnerability, SBOM, and egress evidence.
5. Evaluate baseline versus candidate across quality, trigger precision/recall, determinism, safety, cost, and negative/adversarial fixtures.
6. Require policy and exact human approval where activation risk requires it; activate reversibly and update `ToolAvailabilitySnapshot` explicitly.

Tests first:

- Builder-generated content cannot become active from generation, static validation, or a model-authored score alone;
- failed/unknown scanner, evaluator, dependency, license, or rehearsal state fails closed;
- the exact evaluated digest/lock/config is the only version eligible for activation;
- deactivation restores the previous capability surface without data loss;
- no Builder script executes inside the Runtime API/model process or `sealed_test_fake` lane.

Exit criteria:

- A Builder candidate moves through `Draft -> Validated -> Rehearsed -> Evaluated -> Approved -> Active` with immutable evidence and tested rollback; Method semantics remain the product foundation and runtime policy remains external.

### Phase 7: Operator Console and Operations

Goal: Make runtime health, release evidence, and security posture visible.

Primary development modes:

- UI state-machine development.
- TDD for operator authz.
- Replay-driven incident fixtures.

Work packets:

1. Operator auth and audit isolation.
2. Runtime health dashboard.
3. Effective tool surface view.
4. Provider routing and fallback view.
5. Execution lane and worker image view.
6. Egress denial and owner-scope denial view.
7. Package activation findings view.
8. Release gate evidence dashboard.
9. Incident export with redaction.

Tests first:

- non-operator cannot access operator routes;
- raw secrets and full tokens do not render;
- degraded optional services show clear state;
- release gate dashboard reflects failing/passing evidence;
- incident export includes hashes/refs, not raw privileged traces by default.

Exit criteria:

- Operators can answer "what is safe, what is broken, what changed, and what evidence proves it" without reading raw logs.

### Phase 8: Production Hardening and Release

Goal: Turn the vertical slice and core extensions into a releasable product.

Primary development modes:

- Release-gate-driven development.
- Migration-first development.
- Threat-model-driven testing.

Work packets:

1. Harden and promote the already-proven Phase-2 Bicep/AVM environments; Phase 8 does not first-provision cloud foundations.
2. Review managed identities, role assignments, Key Vault rotation, network controls, separation of duties, and break-glass procedures.
3. Exercise Azure SQL/Blob migrations, retention, deletion, backup, restore, and Evidence Ledger reconciliation.
4. Enforce ACR remote-build, immutable digest, scan, license, SBOM, provenance/attestation, signing, promotion, and rollback gates.
5. OpenTelemetry dashboards and alerts.
6. Backup/restore runbook.
7. Fresh-install smoke test.
8. Load and cost smoke.
9. Security regression suite.
10. Release evidence bundle.

Tests first:

- IaC build/what-if passes;
- fresh environment deploys;
- first operator setup works;
- rollback runbook dry run succeeds;
- replay suite passes;
- security negative suite passes.
- no-Docker clean-machine onboarding, remote build, deploy, ACA Job smoke, and rollback pass;
- model-profile rollback and package/tool/lane kill switches work without redeployment.

Exit criteria:

- Release candidate has evidence for contracts, tests, replay, security, supply chain, infrastructure, observability, and rollback.

### Phase 9: V1.5 Research and Expansion

Goal: Expand only after core governance is proven.

Spike candidates:

- ACA Dynamic Sessions as interactive execution lane.
- Additional Azure regions/providers after residency, capability, credential, cost, and evaluation gates.
- Broad MCP connector marketplace.
- External memory providers.
- Multi-agent package authoring.
- Advanced artifact types.
- Foundry Agent Service evaluation.

Rule:

- No candidate becomes default until it has an ADR, threat model, contract, test fixture, operator visibility, cost model, and rollback path.

## 7. TDD Guidance By Component

| Component | TDD style | Start with |
|---|---|---|
| Runtime state machine | Classic unit TDD | invalid transition, stale version, idempotent duplicate. |
| Airlock policy | Security-test-first | bypass attempt, expired approval, policy hash mismatch. |
| Workspace service | TDD plus property-style path tests | path traversal, symlink escape, preimage drift. |
| Context packs | Fixture TDD | secret redaction, trust class, invalidation. |
| Model Gateway | Contract TDD | invalid structured output, fallback transition, credential binding. |
| Worker execution | Replay-driven TDD | timeout, crash, manifest mismatch, output truncation. |
| BMAD parser | Golden testing | known valid package, malformed package, legacy override. |
| Package activation | Security-test-first | malicious package text, dependency drift, skipped rehearsal. |
| Frontend workbench | UI state-machine tests | reconnect, unknown event, approval expired, evidence loaded. |
| Operator console | Authz and state tests | non-operator denied, redaction, degraded state. |
| IaC | Validation and smoke tests | Bicep build, what-if, deploy smoke, rollback dry run. |

## 7.1 Test Pyramid For This Product

Do not let expensive end-to-end tests carry all confidence. The test suite should be layered:

| Layer | Purpose | Typical owner |
|---|---|---|
| Unit | Pure state, policy, parser, validator, and utility behavior. | Backend, Airlock, Workspace, Package agents. |
| Contract | OpenAPI, JSON Schema, generated clients, event payloads, worker manifests. | Contract agent. |
| Integration | SQL/Blob/event/outbox/authz boundaries and module ports. | Backend and Test agents. |
| Security negative | Bypass attempts, owner leaks, SSRF, prompt injection, path traversal, secret leakage. | Security agent. |
| Replay | Deterministic vertical flows with fake provider and fake worker. | Test/Replay agent. |
| E2E/UI | User-visible workflows, stream reconnection, approval/evidence/operator states. | Frontend agent. |
| Smoke | Fresh install, deployment, worker image, provider probe, degraded optional services. | DevEx/Ops agents. |

Rule of thumb: if a behavior can be proven below E2E, prove it below E2E and use E2E only to prove the wiring.

## 7.2 Flake Policy

Flaky tests are production risks, not annoyances.

| Flake type | Required action |
|---|---|
| Timing-sensitive UI stream | Add deterministic event fixtures, sequence ids, and bounded waits. |
| External provider | Replace with fake provider in CI; keep live provider tests as opt-in smoke. |
| Worker timing | Assert state transitions and manifest content, not exact timestamps. |
| Infrastructure availability | Separate local validation from cloud smoke; record cloud failures as environment evidence. |
| Snapshot churn | Replace broad snapshots with semantic assertions. |

No flaky test may be ignored without owner, expiry, linked issue, and risk statement.

## 8. Standard LLM Work Packet

Every implementation prompt should provide this packet:

```md
# Work Packet

## Mission
One concrete behavior to implement.

## Read First
- vault notes:
- code files:
- schemas/contracts:
- tests:

## Non-goals
- what not to change:

## Stop conditions
- when the agent must stop and ask or update docs:

## Development mode
Contract-first / TDD / security-test-first / replay-driven / spike-first / migration-first / UI state-machine.

## Contracts touched
- APIs:
- JSON Schemas:
- Events:
- SQL tables:
- Blob prefixes:
- package manifests:

## Tests to write first
- unit:
- contract:
- integration:
- security:
- replay/e2e:

## Implementation constraints
- Airlock:
- owner scope:
- worker SQL access:
- command DSL:
- provider boundary:
- secret/log retention:

## Rollback / disable path
- flag/config/revert/migration strategy:

## Observability impact
- events:
- metrics:
- logs:
- operator/user-visible states:

## Verification commands
- command:
- expected result:

## Done means
- acceptance criteria:
- docs updated:
- manifest/evidence updated:

## Context ledger
- files read:
- assumptions:
- risks:
```

## 9. LLM Prompt Patterns

### Implementation Prompt

```text
You are implementing one Sapphirus work packet.
Read the listed files first.
Do not broaden scope.
Write or update failing tests before risky behavior.
Preserve Runtime API lifecycle ownership.
Do not bypass Airlock for side effects.
Do not expose provider SDK objects above Model Gateway.
Do not let worker code write lifecycle SQL.
Return changed files, tests run, and remaining risks.
If ownership, rollback, policy authority, or schema versioning is unclear, stop before editing and report the blocker.
```

### Review Prompt

```text
Review this change as a bug-finding reviewer.
Prioritize security bypasses, state ownership violations, missing tests, schema drift, owner-scope leaks, Airlock bypasses, provider credential leaks, worker SQL writes, and evidence gaps.
List findings first with file/line references.
Do not focus on style unless it causes a concrete risk.
Call out missing rollback paths, missing observability, broad WIP, and unowned state.
```

### Refactor Prompt

```text
Refactor only the named module slice.
Keep behavior unchanged.
Add compatibility shims if callers exist.
Run existing tests and add characterization tests before moving code.
Do not mix behavior changes with file movement.
```

### Spike Prompt

```text
Run a time-boxed spike.
Do not productionize.
Measure the decision criteria.
Return options, evidence, risks, recommendation, and follow-up ADR text.
```

## 10. Story Splitting Rules

Stories should be split when they touch more than one durable boundary.

Split by:

- schema first;
- persistence second;
- service behavior third;
- API route fourth;
- UI fifth;
- replay/evidence sixth;
- docs and runbooks with the same PR when contracts change.

Split again if the review would require more than one expert reviewer to understand the risk.

Do not combine:

- provider integration and repair loop;
- package parsing and package activation;
- worker dispatch and manifest import;
- Airlock policy and UI approval rendering;
- SQL migration and unrelated route behavior;
- refactor/move and behavior change;
- Dynamic Sessions spike and ACA Jobs baseline.
- owner-scope foundation and unrelated feature behavior;
- provider credential storage and provider routing;
- package activation and marketplace UI;
- migration and retention-policy rewrite.

## 10.1 Estimation Heuristics

Use this sizing model before assigning AI agents:

| Size | Description | Expected handling |
|---|---|---|
| XS | One pure function, one schema example, one small UI state. | One agent, direct TDD. |
| S | One route/service behavior with tests and docs. | One implementation agent plus review. |
| M | One durable boundary across API, persistence, UI, or worker. | Split into contract and implementation PRs if possible. |
| L | Multiple components or a security-sensitive path. | Break into S/M stories; design review first. |
| XL | Platform adoption, major refactor, or new execution lane. | Spike and ADR first; do not implement directly. |

If an LLM estimates a story as L or XL, it should not code in the same turn.

## 11. Branch and PR Strategy

Recommended PR sizes for LLM-assisted work:

| PR type | Size target | Must include |
|---|---|---|
| Contract PR | 1 to 5 schemas/routes | examples, generated clients, contract tests. |
| Domain PR | 1 state machine or policy surface | unit tests and invalid cases. |
| Infrastructure PR | 1 environment or module slice | build/what-if and rollback note. |
| Worker PR | 1 command/manifest behavior | replay fixture and output cap tests. |
| UI PR | 1 screen or event-card family | mock payloads and Playwright state test. |
| Refactor PR | 1 module/domain move | characterization tests and shims. |
| Spike PR | docs and measurement only | evidence, recommendation, no production dependency. |

## 11.1 Merge Discipline

| Rule | Requirement |
|---|---|
| Green main | Main branch must stay deployable or at least contract-testable. |
| Generated code | Generated clients/schemas must be regenerated in the same PR as contract changes. |
| Migrations | Migration PRs include fixture data and forward/rollback or forward-only recovery notes. |
| Feature flags | Incomplete risky features are disabled by default and visible to operators. |
| Review ownership | Security-sensitive PRs require security review; infra PRs require ops review; schema PRs require contract review. |
| No unrelated churn | Formatting or refactor churn is isolated from behavior work. |

## 12. Release Gates For LLM Work

An LLM-authored change cannot merge unless:

- tests relevant to its development mode pass;
- generated contracts are updated when schema changes;
- docs/route/catalog notes are updated when public behavior changes;
- new security-sensitive behavior includes a negative test;
- evidence/replay fixtures are updated when side effects change;
- no unrelated files were reformatted or churned;
- no secrets, raw prompts, or raw privileged logs were added;
- the final answer states what was verified and what was not.
- rollback/disable behavior is documented for stateful or risky changes;
- observability events or metrics exist for new failure states;
- owner and retention class are stated for new data.

## 12.1 Exception Process

Exceptions are allowed only when they are explicit:

| Field | Required |
|---|---|
| Exception owner | Named human owner, not an AI agent. |
| Reason | Why the gate cannot pass now. |
| Risk | Security, data, reliability, UX, cost, or schedule impact. |
| Compensating control | What reduces the risk until fixed. |
| Expiry | Date or objective trigger. |
| Exit plan | Test, migration, refactor, or deletion that closes the exception. |

An exception without expiry is architecture debt and cannot ship to v1.

## 13. Concepts LLMs Must Preserve

| Concept | Short definition |
|---|---|
| `Proposal` | A normalized candidate action created by Runtime/Orchestrator from typed model output or user intent. |
| `Airlock` | Policy and approval layer that decides whether side effects may proceed. |
| `ExecutionSpecCandidate` | Fully bound proposed effect—including inputs, mutable hashes, lane, policy, audience, expiry, and expected outputs—whose exact hash is evaluated and, when needed, shown for approval. |
| `ApprovedExecutionSpec` | Audience-bound, expiring, single-use durable authority minted by Airlock only after policy and any required exact-hash approval. |
| `WebWorkerResultManifest` | Worker-produced, attempt-bound result claim imported and validated by Runtime; it reports what happened but does not alone define domain success. |
| `Checkpoint` | Runtime-owned workspace state marker after imported execution results. |
| `EvidenceLedgerEvent` | Durable, sequenced domain evidence written atomically with lifecycle state and outbox; authoritative even if telemetry is missing. |
| `EvidenceBundle` | Canonical user/operator-readable materialization linking intent, proposal, candidate, policy, approval, spec, attempt, result, outputs, state transition, and rollback. |
| `TraceProjection` | Redacted diagnostic/telemetry projection that may sample or fail and is never domain authority. |
| `OwnerScope` | Canonical ownership context for user, project, token, resource, and legacy/null-owner rules. |
| `TurnContext` | Per-run/turn actor, session, prompt, tool, model, approval, and correlation context. |
| `ToolAvailabilitySnapshot` | Effective tool surface after package, actor, policy, dependency, and run-state filtering. |
| `UntrustedContextEnvelope` | Labeled model-context wrapper for files, notes, memory, web content, package text, and tool output. |
| `RuntimeProviderResolution` | Effective provider/model/credential/base URL/fallback result for a model call. |
| `ProviderCapabilities` | Exact deployment/region/model/API/retention/tool/schema capability snapshot attached to a model call. |
| `ProviderSchemaProjection` | Versioned mapping from canonical schema to provider-supported subset, retaining both hashes and canonical validation evidence. |
| `ModelProfile` | Versioned role alias mapped to an exact deployment and settings only after evaluation/canary promotion. |
| `ExecutionLaneSpec` | Defines worker process model, env, network, output caps, cancellation, and cleanup. |
| `SkillPackageProposal` | Staged package/skill change before activation. |

## 14. Anti-Patterns

Reject or rewrite work that does any of these:

- controller writes directly to many tables instead of using domain services;
- worker has SQL lifecycle credentials;
- model output is trusted because it is valid JSON but not schema/policy validated;
- frontend invents payloads instead of using generated clients;
- package discovery imports untrusted code;
- tool schemas are hand-assembled inside prompts;
- raw shell strings become the default command path;
- provider credentials are selected by URL string coincidence;
- `sealed_test_fake` is described as a sandbox, runs imported/generated code, or becomes a prerequisite for real execution;
- onboarding, testing, image building, or deployment requires a local Docker/Kubernetes/model-serving stack;
- provider credentials are released after a substring URL check or before exact endpoint/capability binding;
- a provider response chain, stored response, trace, or in-memory event buffer becomes authoritative run state;
- a model/profile is promoted from one aggregate score, model self-review, or an unevaluated fallback;
- logs become the only way to understand user-visible outcome;
- a refactor also changes behavior without characterization tests.
- an AI agent claims "implemented" without listing tests and skipped checks;
- a story merges with no rollback/disable plan for stateful behavior;
- optional dependency failures are hidden from the operator;
- tests assert prompt wording instead of behavior;
- a spike leaves production code behind.

## 14.1 Senior Engineer Smell Tests

Ask these before merging:

1. Could a new engineer explain this change from contracts and tests alone?
2. Can we roll it back or disable it without data loss?
3. Does the operator know when it fails?
4. Does the user see an actionable state instead of silence?
5. Does this make the first vertical slice stronger?
6. Did we add a reusable boundary or just a convenient shortcut?
7. Would this still be safe if the model output is malicious?
8. Would this still be safe if two runs happen at the same time?
9. Are we relying on a human reading logs to detect correctness?
10. Did we create future cleanup without an owner and date?

## 14.2 Source-Proven Conventions From AI-Developed Runtimes

The complete OpenClaw and Hermes archive snapshots in `_full/` and the available Odysseus source snapshot contain useful repository practices for AI-assisted development. Their test counts, maturity YAML, and author-written policies are evidence of intent and coverage surface—not independent proof that every behavior or score is correct. Adopt the following selectively behind Sapphirus contracts; BMAD Method and Builder remain the authoritative product foundation.

### Repo Instruction Files (OpenClaw, Hermes)

| Rule | Source practice |
|---|---|
| One root policy file | A root `AGENTS.md` holds hard policy and routing only, written telegraph-style so agents actually read all of it. Workflows live in skills/docs, not the root file. |
| Scoped guides | Each major subtree gets a concise scoped `AGENTS.md`; agents must read it before touching that subtree. Cross-tool aliases are generated/validated in hosted CI or represented by ordinary files on platforms where symlinks are unavailable—local Windows setup must not depend on restoring archive symlinks. |
| Intent layer, not just rules | Hermes's root guide states the two properties every change is reviewed against (cache stability, narrow core). Sapphirus's equivalents — Airlock exclusivity, API-owned lifecycle state, argv-only commands — must be stated the same way: as the review lens, not just as prohibitions. |
| Premise verification | Before "fixing" anything, the agent must reproduce the symptom on current main and give a line-level account of where the bug manifests and how the fix changes that line's behavior. Use `git log -p -S <symbol>` to check whether an apparent gap is intentional design — absences can be load-bearing. |

### Review Discipline (OpenClaw)

| Rule | Requirement |
|---|---|
| Evidence map before verdict | A reviewer (human or agent) must assemble: changed surface, entry point, owner boundary, at least one caller and callee, sibling surfaces sharing the invariant, existing tests, and current main behavior. Any missing cell is stated as a gap, not glossed into a conclusion. |
| Best-fix question | Every review explicitly asks whether the PR is the *best* fix, not merely a plausible one — compared against owner boundaries and sibling implementations. |
| Diff-only review is insufficient | Read the whole changed function/module plus callers, callees, sibling implementations, and adjacent tests before verdict. If challenged, read the missing path before defending the verdict. |
| Sibling-surface proof | A one-sided fix needs proof siblings are unaffected, an explanation why, or an explicit follow-up — otherwise it fixes one call path and leaves the same bug class live elsewhere. |
| Dependency inspection gate | Claims about dependency behavior require reading the dependency's source/types/docs, not memory or wrappers. No direct inspection means no verdict. |

### Test Policy (Odysseus, Hermes)

These sharpen sections 7 and 12 with rules proven at scale:

| Rule | Requirement |
|---|---|
| Behavior contracts over snapshots | Tests assert how data must relate (invariants), not frozen current values (model lists, enum counts, config literals). No change-detector tests that break on benign growth. |
| Behavioral-first assertions | Do not assert on source text or AST when the behavior can be driven directly; source-text asserts break on benign refactors and pass on real regressions. The narrow exception (an invariant that cannot be exercised at runtime) must say why in the test docstring. |
| Determinism and isolation | No reliance on wall-clock, network, RNG, collection order, leaked env/module/CWD state. Order-sensitivity is a bug to fix, not a constraint to encode. |
| E2E over green mocks | Resolution/config/domain behavior is first proven with deterministic `sealed_test_fake` adapters and sealed temp fixtures. Anything claiming web isolation, file/command/network side effects, dependency install, or package execution is then exercised through the real fixed Azure lane; mocks cannot prove containment. |
| Never silence red | Do not weaken assertions, add skip/xfail, or delete coverage to make CI green. Distinguish a stale test expectation from a real regression before touching the test. |
| Evidence-driven slow marks | A test is marked slow only from measured duration output, never by guessing, and never to hide a failure. |
| No premature test abstraction | Extract a shared helper only when the duplicated shape is proven across files; document each helper's limits. Small and boring beats clever and general. |

### Change Scoping and Capability Footprint (Hermes)

| Rule | Requirement |
|---|---|
| One kind of change per PR | Never mix file moves with assertion changes, helper extraction with logic changes, or formatting with behavior. A declared refactor's "request" is the extraction itself. |
| Footprint ladder | New capability takes the least-footprint rung that solves it: extend existing code → CLI/skill → availability-gated tool → package/plugin → external tool server → new core surface (last resort, because core surface is paid for on every model call). |
| Shared interface over serial merges | When three or more work items integrate the same category of thing (providers, memory backends, notifiers), design one interface and orchestrator first and convert the items into implementations of it. |
| Config surface bar | Before adding a config option or environment variable, prove existing behavior cannot solve it; prefer consolidating options when touching config surfaces. Environment variables carry secrets only; behavioral settings live in versioned config. |
| No speculative hooks | Extension points without a concrete stated consumer are rejected — removing a hook after packages depend on it is far harder than adding it. |

### One Canonical Path (OpenClaw)

| Rule | Requirement |
|---|---|
| Fallback is a product decision | Before adding any fallback, name the shipped contract it protects, the failure mode, and the removal plan — otherwise delete it. No "if the new path fails use the old one" branches as implementation convenience. |
| Migrations have one owner | A dedicated migration path (the doctor pattern) migrates, verifies, and then runtime assumes the new shape. No dual-write, read-through fallback, or lazy import in steady-state runtime code. |
| Compatibility is opt-in | "Shipped" means reachable from a release tag; unreleased code gets no compat shims. Tests alone do not make internals contracts. |
| Prepared facts on hot paths | Resolve canonical facts (provider id, model ref, capability class) once and carry them forward; do not fix repeated request-time discovery with scattered caches. |

### Parallel Agent Work Scheduling (Odysseus)

Odysseus ships a read-only PR overlap audit (`scripts/pr_blocker_audit.py`) that classifies open PRs into subsystem areas by path/keyword rules and reports collisions. Sapphirus should adopt the same mechanism for scheduling agent work packets: before dispatching parallel packets, audit open branches/PRs by owning area (contracts, Airlock, workspace, workers, UI) and refuse to start a packet whose area already has an open packet in flight. This is the executable form of the WIP limits in section 3.2.

### QA Maturity Register (OpenClaw)

OpenClaw maintains a versioned YAML maturity/coverage register with scenario packs, executable checks, manual probes, and inventory output. The reviewed snapshot also contains human overrides and authored scores, so the register is useful organization—not objective evidence by itself. Sapphirus release gates ([[33 - Release Gates and Acceptance Matrix]]) may keep a similar per-component view only when every score links to executable evidence or a named human override with reason, owner, timestamp, and expiry. Model-authored scores cannot promote a component.

## 15. Recommended First 20 Work Packets

1. Source Intake and component-license decisions for BMAD Method/Builder and the three research runtimes; quarantine every snapshot without immutable upstream identity.
2. Golden `MethodCliV6` and `StandaloneBuilderSetupV2` fixtures plus the sealed first-slice capability and hashes.
3. Canonical BMAD package/config/capability/help/step/artifact contracts and Builder proposed-to-active lifecycle.
4. `OwnerScope`, principal, `UntrustedContextEnvelope`, archive-safety, redaction, and context-budget contracts before project persistence.
5. `Proposal`, `ExecutionSpecCandidate`, policy result, exact approval, `ApprovedExecutionSpec`, `WebWorkerResultManifest`, checkpoint, and Evidence Ledger schemas.
6. `WorkItem -> Attempt -> Lease -> Completion -> Outbox`, event-stream identity/sequence, cursor gap, and replay contracts.
7. Provider capability/schema projection, typed failure, credential/retention, model profile, evaluation, canary, fallback, and rollback contracts.
8. Cloud-first repo skeleton and exact toolchain pins with a no-Docker clean-machine guide.
9. OpenAPI 3.1/JSON Schema 2020-12 skeleton and generated TypeScript/.NET compile gate.
10. Domain/BMAD/lifecycle/idempotency state-machine tests.
11. Runtime API development identity, project/thread/run/method-state persistence using in-memory/fake ports first.
12. Workspace safe upload, immutable snapshot, file manifest, and read-only viewer.
13. Context pack v0 with trust class, owner scope, source refs, deterministic budget, and redaction.
14. Fake model provider and BMAD-bound typed output/refusal/incomplete fixtures.
15. Proposal normalization plus candidate hashing with BMAD, policy, workspace, model, schema, and mutable-input lineage.
16. Airlock policy v0, exact candidate approval card/decision, and single-use audience-bound spec minting.
17. `sealed_test_fake` result path with no shell/network/package execution and simulated validation.
18. Atomic result import, checkpoint, BMAD artifact/step transition, Evidence Ledger, and outbox recovery.
19. Evidence Bundle plus success/denial/expiry/duplicate/crash/gap replay fixtures.
20. First UI walkthrough labeled simulated, with clean-machine verification that uses no Docker, emulator, or local model server.

## 15.1 Reinforced First 30 Work Packets

After the first 20, continue with:

21. `ToolAvailabilitySnapshot`, outbound URL/SSRF, prompt-injection, secret/retention, and owner-isolation hardening fixtures.
22. Minimal dev/staging Bicep, Entra scopes, managed identities, Key Vault, SQL, Blob, ACR, monitoring, Container Apps environment, and disabled fixed Job template.
23. ACR Tasks/hosted-CI remote image build with lock, license/scan, SBOM/provenance, digest, and no-local-Docker smoke.
24. Fixed ACA Job template/start-only identity and request-time override denial tests.
25. Foundry/Azure OpenAI v1 Responses adapter with exact capability/credential binding, `store=false`, hosted tools off, and canonical schema validation.
26. Frozen model evaluation bundle, role-profile promotion, canary, explicit fallback graph, and rollback drill.
27. Worker command DSL, bounded env/network, raw-shell denial, output cap, and redacted log chunks.
28. First real ACA Job run with attempt/result/outbox recovery and full Evidence Ledger binding.
29. Operator degraded-state, provider/eval, remote-build, worker-image, package, and kill-switch dashboard skeleton.
30. Arbitrary BMAD package trust/license classification, static scan, Azure-isolated rehearsal, reversible activation, and clean-cloud-environment smoke.

## 16. Final Rule

LLMs should move fast inside a small box. The box is made of contracts, tests, owner scope, Airlock, worker isolation, evidence, and narrow stories. When the box is clear, agents can be extremely productive. When the box is vague, they will invent architecture.
