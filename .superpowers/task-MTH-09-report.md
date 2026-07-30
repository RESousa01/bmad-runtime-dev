# MTH-09 completion report

Date: 2026-07-30

Status: complete

## Outcome

`bmm:bmad-spec` now has an exact five-resource source closure and an explicit
six-step execution contract for interactive and headless operation. Both
modes require `SPEC.md` and `.memlog`; companion files remain optional, while
stories remain an interactive-only optional branch as required by the pinned
source.

The canonical catalog row is `adapted`, `runtime_available`, and `qualified`
without a compatibility alias. The normalized envelope, managed instruction,
Help projection, generated UI/Rust consumers, runtime manifest, compatibility
ledger, adoption ledger, and source accounting all derive that same state.
The capability intentionally has no predecessor or follower; Help maps its
result through the generic governed-artifact output location.

Production dispatch resolves the generated canonical row, binds trusted host
identity, invokes the shared generic skill dispatcher, and persists the
`CapabilityBound` durable session state. Runtime tests cover cancel, actual
failure, fresh-session retry, repair, restart/resume, and stale-catalog
upgrade without granting source prose direct authority.

## Exact gates

1. Resource closure: pass, one skill, 100 percent, five accounted members.
2. Strict all-mode conformance: pass, interactive and headless.
3. Rust target: pass, 5 tests.
4. Foundation package: pass, 203 tests.
5. Authority verification: pass, one skill.

## Additional verification

- Foundation verifier: pass, 896 retained source members and 41 managed outputs.
- Canonical receipt is bound to the pinned source archive, package version,
  catalog, semantic ledger, runtime manifest, projection, and envelope hashes.
- Focused desktop production-caller test: pass, 1 test across interactive and
  headless modes; the real canonical dispatch persisted the Method session
  lifecycle through approval and completion.
- No alias, direct effect authority, network authority, or browser authority
  was introduced.

## Evidence

- `artifacts/bmad100/tasks/MTH-09.json`
- `artifacts/bmad100/method/bmad-spec/source-closure.json`
- `artifacts/bmad100/method/bmad-spec/conformance.json`
- `artifacts/bmad100/method/bmad-spec/authority.json`
- `artifacts/bmad100/method/bmad-spec/golden/source-derived-cases.json`

## BigBrain

Consulted: `03-projects/sapphirus-bmad-runtime.md`.

Updated: `03-projects/sapphirus-bmad-runtime.md` with the committed MTH-09
outcome and verification evidence.

Unresolved: none for MTH-09.
