# bmad-review

- **rowId**: `method:src/core-skills/bmad-review/SKILL.md`
- **canonicalId**: `core:bmad-review`
- **pinned source**: `packages/bmad-foundation/source/method/6.10.0/src/core-skills/bmad-review/SKILL.md`

## What it does

Reviews content (diff, doc, spec, or any artifact) through whichever
installed lenses fit -- shipped lenses include adversarial, edge-case,
verification-gap, structure, and prose -- run singly or together per
`{workflow.lenses}`. Independent lenses run first (in parallel when
subagents are available), dependent lenses (`after`) run once their target
completes. Every finding shares one canonical JSON shape; `[]` is a valid,
clean result.

This rowId's catalog `aliases` list six legacy shims this consolidated
skill now serves, including `core:bmad-editorial-review` and
`core:bmad-review-adversarial-general`.

## Declared actions and modes

- **Actions**: none declared as a fixed, separately-invocable action.
  Catalog row records `action: null`. Lens selection is resolved from
  `{workflow.lenses}` and the caller's request, and the lens set itself is
  explicitly "not a fixed list" per the source's Overview.
- **Modes**: `interactive`, `headless` (per capability-catalog.json).
- **Output location**: `report_path` (`{workflow.report_path}`, when set).
- **Artifact kind**: `report`. The persisted form is the findings report,
  written on completion when `{workflow.report_path}` is set --
  `PersistArtifacts` completion hook.

## Compatibility decision

**adopt / exact.** No semantic divergence was found between the pinned
source and the harness descriptor.
