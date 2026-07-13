---
title: "State Machine Reference"
aliases:
  - "54 - State Machine Reference"
tags:
  - bmad-runtime
  - vault/implementation-assets
section: "Implementation Assets"
order: 54
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: state-machine-reference
status: implementation-guide
---



# State Machine Reference

## 0. V6.17 authority rule

“Runtime owner” means the .NET control plane for `web_managed` and the Rust local host for `windows_local`. Every stateful object carries immutable `deliveryModel` and `authorityRef`; only that authority executes transitions and appends evidence. A worker, child process, renderer, sync service, model, or the other delivery model can only report/propose facts.

The common semantic path below remains canonical, but dispatch/import states are realized differently:

| Semantic phase | Web-managed realization | Windows-local realization |
|---|---|---|
| workspace ready | immutable cloud snapshot | valid selected-folder capability + base checkpoint |
| executing | fixed remote worker attempt | local journal/apply or structured process attempt |
| result available | authenticated worker manifest in Blob | local result manifest staged in encrypted CAS |
| import/commit | API SQL transaction + outbox | Rust SQLite transaction + evidence append |
| checkpointed | cloud checkpoint manifest | local checkpoint/journal finalized |

Desktop journal recovery has explicit non-terminal states `apply_preparing`, `apply_mutating`, `apply_recovering`, `rollback_preparing`, `rollback_mutating`, and terminal dispositions `applied`, `rolled_back`, `recovered_to_preimage`, `recovery_requires_user`, or `quarantined`. A remote handoff moves `selected → consented → uploaded → cloud_work_created → cloud_terminal → result_imported_as_local_proposal`; it never transitions directly to local `approved`, `executing`, or `checkpointed`.

## V6.18 current-authority overlay — BMAD Method and Builder lifecycles

This overlay applies [[100 - BMAD Method and Builder Deep Comprehension Audit]] and supersedes the collapsed package/Builder lifecycle retained later in this historical note.

Every imported Method skill is bound to a hashed `PromptSkillExecutionProfile`: `persona_session`, `guided_intent`, `jit_steps`, `embedded_xml`, `rendered_workflow`, `unattended_iteration`, `compatibility_forwarder`, or `atomic_utility`. The profile selects an orchestration adapter and records filesystem/process/network/web/subagent/tool requirements; it grants no transition or side-effect authority. Model output, memlog state, or artifact presence remains evidence only until the delivery authority validates and imports it.

### BuilderAuthoringSession

```text
created
→ authoring
→ draft_snapshot_recorded
→ authoring

authoring → submitted|paused|cancelled
paused → authoring|cancelled
```

Each `draft_snapshot_recorded` event creates a new immutable `BuilderDraftSnapshot`; the session may loop without mutating an older snapshot.

### SkillPackageProposal

```text
drafted
→ static_validating
→ validated
→ rehearsal_pending
→ rehearsing
→ rehearsed
→ promotion_requested
```

Branch/terminal states:

```text
validation_failed
rehearsal_failed
abandoned
quarantined
```

Validation and rehearsal append evidence to the immutable proposed content; a content change creates a new proposal identity.

### PackagePromotion

```text
requested
→ evidence_checking
→ awaiting_decision
→ approved
→ catalog_published
```

Branch/terminal states:

```text
rejected
quarantined
catalog_published → revoked
```

### PackageActivation

```text
requested
→ compatibility_checking
→ policy_evaluated
→ awaiting_approval
→ activation_authorized
→ activating
→ active
```

Policy may move `policy_evaluated` directly to `activation_authorized` only when the action class explicitly permits policy authorization without a human decision. Branch/terminal states are:

```text
blocked
rejected
activation_failed
active → deactivated|revoked
```

Builder authoring cannot transition a proposal, promotion, or activation directly. Submission, promotion request, catalog publication, and activation are authority-owned commands that create or transition separately linked objects. Reactivation after deactivation requires a new `PackageActivation`; revoked content requires a new reviewed package version.

## 1. Authority and transition rules

This file is the sole authority for canonical state names and transitions. Other notes may link to these machines but must not publish abbreviated alternatives.

Every accepted transition:

- is executed by the delivery-specific domain owner, never by a model, package, UI/renderer, sync service, child process, or worker;
- checks the current state and optimistic version;
- is idempotent by command/event key;
- appends an `EvidenceLedgerEvent` and `OutboxMessage` in the same transaction;
- records actor/principal, owner scope, correlation, causation, and schema version;
- rejects an unknown transition rather than coercing it to a nearby state.

Remote workers and the local execution engine report facts in their discriminated `ExecutionResultManifest`; authority-owned import commands cause transitions.

## 2. Run state machine

Primary path:

```text
created
→ intent_classified
→ context_selecting
→ context_ready
→ model_working
→ proposal_ready
→ awaiting_approval
→ approved
→ dispatching
→ executing
→ manifest_importing
→ validation_succeeded
→ checkpointed
→ evidence_pending
→ evidence_ready
→ completed
```

Branch/terminal states:

```text
blocked_missing_input
blocked_policy_denied
blocked_budget_exceeded
proposal_voided
approval_rejected
approval_expired
execution_failed
manifest_invalid
patch_applied_validation_failed
kept_for_repair
rolled_back
user_decision_required
cancelled
```

`evidence_pending` is not permission to discard an execution outcome. If materialization fails, the run keeps its imported execution/checkpoint facts and exposes the independent evidence failure state defined below.

## 3. Proposal state machine

```text
draft_from_model_output
→ normalized
→ hashed
→ accepted
→ execution_candidate_created
→ consumed
```

Invalid/terminal states:

```text
schema_invalid
voided_by_checkpoint
superseded
cancelled
```

A proposal is platform-normalized intent, not execution authority. `accepted` means schema/domain-valid only.

## 4. ExecutionSpecCandidate and approval state machine

```text
candidate_created
→ candidate_hashed
→ policy_evaluated
→ awaiting_approval
→ candidate_approved
→ approved_spec_issued
→ candidate_consumed
```

Policy may move `policy_evaluated` directly to `candidate_approved` only when the action class explicitly permits policy authorization without a human decision. Terminal states are:

```text
policy_denied
rejected_by_user
expired
voided_by_checkpoint
voided_by_candidate_drift
superseded
```

An `Approval` stores the candidate id/hash. `ApprovedExecutionSpec` issuance re-hashes the candidate and fails unless the approved and current hashes match exactly. Issuance may add only its own approval id, policy hash, executor audience, issue/expiry, and single-use nonce; it may not expand command, inputs, outputs, network, lane/image class, or limits.

## 5. Execution state machine

```text
created_from_approved_spec
→ dispatching
→ dispatched
→ running
→ manifest_available
→ manifest_importing
→ manifest_imported
→ succeeded|failed|cancelled|timed_out
```

Failure states:

```text
dispatch_failed
lease_lost
worker_failed
manifest_missing
manifest_invalid
manifest_authentication_failed
import_conflict
```

`succeeded` requires an authenticated or locally authority-produced, schema-valid, spec-hash-matching `ExecutionResultManifest` of the correct delivery branch. A late or duplicate result may confirm the existing terminal outcome but cannot create a second success.

## 6. Durable work state machines

### WorkItem

```text
created
→ queued
→ attempt_pending
→ running
→ completion_pending_delivery
→ completed|failed|cancelled|timed_out|lost
```

### WorkAttempt

```text
created
→ lease_pending
→ leased
→ started
→ heartbeat_active
→ completion_recorded
→ succeeded|failed|cancelled|timed_out|lost
```

An attempt is immutable after terminal completion. Retry creates a new attempt number under the same `WorkItem`; it does not reopen the old attempt.

### WorkLease

```text
available
→ acquired
→ renewed
→ released
```

Each accepted heartbeat may perform the guarded self-transition `renewed → renewed` with a later expiry and optimistic version.

Alternative terminal states:

```text
expired|revoked
```

Acquire/renew/reclaim uses database compare-and-swap. Reclaim after expiry creates or activates a new attempt only after liveness checks prove the previous holder cannot still commit success.

### WorkCompletion

```text
created
→ result_validated
→ recorded_with_outbox
→ import_acknowledged
```

Alternative terminal states:

```text
rejected_invalid|rejected_conflict|quarantined_unknown
```

`WorkCompletion` is immutable after `recorded_with_outbox` and is unique by work item, attempt, audience, and completion nonce. The lifecycle transition, completion record, `EvidenceLedgerEvent`, and `OutboxMessage` commit atomically. A crash before acknowledgement causes redelivery/import of the same completion; it never reopens the consumed spec or starts another effect. Conflicting terminal claims enter explicit quarantine/operator review.

### OutboxMessage

```text
pending
→ publishing
→ delivered
```

Failure path:

```text
publishing → retry_pending → publishing
retry_pending → poisoned
```

`poisoned` is visible operator state. It cannot silently convert governed evidence delivery to success.

## 7. Checkpoint and rollback state machines

Checkpoint:

```text
none
→ pending_after_manifest_import
→ file_manifest_computed
→ checkpoint_recorded
→ rollback_available
```

Rollback:

```text
rollback_requested
→ rollback_candidate_created
→ rollback_policy_checked
→ rollback_approved_spec_issued
→ rollback_running
→ rollback_manifest_imported
→ rollback_checkpoint_recorded
→ rollback_completed
```

Rollback is a new governed side effect. Historical approval/spec records are evidence, not reusable authorization.

## 8. BMAD method and artifact state machines

### BmadMethodState

```text
uninitialized
→ ready
→ step_in_progress
→ step_completed
→ ready
→ method_completed
```

Branch states:

```text
blocked_missing_capability
blocked_missing_input
blocked_artifact_invalid
blocked_policy
cancelled
```

### BmadArtifactExpectation

```text
expected
→ proposed
→ produced_pending_import
→ accepted
→ superseded
```

Invalid/branch states:

```text
rejected_schema
rejected_hash
rejected_provenance
validation_failed
```

Model output or a worker-uploaded file alone cannot complete a method step or accept an artifact. Only Runtime import of a valid manifest/artifact result can cause `produced_pending_import → accepted` and the corresponding method-step transition.

## 9. Historical collapsed Package and Builder lifecycle (superseded by V6.18)

> Retained for change history only. The V6.18 object-specific machines above are canonical and supersede this combined chain, including its direct reactivation path.

```text
draft
→ validated
→ rehearsed
→ awaiting_approval
→ active
```

Alternative states:

```text
validation_failed
rehearsal_failed
rejected
quarantined
deactivated
revoked
```

Allowed exits include `validated → quarantined`, `rehearsed → rejected`, `active → deactivated`, and `active → revoked`. Reactivation follows `deactivated → awaiting_approval → active` only after a fresh compatibility, rehearsal-validity, and policy check.

An active package version is immutable. Editing produces a new draft/version. Reactivation after deactivation requires a fresh compatibility/policy check; revoked content cannot reactivate without a new reviewed version.

## 10. Source Intake and component-license state machines

Source snapshot:

```text
acquired
→ safely_extracted
→ verification_pending
→ verified|verification_incomplete|verification_failed
→ component_license_review
→ research_only|promotable|quarantined
→ promoted|retired
```

A snapshot without required immutable upstream identity may be `research_only` but not `promotable`. Extraction/file completeness cannot substitute for commit/tag/signature provenance.

Component license decision:

```text
pending
→ include|exclude|clean_room_pattern_only|legal_review_required
```

`legal_review_required` is blocking, not an implicit include. Snapshot promotion requires a terminal allowed decision for every inventoried component and a copied/derived-file map.

## 11. ModelProfile promotion state machine

```text
candidate
→ offline_evaluated
→ policy_approved
→ canary
→ active
→ retired
```

Failure/rollback paths:

```text
candidate|offline_evaluated|policy_approved|canary → rejected
canary|active → rolled_back
rolled_back → retired
```

`offline_evaluated` requires separate contract, task-quality, safety/privacy, and operations results. A critical-lane failure cannot be averaged away. `active` requires exact deployment capabilities/credential/retention/schema policy, immutable evaluation evidence, canary thresholds, explicit fallback edges, and a tested rollback target. The candidate model cannot approve its own promotion.

## 12. Evidence materialization state machine

```text
pending
→ materializing
→ ready
```

Failure path:

```text
materializing → failed → retry_pending → materializing
```

The `EvidenceBundle` is a materialized view over authoritative ledger and object references. `TraceBundle` is diagnostic only. Evidence failure is visible and release-blocking where required, but it never changes an imported execution from failed to succeeded or vice versa.

## 13. Invalid transition examples

| Attempt | Reject reason |
|---|---|
| `proposal_ready → executing` | Missing candidate, policy evaluation, approval, approved spec, work attempt, and dispatch. |
| `awaiting_approval → dispatching` | Approval decision and unchanged candidate hash were not persisted. |
| `candidate_approved → approved_spec_issued` after candidate mutation | Candidate hash drift voids approval. |
| `running → succeeded` from an executor report alone | The delivery authority must validate and import the correct `ExecutionResultManifest` branch first. |
| `manifest_imported → completed` | Checkpoint and evidence materialization states remain unresolved. |
| `evidence.materialization failed → execution_failed` | Evidence projection failure cannot rewrite execution truth. |
| `work_attempt.failed → started` | Retry requires a new attempt. |
| `step_in_progress → step_completed` from model output | BMAD state advances only from imported evidence. |
| `draft package → active` | Validation, rehearsal, and approval cannot be skipped. |
| `work_completion.recorded_with_outbox → created` | Completion is immutable; redeliver/import the same nonce instead of reopening the effect. |
| `research_only source → promoted` | Required immutable provenance and/or component-license decisions are missing. |
| `model profile candidate → active` | Four-lane evaluation, policy approval, canary, explicit fallback, and rollback evidence cannot be skipped. |
| `rollback_requested → rollback_running` | Rollback needs a new candidate, policy evaluation, and approved spec. |
