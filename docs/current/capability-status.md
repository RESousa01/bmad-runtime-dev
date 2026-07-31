---
title: "Forge capability status"
doc_type: current
status: current
owner_role: bmad-owner
last_verified: 2026-07-31
review_due: 2026-08-14
source_commit_from: "docs/provenance/knowledge-migration-baseline.json#/implementationWorktree/currentHead"
source_paths:
  - packages/bmad-foundation/capability-catalog.json
  - packages/bmad-foundation/source-accounting.json
  - .bmad100/run-state.json
  - docs/bmad/BMAD-100-IMPLEMENTATION-STATUS-2026-07-30.md
supersedes:
  - docs/archive/legacy-v1-knowledge/knowledge-base/current/02-capability-matrix.md
---

# Forge capability status

## Canonical product denominator

The generated catalog contains 58 rows:

- 48 Method capabilities;
- 9 public Builder actions;
- 1 internal Builder evaluation action.

The source-accounting acceptance denominator is 53 primary capabilities: 48 Method skills and 5
Builder primary skills. Current generated states are:

| State | Primary capabilities |
|---|---:|
| Qualified, exact, runtime available | 1 |
| Blocked pending conformance / implementation pending | 52 |

The qualified capability is `bmm:bmad-prd`. Its historical aliases route to the same canonical
implementation and do not increase the denominator.

Evidence:
[source accounting](../../packages/bmad-foundation/source-accounting.json) and
[canonical catalog](../../packages/bmad-foundation/capability-catalog.json).

## Accepted

`bmm:bmad-prd` has canonical generated dispatch, interactive/headless modes, Create/Update/Validate
behavior, lifecycle coverage, host preparation, generated UI projection, and recorded independent
review. The fresh catalog generation check and 9 focused catalog tests passed in the
[migration baseline](../reviews/forge-knowledge-baseline-2026-07-31.md).

## Partial candidate work

The dated BMAD status records preserved but unaccepted candidates for:

- `bmad-spec`;
- `bmad-ux`;
- `bmad-architecture`;
- `bmad-check-implementation-readiness`;
- `bmad-deep-recon`;
- `bmad-help`;
- `bmad-review`.

Candidate branches, merged reachability, or green narrow checks do not make these capabilities
accepted. Their listed production-route, output, lifecycle, receipt, and independent-review gaps
remain the boundary.

## Campaign ledger

The 156-task modernization campaign has two dated counts:

- 33 tasks recorded complete in `.bmad100/run-state.json`;
- 34 reviewed/recorded tasks when independently reviewed MTH-08 is included.

The durable run-state still identifies MTH-08 as current and points to an older repository commit.
Therefore neither count is promoted as a live completion percentage. Reconcile the run-state and
fresh final verifier before using it as current campaign authority.

## Builder

Builder has 5 primary skills projected into 9 public and 1 internal action. All are
implementation-pending. Generated drafts cannot install, register, activate, or execute
themselves.

## Current blockers

The stabilization commit fixed the three observed catalog/UI truth inconsistencies: generation is
byte-current, the focused catalog test is green, and only the qualified PRD row is advertised as
runtime available. The remaining blocker is capability implementation/conformance breadth, not
catalog projection drift.
