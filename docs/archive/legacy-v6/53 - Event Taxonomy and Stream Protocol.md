---
title: "Event Taxonomy and Stream Protocol"
aliases:
  - "53 - Event Taxonomy and Stream Protocol"
tags:
  - bmad-runtime
  - vault/implementation-assets
section: "Implementation Assets"
order: 53
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: event-protocol
status: implementation-guide
---



# Event Taxonomy and Stream Protocol

## V6.17 event envelope

Every authoritative event includes `deliveryModel`, `authorityRef`, `streamId`, monotonic authority-local `sequence`, `previousEventHash`, `eventHash`, schema version, actor kind, correlation/causation, payload hash/ref, and recorded time. Actor kinds include `cloud_control_plane`, `remote_worker`, `desktop_host`, `local_process`, `user`, and `support_service` with no implied authority escalation.

Web streams are projected over SSE from the cloud ledger. Desktop streams are projected over Tauri events from SQLite and can be replayed after renderer restart; renderer events are never authoritative. Sync exports signed envelopes without renumbering or merging source sequences. A cross-delivery view links two ledgers by handoff hashes rather than creating one total order.

## 1. Durable event envelope

```json
{
  "eventId": "evt_...",
  "streamId": "run:run_...",
  "sequence": 42,
  "aggregateType": "run|package|work_item|source_snapshot|project",
  "aggregateId": "run_...",
  "runId": "run_... | null",
  "projectId": "prj_... | null",
  "ownerScopeRef": "ownerscope_...",
  "type": "proposal.created",
  "schemaVersion": "2026-07-09.v1",
  "occurredAt": "2026-07-09T10:00:00Z",
  "actorType": "user|service|worker|system",
  "actorId": "...",
  "correlationId": "trace-id",
  "causationId": "command-or-prior-event-id",
  "payloadHash": "sha256:...",
  "payloadRef": "blob://...",
  "redactionLevel": "summary|redacted|privileged",
  "retentionClass": "operational|evidence|debug|privileged"
}
```

`EvidenceLedgerEvent` is the authoritative persisted form of this envelope. A `RunEvent` is its run-scoped projection. Package activation, Source Intake, operator, or project events use their own stream and may have no `runId`; `streamId + sequence` is the ordering key. Event IDs are globally unique and consumers deduplicate them.

## 2. Required event families

### Thread events

- `thread.created`
- `thread.message.created`
- `thread.message.redacted`

### Run events

- `run.created`
- `run.intent.classified`
- `run.state.changed`
- `run.blocked`
- `run.completed`

### Context events

- `workspace.scan.requested`
- `workspace.scan.completed`
- `context.search.completed`
- `context.pack.created`
- `context.pack.invalidated`

### BMAD method and package events

- `bmad.install_profile.detected`
- `bmad.package.normalized`
- `bmad.capability.selected`
- `bmad.workflow_step.started`
- `bmad.workflow_step.completed`
- `bmad.artifact.expected`
- `bmad.artifact.updated`
- `bmad.package.validation_completed`
- `bmad.package.activated`
- `bmad.package.deactivated`

Every BMAD event payload includes the source snapshot ID, install profile, package ID/version, skill ID/hash, config hash, and—when applicable—workflow-step and artifact IDs/hashes. Package activation/deactivation also includes the policy decision and evidence references.

### Model events

- `model.call.queued`
- `model.call.started`
- `model.call.completed`
- `model.output.schema_invalid`
- `model.call.failed`
- `model.profile.evaluation_completed`
- `model.profile.canary_started`
- `model.profile.activated`
- `model.profile.rolled_back`

Profile events include the exact deployment/snapshot, `ProviderCapabilities`, credential/retention class, canonical/projected schema policy hashes, evaluation-bundle hash, per-lane pass/fail, fallback graph, and rollback target. They never contain secret values or raw prompts.

### Source Intake and license events

- `source.snapshot.acquired`
- `source.snapshot.verification_completed`
- `source.component_license.decided`
- `source.snapshot.promoted`
- `source.snapshot.quarantined`

Promotion is impossible without immutable origin/ref where required, archive/tree and extraction evidence, fixture/copied-file hashes, and every applicable `ComponentLicenseDecision`.

### Proposal and policy events

- `proposal.created`
- `proposal.voided`
- `execution_spec_candidate.created`
- `execution_spec_candidate.voided`
- `policy.evaluation.started`
- `policy.evaluation.completed`
- `approval.required`
- `approval.approved`
- `approval.rejected`
- `approved_spec.issued`

Every approval event includes the immutable `ExecutionSpecCandidate` id and hash. An issued spec also includes its own hash, executor audience, issue/expiry, and single-use nonce reference.

### Durable work and delivery events

- `work_item.created`
- `work_item.cancel_requested`
- `work_attempt.started`
- `work_attempt.completed`
- `work_attempt.failed`
- `work_lease.acquired`
- `work_lease.heartbeat`
- `work_lease.expired`
- `work_lease.reclaimed`
- `work_completion.recorded`
- `work_completion.import_acknowledged`
- `work_completion.quarantined`

`OutboxMessage` delivery status is durable operational state, not another ordinary message sent through the same outbox. Persisting `delivered`, `retry_pending`, or `poisoned` must not recursively enqueue a delivery-status event. An operator incident may be appended to a separate operator stream by an explicit reconciliation command.

### Execution events

- `execution.dispatched`
- `execution.job.created`
- `execution.log.chunk`
- `execution.completed`
- `execution.failed`
- `execution.manifest.imported`
- `execution.manifest.rejected`

### Validation events

- `validation.started`
- `validation.completed`

### Workspace events

- `workspace.snapshot.created`
- `workspace.checkout.created`
- `workspace.preimage.captured`
- `workspace.preimage.drifted`
- `workspace.checkpoint.created`
- `workspace.rollback.completed`

### Evidence events

- `evidence.materialization.started`
- `evidence.materialization.completed`
- `evidence.materialization.failed`
- `trace.redaction.applied`

This taxonomy is canonical. Other notes must use these exact event types; a new event type is added here first, in the same change that introduces it elsewhere.

## 3. Persistence, replay, and streaming rules

- Sequence is strictly increasing per `streamId` and assigned in the same transaction as the authoritative state transition and outbox record.
- The event/evidence ledger is authoritative. SSE, SignalR, WebSocket, OTEL, logs, dashboards, and `TraceBundle` exports are rebuildable projections and may not advance lifecycle state.
- Client reconnect sends an opaque `EventCursor` containing stream, last sequence, schema epoch, and retention/gap status. The server replays durable events first and then switches to live delivery without a race window.
- A cursor older than retained history returns an explicit cursor-expired/gap response plus the projection/snapshot needed to reconcile. It never silently starts from “now.”
- Delivery is at least once. Consumers deduplicate by `eventId`; projection handlers checkpoint `(projectionName, streamId, sequence)` atomically.
- The stream publisher reads from `OutboxMessage`, not transaction-local memory. Retry exhaustion produces a visible poisoned-delivery state and operator evidence; governed evidence events are never silently dropped.
- Event schemas are versioned. Upcasters are pure, deterministic, side-effect-free, and tested from every retained durable version to the current projection version.
- Unknown future event types render a safe diagnostic card and preserve cursor progress only when the envelope declares them ignorable; unknown required state events stop the projection and surface a compatibility finding.
- Event payload refs and their retention class must remain valid for at least the lifetime of every durable cursor or projection that depends on them.
- Log chunks may be summarized/indexed in SQL, but bounded full content stays in Blob. Redaction occurs before UI/model projections.
- Evidence materialization failure does not rewrite the imported execution outcome. It retries as a projection and remains visibly incomplete until repaired.

## 4. Replay types

| Replay type | Purpose | May execute side effects? |
|---|---|---|
| Transport replay | Resume a client from `EventCursor`. | No. |
| Projection rebuild | Recreate UI/search/operator views from the durable ledger. | No. |
| Scenario replay | Run deterministic fake provider/executor fixtures against contracts. | Simulated effects only inside a sealed, non-isolating test fixture; no process/network/package execution. |
| Rehearsal | Validate package/worker behavior in an approved isolated lane. | Yes, but only under a new proposal/candidate/approval/spec and never by replaying an old event. |
| Forensic reproduction | Explain a historical run from hashes/refs and privileged retained payloads. | No. |
