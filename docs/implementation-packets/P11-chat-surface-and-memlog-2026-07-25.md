# Implementation packet: P11 — chat as the default lane, and the memlog primitive

## Authority and intent

- Owning authority: repository maintainer (RodrigoSousa0), continuing the BMAD
  rework through steps 4 and 5 of the
  [integration redesign](../reviews/bmad-integration-redesign-2026-07-25.md).
- Product intent: Forge is the BMAD runtime — practise the method without a
  separate LLM runtime. **Chat is the surface BMAD runs on, and BMAD must never
  replace chat.**
- Predecessors: [P9](P9-architecture-state-machine-2026-07-25.md) (ordered
  architecture progression), [P10](P10-verbatim-method-adoption-2026-07-25.md)
  (verbatim adoption + workflow graph).

## The defect this closes

Before this packet, **every message the user typed was routed to the BMAD Help
advisor**. `sendBmadMessage` called `createBmadHelpRun`, and the reply was a
routing recommendation card — or, for an ordinary question, *"No recommendation
— no catalog evidence matched the reviewed intent."* The generic
`session.create` / `task.submit` lane was a hard host stub, and there was no
chat, message, turn, or conversation type anywhere in `desktop-runtime` or
`desktop-store`.

So BMAD was not optional; it was the only path, and that path answered the
wrong question. The method's own source says otherwise
(`bmad-agent-architect/SKILL.md`):

> "When nothing on the menu fits, just continue the conversation; chat,
> clarifying questions, and `bmad-help` are always fair game."

## Step 4 — the chat surface

### Renderer

- `lib/hostClient/chatProtocol.ts` validates every reply: bounded multi-line
  message text, a closed role vocabulary, an agent code restricted to the six
  sealed Method agents, and — for history — the strict `user -> assistant`
  alternation the host guarantees. A transcript the host could not have
  produced is rejected rather than displayed.
- `state/chatSessions.ts` gives every task its own transcript, with the same
  routing discipline as the capability store: a reply captured under a task's
  generation is dropped if that task was superseded or closed, so a slow answer
  can never land on the tab that happens to be visible.
- `TaskWorkspace` renders the conversation; `AgentSelector` became a real
  selector.

### The product contract, now pinned by tests

- The composer sends **chat** by default. `run.create`, `bmad.help.*`, and
  `bmad.capability.*` are all asserted *absent* for an ordinary message.
- **"No agent — plain chat" is always the first entry** in the agent menu, so
  BMAD can never become mandatory again by drift.
- Choosing an agent binds that agent's sealed persona to the conversation and
  carries it on subsequent turns; the transcript is never rewritten.
- BMAD Help is now an **explicit** action ("what should I do next?"), not what
  Send does.

### Two real defects the tests caught

1. **No re-entry guard.** A double-click sent the same message twice; the
   host's strict alternation would reject the second and lose it. Fixed in the
   send path *and* by disabling the composer while a turn is in flight — the
   code was fixed, not the test.
2. **Stale session binding.** The Home composer creates a task and sends in the
   same tick, so `selectedSessionId` had not settled and the exchange landed on
   the previous task's lane. `sendChatMessage` now binds to an explicitly named
   session (`routingFor`), which is the id `startNewSession` just returned.

## Step 5 — the memlog primitive

`crates/desktop-runtime/src/bmad/memlog.rs` implements the method's working
memory, faithful to `src/scripts/memlog.py`:

- **Append-only and chronological.** No edit, no delete — the type exposes
  neither. A backwards clock fails closed.
- **No lifecycle status.** `status`, `state`, `complete`, and `completed` are
  *rejected* as frontmatter keys; terminal moments are `event` entries, exactly
  as upstream requires.
- **One dense line per entry**: whitespace runs collapse, matching upstream's
  `" ".join(args.text.split())`.
- **The upstream inline tag** renders exactly: `(idea)`, `(idea by user)`,
  `(by coach)`, or nothing for a plain note.
- `render()` produces the `.memlog.md` document a real run would leave behind.

### P9 refactored onto it

`BmadArchitectureSession::to_memlog()` projects the ordered progression as an
upstream-shaped memlog: an `event` per step, one `decision` per section
carrying its stable `AD-n`, `question` entries for what was not settled, and a
terminal `spine finalized` event. Deriving is read-only and repeatable — the
same session renders an identical log twice.

This is the bridge from the bespoke eight-step machine to the method's own
mechanic: **the spine is derived from an accumulating record, not edited in
place.**

## Verification record

| Check | Result |
|---|---|
| `cargo test --workspace --all-features` | **645 passed**, 0 failed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Clean |
| `cargo fmt --check` | Clean |
| Renderer `vitest run` | **433 passed** / 40 files |
| `pnpm typecheck` | Clean across all six workspace projects |
| `pnpm bmad:foundation:verify` | 116/116 |
| `pnpm contracts:verify:typescript` | 85 passed |
| `pnpm secrets:check` | Clean across 3,911 files |
| `pnpm verify:boundaries` | Green |

## What is still not done

Named plainly, because none of it is claimed above:

1. **The workflow graph is not wired to the host.** `BmadWorkflowGraph` parses
   the sealed CSV and computes state correctly (9 tests), but no command calls
   it, so the deterministic "where you are in the method" panel does not exist
   yet. It needs the config resolver (below) to resolve output locations first.
2. **`BmadConfigResolver` is still idle.** It implements upstream's four-layer
   merge and is used by nothing outside its own tests.
3. **The memlog is not yet persisted or wired to a run folder.** It is a
   verified runtime primitive; no capability writes one to a workspace through
   D3 yet, and `to_memlog()` is a projection rather than the session's storage.
4. **Chat history is in-memory and per-process**, like the coordinators it sits
   beside. `chat.history` returns the host's live transcript; a restart loses
   it.
5. **Menu dispatch from conversation** — typing "let's architect this" does not
   yet dispatch the CA menu item the way an upstream agent would.
6. **The remaining core skills are adopted but not activated.** `bmad-spec`,
   `bmad-advanced-elicitation`, and the three reviewer lenses now ship as sealed
   source (P10) but have no runtime lane.
