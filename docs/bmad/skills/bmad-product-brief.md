# bmad-product-brief

- **rowId**: `method:src/bmm-skills/1-analysis/bmad-product-brief/SKILL.md`
- **canonicalId**: `bmm:bmad-product-brief`
- **pinned source**: `packages/bmad-foundation/source/method/6.10.0/src/bmm-skills/1-analysis/bmad-product-brief/SKILL.md`

## What it does

Coaches the user through creating, updating, or validating a product brief.
Detects intent (create / update / validate) at activation; interactively
asks if the intent is unclear, or infers it headless. Create begins with
`## Discovery` before drafting and writes `brief.md` plus a `.memlog.md`
audit trail into a fresh workspace folder; Update reconciles an existing
brief against a change signal; Validate is an honest critique returned
inline.

## Declared actions and modes

- **Actions**: none declared as a fixed, separately-invocable action.
  Catalog row records `action: null`. The source does name three "Intent
  Operating Modes" in prose (Create, Update, Validate), but intent here is
  detected conversationally within one entry point rather than selected as
  a separate declared action -- see the adapter's doc comment for the full
  reasoning. Reported as a fidelity gap.
- **Modes**: `interactive`, `headless` (per capability-catalog.json and the
  source's own `## Headless Mode` section).
- **Output location**: `{planning_artifacts}` (resolved in activation
  Step 5).
- **Artifact kind**: `document`. The source explicitly writes `brief.md`
  with YAML frontmatter, persisted as it is written ("Persistence is
  real-time. Once Create intent is confirmed, the workspace ... exists on
  disk") -- `PersistArtifacts` completion hook.

## Compatibility decision

**adopt / exact.** No semantic divergence was found between the pinned
source and the harness descriptor; the Create/Update/Validate naming gap
above is a representational limit of the harness's single-action field, not
a behavioral mismatch.
