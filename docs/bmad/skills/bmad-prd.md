# bmad-prd

- **rowId**: `method:src/bmm-skills/2-plan-workflows/bmad-prd/SKILL.md`
- **canonicalId**: `bmm:bmad-prd`
- **pinned source**: `packages/bmad-foundation/source/method/6.10.0/src/bmm-skills/2-plan-workflows/bmad-prd/SKILL.md`

## What it does

Facilitates creating, updating, or validating a PRD. Detects intent
(Create / Update / Validate) at activation, or honors it verbatim when
forwarded by a caller (e.g. `bmad-create-prd` / `bmad-edit-prd` /
`bmad-validate-prd` shims). Create walks Discovery (brain dump, stakes
calibration, working mode) then drafts `prd.md` with a `.memlog.md` audit
trail; Update reconciles the PRD against a change signal; Validate loads
`references/validate.md` for an inline critique. Finalize runs a reviewer
gate, triages open items, polishes, and marks `status: final`.

## Declared actions and modes

- **Actions**: none declared as a fixed, separately-invocable action.
  Catalog row records `action: null`. The source names three intent modes
  in prose (Create, Update, Validate) detected conversationally within one
  entry point, the same representational limit `bmad-product-brief`'s
  adapter documents for its own three modes. Reported as a fidelity gap.
- **Modes**: `interactive`, `headless` (per capability-catalog.json and
  `references/headless.md`).
- **Output location**: `{planning_artifacts}` (resolved in activation
  Step 3).
- **Artifact kind**: `document`. The source writes `prd.md` with YAML
  frontmatter, persisted as it is written and finalized in place --
  `PersistArtifacts` completion hook.
- **Pinned resource closure**: the adapter declares all eight files in its
  upstream directory: `SKILL.md`, `customize.toml`, the three configured
  templates, the headless schema, and both headless/validation references.
  Their SHA-256 identities are passed as immutable package data to the
  generic resolver; this adapter cannot read, execute, or grant authority to
  any source file.
- **Generic execution contract**: the adapter explicitly opts into the shared
  Method preparation bridge. It binds the sealed 6.10.0 package/envelope,
  current capability catalog, full resource set, both declared modes, the
  `facilitate_prd` source step, six typed tool intents, one required document
  artifact, and `MethodCheckpoint` recovery semantics. Desktop host
  reachability is fail-closed against the reviewed canonical table and the
  generated UI-availability projection.
- **Durability and authority**: cancellation, failure, retry, repair,
  restart, resume, and source-upgrade transitions are handled by the generic
  BMAD Method session service through typed plans, governed ports, Airlock,
  and evidence. The adapter remains declarative and has no direct filesystem,
  process, network, scheduler, model, session, or persistence access.
- **Predecessor/follower**: neither is declared. The only upstream
  reference is a misroute suggestion ("one-pager → `bmad-product-brief`"),
  and Finalize names three legitimate next skills side by side
  (`bmad-ux`, `bmad-architecture`, `bmad-create-epics-and-stories`) plus a
  router (`bmad-help`) rather than one fixed follower. Reported as a
  fidelity gap: the harness's single-`follower` field cannot represent an
  "any of these" set.

## Compatibility decision

**adopt / exact.** No semantic divergence was found between the pinned source,
the harness descriptor, and the explicit preparation contract. The
action-naming and follower-set gaps above are representational limits of the
harness's single-value fields, not behavioral mismatches. The three legacy v6
shim rows remain unavailable/deprecated aliases and are not promoted with the
canonical implementation.

## Source-derived golden coverage

`artifacts/bmad100/method/bmad-prd/golden/source-derived-cases.json` records
the pinned identities and representative interactive, headless complete,
partial, blocked, validate, update-recovery, and generic durable-recovery
outcomes. Rust conformance prepares the exact canonical binding in both
interactive and headless modes; catalog, Help, and UI tests prove one
runtime-available canonical row without promoting its aliases. The executable
resource-closure, mode-conformance, and authority receipts are stored beside
this golden record.
