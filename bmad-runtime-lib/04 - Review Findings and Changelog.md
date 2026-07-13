---
title: "Review Findings and Changelog"
aliases:
  - "04 - Review Findings and Changelog"
tags:
  - bmad-runtime
  - vault/foundation
section: "Foundation"
order: 4
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: review-changelog
status: v6-modernized-validated-implementation-guide
generated_on: 2026-07-09
review_pass: v6-modernization-and-platform-validation
architecture_rule: governed-chat-first-agentic-runtime
---



# Review Findings and Changelog

## 2026-07-10 — V6.18 UX/UI implementation-readiness pass

- Rebuilt [[43 - Product UX Flows and Wireframe Notes]] as the canonical product UX blueprint with concrete information architecture, responsive shell behavior, first-slice journey, approval anatomy, RunCapsule interaction model, recovery states, keyboard/focus rules, motion boundaries, and usability gates.
- Turned [[26 - Frontend Design System]] from a token checklist into an implementable visual system with light/dark semantic values, typography, spacing, geometry, density, icon, motion, performance, and visual-regression decisions.
- Locked an evidence-backed frontend UX baseline: React Aria Components, Tailwind CSS 4 semantic tokens, Lucide, Motion, resizable panels, TanStack Virtual, a gated Pierre Diffs spike, safe React Markdown, Storybook, Vitest/Testing Library/axe, and Playwright/axe.
- Reworked [[10 - Chat Workbench]] around one progressive `RunCapsule` per run and a stable evidence inspector instead of one permanent large card per event.
- Expanded [[66 - Frontend Component Specification]] with shell, run, approval, responsive, motion, test, and component-ownership contracts.
- Added measurable UX foundation stories to [[61 - First Vertical Slice User Stories]] and moved design approval/component fixtures ahead of production route wiring in the master sequence and backlog.
- Kept Odysseus as a clean-room interaction reference only: retained its calm workbench, narrow rail, density, responsive panels, and functional micro-motion while rejecting its monolithic CSS/JS, modal-everything structure, tiny all-monospace text, novelty themes, and animation sprawl.
- Resolved the frontend target-path ambiguity: new work uses `apps/web`, `apps/desktop-ui`, and `packages/ui`; `src/web` is explicitly deprecated.

## 2026-07-10 — V6.18 BMAD Method and Builder deep comprehension audit

- Added [[100 - BMAD Method and Builder Deep Comprehension Audit]] as the current semantic source authority for all reviewed Method skills and Builder authoring/evaluation surfaces.
- Corrected the runtime model from one assumed workflow DSL to profile-aware prompt-native execution: direct, inline, JIT-step, rendered, compatibility-shim, persona, multi-agent, and headless archetypes.
- Split installed skills from Help Advisor actions; split Method central TOML, per-skill customization TOML, and profile-specific YAML; treated upstream install manifests as declared staging metadata rather than final observed host inventory.
- Corrected Builder drift: Convert is removed upstream, the eval runner is missing from the standard plugin/help surface, current eval contracts differ from published docs, and clean cwd/reduced env are not containment.
- Rebased the plan so inactive Builder Build/Edit/Analyze drafts arrive with the foundation, while candidate execution, evaluation, rehearsal, signing, publication, activation, memory, and autonomy remain evidence-gated.
- Added explicit validation profiles and safe staging/adapter/path/memory boundaries; updated context, object/state, package, validation, roadmap, backlog, weekly plan, and AI-agent guidance notes.

## 2026-07-10 — V6.17 dual-delivery architecture contract

- Promoted [[93 - Split Web and Windows Desktop Architecture Plans]] to current authority and made `Project.deliveryModel` immutable.
- Preserved the cloud-first React/.NET/Azure architecture as `web_managed` and added a separate Tauri/Rust/SQLite local authority as `windows_local`.
- Added [[94 - Windows Desktop Native Host and IPC]], [[95 - Windows Local Workspace and Execution]], [[96 - Windows Local State, Evidence, Checkpoint, and Rollback]], [[97 - Windows Desktop Security and Trust Model]], [[98 - Azure Support Plane for Windows Desktop]], and [[99 - Dual-Delivery Contract and Conformance Specification]].
- Scoped Azure SQL/Blob/ACA worker contracts to web-managed or explicit remote work; they are not the ordinary desktop edit path.
- Reserved `sealed_test_fake` for the deterministic development fixture so “local” no longer confuses a test double with the installed desktop product.
- Corrected desktop patch language to journaled crash-recoverable batches with per-file atomic replacement where supported; no multi-file atomicity claim.
- Made DESK-01 explicit: Job Objects manage process trees/resources but do not prove filesystem or network confinement for child tools.
- Defined remote jobs as a separate `web_managed` handoff with explicit upload, no direct local apply, and fresh local policy/approval/checkpoint on import.

## V6.16 full-source, technology, LLM, and cloud-first review (2026-07-09)

### Review completed

- Re-extracted the user-supplied OpenClaw and Hermes ZIPs into complete authoritative trees and recorded archive hashes, entry types/counts, verification confidence, symlinks, recovered files, missing Git identity, and component-license boundaries in [[92 - Source Snapshot Verification and Adoption Ledger]].
- Re-reviewed full OpenClaw/Hermes owners and corrected earlier overclaims about approvals, task durability, audit/replay, plugin isolation, context fallback, hosted defaults, skill staging, turn commits, memory, credential routing, automation, compression, budgets, and release evidence in notes `84`–`89`.
- Kept BMAD Method and BMAD Builder as the authoritative product/method foundation while explicitly separating runtime identity, state, Airlock, cloud execution, and evidence authority.
- Used Context7 for framework/API context and official primary documentation for current validation. Locked .NET 10, Node 24/pnpm 11, React 19.2 + Vite 8 + React Router 8 SPA, TypeScript 7 with an isolated TypeScript 6 compiler-API gate, per-worker Python profiles, and OpenAPI 3.1/JSON Schema 2020-12.
- Rebuilt the LLM plan around app-owned state, Azure/OpenAI v1 Responses with `store=false`, hosted tools off, exact provider capabilities/credential binding, schema projection plus canonical validation, role-based model profiles, four-lane evaluation, canary promotion, explicit fallback, and rollback.
- Locked the user's hardware constraint: no local Docker/Kubernetes/emulators/model serving. Local work uses trusted deterministic fakes only; ACR Tasks/hosted CI builds images remotely; a fixed-template ACA Job is the first real isolated execution.
- Reconciled the phase sequence across `00`, `01`, `02`, `34`, `51`, `53`, `54`, `72`, `89`, `90`, and [[91 - Technology, Language, Method, and LLM Implementation Review]]. Azure identity/storage/registry/job foundations now precede real provider/job consumers.

### Canonical naming corrections

| Retired/ambiguous term | Canonical term or rule |
|---|---|
| `ExecutionManifest` | `WorkerResultManifest` |
| `TraceBundle` as proof authority | Diagnostic `TraceProjection`; durable `EvidenceLedgerEvent`; canonical user-facing `EvidenceBundle` |
| Generic approval authorizes execution | Exact `ExecutionSpecCandidate` hash is evaluated/approved; Airlock mints an audience-bound, expiring, single-use `ApprovedExecutionSpec` |
| In-memory task/event/provider state as recovery | Durable `WorkItem -> Attempt -> Lease -> Completion -> Outbox` plus sequenced Evidence Ledger |
| `sealed_test_fake` as sandbox | Deterministic test double only; real web isolation begins in Azure |

### New review notes

- [[91 - Technology, Language, Method, and LLM Implementation Review]]
- [[92 - Source Snapshot Verification and Adoption Ledger]]

---

## 1. Review Summary

The first library version had the correct architecture shape but still read like a broad implementation brief. The v2 library turns it into a stricter implementation contract set.

## 2. Problems Found

| Problem | Severity | Fix Applied |
|---|---|---|
| Several files used the same generic structure and lacked block-specific contracts. | High | Added APIs, schemas, state machines, tests, and release gates. |
| MVP build order was present but not enforced strongly enough. | High | Locked canonical build order in `00 - Common Rules and Product Shape.md` and backlog. |
| Runtime API boundaries needed stronger anti-god-object language. | High | Added module ownership matrix and port rules. |
| Airlock spec requirements needed to appear in multiple affected files. | High | Added spec invariant, policy matrix, approval grants, tests. |
| Execution lane needed a worker protocol. | High | Added worker input/output contract and result manifest. |
| SQL/Blob split needed concrete layout and idempotency. | Medium | Added entity relationship, Blob layout, idempotency keys. |
| Frontend needed more implementation-grade state/card details. | Medium | Added route model, card types, UI state machine, tests. |
| BMAD/Builder/Presentation needed sharper v1 cut line. | Medium | Added adapter-not-rewrite, Builder v1 scope, package validation gates. |

## 3. Files Improved

- `00 - Common Rules and Product Shape.md`: added audit verdict, universal invariants, release gates.
- `01 - First Build - Executable Vertical Slice.md`: added API endpoints, SQL tables, event types, negative tests.
- `02 - Locked Architecture Decisions.md`: added decision ledger, spike evidence, temporary decision sunset rules.
- `03 - Repository and Vault Usage.md`: added monorepo structure, ownership, generated artifact policy.
- `10` through `29`: expanded with block-specific implementation details.
- `30 - Implementation Epics and Backlog.md`: added first 20 delivery stories and milestone demos.
- `31 - Architecture Decision Records.md`: added first ADR set, acceptance criteria, and ADR debt register.

## 4. New Files Added

| File | Purpose |
|---|---|
| `04 - Review Findings and Changelog.md` | Records what was wrong with v1 and what changed. |
| `32 - Integration Contract Map.md` | Shows cross-block calls, ownership, and forbidden bypasses. |
| `33 - Release Gates and Acceptance Matrix.md` | Converts the plan into hard implementation gates. |
| `34 - Canonical Object Model.md` | Defines durable runtime objects and their lifecycle role. |
| `35 - Source Alignment Notes.md` | Captures public-source alignment used to tune platform decisions. |

## 5. Remaining Risks

| Risk | Mitigation |
|---|---|
| Library still large enough to overwhelm implementation agents. | Always start from `00`, `01`, `30`, and the current block file only. |
| Some cloud decisions require live Azure benchmark. | Phase-0 spikes remain mandatory. |
| Presentation workflow adapter depends on exact existing workflow assets. | Inventory before implementing adapter. |
| BMAD contracts may evolve. | Parser schemas must be versioned and fixture-backed. |

---


## V6.14 Comparable-Runtime Deep Dive (2026-07-09)

## Review method

Second, code-level pass over Odysseus, OpenClaw, and Hermes in `_source_review/` (the V6.13 pass verified the earlier reviews; this pass read the runtime modules themselves). Findings were written into the owning implementation notes; the full finding-to-note map lives in [[89 - Consolidated AI Workspace Source Review and Architecture Improvements]] → "Second-Pass Deep Dive (V6.14)".

## Changes applied

| Note | Change |
|---|---|
| `12 - Run Orchestrator and Agent Kernel` | Added detached-run/reconnect contract (server-side drain, replay-then-live, bounded retention, honest durability scope) and cheap-path discipline. |
| `17 - Workspace Intelligence and Context Packs` | Added foreground activity gate, concrete adaptive input-budget contract, and second-pass compression mechanics (reference-only headings, pruning pre-pass, token-budget tail protection, pre-summarizer redaction). |
| `18 - Model Gateway and Microsoft Foundry` | Added cache-aware auxiliary call routing (full replay on shared cache key vs digest on cold key). |
| `14 - Builder Studio and SkillOps` | Added background skill-learning contracts (complexity trigger, null-biased extraction, isolated review fork, staged writes) and creation-time guard for self-destructive scheduled work. |
| `38 - Worker Images and Command DSL` | Added platform self-lifecycle command class to the denylist, command-shaped (not prose) pattern anchoring, checked at creation and execution. |
| `33 - Release Gates and Acceptance Matrix` | Added QA maturity register requirement (per-surface scored YAML, scenario-driven scores, three QA lanes, gate binding). |
| `90 - LLM-Tailored Development Plan and Agent Workflow` | Added §14.2 "Source-Proven Conventions From AI-Developed Runtimes": repo instruction files, review discipline (evidence maps, best-fix, sibling proof), test policy (behavior contracts, behavioral-first, never silence red), change scoping and footprint ladder, one-canonical-path rules, parallel-work overlap audit, QA maturity register. |
| `51 - Master Implementation Sequence` | Added "Deep-Dive Sequence Reinforcements": detached runs join the slice, maturity register starts Phase 1, agent-policy files land Phase 0, overlap-audited packet dispatch, gated background jobs. |
| `89 - Consolidated AI Workspace Source Review` | Added the "Second-Pass Deep Dive (V6.14)" traceability table mapping each finding to source file and target note. |

---


## V6.13 Source-Verified Consistency Pass (2026-07-09)

## Review method

Full-library automated consistency audit (frontmatter, wikilinks, heading structure, term usage, version claims, event/object naming against the canonical contracts in `34`/`53`), followed by a direct source-code review of the four reviewed apps under `_source_review/` (BMAD-METHOD + bmad-builder, OpenClaw, Hermes, Odysseus) to verify the claims in notes `83`–`89` and capture concrete contracts they had not yet extracted.

## Findings and fixes

| Finding | Severity | Fix applied |
|---|---|---|
| 21 files carried a stray `# <file title>` line embedded inside a code fence (residue of the V6.1 heading demotion), corrupting the rendered code blocks. | Medium | Stray lines removed in all 21 files (`07`–`09`, `20`, `27`, `28`, `30`–`33`, `35`–`41`, `44`–`46`, `49`). |
| `75 - Library Validation Protocol.md`: all four bash audit-command comments had been overwritten with the file title, destroying their meaning. | Medium | Restored descriptive comments for each audit command. |
| `31`: ADR template's title line read `# Architecture Decision Records` instead of a per-ADR title placeholder. | Low | Now `# ADR-NNNN: Short Decision Title`. |
| `71`: story template's title line read `# Backlog Story Template and Ready Rules`. | Low | Now `# Story S-NNN: Short Story Title`. |
| Manifest object naming drift: `CommandResultManifest` (`01`), `ResultManifest` (`11`, `20`) vs canonical `WorkerResultManifest` (`34`). | Medium | Normalized to `WorkerResultManifest` in `01`, `11`, `20`. |
| Event-name drift against `53 - Event Taxonomy and Stream Protocol.md`: `proposal.normalized`, `policy.evaluated`, `approval.decided` (`11`); `model.plan.completed`, `evidence.ready` (`10`); `policy.denied` (`19`); `execution.spec.accepted` (`20`). | Medium | Aligned to canonical types; `19` now states denial is `policy.evaluation.completed` with outcome `denied`. |
| Events used legitimately but missing from the canonical taxonomy: `workspace.checkout.created` (`16`), `trace.event.appended` (`11`, `21`), and validation events (`10` card, slice step 12). | Medium | Added `workspace.checkout.created`, `trace.event.appended`, `validation.started`, `validation.completed` to `53`, plus an explicit "taxonomy is canonical" rule. |

## Source-derived additions (verified against `_source_review/` code)

| Note | Addition | Source evidence |
|---|---|---|
| `69` | Config merge corrected to the source-exact four-layer TOML order with typed merge semantics (scalar override, table deep-merge, keyed-array merge by `code`/`id`, array append). | `BMAD-METHOD src/scripts/resolve_config.py`. |
| `69` | Exact deterministic validator rule IDs (`SKILL-01..07`, `PATH-02`, `STEP-01/06/07`, `SEQ-02`, `TPL-01`), name/step-filename regexes, severity model, `--strict`/`--json` gate semantics. | `BMAD-METHOD tools/validate-skills.js`. |
| `39`, `13`, `83` | Assembled `_bmad/_config/bmad-help.csv` catalog contract, `_meta` documentation rows, Help Advisor runtime data sources (catalog, merged config, artifacts-as-state, module docs), `_bmad/scripts/` resolver contract. | `BMAD-METHOD tools/installer/core/installer.js`, `src/core-skills/bmad-help/SKILL.md`. |
| `19` | Approval Request Payload Contract: two-phase register/wait, separate timeouts, server-side expiry, argv + resolved executable path, shell-wrapper resolution, reviewer highlight spans, unavailable-decision list, provenance and reviewer routing fields. | OpenClaw `src/agents/bash-tools.exec-approval-request.ts`. |
| `23` | Concrete `OutboundUrlPolicy` blocklist: IPv4/IPv6 special-use classes, vendor cloud-metadata sentinels beyond `169.254.169.254`, embedded-IPv4-in-IPv6 decoding, documented per-deployment exemptions, post-DNS and per-redirect evaluation. | OpenClaw `packages/net-policy/src/ip.ts`. |
| `40` | Threat-model document shape: trust-boundary statement, role-by-capability matrix with enforcement pointers, reserved identities as security-critical, known-gaps register as release-gate evidence. | Odysseus `THREAT_MODEL.md`. |

## Verified but unchanged

- Version references to .NET 8/9/11, Node 22/26, TypeScript 6, React 18, Python 3.13 all appear in correct avoid/fallback/watchlist context; no active guidance contradicts the locked baseline.
- All wikilinks resolve; all 95 files have frontmatter; every file has exactly one real H1.
- Hermes and Odysseus review claims in `86`–`88` spot-checked against source layout (context compressor, conversation compression, credential pool; auth/session/atomic-io/log-safety modules) — consistent.

## Residual known issue (carried forward)

The generic implementation-depth template (identical "Implementation-depth contract" block in 43 files, and a `WorkerInvocation` starter sketch stamped into 21 files including non-component notes such as `09` and `46`) remains. It is not wrong, but it dilutes non-component notes. A dedicated de-templating pass should replace the sketch with component-relevant contracts or remove it from reference-only notes.

---


## Review finding

Although the Start Here and file frontmatter identified the library as V6, 47 of the 82 active files still carried a body paragraph or blockquote stating "This file is part of the v3 implementation library," and 30 files nested a full historical review section under its own `# V4 ...` H1 heading — creating two competing H1 titles per file and a direct contradiction between a file's stated version (V6, in frontmatter) and its self-description (V3, in body text). Eleven files also carried active, still-referenced content (port interfaces, command DSL, SQL sketches) under headings literally prefixed `V3`, even though the content was current. Separately, 17 active guide files (`07`–`09`, `36`–`49`) had no YAML frontmatter at all, while their siblings did, leaving them without `status`, `type`, or `review_pass` metadata.

## Fixes applied

| Issue | Fix | Files affected |
|---|---|---|
| Body text claimed "part of the v3 implementation library" | Corrected to "part of the V6 implementation library" | 44 files |
| Competing `# V4 Library Review Notes` / `# V4 Full-Library Review Hardening` H1 headings | Demoted to `## Historical Revision Notes (V3 -> V4)` subsections, correctly nested under each file's single real H1 | 46 files (04 excluded — it is the dedicated changelog and the heading is appropriate there) |
| Active content headings prefixed `V3` (e.g. `## V3 internal port set`) | Stale version prefix removed; content unchanged | 11 files |
| Missing frontmatter | Added standard frontmatter (`project`, `type`, `status: v6-modernized-implementation-guide`, `generated_on`, `review_pass`) matching sibling files | 17 files (`07`–`09`, `36`–`49`) |

## Not changed

- `05 - Preserved Source Context.md`, `06 - Preserved Critical Review.md`, `50 - V4 Full Library Audit.md` remain untouched — they are explicitly preserved historical records, not active guidance, and correctly have no current-version frontmatter.
- No content was deleted. Historical revision notes remain in every file, just correctly nested and labeled as history rather than competing with the file's current V6 identity.

## Residual known issue (not fixed in this pass)

Many component files (`10`–`35`) still contain a second and sometimes third H1 heading beyond the historical-notes section (e.g. `# Detailed Implementation Expansion`, `# Workers write manifests and logs; they do not advance SQL lifecycle state.`) used informally as section breaks rather than true document titles. This is a pre-existing structural pattern across the whole library, separate from the V3/V4 identity problem this pass fixed. Flattening it into a single-H1-per-file structure would touch nearly every component file's heading hierarchy and was judged out of scope for a consistency pass — worth a dedicated formatting pass if Markdown outline cleanliness matters for how these files render or get parsed by tooling.


---


## Review finding

`04 - Review Findings and Changelog.md` is part of the implementation library support layer. In v3, support files were useful but not always testable. In v4, every support file must provide either a decision, reference contract, release gate, mapping, runbook, or checklist that can be executed by a developer or coding agent.

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


## Review result

The v3 library preserved the source context and expanded all components, but still had three weaknesses:

1. It relied too much on repeated implementation-depth template sections.
2. Several files did not make ownership of APIs, events, SQL tables, and Blob prefixes explicit.
3. Release gates existed but were not consistently connected to component-level implementation steps.

## Improvements made

- Added component-specific build cards to core architecture files.
- Added per-component API/port touchpoints.
- Added per-component domain events.
- Added per-component SQL ownership and Blob layout notes.
- Added per-component edge case tests and release gates.
- Added new master implementation sequence and state-machine reference.
- Added Airlock rulebook and worker manifest protocol.
- Added replay fixture library plan and first-slice user stories.
- Added coding-agent handoff prompts.

## Non-negotiable v4 preservation

- Full original source context remains preserved.
- Full critical review remains preserved.
- The first executable vertical slice remains the build priority.
- Airlock remains the only path to side effects.
- Workers remain manifest/log producers, not lifecycle-state owners.
