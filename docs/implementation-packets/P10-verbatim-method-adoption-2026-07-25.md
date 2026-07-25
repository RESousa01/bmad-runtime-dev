# Implementation packet: P10 — verbatim method adoption and the workflow graph

## Authority and intent

- Owning authority: repository maintainer (RodrigoSousa0), on the explicit
  direction to rework the BMAD implementation to follow the source, and the
  explicit approval to correct or set aside an ADR that blocks an improvement
  and causes no damage — after careful review.
- Decision record: [ADR-0008](../adr/ADR-0008-verbatim-method-adoption.md).
- Outcome: the sealed foundation now carries the **method itself**, verbatim,
  and the runtime can compute where a project stands in that method without
  asking a model.
- Non-goals in this packet: the chat surface, the memlog primitive, and the
  agent-activation vertical. Those are the remaining redesign steps.

## Why the previous model was wrong

ADR-0003 required every capability to reach the product through *reviewed
rewritten semantics* — paraphrase, never redistribution. That produced exactly
one measurable outcome: **faithful upstream execution: 0** across all 29
capabilities. For a method runtime, the instruction wording *is* the product;
paraphrasing it loses the method. The
[integration redesign](../reviews/bmad-integration-redesign-2026-07-25.md)
traces this in detail.

## What the careful review found

The standing `blocked_provenance` gate bundled three unrelated risks:

| Concern | Finding |
|---|---|
| MIT redistribution | Not a blocker. MIT grants copy and redistribution with notice; the licence text already shipped. |
| Trademark | Real, already satisfied. `TRADEMARK.md` restricts using the marks as a product/company/domain name or implying endorsement, and expressly permits descriptive compatibility references. Forge is named Forge. |
| Source identity | The only real one — and narrowed by the maintainer's attestation that the archive was downloaded directly from the official upstream repository. |

What remains is **reproducibility, not authenticity**: with no commit SHA, a
third party cannot mechanically re-derive the tree. ADR-0008 keeps that as an
explicit obligation to discharge before public distribution.

Corroborating evidence found during intake: **all 67 previously reviewed method
member hashes still match the archive byte-for-byte**, so the tree is unchanged
since the original review.

## What landed

### 1. Provenance, resealed rather than hand-edited

`promotionEligibility` moved to `owner_attested_release` across the semantic
ledger, adoption ledger, gate, tests, and Rust loader. The gate no longer
accepts a null exception: `validOwnerAttestation` now *requires* a grantor, a
grant date, a decision record, the attestation text, the release identity, and
an expiry — so dropping the human or the expiry fails closed rather than
silently converting a reviewed risk into an unbounded one.

`provenanceStatus` in the package descriptor stayed `reviewed_snapshot_only`.
It describes the *kind of identity evidence*, which is unchanged and is
schema-constrained; the *permission* is what moved, and it lives in the ledgers.
Separating the two avoided a cross-language contract regeneration for no
semantic gain.

### 2. `scripts/reseal.mjs` — reproducible identity chain

Several foundation values hash other foundation values: the descriptor hashes
its own body, every projection envelope hashes its projection, the manifest
hashes the ledger bytes and every resource. Editing one sealed byte by hand
breaks a chain spanning four files.

`reseal.mjs` recomputes that chain with byte-identical logic to the gate. It was
**proved as a no-op against the pre-existing green state before being used**,
which is what makes it trustworthy. The reviewed semantic lock stays a manual
edit by design — the gate forbids automatic lock updates, so the script reports
the value instead of writing it.

### 3. `scripts/intake.mjs` — verbatim intake

- **228 adopted bodies shipped** under `source/method/6.10.0/`, covering all 47
  skills including the `core-skills` set that was previously absent entirely
  (`bmad-spec`, `bmad-advanced-elicitation`, the three reviewer lenses,
  `bmad-party-mode`, `bmad-shard-doc`, and the rest).
- **19 executables rejected and never shipped.** ADR-0003's executable
  exclusion survives ADR-0008: adopting an instruction body does not adopt its
  authority.
- **Append, never renumber.** Member ids are referenced by the adoption
  ledger, by runtime projections, and by hand-reviewed gate pins, so existing
  ids and their reviewed treatments are preserved; only new files get new ids.
  Members grew 114 → 294.
- **Idempotent**: a second run reports 0 new members, and drift in any
  previously reviewed member's hash aborts the intake.

The gate's allowlist for adopted bodies is the manifest, whose own hash is a
hand-reviewed pin mirrored in the Rust loader — so every adopted byte is still
pinned, without transcribing 228 paths into the verifier. Manifest and disk are
proved to agree in both directions.

### 4. `BmadWorkflowGraph` — the method as computable state

`crates/desktop-runtime/src/bmad/workflow.rs` parses the sealed
`module-help.csv` into a typed graph: phases, ordering edges, required gates,
and output locations.

Upstream's `bmad-help` hands that file to a model and asks it to guess where the
user is, because it is markdown with no code. Forge computes it: current phase,
completed steps, next required gate, available steps — **no model call, no
receipt, nothing to hallucinate**. This is the one place a native runtime beats
the original outright.

The tests assert against the *real sealed CSV*, including that the required
gates are exactly upstream's seven (`bmad-prd`, `bmad-architecture`,
`bmad-create-epics-and-stories`, `bmad-check-implementation-readiness`,
`bmad-sprint-planning`, `bmad-create-story:create`, `bmad-dev-story`), that
quoted descriptions containing commas survive parsing, that multi-action skills
become distinct steps, and that ordering edges gate *recommendation* rather than
permission — matching upstream's "soft suggestions, not hard gates".

## Verification record

| Check | Result |
|---|---|
| `cargo test --workspace --all-features` | **635 passed**, 0 failed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Clean |
| `cargo fmt --check` | Clean |
| `pnpm bmad:foundation:verify` | 116/116; **294 source members, 228 adopted bodies** |
| `pnpm contracts:verify:typescript` | 85 passed |
| `pnpm secrets:check` | Clean across **3,905** files (was 3,676) |
| `pnpm verify:boundaries` | Green |
| `pnpm typecheck` | Clean across all six workspace projects |
| Renderer `vitest run` | 427 passed / 39 files |
| `pnpm contracts:verify:cross-language` | **Not run** — the native codegen bootstrap lock has drifted in this environment, unrelated to these changes. Avoided by not altering any contract schema. |

## Remaining redesign steps

1. **Chat surface + agent activation** — the host half of the chat lane is in
   (`ChatSession`/`ChatTurn`, `ChatCoordinator`, persona binding,
   `chat.send`/`chat.history` across all six catalog pin sites). The renderer
   half is not, so the composer still routes every message to BMAD Help.
2. **Wire the workflow graph into the host** — resolve output locations through
   the (still idle) `BmadConfigResolver`, scan the workspace, project state.
3. **The memlog primitive** — append-only run log with derived artifacts.
4. **Refactor P9's architecture state machine onto the memlog**, retiring the
   bespoke eight-step machine.
