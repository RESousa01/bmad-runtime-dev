---
title: "Preserved Critical Review"
aliases:
  - "06 - Preserved Critical Review"
tags:
  - bmad-runtime
  - vault/source-and-research
section: "Source and Research"
order: 6
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
status: current
---


# Preserved Critical Review

> Historical review record. Its cloud/desktop conclusions are evidence, not current authority. Apply the separate `web_managed` and `windows_local` contracts in [[93 - Split Web and Windows Desktop Architecture Plans]] through [[99 - Dual-Delivery Contract and Conformance Specification]].

> This file preserves the uploaded critical technical review verbatim. Use it as the audit source for the corrective architecture decisions applied in the v3 implementation guide.


## Bottom-line diagnosis

The plan is strongest where it defines **non-negotiable safety principles**: chat-first UX, BMAD as canonical, no model-direct writes/runs, Airlock before side effects, deterministic executors, no auto-push, platform-owned workspace state, and trace-as-evidence. The main weakness is not lack of ambition or missing architecture blocks; it is that the document tries to make **BMAD runtime, Builder platform, Artifact Creator, governed coding IDE, operator console, supply-chain program, replay system, accessibility/l10n foundation, and Azure operations baseline** all part of v1/MVP-level thinking. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md) [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md), [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

My direct assessment: **the architecture is directionally sound, but the MVP is too broad, the control-plane boundaries are too soft, and the agentic execution loop needs tighter transaction, concurrency, and policy semantics before implementation starts.**

***

## Key Weaknesses

### 1. The MVP is effectively three products, not one MVP

The document explicitly says the MVP must prove three loops: **BMAD Method**, **Builder**, and **Governed coding**. It also includes Auth/project access, BMAD import/init, Help Advisor, method workflows, Builder Studio, Workspace Intelligence, Implementation Engine, trace/rollback/export, operations, budgets, retention, IaC, and baseline policy tests. That is too much for a first internally usable product. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

**Impact:** the team may build a broad but shallow platform where none of the three loops is production-grade. The hardest differentiator is the governed coding loop, and the document itself says the first vertical slice should prove **chat → context → plan → proposal → approval → job → evidence** because that proves the product is not “just chat.” [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

**Action:** redefine MVP as **one executable vertical slice first**, then layer BMAD package loading and presentation adapter after the core execution loop works.

***

### 2. The Runtime API risks becoming a “god control plane”

The Runtime API is described as coordinating Auth/RBAC, Project, Conversation, Run Orchestrator, BMAD Kernel, Package Registry, Workspace Intelligence, Builder Kernel, Model Gateway, Airlock, Execution Dispatcher, Artifact Service, Trace Service, and Operator/Admin modules. The architecture also says not to split every logical box into services, which is correct, but the current module list still risks a tightly coupled modular monolith without explicit internal contracts. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

**Impact:** if the API process owns orchestration, policy, model calls, package interpretation, workspace state, artifacts, and operator actions without strict internal boundaries, changes to one subsystem will cascade into others.

**Action:** keep the modular monolith, but enforce **internal ports/contracts**: `IRunStateStore`, `IAirlockPolicy`, `IWorkspaceSnapshotStore`, `IExecutionDispatcher`, `IModelGateway`, `ITraceWriter`. Treat these like service contracts even if in-process.

***

### 3. The Airlock trust boundary is conceptually strong but deployment-weak

The plan makes Airlock the boundary before writes, commands, exports, dependency restores, and package imports. But it also says the Airlock policy engine can live inside the Runtime API if test-isolated and fail-closed. That is acceptable for v1 only if Airlock is implemented as a **pure policy kernel** with no bypass path. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

**Impact:** if Airlock is just another API module, a future feature can accidentally call Executor or Workspace Service without policy validation. The document warns not to bypass Airlock, but the architecture must make bypass mechanically difficult, not just culturally prohibited. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md), [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

**Action:** every side-effect entry point should require an `ApprovedExecutionSpec` object created only by Airlock. Workspace writes, command runs, artifact exports, package imports, and dependency installs should reject calls that do not include an approval ID, policy version/hash, spec hash, workspace snapshot/preimage hashes, and expiration.

***

### 4. State ownership is not clean enough around Workspace Service and Executor

The document says workspace state is platform-owned and jobs are stateless workers that do not own authoritative run state. It also says Workspace Service owns snapshots, checkouts, preimages, locks, checkpoints, rollback metadata, and TTL cleanup. But the runtime flow shows jobs writing logs/diffs/artifacts to Blob and job result/validation result to SQL, while executor rules also say workers write structured job status to SQL. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md) [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md), [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

**Impact:** direct SQL writes from executors create ambiguity: is the Runtime API the state authority, or can workers mutate authoritative lifecycle state? That matters for retries, duplicate approvals, partial failures, and replay.

**Action:** executors should write **append-only result manifests** to Blob and optionally emit callbacks/events; the Runtime API should be the only component that advances run/proposal/job state in SQL.

***

### 5. Declared decisions and open decisions conflict

The technology stack states Runtime API = **ASP.NET Core on Azure Container Apps**, streaming = **SignalR preferred**, and production networking = **ACA workload profiles environment by default**. Later open decisions still list Runtime API hosting as **App Service vs ACA**, streaming as **SignalR vs SSE**, and frontend hosting as unresolved. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

**Impact:** this creates implementation ambiguity. A coding agent or developer could follow either the “decision” section or the “open decisions” section and make incompatible choices.

**Action:** mark each decision as one of: `LOCKED`, `TEMPORARY`, `PHASE-0 SPIKE`, or `DEFERRED`. Do not keep a stack table and an open-decision table that disagree.

***

## Design Mistakes

### 1. The roadmap does not fully prioritize the hardest product promise

The document says chat-first agentic coding is a core v1 subsystem and ADR-035 locks it as a v1 requirement. Yet the earlier roadmap puts BMAD runtime kernel and Builder Studio before the governed coding loop. The expanded roadmap later fixes this somewhat by placing Chat shell before BMAD package runtime and Airlock/executor before Agentic Coding MVP. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

**Mistake:** the plan has two competing build orders. One says “prove chat-to-execution first”; another delays that behind BMAD and Builder.

**Correction:** build order should be: **Chat shell → Run state → Airlock → Executor → Evidence → minimal patch/test loop → BMAD package loader → presentation adapter → Builder Studio**.

***

### 2. The BMAD Kernel is assigned too much orchestration responsibility

The BMAD Kernel is supposed to interpret package contracts, resolve workflow stages, determine inputs, normalize actions, and bridge Builder/package records. But it also says it routes artifact/coding/planning tasks to orchestration paths. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

**Mistake:** routing coding and artifact tasks inside the BMAD Kernel risks coupling BMAD package semantics to the broader agentic runtime.

**Correction:** the **Run Orchestrator** should route intents. The BMAD Kernel should only answer BMAD-specific questions: package validity, workflow stage, required inputs, capability graph, and method-state transitions.

***

### 3. Model Gateway boundaries are blurry

The architecture diagram shows `ContextPack → Gateway → AI` and then `Gateway → Proposal → Airlock`, while the request flow says the Model Gateway returns a plan/patch/command/artifact proposal to the Agent, and the Agent sends it to Airlock. Later the Model Gateway is defined as the only component that calls models, enforcing structured output, profiles, prompt assembly, cost, quotas, and redaction. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

**Mistake:** if the Gateway “creates proposals,” it becomes part model provider, part agent, part policy boundary.

**Correction:** Model Gateway should return **typed model outputs only**. The Orchestrator/Agent Kernel should construct `Proposal` records. Airlock should validate proposals. This keeps model access, agent reasoning, and policy enforcement separate.

***

### 4. Builder Studio is too large for v1 as described

Builder Studio MVP includes Agent Builder, Workflow Builder, Module Builder, Setup Builder, Convert Skill, Validate Module, and Eval Runner. The validation pipeline includes frontmatter, module, catalog, config, security, prompt-injection review, evals, packaging, installation rehearsal, and invocation rehearsal. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

**Mistake:** “Builder Studio MVP” is itself a platform.

**Correction:** v1 should support **import/convert one existing workflow and validate one module package**. Full authoring UX can follow after package loading, validation, and execution are stable.

***

## Architectural Risks

### 1. Heavy indexing inside the Runtime API may not scale cleanly

Workspace Intelligence includes scanners, ignore/secret rules, lexical search, structural indexing, context-pack building, and project memory. The deployment split says Workspace Intelligence can start as a Runtime API module plus worker utilities, extracting only if indexing becomes long-running/heavy. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

**Risk:** workspace scanning and structural indexing are exactly the kind of work that becomes long-running/heavy. If implemented inside request/response API paths, user-facing latency and API reliability will suffer.

**Improvement:** treat scanning as an asynchronous job from day one, even if the module remains in the monolith.

***

### 2. The Operator Console shares the same React app by default

The plan says Operator Console can be the same React web app and only split if admin security posture requires it. The Operator surface manages users, roles, project assignment, budgets, model profiles, policy, incidents, trace access, executor images, and audit. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md) [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md), [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

**Risk:** operator functions have a different threat profile from normal project chat. Same app is fine technically, but routes, bundles, permissions, and trace access must be strongly separated.

**Improvement:** keep one deployment if desired, but use separate route guards, admin-specific API scopes, separate audit events, and no raw trace expansion in default operator views.

***

### 3. SQL could become an event/log bottleneck

The plan correctly stores bulky logs/artifacts in Blob and structured state in SQL. It also requires live job status/logs without SQL polling and OpenTelemetry correlation across browser, API, model, proposal, approval, job, artifact/export. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

**Risk:** if run events, streaming updates, validation logs, and trace events are all treated as SQL-first records, the system will fight its own design.

**Improvement:** SQL should store lifecycle state and compact indexes; Blob should store large payloads; streaming should use SignalR/SSE event channels; trace bundles should be materialized asynchronously.

***

## Agentic Loop Gaps

### 1. Concurrency and multi-run edits are under-specified

The loop includes preimage validation, locks, checkpoint creation, and rollback. But it does not define how two active runs against the same workspace interact, whether later runs queue, branch, rebase, or block, or how stale context packs are invalidated. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md), [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

**Fix:** define a workspace concurrency policy: `single-writer`, `multi-reader`, `run branch`, `merge/rebase required`, and `proposal voided by newer checkpoint`.

***

### 2. Partial success semantics are missing

The loop says patch apply, checkpoint, validation, failure analysis, repair, and final evidence. The done criteria say changed files, commands/tests, failures, logs, rollback point, and next step must be disclosed. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

**Gap:** the document does not define whether failed validation after a successful patch should keep the patch, rollback automatically, ask the user, or create a “dirty failed checkpoint.”

**Fix:** add explicit states: `patch_applied_validation_failed`, `kept_for_repair`, `rolled_back`, `user_decision_required`.

***

### 3. Repair-loop handling is too simple

The document caps repair loops and says repeated failures should escalate to the user. That is good, but it does not distinguish deterministic failures, flaky tests, environment failures, dependency failures, policy blocks, and model-caused bad patches. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

**Fix:** expand failure classification before repair: `test_failure`, `flake_suspected`, `infra_failure`, `dependency_restore_failure`, `policy_block`, `patch_generation_error`, `timeout`, `output_limit_exceeded`.

***

### 4. Command execution needs a stricter command model

The document requires command allowlists, network policy, timeouts, resource limits, scoped credentials, and blocked dangerous commands. However, examples still model commands as strings such as `pnpm test`. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md), [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md) [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

**Fix:** represent commands as `argv[]`, not raw shell strings. Add canonical `cwd`, environment allowlist, no shell expansion by default, no `sh -c` unless operator-approved, path canonicalization, symlink checks, and output redaction before model repair.

***

### 5. Approval fatigue is likely

The model requires approval for patch apply and command runs, dependency installs every time, and first-time test/build commands per project. This is safe but may create too many approval cards during normal development. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

**Fix:** introduce scoped reusable approvals: “allow `pnpm test` network-off in this workspace for this run,” bound to command class, cwd, network mode, timeout, executor image digest, and policy hash.

***

## Quality & Safety Gaps

### 1. Trace privacy and trace completeness need reconciliation

One criterion says trace bundles include selected context, prompts, model calls, Airlock decisions, approvals, diffs, jobs, logs, artifacts, and checkpoints with secrets redacted. Privacy defaults say raw prompts and raw context packs are not retained by default in production traces, and Trace UI should expose summaries and hashes by default. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

**Gap:** “include prompts/context” and “do not retain raw prompts/context” can both be true only if the schema explicitly separates **redacted summaries, hashes, and privileged raw payload refs**.

**Fix:** define trace views: `operational`, `evidence`, `privileged_raw`, and `replay_fixture`. Store raw content only under explicit policy.

***

### 2. Prompt-injection defense is good conceptually but needs implementation rules

The document says all workspace content is untrusted and cannot change runtime policies, approval requirements, system instructions, command allowlists, network rules, or secret handling. It also lists adversarial workspace tests and prompt-injection policy regression cases. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

**Gap:** it does not define the actual context rendering protocol.

**Fix:** every context item should be wrapped with provenance, trust level, path, hash, and explicit “untrusted content” delimiters. Model outputs that quote workspace instructions should never be interpreted as policy.

***

### 3. Package import is an executable supply-chain path

The document correctly treats module packages as executable supply-chain artifacts and requires SBOM/provenance/signing for worker images, release artifacts, and module packages. The presentation adapter also includes scripts, templates, fixtures, evals, and schemas. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

**Gap:** imported packages should not be executable immediately after upload, even if structurally valid.

**Fix:** add quarantine states: `uploaded`, `parsed`, `static_valid`, `security_review_required`, `executor_rehearsed`, `registered`. Only `registered` packages should be invokable.

***

## MVP Scope Issues

The MVP is too large. The acceptance criteria run through identity, package loading, config resolution, Builder, evals, workspace scanning, secrets, model costs, budget pause, patch apply, command execution, repair, rollback, trace, operator inspection, backup/restore, presentation workflow, OpenAPI, schema versioning, workload profiles, SBOM/provenance, OpenTelemetry, replay, WCAG, localization, and chat coding criteria AT-068 through AT-071. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

A realistic MVP should be renamed into staged releases:

1. **MVP-0: execution skeleton** — auth, project, conversation, run state, fake model, Airlock proposal, approval card, executor job, logs, trace event, evidence card. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)
2. **MVP-1: governed coding loop** — workspace snapshot, context pack, patch proposal, diff approval, patch apply, one validation command, one repair loop, rollback point. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)
3. **MVP-2: BMAD package runtime** — parse SKILL.md/module.yaml/module-help.csv/config, capability catalog, Help Advisor, one method workflow. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)
4. **MVP-3: presentation adapter** — inventory existing workflow, adapter package, outline approval, PPTX export, validation fixture, trace. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md), [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)
5. **Post-MVP v1 hardening** — Builder authoring, richer Operator Console, replay expansion, supply-chain signing, accessibility/l10n completion, quota dashboards. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md), [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

This sequence preserves the non-negotiables while reducing delivery risk.

***

## Concrete Improvements

### Priority 1 — Align the roadmap around the first executable slice

Make the first implementation target exactly what the document already identifies: **authenticated project → chat message → workspace scan → context pack → fake model plan → Airlock proposal → approval card → executor job → logs → trace → evidence**. Do not let Builder Studio or full BMAD package coverage precede this. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

### Priority 2 — Make the Run Orchestrator the only SQL state mutator for lifecycle state

Workers should produce result manifests; Runtime API should transition `ExecutionJob`, `Run`, `Proposal`, and `ValidationResult` states. This reinforces the document’s rule that jobs do not own authoritative run state. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md), [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

### Priority 3 — Turn Airlock into a pure, testable policy kernel

Airlock should accept a normalized proposal plus workspace/policy context and return `blocked`, `approval_required`, or `approved_by_policy`. Every decision should include policy version/hash, risk factors, required approval scope, and immutable spec hash. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md), [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

### Priority 4 — Separate Model Gateway from proposal ownership

Model Gateway should call models and validate typed outputs; Orchestrator should create proposals; Airlock should validate proposals. This resolves the architecture inconsistency where the diagram implies Gateway emits proposals directly while the request flow routes through Agent/Orchestrator. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

### Priority 5 — Define a strict command DSL

Replace raw command strings with structured command specs: `argv`, `cwd`, `env`, `networkMode`, `timeout`, `resourceClass`, `outputLimits`, `allowedPaths`, `blockedPaths`. The document already requires these controls, but the implementation model should make unsafe shell execution impossible by default. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md), [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

### Priority 6 — Add explicit concurrency and drift rules

Preimage checks are necessary but not sufficient. Add run-level single-writer locks, proposal invalidation on new checkpoint, context-pack freshness checks, and user-visible “proposal stale” states. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

### Priority 7 — Reconcile trace privacy with replay requirements

Define trace layers: redacted operational trace, evidence trace, privileged raw trace, and replay fixture trace. The document’s privacy defaults and trace-completeness criteria need this schema distinction to avoid either losing replay value or over-retaining sensitive content. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md), [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

### Priority 8 — Cut Builder Studio v1 to validation/import before authoring

Keep Builder first-class, but do not build all authoring surfaces immediately. Start with package import, conversion wrapper, deterministic validation, and one eval path; defer rich Agent/Workflow/Setup builders until runtime execution and package validation are stable. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md), [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

### Priority 9 — Move policy regression and replay earlier

Policy regression, malicious Start Here cases, secret leakage, unsafe commands, CI workflow tampering, and replay fixtures should be part of Phase 0/Phase 1 test harness, not late hardening. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md), [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)

### Priority 10 — Resolve open decisions in the document itself

Before coding starts, mark API hosting, streaming, workspace source, file editing model, artifact preview, and semantic search as locked/deferred/spike. The document currently contains both decisions and unresolved options for some of these areas. [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md), [\[onedrive-g...l.kpmg.com\]](https://onedrive-global.kpmg.com/personal/rodrigocsousa_kpmg_com/Documents/Microsoft%20Copilot%20Chat%20Files/bmad-runtime-development-context-v8-expanded.md)
