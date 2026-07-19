# P2 D3 Reviewed Recovery Closure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Complete D3 recovery by keeping interrupted effects quarantined at boot, requiring a fresh renderer-reviewed recovery decision, restoring the exact durable checkpoint through governed workspace IO, and proving unresolved journals remain update-blocking.

**Architecture:** Boot reconciliation remains metadata-only. A new `desktop-app::recovery` composition module authenticates journal/checkpoint rows, derives a pure recovery plan through `desktop-execution`, holds only a short-lived in-memory approval, and consumes that approval before any decision. `changes.recovery.prepare` is observation-only; `changes.recovery.decide` revalidates every authority and observation immediately before a deterministic restore. The renderer receives bounded recovery summaries, never checkpoint content, absolute paths, native handles, or private authority material.

**Tech Stack:** Rust 2021 workspace, Tauri 2, SQLite-backed `desktop-store`, React 19, TypeScript 5, Vitest/Testing Library, Node 24.18.0, pnpm 11.12.0.

## Global Constraints

- Treat `docs/superpowers/specs/2026-07-18-p2-d3-recovery-authorization-design.md` as the normative design.
- Keep recovery authority distinct from ordinary `approval.decide`, rollback, D2 consent, and BMAD decisions.
- Boot may update journal metadata but may not observe or mutate workspace files.
- Keep `restoring` crash-terminal: startup changes it to `manual_review`; it is never automatically retried.
- Hold the existing lock order: host workspace-commit barrier, Ready mode, workspace scope/grant, store transaction, governed file capability.
- Consume pending recovery authority before evaluating `restore` or `cancel`; every failed decision remains consumed.
- Revalidate installation, renderer generation, workspace ID, grant epoch, governed-edits enablement, journal/checkpoint identity, workspace target hash, file identity, and plan hash immediately before mutation.
- Keep unresolved `recovery_required`, `restoring`, and `manual_review` journals open so updater install handoff remains blocked.
- Do not expose checkpoint bytes, file contents, absolute paths, file-identity hashes, raw errors, or native authority objects through IPC.
- Preserve the existing Node 24.18.0 and pnpm 11.12.0 pins. Node 26 is outside P2.
- Begin every production slice with a failing test, make the smallest implementation pass, run focused proof, and commit that slice before continuing.

## Public Protocol Locked by This Plan

```text
changes.recovery.prepare {
  workspaceId: ContractId,
  workspaceGrantEpoch: safe integer,
  journalId: ContractId
}

changes.recovery.decide {
  recoveryApprovalId: ContractId,
  displayedRecoveryHash: Sha256Digest,
  choice: "restore" | "cancel"
}
```

Renderer-safe preparation outcomes:

```text
review_required  -> approval id, displayed hash, journal/execution ids,
                    bounded relative-path operation summaries, expiry
already_recovered -> journal/execution ids and terminal state
manual_review    -> journal/execution ids and stable reason code
```

Renderer-safe decision outcomes:

```text
recovered -> journal/execution/checkpoint ids, restored file count, completed time
cancelled -> journal/execution ids; durable state remains recovery_required
```

## Task 1: Correct Boot Quarantine and Store Invariants

**Files:**

- Modify: `crates/desktop-app/src/edits.rs`
- Modify: `crates/desktop-store/src/execution.rs`
- Test: `crates/desktop-app/src/edits.rs`
- Test: `crates/desktop-store/src/execution.rs`
- Test: `crates/desktop-update/src/lib.rs`

- [ ] **Step 1: Add failing boot-reconciliation tests**

Add table-driven tests around `reconcile_execution_journals` that seed every nonterminal state and assert the exact boot result:

```rust
#[test]
fn boot_reconciliation_is_metadata_only_and_preserves_reviewable_recovery() {
    assert_boot_transition("prepared", "recovered");
    assert_boot_transition("checkpoint_durable", "recovered");
    assert_boot_transition("preconditions_verified", "recovered");
    assert_boot_transition("result_recorded", "completed");
    assert_boot_transition("applying", "recovery_required");
    assert_boot_transition("effects_applied", "recovery_required");
    assert_boot_transition("postimages_verified", "recovery_required");
    assert_boot_transition("recovery_required", "recovery_required");
    assert_boot_transition("restoring", "manual_review");
    assert_boot_transition("manual_review", "manual_review");
}
```

Use a workspace sentinel whose metadata and bytes are captured before initialization and compared afterward. This proves boot performs no workspace IO or effects.

Run:

```powershell
cargo test -p desktop-app boot_reconciliation --locked
```

Expected: FAIL because `applying`, `effects_applied`, and `postimages_verified` currently advance to `manual_review`.

- [ ] **Step 2: Add failing store-transition and updater-policy tests**

Pin these invariants in `desktop-store`:

```rust
assert!(transition_allowed(RecoveryRequired, Restoring));
assert!(transition_allowed(RecoveryRequired, Recovered));
assert!(transition_allowed(RecoveryRequired, ManualReview));
assert!(transition_allowed(Restoring, Recovered));
assert!(transition_allowed(Restoring, ManualReview));
assert!(!transition_allowed(ManualReview, Restoring));
```

Seed each unresolved state and assert `list_open_effect_journals()` includes it. In `desktop-update`, add a policy case proving any active recovery journal keeps install unavailable even when a candidate is otherwise eligible.

Run:

```powershell
cargo test -p desktop-store execution --locked
cargo test -p desktop-update active_journal --locked
```

Expected: store tests pass or expose a missing assertion; the new update-policy coverage fails until the recovery case is explicit.

- [ ] **Step 3: Implement metadata-only boot transitions**

Replace the mid-effect double transition with one quarantine transition and preserve existing `recovery_required`:

```rust
"applying" | "effects_applied" | "postimages_verified" => {
    store.update_effect_journal(
        &journal.journal_id,
        "recovery_required",
        &journal.journal_json,
        Some(&reconcile_evidence(&journal, "interrupted mid-effect; review required")),
    )?;
}
"recovery_required" | "manual_review" => {}
"restoring" => {
    store.update_effect_journal(
        &journal.journal_id,
        "manual_review",
        &journal.journal_json,
        Some(&reconcile_evidence(&journal, "reviewed recovery was interrupted")),
    )?;
}
```

Keep `OPEN_JOURNAL_STATES_SQL` excluding only `completed` and `recovered`. Do not add an updater exception for recovery.

- [ ] **Step 4: Run focused proof and commit**

```powershell
cargo fmt --all -- --check
cargo test -p desktop-store execution --locked
cargo test -p desktop-update active_journal --locked
cargo test -p desktop-app boot_reconciliation --locked
```

Expected: all green.

Commit:

```powershell
git add crates/desktop-app/src/edits.rs crates/desktop-store/src/execution.rs crates/desktop-update/src/lib.rs
git commit -m "fix(d3): preserve interrupted effects for reviewed recovery"
```

## Task 2: Add Pure Recovery Observation and Restore Primitives

**Files:**

- Modify: `crates/desktop-execution/src/model.rs`
- Modify: `crates/desktop-execution/src/rollback.rs`
- Modify: `crates/desktop-execution/src/lib.rs`
- Test: `crates/desktop-execution/src/rollback.rs`

- [ ] **Step 1: Add adversarial failing tests for deterministic planning**

Extend the existing `plan_recovery` fixture to cover:

- all checkpoint preimages present -> `NoEffect`;
- all declared postimages present -> `Complete`;
- mixed state with complete checkpoint coverage -> `RestoreCheckpoint`;
- missing checkpoint coverage -> `ManualReview`;
- target-hash drift, journal/checkpoint mismatch, tampered hashes, unsupported bytes, and workspace read failure -> fail closed;
- differently ordered journal operations -> identical canonical plan hash and restore ordering;
- a path alias or identity substitution exposed by the broker -> failure before restore.

Add an assertion that the restore plan contains only checkpoint-derived operations ordered by `RelativeWorkspacePath::canonical_cmp`.

Run:

```powershell
cargo test -p desktop-execution recovery --locked
```

Expected: FAIL because the current `RecoveryPlan` carries only a disposition and reason and there is no executable restore plan or canonical recovery hash.

- [ ] **Step 2: Define closed recovery plan types**

Replace the minimal plan with bounded closed types:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryPlan {
    pub journal_id: ContractId,
    pub execution_id: ContractId,
    pub checkpoint_id: ContractId,
    pub workspace_target_hash: Sha256Digest,
    pub disposition: RecoveryDisposition,
    pub operations: Vec<RecoveryOperation>,
    pub plan_hash: Sha256Digest,
    pub reason: RecoveryReason,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryOperation {
    pub relative_path: RelativeWorkspacePath,
    pub expected_current_exists: bool,
    pub expected_current_content_hash: Option<Sha256Digest>,
    pub expected_current_file_identity_hash: Option<Sha256Digest>,
    pub restore_to: CheckpointFileState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryReason {
    NoEffectObserved,
    PostimagesVerified,
    CompleteCheckpointCoverage,
    IncompleteCheckpointCoverage,
}
```

Hash a serialization-only private binding containing schema version, all journal/checkpoint/workspace identities, disposition, and every ordered operation with hashes. Do not serialize checkpoint content into renderer types.

- [ ] **Step 3: Add a fail-closed restore executor**

Add:

```rust
pub fn restore_checkpoint<W: WorkspaceFileIo>(
    workspace: &W,
    plan: &RecoveryPlan,
) -> Result<RecoveryRestoreResult, ExecutionError>;
```

The executor must:

1. require `RestoreCheckpoint`;
2. recheck `workspace_target_hash`;
3. reobserve all planned paths before the first mutation and compare the canonical observation hash to the plan;
4. restore in canonical relative-path order using `create_utf8_durable`, `replace_utf8_durable`, and `delete_durable` with exact expected hashes and identities;
5. reobserve all paths and verify they equal checkpoint preimages;
6. return only journal ID and restored count.

If the existing broker cannot retain safe handles across validation and effect, extend its implementation behind `WorkspaceFileIo` with a scoped recovery transaction rather than accepting path strings as authority. The public runtime command must never accept a path.

- [ ] **Step 4: Prove restore success and failure behavior**

Add tests for create/replace/delete restoration, deterministic ordering, pre-effect drift, target drift, mid-restore broker failure, and postcondition failure. A failure after the first operation returns `RecoveryRequired`; app composition will convert an already-persisted `restoring` journal to `manual_review`.

Run:

```powershell
cargo fmt --all -- --check
cargo clippy -p desktop-execution --all-features --all-targets -- -D warnings
cargo test -p desktop-execution --all-features --locked
```

Expected: all green.

- [ ] **Step 5: Commit the pure recovery core**

```powershell
git add crates/desktop-execution/src/model.rs crates/desktop-execution/src/rollback.rs crates/desktop-execution/src/lib.rs
git commit -m "feat(d3): add deterministic checkpoint recovery core"
```

## Task 3: Compose Single-Use Recovery Authority in the Desktop Host

**Files:**

- Create: `crates/desktop-app/src/recovery.rs`
- Modify: `crates/desktop-app/src/lib.rs`
- Modify: `crates/desktop-app/src/state.rs`
- Modify: `crates/desktop-app/src/edits.rs`
- Modify: `crates/desktop-app/src/wire.rs`
- Test: `crates/desktop-app/src/recovery.rs`
- Test: `crates/desktop-app/src/state.rs`

- [ ] **Step 1: Write failing authority-lifecycle tests**

Create host-level tests proving:

- prepare requires Ready mode, exact current workspace/grant, governed edits enabled, and a `recovery_required` journal;
- `NoEffect` transitions directly to `recovered` without minting approval;
- ambiguous/incomplete durable data returns a safe `manual_review` outcome;
- a restorable plan produces one opaque approval and bounded file summaries;
- prepare performs zero workspace mutations and does not transition to `restoring`;
- cancel consumes approval and leaves the journal `recovery_required`;
- restore consumes approval once; duplicate restore fails;
- expiry, renderer rebind, restart, workspace revoke/switch, grant change, and recovery entry invalidate approval;
- ordinary approval IDs cannot decide recovery and recovery approval IDs cannot enter `approval.decide`;
- journal/checkpoint/workspace/plan drift after prepare fails before mutation;
- a failure after durable `restoring` ends at `manual_review` and cannot be retried.

Run:

```powershell
cargo test -p desktop-app recovery --locked
```

Expected: FAIL because the recovery composition module and pending authority do not exist.

- [ ] **Step 2: Add bounded in-memory recovery authority**

In `recovery.rs`, define a private pending record and bounded collection:

```rust
pub(crate) struct PendingRecovery {
    approval_id: ContractId,
    installation_id: ContractId,
    renderer_session_id: ContractId,
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    journal_id: ContractId,
    execution_id: ContractId,
    checkpoint_id: ContractId,
    plan: RecoveryPlan,
    displayed_recovery_hash: Sha256Digest,
    expires_at: UnixMillis,
}

#[derive(Default)]
pub(crate) struct PendingRecoveries { /* bounded map plus insertion order */ }
```

Use the same bounded-map discipline as `PendingProposals`. Replacement for the same journal consumes the older approval. Process restart reconstructs none.

Add to `HostState`:

```rust
pending_recoveries: Mutex<crate::recovery::PendingRecoveries>,
```

Provide narrow `insert`, `take`, and `invalidate_all` methods. Call invalidation from renderer bind/rebind, workspace revoke, edits-grant change, workspace switch, and `enter_recovery` while preserving lock order.

- [ ] **Step 3: Implement observation-only preparation**

Add:

```rust
pub(crate) fn prepare_recovery(
    state: &HostState,
    renderer: &RendererSessionGuard<'_>,
    workspace_id: &ContractId,
    workspace_grant_epoch: u64,
    journal_id: &ContractId,
    accepted_at: UnixMillis,
) -> Result<HostCommandData, LocalError>;
```

Authenticate the journal and checkpoint through strict domain decoders, require exact cross-bindings, build governed read-only workspace IO, call `plan_recovery`, and map only closed outcomes. For `NoEffect`, persist `recovered` plus evidence atomically. For `ManualReview`, persist `manual_review` only when the durable structure is unsafe; transient observation failures remain quarantined.

- [ ] **Step 4: Implement consume-first decision and restore**

Add:

```rust
pub(crate) fn decide_recovery(
    state: &HostState,
    renderer: &RendererSessionGuard<'_>,
    approval_id: &ContractId,
    displayed_recovery_hash: Sha256Digest,
    choice: RecoveryApprovalChoice,
    accepted_at: UnixMillis,
) -> Result<HostCommandData, LocalError>;
```

Call `take_pending_recovery` first. Validate all bindings and expiry. `Cancel` returns a cancelled projection without durable state change. `Restore` reacquires the ready workspace commit guard, reloads and authenticates journal/checkpoint rows, rebuilds the plan from fresh observations, constant-time compares its hash with both retained and displayed hashes, writes `restoring` plus evidence, calls `restore_checkpoint`, verifies postconditions, then writes `recovered` plus evidence. Any error after `restoring` writes `manual_review` and returns a sanitized recovery-required error.

- [ ] **Step 5: Add renderer-safe wire projections**

Extend `HostCommandData` with:

```rust
ChangesRecoveryPrepared(ChangesRecoveryPreparedWire),
ChangesRecoveryDecision(ChangesRecoveryDecisionWire),
```

Use a tagged outcome and bounded summaries:

```rust
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ChangesRecoveryPreparedWire {
    ReviewRequired {
        recovery_approval_id: ContractId,
        displayed_recovery_hash: Sha256Digest,
        journal_id: ContractId,
        execution_id: ContractId,
        operations: Vec<RecoveryOperationSummaryWire>,
        expires_at: UnixMillis,
    },
    AlreadyRecovered { journal_id: ContractId, execution_id: ContractId },
    ManualReview { journal_id: ContractId, execution_id: ContractId, reason: String },
}
```

The operation summary contains only relative path, `create|replace|delete`, and a stable explanation. Add serialization tests that recursively reject absolute paths, checkpoint content, private hashes, and raw error text.

- [ ] **Step 6: Run focused proof and commit**

```powershell
cargo fmt --all -- --check
cargo clippy -p desktop-app --all-features --all-targets -- -D warnings
cargo test -p desktop-app recovery --locked
cargo test -p desktop-app governed_changes --locked
```

Expected: all green.

Commit:

```powershell
git add crates/desktop-app/src/recovery.rs crates/desktop-app/src/lib.rs crates/desktop-app/src/state.rs crates/desktop-app/src/edits.rs crates/desktop-app/src/wire.rs
git commit -m "feat(d3): require single-use reviewed recovery authority"
```

## Task 4: Add Strict Runtime, IPC, Catalog, and Boundary Contracts

**Files:**

- Modify: `crates/desktop-runtime/src/command.rs`
- Modify: `crates/desktop-runtime/src/lib.rs`
- Modify: `crates/desktop-ipc/src/envelope.rs`
- Modify: `crates/desktop-app/src/commands.rs`
- Modify: `crates/desktop-app/src/edits.rs`
- Modify: `tools/check-boundaries.mjs`
- Test: `crates/desktop-runtime/src/command.rs`
- Test: `crates/desktop-ipc/src/envelope.rs`
- Test: `crates/desktop-app/src/commands.rs`
- Test: `tools/check-boundaries.test.mjs`

- [ ] **Step 1: Add failing closed-command tests**

Pin exact command names, strict payload keys, unknown/duplicate field rejection, invalid IDs/hashes/epochs/choices, timestamp/session/installation mismatch, capability absence, and mutation classification. Assert the Ready catalog grows from 28 to 30 in this exact neighborhood:

```text
changes.propose
approval.decide
rollback.request
changes.history
changes.recovery.prepare
changes.recovery.decide
```

Keep recovery-mode catalog exactly `app.get_boot_state`, `workspace.list`.

Run:

```powershell
cargo test -p desktop-runtime command --locked
cargo test -p desktop-ipc envelope --locked
node --test tools/check-boundaries.test.mjs
```

Expected: FAIL because the command variants and catalog entries do not exist.

- [ ] **Step 2: Add typed runtime variants**

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryApprovalChoice { Restore, Cancel }

PrepareChangesRecovery {
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    journal_id: ContractId,
},
DecideChangesRecovery {
    recovery_approval_id: ContractId,
    displayed_recovery_hash: Sha256Digest,
    choice: RecoveryApprovalChoice,
},
```

Map names to `changes.recovery.prepare` and `changes.recovery.decide`. Classify prepare as read-only for file-effect admission but exclude it from reply caching because it creates ephemeral authority. Decide is mutating and is also excluded from reply caching so a replay cannot recreate a consumed result.

- [ ] **Step 3: Add strict IPC parsers and host dispatch**

Define `deny_unknown_fields` payloads, reuse safe-integer epoch validation, and route both variants through `desktop_app::recovery`. The dispatcher must pass the already-authenticated renderer guard; recovery code must not reconstruct renderer authority from an ID.

Update `should_cache_reply`:

```rust
| LocalCommand::PrepareChangesRecovery { .. }
| LocalCommand::DecideChangesRecovery { .. }
```

Both must be non-cacheable. A request-gate replay without a safe cached reply returns the existing sanitized conflict and never reruns preparation or restore.

- [ ] **Step 4: Extend the executable boundary guard**

Update `tools/check-boundaries.mjs` to require exact byte-identical command order across Rust host, IPC known-command set, TypeScript catalog, and tests. Add probes proving:

- recovery commands are separate from `approval.decide`;
- neither command accepts arbitrary/absolute paths, shell text, checkpoint content, or provider fields;
- recovery mode does not advertise either command;
- the host cache exclusion remains present;
- unresolved journals remain update-blocking.

- [ ] **Step 5: Run contract proof and commit**

```powershell
cargo fmt --all -- --check
cargo test -p desktop-runtime --locked
cargo test -p desktop-ipc --locked
cargo test -p desktop-app commands --locked
node --test tools/check-boundaries.test.mjs
node tools/check-boundaries.mjs
```

Expected: all green.

Commit:

```powershell
git add crates/desktop-runtime/src/command.rs crates/desktop-runtime/src/lib.rs crates/desktop-ipc/src/envelope.rs crates/desktop-app/src/commands.rs crates/desktop-app/src/edits.rs tools/check-boundaries.mjs tools/check-boundaries.test.mjs
git commit -m "feat(d3): expose strict reviewed recovery commands"
```

## Task 5: Add Shared Changes and Activity Recovery Review UI

**Files:**

- Modify: `apps/desktop-ui/src/lib/hostClient/contracts.ts`
- Modify: `apps/desktop-ui/src/lib/hostClient/commandEnvelopes.ts`
- Modify: `apps/desktop-ui/src/lib/hostClient/changesProtocol.ts`
- Modify: `apps/desktop-ui/src/lib/hostClient/client.ts`
- Modify: `apps/desktop-ui/src/lib/hostClient/index.ts`
- Modify: `apps/desktop-ui/src/lib/hostClient/commandCatalog.test.ts`
- Create: `apps/desktop-ui/src/components/RecoveryReview.tsx`
- Create: `apps/desktop-ui/src/components/RecoveryReview.test.tsx`
- Modify: `apps/desktop-ui/src/components/GovernedChangesPanel.tsx`
- Modify: `apps/desktop-ui/src/components/GovernedChangesPanel.test.tsx`
- Modify: `apps/desktop-ui/src/components/panels/ActivityPanel.tsx`
- Modify: `apps/desktop-ui/src/components/panels/ActivityPanel.test.tsx`
- Modify: `apps/desktop-ui/src/App.tsx`
- Modify: `apps/desktop-ui/src/App.test.tsx`

- [ ] **Step 1: Add failing strict-protocol tests**

Add exact-key parsers for both recovery outcomes and envelope builders for both commands. Test unknown keys, wrong discriminators, oversized operation arrays, invalid IDs/hashes/epochs/timestamps, absolute paths, missing fields, and unsafe numeric values. Pin the 30-command catalog order.

Run:

```powershell
pnpm --filter @sapphirus/desktop-ui test --run src/lib/hostClient
```

Expected: FAIL because recovery contracts and client methods do not exist.

- [ ] **Step 2: Add host-client methods with one-shot semantics**

Expose:

```ts
prepareChangesRecovery(input: {
  workspaceId: string;
  workspaceGrantEpoch: number;
  journalId: string;
}): Promise<ChangesRecoveryPrepared>;

decideChangesRecovery(input: {
  recoveryApprovalId: string;
  displayedRecoveryHash: string;
  choice: "restore" | "cancel";
}): Promise<ChangesRecoveryDecision>;
```

Clear retained recovery review on boot-mode change, renderer generation change, workspace/grant change, refresh, successful decision, or any decision error. Never synthesize approval IDs or hashes in browser-demo mode.

- [ ] **Step 3: Build one shared accessible review component**

`RecoveryReview.tsx` receives only parsed recovery projection plus callbacks. It renders:

- stable explanation that restoration returns listed paths to the durable checkpoint;
- relative-path operation summaries;
- `Restore checkpoint` and `Cancel` actions;
- a non-live-region hash confirmation summary without showing the hash value;
- focus return to the invoking Changes or Activity control;
- disabled one-shot actions while a decision is pending.

It must not render raw file content, absolute paths, checkpoint hashes, file identities, or native error details.

- [ ] **Step 4: Integrate the same flow in Changes and Activity**

Extend open-journal projections with `recoveryAvailability: "review_available" | "quarantined" | "manual_review"`. Both panels call one App-owned preparation callback and open the same review component. `manual_review` is visible but has no restore action. `quarantined` explains that the exact workspace and governed-edits grant must be selected.

After `already_recovered`, `recovered`, or `cancelled`, refresh history from the host. Do not optimistically remove unresolved journals.

- [ ] **Step 5: Add interaction, invalidation, and accessibility proof**

Tests must prove:

- both panel entry points produce the same strict prepare command;
- restore and cancel dispatch exactly once under double click;
- stale/rebound review disappears and cannot dispatch;
- refresh/restart does not resurrect approval;
- manual-review entries have no action;
- focus returns correctly;
- keyboard flow and axe checks pass;
- private recovery material never appears in rendered text or test snapshots;
- all pre-existing governed edits and Activity behavior remains green.

Run five consecutive focused passes after the first green result:

```powershell
pnpm --filter @sapphirus/desktop-ui test --run src/components/RecoveryReview.test.tsx src/components/GovernedChangesPanel.test.tsx src/components/panels/ActivityPanel.test.tsx src/App.test.tsx
```

Expected: each pass green with no timeout increase beyond the existing suite configuration.

- [ ] **Step 6: Commit renderer recovery UX**

```powershell
git add apps/desktop-ui/src
git commit -m "feat(ui): add shared reviewed recovery flow"
```

## Task 6: Integrated Qualification, Packet, and Independent Review

**Files:**

- Create: `docs/implementation-packets/P2-d3-reviewed-recovery-2026-07-18.md`
- Modify: `docs/frontend-evolution-2026-07-18.md`
- Modify: `README.md`
- Modify outside Git: `C:\Users\rodri\source\BigBrain\03-projects\sapphirus-bmad-runtime.md`
- Modify outside Git: `C:\Users\rodri\source\BigBrain\00-meta\active-focus.md`
- Test: repository-wide gates below

- [ ] **Step 1: Add an end-to-end restart recovery test**

Build one native fixture that:

1. creates a durable checkpoint and partial journal;
2. restarts the host and proves the journal remains `recovery_required` with no file effect;
3. selects the exact workspace and enables governed edits at a fresh epoch;
4. prepares review and verifies the safe projection;
5. restarts again and proves the old approval is unusable;
6. prepares a fresh review, restores once, verifies exact checkpoint bytes, and sees `recovered` history;
7. proves the journal no longer blocks update handoff;
8. repeats with a crash/failure after `restoring` and proves terminal `manual_review` remains update-blocking.

- [ ] **Step 2: Run focused native qualification**

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo test --workspace --all-features --locked
```

Expected: all green; no skipped recovery test.

- [ ] **Step 3: Run renderer and cross-boundary qualification**

Use the repository-pinned Node and pnpm launchers resolved by `check-toolchain.mjs`, then run:

```powershell
pnpm --filter @sapphirus/desktop-ui test --run
pnpm verify:source
pnpm contracts:verify:cross-language
```

Expected: renderer suite green, source verification green, and cross-language qualification green except only already-documented environment skips.

- [ ] **Step 4: Run clean-checkout proof**

Create a short-path clean worktree from the exact committed P2 revision and rerun `pnpm verify:source`, Rust all-feature tests, renderer tests, and recovery boundary tests there. Record revision, toolchain versions, command lines, counts, and skips in the P2 packet review ledger. Do not treat evidence from a dirty or different revision as qualification.

- [ ] **Step 5: Perform independent adversarial review**

Review the complete P2 diff against the approved design, specifically checking:

- automatic boot-effect bypass;
- approval replay, expiry, and cross-domain substitution;
- workspace/grant/renderer/installation drift;
- path alias, reparse, hardlink, and file-identity races;
- plan-hash and checkpoint/journal substitution;
- partial restore and postcondition failure;
- IPC leakage and renderer synthesis;
- updater bypass while unresolved.

Record every finding and resolution in `docs/implementation-packets/P2-d3-reviewed-recovery-2026-07-18.md`. P2 is not complete with an unresolved P0 or P1 finding.

- [ ] **Step 6: Update durable project truth**

Update README, frontend evolution, and BigBrain with measured evidence only. Keep readiness percentages separate from implementation completion; do not claim pilot or production readiness from local P2 proof alone.

- [ ] **Step 7: Commit final evidence**

```powershell
git add docs README.md
git commit -m "docs(p2): record reviewed recovery qualification"
git status --short
git log --oneline --decorate -8
```

Expected: clean working tree and a bounded P2 commit series.

## P2 Exit Gate

P2 is complete only when all statements below are proven on the same committed revision:

- [ ] Boot performs no workspace observation or recovery effect.
- [ ] Interrupted mid-effect journals remain durably `recovery_required` and visible.
- [ ] Pre-effect journals reconcile to `recovered`; durable results reconcile to `completed`.
- [ ] Interrupted `restoring` becomes terminal `manual_review` and is never automatically retried.
- [ ] Only a fresh exact reviewed recovery approval can authorize restore.
- [ ] Recovery approval is single-use, short-lived, restart-ephemeral, and cross-domain isolated.
- [ ] Every authority, observation, file identity, and plan binding is revalidated immediately before mutation.
- [ ] Successful restore verifies the exact durable checkpoint before recording `recovered`.
- [ ] Any failure after `restoring` ends in terminal `manual_review`.
- [ ] Unresolved recovery states remain update-blocking.
- [ ] No checkpoint content, private hash, absolute path, raw error, token, or native authority crosses IPC.
- [ ] Changes and Activity use one shared strict recovery review flow.
- [ ] Existing edit, undo, D2 Help, BMAD foundation, packaging, and release gates remain green.
- [ ] Clean-checkout proof and independent adversarial review are recorded with no unresolved P0/P1 finding.
