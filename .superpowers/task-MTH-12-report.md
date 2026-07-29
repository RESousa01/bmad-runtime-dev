# MTH-12 completion report

Date: 2026-07-29

Task: Complete canonical Method skill `bmad-architecture`

Canonical row: `method:src/bmm-skills/3-solutioning/bmad-architecture/SKILL.md`

## Outcome

MTH-12 is complete. The canonical Method 6.10.0 Architecture skill is
`adapted` and `runtime_available`, with a closed seven-member source package,
an action-neutral generic execution contract, and durable dispatch through the
existing Method session service.

The production runtime dispatch path now prepares the exact sealed request,
persists `Created`, persists `CapabilityBound`, and invalidates review into
`ContextReviewRequired` before any effect-capable port is reachable. The
source-derived integration test records those persistence events and executes
stale-binding rebind, fresh review, cancellation, restart-resume, tool failure,
validation failure, bounded retry, and bounded repair.

## Acceptance

1. **Recursive source closure:** `SKILL.md`, `customize.toml`, spine template,
   headless and reviewer references, linter, and linter tests are all bound to
   their pinned hashes. Closure is 100% and activation is allowed.
2. **Adopt/adapt reconciliation:** the semantic ledger, adoption ledger,
   normalized Architecture envelope, compatibility decision, catalog, Help
   package inventory, and documentation agree on the exact seven Architecture
   members and `adapt` treatment.
3. **Source semantics:** create/update/validate remain conversational source
   intents, not a fabricated fixed catalog action. The execution profile is
   `direct`, action is null, and interactive/headless modes share one contract.
4. **Real dispatch and persistence:** `BmadDomainRuntime::dispatch_skill_execution`
   calls exact preparation and `MethodSessionService`; repository evidence
   includes `CapabilityBound` and `ContextReviewRequested`.
5. **Authority separation:** file read, file write, process, network, subagent,
   browser, and external handoff remain distinct host routes. Source prose and
   model output grant no authority.
6. **Durability and recovery:** the Architecture integration test executes
   cancel, fail, retry, repair, restart-resume, and stale-source/config upgrade
   rebind through the established runtime APIs.
7. **Help and output topology:** generated Help projects Architecture as
   `adapted` / `runtime_available`, with output `planning_artifacts` and
   source-required primary follower `bmm:bmad-spec`.

## Exact five gates

| Gate | Result | Detail |
| --- | --- | --- |
| `node packages/bmad-foundation/scripts/resource-closure.mjs --source method@6.10.0 --skill bmad-architecture --check` | PASS | 7 members; 1/1 mandatory edge; 100% closure; activation allowed. |
| `node packages/bmad-foundation/scripts/conformance.mjs --skill bmad-architecture --all-modes --strict` | PASS | 1 canonical skill; strict; interactive and headless. |
| `cargo test -p desktop-runtime --test bmad_bmad_architecture --locked` | PASS | 6 passed, 0 failed. Final replay used `CARGO_INCREMENTAL=0` because the shared target's incremental cache filled the C: drive. |
| `pnpm --filter @sapphirus/bmad-foundation test -- --runInBand bmad-architecture` | PASS | 203 passed, 0 failed. |
| `pnpm verify:bmad:authority -- --skill bmad-architecture` | PASS | 1 canonical skill; no authority grant. |

Additional focused evidence:

- `cargo clippy -p desktop-runtime --test bmad_bmad_architecture --locked -- -D warnings`: pass.
- Desktop host canonical resolver coverage exists for PRD, Architecture, and
  implementation readiness. A fresh desktop-app test replay was attempted but
  its Tauri build script hit the known Windows `Access is denied` process-launch
  restriction after the runtime clippy gate had passed; this is not one of the
  five MTH-12 acceptance gates.
- Canonical receipts:
  `artifacts/bmad100/method/bmad-architecture/source-closure.json`,
  `conformance.json`, `authority.json`, and
  `golden/source-derived-cases.json`.

## BigBrain

Consulted and updated:

- `C:\Users\rodri\source\BigBrain\03-projects\sapphirus-bmad-runtime.md`
  (Method task guidance and the final MTH-12 outcome).

## Unresolved

- The C: drive had zero free bytes during final verification. Rust succeeded
  with incremental compilation disabled, but workspace disk pressure remains
  an operational concern outside MTH-12.
- The desktop-app focused replay remains unconfirmed in this final run because
  its Tauri build-script launch failed with Windows error 5. The exact required
  five gates are all green.
