---
title: "Forge agent development workflow"
doc_type: runbook
status: maintained
owner_role: devex-owner
last_verified: 2026-07-31
review_due: 2026-08-31
source_commit_from: "docs/provenance/knowledge-migration-baseline.json#/implementationWorktree/currentHead"
source_paths:
  - AGENTS.md
  - .codex/config.toml
  - docs/adr/ADR-0011-documentation-authority-and-reference-boundary.md
supersedes:
  - docs/archive/legacy-v6/90 - LLM-Tailored Development Plan and Agent Workflow.md
---

# Forge agent development workflow

## Start

1. Inspect branch/worktree, HEAD, dirty state, and concurrent ownership.
2. Read current repository guidance and the relevant accepted ADR/active plan.
3. Identify task dependencies, owned files, protected files, exact gates, and rollback.
4. Resolve the pinned toolchain before running checks.

## Work

- Use one integration owner for README, package scripts, generated indexes, and metadata manifests.
- Do not run concurrent agents against shared authority files.
- Keep source/tests stronger than docs.
- Preserve unrelated user changes.
- Use generated output only through its owner generator.
- Do not execute vendor/reference scripts.
- Keep plans, candidates, Docker rehearsals, and mock evidence in their correct evidence class.

## Verify

Run the narrowest relevant test first. Record exact command, tool version, exit code, and
limitation. Expand to broader source/native gates only when warranted. Do not describe a gate as
green from old output or another tree.

## Report

State:

- what changed;
- what exact evidence passed or failed;
- which readiness classes were not exercised;
- external blockers;
- remaining TODOs;
- durable context updated.

Plans and agent reports do not prove completion. Only current source plus required acceptance
receipts can advance a task.
