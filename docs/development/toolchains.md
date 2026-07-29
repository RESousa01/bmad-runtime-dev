# Verification toolchains

The BMAD verification lane is pinned to the repository toolchain. Verification
must stop before semantic work when the invoked tools do not match these pins:

| Tool | Pin | Authoritative source |
|---|---:|---|
| Node.js | `24.18.0` | `.node-version`, `.nvmrc`, root/package `engines` |
| pnpm | `11.15.1` | root `packageManager` and root/package `engines` |
| Rust | `1.97.0` | `rust-toolchain.toml` and workspace `rust-version` |
| .NET SDK | `10.0.302` | `global.json` |

## Foundation and fixture package contract

The foundation and fixture package roots expose `check`, `lint`, `test`, and
`verify`. Every command first runs the same compatibility preflight:

- the executing Node version must equal the package engine pin;
- pnpm's invoking user-agent version must equal the package engine pin;
- package test sources must not contain focused, skipped, or todo suite markers.

Tests then use Node's supported default process isolation:

```text
node --test ./tests/*.test.mjs
```

The removed `--test-isolation=none` coupling is not part of the supported
package command contract.

## Fresh compatibility baseline

Recorded on 2026-07-28 from the same working tree and pinned Node/pnpm:

| Package | Direct invocation | Package invocation | Result |
|---|---|---|---|
| `@sapphirus/bmad-foundation` | `node --test ./tests/*.test.mjs` | `pnpm --filter @sapphirus/bmad-foundation test` | 130 tests, 130 passed, 0 failed/skipped/todo |
| `@sapphirus/bmad-fixtures` | `node --test ./tests/*.test.mjs` | `pnpm --filter @sapphirus/bmad-fixtures test` | 81 tests, 81 passed, 0 failed/skipped/todo |

The package and direct commands deliberately select the identical glob. The
package form adds only the compatibility preflight, so suite and test totals
must remain equal.

## Root verification smoke

Run these before accepting changes to verification scripts:

```text
pnpm verify:workspace
pnpm --filter @sapphirus/bmad-foundation verify
pnpm --filter @sapphirus/bmad-fixtures verify
```

`verify:workspace` checks the monorepo membership, pins, required root command
surface, and CI reachability. Each package `verify` command checks its preflight,
package verifier, and complete test glob in that order.
