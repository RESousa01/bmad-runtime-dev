# Reviewed native contract generator (vendored)

The native contract-codegen preflight pins the **exact byte identity** of the
`cargo-typify` generator and its cargo install metadata (see
[`tool-lock.json`](../tool-lock.json) → `tools.rust.executableIdentity` and
`tools.rust.installMetadata`).

Building `cargo-typify` from source with `cargo install` is **not
byte-reproducible across machines**: Rust embeds absolute build paths
(the workspace directory and `CARGO_HOME`) into the binary, so the same
`rustc 1.97.0` toolchain and the same locked dependencies still produce a
binary of a different size on a different host. That is why CI restores this
reviewed prebuilt artifact instead of recompiling it.

## Contents

| File | What it is | Pinned by |
| --- | --- | --- |
| `cargo-typify-0.6.1-x86_64-pc-windows-msvc.exe` | reviewed generator (Git LFS) | `tools.rust.executableIdentity` |
| `cargo-typify-0.6.1-x86_64-pc-windows-msvc.exe.sha256` | raw-byte checksum for pre-trust verification | — |
| `crates2.json` | reviewed `.crates2.json` install metadata | `tools.rust.installMetadata` |
| `crates2.json.sha256` | raw-byte checksum | — |
| `cargo-typify-0.6.1.crate` | reviewed crates.io source archive | `tools.rust.packageSha256` |
| `cargo-typify-0.6.1.crate.sha256` | raw-byte checksum | — |

The `.exe` is stored with Git LFS (see [`.gitattributes`](../../../.gitattributes)).
A fresh checkout must run `git lfs pull` (CI uses `actions/checkout` with
`lfs: true`).

## How CI / local dev uses it

```
node tools/contract-codegen/native-generator.mjs restore
```

restores the vendored files into `target/contract-tools/` at the exact paths the
lock names, and also copies the source archive into
`CARGO_HOME/registry/cache/index.crates.io-6f17d22bba15001f/`, after verifying
all raw-byte checksums. The existing native preflight then validates the
normalized PE identity, the `.crates2.json` content, and the source archive
checksum as before — the security gate is unchanged; only the *source* of the
artifacts moved from "recompiled / freshly fetched" to "reviewed and restored".

`node tools/contract-codegen/native-generator.mjs verify` checks the vendored
artifacts against the lock without touching `target/` or `CARGO_HOME` (safe for
pre-commit / CI lint).

## Re-blessing a new generator version

Binary provenance is a **human review** step — do not automate it.

1. Produce the reviewed binary once on a trusted machine
   (`cargo install --locked --version <v> --root target/contract-tools cargo-typify`).
2. Capture its identity:
   `node tools/contract-codegen/native-generator.mjs identity target/contract-tools/bin/cargo-typify.exe`
3. Update `tool-lock.json` (`executableIdentity` + `installMetadata` + `packageSha256`) and the
   `schema-lock.json` `toolLockSha256`, then copy the new `.exe`, `.crates2.json`, and
   `.crate` files with their `.sha256` files into this directory.
4. `node tools/contract-codegen/native-generator.mjs verify` must pass before commit.
