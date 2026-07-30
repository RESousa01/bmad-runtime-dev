# bmad-spec

- **rowId**: `method:src/bmm-skills/2-plan-workflows/bmad-spec/SKILL.md`
- **canonicalId**: `bmm:bmad-spec`
- **pinned source**: `packages/bmad-foundation/source/method/6.10.0/src/bmm-skills/2-plan-workflows/bmad-spec/SKILL.md`

## What it does

Distills any intent input (idea, brain dump, PRD, GDD, RFC, brief, transcript,
mixed multi-source) into `SPEC.md` -- the five-field kernel (Why,
Capabilities, Constraints, Non-goals, Success signal) -- plus optional
companion files for load-bearing content that would bloat the kernel.
Create-vs-update is detected by whether a prior `.memlog.md` exists at the
target spec folder, not by a caller-selected action. `SPEC.md` and every
spec-authored companion are re-derived from `.memlog.md` on every run. A
self-validate sweep (coherence, then preservation) runs after every
create/update. Story Breakdown is an optional, interactive-only extension
that writes `stories.yaml`.

## Declared actions and modes

- **Actions**: none declared as a fixed, separately-invocable action.
  Catalog row records `action: null`.
- **Modes**: `interactive`, `headless` (per capability-catalog.json). The
  source's own headless detection (activation Step 4) is heuristic --
  "no TTY, programmatic caller ... or the first message pre-supplies all
  inputs" -- rather than an explicit flag, but both modes are still named
  and supported.
- **Output location**: `output_folder`. Unlike its sibling
  `2-plan-workflows` skills, activation Step 3 resolves `{output_folder}`
  (under the merged `core` config), not `{planning_artifacts}`, and the
  spec folder defaults to `{output_folder}/specs/spec-{slug}/`.
- **Artifact kind**: `document`. `SPEC.md` is the canonical working
  artifact, re-derived and persisted on every run -- `PersistArtifacts`
  completion hook. Companions are optional overflow of the same kind, not a
  distinct declared kind.
- **Predecessor/follower**: neither is declared. The source states
  "Multiple skills may call to update the same spec over time" -- no single
  fixed upstream or downstream skill is named.

## Source closure and governed execution

The thin runtime adapter declares the complete five-member pinned subtree,
with each member's SHA-256 identity: `SKILL.md`, `customize.toml`,
`assets/headless-schemas.md`, `assets/spec-template.md`, and
`assets/stories-schema.md`. This includes the template edge declared by
`customize.toml`; the headless response and Story Breakdown schemas remain
available to the generic runtime as immutable source data even though they
are selected by workflow semantics rather than a source-path edge.

The adapter only describes this immutable execution shape. The generic BMAD
runtime owns resolved configuration and customization, canonical model
context, artifact persistence, action plans and approvals, validation,
cancellation, failure/retry/repair, restart/resume, and source-upgrade
checks. Consequently neither source prose nor this adapter can read files,
run a process, call a model/provider, schedule work, or write an artifact
outside typed ports and Airlock policy. Both invocation modes bind the same
`output_folder`, `SPEC.md` document kind, source identities, and persistence
hook; their source-specific interaction differs only in presentation and the
headless JSON response.

## Compatibility decision

**adopt / exact.** No semantic divergence was found between the pinned
source and the harness descriptor. The source account uses `adapt` for the
native governed projection treatment; that is not a user-visible support
state and does not alter the ledger's exact/adopt decision.
