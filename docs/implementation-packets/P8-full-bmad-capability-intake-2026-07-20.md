# Implementation packet: P8 — full BMAD capability intake

## Authority and intent

- Owning authority: repository maintainer (RodrigoSousa0), executing
  readiness-program Gate C under ADR-0005.
- Outcome: every one of the 26 roster menu paths and 5 Builder authoring
  operations in `packages/bmad-foundation/capability-closure-ledger.json`
  becomes an executable, tested capability through one generic sealed
  capability-run lifecycle (readiness Tasks 5-7), with Builder drafts
  remaining inactive (Task 8, ADR-0006 pending).
- Contracts read: ADR-0003 (source intake rules, still in force), ADR-0005
  (denominator + archetypes), the sealed foundation chain, and the
  33-command reviewed catalog.
- Non-goals: shrinking the denominator, copying source bodies, activating
  Builder outputs, or letting any model output write files outside D3.

## Denominator (fixed by ADR-0005, pinned by the foundation gate)

- 26 menu paths across `bmad-agent-analyst` (7), `bmad-agent-tech-writer`
  (5), `bmad-agent-pm` (4), `bmad-agent-ux-designer` (1),
  `bmad-agent-architect` (2), `bmad-agent-dev` (7); DP and IR are shared
  capabilities counted per path. 24 unique menu capabilities.
- 5 Builder operations as `inactive_builder_draft`.
- 3 governed-change-set capabilities (DS, QD, QA); everything else on the
  method side is `document_artifact`.

## Source intake status

- `first_party_semantic_rewrite` (license/provenance approval before
  authoring): bmm:bmad-document-project, the four tech-writer actions
  (WD/MG/VD/EC), bmm:bmad-prd. These are the ADR-0003 exclusions being
  superseded; their instruction projections do not exist yet.
- `semantic_rewrite_from_reviewed_source`: the remaining 18 menu
  capabilities and the 5 Builder operations (whose sealed projections
  already exist under `runtime/builder/2.1.0/`).
- Existing managed projection reuse: `bmm:bmad-architecture` binds
  `runtime/method/6.10.0/architecture-create.instructions.md`.

## Tests first

- The foundation suite pins the exact 26+5 set, one record per capability,
  archetype/schema consistency, and duplicate-path rejection (landed with
  this packet, red-first).
- Each later activation task flips `activationStatus` to `active` only in
  the same commit as its passing focused tests; the foundation gate keeps
  counting.

## Change and rollback

- This packet lands governance only (ADR-0005, ledger, tests, inventory
  admission). Rollback is a revert; no runtime hash chain was touched —
  the closure ledger is deliberately outside `runtime-manifest.json` until
  capabilities activate.
- Sequenced lanes that consume this packet: Task 5 (capability-run
  contract, store v11), Task 6 (generalized D2 lifecycle), Task 7 (26 menu
  paths by family), Task 8 (Builder drafts).

## Review ledger

- 2026-07-20 — Denominator extracted from the reviewed source
  (`_source_review/BMAD-METHOD-main … customize.toml` per agent), matching
  the readiness plan's enumeration exactly (26 paths). Red completeness
  test written first; ledger landed to green; package topology, npm
  distribution list, and `verify.mjs` inventories admitted the ledger with
  the sealed foundation hash unchanged
  (`sha256:8d9c9d5b…daac93687`, 76 source members, 17 managed outputs).
- 2026-07-20 — Foundation suite 70/70; boundary scan green; Rust
  foundation pin tests green (6/6, no pin cascade by design).
