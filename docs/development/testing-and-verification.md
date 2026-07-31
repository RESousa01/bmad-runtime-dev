---
title: "Forge testing and verification"
doc_type: runbook
status: maintained
owner_role: devex-owner
last_verified: 2026-07-31
review_due: 2026-08-31
source_commit_from: "docs/provenance/knowledge-migration-baseline.json#/implementationWorktree/currentHead"
source_paths:
  - package.json
  - .github/workflows
  - docs/current/release-readiness.md
supersedes: []
---

# Forge testing and verification

## Evidence ladder

1. Generator/check mode.
2. Focused unit or contract test.
3. Package typecheck, lint, and tests.
4. `pnpm docs:verify` for knowledge-system changes.
5. `pnpm verify:source` for the broad product source gate.
6. `pnpm verify:deferred-full` for cross-language/Rust breadth.
7. Exact native build and installer lifecycle.
8. Signing/timestamp and clean-machine qualification.
9. Deployed cloud/Foundry qualification and telemetry.
10. Named pilot evidence.

Passing one rung does not imply a later rung.

## Documentation gate

`pnpm docs:verify` runs deterministic unit tests, generated-file checks, maintained Markdown and
authority checks, offline claim/reference verification, and archive verification. External
freshness checks run separately and may not silently rewrite source or docs.

## Product gates

`pnpm verify:source` covers the product source pipeline defined in `package.json`. Use focused
commands while iterating; use the broad gate before final handoff for source-affecting work.

Rust and Windows process launch may be slow. Do not convert quiet output into a timeout without
checking the active process. Record PATH/toolchain drift separately from source failures.

## Failure handling

- Preserve the first exact failure.
- Determine whether it is source, generated drift, toolchain, environment, or external
  prerequisite.
- Do not weaken pins or gates to turn red output green.
- Re-run the smallest reproducer after repair, then the required broader gate.
