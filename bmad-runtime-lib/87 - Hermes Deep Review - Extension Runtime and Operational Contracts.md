---
title: "Hermes Deep Review - Extension Runtime and Operational Contracts"
aliases:
  - "Hermes Second Deep Review"
  - "Hermes Extension Runtime Review"
tags:
  - bmad-runtime
  - vault/source-and-research
section: "Source and Research"
order: 87
vault_role: "source-review"
project: "Sapphirus BMAD Runtime"
status: current
reviewed_on: 2026-07-09
review_revision: "V6.16"
source_archives:
  - "C:\\Users\\rodrigocsousa\\Downloads\\hermes-agent-main.zip"
related:
  - "[[86 - Hermes Source Code Review - Agent Runtime and Learning Loop]]"
  - "[[85 - OpenClaw Structured Code Review]]"
  - "[[83 - BMAD Source Code Review - Method and Builder]]"
---

# Hermes Deep Review - Extension Runtime and Operational Contracts

> Historical source evidence. Provider, extension, secret, and operations lessons remain inputs; delivery authority and desktop security claims are governed by [[93 - Split Web and Windows Desktop Architecture Plans]] through [[99 - Dual-Delivery Contract and Conformance Specification]].

## Review Scope

This is a second, deeper Hermes pass focused on runtime contracts that were easy to miss in the first architectural review. It covers developer-guide pages and source modules for turn finalization, memory promotion, provider resolution, context compression, ACP/editor sessions, secret sources, platform adapters, cron jobs, session storage, browser supervision, verification evidence, kanban coordination, dashboard auth, gateway lifecycle, package acquisition, and release workflows.

The complete archive was reviewed at `_full/h/hermes-agent-main`. `C:\Users\rodrigocsousa\Downloads\hermes-agent-main.zip` has SHA-256 `E5E0941C515867EC024B343E775D07F34B323B363CB0570863CF6690B9291095`, 7,075 ZIP entries, 6,205 regular files, 870 directory entries, and 133,029,443 uncompressed bytes. Every file in the earlier 5,909-file `_source_review` tree is SHA-256-identical to its full-tree counterpart; the 296 recovered files are all documentation-site content/assets/build support under `website/`, so they do not alter the executable-runtime findings.

The archive reports Hermes `0.18.2`, release `2026.7.7.2`, but contains no `.git`, commit, or tag. Root code is MIT, while bundled components include separately attributed MIT works, Apache-2.0-derived security patterns with a notice, and a proprietary PowerPoint skill. Content completeness is high; revision and component-level adoption decisions remain explicit `SourceSnapshot` gates.

## V6.16 Audit Corrections

- **Source identity:** archive digest, manifest counts, extraction completeness, and old/full content equivalence are now verified. Immutable upstream commit/tag identity remains unknown.
- **License boundary:** root MIT is not a blanket component license. `skills/productivity/powerpoint/LICENSE.txt` is proprietary and prohibits extraction/retention outside Anthropic services, copying, derivative works, and third-party distribution. Exclude that skill from Workspace ingestion, Builder conversion, corpora, packages, and releases unless a documented legal grant permits the exact use.
- **Turn truth:** `finalize_turn` persists transcript/trajectory before response footers and plugin transforms and can still report `completed` when persistence failed. Workspace needs one finalized, hash-bound `TurnCommit` that atomically persists turn/session/evidence/outbox state before delivery.
- **Memory truth:** failed or incomplete non-interrupted turns can be queued to external memory; sync is best-effort background work, and recalled text is labelled authoritative. Knowledge promotion must be accepted-BMAD-output-only, evidence scoped, untrusted on recall, durable, and idempotent.
- **Resolution is a contract, not one function:** the main CLI/gateway/cron/ACP path uses `resolve_runtime_provider`; auxiliary and compression calls use `resolve_provider_client` and `_resolve_task_provider_model`, with additional TUI helpers. Every primary, auxiliary, compression, evaluation, background, and fallback call must emit one canonical `RuntimeProviderResolution` record.
- **Hosted-boundary failures:** Azure Anthropic credential selection uses a URL substring in two resolver branches; fallback and auxiliary paths retain raw unscoped secret reads; skill write approval and scanning default off and can fail open; cron may continue after cross-process lock failure and lacks a durable delivery outbox; and compression pre-prunes before an abort that claims to preserve the original transcript.
- **Extensions and release:** model-provider directories autoload during provider discovery and can replace bundled profiles. Plugin install/update uses mutable Git state without immutable package verification. Passive verification can treat a targeted success as passed, while PyPI publication does not depend on CI/test/security workflow results.
- **Aggregate budgets:** delegated agents own independent iteration budgets, so the parent cap is not an aggregate run bound. BMAD `RunPlan` must own the cross-worker call/token/cost/time/tool/output ledger.
- **Lifecycle naming:** Hermes fires `on_session_end` at the end of each `run_conversation` call and uses `on_session_finalize` for actual closure. AI Workspace events must distinguish turn completion, run completion, session finalization, and session reset.

BMAD Method and BMAD Builder remain the kernel. Hermes patterns sit beneath the BMAD `RunPlan`, package, artifact, evidence, and approval contracts; they are not the Workspace's product model or canonical control plane.

## Executive Findings

Hermes has most of its durable lessons in operational boundaries rather than feature lists. The most valuable Sapphirus improvements are not "add more adapters"; they are one canonical turn commit, governed memory promotion, tighter provider/credential identity, prompt-cache and compression safety, complete profile-scoped secret access, durable automation claims/outbox delivery, immutable extension acquisition, aggregate run budgets, and artifact-bound verification evidence. All remain subordinate to the BMAD Method and Builder execution model.

## High-Value Runtime Patterns

### Provider Resolution and Credential Binding

Hermes uses `resolve_runtime_provider` across the primary CLI, gateway, cron, and ACP entry points. Auxiliary/compression calls have a separate path through `resolve_provider_client` and `_resolve_task_provider_model`, and TUI code adds resolution helpers. The main-path precedence is explicit request, saved config, environment, then provider defaults; saved config therefore beats a stale shell export. The implementation is convergent, but it is not literally one resolver for every model call.

Sapphirus should adopt `RuntimeProviderResolution` and `ProviderCredentialBinding` records. Each model call should know provider, API mode, canonical base URL, credential source/binding id, model identity, fallback state, cache strategy, and routing metadata. `runtime_provider.py` currently uses the substring `azure.com` to select Azure/Anthropic secrets in two branches, so an explicit lookalike hostname or path can become associated with that credential even though safer host-parsing helpers exist elsewhere. Parse and canonicalize HTTPS URLs first; enforce exact approved host suffixes, paths, ports, sovereign-cloud variants, redirect policy, and account identity; retrieve the secret only after binding succeeds. Mid-session model, account, or credential-pool changes must emit a cache/cost transition event because they break prompt-cache continuity and spend predictability.

### Context Compression and Prompt Cache Safety

Hermes separates the gateway high-water trigger from normal in-loop compression, but both ultimately use `AIAgent._compress_context` and `ContextCompressor`. The trigger and observability paths differ; the compression engine is shared.

Sapphirus should model compression as `ContextCompressionRecord`, not as an invisible string rewrite. Records must include source hashes and token window, protected head/tail ranges, summary template version, compression model, context-limit source, dropped/retained tool groups, and whether the previous summary was incrementally recompressed. Hermes requires a 64K auxiliary floor and may lower the live compression threshold when the auxiliary context is smaller; by default, an ordinary summary failure may insert a deterministic fallback and discard the middle window. More subtly, Hermes prunes old tool results before requesting the summary, so the abort path returns an already-pruned transcript despite saying the original is preserved unchanged. Run-critical BMAD compression must stage all pruning and summarization against an immutable source snapshot and commit only after success; failure retains the byte-identical source and fails closed.

Prompt-cache adaptation should remain in Model Gateway. The orchestrator provides a stable `PromptCacheContract`; the gateway maps it to provider-specific markers. Cache marker placement must be deterministic and unit-tested. Fallback model/provider changes must create a cache-break event.

### Turn Commit, Memory Promotion, and Run Budgets

`agent.turn_finalizer.finalize_turn` computes completion and persists trajectory/session state before it appends file-mutation diagnostics, generates abnormal-completion explanations, or invokes the response-transform plugin. A persistence exception is reported in `cleanup_errors`, but the returned result can remain completed. That creates distinct persisted, plugin-observed, and delivered response truths.

The same finalizer calls external-memory synchronization without passing `completed` or `failed`; only interruption is excluded. `MemoryManager.sync_all` dispatches provider writes as best-effort background work with no durable outbox, while recalled provider text is wrapped as authoritative persistent memory. Separately, delegated agents have independent iteration budgets, allowing aggregate parent/child/background/auxiliary work to exceed the parent cap.

Sapphirus should finalize transforms and evidence into one immutable `TurnCommit`, then atomically persist the turn, session transition, BMAD artifact lineage, and delivery outbox before acknowledging completion. Only accepted finalized BMAD outputs may create a `MemoryPromotionProposal`; recall remains untrusted context and durable promotion is idempotent. `RunBudgetLedger` must aggregate model calls, tokens, cost, wall time, tool calls, and output across every worker while retaining narrower per-worker caps.

### Editor and ACP Session Contracts

Hermes ACP wraps a synchronous agent loop inside an async JSON-RPC stdio server. Important details: stdout is reserved for protocol frames, logs go to stderr, working directory is bound per editor session, cancellation sets both an event and an agent interrupt, and permission prompts fail closed on timeout or bridge failure.

Sapphirus should define `EditorSessionContext` and `ToolEventCorrelation`. Editor-origin runs need session id, cwd, active model, history pointer, cancel token, permission bridge, and log/protocol separation. Tool events with duplicate tool names need FIFO correlation by tool id, not name-only matching. Any temporary approval callback or permission handler must be restored after execution.

### Platform Adapter and Delivery Contracts

Hermes platform adapters declare configuration, required and optional env, validation, authorization policy, cron delivery behavior, user allowlists, max message length, and platform-specific tools. Slow-response platforms acknowledge immediately, cache the pending request, then deliver proactively later through a `PENDING -> READY -> DELIVERED` state machine, with `ERROR` for stop/cancel cases.

Sapphirus should add `DeliveryTarget`, `ConnectorCredentialLock`, and `ConnectorConfigBridge`. A connector must translate platform-specific YAML/env into generic runtime config without making core runtime know every platform key. Persistent adapters using unique credentials need scoped locks so two profiles do not use the same credential simultaneously.

Authorization delegation must be explicit. If an upstream relay authenticates a user, the gateway may trust it only when the transport carries an authenticated upstream marker. A stamped secondary profile must resolve its own live adapter and must fail closed rather than falling back to the default profile adapter.

### Secret Sources and Profile Scopes

Hermes secret-source plugins resolve credentials at startup into environment-like maps, but the framework owns ordering, precedence, provenance, timeouts, protected bootstrap vars, and conflict reporting. Backends must not prompt and must not raise; they return a result object. Mapped secrets beat bulk imports, first claim wins, and override settings may beat shell or `.env` values but never another secret source or protected variable.

Sapphirus should add `SecretSourceApplyReport`, `AppliedSecretVar`, and `ProfileSecretScope`. Hermes' main resolver adopts its fail-closed `get_secret` abstraction, but `try_activate_fallback` still reads configured `key_env` and `OLLAMA_API_KEY` with raw `os.getenv`, and auxiliary/model-switch paths retain additional raw credential reads. In multiplex/profile scenarios, every primary, auxiliary, compression, fallback, background, and switch call must carry a profile scope and credential-binding id; unscoped reads fail closed. CI should reject raw environment reads for credential names. Child secret helpers should be argv-only, run with minimal allowlisted environment, closed stdin, a hard timeout, and scrubbed output.

### Approval and Extension Trust Boundaries

Hermes has strong hardline command blocks, context-local session/turn/tool identities, and fail-closed gateway callback handling. Its historical dangerous-command approval path can nevertheless auto-approve when neither an interactive CLI nor gateway is present; plugin escalation separately opts into fail-closed behavior. Enabled plugins are imported into the agent process and therefore have agent privileges, not sandboxed extension privileges. Model-provider plugins form a sharper exception: all user provider directories are imported on first provider discovery, the generic plugin manager records this class as enabled, and last-writer-wins registration can replace a bundled profile without a distinct replacement grant.

Hermes plugin acquisition also shallow-clones mutable default HEAD, treats HTTP/`file://` sources and missing manifests as warnings, and updates with mutable pulls. Sapphirus must deny when the principal, pre-granted policy envelope, or approval bridge is missing. Airlock authorizes and audits; an OS/container worker contains untrusted execution. Extensions need publisher-qualified immutable ids, signed provenance, declared capabilities, an isolated execution boundary, and separate activation/replacement authority. `ExtensionPackageLock` must bind immutable source ref/tree/archive digest, manifest, signer, dependency lock/SBOM, scans, rehearsal, and policy decision; update creates a staged version. Lifecycle events must use distinct names for `run.turn.committed`, `run.completed`, `session.finalized`, and `session.reset`.

### Skill Learning and Component Adoption

Hermes' background review fork has useful isolation and read-before-write controls, but its durable-write claim is configuration dependent. `skills.write_approval` defaults off, agent-created-skill scanning defaults off, scanner exceptions allow the write, approval-module import failure explicitly fails open, and a staging disk error can still return a successful pending id. The review prompt actively encourages at least one skill update in most sessions. Curator snapshots are best-effort while deterministic pruning of eligible bundled skills defaults on.

Sapphirus learning always emits an immutable `SkillPackageProposal`; it never edits the active package. The proposal binds base/version hash, exact diff, origin and source-read evidence, component-license decision, scan/test/rehearsal evidence, policy result, and approval identity. Missing gates, scanners, snapshots, or durable proposal storage deny the transition. BMAD Method, BMAD Builder, and active package versions are pinned and cannot be autonomously curated.

Component adoption is path scoped. The root Hermes license is MIT, `plugins/security-guidance/patterns.py` is Apache-2.0-derived with a notice, and other components carry separate MIT attribution. The bundled PowerPoint skill is explicitly proprietary and its terms prohibit retention outside Anthropic services, copying, derivative works, and third-party distribution. It must be excluded from Workspace ingestion, Builder conversion, datasets, packages, and release artifacts unless a separately documented permission and legal review cover the exact use.

### Cron and Scheduled Work

Hermes cron jobs run in fresh sessions with self-contained prompts. They do not inherit prior conversation history or memory, cannot clarify interactively, and always disable cron-management, messaging, and clarify toolsets. Skill-backed jobs load skills in declared order. Script pre-runs have separate timeouts from the agent's default 600-second inactivity timeout, and long active jobs can run for hours without blocking scheduler ticks. Built-in persistence is local `jobs.json` with advisory locking; the external one-shot/JWT flow is an optional managed provider. Cross-process lock failure may continue with only an in-process lock; a handed-in job absent from the store may still dispatch; recurring schedules advance before execution; and a finite one-shot can be removed after execution/delivery failure. Cron toolset lookup failure falls back to the full default set, and globally enabled MCP servers may be unioned unless a job explicitly says `no_mcp`.

Sapphirus scheduled work should keep `AutomationFireClaim` but extend it with job generation, persisted package/tool/model/policy snapshot, fresh-session requirements, recursion guards, skill/context injection records, fire-time delivery target resolution, idempotency, and "silent" delivery behavior. Revalidate existence, enabled/paused state, generation, and claim immediately before dispatch. Use a shared transactional store, retain a separate `AutomationRunAttempt`, fail closed on lock/claim/capability resolution, require explicit MCP grants, and enforce wall-clock, inactivity, aggregate cost, and output budgets. Execution and delivery are separate states; a durable idempotent `DeliveryOutbox` retries without erasing the job or attempt. History attachment remains explicit, labelled, role-safe, and auditable.

### Session Storage and Search

Hermes persists sessions and messages in SQLite with WAL, schema-version migrations, FTS search, session lineage, tool call metadata, token/cost counters, and source tagging. It retries write contention with jitter and checkpoints periodically. Batch/RL trajectories are stored separately from normal session state. This persistence is not yet one canonical turn transaction: trajectory and session writes precede final response transformation, and external memory is a separate best-effort side effect.

Sapphirus should separate interactive session state, replay/evaluation trajectories, training/export artifacts, recalled context, and promoted knowledge. Session search should index tool names, tool calls, source, model, title, and redacted message text. If persistence, an optional search index, or a promotion outbox is unavailable, degraded/not-committed behavior must be visible in state and telemetry.

### Tool Registry and Dynamic Schema

Hermes tools register with name, toolset, schema, handler, availability check, async flag, description, and env requirements. Tool definitions are filtered by availability, then dynamic schema patching removes references to unavailable companion tools. Handlers return JSON strings and encode errors as structured payloads instead of throwing through the model loop.

Sapphirus should add a formal `ToolContract`: structured result, structured error, availability gate, schema hash, toolset membership, dangerous-action class, and built-in justification. Dynamic schema generation must run after availability filtering, and run-scoped tool schemas must not change invisibly mid-turn.

### Verification Evidence

Hermes records passive verification evidence from terminal commands. The ledger classifies canonical test/lint/build commands, summarizes output, stores recent evidence with retention limits, and only nudges when code changed without fresh verification. Documentation/prose-only edits do not trigger verification nudges. A successful targeted command can nevertheless become overall `passed` and clear changed paths; records are not bound to source tree, diff, artifact, toolchain, or image hashes.

Sapphirus should add `VerificationEvidenceRecord` and require result manifests to distinguish full-suite verification, targeted verification, ad-hoc verification, skipped verification, and impossible verification. Bind source tree/diff/package/artifact hashes, exact argv, toolchain/image, exit code, logs, scope, freshness, and policy requirement. Verification nudges may remain bounded, but release authority is a server-side gate. Hermes' PyPI workflow publishes after `build` without depending on CI tests, lint, supply-chain, or OSV results; Sapphirus must attest and publish only the exact artifact that passed all required gates.

### Kanban and Task Claim Liveness

Hermes kanban uses per-board SQLite databases, WAL, compare-and-swap task claims, claim TTLs, worker heartbeats, stale-claim reclaim, and typed block reasons. It distinguishes dependency blocks from true human-needed blocks, and it breaks unblock/re-block loops by routing recurring blockers to triage.

Sapphirus should add `TaskClaim`, `WorkerHeartbeat`, `BlockReason`, and `BoardIsolationKey` semantics to backlog/dispatcher planning. Block kinds should at least distinguish `dependency`, `needs_input`, `capability`, and `transient`. Reclaim logic should consider both process liveness and heartbeat freshness, and it should avoid spawning duplicate workers while a previous worker may still be alive.

### Dashboard and WebSocket Auth

Hermes dashboard auth redacts token-like fields from audit logs, uses short-lived OAuth/PKCE cookies, refreshes sessions, supports bearer-token principals for machine callers, and uses single-use WebSocket tickets because browsers cannot set Authorization headers on upgrades. Internal server-to-server WebSocket credentials are a separate process-lifetime credential guarded by host/origin checks.

Sapphirus Operator Console should adopt `DashboardSession`, `TokenPrincipal`, `WebSocketTicket`, and `DashboardAuthAuditEvent` objects. Browser WebSocket tickets should be short-lived, single-use, identity-bound, and never fully logged. Internal credentials should be scoped to loopback/internal channels and clearly distinguished from browser tickets.

### Browser Supervision

Hermes keeps a long-lived CDP supervisor to track dialogs, frame trees, OOPIF session ids, and recent console errors. Dialog handling has policy modes, a watchdog timeout, and snapshot extensions rather than permanent schema bloat. State is discarded on session teardown or CDP rebind.

Sapphirus browser/tooling plans should use a `BrowserSupervisorState` concept if browser automation becomes v1.5/v2 scope. Pending dialogs, recent dialogs, frame tree truncation, and session rebind should be explicit. Tool availability should hide dialog tools when CDP is unreachable.

### Gateway Drain and Operational Lifecycle

Hermes uses a drain marker with an instantiation epoch so stale drain files after machine/container restart do not wedge a fresh gateway. Suppressing home-channel shutdown broadcast does not suppress active per-session interruption notices.

Sapphirus should add `GatewayDrainRequest` with principal, reason, creation time, instantiation epoch, and notification policy. Drain status should be visible in Operator Console and telemetry. Stale markers must be ignored or surfaced as degraded metadata, not treated as current truth.

## Concrete Sapphirus Changes

Add these canonical objects and schemas:

| Object/schema | Purpose |
|---|---|
| `SourceSnapshot` | Reproducible upstream URL, commit/tag, archive digest, version, license/attribution, extraction, and verification record. |
| `ComponentLicenseDecision` | Path-level license/notice evidence and explicit import, derivation, training, activation, and distribution decision. |
| `TurnCommit` | Canonical finalized response/evidence hash and atomic turn/session/artifact/outbox commit status. |
| `MemoryPromotionProposal` | Accepted-output-only, owner/source/evidence-scoped candidate for durable knowledge. |
| `RunBudgetLedger` | Aggregate model/tool/token/cost/time/output budget across parent, delegated, auxiliary, and background work. |
| `RuntimeProviderResolution` | Effective model/provider/base URL/API mode/credential source for a model call. |
| `ProviderCredentialBinding` | Binds provider credential to allowed base URL and provider account identity. |
| `ContextCompressionRecord` | Auditable record of compression threshold, protected ranges, summary model, and retained/dropped regions. |
| `EditorSessionContext` | Editor/ACP/TUI session id, cwd, history pointer, cancellation, and permission bridge state. |
| `ToolEventCorrelation` | Stable tool-call event mapping for duplicate same-name calls. |
| `ConnectorConfigBridge` | Adapter-owned translation of platform env/YAML/config into generic connector config. |
| `ConnectorCredentialLock` | Scoped lock for adapters whose credentials cannot be shared by simultaneous profiles. |
| `DeliveryTarget` | Fire-time platform/channel/thread destination resolved by gateway or scheduler. |
| `DeliveryOutbox` | Durable idempotent delivery state separated from execution success and retained run history. |
| `AutomationRunAttempt` | Immutable execution attempt bound to job generation, fire claim, package/policy snapshot, result, and delivery ids. |
| `SecretSourceApplyReport` | Startup secret apply provenance, conflicts, skipped vars, and source timing. |
| `ProfileSecretScope` | Fail-closed per-profile secret map used by workers and gateway tasks. |
| `SkillPackageProposal` | Versioned learning/Builder proposal with base hash, diff, provenance, license, scans, tests, rehearsal, policy, and approval. |
| `ExtensionPackageLock` | Immutable extension source/digest/signer/manifest/SBOM/scan/rehearsal/activation record. |
| `VerificationEvidenceRecord` | Policy-evaluated evidence bound to source/diff/artifact/toolchain/log hashes, scope, and freshness. |
| `TaskClaim` | CAS task claim with TTL, heartbeat, worker id, board id, and reclaim state. |
| `DashboardSession` | Authenticated human session for Operator Console. |
| `TokenPrincipal` | Authenticated machine caller with scopes. |
| `WebSocketTicket` | Short-lived single-use browser upgrade credential. |
| `GatewayDrainRequest` | Drain marker with instantiation epoch and notification policy. |
| `ExtensionLifecycleEvent` | Typed turn/run/session event with package identity, permissions, and correlation ids. |

Update these vault files:

- [[17 - Workspace Intelligence and Context Packs]]: add compression summary and context-pack invalidation rules.
- [[18 - Model Gateway and Microsoft Foundry]]: add provider resolution, credential binding, and cache-break events.
- [[14 - Builder Studio and SkillOps]]: add fail-closed proposal-only skill learning, component-license decisions, and immutable active packages.
- [[23 - Security, Identity, and Secrets]]: add secret source apply reports, profile secret scopes, dashboard auth, and upstream auth delegation rules.
- [[24 - Operator Console and Operations]]: add drain state, delivery target status, connector locks, provider fallback/cache break warnings, and WebSocket ticket status.
- [[25 - OpenAPI, Schemas, and Generated Clients]]: add schemas listed above.
- [[27 - Testing, Validation, and Replay]]: add fixtures for compression, provider fallback, auth tickets, secret scopes, kanban claims, and verification evidence.
- [[28 - Supply Chain, Deployment, and IaC]]: add immutable extension locks, component-license inventory, exact-artifact attestations, and release-workflow dependency gates.
- [[29 - Concurrency, Transactions, and Failures]]: add claim TTL/heartbeat/reclaim and profile scoped secret handling.
- [[32 - Integration Contract Map]]: add adapter config bridge, delivery target, editor session, and dashboard auth boundaries.
- [[34 - Canonical Object Model]]: add second-pass objects.
- [[41 - Observability Dashboards and Alerts]]: add cache-break, fallback, drain, secret conflict, claim liveness, and auth-ticket metrics.
- [[42 - Migrations, Retention, and Cleanup]]: add session/trajectory separation, verification evidence retention, and drain marker cleanup.

## Do Not Copy Blindly

Hermes is a useful source, but Sapphirus has a different product shape. Do not copy Hermes' full CLI surface, plugin tree, or dashboard implementation. Extract the contracts:

- provider identity must be explicit
- BMAD Method and Builder package versions must be immutable during a run
- one finalized `TurnCommit` must back persistence, delivery, memory promotion, and evidence
- failed/incomplete turns must not become durable memory truth
- secrets must be scoped and provenance-backed
- credential binding must use parsed canonical hosts, never URL substrings
- compression must be auditable
- compression abort must retain the byte-identical source transcript
- tools must be availability-gated before schema publication
- editor sessions must isolate cwd, approval, and cancellation
- scheduled jobs must be self-contained and recursion-guarded
- task claims need CAS plus heartbeat liveness
- browser/websocket credentials need short-lived single-use boundaries
- drain/restart state must handle stale markers safely
- provider collision must not imply replacement authority
- provider lookup must not auto-import an unapproved executable package
- extension acquisition/update must be immutable, signed, scanned, rehearsed, and versioned
- skill learning must remain proposal-only and fail closed when a gate or durable store is unavailable
- no-human approval and capability-resolution failure must deny in hosted execution
- executable extensions should be isolated rather than imported into the orchestrator
- automation claims and delivery need transactional persistence and a durable idempotent outbox
- release evidence must bind the exact source and artifact and gate publication
- aggregate BMAD run budgets must include delegated and background work
- root MIT does not authorize the proprietary PowerPoint skill; do not import, derive, or redistribute it without separate documented permission

## Related Vault Notes

- [[18 - Model Gateway and Microsoft Foundry]]
- [[14 - Builder Studio and SkillOps]]
- [[23 - Security, Identity, and Secrets]]
- [[25 - OpenAPI, Schemas, and Generated Clients]]
- [[28 - Supply Chain, Deployment, and IaC]]
- [[29 - Concurrency, Transactions, and Failures]]
- [[34 - Canonical Object Model]]
- [[86 - Hermes Source Code Review - Agent Runtime and Learning Loop]]
