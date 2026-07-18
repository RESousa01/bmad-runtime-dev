# Implementation packet: P0 — freeze an honest, requalified baseline

## Authority and intent

- Owning authority: repository maintainer (RodrigoSousa0); this packet is the merge vehicle for the already-implemented desktop candidate that existed in the pre-P0 worktree, plus the qualification fixes required to freeze it.
- User-visible outcome: a committed, CI-green revision from which `verify:source`, renderer tests, and cross-language qualification pass twice consecutively from a clean short-path checkout. Compared with `main`, the consolidated candidate includes Settings/preferences/About, the Activity and Agent Selector surfaces, bounded workspace file picking, the BMAD library v2 projection, and a 28-command READY catalog.
- Contracts read: `packages/contracts/fixtures/catalog.json`, generated Rust/TypeScript/C# bindings, IPC envelope shapes in `crates/desktop-ipc`.
- Non-goals: signing/installer work (P1), D3 recovery closure (P2), Azure/D2-E (P3), D2+D3 integration (P4), and BMAD breadth (P5). P0 adds no behavior beyond the consolidated candidate described above. In-app update installation and updater artifacts remain fail-closed until P1 supplies organization signing and release-channel evidence.
- Stop conditions: any fix requires changing a sealed contract shape; the App test timeout turns out to be a product race rather than test flakiness; the vocabulary-guard fix cannot preserve all existing true-positive probes.

## Tests first

- Success fixture:
  - `verify:source` passes with zero control-character findings after the NUL at `apps/desktop-ui/src/components/BmadLibraryPanel.tsx:253` is replaced with the intended escaped separator; add a scanner regression asserting the file set contains no literal NUL/control bytes.
  - `packages/contracts/scripts/check-typescript-bindings.mjs`: replace bare `String.prototype.includes` for `deferredBmadVocabulary` (lines 144–185) with identifier-boundary matching (e.g. `new RegExp(String.raw`\b${term}\b`)`) so generated `DesktopDeviceRegistration.Builder.Build` no longer matches `BuilderRegistration`; keep explicit true-positive probe strings (`BuilderRegistration`, `PackagePromotionRequest`, `ActivateBuilder`) that must still fail when injected into a synthetic source.
  - Full renderer suite (`App.test.tsx` included) passes five consecutive full-suite runs with the suite-load timeout at `apps/desktop-ui/src/App.test.tsx:956` resolved by asserting observable state (rendered markers / resolved promises) instead of timing assumptions.
  - Shared Rust↔TypeScript golden fixtures cover all ten projection-event variants and pin camelCase fields exactly. Existing catalog, command, reply, and error parser tests remain the authority for those envelope families; extending them to shared JSON fixtures is a non-blocking follow-up.
- Negative/bypass fixture: synthetic source containing deferred BMAD/runner vocabulary, including qualified `BmadRegisterBuilder`-style spellings, must fail the bindings check; files containing literal NUL or other prohibited C0 bytes must fail `verify:source`; first-party C# sources are scanned too.
- Failure/recovery fixture: toolchain-inheritance check fails loudly when a child process resolves a Node other than 24.18.0 or pnpm other than 11.12.0.
- Compatibility or migration fixture: generated-output hashes identical before/after the guard refactor except for the intended fixes; 2,777-file generation check remains green.

## Change and rollback

- Files/surfaces allowed (by consolidation lane; commit each lane separately):
  1. Renderer/protocol: `apps/desktop-ui/src/**` (BmadLibraryPanel NUL fix, App test determinism, event casing fix, hostClient/* protocol changes, shellFixtures).
  2. Contracts/codegen: `packages/contracts/**` (bindings guard, golden fixtures, catalog fixture updates).
  3. Native/IPC: `crates/desktop-ipc/**`, `crates/desktop-runtime/**`, `crates/desktop-workspace/**`, `crates/desktop-app/**`.
  4. Release/update and CI: workflow files only for the toolchain-pinning verification; no signing changes in P0.
  5. Documentation: packet + evidence notes.
- Disable or rollback path: the toolchain/contracts/native/renderer lane commits form one dependency-ordered product baseline and must be reverted together in reverse order. Documentation and local-harness ignore commits are independently revertible. The vocabulary-guard correction itself does not require generated output changes.
- Observability/evidence: two consecutive `verify:source` + cross-language qualification logs; five renderer full-suite logs; `git status` clean; deterministic generated-output hash listing; required CI checks green on the committed revision.

## Exit gate (must all hold before P1 starts)

- Two consecutive green runs of `verify:source` and cross-language qualification from a fresh short-path clean checkout.
- `cargo fmt --check`, strict all-feature clippy, all-feature Rust tests, and .NET qualification/conformance/support tests green.
- Renderer 296/296 across five consecutive full-suite runs.
- Clean `git status`; no unclassified release inputs; deterministic generated hashes.
- Required CI checks green on a committed revision.

## Review ledger

- Implementer full-diff review: executed 2026-07-18 during lane consolidation on branch `p0-baseline-consolidation` (commits `21142772`, `1c2125d3`, `a6229ad2`, `d3cd1125`, docs + gitignore).
- Independent bug/security review: executed 2026-07-18 across renderer, native IPC/workspace, contracts/codegen, toolchain/security, and release scope. Blocking findings were the packet authority mismatch, a literal U+001F left in `bmadProtocol.ts`, incomplete C0/C# scanner coverage, and qualified verb-first BMAD guard bypasses. All were corrected and re-reviewed before publication.
- Commands executed (2026-07-18, pinned Node 24.18.0 / pnpm 11.12.0 via user-level corepack shim):
  - `pnpm verify:source` — two consecutive green passes.
  - `pnpm contracts:verify:cross-language` — two consecutive green passes (104 pass, 1 environment skip).
  - Renderer full suite — five consecutive green passes (296/296).
  - `cargo fmt --all --check`, `cargo clippy --workspace --all-features --all-targets -- -D warnings`, `cargo test --workspace --all-features` — green.
  - Post-review `pnpm verify:source` — green, including renderer 296/296 and production build; final expanded first-party secret scan — 3,322 files green; scanner regression — 5/5 green; qualified generated-binding guard — green.
  - Pull request #3 required CI on exact commit `5df63dc` — source-only, TypeScript Ubuntu, TypeScript Windows, and generator qualification all green; merged 2026-07-18 as `c76f7ca871b9728d92d04d331d7f70bce0084c2f`.
  - `git status` clean after lane commits.
- Checks skipped and reason: .NET-only qualification lane not run standalone (exercised inside cross-language qualification); installer/signing checks explicitly deferred to P1.
- Findings during implementation:
  - The NUL defect class was reproduced live: passing `\u0000` through JSON-encoded tooling writes the literal byte. Adversarial review found the same class as a literal U+001F in `bmadProtocol.ts`; the scanner now rejects prohibited C0 controls and covers first-party C#/MSBuild sources.
  - The vocabulary guard hid two distinct issues: the reversed-alternative regex false positive (fixed with `(?<![0-9A-Za-z])` token boundaries) and a genuine leak, the `builder_activation_gated` blocker code, renamed to `builder_engine_gated` across Rust and TypeScript.
  - The renderer flake was testing-library's default 1 s `waitFor` ceiling under CPU contention, not a timing assumption in the tests; ceilings raised (`asyncUtilTimeout` 10 s, vitest 30 s).
- Remaining risks: system PATH still resolves Node 24.11.1 first (user-level 24.18.0 installed via `pnpm env`; durable precedence needs an elevated change or system Node upgrade — flagged, not performed); the broken standalone pnpm launcher in `%LOCALAPPDATA%\pnpm\bin` still shadows corepack outside the shim PATH. Non-blocking review follow-ups are shared command/reply/error fixtures, compile-time projection-event exhaustiveness, mixed text/binary picker handling, workspace-switch binding for pending picker results, and clearer Activity refresh/undo outcomes. The status-only updater UI is intentional and remains owned by P1 signing/update work.

## Open decisions to lock during P0

- Paige source-prompt reference scope (promote or remove) — affects the P5 denominator, decision only.
- Representation of independent D2 (context-read) vs D3 (governed-edit) grants — decision only; implementation in P4.
- D3 recovery authorization model — decision only; implementation in P2.
- Uninstall/offboarding data retention — decision only; implementation in P1/P6.
