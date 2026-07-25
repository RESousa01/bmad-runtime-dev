# Implementation packet: P12 — deterministic navigation, menu dispatch, run folders, durable chat

## Authority and intent

- Owning authority: repository maintainer (RodrigoSousa0), closing the six
  items left open by [P11](P11-chat-surface-and-memlog-2026-07-25.md).
- Predecessors: [P9](P9-architecture-state-machine-2026-07-25.md),
  [P10](P10-verbatim-method-adoption-2026-07-25.md), P11.

## What landed

### 1 + 2. The workflow graph, wired — and the config resolver it needed

`BmadConfigResolver` had been implemented, tested, and used by nothing. It is
now the front half of the deterministic navigation lane.

- **`toml_subset.rs`** — a bounded TOML reader for the four `config.toml`
  layers and every `customize.toml`. These live in the *user's workspace* and
  are untrusted, so it reads exactly the subset the method uses and **fails
  closed on anything else**: an unrecognised construct is an error, never a
  silent skip. Adding a general-purpose TOML dependency would have been a
  supply-chain change, which is not a call to make unilaterally.
  Tests parse the **real sealed** `customize.toml` files, including
  `[[agent.menu]]` arrays-of-tables and multi-line arrays.
- **`bmad_workflow.rs`** — resolves the four central layers through the
  reviewed resolver (a malformed override is recorded *invalid*, not partially
  accepted), resolves each step's `output-location` variable, scans it through
  the governed broker, and matches artifacts to steps.

  The matching rule is written down rather than left to judgement: an
  artifact's normalized file name must contain **every** significant token of
  the step's `outputs` description. `outputs = "epics and stories"` matches
  `epics-and-stories.md` but **not** `epics.md`. A step with no declared
  artifact stays incomplete rather than being guessed complete.
- **`bmad.workflow.state`** — one new command across all six catalog pin sites
  (45 → 46), plus `BmadWorkflowPanel` and its protocol.

  The panel shows the *evidence path* behind every completion, because that is
  **why** a step reads complete; and when a config variable is undefined it
  says so instead of guessing.

Proven end to end (`workflow_state_is_computed_from_config_and_artifacts_without_a_model`):
an empty workspace reports "no install"; writing `_bmad/config.toml` plus a PRD
and a spine advances the phase to solutioning and the next gate to
epics-and-stories — **with no model call at any point**.

### 5. Menu dispatch from the conversation

`lib/menuIntent.ts` implements what the method's own activation describes:
> "If the user's initial message already names an intent that clearly maps to a
> menu item … dispatch that item directly."
> "When nothing on the menu fits, just continue the conversation."

So it is deliberately conservative — an explicit menu code, or every
significant word of the label. **Two matches means no dispatch**: an ambiguous
intent continues the conversation rather than hijacking it. It grants no
authority; it can only select a menu the host already projected with a
host-validated `capabilityId`.

### 3. The memlog written through D3

A finalized architecture progression now renders the two files a real
`bmad-architecture` run leaves behind — `ARCHITECTURE-SPINE.md` and the
append-only `.memlog.md` it was **derived from** — into one run folder. They go
out through `bmad.capability.propose_changes`, so they enter the **same D3
review as any other proposed edit**. No new command, no write outside review.

### 4. Durable chat transcripts

Store schema **v11 → v12**: `chat_sessions` + `chat_messages`. Message bodies
are encrypted content-addressed payloads — the same protection a capability
result gets — while the transcript's shape stays in ordinary rows so it reads
back in order without decrypting everything.

The store enforces the runtime's own invariant: strict `user → assistant`
alternation, opening with a question. A write that would break it is rejected,
so **a corrupt transcript cannot be reconstructed from disk**. Rehydration is
equally strict — a stored transcript that fails the runtime's checks is dropped
rather than half-loaded.

All eight legacy-migration simulations across four test files were extended so
the v3/v7/v8/v10 upgrade paths still prove equivalence with a fresh schema.

## Two defects the tests caught (code fixed, not tests)

- **Ambiguous headings.** The workflow panel rendered "Solutioning" as both the
  current-phase heading and a phase-group heading. Qualified to "Current phase:
  …" — a real screen-reader ambiguity, not a test artifact.
- The P11 re-entry and stale-session defects remain fixed; no regressions.

## Verification record

| Check | Result |
|---|---|
| `cargo test --workspace --all-features` | **665 passed**, 0 failed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Clean |
| `cargo fmt --check` | Clean |
| Renderer `vitest run` | **453 passed** / 43 files |
| `pnpm bmad:foundation:verify` | 116/116 |
| `pnpm verify:boundaries` | Green |
| `pnpm typecheck` | Clean |

## 6. Core-skill runtime lane — blocked, with the reason

The adopted `core-skills` (`bmad-spec`, `bmad-advanced-elicitation`, the three
reviewer lenses, and the rest) ship as sealed source from P10 but still have no
runtime lane, and **could not get one in this environment**.

The clean implementation needs a `core:` capability-id prefix. That id shape is
constrained in three places, one of which is a **contract schema**
(`bmad-capability-run.schema.json`, `^(bmm|builder):…`) whose Rust, TypeScript,
and C# bindings are generated and checked in. Changing it requires
`contracts:generate:cross-language`, which refuses to run:

```
CONTRACT_LOCK_BOOTSTRAP_UNREVIEWED: pnpm bootstrap lock hash changed
```

`schema-lock.json` records **reviewed** supply-chain hashes for
`pnpm-lock.yaml` and `Cargo.lock`. Both differ from their recorded values —
**pre-existing drift from the in-flight Forge cutover**, not from this work
(both were already modified at session start). Clearing it means re-marking
those locks `reviewed`, which asserts a supply-chain review of the current
dependency set. That is a review I have not performed, and re-blessing it
silently would convert a deliberate human checkpoint into a rubber stamp.

The rejected shortcut: giving core skills `bmm:` ids. It would compile and pass
every gate, and it would be **false** — it misstates the upstream module a
capability came from, in the one ledger whose entire purpose is provenance.

**To unblock:** review the current dependency set, update the two bootstrap
lock hashes in `packages/contracts/schema-lock.json`, then the `core:` prefix,
the closure-ledger records, and the capability-table bindings are a
straightforward slice — the instruction bodies are already shipped and pinned.

## Also still open (unchanged, honestly scoped)

- The memlog is a projection of the architecture progression rather than the
  storage every skill run appends to. Generalising it is the step that would
  let P9's bespoke state machine retire entirely.
- Chat history is durable but not yet pruned or offboarded; retention policy
  for transcripts is unaddressed.
- Production capability transport, Azure, signed installer, pilot evidence —
  all unchanged and outside a coding session.
