---
title: "Evidence-Gated Milestone Build Plan"
aliases:
  - "72 - Week-by-Week Build Plan"
  - "Milestone Build Plan"
tags:
  - bmad-runtime
  - vault/delivery-plan
section: "Delivery Plan"
order: 72
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: milestone-build-plan
status: planning-guide
---



# Evidence-Gated Milestone Build Plan

## V6.17 parallel calendar

Schedule shared S0/S1 contract and fixture work first, then staff independent W and D streams. W weeks preserve the cloud-first Azure foundation, web slice, model, fixed remote executor, packages/artifacts, and operations sequence. D weeks begin with security/auth/filesystem/recovery/signing spikes, then shell/state, model/context, governed patching, commands, packages/sync, and remote handoff/enterprise release.

Do not put remote handoff on the critical path for local editing. Do not claim desktop command completion until DESK-01 closes for the supported matrix. Weekly demos must identify which authority produced the evidence and rollback proof.

This is a sequencing and promotion plan, not a calendar promise. A milestone advances only when its exit evidence passes. Calendar estimates are created after the Phase -1/0 compatibility spikes reveal team throughput, toolchain friction, and integration risk; they are not embedded in architecture.

For LLM implementation details, use [[90 - LLM-Tailored Development Plan and Agent Workflow]]. [[51 - Master Implementation Sequence]] is the canonical detailed sequence. This note is its compact milestone view.

## V6.18 BMAD/Builder evidence milestones

Source: [[100 - BMAD Method and Builder Deep Comprehension Audit]]. These gates fit inside the existing parallel W/D calendar and do not imply equal calendar duration:

- **B0 — source freeze:** before Milestone 0 exits, freeze explicit Method/Builder source/install/validation/runtime profiles; archive source-derived golden fixtures and the drift ledger.
- **B1 — foundation proof:** in Milestone 1, demonstrate one real, source-derived sealed Method path and inactive Builder `Build`/`Edit`/`Analyze` for one stateless agent and one simple workflow. Generated content cannot execute or activate.
- **B2 — real authoring:** after Milestone 3 proves Model Gateway, enable conversational draft generation with immutable source/prompt/config/model/content lineage.
- **B3 — isolated eval/rehearsal:** after Milestone 4 proves governed execution for the applicable authority, run static scans, isolated baseline/variant/quality/trigger evals, and exact install/invocation rehearsal.
- **B4 — promotion:** only after durable evidence and rollback proof may the exact evaluated digest be signed, published where applicable, approved, and reversibly activated.
- **B5 — advanced agents:** memory waits for owner-scoped durable storage; autonomy additionally waits for scheduler, quiet-hours, lifecycle, and containment proof.

Web and desktop may reach these gates independently and must identify the authority that produced the evidence. The pinned upstream Builder exposes `Build`, `Edit`, and `Analyze`, not `Convert`; any conversion milestone is a separately named Sapphirus adaptation over `Build`/`Edit`.

## Milestone -1 — Governance and source readiness

- Record BMAD Method/Builder plus OpenClaw/Hermes/Odysseus source/archive, license/notice, extraction, fixture, provenance-gap, and separate verification/adoption decisions; research snapshots without immutable Git identity remain quarantined.
- Lock the two `BmadInstallProfile` compatibility fixtures and sealed first capability.
- Decide the isolated Node/Python BMAD import/rehearsal worker boundary and normalized .NET contract.
- Prove the pinned toolchain and generated-client path.
- Establish repository/scoped agent policy, architecture precedence, and threat model.

Exit evidence: Source Intake records, license decisions, importer ADR, toolchain matrix, and reproducible fixture hashes.

## Milestone 0 — Contracts and headless harness

- Create repository structure and the minimum OpenAPI/JSON Schema/SQL/Blob skeleton.
- Define canonical BMAD objects plus `OwnerScope`, principal, `UntrustedContextEnvelope`, and `PromptCacheContract`.
- Define proposal, `ExecutionSpecCandidate`, exact approval, `ApprovedExecutionSpec`, `WebWorkerResultManifest`, checkpoint, and evidence contracts.
- Define `WorkItem`, `WorkAttempt`, `WorkLease`, outbox, evidence-ledger, cursor, and projection-checkpoint contracts.
- Add fake auth, model, executor, contract generation, invalid fixtures, and the headless BMAD flow.

Exit evidence: contracts validate, generated clients compile, canonical transition tests pass, both install profiles normalize deterministically, and the headless governance chain completes without UI/Azure, Docker/Kubernetes/emulators, local model serving, shell execution, or a containment claim.

## Milestone 1 — Trusted local BMAD-native simulation

- Add owner-scoped dev auth, project/thread/run/method persistence, safe sample upload, immutable snapshot, and read-only file view.
- Build deterministic, budgeted, provenance-backed untrusted context pack v0.
- Select the sealed Help Advisor action and persist BMAD package/skill/step/config/artifact lineage.
- Normalize fake model output into proposal and exact candidate; approve its hash; mint matching spec.
- Apply one predefined patch to a sealed temporary fixture through a non-isolating deterministic fake with no process/shell/network/dependency/package surface; import its simulated `WebWorkerResultManifest`, advance method/artifact state, checkpoint, and materialize evidence.
- Prove durable replay-then-live reconnect and restart without rerunning side effects.

Exit evidence: [[01 - First Build - Executable Vertical Slice]] succeeds locally with success, denial, candidate-drift, owner-scope, injection, fake-failure, manifest-invalid, cursor-gap, and rollback fixtures. It is labeled simulated/non-isolating and requires no ACA or local container/model/emulator dependency.

## Milestone 2 — Security, context, replay, and cloud readiness

- Complete authorization coverage for implemented resources and principals.
- Add safe extraction/file tests, secret redaction, prompt-injection corpus, egress/SSRF/DNS policy, and provider credential binding.
- Add model/context/safety evaluation fixtures and provider-neutral capability/error contracts.
- Prove cursor expiry reconciliation, projection rebuild, schema upcasting, outbox retry/poison, and evidence retry.
- Provision minimum cost-capped dev/staging Entra scopes/identities, Key Vault, SQL, Blob, ACR, monitoring, Container Apps environment, network, scoped worker resources, and a disabled fixed Job template.
- Prove ACR Tasks/hosted-CI remote image build with lock/component-license/scan/SBOM/provenance/digest evidence and no local Docker daemon.

Exit evidence: negative security/replay suites pass and the minimum cloud substrate is ready before real provider or ACA consumption.

## Milestone 3 — Real Model Gateway

- Add one Microsoft Foundry/Azure OpenAI v1 Responses adapter behind the fake-proven contract with `store=false`, app-owned state, and hosted tools off.
- Record exact deployment/region/model/API/retention capabilities, parsed-HTTPS credential binding, schema projection/canonical hashes, and cache resolution for every call purpose.
- Handle refusal/incomplete/content-policy/rate-limit/timeout/capability/schema failures, budget/quota, and bounded repair.
- Promote role-based profiles through contract, task-quality, safety/privacy, and operations/cost suites, policy, canary, and tested rollback; keep fake provider CI blocking and fallback edges explicit.

Exit evidence: real provider produces valid proposal candidates without owning proposals or policy, invalid output cannot progress, credential/fallback violations fail closed, and promotion thresholds pass.

## Milestone 4 — ACA finite execution

- Build digest-pinned general and BMAD import/rehearsal worker images remotely and promote only lock/license/scan/SBOM/provenance/signature-backed digests.
- Dispatch candidate-bound, audience-bound single-use specs through fixed ACA Jobs with start-only dispatcher identity and scoped worker storage/network/output; request-time template/image/entrypoint/identity/secret/environment overrides are denied.
- Add CAS leases, heartbeats, cancellation, reclaim, late-manifest, and crash-import behavior.
- Require fake and ACA lanes to emit the same authenticated `WebWorkerResultManifest` contract.

Exit evidence: cloud patch/test flow completes without worker lifecycle SQL authority or local Docker; duplicate delivery, timeout, lease loss, completion/import crash, and late manifest remain single-outcome and reconstructable.

## Milestone 5 — Arbitrary BMAD package quality and activation

- Generalize the sealed parsers across supported source/installed layouts while retaining install-profile distinctions.
- Add safe extraction, package trust/install policy, static validation, config ownership, dependency review, install/invocation rehearsal, and reversible activation.
- Update tool availability explicitly after activation/deactivation.

Exit evidence: valid packages activate through all gates; malformed, mixed-layout, injected, hash-drifted, or unrehearsed packages remain inactive with actionable findings.

## Milestone 6A — Presentation adapter

Wrap the existing presentation workflow through BMAD workflow/artifact, approval, worker, provenance, golden regression, and evidence contracts. This is an independent milestone from Builder authoring.

## Milestone 6B — Builder authoring and evaluation

Support one real Builder create/import/convert/validate/evaluate path. Generated output remains Draft until validation, isolated rehearsal, approval, and immutable activation complete.

## Milestone 7 — Operator controls

Expose audited provider, package, tool-snapshot, worker-lane, egress, degraded-dependency, work-lease, evidence-gap, and release controls with real durable disable paths.

## Milestone 8 — Production release

Pass migration, retention, backup/restore, rollback, fresh-install, load/cost, security, accessibility, observability, supply-chain, release-manifest, and exception-register gates.

## Estimation rule

Each milestone is decomposed into XS/S/M work packets. L/XL items are split or spiked before implementation. Calendar forecasting uses completed-packet throughput and measured integration lead time after Milestone 0; architecture notes never imply that a milestone equals one week.
