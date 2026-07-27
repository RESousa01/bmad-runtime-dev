# Session handoff — 2026-07-27, overnight autonomous run

Written by the outgoing session when its context ran low. The user is asleep and
has granted full autonomy to keep auditing, fixing and improving until they wake.
Read this top to bottom before doing anything.

## Ground truth

| | |
|---|---|
| Repo | `C:\tmp\forge-v2` — a git **worktree** of `C:\Users\rodri\source\bmad-runtime-dev` |
| Current branch | `wip/defect-fixes-2026-07-27` |
| `main` | `3dfd5478` — **green**, clean, and NOT pushed (`origin/main` is still `9a5bfa36`) |
| Free disk | ~82 GB (was 10.8 GB; 71.6 GB of Rust build cache was reclaimed) |

`main` is the source of truth for the product. The old `main` was deprecated by
the user and fast-forwarded. Never push without being asked.

## The single most important rule

**Do not trust a subagent report. Verify every claim against the code.**

Every subagent report in this session contained at least one thing that did not
survive checking — including two where the conclusion was right but the stated
*mechanism* was wrong. The gate output and mutation results are evidence; the
prose is a summary.

Corollary: **mutation-verify every fix**. Break it, confirm a test fails, restore,
record the failure message. This repo has produced several tests that pass whether
or not the code works. If a security test would pass with the protection deleted,
it is worthless.

## What has been fixed and committed (verified)

On `main` (`3dfd5478`) and earlier:
- `glob`/`grep` behind the broker, with structural secret exclusion
- Tool-call model contract (v1 text / v2 tool-bearing schemas)
- Transcript roles `tool_call`/`tool_result` + store migration v15→v16
- Turn-scoped egress accounting (`TurnEgressGrant`)
- The host-driven tool loop, made reachable (it was sealing v1 and never telling
  the model tools existed)
- Nonce-fenced context provenance + persona/content separation
- `list` tool no longer leaks sensitive filenames to the model
- Turn-atomic persistence (`append_chat_turn`) — a partial turn used to wedge a
  conversation permanently with no delete anywhere able to free it
- Three `match`-scrutinee self-deadlocks removed; `significant_drop_in_scrutinee`
  now denied at the workspace root
- Per-conversation `in_flight`, per-workspace capability runs
- Task rename / close / delete

On `wip/defect-fixes-2026-07-27` (`7c5ef70c`), verified, 484/484 renderer tests:
- `\p{C}` → `\p{Cc}` + bidi denylist (a ZWJ emoji in a folder name was a
  permanent, unrecoverable boot brick)
- `decisionLog` accepted (the host always sends it; the final step of an 8-step
  architecture run was being discarded)
- `summary` added to `focusableSelector` (governed-edit approval was unreachable
  by keyboard at ≤1240px)
- `clearAllLanes` generation bump (a reply from a revoked workspace could land in
  another workspace's transcript)
- `hasUnpairedSurrogate` fixed — the narrowing above **opened** this hole, which
  the over-broad pattern had been accidentally covering

## Known-unfixed defects, ranked

1. **`sk-` has no word boundary** — `crates/desktop-egress/src/preparation.rs`
   ~:82, ~:303-331. `risk-assessment` → `ri[REDACTED:openai_token]`. Corrupts
   ordinary prose in every lane, and raises a FALSE secret finding that trains
   reviewers to ignore the real ones. An agent was mid-fix; verify its state.
2. **Post-redaction hash given to the model, on-disk hash used to judge it** —
   `desktop-cloud/src/model.rs` ~:155 vs `desktop-app/src/bmad_governed_proposal.rs`
   ~:81-111. Makes governed change sets permanently un-proposable on any file
   containing a redactable pattern, with the false message "The workspace changed
   after the model saw it." Defect 1 makes this fire on everyday documents.
3. **Change-set ceiling mismatch** — archetype accepts 128
   (`desktop-runtime/src/bmad/capability.rs` ~:29), proposal path enforces 20
   (`domain.rs` ~:11). `bmad-shard-doc` fails AFTER a paid model call.
4. **A gate asserting against dead code** — `tools/check-boundaries.mjs`
   ~:1208-1218 asserts an update-blocking invariant against `crates/desktop-update`,
   which appears in ZERO `.rs` files. Real enforcement is `desktop-app/src/update.rs`
   ~:83-96, ungated. False assurance.
5. **Rate limiter untested + wall-clock window** — `desktop-ipc/src/gate.rs`.
   `RateLimited` is never asserted anywhere; the one test claiming the property
   passes if you delete the whole rate-limit block. An NTP step backwards appends
   an out-of-order timestamp that blocks all eviction → session permanently
   rate-limited until restart.
6. **`revoke_workspace` invalidates almost nothing** — `commands.rs` ~:2859 only
   touches `bmad_model`. Not `chat`, `bmad_capabilities`, `bmad_architecture`, and
   never clears `tool_grants`. Contradicts its own ADR-0002 comments.
7. **Authority held across a provider round trip** — `submit_bmad_capability_run`
   ~:2005 and `approve_bmad_capability_run`. `send_chat_message` was fixed; copy
   that shape.
8. **`resume_turn` maps expired and exhausted both to `Conflict`** — an expired
   turn tells the user "A reply is already in flight."
9. **Default-features clippy fails** — dead-code warnings in
   `bmad_model/coordinator.rs`. `pnpm rust:clippy` uses `--all-features` and hides it.
10. **Sync `host_dispatch` on the UI thread** — a turn is up to 16 blocking Azure
    round trips inside the WebView2 event handler. The window freezes and Windows
    marks the app Not Responding. Needs the async migration (see
    `docs/design/consent-exclusivity-without-a-guard.md` for the correct order).
11. **Governed-write TOCTOU** — `governed_io.rs` CAS-checks an identity through a
    handle then applies by path (`fs::rename`, `fs::remove_file`). The recovery
    path does it correctly (pinned handles, `SetFileInformationByHandle`); the
    ordinary path does not.
12. **No retention anywhere** — `MAX_SESSION_MESSAGES` prunes memory only. Every
    table is append-only, `verify_integrity()` decrypts EVERY payload on every
    boot, and one unreadable payload puts the whole app into read-only recovery.

## Structural lessons — these caused the defects above

- `pnpm lint` in `apps/desktop-ui` is `tsc --noEmit` under another name. **There is
  no ESLint in this repo.** `react-hooks/exhaustive-deps`, `rules-of-hooks` and
  `jsx-a11y` have never run on ~13k lines of React. Adding real linting would
  prevent a whole class of what was found.
- A gate that asserts a *string appears in a file* rather than that *behaviour
  holds* is worse than no gate. Three instances found (`desktop-update`, the lint
  script, a v16 migration test that never ran the migration).
- The command catalog is pinned across **five** sites, not four —
  `tools/check-boundaries.mjs` holds its own copy.
- Comments in this codebase are provably false in several places. Read code.

## Environment facts worth not rediscovering

- `pnpm verify:deferred-full` is the real gate. `verify:source` does NOT run
  `rust:test` or `rust:clippy`.
- `pnpm desktop:build` uses `--features deterministic-help` → canned replies, no
  real provider. Use **`pnpm desktop:build:direct`** (added this session) for a
  testable artifact. `desktop:build` itself is pinned by `check-boundaries.mjs:413`
  and cannot be changed.
- No docker stack is needed. A default build has no support-plane dependency —
  verified four ways. `ProductionHelpTransport` is never constructed.
- `OPENAI_BASE_URL` is pinned to exactly `api.openai.com` by `validate_base_url`.
  A local model proxy (LiteLLM/Ollama) is rejected at startup by design.
- `.env` is inert — no dotenv anywhere. Set the key in the shell, or better, in
  the app's Settings → Providers form (zeroizing memory, never touches disk).
- `du -sh` is extremely slow here. Use PowerShell `Measure-Object` scoped to one
  directory.
- Seven `target/` trees exist because Cargo does not share them across worktrees.
  Setting `CARGO_TARGET_DIR` would stop this recurring.

## Sequencing for the installer (user asked for it last)

1. All fixes landed and `pnpm verify:deferred-full` green.
2. Commit in coherent slices, fast-forward `main` from
   `C:\Users\rodri\source\bmad-runtime-dev` with `git merge --ff-only`.
3. **Then** `pnpm desktop:build:direct`. Not before — an installer built from a
   half-applied security fix is worse than none, because it looks finished.
4. Disk is no longer a blocker (~82 GB free).

## How to work

Spawn subagents with `model: "opus"` and disjoint file scopes — concurrent edits
to one crate produce unverifiable results. Give each brief: the file paths, the
failure scenario, a worked example of the bug class, and an explicit instruction
to verify before fixing. "Here is one instance, find the others" produces depth;
"be thorough" produces breadth.

Do not commit another agent's work without running the gate yourself.
