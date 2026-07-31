---
title: "Start Here - Sapphirus BMAD Runtime Vault"
aliases:
  - "Vault Home"
  - "Start Here"
tags:
  - bmad-runtime
  - vault/home
section: "Vault Home"
order: 0
vault_role: "home"
project: Sapphirus BMAD Runtime
status: supporting-reference
updated_on: 2026-07-16
---

# Start Here - Sapphirus BMAD Runtime Vault

The current, evidence-backed product authority begins at [Current Product State](knowledge-base/current/00-current-product-state.md). Its companion notes separate implemented facts, verified external facts, decisions, plans, worktree candidates, history, and unknowns.

The numbered root notes are preserved legacy or supporting evidence. Their historical plans and architecture proposals remain useful research material, but opening one does not make it current. [note-catalog.json](knowledge-base/evidence/note-catalog.json) records the authority class of every root note, and [claims.json](knowledge-base/evidence/claims.json) records the evidence behind current assertions.

The living knowledge layer is intentionally optional to product builds, tests, runtime, packaging, and CI. It is validated offline and anchored to an explicit repository commit and research cutoff.

## Read First

| Need | Start with |
|---|---|
| Know what is true in the product now | [Current Product State](knowledge-base/current/00-current-product-state.md) |
| Understand claim classes and evidence | [Evidence and Claim Policy](knowledge-base/current/01-evidence-and-claim-policy.md) |
| See delivery decisions and open questions | [Decisions, Risks, and Open Questions](knowledge-base/current/07-decisions-risks-and-open-questions.md) |
| Understand the product shape | [[00 - Common Rules and Product Shape|Common Rules and Product Shape]] |
| Understand the BMAD Method and Builder foundation | [[100 - BMAD Method and Builder Deep Comprehension Audit|BMAD Method and Builder Deep Comprehension Audit]] |
| Choose the web or Windows delivery model | [[93 - Split Web and Windows Desktop Architecture Plans|Split Web and Windows Desktop Architecture Plans]] |
| Implement the Windows native boundary | [[94 - Windows Desktop Native Host and IPC|Windows Desktop Native Host and IPC]], then [[97 - Windows Desktop Security and Trust Model|Windows Desktop Security and Trust Model]] |
| Implement local folders, execution, and recovery | [[95 - Windows Local Workspace and Execution|Windows Local Workspace and Execution]], [[96 - Windows Local State, Evidence, Checkpoint, and Rollback|Windows Local State, Evidence, Checkpoint, and Rollback]] |
| Implement desktop Azure services | [[98 - Azure Support Plane for Windows Desktop|Azure Support Plane for Windows Desktop]] |
| Implement shared wire contracts | [[99 - Dual-Delivery Contract and Conformance Specification|Dual-Delivery Contract and Conformance Specification]] |
| See the first shippable slice | [[01 - First Build - Executable Vertical Slice|First Build - Executable Vertical Slice]] |
| Follow the build order | [[51 - Master Implementation Sequence|Master Implementation Sequence]] |
| Check current technology choices | [[82 - Current Technology Decision Summary|Current Technology Decision Summary]] |
| Read the complete technology/language/LLM verdict | [[91 - Technology, Language, Method, and LLM Implementation Review|Technology, Language, Method, and LLM Implementation Review]] |
| Verify source snapshots and adoption boundaries | [[92 - Source Snapshot Verification and Adoption Ledger|Source Snapshot Verification and Adoption Ledger]] |
| Browse every note by section | [[Vault Map|Vault Map]] |
| See the research-driven tech corrections | [[Technology Baseline Fact Check|Technology Baseline Fact Check]] |

## Reading Paths

| If you are... | Read these notes |
|---|---|
| Planning the web MVP | [[93 - Split Web and Windows Desktop Architecture Plans]], [[01 - First Build - Executable Vertical Slice]], [[08 - Phased Roadmap and Build Order]], [[51 - Master Implementation Sequence]], [[61 - First Vertical Slice User Stories]], [[72 - Week-by-Week Build Plan]] |
| Planning the Windows desktop MVP | [[93 - Split Web and Windows Desktop Architecture Plans]], [[94 - Windows Desktop Native Host and IPC]], [[95 - Windows Local Workspace and Execution]], [[96 - Windows Local State, Evidence, Checkpoint, and Rollback]], [[97 - Windows Desktop Security and Trust Model]], [[98 - Azure Support Plane for Windows Desktop]], [[99 - Dual-Delivery Contract and Conformance Specification]] |
| Implementing with AI coding agents | [[90 - LLM-Tailored Development Plan and Agent Workflow]], [[44 - AI Coding Agent Handoff Prompts]], [[71 - Backlog Story Template and Ready Rules]], [[59 - Definition of Done by Component]] |
| Implementing BMAD Method or Builder | [[100 - BMAD Method and Builder Deep Comprehension Audit]], then [[13 - BMAD Kernel, Package Loader, and Help Advisor]], [[14 - Builder Studio and SkillOps]], [[39 - BMAD Package Format]], and [[69 - BMAD Validation Rules]] |
| Implementing the runtime core | [[10 - Chat Workbench]], [[11 - Runtime API Control Plane]], [[12 - Run Orchestrator and Agent Kernel]], [[16 - Workspace Service]], [[19 - Airlock Policy and Approvals]], [[20 - Execution Lanes and Container App Jobs]], [[21 - Trace, Evidence, and Observability]] |
| Working on contracts and state | [[99 - Dual-Delivery Contract and Conformance Specification]], [[25 - OpenAPI, Schemas, and Generated Clients]], [[32 - Integration Contract Map]], [[34 - Canonical Object Model]], [[52 - API, Event, Table, and Blob Ownership]], [[54 - State Machine Reference]], [[63 - Backend Port Interfaces]] |
| Building frontend | [[10 - Chat Workbench]], [[26 - Frontend Design System]], [[43 - Product UX Flows and Wireframe Notes]], [[66 - Frontend Component Specification]] |
| Building workers | [[20 - Execution Lanes and Container App Jobs]], [[38 - Worker Images and Command DSL]], [[56 - Worker Manifest Protocol]], [[57 - Replay Fixture Library Plan]] |
| Checking security and governance | [[97 - Windows Desktop Security and Trust Model]], [[19 - Airlock Policy and Approvals]], [[23 - Security, Identity, and Secrets]], [[40 - Threat Model and Security Tests]], [[55 - Airlock Policy Rulebook]], [[68 - Security Test Case Catalog]] |
| Validating platform and LLM claims | [[91 - Technology, Language, Method, and LLM Implementation Review]], [[60 - External Platform References and Verification Sources]], [[73 - Verification Register]], [[76 - Current Stack Baseline]], [[77 - Platform Revalidation Register]], [[78 - Deprecation and Preview Watchlist]] |
| Tracing source evidence | [[92 - Source Snapshot Verification and Adoption Ledger]], [[100 - BMAD Method and Builder Deep Comprehension Audit]], [[05 - Preserved Source Context]], [[06 - Preserved Critical Review]], [[07 - Source Coverage Matrix]], [[35 - Source Alignment Notes]], [[50 - V4 Full Library Audit]], [[83 - BMAD Source Code Review - Method and Builder]], [[84 - OpenClaw Source Review - Comparable Runtime Patterns]], [[85 - OpenClaw Structured Code Review]], [[86 - Hermes Source Code Review - Agent Runtime and Learning Loop]], [[87 - Hermes Deep Review - Extension Runtime and Operational Contracts]], [[88 - Odysseus Source Code Review - Self-Hosted AI Workspace]], [[89 - Consolidated AI Workspace Source Review and Architecture Improvements]] |

## Current Delivery Baselines

- Shared foundation: BMAD Method owns prompt-native workflow/artifact/help semantics; BMAD Builder owns governed authoring/evaluation semantics. Builder contracts and inactive drafts arrive early; execution, evaluation, publication, and activation remain gated. Models propose; Airlock/policy governs; delivery-specific executors perform side effects; evidence and rollback remain first-class. See [[100 - BMAD Method and Builder Deep Comprehension Audit]].
- Shared contracts: OpenAPI 3.1.2 + JSON Schema 2020-12, with domain/JSON `deliveryModel` and persistence `delivery_model`; shared conformance fixtures target C#, Rust, and TypeScript.
- Shared AI: Microsoft Foundry/Azure OpenAI behind an evaluated Model Gateway/Model Access API, with app-owned state, hosted tools off by default, structured-output projection, canonical validation, and exact model profiles.
- Web-managed baseline: React/TypeScript browser UI, ASP.NET Core/.NET control plane, Azure SQL/Blob authority, Entra/managed identity/Key Vault, and fixed isolated remote execution (ACA Jobs reference implementation).
- Windows-local baseline: React/TypeScript UI in Tauri/WebView2, Rust local authority, user-selected folder capabilities, SQLite/encrypted local payloads, Rust patch/checkpoint/rollback engine, and an approval-gated Win32 command runner.
- Azure desktop support plane: Entra, licensing, model access, signed shared BMAD packages, optional sync/collaboration, telemetry, secrets, and explicit remote jobs. It is not in the ordinary local edit/test path.
- Neither product requires local Docker, Kubernetes, a self-hosted server, a local model server, or a GPU.

## Non-Negotiables

1. Models propose; they do not write files or run commands directly.
2. Airlock mints an audience-bound, expiring, single-use `ApprovedExecutionSpec` only after policy and any required approval of the exact `ExecutionSpecCandidate` hash; consumption is a separate immutable record.
3. Executors report result manifests/logs/artifacts; the delivery-specific authority validates results and owns lifecycle state plus the durable Evidence Ledger.
4. Commands are structured `argv[]`, not raw shell strings.
5. Web authority uses SQL/Blob; Windows local authority uses SQLite plus an encrypted local payload store. Sync is never a silent authority transfer.
6. Web ordinary effects use isolated remote execution; desktop ordinary effects use the approved local host. Neither may silently fall back to the other.
7. The installed app accesses only user-selected workspace roots through its brokered file API. Child-process containment is a separate D0 proof gate and must not be overclaimed.
8. Desktop multi-file change is a journaled, crash-recoverable batch with per-file atomic replacement where supported; the product does not claim a multi-file atomic transaction.
9. An explicit desktop remote job creates separate `web_managed` work. Its result cannot apply directly and requires a fresh local candidate, approval, checkpoint, and apply.

## Vault Conventions

- Use [[Vault Map]] when you do not know the file number.
- Use aliases in Obsidian quick switcher: search for readable names like `Airlock Policy`, `Stack Baseline`, or `Worker Manifest`.
- Use tags such as `#vault/core-runtime`, `#vault/source-and-research`, and `#vault/audit-and-validation` to browse by section.
- Keep the numbered prefixes unless you also update all links and references.
- When platform facts change, update [[Technology Baseline Fact Check]], [[60 - External Platform References and Verification Sources]], [[76 - Current Stack Baseline]], [[77 - Platform Revalidation Register]], [[78 - Deprecation and Preview Watchlist]], and [[82 - Current Technology Decision Summary]] together.

## Latest Source Review

The delivery-model split in [[93 - Split Web and Windows Desktop Architecture Plans]] is the latest product-architecture decision. It preserves the V6.16 cloud-first result as the web plan and introduces a separate Windows-local authority instead of weakening the web execution model.

The V6.16 decision record is [[91 - Technology, Language, Method, and LLM Implementation Review]]. The archive/extraction/license evidence is [[92 - Source Snapshot Verification and Adoption Ledger]]. Together they supersede earlier partial-extraction assumptions, lock the no-local-Docker cloud-first path, and define the evaluated Model Gateway and corrected phase sequence.

The latest development-plan reinforcement is [[90 - LLM-Tailored Development Plan and Agent Workflow]]. Use it before creating or assigning AI-coded work; it now defines senior review gates, WIP limits, stop conditions, rollback/disable expectations, observability expectations, context ledger rules, story sizing, and exception handling. Section 14.2 adds source-proven conventions from the AI-developed comparable runtimes (repo agent-policy files, evidence-map reviews, premise verification, footprint ladder, behavior-contract tests, overlap-audited parallel dispatch, QA maturity register).

The V6.14 comparable-runtime deep dive added code-verified contracts to [[12 - Run Orchestrator and Agent Kernel]] (detached runs), [[17 - Workspace Intelligence and Context Packs]] (foreground gate, adaptive budgets, compression mechanics), [[18 - Model Gateway and Microsoft Foundry]] (cache-aware auxiliary routing), [[14 - Builder Studio and SkillOps]] (skill-learning loop), [[38 - Worker Images and Command DSL]] (self-lifecycle command class), [[33 - Release Gates and Acceptance Matrix]] (QA maturity register), and [[51 - Master Implementation Sequence]] (sequence reinforcements). Traceability: [[89 - Consolidated AI Workspace Source Review and Architecture Improvements]] → "Second-Pass Deep Dive (V6.14)".

The latest source-driven architecture synthesis is [[89 - Consolidated AI Workspace Source Review and Architecture Improvements]]. Read it when reviewing technology choices, architecture, infrastructure, release gates, package governance, tool availability, model gateway boundaries, execution lanes, source-derived risks, or build order.

The latest comparable-runtime deep review is [[88 - Odysseus Source Code Review - Self-Hosted AI Workspace]]. Read it when working on self-hosted trust boundaries, owner scoping, internal tool loopback, SSRF/DNS-pinned fetches, uploads and documents, adaptive context budgets, task chains, provider probes, memory, skills, degraded-state operations, or fresh-install UX.

The Hermes reviews remain the main references for prompt-cache stability, extension contracts, profile-scoped secrets, editor sessions, connector delivery, task claims, verification evidence, dashboard auth, and drain state: [[86 - Hermes Source Code Review - Agent Runtime and Learning Loop]] and [[87 - Hermes Deep Review - Extension Runtime and Operational Contracts]].
