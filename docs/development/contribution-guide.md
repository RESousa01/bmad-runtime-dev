---
title: "Forge contribution guide"
doc_type: runbook
status: maintained
owner_role: devex-owner
last_verified: 2026-07-31
review_due: 2026-08-31
source_commit_from: "docs/provenance/knowledge-migration-baseline.json#/implementationWorktree/currentHead"
source_paths:
  - CONTRIBUTING.md
  - AGENTS.md
  - package.json
supersedes: []
---

# Forge contribution guide

## Select work

Choose one bounded item from [roadmap and known gaps](../current/roadmap-and-known-gaps.md) or an
accepted task ledger. Record:

- authority source and related ADRs;
- owned files and files that must not change;
- acceptance checks;
- external prerequisites;
- rollback.

Branch, push, release, deployment, and external-message actions require their normal
authorization. Worktree isolation is preferred; if the user explicitly requests `main`, inspect
concurrent ownership before every integration edit.

## Implement

- Preserve unrelated changes.
- Keep effects fail-closed.
- Avoid broad generation; use owner generators and `--check` modes.
- Keep product, docs, archive, and external-source evidence separate.
- Do not execute imported reference-source code.

## Verify

Run in increasing scope:

1. Focused unit/generator check.
2. Affected package tests and typecheck/lint.
3. `pnpm docs:verify` when maintained docs, metadata, pins, source locks, or archive state change.
4. `pnpm verify:source` for source changes.
5. Native, installer, Azure, signing, or pilot gates only when the change and environment require
   them.

Always run `git diff --check` and inspect the final dirty state.

## Document and close

Update `docs/current` when behavior, architecture, provider/auth direction, BMAD state, pins,
verification, release gates, or runbooks change. Archive completed plans and handoffs. Record
limitations and never upgrade a partial capability because code or a receipt merely exists.
