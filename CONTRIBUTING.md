# Contributing to Forge

Start with the [documentation hub](docs/README.md), then use the full
[contribution guide](docs/development/contribution-guide.md).

## Before editing

```powershell
git branch --show-current
git rev-parse HEAD
git status --short
pnpm toolchain:check
```

Choose one bounded task, identify its source/test authority and accepted ADRs, and preserve every
unrelated worktree change.

## Required workflow

1. Reproduce with the narrowest relevant test or generator check.
2. Make the smallest coherent change within the native authority boundary.
3. Run affected package checks.
4. Run `pnpm docs:generate` and `pnpm docs:verify` when maintained docs change.
5. Run `pnpm verify:source` when first-party source changes.
6. Run `git diff --check` and inspect the final status.

Update current docs only when verified behavior, decisions, pins, gates, or procedures change.
Plans, reviews, audits, commits, and agent reports are not completion proof.

The handoff must list exact verification, failures or unrun external gates, preserved unrelated
changes, and the resulting Git state.
