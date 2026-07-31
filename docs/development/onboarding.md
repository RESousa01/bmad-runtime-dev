---
title: "Forge onboarding"
doc_type: runbook
status: maintained
owner_role: devex-owner
last_verified: 2026-07-31
review_due: 2026-08-14
source_commit: "5836aea13453acd4ccadce00ad8d992201bd0e3f"
source_paths:
  - docs/README.md
  - docs/current
  - package.json
supersedes:
  - docs/archive/legacy-v6/Start Here.md
---

# Forge onboarding

## Before you start

Confirm which evidence you are looking at:

```powershell
git branch --show-current
git rev-parse HEAD
git status --short
```

Committed `HEAD`, an uncommitted candidate, and historical receipts are different evidence strata.
Never use a dirty candidate to silently update claims about the committed baseline.

## 15-minute orientation

Read these in order:

1. [Documentation hub](../README.md) — navigation and authority.
2. [Product state](../current/product-state.md) — proven, partial, and disconnected behavior.
3. [Architecture and authority](../current/architecture-and-authority.md) — what the Rust host owns.
4. [Authentication and model access](../current/authentication-and-model-access.md) — prototype
   providers versus incomplete production composition.
5. [Release readiness](../current/release-readiness.md) — independent acceptance gates.
6. [Roadmap and known gaps](../current/roadmap-and-known-gaps.md) — bounded next work.

You should then be able to explain:

- why the renderer cannot directly read files, run processes, or hold durable credentials;
- why model output grants no authority;
- which BMAD capabilities are qualified versus merely cataloged;
- why Docker, source checks, installers, signing, Azure, and pilot evidence are separate gates;
- why archive and source-analysis material is not current product authority.

## 30-minute environment setup

Follow the [local setup guide](local-setup.md), then run:

```powershell
pnpm toolchain:check
pnpm install --frozen-lockfile
pnpm docs:verify
node packages/bmad-foundation/scripts/generate-catalog.mjs --check
```

Do not diagnose product failures until the toolchain check confirms the pinned Node, pnpm, Rust,
and .NET versions.

## Run one mode

Deterministic local behavior:

```powershell
pnpm desktop:dev:deterministic
```

Development-only direct-provider behavior:

```powershell
pnpm desktop:dev
```

The direct-provider mode is not production authentication evidence. Every request still requires
exact-context review and a fresh one-shot decision.

## First contribution

Use the [contribution guide](contribution-guide.md):

1. Choose one bounded task and identify its authority source.
2. List owned files, protected files, prerequisites, and acceptance checks.
3. Preserve unrelated worktree changes.
4. Run the narrowest failing/passing check while editing.
5. Expand to the required package and repository gates.
6. Update durable docs only for verified behavior or decisions.
7. Inspect the final diff and report every remaining blocker.

Do not consult the legacy archive to decide current behavior. Use it only through the explicit
[reference route](../reference/README.md).
