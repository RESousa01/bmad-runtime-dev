---
title: "Forge product scope and modes"
doc_type: current
status: current
owner_role: forge-maintainers
last_verified: 2026-07-31
review_due: 2026-08-14
source_commit_from: "docs/provenance/knowledge-migration-baseline.json#/implementationWorktree/currentHead"
source_paths:
  - README.md
  - package.json
  - crates/desktop-app/Cargo.toml
  - infra/local/compose.yaml
supersedes:
  - docs/archive/legacy-v1-knowledge/knowledge-base/current/00-current-product-state.md
---

# Forge product scope and modes

## Deterministic local mode

Command: `pnpm desktop:dev:deterministic`.

This mode exercises the native desktop authority, local workspace operations, deterministic BMAD
Help, review, consent, and governed-edit paths without sending content to a model provider. It is
the strongest offline development path but is not release qualification.

## Direct-provider development PoC

Command: `pnpm desktop:dev`.

This feature build permits direct OpenAI or Azure OpenAI Responses requests. The Rust host holds
the API key in zeroizing memory until disconnect or exit. Every request still requires exact
context review and single-use approval. Receipts from this mode are development evidence, not
production trust evidence.

`pnpm desktop:local` adds the local Docker support seam and active frontend. It does not move the
workspace authority into Docker and does not reproduce managed cloud services.

## Intended no-Entra managed production mode

The intended production composition keeps the signed native host authoritative and uses a
governed support path that does not require Entra. The design must close credential custody,
rotation, revocation, attribution, tenant isolation, bounded egress, telemetry, and recovery.
Those controls are not yet qualified end to end.

See [ADR-0012](../adr/ADR-0012-no-entra-authentication-and-model-access.md).

## Unsupported or deferred

- Browser-only preview is not native authority and cannot receive device, credential, model, or
  governed-edit privileges.
- Docker-only execution is not a desktop substitute.
- Local models and GPU execution are not required or implemented product modes.
- Historical dual-delivery and account-broker designs are not current prerequisites.
- Builder-generated assets remain inert until their install, activate, execute, rollback, and
  evidence lifecycle is accepted.
