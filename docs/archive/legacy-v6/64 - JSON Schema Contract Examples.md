---
title: "JSON Schema Contract Examples"
aliases:
  - "64 - JSON Schema Contract Examples"
tags:
  - bmad-runtime
  - vault/implementation-assets
section: "Implementation Assets"
order: 64
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: json-schema-reference
status: implementation-guide
---



# JSON Schema Contract Examples

## V6.17 canonical discriminators

All durable examples add `deliveryModel` and `authorityRef`. JSON/domain names are camelCase; SQL/SQLite use snake_case. Exact authoritative schemas and vectors are in [[99 - Dual-Delivery Contract and Conformance Specification]]. Minimum shapes:

```json
{
  "schemaVersion": "2026-07-10.v1",
  "objectType": "project",
  "objectId": "prj_01...",
  "deliveryModel": "windows_local",
  "authorityRef": { "kind": "desktop_local_store", "storeId": "store_01...", "deviceId": "dev_01..." },
  "ownerScopeRef": "owner_01...",
  "projectId": "prj_01...",
  "createdAt": "2026-07-10T12:00:00Z",
  "contentHash": "sha256:..."
}
```

`WorkspaceTarget`, `ExecutorAudience`, `ExecutionResultManifest`, and `SyncEnvelope` are closed discriminated unions. Unknown discriminator values fail validation. A Windows command candidate binds executable identity/hash, argv, cwd, environment names/source hashes, script hashes, limits, expected writes, containment profile, workspace root identity, base checkpoint, policy, and expiry.

## 1. Patch Proposal schema skeleton

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://sapphirus.local/schemas/patch-proposal.schema.json",
  "type": "object",
  "required": ["schemaVersion", "proposalType", "workspaceSnapshotId", "operations", "rationale"],
  "properties": {
    "schemaVersion": { "const": "2026-07-09.v1" },
    "proposalType": { "const": "patch" },
    "workspaceSnapshotId": { "type": "string" },
    "operations": {
      "type": "array",
      "minItems": 1,
      "items": {
        "type": "object",
        "required": ["op", "path", "preimageHash"],
        "properties": {
          "op": { "enum": ["create", "modify", "delete"] },
          "path": { "type": "string", "pattern": "^[^\\0]+$" },
          "preimageHash": { "type": ["string", "null"] },
          "unifiedDiff": { "type": "string" },
          "newContentRef": { "type": ["string", "null"] }
        },
        "additionalProperties": false
      }
    },
    "rationale": { "type": "string" },
    "expectedValidation": { "type": "array", "items": { "type": "string" } }
  },
  "additionalProperties": false
}
```

## 2. Command Proposal schema skeleton

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://sapphirus.local/schemas/command-proposal.schema.json",
  "type": "object",
  "required": ["schemaVersion", "proposalType", "command"],
  "properties": {
    "schemaVersion": { "const": "2026-07-09.v1" },
    "proposalType": { "const": "command" },
    "command": {
      "type": "object",
      "required": ["argv", "cwd", "networkMode", "timeoutSeconds"],
      "properties": {
        "argv": { "type": "array", "minItems": 1, "items": { "type": "string" } },
        "cwd": { "type": "string" },
        "networkMode": { "enum": ["off", "package_registry", "private_endpoints", "public_internet"] },
        "timeoutSeconds": { "type": "integer", "minimum": 1, "maximum": 3600 },
        "envAllowlist": { "type": "array", "items": { "type": "string" } },
        "expectedEffect": { "type": "string" }
      },
      "additionalProperties": false
    }
  },
  "additionalProperties": false
}
```

## 3. ApprovedExecutionSpec skeleton

```json
{
  "schemaVersion": "2026-07-09.v1",
  "specId": "spec_...",
  "proposalId": "prop_...",
  "approvalId": "appr_...",
  "policyVersion": "2026.07.09",
  "policyHash": "sha256:...",
  "proposalHash": "sha256:...",
  "specHash": "sha256:...",
  "projectId": "proj_...",
  "runId": "run_...",
  "workspaceSnapshotId": "snap_...",
  "checkpointBaseId": "chk_...",
  "preimageHashes": [],
  "effectClass": "patch_apply|command_run|artifact_export|package_import|rollback",
  "limits": {
    "timeoutSeconds": 600,
    "maxLogBytes": 10485760,
    "networkMode": "off"
  },
  "expiresAt": "2026-07-09T12:00:00Z"
}
```

## 4. Validation rules

- All model outputs validate before Orchestrator constructs platform proposals.
- All proposal hashes are computed from canonicalized JSON.
- Specs are immutable and stored by content hash.
- Unknown schema version fails unless migration exists.
