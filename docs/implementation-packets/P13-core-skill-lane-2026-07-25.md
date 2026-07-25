# Implementation packet: P13 — the core-skill runtime lane, and the blocker that wasn't

## Authority and intent

- Owning authority: repository maintainer (RodrigoSousa0), directing that the
  forge-v2 worktree is the true version and that the blocking issue be fixed.
- Closes the sixth and last open item from
  [P12](P12-remaining-integration-2026-07-25.md).

## The blocker, correctly diagnosed

P12 reported item 6 as blocked, and gave two reasons. **One was a real bug I
had mislocated, and the other was my own reasoning error.**

### The real bug: a lock check that could never pass

Chasing `CONTRACT_LOCK_BOOTSTRAP_UNREVIEWED` to its source turned up two
things:

1. I had been reading `packages/contracts/schema-lock.json`, which is
   *generated output*. The reviewed source is
   `tools/contract-codegen/tool-lock.json`, and it is duplicated in
   `expectedBootstrapLocks` inside `native-codegen.mjs` — deliberately, so a
   lock cannot be re-blessed by editing data alone.
2. The two locks genuinely were stale, recording pre-cutover hashes for
   `pnpm-lock.yaml` and `Cargo.lock`.

Fixed by performing the review the checkpoint exists to demand, recorded in
[the supply-chain review](../reviews/supply-chain-review-2026-07-25.md):

- **78 crates added, 0 removed** — all gitoxide (`gix`) and its transitive
  tree, serving one reviewed feature: read-only Git status and diff. It
  *reduces* process-execution surface, since it replaces shelling out to `git`.
- **All 78 licences inside `deny.toml`'s allowlist**, verified crate by crate
  against the local registry. `portable-atomic` and `portable-atomic-util` are
  never extracted for this target, so their exact pinned versions were fetched
  to confirm `Apache-2.0 OR MIT` rather than asserted from memory.
- **Every dependency from crates.io** — `Cargo.lock` contains exactly one
  distinct `source` value across all 647 external packages; no git or alternate
  registries, all checksum-pinned.
- **Zero new npm packages** — the `pnpm-lock.yaml` diff adds only the
  `apps/desktop-ui-v2` workspace member, naming versions already in the lock.

Residual risk stated in the review: `cargo-deny` is not installed here, so the
RUSTSEC **advisory** scan was not run and remains CI's job.

### My reasoning error

P12 argued that giving core skills `bmm:` capability ids would be "false"
because their upstream module is `core`, and that a `core:` prefix therefore
required a contract-schema change and full cross-language regeneration.

That was wrong, and the repository already showed it: **`bmad-brainstorming` is
a `core-skills` skill upstream** (`src/core-skills/bmad-brainstorming/`) and has
been recorded as **`bmm:bmad-brainstorming`** in the reviewed closure ledger
since ADR-0005. The `bmm:` prefix is Forge's *product* capability namespace for
the BMAD Method package; the module a capability came from is recorded in the
package descriptor's `modules` array, not in the id.

So item 6 needed no schema change, no codegen, and no lock update at all. The
lock fix is still a genuine repair — codegen was unrunnable for anyone who
needed it — but it was never what stood in the way.

## What landed

### Nine core skills, adopted verbatim and runnable

`CAPABILITY_TABLE` grows 29 → 38. Each new binding points its
`projection_path` directly at the **adopted upstream `SKILL.md`** under
`source/method/6.10.0/src/core-skills/…` — no authored projection, which is the
whole point of ADR-0008.

| Capability | Archetype |
|---|---|
| `bmm:bmad-spec` | governed change set |
| `bmm:bmad-shard-doc` | governed change set |
| `bmm:bmad-advanced-elicitation` | document artifact |
| `bmm:bmad-review-adversarial-general` | document artifact |
| `bmm:bmad-review-edge-case-hunter` | document artifact |
| `bmm:bmad-review-verification-gap` | document artifact |
| `bmm:bmad-editorial-review-prose` | document artifact |
| `bmm:bmad-editorial-review-structure` | document artifact |
| `bmm:bmad-index-docs` | document artifact |

Enablement is the adopted-source binding itself: core skills have no agent menu
path, because upstream invokes them from *inside* other skills.

### The adopted-instruction boundary

Adopting a body verbatim means the model receives upstream text that says
things like `uv run {project-root}/_bmad/scripts/memlog.py`. None of that is
available, and ADR-0003's executable exclusion still stands.

Rather than rewrite the body — which is exactly what lost the method's wording
in the first place — the host states the boundary **alongside** it, as one more
reviewed context item injected ahead of every adopted instruction: no scripts,
no file reads or writes, no subagents, no web; never claim to have run
something; record a missing input as an open question; everything else in the
instruction applies in full.

That is the `adapt` layer ADR-0008 anticipated, applied uniformly and once,
instead of per-skill paraphrase.

## Verification record

| Check | Result |
|---|---|
| `cargo test --workspace --all-features` | **665 passed**, 0 failed |
| — including the capability matrix | **38 capabilities** complete the generic lifecycle end to end |
| `cargo clippy … -- -D warnings` / `cargo fmt --check` | Clean |
| Renderer `vitest run` | **453 passed** / 43 files |
| `pnpm bmad:foundation:verify` | 116/116 — closure ledger now 38 records |
| `pnpm contracts:verify:typescript` | 85 passed |
| `pnpm secrets:check` | Clean across 3,924 files |
| `pnpm verify:boundaries` / `pnpm typecheck` | Green / clean |

## Still not done

- **Generator qualification cannot run in this environment.** With the locks
  fixed it now gets past the bootstrap check and stops at the next gate:
  `Missing repo-local target/contract-tools/bin/cargo-typify.exe`. The pinned
  generator toolchain is not installed here, and it demands a byte-identical
  executable (`pe-coff-debug-normalized-sha256-v1`). Provisioning that is a
  separate reproducible-build exercise. Nothing in this work needed it.
- **Core skills have no dedicated renderer surface yet.** They are runnable
  capabilities the host will prepare and complete, but the library panel lists
  capabilities by agent menu, and these have none. A "Core skills" section is
  the natural next slice.
- The memlog is still a projection rather than the storage every run appends
  to; chat retention/offboarding is unaddressed; production transport, Azure,
  installer, and pilot evidence are unchanged.
