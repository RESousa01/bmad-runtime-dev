# bmad-forge-idea

- **rowId**: `method:src/core-skills/bmad-forge-idea/SKILL.md`
- **canonicalId**: `core:bmad-forge-idea`
- **pinned source**: `packages/bmad-foundation/source/method/6.10.0/src/core-skills/bmad-forge-idea/SKILL.md`

## What it does

Pressure-tests a half-formed idea in conversation, one question at a time,
using two persona voices per turn, until it hardens, dies cheaply, or
becomes clearer. Every decision, assumption, crack, kill, and lock is
logged to a memlog. Exits Hardened (distills `forged-idea.md`), Killed, or
Clearer; every exit renders a self-contained `forge-report.html` stamped
with the outcome.

## Declared actions and modes

- **Actions**: none declared as a fixed, separately-invocable action.
  Catalog row records `action: null`. "attack this" / "defend this" /
  "switch roles" are in-conversation stance changes, not fixed actions.
- **Modes**: `interactive`, `headless` (per capability-catalog.json).
- **Output location**: `forge_output_path` (`{workflow.forge_output_path}`).
- **Artifact kind**: `document`. `forge-report.html` (always) and
  `forged-idea.md` (on Hardened) are both documents, persisted as written --
  `PersistArtifacts` completion hook.

## Compatibility decision

**adopt / exact.** No semantic divergence was found between the pinned
source and the harness descriptor. `follower` is left unset: the Hardened
exit names four legitimate next skills side by side
(`bmad-spec`, `bmad-prd`, `bmad-prfaq`, `bmad-quick-dev`), not one fixed
follower.
