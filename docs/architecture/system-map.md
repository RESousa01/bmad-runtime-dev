---
title: "Forge system map"
doc_type: reference
status: maintained
owner_role: architecture-owner
last_verified: 2026-07-31
review_due: 2026-08-14
source_commit_from: "docs/provenance/knowledge-migration-baseline.json#/implementationWorktree/currentHead"
source_paths:
  - apps/desktop-ui
  - crates/desktop-app
  - crates/desktop-runtime
  - crates/desktop-cloud
  - packages/bmad-foundation
  - services/desktop-support-api
supersedes:
  - docs/archive/legacy-v6/94 - Windows Desktop Native Host and IPC.md
---

# Forge system map

```text
React/WebView2 renderer
  | typed intents and projections
  v
desktop-app (sole native composition and local authority)
  |-- WorkspaceBroker -> bounded local reads/search and governed files
  |-- local durable store -> grants, decisions, journals, checkpoints, evidence
  |-- desktop-runtime -> BMAD/session/domain contracts
  |-- desktop-cloud -> direct development provider or support transport
  `-- desktop-execution -> governed effect primitives

packages/bmad-foundation
  -> sealed Method/Builder inputs, accounting, catalog, generated projections

desktop-support-api / Azure infrastructure
  -> separate intended production support composition; not local desktop authority
```

## Authority

The renderer presents state and requests actions. It never receives generic device, process,
credential, database, or updater authority. `desktop-app` binds workspace, capability, consent,
provider, and effect identity before calling lower-level crates.

## Current versus intended

- Bounded workspace reads, exact-context review, provider calls, governed edits, rollback, and
  recovery have current source.
- ADR-0009 accepts a general coding-agent direction.
- ADR-0010's complete native tool loop remains proposed/partial.
- Support-plane and Foundry code/contracts exist, but real deployed no-Entra qualification is
  incomplete.

## Evidence

- [Architecture and authority](../current/architecture-and-authority.md)
- [Runtime boundaries](runtime-boundaries.md)
- [Model connectivity](model-connectivity.md)
- [BMAD Method and Builder](bmad-method-and-builder.md)
