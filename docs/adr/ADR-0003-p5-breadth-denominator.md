# ADR-0003: P5 breadth denominator and the Paige prompt-reference scope

- Status: accepted (2026-07-20)
- Decides: the P0 open decision "Paige source-prompt reference scope
  (promote or remove) — affects the P5 denominator".

## Context

The adoption ledger records `promotionEligibility: "blocked_provenance"`,
and the foundation verifier asserts that value in three places: promotion
of BMAD *source material* (including Paige's four prompt-reference bodies,
`method-010` … `method-013`) is blocked until immutable upstream identity,
trademark, and redistribution review complete. Those are human/legal
actions outside any implementation phase.

Winston's architect capability demonstrated the sanctioned alternative:
capability breadth through **reviewed rewritten semantics** — sealed,
repository-authored instruction projections that adopt or adapt the
method's intent without redistributing source bodies.

## Decision

1. **Paige's four prompt references are excluded from the P5 promotion
   denominator and retained exactly as they are**: reference-only menu
   closure (`unavailable_reference_only`), bodies never entering runtime,
   renderer, or prompt surfaces. "Promote" is structurally unavailable
   under the standing provenance gate; "remove" would break the reviewed
   menu-graph closure for no benefit. This choice requires no ledger or
   verifier change and is revisited only if the provenance gate is ever
   cleared by its own review.
2. **The P5 denominator is the ledger's deferred treatments**: of the 76
   reviewed source members, 26 carry at least one `defer` treatment
   (58 adapt / 55 reject / 26 defer / 23 adopt treatments overall,
   measured from `packages/bmad-foundation/adoption-ledger.json`). P5
   breadth progress is the conversion of `defer` treatments into
   `adopt`/`adapt` (rewritten semantics) or `reject` (with rationale) —
   never into source-body redistribution.
3. **Every P5 capability slice follows the Winston pattern**: sealed
   instruction projection, canonical closed output schema, catalog and
   conformance coverage in all three languages, and D2 consent/egress
   review for anything that reaches a model.

## Consequences

- `promotionEligibility` stays `blocked_provenance`; no P5 slice may relax
  a verifier pin on it.
- The denominator is auditable by one command over the adoption ledger and
  shrinks monotonically as slices land.
- Paige can gain an *enabled* capability in a later slice only through
  rewritten semantics with a new sealed projection — not by promoting
  `method-010` … `method-013`.
