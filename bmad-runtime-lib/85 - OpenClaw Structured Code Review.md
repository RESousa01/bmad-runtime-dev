---
title: "OpenClaw Structured Code Review"
aliases:
  - "OpenClaw Full Code Review"
  - "Structured OpenClaw Review"
tags:
  - bmad-runtime
  - vault/source-and-research
section: "Source and Research"
order: 85
vault_role: "source-review"
project: Sapphirus BMAD Runtime
status: current
reviewed_on: 2026-07-09
review_revision: "V6.16-full-snapshot"
source_archives:
  - "C:\\Users\\rodrigocsousa\\Downloads\\openclaw-main.zip"
related:
  - "[[84 - OpenClaw Source Review - Comparable Runtime Patterns]]"
  - "[[83 - BMAD Source Code Review - Method and Builder]]"
---

# OpenClaw Structured Code Review

> Historical source evidence. Current product and trust boundaries are [[93 - Split Web and Windows Desktop Architecture Plans]] through [[99 - Dual-Delivery Contract and Conformance Specification]]. Comparable code patterns do not override the selected-folder, Airlock, evidence, or executor authorities.

## Review Scope

This is a structured source review of the complete regular-file contents of `openclaw-main.zip`, focused on patterns that should improve the Sapphirus BMAD Runtime plan. The authoritative tree is `_full/o/openclaw-main`. The archive SHA-256 is `6D1F477A4C69204FB22C9480081281EB547FF2BC353592077559F02D01B4ED8E`; it contains 21,980 regular files, 1,293 directories, and 32 Unix links. Windows did not materialize the links, but they are `CLAUDE.md` aliases and workspace dependency links whose targets exist; no implementation-owner source is missing. The ZIP has no Git commit/tag metadata, so archive identity is reproducible while upstream revision identity is not.

This is not a recommendation to copy OpenClaw's product shape. Sapphirus remains a cloud-governed, BMAD-native runtime. The useful transfer is in contracts, safety boundaries, package governance, scenario proof, and user/operator diagnostics.

## Executive Findings

| Finding                                                                                      | Why it matters for Sapphirus                                                                                                                                                |
| -------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| OpenClaw separates plugin install policy, tool policy, sandbox placement, and exec approval. | Airlock should keep these as distinct policy layers so one approval cannot accidentally imply broad execution rights.                                                       |
| Extension metadata is manifest-assisted but runtime registration remains partly imperative.  | BMAD packages and future extensions need mandatory typed descriptors for capabilities, side effects, config, tools, compatibility, provenance, and permission scopes.       |
| Runtime config has one canonical shape.                                                      | Sapphirus should reject runtime fallback readers and move old config through explicit migrations/doctor repair flows.                                                       |
| Node `system.run` approvals concretely bind argv, canonical cwd, agent/session, normalized environment, node/requester context, and selected mutable files, then revalidate important TOCTOU seams. | Preserve that exact-request discipline, but make `ApprovedExecutionSpec` durable and also bind image/workspace, every mutable input, limits, policy, audience, expiry, and its own hash. Generic plugin consent is not execution authority. |
| Generated skills/packages enter a proposal queue.                                            | Builder Studio and SkillOps should create reviewable package proposals with draft hash, target hash, scan result, origin, and support files before install.                 |
| Maturity has a useful taxonomy, but current scoring includes self-scored and human-override data. | Release readiness should be computed from required scenario evidence, not manual or model-authored status.                                                               |
| Safe file operations are a first-class library.                                              | Workspace/package import must use root-bounded reads, safe archive extraction, symlink checks, atomic writes, and rollback snapshots.                                       |
| Hot paths use prepared snapshots.                                                            | Agent/runtime loops should carry prepared facts and policy snapshots instead of rediscovering package, tool, and workspace metadata repeatedly.                             |
| Public control envelopes are schema-owned, while some plugin payloads remain opaque.          | UI/plugin/session surfaces should use versioned discriminated payload schemas and avoid `Type.Unknown()` at governed action boundaries.                                     |
| Supply-chain controls are explicit.                                                          | Lockfile drift, minimum dependency age, shrinkwrap/published package locks, allowed native builds, and plugin-local dependency graphs should be release gates.              |
| SQLite task/flow/delivery state is strong for one gateway but is not a multi-writer work queue. | Sapphirus needs database-level claims/CAS, immutable attempts/events, heartbeats, stale-lease recovery, and non-dropping outbox/evidence authority. |
| Durable audit rows remain metadata-only and the async writer can drop them. | Governed effects must atomically persist evidence; audit, OTEL, logs, and support bundles are projections. |

## Repository Inventory

| Area       | Observed shape                                                                                                                                                               | Sapphirus implication                                                                                                                                          |
| ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Workspace  | PNPM monorepo with root app, `packages/*`, `extensions/*`, `examples/*`, docs, deploy, QA, and security folders.                                                             | Keep Sapphirus modular, but enforce one runtime control plane and explicit package boundaries.                                                                 |
| Packages   | 21 core package directories, including agent core, gateway protocol, plugin SDK, model catalog, markdown/media/speech cores, normalization, net policy, terminal, and tool-call repair. | Shared contracts and normalizers belong in small packages/modules instead of being copied across runtime, workers, and UI.                                  |
| Extensions | 140 plugin manifests found. Many declare `configSchema`, providers, channels, contracts, skills, and UI hints.                                                               | BMAD package metadata should remain primary, but the plan needs an optional extension descriptor boundary for future non-BMAD capabilities.                    |
| Protocol   | Gateway protocol uses JSON WebSocket frames with handshake, negotiated role/scopes, payload caps, feature lists, and method-level idempotency. Events have optional sequence/state version and are documented as non-replayed. | Define payload limits, scopes, negotiation, and idempotency, but back live streams with durable replay and resumable cursors. |
| QA         | 202 scenario YAML files define agent identity, kickoff task, metadata, coverage IDs, execution kind, flow steps, and assertions; proof strength varies by scenario.                    | Require action/event/state/artifact assertions for release gates, not only plausible response text.                                                |
| Maturity   | Scorecard tracks 50 surfaces and 281 capability areas with coverage, quality, completeness, support status, and promotion rules.                                             | Release gates should use a small but explicit maturity taxonomy for Sapphirus surfaces.                                                                        |

## Architecture and Governance

OpenClaw's root governance model is very useful: root policy owns hard boundaries, while local scoped instructions own workflows. It also insists that dependency-backed behavior must be inspected from source/contracts before custom implementation. That maps directly to Sapphirus as:

- Keep locked product rules in `00 - Common Rules and Product Shape`.
- Keep implementation-specific rules in component notes.
- Require source/contract inspection before adding package loaders, provider adapters, or external execution behavior.
- Do not let generated package instructions override platform policy.

The most important architecture lesson is that plugin or package extensibility should never pierce the core by convenience imports. OpenClaw routes plugin code through public SDKs, manifests, injected helpers, and public barrels. For Sapphirus, BMAD packages and future extensions should cross the platform boundary only through:

- BMAD package descriptors and validation results.
- Runtime ports for package loading, tool proposal, artifact creation, and evidence emission.
- Airlock policy context and `ApprovedExecutionSpec`.
- Generated clients and schema-versioned DTOs.

## Runtime and Tooling Review

OpenClaw's agent runtime has a before-tool-call chain that covers loop detection, Skill Workshop policy, trusted plugin policies, approval requests, ordinary plugin hooks, adjusted parameters, and diagnostics. Installed trusted plugin policies can modify parameters and request approval; unreadable/failed policies block. Tool availability and sandbox/workspace policy are also prepared during tool construction. These are related stages, not proof that a plugin should own platform trust. Sapphirus should use the clearer structure below:

1. Normalize model output into a platform-owned `Proposal`.
2. Build a complete policy context before the side effect.
3. Evaluate install policy, tool policy, sandbox/execution policy, secrets policy, network policy, and approval policy as separate gates.
4. Mint a narrow `ApprovedExecutionSpec`.
5. Dispatch only from that spec.
6. Preserve diagnostics explaining why an action was denied, downgraded, sandboxed, or escalated.

OpenClaw also sanitizes exec commands and environment details in logs. Sapphirus should log command hashes, argv metadata, executable identity, policy versions, and redaction counters, not raw secrets or broad environment dumps.

## Sandbox, Approval, and Policy Layers

OpenClaw's docs and executable source distinguish three layers. One hosted-boundary warning is essential: sandbox mode defaults to `off`, and exec defaults are `security: full`, `ask: off`. The strong Docker defaults (read-only root, tmpfs, no network, all capabilities dropped) apply only after isolation is enabled.

| Layer | Question answered | Sapphirus object |
|---|---|---|
| Sandbox | Where does execution run and what workspace/network access exists? | `ExecutionLane`, worker image, network policy, mount policy. |
| Tool policy | Which tools or actions are available at all? | `AirlockPolicy`, package capability allowlist, command DSL allowlist. |
| Exec approval | Is this exact execution allowed now? | `Approval`, `ApprovalGrant`, `ApprovedExecutionSpec`. |

The Sapphirus plan should keep this split. OpenClaw's server-minted `system.run` binding is strong: it compares exact argv/cwd/agent/session/environment, binds node/requester context, can hash a mutable operand, revalidates before execution, and consumes allow-once. It remains process-memory approval state and is narrower than the required Sapphirus grant. Generic plugin approval carries reviewer-facing plugin metadata, not a typed action payload or digest. An Airlock approval therefore binds the precomputed `ExecutionSpecCandidate` hash; only then may the server mint an audience-bound, single-use `ApprovedExecutionSpec`. Neither consent nor a plugin policy may rewrite sandbox placement, denied tools, install rights, inputs, limits, or network access after approval.

## Plugin and Package Model

Across the repository, OpenClaw manifests demonstrate a practical but non-uniform extension descriptor vocabulary:

- `id` and activation are universal in the reviewed manifests; names, descriptions, platform constraints, contracts, and UI hints are optional and unevenly populated.
- `providers`, `channels`, `skills`, `contracts`.
- `configSchema`, `uiHints`, setup/environment requirements.
- provider catalog entries and model catalog metadata.
- compatibility and migration metadata.

For Sapphirus, this should become a future-compatible `ExtensionManifest` and should also influence `BmadPackage`:

- BMAD packages remain the first-class package type.
- Optional extension descriptors are allowed only when validated and installed by policy.
- Package compatibility metadata must declare runtime API version, BMAD schema version, minimum host version, and builder output version where applicable.
- Package config schema and UI hints should be data, not hard-coded UI.
- Package-owned migrations must run through a doctor/migration contract, never runtime fallback parsing.

## Skills and Builder Studio

OpenClaw's skill workshop pattern is directly useful. Generated or updated skills are not silently installed; they are saved as proposals with origin, draft hash, target content hash, support files, scan result, goal, evidence, and applied/rejected timestamps.

Builder Studio should use the same pattern:

- A generated package is a `SkillPackageProposal`, not an installed package.
- The proposal stores source prompt, origin run/session, generated package hash, target package identity, validation result, and security scan result.
- Applying a proposal is a side effect and requires Airlock.
- Rejected proposals remain evidence for future debugging.
- Support files are bounded by count, size, file type, and safe extraction rules.

## Config, Secrets, and Migration

OpenClaw's strongest config lesson is strict: runtime reads the current schema only; migration and repair live in `doctor`-style flows. Sapphirus should apply this rule to:

- BMAD package schemas.
- Builder-generated package schemas.
- project configuration.
- model/provider configuration.
- Airlock and execution policy.

Secrets handling should follow a plan/apply contract. OpenClaw validates secret mutation plans before write, checks path segments, rejects prototype-pollution names, ensures provider/account consistency, snapshots touched files, restores on failure, and scrubs plaintext residue. Sapphirus should express this as `SecretsApplyPlan` plus rollback evidence and should avoid ad hoc partial secret writes.

## QA, Maturity, and Release Gates

OpenClaw's QA scenario pack is more structured than the existing Sapphirus notes. Its format is useful, but scenario presence and model-authored maturity scores are not proof by themselves:

- one scenario file declares surface, coverage IDs, objective, success criteria, docs refs, code refs, execution kind, required files, and prompt;
- flow steps can reset state, run an agent prompt, wait for conditions, assert tool calls, and capture evidence;
- coverage IDs map scenarios to maturity taxonomy entries;
- mature areas need repeated machine-verifiable proof, not just implementation, response-text matching, model scoring, or a human override.

## V6.15 Audit Corrections

### Evidence and Provenance Limit

The reviewed tree is package version `2026.6.11` and includes the gateway, plugin loader, config, context engine, task, logging, infra, UI, test, and diagnostics owners that were absent from the earlier extraction. The archive SHA-256 and regular-file/link inventory are now recorded. It is still not a Git checkout and carries no commit/tag mapping. Source Intake must retain the archive digest, acquisition path/date, extraction and link report, reviewed-path inventory, per-component notices/licenses, and test status, and must not claim equivalence to an upstream revision without a separate mapping.

### Newly Confirmed Plan-Changing Patterns

- `src/tasks/**` and the subagent registry separate task/flow execution, completion, owner scope, notification/delivery, retries, generation, and recovery. SQLite persistence is useful, but flow revision checks are in-memory before unconditional upsert, subagent persistence can replace a whole snapshot, some failures are swallowed, and there is no immutable attempt ledger. Sapphirus therefore needs BMAD-native `WorkItem`, `WorkAttempt`, `WorkLease`, `Completion`, and `OutboxMessage` with database CAS/idempotency, heartbeat/reclaim, and immutable events rather than chat child sessions as the work model.
- `src/context-engine/**` defines bootstrap, assemble, ingest, maintenance, token budgets, epoch/fingerprint, degradation, quarantine, and untrusted projections. Failed custom engines can silently fall back to the default. Sapphirus should expose a deterministic `ContextPack` lifecycle with provenance, policy and token budget; optional enrichment may degrade visibly, while required BMAD method/artifact context fails closed.
- `src/agents/sessions/session-manager.ts::SessionManager` proves the value of parent-linked history, branching, compaction entries, and checkpoints, but also shows the cost of JSONL synchronization, cache validation, rewrites, and side-branch recovery. Use database-backed event lineage and optimistic concurrency for cloud state.
- `packages/gateway-protocol` provides useful negotiation, scopes, capabilities, and payload limits. Gateway broadcast state is per-client/in-memory, while the SDK retains only 1,000 process-local events and clears them on close. Sapphirus needs a durable owner-scoped stream with cursor resume, retention, gaps, projection checkpoints, schema upcasting, and idempotent replay behind SSE/WebSocket projections.
- `src/audit/**` provides idempotent SQLite metadata rows, stable cursors, bounded retention, and operator reads. The async writer can drop events when its 4,096-item queue is full or unavailable, and the contract omits approval, policy, spec, artifact, and evidence references. Sapphirus Evidence Ledger must be transactional/non-dropping; audit and OTEL are export projections.
- `src/gateway/methods/core-descriptors.ts` centralizes action name, scope, startup availability, and control-plane-write classification before dispatch. Sapphirus should own a canonical `ActionDescriptor` that also binds owner scope, input/output schema, side-effect and approval class, rate/retention, and evidence policy.

### Explicit Non-Adoption Boundaries

- Do not adopt heuristic silent memory capture. `extensions/memory-lancedb/index.ts::MemoryEntry` lacks tenant/project/source/evidence/consent/revision fields, `shouldCapture` uses regex triggers, and the `agent_end` hook stores matches automatically. BMAD knowledge requires explicit promotion, provenance, scope, retention, supersession, and deletion evidence.
- Do not treat generic plugin approvals as execution grants. `PluginApprovalRequestParamsSchema` carries reviewer copy and decisions, not a parameter/spec hash. Airlock must authorize an immutable `ApprovedExecutionSpec`.
- Do not treat opaque plugin JSON as a governed UI contract. `PluginJsonValueSchema` is `Type.Unknown()`; Builder and operator actions require versioned discriminated schemas.
- Do not treat the 50-surface/281-category scorecard as objective maturity proof. `qa/maturity-scores.yaml` includes Codex-authored scoring and human overrides, and generated coverage is low on several relevant surfaces. Only immutable scenario runs with required evidence should promote a BMAD surface.
- Do not execute native plugins in the Runtime API process. OpenClaw documents plugins as unsandboxed and core-equivalent trust; contract/dispatch entitlement is API hygiene, not containment. BMAD packages stay inert and executable extensions use isolated workers with explicit grants.
- Do not inherit sandbox-off or exec-full/ask-off defaults into a hosted workspace. Production isolation and deny-by-default authorization are mandatory.

### License Boundary

OpenClaw's root is MIT licensed (`LICENSE`, copyright 2026 OpenClaw Foundation), and `THIRD_PARTY_NOTICES.md` records Pi/pi-mono and `@earendil-works/pi-tui` MIT attribution. The distributed tree also contains component licenses/notices, including Apache-2.0 for `skills/skill-creator/license.txt`, and `skills/` is included in the npm package. Studying patterns does not imply one blanket reuse decision. Any copied, adapted, packaged, or redistributed component requires a per-component `LicenseDecision`, retained notices, rights/provenance record, and SBOM entry.

BMAD Method and BMAD Builder remain the foundation and workflow authority. OpenClaw patterns sit below that foundation as runtime infrastructure; they must not replace BMAD package semantics, compiled workflow state, Builder proposal/review flow, artifact ownership, or Airlock authority.

Sapphirus should add a small release-readiness model:

| Level | Meaning |
|---|---|
| M0 Planned | Design exists, but no supported path. |
| M1 Experimental | Maintainer can run it from source with caveats. |
| M2 Alpha | Real users can try it with known caveats and docs. |
| M3 Beta | Main workflow is usable with bounded caveats and regression tests. |
| M4 Stable | Recommended path; failures are release regressions. |
| M5 Polished | Stable plus representative user scorecard pass. |

For the first Sapphirus slice, the surfaces should be: Chat Workbench, Runtime API, Run Orchestrator, BMAD Kernel, Airlock, Execution Lanes, Workspace Service, Evidence/Trace, and Package Import.

## Supply Chain Review

OpenClaw's workspace and release docs show several useful gates:

- minimum package release age for dependency resolution;
- explicit overrides and allowed native builds;
- shrinkwrap for published npm packages so installs use a reviewed transitive graph;
- plugin-local dependency locks;
- install policy hooks for plugin/package installation;
- package compatibility metadata checked before load.
- a commit-bound Full Release Validation workflow emits a validation manifest and is required for stable npm publication; specifically named alpha lanes are advisory rather than silently ignored.

Sapphirus should add equivalent release checks even if the stack is not identical: NuGet lock/restore policy, npm lockfile drift detection, container image digest pinning, SBOM generation, dependency age/provenance checks where available, and package-local dependency review for imported BMAD/Builder artifacts.

## Do Not Copy

| OpenClaw pattern | Why not copy directly |
|---|---|
| Local-first gateway daemon as the central product shape. | Sapphirus is a cloud-governed runtime with Azure-hosted execution and audit. |
| Broad provider/channel marketplace in v1. | Sapphirus should prove BMAD-native execution first. |
| Host tool execution defaults. | Sapphirus side effects must go through isolated execution lanes and Airlock. |
| Plugin-driven UI breadth early. | Builder Studio and broad extension surfaces remain after the executable vertical slice. |
| Non-replayed WebSocket events as control-plane truth. | Cloud runs require durable events, resume cursors, and rebuildable projections. |
| JSONL session files as authoritative run state. | BMAD runs need transactional, versioned cloud state and separate artifact storage. |
| Heuristic automatic long-term memory capture. | Project knowledge needs explicit promotion, provenance, scope, retention, and deletion evidence. |
| Generic plugin approvals and opaque action payloads. | Airlock actions must bind a typed, hashed execution or mutation spec. |
| Runtime fallback readers for old config. | OpenClaw explicitly rejects this too; Sapphirus should follow the stricter migration-only model. |

## Required Plan Updates

The following Sapphirus notes should carry the OpenClaw-derived corrections:

- [[19 - Airlock Policy and Approvals]]: split install policy, tool policy, sandbox policy, exec approval, and reusable grants.
- [[20 - Execution Lanes and Container App Jobs]]: bind execution specs to exact argv, cwd, env policy, image digest, mutable input hashes, and sandbox/mount policy.
- [[23 - Security, Identity, and Secrets]]: add safe archive extraction, secrets apply plans, rollback snapshots, and path-segment validation.
- [[25 - OpenAPI, Schemas, and Generated Clients]]: add schema-owned descriptors for package manifests, approvals, scenarios, UI actions, and event streams.
- [[27 - Testing, Validation, and Replay]]: add scenario manifests, coverage IDs, maturity taxonomy, and proof assertions.
- [[28 - Supply Chain, Deployment, and IaC]]: add dependency age/provenance, lockfile drift, shrinkwrap/published lock equivalents, and package install policy.
- [[32 - Integration Contract Map]]: add package/extension descriptor boundary.
- [[34 - Canonical Object Model]]: add `ExtensionManifest`, `PackageInstallPolicy`, `SkillPackageProposal`, `ScenarioManifest`, `MaturitySurface`, `ConfigMigrationPlan`, and `SafeArchiveExtractionReport`.
- [[39 - BMAD Package Format]]: add compatibility metadata, config schema, UI hints, migration paths, and install provenance.
- [[34 - Canonical Object Model]]: add `ActionDescriptor`, `WorkItem`, `WorkAttempt`, `WorkLease`, `OutboxMessage`, `EvidenceLedgerEvent`, `EventCursor`, and `ProjectionCheckpoint`.
- [[53 - Event Taxonomy and Stream Protocol]]: define reconnect/restart replay, cursor expiry/gap reconciliation, at-least-once dedupe, projection rebuild, and schema upcasting; replay must never rerun side effects.
- [[90 - LLM-Tailored Development Plan and Agent Workflow]]: test concurrent claims, stale leases, DB revision conflicts, persistence/evidence saturation, approval mismatch/TOCTOU, and proof that plugin consent cannot authorize execution.
