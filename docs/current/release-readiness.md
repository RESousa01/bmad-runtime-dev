---
title: "Forge release readiness"
doc_type: current
status: current
owner_role: security-operations-owner
last_verified: 2026-07-31
review_due: 2026-08-14
source_commit_from: "docs/provenance/knowledge-migration-baseline.json#/implementationWorktree/currentHead"
source_paths:
  - package.json
  - .github/workflows
  - tools/resolve-release-metadata.mjs
  - docs/quality
supersedes:
  - docs/archive/legacy-v1-knowledge/knowledge-base/current/06-verification-and-release-readiness.md
---

# Forge release readiness

Readiness is a set of independent gates, not a percentage.

| Gate | Current evidence | Status |
|---|---|---|
| Source checks | Narrow baseline gates passed; broad `pnpm verify:source` must be rerun after this change | Pending fresh final run |
| Desktop build | Build scripts and prior local evidence exist | Not rerun for this documentation slice |
| Installer build | Offline NSIS lifecycle evidence exists for earlier candidates | Requires exact current candidate |
| Clean-machine lifecycle | Prior exact-source qualification is not current-candidate proof | Outstanding |
| Signing and timestamping | Protected workflow/predicate exists | Outstanding publisher and timestamp evidence |
| Azure/support plane | Local and fake-server contracts exist | Real deployed qualification outstanding |
| Pilot | No current named pilot cohort/evidence receipt | Outstanding |

## Rules

- A source-green commit is not an installer.
- A built installer is not a signed or timestamped release.
- A Docker rehearsal is not deployed Azure evidence.
- A fake-server contract is not a live Foundry project.
- A signed artifact is not a clean-machine lifecycle or pilot result.

## Release inputs that remain explicit

Release metadata, toolchain pins, dependency locks, application/installer hashes, SBOM, build
attestation, signing identity, timestamp, lifecycle qualification, bundled BMAD identity, and
approval evidence must bind to the exact candidate. Drift fails closed.

## Current decision

Forge is not release- or pilot-ready based on the evidence captured for this migration. This page
must be updated only from fresh candidate-specific receipts.
