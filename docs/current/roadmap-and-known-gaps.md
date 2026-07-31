---
title: "Forge roadmap and known gaps"
doc_type: current
status: current
owner_role: forge-maintainers
last_verified: 2026-07-31
review_due: 2026-08-14
source_commit_from: "docs/provenance/knowledge-migration-baseline.json#/implementationWorktree/currentHead"
source_paths:
  - docs/current/product-state.md
  - docs/bmad/BMAD-100-IMPLEMENTATION-STATUS-2026-07-30.md
  - .foundry-connection/run-state.json
  - package.json
supersedes:
  - docs/archive/legacy-v1-knowledge/knowledge-base/current/07-risks-roadmap-and-open-decisions.md
---

# Forge roadmap and known gaps

## Priority 1: Reconcile truth and durable state

- Reconcile `.bmad100/run-state.json` with the accepted PRD evidence and current main commit.
- Keep partial Method branches/candidates explicitly partial.
- Require fresh canonical receipts and independent review before advancing capability states.
- Preserve unrelated in-flight work and active task leases.

Exit evidence: updated run-state, strict final verifier result, and current generated catalog.

## Priority 2: Complete the native agent runtime

- Implement and qualify the host-native tool loop from ADR-0010.
- Close permission, read/write/shell/Git, timeout, output-bound, recovery, and prompt-injection
  behavior.
- Prove that the renderer cannot bypass host authority.

Exit evidence: focused authority tests, lifecycle tests, strict Clippy, and architecture boundary
checks.

## Priority 3: Expand BMAD and Builder faithfully

- Complete the seven preserved partial Method candidates before starting broad new surface area.
- Implement the remaining Method skills against exact upstream outputs and lifecycle semantics.
- Keep Builder inactive until install, activate, execute, upgrade, rollback, and evidence behavior
  is proven.

Exit evidence: per-capability production-route proof, canonical receipts, source accounting,
generated projections, and independent review.

## Priority 4: Close no-Entra production model access

- Resolve real endpoint/deployment inventory and operator inputs.
- Define credential custody, rotation, revocation, attribution, isolation, egress, telemetry, and
  recovery.
- Exercise the actual deployed path; do not substitute fake configuration.

Exit evidence: fresh prerequisite report, deployed staging receipts, negative-path tests, and
operator review.

## Priority 5: Qualify release and pilot

- Run broad source and native gates on the exact candidate.
- Produce and qualify the installer on an independent clean machine.
- Bind SBOM, attestations, publisher, timestamp, and lifecycle evidence.
- Complete deployed support-plane telemetry and a named internal pilot.

Exit evidence: exact candidate scorecard with every gate independently satisfied.

## Knowledge-system gaps

- Historical July 16 archive is unavailable, so its hash is blocked.
- Real CODEOWNERS handles are not yet supplied.
- Approved storage locators for externalized source archives are not yet supplied.
- The final takeover exercise requires a developer who did not author this migration.
