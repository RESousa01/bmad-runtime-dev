---
title: "Forge team documentation"
doc_type: current
status: current
owner_role: forge-maintainers
last_verified: 2026-07-31
review_due: 2026-08-14
source_commit: "5836aea13453acd4ccadce00ad8d992201bd0e3f"
source_paths:
  - README.md
  - docs/current
supersedes:
  - docs/archive/legacy-v6/Start Here.md
  - docs/archive/legacy-v6/Vault Map.md
---

# Forge team documentation

This is the single entrypoint for current Forge guidance. Unless a page says otherwise, its claims
describe the committed baseline named in its metadata. Uncommitted worktree changes, plans, and
receipts are not automatically current product truth.

## Start by intent

| If you want to… | Start here |
|---|---|
| Understand what Forge can do now | [Product state](current/product-state.md) |
| Understand scope and operating modes | [Product scope and modes](current/product-scope-and-modes.md) |
| Set up a development machine | [Onboarding](development/onboarding.md) |
| Understand the trust boundary | [Architecture and authority](current/architecture-and-authority.md) |
| Configure model access safely | [Authentication and model access](current/authentication-and-model-access.md) |
| Check what is actually release-ready | [Release readiness](current/release-readiness.md) |
| Choose the next bounded work | [Roadmap and known gaps](current/roadmap-and-known-gaps.md) |
| Make a contribution | [Contribution guide](development/contribution-guide.md) |
| Find a decision | [ADR index](architecture/adr-index.md) |
| Research historical or imported material | [Reference and archive route](reference/README.md) |

## Current truth set

- [Product state](current/product-state.md)
- [Capability status](current/capability-status.md)
- [Architecture and authority](current/architecture-and-authority.md)
- [Authentication and model access](current/authentication-and-model-access.md)
- [Security and trust](current/security-and-trust.md)
- [Toolchain and verification](current/toolchain-and-verification.md)
- [Release readiness](current/release-readiness.md)
- [Roadmap and known gaps](current/roadmap-and-known-gaps.md)

These pages separate implemented behavior from development-only paths, pending work, and external
qualification gates. Do not combine their evidence into a single readiness percentage.

## Working agreement

1. Inspect branch, HEAD, worktree, and dirty state before reading status claims.
2. Treat source, focused tests, and named gates as stronger evidence than prose.
3. Use accepted ADRs for decisions and `docs/current` for maintained summaries.
4. Preserve unrelated changes and do not execute imported reference-source scripts.
5. Update docs when verified behavior, decisions, pins, gates, or operating procedures change.
6. Run `pnpm docs:generate`, `pnpm docs:verify`, and `git diff --check` for maintained-doc changes.

Authority order:

```text
source/tests/gates > accepted ADR > docs/current > runbook > active plan
> dated review > source analysis > archive
```

See [ADR-0011](adr/ADR-0011-documentation-authority-and-reference-boundary.md) and the
[documentation policy](development/documentation-policy.md) for the complete lifecycle rules.
