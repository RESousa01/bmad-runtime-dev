---
title: "Forge toolchain and verification"
doc_type: current
status: current
owner_role: devex-owner
last_verified: 2026-07-31
review_due: 2026-08-14
source_commit_from: "docs/provenance/knowledge-migration-baseline.json#/implementationWorktree/currentHead"
source_paths:
  - .node-version
  - .nvmrc
  - package.json
  - rust-toolchain.toml
  - global.json
supersedes:
  - docs/archive/legacy-v1-knowledge/knowledge-base/current/05-toolchain-and-dependencies.md
---

# Forge toolchain and verification

## Pinned tools

| Tool | Pin owner | Current pin |
|---|---|---:|
| Node.js | `.node-version`, `.nvmrc`, `package.json` | 24.18.0 |
| pnpm | `package.json` | 11.15.1 |
| Rust | `rust-toolchain.toml` | 1.97.0 |
| .NET SDK | `global.json` | 10.0.302 |
| Vite | `apps/desktop-ui/package.json` | 8.1.5 |
| TypeScript | `apps/desktop-ui/package.json` | 7.0.2 |

Run `pnpm toolchain:check` before interpreting failures. The 2026-07-31 baseline found a default
PATH Node `24.11.1`; the installed pinned binary was used explicitly.

## Setup

```powershell
pnpm install --frozen-lockfile
pnpm exec tauri --version
```

## Named gates

| Gate | Scope |
|---|---|
| `pnpm docs:verify` | Current documentation, metadata, archive, and source-reference integrity |
| `pnpm verify:source` | Product source, contracts, frontend, local/cloud contract tests, BMAD foundation, and build |
| `pnpm verify:deferred-full` | Cross-language, Rust, and broader deferred source verification |
| `pnpm desktop:build` | Deterministic native application build |
| `pnpm verify:bmad:100` | Strict BMAD campaign final verifier |

Documentation verification and product verification are separate required evidence classes.
Neither substitutes for installer, Azure, signing, clean-machine, or pilot qualification.

## Fast narrow checks

Start with the smallest relevant test or generator `--check`, then expand. The current knowledge
baseline records catalog generation, focused catalog tests, workflow resolution, architecture
boundaries, frozen vault integrity, and the expected v1 pin-drift failure separately.
