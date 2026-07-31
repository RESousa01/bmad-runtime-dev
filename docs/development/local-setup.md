---
title: "Forge local setup"
doc_type: runbook
status: maintained
owner_role: devex-owner
last_verified: 2026-07-31
review_due: 2026-08-14
source_commit_from: "docs/provenance/knowledge-migration-baseline.json#/implementationWorktree/currentHead"
source_paths:
  - package.json
  - .node-version
  - rust-toolchain.toml
  - global.json
  - crates/desktop-app/tauri.conf.json
supersedes:
  - docs/archive/legacy-v6/36 - Local Development and DevEx.md
---

# Forge local setup

## Requirements

- Windows with WebView2 and a supported Visual Studio C++/Windows SDK toolchain for native builds.
- Node.js `24.18.0`.
- pnpm `11.15.1`.
- Rust `1.97.0` with the `x86_64-pc-windows-msvc` target, Clippy, and rustfmt.
- .NET SDK `10.0.302` for cross-language generator qualification.
- Docker Desktop only for the optional local support-plane rehearsal.

## Install

```powershell
node --version
pnpm --version
rustc --version
dotnet --version
pnpm toolchain:check
pnpm install --frozen-lockfile
pnpm exec tauri --version
```

If the first four versions do not match repository pins, fix PATH/tool selection before
interpreting later failures.

## Fast orientation gates

```powershell
pnpm docs:verify
node packages/bmad-foundation/scripts/generate-catalog.mjs --check
node --test packages/bmad-foundation/tests/bmad-capability-catalog.test.mjs
```

The documentation gate is introduced by this migration. It does not replace product verification.

## Run Forge

Deterministic native mode:

```powershell
pnpm desktop:dev:deterministic
```

Direct-provider development mode:

```powershell
pnpm desktop:dev
```

Local Docker support seam plus native host:

```powershell
pnpm desktop:local
pnpm local:down
```

## Broader gates

```powershell
pnpm verify:source
pnpm verify:deferred-full
pnpm desktop:build
```

`verify:source` is the broad source gate. `verify:deferred-full` adds cross-language and Rust
coverage. Native builds require the Windows/Tauri toolchain. Azure, signing, installer
qualification, and pilot evidence require separately authorized infrastructure and cannot be
reproduced by these commands alone.
