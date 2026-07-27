# Master Audit v3 — reconciliation against HEAD

**Date:** 2026-07-28
**Reconciled against:** `main` @ `ec10d37f`
**Subject:** `sapphirus-master-audit-v3` (audit dated 2026-07-27), 26 findings and a
132-item backlog
**Machine-readable result:** `audit-v3-reconciliation.csv`

The audit arrived as three archives: the core plans, the deliverables (same plans
plus a 3.9 MB `file_action_register_v3.csv`), and the UI polish review. This
document records which of its claims survive contact with the repository, which
do not, and what that changes about the order of work.

## Why this was needed

The audit was taken against an **upload of a working directory, not this git
tree**, and a few commits behind `ec10d37f`. Both facts change what its findings
mean.

The snapshot skew is small and measurable: it reports `App.tsx` at 3,497 lines
where HEAD has 3,453, and `SettingsDialog.tsx` at 942 exactly. A ~2% drift on
mechanical counts means the inventory was real, not estimated — which raises
confidence in its measurable claims.

The upload scope is the problem. Whatever was zipped lacked the repository root
and included untracked build output. That single fact invalidates the audit's
most severe finding and, through it, most of its phase 0.

## Headline result: phase 0 largely dissolves

Finding **F-01** (Critical) asserts there is no authoritative monorepo root with
complete workspace manifests and CI topology, and that 7 of 74 synthetic-root
tooling tests failed because root workflows and package metadata were absent.

**F-01 does not hold for this repository.** The root carries `package.json`,
`pnpm-workspace.yaml`, `Cargo.toml`, `global.json`, `rust-toolchain.toml` and
`deny.toml`, alongside 10 workflows in `.github/workflows`.

F-01 is the dependency root for the audit's entire phase 0 — `RT-01`, `CL-01`,
`QR-01` through `QR-05`. Those seven items are what everything else in the plan
waits on, so they were verified individually rather than struck as a group:

| Item | Verdict | Evidence at HEAD |
|---|---|---|
| `RT-01` Restore the Rust workspace and toolchain manifest | **Struck** | Root `Cargo.toml` declares 12 workspace members; `rust-toolchain.toml` pins 1.97.0 with `clippy` and `rustfmt`. |
| `QR-01` Restore the canonical monorepo root and bootstrap | **Struck** | All six root manifests present, plus 10 CI workflows. |
| `QR-04` Remove build outputs, define generated-file policy | **Struck** | `git ls-files`: **0 of 25,013** tracked files match `bin\|obj\|dist\|coverage\|node_modules\|target`. |
| `QR-02` Normalize scripts and one-command verification | **Done** | Root exposes `verify`, `verify:source`, `verify:deferred-full`, plus typecheck/lint/test/build, `contracts:*`, `rust:fmt\|clippy\|test`, `secrets:check`, `release:*`. Residue: no dedicated `audit` entrypoint. |
| `QR-05` Qualify contract generators, remove topology assumptions | **Done** | `packages/contracts/scripts/qualify-generators.mjs` plus `tests/generator-qualification` with .NET and Rust suites, `catalog.json` and invalid-input fixtures. |
| `CL-01` Restore the .NET solution, SDK pin, deterministic restore | **Partial** | `global.json` pins SDK 10.0.302 with `rollForward: disable`; `Directory.Build.props` present. **Residue:** no `.slnx`/`.sln` at root and `Directory.Packages.props` absent, so central package management is not in place. |
| `QR-03` Rebuild CI as a platform and trust-mode matrix | **Partial** | Workflows cover `ubuntu-24.04`, `windows-2025-vs2026` and self-hosted signing runners. **Residue:** zero workflows pass `--features`, so neither trust mode is compiled or tested in CI. |

Five of seven phase-0 items are struck or done. The two survivors are narrow and
specific, and neither blocks the rest of the plan.

The `QR-04` collapse is worth naming as a class of error: the audit's "4,368 of
5,411 app files are generated or build output" measured an unzipped working
directory. Git tracks none of it. Any future audit should be run against
`git ls-files`, or from a clean clone.

## Findings that do hold

Four Critical findings were confirmed by direct inspection:

| Finding | Confirmed at |
|---|---|
| **F-09** — `desktop-app` compiles the development egress path by default | `crates/desktop-app/Cargo.toml`: `default = ["direct-model-providers"]`; `production-support = []` declared but empty and not defaulted. |
| **F-10** — no secure update chain | `crates/desktop-app/tauri.conf.json`: `createUpdaterArtifacts` false, updater `pubkey` empty, `endpoints` empty. |
| **F-16** — Azure perimeter conflict | `infra/desktop-support/main.bicep:358` sets `publicNetworkAccess: 'Enabled'` on a VNet-injected Container Apps environment; `:380` sets `external: true` ingress, alongside five private endpoints. |
| **F-19** — BMAD snapshot drift | `packages/bmad-foundation/source` holds exactly 228 files against 597 upstream. |

**F-16 is already tracked.** `docs/reviews/sapphirus-desktop-support-bicep-issues.md`
carries it as **NET-001** (Open), and NET-002 covers private-endpoint
reachability. Backlog items `CL-21` and `CL-23` are therefore duplicates of an
existing register, not new work. Track them in one place.

## The finding that is stale rather than wrong

**F-04** (Critical) describes the composer wired to `panel: "files"`, selections
stored in a global `contextFiles`, `Context 3` rendered as a file count, and a
hard-coded `32K` budget beside a `256,000` model limit.

None of that matches the renderer: `contextFiles`, `panel: "files"` and `32K`
return zero matches across `apps/desktop-ui/src`. F-04 accurately describes the
**HTML prototype** in the polish review, which is a separate artifact from the
shipped app. `design-qa.md` records the composer rework as already passed.

The distinction matters because F-04 is cited as the driver for nineteen backlog
items across five subsystems. Those items need re-reading against the renderer
before any of them is scheduled.

## Two places where the audit's own numbers mislead

**`FE-11` looks 80% done by path presence and is genuinely half done, but not the
half the percentage implies.** The audit proposes `lib/hostClient/clients/*Client.ts`.
The repo has `lib/hostClient/*Protocol.ts` — nine domain modules (chat, workspace,
changes, git, bmad, capability, projection, workflow, app) totalling ~3,600 lines.
The domain split already happened under a different naming convention. What did
not happen is thinning the facade: `client.ts` is still 2,031 lines and every
domain routes through it. So finding **F-06** still stands, but the remaining work
is extracting call surfaces, not parsers — materially smaller than the item reads.

**`FE-02` and `FE-03` diverge sharply despite both showing 100% path presence.**
There are 52 custom properties and they are already semantic (`--color-canvas`,
`--color-status-pending-soft`), so FE-02 is partial with a real residue: one flat
tier, no primitive layer beneath and no component/state layer above. FE-03 has not
started at all — `@layer` appears **zero times** across all 15 CSS files. Tokens
get adopted because they pay off immediately; cascade ownership does not, because
its payoff only appears when you try to delete something. That asymmetry is how
`styles.css` reached 3,858 lines with four `redesign/` override files beside it.

## Coverage and confidence

Every row in `audit-v3-reconciliation.csv` carries a `confidence` column. Read it
before acting on a row.

| | Items | Meaning |
|---|---:|---|
| `verified` | 17 | Determined by direct inspection this session, evidence recorded per row. |
| `inferred` | 115 | Mechanical signal only: declared paths resolved against the tree. |

The mechanical signal resolves each item's declared `files` globs (normalising the
audit's `desktop-ui/` to this repo's `apps/desktop-ui/`) and reports how many
exist. **Existence is not completion.** A `review` status means the area is built
and its acceptance criteria were not checked; `partial?` means some declared paths
exist. Both are prompts for review, not verdicts.

Frontend items are the exception where absence is informative: they propose
`src/features`, `src/design`, `src/primitives`, `src/shell` and
`lib/hostClient/clients`, and **none of those directories exist**. Structural
frontend work has genuinely not started.

## Decisions taken

Recorded here because they were made in response to this audit and change what
the plan means.

1. **Target variant is 3a, exactly as drawn** — including its 10px uppercase label
   tier. This is a deliberate departure from design spec §3.3, which sets an 11px
   floor and says not to ship the prototype's 10px labels. The trade-off is
   legibility at Windows text scaling and 200% zoom; it was raised and accepted.
2. **`Add file` keeps folder support but loses the menu.** One click opens the
   native dialog in a mode accepting both files and folders. This honours spec
   §7.4's prohibition on a Forge popover preceding the dialog while keeping the
   capability that `TaskWorkspace.tsx:767` ships today. Roughly 14 tests currently
   pin the two-step menu and will need updating.
3. **Backlog reconciliation before implementation** — this document.

## What this changes about sequencing

The audit presents 132 items, 118 marked P0, gated behind a phase 0 that is
mostly already satisfied. The real starting position is better than the plan
assumes.

Genuinely blocking, and none of it frontend:

- **F-09 / `RT-09`, `RT-10`** — the development egress path is the compiled
  default. This is the one finding that can silently mislead about which authority
  served a request.
- **F-10 / `QR-16`** — no signed update chain exists.
- **F-19 / `BM-02`** — 228 of 597 upstream files, and no coverage matrix.
- **`QR-03` residue** — CI compiles neither trust mode explicitly, which is what
  would have caught F-09 mechanically.

Frontend work is not blocked by any of these. `FE-03` (cascade layers) is the
prerequisite for variant 3a landing as a replacement rather than a sixteenth
override file, and it has no dependency on the runtime items above.

## Method

- Path resolution normalises `desktop-ui/` to `apps/desktop-ui/`; globs count as
  present when anything matches; `**` degrades to a directory-existence check.
- The two `.dc.html` prototype documents ship in the polish-review archive, not
  the repository, and are excluded from path scoring.
- Verified rows were determined by reading the named file at HEAD. Where a claim
  was mechanical (file counts, feature flags, config values), the command is
  recorded in the evidence column.
