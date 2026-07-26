# BMAD implementation — deep-dive audit and closure of the ordered lane

**Date:** 2026-07-25
**Candidate:** `codex/forge-v2` worktree, dirty tree on top of `453be1a3`
**Scope:** the BMAD implementation only — foundation, closure denominator,
capability lane, semantics, and the renderer surface that presents them.
**Companion documents:**
[comprehensive audit](forge-v2-comprehensive-audit-2026-07-23.md),
[fidelity matrix](bmad-capability-fidelity-matrix.md),
[P9 packet](../implementation-packets/P9-architecture-state-machine-2026-07-25.md).

## Verdict

The BMAD implementation was **complete except for one thing, and that thing was
load-bearing**: the runtime had no ordered execution lane, so the single
capability whose sealed guidance requires one was fail-closed, and the sealed
Help binding recommended an action the product could not perform.

That gap is now closed and verified. Everything else the prior audits flagged
as "not finished" on the BMAD side is either (a) done, (b) an honest labelling
decision that must *stay* a labelling decision, or (c) blocked on external
infrastructure that no coding session can close.

## What the audit found

### The foundation already knew

Of the 40 managed outputs in the sealed foundation, exactly one carries
`entrypointKind: "step_jit"`: `runtime/method/6.10.0/architecture-create.instructions.md`.
Every other projection is `direct`, `inline`, or `null`. The source-intake
process had recorded, at intake time, that this one capability is stepwise.

The managed instruction says so in prose too: work from "the current
host-selected step", "do not skip, repeat, invent, or finalize a step", "the
host validates progression" — and closes with "the capability remains
unavailable until its later state-machine packet".

The runtime had no such packet. So `reviewed_method_menu_bindings` filtered the
Architect/CA path out, `runtime_capability_enabled` returned false for a
crafted direct request, and the UI badge read `Unavailable`. That was the
**correct** fail-closed behaviour — offering the menu through the one-shot lane
would have contradicted the capability's own sealed instruction — but it left
three visible consequences:

1. 25 of 26 reviewed menu paths were runnable; the denominator never closed.
2. The fidelity matrix recorded "Faithful upstream execution: 0" with no
   ordered lane anywhere in the product.
3. The sealed Help binding's `Create Architecture` recommendation pointed at a
   disabled capability — a contradiction between sealed data and runtime state
   that could not be fixed by editing the sealed data.

### What was already sound

These were checked and needed no work:

- **Capability identity is cryptographic, not conditional.** Capability id is
  carried in the egress purpose label (`bmm:x` → `bmm.x`), which is folded into
  the context manifest hash and the invocation binding. A manifest, decision, or
  output minted for capability A cannot be replayed against capability B — it
  fails at consent, not at a business-logic branch.
- **Untrusted output has one door.** All model JSON enters through
  `parse_capability_result`, which enforces exact key sets at every nesting
  level; an unexpected field is treated as authority smuggling and rejected
  rather than ignored. Duplicate decoded keys fail closed at the envelope,
  verifier, and stored-result reparse boundaries.
- **Builder output is genuinely inert.** Activation, registration, command,
  hook, and network fields in a draft are all rejected with red tests proving
  it.
- **The renderer holds no authority.** It receives typed projections and
  validates each one; it cannot construct a capability id the host did not
  project.
- **The lock discipline is right.** The coordinator mutex is released across
  the model round trip and re-acquired only to record the verified result, with
  a dispatch nonce rejecting a superseded finalize.

### What was fixed

**The ordered progression now exists** — see the
[P9 packet](../implementation-packets/P9-architecture-state-machine-2026-07-25.md)
for the full design, tests, and rollback.

The essential choice: the progression **rides the existing generic lane** rather
than introducing a second one. An architecture run is a *sequence of ordinary
reviewed capability runs*. Nothing about egress, consent, verification, or
durability changed. The host contributes exactly four things — step selection,
step context (which therefore lands inside the manifest hash the user
approves), progression validation, and a local distillation that contacts no
model at all.

Consequences:

| Before | After |
|---|---|
| 25 of 26 Method menu paths runnable | **26 of 26**; 24 of 24 unique capabilities active |
| Architecture badge: `Unavailable` | Badge: `Stepwise`, with the caveat inline |
| Help recommends a disabled action | Help recommendation is actionable |
| No ordered lane in the product | One ordered lane, verified end to end |

The rejected alternatives are worth recording, because each looked cheaper:

- **Re-mint the sealed projection** to delete the "remains unavailable"
  sentence. Rejected: that sentence states a *condition*, and delivering the
  packet satisfies it. Rewriting sealed bytes would have cascaded the foundation
  hash chain across five pin sites for no semantic gain.
- **Reuse the durable `MethodSession` / `MethodStepTable` engine.** Rejected:
  that engine is welded to Help's sealed `MethodExactBinding`. Architecture has
  no sealed exact binding, so reuse would have meant inventing binding data in
  a provenance-gated codebase.
- **Add a fourth result archetype** for step output. Rejected: it would have
  meant regenerating cross-language contracts, a store migration, and new IPC
  commands across five catalog pin sites — to express something the existing
  bounded document archetype already carries exactly.
- **Advance the state machine inside the output verifier.** Rejected: it would
  make verification side-effecting and create a coordinator↔session lock-order
  inversion — precisely the bug class the 2026-07-24 session spent itself on.

## What is deliberately still open

These are honest limits, not oversights. Each is stated in the fidelity matrix
and the P9 packet.

1. **Fidelity is still not upstream fidelity — for any capability.** The
   ordered lane is a real state machine, but it is nine reviewed one-shot
   steps, not the upstream coaching dialogue with its reviewer-gate
   subagents and web verification. (The memlog half of this gap closed in
   P14, 2026-07-26; what remains is that entries are written at the end of an
   exchange rather than interleaved with a conversation.) The matrix still reports **faithful
   upstream execution: 0**, and the UI says "Stepwise", never "faithful".
   Availability is not fidelity. Upgrading that claim would require tool
   intents (`web`, `subagent`, `file_write`, `process`) that ADR-0003 blocks by
   design — so this is a standing product boundary, not a backlog item.
2. ~~**The progression is in-memory.**~~ **Closed (P14, 2026-07-26.)** The
   live session is still in-memory, but its *boundary* is durable — which
   session a workspace has open and the ordered runs that answered its steps —
   so a restart resumes at the step it interrupted. Resuming re-reads each
   recorded step from its own capability run and re-parses it through the same
   strict constructor that accepted it, and fails closed if any of it no longer
   verifies. The generic coordinator remains non-durable between prepare and
   submit.
3. **The Help catalog is still narrower than the enabled denominator** — two
   installed skills and two actions against 24 unique capabilities. The
   Architecture half of that conflict is resolved; the breadth half is
   unchanged and belongs to the production model-backed Help milestone, since
   the binding is hash-verified sealed data.
4. **`bmadHelp` state is still global per-app, not per-task.** The 2026-07-24
   session proved a render-gate insufficient; correct isolation means threading
   per-task state through all 27 `applyBmadEvent` sites. Untouched here — it is
   a renderer state-ownership milestone, not a BMAD-semantics gap.
5. **Production transport is still not composed.** Capabilities run on the
   direct-development, deterministic, or offline transport. Deploying the
   support API does not make any advertised capability production-operational.
   This is the largest remaining BMAD-adjacent gap and it is blocked on the
   Windows identity broker and the deployed round trip, not on code here.
6. **The readiness scorecard was not touched.** `full_bmad_breadth` still reads
   35% against a stale `sourceRevision`. Its `menu_path_matrix` evidence kind
   requires produced CI evidence records; editing the percentage without them
   would fabricate readiness. It should be regenerated by the evidence pipeline,
   not by hand.

## Verification record

| Check | Result |
|---|---|
| `cargo fmt --all --check` | Clean |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | Clean |
| `cargo test --workspace --all-features --locked` | **613 passed**, 0 failed |
| — of which `desktop-app` | 130 passed (was 120 before this work) |
| — of which new progression cases | 12 `desktop-runtime` + 7 host + 4 command-lane (one replaced the old "stays deferred" pin) |
| Renderer `vitest run` | **427 passed** / 39 files (was 415 / 39) |
| `pnpm typecheck` | Clean across all six workspace projects |
| `pnpm verify:boundaries` | Green |
| `pnpm bmad:foundation:verify` | 116/116; sealed hash **unchanged** |
| `pnpm contracts:verify:typescript` | 85 passed, 0 failed |
| `pnpm secrets:check` | Clean across 3,676 first-party files |
| Full `pnpm verify:source` | **Not run** — environment is Node 24.11.1 against the pinned 24.18.0, so `toolchain:check` fails first and later steps need Docker |
| Azure / signed installer / pilot evidence | **Not run** — unchanged external lanes |

## Release decision

This closes the first BMAD checkpoint the 2026-07-23 audit named. It does
**not** change that audit's release verdict: the candidate is still blocked for
production-cloud or pilot readiness on delegated authentication, a trusted
production image publisher, the composed native round trip, and signed-installer
evidence. None of those are BMAD gaps.
