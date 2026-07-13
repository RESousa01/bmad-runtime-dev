---
title: "Windows Local State, Evidence, Checkpoint, and Rollback"
aliases:
  - "Desktop Local State"
  - "Desktop Evidence and Rollback"
tags:
  - bmad-runtime
  - windows-desktop
  - sqlite
  - evidence
  - rollback
section: "Windows Desktop Runtime"
order: 96
vault_role: "desktop-local-state-authority"
project: Sapphirus BMAD Runtime
status: current
updated_on: 2026-07-10
---

# Windows Local State, Evidence, Checkpoint, and Rollback

## 1. Authority rule

For `windows_local`, the signed Rust Local Runtime and its local store own lifecycle state, Airlock records, specs, executions, checkpoints, rollback plans, and the durable Evidence Ledger.

Azure sync, telemetry, model-call records, package catalogs, review feedback, and remote-job evidence are inputs or replicas. They cannot transition local state, consume a local spec, or mutate a selected workspace.

## 2. Local data layout

```text
%LOCALAPPDATA%/Sapphirus/
  state/
    runtime.sqlite3
    runtime.sqlite3-wal
    runtime.sqlite3-shm
  objects/
    sha256/aa/bb/<digest>.enc
  journals/
    <journal-id>.guard
  packages/
    <package-id>/<version>/...
  logs/
    redacted operational logs
  updates/
    staged signed artifacts
```

The selected workspace is not used for hidden application metadata by default. An opt-in `.sapphirus/` project file requires a separate ADR and must never contain tokens, keys, raw prompts, or authority state.

## 3. Storage split

| Store | Content |
|---|---|
| SQLite | Compact metadata, state, indexes, locks, ledger, outbox, refs |
| Encrypted content-addressed store (CAS) | Prompts, outputs, diffs, logs, checkpoint bytes, manifests, evidence payloads |
| DPAPI-protected key record | Local CAS encryption master key and selected credential material |
| Workspace | User project files only; never authority database |

SQLite runs with foreign keys, WAL, integrity checks, explicit busy timeout, monotonic schema versions, and one Rust-owned write coordinator. Renderer and child processes never open it.

## 4. Recommended table ownership

| Tables | Owner |
|---|---|
| `store_metadata`, `schema_migrations`, `key_records` | Local Store |
| `projects`, `threads`, `messages`, `runs` | Local Runtime |
| `proposals`, `execution_spec_candidates` | Local Orchestrator |
| `airlock_decisions`, `approvals`, `approved_specs`, `spec_consumptions` | Local Airlock |
| `workspace_capabilities`, `filesystem_capability_snapshots`, `workspace_manifests` | Workspace Broker |
| `local_effect_journals`, `local_effect_operations`, `executions`, `local_result_manifests` | Execution Engine |
| `checkpoints`, `checkpoint_entries`, `rollback_plans` | Checkpoint Engine |
| `evidence_ledger_events`, `evidence_materializations`, `outbox_messages` | Evidence Module |
| `model_calls`, `context_egress_records` | Desktop Cloud Client |
| `sync_outbox`, `sync_receipts`, `replica_links`, `remote_job_handoffs` | Sync/Handoff Module |
| `package_cache_index`, `package_activations` | BMAD Package Runtime |
| `device_registration`, `entitlement_leases` | Identity/Licensing Client |

## 5. SQLite starter constraints

Every authoritative table includes:

- stable string ID;
- `delivery_model` constrained to `windows_local` where applicable;
- `authority_id`, `installation_id`, and `authority_epoch` where the object is portable or synced;
- owner/project scope;
- `schema_version`;
- optimistic `version` for mutable lifecycle rows;
- created/updated/terminal timestamps;
- classification and retention class;
- content/ref hash.

Critical uniqueness:

- run idempotency key within project;
- candidate hash within run;
- one approval decision per decision ID;
- one spec consumption per spec nonce/audience;
- execution result unique by execution/attempt/consumption nonce;
- ledger stream sequence and event hash;
- outbox message ID;
- checkpoint sequence within workspace;
- sync source authority/sequence.

## 6. Encryption and keys

Baseline design:

1. Generate a random per-store content-encryption master key.
2. Protect it with user-scoped Windows DPAPI; do not use machine-wide protection for user data.
3. Encrypt each CAS object with authenticated encryption, a unique nonce, object type/schema/hash as associated data, and algorithm/key version metadata.
4. Store only minimal non-sensitive indexes in plaintext SQLite; encrypt path, prompt, diff, log, checkpoint, and evidence payloads.
5. Scrub keys/plaintext from logs, crash dumps, telemetry, swap-sensitive buffers where practical, and renderer memory.

DPAPI protects data at rest against casual offline access. It does not protect data from the signed-in user, privileged local malware, a compromised live process, or an administrator who can instrument the process. Local evidence is tamper-evident by hash chain, not tamper-proof.

Unresolved DESK-04 chooses between minimal SQLite metadata plus encrypted CAS, SQLCipher, or another reviewed implementation and defines backup portability, rotation, and recovery.

## 7. Transaction invariant

Each accepted local domain transition commits in one SQLite transaction:

```text
guard current state + optimistic version
-> write new state/immutable record
-> append EvidenceLedgerEvent
-> append local OutboxMessage
-> commit
```

Filesystem effects cannot be in the same atomic transaction. They use the durable `LocalEffectJournal` described in [[95 - Windows Local Workspace and Execution]]. The local state machine must expose `applying`, `recovering`, and `manual_review` rather than pretending the file batch and SQLite commit are atomic.

## 8. Local evidence ledger

```json
{
  "schemaVersion":"evidence-ledger-event.v2",
  "deliveryModel":"windows_local",
  "authorityRef":{
    "authorityKind":"desktop_local_store",
    "authorityId":"authority_...",
    "installationId":"install_...",
    "localStoreId":"store_...",
    "authorityEpoch":1
  },
  "streamId":"run_...",
  "sequence":42,
  "eventType":"local_execution.completed",
  "actorKind":"desktop_host",
  "payloadHash":"sha256:...",
  "previousEventHash":"sha256:...",
  "eventHash":"sha256:...",
  "createdAt":"..."
}
```

The chain detects missing/reordered/modified local records. It does not establish non-repudiation against a user controlling the device. Optional signed or cloud-anchored ledger checkpoints require an ADR if customers need higher assurance.

Operational telemetry can be deleted, sampled, or disabled without changing ledger truth.

## 9. Checkpoint contract

```json
{
  "schemaVersion":"local-checkpoint.v1",
  "checkpointId":"lcp_...",
  "deliveryModel":"windows_local",
  "workspaceCapabilityId":"lwc_...",
  "grantEpoch":4,
  "rootIdentityHash":"sha256:...",
  "sequence":12,
  "kind":"pre_effect|post_effect|user_kept_partial|rollback_result",
  "baseCheckpointId":"lcp_...",
  "entries":[
    {
      "path":"src/App.tsx",
      "exists":true,
      "fileIdHash":"sha256:...",
      "metadataHash":"sha256:...",
      "contentRef":"cas://sha256/...",
      "contentHash":"sha256:..."
    }
  ],
  "manifestHash":"sha256:...",
  "createdAt":"..."
}
```

Before a governed file mutation, checkpoint payloads are encrypted and durably flushed before the effect journal enters `applying`.

Checkpoint coverage is explicit. A command that may modify unknown files is either preceded by a policy-approved broader checkpoint strategy or labelled partially/non-reversible.

## 10. Journal state machine

```text
prepared
-> checkpoint_durable
-> preconditions_verified
-> applying
-> effects_applied
-> postimages_verified
-> result_recorded
-> completed
```

Recovery branches:

```text
applying|effects_applied|postimages_verified
-> recovery_required
-> restoring|completing|manual_review
-> recovered|completed|abandoned_with_evidence
```

Each file operation has its own ordinal and state. Startup scans nonterminal journals before allowing another writer for the workspace.

## 11. Rollback

Rollback is a new governed effect:

```text
rollback_requested
-> rollback_candidate_created
-> policy_evaluated
-> awaiting_approval when required
-> local_spec_issued
-> journaled_restore
-> rollback_checkpoint_recorded
-> rollback_completed
```

Rules:

- historical approval/spec is evidence, not reusable authority;
- current root/grant/base checkpoint and target identities are revalidated;
- external edits since the target checkpoint produce a conflict preview;
- overwrite of newer user work requires explicit decision or a new destination;
- rollback covers checkpointed filesystem state only;
- network, registry, package publication, remote database, and unknown command effects are non-reversible unless a tested adapter supplies compensation.

## 12. Crash recovery

Startup recovery checks:

- SQLite integrity and migration state;
- key unwrap and CAS authentication;
- incomplete effects/journals;
- stale workspace writer leases;
- result recorded without projection/outbox delivery;
- orphaned temporary/replacement files owned by Sapphirus;
- mismatched checkpoint/CAS refs;
- sync records awaiting acknowledgement.

The repair tool never deletes unknown user files. It previews owned cleanup, reconstructs indexes from immutable records where possible, and exports a support bundle before destructive repair.

## 13. Retention, deletion, uninstall, and backup

Retention classes are separate for messages/model context, logs, checkpoints, evidence, package cache, telemetry, and sync replicas.

- Deleting a project does not delete workspace source unless the user explicitly selects and confirms those exact files.
- Uninstall offers keep/remove app-local state; it never deletes selected workspaces.
- Cache cleanup verifies CAS reachability and never uses age alone for checkpoint/evidence still referenced by a run.
- Cloud replica deletion does not delete local authoritative evidence or workspace files.
- Backup exports are encrypted, versioned, integrity-checked, and disclose whether they are bound to the current Windows user/device.
- Restore runs into a new authority epoch and never silently merges approvals/specs/executions.

## 14. Sync replica rules

The local store remains authoritative when sync is enabled:

- source files are never automatic sync items;
- evidence events are append-only and deduplicated by authority/sequence/hash;
- cloud acknowledgement means replica received, not local state applied;
- review feedback is imported as new input, not a mutation of a decision;
- no last-writer-wins merge for approvals, specs, executions, checkpoints, or policy;
- replica tombstones never delete workspace files;
- conflict state is user-visible and evidence-backed.

## 15. Migration policy

Local migrations are forward-only within an installed release, transactional where SQLite permits, and paired with application rollback constraints.

Every release records:

- minimum/maximum readable schema;
- whether downgrade is safe;
- pre-migration backup/checkpoint behavior;
- CAS/key algorithm compatibility;
- rollback installer compatibility;
- repair/upcaster fixtures.

An app binary must refuse to open a newer unsupported store rather than attempt a lossy downgrade.

## 16. Tests and release gates

- crash injection at every SQLite/domain/journal/file boundary;
- concurrent command, watcher, sync, and external-editor races;
- WAL recovery, disk full, corruption, busy/lock timeout, schema failure;
- DPAPI wrong user/device, key loss, rotation interruption, authenticated-encryption tamper;
- checkpoint completeness and large/binary/metadata edge cases;
- rollback conflict with newer external edits;
- evidence chain gap/reorder/tamper detection and materialization after telemetry deletion;
- sync duplicate/out-of-order/conflicting/tombstone behavior;
- uninstall/repair/backup/restore on clean supported Windows;
- child/renderer cannot open the authority store or decrypt CAS.

Release requires a demonstrated recovery path for every nonterminal journal state and a documented outcome for unrecoverable key/store corruption.

## 17. Unresolved decisions

- **DESK-04:** encryption/database implementation, key rotation, backup portability, and recovery UX.
- Whether optional TPM-backed installation signing/attestation is required.
- Whether local evidence needs cloud anchoring or device signatures for enterprise assurance.
- Checkpoint size/retention policy and behavior for commands with unknown writes.
- Whether opt-in repository metadata files are ever allowed.

## 18. Primary references

- [Windows CryptProtectData](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata)
- [CNG DPAPI](https://learn.microsoft.com/en-us/windows/win32/seccng/cng-dpapi)

