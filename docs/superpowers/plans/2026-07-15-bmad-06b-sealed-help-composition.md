# BMAD-06B Sealed Help Composition Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use
> `superpowers:subagent-driven-development` (recommended) or
> `superpowers:executing-plans`; use `superpowers:test-driven-development` for
> every behavior change and `superpowers:verification-before-completion` before
> accepting a checkpoint.

**Goal:** Bind the installed Method 6.10.0 `core/bmad-help` source to an exact,
non-runnable native invocation plan, then define the complete trusted-host path
from an untrusted bounded Help proposal to canonical recommendation, canonical
advance result, durable content, and replayable Method lineage without enabling
a model call.

**Architecture:** Generated cross-language contracts own the proposal and the
two host-canonical record shapes. `desktop-runtime` owns the verified package
source, compiler, proposal interpretation, Method transition, and canonical
hashes. `desktop-store` owns the only operation that may register canonical
content and persist the aggregate/checkpoint/evidence/outbox transaction.
Renderer, package content, and model output never choose tools, paths,
providers, storage references, evidence upgrades, or lifecycle transitions.
The resulting Help run remains `created_unbound`, `runnable: false`, and
`completion_claimed: false`; D2 activation is a later reviewed composition
gate.

**Tech stack:** JSON Schema 2020-12, Node.js 24.18.0, pnpm 11.12.0, TypeScript,
Rust 1.97.0, Serde, canonical SHA-256, C#/.NET 10.0.302, SQLite/rusqlite.

## Global constraints

- Treat `bmad-runtime-lib` as review-only context. Production code must use the
  independently reviewed managed projection under `packages/bmad-foundation`.
- Do not enable a model call, connect task submission, mark Help runnable,
  claim completion, or add apply/checkpoint/undo behavior in this milestone.
- Do not modify or cherry-pick the separate D2 worktree at `C:\tmp\d2`.
- Do not accept caller-supplied schema-closure, descriptor, instruction,
  customization, validation, egress, or binding hashes when a generated or
  sealed host-owned value exists.
- Do not expose arbitrary-byte constructors, `Deserialize`, or content-bearing
  `Debug` output for sealed managed instructions or trusted result envelopes.
- Preserve the exact execution profile, including Node `>=20.12.0` as a
  descriptive runtime and `completionEvidence == ["artifact"]`; neither grants
  tool or process authority.
- Strictly separate raw proposal bytes, canonical recommendation bytes, and the
  canonical advance result. `responseContentRef` points to the canonical
  recommendation, never to raw model output.
- Reuse the BMAD-06A aggregate validator. Failed validation/finalization leaves
  the authoritative aggregate, content links, checkpoint index, evidence, and
  outbox unchanged. Existing unreferenced CAS staging semantics remain honest.
- Existing Created/unbound v1 Help sessions must restore without migration.
- Preserve unrelated dirty files. Before each task, prove all planned files are
  clean or move the task to an isolated worktree. Stage only the task's exact
  files, run `git diff --cached --check`, and create one focused commit.
- Generated files are never hand-edited. Run the transactional generator and
  inspect the complete path-scoped output.
- Every task requires an independent spec/quality review before commit. If a
  review fails, fix and re-review the frozen task diff.

## Exact toolchain

Use this shell prefix for all Node/pnpm gates:

```powershell
$env:PATH = 'C:\tmp\sapphirus-toolchain;C:\tmp\node-v24.18.0-win-x64;' + $env:PATH
& 'C:\tmp\node-v24.18.0-win-x64\node.exe' --version
& 'C:\tmp\sapphirus-toolchain\pnpm.cmd' --version
```

Use `C:\Users\rodri\.cargo\bin\cargo.exe` for Rust gates. The qualified
cross-language generator must discover the signed host at
`C:\Program Files\dotnet\dotnet.exe` and requires exact SDK `10.0.302`; it
rejects inherited `DOTNET_ROOT`, portable-host substitution, and roll-forward.
Do not weaken this rule. If exact 10.0.302 is not available machine-wide, all
source work may continue, but the contract checkpoint remains unaccepted until
the user approves and the exact gate passes.

## Frozen contract decisions

Add three standalone roots, in alphabetical production-root order:

1. `bmad-method-advance-result.schema.json` / `MethodAdvanceResult`
2. `bmad-method-help-proposal.schema.json` / `MethodHelpProposal`
3. `bmad-method-help-recommendation.schema.json` /
   `MethodHelpRecommendation`

`MethodHelpRecommendation` and `MethodAdvanceResult` are thin `$ref` roots to
the already closed definitions in `bmad-method-session.schema.json`.
`MethodHelpProposal` is a closed union:

```text
recommended_capability:
  proposalKind       = "recommended_capability"
  capabilityKey      = complete BmadCapabilityKey
  evidenceTokenIds   = 1..64 unique ContractId values
  rationaleSummary   = 1..4096 safe Unicode scalar values

no_recommendation:
  proposalKind = "no_recommendation"
  reasonCode   = "catalog_evidence_absent" |
                 "completion_evidence_ambiguous" |
                 "dependency_unavailable"
```

The proposal has no schema version, self-hash, ID, timestamp, confidence,
artifact/content reference, authority, path, URI, provider, tool, effect,
completion, disposition, or lifecycle field.

Each root receives a generated schema-closure constant. Traverse `$ref`s
fragment-aware, record each document once as `{schemaId, canonicalSha256}`,
strictly sort by schema ID, canonicalize `{rootSchemaId, members}`, and take raw
SHA-256 over those JCS bytes. Expected closures are:

- proposal: proposal + capability catalog + common;
- recommendation: recommendation + Method session + capability catalog +
  common;
- advance: advance + Method session + common.

Add exactly two cross-language self-hash vectors:

- `bmad-method-help-recommendation/v1`, excluding `recommendationHash`;
- `bmad-method-canonical-advance-result/v1`, excluding `resultHash`.

The existing private Rust transition domain
`bmad-method-advance-result/v1` remains unchanged and distinct.

## Task 0: Requalify the Cargo bootstrap lock digest

**Completed:** commit `cddb8803` (`fix(contracts): requalify Cargo bootstrap
lock`).

**Files:**

- Modify: `tools/contract-codegen/tool-lock.json`
- Modify: `packages/contracts/scripts/lib/native-codegen.mjs`
- Test: `packages/contracts/tests/native-codegen-preflight.test.mjs` or the
  existing native preflight tests selected by `verify:typescript`

The committed `Cargo.lock` raw SHA-256 is
`03ba69718d4998793112dab704e54866da611542b10fb125c9b0d3e5b5f97071`.
The two source authorities still expect the obsolete reviewed digest
`34c68bd3920665cc5e59dcbf9ccccddfe295fc07cdbb50026726e76c1707aa22`.
Do not edit `Cargo.lock` in this task.

- [x] Preserve the observed RED: the qualified cross-language verifier reports
  `CONTRACT_LOCK_BOOTSTRAP_UNREVIEWED`, and TypeScript verification has exactly
  the native-preflight failures masked by that stale digest.
- [x] Independently hash the clean committed `Cargo.lock` bytes and prove the
  new literal matches them.
- [x] Replace the reviewed Cargo bootstrap digest in both source authorities.
- [x] Run the focused native preflight suite, then
  `pnpm --filter @sapphirus/contracts verify:typescript`.
- [x] Run `git diff --check --` for the two files and obtain independent review
  that no toolchain/version/production-root policy changed.
- [x] Commit: `fix(contracts): requalify Cargo bootstrap lock`.

## Task 1: Qualify standalone Help composition contracts

**Completed:** commit `bae0d01a` (`feat(contracts): qualify sealed Help
composition`). The exact Node/pnpm verifier passed once through the repository
entry point and once as its exact seven-stage direct replay because pnpm's
dependency-status preflight attempted an unnecessary workspace refresh. Rust
Clippy/tests and .NET 10.0.302 conformance passed; the independent review found
no Critical or Important issues.

**Files:**

- Add: `packages/contracts/schemas/bmad-method-advance-result.schema.json`
- Add: `packages/contracts/schemas/bmad-method-help-proposal.schema.json`
- Add: `packages/contracts/schemas/bmad-method-help-recommendation.schema.json`
- Modify: `tools/contract-codegen/tool-lock.json`
- Modify: `packages/contracts/scripts/lib/native-codegen.mjs`
- Modify: `packages/contracts/scripts/generate.mjs`
- Modify: `packages/contracts/scripts/lib/bmad-semantics.mjs`
- Modify: `packages/contracts/scripts/lib/semantics.mjs`
- Modify: `packages/contracts/scripts/lib/bmad-fixtures.mjs`
- Modify: `packages/contracts/scripts/check-typescript-bindings.mjs`
- Modify: `packages/contracts/tests/bmad-conformance.test.mjs`
- Modify: `packages/contracts/tests/standalone-validation.test.mjs`
- Modify: Rust and C# conformance semantic/test sources under
  `tests/conformance/{rust,dotnet}`
- Modify: `packages/contracts/README.md`
- Generate transactionally: schema lock, fixture/catalog/vector outputs,
  TypeScript/Rust/C# bindings, validators, declarations, and closure constants

- [x] Add RED tests for all three missing root validators and exported types;
  proposal branch bounds/unknown and duplicate members/unsafe text; strict UTC
  host records; semantic self-hash mismatch; eight-vector agreement; closure
  drift/collision/domain separation; and Rust/C# dispatch.
- [x] Register the roots identically in the production-root lock, generator,
  TypeScript target inventory, public type barrel, Ajv declarations/map, and
  binding checker.
- [x] Implement fragment-aware transitive closure qualification. Emit
  `SCHEMA_CLOSURES` in TypeScript, append generated Rust constants to
  `generated/rust/contracts.rs`, generate `BmadSchemaClosures.g.cs`, and retain
  full manifests in `schema-lock.json`.
- [x] Implement explicit semantic entry points:
  `validateMethodHelpProposalSemantics`,
  `validateMethodHelpRecommendationSemantics`, and
  `validateMethodAdvanceResultSemantics`.
- [x] Use the same safe-text predicate in JavaScript, Rust, and C#: reject C0
  `U+0000..U+001F`, DEL, and bidi controls `U+061C`, `U+200E`, `U+200F`,
  `U+202A..U+202E`, `U+2066..U+2069`. Keep catalog membership, evidence-token
  resolution, guidance derivation, lineage, and CAS outside contract semantics.
- [x] Extend fixture generation and tests from six to exactly eight qualified
  BMAD self-hash vectors; do not add a proposal self-hash.
- [x] Run `pnpm contracts:generate:cross-language`, inspect every generated
  path, then run `pnpm contracts:verify:cross-language` twice.
- [x] Run locked Rust and exact .NET 10.0.302 conformance suites. The checkpoint
  remains RED/blocked if the qualified generator cannot use the exact SDK.
- [x] Obtain independent cross-language spec/quality review and commit only the
  exact source plus deterministic generated outputs:
  `feat(contracts): qualify sealed Help composition`.

## Task 2: Retain the sealed installed Help source

**Completed:** commit `24c401b5` (`feat(bmad): retain sealed Help source`).
The package-owned wrapper retains the exact 1,283-byte managed instruction by
shared `Arc`, validates the complete descriptor/inventory/projection/profile/
config/module/ledger chain, and exposes no constructor or Serde/content-bearing
debug surface. The desktop app shares the manifest-owned bytes without a second
copy. The focused and full runtime/app suites, compile-fail privacy check,
formatting, and strict all-target Clippy passed. Independent review findings on
source-inventory cross-binding and byte duplication were fixed and the re-review
found no remaining Critical or Important issue.

**Files:**

- Modify: `crates/desktop-runtime/src/bmad/package.rs`
- Add/modify: `crates/desktop-runtime/src/bmad/help_run.rs`
- Modify: `crates/desktop-runtime/src/bmad/mod.rs`
- Modify: `crates/desktop-runtime/src/lib.rs`
- Modify: `crates/desktop-runtime/tests/bmad_kernel.rs`
- Add: `crates/desktop-runtime/tests/bmad_help_run.rs`
- Modify: `crates/desktop-app/src/bmad_foundation.rs`

- [x] Add RED tests for a package-owned sealed wrapper, exact 1,283 managed
  bytes/hash, redacted `Debug`, no Serde/arbitrary-byte construction, exact
  package/skill/profile/projection/config/module/ledger identities, and app
  manifest tamper rejection.
- [x] Make `BmadPackageLoader` build one `BmadLoadedMethodPackage` containing
  the existing display package and one opaque `BmadSealedHelpInvocation` while
  it still holds the generated descriptor, verified JSON, managed bytes, and
  manifest-verified ledgers. Keep `BmadLoadedSkill` unchanged.
- [x] Retain exact package, descriptor, source-snapshot, observed inventory,
  source/projection/resource/profile/config/module/ledger, distribution,
  install, validation-profile-name, and native catalog-binding facts. Retain
  instruction bytes in a private `Arc<[u8]>`; expose read-only access only.
- [x] Independently recompute and compare every nested hash:
  `bmad-execution-profile/v1` excluding `profileHash`;
  `bmad-skill-descriptor/v1` excluding `skillDescriptorHash`;
  `bmad-instruction-projection/v1` excluding `projectionHash`;
  `bmad-skill-resource-set/v1` over source entrypoint/resources/managed
  instruction; `bmad-config-graph/v1` excluding `graphHash`; and
  `bmad-config-resolution/v1` excluding `resolutionHash`.
- [x] Cross-bind exactly one `bmad-method/6.10.0`, `core/bmad-help` skill,
  projection, module, central graph/resolution, managed inventory entry, and
  five-member adoption closure (`method-001` through `method-005`). Validate
  source refs against descriptor inventory and pinned ledgers; validate only
  the managed instruction against observed snapshot bytes.
- [x] Require exact blocked intents `file_read` and `web`, their reviewed host
  replacements, direct profile, empty action set, no declared tools/state,
  Node runtime, artifact completion evidence, 64 KiB Help-specific byte bound,
  sorted/unique source closure, and exact managed path/format.
- [x] Extend `BmadLoadedFoundation` to retain the wrapper and delegate
  `package()` / `help_invocation()` without parsing, I/O, or a second byte copy.
- [x] Run focused runtime kernel/Help/app tests, formatting, and strict Clippy.
  Obtain independent review and commit:
  `feat(bmad): retain sealed Help source`.

## Task 3: Compile an exact non-runnable Help binding

**Completed:** commit `510a7fb8` (`feat(bmad): compile inert Help binding`).
The closed compiler consumes only the Task 2 sealed Help source, its exact
package-bound native catalog, and an opaque inert host model/profile assertion.
It derives the no-agent/direct binding, one-step `recommend` table, generated
schema identities, domain-separated empty-customization commitment, and fixed
validation-profile commitment. The aggregate shares the exact 1,283 instruction
bytes, retains both reviewed catalog candidates, implements neither Serde trait,
redacts debug output, and cannot become runnable or claim completion. Catalog
projection mutation and coordinated graph substitution fail closed. Focused and
full runtime/IPC/app suites, four compile-fail privacy tests, formatting,
CI-equivalent strict Clippy, and architecture boundaries passed. Independent
review found no P0-P3 issue.

**Files:**

- Add: `crates/desktop-runtime/src/bmad/help_binding.rs`
- Modify: `crates/desktop-runtime/src/bmad/catalog.rs`
- Modify: `crates/desktop-runtime/src/bmad/help_run.rs`
- Modify: `crates/desktop-runtime/src/bmad/kernel_error.rs`
- Modify: `crates/desktop-runtime/src/bmad/mod.rs`
- Modify: `crates/desktop-runtime/src/lib.rs`
- Modify: `crates/desktop-runtime/tests/bmad_help_run.rs`
- Modify only if needed for inert projection coverage:
  `crates/desktop-ipc/src/bmad_run.rs`,
  `crates/desktop-app/src/commands.rs`, and their focused tests

- [x] Add RED tests for exact no-agent/direct binding, exact instruction and
  native catalog retention, generated request/proposal/recommendation/result
  schema closures, fixed validation rules, domain-separated empty Help
  customization, model/request/egress facts, redaction, and non-runnability.
- [x] Define a closed compiler consuming only `&BmadSealedHelpInvocation`, the
  package-bound native catalog, and an opaque trusted D2 model/profile record.
  The D2 record is a host assertion needed to compile an inert plan; it is not
  evidence that consent, egress, or a model call occurred.
- [x] Derive the no-agent binding and exact direct Help step table locally.
  Derive an explicit domain-separated empty customization commitment from the
  capability identity and empty customization-layer set. Retain the core
  module metadata hash under its honest source name; do not alias it as a
  resolved customization graph.
- [x] Derive a fixed Help validation-profile commitment from a reviewed local
  canonical descriptor of the proposal/catalog/token/guidance/lineage rules.
  Add golden literal tests. Do not synthesize it from the profile name or reuse
  an execution/config/module hash.
- [x] Use generated closure constants directly. Reject coordinated descriptor,
  instruction, catalog, schema, customization, validation, model, request, or
  egress substitution. Keep exact managed bytes and facts behind sealed,
  redacted, non-Serde types.
- [x] Prove runtime, IPC, and app projections remain `created_unbound`,
  `runnable: false`, and `completion_claimed: false`; do not add an activation
  command.
- [x] Run focused runtime/IPC/app tests, formatting, strict Clippy, and
  architecture boundaries. Obtain independent review and commit:
  `feat(bmad): compile inert Help binding`.

## Task 4: Materialize canonical Help records from a verified proposal

**Files:**

- Modify: `crates/desktop-runtime/src/bmad/help_run.rs`
- Modify: `crates/desktop-runtime/src/bmad/method.rs`
- Modify: `crates/desktop-runtime/src/bmad/mod.rs`
- Modify: `crates/desktop-runtime/src/lib.rs`
- Modify: `crates/desktop-runtime/tests/bmad_help_run.rs`
- Modify: `crates/desktop-runtime/tests/bmad_method_session.rs`

- [ ] Add RED tests for byte limit, duplicate-key strict parsing, standalone
  schema/semantic validation, safe Unicode, exact non-`_meta` catalog member,
  unique known evidence tokens, evidence downgrade/no-upgrade, catalog-derived
  guidance, provable no-recommendation reasons, and aggregate non-mutation.
- [ ] Accept only an opaque verified-output/receipt boundary containing exact
  raw proposal bytes and pre-call D2/Method lineage. Until D2 composition lands,
  keep its trusted host constructor crate-private/test-only or behind the same
  explicit inert boundary used in BMAD-06A; do not claim cryptographic proof.
- [ ] Strict-parse and validate the proposal, preserve exact bytes and raw-byte
  hash, resolve tokens from the compiled host allowlist, and derive evidence
  class/guidance/no-recommendation truth exclusively from trusted catalog and
  token facts.
- [ ] Create a canonical `MethodHelpRecommendation` using host-owned ID,
  session ID, UTC time, resolved `ArtifactRef`s, derived evidence, and
  `bmad-method-help-recommendation/v1` self-hash. Registerable canonical bytes
  must be distinct from raw proposal bytes.
- [ ] Create a canonical completion-candidate `MethodAdvanceResult` using
  host-owned ID/time, exact request/invocation/schema lineage, a local
  `ContentRef` for canonical recommendation bytes, no produced artifacts, zero
  unresolved items, and `bmad-method-canonical-advance-result/v1` self-hash.
  The fixed transition remains Completed / recommend / no next step / empty
  working artifacts; no proposal field chooses it.
- [ ] Extend `MethodVerifiedResultBindingData`, checkpoints, private verification
  hash, persistence validation, and restart recomputation with
  `canonical_advance_result_hash` plus the exact canonical data required to
  recompute it. Reuse the aggregate's non-mutating verified-result validator.
- [ ] Preserve Created/unbound v1 compatibility and reject synchronized nested
  tampering, raw/canonical content substitution, invented absence, `_meta`,
  unknown token, evidence upgrade, hash/domain collision, and replay drift.
- [ ] Run focused Help/Method tests, full runtime tests, formatting, and strict
  Clippy. Obtain independent review and commit:
  `feat(bmad): canonicalize verified Help proposals`.

## Task 5: Atomically persist proposal and canonical Method lineage

**Files:**

- Modify only after proving clean ownership or using an isolated worktree:
  `crates/desktop-store/src/lib.rs`
- Modify only after proving clean ownership or using an isolated worktree:
  `crates/desktop-store/src/migrations.rs`
- Modify/create the focused BMAD Method store module selected by current source
  structure
- Modify: `crates/desktop-store/tests/bmad_method_store.rs`

The active workspace currently contains unrelated unstaged store changes. Do
not overwrite, stage, or silently combine them. Freeze their owner/base first;
implement this task in an isolated worktree if overlap remains.

- [ ] Add RED tests proving distinct raw proposal and canonical recommendation
  records, canonical content registration, exact content refs, aggregate and
  checkpoint persistence, evidence/outbox linkage, restart recomputation, CAS
  conflict behavior, relational rollback, and Created/unbound v1 restore.
- [ ] Add one store-owned finalization operation. It may stage encrypted
  content-addressed payloads, but only its SQLite transaction may link canonical
  recommendation content, aggregate projection, checkpoint index, evidence,
  and outbox as authoritative.
- [ ] Before commit, call the same non-mutating aggregate validator used by the
  live transition (or validate an exact clone), including
  `canonical_advance_result_hash`, schema closures, raw proposal hash,
  recommendation content ref/hash, D2/Method bridge, receipt evidence, and
  checkpoint chain.
- [ ] Prove every semantic/hash/CAS/transaction failure leaves all authoritative
  rows and the in-memory aggregate unchanged. Document only the already honest
  possibility of an unreferenced append-only CAS orphan.
- [ ] Add no migration unless a failing compatibility test proves persisted
  shape cannot remain nested JSON/backward compatible. If a migration is
  required, make it independently restart/idempotency reviewed.
- [ ] Run focused and full store tests, full workspace formatting/strict Clippy,
  and independent persistence review. Commit:
  `feat(bmad): persist canonical Help lineage`.

## Final acceptance

After all five implementation checkpoints pass independent review, run from a
clean exact-task baseline:

```powershell
$env:PATH = 'C:\tmp\sapphirus-toolchain;C:\tmp\node-v24.18.0-win-x64;' + $env:PATH
Remove-Item Env:DOTNET_ROOT -ErrorAction SilentlyContinue

& 'C:\tmp\sapphirus-toolchain\pnpm.cmd' contracts:verify:cross-language
& 'C:\tmp\sapphirus-toolchain\pnpm.cmd' contracts:verify:cross-language
& 'C:\tmp\sapphirus-toolchain\pnpm.cmd' contracts:verify:typescript
& 'C:\tmp\sapphirus-toolchain\pnpm.cmd' bmad:foundation:verify
& 'C:\tmp\sapphirus-toolchain\pnpm.cmd' typecheck
& 'C:\tmp\sapphirus-toolchain\pnpm.cmd' test
& 'C:\tmp\node-v24.18.0-win-x64\node.exe' tools/check-boundaries.mjs
& 'C:\tmp\sapphirus-toolchain\pnpm.cmd' --filter @sapphirus/desktop-ui build

& 'C:\Users\rodri\.cargo\bin\cargo.exe' fmt --all -- --check
& 'C:\Users\rodri\.cargo\bin\cargo.exe' clippy --workspace --all-targets --all-features --locked -- -D warnings
& 'C:\Users\rodri\.cargo\bin\cargo.exe' test --workspace --all-features --locked

& 'C:\Program Files\dotnet\dotnet.exe' --version
& 'C:\Program Files\dotnet\dotnet.exe' restore tests/conformance/dotnet/Sapphirus.Contracts.Conformance.Tests.csproj --locked-mode
& 'C:\Program Files\dotnet\dotnet.exe' test --project tests/conformance/dotnet/Sapphirus.Contracts.Conformance.Tests.csproj --configuration Release --no-restore

git diff --check
```

Request an independent whole-slice review for spec compliance, trust-boundary
integrity, cross-language agreement, restart/atomicity, and unintended
activation. Record exact commits and gate counts in BigBrain. Do not claim
BMAD-06B complete unless the exact .NET 10.0.302 gate and all final gates pass.

## Next task

Start Task 3: compile the sealed source into an exact non-runnable Help binding,
retain the generated schema closures and fixed local commitments, prove every
projection remains created/unbound, and commit it before canonicalizing any
verified proposal.
