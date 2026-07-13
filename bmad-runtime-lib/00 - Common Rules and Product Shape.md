---
title: "Common Rules and Product Shape"
aliases:
  - "00 - Common Rules and Product Shape"
tags:
  - bmad-runtime
  - vault/foundation
section: "Foundation"
order: 0
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: implementation-plan-library
status: v6-modernized-validated-implementation-guide
generated_on: 2026-07-09
review_pass: v6-modernization-and-platform-validation
architecture_rule: governed-chat-first-agentic-runtime
---



# Common Rules and Product Shape

## 0. Dual-delivery authority (V6.17)

Sapphirus is one governed BMAD product with two distinct delivery models. A project sets `deliveryModel` once and every run inherits it:

| Delivery model | Workspace and lifecycle authority | Ordinary execution | Durable evidence authority |
|---|---|---|---|
| `web_managed` | ASP.NET Core control plane over Azure SQL/Blob and Cloud Workspace Service | Fixed isolated Azure executor; ACA Jobs are the reference implementation | Azure SQL ledger plus immutable Blob payloads |
| `windows_local` | Signed Tauri/Rust host over a user-selected folder and app-local state | Approval-gated local Rust/Win32 runner | SQLite ledger plus encrypted local content-addressed payloads |

BMAD semantics, Airlock rule identifiers, canonical schemas, model profiles, UI vocabulary, and conformance fixtures are shared. Workspace authority, lifecycle database, approval token, executor process, and checkpoint store are delivery-specific and must never be shared by fallback. Azure supports desktop identity, licensing, model access, packages, optional sync, telemetry, and explicit remote jobs, but does not perform ordinary local edits. See [[93 - Split Web and Windows Desktop Architecture Plans]] and [[94 - Windows Desktop Native Host and IPC]] through [[99 - Dual-Delivery Contract and Conformance Specification]].

## V6.18 BMAD foundation delivery lock

[[100 - BMAD Method and Builder Deep Comprehension Audit]] is the semantic source map for the pinned Method and Builder snapshots. The product sequence is locked as follows:

1. Freeze source semantics and explicit parser/validator/runtime profiles before defining generalized BMAD contracts.
2. Prove one real, source-derived sealed Method path and, in the same early foundation milestone, support inactive Builder `Build`, `Edit`, and `Analyze` drafts for one stateless agent and one simple workflow.
3. Enable real conversational Builder authoring only after Model Gateway is proven; generated files remain inactive drafts.
4. Run isolated evaluation and rehearsal—including install, invocation, and the four Builder evaluation modes—only after the applicable delivery-specific governed execution boundary is proven.
5. Promote, sign, publish, or activate only the exact evidenced digest after validation, evaluation, rehearsal, policy, and rollback gates pass.
6. Defer memory agents until owner-scoped durable storage exists, and autonomous agents until storage, scheduler, quiet-hours, lifecycle, and containment contracts pass.

These gates are shared, while execution and activation evidence remain `web_managed` or `windows_local`. `Convert` is **not** an upstream Builder capability in the pinned source; any Sapphirus conversion experience is an adaptation built on `Build`/`Edit` and must not be represented as a native BMAD Builder command. This overlay controls where older phase text defers all Builder authoring until after package or presentation breadth.

## 1. Product Definition

Sapphirus BMAD Runtime is a **chat-first, BMAD-native, governed agentic runtime** for running BMAD Method workflows, building BMAD assets, adapting the existing presentation workflow as a BMAD package, and performing controlled coding work under Airlock.

It is not a generic chatbot, not an IDE plugin, not a thin wrapper over Claude Code/Copilot, not a pure document generator, and not a microservice showcase. The runtime owns state, proposals, approvals, execution, evidence, rollback, and artifact export.

## 2. Final Architecture Principle

```text
read / reason / propose
→ Airlock policy + explicit approval
→ deterministic write / run / export
→ evidence + rollback
```

The model is allowed to reason and propose. The platform decides whether a proposal is valid. The user approves side effects. The executor performs deterministic side effects. The runtime records evidence.

## 3. Non-Negotiables

| Rule | Meaning | Implementation Consequence |
|---|---|---|
| Chat is the shell | The primary workflow is a project conversation with side panels. | Every flow must emit conversation cards and run events. |
| BMAD is canonical | Runtime semantics use BMAD packages, skills, modules, help catalogs, and config. | No Cortex namespace in final runtime objects. |
| Model cannot write | The model returns typed outputs only. | No model tool can directly mutate files. |
| Model cannot run commands | Commands are proposals, not actions. | All command execution requires Airlock and an execution lane. |
| Airlock before governed side effects | Model- or automation-originated workspace writes, command runs, exports, package activation, worker dispatch, and policy-sensitive network actions go through policy. | Executor accepts only `ApprovedExecutionSpec`; ordinary authenticated control-plane CRUD uses authorization, idempotency, and audit instead of recursively requiring Airlock. |
| Delivery-specific authority owns state | Models, UIs, workers, sync services, and remote jobs do not advance authoritative lifecycle state. | Web API imports remote manifests into SQL; desktop Rust host records local results in SQLite. |
| Workspace state is authority-owned | Snapshots/checkouts/checkpoints are not improvised per executor. | Cloud Workspace Service owns web state; Desktop Local Workspace owns selected-folder checkpoints and rollback. |
| Traces are evidence | Logs are not random debug noise. | Every user-visible outcome is reconstructable by hash/reference. |
| No auto-push in v1 | The runtime can prepare commits/bundles but not push. | Remote writes are explicit v1.5/v2 ADR only. |
| v1 is not a microservice mess | Logical modules live inside few deployable units. | Use ports/interfaces internally before extraction. |

## 4. Corrected MVP Scope

The original context was too broad because it tried to launch BMAD runtime, Builder platform, Artifact Creator, governed coding IDE, operator console, supply-chain program, replay system, accessibility, localization, and operations baseline together. The corrected MVP is a staged vertical slice.

### Phase Order

1. Source Intake, license/adoption decisions, and pinned BMAD Method/Builder compatibility fixtures.
2. Canonical BMAD, identity, context, work, evidence, API, SQL, and Blob contracts with fake adapters.
3. A trusted simulated BMAD-native slice using one sealed capability, a fake model, and a non-isolating `sealed_test_fake` that emits the production result-contract shape but cannot run a process, network, dependency, or imported package. It is not the Windows desktop product.
4. Security/context/replay/secret/egress hardening plus the minimum Azure foundation and remote ACR/hosted-CI image build; no local Docker/Kubernetes/emulator/model server is required.
5. One real Microsoft Foundry/Azure OpenAI v1 Responses adapter behind provider-neutral capability, schema-projection, credential, evaluation, fallback, and rollback contracts.
6. Fixed-template ACA Jobs as the first real isolated execution lane, without changing approved-spec, attempt/outbox, manifest, or evidence contracts.
7. Arbitrary BMAD package import, validation, rehearsal, and reversible activation.
8. Existing presentation workflow adapter.
9. Builder authoring/evaluation surfaces over the package-quality plane established in Phase 0.
10. Operator, supply-chain, migration, recovery, and release hardening.

The sealed Phase-0/1 BMAD seam is not the arbitrary package loader. It proves Method and Builder compatibility early without pulling package breadth or visual Builder Studio into the first slice.

## 5. Component Boundary Rules

### Runtime API

The Runtime API can be a modular monolith, but it must not become a god object. Enforce ports:

```csharp
public interface IRunStateStore { /* append events, transition state, read snapshots */ }
public interface IAirlockPolicy { AirlockDecision Evaluate(Proposal proposal, PolicyContext context); }
public interface IWorkspaceSnapshotStore { SnapshotRef CreateSnapshot(SourceRef source); }
public interface IExecutionDispatcher { ExecutionRef Dispatch(ApprovedExecutionSpec spec); }
public interface IModelGateway { Task<ModelOutput<T>> CompleteStructured<T>(ModelRequest request); }
public interface ITraceWriter { Task AppendTraceEvent(TraceEvent evt); }
```

### BMAD Kernel

The BMAD Kernel does **BMAD-specific interpretation only**:

- package validity;
- workflow stage and capability graph;
- required inputs and outputs;
- config merge semantics;
- method-state transition validation.

The Run Orchestrator routes intents. Do not let BMAD Kernel become the general agent orchestrator.

### Model Gateway

The Model Gateway returns typed model outputs only. It does not construct platform `Proposal` records and does not enforce execution policy. Typed outputs become proposals only inside the Orchestrator/Agent Kernel.

### Executor

Executor workers are not authoritative state owners. They write:

- append-only result manifests to Blob;
- structured stdout/stderr chunks to Blob/log stream;
- optional callback/event notification.

The Runtime API validates and imports the manifest, then advances SQL state.

## 6. Side-Effect Contract

The runtime distinguishes authorization from human approval:

| Action class | Required authority |
|---|---|
| Ordinary authenticated control-plane CRUD, such as creating a project, thread, message, approval decision, or policy audit row | Owner authorization, idempotency, concurrency control, and durable audit. It does not recursively require Airlock. |
| Offline build-time Source Intake, fixture generation, dependency restore, and CI | Repository policy, reviewed lockfiles, provenance, and CI evidence. These are not runtime user actions. |
| Read-only but policy-sensitive operations, such as provider calls or outbound fetches | Egress, credential, owner, data-classification, and budget policy; human approval only when the configured risk class requires it. |
| Model- or automation-originated workspace mutation, command execution, export, package activation, external mutation, or worker dispatch | Airlock evaluation and an exact `ApprovedExecutionSpec`; human approval is mandatory where policy marks the candidate approval-required. |

Before approval, the Orchestrator and Airlock produce an immutable `ExecutionSpecCandidate` containing the exact command, cwd, image/lane class, environment policy, inputs, outputs, network policy, limits, workspace target, and hashes. When policy requires a human decision, the user approves the candidate hash. Airlock may then mint an `ApprovedExecutionSpec` only from that unchanged approved or explicitly policy-authorized candidate:

```json
{
  "kind": "ApprovedExecutionSpec",
  "spec_version": "execution-spec.v1",
  "execution_spec_candidate_id": "esc_...",
  "execution_spec_candidate_hash": "sha256:...",
  "approval_id": "appr_...",
  "executor_audience": "lane:local-fake-v1",
  "single_use_nonce": "nonce_...",
  "policy_version": "airlock-policy.v1",
  "policy_hash": "sha256:...",
  "proposal_id": "prop_...",
  "proposal_hash": "sha256:...",
  "workspace_snapshot_id": "snap_...",
  "checkpoint_base_id": "chk_...",
  "execution_lane": {"class":"local_fake","image_digest":null},
  "command": {
    "argv": ["pnpm", "test"],
    "cwd": "/workspace/project",
    "environment_policy_hash": "sha256:..."
  },
  "preimage_hashes": [{"path":"src/App.tsx","sha256":"..."}],
  "declared_outputs": [{"path":"test-results/**","max_bytes":10485760}],
  "expires_at": "2026-07-09T12:00:00Z",
  "side_effects": ["file_write", "command_run"],
  "execution_limits": {"timeout_seconds": 900, "cpu": 2, "memory_mb": 4096, "network": "none"}
}
```

No governed workspace write, command run, package activation, artifact export, runtime dependency install, worker dispatch, or external mutation is accepted without this object. Approval is invalid if the candidate hash, proposal hash, policy hash, target snapshot, mutable input, lane/image identity, or execution limit changes before dispatch.

## 7. Command Model

Commands must be structured as `argv[]`, not shell strings.

```json
{
  "command_class": "test",
  "argv": ["pnpm", "test"],
  "cwd": "/workspace/project",
  "environment": {"CI":"true"},
  "network_mode": "none",
  "timeout_seconds": 600,
  "expected_effect": "Run unit tests without modifying source files"
}
```

Rules:

- no implicit shell expansion;
- no `sh -c` by default;
- canonicalize cwd and all path arguments;
- reject symlink escape;
- block destructive commands unless explicitly operator-approved;
- redact output before it can enter model repair context.

## 8. Trace Privacy Model

Trace completeness and privacy are reconciled through views:

| View | Contents | Audience |
|---|---|---|
| Operational | IDs, statuses, latency, costs, error class, no raw context. | Operators. |
| Evidence | redacted summaries, hashes, file diffs, approvals, job manifests. | Reviewers/developers. |
| Forensic | privileged raw payload references where retained. | Security/admin only. |

Production traces retain raw prompts/context only when policy allows. Default evidence uses summaries, hashes, and redacted payload references.

`EvidenceBundle` is the user/reviewer-facing materialization over authoritative objects and `EvidenceLedgerEvent` records. `TraceBundle` is a diagnostic or privileged forensic export only; it never authorizes work or advances lifecycle state.

## 9. Implementation Definition of Done

A block is not done until it has:

- typed API/schema contracts;
- unit tests;
- integration tests;
- failure-state tests;
- security/policy tests where relevant;
- telemetry spans/events;
- evidence output;
- rollback/retry semantics where side effects exist;
- documentation in the matching `.md` file;
- at least one fixture in the replay corpus if it affects workflows.


## Reference Basis

This file is derived from the uploaded **Sapphirus BMAD Runtime v8 expanded context** and the uploaded **Critical Technical Review**. It also aligns implementation choices with current public platform references checked on 2026-07-09:

- Azure Container Apps jobs: https://learn.microsoft.com/en-us/azure/container-apps/jobs
- Azure Container Apps environments: https://learn.microsoft.com/en-us/azure/container-apps/environment
- Azure Container Apps dynamic sessions: https://learn.microsoft.com/en-us/azure/container-apps/sessions
- Azure App Service authentication/authorization and Microsoft Entra provider setup: https://learn.microsoft.com/en-us/azure/app-service/overview-authentication-authorization and https://learn.microsoft.com/en-us/azure/app-service/configure-authentication-provider-aad
- Azure OpenAI / Azure AI Foundry structured outputs: https://learn.microsoft.com/en-us/azure/foundry/openai/how-to/structured-outputs
- Azure Key Vault authentication and RBAC direction: https://learn.microsoft.com/en-us/azure/key-vault/general/authentication and https://learn.microsoft.com/en-us/azure/key-vault/general/access-control-default
- OpenAPI: https://www.openapis.org/ and https://spec.openapis.org/oas/v3.1.0.html
- OpenTelemetry .NET: https://opentelemetry.io/docs/languages/dotnet/
- OWASP Top 10 for LLM Applications: https://owasp.org/www-project-top-10-for-large-language-model-applications/
- SLSA provenance: https://slsa.dev/spec/v0.1/provenance

---

## v2 Review Improvements

### 1. Audit Verdict

The library is now treated as a **contract library**, not a long-form product narrative. Every downstream implementation file must define:

- owned runtime objects;
- inbound and outbound ports;
- state transitions it may perform;
- side effects it may request but not perform directly;
- failure states;
- release-gate tests;
- telemetry and evidence obligations.

A block file is considered incomplete if it only describes responsibilities without naming contracts, state, tests, and integration points.

### 2. Canonical Build Order

The implementation order is locked as follows:

| Rank | Capability | Reason |
|---:|---|---|
| 1 | Source Intake plus Method/Builder compatibility fixtures | The product foundation must be reproducible before code derives contracts or fixtures from it. |
| 2 | Canonical BMAD, owner/principal, untrusted-context, work, event, evidence, and API contracts | These identities cannot be safely retrofitted after data and streams exist. |
| 3 | Sealed BMAD capability/help seam with fake model and non-isolating trusted fake | Proves a BMAD-native product without arbitrary package breadth, real effects, or a local container/model stack. |
| 4 | Proposal → candidate → policy → exact approval → approved spec → manifest import | Proves the governance invariant and durable evidence chain. |
| 5 | Context/security/replay/egress/secrets plus Entra/MI, Key Vault, SQL, Blob, ACR remote build, monitoring, ACA environment, and fixed disabled Job template | Makes real provider and hosted execution safe before consumers arrive. |
| 6 | Evaluated real Model Gateway adapter | Replaces the fake while preserving typed/replayable contracts, app-owned state, canonical schemas, and explicit rollback. |
| 7 | Fixed ACA Job execution lane | Introduces the first real isolated effect while preserving candidate/spec, attempt/outbox, manifest, and Evidence Ledger contracts. |
| 8 | Arbitrary BMAD package validation and reversible activation | Expands the sealed seam only after runtime authority is proven. |
| 9 | Presentation adapter, then Builder authoring/evaluation | Reuses the package, Airlock, worker, artifact, and evidence planes. |
| 10 | Operator and release hardening | Promotes evidence-backed surfaces rather than introducing their first production dependencies. |

Any implementation plan that starts with broad Builder Studio, Artifact Creator, arbitrary package breadth, or real ACA execution before the sealed simulated BMAD slice and Phase-2 Azure/security foundation is considered a scope regression.

### 3. Universal State Ownership Rule

| State Type | Sole Authoritative Writer | Non-authoritative Writers |
|---|---|---|
| Run lifecycle | Runtime API | None |
| Proposal lifecycle | Runtime API | None |
| Approval lifecycle | Runtime API / Airlock module | None |
| Work item/attempt/lease lifecycle | Runtime API / Work Dispatcher module | Workers submit heartbeat/completion facts through authorized commands; they do not mutate lifecycle tables directly |
| Job lifecycle | Runtime API imports worker manifest | Worker manifest in Blob is evidence, not SQL truth |
| Workspace snapshot/checkpoint | Workspace Service module via Runtime API | Workers can produce candidate manifests only |
| Trace index | Runtime API / Trace module | Workers can write raw payloads to Blob |
| Evidence ledger and projection checkpoints | Runtime API in the same transaction as state changes/imports | Stream transports, OTEL, dashboards, and diagnostic traces are rebuildable projections |
| Artifact metadata | Runtime API / Artifact module | Workers can write artifact objects to Blob |
| Policy version | Operator/API controlled config | Workspace content cannot modify it |

### 4. Universal Proposal-to-Execution Invariant

```text
ModelOutput
→ Orchestrator validates schema and creates Proposal
→ Orchestrator/Airlock create and hash exact ExecutionSpecCandidate
→ Airlock evaluates Proposal, candidate, and policy context
→ User approves the exact candidate hash when required
→ Airlock mints ApprovedExecutionSpec bound to the unchanged candidate
→ Execution Dispatcher submits immutable spec
→ Worker writes append-only manifest
→ Runtime API imports manifest and advances state
```

No implementation may perform a governed workspace mutation, command execution, artifact export, package activation, runtime dependency install, worker dispatch, or external mutation without an `ApprovedExecutionSpec` minted by Airlock. Normal authenticated control-plane CRUD and offline build/CI work follow their own authority classes defined in section 6.

### 5. Required Release Gates

| Gate | Minimum Evidence |
|---|---|
| Contract gate | OpenAPI schema, JSON Schema, generated DTO/client compile. |
| Policy gate | Side-effect endpoint rejects missing/expired/spec-hash-mismatched approvals. |
| Workspace gate | Preimage drift rejects stale patches. |
| Execution gate | Worker cannot write authoritative SQL state. |
| Evidence gate | Final report contains proposal, approval, spec, job manifest, changed hashes, logs, and rollback reference. |
| Security gate | Prompt-injection fixture cannot bypass policy or approval. |
| Observability gate | Trace ID connects browser event → API span → model call → approval → job → artifact. |
| Recovery gate | Failed validation produces explicit partial-success state and user decision. |

### 6. Review Fixes Applied

| Finding From Review | Library Correction |
|---|---|
| MVP was three products. | MVP now starts with one executable vertical slice. |
| Runtime API risked becoming god control plane. | Ports/interfaces and state ownership rules are explicit. |
| Airlock was conceptually strong but deployment-weak. | `ApprovedExecutionSpec` is mandatory for all side effects. |
| Workers could mutate SQL lifecycle state. | Workers produce Blob manifests; Runtime API imports and transitions. |
| Build order conflicted. | Canonical build order is locked above. |
| BMAD Kernel had too much routing responsibility. | Run Orchestrator routes; BMAD Kernel only interprets BMAD. |
| Model Gateway boundary was blurry. | Gateway returns typed outputs only. |
| Builder MVP was too large. | v1 Builder is import/convert/validate one package. |
| Indexing risked blocking API requests. | Workspace Intelligence scanning is async from day one. |
| Command model was too loose. | Commands are `argv[]`, not shell strings. |

### 7. Completion Standard For Every Block File

Each architecture block must answer these questions before implementation starts:

1. What does this block own?
2. What does it explicitly not own?
3. What interfaces does it expose?
4. What interfaces does it call?
5. What state transitions can it perform?
6. What side effects can it request?
7. What evidence must it write?
8. What policy gates protect it?
9. What failure states can it produce?
10. What tests prove it works?


---

## Historical Revision Notes (V3 -> V4)
## Review finding

`00 - Common Rules and Product Shape.md` is part of the implementation library support layer. In v3, support files were useful but not always testable. In v4, every support file must provide either a decision, reference contract, release gate, mapping, runbook, or checklist that can be executed by a developer or coding agent.

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

## BMAD Foundation Boundary

BMAD Method and BMAD Builder are the foundation of the AI Workspace, but they own different authority from the runtime. The product must preserve this split from the first commit:

| Layer | Foundation responsibility | Must not own |
|---|---|---|
| BMAD Method kernel | Method phases, workflows, skills, help/capability graph, artifact expectations, configuration semantics, and next-action guidance. | Authentication, tenant/owner authorization, provider credentials, side-effect approval, worker dispatch, or lifecycle SQL. |
| BMAD Builder quality plane | Authoring, conversion, quality analysis, module scaffolding, validation, eval definitions, and rehearsal inputs. | Direct activation, unsandboxed eval execution, policy bypass, or mutation of the active package catalog. |
| Sapphirus agent runtime | Turns, runs, context packs, model calls, tool-call normalization, streaming, detached execution, and failure recovery. | Redefining BMAD package/method semantics ad hoc. |
| Airlock and execution plane | Policy, approval, exact execution specs, isolation, manifests, evidence, checkpoints, and rollback. | Treating model or package instructions as authority. |
| Workspace experience | Projects, chat, artifacts, Builder surfaces, source/evidence views, and operator UX. | Becoming a second source of truth for method, run, or approval state. |

Locked consequences:

1. The first executable slice is BMAD-native: it runs one sealed, pinned Method-compatible skill/workflow fixture and records package, skill, workflow-step, config, and artifact lineage.
2. The full arbitrary package loader and visual Builder Studio may arrive later, but their canonical data contracts and draft-to-active lifecycle exist in Phase 0.
3. Builder output is always `Draft -> Validated -> Rehearsed -> Approved -> Active`; generation alone never confers trust.
4. OpenClaw, Hermes, and Odysseus are pattern sources, not product kernels or required runtime dependencies.
5. Source reuse must respect license, notices, provenance, and trademark boundaries; architectural learning is not permission to copy implementation without review.

## Hermes-Informed Product Rules

Source: [[86 - Hermes Source Code Review - Agent Runtime and Learning Loop]].

| Rule | Meaning | Implementation effect |
|---|---|---|
| Narrow runtime core | A capability is not added to the always-loaded core unless every run needs it. | Prefer BMAD packages, SkillOps packages, connectors, service-gated tools, and worker manifests before core runtime features. |
| Stable active-run contract | The system prompt, tool schema set, and context pack hash are stable during a run except for explicit compression/finalization transitions. | Persist `PromptCacheContract` and reject hidden mid-run toolset/system-prompt mutation. |
| Airlock is governance, not containment | Airlock authorizes, approves, and audits side effects; it is not an OS security boundary. | Security docs and UX must distinguish Airlock from container/process isolation. |
| Context-local authority | Approval/session/run/tool identity must travel in typed context objects, not mutable process-global flags. | Parallel sessions and background jobs cannot share approval callbacks by accident. |
| Self-improvement is staged | Background package, memory, or skill writes are proposals with provenance. | SkillOps creates pending write records and requires review before activation. |

## Odysseus-Informed Product Rules

Source: [[88 - Odysseus Source Code Review - Self-Hosted AI Workspace]].

| Rule | Meaning | Implementation effect |
|---|---|---|
| Private/cloud deployment is still privileged | An internal Azure deployment can run commands, touch files, call providers, and expose secrets. | Do not let “internal” or “development” mode bypass auth, authorization, Airlock, fixed-job isolation, or evidence; local/self-hosted deployment is not a v1 requirement. |
| Owner scope is product shape | Sessions, uploads, documents, memory, tasks, tokens, and provider endpoints need an explicit owner. | Make `OwnerScope` mandatory on user data and hide object existence on mismatch. |
| Internal loopback is not a shortcut | In-process tool calls may need special credentials, but they still represent a real principal. | Add `InternalLoopbackPrincipal` with startup-only credentials and privilege checks before dispatch. |
| Network egress is a resource | URLs, webhooks, search fetches, model endpoints, and provider probes all cross a trust boundary. | Add `OutboundUrlPolicy` and `DnsPinnedFetch` before enabling arbitrary outbound fetches. |
| Constrained models need smaller tools | Any selected Azure deployment with a smaller context/tool budget needs explicit prompt, context, and tool diets. | Tool selection, context compaction, and budget events are core runtime behavior, not polish; this does not create a local-model requirement. |

## Consolidated AI Workspace Doctrine

Source: [[89 - Consolidated AI Workspace Source Review and Architecture Improvements]].

Across BMAD, OpenClaw, Hermes, and Odysseus, the durable product rule is:

```text
BMAD Method frames the work.
BMAD Builder proposes/evaluates extensions.
Models propose.
Runtime normalizes.
Policy evaluates an exact candidate.
Humans approve the candidate hash when required.
Airlock mints single-use authority.
Fixed workers execute.
The applicable `ExecutionResultManifest` branch reports executor-observed facts; the delivery authority validates and imports them.
Runtime imports.
Evidence Ledger proves; EvidenceBundle explains.
Packages extend only after validation.
```

Any new feature that skips one of those verbs is probably smuggling side effects, state mutation, package activation, or provider authority into the wrong layer.

Before accepting a new runtime capability, ask:

1. Is this needed by the first executable vertical slice?
2. Is it a package/connector/tool before it is a core runtime feature?
3. Does it have owner scope, principal context, policy gate, schema, failure state, and evidence?
4. Can it be replayed or tested with a negative/bypass fixture?
5. Does it preserve the Runtime API as lifecycle owner and workers as manifest producers?
