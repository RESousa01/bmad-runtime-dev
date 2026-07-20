# Sapphirus 100 Percent Readiness Program Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move every capability in the 2026-07-20 readiness assessment to evidence-backed 100% on one immutable, signed, deployed, pilot-qualified revision.

**Architecture:** Preserve the signed Rust host as the sole local authority. Extend the existing sealed BMAD foundation through a generic capability-run layer, keep model output inert until it enters the existing D3 review/checkpoint/apply authority, finish the production D2 support-plane round trip, and qualify the exact same source revision through CI, Azure, signing, update, clean-machine, security, accessibility, recovery, and pilot gates. A machine-readable scorecard prevents scope changes or documentation-only claims from counting as completion.

**Tech Stack:** Windows 11, Tauri 2.11.x, Rust/Cargo 1.97.0, React 19, TypeScript 7.0.2, Node 24.18.0, pnpm 11.15.1, JSON Schema 2020-12, SQLite/DPAPI, .NET 10, Azure Container Apps, Azure SQL, App Configuration, Key Vault, Azure OpenAI, GitHub Actions/OIDC, NSIS, Authenticode.

## Global Constraints

- `desktop-app` remains the sole composition root and local lifecycle authority.
- The renderer receives bounded projections only; it cannot mint filesystem, model, signing, updater, or approval authority.
- Model output is data. It never directly writes files, runs commands, installs packages, changes policy, or approves its own proposal.
- D2 context-read and D3 governed-edit epochs remain independent as locked by ADR-0002.
- Every outbound request requires an exact, short-lived, single-use reviewed consent bound to the current workspace, capability, context, model profile, and session epoch.
- `bmad-runtime-lib` remains reference-only and absent from build, runtime, CI, and distribution inputs.
- Source semantics may be rewritten into repository-owned projections only after provenance, license, and source-intake review; source bodies are never copied into runtime artifacts.
- All JSON contracts are closed Draft 2020-12 schemas, generated into Rust, TypeScript, and C#, and cross-language qualified.
- Keep Node 24.18.0 as the release toolchain while it is Active LTS. If release freeze occurs after Node 26 enters Active LTS, execute a separately reviewed Node 26 pin migration before rebuilding release evidence.
- Every GitHub Action reference must be an immutable reviewed commit SHA.
- A capability reaches 100% only when its required evidence is attached to the same immutable Git revision and, where applicable, the same signed artifact digest and deployed container digest.
- No required gate may be waived by lowering a threshold, deleting a negative fixture, broadening an allowlist, or reclassifying unfinished work out of the denominator.

---

## Definition of 100%

| Capability | Required evidence for 100% |
|---|---|
| BMAD-06B/foundation | Relocation, provenance, managed-output, tamper, vocabulary, and cross-language gates pass twice from a clean checkout and once in required CI. |
| Full user-facing BMAD breadth beyond Help | All 26 roster menu targets are visible and executable through reviewed capability definitions; the five Builder authoring actions are usable as inactive-draft operations; no target is counted as complete merely because it is deferred or excluded. |
| Offline developer-checkout prototype | Fresh clone, frozen zero-download install from the approved cache, full verification, desktop launch, deterministic Help, D3 apply/undo/recovery, Builder drafts, and offboarding pass. |
| Reproducible/installable offline prototype | Two clean builds of the same revision produce matching normalized release metadata/SBOM/resource identities, and an offline installer completes install/launch/upgrade/uninstall on an isolated Windows account. |
| Complete current-source installable EXE | The exact candidate is Authenticode-signed and timestamped, attested, clean-machine qualified, update/rollback qualified, and promoted by the approved internal release authority. |
| Deterministic Help backend | All deterministic success, interruption, restart, replay, invalidation, malformed-output, and tamper paths pass with no network access. |
| User-facing deterministic Help | Keyboard-accessible review, approval, cancellation, completion, history, error, recovery, and restart states pass renderer and Windows end-to-end tests. |
| Production model-backed Help | One deployed test installation completes sign-in, registration, policy, lease, consent, no-store model call, signed receipt verification, local materialization, restart/history, revocation, and kill-switch tests. |
| D3 governed-edits backend | Proposal, review binding, approval, apply, checkpoint, undo, crash recovery, conflict, epoch, replay, and durable-integrity matrices pass on Windows. |
| User-facing governed edits | Model- and renderer-originated proposals share the same review/apply/undo/recovery UX and pass accessibility plus Windows end-to-end tests. |
| Integrated D2+D3 desktop | D2 and D3 independence, model-to-proposal conversion, sign-out, revocation, edit escalation, crash recovery, update blocking, and offboarding pass in one integrated test matrix. |
| First honest AI desktop prototype | A real provider response produces a verified Help result and a separate verified governed proposal that can only change files after fresh human review and D3 approval. |
| Horizontal governed foundation | Contracts, authority epochs, store integrity, egress, evidence, recovery, updater, offboarding, privacy, and supply-chain policies are enforced across every vertical. |
| Internal pilot readiness | Signed artifact and deployed support plane complete the security, privacy, accessibility, performance, incident, recovery, update, offboarding, support, and bounded pilot acceptance gates below. |

## Program dependency order

1. Gate A publishes P6 and locks the scorecard.
2. Gate B hardens CI and supply-chain evidence.
3. Gates C (BMAD breadth) and D (production D2) may proceed independently after Gate B.
4. Gate E connects verified model output to D3 and depends on C's capability contracts plus D's verified production response.
5. Gate F qualifies the signed installer and updater after C-E merge.
6. Gate G runs whole-product security, accessibility, performance, and recovery qualification on the signed candidate.
7. Gate H runs the pilot and closes the scorecard on the exact promoted digests.

---

### Task 1: Publish and remotely qualify the P6 baseline

**Files:**
- Verify: `docs/implementation-packets/P6-offboarding-retention-2026-07-20.md`
- Verify: `.github/workflows/source.yml`
- Verify: `.github/workflows/contracts.yml`

**Interfaces:**
- Consumes: local `main` revision `f53645efa09b5cc4ec5a7fc0fae72454fc21f60c`.
- Produces: an immutable `origin/main` revision containing P6 with required GitHub checks and no tree drift.

- [ ] **Step 1: Re-run the exact local gate before publication**

  Run with `C:\tmp\node-v24.18.0-win-x64` first on `PATH`:

  ```powershell
  pnpm verify:deferred-full
  git status --short
  ```

  Expected: exit 0; renderer 336/336 or higher; Rust workspace green; final status empty.

- [ ] **Step 2: Publish the existing six commits without rewriting them**

  ```powershell
  git branch codex/p6-offboarding-publication f53645efa09b5cc4ec5a7fc0fae72454fc21f60c
  git push -u origin codex/p6-offboarding-publication
  gh pr create --base main --head codex/p6-offboarding-publication --title "feat: close P6 offboarding retention" --body-file docs/implementation-packets/P6-offboarding-retention-2026-07-20.md
  ```

  Expected: one PR whose head SHA is exactly `f53645ef…`.

- [ ] **Step 3: Require independent review and all repository checks**

  ```powershell
  gh pr checks --watch
  gh pr view --json reviewDecision,mergeStateStatus,statusCheckRollup,headRefOid
  ```

  Expected: approved review, mergeable state, all required checks successful, exact head SHA.

- [ ] **Step 4: Merge and verify tree identity**

  Merge through the protected GitHub UI or approved `gh pr merge --merge`. Fetch without force-resetting local work, then compare the merged tree with P6:

  ```powershell
  git fetch origin --prune
  git diff --exit-code f53645efa09b5cc4ec5a7fc0fae72454fc21f60c^{tree} origin/main^{tree}
  ```

  Expected: exit 0. Record PR URL, merge SHA, checks, and tree comparison in the P6 review ledger.

---

### Task 2: Add a machine-enforced 100-percent scorecard

**Files:**
- Create: `docs/readiness/100-percent-scorecard.schema.json`
- Create: `docs/readiness/100-percent-scorecard.json`
- Create: `tools/check-readiness-scorecard.mjs`
- Create: `tools/check-readiness-scorecard.test.mjs`
- Modify: `package.json`
- Modify: `tools/check-boundaries.mjs`

**Interfaces:**
- Consumes: immutable Git revision, CI run URLs, artifact SHA-256 values, container digests, deployment identifiers, and pilot evidence summaries.
- Produces: `pnpm readiness:check`, which exits 0 only when all 14 capability records are `complete` and every required evidence reference is bound to the same release identity.

- [ ] **Step 1: Write failing scorecard tests**

  Cover: missing capability; percentage other than 100 for `complete`; unknown capability; empty evidence; mismatched source revision; mismatched installer/container digest; duplicate evidence kind; expired evidence; and all-green fixture.

  ```javascript
  assert.equal(validateScorecard(validFixture).releaseReady, true);
  assert.throws(
    () => validateScorecard({...validFixture, sourceRevision: "0".repeat(40)}),
    /source revision mismatch/,
  );
  ```

  Run: `node --test tools/check-readiness-scorecard.test.mjs`.
  Expected: FAIL because the validator does not exist.

- [ ] **Step 2: Define the closed scorecard shape**

  Each capability record must have this exact logical shape:

  ```json
  {
    "capability": "production_model_backed_help",
    "status": "incomplete",
    "percentage": 78,
    "requiredEvidenceKinds": ["source_ci", "azure_e2e", "signed_receipt"],
    "evidence": []
  }
  ```

  Permit only `incomplete` or `complete`; require percentage `100` for complete; require evidence records to contain `kind`, `sourceRevision`, `observedAt`, `urlOrRef`, and optional exact `installerSha256`/`containerDigest` values.

- [ ] **Step 3: Implement validation and boundary wiring**

  Export `validateReadinessScorecard(scorecard, expectedRevision)` from `tools/check-readiness-scorecard.mjs`. Add:

  ```json
  "readiness:check": "node tools/check-readiness-scorecard.mjs"
  ```

  to root scripts and make `check-boundaries.mjs` assert the exact 14-capability key set.

- [ ] **Step 4: Prove red and green states**

  ```powershell
  node --test tools/check-readiness-scorecard.test.mjs
  pnpm readiness:check
  ```

  Expected: tests pass; live scorecard exits non-zero until Gate H, listing missing evidence without paths, secrets, tokens, or raw provider content.

- [ ] **Step 5: Commit**

  ```powershell
  git add docs/readiness package.json tools/check-readiness-scorecard.mjs tools/check-readiness-scorecard.test.mjs tools/check-boundaries.mjs
  git commit -m "chore(readiness): enforce the 100 percent evidence scorecard"
  ```

---

### Task 3: Make CI and dependency policy production-grade

**Files:**
- Modify: `.github/workflows/source.yml`
- Modify: `.github/workflows/contracts.yml`
- Modify: `.github/workflows/desktop.yml`
- Modify: `.github/workflows/desktop-support.yml`
- Modify: `.github/workflows/security-nightly.yml`
- Modify: `tools/check-boundaries.mjs`
- Modify: `tools/check-boundaries.test.mjs`
- Create: `docs/security/dependency-and-action-policy.md`

**Interfaces:**
- Consumes: organization-approved registry/advisory sources and immutable action commits.
- Produces: required source/contracts/desktop/support checks plus scheduled security evidence with no mutable action reference.

- [ ] **Step 1: Add red boundary fixtures for every mutable action reference**

  The test must reject `@v4`, `@v6`, `@master`, branch names, and shortened SHAs in every workflow, not only release workflows.

  Run: `node --test tools/check-boundaries.test.mjs`.
  Expected: FAIL on current workflows.

- [ ] **Step 2: Replace every mutable action reference with a reviewed 40-character commit**

  Resolve each official tag to its peeled commit, review `action.yml`, record tag and commit in `docs/security/dependency-and-action-policy.md`, and pin checkout, Node, pnpm, .NET, Rust, Azure login, Anchore SBOM, and Anchore scan actions. Do not copy release-workflow SHAs into another action without verifying the upstream repository and tag.

- [ ] **Step 3: Activate security automation**

  Change `security-nightly.yml` to run on `schedule` and `workflow_dispatch`, keep permissions at `contents: read`, use exact cargo-deny `0.19.4`, run `pnpm audit --prod` only after the repository owner approves sending the lock graph to the configured registry, and otherwise use the organization-mirrored advisory database. Fail on Critical/High advisories, banned licenses, unapproved git sources, or duplicate cryptographic/runtime packages prohibited by policy.

- [ ] **Step 4: Remove skipped container security work**

  Pin both .NET base-image digests in `services/desktop-support-api/Dockerfile`. Make the support-container job fail if placeholders remain; build the image, create the SBOM, scan it, and upload hash-bound reports.

- [ ] **Step 5: Verify**

  ```powershell
  node --test tools/check-boundaries.test.mjs
  pnpm verify:boundaries
  pnpm verify:deferred-full
  ```

  Expected: all green and `rg -n "@v[0-9]|@master" .github/workflows` returns no matches.

- [ ] **Step 6: Commit**

  ```powershell
  git add .github/workflows services/desktop-support-api/Dockerfile docs/security tools/check-boundaries.mjs tools/check-boundaries.test.mjs
  git commit -m "ci(security): pin and enforce the complete supply chain"
  ```

---

### Task 4: Lock the full BMAD product denominator and source intake

**Files:**
- Create: `docs/adr/ADR-0005-full-bmad-capability-denominator.md`
- Create: `docs/implementation-packets/P8-full-bmad-capability-intake-2026-07-20.md`
- Create: `packages/bmad-foundation/capability-closure-ledger.json`
- Modify: `packages/bmad-foundation/adoption-ledger.json`
- Modify: `packages/bmad-foundation/semantic-source-ledger.json`
- Modify: `packages/bmad-foundation/scripts/verify.mjs`
- Modify: `packages/bmad-foundation/tests/foundation.test.mjs`

**Interfaces:**
- Consumes: all 26 exact roster menu targets, the five existing Builder instruction projections, reviewed source identities, license decisions, and human product approval.
- Produces: a monotonic ledger in which every target has a stable capability ID, output archetype, source decision, privacy class, authority class, and activation status.

- [ ] **Step 1: Add failing completeness tests**

  Extract the 26 roster target tuples `(agentCode, menuCode, capabilityKey)` and assert each has exactly one closure-ledger record. Also require Builder `agent.analyze`, `agent.create_rebuild`, `agent.edit`, `workflow.analyze`, and `workflow.build_edit`.

  Run: `pnpm --filter @sapphirus/bmad-foundation test`.
  Expected: FAIL because the closure ledger is absent.

- [ ] **Step 2: Approve ADR-0005 before semantic implementation**

  ADR-0005 must supersede only ADR-0003's executable-scope exclusions. It must keep the no-source-body rule and define these three output archetypes:

  ```text
  document_artifact       -> inert structured document stored locally
  governed_change_set     -> candidate D3 proposal requiring fresh review
  inactive_builder_draft  -> versioned draft that cannot install or activate
  ```

  Paige's five targets and John's PRD target require explicit first-party semantic rewrites and license/provenance approval. If any target cannot be legally or semantically rewritten, the program cannot claim 100% breadth; the denominator must not shrink.

- [ ] **Step 3: Create all closure records**

  Each record uses this closed shape:

  ```json
  {
    "capabilityId": "bmm:bmad-product-brief",
    "agentCodes": ["bmad-agent-analyst"],
    "menuCodes": ["CB"],
    "outputArchetype": "document_artifact",
    "authorityClass": "model_with_reviewed_context",
    "sourceDecision": "first_party_semantic_rewrite",
    "activationStatus": "planned",
    "managedProjection": null,
    "outputSchema": "sapphirus.bmad-document-artifact.v1"
  }
  ```

  Shared targets such as implementation readiness and document project may share one capability ID, but every agent/menu path remains independently counted and tested.

- [ ] **Step 4: Re-run foundation qualification**

  ```powershell
  pnpm bmad:foundation:verify
  pnpm --filter @sapphirus/bmad-foundation test
  pnpm contracts:verify:cross-language
  ```

  Expected: green; closure count exactly 26 menu paths plus five Builder operations; all remain `planned` until their implementation task lands.

- [ ] **Step 5: Commit**

  ```powershell
  git add docs/adr/ADR-0005-full-bmad-capability-denominator.md docs/implementation-packets/P8-full-bmad-capability-intake-2026-07-20.md packages/bmad-foundation
  git commit -m "docs(bmad): lock the complete capability denominator"
  ```

---

### Task 5: Build one generic sealed BMAD capability-run contract

**Files:**
- Create: `packages/contracts/schemas/bmad-capability-run.schema.json`
- Create: `packages/contracts/schemas/bmad-capability-result.schema.json`
- Modify: `packages/contracts/scripts/generate.mjs`
- Modify: `packages/contracts/tests/bmad-contracts.test.mjs`
- Create: `crates/desktop-runtime/src/bmad/capability.rs`
- Create: `crates/desktop-runtime/tests/bmad_capability.rs`
- Create: `crates/desktop-store/tests/bmad_capability_store.rs`
- Modify: `crates/desktop-store/src/lib.rs`
- Modify: `crates/desktop-runtime/src/bmad/mod.rs`

**Interfaces:**
- Consumes: `capabilityId`, sealed instruction projection, exact context manifest, output schema ID, model profile, and D2 consent evidence.
- Produces: immutable `BmadCapabilityRun` history and one tagged `BmadCapabilityResult` containing exactly one of `documentArtifact`, `governedChangeSet`, or `inactiveBuilderDraft`.

- [ ] **Step 1: Write red cross-language and Rust fixtures**

  Test valid results plus unknown fields, capability substitution, output-archetype substitution, schema mismatch, oversized text/files, absolute paths, authority fields, commands, tool calls, and approval claims.

- [ ] **Step 2: Define the runtime interface**

  Implement this public shape in `capability.rs`:

  ```rust
  pub enum BmadCapabilityOutput {
      DocumentArtifact(BmadDocumentArtifact),
      GovernedChangeSet(BmadGovernedChangeSet),
      InactiveBuilderDraft(BmadInactiveBuilderDraft),
  }

  pub struct BmadCapabilityRun {
      pub run_id: ContractId,
      pub capability_id: ContractId,
      pub workspace_id: ContractId,
      pub instruction_hash: Sha256Digest,
      pub context_manifest_hash: Sha256Digest,
      pub output_schema_id: ContractId,
      pub result: Option<BmadCapabilityOutput>,
  }
  ```

  Constructors must validate the exact closure-ledger binding and reject direct effect/authority fields.

- [ ] **Step 3: Persist runs atomically**

  Add schema version 11 tables for capability runs, result payload references, evidence references, and outbox events. Reuse encrypted CAS and standard durable envelopes. Migration tests must prove v10 history remains byte-valid and interrupted migration rolls back.

- [ ] **Step 4: Generate and qualify all languages**

  ```powershell
  pnpm contracts:generate:cross-language
  pnpm contracts:verify:cross-language
  cargo test -p desktop-runtime --test bmad_capability --all-features --locked
  cargo test -p desktop-store --test bmad_capability_store --all-features --locked
  ```

  Expected: all valid fixtures round-trip identically; all adversarial fixtures fail in Rust, TypeScript, and C# with the same stable category.

- [ ] **Step 5: Commit**

  ```powershell
  git add packages/contracts crates/desktop-runtime crates/desktop-store
  git commit -m "feat(bmad): add sealed generic capability runs"
  ```

---

### Task 6: Generalize Help's D2 lifecycle for all model-backed capabilities

**Files:**
- Create: `crates/desktop-app/src/bmad_model/capability_coordinator.rs`
- Create: `crates/desktop-app/src/bmad_model/capability_coordinator_tests.rs`
- Modify: `crates/desktop-app/src/bmad_model/coordinator.rs`
- Modify: `crates/desktop-app/src/bmad_model/bridge.rs`
- Modify: `crates/desktop-app/src/bmad_model/context.rs`
- Modify: `crates/desktop-egress/src/manifest.rs`
- Modify: `crates/desktop-egress/src/consent.rs`

**Interfaces:**
- Consumes: `BmadCapabilityRun`, P4 context-read epoch, sealed outbound manifest, one reviewed decision, and a `BmadCapabilityTransport`.
- Produces: review, approval, cancellation, advancing, completed, and terminal projections that are capability-bound and reusable by Help and every P8 vertical.

- [ ] **Step 1: Add red parity and substitution tests**

  Clone Help's lifecycle matrix and parameterize it with two different capability IDs. Prove that a decision, manifest, output schema, instruction projection, or result from capability A cannot be used by capability B.

- [ ] **Step 2: Introduce the transport boundary**

  ```rust
  #[async_trait::async_trait]
  pub trait BmadCapabilityTransport: Send + Sync {
      async fn send(
          &self,
          request: AuthorizedModelRequest,
          now: UnixMillis,
      ) -> Result<(DispatchedModelRequest, RawModelOutput), CloudError>;
  }
  ```

  Keep deterministic and offline adapters explicit. Do not hold the coordinator mutex, renderer guard, workspace guard, or store transaction across `.await`; transition durably to `advancing`, release guards, dispatch, then re-authenticate every binding before finalization.

- [ ] **Step 3: Move Help onto the generic coordinator**

  Preserve Help's existing command names and wire payloads through an adapter. Golden projection fixtures must remain byte-identical.

- [ ] **Step 4: Run focused and complete gates**

  ```powershell
  cargo test -p desktop-app bmad_model::capability_coordinator_tests --all-features --locked
  cargo test -p desktop-egress --all-features --locked
  pnpm verify:deferred-full
  ```

  Expected: Help compatibility green; cross-capability substitution and cancellation races fail closed.

- [ ] **Step 5: Commit**

  ```powershell
  git add crates/desktop-app/src/bmad_model crates/desktop-egress
  git commit -m "refactor(bmad): generalize reviewed model capability runs"
  ```

---

### Task 7: Implement all 26 roster menu paths

**Files:**
- Modify: `packages/bmad-foundation/runtime/method/6.10.0/`
- Modify: `packages/bmad-foundation/runtime-manifest.json`
- Modify: `packages/bmad-foundation/capability-closure-ledger.json`
- Modify: `packages/bmad-foundation/semantic-source-ledger.json`
- Modify: `crates/desktop-runtime/src/bmad/capability.rs`
- Modify: `crates/desktop-ipc/src/envelope.rs`
- Create: `crates/desktop-ipc/src/bmad_capability.rs`
- Modify: `crates/desktop-app/src/commands.rs`
- Modify: `crates/desktop-app/src/lib.rs`
- Create: `apps/desktop-ui/src/components/BmadCapabilityPanel.tsx`
- Create: `apps/desktop-ui/src/components/BmadCapabilityPanel.test.tsx`
- Modify: `apps/desktop-ui/src/components/BmadLibraryPanel.tsx`
- Modify: `apps/desktop-ui/src/lib/hostClient/`

**Interfaces:**
- Consumes: the generic capability-run lifecycle and 26 reviewed closure records.
- Produces: every roster menu item can start, review, approve/cancel, complete, persist, reopen, and display its exact archetype.

- [ ] **Step 1: Implement repository-owned semantic projections by capability family**

  Add one managed instruction projection per unique capability key. Cover Mary BP/MR/DR/TR/CB/WB/DP; Winston CA/IR; John PRD/CE/IR/CC; Sally CU; Paige DP/WD/MG/VD/EC; Amelia DS/QD/QA/CR/SP/CS/ER. Shared capability keys reuse one projection only when source semantics and output schema are identical.

- [ ] **Step 2: Activate document-artifact capabilities first**

  Use a closed artifact result containing title, sections, evidence references, open questions, and optional Mermaid text. Reject HTML, scripts, URLs outside reviewed evidence refs, filesystem paths, and any claim that the artifact was written or applied.

- [ ] **Step 3: Activate governed-change-set capabilities**

  DS, QD, QA, and any capability proposing workspace changes emit only canonical relative-path create/replace/delete candidates with preimage hashes. They do not execute tests or commands and do not write files in this task.

- [ ] **Step 4: Add strict IPC commands**

  Add `bmad.capability.prepare`, `.approve`, `.cancel`, `.submit`, and `.latest`. Bind all five to the capability ID, renderer session, workspace ID, context-read epoch, catalog version, and request fingerprint. Update all catalog pin sites and boundary count tests together.

- [ ] **Step 5: Add the unified renderer flow**

  `BmadLibraryPanel` launches `BmadCapabilityPanel`; the panel shows source-grounded description, exact context review, destination/model status, consent, completion, history, and artifact-specific output. Governed changes display “Review in Changes” and never an apply button inside the model result.

- [ ] **Step 6: Prove every menu path**

  Generate a table-driven fixture with all 26 `(agentCode, menuCode)` paths. Each must launch the expected capability and reject swapped agent/menu/capability bindings. Run axe on empty, review, completed-document, governed-change, error, and restart states.

- [ ] **Step 7: Re-measure the denominator**

  Change every menu-path closure record from `planned` to `active` only after its focused tests pass. Foundation verification must report 26/26 active menu paths and zero excluded/deferred menu path.

- [ ] **Step 8: Verify and commit in reviewable family commits**

  For each family run foundation, cross-language, relevant Rust, renderer, and boundary tests before committing. After the final family:

  ```powershell
  pnpm verify:deferred-full
  ```

  Expected: exit 0 and 26/26 executable menu paths.

---

### Task 8: Activate the five Builder authoring operations as inactive drafts

**Files:**
- Create: `docs/adr/ADR-0006-builder-authoring-activation.md`
- Modify: `crates/desktop-runtime/src/bmad/builder.rs`
- Modify: `crates/desktop-store/tests/bmad_builder_store.rs`
- Create: `crates/desktop-ipc/src/bmad_builder.rs`
- Modify: `crates/desktop-ipc/src/envelope.rs`
- Modify: `crates/desktop-app/src/commands.rs`
- Create: `apps/desktop-ui/src/components/BmadBuilderPanel.tsx`
- Create: `apps/desktop-ui/src/components/BmadBuilderPanel.test.tsx`
- Modify: `packages/bmad-foundation/capability-closure-ledger.json`

**Interfaces:**
- Consumes: existing immutable Builder aggregate, five sealed Builder instruction projections, generic capability coordinator, and encrypted Builder store.
- Produces: Analyze/Create-Rebuild/Edit Agent and Analyze/Build-Edit Workflow draft flows; outputs remain inactive and cannot register, install, execute, or alter the capability catalog.

- [ ] **Step 1: Approve ADR-0006 and add red authority tests**

  Lock the inactive-draft boundary. Tests must reject activation, registration, package installation, arbitrary files, scripts, hooks, commands, network grants, and catalog mutation.

- [ ] **Step 2: Add strict Builder IPC and host composition**

  Add prepare/approve/cancel/submit/latest commands using a separate Builder capability family. Persist immutable revisions and consumed model-lens decisions through the existing Builder repository.

- [ ] **Step 3: Implement the Builder UI**

  Provide kind selection, revision history, analysis result, exact file inventory preview, and explicit inactive status. Never label drafts installed, valid for execution, or available in the main capability catalog.

- [ ] **Step 4: Qualify all five operations**

  Run Builder domain/store/IPC/renderer tests, restart and tamper matrices, cross-language qualification, and full deferred verification. Change the five closure records to `active` only after the complete gate passes.

- [ ] **Step 5: Commit**

  ```powershell
  git add docs/adr/ADR-0006-builder-authoring-activation.md crates/desktop-runtime crates/desktop-store crates/desktop-ipc crates/desktop-app apps/desktop-ui packages/bmad-foundation
  git commit -m "feat(bmad): activate governed inactive Builder drafts"
  ```

---

### Task 9: Complete the production desktop-to-support-plane round trip

**Files:**
- Create: `crates/desktop-cloud/src/production_round_trip.rs`
- Create: `crates/desktop-cloud/tests/production_round_trip.rs`
- Modify: `crates/desktop-cloud/src/production.rs`
- Modify: `crates/desktop-cloud/src/transport.rs`
- Modify: `crates/desktop-app/src/bmad_model/transport.rs`
- Modify: `crates/desktop-app/src/bmad_model/coordinator.rs`
- Modify: `helpers/windows-auth-broker/`
- Modify: `services/desktop-support-api/Routes.cs`
- Modify: `services/desktop-support-api.Tests/`

**Interfaces:**
- Consumes: `CloudSession`, WAM access, `ProductionSupportClient`, `SupportApiTransport`, installation key, signed policy/lease types, canonical model-access request, and receipt verifier.
- Produces: `ProductionRoundTrip::send(AuthorizedModelRequest, UnixMillis) -> Result<(DispatchedModelRequest, RawModelOutput), CloudError>`.

- [ ] **Step 1: Write a red scripted-server lifecycle test**

  The test server must require this exact sequence: bootstrap, register/recover installation, policy, lease, model access, receipt. Assert bearer scope, idempotency key, consent signature, registration/policy/lease bindings, transient-no-store profile, and receipt proof. Add failures for every reordering, stale document, wrong tenant/region/key, replay, timeout, oversized body, redirect, and sign-out race.

- [ ] **Step 2: Implement `ProductionRoundTrip`**

  Compose existing verification methods rather than duplicating them:

  ```rust
  pub struct ProductionRoundTrip<B, E> {
      session: CloudSession<B>,
      client: ProductionSupportClient,
      transport: SupportApiTransport<E>,
      installation_identity: WindowsInstallationIdentity,
  }
  ```

  Every server document is untrusted until `accept_policy`, `accept_lease`, or `accept_receipt_proof` succeeds. Persist only verified public documents and renderer-safe completion evidence.

- [ ] **Step 3: Replace the production offline stub**

  `ProductionHelpTransport` owns the round-trip and calls it asynchronously. Offline and deterministic feature compositions remain behaviorally unchanged. Partial production configuration still composes offline; exact production configuration that cannot initialize must fail closed.

- [ ] **Step 4: Complete server route parity**

  Ensure bootstrap/registration/policy/lease/model routes expose only the canonical shapes consumed by Rust. Add C#/Rust golden fixtures for every signed document and error code.

- [ ] **Step 5: Verify**

  ```powershell
  cargo test -p desktop-cloud --all-features --locked
  cargo test -p desktop-app --all-features --locked
  dotnet test services/desktop-support-api.Tests/Sapphirus.DesktopSupportApi.Tests.csproj --no-restore
  pnpm contracts:verify:cross-language
  ```

  Expected: scripted full lifecycle and all fail-closed variants pass; no production path returns the deliberate offline stub.

- [ ] **Step 6: Commit**

  ```powershell
  git add crates/desktop-cloud crates/desktop-app helpers/windows-auth-broker services/desktop-support-api services/desktop-support-api.Tests
  git commit -m "feat(d2): complete the production Help round trip"
  ```

---

### Task 10: Deploy and qualify the Azure support plane

**Files:**
- Modify: `.github/workflows/desktop-support.yml`
- Modify: `infra/desktop-support/main.bicep`
- Modify: `infra/desktop-support/main.example.bicepparam`
- Modify: `tools/support-smoke/deployed-smoke.ps1`
- Create: `tools/support-smoke/full-round-trip.ps1`
- Create: `tools/support-smoke/failure-injection.ps1`
- Modify: `docs/superpowers/plans/2026-07-20-d2-e-rollout-runbook.md`
- Create: `docs/qualification-evidence/d2-production-summary.json`

**Interfaces:**
- Consumes: signed/SBOM-attested container digest, organization Azure subscription, protected GitHub environment, OIDC identity, SQL migration identity, policy/signing/model identities, and test tenant.
- Produces: immutable deployed digest plus successful smoke, privacy, end-to-end, failure-injection, key-rotation, backup/restore, alert, and kill-switch evidence.

- [ ] **Step 1: Turn Azure gates into executable automation**

  Replace the current SQL-inspection `Write-Host` placeholder with an actual `sqlcmd` invocation under the read-only inspection identity. Add full-round-trip and failure-injection scripts with bounded parameters and safe output.

- [ ] **Step 2: Validate infrastructure before applying**

  Run Bicep build, resource-group validate, and what-if. Human review must confirm no local auth, public data plane, plaintext secret, mutable image tag, over-broad role, caller-controlled model endpoint, or destructive SQL migration.

- [ ] **Step 3: Deploy in the runbook’s fixed order**

  Deploy infrastructure with API disabled; apply migrations under the migration identity; publish signed image by digest; deploy model-disabled revision; enable alerts; run privacy/health gates; enable one fixed deployment for the test group.

- [ ] **Step 4: Run the real end-to-end and failure matrix**

  Require successful sign-in → registration → signed policy → lease → reviewed consent → one no-store model call → signed receipt → Rust verification → local Help completion. Then exercise revocation race, SQL transient, Key Vault throttle, model timeout, receipt replay, key rotation overlap, Container Apps revision rollback, database point-in-time restore, and policy kill switch.

- [ ] **Step 5: Record bounded durable evidence**

  `d2-production-summary.json` records source revision, container digest, deployment revision, workflow URLs, test counts, timestamps, key-version identifiers, and pass/fail only. It must contain no tenant subject, access token, prompt, response body, SQL content, or secret URI.

- [ ] **Step 6: Commit automation and evidence summary**

  ```powershell
  git add .github/workflows/desktop-support.yml infra/desktop-support tools/support-smoke docs/superpowers/plans/2026-07-20-d2-e-rollout-runbook.md docs/qualification-evidence/d2-production-summary.json
  git commit -m "ops(d2): qualify the deployed production support plane"
  ```

---

### Task 11: Convert verified model change output into D3 proposals

**Files:**
- Create: `docs/adr/ADR-0007-model-to-governed-edits.md`
- Create: `crates/desktop-app/src/bmad_governed_proposal.rs`
- Create: `crates/desktop-app/tests/bmad_governed_proposal.rs`
- Modify: `crates/desktop-app/src/edits.rs`
- Modify: `crates/desktop-runtime/src/edits.rs`
- Modify: `crates/desktop-store/src/lib.rs`
- Modify: `apps/desktop-ui/src/components/BmadCapabilityPanel.tsx`
- Modify: `apps/desktop-ui/src/components/GovernedChangesPanel.tsx`

**Interfaces:**
- Consumes: verified `BmadGovernedChangeSet`, exact workspace observations, current governed-edit epoch, and verified model receipt evidence.
- Produces: an ordinary sealed D3 candidate proposal; it has no approval and cannot apply until the existing Changes review creates a fresh D3 decision.

- [ ] **Step 1: Lock the authority design in ADR-0007**

  Specify that D2 consent authorizes only egress/model use. D3 approval remains separate, local, fresh, single-use, and bound to newly observed preimages. The model receipt is evidence, never authority.

- [ ] **Step 2: Add red transplantation and race tests**

  Reject workspace, capability, model receipt, path, content, preimage, grant epoch, edit epoch, renderer session, and run-ID substitution. Reject external edits after model completion and before D3 review. Prove sign-out does not invalidate an already reviewed D3 proposal, while edit-epoch change does.

- [ ] **Step 3: Implement the adapter**

  ```rust
  pub fn propose_verified_bmad_changes(
      output: &BmadGovernedChangeSet,
      receipt: &VerifiedModelReceiptEvidence,
      authority: &GovernedWorkspaceAuthority,
  ) -> Result<SealedCandidateAction, LocalError>
  ```

  Re-observe every target through governed workspace I/O, derive preimages locally, reject sensitive/unsupported paths, then call the existing D3 candidate constructor.

- [ ] **Step 4: Join the existing user flow**

  The completed capability result exposes “Review in Changes.” Changes shows origin `BMAD <capability>` and receipt status, but uses the same diff, approval, apply, checkpoint, undo, conflict, and recovery components as renderer-originated proposals.

- [ ] **Step 5: Run the integrated matrix**

  Test deterministic and production model outputs, D2 sign-out, D3 escalation, whole-workspace revocation, apply/undo, crash recovery, interrupted restore, update blocking, and offboarding erasure.

- [ ] **Step 6: Commit**

  ```powershell
  git add docs/adr/ADR-0007-model-to-governed-edits.md crates/desktop-app crates/desktop-runtime crates/desktop-store apps/desktop-ui
  git commit -m "feat(d3): route verified BMAD output through governed proposals"
  ```

---

### Task 12: Qualify the signed installer, updater, rollback, and offboarding lifecycle

**Files:**
- Create: `crates/desktop-app/tauri.release.conf.json`
- Modify: `crates/desktop-app/tauri.conf.json`
- Modify: `crates/desktop-app/src/update.rs`
- Modify: `crates/desktop-update/src/lib.rs`
- Modify: `.github/workflows/release-windows-signed.yml`
- Modify: `tools/build-signed-windows-installer.ps1`
- Modify: `tools/qualify-windows-installer.ps1`
- Create: `tools/qualify-windows-update.ps1`
- Create: `tools/release-update-manifest.test.mjs`
- Modify: `tools/create-release-attestation-predicate.mjs`
- Create: `docs/qualification-evidence/signed-release-summary.json`

**Interfaces:**
- Consumes: exact merged source revision, Authenticode certificate, timestamp service, Tauri updater signing key, approved prior installer, protected signing and qualification runners.
- Produces: signed/timestamped EXE and NSIS installer, signed updater artifacts, build/SBOM/qualification attestations, install/upgrade/update/rollback/uninstall/offboarding evidence, and approved release summary.

- [ ] **Step 1: Add a release-only Tauri overlay**

  Keep base `createUpdaterArtifacts: false`. The release overlay sets it to true and is accepted only by the signed workflow with non-empty HTTPS endpoint and public key. Tests must prove developer and deterministic builds cannot emit updater artifacts.

- [ ] **Step 2: Extend lifecycle evidence**

  Add two isolated-account scenarios: uninstall without in-app erase preserves documented app-owned retained state and never touches workspace files; in-app typed erase followed by uninstall leaves no app-owned database/key/CAS/install-root/uninstall-registration residue while workspace files remain byte-identical.

- [ ] **Step 3: Add signed update and rollback qualification**

  Install prior signed version, update to current via the signed immutable feed, verify publisher/version/hash/BMAD resources and launch, simulate interrupted download, reject unsigned/wrong-key/downgrade manifests, roll back by promoting the prior signed feed entry under the documented incident process, then upgrade again.

- [ ] **Step 4: Provision and execute protected lanes**

  Enable `SAPPHIRUS_NATIVE_LANE_ENABLED` and `SAPPHIRUS_SIGNING_LANE_ENABLED` only after runner, environment, certificate, timestamp, and prior-artifact controls are reviewed. Dispatch from exact protected `main`; require independent qualification runner and post-qualification attestations.

- [ ] **Step 5: Bind the durable release summary**

  Record source revision, installer/application/updater/SBOM hashes, publisher, timestamp authority, prior/current versions, workflow and attestation URLs, lifecycle result counts, and release approval. Never commit certificate material, private keys, account names, or machine paths.

- [ ] **Step 6: Commit release code before running final signed evidence**

  ```powershell
  git add crates/desktop-app crates/desktop-update .github/workflows/release-windows-signed.yml tools docs/qualification-evidence/signed-release-summary.json
  git commit -m "feat(release): qualify signed updates and complete lifecycle"
  ```

  Re-dispatch after merge so the committed summary references the final immutable artifact; if evidence changes executable or workflow code, invalidate it and rebuild.

---

### Task 13: Close whole-product quality, security, privacy, and operability

**Files:**
- Create: `apps/desktop-ui/e2e/critical-paths.spec.ts`
- Create: `apps/desktop-ui/e2e/accessibility.spec.ts`
- Modify: `apps/desktop-ui/package.json`
- Create: `.github/workflows/desktop-e2e.yml`
- Create: `docs/security/threat-model-production.md`
- Create: `docs/security/penetration-test-summary.md`
- Create: `docs/operations/desktop-support-runbook.md`
- Create: `docs/operations/incident-and-revocation-runbook.md`
- Create: `docs/operations/pilot-slo.md`
- Create: `docs/qualification-evidence/product-qualification-summary.json`

**Interfaces:**
- Consumes: exact signed candidate and deployed support-plane digest.
- Produces: reproducible Windows E2E, WCAG 2.2 AA, performance, security, privacy, recovery, incident, and support evidence.

- [ ] **Step 1: Add Windows end-to-end coverage**

  Automate first launch, workspace grant/revoke, all 26 BMAD menu paths, deterministic Help, production Help test tenant, model-to-D3 proposal, apply/undo, crash recovery, update blocking, settings, update, erase, and restart. Run against the packaged WebView2 app, not only jsdom.

- [ ] **Step 2: Complete accessibility qualification**

  Run axe for every critical state and a manual keyboard/screen-reader/high-contrast/200%-zoom/reduced-motion pass. Acceptance is zero Critical/Serious automated violations and zero unresolved WCAG 2.2 A/AA failure in critical paths.

- [ ] **Step 3: Establish measured performance budgets**

  On the qualification image require: cold launch to interactive ≤5 seconds at p95 across 20 runs; local navigation response ≤100 ms p95; bounded workspace scan remains within configured file/byte limits; model operations expose progress/cancel and respect transport timeout; no unbounded renderer, store, or receipt-history growth.

- [ ] **Step 4: Run security and privacy review**

  Update the threat model for production D2, model-to-D3, updater, Builder, and offboarding. Run independent code review and penetration testing of WAM/helper boundary, IPC, local store, workspace race defenses, egress redaction, API auth, consent replay, SQL partitioning, Key Vault signing, update trust, and installer. Acceptance: no open Critical/High finding; Medium findings are fixed or explicitly accepted with owner and expiry.

- [ ] **Step 5: Rehearse operations**

  Execute model kill switch, installation revocation, signing-key rotation, certificate incident, Container Apps rollback, database restore, update rollback, corrupt local store recovery, interrupted D3 restore, and user offboarding. A second operator must execute the runbooks without undocumented knowledge.

- [ ] **Step 6: Record qualification summary**

  Bind test counts, findings disposition, accessibility report, performance distributions, drill timestamps, signed artifact hashes, container digest, and reviewer approvals. Raw prompts, workspace content, tokens, identities, and secrets remain outside committed evidence.

---

### Task 14: Run a bounded internal pilot and close the scorecard

**Files:**
- Create: `docs/pilot/internal-pilot-plan.md`
- Create: `docs/pilot/internal-pilot-acceptance.md`
- Create: `docs/qualification-evidence/internal-pilot-summary.json`
- Modify: `docs/readiness/100-percent-scorecard.json`
- Modify: `README.md`
- Modify: `docs/implementation-packets/`

**Interfaces:**
- Consumes: exact signed candidate, exact deployed container digest, all Gate G evidence, support/on-call ownership, and approved pilot users.
- Produces: approved internal-pilot evidence and a scorecard that passes `pnpm readiness:check` with all 14 capabilities at 100%.

- [ ] **Step 1: Freeze the pilot candidate**

  Record source revision, installer/application/updater hashes, container digest, policy version, model deployment, signing key versions, and scorecard revision. Any executable, workflow, infrastructure, policy-schema, or trust-root change ends the freeze and requires affected gates to rerun.

- [ ] **Step 2: Execute the bounded pilot**

  Use at least five internal users across at least two job roles for ten business days and at least fifty completed sessions. Require users to exercise deterministic Help, production Help, at least six distinct BMAD capability families, a governed apply/undo, update, restart, recovery simulation, and offboarding on a disposable profile.

- [ ] **Step 3: Apply pilot acceptance criteria**

  Require: zero workspace corruption or unauthorized effect; zero secret/content privacy incident; zero unsigned/unverified model completion; zero updater trust bypass; ≥99% successful launches excluding documented host outages; ≥95% successful non-cancelled governed operations; every Sev-1 incident resolved and rehearsed; all user-blocking accessibility defects closed; support and revocation procedures completed within their runbook targets.

- [ ] **Step 4: Perform independent final review**

  Review the complete diff from the last qualified baseline, all evidence summaries, GitHub settings, Azure configuration, signed artifacts, deployed digest, and pilot outcomes. Require no open P0/P1 finding and explicit product, security, release, and operations approval.

- [ ] **Step 5: Close all capability records**

  Change a scorecard entry to `complete`/`100` only when every listed evidence kind exists and matches the frozen revision/digests. Run:

  ```powershell
  pnpm readiness:check
  pnpm verify:deferred-full
  git diff --check
  ```

  Expected: all commands exit 0; scorecard reports 14/14 complete with no waiver.

- [ ] **Step 6: Correct documentation and publish final evidence**

  Update README product/repository descriptions, remove stale “frozen scaffold” claims, link operator/user documentation, record exact release/pilot identities in implementation ledgers, and update BigBrain with the completed evidence—not projected percentages.

- [ ] **Step 7: Commit and qualify the final documentation-only revision**

  ```powershell
  git add docs README.md
  git commit -m "docs(readiness): record 100 percent pilot qualification"
  pnpm verify:source
  ```

  Push through a final reviewed PR. Documentation-only finalization must not change artifact or deployment identities; if it does, return to the affected gate.

---

## Final self-review checklist

- [ ] Every one of the 26 roster menu paths maps to an active tested capability.
- [ ] All five Builder operations are usable and remain inactive/non-executable drafts.
- [ ] Help compatibility fixtures are byte-stable after generic capability refactoring.
- [ ] Production transport performs the deployed round trip and no longer returns the rollout stub.
- [ ] Verified model output reaches D3 only as a candidate requiring fresh local approval.
- [ ] P4 independent D2/D3 epochs remain intact across sign-out, edit escalation, and revocation.
- [ ] Signed installer, updater, rollback, uninstall, retention, and erase scenarios bind the same revision and artifact hashes.
- [ ] All workflow actions and container bases are immutable and reviewed.
- [ ] No required test, container scan, advisory check, Azure drill, accessibility state, or pilot criterion is skipped.
- [ ] The final scorecard is machine-green and backed by immutable evidence rather than estimates.

## Expected percentage progression

| Gate | Capabilities expected to reach 100% |
|---|---|
| A-B | BMAD foundation, offline developer checkout, horizontal governed foundation |
| C | Full BMAD breadth, deterministic Help backend/UI |
| D | Production model-backed Help |
| E | D3 backend/UI, integrated D2+D3, first honest AI desktop prototype |
| F | Reproducible/installable offline prototype, complete current-source EXE |
| G-H | Internal pilot readiness and final confirmation of all earlier capabilities |
