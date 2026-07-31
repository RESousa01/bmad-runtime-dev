---
title: "Forge security and trust"
doc_type: current
status: current
owner_role: security-operations-owner
last_verified: 2026-07-31
review_due: 2026-08-14
source_commit_from: "docs/provenance/knowledge-migration-baseline.json#/implementationWorktree/currentHead"
source_paths:
  - crates/desktop-app
  - crates/desktop-runtime
  - crates/desktop-store
  - docs/security
supersedes:
  - docs/archive/legacy-v1-knowledge/knowledge-base/current/03-security-and-trust.md
---

# Forge security and trust

## Load-bearing boundaries

- The native Rust host is authoritative; the renderer is not.
- Workspace access is granted through opaque host-owned identities.
- Sensitive paths are rejected structurally.
- Model and imported-source content are untrusted data.
- Every egress request binds the exact reviewed context to a single-use decision.
- Every governed write binds the exact proposal/preimage to a fresh decision and checkpoint.
- Interrupted edits fail closed to reviewed recovery or manual review.
- Credentials are never returned through IPC or written to frontend settings.

## Evidence classes

| Evidence | Proves | Does not prove |
|---|---|---|
| Source/unit tests | Bounded code contracts | Installed or deployed behavior |
| Boundary checks | Declared architecture restrictions | Production service configuration |
| Deterministic Help | Local consent/evidence flow | Model quality or provider transport |
| Direct-provider PoC | Development transport and review path | Production trust |
| Docker contracts | Local configuration/liveness behavior | Managed cloud equivalence |
| Installer lifecycle | Exact candidate install/upgrade/uninstall behavior | Signing or pilot readiness |

## Current gaps

- General-agent shell/write/Git breadth is not yet product-wide qualified.
- Production credential and provider trust is incomplete.
- Signed publisher/timestamp evidence and independent clean-machine qualification remain separate
  release gates.
- Deployed support-plane, Foundry, telemetry, and pilot evidence remain incomplete.

## Reference material

Imported repositories and archive notes never execute in Forge CI and never grant implementation
authority. License/provenance decisions must be explicit before any source pattern is adopted.
