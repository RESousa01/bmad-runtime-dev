---
title: "Worker Manifest Protocol"
aliases:
  - "56 - Worker Manifest Protocol"
tags:
  - bmad-runtime
  - vault/implementation-assets
section: "Implementation Assets"
order: 56
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: worker-protocol
status: implementation-guide
---



# Worker Manifest Protocol

## V6.17 scope and result union

This note specifies `WebWorkerResultManifest` for `web_managed` and explicitly requested remote jobs. The cross-delivery contract is now `ExecutionResultManifest = WebWorkerResultManifest | WindowsLocalExecutionResultManifest`, discriminated by `deliveryModel` and `kind`.

The Windows-local variant is produced/imported entirely by the signed Rust authority and binds host build/binary hash, workspace capability/root identity, base/final checkpoint, journal ID, executable/script identity for commands, changed-file pre/post hashes, output/redaction refs, rollback readiness, and recovery disposition. It contains no Blob or ACA requirement. Exact union fields and validation are in [[99 - Dual-Delivery Contract and Conformance Specification]].

## 1. Worker input

Workers receive:

- `owner_scope_ref`, run/work-item/attempt/lease ids, and executor audience;
- `execution_id`;
- immutable `ApprovedExecutionSpec` Blob ref;
- expected spec hash;
- workspace checkout ref;
- output Blob prefixes;
- allowed environment variables;
- resource/time/log limits;
- correlation ID.
- fixed job-template id plus the expected IaC-bound image digest, entrypoint, identity/network class, and completion nonce.

Workers do not receive:

- SQL write connection strings;
- broad Key Vault access;
- unrestricted project credentials;
- raw user secrets unless a policy-approved scoped credential explicitly permits them.

## 2. Canonical `WebWorkerResultManifest`

```json
{
  "schemaVersion": "2026-07-09.v1",
  "ownerScopeRef": "ownerscope_...",
  "runId": "run_...",
  "workItemId": "work_...",
  "attemptId": "attempt_...",
  "leaseId": "lease_...",
  "executionId": "exec_...",
  "executionSpecCandidateHash": "sha256:...",
  "specId": "spec_...",
  "specHash": "sha256:...",
  "policyHash": "sha256:...",
  "approvalId": "approval_...",
  "executorAudience": "aca-job-template:patch-v1",
  "jobTemplateId": "patch-v1",
  "workerImageDigest": "sha256:...",
  "workspaceSnapshotHash": "sha256:...",
  "mutableInputHashes": [],
  "commandSpecHash": "sha256:...",
  "completionNonce": "nonce_...",
  "startedAt": "...",
  "completedAt": "...",
  "status": "succeeded|failed|cancelled|timed_out",
  "exitCode": 0,
  "changedFiles": [],
  "outputArtifacts": [],
  "logRefs": [],
  "redactionReportRef": "blob://...",
  "failureClassification": null,
  "checkpointCandidate": {},
  "manifestHash": "sha256:..."
}
```

## 3. Manifest import rules

- Runtime API validates schema version.
- Runtime API verifies manifest hash and spec hash.
- Runtime API verifies owner/run/work-item/attempt/lease, candidate, policy, approval, audience, single-use nonce, fixed template/image, workspace snapshot, mutable inputs, command, and output hashes.
- Runtime API verifies execution is in an importable state.
- Runtime API verifies changed files are within spec scope.
- Runtime API atomically records `WorkCompletion`, lifecycle transition, `EvidenceLedgerEvent`, and `OutboxMessage`; compact state stays in SQL and large payloads stay in Blob.
- Duplicate manifest import is idempotent.
- A conflicting terminal manifest is quarantined/visible and cannot overwrite the accepted outcome.
- A valid worker claim does not define success by itself; only Runtime validation/import can advance authoritative state.

## 4. Worker steps

1. Download spec by immutable ref.
2. Verify hash.
3. Materialize checkout.
4. Validate path/cwd/symlink constraints again defensively.
5. Execute approved action.
6. Redact logs.
7. Compute changed-file manifest.
8. Upload logs/artifacts/result manifest.
9. Exit with status matching manifest.

## 5. Cloud-first and recovery gates

- The image is remotely built by ACR Tasks/hosted CI with lock, component-license, scan, SBOM, provenance/attestation, signature, and digest evidence; no local Docker build is required.
- The dispatcher may start an allowlisted fixed ACA Job template but cannot override image, entrypoint, identity, secret refs, network profile, or arbitrary environment at request time.
- The worker has no lifecycle SQL credential and writes only bounded output locations assigned to its owner/attempt.
- A crash after completion upload but before API acknowledgement redelivers/imports the same completion nonce. It cannot rerun under a consumed spec.
- Fake and ACA lanes share the schema, but fake manifests are explicitly marked simulated/non-isolating and cannot be used as internal-alpha real-execution evidence.
