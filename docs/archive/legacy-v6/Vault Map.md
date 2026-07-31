---
title: "Vault Map"
aliases:
  - "Map of Content"
  - "Library Index"
tags:
  - bmad-runtime
  - vault/home
section: "Vault Home"
order: 1
vault_role: "map-of-content"
project: Sapphirus BMAD Runtime
status: supporting-reference
updated_on: 2026-07-10
---

# Vault Map

Use this map instead of decoding numbered filenames. The filenames remain stable, but the readable link text tells you what each note is for.

## Foundation

| Note | Use it for |
|---|---|
| [[00 - Common Rules and Product Shape|Common Rules and Product Shape]] | Product definition, non-negotiables, side-effect model, and completion standard. |
| [[01 - First Build - Executable Vertical Slice|First Build - Executable Vertical Slice]] | The first slice that proves governed execution works. |
| [[02 - Locked Architecture Decisions|Locked Architecture Decisions]] | Decisions that require ADRs to reverse. |
| [[93 - Split Web and Windows Desktop Architecture Plans|Split Web and Windows Desktop Architecture Plans]] | Current authority for the separate web-managed and Windows-local delivery plans, their shared contracts, risks, roadmaps, and required document changes. |
| [[03 - Repository and Vault Usage|Repository and Vault Usage]] | Repo shape, docs rules, and traceability expectations. |
| [[04 - Review Findings and Changelog|Review Findings and Changelog]] | What changed after review passes. |
| [[09 - Glossary and Naming|Glossary and Naming]] | Canonical terms and naming rules. |

## Source and Research

| Note | Use it for |
|---|---|
| [[05 - Preserved Source Context|Preserved Source Context]] | Original source context archive. |
| [[06 - Preserved Critical Review|Preserved Critical Review]] | Critical review archive. |
| [[07 - Source Coverage Matrix|Source Coverage Matrix]] | Maps source/review topics to implementation notes. |
| [[35 - Source Alignment Notes|Source Alignment Notes]] | Checks that implementation notes still reflect the source. |
| [[50 - V4 Full Library Audit|V4 Full Library Audit]] | Historical audit baseline. |
| [[60 - External Platform References and Verification Sources|External Platform References and Verification Sources]] | Platform/standards claims that affect implementation. |
| [[Technology Baseline Fact Check|Technology Baseline Fact Check]] | Research-driven corrections from the attached fact-check document. |
| [[83 - BMAD Source Code Review - Method and Builder|BMAD Source Code Review - Method and Builder]] | Source-code review of BMAD Method and Builder contracts used to correct the plan files. |
| [[100 - BMAD Method and Builder Deep Comprehension Audit|BMAD Method and Builder Deep Comprehension Audit]] | Current semantic authority for all Method skills/workflow archetypes and Builder authoring/evaluation/productization boundaries. |
| [[84 - OpenClaw Source Review - Comparable Runtime Patterns|OpenClaw Source Review - Comparable Runtime Patterns]] | Comparable runtime patterns from OpenClaw used to improve governance, sandboxing, package, and QA plans. |
| [[85 - OpenClaw Structured Code Review|OpenClaw Structured Code Review]] | Deeper subsystem review of OpenClaw source, including package descriptors, policy layers, protocol schemas, QA maturity, and supply-chain gates. |
| [[86 - Hermes Source Code Review - Agent Runtime and Learning Loop|Hermes Source Code Review - Agent Runtime and Learning Loop]] | Structured review of Hermes agent runtime, prompt caching, approvals, sessions, managed cron, connectors, skill self-improvement, plugin safety, and supply-chain policy. |
| [[87 - Hermes Deep Review - Extension Runtime and Operational Contracts|Hermes Deep Review - Extension Runtime and Operational Contracts]] | Second Hermes pass covering provider resolution, compression, secrets, editor sessions, connector delivery, task claims, auth, and operations. |
| [[88 - Odysseus Source Code Review - Self-Hosted AI Workspace|Odysseus Source Code Review - Self-Hosted AI Workspace]] | Deep review of Odysseus self-hosted workspace patterns: owner scope, loopback tools, SSRF guards, uploads, context budgets, task chains, providers, memory, skills, and operations. |
| [[89 - Consolidated AI Workspace Source Review and Architecture Improvements|Consolidated AI Workspace Source Review and Architecture Improvements]] | Consolidated lessons from BMAD, OpenClaw, Hermes, Odysseus, and verified platform docs for technology, architecture, infrastructure, and release gates. |
| [[91 - Technology, Language, Method, and LLM Implementation Review|Technology, Language, Method, and LLM Implementation Review]] | V6.16 verdict for languages, frameworks, BMAD authority, cloud-first development, Model Gateway/evaluation, and corrected phase gates. |
| [[92 - Source Snapshot Verification and Adoption Ledger|Source Snapshot Verification and Adoption Ledger]] | Exact archive hashes/counts, extraction confidence, symlink and missing-file recovery, component licenses, provenance gaps, and adoption decisions. |

## Delivery Plan

| Note | Use it for |
|---|---|
| [[93 - Split Web and Windows Desktop Architecture Plans|Split Web and Windows Desktop Architecture Plans]] | Choose one delivery model and follow its independent onboarding, architecture, security, execution, and roadmap. |
| [[08 - Phased Roadmap and Build Order|Phased Roadmap and Build Order]] | Staged roadmap. |
| [[30 - Implementation Epics and Backlog|Implementation Epics and Backlog]] | Epics and backlog structure. |
| [[51 - Master Implementation Sequence|Master Implementation Sequence]] | The canonical implementation sequence. |
| [[61 - First Vertical Slice User Stories|First Vertical Slice User Stories]] | User stories for the first slice. |
| [[71 - Backlog Story Template and Ready Rules|Backlog Story Template and Ready Rules]] | Story format and readiness rules. |
| [[72 - Week-by-Week Build Plan|Week-by-Week Build Plan]] | Weekly build plan. |
| [[90 - LLM-Tailored Development Plan and Agent Workflow|LLM-Tailored Development Plan and Agent Workflow]] | Detailed BMAD-first AI-agent workflow, cloud-first phase gates, LLM evaluation, TDD/replay guidance, work packets, and stop rules. |

## Core Runtime

| Note | Use it for |
|---|---|
| [[10 - Chat Workbench|Chat Workbench]] | Chat shell, event cards, panels, approvals, and evidence UI. |
| [[11 - Runtime API Control Plane|Runtime API Control Plane]] | API ownership, lifecycle state, and module boundaries. |
| [[12 - Run Orchestrator and Agent Kernel|Run Orchestrator and Agent Kernel]] | Intent routing, proposal creation, and agent kernel responsibilities. |
| [[13 - BMAD Kernel, Package Loader, and Help Advisor|BMAD Kernel, Package Loader, and Help Advisor]] | BMAD-specific interpretation and package/help behavior. |
| [[14 - Builder Studio and SkillOps|Builder Studio and SkillOps]] | Early inactive Builder authoring plus gated evaluation, promotion, activation, and later SkillOps. |
| [[15 - Artifact Creator and Presentation Adapter|Artifact Creator and Presentation Adapter]] | Presentation workflow adapter decision and artifact creator path. |
| [[16 - Workspace Service|Workspace Service]] | Snapshots, checkouts, preimages, checkpoints, and rollback. |
| [[17 - Workspace Intelligence and Context Packs|Workspace Intelligence and Context Packs]] | Async scanning and context pack production. |
| [[18 - Model Gateway and Microsoft Foundry|Model Gateway and Microsoft Foundry]] | Model abstraction, structured outputs, and Foundry/OpenAI integration. |
| [[19 - Airlock Policy and Approvals|Airlock Policy and Approvals]] | Policy checks, approvals, and side-effect governance. |
| [[20 - Execution Lanes and Container App Jobs|Execution Lanes and Container App Jobs]] | ACA Jobs, workers, dispatch, and manifests. |
| [[21 - Trace, Evidence, and Observability|Trace, Evidence, and Observability]] | Evidence bundles, telemetry, traces, and audit views. |
| [[22 - Data Model - SQL and Blob|Data Model - SQL and Blob]] | SQL lifecycle state and Blob payload division. |
| [[23 - Security, Identity, and Secrets|Security, Identity, and Secrets]] | Identity, secrets, authorization, and hardening. |
| [[24 - Operator Console and Operations|Operator Console and Operations]] | Operator controls, monitoring, and production operations. |
| [[25 - OpenAPI, Schemas, and Generated Clients|OpenAPI, Schemas, and Generated Clients]] | Contract-first APIs and generated clients. |
| [[26 - Frontend Design System|Frontend Design System]] | UI system and frontend implementation baseline. |
| [[27 - Testing, Validation, and Replay|Testing, Validation, and Replay]] | Test strategy and replay fixtures. |
| [[28 - Supply Chain, Deployment, and IaC|Supply Chain, Deployment, and IaC]] | Supply chain, deployment, and infrastructure. |
| [[29 - Concurrency, Transactions, and Failures|Concurrency, Transactions, and Failures]] | Locks, stale proposals, partial failure, and recovery. |

## Architecture Contracts

| Note | Use it for |
|---|---|
| [[93 - Split Web and Windows Desktop Architecture Plans|Split Web and Windows Desktop Architecture Plans]] | Delivery authority boundary and no-mixing rules. |
| [[99 - Dual-Delivery Contract and Conformance Specification|Dual-Delivery Contract and Conformance Specification]] | Normative discriminated wire contracts, canonical hashing, schema evolution, and C#/Rust/TypeScript conformance. |
| [[31 - Architecture Decision Records|Architecture Decision Records]] | Decision records and ADR workflow. |
| [[32 - Integration Contract Map|Integration Contract Map]] | Producer/consumer boundaries. |
| [[33 - Release Gates and Acceptance Matrix|Release Gates and Acceptance Matrix]] | Release gates and acceptance evidence. |
| [[34 - Canonical Object Model|Canonical Object Model]] | Canonical runtime objects. |

## Windows Desktop Runtime

| Note | Use it for |
|---|---|
| [[94 - Windows Desktop Native Host and IPC|Windows Desktop Native Host and IPC]] | Tauri/Rust process authority, module boundaries, narrow renderer IPC, events, and startup/recovery ordering. |
| [[95 - Windows Local Workspace and Execution|Windows Local Workspace and Execution]] | Selected-folder capability, NTFS safety, context, journaled patching, local commands, result manifests, and remote handoff. |
| [[96 - Windows Local State, Evidence, Checkpoint, and Rollback|Windows Local State, Evidence, Checkpoint, and Rollback]] | SQLite/encrypted-CAS authority, evidence hash chain, checkpoints, crash recovery, rollback, retention, and migrations. |
| [[97 - Windows Desktop Security and Trust Model|Windows Desktop Security and Trust Model]] | Trust boundaries, DESK-01 containment tiers, identity/tokens, egress/privacy, supply chain, abuse cases, and release gates. |
| [[98 - Azure Support Plane for Windows Desktop|Azure Support Plane for Windows Desktop]] | Entra/licensing, Model Access, signed packages, opt-in sync/collaboration/telemetry, secrets/configuration, and explicit remote jobs. |

## Build References

| Note | Use it for |
|---|---|
| [[36 - Local Development and DevEx|Local Development and DevEx]] | No-container local toolchains, deterministic fakes, contract/replay tests, and remote-build developer workflow. |
| [[37 - Azure Environments and Deployment Runbooks|Azure Environments and Deployment Runbooks]] | Early Azure foundations, ACR/hosted remote builds, fixed ACA Jobs, environments, and deployment operations. |
| [[38 - Worker Images and Command DSL|Worker Images and Command DSL]] | Web/explicit-remote worker image rules and command DSL; not the local runner. |
| [[39 - BMAD Package Format|BMAD Package Format]] | BMAD package file format. |
| [[40 - Threat Model and Security Tests|Threat Model and Security Tests]] | Threat model and security tests. |
| [[41 - Observability Dashboards and Alerts|Observability Dashboards and Alerts]] | Dashboards, alerting, and signals. |
| [[42 - Migrations, Retention, and Cleanup|Migrations, Retention, and Cleanup]] | Migration, retention, and cleanup rules. |
| [[43 - Product UX Flows and Wireframe Notes|Product UX Flows and Wireframe Notes]] | UX flows and wireframe notes. |
| [[44 - AI Coding Agent Handoff Prompts|AI Coding Agent Handoff Prompts]] | Agent handoff prompts. |
| [[45 - Trace Bundle Schema|Trace Bundle Schema]] | Trace bundle schema. |
| [[46 - API Route Catalog|API Route Catalog]] | Route catalog. |
| [[47 - Database DDL Starter|Database DDL Starter]] | Azure SQL DDL starter; desktop SQLite authority is in note 96. |
| [[48 - Blob Storage Layout|Blob Storage Layout]] | Web and explicit opt-in Blob layout; not ordinary desktop storage. |
| [[49 - Detailed Component Build Checklists|Detailed Component Build Checklists]] | Component implementation checklists. |

## Implementation Assets

| Note | Use it for |
|---|---|
| [[52 - API, Event, Table, and Blob Ownership|API, Event, Table, and Blob Ownership]] | Ownership map across APIs, events, tables, and Blob payloads. |
| [[53 - Event Taxonomy and Stream Protocol|Event Taxonomy and Stream Protocol]] | Event taxonomy and streaming rules. |
| [[54 - State Machine Reference|State Machine Reference]] | State machines. |
| [[55 - Airlock Policy Rulebook|Airlock Policy Rulebook]] | Concrete Airlock policy rules. |
| [[56 - Worker Manifest Protocol|Worker Manifest Protocol]] | Web worker result manifest branch of the shared result union. |
| [[57 - Replay Fixture Library Plan|Replay Fixture Library Plan]] | Replay fixture plan. |
| [[58 - Risk Register and Mitigation Plan|Risk Register and Mitigation Plan]] | Risks and mitigations. |
| [[59 - Definition of Done by Component|Definition of Done by Component]] | Done criteria by component. |
| [[62 - Component Interview Prompts|Component Interview Prompts]] | Prompts for implementation agents. |
| [[63 - Backend Port Interfaces|Backend Port Interfaces]] | Backend port/interface reference. |
| [[64 - JSON Schema Contract Examples|JSON Schema Contract Examples]] | JSON Schema examples. |
| [[65 - SQL Migration and Index Plan|SQL Migration and Index Plan]] | SQL migration and indexing plan. |
| [[66 - Frontend Component Specification|Frontend Component Specification]] | Frontend component specs. |
| [[67 - Azure IaC Module Specification|Azure IaC Module Specification]] | Azure IaC module specs. |
| [[68 - Security Test Case Catalog|Security Test Case Catalog]] | Security test cases. |
| [[69 - BMAD Validation Rules|BMAD Validation Rules]] | BMAD validation rules. |
| [[70 - Presentation Adapter Mapping Workbook|Presentation Adapter Mapping Workbook]] | Presentation adapter mapping. |

## Audit and Validation

| Note | Use it for |
|---|---|
| [[73 - Verification Register|Verification Register]] | Claim classification and validation. |
| [[74 - Targeted Corrections|Targeted Corrections]] | Concrete V6 corrections. |
| [[75 - Library Validation Protocol|Library Validation Protocol]] | Repeatable validation process. |
| [[76 - Current Stack Baseline|Current Stack Baseline]] | Current toolchain and platform baseline. |
| [[77 - Platform Revalidation Register|Platform Revalidation Register]] | What was revalidated and why. |
| [[78 - Deprecation and Preview Watchlist|Deprecation and Preview Watchlist]] | Items to avoid or monitor. |
| [[79 - Corrections Applied|Corrections Applied]] | Corrections applied in the modernization pass. |
| [[80 - Modern Engineering Methods|Modern Engineering Methods]] | Engineering methods that apply across the build. |
| [[81 - Modernization Spike Backlog|Modernization Spike Backlog]] | Required spikes and compatibility checks. |
| [[82 - Current Technology Decision Summary|Current Technology Decision Summary]] | One-page current stack summary. |
| [[Library Quality Report|Library Quality Report]] | Quality report for the library itself. |

## Source Reviews

| Note | Use it for |
|---|---|
| [[83 - BMAD Source Code Review - Method and Builder|BMAD Source Code Review]] | BMAD method and builder source review. |
| [[100 - BMAD Method and Builder Deep Comprehension Audit|BMAD Foundation Semantic Audit]] | Current deep Method/Builder semantic, compatibility, safety, and productization review. |
| [[84 - OpenClaw Source Review - Comparable Runtime Patterns|OpenClaw Comparable Runtime Patterns]] | Comparable runtime patterns from OpenClaw. |
| [[85 - OpenClaw Structured Code Review|OpenClaw Structured Code Review]] | Structured OpenClaw code-review findings. |
| [[86 - Hermes Source Code Review - Agent Runtime and Learning Loop|Hermes Agent Runtime Review]] | Hermes first-pass agent runtime and learning-loop review. |
| [[87 - Hermes Deep Review - Extension Runtime and Operational Contracts|Hermes Extension Runtime Review]] | Hermes second-pass provider, compression, secret, adapter, task-claim, auth, and operations contracts. |
| [[88 - Odysseus Source Code Review - Self-Hosted AI Workspace|Odysseus Self-Hosted Workspace Review]] | Odysseus self-hosted AI workspace review and Sapphirus plan improvements. |
| [[89 - Consolidated AI Workspace Source Review and Architecture Improvements|Consolidated AI Workspace Architecture Review]] | Unified architecture and technology synthesis across reviewed AI apps/workspaces. |
