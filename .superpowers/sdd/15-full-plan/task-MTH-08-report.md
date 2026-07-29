# MTH-08 implementation report

Date: 2026-07-29
Status: `DONE_WITH_CONCERNS` — the MTH-08 candidate is complete and the five binding gates pass. The coordinator still owns final tree binding, the task evidence record, lease removal, and dependent-task unblocking.

## Outcome and design boundary

The canonical Method capability `bmm:bmad-prd` is now source-accounted, normalized, executable through the existing generic Method preparation/session boundary, and reachable through the canonical catalog/Help/UI/desktop-host path.

- The normalized envelope closes all eight files in the pinned upstream `bmad-prd` directory and records exact hashes, both supported modes, the six governed tool intents, one document artifact, and the `MethodCheckpoint` completion hook.
- The Rust adapter declares the same eight resources and an explicit execution contract bound to the exact package, normalized envelope, catalog row, model-context profile, and input schema. It produces one `facilitate_prd` generic Method step rather than adding another executor, registry, session model, or effect port.
- Shared runtime preparation compiles an explicitly opted-in `BmadMethodSkill` into the existing `MethodExactBinding` and `MethodStepTable` lifecycle. Default behavior remains fail-closed when an adapter has no execution contract.
- Source accounting promotes only the canonical PRD row to `exact`, `runtime_available`, and `qualified`. Compatibility aliases remain unavailable.
- Catalog, Help graph, generated Rust/TypeScript, and desktop host availability are generated or checked against the compatibility ledger and source accounting. The Help capability graph is generated from the catalog and alias registry, eliminating its previous handwritten second source of truth.
- The desktop host fail-closes PRD dispatch unless the generated UI-advertised projection includes the canonical capability and the exact adapter execution contract validates.
- No source prose receives authority. The adapter declares typed tool intents and relies on the existing governed runtime ports, Airlock/effect boundary, checkpoint service, and evidence lifecycle.

## RED/GREEN evidence

The work used focused test-first changes:

1. PRD preparation initially failed with `ContractUnavailable`; after adding the explicit contract and generic preparation bridge, `bmad_bmad_prd` passed 5/5 in both modes.
2. Compatibility projection initially reported PRD as `implementation_pending`; after projecting support from the authoritative ledger/catalog pipeline, compatibility tests passed.
3. Source accounting initially reported PRD as `implementation_pending`; after the exact qualified mapping, source-accounting tests passed 3/3.
4. The stale handwritten Help graph failed exact-projection and runtime-availability expectations; after deterministic generation, Help catalog tests passed 7/7.
5. The generated catalog/UI contract was added RED and then passed 4/4 after fixing the generator projection.
6. The package suite initially had 25 failures. Twenty-two were cascading from two literal NUL bytes in the tracked dependency-graph verifier; replacing those bytes with source `\u0000` preserved separator semantics and restored the verifier to 5 nodes/4 edges. The final package suite passed 198/198.

## Binding verification

Pinned toolchain: Node `24.18.0`, pnpm `11.15.1`.

| Command | Exit | Result |
| --- | ---: | --- |
| `node packages/bmad-foundation/scripts/resource-closure.mjs --source method@6.10.0 --skill bmad-prd --check` | 0 | Activation allowed; 100% closure; 8/8 files accounted |
| `node packages/bmad-foundation/scripts/conformance.mjs --skill bmad-prd --all-modes --strict` | 0 | Strict, both modes, 1 exact PRD row |
| `cargo test -p desktop-runtime --test bmad_bmad_prd --locked` | 0 | 5 passed, 0 failed |
| `pnpm --filter @sapphirus/bmad-foundation test -- --runInBand bmad-prd` | 0 | 198 passed, 0 failed; the package runner ignores the pseudo-filter and ran the full suite |
| `pnpm verify:bmad:authority -- --skill bmad-prd` | 0 | 1 PRD authority row passed |

Additional focused checks:

- `rustfmt --edition 2021 --check` over the leased PRD, shared bridge, generated, and app Rust files: passed.
- `cargo clippy -p desktop-runtime --test bmad_bmad_prd --test bmad_skill_execution --locked -- -D warnings`: passed.
- `node packages/bmad-foundation/scripts/verify-dependency-graph.mjs`: passed, 5 nodes/4 edges.
- `node packages/bmad-foundation/scripts/generate-catalog.mjs --check`: passed.
- `node packages/bmad-foundation/scripts/reseal.mjs --check`: passed; manifest already sealed.
- `git diff --check`: passed before staging.

The broader `cargo fmt --all -- --check` still reports unrelated leased-out Foundry files. Focused formatting for every MTH-08 Rust file passes. A final `desktop-app` unit-test/clippy retry reached the Tauri custom build script but failed with Windows `Access is denied (os error 5)` before a Rust diagnostic. The same canonical desktop-host dispatch test had passed earlier in this task; the later app-only change was a sealed semantic-ledger hash string.

## Acceptance criteria

1. **Complete source accounting.** `packages/bmad-foundation/normalized/bmad-prd.package.json` lists the complete eight-file recursive closure with exact identities and classifications; `packages/bmad-foundation/source-accounting.json` reports the canonical row as exact/runtime-available/qualified; `artifacts/bmad100/method/bmad-prd/source-closure.json` records 100% accounting.
2. **Executable declared semantics.** The normalized descriptor and `crates/desktop-runtime/src/bmad/skills/adapters/bmad_prd.rs` bind actions, arguments, both modes, resources, predecessor/follower metadata, the document artifact, and completion hook to one validated generic Method step. `artifacts/bmad100/method/bmad-prd/conformance.json`, the Rust adapter tests, and golden cases prove the projection.
3. **Interactive/headless parity.** The contract explicitly allows both modes; the Rust test prepares exact bindings in both modes, strict all-mode conformance passes, and `artifacts/bmad100/method/bmad-prd/golden/source-derived-cases.json` records equivalent required outputs.
4. **Durable lifecycle behavior.** The adapter binds completion to `MethodCheckpoint`; the generic execution bridge uses the existing Method session/checkpoint service rather than new state. Shared bridge tests cover preparation and fail-closed behavior, while the golden cases specify cancel, fail, retry, repair, restart, resume, and stale-source upgrade semantics.
5. **Governed authority only.** The adapter declares six typed intents and imports no filesystem, process, network, scheduler, or provider API. The bridge prepares plans only and exposes no effects. `artifacts/bmad100/method/bmad-prd/authority.json` passes the canonical PRD row.
6. **One support truth.** `packages/bmad-foundation/capability-catalog.json`, `compatibility-aliases.json`, generated Help graph, generated Rust/TypeScript, source accounting, runtime manifest, and desktop host agree that canonical PRD is exact and runtime available. Catalog, compatibility, Help, foundation, and host tests enforce this projection.
7. **Primary skill active.** The canonical `bmm:bmad-prd` row is `exact`, `runtime_available`, and `qualified`; it is UI-advertised and dispatchable through the canonical host. Legacy alias/shim rows remain explicitly unavailable, so no pinned primary PRD capability remains deferred, rejected, unprojected, normalized-only, or unavailable.

## Fresh evidence

- `artifacts/bmad100/method/bmad-prd/source-closure.json`
- `artifacts/bmad100/method/bmad-prd/conformance.json`
- `artifacts/bmad100/method/bmad-prd/authority.json`
- `artifacts/bmad100/method/bmad-prd/golden/source-derived-cases.json`

These receipts are freshly regenerated from the passing candidate. The coordinator must bind the final commit/tree in `artifacts/bmad100/tasks/MTH-08.json`; this implementation did not edit that coordinator-owned artifact or `.bmad100/run-state.json`.

## Exact committed file scope

- `apps/desktop-ui/src/generated/bmadCapabilities.ts`
- `crates/desktop-app/src/bmad_capability_host.rs`
- `crates/desktop-app/src/bmad_foundation.rs`
- `crates/desktop-app/src/generated/bmad_capabilities.rs`
- `crates/desktop-runtime/src/bmad/mod.rs`
- `crates/desktop-runtime/src/bmad/skills/adapters/bmad_prd.rs`
- `crates/desktop-runtime/src/bmad/skills/execution.rs`
- `crates/desktop-runtime/src/bmad/skills/harness.rs`
- `crates/desktop-runtime/src/bmad/skills/mod.rs`
- `crates/desktop-runtime/src/lib.rs`
- `crates/desktop-runtime/tests/bmad_bmad_prd.rs`
- `crates/desktop-runtime/tests/bmad_skill_execution.rs`
- `docs/bmad/skills/bmad-prd.md`
- `packages/bmad-foundation/capability-catalog.json`
- `packages/bmad-foundation/compatibility-aliases.json`
- `packages/bmad-foundation/normalized/bmad-help-capability-graph.json`
- `packages/bmad-foundation/normalized/bmad-prd.package.json`
- `packages/bmad-foundation/runtime-manifest.json`
- `packages/bmad-foundation/scripts/generate-catalog.mjs`
- `packages/bmad-foundation/scripts/source-accounting.mjs`
- `packages/bmad-foundation/scripts/verify-dependency-graph.mjs`
- `packages/bmad-foundation/scripts/verify.mjs`
- `packages/bmad-foundation/source-accounting.json`
- `packages/bmad-foundation/tests/bmad-capability-catalog.test.mjs`
- `packages/bmad-foundation/tests/compatibility-ledger.test.mjs`
- `packages/bmad-foundation/tests/foundation.test.mjs`
- `packages/bmad-foundation/tests/help-catalog.test.mjs`
- `packages/bmad-foundation/tests/source-accounting.test.mjs`
- the four fresh PRD evidence receipts listed above
- `.superpowers/sdd/15-full-plan/task-MTH-08-report.md`

`crates/desktop-runtime/src/bmad/skills/adapters/mod.rs` was intentionally omitted: its current diff is only unrelated MTH-09/MTH-10 export reordering and is not needed by MTH-08. Coordinator state/artifacts and all Foundry, MTH-09, and MTH-10 files were also excluded.

## Self-review and handoff

- Reviewed the candidate against all seven acceptance criteria and the exclusive lease.
- Confirmed the bridge is opt-in and fail-closed, with no parallel executor, registry, session state, or effect port.
- Confirmed catalog/Help/UI support is derived from authoritative data and legacy negative aliases remain unavailable.
- Confirmed the five binding commands are green on the candidate and fresh evidence exists.
- Concern: the final desktop-app-only retry is environmentally blocked at the Tauri build script with Windows access denial; focused runtime tests/lint and the earlier desktop host test are green.
- Coordinator TODO: independently rerun gates, add final commit/tree bindings to the task artifact, remove the lease, and unblock dependents.

## BigBrain

Consulted: `C:\Users\rodri\source\BigBrain\03-projects\sapphirus-bmad-runtime.md` (MTH-08/BMAD completion context from line 2421 onward).
Updated: none; BigBrain is outside the exclusive implementation lease and durable checkpoint publication remains coordinator-owned.
Unresolved TODOs: final coordinator evidence/tree binding and, when the Windows process-launch restriction clears, one desktop-app test/clippy confirmation.

## Reviewer fix round 1 of 5

Date: 2026-07-29

All five Critical findings were repaired:

1. Production PRD preparation now resolves the canonical adapter, calls
   `prepare_skill_execution`, and exact-binds the resulting binding and step
   table into the existing `MethodSessionService`. The handwritten canonical
   Method registry predicate was removed.
2. The adapter now exposes source-faithful Create, Update, and Validate
   branches in both modes. Create and Update include activation, discovery or
   reconciliation, reviewer, and finalization stages; Validate emits Markdown
   and HTML validation reports. Existing Method-session transitions execute
   cancel, incomplete failure, repair, retry, restart, resume, and rebind
   upgrade behavior.
3. `BmadLibraryPanel`, skill availability, and Help runtime construction now
   consume generated support truth. The legacy UI decision matrix and old
   handwritten Help action graph no longer decide runtime availability.
4. Web access, browser control, and external handoff are distinct governed
   authority requirements.
5. Production-path consumer tests cover the library panel and canonical
   Method-session persistence. TypeScript typecheck and strict ESLint pass.

Fresh reviewer-round checks:

- five mandated binding gates: passed; foundation package 198/198
- focused Rust PRD and execution lifecycle tests: 10/10
- focused Rust clippy with `-D warnings`: passed
- UI consumer tests: 66/66
- desktop UI typecheck and ESLint with zero warnings: passed
- catalog generation, dependency graph, reseal, and `git diff --check`: passed
- repository-wide runtime clippy remains red on pre-existing warnings in
  unrelated runtime modules and tests
- desktop-app route test was retried twice and reached the Tauri custom build,
  which failed with `Access is denied. (os error 5)` before any Rust diagnostic

The pinned source hashes for ignored `.vscode/settings.json` and
`package-lock.json` were verified against the normalized manifest and both
files are force-included in the focused commit.

BigBrain consulted and updated:
`C:\Users\rodri\source\BigBrain\03-projects\sapphirus-bmad-runtime.md`.

Unresolved: the current desktop IPC has no PRD action selector, so production
dispatch uses the canonical Create branch; Update and Validate are compiled and
executable through the shared runtime/session contract but require a future IPC
surface to select them directly. The coordinator still owns final task/tree
binding and dependent-task unblocking.

## Reviewer fix round 2

Date: 2026-07-29
Status: `DONE`

The second reviewer round closes every remaining production and evidence gap:

- Product IPC and UI now select Create, Update, or Validate and Interactive or
  Headless. The selected mode is part of `MethodExactBinding`, so the two modes
  cannot collapse to one authority identity.
- Generated canonical catalog data selects the Method adapter and dispatch
  branch. `commands.rs` no longer constructs a PRD adapter or hardcodes the
  Create/Interactive branch.
- Prepare creates or resumes/rebinds the persisted Method session. Approve
  records the governed context decision; submit begins real Method advances,
  persists authenticated required artifacts and checkpoints every source-bound
  stage; cancel and provider failure persist terminal Cancelled and Incomplete
  states.
- Declared file, web, browser, external-handoff, process, and subagent
  requirements are converted into distinct governed authority-evidence
  dispositions and bound into each Method review decision.
- The sealed foundation now admits the complete 85-resource managed topology,
  including the reviewed root catalogs/provenance ledgers. `.gitattributes`
  preserves pinned upstream bytes, the runtime manifest is resealed, and the
  Method receipt regression covers short IDs and stale-receipt removal.
- The canonical updater-compatible task record is
  `artifacts/bmad100/tasks/MTH-08.json`. It binds lease baseline
  `15645c295c223bc9b509adf0dc53bbe937f43c7c`, candidate base
  `50591fd383442be9b4b20c8ca76dbb29a13a76d9`, and final verified tree
  `820fe9a065202499403a21f0d317b15dbe89fd17`.

Fresh clean-tree evidence at commit `cfdaef333bb285f8126e844d6225bafd85b2ad3c`:

- exact five MTH-08 gates: passed
- foundation package: 196/196
- production Method entrypoints: 4/4, including six action/mode completions,
  provider failure, and cancellation
- runtime PRD adapter: 5/5
- runtime Method lifecycle: 36/36
- desktop UI: 1540/1540; typecheck and zero-warning ESLint passed
- focused runtime clippy and desktop-app production-library clippy with
  `-D warnings`: passed
- repository-wide desktop-app test clippy still encounters two pre-existing
  `clippy::panic` findings in `tests/foundry_model_commands.rs`; MTH-08 does not
  modify that Foundry test.

BigBrain consulted and updated:
`C:\Users\rodri\source\BigBrain\03-projects\sapphirus-bmad-runtime.md`.

Unresolved questions or TODOs: none for MTH-08. Coordinator state/lease removal
and dependent-task unblocking remain coordinator-owned operations.
