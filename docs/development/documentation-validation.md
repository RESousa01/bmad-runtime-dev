---
title: "Forge documentation validation"
doc_type: runbook
status: maintained
owner_role: devex-owner
last_verified: 2026-07-31
review_due: 2026-08-31
source_commit_from: "docs/provenance/knowledge-migration-baseline.json#/implementationWorktree/currentHead"
source_paths:
  - tools/knowledge
  - docs/_meta
  - package.json
supersedes:
  - docs/archive/legacy-v6/75 - Library Validation Protocol.md
---

# Forge documentation validation

## Fast gate

```powershell
pnpm docs:verify
```

The gate runs first-party unit tests, checks generated pins/indexes, validates maintained Markdown
and authority rules, verifies current claims offline, checks source-lock state, and verifies the
legacy archive manifest.

## Regeneration

```powershell
pnpm docs:generate
pnpm docs:check:generated
```

Pin values are derived from owner manifests. The note index is derived from
`docs/_meta/knowledge-manifest.json`. Do not edit generated JSON by hand.

## Evidence boundaries

- Archive integrity proves preserved bytes, not current semantic truth.
- Current claims require repository, ADR, or allowlisted gate evidence and future review dates.
- External freshness is a separate networked/scheduled job.
- Documentation checks do not qualify product source, installer, Azure, signing, or pilot state.
