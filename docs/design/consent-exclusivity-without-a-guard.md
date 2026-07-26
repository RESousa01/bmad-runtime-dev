# Consent exclusivity without holding a guard

- Status: **proposed — design only, no code written**
- Author: investigation of `crates/desktop-egress/src/consent.rs` at `codex/forge-v2`
- Touches: a security invariant (single-use egress consent). **Must be reviewed
  before implementation.**
- Related: [ADR-0010](../adr/ADR-0010-native-agent-runtime-and-turn-scoped-egress.md),
  P15 D7 and Phase 0
  (`docs/implementation-packets/P15-hybrid-coding-agent-2026-07-26.md:369`, `:530`)

---

## 0. Summary, up front

The brief asked how to keep the exclusivity guarantee without holding a lock for
the lifetime of `ContextDecisionEvidence`. Reading the code changes the question.

1. **The guard protects a critical section that reads no ledger state.**
   `ContextDecisionEvidence` is consumed by exactly one production function,
   `bridge_method_context_decision` (`crates/desktop-app/src/bmad_model/bridge.rs:52`),
   which performs hashing and field comparison over data already copied out of
   the ledger. It never re-reads the ledger. A concurrent mutation during that
   window is therefore unobservable, and excluding it changes no outcome.

2. **The guard is not what enforces single use.** Single use is enforced by the
   `Pending -> Consumed` transition inside `consume_if_pending`
   (`consent.rs:629-679`), which takes its own lock. The crate's existing test
   `cancellation_and_consumption_race_has_exactly_one_terminal_winner`
   (`crates/desktop-egress/tests/consent.rs:281`) proves this without evidence
   being involved at all.

3. **The guard is not the concurrency blocker Phase 0 names.** It is released
   before every durable side effect. The `!Send` guard that is actually held
   across a blocking model round trip is
   `ReadyWorkspaceCommitGuard`/`MutexGuard<'_, ()>`
   (`crates/desktop-app/src/state.rs:155`), acquired at
   `commands.rs:1364` and held across the whole chat tool loop including
   `transport.send` at `commands.rs:1422`. Deleting the consent guard would not
   make `send_chat_message` viable as an `async fn`.

4. **There is nevertheless a real, currently-unenforced invariant here** — just
   not the one the guard is written to defend. Between the pendingness check in
   `evidence()` and the `consume()` that spends the decision, the BMAD Help
   coordinator writes a *durable* record of the user's consent
   (`coordinator.rs:731`, `record_context_review`). That window is not covered by
   anything. A cancel landing in it produces a persisted "the user reviewed and
   approved this context" row for a decision that was withdrawn.

**Recommendation: do not replace the guard — delete the gap it was gesturing
at.** Fold evidence minting into the consume transition, so evidence is a
by-product of the same atomic ledger state change that spends the decision
(Option C, §4.3). This is strictly stronger than today, removes the lock from the
derivation entirely, makes `ContextDecisionEvidence` owned and `Send`, and costs
one reordering in one function.

**And do Phase 0's concurrency work separately** (Option E, §4.5), because the
consent change delivers none of it and coupling them means doing surgery on a
security invariant for a benefit it does not provide.

---

## 1. Complete inventory of the type

Every site, found by exhaustive search of the worktree.

### Construction — one production site

| Site | Kind |
|---|---|
| `crates/desktop-app/src/bmad_model/coordinator.rs:715-720` | **production** — the only one |
| `crates/desktop-app/src/bmad_model/bridge_tests.rs:179`, `:201` | test helpers `bridge` / `bridge_with` |
| `crates/desktop-egress/tests/consent.rs:414`, `:461`, `:483`, `:499`, `:524`, `:534` | crate tests |

The only constructor is `ConsentService::evidence` (`consent.rs:729-765`).

### Use — one production consumer

`bridge_method_context_decision` (`bridge.rs:52-118`), reached via
`bridge_decision` (`coordinator.rs:1240-1272`). It takes the evidence **by
value** and returns a `MethodContextDecision`.

### Lanes that do *not* use it

- **`ChatCoordinator`** (`chat.rs:199-214`) — owns its own
  `MemoryDecisionLedger` (`:200`) and calls only `approve` + `consume`, back to
  back, in `authorize()` (`chat.rs:501-533`). This is the lane the agent loop
  runs in. It never constructs evidence.
- **`BmadCapabilityCoordinator`** (`capability_coordinator.rs:293`) — same:
  `approve`, `cancel`, `consume`; no evidence.

### Ledger ownership — three separate ledgers, each behind an outer mutex

```
HostState (state.rs:100-124)
├── bmad_model:        Mutex<BmadHelpCoordinator>        (:116) → owns ledger (coordinator.rs:261)
├── bmad_capabilities: Mutex<BmadCapabilityCoordinator>  (:118) → owns ledger (capability_coordinator.rs:293)
└── chat:              Mutex<ChatCoordinator>            (:124) → owns ledger (chat.rs:200)
```

The `ledger` field is private in all three, with no accessor. `MemoryDecisionLedger`
is not `Clone` and is never wrapped in `Arc`. Consequence: **the consent ledger
mutex is always acquired from inside an outer coordinator mutex, by one thread,
and is never contended across lanes.** The three lanes cannot see each other's
ledgers at all.

The P15 Phase 0 line *"Rework or every lane serialises on consent"*
(`P15...md:559-561`) is therefore not accurate as written. Lanes serialise on
their coordinator mutex — and, for the chat lane, on the process-global
`workspace_commits: Mutex<()>` (`state.rs:102`).

---

## 2. What the guard actually provides

### 2.1 The claimed property

`consent.rs:722-723`:

> The returned view retains the ledger lock until it is dropped, so a concurrent
> consume or cancel cannot race bridge materialization.

Restated as an invariant: *for the lifetime of a `ContextDecisionEvidence`, the
ledger entry it was derived from cannot change state.*

That statement is **true**. The question is whether it is **load-bearing**.

### 2.2 Test 1 — does the guarded region read the ledger?

`bridge_method_context_decision` (`bridge.rs:59-117`) does, in order:

- `manifest.verify()` — canonical hash over the manifest. No ledger.
- `d2_binding.verify_for(manifest)` — hash + field comparison. No ledger.
- `method_binding.binding_hash()` — hash. No ledger.
- ~18 equality comparisons between `evidence` getters, `manifest.draft`,
  `d2_binding.draft` and a caller-held `expected` struct. Every `evidence`
  getter (`consent.rs:267-317`) reads `self.decision`, which is
  `&'a PendingContextDecision` — **the caller's copy** (`&approved.decision`,
  owned by `ActiveBmadHelp::Approved`), not the ledger's `stored_copy`.
- `canonical_hash` over the manifest items.
- Construction of `MethodContextDecision` from evidence getters.

**Nothing in the guarded window reads `MemoryDecisionLedgerState`.** The
exclusion is over a pure function of data already copied out. A concurrent
`consume` or `cancel` could not have changed any value this function observes.

### 2.3 Test 2 — does the guard cover the window that matters?

No. In `submit_bmad_help_review` (`coordinator.rs:713-777`) the evidence is
created and dropped inside an immediately-invoked closure:

```
713   let method_decision_result = (|| {
714       let consent_service = ConsentService::new(&self.ledger);
715       let evidence = consent_service.evidence(...)?;      // guard acquired
721       bridge_decision(evidence, &session_v3, &approved, input.submitted_at)
722   })();                                                   // guard released here
...
731   service.record_context_review(...)                      // DURABLE WRITE, v3 -> v4
...
763   ConsentService::new(&self.ledger).consume(...)           // the actual spend
803   store.begin_method_advance(...)                          // v4 -> v5
```

The guard is released at line 722. The durable consent receipt is written at
731. The decision is spent at 763. **The check-to-effect window — lines 722
through 763 — is entirely unguarded**, and it contains a SQLite write.

The closure is not incidental: because `parking_lot::Mutex` is not reentrant,
calling `consume` while the evidence guard were still alive would **deadlock**,
not error. The closure exists to prevent that. So the code is structured to
release the guard as early as possible, which is a fair summary of how much it
is worth.

### 2.4 Test 3 — does the guard enforce single use?

No. Single use is the `Pending -> Consumed` write inside `consume_if_pending`
(`consent.rs:675`), performed under its own acquisition of the same lock, plus
the `DecisionState::Consumed { .. } => Err(DecisionAlreadyConsumed)` arm
(`:641`). `turn_grant.rs:12-16` states this explicitly for the turn layer, and
the same reasoning applies here. `tests/consent.rs:281-319` proves it across two
real threads with no evidence in the picture.

Two concurrent `evidence()` calls for the same decision (were the outer mutex
absent) would both succeed, both bridge, and then both call `consume()` — of
which exactly one would win. The guard does not change that outcome; it only
orders the two bridges.

### 2.5 What the guard *does* provide

Being rigorous cuts both ways. Two properties are genuine:

**G1 — `ContextDecisionEvidence` is `!Send` and cannot escape.** `parking_lot` is
pinned at `=0.12.5` with default features (`Cargo.toml:35`), so `send_guard` is
off and `MutexGuard` is `!Send`. Evidence therefore cannot be moved to another
thread, cannot be stored in a `Send` structure, and cannot live across an
`.await`. Without the guard, `ContextDecisionEvidence<'a>` would be `Send` (its
other fields are `&PendingContextDecision` and `UnixMillis`), and the compiler
would stop objecting to code that mints evidence on one thread and uses it on
another while the first cancels. Any replacement must preserve *some* barrier
against evidence becoming a storable, deferred-use token — that shape is a
capability leak.

**G2 — reentrancy fails loudly.** Any future code that touches the ledger while
evidence is alive deadlocks deterministically in development rather than
silently observing a stale view. Crude, but it is enforcement. A replacement
that returns an error is better; one that silently succeeds is worse.

**G3 — it is crate-local.** The invariant lives in `desktop-egress`. The outer
`Mutex<BmadHelpCoordinator>` that currently makes the guard redundant lives in
`desktop-app` and is invisible from the egress crate. Relying on it is relying
on a property that a caller in another crate may withdraw without any signal.
This is the strongest argument for the guard, and it is an argument for keeping
*an* enforcement mechanism, not specifically for keeping *this* one.

### 2.6 What would break if the guard were simply deleted

**Today: nothing observable.** `submit_bmad_help_review` runs under
`state.bmad_model.lock()` (`commands.rs:982-984`) with `&mut self`, so the
coordinator — and therefore its private ledger — is already exclusively held for
the whole function. Deleting `_ledger_state` would compile (after removing the
`compile_fail` doctest expectations) and change no behaviour in any current
execution.

**Concrete corruption scenario, once the ledger is shared.** Phase 0 plausibly
moves to `Arc<MemoryDecisionLedger>` so that replay tombstones are global rather
than per-coordinator. At that point the outer mutex stops covering the ledger,
and:

> **Scenario A — phantom approval in the receipt trail.**
> Thread 1 is in `submit_bmad_help_review`. It calls `evidence()` at
> `coordinator.rs:715`; the ledger says `Pending`; evidence is returned.
> Thread 2 — a sign-out, a workspace grant revocation
> (`state.rs:685`, `bmad_model.invalidate(...)`), or the user pressing Cancel in
> a second window — calls `cancel()`. The entry flips to `Cancelled`.
> Thread 1 does not re-read the ledger. It completes the bridge, and at
> `coordinator.rs:731` writes a durable `MethodContextDecision` into the Method
> session, advancing it to `Ready` / version 4. That row asserts, in the
> persisted receipt trail, *"the user reviewed manifest M, consent disclosure C,
> at time T"*.
> Thread 1 then reaches `consume()` at `:763`, which correctly returns
> `DecisionCancelled`; no bytes go to Azure. The coordinator goes `Terminal`.
> **Net result: no egress, but a durable record of a consent the user withdrew.**
> For a deployment whose stated priority is that *"egress accounting outranks
> execution sandboxing"* (`P15...md:118-121`), a receipt trail that can assert an
> approval which was revoked is the failure mode that matters.

Note carefully: **the guard does not prevent Scenario A even today**, because it
is released at `coordinator.rs:722`, before the durable write at `:731`. The
scenario needs only that the ledger become shared; the guard buys a few
microseconds of the window, not the window.

> **Scenario B — double egress.** Does not occur, guard or no guard.
> `consume_if_pending` is atomic and one-way. This is worth stating so that
> nobody defends the guard on this ground.

### 2.7 Two doc comments that are wrong and should be corrected regardless

- `bridge.rs:48-51`: `#[expect(clippy::needless_pass_by_value, reason = "consuming
  the one-use evidence view keeps its ledger guard live through bridge validation
  and prevents reuse")]`. Taking by value **ends** the guard at the close of the
  function; taking by reference would keep it live *longer*, in the caller. And
  reuse is prevented by the absence of `Clone` plus by-value consumption, not by
  the guard. The lint suppression is correct; its stated reason is not.
- `consent.rs:722-723`: true as written, but implies the excluded race has an
  observable effect. It does not (§2.2).

---

## 3. The invariant worth having, stated precisely

Strip away the mechanism and write down what must hold:

> **I1 (egress single use).** For each `decision_id`, at most one
> `DecisionConsumption` is ever produced, and no model request is dispatched
> without one. — *Holds today, enforced by `consume_if_pending`. Not at risk.*

> **I2 (binding).** Any `DecisionConsumption` names exactly the manifest,
> invocation binding, policy, installation and session authority that were
> approved. — *Holds today, enforced by the field comparisons at
> `consent.rs:650-660`. Not at risk.*

> **I3 (receipt correspondence).** Any durable record derived from a consent
> decision — today, `MethodContextDecision` — exists **if and only if** that
> decision was consumed. No derived receipt for a decision that was cancelled,
> expired, or never spent; no consumption without its receipt.
> — ***Does not hold today.*** This is the gap.

I3 is what the guard was reaching for and does not deliver. Everything below is
judged against I3.

---

## 4. Options

### 4.1 Option A — monotonic ledger generation counter, re-validated at consume

Add `generation: u64` to `MemoryDecisionLedgerState`, incremented on every
mutation. `evidence()` validates under the lock, copies fields into an **owned**
evidence value carrying the generation, releases the lock. `consume()` takes the
evidence and, under the lock, requires `state.generation == evidence.generation`,
else a new `EgressError::DecisionRaced`.

- **Guarantees:** linearisability of "the ledger did not change at all between
  observation and consumption" — strictly stronger than the guard, and it covers
  the durable-write window.
- **Stops guaranteeing:** nothing, but it over-guarantees. A global counter is
  invalidated by *any* unrelated approve, cancel or prune.
- **How it fails:** false positives, at scale. The chat lane mints a decision per
  message (`chat.rs:506`); the moment ledgers are shared, any concurrent turn
  invalidates every outstanding witness. Callers then either retry — re-prompting
  the user for consent they already gave — or the check is quietly relaxed to
  make the failures stop, which is how a security check becomes decorative.
- **Refinement, and why it collapses:** make the counter per-decision. But a
  decision has exactly one transition out of `Pending`, and its content is
  already covered by `decision_hash`. "Same decision, still `Pending`" *is* the
  per-decision generation check — which is Option B, already implemented.

**Reject.** Over-broad, and the correct narrowing is another option.

### 4.2 Option B — owned snapshot plus compare-and-swap on consume

`evidence()` validates under the lock, copies fields into an owned
`ContextDecisionEvidence` (no lifetime on the ledger, no guard), releases the
lock. `consume()` requires that evidence and, under the lock, re-checks that the
entry is still `Pending` with the same `decision_hash`.

- **Guarantees:** the receipt and the consumption refer to the same live
  decision, checked atomically at the point of spend. I1 and I2 unchanged.
- **Stops guaranteeing:** nothing that is guaranteed today.
- **How it fails:** it does not, on its own, fix I3. The durable write at
  `coordinator.rs:731` still happens *before* the CAS, so Scenario A survives
  verbatim. B is only correct combined with reordering the coordinator so the
  consume precedes the durable write.
- **Note:** the CAS already exists. `consume_if_pending` performs
  `stored.as_ref() != input.decision` (`consent.rs:651`) — content equality over
  the whole decision. B adds nothing to the ledger; it only threads the evidence
  through so the two calls are visibly one transaction.

**Viable, but strictly dominated by C**, which achieves the same thing in one
lock acquisition and with no token that can be dropped, stashed, or forgotten.

### 4.3 Option C — mint evidence *inside* the consume transition (recommended)

Replace `ConsentService::evidence` with a single call that performs the
`Pending -> Consumed` transition and, in the same critical section, mints the
evidence view from the ledger's own stored copy:

```
ConsentService::consume_with_evidence(ConsumeDecisionInput<'_>)
    -> Result<ConsumedDecision, EgressError>
```

where `ConsumedDecision` is a non-`Clone`, non-`Serialize` wrapper welding the
two together:

```
ConsumedDecision::evidence(&self) -> &ContextDecisionEvidence   // owned, no lifetime, Send
ConsumedDecision::into_consumption(self) -> DecisionConsumption
```

`ContextDecisionEvidence` becomes an **owned** struct with the same private
fields and the same nine getters, plus `consumption_hash()` binding it to *this*
consumption. `DecisionEvidenceInput` and `ConsentService::evidence` are deleted.
`observed_at()` becomes `consumed_at` — the same value the caller passes today
(`input.submitted_at` is used for both, `coordinator.rs:718` and `:767`), so the
bridge's `evidence.observed_at() != expected.observed_at` check at `bridge.rs:82`
is unchanged in meaning and in value.

Caller change in `submit_bmad_help_review` — a reordering, not a rewrite:

```
current:  evidence → bridge → record_context_review(v3→v4) → consume → begin_method_advance(v4→v5)
proposed: consume_with_evidence → bridge → record_context_review(v3→v4) → begin_method_advance(v4→v5)
```

`invocation_id` (`coordinator.rs:751-762`) derives from `session_id`,
`decision_id` and `submitted_at`, all available before the bridge, so it simply
moves up. `session_authority_hash` for the bridge comes from `session_v3`
(loaded at `:692`), unaffected. `model_bridge_binding_hash` needs
`ready_session` (v4) and stays after the review write.

- **Guarantees:** I3 holds by construction. Evidence cannot exist for a decision
  that was not consumed, and the consumption cannot exist without the evidence.
  There is one lock acquisition, one atomic state change, and zero TOCTOU window
  — the "check" and the "use" are the same instruction sequence. I1 and I2 are
  untouched (`consume_if_pending`'s logic does not change; only its return type
  widens).
- **Stops guaranteeing:** two things, both of which must be argued explicitly.
  - **(a) A bridge failure now burns the decision.** Today
    `BindingMismatch` from the bridge returns the coordinator to `Approved`
    (`coordinator.rs:726`) and the user can retry. Under C the decision is
    already `Consumed`, so the user must re-approve. Assessment: acceptable and
    arguably correct. Every value the bridge compares was minted by the host
    moments earlier from `approved.prepared` and `session_v3`; a mismatch means
    host-side corruption or substitution, not user error, and the coordinator's
    own reaction to the adjacent failures is already `Terminal` with
    `terminal_reason_for_egress`. Fail-closed on an integrity fault is the right
    default, and the decision lifetime is five minutes
    (`MAX_DECISION_LIFETIME_MS`, `consent.rs:12`) so re-approval is cheap.
  - **(b) A dropped `ConsumedDecision` is an unaccounted consumption.** The
    ledger records `Consumed` but no request was dispatched. This under-claims
    egress in the *safe* direction — consent spent, no bytes sent. The dangerous
    direction (bytes sent, no consumption) remains impossible.
  - **(c) `ContextDecisionEvidence` becomes `Send`.** G1 from §2.5 is lost. It is
    replaced by three narrower controls: the type stays non-`Clone` and
    non-`Serialize` (existing `compile_fail` doctests carry over); it can only be
    obtained by welding it to a `DecisionConsumption` that is itself single-use;
    and it carries `consumption_hash`, so a stashed evidence cannot be paired
    with a different consumption without the bridge noticing. A stashed evidence
    is then only usable to re-derive a `MethodContextDecision` identical to the
    one it already produced — which is a re-computation, not a second authority.
- **How it fails:** if someone later adds a second consumer of evidence that
  wants it *before* deciding whether to spend the decision, C forces them to
  spend first. That is a real constraint. It is also the correct one — "look at
  the consent, then decide whether to use it" is precisely the shape that
  reintroduces I3's gap. If such a caller genuinely appears, the right answer is a
  separate read-only projection that is explicitly *not* consent evidence.
- **Cost:** ~60 lines in `consent.rs`, one reordering in `coordinator.rs`, one
  signature change in `bridge.rs`, and test churn — `bridge_tests.rs:172-211`
  (`bridge` / `bridge_with`, 9 call sites across 5 tests) must build one fixture
  per bridge instead of reusing `fixture.ledger`, and `tests/consent.rs:403-540`
  restructures to consume-then-inspect.

### 4.4 Option D — keep the guard, scope it to a closure

`ConsentService::with_evidence(input, |evidence| { ... }) -> Result<R, E>`, with
the guard living only for the closure body.

- **Guarantees:** exactly today's property, plus non-escape becomes structural
  rather than a consequence of lifetime plumbing. And it removes the `!Send`
  obstruction for async: you cannot `.await` inside a non-`async` `FnOnce`, so
  the compiler proves the guard never spans an await point.
- **Stops guaranteeing:** nothing.
- **How it fails:** it does nothing whatsoever about I3, because I3's window is
  outside the guard in the first place. It also preserves the reentrancy
  deadlock, and it constrains the closure to do no I/O — correct, but it means
  the durable write must stay outside, which is exactly where the gap is.

**Useful as an intermediate step** if the team wants a zero-semantic-change
commit that unblocks async before touching consent ordering. Not sufficient.

### 4.5 Option E — leave consent alone; fix the actual concurrency blocker

The blockers on `async host_dispatch` are, in order of severity:

1. **`ReadyWorkspaceCommitGuard`** (`state.rs:153-157`), which owns
   `MutexGuard<'_, ()>` over the process-global `workspace_commits`
   (`state.rs:102`) and a `RwLockReadGuard<BootMode>` — both `!Send`. It is
   acquired at `commands.rs:1364` and held across the entire chat tool loop
   including `transport.send` at `:1422`. The comment at `:1395-1396` — *"a slow
   reply never blocks sign-out, revocation, or another conversation"* — is true
   of `state.chat`, and not true of this guard.
2. **`Mutex<BmadHelpCoordinator>`**, held across `transport.send`
   (`coordinator.rs:837`) for the whole of `submit`, because
   `commands.rs:982-984` calls `.lock().submit(...)`. Same for
   `Mutex<BmadCapabilityCoordinator>`.
3. `host_dispatch` itself being a synchronous `#[tauri::command]`
   (`commands.rs:247`).

The chat lane already demonstrates the fix: per-session `in_flight: BTreeMap`
(`chat.rs:212`), per-session conflict check (`chat.rs:348-365`), and
seal-under-lock / dispatch-unlocked / complete-under-lock. Applying that shape to
the two BMAD coordinators, and narrowing `ReadyWorkspaceCommitGuard` so it is not
held across a network round trip, is what delivers Phase 0's *"two task tabs run
turns simultaneously"*.

- **Guarantees:** everything currently guaranteed, unchanged.
- **Stops guaranteeing:** nothing.
- **How it fails:** it leaves I3 broken. If the ledger is later shared, Scenario A
  becomes live.

**This is not an alternative to C — it is the necessary parallel track.** C is a
correctness fix; E is the concurrency fix. Neither substitutes for the other.

---

## 5. Recommendation

**Adopt Option C.** Do it as a correctness change to close I3, explicitly not as
a Phase 0 concurrency deliverable. Do Option E in parallel and independently.

Reasoning tied to this deployment:

- **Internal org, Azure Foundry, egress accounting is the priority.** P15's
  deployment section (`:104-148`) puts accounting above execution sandboxing and
  calls region-binding non-decorative. A durable receipt trail that can assert a
  withdrawn approval (Scenario A) is a defect in exactly that priority. C fixes
  it; A, B-alone and D do not.
- **Security is not weakened.** C strengthens: the check-to-effect window goes
  from "a SQLite write wide" to zero. Every property that holds today still
  holds, and the two things it changes (bridge failure burns the decision; a
  dropped consumption under-claims) both fail closed.
- **Performance is not gutted, because it was never the cost.** The ledger lock
  is held for a `HashMap` lookup and a state write. Under C it is held for
  exactly that, once, instead of twice with a hash-comparison sequence in
  between. The dominant cost is the Azure round trip, which no version of this
  ever holds the ledger across.
- **It is small and reviewable.** One new function in `consent.rs`, one
  reordering in `coordinator.rs`, one signature in `bridge.rs`. A reviewer can
  hold the whole diff in their head — which matters more than elegance for a
  change to a consent invariant.
- **It removes the reentrancy trap.** After C there is no window in which a
  caller holds a ledger lock and might reach for the ledger again.

**What I am explicitly *not* recommending, and why:** I am not recommending
"delete the guard and rely on the outer coordinator mutex". That would be
correct today and silently wrong the first time someone shares the ledger across
lanes — which Phase 0 has a plausible reason to do. The invariant must be
enforced inside `desktop-egress`, where it is stated.

If the team rejects C's failure-ordering change, the fallback is **B + reorder**
(§4.2), which reaches the same guarantee with a two-call shape and one extra
token to lose. **D alone is not acceptable as a resting place** — it leaves I3
broken while making the code look more careful than it is.

---

## 6. Tests that would prove the replacement equivalent

Each property below is named, given a test, and given a **deliberate mutation of
the implementation that the test must catch**. A test that does not fail under
its mutation has not been shown to test anything.

### T1 — Single use survives (I1)

- *Test:* extend `tests/consent.rs:281` to N threads racing
  `consume_with_evidence` and `cancel` on one decision; assert exactly one
  `Ok(ConsumedDecision)` across all of them and that every loser is
  `DecisionAlreadyConsumed` or `DecisionCancelled`.
- *Mutation:* in `consume_if_pending`, move the `*state = DecisionState::Consumed`
  write to after the `Ok(...)` return, or delete the
  `DecisionState::Consumed { .. } => Err(...)` arm (`consent.rs:641`).
- *Must:* fail, reliably, on every run — not probabilistically. Run it under
  `--test-threads` pressure and at least 1000 iterations in CI.

### T2 — Evidence cannot be obtained without consuming (I3, forward direction)

- *Test:* a `compile_fail` doctest on `ConsentService` attempting
  `service.evidence(DecisionEvidenceInput { .. })`, and a second attempting to
  construct `ContextDecisionEvidence { .. }` literally from outside the crate.
- *Mutation:* re-add a `pub fn evidence(...)` returning evidence without a
  consumption.
- *Must:* `cargo test --doc -p desktop-egress` fails because the `compile_fail`
  block now compiles. This is the pattern already used at `consent.rs:133-144`,
  `:248-252` and `:341-354`, so it is proven in this codebase.

### T3 — Consumption cannot be obtained without its evidence (I3, reverse direction)

- *Test:* `compile_fail` doctests asserting `ConsumedDecision: !Clone` and that
  `DecisionConsumption` has no public constructor other than
  `ConsumedDecision::into_consumption`.
- *Mutation:* `#[derive(Clone)]` on `ConsumedDecision`.
- *Must:* the doctest compiles → the test fails.

### T4 — A terminal decision yields neither evidence nor consumption

- *Test:* the four cases already at `tests/consent.rs:439-540` (consumed,
  cancelled, expired, pruned) plus post-restart, rewritten against
  `consume_with_evidence`. Assert the exact error variant *and* that no partially
  constructed value is returned.
- *Mutation:* in `consume_with_evidence`, build the evidence value **before**
  matching on `DecisionState` (the natural refactoring mistake).
- *Must:* still pass on the error variants, so add a stronger assertion: a
  counter in a test-only ledger wrapper proving the evidence constructor was not
  reached. If that seam is judged too invasive, state in review that T4 catches
  the returned value but not a wasted construction — the former is what matters,
  the latter is hygiene.

### T5 — The bridge output is byte-identical to today (equivalence)

- *Test:* golden test. Capture, from the **current** code, the
  `canonical_hash("bmad-help-reviewed-context", 1, ..)` context digest and every
  field of the `MethodContextDecision` produced by
  `bridge_produces_the_locked_method_context_mapping`
  (`bridge_tests.rs:228`). Hard-code them. Assert the post-C path reproduces them
  exactly.
- *Mutation:* change `reviewed_at: evidence.issued_at()` to
  `evidence.observed_at()` at `bridge.rs:116`; or swap `consumed_at` for
  `issued_at` when populating the new owned evidence.
- *Must:* fail. This is the single most important test in the set — it is the one
  that proves *equivalence* rather than merely *correctness*.

### T6 — Every evidence getter carries the same value as before

- *Test:* the field-by-field assertions at `tests/consent.rs:403-437`, retargeted,
  plus `assert_eq!(evidence.observed_at(), consumed_at)` and
  `assert_eq!(evidence.consumption_hash(), consumption.consumption_hash())`.
- *Mutation:* populate the owned evidence from `input.decision` (the caller's
  copy) instead of the ledger's `stored` copy.
- *Must:* pass in the happy path — so this test alone is insufficient; pair with
  T7.

### T7 — Evidence reflects the *ledger's* decision, not the caller's

- *Test:* construct a `PendingContextDecision` whose content differs from the
  stored copy (the existing `stored.as_ref() != input.decision` check at
  `consent.rs:651` should reject it). Assert `DecisionBindingMismatch` and that
  no evidence is produced.
- *Mutation:* delete the `stored.as_ref() != input.decision` comparison.
- *Must:* fail. This is what stops a caller substituting a decision that hashes
  correctly in isolation.

### T8 — No durable review row without a consumption (I3, at the coordinator)

- *Test:* in `coordinator_tests.rs`, drive a BMAD Help review to the submit
  boundary, force the bridge to fail (by mutating the persisted Method session's
  authority inputs between approve and submit, so `session_authority_hash`
  diverges from the expectation), then assert:
  (a) the command errors; (b) the persisted Method session is **still version 3 /
  `ContextReviewRequired`**; (c) the decision is `Consumed`.
- *Mutation:* restore the original order (bridge → `record_context_review` →
  consume).
- *Must:* fail on assertion (b) — the session would be at version 4 with a
  `MethodContextDecision` for a decision that was never spent.
- *Honest caveat:* forcing a bridge failure deterministically may require a
  narrow `#[cfg(test)]` seam. If the reviewer rejects the seam, this property
  degrades to review-enforced, and **that must be written into the ADR rather
  than assumed** — an ordering invariant with no test is a comment.

### T9 — `Send`-ness moved where intended

- *Test:* compile-time assertions —
  `const fn assert_send<T: Send>() {}` instantiated for
  `ContextDecisionEvidence`, `ConsumedDecision`, `DecisionConsumption`; plus an
  `async fn` in a test that holds a `ConsumedDecision` across an `.await` and is
  passed to a `fn requires_send<F: Future + Send>(_: F) {}`.
- *Mutation:* add a `PhantomData<*const ()>` field, or reintroduce any guard.
- *Must:* fail to compile → test fails.

### T10 — No self-deadlock

- *Test:* run the full `submit_bmad_help_review` path on a worker thread with a
  watchdog; fail if it does not complete within a generous bound.
- *Mutation:* reintroduce a ledger guard on the evidence and call `cancel` from
  inside the bridge.
- *Must:* time out → test fails. Without this, a reentrancy regression manifests
  as a hung app, not a red test.

### T11 — Stress invariant for a future shared ledger

- *Test:* M threads × N decisions against one `Arc<MemoryDecisionLedger>`;
  assert that for every `decision_id`, `successful_consumes + successful_cancels
  <= 1`, and that the count of `DecisionConsumption` values equals the count of
  entries in state `Consumed`.
- *Mutation:* split `consume_if_pending` into a read acquisition followed by a
  separate write acquisition.
- *Must:* fail with high probability. Today this is a regression net; it becomes
  load-bearing the day the ledger is shared, and it should exist *before* that
  day.

### T12 — CI gate against the shortcut

- *Test:* a grep gate in `check-boundaries.mjs` (or `deny.toml`) failing on
  `send_guard` appearing in any `parking_lot` feature list.
- *Rationale:* the tempting way to make an `!Send` guard compile across an
  `.await` is to enable that feature. It would silently permit holding *any*
  guard — including `ReadyWorkspaceCommitGuard`'s `MutexGuard<'_, ()>` — across a
  network round trip, workspace-wide, to solve a problem in one type.
- *Mutation:* add the feature.
- *Must:* CI fails.

---

## 7. Migration order, and what must not happen concurrently

### Order

| # | Step | Crate(s) | Depends on |
|---|---|---|---|
| 0 | **Option E, part 1:** BMAD Help + capability coordinators adopt the chat lane's seal-under-lock / dispatch-unlocked shape. No egress change. | `desktop-app` | — |
| 1 | **Option E, part 2:** narrow `ReadyWorkspaceCommitGuard` so it is not held across `transport.send`. | `desktop-app` | 0 |
| 2 | **Characterisation commit.** Land T1, T5, T6, T7, T11, T12 against the *current* implementation. Land T8 as `#[ignore]` with a comment naming I3 as a known gap. Nothing changes behaviour. | tests only | — |
| 3 | **Option C in `desktop-egress`.** New `consume_with_evidence` + `ConsumedDecision`; `ContextDecisionEvidence` becomes owned; `evidence()` and `DecisionEvidenceInput` deleted; `compile_fail` doctests updated **in the same commit**. | `desktop-egress` | 2 |
| 4 | **Coordinator reorder.** `submit_bmad_help_review` becomes consume → bridge → record → advance. Bridge failure goes `Terminal`. Un-`ignore` T8. | `desktop-app` | 3 |
| 5 | Add T9, T10. | both | 4 |
| 6 | **Async `host_dispatch`.** | `desktop-app` | 1, 5 |
| 7 | *If ever:* shared `Arc<MemoryDecisionLedger>`. T11 becomes load-bearing. | both | 6 |

Steps 0–1 and 2–5 are independent and may proceed in parallel by different
people. Step 3 must not be split across releases: a `desktop-egress` version
whose `evidence()` is gone but whose `compile_fail` doctests still reference it
is a broken build, and one where the doctests lag is worse than a broken build.

### What must **not** be done at the same time

1. **Do not land C together with sharing the ledger (step 7).** Both change the
   same invariant. If a defect appears, the two are indistinguishable, and the
   safe rollback is unavailable because the halves are entangled.
2. **Do not land C together with the async migration (step 6).** The async work
   changes what must be `Send`. If a `Send` failure surfaces while consent is
   mid-change, the cheapest-looking fix is to weaken consent. Land consent first,
   with its `Send` posture explicitly asserted by T9, so the async work has a
   fixed target.
3. **Do not enable `parking_lot`'s `send_guard` feature.** Ever, for this. See
   T12.
4. **Do not touch `consume_if_pending`'s comparison logic** (`consent.rs:645-660`)
   in the same commit as C. C widens the return type; it must not adjust a single
   equality check. Any change there is a separate, separately-reviewed commit.
5. **Do not wire `TurnEgressGrant` into the BMAD lanes** while steps 3–4 are in
   flight. D7's integration list (`P15...md:418-430`) touches the same function;
   two people editing `submit_bmad_help_review`'s ordering simultaneously will
   produce a merge that compiles and is wrong.
6. **Do not delete the guard as a standalone "cleanup" commit.** It is redundant
   today, so a reviewer will approve the deletion on its own merits, and the
   result is a period in which nothing — not the guard, not the reorder —
   enforces anything on this path. The guard leaves as part of step 3 or not at
   all.
7. **Do not change `MAX_DECISION_LIFETIME_MS`, the tombstone bounds, or the
   pruning policy** during this work. C's failure mode (a) makes re-approval more
   frequent; if the lifetime changes at the same time, the two effects cannot be
   told apart in the field.

---

## 8. What this document does not settle

- **Whether the ledger should be shared at all.** Three per-coordinator ledgers
  means a `decision_id` minted by the chat lane and one minted by the Help lane
  occupy different tombstone spaces. Whether that is a gap depends on whether ids
  can collide across lanes, which turns on the `derived_contract_id` domains
  (`"decision"` in both `chat.rs:425` and `coordinator.rs:528`). **That deserves
  its own investigation** and is a prerequisite for step 7.
- **Whether `region` survives a looping turn.** P15 D7 flags that
  `TurnEgressEntry` has no `region` field (`turn_grant.rs:52-57`), so the
  data-residency claim does not hold across a multi-call turn. Unrelated to this
  change, but it is the other half of the same priority.
- **The BMAD architecture coordinator** (`state.rs:121`) was not audited for
  consent usage beyond confirming it constructs no evidence.
- **Empirical contention.** No profiling was done. The argument that the ledger
  lock is not a bottleneck rests on static reading — the lock is held for a
  `HashMap` operation, never across I/O — not on measurement. If someone
  disagrees, the way to settle it is a measurement, not an argument.

---

## 9. Note on the worktree

`C:\tmp\forge-v2` is on branch **`codex/forge-v2`**, not `main`, with
uncommitted modifications across `desktop-app`, `desktop-egress`,
`desktop-runtime`, `desktop-store` and the P15 packet, plus untracked
`turn_grant.rs`, `chat_tools.rs` and `ADR-0010`. Every line reference in this
document is to that working tree state. Confirm the branch before implementing.
