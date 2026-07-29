# bmad-party-mode

- **rowId**: `method:src/core-skills/bmad-party-mode/SKILL.md`
- **canonicalId**: `core:bmad-party-mode`
- **pinned source**: `packages/bmad-foundation/source/method/6.10.0/src/core-skills/bmad-party-mode/SKILL.md`

## What it does

Orchestrates a round-table between installed BMAD agents or custom personas
who talk to each other and the user like distinct people, in one of four
run modes (`session`/`auto`/`subagent`/`agent-team`). Interactive and
open-ended by default -- it runs round after round until the user signals
done, the one exception being an explicit `--non-interactive` run. When
memory is enabled it keeps a per-party memlog; wrap-up offers a
self-contained keepsake HTML of the session.

## Declared actions and modes

- **Actions**: none declared as a fixed, separately-invocable action.
  Catalog row records `action: null`. The four run modes are resolved from
  `{workflow.party_mode}` or a runtime `--mode` flag, not fixed actions.
- **Modes**: `interactive`, `headless` (per capability-catalog.json; the
  source's one explicit non-interactive path is the `--non-interactive`
  flag).
- **Output location**: `memory_dir` and `output_dir`
  (`{workflow.memory_dir}/<party>/.memlog.md` and the Wrap-Up keepsake
  root).
- **Artifact kind**: `document`. Both the per-party memlog and the keepsake
  HTML are documents, persisted as written -- `PersistArtifacts` completion
  hook.

## Compatibility decision

**adopt / exact.** No semantic divergence was found between the pinned
source and the harness descriptor.
