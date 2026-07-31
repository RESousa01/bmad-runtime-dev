---
title: "Master Implementation Sequence"
aliases:
  - "51 - Master Implementation Sequence"
tags:
  - bmad-runtime
  - vault/delivery-plan
section: "Delivery Plan"
order: 51
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: master-build-sequence
status: implementation-guide
---



# Master Implementation Sequence

## V6.17 master sequence

1. S0: accept ADR-031–039/045–048, discriminator/authority unions, canonical hashing, BMAD and Airlock fixtures.
2. S1: generate C#/Rust/TypeScript contracts and pass golden conformance vectors.
3. In parallel, execute W0–W5 for the existing cloud-first product and D0–D6 for the installed product as defined in [[08 - Phased Roadmap and Build Order]].
4. Do not merge execution/storage authorities into one service while extracting shared code.
5. Build remote-job handoff only after both independent local and cloud evidence/rollback proofs pass.

W0–W5 retain the current Azure sequence. D0 must resolve folder/child-process containment, auth, path semantics, recovery ordering, and signing/update before D1 broadens the desktop shell. A failing DESK-01 proof narrows the desktop command promise or blocks release; it does not cause silent fallback to cloud execution.

For detailed AI-agent execution rules, work packet structure, TDD mode selection, prompt templates, and LLM-specific phase gates, use [[90 - LLM-Tailored Development Plan and Agent Workflow]] alongside this sequence.

## V6.18 normative BMAD/Builder sequence overlay

[[100 - BMAD Method and Builder Deep Comprehension Audit]] controls the semantic and promotion order wherever older sections treat Builder authoring as one late block:

1. Freeze Method/Builder source semantics and explicit source/install/validation/runtime profiles; generate source-derived fixtures for the supported execution archetypes.
2. In the early foundation proof, run one real, source-derived sealed Method path and prove inactive Builder `Build`/`Edit`/`Analyze` for one stateless agent and one simple workflow using fake-safe contracts.
3. After Model Gateway passes, replace fake authoring output with real conversational draft generation while retaining immutable lineage and inactive state.
4. After the target delivery model proves governed execution, add static scans plus isolated four-mode evaluation and exact-digest install/invocation rehearsal.
5. After evidence, rollback, and policy gates pass, add promotion, signing/publication where applicable, and reversible activation of only the evaluated digest.
6. Add memory agents only after owner-scoped durable storage; add autonomous agents only after storage plus scheduler, quiet-hours, lifecycle, and containment gates.

The shared sequence does not create a shared executor or store: W and D produce separate execution, activation, evidence, and rollback records. `Convert` is not an upstream Builder capability in the pinned snapshot; any conversion surface is a Sapphirus adapter implemented through `Build`/`Edit` and labeled accordingly.

## 1. Corrected build order

The only safe build order is:

```text
Source/adoption decisions + pinned BMAD compatibility profiles
→ canonical BMAD + owner/principal + context + work/evidence contracts
→ headless contract harness with fake adapters
→ local BMAD-native vertical slice with durable replay
→ security/context/egress/secrets + minimum cloud readiness
→ one real model adapter
→ ACA Job execution lane using the same approved-spec/manifest contract
→ arbitrary BMAD package validation and reversible activation
→ Presentation adapter
→ Builder authoring/evaluation
→ Operator controls
→ production/release hardening
```

The sealed Phase-0/1 BMAD fixture is a narrow compatibility seam, not the arbitrary package loader. Phase 1 uses a non-isolating trusted fake. Azure foundations and a disabled fixed Job template arrive in Phase 2, but no real ACA execution enters the product path until Phase 4.

## 2. Phase -1 — governance, source, and compatibility decisions

### Goal

Remove bootstrap ambiguity before implementation agents create contracts or copy source-derived fixtures.

### Deliverables

- Source/adoption records for BMAD Method, BMAD Builder, and any comparable source that influences implementation.
- Immutable archive/license/notice/extraction hashes plus append-only verification records.
- `BmadCompatibilityProfile` decision for Method `MethodCliV6` and Builder `StandaloneBuilderSetupV2` inputs.
- ADR for the isolated Node/Python BMAD import/rehearsal worker and the normalized .NET consumption boundary.
- Toolchain compatibility matrix and generated-client spike.
- Root/scoped repository agent policy, threat model, and canonical document-precedence rule.

### Exit criteria

- Foundation fixtures have a recorded license/adoption decision and reproducible archive identity.
- Unknown upstream commit identity is reported as a confidence limit rather than silently inferred.
- No upstream executable package is loaded into the Runtime API process.

## 3. Phase 0 — contracts, repository, and headless harness

### Goal

Create a compilable BMAD-native skeleton and prove its irreversible contracts without production UI implementation, real providers, or Azure execution. Product-design decisions are approved here so Phase 1 does not improvise the shell while wiring the slice.

### Deliverables

- Monorepo folders matching `03 - Repository and Vault Usage.md`.
- Golden installed fixtures for `BmadInstallProfile.MethodCliV6` and `BmadInstallProfile.StandaloneBuilderSetupV2`.
- Canonical `BmadPackageDescriptor`, `BmadConfigLayer`, capability/help, workflow-step, artifact, and draft-to-active package lifecycle contracts.
- One sealed `BmadFoundationFixture` selected for the first vertical slice.
- Minimum `OwnerScope`, human/dev principal, `UntrustedContextEnvelope`, and `PromptCacheContract` contracts.
- `Proposal`, `ExecutionSpecCandidate`, `AirlockDecision`, `Approval`, immutable `ApprovedExecutionSpec`, `SpecConsumptionRecord`, `ExecutionResultManifest` union, `Checkpoint`, and `EvidenceBundle` schemas.
- Durable `WorkItem`, `WorkAttempt`, `WorkLease`, `OutboxMessage`, `EvidenceLedgerEvent`, `EventCursor`, and `ProjectionCheckpoint` contracts.
- OpenAPI skeleton, SQL migration skeleton, Blob layout constants, fake auth, fake Model Gateway, and fake executor ports.
- Contract, invalid-fixture, generated-client, and state-transition test pipeline.
- Approved first-slice UX blueprint, desktop/narrow/mobile wireframes, design tokens, component anatomy, motion rules, and frontend dependency compatibility plan from files 26, 43, and 66.

### Exit criteria

- `dotnet test` passes for domain/contract tests.
- TypeScript client generated from OpenAPI compiles.
- Both BMAD installation profiles normalize into deterministic golden records or fail with profile-specific findings.
- A headless in-memory harness selects the sealed capability and proves proposal → candidate hash → policy → exact approval → approved spec → fake manifest → imported transition → evidence references.
- Phase 0 does not claim persistent UI, cloud execution, arbitrary package support, or local containment, and its clean-machine tests require no Docker, Kubernetes, emulator, or local model server.
- The first-slice workbench direction is visually reviewed before Phase 1; this approval is a design artifact, not a claim that the UI exists.

## 4. Phase 1 — trusted local simulated vertical slice

### Goal

Prove Sapphirus is a BMAD-native governed workspace by simulating one predefined file change and validation through deterministic trusted fakes. This proves contracts and recovery, not isolation.

### Steps

Before step 1, implement the minimum `packages/ui` foundation and Storybook fixtures for AppShell, RunCapsule, ApprovalReview, ExecutionProgress, PartialFailureDecision, and RunOutcome. The approved file-43 wireframes and file-26 tokens are the visual source of truth; fixture stories must pass keyboard, theme, density, and reduced-motion checks before API binding.

1. Implement a local dev auth shim and persist owner-scoped project/thread/message/run/method state.
2. Upload the sample repo through safe extraction and create an immutable snapshot.
3. Render file tree and read-only viewer.
4. Build lexical context pack v0 with deterministic ordering/budget, trust labels, redaction, protected BMAD inputs, and a `PromptCacheContract`.
5. Persist transactional evidence-ledger/outbox events and support replay-then-live subscription from a durable cursor.
6. BMAD Kernel selects the sealed package/skill/workflow step and expected artifact from the capability/help graph.
7. Fake model returns a deterministic BMAD-bound plan and patch proposal.
8. Orchestrator normalizes proposal and computes proposal hash with package, skill, step, config, and context lineage.
9. Orchestrator/Airlock create a fully specified, immutable `ExecutionSpecCandidate`.
10. Airlock evaluates paths, preimages, command, lane class, environment, network, inputs, outputs, and limits.
11. UI shows BMAD action, artifact, diff, exact candidate fields, and candidate hash.
12. User approves the candidate hash; Airlock mints a matching `ApprovedExecutionSpec`.
13. `sealed_test_fake` applies one predefined patch to a sealed temporary fixture and evaluates a represented `argv[]` validation through an in-process deterministic validator; it has no process, shell, network, dependency restore, or package-import surface.
14. Fake executor writes simulated bounded logs and an append-only `WebWorkerResultManifest` through the same port later used by ACA; the UI/evidence labels the attempt as simulated/non-isolating.
15. Runtime API imports the manifest idempotently and advances execution state.
16. Runtime advances BMAD step/artifact state only after successful import.
17. Runtime records checkpoint and evidence report with source/package/skill/step/config/artifact hashes.

### Exit criteria

- Every governed side effect has a proposal, candidate, policy decision, exact approval, spec, work attempt, execution, manifest, checkpoint, and evidence-ledger record.
- The flow is unavailable if its sealed BMAD capability/step is absent, invalid, or hash-mismatched.
- Method/artifact state cannot advance from model output or worker output alone.
- Fake executor has no lifecycle-store mutation interface.
- Fake executor cannot execute imported/generated code or arbitrary workspace commands and is never called a sandbox.
- Re-running approval submission with same idempotency key does not duplicate execution.
- Preimage drift blocks patch apply.
- Client disconnect/reconnect and process restart replay durable events without repeating model, approval, or execution work.
- No ACA, Dynamic Sessions, Key Vault, real provider, local container engine, infrastructure emulator, or local model dependency is needed for this simulated milestone.

## 5. Phase 2 — security, context, replay, and cloud readiness

### Goal

Harden every dependency required before sending real project context to a provider or dispatching a cloud worker.

### Steps

1. Expand `OwnerScope` and principals across every implemented route/resource; add not-found equivalence tests.
2. Add safe archive/file primitives, symlink rejection, secret redaction, and prompt-injection fixtures.
3. Add `OutboundUrlPolicy`, DNS/redirect/private-network fixtures, provider credential binding, and data-egress policy.
4. Add provider-neutral model profile/capability/error contracts and model/context/safety evaluation fixtures.
5. Add replay cursor expiry, projection rebuild, schema-upcaster, outbox retry/poison, and evidence-materialization failure tests.
6. Provision minimum cost-capped dev/staging Bicep for Entra scopes, distinct managed identities, Key Vault, SQL, Blob, ACR, monitoring, Container Apps environment, network policy, scoped worker access, and a disabled/non-production fixed Job template.
7. Prove ACR Tasks (`az acr build`) or hosted CI remote build with lock/component-license/scan/SBOM/provenance/digest evidence and a clean-machine workflow that uses no local Docker/Kubernetes/emulators/model server.

### Exit criteria

- Security and context fixtures fail closed before real provider/tool/package breadth.
- Durable replay survives API restart and never re-executes side effects.
- Minimum cloud resources, identities, remote build, and immutable fixed-template controls exist before their Phase-3/4 consumers.

## 6. Phase 3 — real Model Gateway and repair loop

### Goal

Replace fake model output with one real provider adapter while preserving deterministic CI and platform-owned proposals.

### Steps

1. Implement one Microsoft Foundry/Azure OpenAI v1 Responses adapter behind Model Gateway with `store=false`, app-owned state, and provider-hosted tools disabled.
2. Record exact deployment/region/model/API/retention capabilities plus parsed-HTTPS credential binding for primary, repair, compression, review, and fallback calls.
3. Project canonical schemas into the provider-supported subset, retain both hashes, validate canonically, and handle refusal/incomplete/content-policy/rate-limit/timeout/capability/schema failures with bounded repair.
4. Promote role-based `ModelProfile` aliases only through contract, BMAD task-quality, safety/privacy, and operations/cost suites, then policy approval, canary, active, and tested rollback; retain the fake provider as blocking CI baseline.
5. Pre-evaluate explicit fallback edges and stop when provider, credential, residency, retention, tool, schema, or material-quality boundaries would change.

### Exit criteria

- Schema-invalid output cannot create a proposal.
- Credential or endpoint mismatch fails closed.
- Fallback never silently changes capability, endpoint trust, residency, or credential scope.
- Repair stops on repeated failure signature, attempt cap, or budget cap.

## 7. Phase 4 — ACA Job execution lane

### Goal

Introduce the first real isolation/effect boundary as finite fixed-template cloud work without changing runtime authority or evidence contracts.

### Steps

1. Build digest-pinned general execution and BMAD Node/Python import/rehearsal worker images remotely with ACR Tasks/hosted CI and promote only scan/license/SBOM/provenance/signature-backed digests.
2. Add a fixed ACA Job dispatcher with start-only identity, scoped worker identity/SAS, bounded environment/network/output, and manifest signing/authentication; request-time image, entrypoint, identity, secret, network, and arbitrary environment overrides are rejected.
3. Implement database-CAS `WorkLease`, heartbeat, timeout, cancellation, reclaim, late-manifest, and crash import behavior.
4. Require ACA and fake executors to emit the same `WebWorkerResultManifest` schema and evidence fields.

### Exit criteria

- ACA can execute an approved patch/test spec and produce an authenticated importable manifest.
- Worker has no lifecycle SQL credentials.
- Lost lease, duplicate delivery, timeout, and late manifest cannot produce duplicate success.
- The remote build, ACA smoke, result import, and rollback run from a developer machine without a local Docker engine.

## 8. Phase 5 — arbitrary BMAD package quality and activation

### Goal

Expand the sealed foundation seam into arbitrary package import, validation, rehearsal, capability updates, and reversible activation.

### Steps

1. Generalize fixture parsers for source and installed Method/Builder layouts while retaining `BmadInstallProfile`.
2. Separate installer-managed, user, custom, and compatibility configuration layers.
3. Add package trust classification, safe extraction, static validation, dependency review, install/invocation rehearsal, and package install policy.
4. Activate only approved immutable versions and update `ToolAvailabilitySnapshot` explicitly.

### Exit criteria

- Invalid/mixed-layout packages never enter the active catalog.
- Duplicate menu codes, orphan help rows, config ambiguity, and hash drift produce actionable findings.
- Activation/deactivation is reversible, audited, and cannot skip validation/rehearsal/approval.

## 9. Phase 6A — existing presentation workflow adapter

Wrap the existing presentation workflow as the seed Artifact Creator package. Preserve behavior with golden fixtures, map stages to BMAD workflow/artifact state, and route source/outline/draft/export effects through existing approvals, workers, provenance, and evidence. This is independent from Builder authoring and must not share one oversized work packet with it.

## 10. Phase 6B — Builder authoring and evaluation

Recognize the actual Builder source skills and package shapes; create/import/convert/validate one package class; execute install, invocation, and eval rehearsals in approved isolated workers; and keep generated output in `Draft -> Validated -> Rehearsed -> Approved -> Active`. Generation alone never confers trust.

## 11. Phase 7 — operator controls

Expose owner-scoped, audited controls for provider profiles, packages, tool snapshots, worker lanes/images, egress policy, degraded dependencies, work leases, evidence gaps, and release status. Disable paths must be backed by durable configuration/control records rather than UI-only state.

## 12. Phase 8 — production and release hardening

Complete migration, retention, backup/restore, rollback drills, observability/SLOs, load/cost checks, supply-chain signing/provenance, fresh-install smoke, security regression, accessibility, and target-commit release evidence. Phase 8 hardens production-shaped dependencies introduced earlier; it does not introduce the first provider secret store or worker environment.

## Consolidated Source-Review Sequence Refinement

Source: [[89 - Consolidated AI Workspace Source Review and Architecture Improvements]].

Before Phase 1 starts, complete these sequence refinements:

1. Add `OwnerScope`, principal resolution, idempotency, evidence-ledger, and event-outbox contracts to the Runtime API skeleton.
2. Add `ToolAvailabilitySnapshot` and active-run prompt/tool/context hash records before real model calls.
3. Add fake `OutboundUrlPolicy` plus full SSRF fixture pack before webhook/search/provider fetch routes.
4. Add TypeScript 7 generated-client gate before web feature breadth.
5. Add worker image manifest/SBOM/provenance placeholders even for fake workers, so the evidence shape is stable.
6. Add package activation states before Builder Studio can write any active package record.
7. Add degraded-state reporting before optional services such as vector memory, email, notifications, and local provider probes.

The sequence should optimize for irreversible contract clarity first, feature breadth second.

## LLM-Tailored Implementation Overlay

Source: [[90 - LLM-Tailored Development Plan and Agent Workflow]].

The master sequence remains canonical, but AI agents must execute it through these additional rules:

| Sequence area | LLM execution rule |
|---|---|
| Foundation repository | Use contract-first and TDD; do not add feature breadth until generated clients compile. |
| Vertical slice | Use replay-driven development; every side effect must have evidence before expanding scope. |
| Runtime hardening | Use security-test-first; owner scope, loopback, egress, and prompt-injection fixtures come before real integrations. |
| Model Gateway | Use contract TDD; provider SDK objects never leak above gateway. |
| Execution lanes | Use manifest-first worker tests; ACA Jobs baseline comes before Dynamic Sessions adoption. |
| BMAD package loader | Use golden fixtures; package text is untrusted until activation. |
| Builder validation | Use rehearsal fixtures; generation alone cannot mark a package valid. |
| Operator hardening | Use UI state-machine tests; operator views show server-authoritative states and redacted evidence. |

Senior execution rule: no phase may start broad feature work until its contract, rollback/disable path, observability surface, owner/retention model, and first negative test are named. L and XL work packets must be split into contract, behavior, UI, operations, and release-gate slices, or converted into a spike that produces an ADR and follow-up stories.

## Deep-Dive Sequence Reinforcements (V6.14)

Source: second-pass source review of Odysseus, OpenClaw, and Hermes (see [[89 - Consolidated AI Workspace Source Review and Architecture Improvements]] → "Second-Pass Deep Dive"). Three items move earlier in the sequence; two get concrete mechanics:

1. **Detached-run contract joins the vertical slice (Phase 1).** The slice's stream skeleton must implement server-side drain, durable replay-then-live subscribe, cursor-gap reconciliation, and projection checkpoints from the start (contract in [[12 - Run Orchestrator and Agent Kernel]]). Approval waits and executions outlive browser tabs; retrofitting reconnect semantics later rewrites the streaming path.
2. **QA maturity register starts at Phase 1, not at hardening.** Create the per-surface register ([[33 - Release Gates and Acceptance Matrix]]) with the slice's surfaces labeled `experimental`. Machine-verifiable scenario coverage and immutable run evidence may promote a surface; model-authored or manually edited optimism may not. This replaces "gate evidence accumulates someday" with evidence from the first milestone.
3. **Repo agent-policy files precede the first work packet (Phase -1/0).** Root `AGENTS.md` (hard policy, review lens, premise-verification rule) and scoped guides per module land with the repo skeleton, per [[90 - LLM-Tailored Development Plan and Agent Workflow]] §14.2. Platform-specific symlinks are optional; CI must detect drift between any duplicated tool-specific instruction entry points.
4. **Parallel packet dispatch uses an overlap audit.** Before starting parallel work packets, run the area-overlap audit (Odysseus `pr_blocker_audit.py` pattern): classify open branches/PRs by owning area and refuse to dispatch a packet into an area with one already in flight. This is the executable form of the WIP limits.
5. **Background jobs respect a foreground gate from their first appearance.** When Phase 2+ introduces scans and scheduled work, they ship with the quiet-period gate ([[17 - Workspace Intelligence and Context Packs]]) and the creation-time self-lifecycle guard ([[38 - Worker Images and Command DSL]]) — not as later hardening.
