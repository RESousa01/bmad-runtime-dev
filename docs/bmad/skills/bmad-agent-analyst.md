# bmad-agent-analyst

- **rowId**: `method:src/bmm-skills/1-analysis/bmad-agent-analyst/SKILL.md`
- **canonicalId**: `bmm:bmad-agent-analyst`
- **pinned source**: `packages/bmad-foundation/source/method/6.10.0/src/bmm-skills/1-analysis/bmad-agent-analyst/SKILL.md`

## What it does

Persona-adoption agent skill ("Mary — Business Analyst"). On activation it
resolves a customization-layered `agent` block (base `customize.toml` → team
→ user overlay), executes any prepend/append activation steps, adopts the
Mary persona, loads persistent facts and `{project-root}/_bmad/bmm/config.yaml`,
greets the user, and then either dispatches a clearly-intended request
directly or renders `{agent.menu}` as a numbered table and waits for input.

## Declared actions and modes

- **Actions**: none declared in the pinned source. Menu dispatch is
  entirely `customize.toml`-driven (`{agent.menu}`), not a fixed,
  separately-invocable action; the catalog row records `action: null`.
- **Modes**: `interactive`, `headless` (per capability-catalog.json).
- **Output location**: `{planning_artifacts}` (explicitly resolved in
  activation Step 5, "for output location and artifact scanning").
- **Artifact kind**: none declared -- the skill adopts a persona and
  dispatches to whatever menu item is chosen; it produces no artifact
  itself.

## Compatibility decision

**adopt / exact.** No semantic divergence was found between the pinned
source and the harness descriptor; every field either matches source text
directly or is left empty because the source does not declare it (see the
adapter's own doc comment for why `declared_actions` and `artifact_kinds`
are empty rather than guessed).
