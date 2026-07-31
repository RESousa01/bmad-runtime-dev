---
title: "Hermes source analysis"
doc_type: reference
status: blocked
owner_role: architecture-owner
last_verified: 2026-07-31
review_due: 2026-08-31
source_paths:
  - docs/archive/legacy-v6/86 - Hermes Source Code Review - Agent Runtime and Learning Loop.md
  - docs/archive/legacy-v6/87 - Hermes Deep Review - Extension Runtime and Operational Contracts.md
  - docs/archive/library-audits/forge-library-review-evidence.json
supersedes:
  - docs/archive/legacy-v6/86 - Hermes Source Code Review - Agent Runtime and Learning Loop.md
  - docs/archive/legacy-v6/87 - Hermes Deep Review - Extension Runtime and Operational Contracts.md
---

# Hermes source analysis

The integrated snapshot contains 5,873 files, while legacy notes described different/full paths
that are not reproducible as written. No immutable upstream commit and approved storage locator
are currently locked, so detailed source-path claims remain blocked.

Historical reviews found useful patterns around agent runtime organization, extension contracts,
learning/evaluation loops, and operational separation. Forge adopts none of those patterns merely
because they were observed. Any adoption requires a current ADR, first-party implementation, and
tests under the native host authority.

The original reviews are preserved in the archive. They must not be read as current Forge
architecture.
