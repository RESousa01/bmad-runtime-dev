---
title: "ADR-0011: Documentation authority and reference boundary"
doc_type: adr
status: accepted
owner_role: forge-maintainers
last_verified: 2026-07-31
review_due: 2026-10-31
source_paths:
  - README.md
  - docs/provenance/knowledge-migration-baseline.json
supersedes:
  - docs/archive/legacy-v6/Start Here.md
  - docs/archive/legacy-v6/Vault Map.md
---

# ADR-0011: Documentation authority and reference boundary

- Status: accepted (2026-07-31)

## Context

Forge has current source and tests, accepted ADRs, implementation packets, dated reviews, a
living-knowledge layer, and 105 numbered legacy notes. Several of those surfaces described
themselves as canonical at the same time. The result was a mixed reference vault whose frozen
integrity could pass while its current product guidance was stale.

## Decision

`docs/README.md` is the single current team documentation entrypoint. The authority order is:

```text
source, tests, and named gates
  > accepted ADR
  > docs/current
  > runbook
  > active plan
  > dated review
  > source analysis
  > archive
```

Plans, reviews, audits, generated agent reports, and archive notes are never implementation proof.
They may identify work or evidence, but current behavior must be rechecked against source or a
named deterministic gate.

Maintained current documents identify an owner role, exact source commit (directly or through the
tracked migration baseline), verification date, review-due date, and relevant source paths.
Documents that cannot meet that contract are classified as plans, reviews, references, or
archives—not current authority.

`bmad-runtime-lib` is reference/archive material. Its original notes and evidence remain valuable,
but no file under that path may override an accepted ADR or `docs/current`. The runtime, build,
installer, and release process remain independent of the documentation/reference system.

## Consequences

- Existing `/docs` content is retained and classified rather than rewritten in one bulk pass.
- Legacy notes are preserved with hashes and migration dispositions, then removed from current
  navigation.
- First-party knowledge tooling moves to `tools/knowledge` and receives normal tests and review.
- Imported source repositories remain non-executable and move outside the normal product tree only
  after immutable identity, license, hash, and recoverable storage are verified.

## Evidence

- [Knowledge migration baseline](../provenance/knowledge-migration-baseline.json)
- [Baseline review](../reviews/forge-knowledge-baseline-2026-07-31.md)
