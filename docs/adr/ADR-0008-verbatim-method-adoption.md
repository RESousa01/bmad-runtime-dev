# ADR-0008: verbatim adoption of the MIT-licensed method under a scoped, expiring source-identity exception

- Status: accepted (2026-07-25)
- Supersedes: ADR-0003 decisions 1 and 3, and the "never into source-body
  redistribution" clause of decision 2.
- Decided by: repository maintainer (RodrigoSousa0), on explicit review of the
  licence, trademark, and source-identity positions recorded below.

## Context

Forge's purpose is to be the BMAD runtime — to practise the method without a
separate LLM runtime. ADR-0003 required every capability to reach the product
through **reviewed rewritten semantics**: repository-authored instruction
projections that "adopt or adapt the method's intent without redistributing
source bodies".

That policy achieved its provenance goal and produced a specific, measurable
product failure. The
[fidelity matrix](../reviews/bmad-capability-fidelity-matrix.md) records
**faithful upstream execution: 0** across all 29 capabilities, and the
[integration redesign](../reviews/bmad-integration-redesign-2026-07-25.md)
traces the cause: paraphrasing an instruction whose *exact wording is the
product* loses the method. The instructions are not documentation about the
software; for a method runtime they **are** the software.

### The three concerns ADR-0003 conflated

Re-reading the upstream terms and `packages/bmad-foundation/NOTICE.md`, the
standing `promotionEligibility: "blocked_provenance"` gate bundled three
unrelated risks:

1. **Copyright / redistribution.** Upstream `LICENSE` is MIT
   (Copyright (c) 2025 BMad Code, LLC). MIT grants the rights to "use, copy,
   modify, merge, publish, distribute, sublicense, and/or sell", conditioned
   only on including the copyright notice and permission notice. Redistribution
   of source bodies is therefore **expressly permitted**. The exact licence
   text already ships at `packages/bmad-foundation/licenses/BMAD-METHOD-MIT.txt`.
   *This was never a real blocker.*

2. **Trademark.** Upstream `TRADEMARK.md` reserves "BMad", "BMad Method",
   "BMad Core", "BMad Code", the logo, and the tagline, and states these are
   **not** licensed under MIT. The restriction is on *naming and endorsement*:
   one may not use the marks as a product, service, company, or domain name,
   nor imply endorsement. It expressly permits referring to BMad "to accurately
   describe compatibility or integration". Forge is named Forge, uses the marks
   descriptively, and claims no endorsement. *Already satisfied; unaffected by
   how instruction text is carried.*

3. **Source identity.** The reviewed tree is a GitHub archive snapshot
   (`BMAD-METHOD-main`) with no `.git` directory, so there is no commit SHA or
   signature proving the bytes are upstream's. This is the concern NOTICE.md
   actually names, and it is legitimate. *This is the only real blocker.*

### Identity evidence that does exist

- **Owner attestation of chain of custody.** The repository maintainer states
  that this archive was downloaded personally and directly from the official
  upstream GitHub repository. That is the same trust basis on which virtually
  all open-source consumption rests: TLS to the official host, fetched from the
  canonical org, by a named human who is accountable for the claim.
- `package.json` → `"name": "bmad-method"`, `"version": "6.10.0"`
- `CHANGELOG.md` → `## v6.10.0 - 2026-07-03`
- Per-member content hashes for every adopted file, pinned by the foundation
  gate, which fails closed on any drift after intake.

Taken together this is an ordinary, acceptable provenance basis for
development. What is still missing is not authenticity but **reproducibility**:
there is no commit SHA or signed tag, so a third party cannot mechanically
re-derive this exact tree. That is the gap decision 3 closes, and it is a
narrower gap than ADR-0003 assumed.

## Decision

1. **Verbatim adoption is permitted** for MIT-licensed method and builder
   source, under the existing `adopt` treatment, with the copyright notice and
   MIT permission notice retained. Rewritten semantics remain available as
   `adapt` where Forge deliberately diverges (for example where ADR-0003's
   blocked tool intents make an upstream step inapplicable), and every such
   divergence must be recorded as `adapt`, never presented as upstream text.

2. **`promotionEligibility` moves from `blocked_provenance` to
   `owner_attested_release`** — a human-owned exception, exactly the mechanism
   NOTICE.md anticipates. It records: the maintainer's chain-of-custody
   attestation, the release identity above, the archive digest, the granting
   authority, and the reproducibility condition below.

3. **Reproducibility is pinned before first public distribution.** Before any
   signed installer, published artifact, or pilot build ships, the adopted
   bytes must be pinned to an immutably identified upstream release (a tagged
   commit or signed release archive) and this ADR updated with that identity.
   This is a reproducibility and supply-chain-hygiene requirement, not a
   licensing one; local development and internal review proceed on the owner
   attestation alone.

4. **Trademark posture is pinned, not assumed.** Forge must not use a BMAD mark
   as its product name and must not claim endorsement. The foundation gate
   already quarantines authority-bearing projection fields; the naming
   constraint is recorded here and in NOTICE.md as a standing product rule.

5. **Attribution is per-member, not per-package.** Every adopted file records
   its upstream path, content hash, and licence in the adoption ledger, so the
   provenance of any single instruction is answerable without re-reading the
   archive.

## Consequences

### What this unblocks

- Instruction fidelity becomes achievable at all. The fidelity matrix can move
  off "faithful upstream execution: 0" honestly, rather than by relabelling.
- The ~half of the method currently outside the denominator (`core-skills`,
  the reviewer lenses, `bmad-spec`, `module-help.csv`) can be ingested.
- Intake cost per skill drops from "author a semantic rewrite" to "hash and
  seal", which is what makes full coverage tractable.

### Residual risk, stated plainly

The archive's authenticity rests on the maintainer's attestation that it was
fetched directly from the official upstream repository — a normal and
sufficient basis for development, and the same one nearly every dependency in
this repository already rests on.

What remains is **reproducibility, not authenticity**: with no commit SHA or
signed tag, a third party cannot mechanically re-derive this exact tree, and a
future upstream force-push or branch rewrite could not be detected by
comparison. Content hashes prove the bytes have not drifted *since* intake;
they do not identify *which* upstream state they came from. Decision 3 closes
this before anything ships publicly.

### What does not change

- ADR-0003's **executable exclusion stands**: `blockedToolIntents`
  (`external_handoff`, `file_read`, `file_write`, `process`, `subagent`, `web`)
  remain blocked. Adopting an instruction body verbatim does not adopt its
  authority. An upstream step that shells out, spawns a subagent, or fetches
  the web is still host-replaced or refused, and that replacement is an `adapt`
  treatment recorded per member.
- Builder output stays inert (ADR-0006).
- Everything reaching a model still passes D2 consent and egress review.
- The foundation gate still fails closed on hash drift, duplicate members,
  authority-bearing projection fields, and reference escape.
