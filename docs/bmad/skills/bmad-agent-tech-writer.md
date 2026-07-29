# bmad-agent-tech-writer

- **rowId**: `method:src/bmm-skills/1-analysis/bmad-agent-tech-writer/SKILL.md`
- **canonicalId**: `bmm:bmad-agent-tech-writer`
- **pinned source**: `packages/bmad-foundation/source/method/6.10.0/src/bmm-skills/1-analysis/bmad-agent-tech-writer/SKILL.md`

## What it does

Persona-adoption agent skill ("Paige — Technical Writer"). Same activation
shape as `bmad-agent-analyst`: resolves a customization-layered `agent`
block, runs prepend/append activation steps, adopts the Paige persona, loads
persistent facts and config, greets the user, then dispatches on a clear
intent or renders `{agent.menu}` and waits for input.

## Declared actions and modes

- **Actions**: none declared in the pinned source; menu dispatch is
  `customize.toml`-driven. Catalog row records `action: null`.
- **Modes**: `interactive`, `headless` (per capability-catalog.json).
- **Output location**: `{planning_artifacts}` (explicitly resolved in
  activation Step 5, "for output location and artifact scanning").
- **Artifact kind**: none declared -- persona adoption plus menu dispatch,
  no artifact produced by this skill itself.

## Compatibility decision

**adopt / exact.** No semantic divergence was found between the pinned
source and the harness descriptor.
