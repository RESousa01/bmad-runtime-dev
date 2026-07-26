# ADR-0009: Forge is a general coding agent; the BMAD Method is its interface

- Status: accepted (2026-07-26)
- Supersedes: [ADR-0001](ADR-0001-windows-local-authority.md) decision paragraph 2
  (the "no callable shell, terminal, command runner, test runner, package
  script, hook, or child-process primitive" exclusion) and the E3-containment
  precondition attached to it.
- Amends: [ADR-0003](ADR-0003-p5-breadth-denominator.md) — the *executable
  exclusion* on adopted upstream **files** stands unchanged; it never governed
  the host's own execution authority, and this ADR does not touch it.
- Unchanged: [ADR-0002](ADR-0002-independent-d2-d3-grants.md) grant epochs,
  [ADR-0007](ADR-0007-model-to-governed-edits.md) governed-edit review,
  [ADR-0008](ADR-0008-verbatim-method-adoption.md) verbatim adoption, and the
  sealed-foundation hash chain.

## Context

ADR-0001 froze Forge as a read-and-propose host: the model could produce
documents and change sets, and a human applied them through the D3 review. That
made the product safe and inert, and it is why the capability fidelity matrix
has to label 22 of 24 Method capabilities "Adapted (one-shot)" — upstream BMAD
workflows are coached, iterative, and tool-using, and a host that cannot read a
file on its own initiative, run a test, or search a tree cannot run them.

The product intent has changed. Forge is to be a general coding agent of the
same class as OpenCode — able to explore a repository, run commands, and edit
files as part of a conversation — **differentiated not by withholding capability
but by the BMAD Method being the interface through which that capability is
organised.**

## Decision

1. **Forge executes.** The host may run child processes, read and write
   workspace files, and search the tree on the model's initiative. The blanket
   exclusion in ADR-0001 is withdrawn.

2. **Execution is gated by consent, not by absence.** The safety property ADR-0001
   bought by having no execution primitive is re-bought by a permission model:
   every tool call is evaluated against a rule set, and anything not explicitly
   allowed suspends the run and asks the user. This is the same posture the
   existing D2 egress consent and D3 change review already take — asked before
   the fact, not audited after it.

3. **The permission model is rule-based and last-match-wins.** A rule is
   `{tool, pattern, action}` with `action ∈ allow | ask | deny`. Evaluation
   scans the merged rule set and takes the **last** match, so later sources
   override earlier ones. **An unmatched call is `ask`, never `allow`.** A
   `deny` is terminal for that call and is reported to the model as a refusal it
   may reason about, not as a crash.

4. **"Always" grants are session-scoped and in-memory.** A user answering
   "allow always" adds an allow rule for the remainder of the process lifetime.
   Durable, cross-restart grants are **not** part of this decision; they would
   need their own ADR because they turn a decision the user made once in context
   into standing authority they may not remember granting.

5. **Every effect remains journaled and recoverable.** Tool-driven file writes
   go through the same governed IO, checkpointing, and effect journal as D3
   applies. Execution being agent-initiated changes who proposes an effect, not
   whether it is recorded or whether it can be rolled back.

6. **Process execution is bounded by construction**: a working directory pinned
   inside the granted workspace, a wall-clock timeout, a captured-output byte
   cap, and no inherited ambient credentials beyond the user's own environment.

7. **The BMAD Method remains the interface.** Capabilities, the sealed
   foundation, the agent roster, and the menu graph are unchanged and stay the
   organising layer. Tools are what let a capability actually do its work; they
   do not become a second, parallel product surface.

## Consequences

- The fidelity matrix's "Adapted (one-shot)" label becomes a statement about
  work still to do rather than a standing product boundary. It must not be
  upgraded per capability until that capability genuinely runs its upstream
  loop.
- `crates/desktop-execution`'s header claim that it "intentionally exposes no
  child-process API" is now false and is corrected as part of this change.
- The threat model changes materially. A model that can run commands can
  exfiltrate data or damage a workspace, and the permission prompt is the only
  thing standing in the way. The prompt must therefore show **what** is being
  run and **where**, and the default for anything unmatched must remain `ask`.
- Prompt-injection reachable through file contents and command output is now a
  live concern in a way it was not when the host could not act. Content read
  from the workspace or returned by a command is data, never instruction.
- ADR-0001's first paragraph still holds and is load-bearing: the signed Rust
  host remains the only local authority, and the renderer remains untrusted.
  Tools are executed by the host, never by the renderer.
