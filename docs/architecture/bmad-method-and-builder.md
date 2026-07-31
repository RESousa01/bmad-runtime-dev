---
title: "Forge BMAD Method and Builder architecture"
doc_type: reference
status: maintained
owner_role: bmad-owner
last_verified: 2026-07-31
review_due: 2026-08-14
source_commit_from: "docs/provenance/knowledge-migration-baseline.json#/implementationWorktree/currentHead"
source_paths:
  - packages/bmad-foundation/source-manifest.json
  - packages/bmad-foundation/source-accounting.json
  - packages/bmad-foundation/capability-catalog.json
  - crates/desktop-runtime/src/bmad
supersedes:
  - docs/archive/legacy-v6/83 - BMAD Source Code Review - Method and Builder.md
  - docs/archive/legacy-v6/100 - BMAD Method and Builder Deep Comprehension Audit.md
---

# Forge BMAD Method and Builder architecture

## Sealed inputs

The foundation owns exact archives:

- Method `6.10.0`, archive SHA-256
  `2993a38a36a9586d16a62a4eacb3400db25dd1ea594bbfe4682789d7f95ac02b`;
- Builder `2.1.0`, archive SHA-256
  `d3c70744a9875623b01856cc907cf558324bacc920f0d860c36ad2788a4d2852`.

The source manifest closes 896 members. Source accounting classifies every member without
executing imported scripts.

## Method as interface

BMAD capabilities and their menu/agent relationships organize Forge's coding-agent surface. A
capability is not accepted merely because its source is preserved or it has an adapter. It needs
typed plans, authority requirements, deterministic/verified execution, lifecycle/recovery,
production-route proof, generated projections, and durable evidence.

Current source accounting qualifies one primary capability (`bmm:bmad-prd`) and leaves 52 primary
capabilities implementation-pending.

## Builder

Builder source projects 5 primary skills into 9 public actions and 1 internal evaluation action.
All remain implementation-pending. Draft output is inert until install, register, activate,
execute, upgrade, rollback, and evidence lifecycles are accepted.

## Runtime/source separation

Sealed source text is untrusted data. Capability identity is bound host-side. Generated catalogs
describe availability but grant no effect authority. Every capability result that proposes a file
change re-enters governed edits.
