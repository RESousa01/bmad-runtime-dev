# bmad-agent-pm

- **rowId**: `method:src/bmm-skills/2-plan-workflows/bmad-agent-pm/SKILL.md`
- **canonicalId**: `bmm:bmad-agent-pm`
- **pinned source**: `packages/bmad-foundation/source/method/6.10.0/src/bmm-skills/2-plan-workflows/bmad-agent-pm/SKILL.md`

## What it does

Adopts the John / Product Manager persona. Resolves a customization-layered
`agent` block (base → team → user), runs prepend/append activation steps,
loads persistent facts and config, greets the user by name, and either
dispatches directly on a clearly-stated initial intent or renders
`{agent.menu}` as a numbered table and waits for input.

## Declared actions and modes

- **Actions**: none declared as a fixed, separately-invocable action.
  Catalog row records `action: null`. Dispatch is entirely
  `customize.toml`-driven menu resolution at runtime, not a fixed list this
  adapter can read.
- **Modes**: `interactive`, `headless` (per capability-catalog.json).
- **Output location**: `{planning_artifacts}` (resolved in activation
  Step 5, "for output location and artifact scanning").
- **Artifact kind**: none. The skill adopts a persona and dispatches to
  whatever menu item the user picks; it does not itself write a document.

## Compatibility decision

**adopt / exact.** No semantic divergence was found between the pinned
source and the harness descriptor. Structurally identical to the
`1-analysis` agent skills (`bmad-agent-analyst`, `bmad-agent-tech-writer`)
apart from the persona.
