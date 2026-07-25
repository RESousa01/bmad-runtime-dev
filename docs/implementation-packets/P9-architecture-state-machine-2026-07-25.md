# Implementation packet: P9 — the architecture state-machine packet

## Authority and intent

- Owning authority: repository maintainer (RodrigoSousa0), closing the first
  BMAD checkpoint named by the
  [2026-07-23 comprehensive audit](../reviews/forge-v2-comprehensive-audit-2026-07-23.md)
  ("BMAD semantic engines") and the
  [fidelity matrix](../reviews/bmad-capability-fidelity-matrix.md) row for
  Architecture.
- User-visible outcome: **Architect / CA is available.** It runs as an ordered,
  host-validated progression of eight steps with a stable decision log,
  distilled into one inert architecture spine. The runtime-enabled Method menu
  denominator moves 25 -> 26 paths; 24 -> 24 unique capabilities, all active.
- Contracts read: the sealed `architecture-create` managed projection, its
  foundation record (the only `entrypointKind: "step_jit"` projection in the
  adoption ledger), ADR-0003 (blocked tool intents), ADR-0005 (the closure
  denominator), and the reviewed upstream `bmad-architecture` skill.
- Non-goals: re-minting any sealed foundation byte, adding an IPC command,
  adding a result archetype, bumping the store schema, changing the egress or
  consent model, or claiming upstream coaching-dialogue fidelity.

## Why this was the blocking gap

The sealed guidance for this one capability says the model must "work only from
the reviewed context snapshot and current host-selected step", must "not skip,
repeat, invent, or finalize a step", and that "the host validates progression".
The runtime had no ordered lane, so offering the menu through the one-shot
generic lane would have contradicted the capability's own sealed instruction.
The host therefore filtered the path out and the UI reported `Unavailable` —
which in turn left the sealed Help binding recommending an action the product
could not perform.

The sealed foundation had already recorded the distinction: of 40 managed
projections, `architecture-create` is the only one carrying
`entrypointKind: "step_jit"`. The provenance data knew this capability was
stepwise; only the runtime did not.

## Design: the progression rides the existing lane

One architecture run is a *sequence of ordinary reviewed capability runs*. The
egress path is unchanged — same context manifest, same single-use consent
decision, same untrusted-output verification, same durable capability-run
record per step. The host adds exactly four things:

1. **Step selection.** `BmadArchitectureStep::ORDERED` fixes the eight
   dimensions the sealed guidance names: system context, components, trust
   boundaries, data, failure handling, observability, deployment, verification.
2. **Step context.** The selected step and the accepted decision log enter the
   run as ordinary reviewed context candidates, so the step the user consents
   to sending is covered by the manifest hash like any other context item.
3. **Progression validation.** `accept_step` refuses a skipped step, a repeated
   step, a step after the progression completes, a step on a finalized session,
   a backwards clock, and any cumulative bound violation.
4. **Local distillation.** `finalize` runs only when all eight steps are
   answered, and produces one `BmadDocumentArtifact` spine with stable `AD-n`
   identifiers. **No model call, no egress, no workspace effect.**

A step advances the progression only after its artifact is *durably stored and
re-parsed* through the same strict archetype constructors, so the state machine
can never advance on anything the store and the verifier have not both
accepted.

## Tests first

- Success: `the_architecture_progression_runs_ordered_to_a_distilled_spine`
  drives all eight steps through the real `prepare` / `approve` / `submit`
  command path, asserts the selected step at each stage, asserts that only the
  eighth finalizes and only it carries a spine, checks `AD-n` continuity in the
  distilled sections, and re-reads the durable run.
- Negative/bypass: skipping, repeating, accepting after completion, accepting
  onto a finalized session, a foreign-workspace result, a backwards clock, the
  cumulative decision bound, forged step codes and mismatched numbering at the
  renderer seam, spine-without-finalization and finalization-without-spine, and
  unknown keys smuggled into the progression block or the spine.
- Compatibility: `one_shot_capabilities_carry_no_progression` pins that every
  other capability's wire shape is byte-identical to before — the
  `architecture` field is absent, not null.

## Change and rollback

- Files: `crates/desktop-runtime/src/bmad/architecture.rs` (new),
  `crates/desktop-app/src/bmad_architecture.rs` (new), plus wiring in
  `bmad_capability_host.rs`, `commands.rs`, `state.rs`, `wire.rs`, and the
  renderer's capability protocol, panel, fidelity classifier, and styles.
- Rollback is a revert. No sealed foundation byte, IPC command, contract
  schema, or store migration was touched, so nothing downstream is pinned to
  this change.
- The sealed projection was deliberately **not** re-minted. Its sentence "the
  capability remains unavailable until its later state-machine packet" states a
  condition; this packet satisfies it. Re-writing sealed bytes to record that
  fact would have cascaded the foundation hash chain across five pin sites for
  no semantic gain.

## Honest limits (unchanged by this packet)

- **Not upstream fidelity.** The upstream workflow is a coaching dialogue with
  a memlog, reviewer-gate subagents, and web verification. Forge runs eight
  reviewed one-shot steps under host-owned ordering. The UI says "Stepwise",
  never "faithful", and `bmadFidelity.ts` carries the caveat inline.
- **In-memory progression.** The session lives alongside the generic
  coordinator, which the 2026-07-23 audit already flags as non-durable. Each
  completed step is durably stored as its own capability run, so the evidence
  survives a restart; an *in-progress* progression does not, and restarts at
  step one.
- **Sign-out and workspace revocation end the progression** (ADR-0002), and
  switching workspaces starts a fresh one.
- **Production transport is still not composed.** Like every other capability,
  architecture runs on the direct-development, deterministic, or offline
  transport. Deploying the support API does not make it production-operational.

## Review ledger

- 2026-07-25 — Implemented and verified locally. `cargo fmt --check` clean;
  `cargo clippy --workspace --all-targets --all-features --locked -D warnings`
  clean; `cargo test -p desktop-app --all-features` 130 passed (was 120: +7 host
  progression cases, +4 command-lane cases, -1 replaced "stays deferred" pin);
  `cargo test --workspace --all-features --locked` 613 passed, 0 failed;
  `desktop-runtime` adds 12 progression cases; renderer `vitest run` 427 passed across 39
  files (was 415); `pnpm typecheck` clean across all six workspace projects;
  `pnpm verify:boundaries` green; `pnpm bmad:foundation:verify` 116/116 with the
  sealed hash unchanged.
- 2026-07-25 — Not run in this pass, unchanged from the prior session's
  boundary: full `pnpm verify:source` (the environment is Node 24.11.1 against
  the pinned 24.18.0, so `toolchain:check` fails first and later steps need
  Docker), and every external-infrastructure lane (Azure, signed installer,
  pilot evidence).
