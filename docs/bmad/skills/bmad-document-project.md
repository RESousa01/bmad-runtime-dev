# bmad-document-project

- **rowId**: `method:src/bmm-skills/1-analysis/bmad-document-project/SKILL.md`
- **canonicalId**: `bmm:bmad-document-project`
- **pinned source**: `packages/bmad-foundation/source/method/6.10.0/src/bmm-skills/1-analysis/bmad-document-project/SKILL.md`

## What it does

Documents brownfield projects for AI context. The activation shell resolves
a customization-layered `workflow` block, runs prepend/append activation
steps, loads persistent facts and `{project-root}/_bmad/bmm/config.yaml`,
greets the user, then defers its entire execution to
`./instructions.md` ("Read fully and follow: `./instructions.md`").

## Declared actions and modes

- **Actions**: none declared in the pinned `SKILL.md`. Catalog row records
  `action: null`.
- **Modes**: `interactive`, `headless` (per capability-catalog.json).
- **Output location**: `{planning_artifacts}` (explicitly resolved in
  activation Step 4, "for output location and artifact scanning").
- **Artifact kind**: not declared in `SKILL.md` -- the artifact kind and
  completion behavior live in `./instructions.md`, which is outside this
  adapter's parsed source (only the pinned `SKILL.md` is read). Reported as
  a fidelity gap rather than guessed.

## Compatibility decision

**adopt / exact.** Everything the pinned `SKILL.md` declares is represented;
the one gap (artifact kind living in `instructions.md`) is a scope boundary
of what this adapter parses, not a semantic divergence from what it does
parse.
