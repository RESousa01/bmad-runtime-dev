# P2 D3 reviewed recovery authorization design

**Date:** 2026-07-18  
**Status:** Approved design checkpoint; implementation plan pending written-spec review  
**Branch:** `codex/p2-d3-recovery-closure`  
**Base:** corrected local `main` checkpoint `6728b78`

## 1. Outcome

P2 closes the governed-edits recovery gap without granting automatic file-effect authority. An interrupted D3 execution is quarantined durably. After the user reopens the exact workspace and explicitly enables governed edits, Sapphirus may inspect the retained checkpoint and current files read-only and present a bounded recovery review. Restoring files requires a fresh, short-lived, single-use recovery approval bound to that exact review and current authority. Boot, restart, history display, and workspace reopening never restore files by themselves.

P2 is complete when an interrupted journal can be classified, reviewed, safely restored to its durable pre-effect checkpoint, or kept quarantined; every authority, file, checkpoint, or review drift fails closed without claiming recovery.

## 2. Existing boundary

The current D3 vertical already provides:

- durable encrypted execution checkpoints;
- a validated effect-journal state machine and atomic execution results;
- governed workspace IO with exact path, grant-epoch, file-identity, and preimage checks;
- single-use edit and undo proposals;
- boot reconciliation that completes safe durable tails and surfaces ambiguous journals;
- renderer-safe history with open-journal summaries;
- update blocking while unresolved journals remain open.

The missing authority is an explicit, user-reviewed path from `recovery_required` to `restoring` and then `recovered`. Today boot reconciliation moves ambiguous interrupted effects immediately to terminal `manual_review`, making safe in-product restoration impossible.

## 3. Non-goals

P2 does not:

- automatically restore at boot, shutdown, update, workspace mount, or edit enablement;
- retry a recovery that was itself interrupted;
- treat a normal edit/undo approval as recovery authority;
- persist pending approvals across restart;
- recover journals from another workspace, grant epoch, installation identity, or checkpoint;
- overwrite files whose reviewed current state has drifted;
- expose absolute paths, checkpoint bytes, file content, hashes, authority objects, or raw storage errors across IPC;
- add cloud/model authority, P1 updater enablement, or broader BMAD behavior;
- provide destructive “discard journal” or “assume recovered” controls;
- claim crash/power-loss atomicity across multiple filesystem operations.

## 4. Recovery state model

The durable journal remains the recovery source of truth.

| Observed boot state | Boot disposition | File effect at boot |
|---|---|---|
| `prepared`, `checkpoint_durable`, `preconditions_verified` | `recovered` | None; effects cannot have started |
| `result_recorded` | `completed` | None; result is already durable |
| `applying`, `effects_applied`, `postimages_verified` | `recovery_required` | None; quarantine for explicit review |
| `recovery_required` | Remain `recovery_required` | None |
| `restoring` | `manual_review` | None; interrupted recovery is ambiguous and never retried automatically |
| `manual_review` | Remain `manual_review` | None |

`completed`, `recovered`, and `manual_review` remain terminal journal states. `manual_review` remains visible and update-blocking because the durable open-journal query intentionally excludes only `completed` and `recovered`.

The only P2 file-effect transition is:

```text
recovery_required
  -> read-only preparation
  -> fresh user review
  -> single-use approval
  -> exact authority and file revalidation
  -> restoring
  -> recovered
```

Any failure after `restoring` begins transitions to `manual_review`. Failures before `restoring` leave the journal at `recovery_required`, consume the pending in-memory review, and require a fresh review.

## 5. Authority model

Recovery authority is distinct from ordinary edit, undo, D2 consent, and BMAD decisions.

A pending recovery review is process-local, bounded, and contains the minimum private binding needed to decide one journal:

- recovery approval ID and journal/execution/checkpoint IDs;
- local installation authority reference;
- workspace ID and the current governed-edits grant epoch;
- renderer generation;
- authenticated checkpoint payload identity and hash;
- exact journal state and authenticated journal identity;
- canonical current-observation set for every checkpoint entry;
- canonical recovery-plan hash and displayed-review hash;
- issue and expiry instants;
- a recovery-only audience/domain separator.

The renderer receives only the approval ID, expiry, journal/execution IDs, bounded relative paths, operation labels such as `restore_content`, `recreate`, or `remove_partial_create`, and human-readable conflict/status facts. Private hashes, checkpoint content, absolute paths, file identities, and authority references never cross IPC.

Approval is valid only when all bindings still match. It is consumed once for restore, cancel, expiry, renderer rebind, workspace revoke/switch, grant change, app recovery entry, or any failed decision. Restart discards all pending recovery reviews.

## 6. Commands and projections

P2 adds two explicit capability-gated commands rather than extending `approval.decide`:

- `changes.recovery.prepare`: requests read-only preparation for one open journal in the exact current workspace/grant.
- `changes.recovery.decide`: accepts `restore` or `cancel` for the exact displayed recovery review.

Keeping these commands separate makes cross-authority replay unrepresentable at the command boundary. A normal changes approval ID must be rejected by recovery decide, and a recovery approval ID must be rejected by `approval.decide` and all D2/BMAD decision consumers.

History adds a renderer-safe recovery availability projection for each open journal:

- `review_available` only for authenticated `recovery_required` journals in the current workspace with governed edits enabled;
- `quarantined` for a journal awaiting workspace/grant authority;
- `manual_review` for an interrupted recovery or structurally ambiguous durable record;
- no action for another workspace or stale grant.

The Changes and Activity surfaces may start recovery review from the same host projection. Both render one shared recovery-review component and send the same strict host command shapes.

## 7. Preparation data flow

`changes.recovery.prepare` performs these steps while holding the normal ready/workspace authority order:

1. Validate command envelope, renderer generation, workspace ID, and exact grant epoch.
2. Require governed edits to be explicitly enabled for the workspace.
3. Load and authenticate one `recovery_required` journal and its referenced checkpoint.
4. Verify journal/checkpoint/workspace/candidate identities and deserialize through closed bounded types.
5. Observe every checkpoint path through governed read-only workspace IO. Reject aliases, reparse points, sensitive paths, volume changes, oversized files, and unsupported bytes.
6. Derive a deterministic recovery plan from checkpoint preimages and current observations.
7. If no restoration is necessary, transition directly to `recovered` with evidence and return a closed already-restored projection; no approval is minted.
8. If observation is ambiguous or unsupported, leave the journal quarantined or move structurally unsafe durable state to `manual_review`; emit only a stable safe reason.
9. Otherwise insert one pending in-memory recovery review and return its bounded renderer projection.

Preparation performs no write, journal transition to `restoring`, checkpoint mutation, update action, process launch, or network call.

## 8. Restore data flow

`changes.recovery.decide` always removes the pending review before evaluating the choice.

For `cancel`, it returns a cancelled projection and leaves the journal `recovery_required`.

For `restore`:

1. Reacquire the exact ready, renderer, workspace, governed-edits, installation, journal, and checkpoint authorities.
2. Reject expiry or any binding drift.
3. Reobserve every planned path and require the canonical observation set and plan hash to equal the displayed review.
4. Open and retain the governed file handles needed to prevent a path-identity race between final validation and mutation.
5. Persist `restoring` plus evidence before the first file effect.
6. Restore checkpoint entries in deterministic relative-path order using the existing durable governed IO primitives; flush each file and owning directory.
7. Reobserve and verify every restored postcondition against the checkpoint.
8. Persist `recovered` plus bounded recovery evidence only after all postconditions pass.
9. Emit execution/checkpoint projection events and return a renderer-safe recovered result.

If validation fails before step 5, no files change and the journal remains `recovery_required`. If any operation or verification fails after step 5, the journal becomes `manual_review`; the response is a stable non-retryable safe error and never claims success.

## 9. Storage and evidence

No new durable approval table is introduced. Pending recovery authority remains memory-only so restart cannot resurrect consent.

The existing effect journal and checkpoint remain authoritative. P2 may add closed recovery metadata to the bounded journal JSON, but it must not rewrite the original candidate/spec/consumption/checkpoint identities. Evidence records use dedicated event types and purpose-separated hashes, including:

- recovery review prepared;
- recovery cancelled;
- restore started;
- restore completed;
- restore failed into manual review;
- boot quarantined an interrupted effect;
- boot detected an interrupted restore.

Evidence payloads remain metadata-only and contain no path, content, or raw error text. Store transactions must keep every journal transition and its evidence append atomic.

## 10. Failure and recovery behavior

Stable public outcomes are deliberately narrow:

- `recovery_review_required`;
- `recovery_not_available`;
- `recovery_review_expired`;
- `recovery_binding_changed`;
- `recovery_conflict`;
- `recovery_manual_review_required`;
- `recovery_completed`;
- `recovery_already_satisfied`.

All storage, filesystem, serialization, and identity causes map to these safe categories. Errors do not disclose absolute paths, source bytes, OS codes, database details, hashes, or internal journal content.

An unresolved `recovery_required`, `restoring`, or `manual_review` journal continues to block update installation and release handoff. A successful `recovered` journal no longer blocks updates.

## 11. Test strategy

### State and store

- Boot leaves interrupted effect states at `recovery_required` and never touches files.
- Boot finalizes safe pre-effect/result-recorded tails exactly as today.
- Boot converts interrupted `restoring` to terminal `manual_review`.
- Only the approved recovery transitions are accepted; terminal states remain terminal.
- Every transition and evidence append is atomic and survives restart.

### Authority and replay

- Recovery preparation requires the exact workspace and current governed-edits epoch.
- Renderer rebind, workspace switch/revoke, grant change, expiry, restart, and app recovery invalidate pending review.
- Recovery IDs cannot authorize normal edit/undo or D2/BMAD actions, and their IDs cannot authorize recovery.
- Identical retries require a fresh review; concurrent decisions have exactly one terminal winner.

### Filesystem effects

- Partial create/replace/delete sequences restore the exact checkpoint.
- External drift after review fails before writes and leaves `recovery_required`.
- Alias, reparse, volume/file-identity, sensitive-path, unsupported-content, and size cases fail closed.
- A failure after `restoring` begins reaches `manual_review` and cannot be retried automatically.
- Successful recovery verifies all postconditions before `recovered` becomes durable.

### IPC and renderer

- Commands and replies are closed, bounded, capability-gated, and strict against duplicate/unknown fields.
- Projections contain relative paths and safe labels only.
- History, Changes, and Activity show consistent availability and outcomes.
- Refresh/restart never produces a stale restore button.

### Integrated gates

- Existing edit, undo, history, D2 Help, update-blocking, and recovery-mode tests remain green.
- Full Rust formatting, strict all-target/all-feature Clippy and workspace tests.
- Exact pinned source verification, renderer tests, typecheck, lint, boundaries, secret scan, and production build.
- Independent security/data-loss review before merge.

## 12. Delivery slices

P2 will be planned and reviewed as bounded slices:

1. Correct boot quarantine semantics and lock state/store invariants.
2. Add pure recovery planning and adversarial filesystem tests.
3. Add process-local recovery authority and single-use decision flow.
4. Add IPC commands, safe projections, catalog, and boundary enforcement.
5. Add Changes/Activity recovery review UI and restart/refresh behavior.
6. Run integrated recovery, update-blocking, security, and clean-checkout proof; independently review the full P2 diff.

Each slice begins with a meaningful failing test and ends with focused proof before commit.

## 13. Acceptance criteria

P2 is complete only when:

- boot performs no recovery file effects;
- interrupted effects remain durably quarantined and visible;
- only a fresh exact recovery review can authorize restoration;
- the restore decision is single-use, short-lived, restart-ephemeral, and cross-domain isolated;
- every file and authority binding is revalidated immediately before mutation;
- successful recovery restores and verifies the exact durable checkpoint;
- interrupted recovery becomes non-retryable `manual_review`;
- unresolved recovery blocks update/install handoff;
- no private authority, hashes, checkpoint bytes, absolute paths, or raw errors cross IPC;
- existing edit/undo/D2 behavior is unchanged;
- full local gates and independent review are green on one committed revision.
