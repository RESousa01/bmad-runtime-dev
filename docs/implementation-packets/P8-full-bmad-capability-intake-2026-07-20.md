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
- 2026-07-20 — Task 5 complete across all three lanes: sealed
  `bmad-capability-run`/`bmad-capability-result` contracts generated into
  Rust/TypeScript/C# (29 schemas, 9 adversarial fixtures), the
  `BmadCapabilityRun` runtime type (7 integration tests), and store schema
  v11 (`bmad_capability_runs`/`bmad_capability_results`, encrypted CAS
  results, consent-evidence uniqueness; legacy v3-v8 migration fixtures
  extended and green).
- 2026-07-20 — Task 6 core landed: `BmadCapabilityCoordinator` drives
  prepare -> single reviewed decision -> single-use consumption ->
  transport -> output verification -> durable persistence for any ADR-0005
  capability. Capability identity is bound through the egress purpose
  label, decision digests, and the declared archetype schema; 7
  parity/substitution tests prove cross-capability manifest, decision,
  and output substitution fail closed (desktop-app 98/98, strict clippy
  clean). Deliberate deferral: Help stays on its existing coordinator —
  its golden projections remain byte-identical by construction — and the
  Help-onto-generic migration is re-scheduled to land with Task 7's
  vertical wiring, where the shared transport composition is decided.
- NEXT (Task 7): author the managed instruction projections per capability
  family (18 semantic rewrites; 6 first-party targets pending
  license/provenance approval), add the five `bmad.capability.*` IPC
  commands (catalog 33 -> 38 across all five pin sites), the renderer
  `BmadCapabilityPanel`, and the 26-path table-driven activation matrix.
- 2026-07-20 — Task 7 Step 1 foundation lane COMPLETE for every
  semantic-rewrite target: 17 new sealed capability instruction
  projections minted in two family passes (analyst 6, planning/dev 11),
  joining the pre-existing architecture projection for 18/18. Source
  intake grew the reviewed member set 76 -> 110 (method-030..063,
  SKILL.md + customize.toml per skill, hashed from the vault); manifest
  30 -> 64 resources; descriptor 8 -> 25 managed method projections. All
  pins moved: verify.mjs (lock, counts, projection map, identity set),
  foundation tests (104/104), Rust bmad_foundation.rs, and the four
  kernel/materialization/help-run/method-store snapshots. Workspace 0
  failures, strict clippy clean, boundaries green. The closure ledger now
  binds managedProjection for all 18; activationStatus stays `planned`
  until the bmad.capability.* vertical (next slice) proves each path.
- REMAINING for Task 7: five `bmad.capability.*` IPC commands (catalog
  33 -> 38 across five pin sites), host wiring onto the Task 6
  coordinator, renderer BmadCapabilityPanel, the 26-path activation
  matrix, and the six first-party targets awaiting the owner's
  license/provenance approval (Paige DP/WD/MG/VD/EC + John PRD).
- 2026-07-20 — OWNER APPROVAL RECORDED: the repository maintainer approved
  the six first-party targets (Paige DP/WD/MG/VD/EC and John PRD) for
  first-party semantic rewrite, directing that the rewrites follow the
  reviewed BMAD sources for accuracy. Executed: the four ADR-0003-deferred
  tech-writer action members (method-010..013) were treatment-converted
  from adapt/defer/reject to adopt/adapt (the exclusion tripwire test now
  pins the resolved state); bmad-document-project and bmad-prd were
  intake'd as method-064..067. Six sealed projections minted, grounded in
  the vault sources (PRD facilitation with decision-log discipline,
  tech-writer four-action processes, document-project brownfield router).
  Members 110 -> 114, manifest 76 resources, descriptor 31 managed method
  projections. All 24 unique menu capabilities and 26 menu paths now carry
  sealed managedProjection bindings. Foundation 116/116, workspace 0
  failures, strict clippy clean, boundaries green.
