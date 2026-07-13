---
title: "Hermes Source Code Review - Agent Runtime and Learning Loop"
aliases:
  - "Hermes Structured Code Review"
  - "Hermes Agent Source Review"
tags:
  - bmad-runtime
  - vault/source-and-research
section: "Source and Research"
order: 86
vault_role: "source-review"
project: "Sapphirus BMAD Runtime"
status: current
reviewed_on: 2026-07-09
review_revision: "V6.16"
source_archives:
  - "C:\\Users\\rodrigocsousa\\Downloads\\hermes-agent-main.zip"
related:
  - "[[83 - BMAD Source Code Review - Method and Builder]]"
  - "[[84 - OpenClaw Source Review - Comparable Runtime Patterns]]"
  - "[[85 - OpenClaw Structured Code Review]]"
---

# Hermes Source Code Review - Agent Runtime and Learning Loop

> Historical source evidence. Apply reusable agent/session/learning patterns separately to the .NET web authority and Rust desktop authority; do not infer shared filesystem, state, approval, or executor code.

## Review Scope

Reviewed the complete Hermes source archive provided at `C:\Users\rodrigocsousa\Downloads\hermes-agent-main.zip`, SHA-256 `E5E0941C515867EC024B343E775D07F34B323B363CB0570863CF6690B9291095`. The ZIP manifest contains 7,075 entries: 6,205 regular files, 870 directory entries, and 133,029,443 uncompressed bytes. All regular files were extracted and verified at `_full/h/hermes-agent-main`.

The earlier `_source_review/hermes-agent-main/hermes-agent-main` tree contained 5,909 files. Every shared file was SHA-256 compared with the full extraction: zero files were missing from the full tree and zero content mismatches were found. The 296 recovered files are all under `website/` and consist of Docusaurus localization/content, site components and build scripts, package/config files, and static media. No agent runtime, provider, cron, tool, plugin, workflow, or test implementation was recovered, so the runtime conclusions below are unchanged.

The content snapshot is complete, but revision provenance is not: the archive contains no `.git` directory, commit, or tag. It reports Hermes `0.18.2`, release `2026.7.7.2`. Root licensing is MIT, but component licensing is heterogeneous and must be evaluated per path rather than inferred from the root license.

## V6.16 Audit Corrections

This pass checked the review against the extracted executable source rather than relying on descriptive docs alone.

| Area | Corrected finding and AI Workspace consequence |
| --- | --- |
| Snapshot provenance | Archive identity, SHA-256, entry counts, byte counts, and extraction completeness are now verified. The archive still has no `.git`, immutable commit, or tag, so `SourceSnapshot` remains incomplete until an upstream URL and immutable revision are bound to this digest. |
| Component licenses | Root `LICENSE` is MIT, while `plugins/security-guidance/patterns.py` is Apache-2.0-derived with a component notice, `plugins/hermes-achievements` and `skills/creative/humanizer` have separate MIT attributions, and `skills/productivity/powerpoint` is proprietary. Root MIT must not be applied as a blanket license to every bundled path. The PowerPoint terms prohibit extraction/retention outside Anthropic services, copying, derivative works, and third-party distribution; exclude this skill from Workspace imports, generated packages, training corpora, and distribution artifacts unless a documented legal grant explicitly permits the intended use. |
| Turn commit ordering | `agent.turn_finalizer.finalize_turn` calculates `completed`, saves trajectory, and persists the session before adding mutation diagnostics/completion explanations or invoking `transform_llm_output`. Persistence failure becomes `cleanup_errors` while the result may still say completed. Build one canonical transformed `TurnResult`, bind its hashes and hook provenance, atomically commit turn/session/outbox state, and expose `degraded` or `not_committed` rather than completed success on commit failure. |
| Memory promotion | External memory sync is gated on interruption, not on `completed`/`failed`, and is dispatched as best-effort background work without a durable outbox. Recalled provider text is labelled authoritative reference data without an instruction-trust boundary. Only accepted, finalized BMAD outputs may create evidence-scoped `MemoryPromotionProposal` records; recalled memory remains untrusted and promotion uses an idempotent durable outbox. |
| Provider precedence | `providers/__init__.py:register_provider` is last-writer-wins and explicitly lets user providers replace bundled providers on name collision. This corrects the earlier statement that bundled providers win. AI Workspace packages need immutable publisher-qualified ids and an explicit, trusted `replaces` permission; name collision must not grant replacement authority. |
| Provider resolution | `hermes_cli.runtime_provider.resolve_runtime_provider` serves the main runtime surfaces, but auxiliary/compression paths also use `agent.auxiliary_client.resolve_provider_client` and `_resolve_task_provider_model`; TUI paths add further resolution helpers. Treat resolution as one canonical auditable contract per model call, not as one existing Hermes function. |
| Credential binding | `runtime_provider.py` detects Azure Anthropic through substring checks for `azure.com` in two resolution branches before selecting Azure/Anthropic credentials. An explicit lookalike host or URL path can therefore be associated with the secret even though safer parsed-host helpers exist elsewhere in the same module. Parse and canonicalize HTTPS URLs, enforce exact approved host suffix/path/port rules, and resolve the credential only after `ProviderCredentialBinding` succeeds. |
| Profile secret scope | `agent.secret_scope.get_secret` provides fail-closed multiplex scoping, but fallback resolution still reads configured `key_env` and `OLLAMA_API_KEY` with raw `os.getenv`; auxiliary/model-switch code has additional raw credential reads. Enforce one secret-access port across primary, auxiliary, compression, fallback, and switching paths, with a CI rule against raw credential environment reads. |
| Skill learning and curation | `skills.write_approval` and agent-created-skill scanning both default off; scanner exceptions allow the write, approval-module import failure fails open, and a staging disk failure can still return a successful staged record. Deterministic curation can archive eligible bundled skills because built-in pruning defaults on while LLM consolidation defaults off. BMAD Method, Builder, and active packages remain immutable; learning emits a versioned proposal with base hash, diff, evidence, scan/test results, and approval identity, and every gate/scanner/persistence failure denies. |
| Cron durability | Built-in cron uses local `jobs.json`; cross-process lock failure can continue with only an in-process lock, a missing stored job can still be dispatched, recurring jobs advance before execution, and a finite one-shot can be removed despite delivery failure. Hosted runs need transactional generation-aware claims, run attempts, retained history, a durable idempotent delivery outbox, fail-closed capability/MCP resolution, and wall-clock, inactivity, aggregate cost, and output budgets. |
| Compression | Gateway high-water hygiene invokes the shared `ContextCompressor`. Old tool results are pruned before summary generation, so even the documented abort path returns a pruned transcript rather than the original unchanged input. Ordinary summary failure can also insert a deterministic fallback and drop the middle window. Stage compression against an immutable source snapshot and commit pruning plus summary only after success, with a hash/range-bound `ContextCompressionRecord`. |
| Approvals | Hardline command blocks, session-local identities, and callback failure handling are strong. However, the historical dangerous-command path can auto-approve when no interactive CLI or gateway is present; plugin escalation separately opts into fail-closed behavior. A hosted Workspace must deny when principal, policy envelope, or approval bridge is absent. Airlock remains authorization/audit; OS or container isolation remains containment. |
| Extensions and acquisition | Model-provider directories are imported automatically on first provider discovery, are recorded as enabled by the generic plugin manager, execute in-process, and can replace bundled profiles. Plugin installation shallow-clones mutable default HEAD, treats HTTP/`file://` and missing manifests as warnings, and updates with mutable pulls. Require explicit activation, signed immutable package locks, manifests/SBOMs/scans/rehearsals, isolated adapters, and separate replacement authority. |
| Verification and release | Hermes verification evidence is passive; a targeted success can clear changed paths and report passed without source/diff/artifact/toolchain hashes. The PyPI workflow publishes from a tag or manual dispatch with `publish` depending only on `build`, not CI tests, lint, supply-chain, or OSV gates. Workspace release evidence must be server-enforced, hash/freshness/scope bound, and attached to the exact artifact before publication. |
| Aggregate budget | Parent and delegated agents have independent iteration budgets, so aggregate calls, tokens, cost, wall time, tools, and outputs can exceed the parent cap. A BMAD `RunPlan` owns a durable run-level budget ledger plus per-worker limits. |
| Lifecycle events | `on_session_end` is fired after each `run_conversation` call, while `on_session_finalize` represents actual closure. Use unambiguous events such as `run.turn.committed`, `run.completed`, `session.finalized`, and `session.reset`. |

Adoption boundary: BMAD Method and BMAD Builder remain the product kernel. Hermes contributes lower-level worker-loop, tool-execution, approval, verification-evidence, and adapter patterns. It does not define the canonical `RunPlan`, artifact graph, package lifecycle, or control-plane state for the AI Workspace.

## Executive Findings

Hermes is useful to Sapphirus because it has already wrestled with long-running agent runtime concerns: prompt-cache stability, tool availability, background self-improvement, session lifecycle, external adapters, scheduled jobs, and security boundary wording. It remains a supporting runtime reference beneath the BMAD Method and Builder kernel.

The strongest lessons for Sapphirus are:

1. **Keep the runtime core narrow.** Hermes treats the always-loaded model tool surface as expensive and risky. Sapphirus should use BMAD packages, SkillOps packages, service-gated tools, connectors, and worker manifests before adding core runtime features.
2. **Make prompt and tool schema stability an invariant.** Hermes says active conversations must not silently mutate system prompts, cached prefixes, or model tool schemas except through explicit compression/finalization paths.
3. **Do not overclaim Airlock as containment.** Hermes is explicit that approvals, scanners, redactors, and allowlists are in-process controls, not isolation boundaries. Sapphirus should document Airlock as governance and authorization; real containment comes from OS/container/process isolation.
4. **Use per-turn context, not process-global flags.** Hermes uses context variables for approval/session/tool-call identity so concurrent gateway sessions cannot stomp each other.
5. **Govern self-improvement writes explicitly.** Hermes separates foreground user-directed skill writes from background review writes and preserves read-before-write provenance, but staging is conditional on the opt-in write-approval gate. Workspace background learning always creates a proposal; it never directly mutates an active package.
6. **Gate tools by real availability.** Hermes tools register centrally with `check_fn` availability, TTL caching, transient failure grace, dynamic schemas, and plugin override restrictions.
7. **Model scheduled jobs as claimed fire events.** Hermes' optional managed cron provider uses a one-shot external scheduler, purpose-scoped callback JWT, immediate `202`, and store-level compare-and-swap claim; its built-in local scheduler is not the hosted reference architecture.
8. **Use connector descriptors.** Hermes relay connectors advertise capabilities such as message length, markdown dialect, thread support, and edit/draft-streaming support before normalized message events flow.
9. **Pin core dependencies and lazy-load optional providers.** Hermes keeps core dependencies exact-pinned, documents why, and pushes provider-specific dependencies into extras/lazy paths.
10. **Test invariants, not snapshots.** Hermes' test tree is full of targeted behavior tests: approval isolation, prompt cache policy, cron fire claiming, profile isolation, background review restrictions, provider discovery, and concurrency races.
11. **Commit one canonical turn truth.** Final response transforms, evidence, persistence, and delivery must refer to the same finalized object and hashes.
12. **Treat licenses and release evidence per component and artifact.** A root license, a large test inventory, or a successful targeted command cannot authorize or promote every bundled component.

## Runtime Architecture Lessons

Hermes' `AGENTS.md` is especially relevant. It defines a "narrow waist" runtime: the same core serves CLI, gateway, TUI, desktop, messaging, cron, and subagents, but capabilities should arrive first as commands, skills, service-gated tools, plugins, or MCP servers. A new always-present model tool is the last resort.

Sapphirus adaptation:

- Add a **capability footprint ladder** to architecture reviews: extend existing contract, then BMAD package/skill, then service-gated tool, then connector/plugin, then core runtime feature.
- Treat all BMAD/Builder extensions as package capabilities unless they must be present for every run.
- Track "core tool schema budget" as a release concern because schema churn affects provider caching, prompt size, and model behavior.

## Prompt, Context, and Cache Stability

Hermes' prompt modules show three concrete practices:

| Hermes pattern | Sapphirus adaptation |
| --- | --- |
| Prompt cache markers are pure/provider-specific functions. | `ModelGateway` should own a `PromptCacheContract` and emit provider-specific cache directives without mutating the run plan. |
| Context files are searched only inside bounded project roots. | Workspace context pack discovery must stop at repository/workspace boundaries and must not ingest arbitrary parent-directory instructions. |
| Suspected prompt-injection in context is blocked before prompt assembly. | Context Pack Builder should scan and quarantine high-risk instructions before they reach the model. |

Sapphirus should persist a `PromptCacheContract` per run containing:

- system prompt version hash;
- model/tool schema hash;
- context pack hash;
- provider cache mode;
- allowed compression/finalization transitions;
- reason if the cache contract is broken.

Two implementation details narrow the safe adaptation. First, `ContextCompressor.compress` performs old-tool-result pruning before the summary call; when summary generation aborts, it returns the already-pruned copy despite comments saying the transcript is unchanged. Compression must therefore be staged against an immutable input and committed only when the complete summary/pruning record succeeds. Second, external memory provider output is inserted as “authoritative” persistent memory without a separate instruction-trust policy. Workspace memory is untrusted context until source, owner, evidence, and promotion policy say otherwise.

## Session Lifecycle and State

Hermes' session lifecycle docs and `hermes_state.py` show durable session metadata as more than a transcript ID. The useful parts for Sapphirus are:

- deterministic session keys with platform, chat type, chat/thread/user discriminators;
- explicit suspend, resume-pending, reset, expiry, and finalization flags;
- transcript append/rewrite/rewind support for retry, undo, and compression;
- token/cost counters and last prompt token metadata;
- SQLite WAL for normal concurrency, with documented fallback for filesystems that cannot support WAL;
- FTS-backed session search with degraded behavior when optional SQLite features are unavailable.

Sapphirus adaptation:

- Add `SessionLifecycleRecord` to the canonical object model.
- Keep run/session identity separate from authentication; session IDs are routing handles, not auth boundaries.
- Test session-key conformance across external adapters before enabling multi-channel routing.
- Add `TurnCommit`: finalize response transforms and evidence first, then atomically persist the canonical turn, session transition, and delivery outbox. A failed commit is visible as `not_committed` or `degraded`, never completed success.
- Add a BMAD-run aggregate budget ledger across the parent, subagents, background review, compression, and auxiliary calls; retain per-worker caps but do not let their sum bypass the `RunPlan` limit.

## Airlock and Approval Lessons

Hermes' approval modules provide concrete implementation guidance:

- dangerous-mode policy is frozen at process start/import, so a skill cannot flip an environment variable mid-run to bypass approvals;
- approval state is context-local and includes session key, turn id, tool-call id, and interactivity mode;
- unattended cron jobs are not allowed to borrow interactive gateway approval flows by accident;
- policy-sensitive config writes are treated as approval-required side effects;
- notification or callback failure fails closed;
- raw command text is used for execution and smart approval, while displayed text is redacted.

Sapphirus adaptation:

- `PolicyContext` must include explicit run/session/turn/tool-call identity.
- Airlock evaluation must never depend on mutable process-global flags after startup.
- Scheduled jobs default to deny or pre-granted policy envelopes; they do not prompt interactively.
- Config, policy, connector, package, and dependency files are policy-sensitive write targets.
- No principal, no human bridge, or an unreadable policy envelope must fail closed; do not inherit Hermes' historical non-interactive dangerous-command auto-approval behavior.

## Security Boundary Wording

Hermes' `SECURITY.md` is clear that in-process controls are not security boundaries against an adversarial model. This is important for Sapphirus because Airlock can otherwise be described too strongly.

Sapphirus wording should be:

- Airlock is the authorization, approval, and audit boundary for side effects.
- OS/container/process isolation is the containment boundary for untrusted execution.
- File scanners, prompt-injection scanners, redactors, allowlists, and package validators are defense-in-depth controls.
- In-process plugins and package hooks run with runtime privileges unless isolated in a separate worker process.
- External surfaces require real authorization; session IDs and correlation IDs are not credentials.

## Tool Registry and Capability Availability

Hermes' `tools/registry.py` is a strong template for service-gated tools:

- tools self-register with schema, handler, toolset, `check_fn`, required env, async flag, output limit, and optional dynamic schema;
- availability checks are TTL-cached;
- recent success grants transient grace so a flaky dependency does not strip tools mid-session;
- tool definitions are generated from registry snapshots with a generation counter;
- plugin override/deregister is restricted unless explicitly allowed.

Sapphirus adaptation:

- Add `ToolAvailabilitySnapshot` and `ToolDefinitionVersion`.
- Expose effective tool/package availability in Operator Console.
- Do not include unavailable tools in model schemas unless there is an explicit degraded fallback.
- Record why a capability is unavailable: missing secret, disabled connector, failed health check, policy, or package validation failure.

## Execution Backends and Output Limits

Hermes execution modules support local, Docker, SSH, Singularity, Modal, and Daytona style backends. Useful design details:

- define whether commands spawn fresh processes or reuse shell state;
- persist CWD separately from environment/functions/aliases;
- heartbeat long-running process activity;
- support interruption;
- scope cached secrets/UI callbacks per session/thread;
- cap tool output by bytes, lines, and line length through central config;
- validate forwarded environment variable names and values;
- label containers and reap only known, old, exited containers.

Sapphirus adaptation:

- `ExecutionLaneSpec` must define process model, persisted state, output limits, heartbeat, interrupt semantics, env forwarding rules, and cleanup policy.
- Container labels/profiles must be part of worker identity and cleanup.
- Output caps belong in worker manifest/config, not scattered constants.

## Managed Cron Provider and Built-In Scheduler

Hermes' optional managed cron provider is one of the clearest reusable pieces:

- an external service arms exactly one future one-shot event per job;
- callback requests use short-lived, purpose-scoped JWTs;
- the agent endpoint returns `202` before running the job;
- a store-level CAS claim prevents duplicate fire execution;
- recurring jobs advance under store lock and re-arm;
- reconciliation runs at startup, mutation, and after fire.

The built-in path has a different boundary: local `jobs.json`, advisory cross-process locking, fresh cron sessions, and a 600-second inactivity timeout by default. A cross-process lock failure may proceed with process-only protection; handed-in jobs missing from the store may still execute; recurring schedules advance before execution; and a finite one-shot can be removed even when delivery fails. A toolset-resolution exception returns `None`, which gives the agent its full default toolset, and MCP servers may be unioned into per-job allowlists unless `no_mcp` is explicit. These compatibility defaults are inappropriate for a hosted control plane.

Sapphirus adaptation:

- Add `AutomationFireClaim` with `job_id`, `fire_at`, `claim_status`, `claimed_by`, `claimed_at`, and idempotency key.
- Scheduled BMAD runs should be claimed before any Airlock grant or worker dispatch.
- Multi-replica scheduling requires a shared store claim, not just in-memory locks.
- Revalidate job generation, enabled/paused state, package/tool/model/policy snapshot, and claim ownership immediately before dispatch.
- Persist `AutomationRunAttempt` separately from the job and deliver through a durable idempotent outbox; execution success and delivery success are distinct states, and neither erases history.

## Relay Connector Contract

Hermes' relay connector model is useful for future external chat surfaces:

- the runtime dials out to a connector WebSocket instead of requiring every deployment to expose inbound HTTP;
- a capability descriptor advertises platform behavior before messages flow;
- author-first routing prevents a shared guild/workspace/channel from choosing the wrong instance;
- deprovisioning revokes secrets and fails closed;
- going-idle and inbound-ack frames avoid dropping messages during scale-down.

Sapphirus adaptation:

- Add `ConnectorCapabilityDescriptor`.
- Require session source discriminators for platform, scope/workspace, chat, thread, and author.
- Treat connector routing as an authz concern, not a convenience lookup.

## Skill and Package Self-Improvement

Hermes' skill system has several direct SkillOps lessons:

- external skills pass through a trust-aware scanner before install;
- community sources with dangerous findings are blocked even with force;
- `SKILL.md` is never excluded from scanning by ignore files;
- background review-created skills carry provenance;
- background review edits must target content the review fork actually read;
- when the opt-in write-approval gate is enabled, pending writes preserve exact replay payloads and origin;
- foreground pinned skills prevent deletion but still allow user-directed patches;
- autonomous background review cannot write pinned, bundled, hub-installed, protected, or external skills;
- deterministic curation may archive eligible bundled skills because `curator.prune_builtins` defaults to true, while LLM consolidation defaults to false.
- `skills.write_approval` and agent-created-skill scanning default off; scanner exceptions and approval-module import failure allow writes, while staging persistence failure can still be surfaced as successful staging.

Sapphirus adaptation:

- SkillOps should create `SkillPackageProposal` records rather than editing active packages directly.
- Package writes need origin: foreground user request, Builder import, migration, background review, or operator repair.
- Background self-improvement must always create a `SkillPackageProposal` containing base/version hash, exact diff, read evidence, scan and test evidence, origin, policy decision, and approver identity.
- BMAD Method, Builder, and active package versions are immutable to autonomous maintenance; patches create new reviewed versions rather than mutating the active package.
- Scanner, approval-gate, snapshot, or pending-store failure denies the proposal transition; it does not fall through to a direct write or report a durable staged record.

## Plugin and Provider Lessons

Hermes plugins are used for web providers, browser providers, memory providers, cron providers, dashboard auth, messaging platforms, and more. Useful rules:

- plugin discovery reads metadata without importing when possible;
- user-installed provider modules load under synthetic namespaces;
- user-installed model-provider profiles replace bundled profiles on the same name because registration is last-writer-wins;
- model-provider discovery imports every user provider directory on first profile lookup and the generic plugin manager records this special class as enabled rather than applying the normal opt-in path;
- register-context patterns collect only the intended extension type;
- provider availability is checked separately from registration.
- plugin installation shallow-clones mutable default HEAD; HTTP and `file://` sources and missing manifests are warning-only, while update pulls mutate the installed checkout.

Sapphirus adaptation:

- Package import should separate metadata parsing, trust validation, capability registration, and runtime availability checks.
- Untrusted package code should not be imported during catalog discovery.
- Extension ids should be publisher-qualified, versioned, and immutable; replacement of a first-party capability requires an explicit trusted grant rather than a matching name.
- Executable extensions should run outside the orchestrator process with declared permissions and typed lifecycle events.
- Activation, execution permission, and replacement authority are separate decisions. A model-provider directory is metadata-only until all three relevant gates succeed.
- Store an `ExtensionPackageLock` containing source URL, immutable ref, archive/tree digest, publisher/signature, manifest schema, dependency lock/SBOM, scan and rehearsal evidence, and activation decision. Updates create a new staged version.

## Supply Chain Lessons

Hermes' `pyproject.toml` is unusually explicit about dependency policy. Core dependencies are exact-pinned, provider-specific dependencies live in optional extras, and comments document why several floors/ceilings exist. However, repository licensing is not homogeneous:

- root code is MIT under Nous Research's notice;
- `plugins/security-guidance/patterns.py` is copied from Anthropic under Apache-2.0 with a component `NOTICE`;
- `plugins/hermes-achievements` and `skills/creative/humanizer` carry separate MIT copyright notices;
- the bundled `skills/productivity/powerpoint` declares a proprietary Anthropic license that prohibits retaining the materials outside the Services, copying, derivatives, and third-party distribution.

The PowerPoint skill is therefore not an adoptable or redistributable Hermes pattern under the root MIT grant. Sapphirus should quarantine/exclude it from source ingestion, skill conversion, Builder output, training/evaluation corpora, images, packages, and release artifacts unless a documented legal review records a separate permission that covers the exact use and distribution model.

Sapphirus adaptation:

- Exact-pin runtime core dependencies and keep lockfiles authoritative.
- Keep optional provider dependencies outside the core deployment path.
- Every security-motivated pin or override needs an inline rationale and an expiry/review process.
- CI must fail when lockfiles, manifests, or SBOMs drift from package metadata.
- Produce a path-level component license/notice inventory and an explicit `ComponentLicenseDecision` before any source-derived package can enter Builder or activation review.
- Build from immutable source and dependency locks; sign and attest the exact artifact only after its blocking test, lint, security, license, and provenance gates pass.

## Test Lessons

The Hermes test tree suggests concrete Sapphirus test categories:

| Test category | Sapphirus equivalent |
| --- | --- |
| Prompt cache policy | Same run keeps stable prompt/tool schema hashes unless compression/finalization explicitly changes them. |
| Approval isolation | Parallel sessions cannot share approval state or callback identity. |
| Cron claim tests | Duplicate fire events produce at-most-once execution. |
| Background review restrictions | Autonomous self-improvement cannot edit content it did not read and cannot auto-curate user-owned packages. |
| Provider/plugin discovery | Discovery reads metadata safely and does not import untrusted code. |
| Session provenance | Session key generation is deterministic and includes required discriminators. |
| Concurrency stress | Dispatch, database writes, approvals, and worker claims survive races. |
| Security fixtures | Prompt injection, path traversal, secret reads/writes, unsafe config writes, and egress attempts are blocked or audited. |
| Credential binding | Azure/custom URL lookalikes, paths, queries, userinfo, redirects, ports, and sovereign-cloud variants cannot receive a credential outside its approved host binding. |
| Turn durability | Delivered response, transcript, trajectory, memory promotion, evidence, and outbox all reference the same finalized turn hash; commit failure cannot report completed. |
| Release evidence | Verification binds source tree, diff, package/artifact, command argv, toolchain/image, exit code, logs, scope, and freshness; targeted success cannot stand in for a required full gate. |

Hermes' verification ledger is a useful bounded nudge, not a release authority: it is explicitly passive, a targeted success can clear changed paths, and records are not bound to source/diff/artifact/toolchain hashes. The PyPI workflow can publish after its build job without depending on the CI test, lint, supply-chain, or OSV workflows. Sapphirus must make release policy server-enforced and artifact-bound. The 1,983 `test_*.py` files in this snapshot are inventory evidence only; this source audit did not execute them.

## What Not To Copy

Do not copy Hermes' broad personal-assistant surface into Sapphirus v1. Sapphirus is a BMAD runtime first. The useful parts are the runtime contracts, not every channel or feature.

Avoid copying:

- always-on personal memory loops before BMAD package execution is solid;
- a very broad built-in channel matrix;
- in-process third-party plugin execution as a default trust model;
- unqualified provider ids that let a user package silently replace a first-party profile;
- autonomous pruning or mutation of BMAD Method, Builder, or active package versions;
- local shell convenience defaults that weaken hosted/runtime containment;
- fail-open cron toolset resolution or no-human command approval;
- best-effort memory promotion from failed/incomplete turns or treating recalled memory as trusted instructions;
- provider credential selection based on URL substrings or raw unscoped environment reads;
- resolution-triggered model-provider autoload, mutable plugin pulls, or package replacement by name collision;
- optional/fail-open learning gates that can directly mutate active skills;
- publishing artifacts without exact-source CI, security, provenance, and component-license gates;
- the proprietary PowerPoint skill or any derivative/redistributed copy without a separately documented permission;
- huge core modules that mix CLI, agent loop, gateway, and persistence concerns.

## Plan Changes Applied

This review should inform these vault files:

- [[00 - Common Rules and Product Shape]]
- [[12 - Run Orchestrator and Agent Kernel]]
- [[14 - Builder Studio and SkillOps]]
- [[17 - Workspace Intelligence and Context Packs]]
- [[18 - Model Gateway and Microsoft Foundry]]
- [[19 - Airlock Policy and Approvals]]
- [[20 - Execution Lanes and Container App Jobs]]
- [[23 - Security, Identity, and Secrets]]
- [[24 - Operator Console and Operations]]
- [[25 - OpenAPI, Schemas, and Generated Clients]]
- [[27 - Testing, Validation, and Replay]]
- [[28 - Supply Chain, Deployment, and IaC]]
- [[29 - Concurrency, Transactions, and Failures]]
- [[32 - Integration Contract Map]]
- [[34 - Canonical Object Model]]
- [[39 - BMAD Package Format]]
- [[41 - Observability Dashboards and Alerts]]
- [[42 - Migrations, Retention, and Cleanup]]
