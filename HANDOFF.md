# Session handoff — 2026-07-28, overnight run

Supersedes the 2026-07-27 daytime handoff; its content is in git history at
`ec10d37f:HANDOFF.md`. The durable rules from it are carried forward below.
Everything that handoff listed as fixed is now committed.

## Ground truth

| | |
|---|---|
| Repo | `C:\Users\rodri\source\bmad-runtime-dev`, branch `main` |
| `main` | `a764745a` — **not pushed**, six commits ahead of `ec10d37f` |
| Working tree | clean apart from one pre-existing untracked reference tree |
| Gate | 13 of 14 steps pass. `toolchain:check` fails on the environment, see below |
| Scope | renderer and docs only — no Rust, .NET, Bicep or lockfile touched |

Committing was done in slices without being asked, because the session was
explicitly autonomous. **Nothing was pushed. Do not push without being asked.**

## The blocker to clear first

`pnpm verify:source` cannot complete on this machine. Its first step is
`toolchain:check`, which asserts the running Node against the pin:

```
AssertionError: Node 24.11.1 is running, but the pinned toolchain requires 24.18.0.
```

This is environmental and predates the session — nothing here changed a
toolchain file. It matters more than it looks: the gate is a `&&` chain, so a
red first step means **none of the other fourteen ever ran**. The same shape as
`verify:deferred-full`, whose first step is `contracts:verify:cross-language`.

Until Node 24.18.0 is installed, verify by running the steps individually. All
thirteen that can run do pass: `ci:policy:test`, `boundaries:test`,
`brand:test`, `support:cloud-contracts:test`, `bmad:foundation:verify`,
`contracts:verify:typescript`, `secrets:check`, `release:test`, `typecheck`,
`lint`, `test`, `verify:boundaries`, `build`.

The Rust gate was not run, and that is a deliberate call rather than an
omission: no `.rs` file was touched this session.

## Done this session

**The v3 master audit was reconciled against HEAD** before any of it was
worked — `docs/reviews/audit-v3-reconciliation-2026-07-28.md` plus a
132-row CSV. The headline is that the audit's most severe finding, F-01,
does not hold here, and it is the dependency root for the whole of phase 0:
five of those seven items are already struck or done. Four Critical findings
do hold. Every row carries a confidence column; 17 were determined by reading
code, 115 carry a path signal only and say so.

**The renderer now has a type scale.** It had 58 distinct font sizes —
`0.56rem`, `0.5625rem`, `0.57rem`, `0.58rem` and so on, a continuum rather
than a scale. `packages/ui/src/tokens.css` now declares primitives, semantic
roles, and a component tier, and 355 declarations were rewritten onto seven
steps. Radii went from seventeen to the three the design specification names.
The uppercase label went from fourteen definitions carrying four weights and
nine letter-spacings to one.

**The composer reaches the native dialog in one press.** The popover is gone.

## Decisions taken, and what they cost

- **Variant 3a is the target, exactly as drawn**, including its 10px uppercase
  label tier. Design specification 3.3 sets an 11px floor and advises against
  shipping the prototype's 10px labels; this was raised and the choice was
  made deliberately. It is confined to the single `--text-label` token, so it
  reverses in one edit. Note that `forge-v2.css:21` already carries a contrast
  rationale for `--color-text-muted` — this codebase does reason about WCAG
  floors, and the two positions should be reconciled at some point.
- **`Add file` keeps folder support and loses the menu.** The agreed shape was
  one dialog accepting both; Windows has no such mode, because
  `IFileOpenDialog` either has `FOS_PICKFOLDERS` set or it does not. Folder
  support survives as a sibling control instead. The constraint is recorded at
  the call site.
- **Control heights were left alone.** 3a draws a 26px compact control where
  the shell uses 30px, but the design specification requires a 28px minimum hit
  target, and there is no evidence about which of 3a's 26px elements are
  interactive. Guessing here trades an accessibility floor for a pixel, so it
  is a question for the next session rather than a silent change.

## Still open

1. **FE-03 was attempted and reverted.** Read the section in the reconciliation
   document before retrying — the premise that reproducing emission order
   preserves behaviour is false, and the retry needs the anonymous layer
   identified and Tailwind preflight's shorthand resets handled.
2. **The four confirmed Critical findings**, none of them frontend: the
   `direct-model-providers` compile default, the empty updater key, the Bicep
   perimeter conflict (already tracked as NET-001), and 228-of-597 BMAD drift.
3. **CI compiles neither trust mode.** No workflow passes `--features`, which
   is what would have caught the compile default mechanically.
4. **F-04 is partially live, not stale** — the reconciliation said stale, and
   the re-read of its seventeen dependents corrected it. `ContextWindow.tsx:19`
   derives its budget from `workspaceReadLimits.contextBytes / 4`
   (`contracts.ts:267` = `256 * 1024`), so the surface labelled **Context**
   shows a workspace read cap as `64K` rather than a model context window,
   with the attached-file count beside it. The prototype hard-coded `32K`; the
   literal moved and got a better name, and the conflation survived. `FE-16` is
   the real work: read model-route data, and label estimates as estimates per
   specification 7.5.

## Verifying renderer work without a screenshot

The Browser pane could not composite frames this session, so screenshots were
unavailable. Computed styles turned out to be the better instrument anyway, and
this is worth reusing:

- `.claude/launch.json` had the wrong filter (`@sapphirus/desktop-ui`; the
  package is `@forge/desktop-ui`). Fixed — it is gitignored.
- Assert the scale rather than eyeball it: walk `document.body.querySelectorAll('*')`,
  skip elements with no client rects, and check every computed `fontSize` and
  `borderTopLeftRadius` against the allowed set. Zero off-scale is a pass.
- For refactors that should change nothing, snapshot ~20 computed properties
  per element into `localStorage` before the change and diff after. **This is
  what caught the cascade-layer regression on 83 elements. All 550 frontend
  tests passed throughout it.**

## Environment facts worth not rediscovering

- Installed Node is **24.11.1**; the pin is **24.18.0**. `toolchain:check` is
  the first gate step and hard-fails on it.
- `pnpm verify:deferred-full` is the real gate. Its **first** step is
  `contracts:verify:cross-language`; if that fails, clippy/tests/build never ran.
- **`target/contract-tools/` does not survive a `target/` clean.** Recovery:
  `node tools/contract-codegen/native-generator.mjs restore` (`verify` first is
  read-only). The vendored `.exe` is Git LFS — a fresh clone needs `git lfs pull`.
- `schema-lock.json` holds two sibling digests, `toolLockSha256` and
  `generationConfigSha256`. Re-recording one without the other leaves the gate red.
- `pnpm desktop:build` uses `--features deterministic-help` → canned replies.
  Use **`pnpm desktop:build:direct`** for a testable artifact. `desktop:build`
  itself is pinned by `check-boundaries.mjs:413`.
- `OPENAI_BASE_URL` is pinned to exactly `api.openai.com` by `validate_base_url`
  (`direct_provider.rs:678`). A local proxy is rejected at startup by design.
- `.env` is inert — no dotenv anywhere.
- In-src test modules use an inner `#![allow(clippy::expect_used)]`;
  `expect_used` and `unwrap_used` are `deny` at the workspace root.
- Setting `CARGO_TARGET_DIR` would stop the per-worktree `target/` sprawl.
- Tailwind 4.3.3 is imported at `styles.css:1` and emits five cascade layers
  (`properties, theme, base, components, utilities`). All app CSS is unlayered
  and therefore beats every one of them. That is load-bearing.

## How to work

**Do not trust a subagent report, and do not trust a tool-result summary.** The
harness once reported "exit code 0" for a gate run that had failed, because the
wrapper ended in `tail`. Capture the exit code before formatting output.

**Mutation-verify every fix.** Break it, confirm a test fails, record the
message, restore. Applied this session to the no-menu assertion, which fails at
`TaskWorkspace.test.tsx:234` when the container is given `role="menu"`.

**A mechanical migration needs a rule-aware pass.** Bucketing font sizes purely
by computed pixel size would have put 83 declarations at 10px, because the
current values cluster below the prototype's floor. Labels reach 10px only if
they actually carry `text-transform: uppercase`, and label *overrides* have to
be found by selector, since a rule can be defined in one file and half-reset in
another that never mentions the property that identifies it.

Spawn subagents with `model: "opus"` and disjoint file scopes. Give each brief:
the file paths, the failure scenario, a worked example of the bug class, and an
explicit instruction to verify before fixing. "Here is one instance, find the
others" produces depth; "be thorough" produces breadth. Require mutation evidence
verbatim, and treat any claim without a recorded failure message as unverified.

Read-only audit agents must be told not to run `cargo build/test/clippy` — the
target-directory lock is contended and they will block the agents doing real work.

Do not commit another agent's work without running the gate yourself.
