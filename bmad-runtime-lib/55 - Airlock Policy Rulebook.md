---
title: "Airlock Policy Rulebook"
aliases:
  - "55 - Airlock Policy Rulebook"
tags:
  - bmad-runtime
  - vault/implementation-assets
section: "Implementation Assets"
order: 55
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: policy-rulebook
status: implementation-guide
---



# Airlock Policy Rulebook

## 0. V6.17 candidate and audience requirements

Every decision input/output adds `deliveryModel`, `authorityRef`, a discriminated `workspaceTarget`, exact `executorAudience`, and evidence-calibrated containment claims. Web audiences name a fixed Azure job template/image/network profile. Desktop audiences bind the device/install/host binary, workspace grant/root identity, app-file-API enforcement, process-tree control, filesystem enforcement tier, and network enforcement tier.

Shared rule IDs produce semantically equivalent allow/deny/risk results, but the .NET and Rust issuers mint non-interchangeable specs. Desktop command policy also binds resolved executable path/file identity/hash/signature, `argv[]`, cwd, environment variable names plus value-source hashes, script operand hashes, expected writes, limits, and rollback class. A declaration is not enforcement: `network: declared_only` and `filesystem: not_enforced` must be shown in risk/approval UX.

Spec use writes a separate `SpecConsumptionRecord`; it never mutates the signed/hashed spec. Remote results require a fresh local candidate and decision.

## 1. Policy decision output

```json
{
  "decisionId": "pd_...",
  "proposalId": "prop_...",
  "executionSpecCandidateId": "candidate_...",
  "executionSpecCandidateHash": "sha256:...",
  "policyVersion": "2026.07.09",
  "policyHash": "sha256:...",
  "decision": "allow_with_approval|allow_with_grant|deny|needs_operator",
  "risk": "low|medium|high|critical",
  "reasons": [],
  "requiredApproval": {},
  "approvedAudience": {
    "kind": "azure_job_template",
    "jobTemplateId": "patch-v1"
  }
}
```

## 2. Rule classes

| Rule class | Examples | Default |
|---|---|---|
| Path policy | protected dirs, generated dirs, secrets, symlinks | deny ambiguous/protected |
| Command policy | command class, argv, cwd, shell use | deny unknown shell |
| Network policy | off, package registry, public internet, private endpoint | off for tests |
| Dependency policy | package manager install/restore | approval required |
| Artifact export policy | internal Blob, download, external destination | internal allowed, external denied in v1 |
| Package import/activation policy | Already-intaken BMAD archive, setup scripts, templates, exact digest/lock, component licenses | static validation then Azure-isolated rehearsal before activation |
| Resource policy | CPU/memory/time/log limits | bounded |
| Identity policy | user role, project membership, operator permission | least privilege |

## 3. Deny examples

- `rm -rf`, `curl | sh`, unrestricted `bash -c`, path traversal, writing `.env`, modifying `.git`, pushing to remote, unknown executable, unbounded network, missing preimage, expired approval, mismatched spec hash, worker image not approved.

## 4. Candidate-bound approval and grant model

The executable flow is always:

`Proposal -> ExecutionSpecCandidate -> policy decision -> exact candidate-hash approval when required -> audience-bound, expiring, single-use ApprovedExecutionSpec`.

An `ApprovalGrant` is not executable. It may only make a future fresh candidate eligible for `allow_with_grant` after full policy/mutable-input revalidation. Airlock still mints a new single-use spec for that exact candidate.

Reusable policy grant is considered only when all of these match:

- project ID;
- run ID or explicit run scope;
- command class;
- exact `argv[]` or approved pattern;
- canonical cwd;
- network mode;
- timeout/resource limits;
- worker image digest;
- policy hash;
- expiration;
- approving actor.
- action/schema/candidate class and all mutable-input constraints;
- provider/credential/residency/retention boundary where applicable.

Ordinary authenticated application CRUD uses owner scope, authz, validation, idempotency, audit, and domain transactions without an execution spec. Offline Source Intake/build approval uses repository/CI provenance and component-license gates. Neither path may dispatch a worker or perform a governed external/workspace mutation.

## 5. Required tests

- Every governed mutation/worker-dispatch endpoint rejects requests without a valid audience-bound, unconsumed `ApprovedExecutionSpec`; ordinary CRUD endpoints reject attempts to smuggle executable fields.
- Policy denies unknown command class.
- Policy denies shell execution unless operator-approved.
- Policy denies writes to secret/protected paths.
- Policy denies spec with mismatched policy hash/proposal hash/preimage hash.
- Policy grant cannot be reused across project/run/image/network mode.
- Approval of candidate hash A cannot mint a spec for candidate hash B.
- Spec reuse, expiry, audience mismatch, policy drift, preimage/mutable-input drift, and fixed-job-template mismatch fail closed.
