---
title: "SQL Migration and Index Plan"
aliases:
  - "65 - SQL Migration and Index Plan"
tags:
  - bmad-runtime
  - vault/implementation-assets
section: "Implementation Assets"
order: 65
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: sql-migration-plan
status: implementation-guide
---



# SQL Migration and Index Plan

## V6.17 migration families

This note's SQL migrations apply to Azure SQL for `web_managed` and bounded support-plane data. Add immutable `delivery_model`/authority constraints and remote-handoff indexes where applicable; never use the value to co-locate desktop-local lifecycle rows in Azure SQL.

Desktop uses a separate ordered SQLite migration set owned by the Rust app. It covers local projects/workspaces/grants, runs/proposals/candidates/decisions/approvals/specs/consumptions, executions/results, journals/checkpoints/rollback, evidence/outbox, CAS refs, packages/policies/model calls/egress, sync export/import receipts, and remote-handoff links. Migration/recovery details live in [[96 - Windows Local State, Evidence, Checkpoint, and Rollback]].

## 1. Migration principles

- Use forward-only migrations with rollback notes.
- Never drop or rewrite evidence-bearing data without retention decision.
- Add schema version columns before introducing new payload structures.
- Mutable lifecycle tables use rowversion/concurrency token.
- Large payloads stay in Blob; SQL stores refs/hashes.

## 2. Core indexes

| Table | Index | Reason |
|---|---|---|
| `projects` | `(tenant_id, created_at)` | project listing |
| `project_memberships` | `(user_id, project_id)` unique | RBAC checks |
| `threads` | `(project_id, updated_at desc)` | thread list |
| `messages` | `(thread_id, sequence)` unique | ordered chat |
| `runs` | `(project_id, state, updated_at)` | run dashboard |
| `evidence_ledger_events` | `(stream_id, sequence)` unique | authoritative event/evidence replay |
| `proposals` | `(run_id, state)` | active proposals |
| `execution_spec_candidates` | `(proposal_id, candidate_hash)` unique | exact policy/approval boundary |
| `approvals` | `(candidate_id, candidate_hash, state)` | exact candidate approval lookup |
| `approved_execution_specs` | `(executor_audience, single_use_nonce)` unique plus `(expires_at, consumed_at)` | dispatch/consumption validation |
| `executions` | `(run_id, state)` | execution tracking |
| `work_items` | `(owner_scope_id, idempotency_key)` unique | owner-scoped deduplication |
| `work_attempts` | `(work_item_id, attempt_number)` unique | immutable retries |
| `work_leases` | `(attempt_id)` unique plus `(state, expires_at)` | CAS/reclaim |
| `work_completions` | `(attempt_id, executor_audience, completion_nonce)` unique | redelivery without re-execution |
| `workspace_files` | `(snapshot_id, path_hash)` | file lookup |
| `preimage_hashes` | `(proposal_id, path_hash)` | drift verification |
| `checkpoints` | `(workspace_id, sequence)` | checkpoint chain |
| `trace_events` | `(run_id, sequence)` | diagnostic projection only |
| `outbox_messages` | `(state, available_at)` | durable publisher/retry |
| `source_snapshots` | `(archive_hash)` plus `(upstream_url, immutable_ref)` | provenance/deduplication |
| `component_license_decisions` | `(source_snapshot_id, component_path)` unique | promotion gate |
| `model_profiles` | `(role, version)` unique plus `(state, updated_at)` | promotion/rollback |
| `model_evaluation_bundles` | `(candidate_profile_id, bundle_hash)` unique | immutable eval evidence |

## 3. Migration phases

### Migration 001 — foundation

- users/groups/project memberships;
- owner scopes/principals and source snapshots/verifications/component-license decisions;
- projects/threads/messages/runs;
- evidence_ledger_events and outbox_messages;
- schema_versions table.

### Migration 002 — workspace

- workspaces;
- workspace_snapshots;
- workspace_files;
- workspace_locks;
- checkouts;
- checkpoints;
- rollback_records.

### Migration 003 — proposals and Airlock

- proposals;
- execution_spec_candidates;
- policy_versions;
- policy_decisions;
- approvals;
- approval_grants;
- approved_execution_specs.

### Migration 004 — execution

- worker_images and fixed_job_templates;
- executions;
- work_items/work_attempts/work_leases/work_completions;
- execution_manifest_index;
- execution_log_index.

### Migration 005 — BMAD/artifacts/evidence

- bmad_packages;
- bmad_modules;
- bmad_capabilities;
- artifacts;
- artifact_versions;
- artifact_exports;
- model_profiles/model_evaluation_bundles/provider capability/schema-projection records;
- trace_events (projection);
- evidence_bundles.

## 4. Data backfill policy

- Backfills are idempotent.
- Backfills emit audit logs if touching evidence-bearing records.
- Backfills preserve original hashes and payload refs.
