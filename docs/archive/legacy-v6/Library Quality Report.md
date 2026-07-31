---
title: "Library Quality Report"
aliases:
  - "Library Quality Report"
tags:
  - bmad-runtime
  - vault/vault-operations
section: "Vault Operations"
order: 999
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: v6-quality-report
status: supporting-reference
generated_on: 2026-07-16
patch: living-knowledge-v1
---



# Library Quality Report

## 2026-07-16 living knowledge authority

- Added a current authority layer under `knowledge-base/current`, with every substantive assertion linked to a closed claim registry.
- Added source, claim, note-catalog, and repository-pin registries under `knowledge-base/evidence`; external facts use official primary sources and repository facts use implementation plus test/check evidence.
- Reclassified misleading root `status: current` notes as legacy, source, or supporting evidence. The preserved critical review remains explicitly current as source context, not product authority.
- Added deterministic root and living manifests plus offline validation for catalog coverage, evidence depth, pin drift, authority location, and stale file sets.
- Product builds, tests, runtime, packaging, and CI remain independent of this optional reference vault.

## V6.18 BMAD foundation semantic audit

- Added [[100 - BMAD Method and Builder Deep Comprehension Audit]] after a complete semantic pass over all 47 Method skill entrypoints and all five live Builder skills plus their invoked references/scripts/templates.
- Corrected source authority, execution archetypes, composite install inventory, config graphs, help confidence, validation profiles, Builder capability drift, eval boundaries, candidate lifecycle, and memory/autonomy prerequisites.
- Reconciled the active plan around early inactive Builder drafts and later governed execution/evaluation/promotion without changing the V6.17 dual-delivery authority.
- The Markdown structure, links, manifest hashes, and library metrics must be regenerated and validated after this pass; source release promotion still requires immutable upstream ref/signature and component-license decisions.

## V6.17 dual-delivery architecture migration

- Promoted [[93 - Split Web and Windows Desktop Architecture Plans]] as current authority for separate `web_managed` and `windows_local` products.
- Added six implementation-depth authorities: [[94 - Windows Desktop Native Host and IPC]], [[95 - Windows Local Workspace and Execution]], [[96 - Windows Local State, Evidence, Checkpoint, and Rollback]], [[97 - Windows Desktop Security and Trust Model]], [[98 - Azure Support Plane for Windows Desktop]], and [[99 - Dual-Delivery Contract and Conformance Specification]].
- Updated the active foundation, runtime, build-reference, implementation-asset, validation, stack, roadmap, and AI-workflow notes with explicit shared/web/desktop applicability; preserved historical source/audit documents with supersession notices.
- Corrected authority-critical language: immutable project discriminator, `sealed_test_fake`, journaled multi-file recovery rather than atomicity, Job Objects not a filesystem/network sandbox, Azure desktop support plane not local authority, and remote output requiring fresh local approval.
- Added independent C#/Rust/TypeScript conformance, desktop threat/release gates, SQLite/encrypted-CAS recovery, selected-folder capability, narrow IPC, context-egress, signing/update, sync-replica, and remote-handoff contracts.
- The manifest, link graph, JSON examples, fences, ADR IDs, and architecture terminology are revalidated after the migration; real product claims still require the implementation evidence in [[73 - Verification Register]].

## Scope

V6 was a modernization and validation pass over V5. It intentionally avoids line-count expansion as the success metric.

## V6.16 full-source, technology, LLM, and cloud-first review

- OpenClaw and Hermes now have complete authoritative extraction trees and a precise verification/adoption ledger in [[92 - Source Snapshot Verification and Adoption Ledger]]. OpenClaw regular entries/sizes are complete but a full per-file content-hash pass is not claimed; Hermes regular files passed per-file SHA-256 comparison.
- Full-owner source reviews corrected approval, durability, audit/replay, plugin containment, context, turn/memory, credential, automation, budget, release, and component-license assumptions in notes `84`–`89`.
- [[91 - Technology, Language, Method, and LLM Implementation Review]] consolidates the Context7 plus official-primary-source validation, language boundaries, BMAD authority, cloud-first runtime, and evaluated Model Gateway.
- Canonical contracts now distinguish exact `ExecutionSpecCandidate` approval, single-use `ApprovedExecutionSpec`, `WorkerResultManifest`, authoritative `EvidenceLedgerEvent`, user-facing `EvidenceBundle`, diagnostic `TraceProjection`, and durable attempt/lease/completion/outbox recovery.
- The no-local-Docker/Kubernetes/emulator/model-server requirement is locked. `sealed_test_fake` adapters are non-isolating test doubles; remote ACR/hosted-CI builds and fixed ACA Jobs are the first real web build/execution lane.
- The phase map was reconciled across the foundation, object/state/event, implementation-sequence, weekly-gate, source-synthesis, and LLM-plan notes. Minimum Azure foundations now precede real provider/job consumers.
- New validation still required before implementation promotion: complete Git/ref provenance for source snapshots, component-license decisions, generated-client/toolchain compile, Azure IaC/identity/remote-build smoke, model evaluation/canary, and ACA result/evidence recovery.

## V6.1 patch (consistency pass)

A follow-up pass fixed a library-wide identity inconsistency: 47 files self-described in body text as "part of the v3 implementation library" despite V6 frontmatter, 30 files had a competing second H1 heading for embedded historical notes, 11 files carried stale `V3`-prefixed headings on active content, and 17 files had no frontmatter at all. See `04 - Review Findings and Changelog.md` → "V6.1 Consistency Pass" for full detail. No content was deleted or rewritten; this was a labeling/structure fix only.

## Main improvements

- Added explicit 2026 toolchain baselines: .NET 10 LTS, Node.js 24 LTS, React 19.2 + Vite 8 + React Router 8 SPA, TypeScript 7 application compiler with an isolated TypeScript 6 compiler-API gate, pnpm 11, and per-worker Python/uv profiles.
- Revalidated Azure execution assumptions: ACA Jobs remain v1 baseline; Dynamic Sessions remain a spike/v1.5 candidate.
- Updated AI platform stance: Microsoft Foundry/Azure OpenAI v1 Responses behind Model Gateway with `store=false`, hosted tools off, app-owned state, schema projection plus canonical validation, evaluated exact model profiles, explicit fallback, and rollback.
- Added deprecation/preview watchlist to avoid adopting unstable features as v1 foundations.
- Added modernization spike backlog for Dynamic Sessions, model routing, structured-output schema limits, TypeScript 7 compatibility checks, and Foundry Agent Service.
- Locked a no-container local workflow; Aspire, Docker, Kubernetes, infrastructure emulators, and local model serving are not baseline requirements. Bicep/AVM plus ACR Tasks/hosted CI and fixed ACA Jobs are the cloud source/build/execution path.

## V6.2 Obsidian vault pass

- Added readable titles, aliases, tags, sections, and ordering metadata to the markdown library.
- Added [[Vault Map]] as the main map of content.
- Replaced the old Start Here with a vault home page.
- Added [[Technology Baseline Fact Check]] from the attached research result.
- Updated Obsidian workspace, bookmarks, and app settings so the vault opens on the readable start page.
- Corrected TypeScript/OpenAPI baseline guidance across the library.

## V6.12 senior LLM development reinforcement

- Strengthened [[90 - LLM-Tailored Development Plan and Agent Workflow]] with senior engineering review gates, WIP limits, non-negotiable stop rules, context ledger requirements, story sizing, merge discipline, flake policy, and exception handling.
- Updated [[71 - Backlog Story Template and Ready Rules]], [[59 - Definition of Done by Component]], [[33 - Release Gates and Acceptance Matrix]], [[44 - AI Coding Agent Handoff Prompts]], and [[51 - Master Implementation Sequence]] so rollback/disable paths, observability impact, stop conditions, data ownership, and AI handoff evidence are enforced outside the plan itself.
- Updated [[Start Here]] to direct AI-coded implementation work through the reinforced plan.

## V6.14 comparable-runtime deep dive

- Code-level second pass over Odysseus, OpenClaw, and Hermes runtime modules (not just their docs). New contracts landed in `12` (detached runs), `17` (foreground gate, adaptive budget, compression mechanics), `18` (cache-aware auxiliary routing), `14` (skill-learning loop, scheduled-work guard), `38` (self-lifecycle command class), `33` (QA maturity register), `51` (sequence reinforcements), and `90` §14.2 (source-proven AI-agent development conventions).
- Traceability table: `89` → "Second-Pass Deep Dive (V6.14)". Changelog: `04` → "V6.14 Comparable-Runtime Deep Dive".

## V6.13 source-verified consistency pass

- Full automated audit: links, frontmatter, heading structure, version claims, and naming against the canonical contracts (`34`, `53`). Fixed 21 corrupted code fences, four destroyed audit-command comments (`75`), two corrupted template titles (`31`, `71`), manifest-object naming drift (`WorkerResultManifest` in `01`/`11`/`20`), and event-name drift in `10`/`11`/`19`/`20`; extended `53` with `workspace.checkout.created`, `trace.event.appended`, and validation events.
- Re-reviewed the four app sources under `_source_review/` against notes `83`–`89` and folded in newly verified contracts: BMAD deterministic validator rule IDs and four-layer config-merge semantics (`69`), assembled `bmad-help.csv` catalog and Help Advisor data sources (`39`, `13`, `83`), OpenClaw approval-request payload and egress blocklist specifics (`19`, `23`), Odysseus threat-model document shape (`40`).
- Full detail in `04 - Review Findings and Changelog.md` → "V6.13 Source-Verified Consistency Pass".
- Known issue carried forward: the generic implementation-depth template block still pads 43 files; a de-templating pass is recommended but was out of scope here.

## Validation limits

- Preserved original context and review files remain source evidence, not externally fact-checked technical documentation.
- Active implementation files now classify platform facts, architecture decisions, implementation specs, and spike-required items.
- Real validation still requires code: CI, contract tests, policy tests, worker manifest tests, IaC validation, replay fixtures, and threat-model tests.
