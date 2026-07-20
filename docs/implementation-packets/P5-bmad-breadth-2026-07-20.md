# Implementation packet: P5 — BMAD breadth through rewritten semantics

## Authority and intent

- Owning authority: repository maintainer (RodrigoSousa0). Executes the
  "BMAD breadth" phase deferred since P0, under ADR-0003's denominator
  decision and the standing `blocked_provenance` promotion gate.
- User-visible outcome, per slice: additional BMAD Method capabilities
  become usable in the desktop (beyond Winston's architect binding and the
  BMAD Help vertical), each carried by repository-authored sealed
  instruction projections and closed canonical output schemas — never by
  redistributed source bodies.
- Contracts read: `packages/bmad-foundation/*` ledgers and normalized
  packages, `packages/contracts` catalog/fixtures, `crates/desktop-ipc`
  envelope shapes, `crates/desktop-runtime` canonical schemas.
- Non-goals: relaxing any `blocked_provenance` verifier pin; promoting
  Paige's `method-010`…`method-013` bodies (ADR-0003); new Tauri
  permissions or network/filesystem primitives; production model-path
  changes (P3 owns the support plane); D2/D3 authority semantics (P4 owns
  the grant model).
- Stop conditions: a slice would require redistributing source material;
  a catalog change cannot preserve envelope-family compatibility; the
  adversarial review of a sealed projection finds semantic drift from the
  audited method reference (note 100).

## Denominator (ADR-0003)

Measured 2026-07-20 from `packages/bmad-foundation/adoption-ledger.json`:
76 reviewed source members; treatments 58 adapt / 55 reject / 26 defer /
23 adopt; **26 members carry at least one `defer`** — the P5 backlog.
Progress is the monotonic conversion of defers, re-measured in each
slice's evidence.

## Slices

- **P5-A (this commit): denominator closure.** ADR-0003 locks the Paige
  decision (exclude from denominator, retain reference-only closure) and
  pins the measured denominator. Docs only; no ledger or verifier drift.
- **P5-B: analyst brief vertical (Mary).** The first breadth capability:
  a sealed `bmad-analyst` instruction projection (rewritten semantics for
  the analysis-brief workflow), a closed
  `sapphirus.bmad-analysis-brief-proposal.v1` output schema in
  contracts + desktop-runtime validation, catalog/envelope additions in
  the reviewed additive pattern, D2 Help-vertical reuse for consent and
  egress (the existing eight-command surface parameterized by capability,
  or a deliberate catalog extension if parameterization would overload
  command semantics), renderer projection, and cross-language conformance
  fixtures. Converts Mary's deferred treatments.
- **P5-C: PM PRD-draft vertical (John).** Same pattern over the PRD
  workflow; converts John's deferred treatments.
- **P5-D: breadth qualification.** Adversarial review of every new sealed
  projection against note 100, per-capability privacy/consent fixtures,
  denominator re-measurement, and a breadth section in the qualification
  evidence.

## Tests first (per capability slice)

- Success: the new sealed projection round-trips with pinned hashes; the
  closed output schema rejects unknown fields/oversizes in all three
  languages; the capability's Help-style lifecycle passes the P4
  independence matrix (context-read withdrawal invalidates it, edit
  escalation does not); catalog additions appear in Rust/TS/C# with
  byte-identical generated bindings.
- Negative/bypass: source-body text cannot appear in any runtime
  projection, renderer payload, or prompt (vocabulary/canary guards
  extended per slice); a capability disabled in the ledger cannot
  dispatch; schema substitution across capabilities fails closed.
- Compatibility: existing 28-command catalog behavior byte-identical;
  deferred-vocabulary guards keep failing on their true-positive probes.

## Change and rollback

- Lanes per slice: foundation ledgers/normalized packages; contracts +
  generated bindings; desktop-runtime schema validation; desktop-ipc +
  desktop-app composition; renderer; docs/evidence. One commit per lane,
  dependency-ordered, revertible in reverse.
- Rollback: a capability slice reverts cleanly to display-only state; the
  ledger conversion commits are the record of what must be re-deferred.

## Exit gate (per slice and for P5 overall)

- `pnpm verify:deferred-full` green (includes cross-language
  qualification, secret/boundary scans, renderer suite, build).
- Workspace fmt/clippy/tests green.
- Denominator strictly smaller than the previous slice's measurement, or
  the slice explicitly records why it held steady.
- Adversarial review recorded for every new sealed projection.

## Review ledger

- P5-A executed 2026-07-20: ADR-0003 + this packet; denominator measured
  (76 members / 26 deferred); no ledger, verifier, or product changes, so
  the standing green gates from P4 (`d9fabb90`) remain the evidence
  baseline. Remaining slices execute under their own lanes.
