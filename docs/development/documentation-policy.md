---
title: "Forge documentation policy"
doc_type: runbook
status: maintained
owner_role: devex-owner
last_verified: 2026-07-31
review_due: 2026-08-31
source_commit_from: "docs/provenance/knowledge-migration-baseline.json#/implementationWorktree/currentHead"
source_paths:
  - docs/adr/ADR-0011-documentation-authority-and-reference-boundary.md
  - docs/_meta
  - tools/knowledge
supersedes:
  - docs/archive/legacy-v6/75 - Library Validation Protocol.md
---

# Forge documentation policy

## Classes and authority

Every maintained document is one of `current`, `adr`, `runbook`, `plan`, `review`, `reference`, or
`archive`. Authority follows [ADR-0011](../adr/ADR-0011-documentation-authority-and-reference-boundary.md).

Current implementation claims require an owner role, exact source commit or baseline pointer,
verification date, review-due date, source paths, and evidence. Plans cannot use `status: current`.
References explain observed external/historical material and cannot prescribe Forge behavior
without current repository evidence.

## Update triggers

Update maintained docs when a change affects:

- product behavior or mode;
- architecture or trust boundary;
- authentication/provider route;
- BMAD or Builder capability state;
- toolchain pin;
- verification/release gate;
- operational procedure;
- source-reference identity.

## Lifecycle

- Current: concise, source-backed, reviewed on its due date.
- Plan: active until its exit gate, then moved to completed/archive.
- Review: dated evidence, never current authority.
- Reference: immutable source identity and adoption/rejection boundary.
- Archive: original hash, archive banner/sidecar, excluded from current-language rules.

## Ownership roles

| Role | Logical ownership |
|---|---|
| `forge-maintainers` | `docs/current`, root README, ADR index |
| `architecture-owner` | `docs/adr`, `docs/architecture` |
| `bmad-owner` | `docs/bmad`, BMAD source analysis |
| `security-operations-owner` | `docs/security`, `docs/operations`, license decisions |
| `devex-owner` | `docs/development`, `tools/knowledge`, docs workflow |

Real repository user/team handles have not been supplied. `.github/CODEOWNERS` must not be created
with example or unresolvable handles; mapping these roles is an explicit merge blocker.

## Cadence

- Per PR: reconcile affected current docs and metadata.
- Weekly: review roadmap/gaps, active plan status, and completed handoffs.
- Monthly: review due dates, source availability/licenses, capability and release state.
- Quarterly or pre-pilot/release: independent docs-vs-source audit, onboarding replay, provenance
  review, and active-plan cleanup.
