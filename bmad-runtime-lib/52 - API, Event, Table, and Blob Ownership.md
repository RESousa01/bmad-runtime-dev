---
title: "API, Event, Table, and Blob Ownership"
aliases:
  - "52 - API, Event, Table, and Blob Ownership"
tags:
  - bmad-runtime
  - vault/implementation-assets
section: "Implementation Assets"
order: 52
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: ownership-map
status: v6-modernized-implementation-guide
validated_on: 2026-07-09
---



# API, Event, Table, and Blob Ownership

## V6.17 ownership matrix

| Resource | Shared definition owner | `web_managed` runtime owner | `windows_local` runtime owner |
|---|---|---|---|
| Project/run/proposal schema | contracts package | Runtime API + Azure SQL | Rust domain + SQLite |
| Workspace | `WorkspaceTarget` union | Cloud Workspace Service + Blob | Workspace Capability Broker + selected folder |
| Approved spec | canonical schema/Airlock rules | .NET Airlock, Azure audience | Rust Airlock, exact local-host audience |
| Execution result | `ExecutionResultManifest` union | Fixed remote worker + API importer | Rust runner/apply engine + local importer |
| Evidence | event/hash contract | SQL ledger + Blob payloads | SQLite ledger + encrypted local CAS |
| UI | shared components | WebRuntimeFacade | DesktopRuntimeFacade |
| Sync/remote job | handoff/envelope schemas | support API stores replica/cloud work | local host selects/imports; remains local authority |

An owner may expose a projection but cannot delegate authoritative transition rights to the other delivery model. Azure support records never own local path/spec/checkpoint state.

## 1. Purpose

This file prevents god-control-plane drift. A route, event, SQL table, or Blob prefix must have one owner. Other modules consume through ports, not direct writes. Ownership here means responsibility for contract, authorization, lifecycle, retention, migration, test fixtures, and backward compatibility.

## 2. Ownership principles

- Public route owner is responsible for OpenAPI contract, authorization, idempotency, validation, and error model.
- Domain event owner is responsible for schema version, ordering rules, replay behavior, and compatibility.
- SQL table owner is responsible for migrations, state transitions, indexes, retention, and row-level authorization semantics.
- Blob prefix owner is responsible for schema version, hash validation, lifecycle policy, retention class, and access grants.
- Cross-owner writes require application service/port methods, not direct repository/table access.
- Worker processes own only their local execution and Blob result manifest; they do not own SQL lifecycle state.

## 3. Critical ownership map

| Object | Owner | Consumers | Write rule | Notes |
|---|---|---|---|---|
| `/api/runs` | Runtime API / Runs | Chat UI, Orchestrator | Runtime API only | Creates run lifecycle only. |
| `/api/messages` | Runtime API / Threads | Chat UI, Orchestrator | Runtime API only | User/model-visible messages; redact before trace. |
| `/api/proposals` | Runtime API / Proposals | Airlock, Chat UI | Orchestrator via proposal port | Created from typed model output, not directly by Model Gateway. |
| `/api/approvals/*` | Airlock/Approvals | Chat UI, Airlock spec factory | Approval module only | Records a decision bound to the exact candidate hash; only Airlock issues `ApprovedExecutionSpec`. |
| `/api/executions/*` | Execution Dispatcher | Chat UI, Evidence | Runtime API dispatch/import only | Worker produces manifest, API imports state. |
| `/api/workspaces/*` | Workspace Service | Context, Execution, Evidence | Workspace Service only | Snapshot/checkpoint/preimage authority. |
| `/api/context-packs/*` | Workspace Intelligence | Orchestrator, UI | Context service only | Content provenance and redaction required. |
| `/api/models/*` | Model Gateway | Orchestrator, Operator | Model Gateway only | Exact capabilities, schema projections, profiles/evaluations/canaries/rollback, quotas, cost summaries. |
| `/api/source-intake/*` | Source Intake | Package Importer, Release | Source Intake only | Snapshot verification and component-license decisions; no runtime activation. |
| `/api/runs/{id}/evidence` | Evidence Service | Chat UI, Operator, Release | Materializer only; source ledger remains immutable | Canonical EvidenceBundle over ledger/object hashes. |
| `/api/packages/import` | BMAD Kernel | Help Advisor, Builder | BMAD Kernel import port | Invalid package cannot activate capabilities. |
| `/api/artifacts/*` | Artifact Service | Artifact Creator, Evidence | Artifact Service only | Export uses approved spec for side effects. |
| `/api/operator/*` | Operator Console/API | Operators only | Operator service only | Separate scopes and audit events. |

## 4. SQL ownership map

| Table | Owner | Mutable? | Required guards |
|---|---|---:|---|
| `projects` | Project/RBAC | yes | tenant/project authorization, audit on assignment changes. |
| `threads` | Thread service | yes | project scope, user membership. |
| `messages` | Thread service | append-only | content classification, redaction flags. |
| `runs` | Run State Store | state-machine only | legal transitions, idempotency key, actor, rowversion. |
| `proposals` | Proposal Store | append + terminal updates | schema version, input context hash, stale/void checks. |
| `execution_spec_candidates` | Orchestrator/Airlock boundary | immutable + terminal void state | complete normalized effect hash, mutable inputs, audience/template, expiry. |
| `approvals` | Airlock | append + terminal updates | expiry, policy hash, approval actor, spec hash. |
| `approved_execution_specs` | Airlock | immutable + single consumption | candidate/proposal/approval/policy hashes, audience/template, issue/expiry, nonce, worker image digest. |
| `executions` | Execution Dispatcher | state-machine only | dispatch ID, idempotency, manifest import only. |
| `work_items`, `work_attempts`, `work_leases`, `work_completions` | Runtime Work Store | state machine / immutable completion | owner scope, CAS/heartbeat/reclaim, idempotency, completion nonce and conflict quarantine. |
| `outbox_messages` | Runtime Outbox | append + delivery state | transactionally written with state/evidence; poison state visible. |
| `evidence_ledger_events` | Runtime Evidence Ledger | append-only | stream/sequence, owner/aggregate, schema, causation/correlation, payload hash/ref. |
| `workspace_snapshots` | Workspace Service | immutable | content hash, source ref, retention class. |
| `workspace_checkouts` | Workspace Service | yes | TTL, lock ownership, job scope. |
| `checkpoints` | Workspace Service | append-only | parent checkpoint, file manifest hash, rollback metadata. |
| `model_calls` | Model Gateway | append-only | redaction mode, cost, latency, schema status. |
| `model_profiles`, `model_evaluation_bundles` | Model Gateway/Evaluation | versioned state / immutable bundles | exact deployment/capability/schema/retention, four-lane thresholds, canary/rollback. |
| `source_snapshots`, `source_verifications`, `component_license_decisions` | Source Intake | immutable/versioned decisions | origin/ref/hash/extraction plus path-level include/exclude/clean-room/legal-review. |
| `trace_events` | Trace Writer | rebuildable compact projection | no large payloads, payload refs only; never evidence authority. |
| `artifact_versions` | Artifact Service | append-only | content hash, provenance ref, export status. |
| `audit_events` | Security/Operator | append-only | actor, target, decision, IP/session metadata where available. |

## 5. Blob ownership map

| Prefix | Owner | Writer | Reader | Validation |
|---|---|---|---|---|
| `snapshots/{snapshotId}/` | Workspace Service | Workspace Service | Execution, Context | content manifest hash. |
| `checkouts/{checkoutId}/` | Workspace Service | Workspace Service / worker mount | Worker only during job | TTL cleanup. |
| `logs/{executionId}/` | Execution Worker | Worker | UI, API importer, Evidence | chunk hash, redaction marker. |
| `manifests/{executionId}/` | Execution Worker | Worker | Runtime API importer | JSON Schema + hash validation. |
| `diffs/{proposalId}/` | Proposal service | Runtime API | UI, Airlock | preimage hashes and file risk labels. |
| `artifacts/{artifactId}/versions/{versionId}/` | Artifact Service | Artifact Service / approved worker | UI, Evidence | content hash and provenance. |
| `trace-bundles/{runId}/` | Trace Writer | Trace Writer | Evidence, privileged operator | redacted default view. |
| `evidence/{runId}/bundles/{bundleId}/` | Evidence Service | Evidence materializer | UI, operator, release | ledger range + canonical object hashes + materializer version. |
| `exports/{exportId}/` | Artifact/Export Service | approved worker | user download/evidence | expiry and external-destination policy. |
| `replay-fixtures/{fixtureId}/` | Testing/Replay | CI/test harness | CI/replay runner | deterministic input/output hash. |

## 6. Event ownership map

| Event family | Owner | Delivery | Notes |
|---|---|---|---|
| `run.*` | Run State Store | Evidence Ledger/outbox then SignalR/SSE projection | compact lifecycle events only; transport is not authority. |
| `proposal.*` | Proposal Store | UI + Evidence | no raw model context. |
| `approval.*` | Airlock | UI + audit | include policy hash/spec hash. |
| `execution.*` | Execution Dispatcher | UI + Evidence | logs are refs, not SQL payloads. |
| `workspace.*` | Workspace Service | Evidence/Context | snapshot/checkpoint changes. |
| `artifact.*` | Artifact Service | UI + Evidence | versioned artifact lifecycle. |
| `policy.*` | Airlock/Security | audit + operator | denial reasons are safe summaries. |
| `operator.*` | Operator API | audit only | no normal chat stream by default. |
| `source.*` | Source Intake | Evidence Ledger/outbox | acquisition, verification, license decision, promotion/quarantine. |
| `model.profile.*` | Model Gateway/Evaluation | Evidence Ledger/outbox | evaluation, canary, activation, rollback; no raw prompt/secret. |
| `work_*` completion/delivery | Runtime Work Store | Evidence Ledger/outbox | completion is immutable; delivery/import retry never re-executes. |

## 7. Enforcement tests

- Static architecture test blocks repository classes from importing another module's persistence namespace.
- Runtime integration test runs worker without SQL connection string.
- API test calls execution dispatch without approved spec and expects rejection.
- API test attempts cross-project read/write and expects authorization failure.
- Migration test verifies each mutable table has owner metadata, rowversion/etag, and expected indexes.
- Blob import test rejects mismatched hash, unknown schema version, and manifest written outside expected prefix.
- Event projection test ensures unknown events render safely and do not break the chat UI.
- Trace test ensures large payloads are stored in Blob refs, not SQL trace event rows.
- Crash-boundary tests prove lifecycle + `EvidenceLedgerEvent` + outbox atomicity and completion redelivery without re-execution.
- Source tests block promotion on missing immutable provenance or any unresolved/restrictive component-license decision.
- Model-profile tests block activation/fallback on exact-capability, schema, credential/retention, critical eval, canary, or rollback failure.
