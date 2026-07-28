# Rust runtime verification baseline — RT-02

**Recorded:** 2026-07-28, against `main` after the trust-mode change
**Backlog item:** RT-02, phase 0, depends on RT-01
**Verification the plan names:** `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`,
`cargo test --workspace --all-features`

## Result

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | passes |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | passes |
| `cargo test --workspace --all-features` | **865 tests, 67 suites, 0 failures** |

`cargo metadata --format-version 1` resolves, and the workspace declares 12
members from the root `Cargo.toml` — which is what RT-01 asked for and why it is
struck rather than open. The toolchain is pinned in `rust-toolchain.toml` to
1.97.0 with `clippy` and `rustfmt`.

## Feature matrix

RT-02's second acceptance criterion is that **every supported feature
combination compiles or is explicitly removed**. Until this session none of
them had been checked, because `desktop-app` defaulted to
`direct-model-providers` and CI only ever ran `--all-features`. `--all-features`
compiles all three trust modes at once, which is a shape that never ships.

Removing the default found that **three of the four modes did not compile at
all**. Both causes are recorded in the commit that fixed them: `provider_failure`
was gated on `direct-provider` while `desktop-app` typed a core trait on it, and
`coordinator.rs` imported `OfflineHelpTransport` under a condition narrower than
its own uses.

Current state, each verified with `-D warnings`:

| Configuration | Trust mode (master audit 6.2) | clippy | tests |
|---|---|---|---|
| no features | `local_only` | passes | 232 tests, 17 suites |
| `direct-model-providers` | `development_direct` | passes | — |
| `deterministic-help` | offline installer, canned Help | passes | — |
| `production-support` | `organization_managed` | passes | — |
| `direct-model-providers,deterministic-help` | development, both compiled | passes | — |
| `--all-features` | not a shipping shape | passes | 865 tests, 67 suites |

No combination is removed; all are supported. `.github/workflows/desktop.yml`
now runs each one separately, so a mode that stops compiling fails on its own
line rather than being masked by `--all-features`.

## Where this baseline is still weak

RT-02's first acceptance criterion — **the matrix distinguishes unavailable
environment checks from failures** — is *not* met, and the gap is easy to
demonstrate on this machine.

`pnpm verify:source` is a `&&` chain whose first step is `toolchain:check`,
which asserts the running Node against `.node-version`. This machine runs
24.11.1 against a pinned 24.18.0, so the chain stops at step one and the other
fifteen steps never run. The output is indistinguishable from a real failure:
exit 1, no indication that fourteen checks were skipped rather than passed, and
nothing that says the cause was the environment rather than the code.

The same shape applies to `verify:deferred-full`, whose first step is
`contracts:verify:cross-language`.

Closing this properly means the runner reporting three states per check —
passed, failed, unavailable — and continuing past an unavailable one. That is a
change to the gate's semantics and belongs in QR-02's territory, so it is
recorded here rather than done as a side effect of RT-02.

Until then: when the toolchain does not match the pin, run the steps
individually. All fifteen that can run do pass.
