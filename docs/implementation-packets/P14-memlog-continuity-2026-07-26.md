# Implementation packet: P14 — memlog continuity for document capabilities

## Authority and intent

- Owning authority: repository maintainer (RodrigoSousa0).
- Closes the last open item from the BMAD fidelity work: PRD, product brief and
  UX declare that they do not write a memory log, while upstream treats the
  memlog as the thing those artifacts are *derived from*.
- Status: **plan only.** Nothing here is implemented.

## The gap, stated exactly

Upstream `memlog.py` describes the mechanic:

> A memlog is the dense, chronological record of everything that mattered in a
> piece of work … It persists ACROSS sessions, so a fresh session can load it
> and continue. It is NOT a deliverable; downstream artifacts (a brief, a PRD,
> a deck, a report) are *derived* from it on demand.

`bmad-spec` is blunter: `SPEC.md` is "DERIVED from `.memlog.md`, never
hand-edited". `bmad-architecture` says the spine "is **distilled from the
memlog at the end**, not written as you go".

Forge today:

| | State |
|---|---|
| `Memlog` runtime type, 15 entry kinds, frontmatter rules | Implemented, tested |
| `bmm:bmad-architecture` | Emits `.memlog.md` as a proposed run file |
| PRD, product brief, UX, and every other document capability | No memlog at all |
| Any memlog surviving a process restart | **None** |

The last row is the one that makes this more than a projection edit. The
architecture progression lives in
`BmadArchitectureCoordinator { active: Option<BmadArchitectureSession> }`
(`crates/desktop-app/src/bmad_architecture.rs:174`) — process-local, with no
row in `desktop-store`. A restart loses it. Upstream's central claim for the
memlog is precisely that it survives one.

## Design decision: do not copy the architecture step list

The obvious move — give PRD and friends a fixed ordered progression like
`BmadArchitectureStep` — would repeat a mistake this branch already corrected.

Upstream prescribes no fixed step list for architecture either; it asks the
spine to sweep "the breadth the altitude owns". Forge's nine steps are a
host-owned reading of that breadth, and `BmadArchitectureStep`'s own doc now
says so. Upstream's PRD, brief and UX are explicitly *coached conversations*
with create / update / validate intents — not step machines. Inventing a step
ladder for them would manufacture a second false structure.

**The unit of continuity is the doc workspace, not the step.** Upstream binds a
memlog to `{doc_workspace}` — a run folder — and multiple runs (create, then
update) append to the same log. That maps onto Forge without inventing
anything: several capability runs, over time, against one workspace and one doc
workspace key.

## Phases

Each phase is independently landable and independently verifiable. Phase 0 and
Phase 1 are worth doing even if the rest is deferred.

### Phase 0 — dispatch on the sealed entrypoint kind, not a hard-coded id

The host decides a run is stepwise with
`is_architecture_capability(capability_id)` — a literal comparison against
`"bmm:bmad-architecture"`, at `commands.rs:1561`, `:1731`, `:1880`. Meanwhile
the foundation already records `entrypointKind: "step_jit"` per skill
(`bmad_foundation.rs:417`, projected at `desktop-ipc/src/bmad.rs:151`) and the
host never reads it for dispatch.

Route on the sealed `entrypointKind` instead. Nothing behavioural changes today
— architecture is still the only `step_jit` record — but the special case stops
being something the next multi-turn capability has to copy.

*Verify:* existing architecture tests pass unchanged; add one asserting that a
capability whose sealed record is not `step_jit` never reaches the stepwise
coordinator.

### Phase 1 — durable, append-only memlog storage

New store migration **v12 → v13**, one table:

```
bmad_memlog_entries(
  entry_id      TEXT PRIMARY KEY,
  workspace_id  TEXT NOT NULL,
  doc_workspace TEXT NOT NULL,   -- e.g. 'docs/architecture', 'docs/prd'
  sequence      INTEGER NOT NULL,-- append order within (workspace, doc_workspace)
  kind          TEXT NOT NULL,   -- MemlogEntryKind::code()
  by            TEXT,            -- 'user', 'coach', an agent code, or NULL
  text          TEXT NOT NULL,
  recorded_at_ms INTEGER NOT NULL,
  UNIQUE(workspace_id, doc_workspace, sequence)
) STRICT;
```

Append-only by construction: an `append_memlog_entry` that takes the next
sequence inside the same transaction, and **no update or delete API** — mirroring
upstream's "There is no edit or delete subcommand by design; history is never
rewritten."

Two obligations that are easy to forget and are both already guarded:

- Add `("memlog_entries", "bmad_memlog_entries")` to `RETENTION_CATEGORIES`
  (`desktop-store/src/lib.rs`). The test
  `retention_manifest_discloses_every_user_content_table` cross-checks the
  disclosure list against `V12_TABLES` and **will fail** if this is skipped —
  that guard exists because this list had already fallen behind two migrations.
- `V13_TABLES` must be an exact-match set, like every prior migration.

*Verify:* migration test from a seeded v12 store; append ordering under
concurrent writers; the retention guard; and a legacy-simulation test in the
existing style.

### Phase 2 — write the log

On a completed capability run whose archetype is `document_artifact`, append:

- one `event` entry naming the capability and intent (create / update / validate);
- one `decision` per accepted artifact section;
- one `question` per open question the artifact carries;
- `change` and `override` entries when an update supersedes an earlier decision
  — the two kinds that make the log an audit trail, added to the vocabulary in
  this branch and currently unused.

**Honest limitation to record in the packet and the code comment:** upstream
appends "as each lands", during the conversation. Forge writes at the end of a
single reviewed exchange, so within one run the log is a projection of the
artifact rather than its source. The continuity benefit is real *across* runs;
the within-run ordering is not upstream's. Do not describe this as parity.

Architecture keeps emitting its `.memlog.md` run file; Phase 2 additionally
persists its entries so the progression survives a restart, which is the fix
for the in-memory coordinator noted above.

### Phase 3 — read the log, so the artifact is derived from it

The half that makes it the mechanic rather than a diary.

When preparing a `document_artifact` run whose doc workspace already has
entries, render them with the existing `Memlog` type and inject the result as
**one reviewed context candidate**, exactly as the persona and sealed
instruction candidates are injected today (`bmad_capability_host.rs:1030`).

Budget: the capability ceiling is `MAX_CAPABILITY_CONTEXT_ITEMS = 104`, sized in
this branch to cover 100 attachments plus host-injected candidates. The memlog
adds a fifth injected item, so raise it to 105 and update the comment — the cap
must stay at or under `desktop-egress`'s `HARD_MAX_CONTEXT_ITEMS = 128`.
Getting this wrong is exactly how the chat lane was dead on arrival, so the
existing "a full context load seals" test pattern should be repeated here.

Bound the rendered log (suggest 32 KiB, matching `RENDERED_STEP_LIMIT`) and, on
overflow, include the most recent entries with an explicit truncation notice
rather than silently dropping history.

### Phase 4 — projections and disclosure

Once Phases 1–3 land, `prd`, `product-brief` and `ux-design` stop disclaiming
the memlog and instead state the real contract: this run appends to the doc
workspace's log, and a later run is derived from it. Requires the standard
reseal cycle — `reseal.mjs`, the hand-applied semantic lock, and the Rust
`EXPECTED_*` constants in `bmad_foundation.rs`.

Do **not** touch `bmad-help.instructions.md` as part of this: its exact byte
length is pinned in five places across two crates by design.

## Risks

| Risk | Handling |
|---|---|
| Retention disclosure silently falls behind again | Already guarded; the test fails closed. Do not weaken it to land the migration. |
| Context budget overflow surfaces as an opaque integrity error | Raise the ceiling in the same commit as the injection, with a full-load seal test. |
| The log becomes an unbounded store of user prose | Bound per entry and per doc workspace; decide a retention story before Phase 2, not after. |
| Doc-workspace key chosen ad hoc per capability | Fix the mapping in one place next to `ARCHITECTURE_RUN_FOLDER` (`docs/architecture`), so keys cannot drift per call site. |
| Phase 1 lands without Phase 3 | Acceptable only briefly: storage without a reader adds a privacy surface and delivers no continuity. If Phase 3 will not follow, do not ship Phase 1. |

## Out of scope

- Making PRD, brief or UX genuinely multi-turn *within* one run. That is a
  larger conversation-state feature; this packet delivers continuity across
  runs, which is where upstream's value actually sits.
- `uv run memlog.py`, run folders bound by config, and resume-by-reload: all
  depend on execution authority ADR-0003 withholds.

## Definition of done

- A PRD run in a workspace that already has brief entries receives them as
  reviewed context, and its own decisions are appended.
- ~~The architecture progression survives a process restart.~~ **Corrected
  during implementation.** This contradicts *Out of scope*, which already
  excludes resume-by-reload. The deliverable is the weaker, real one: a
  progression's *decisions* survive a restart and are re-injected as context.
  Restoring the step cursor needs durable progression state — see below.
- `app.offboarding.inspect` counts memlog entries.
- Full gate suite green, including the retention guard and a full-context seal
  test for the raised capability ceiling.

## Outcome — implemented 2026-07-26

All four phases landed. Full gate green: 693 Rust tests, 426 renderer tests,
116 foundation tests, `cargo clippy -D warnings` and `cargo fmt --check` clean,
boundaries and secret scan clean.

| Phase | What landed |
|---|---|
| 0 | Dispatch routes on the sealed `entrypointKind`; `is_architecture_capability` deleted (it had no production caller left). |
| 1 | Store v13 + v14: `bmad_memlog_entries`, append-only (no update, no delete), sequence allocated inside an `IMMEDIATE` transaction, monotonic within a doc workspace; retention disclosure raised to 21 categories. |
| 2 | `finalize_dispatch` projects a completed `document_artifact` into `event` / `decision` / `question` / `change` / `override` entries. |
| 3 | `prepare` injects the rendered log as one reviewed candidate; capability ceiling 104 → 105 with a full-load seal test. |
| 4 | `prd`, `product-brief` and `ux-design` state the continuity contract; foundation resealed. |

## Audit — 2026-07-26, after the first implementation

Re-reading the implementation against this packet turned up five defects, all
introduced by the first pass and all now fixed with regression tests.

| # | Defect | Fix |
|---|---|---|
| 1 | **A long section body failed the run.** A section body is bounded at 32 KiB and a memlog entry at 4 KiB. The projection passed the body through raw, the store rejected the append, and the error propagated out of `finalize_dispatch` — failing a run whose result was *already durably stored*. An ordinary long PRD section triggered it. | The projection condenses each section to one entry-sized line, which is also upstream's shape ("one line each, the gist"). Recording memory is now infallible: a full log, or a store fault, loses continuity for that run and nothing else. |
| 2 | **Headings containing `": "` were mis-attributed.** `change`/`override` matching recovered the heading by splitting the rendered text on its first separator, so `Phase 1: Discovery` matched as `Phase 1`. | The store carries an explicit `subject` column; matching never re-parses text. |
| 3 | **A withdrawal re-emitted its `override` on every later run**, growing an unbounded tail of identical entries from one dropped heading. | Prior entries are *replayed* into live decision state — a `decision` records, an `override` withdraws — so a withdrawn subject is no longer live and cannot be withdrawn twice. |
| 4 | **Context rendering was a second copy of the formatting rules**, free to drift from the `.memlog.md` a run proposes. The packet asked for the existing `Memlog` type and the first pass did not use it. | Rendering goes through `Memlog::render`. That needed `MemlogEntryKind::from_code`, and needed the store to enforce `Memlog`'s monotonic-clock rule so a stored log always rebuilds. |
| 5 | **v13 was edited in place** to add `subject`, which would leave any store that had already run v13 disagreeing with its own version number. | v13 is treated as immutable; `subject` arrives in a separate **v13 → v14**, with a test that upgrades a real v13 store and keeps its rows. |

### The three limits, restated after the audit

**Architecture progression across a restart.** Unchanged, and now stated
correctly in *Definition of done* above. Decisions survive and are re-injected;
the cursor is not restored. `BmadArchitectureSession::current_step` is
`ORDERED.get(steps.len())`, so restoring it means rebuilding `steps` — and that
needs a durable session boundary, because one workspace can hold several
progressions over time and the accepted runs alone cannot say where the current
one began. It is a genuine feature (durable progression state), not a wiring
gap, and it carries a review question this packet should not answer quietly:
resuming means acting on consent given in a previous session. Left for a
follow-up packet.

**`change` vs `override` record structure, not intent.** Still true, but no
longer *unreliable*: defects 2 and 3 meant the distinction was also frequently
wrong. It is now exact — a revised heading is a `change`, a heading the run
stopped carrying is an `override`. What remains is only that a reviewed
document diff carries no overrule signal, so neither kind claims anyone meant
to overrule anything. That is a property of the input, not something more
implementation could recover.

**Within one run the log is a projection of the artifact.** Inherent: Forge
writes once, at the end of a single reviewed exchange, where upstream appends
as each decision lands. Closing it means making these capabilities multi-turn
within a run, which *Out of scope* excludes. The continuity benefit is across
runs and is real; this is not parity and is not described as such anywhere in
the code or the sealed projections.

**Intent is `create` or `update`, never `validate`.** The packet asked for all
three. The host can tell a first run against a folder from a later one; nothing
on a reviewed run distinguishes a validation pass from an update, so it is not
claimed rather than guessed.

### Consent is over the sealed manifest, not itemised

`CapabilityReviewWire` discloses the manifest hash, not the list of context
items. Every candidate has always worked this way, so the memlog is no worse
than the rest — but it is the first candidate that is *host-injected from prior
runs* rather than chosen by the user, so approving it means approving content
the user did not select and cannot enumerate at the review. The memlog is
secret-scanned like any other candidate and is bounded, so nothing unbounded or
unscanned reaches egress; what is missing is disclosure. Itemising the review
projection is a change to the consent surface and belongs in its own packet.

### Known overlap

For the stepwise capability the persisted log overlaps its in-session decision
log; the persisted one is the superset. Both are injected and both are counted
by the ceiling. Collapsing them is a change to the architecture lane that this
packet deliberately did not make.
