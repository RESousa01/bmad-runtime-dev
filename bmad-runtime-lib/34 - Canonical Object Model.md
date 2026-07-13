---
title: "Canonical Object Model"
aliases:
  - "34 - Canonical Object Model"
tags:
  - bmad-runtime
  - vault/architecture-contracts
section: "Architecture Contracts"
order: 34
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: canonical-object-model
status: v6-modernized-validated-implementation-guide
generated_on: 2026-07-09
review_pass: v6-modernization-and-platform-validation
architecture_rule: governed-chat-first-agentic-runtime
---



# Canonical Object Model

## 0. V6.17 durable envelope and authority split

Every durable object is wrapped by or contains the equivalent of `DurableObjectEnvelope`: `schemaVersion`, `objectType`, `objectId`, `deliveryModel`, `authorityRef`, `ownerScopeRef`, `projectId`, `createdAt`, and `contentHash`. `Project.deliveryModel` is immutable; every descendant inherits it. Cross-delivery transfer creates a `RemoteJobHandoff` or linked project rather than mutating the object.

| New canonical object | Purpose |
|---|---|
| `AuthorityRef` | Discriminated `azure_control_plane` or `desktop_local_store` authority identity. |
| `WorkspaceTarget` | Discriminated immutable cloud snapshot or revocable local folder capability/root identity. |
| `FilesystemCapabilitySnapshot` | Filesystem/volume identity and proven reparse, file-ID, atomic-replace, durable-flush, support-tier capabilities. |
| `ExecutorAudience` | Fixed Azure job template or exact Windows host/install with evidence-calibrated containment profile. |
| `SpecConsumptionRecord` | Immutable record of consuming a single-use spec; the spec itself remains immutable. |
| `ExecutionResultManifest` | Union of `WebWorkerResultManifest` and `WindowsLocalExecutionResultManifest`. |
| `SyncEnvelope` | Signed, classified, consented non-authoritative replication operation. |
| `RemoteJobHandoff` | Explicit local selection/upload → separate web work → returned local proposal relationship. |
| `PackageCompatibility` | Signed product/schema/BMAD/host-version compatibility and revocation metadata. |

Exact schemas, canonicalization, hashes, version negotiation, and cross-language fixtures are authoritative in [[99 - Dual-Delivery Contract and Conformance Specification]].

## V6.18 current-authority overlay — BMAD and Builder object boundaries

This overlay applies [[100 - BMAD Method and Builder Deep Comprehension Audit]] and supersedes conflicting flattened package/Builder definitions later in this historical note.

| Canonical object/value | Current meaning |
|---|---|
| `BmadDistributionProfile` | Source/distribution shape such as Method source tree, Claude marketplace plugin, Web Bundle v1, or Builder source; independent of installation layout. |
| `BmadInstallProfile` | Installation semantics such as composite Method CLI host-native install or standalone Builder setup. |
| `BmadInstallationComposite` | One `_bmad` control/config tree plus all selected host-native skill roots and adapter identities. |
| `BmadUpstreamManifestObservation` | Preserved upstream staging descriptor and declared paths/hashes; never final observed inventory authority. |
| `BmadSkillLocationObservation` | Declared staging path, resolved runtime path, location kind, host adapter, and independently observed content hash. |
| `BmadObservedInventory` | Final post-install composite file/skill inventory used for compatibility, promotion, and activation evidence. |
| `BmadSkillDefinition` | Required `name`/`description`, canonical ID, raw prompt/resources, optional customization surface, inferred archetype, and required host capabilities. |
| `PromptSkillExecutionProfile` | One of the supported prompt-native archetypes plus filesystem, process, network, web, subagent, and tool requirements; it is not execution authority. |
| `BmadConfigSet` | Ordered central TOML layers, per-skill TOML layers, and generated per-module YAML compatibility projections, each retaining ownership and source hash. |
| `BmadHelpAction` | `(module, skill, action?)` capability row with scoped menu alias, phase hints, output evidence rules, and advertised/hidden/deprecated status. |
| `BuilderAuthoringSession` | Mutable conversational/workspace activity that may emit multiple immutable drafts; never a package or activation authority. |
| `BuilderDraftSnapshot` | Immutable authored bytes and lineage for one candidate revision. |
| `SkillPackageProposal` | Immutable reviewable package candidate derived from a draft/import and bound to validation/rehearsal evidence. |
| `PackagePromotion` | Decision/evidence object that admits one immutable proposal version to a governed catalog. |
| `PackageActivation` | Project- and delivery-specific binding that makes one promoted package version available after compatibility, policy, and approval checks. |

Object invariants:

- Builder authoring, package proposal, promotion, and activation are separate identities and state machines; no state on one object is shorthand for another.
- A Method CLI descriptor is incomplete without the final host-native inventory. Upstream manifests remain linked observations, not hash authority.
- `module.yaml` is optional/synthetic normalized input and primarily describes installer prompts, directories, and roster. `SKILL.md` requires `name` and `description`; help actions and inferred inputs/outputs live outside required frontmatter.
- BMAD artifact/memlog state is domain context. Only delivery authority transitions `BmadMethodState`, package promotion/activation, or Evidence Ledger objects.

## 1. Runtime Objects

| Object | Description | Durable? |
|---|---|---|
| `Project` | Security and workspace boundary. | yes |
| `OwnerScope` | Canonical owner/project/legacy scope attached before persistence to user data and machine actions. | yes |
| `PrincipalContext` | Authenticated actor kind, subject, tenant/project scope, grants, authentication method, and delegation chain for one command. | request + audit ref |
| `Thread` | Conversation container. | yes |
| `Message` | User/assistant/system-visible conversation item. | yes |
| `Run` | Execution attempt triggered by message/workflow action. | yes |
| `RunEvent` | Rebuildable run-scoped projection of authoritative `EvidenceLedgerEvent` records. | projection |
| `ContextPack` | Hash-addressed context selection. | yes, summary/ref |
| `UntrustedContextEnvelope` | Trust-labeled wrapper for workspace/package/web/tool content that keeps data from becoming policy or system authority. | yes, summary/ref |
| `PromptCacheContract` | Stable run-level system-prompt, tool-schema, BMAD-fixture, and context-pack hashes plus allowed transition reasons. | yes |
| `ModelCall` | Provider call metadata and output hash. | yes |
| `TypedModelOutput` | Schema-valid model output. | yes, redacted/ref |
| `ModelProfile` | Versioned role alias mapped to an exact provider deployment/snapshot, settings, capability snapshot, evaluation bundle, canary status, and rollback target. | yes |
| `ProviderCapabilities` | Exact provider/deployment/region/model/API/retention/tool/schema capability snapshot used for one call/profile. | yes |
| `ProviderSchemaProjection` | Canonical-schema to provider-subset mapping with both hashes, unsupported keywords, projection version, and canonical validation result. | yes |
| `ProviderCredentialBinding` | Parsed normalized HTTPS endpoint, host/suffix/path/port/tenant/resource/sovereign-cloud class, credential source, and allowed purpose binding. | yes, secret ref only |
| `ModelEvaluationBundle` | Immutable contract/task-quality/safety-privacy/operations evaluation evidence, thresholds, graders, dataset hashes, candidate, and result. | yes |
| `Proposal` | Platform-normalized action candidate. | yes |
| `ExecutionSpecCandidate` | Immutable, policy-evaluated candidate containing the exact lane/image class, cwd, argv, environment policy, workspace target, mutable inputs, outputs, network policy, and limits presented for approval. | yes |
| `AirlockDecision` | Policy evaluation result. | yes |
| `Approval` | Human decision bound to an exact `ExecutionSpecCandidate` hash. | yes |
| `ApprovalGrant` | Explicit future scoped policy envelope that may affect whether a fresh human decision is required; never executable and never a replacement for a candidate-bound spec. | yes, expiring |
| `ApprovedExecutionSpec` | Immutable, audience-bound execution authorization minted only from an unchanged approved candidate. | yes |
| `Execution` | Delivery-authority-dispatched executor instance. | yes |
| `ExecutionResultManifest` | Discriminated web-worker or Windows-local result evidence. | yes, authority-specific payload store |
| `WorkItem` | Durable unit of orchestrated work with owner scope, idempotency key, desired outcome, and current scheduling state. | yes |
| `WorkAttempt` | One immutable attempt to complete a `WorkItem`, with execution/worker correlation and terminal outcome. | yes |
| `WorkLease` | Expiring compare-and-swap claim for one attempt, including holder, heartbeat, and reclaim state. | yes |
| `WorkCompletion` | Immutable terminal claim for one attempt with completion nonce, outcome, result refs/hashes, and outbox/import acknowledgement state. | yes |
| `OutboxMessage` | Transactionally persisted delivery record for a domain/evidence event. | yes |
| `EvidenceLedgerEvent` | Append-only authoritative event linking intent, state transition, policy, approval, execution, artifact, and evidence references. | yes |
| `EventCursor` | Resumable position in one durable event stream, including retention/gap status. | yes |
| `ProjectionCheckpoint` | Last applied stream sequence for a named rebuildable projection. | yes |
| `WorkspaceSnapshot` | Immutable source state. | yes |
| `WorkspaceCheckout` | Job-scoped mutable copy. | ephemeral |
| `Checkpoint` | Approved post-execution state. | yes |
| `RollbackPlan` | File-level reversal plan. | yes |
| `Artifact` | Generated/exported output. | yes |
| `EvidenceBundle` | User/reviewer-facing materialization of authoritative ledger, proposal, approval, spec, manifest, artifact, checkpoint, and rollback references. | yes |
| `TraceBundle` | Diagnostic export/projection for support or privileged forensics; never lifecycle or evidence authority. | yes, generated/ref |
| `SourceSnapshot` | Immutable acquired-source identity: upstream URL, immutable ref/commit when known, archive and license/notice hashes, extraction inventory/completeness, and fixture inventory. | yes |
| `SourceVerificationRecord` | Append-only verification/adoption result for one immutable `SourceSnapshot`, including checks run, confidence, license decision, and promotion status. | yes |
| `ComponentLicenseDecision` | Path/component-scoped include, exclude, clean-room-only, or legal-review decision with license/notice hashes, obligations, owner, and revalidation trigger. | yes |
| `BmadPackageDescriptor` | Normalized distribution/install profiles, package/module/skill metadata, upstream manifest observations, final composite inventory, compatibility versions, capabilities, help refs, and validation refs. | yes |
| `BmadConfigLayer` | One central-TOML, per-skill-TOML, or compatibility-YAML layer with surface, order, ownership, source path/hash, parsed values, and warnings. | yes |
| `BmadMethodState` | Project-scoped authoritative method/workflow/step state with package/skill/config/execution-profile hashes and optimistic version; artifact presence alone cannot advance it. | yes |
| `BmadArtifactExpectation` | Artifact required or produced by a method step, including schema, target, provenance, status, and content hash/ref. | yes |
| `BmadPackage` | Read projection of a promoted package plus its project/delivery-specific `PackageActivation`; never the mutable Builder draft. | projection |
| `BuilderPackage` | Deprecated compatibility alias for `BuilderDraftSnapshot`; new contracts use explicit authoring, proposal, promotion, and activation objects. | yes |
| `BuilderAuthoringSession` | Mutable authoring activity and conversation/workspace lineage; may produce many immutable draft snapshots. | yes |
| `BuilderDraftSnapshot` | Immutable source/support-file snapshot emitted by authoring or import. | yes |
| `PackagePromotion` | Review, validation, rehearsal, trust, and catalog-publication decision for one immutable proposal hash. | yes |
| `PackageActivation` | Project/delivery binding of one promoted immutable version, with compatibility/policy/approval and deactivation/revocation evidence. | yes |
| `ExtensionManifest` | Optional non-BMAD extension descriptor with config schema, UI hints, contracts, compatibility, and provenance. | yes |
| `PackageInstallPolicy` | Install/update decision context for BMAD and future extension packages. | yes |
| `SkillPackageProposal` | Immutable review candidate derived from a `BuilderDraftSnapshot` or import, with origin, content hash, scan, support-file inventory, validation/rehearsal evidence, and proposal state. | yes |
| `ScenarioManifest` | Test scenario descriptor with surface, coverage IDs, execution kind, fixtures, success criteria, and proof assertions. | yes |
| `MaturitySurface` | Release-readiness surface with quality, completeness, coverage, support status, and evidence refs. | yes |
| `ConfigMigrationPlan` | Doctor/import plan that migrates old package/runtime config into the current canonical schema. | yes |
| `SafeArchiveExtractionReport` | Evidence record for package/archive extraction limits, blocked entries, hash inventory, and destination root. | yes |

## 2. State-Machine Authority

[[54 - State Machine Reference]] is the sole authority for state names, transitions, terminal states, retry/reclaim behavior, and invalid-transition examples. This object-model note defines identities, fields, relationships, hashes, and persistence semantics only. It must not duplicate abbreviated lifecycle diagrams because parallel summaries drift and become accidental contracts.

Every durable lifecycle object stores a state value defined in `54`, an optimistic version where concurrent mutation is possible, and the event/command id that caused its last accepted transition.

## 3. Hashing Rules

| Object | Hash Basis |
|---|---|
| ContextPack | ordered item refs + content hashes + trust labels + token counts + selection reasons + protected-input markers + redaction summary. |
| PromptCacheContract | system-prompt hash + tool-schema hash + BMAD-fixture hash + context-pack hash + provider cache mode + allowed transition reasons. |
| ModelProfile | role + exact deployment/snapshot/settings + capability hash + schema-policy hash + evaluation bundle hash + canary/rollback policy. |
| ProviderCapabilities | provider + deployment + region + model snapshot + API version + retention/tool/schema/feature support + observed timestamp. |
| ProviderSchemaProjection | canonical schema id/hash + projected schema hash + projector version + unsupported-keyword decisions. |
| ModelEvaluationBundle | candidate profile/prompt/context hashes + frozen dataset/grader hashes + per-lane results/thresholds + reviewer/policy identity. |
| Proposal | normalized proposal JSON + target snapshot/checkpoint. |
| ExecutionSpecCandidate | proposal hash + exact lane/image class + cwd/argv + environment/network policy + workspace target + mutable input hashes + declared outputs + limits + expiry. |
| ApprovedExecutionSpec | candidate hash + proposal hash + approval id + policy hash + executor audience + issue/expiry + single-use nonce. |
| ExecutionResultManifest | common delivery/authority/candidate/spec/policy/approval/result hashes plus the web template/image/lease fields or Windows host/workspace-grant/journal/checkpoint/executable/recovery fields selected by its discriminator. |
| EvidenceBundle | ledger cursor/range + proposal/candidate/policy/approval/spec/attempt/result/artifact/checkpoint/rollback hashes and materializer version. |
| TraceBundle | ordered diagnostic refs + redaction policy + export reason + source ledger cursor; never used to authorize or transition state. |
| EvidenceLedgerEvent | stream id + sequence + schema version + event type + actor/owner scope + causation/correlation + payload hash/ref. |
| SourceSnapshot | upstream URL + immutable ref/commit when known + archive hash + license/notice hashes + extracted file inventory/completeness hash + fixture inventory hash. Verification or promotion status is excluded. |
| SourceVerificationRecord | source snapshot hash + ordered checks/evidence refs + reviewer/tool identity + license/adoption decision + result timestamp. |
| ComponentLicenseDecision | source snapshot hash + path/component + license/notice hashes + decision + obligations/exclusion reason + owner/revalidation trigger. |
| BmadObservedInventory | installation-composite identity + ordered final observed paths/location kinds/host adapters + independently computed content hashes. |
| BmadPackageDescriptor | source snapshot hash + distribution/install profiles + upstream-manifest observation hashes + final composite inventory hash + normalized package/skill/config/help/execution-profile/capability hashes. |
| BmadMethodState | project + activated package/skill/config/execution-profile hashes + workflow/step + artifact expectation/evidence hashes + optimistic version. |
| BuilderDraftSnapshot | authoring-session lineage + ordered authored/support-file hashes + source/import refs + draft schema version. |
| SkillPackageProposal | draft/import hash + immutable proposed file inventory + target package/version + scan + validation/rehearsal evidence hashes. |
| PackagePromotion | immutable proposal hash + reviewer/policy/trust decision + accepted evidence hashes + catalog/version target + decision timestamp. |
| PackageActivation | promoted package/version hash + project + delivery/authority + compatibility/policy/approval hashes + activation/deactivation/revocation evidence. |
| ScenarioManifest | scenario ID + ordered coverage IDs + fixture hashes + assertion definitions. |

## 4. Audit Rule

A reviewer must be able to move from `EvidenceBundle` backward to every relevant object without needing hidden model chain-of-thought or external chat context.


---




---

## Implementation-depth contract

This file is part of the V6 implementation library. It is written as an implementation guide, not as a strategy memo. Every component must be built against the same system-wide constraints:

1. **The first executable slice comes before breadth.** The first demonstrable web proof must use owner-scoped chat, the sealed BMAD capability, untrusted workspace context, typed plan output, proposal/candidate creation, Airlock validation, exact approval, `sealed_test_fake` through the production port, manifest import, checkpoint, durable replay, and evidence.
2. **The delivery-specific authority owns lifecycle state.** The web Runtime API imports remote-worker facts into SQL; the signed desktop Rust host imports local-executor facts into SQLite. Workers, child processes, renderers, models, sync services, and support APIs do not advance authoritative lifecycle state.
3. **Airlock creates the only governed-execution token.** Model- or automation-originated workspace mutations, command runs, exports, package activation, runtime dependency restore, external mutation, and worker dispatch require an `ApprovedExecutionSpec` bound to an approved `ExecutionSpecCandidate`. Ordinary authenticated control-plane CRUD and offline build/CI work use their separate authority classes.
4. **The model does not own proposals.** Model Gateway returns typed model outputs. Run Orchestrator creates normalized `Proposal` records. Airlock validates proposals.
5. **No raw shell by default.** Commands are represented as `argv[]` plus policy metadata; `sh -c`, shell expansion, broad environment access, and open network access are blocked unless explicitly operator-approved.
6. **Every side effect is reconstructable.** Diffs, preimages, spec hashes, policy hashes, approvals, job image digests, result manifests, logs, artifacts, and rollback metadata must be traceable.
7. **Each module has ports.** Even inside a modular monolith, use explicit interfaces and contracts to avoid creating a god control plane.


## 1. Component identity

| Field | Value |
|---|---|
| Component | `Canonical Object Model` |
| Area | `Domain model` |
| Primary implementation package | `src/Runtime.Domain + schemas` |
| Runtime/technology | `C# domain entities + JSON schemas` |
| First-slice priority | `phase-0 foundation` |


## 2. Purpose

Define durable objects that all components use: Project, Thread, Run, Proposal, AirlockDecision, Approval, ApprovedExecutionSpec, ExecutionJob, Manifest, Artifact, Evidence, Package.

The implementation must be narrow enough to fit the corrected first vertical slice, but designed so BMAD package execution, the existing presentation adapter, Builder Studio, SkillOps, replay, and operator controls can plug into the same contracts later.


## 3. Owns / does not own

### Owns
- Object names
- Required fields
- Hash rules
- Links from lifecycle objects to the canonical states in [[54 - State Machine Reference]]
- Relationships
- Schema IDs
- Versioning rules

### Does not own
- UI-specific view models
- Provider-specific SDK objects
- Worker-only transient types


## 4. Public/API surface and internal ports

### Required API/routes or callable operations
- `All APIs use these objects or stable DTO projections`


### Internal contract rules

- Every boundary uses typed, schema-versioned values. C# uses `Runtime.Contracts` / `Runtime.Domain`, Rust uses generated contract types plus `desktop-domain`, and TypeScript uses generated web or desktop facade types; no generated DTO grants runtime authority.
- External payloads must be schema-versioned. Internal objects may evolve faster but must not leak into OpenAPI without a contract version.
- Every state mutation must be idempotent or protected by optimistic concurrency.
- Every governed execution operation must receive an `ApprovedExecutionSpec` bound to an unchanged approved candidate. Ordinary authenticated control-plane CRUD and offline build/CI mutations follow the authority classes in [[00 - Common Rules and Product Shape]].
- Every error response must use the standard error envelope with `code`, `message`, `correlationId`, `retryable`, and optional `detailsRef`.


### Starter interface/type sketch

```csharp
public interface IComponentPort<TRequest, TResult>
{
    Task<TResult> ExecuteAsync(TRequest request, CancellationToken ct);
}

public sealed record OperationContext(
    Guid ProjectId,
    Guid RunId,
    string ActorUserId,
    string CorrelationId,
    string PolicyVersion,
    DateTimeOffset RequestedAt);
```


## 5. State model

This component does not define a second lifecycle. Implementations use the exact state sets and transition guards in [[54 - State Machine Reference]]. Object schemas may add versioned fields, but they cannot add a state or transition until `54` is updated in the same change.

Schema lifecycle such as deprecation or migration is metadata, not a runtime domain state. It belongs in schema/version and migration records rather than in the run, proposal, work, package, or execution state machines.


## 6. Persistence responsibilities

### SQL tables or domain records touched
- `All core tables map to canonical model`

### Blob/object storage paths touched
- `Canonical object refs stored in blob manifests`


### Persistence rules

- In `web_managed`, SQL stores lifecycle state, compact indexes, ownership metadata, and references. In `windows_local`, SQLite stores the corresponding local authority records.
- In `web_managed`, Blob stores large immutable payloads: snapshots, logs, diffs, manifests, artifacts, exports, packages, diagnostic traces, and validation reports. In `windows_local`, encrypted local content-addressed storage holds authority-owned payloads and optional cloud upload is explicit. Durable ledger metadata and lifecycle state remain authoritative independently of diagnostic exports or replicas.
- Any Blob payload referenced from SQL must include content hash, schema version, created timestamp, and retention class.
- No raw secrets, broad credentials, or unredacted prompt/context payloads are stored by default.
- Migrations must be forward-safe and testable against fixture data.


## 7. Implementation sequence

This file follows, and does not redefine, [[51 - Master Implementation Sequence]]:

1. Phase -1/0 defines immutable source identity, BMAD compatibility, owner/principal, untrusted context, prompt contract, proposal/candidate/approval/spec, durable work, evidence-ledger, cursor, checkpoint, and sealed fixture objects.
2. Phase 1 persists only the objects required by the `sealed_test_fake` BMAD slice and proves their canonical transitions through [[54 - State Machine Reference]].
3. Phase 2 adds the security, replay, provider-neutral, and minimum cloud-readiness records needed by implemented surfaces.
4. Phase 3 adds real model-resolution/call/evaluation records without changing proposal authority.
5. Phase 4 adds ACA-backed work attempts, leases, authenticated manifests, and worker-image identity without changing the manifest contract.
6. Phase 5+ adds arbitrary package, artifact adapter, Builder, operator, and release records only as their milestones start.

Each phase adds public schemas before behavior, includes invalid fixtures, uses optimistic concurrency/idempotency, and updates `54` before introducing a new lifecycle state.


## 8. Validation and test plan

### Required tests
- DTOs map to domain objects
- schema examples validate
- hash computation deterministic
- object state values and transitions conform to [[54 - State Machine Reference]]


### Minimum test layers

| Layer | What to test | Required before merge |
|---|---|---|
| Unit | object validation, state transitions, parsing, policy predicates | yes |
| Contract | OpenAPI/JSON Schema compatibility, generated clients, worker manifests | yes for public/durable payloads |
| Integration | SQL + Blob references, dispatch/import, authz, Airlock boundary | yes for side-effect paths |
| E2E | chat → proposal → approval → execution → evidence | yes for first slice files |
| Replay/golden | BMAD package fixtures, presentation adapter, evidence bundle | yes before v1 beta |
| Security negative | prompt injection, secret leak, policy bypass, path traversal, raw shell | yes for all side-effect components |


## 9. Failure modes and recovery

| Failure | Detection | Required behavior | User/operator visibility |
|---|---|---|---|
| Invalid schema | contract validation | reject before persistence or dispatch | show actionable error with correlation ID |
| Stale proposal/preimage | hash mismatch | void proposal or require rebase/new proposal | show stale context warning |
| Approval expired | expiry check | reject dispatch | show re-approve option |
| Policy mismatch | policy hash mismatch | reject spec | operator audit event |
| Worker timeout | job monitor | mark job timed out; preserve partial logs | timeline event + retry option if safe |
| Manifest missing/invalid | manifest import validation | do not advance success state | incident/failure card |
| Partial success | checkpoint/validation state | enter `user_decision_required` or `kept_for_repair` | explicit decision card |
| Secret detected | scanner/redactor | redact and block if high confidence | security finding card/operator event |


## 10. Security and policy requirements

- Treat workspace files, package files, generated artifacts, model outputs, and logs as untrusted input.
- Never let untrusted content override system instructions, Airlock policy, command allowlists, network policy, or secret handling.
- Enforce project-level authorization on every read and write.
- Log security-relevant denials as audit events, but do not include raw secret values.
- Prefer fail-closed behavior when policy, identity, schema, or storage checks are ambiguous.
- Add negative tests for the most likely bypass path before writing happy-path code.


## 11. Observability

Minimum telemetry fields for this component:

- `correlation.id`
- `project.id`
- `run.id` when available
- `component.name`
- `operation.name`
- `operation.outcome`
- `policy.version` when applicable
- `spec.id` when applicable
- `job.id` when applicable
- `artifact.id` when applicable
- redaction counters, not raw secrets

Metrics to consider: request latency, state-transition count, policy denials, approval wait time, job duration, manifest import failures, schema validation failures, retry count, budget blocks, and evidence materialization time.


## 12. Acceptance criteria

- [ ] The component has a clear owner package and does not leak responsibilities into unrelated modules.
- [ ] Public routes/payloads are represented in OpenAPI/JSON Schema where applicable.
- [ ] Governed execution paths cannot execute without Airlock evaluation and a candidate-bound `ApprovedExecutionSpec`; ordinary control-plane CRUD uses explicit owner authorization/idempotency/audit.
- [ ] SQL lifecycle state is mutated only by the Runtime API/Application layer.
- [ ] Blob payloads have content hashes and schema versions.
- [ ] Tests include at least one negative/bypass case.
- [ ] Events and evidence are emitted for user-visible actions.
- [ ] The component is represented in the release gate matrix.
- [ ] The implementation does not introduce Cortex as a runtime namespace.
- [ ] Documentation includes deferred v1.5/v2 scope explicitly rather than silently omitting it.


## 13. Integration checklist

- [ ] Update `32 - Integration Contract Map.md` with any new caller/callee relationship.
- [ ] Update `25 - OpenAPI, Schemas, and Generated Clients.md` for public route or schema changes.
- [ ] Update `22 - Data Model - SQL and Blob.md`, `47 - Database DDL Starter.md`, or `48 - Blob Storage Layout.md` for persistence changes.
- [ ] Update `27 - Testing, Validation, and Replay.md` for new fixtures or replay needs.
- [ ] Update `33 - Release Gates and Acceptance Matrix.md` if the change affects release readiness.
- [ ] Add or update ADR in `31 - Architecture Decision Records.md` if the change alters architecture, hosting, policy, or security posture.


---

## Historical Revision Notes (V3 -> V4)
## Review finding

`34 - Canonical Object Model.md` is part of the implementation library support layer. In v3, support files were useful but not always testable. In v4, every support file must provide either a decision, reference contract, release gate, mapping, runbook, or checklist that can be executed by a developer or coding agent.

## Required usage

1. Read this file before changing the related implementation area.
2. Cross-check it against `07 - Source Coverage Matrix.md` and `50 - V4 Full Library Audit.md`.
3. When implementing a task, copy the relevant checklist items into the issue/story.
4. When a decision changes, update this file and `31 - Architecture Decision Records.md` in the same PR.
5. When a contract changes, update `25 - OpenAPI, Schemas, and Generated Clients.md`, `46 - API Route Catalog.md`, and generated clients.

## V4 quality rules for this file

- It must not contradict locked architecture decisions.
- It must not reintroduce a broad v1 scope that competes with the executable vertical slice.
- It must preserve BMAD source contracts and the existing presentation workflow adapter decision.
- It must reflect the Runtime API as lifecycle state owner and the worker as manifest/log producer only.
- It must identify whether guidance is `LOCKED`, `TEMPORARY`, `PHASE-0 SPIKE`, `V1`, `V1.5`, or `V2`.

## Implementation checklist linkages

| Related guide | What to cross-check |
|---|---|
| `01 - First Build - Executable Vertical Slice.md` | Does this file support or distract from the first slice? |
| `29 - Concurrency, Transactions, and Failures.md` | Are state and partial failure semantics compatible? |
| `32 - Integration Contract Map.md` | Are producer/consumer boundaries clear? |
| `33 - Release Gates and Acceptance Matrix.md` | Is there a release gate for this guidance? |
| `49 - Detailed Component Build Checklists.md` | Are implementation tasks represented as checklist items? |

## Hermes-Informed Canonical Objects

Source: [[86 - Hermes Source Code Review - Agent Runtime and Learning Loop]].

Add these objects to the canonical model:

| Object | Definition |
|---|---|
| `TurnContext` | Per-turn authority and routing context: run, session, turn, actor, approval mode, tool call, and correlation ids. |
| `PromptCacheContract` | Stable run-level prompt/tool/context hash contract and provider cache strategy. |
| `ToolAvailabilitySnapshot` | Effective tool/package capability set with health, unavailable reasons, schema hash, and generated-at timestamp. |
| `PendingKnowledgeWrite` | Staged memory, skill, package, or context write with origin, read evidence, summary, and replay payload. |
| `SkillPackageProposal` | Immutable BMAD skill/package create, patch, or delete candidate before review; promotion and project activation are separate objects and events. |
| `AutomationFireClaim` | Idempotent scheduled-job fire claim with CAS status and deduplication key. |
| `ConnectorCapabilityDescriptor` | External adapter capability and routing descriptor. |
| `SessionLifecycleRecord` | Session source, lifecycle flags, counters, transcript pointer, and resume/finalization state. |

Object names above are canonical. Do not introduce synonyms such as `ToolHealthBag`, `PromptCacheHint`, or `KnowledgePatch` without updating this file and the generated schemas.

## Hermes Deep-Review Canonical Objects

Source: [[87 - Hermes Deep Review - Extension Runtime and Operational Contracts]].

Add these object names to the canonical vocabulary:

| Object | Definition |
|---|---|
| `RuntimeProviderResolution` | Effective provider routing result for a single model call. |
| `ProviderCredentialBinding` | Provider credential scoped to provider, endpoint, account, and allowed usage. |
| `ContextCompressionRecord` | Durable record of context compression inputs, summary, retained ranges, and safety checks. |
| `EditorSessionContext` | Editor-origin session envelope with cwd, history, cancel, and permission bridge state. |
| `ToolEventCorrelation` | Stable mapping from streamed tool events to concrete tool-call ids. |
| `ConnectorConfigBridge` | Adapter-owned translation from platform config into generic runtime connector config. |
| `ConnectorCredentialLock` | Exclusive credential claim for persistent connector profiles. |
| `DeliveryTarget` | Normalized destination resolved at send/fire time. |
| `SecretSourceApplyReport` | Secret source provenance and conflict report from startup resolution. |
| `ProfileSecretScope` | Fail-closed scoped secret map for a profile/session/worker. |
| `VerificationEvidenceRecord` | Passive test/lint/build/ad-hoc verification evidence. |
| `TaskClaim` | Atomic worker claim with TTL, heartbeat, and reclaim metadata. |
| `DashboardSession` | Human console auth session. |
| `TokenPrincipal` | Machine caller identity and scopes. |
| `WebSocketTicket` | Single-use browser WebSocket credential. |
| `GatewayDrainRequest` | Operational drain request tied to process/container instantiation. |

## Odysseus-Informed Canonical Objects

Source: [[88 - Odysseus Source Code Review - Self-Hosted AI Workspace]].

Add these object names to the canonical vocabulary:

| Object | Definition |
|---|---|
| `DeploymentTrustProfile` | Azure environment exposure/auth/identity/network/execution-lane trust profile. The Odysseus `SelfHostedDeploymentProfile` source name is not a v1 local-deployment requirement. |
| `OwnerScope` | Canonical owner/project/legacy scope used by sessions, uploads, documents, tasks, memory, endpoints, and tokens. |
| `InternalLoopbackPrincipal` | Private runtime principal for in-process tool calls that still requires owner and privilege checks. |
| `OutboundUrlPolicy` | Policy object for external URL validation, redirect handling, DNS pinning, and private-network exceptions. |
| `DnsPinnedFetch` | Bounded fetch record tying the original URL to validated resolved IPs, redirect hops, and size/time limits. |
| `UploadObject` | Owner-scoped uploaded object with safe id, display name, hash, media type, and retention metadata. |
| `UploadIndexRecord` | Atomic index entry for upload lookup, cleanup, quarantine, and recovery. |
| `FileToolWorkspaceScope` | Allowed file-tool root, denied paths, symlink policy, and read/search limits. |
| `UntrustedContextEnvelope` | Guarded model-context wrapper for external or user-editable content treated only as data. |
| `AdaptiveContextBudget` | Model-aware token budget and trimming strategy for context assembly. |
| `ToolLoopProgressGuard` | Repeated-tool-call stall detector and action classifier. |
| `ProviderEndpointProfile` | Owner-scoped model/provider endpoint configuration and network classification. |
| `ModelCatalogEntry` | Normalized provider model record with visibility, modality, and context metadata. |
| `ProviderProbeStatus` | Health/degraded state for provider or local model endpoint probes. |
| `ScheduledTaskChain` | Owner-checked scheduled task handoff with cycle guard. |
| `BackgroundJobRecord` | Durable long-running job state and cancellation semantics. |
| `DocumentSourceBinding` | Owner-checked binding between documents and source uploads/images/signatures. |
| `SkillRetrievalAudit` | Precision and breadth audit for skill/package retrieval triggers. |
| `MemoryProviderHealth` | Active memory provider state, vector health, fallback mode, and owner-filtered recall status. |

## Consolidated Source-Review Canonical Groups

Source: [[89 - Consolidated AI Workspace Source Review and Architecture Improvements]].

Group canonical objects by implementation slice:

| Slice | Required objects |
|---|---|
| Runtime identity | `OwnerScope`, `PrincipalContext`, `TurnContext`, `TokenPrincipal`, `InternalLoopbackPrincipal`, `DashboardSession`, `WebSocketTicket`. |
| Prompt and tools | `PromptCacheContract`, `ToolAvailabilitySnapshot`, `ToolEventCorrelation`, `UntrustedContextEnvelope`, `AdaptiveContextBudget`, `ToolLoopProgressGuard`. |
| Provider routing/evaluation | `RuntimeProviderResolution`, `ProviderCapabilities`, `ProviderSchemaProjection`, `ProviderCredentialBinding`, `ModelProfile`, `ModelEvaluationBundle`, `ProviderEndpointProfile`, `ModelCatalogEntry`, `ProviderProbeStatus`. |
| Workspace files | `UploadObject`, `UploadIndexRecord`, `FileToolWorkspaceScope`, `DocumentSourceBinding`, `ContextCompressionRecord`. |
| Execution and tasks | `ExecutionSpecCandidate`, `ApprovedExecutionSpec`, `ExecutionLaneSpec`, `WorkItem`, `WorkAttempt`, `WorkLease`, `WorkCompletion`, `BackgroundJobRecord`, `AutomationFireClaim`, `TaskClaim`, `ScheduledTaskChain`. |
| Events and evidence | `EvidenceLedgerEvent`, `OutboxMessage`, `EventCursor`, `ProjectionCheckpoint`, `ExecutionResultManifest`, `EvidenceBundle`, `TraceBundle`. |
| Packages and knowledge | `SourceSnapshot`, `SourceVerificationRecord`, `ComponentLicenseDecision`, `SkillPackageProposal`, `PendingKnowledgeWrite`, `SkillRetrievalAudit`, `MemoryProviderHealth`, `ConnectorCapabilityDescriptor`. |
| Operations | `SecretSourceApplyReport`, `ProfileSecretScope`, `GatewayDrainRequest`, `VerificationEvidenceRecord`, `DeploymentTrustProfile`. |
