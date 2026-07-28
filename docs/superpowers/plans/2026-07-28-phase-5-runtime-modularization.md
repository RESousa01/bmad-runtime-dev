# Phase 5 — Modularize the Local Desktop Runtime and Close Renderer Quality

> **For agentic workers:** implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
> Source of truth for scope: `sapphirus-master-audit-v3` §12 phase 5 —
> tasks `FE-27`, `FE-28`, `FE-29`, `FE-30`, `RT-03`, `RT-04`, `RT-05`, `RT-06`, `RT-07`, `RT-08`.

**Goal:** Reduce the desktop host from a handful of very large coordinators to
capability-minimized domain modules with typed ports, and close the renderer's
state/accessibility/performance debt — without changing a single command ID, wire
result, or authority boundary while doing it.

**Exit gate (from the audit):** legacy CSS is deleted; local command/state/chat/tool
boundaries are modular and capability-minimized.

**Baseline revision:** `d947ad2d` (`main`).

---

## Concurrency context — this plan runs alongside an active Phase 3/4 lane

A separate session is executing Phase 3 (`FE-13`..`FE-20`) and Phase 4
(`FE-21`..`FE-26`). At the time of writing it holds uncommitted work in:

```
apps/desktop-ui/src/App.tsx
apps/desktop-ui/src/components/ContextWindow.tsx
apps/desktop-ui/src/components/TaskWorkspace.tsx
apps/desktop-ui/src/components/TaskWorkspace.test.tsx
apps/desktop-ui/src/components/context-window.css
apps/desktop-ui/src/features/composer/{composerEvents,composerMachine,useComposer}.ts
```

This plan is therefore **partitioned by file ownership, not by task number**.

| Lane | Tasks | Owns | Blocked on Phase 3/4? |
|---|---|---|---|
| **R** (runtime) | `RT-03`, `RT-04`, `RT-05`, `RT-06`, `RT-07`, `RT-08` | `crates/**`, `tools/check-tauri-capabilities.mjs` | No |
| **F** (renderer) | `FE-27`, `FE-28`, `FE-29`, `FE-30` | `apps/desktop-ui/src/**`, `packages/ui/src/**` | **Yes** — every one lists a Phase 3/4 task as a dependency |
| **D** (contracts-first docs) | `FE-27`/`FE-28` prerequisites | `docs/design/*.md` (new files only) | No |

Lane F does not start in this plan. Its four tasks cannot be implemented against
surfaces that do not exist yet, and attempting them now would mean continuous
conflict on the six files above. Lane D writes the *governing contracts* those
tasks consume — new files, zero overlap — so lane F becomes executable the moment
Phase 4 lands.

### Cross-lane hazards

1. **The four-site command catalog.** `tools/check-boundaries.mjs:1128-1249`
   regex-extracts `const READY_COMMANDS` from `crates/desktop-app/src/commands.rs`
   and asserts it is same-ordered and byte-identical against `is_known_command`
   (`crates/desktop-ipc/src/envelope.rs`), `desktopHostCommands`
   (`apps/desktop-ui/src/lib/hostClient/contracts.ts`), and the expectation inside
   `commandCatalog.test.ts`. Two of those four sites are lane R's; two are lane F's.
   If Phase 3/4 adds an attachment command, it edits `commands.rs` too.
   **Mitigation:** `READY_COMMANDS` and `RECOVERY_COMMANDS` stay as literal
   `const` arrays at the top of `crates/desktop-app/src/commands.rs`, in the same
   shape and order, for the whole of `RT-03`. A catalog addition from the other
   lane then merges as a one-line insertion.
2. **`check-boundaries.mjs` is a source-text gate, not a behavioral one.** It also
   asserts, by regex against `commands.rs`, that `should_cache_reply` excludes both
   recovery commands, that dispatch reads `allowed_commands: supported_commands(state.boot_mode())`,
   and that recovery dispatch forwards `renderer_session`. Moving those functions
   into `commands/` submodules breaks the gate. The gate must be updated to follow
   the code to its new home **and must not get weaker in the process** — no
   assertion may be deleted, only re-pointed.
3. **No concurrent `pnpm`.** The pnpm store clashes when two invocations run at
   once. Lane R's verification is `cargo` plus bare `node tools/*.mjs`; anything
   needing `pnpm --filter` is serialized through the coordinating session.
4. **One shared checkout — no worktrees.** All lanes run in
   `C:\Users\rodri\source\bmad-runtime-dev` on `main`, alongside the Phase 3/4
   session. This has three consequences that shape the wave order:
   - **Never switch branches.** The other session's checkout is this checkout.
     Commit to `main`.
   - **Never `git add -A` or `git commit -a`.** Stage only your own explicit
     paths, or you will sweep up the other lane's in-flight work. (This already
     happened once: commit `9a5e6d85` swept up this plan document.)
   - **Only one Rust lane at a time.** Two `cargo` invocations serialize on the
     target-dir lock, which is merely slow — but a mid-refactor tree does not
     compile, so a second Rust lane would see failures that are not its own.
     Non-building lanes (documents) may run in parallel with a Rust lane.

---

## Global constraints (inherited — none of these may be relaxed by this plan)

- `desktop-app` remains the sole composition root and local lifecycle authority.
- Adapter crates (`desktop-workspace`, `desktop-store`, `desktop-execution`,
  `desktop-cloud`, `desktop-airlock`) may not depend on each other; bridging trait
  impls live in `desktop-app`. `desktop-runtime` may not depend on adapters.
- Public Tauri exposure stays exactly `host_bootstrap`, `host_dispatch`,
  `host_projection_snapshot`, `host_projection_events`, `host_update`.
- Command IDs and wire results stay backward compatible for the entire phase.
- Clippy runs `--workspace --all-targets --all-features --locked -- -D warnings`
  with pedantic on: every `pub fn` needs `# Errors` docs, `too_many_lines` (100) is
  enforced, `unwrap`/`expect`/`panic` denied outside tests.
- The renderer is untrusted: source must not contain `write_path`,
  `apply_patch_text`, `run_shell`, `read_path`, `spawn`, `execute_sql`.
- D2 context-read and D3 governed-edit epochs remain independent (ADR-0002).
- Model output is data. It never directly writes files or approves its own proposal.

---

## Wave order

One Rust lane at a time, plus any number of non-building document lanes.

```
Wave A  (starts now)
├── R1 = RT-03   split the command router          crates/desktop-app/src/commands{.rs,/}  + tools/check-boundaries.mjs
└── D1 = FE-27/28 governing contracts              docs/design/*.md (new files, no build — safe in parallel)

Wave B  (after R1)
└── R2 = RT-05   minimize Tauri capabilities       capabilities/, permissions/, tauri.conf.json, tools/check-tauri-capabilities.mjs (new)

Wave C  (after R2)
├── R3 = RT-04   IPC envelope / version / replay   crates/desktop-ipc/src/envelope.rs, crates/desktop-app/src/wire.rs, packages/contracts/schemas/*
└── R4 = RT-06   split desktop state               crates/desktop-app/src/state{.rs,/}

Wave D  (after R4)
└── R5 = RT-07   split chat orchestration          crates/desktop-app/src/chat{.rs,/}, desktop-runtime/src/chat.rs, desktop-store/src/chat.rs

Wave E  (after R5)
└── R6 = RT-08   bounded chat tool registry        crates/desktop-app/src/chat_tools.rs, desktop-tools, desktop-execution, desktop-airlock

Lane F  (blocked — unblocks when Phase 4 lands)
└── FE-27 → FE-28 → FE-29 → FE-30
```

`RT-05` is dependency-independent of `RT-03` in the audit's graph and would run
concurrently with it given isolated checkouts; in one shared checkout it is
sequenced instead. `RT-04` and `RT-06` are likewise a logical parallel pair —
both declare only `RT-03`, and they touch disjoint files — but they are run one
after the other for the same reason. `RT-07` depends on `RT-06` and `RT-08` on
`RT-07`, so that tail is inherently serial regardless.

---

## Measured starting state

| File | Lines | Task |
|---|---:|---|
| `crates/desktop-app/src/commands.rs` | 6,443 | `RT-03` |
| `crates/desktop-app/src/recovery.rs` | 2,553 | (out of phase) |
| `crates/desktop-ipc/src/envelope.rs` | 1,994 | `RT-04` |
| `crates/desktop-app/src/chat.rs` | 1,907 | `RT-07` |
| `crates/desktop-app/src/edits.rs` | 1,883 | (out of phase) |
| `crates/desktop-app/src/state.rs` | 1,779 | `RT-06` |
| `crates/desktop-app/src/wire.rs` | 614 | `RT-04` |
| `crates/desktop-app/src/chat_tools.rs` | 303 | `RT-08` |
| `tools/check-boundaries.mjs` | 1,512 | `RT-03` (co-evolve) |

`READY_COMMANDS` currently holds **46** command IDs; `RECOVERY_COMMANDS` holds 2
(`app.get_boot_state`, `workspace.list`).

---

## Task R1 — `RT-03`: Split the desktop command router by domain

**Outcome:** `commands.rs` becomes a small dispatch table plus focused domain
modules. No command ID, permission requirement, or wire result changes.

**Files:**
- Modify: `crates/desktop-app/src/commands.rs` → dispatch table + catalog only
- Create: `crates/desktop-app/src/commands/{mod,app,workspace,chat,changes,runs,bmad,recovery}.rs`
- Modify: `tools/check-boundaries.mjs` (re-point source-text assertions, never weaken)

**Interfaces and invariants:**
- Public Tauri exposure unchanged.
- Domain handlers consume typed state ports and return wire envelopes; no
  handler-to-handler calls — cross-domain work goes through application services.
- `READY_COMMANDS` / `RECOVERY_COMMANDS` stay literal `const` arrays in
  `commands.rs`, same order (see cross-lane hazard 1).

- [ ] **Step 1:** Characterize every current command ID and its permission
      requirement as a test before moving anything.
- [ ] **Step 2:** Move parse/authorize-free dispatch selection into `commands/mod.rs`;
      place domain behavior in focused modules.
- [ ] **Step 3:** Remove handler-to-handler calls.
- [ ] **Step 4:** Add a compile-time command catalog test and unknown-command
      fail-closed behavior.
- [ ] **Step 5:** Re-point every `check-boundaries.mjs` assertion that reads
      `commands.rs` to wherever that code now lives. Prove the gate still fails when
      the invariant is violated — delete an exclusion from `should_cache_reply` and
      confirm red, then restore.
- [ ] **Step 6:** Run verification, inspect full output, attach to evidence.
- [ ] **Step 7:** Commit naming the delivered behavior, not the refactor mechanism.

**Verification:**
```powershell
cargo test -p desktop-app command_catalog
cargo clippy -p desktop-app --all-targets --all-features --locked -- -D warnings
node tools/check-boundaries.mjs
```

**Acceptance criteria:**
- [ ] No domain command module exceeds the agreed responsibility boundary.
- [ ] Command IDs and wire results remain backward compatible.
- [ ] `check-boundaries.mjs` has the same number of assertions as before, all green,
      each demonstrably still able to fail.

---

## Task R2 — `RT-05`: Minimize and continuously verify Tauri capabilities

**Outcome:** The WebView privilege surface is explicit, machine-checked, and cannot
grow without a named threat-model update.

**Files:**
- Modify: `crates/desktop-app/capabilities/main.json`
- Modify: `crates/desktop-app/permissions/autogenerated/*.toml`
- Modify: `crates/desktop-app/tauri.conf.json`
- Modify: `crates/desktop-app/src/main.rs`
- Create: `tools/check-tauri-capabilities.mjs`
- Create: the approved permission manifest this checker diffs against

**Starting state (already narrow — this task certifies and locks it):**
`capabilities/main.json` grants four host permissions plus four
`core:window` permissions, scoped to `windows: ["main"]`, `local: true`.
`tauri.conf.json` sets `withGlobalTauri: false`, `removeUnusedCommands: true`,
`assetProtocol.enable: false`, `freezePrototype: true`, and a strict CSP.
Note `permissions/autogenerated/install_app_update.toml` exists but is **not**
granted in `main.json` — confirm whether that is deliberate and record the answer.

**Do not edit `tools/check-boundaries.mjs`** — R1 owns that file this wave. Put all
new logic in `tools/check-tauri-capabilities.mjs`.

- [ ] **Step 1:** Inventory core/plugin permissions; remove defaults the product
      does not exercise.
- [ ] **Step 2:** Separate main-window, updater, file-dialog, and development
      capabilities where Tauri 2 supports it.
- [ ] **Step 3:** Add the CI diff check against an approved permission manifest.
- [ ] **Step 4:** Test navigation rejection, CSP, the disabled asset-protocol state,
      and plugin command denial.
- [ ] **Step 5:** Write the failing test first for each new guard.
- [ ] **Step 6:** Run verification, inspect full output, attach to evidence.
- [ ] **Step 7:** Commit.

**Verification:**
```powershell
node tools/check-tauri-capabilities.mjs
cargo test -p desktop-app capability
cargo clippy -p desktop-app --all-targets --all-features --locked -- -D warnings
```

**Acceptance criteria:**
- [ ] No generic shell, filesystem, HTTP, or process privilege reaches the renderer.
- [ ] Capability growth fails the checker until the approved manifest is updated.
- [ ] Remote origins have no command access.

---

## Task D1 — `FE-27`/`FE-28` governing contracts

**Outcome:** The three documents `FE-27` and `FE-28` name in their file scope exist
and are specific enough to test against, so lane F is executable on day one.

**Files (all new — no conflict with the active Phase 3/4 lane):**
- Create: `docs/design/state-matrix.md`
- Create: `docs/design/accessibility-contract.md`
- Create: `docs/design/platform-shortcut-contract.md`

**Interfaces and invariants (from the audit):**
- Five states per surface: happy, empty, loading/pending, error/failure,
  edge/constraint.
- Errors name the failed action, preserve user work, and offer a recovery.
- Loading never replaces stable context unnecessarily; empty states present exactly
  one next action.
- Every host error code maps to a user-facing state or a named safe generic fallback.
- Focus destinations are explicit for: panel open/close, task switch, native file
  dialog return, command execution, attachment error, model-context warning, recovery.
- Shortcut *display* and *behavior* both come from the command registry and the
  current platform — Windows labels, not `⌘` glyphs.
- 28px minimum hit target; no information by color alone; no 10px critical text.

- [ ] **Step 1:** Enumerate every product surface and fill its five-state row.
- [ ] **Step 2:** Enumerate host error codes from the Rust `LocalErrorCode` set and
      map each to a state. Flag any without a mapping.
- [ ] **Step 3:** Write the focus-destination table and the announcement-scoping rule.
- [ ] **Step 4:** Write the platform shortcut table, separating resolution from
      presentation.
- [ ] **Step 5:** Cross-check against `01-forge-prototype-design-spec.md` so the
      Add file / Files / Context separation is not re-conflated.

**Acceptance criteria:**
- [ ] Every surface has all five states written, not inferred.
- [ ] Every `LocalErrorCode` variant is either mapped or explicitly listed as
      falling to the generic fallback.
- [ ] No document contains `TODO`, `TBD`, or an undefined interface.

---

## Tasks R3–R6 and lane F

Briefs are written when their wave opens, against the code as it actually stands
after the preceding wave — not speculatively now. Scope and acceptance criteria are
fixed by the audit (`03-desktop-runtime-implementation-plan.md` `RT-04`, `RT-06`,
`RT-07`, `RT-08`; `02-frontend-implementation-plan.md` `FE-27`..`FE-30`).

---

## Phase 5 exit gate

Phase 5 is complete only when all statements below hold on one committed revision:

- [ ] `commands.rs` is a dispatch table; domain behavior lives in `commands/`.
- [ ] Renderer and host reject the same invalid envelope fixtures.
- [ ] Mutation replay cannot duplicate a write or a model call.
- [ ] The Tauri permission surface is diffed against an approved manifest in CI.
- [ ] No command receives whole mutable app state where a narrow port suffices.
- [ ] Projection generation cannot deadlock or leak secrets.
- [ ] Ordinary chat has no dependency on BMAD method services.
- [ ] A cancelled or failed turn has a durable, recoverable terminal state.
- [ ] Every mutation is traceable to an approved execution spec.
- [ ] No legacy override CSS file is loaded (`FE-30`).
- [ ] Zero serious/critical automated accessibility violations in canonical states.
- [ ] Existing D2 Help, D3 governed-edit, recovery, foundation, packaging, and
      release gates remain green.
