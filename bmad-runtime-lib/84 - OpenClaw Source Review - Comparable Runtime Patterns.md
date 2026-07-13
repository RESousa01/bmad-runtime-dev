---
title: "OpenClaw Source Review - Comparable Runtime Patterns"
aliases:
  - "OpenClaw Source Evidence"
  - "OpenClaw Comparable Runtime Review"
tags:
  - bmad-runtime
  - vault/source-and-research
section: "Source and Research"
order: 84
vault_role: "source-review"
project: Sapphirus BMAD Runtime
status: current
reviewed_on: 2026-07-09
review_revision: "V6.16-full-snapshot"
source_archives:
  - "C:\\Users\\rodrigocsousa\\Downloads\\openclaw-main.zip"
---

# OpenClaw Source Review - Comparable Runtime Patterns

> Historical comparable-runtime review. Reuse patterns only after classifying them as shared, `web_managed`, or `windows_local`; this note does not authorize self-hosting, containers, broad local tools, or a merged runtime authority.

This note captures OpenClaw patterns that are useful for the Sapphirus BMAD Runtime plan. It is not a request to copy OpenClaw architecture. The value is in the similarities: governed agent execution, skills/plugins, sandboxed tools, approvals, gateway/control UI, and scenario-based QA.

## Reviewed Source

| Source area | Reviewed focus |
|---|---|
| `README.md`, `VISION.md`, `AGENTS.md` | Product posture, local-first gateway, plugin boundaries, config compatibility, review/testing discipline. |
| `docs/gateway/sandboxing.md` | Sandbox modes, scope, backend, workspace access, elevated escape hatches, no-network defaults. |
| `docs/tools/exec-approvals.md`, `src/infra/system-run-approval-binding.ts`, `src/gateway/node-invoke-system-run-approval.ts` | Host exec approvals, allowlists, strict inline eval, fallback behavior, concrete request binding, and execution-time revalidation. |
| `docs/gateway/security/secure-file-operations.md` | Root-bounded file operations, archive limits, atomic writes, symlink/link protections. |
| `docs/tools/plugin.md`, `extensions/*/openclaw.plugin.json` | Plugin manifest structure, activation, contracts, config schema, provider setup, UI hints, install policy. |
| `docs/tools/skills.md` | Skill precedence, agent allowlists, skill proposal/workshop lifecycle, install verification. |
| `qa/scenarios/index.yaml`, `qa/scenarios/**/*.yaml` | Scenario pack format, coverage IDs, runtime parity tiers, flow steps, proof expectations. |
| `packages/gateway-protocol/src/schema/*.ts` | Plugin UI descriptor and plugin approval payload schemas. |
| `src/tasks/**`, `src/audit/**`, `src/state/**` | SQLite task/flow/delivery state, owner access, local recovery, durable metadata audit, cursoring, and retention. |
| `src/context-engine/**`, `src/plugins/**`, `src/agents/sandbox/**` | Context lifecycle/degradation, plugin activation and trusted policies, sandbox defaults, and containment limits. |
| `.github/workflows/**` | Aggregate release validation, commit-bound validation manifests, stable release gates, and explicitly advisory lanes. |

The authoritative archive is `C:\Users\rodrigocsousa\Downloads\openclaw-main.zip`, SHA-256 `6D1F477A4C69204FB22C9480081281EB547FF2BC353592077559F02D01B4ED8E`, package version `2026.6.11`. It contains 23,305 entries: 21,980 regular files, 1,293 directories, and 32 Unix symlinks. All regular members are present under `_full/o/openclaw-main` with matching sizes; the 32 links are 23 `CLAUDE.md -> AGENTS.md` aliases and nine workspace/`node_modules` links that Windows did not materialize. Their targets remain available, so implementation-owner source is not missing, but the tree is not directly install-runnable until workspace links are reconstructed. The ZIP contains no `.git`, commit, or tag metadata, so archive identity is reproducible while upstream revision identity is not.

## Transferable Patterns

| Pattern | OpenClaw evidence | Sapphirus plan improvement |
|---|---|---|
| Skills and plugins are separate trust surfaces | Skills teach behavior; plugins expose runtime capabilities, config, providers, tools, channels, and UI surfaces through manifests. | Keep BMAD skills/packages separate from runtime extensions; do not let package metadata imply execution authority. |
| Install policy is separate from runtime tool policy | `security.installPolicy` gates plugin/skill install before runtime activation. | Add explicit package-import/install policy distinct from Airlock execution policy. |
| Sandbox policy is not tool policy | Sandbox mode/scope/backend/workspace access are separate from tool allow/deny/elevated rules. Sandbox mode itself defaults to `off`; when Docker isolation is enabled, defaults include a read-only root, tmpfs, no network, and all Linux capabilities dropped. | Execution lanes should model isolation, tool permission, approval, and network as independent fields, with production worker isolation enabled by default. |
| `system.run` approval binds a concrete request | Server-minted bindings cover exact argv, canonical cwd, agent/session identity, normalized environment hash, node/requester context, and an optional mutable-file hash; mismatch and TOCTOU checks fail closed, and allow-once is consumed atomically in the manager. The record is still process-local and omits image digest, workspace snapshot, policy version, all mutable inputs, and a canonical spec hash. Generic plugin approval is only reviewer consent metadata. | Keep consent separate from authorization. `ApprovedExecutionSpec` must be durable and bind command, cwd, env policy, image, workspace snapshot, all mutable inputs, limits, policy version, approval, and its own hash. |
| Personal-assistant approval defaults are unsafe for hosted use | Core exec defaults are `security: full` and `ask: off`; `askFallback: deny` only governs a prompt path that may not be active. Trusted full/fallback modes also exist. | Hosted Airlock defaults must deny execution unless an explicit policy path authorizes it; unavailable/timeout/no-principal behavior is unconditionally closed. |
| Strict inline eval | Interpreter inline forms such as `python -c` and `node -e` need explicit approval even if the interpreter is allowlisted. | Airlock command policy should classify inline eval as high risk or blocked by default. |
| Safe filesystem primitives | File helpers enforce root-bounded paths, atomic replace, archive limits, link protections, and private modes. | Workspace and package extraction code should use structured safe-file operations, not ad hoc path-prefix checks. |
| Runtime reads canonical config only | Compatibility is handled by doctor/migration; steady-state runtime reads current config shape. | Avoid long-lived runtime shims for old BMAD/runtime config; put migration into explicit import/doctor flows. |
| Plugin UI descriptors | Plugin UI actions have typed envelopes for plugin id, surface, label, and scopes, but action schema/result payloads are opaque (`Type.Unknown()`). | Future Builder/SkillOps extension UI should require versioned, discriminated action payload schemas rather than opaque JSON. |
| Skill proposal queue | Skill Workshop drafts proposals and requires user approval before changing skills. | Builder-generated skill/package edits should remain draft proposals until validated and approved. |
| Scenario QA packs | YAML scenarios have objective, coverage IDs, docs/code refs, execution kind, flow steps, and assertions. Some scenarios still prove outcomes through response text or other proxies rather than the actual action event. | Replay fixtures should gain scenario manifests with coverage IDs and machine-verifiable tool, event, state, and artifact proof targets. |

## V6.15 Audit Corrections

| Correction | Source evidence | BMAD Workspace consequence |
|---|---|---|
| The archive is complete for regular files but not commit-mapped. | The verified ZIP supplies the previously missing gateway, plugins, config, context-engine, tasks, logging, infra, UI, and test owners. Thirty-two symlinks were inventoried but not materialized. No Git ref is embedded. | Treat the archive SHA, extraction report, link inventory, component licenses, and review paths as auditable evidence; do not claim equivalence to an upstream commit/tag until separately mapped. |
| Inventory is 21 core package directories, 140 plugin manifests, and 202 QA scenario YAML files in this snapshot. | `packages/*`, `extensions/*/openclaw.plugin.json`, `qa/scenarios/**/*.yaml`. | Correct the earlier 20-package count and treat breadth as inventory, not proof of maturity. |
| Manifest-driven extensibility is rich but not a complete permission model. | All 140 manifests have `activation` and `configSchema`; 77 have `contracts`, 31 `uiHints`, and only two a `kind`. Tool declarations, gateway dispatch entitlements, and route ownership are enforced, but native plugins activate in-process with core-equivalent trust and can register global runtime effects. | BMAD packages remain inert data. Executable extensions require explicit filesystem/network/secret/tool/evidence grants and isolated workers; descriptor metadata never grants authority. |
| Task state is a strong single-gateway recovery model, not distributed work authority. | SQLite persists task, flow, completion, delivery, retry, generation, and parent/child state. However, flow revision checks occur in memory before unconditional upsert, subagent saves replace whole snapshots, some persistence errors are swallowed, and no immutable attempt/event ledger exists. | Model cloud work as `WorkItem -> Attempt -> Lease -> Completion -> Outbox`, with database-level CAS, idempotency, heartbeats, stale-lease recovery, immutable events, and owner scope. A BMAD run/workflow remains orchestration authority. |
| Context engines have useful lifecycle and health semantics, but custom-engine fallback can be silent. | Context contracts cover bootstrap, assemble, ingest, maintenance, budgets, thread epochs/fingerprints, degraded reasons, quarantine, and untrusted-reference projection. Failed custom engines are silently replaced by the default. | Optional enrichment may degrade only with explicit user/operator evidence. Required BMAD method/artifact context assembly fails closed rather than silently changing engines. |
| Session lineage is useful; JSONL authority is not. | `src/agents/sessions/session-manager.ts` implements an append-only parent-linked tree with compaction and branch summaries, plus substantial sync-file, cache, rewrite, and side-branch handling. | Preserve event lineage, branches, and compaction checkpoints in database-backed, versioned state; store large artifacts in blob storage rather than making JSONL the cloud source of truth. |
| OpenClaw memory is not an acceptable BMAD knowledge model. | `extensions/memory-lancedb/index.ts` uses heuristic auto-capture. `MemoryEntry` lacks tenant/project/source/evidence/consent/revision fields, and `memory_forget` deletes one vector record while describing itself as GDPR-compliant. | Require explicit promotion, scope, provenance, evidence links, supersession, retention/legal state, and deletion audit. Do not silently turn chat text into project truth. |
| Gateway delivery has only bounded process-local replay. | Gateway broadcast sequence is per-client/in-memory and targeted events may omit it; the SDK event hub retains at most 1,000 process-local events and clears them on close. There is no reconnect/restart replay authority. | Keep negotiation, scopes, payload limits, and idempotency, but put a durable owner-scoped event/evidence log with resumable cursors, retention, gap reconciliation, and projection checkpoints behind SSE/WebSocket delivery. |
| Maturity scoring is structured but not objective release proof. | `qa/maturity-scores.yaml` records 50 surfaces/281 category scores, includes Codex-authored runs and human overrides, while generated scorecard coverage is low for several relevant surfaces. | Use the taxonomy shape, but promotion must be computed from immutable scenario runs and required machine evidence, never self-scored status. |
| Audit metadata is durable but best-effort and too thin for Sapphirus evidence. | SQLite audit rows have idempotent source ids, stable sequence cursors, 30-day/100,000-row bounds, and operator reads. The asynchronous writer can drop events when its 4,096-item queue is full or unavailable, and rows omit approval, policy, execution-spec, artifact, and evidence references. | Governed side effects must transactionally/outbox-persist non-dropping Evidence Ledger records. Audit, OTEL, logs, dashboards, and support bundles are projections. |
| Release validation is stronger than ordinary CI aggregation. | Stable publication requires a successful commit-bound Full Release Validation run and writes a validation manifest; named Tideclaw alpha lanes are explicitly advisory. | Produce a target-SHA/package-hash release evidence manifest that enumerates required child runs and artifacts; advisory lanes must be explicit and cannot satisfy stable gates. |

### License and Adoption Boundary

OpenClaw's root is MIT licensed (`LICENSE`, copyright 2026 OpenClaw Foundation), and `THIRD_PARTY_NOTICES.md` records adapted Pi/pi-mono portions and `@earendil-works/pi-tui` under MIT. The archive is not uniformly MIT: additional notice/license files exist in bundled components, and `skills/skill-creator/license.txt` is Apache-2.0 while `skills/` is included in the npm artifact. Source Intake therefore needs a per-component `LicenseDecision`, notice inventory, redistribution decision, and SBOM lineage rather than one root SPDX assumption.

BMAD Method and BMAD Builder remain the product foundation and workflow authority. OpenClaw contributes infrastructure patterns only: typed boundaries, policy staging, sandbox backends, session lineage, durable child-work delivery, context lifecycle, and scenario QA. It must not redefine the BMAD package format, compiled workflow semantics, artifact lifecycle, Builder proposal model, or Airlock authority.

## Planning Changes Applied

1. [[12 - Run Orchestrator and Agent Kernel]] now calls out prepared facts, extension descriptors, and proposal queues for generated skill/package changes.
2. [[19 - Airlock Policy and Approvals]] now distinguishes install policy, tool policy, sandbox policy, and approval grants; it also adds strict inline-eval and no-UI fallback requirements.
3. [[20 - Execution Lanes and Container App Jobs]] now models sandbox/isolation mode, workspace access, and network as separate execution fields.
4. [[23 - Security, Identity, and Secrets]] now requires safe file/archive primitives and migration-only compatibility for old config shapes.
5. [[27 - Testing, Validation, and Replay]] now adds scenario-pack manifests with coverage IDs and proof-oriented flow assertions.

## Local Source Pointers

- OpenClaw package: `_full/o/openclaw-main/package.json`
- OpenClaw root guidance: `_full/o/openclaw-main/AGENTS.md`
- Vision: `_full/o/openclaw-main/VISION.md`
- Sandboxing: `_full/o/openclaw-main/docs/gateway/sandboxing.md`
- Exec approvals: `_full/o/openclaw-main/docs/tools/exec-approvals.md`
- System-run binding: `_full/o/openclaw-main/src/infra/system-run-approval-binding.ts`
- Safe file operations: `_full/o/openclaw-main/docs/gateway/security/secure-file-operations.md`
- Plugins: `_full/o/openclaw-main/src/plugins/`
- Task/flow state: `_full/o/openclaw-main/src/tasks/`
- Audit store: `_full/o/openclaw-main/src/audit/`
- Context engines: `_full/o/openclaw-main/src/context-engine/`
- QA scenarios: `_full/o/openclaw-main/qa/scenarios/index.yaml`
- Gateway protocol schemas: `_full/o/openclaw-main/packages/gateway-protocol/src/schema/`
