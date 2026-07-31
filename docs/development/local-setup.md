---
title: "Forge local setup"
doc_type: runbook
status: maintained
owner_role: devex-owner
last_verified: 2026-07-31
review_due: 2026-08-14
source_commit: "5836aea13453acd4ccadce00ad8d992201bd0e3f"
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

| Dependency | Required version or role |
|---|---|
| Windows | WebView2 plus supported Visual Studio C++/Windows SDK tooling |
| Node.js | `24.18.0` |
| pnpm | `11.15.1` |
| Rust | `1.97.0`, MSVC target, Clippy, and rustfmt |
| .NET SDK | `10.0.302` for cross-language qualification |
| Docker Desktop | Optional; only for the local support-plane rehearsal |

Repository pin owners are listed in
[toolchain and verification](../current/toolchain-and-verification.md).

## Verify the shell first

```powershell
node --version
pnpm --version
rustc --version
dotnet --version
pnpm toolchain:check
```

If a version differs, fix PATH or tool selection before installing dependencies or interpreting
test failures. On this Windows workstation, the system Node path has previously shadowed the
pinned pnpm-managed Node binary.

## Install reproducibly

```powershell
pnpm install --frozen-lockfile
pnpm exec tauri --version
```

Do not regenerate lockfiles or install alternate dependency versions to work around a failing
gate.

## Establish a fast baseline

```powershell
pnpm docs:verify
node packages/bmad-foundation/scripts/generate-catalog.mjs --check
node --test packages/bmad-foundation/tests/bmad-capability-catalog.test.mjs
```

These are orientation checks, not whole-product or release proof.

## Run Forge

| Mode | Command | Meaning |
|---|---|---|
| Deterministic native | `pnpm desktop:dev:deterministic` | Local deterministic Help; no provider required |
| Direct-provider development | `pnpm desktop:dev` | Opt-in OpenAI/Azure OpenAI prototype transport |
| Native host plus local support seam | `pnpm desktop:local` | Docker configuration/liveness rehearsal plus native authority host |

Stop the local support seam with `pnpm local:down`.

Provider credentials belong only in **Forge Settings > Providers**. Do not put keys in `.env`,
Compose configuration, frontend settings, fixtures, logs, or documentation.

## Choose the right verification gate

```powershell
pnpm docs:verify
pnpm verify:source
pnpm verify:deferred-full
pnpm desktop:build
```

- Use `docs:verify` for maintained documentation, metadata, generated indexes, source locks, and
  archive changes.
- Use `verify:source` for the broad first-party source path.
- Use `verify:deferred-full` when cross-language and native Rust coverage is required.
- Use `desktop:build` to prove a local application build.

Azure deployment, signing, timestamping, clean-machine installer qualification, and pilot evidence
require separately authorized infrastructure and receipts. None is implied by these local commands.

## Common failure triage

1. Re-run `pnpm toolchain:check` and record the resolved executable versions.
2. Check `git status --short`; distinguish pre-existing changes from your patch.
3. Reproduce with the narrowest owner test or generator check.
4. Read the first causal failure, not only the final aggregate exit code.
5. Expand verification only after the narrow gate is understood.
