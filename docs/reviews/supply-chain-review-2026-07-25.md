# Supply-chain review — forge-v2 bootstrap locks

**Date:** 2026-07-25
**Reviewer:** performed for the repository maintainer, who is the accountable
authority for the resulting lock update.
**Scope:** `Cargo.lock` and `pnpm-lock.yaml` as they stand in the `codex/forge-v2`
worktree, against the reviewed values recorded in
`tools/contract-codegen/tool-lock.json` → `bootstrapLocks`, which are
deliberately duplicated in `expectedBootstrapLocks` inside
`packages/contracts/scripts/lib/native-codegen.mjs` so a lock cannot be
re-blessed by editing data alone. (`packages/contracts/schema-lock.json` is
*generated output* and is not the review target.)

## Why this review exists

The contract generator refuses to run when a bootstrap lock hash differs from
its recorded, reviewed value:

```
CONTRACT_LOCK_BOOTSTRAP_UNREVIEWED: pnpm bootstrap lock hash changed
```

That is deliberate. The recorded hash is a human checkpoint asserting that
somebody looked at the dependency set before code generation — and therefore
before generated bindings enter three languages. Updating the hash without
looking would convert the checkpoint into a rubber stamp, so this document is
the "looking" half.

## What changed, and why

The drift is **pre-existing**, from the forge-v2 cutover, not from contract
work. Both locks were already modified before the current session began.

### Rust — 78 crates added, 0 removed

Every added crate is **gitoxide** (`gix`) or one of its transitive
dependencies. `gix` is the pure-Rust Git implementation used by
`crates/desktop-workspace/src/git.rs` for **read-only** working-tree status and
diff (`gix = { version = "=0.85.0", default-features = false, features = ["sha1", "status"] }`).

That is a coherent, single-purpose addition serving one reviewed feature: the
Git review panel. It replaces shelling out to `git`, which the architecture
boundary forbids — so it *reduces* process-execution surface rather than adding
any.

Notable transitive members and why they appear:

| Crate | Reason |
|---|---|
| `gix-*` (≈45 crates) | gitoxide is deliberately split into small crates; these are its own components |
| `jiff`, `jiff-*` | date/time handling for commit metadata |
| `zlib-rs` | pure-Rust zlib for packfile decompression (replaces a C dependency) |
| `sha1`, `sha1-checked` | Git object hashing, with collision detection |
| `memmap2`, `dashmap`, `arc-swap`, `clru`, `prodash` | object-database caching and progress reporting |
| `defmt`, `heapless`, `hash32`, `portable-atomic*` | conditional `no_std`/embedded paths, pulled in by `jiff`; **not built for `x86_64-pc-windows-msvc`** |

### Licences — all within `deny.toml`'s allowlist

All 78 added crates were checked against the allowlist
(`Apache-2.0`, `BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `MIT`, `MPL-2.0`,
`Unicode-3.0`, `Zlib`). Every one resolves inside it:

- the overwhelming majority are `MIT OR Apache-2.0`
- `zlib-rs` — `Zlib`
- `unicode-bom` — `Apache-2.0`
- `jiff*` — `Unlicense OR MIT` (MIT selected)
- `encoding_rs` — `(Apache-2.0 OR MIT) AND BSD-3-Clause`
- `portable-atomic`, `portable-atomic-util` — `Apache-2.0 OR MIT`, verified by
  fetching the exact checksum-pinned versions, since they are never extracted
  for this target

**Outside the allowlist: none.**

### Provenance — every dependency from crates.io

`Cargo.lock` contains exactly one distinct `source` value:
`registry+https://github.com/rust-lang/crates.io-index`, across all 647
external packages. The remaining 12 entries are this workspace's own crates.

No git dependencies, no alternate registries, no path escapes — satisfying
`deny.toml`'s `unknown-registry = "deny"` and `unknown-git = "deny"`. Every
external crate is pinned by checksum.

### npm — zero new packages

The `pnpm-lock.yaml` diff originally added only the `apps/desktop-ui-v2`
workspace member and its dependency *declarations*. Every version it named
(`react@19.2.7`, `react-dom@19.2.7`, `vite` plugins, `@types/react`) was
already present in the lock for `apps/desktop-ui`.

Diffing the set of resolved package versions before and after yielded an
**empty set of additions**. Nothing new entered the npm supply chain.

**Amended 2026-07-25 (app consolidation).** `apps/desktop-ui-v2` was a
seven-file shell around `apps/desktop-ui`; the two were merged into the single
`@forge/desktop-ui` package at `apps/desktop-ui`, and the extra workspace
member was removed. `pnpm-lock.yaml` is therefore now **byte-identical to the
committed lock at HEAD** (`sha256:28fa1837…`), so the npm side of this review
has no delta left to assess. `Cargo.lock` is unchanged from this review's
conclusion (`sha256:5e1d1eac…`).

**Amended 2026-07-29 (fast-uri advisory).** Production audit identified
`fast-uri@3.1.3` through `@sapphirus/contracts > ajv` as affected by
GHSA-v2hh-gcrm-f6hx (host confusion through a literal backslash authority
delimiter). The workspace now overrides that single transitive package to the
patched `fast-uri@3.1.4`. The reviewed lock diff changes only the override
metadata, the `fast-uri` resolution/integrity, and Ajv's resolved edge; no
other resolved npm package version changed. A fresh `pnpm audit --prod`
reported zero advisories. The reviewed `pnpm-lock.yaml` digest is
`fbfa675b0fab819d51cee9b93361a0a61383610099d7b6ebf6a455e9bf093cf9`.

## Residual risk

- `cargo-deny` is not installed in this environment, so the licence and
  advisory checks were performed by direct inspection of each crate's
  `Cargo.toml` in the local registry rather than by the tool. The licence
  conclusion is therefore hand-verified; the **advisory** (RUSTSEC) check was
  *not* run and remains outstanding for CI, where `deny.toml`'s
  `advisories.yanked = "deny"` applies.
- gitoxide is a large surface for a read-only feature. It is confined to
  `desktop-workspace` behind the existing architecture boundary scan, which
  continues to pass.

## Conclusion

The dependency set is coherent, single-purpose, fully crates.io-sourced,
checksum-pinned, and licence-clean against the repository's own policy. The
bootstrap lock hashes in `tools/contract-codegen/tool-lock.json` and in
`expectedBootstrapLocks` (`packages/contracts/scripts/lib/native-codegen.mjs`)
are updated to match — both sites, since either alone fails the check — with
this document as the review record.
