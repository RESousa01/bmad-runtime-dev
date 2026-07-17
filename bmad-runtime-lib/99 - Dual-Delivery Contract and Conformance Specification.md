---
title: "Dual-Delivery Contract and Conformance Specification"
aliases:
  - "Dual-Delivery Contracts"
  - "Cross-Runtime Conformance"
tags:
  - bmad-runtime
  - architecture-contracts
  - web-managed
  - windows-desktop
  - conformance
section: "Architecture Contracts"
order: 99
vault_role: "dual-delivery-contract-authority"
project: Sapphirus BMAD Runtime
status: legacy-reference
updated_on: 2026-07-10
---

# Dual-Delivery Contract and Conformance Specification

## 1. Scope, precedence, and normative language

This document is the normative wire-contract and conformance authority shared by:

- the .NET/C# Web Runtime for `web_managed` projects;
- the Rust/Tauri Local Runtime for `windows_local` projects; and
- the React/TypeScript presentation layers that render projections and submit typed commands.

It standardizes semantic names, discriminators, hashes, identifiers, evidence links, package compatibility, and compatibility tests. It does **not** merge runtime authority, execution, persistence, approvals, or workspace access.

The delivery plans and desktop implementation boundaries remain authoritative in [[93 - Split Web and Windows Desktop Architecture Plans]], [[94 - Windows Desktop Native Host and IPC]], [[95 - Windows Local Workspace and Execution]], and [[96 - Windows Local State, Evidence, Checkpoint, and Rollback]]. Canonical domain names, event names, and state names remain owned by [[34 - Canonical Object Model]], [[53 - Event Taxonomy and Stream Protocol]], and [[54 - State Machine Reference]]. Where older cloud-only examples conflict with the discriminated contracts here, this document controls the cross-delivery wire shape.

The terms **MUST**, **MUST NOT**, **REQUIRED**, **SHOULD**, and **MAY** are normative.

### 1.1 Non-negotiable separation

| Concern | `web_managed` | `windows_local` |
|---|---|---|
| Lifecycle authority | .NET Runtime API and Azure control-plane store | Signed Rust Local Runtime and local SQLite/CAS store |
| Workspace authority | Immutable cloud snapshot and job checkout | Revocable user-selected local folder capability |
| Execution authority | Approved fixed Azure job audience | Approved signed local-host audience |
| Evidence authority | Azure SQL/Blob ledger and payloads | Local SQLite/encrypted-CAS ledger and payloads |
| Cloud sync role | Source authority or collaboration service | Replica/support plane only |
| Ordinary file edit | Cloud checkout mutation | Local brokered and journaled mutation |

The following are invalid by construction:

- a `web_managed` object with a desktop authority, local folder target, or Windows local-host audience;
- a `windows_local` object with an Azure lifecycle authority, cloud-snapshot write target, or Azure job audience for an ordinary local effect;
- changing a `Project.deliveryModel` or `Run.deliveryModel` after creation;
- consuming an approved spec under an authority, workspace target, audience, or delivery model different from the one hashed into that spec;
- treating a synced replica, cloud review, telemetry item, or remote result as local lifecycle or file-write authority; and
- using one structure with nullable Azure and Windows fields whose irrelevant fields are ignored.

A cross-product transfer creates linked but separate objects under their respective authorities. It never changes an existing object's delivery discriminator.

## 2. Contract form and primitive vocabulary

The source format is JSON Schema 2020-12. The TypeScript notation below is a language-neutral, normative shorthand for the generated wire shapes. JSON names are `camelCase`; SQL and SQLite projections use `snake_case`; C# and Rust may use their normal identifier conventions while preserving the serialized JSON names exactly.

Illustrative JSON uses repeated numeric hash values and example signature bytes to keep bindings readable. Those values are syntactically valid placeholders, not claims that the surrounding example was hashed or signed. Only the vectors in section 14.2 assert exact computed hashes.

```ts
type DeliveryModel = "web_managed" | "windows_local";

type Sha256 = `sha256:${Lowercase<string>}`;
type Base64Url = string;
type UtcInstant = string;          // canonical form: YYYY-MM-DDTHH:mm:ss.sssZ
type ContractId = string;          // prefix + "_" + 16-64 Crockford Base32 characters
type OpaqueRef = string;           // scheme is constrained by the owning schema
type RelativeWorkspacePath = string;

type DeliveryBound<TWeb, TDesktop> =
  | ({ deliveryModel: "web_managed" } & TWeb)
  | ({ deliveryModel: "windows_local" } & TDesktop);
```

`DeliveryModel` has exactly two values in contract epoch 1. A third model is a breaking contract-epoch change, not an enum addition inside v1.

### 2.1 Delivery binding

1. `Project.deliveryModel` is REQUIRED and immutable at project creation.
2. Every `Run`, proposal, candidate, approval, spec, execution, result, checkpoint, and evidence stream inherits the project value.
3. A child object's authority and target MUST match its delivery model.
4. A run cannot fall back from local to remote or remote to local. Explicit remote work uses `RemoteJobHandoff` and a separate `web_managed` cloud record.
5. An imported replica retains the source `deliveryModel` and `AuthorityRef`; the receiver does not relabel it.

### 2.2 Required schema posture

- Security- or authority-bearing schemas use `additionalProperties: false` at every object level.
- Arrays are ordered unless the field explicitly defines set canonicalization.
- An absent optional property and a property set to `null` are different. Schemas MUST choose one representation.
- Integers MUST remain within the interoperable JSON integer range `[-9007199254740991, 9007199254740991]`.
- Non-finite numbers, duplicate JSON member names, invalid Unicode, and unpaired surrogates are rejected before domain parsing.
- A renderer DTO is never accepted as an authority object without native/server validation.

## 3. Authority and durable-object envelope

### 3.1 `AuthorityRef`

```ts
type AuthorityRef = AzureControlPlaneAuthorityRef | DesktopLocalStoreAuthorityRef;

interface AzureControlPlaneAuthorityRef {
  authorityKind: "azure_control_plane";
  authorityId: ContractId;
  tenantId: ContractId;
  controlPlaneInstanceId: ContractId;
  authorityEpoch: number;          // integer >= 1
  region: string;
}

interface DesktopLocalStoreAuthorityRef {
  authorityKind: "desktop_local_store";
  authorityId: ContractId;
  installationId: ContractId;
  localStoreId: ContractId;
  authorityEpoch: number;          // integer >= 1
}
```

Binding rule:

```text
web_managed   -> authorityKind MUST equal azure_control_plane
windows_local -> authorityKind MUST equal desktop_local_store
```

`authorityEpoch` changes only when authority continuity is deliberately broken, such as restoring a desktop backup into a new authority instance. It is not a row version. Authority identity and epoch are immutable on an existing durable object.

Example:

```json
{
  "authorityKind": "desktop_local_store",
  "authorityId": "authority_01J0000000000000000000000",
  "installationId": "install_01J00000000000000000000000",
  "localStoreId": "store_01J000000000000000000000000",
  "authorityEpoch": 1
}
```

### 3.2 `DurableObjectEnvelope`

```ts
interface DurableObjectEnvelope {
  schemaVersion: string;
  objectType: string;
  objectId: ContractId;
  deliveryModel: DeliveryModel;
  authorityRef: AuthorityRef;
  ownerScopeRef: ContractId;
  projectId: ContractId;
  runId?: ContractId;
  createdAt: UtcInstant;
  contentHash: Sha256;
}

interface DurableObject<TPayload> {
  envelope: DurableObjectEnvelope;
  payload: TPayload;
}
```

`contentHash` is the purpose-separated hash of `payload`, not a hash of the envelope and not a storage ETag. An object's immutable identity is `(authorityRef, objectType, objectId, contentHash)`. Mutable lifecycle projections use a separate optimistic `version`; they do not overwrite immutable evidence payloads.

Example:

```json
{
  "envelope": {
    "schemaVersion": "sapphirus.durable-object.v1",
    "objectType": "execution_spec_candidate",
    "objectId": "candidate_01J000000000000000000000",
    "deliveryModel": "windows_local",
    "authorityRef": {
      "authorityKind": "desktop_local_store",
      "authorityId": "authority_01J0000000000000000000000",
      "installationId": "install_01J00000000000000000000000",
      "localStoreId": "store_01J000000000000000000000000",
      "authorityEpoch": 1
    },
    "ownerScopeRef": "ownerscope_01J00000000000000000000",
    "projectId": "project_01J0000000000000000000000",
    "runId": "run_01J00000000000000000000000",
    "createdAt": "2026-07-10T10:00:00.000Z",
    "contentHash": "sha256:1111111111111111111111111111111111111111111111111111111111111111"
  },
  "payload": {
    "candidateId": "candidate_01J000000000000000000000"
  }
}
```

The abbreviated example demonstrates envelope shape only; production fixtures use a hash that matches the complete payload.

## 4. Workspace targets and measured filesystem capability

### 4.1 `WorkspaceTarget`

```ts
type WorkspaceTarget = CloudSnapshotTarget | LocalFolderCapabilityTarget;

interface CloudSnapshotTarget {
  targetKind: "cloud_snapshot";
  workspaceId: ContractId;
  snapshotId: ContractId;
  snapshotHash: Sha256;
  snapshotObjectRef: OpaqueRef;
  baseCheckpointId: ContractId | null;
  checkoutPolicyHash: Sha256;
}

interface LocalFolderCapabilityTarget {
  targetKind: "local_folder_capability";
  workspaceCapabilityId: ContractId;
  grantEpoch: number;              // integer >= 1
  rootIdentityHash: Sha256;
  filesystemCapabilityHash: Sha256;
  baseCheckpointId: ContractId;
  workspaceManifestHash: Sha256;
}
```

Binding rule:

```text
web_managed   -> targetKind MUST equal cloud_snapshot
windows_local -> targetKind MUST equal local_folder_capability
```

`snapshotObjectRef` is an opaque Azure-side object reference. A local target never contains an absolute path, drive letter, OS handle, or cloud address. The absolute selected root is encrypted local-only material resolved from `workspaceCapabilityId` by the Rust Workspace Broker.

Examples:

```json
{
  "targetKind": "cloud_snapshot",
  "workspaceId": "workspace_01J00000000000000000000",
  "snapshotId": "snapshot_01J000000000000000000000",
  "snapshotHash": "sha256:2222222222222222222222222222222222222222222222222222222222222222",
  "snapshotObjectRef": "azure-blob://workspace-snapshots/sha256/22/22",
  "baseCheckpointId": null,
  "checkoutPolicyHash": "sha256:3333333333333333333333333333333333333333333333333333333333333333"
}
```

```json
{
  "targetKind": "local_folder_capability",
  "workspaceCapabilityId": "lwc_01J00000000000000000000000",
  "grantEpoch": 4,
  "rootIdentityHash": "sha256:4444444444444444444444444444444444444444444444444444444444444444",
  "filesystemCapabilityHash": "sha256:5555555555555555555555555555555555555555555555555555555555555555",
  "baseCheckpointId": "lcp_01J00000000000000000000000",
  "workspaceManifestHash": "sha256:6666666666666666666666666666666666666666666666666666666666666666"
}
```

### 4.2 `FilesystemCapabilitySnapshot`

This is measured evidence, not a claim inferred from a path string.

```ts
type CapabilityStatus = "verified" | "unsupported" | "unknown";

interface FilesystemCapabilitySnapshot {
  schemaVersion: "sapphirus.filesystem-capability.v1";
  snapshotId: ContractId;
  workspaceCapabilityId: ContractId;
  grantEpoch: number;
  capturedAt: UtcInstant;
  filesystemKind: "ntfs" | "refs" | "fat" | "exfat" | "other";
  locationClass: "fixed_local" | "removable" | "unc" | "virtual" | "cloud_backed" | "unknown";
  volumeIdentityHash: Sha256;
  rootFileIdentityHash: Sha256;
  caseSensitiveDirectory: boolean;
  cloudPlaceholderState: "none" | "fully_hydrated" | "contains_placeholders" | "unknown";
  capabilities: {
    stableFileIds: CapabilityStatus;
    reparseInspection: CapabilityStatus;
    hardlinkCount: CapabilityStatus;
    perFileAtomicReplace: CapabilityStatus;
    durableFileFlush: CapabilityStatus;
    durableDirectoryFlush: CapabilityStatus;
  };
  supportTier: "supported_writable" | "read_only_evaluation" | "blocked";
  policyVersion: string;
  policyHash: Sha256;
  snapshotHash: Sha256;
}
```

```json
{
  "schemaVersion": "sapphirus.filesystem-capability.v1",
  "snapshotId": "fsc_01J00000000000000000000000",
  "workspaceCapabilityId": "lwc_01J00000000000000000000000",
  "grantEpoch": 4,
  "capturedAt": "2026-07-10T10:01:00.000Z",
  "filesystemKind": "ntfs",
  "locationClass": "fixed_local",
  "volumeIdentityHash": "sha256:7777777777777777777777777777777777777777777777777777777777777777",
  "rootFileIdentityHash": "sha256:8888888888888888888888888888888888888888888888888888888888888888",
  "caseSensitiveDirectory": false,
  "cloudPlaceholderState": "none",
  "capabilities": {
    "stableFileIds": "verified",
    "reparseInspection": "verified",
    "hardlinkCount": "verified",
    "perFileAtomicReplace": "verified",
    "durableFileFlush": "verified",
    "durableDirectoryFlush": "unknown"
  },
  "supportTier": "supported_writable",
  "policyVersion": "desktop-filesystem.2026-07-10",
  "policyHash": "sha256:9999999999999999999999999999999999999999999999999999999999999999",
  "snapshotHash": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
}
```

The snapshot is remeasured before a governed mutation. A changed grant epoch, root identity, filesystem capability hash, or support tier voids the candidate and any unconsumed spec.

## 5. Executor audiences and containment claims

### 5.1 Exact audience union

```ts
type ExecutorAudience = AzureJobAudience | WindowsLocalHostAudience;

interface AzureJobAudience {
  audienceKind: "azure_job_template";
  jobTemplateId: string;
  workerImageDigest: Sha256;
  azureRegion: string;
  workloadIdentityRef: ContractId;
  networkProfileHash: Sha256;
  sandboxProfileHash: Sha256;
  isolation: {
    isolationKind: "managed_job_sandbox";
    filesystemScope: "ephemeral_checkout_only";
    processScope: "job_instance";
    networkEnforcement: "azure_network_policy";
  };
}

interface WindowsLocalHostAudience {
  audienceKind: "windows_local_host";
  installationId: ContractId;
  hostBuildId: string;
  hostBinarySha256: Sha256;
  runnerProfile: "standard_user_job" | "restricted_token_job" | "appcontainer_brokered";
  containment: WindowsContainmentClaim;
}

interface WindowsContainmentClaim {
  appFileApi: "selected_root_enforced";
  processTree: "job_object_controlled";
  childProcessIdentity: "signed_in_user" | "restricted_token" | "appcontainer";
  childFilesystemEnforcement: "not_enforced" | "restricted_token_verified" | "appcontainer_verified";
  childNetworkEnforcement: "declared_only" | "firewall_enforced";
  measuredProfileHash: Sha256;
}
```

Binding rule:

```text
web_managed   -> audienceKind MUST equal azure_job_template
windows_local -> audienceKind MUST equal windows_local_host
```

An explicit remote job requested from desktop does not put an Azure audience into a local spec. The local spec authorizes the upload/handoff effect. The separate `web_managed` cloud run receives its own Azure candidate and spec.

Example desktop audience:

```json
{
  "audienceKind": "windows_local_host",
  "installationId": "install_01J00000000000000000000000",
  "hostBuildId": "desktop-0.1.0+20260710.1",
  "hostBinarySha256": "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  "runnerProfile": "standard_user_job",
  "containment": {
    "appFileApi": "selected_root_enforced",
    "processTree": "job_object_controlled",
    "childProcessIdentity": "signed_in_user",
    "childFilesystemEnforcement": "not_enforced",
    "childNetworkEnforcement": "declared_only",
    "measuredProfileHash": "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
  }
}
```

The `standard_user_job` example intentionally does not claim child-process filesystem or network confinement. A Job Object controls process-tree lifetime and resource accounting; it does not upgrade those fields.

## 6. Candidate actions

`CandidateAction` is the normative discriminated wire representation of the canonical `ExecutionSpecCandidate` domain object. The orchestrator creates it from typed model output; the model does not supply authority fields, target identities, policy hashes, measured containment, or final hashes.

### 6.1 Shared fields

```ts
interface CandidateCommon {
  schemaVersion: "sapphirus.candidate-action.v1";
  candidateId: ContractId;
  projectId: ContractId;
  runId: ContractId;
  proposalId: ContractId;
  proposalHash: Sha256;
  authorityRef: AuthorityRef;
  ownerScopeRef: ContractId;
  policyContextHash: Sha256;
  mutableInputs: MutableInputBinding[];
  declaredWrites: DeclaredWrite[];
  networkIntent: NetworkIntent;
  limits: ExecutionLimits;
  rollbackClass: "file_tracked" | "adapter_compensated" | "partially_reversible" | "non_reversible";
  createdAt: UtcInstant;
  expiresAt: UtcInstant;
  candidateHash: Sha256;
}

interface MutableInputBinding {
  inputKind: "workspace_manifest" | "path_preimage" | "package" | "toolchain" | "policy" | "external_resource";
  inputId: string;
  contentHash: Sha256;
}

interface DeclaredWrite {
  pathPattern: RelativeWorkspacePath;
  operation: "create" | "modify" | "delete" | "generated_output";
  preimageHash: Sha256 | null;
}

interface NetworkIntent {
  declaredMode: "off" | "package_registry" | "private_endpoints" | "public_internet";
  allowedDestinationSetHash: Sha256 | null;
  enforcementClaim: "azure_network_policy" | "declared_only" | "firewall_enforced";
}

interface ExecutionLimits {
  timeoutSeconds: number;
  maxOutputBytes: number;
  maxChangedFiles: number;
  maxChangedBytes: number;
  maxProcessCount: number;
}
```

Collections whose order has no domain meaning (`mutableInputs` and `declaredWrites`) are sorted by their declared canonical keys before hashing. The stored order MUST already be canonical; validators reject an unsorted authority object rather than silently reorder it.

### 6.2 Exact candidate union

```ts
type CandidateAction =
  | WebManagedCandidateAction
  | WindowsPatchCandidate
  | WindowsCommandCandidate
  | WindowsRollbackCandidate
  | WindowsRemoteHandoffCandidate;

interface WebManagedCandidateBase extends CandidateCommon {
  deliveryModel: "web_managed";
  workspaceTarget: CloudSnapshotTarget;
  executorAudience: AzureJobAudience;
}

type WebManagedCandidateAction = WebManagedCandidateBase & (
  | { actionKind: "patch_apply"; webAction: { patchRef: OpaqueRef; patchHash: Sha256 } }
  | { actionKind: "command_run"; webAction: { fixedCommandSpecRef: OpaqueRef; fixedCommandSpecHash: Sha256 } }
  | { actionKind: "artifact_export"; webAction: { exportManifestRef: OpaqueRef; exportManifestHash: Sha256 } }
  | { actionKind: "package_operation"; webAction: { packageActionRef: OpaqueRef; packageActionHash: Sha256 } }
  | { actionKind: "rollback"; webAction: { rollbackPlanRef: OpaqueRef; rollbackPlanHash: Sha256 } }
);

interface WindowsPatchCandidate extends CandidateCommon {
  deliveryModel: "windows_local";
  actionKind: "patch_apply";
  workspaceTarget: LocalFolderCapabilityTarget;
  executorAudience: WindowsLocalHostAudience;
  patchRef: OpaqueRef;
  patchHash: Sha256;
  preimages: LocalPathPreimage[];
}

interface WindowsRollbackCandidate extends CandidateCommon {
  deliveryModel: "windows_local";
  actionKind: "rollback";
  workspaceTarget: LocalFolderCapabilityTarget;
  executorAudience: WindowsLocalHostAudience;
  rollbackPlanId: ContractId;
  rollbackPlanHash: Sha256;
  conflictPreviewHash: Sha256;
}

interface WindowsRemoteHandoffCandidate extends CandidateCommon {
  deliveryModel: "windows_local";
  actionKind: "remote_job_handoff";
  workspaceTarget: LocalFolderCapabilityTarget;
  executorAudience: WindowsLocalHostAudience;
  uploadManifestRef: OpaqueRef;
  uploadManifestHash: Sha256;
  retentionPolicyHash: Sha256;
}

interface LocalPathPreimage {
  relativePath: RelativeWorkspacePath;
  exists: boolean;
  fileIdentityHash: Sha256 | null;
  contentHash: Sha256 | null;
  metadataHash: Sha256 | null;
}
```

### 6.3 `WindowsCommandCandidate`

```ts
interface WindowsCommandCandidate extends CandidateCommon {
  deliveryModel: "windows_local";
  actionKind: "command_run";
  workspaceTarget: LocalFolderCapabilityTarget;
  executorAudience: WindowsLocalHostAudience;
  command: {
    resolvedExecutable: {
      displayName: string;
      encryptedPathRef: OpaqueRef;
      fileIdentityHash: Sha256;
      contentHash: Sha256;
      signatureStatus: "valid" | "unsigned" | "invalid" | "unknown";
      signerThumbprintHash: Sha256 | null;
    };
    argv: string[];
    cwd: RelativeWorkspacePath;
    environment: {
      allowedNames: string[];
      valueSourceHash: Sha256;
    };
    scriptInputs: {
      relativePath: RelativeWorkspacePath;
      contentHash: Sha256;
    }[];
    commandSpecHash: Sha256;
  };
}
```

Example:

```json
{
  "schemaVersion": "sapphirus.candidate-action.v1",
  "candidateId": "candidate_01J000000000000000000000",
  "projectId": "project_01J0000000000000000000000",
  "runId": "run_01J00000000000000000000000",
  "proposalId": "proposal_01J000000000000000000000",
  "proposalHash": "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
  "authorityRef": {
    "authorityKind": "desktop_local_store",
    "authorityId": "authority_01J0000000000000000000000",
    "installationId": "install_01J00000000000000000000000",
    "localStoreId": "store_01J000000000000000000000000",
    "authorityEpoch": 1
  },
  "ownerScopeRef": "ownerscope_01J00000000000000000000",
  "policyContextHash": "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
  "mutableInputs": [
    {
      "inputKind": "workspace_manifest",
      "inputId": "manifest_01J00000000000000000000",
      "contentHash": "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
    }
  ],
  "declaredWrites": [
    {
      "pathPattern": "TestResults/**",
      "operation": "generated_output",
      "preimageHash": null
    }
  ],
  "networkIntent": {
    "declaredMode": "off",
    "allowedDestinationSetHash": null,
    "enforcementClaim": "declared_only"
  },
  "limits": {
    "timeoutSeconds": 600,
    "maxOutputBytes": 4000000,
    "maxChangedFiles": 200,
    "maxChangedBytes": 50000000,
    "maxProcessCount": 32
  },
  "rollbackClass": "file_tracked",
  "createdAt": "2026-07-10T10:02:00.000Z",
  "expiresAt": "2026-07-10T10:17:00.000Z",
  "deliveryModel": "windows_local",
  "actionKind": "command_run",
  "workspaceTarget": {
    "targetKind": "local_folder_capability",
    "workspaceCapabilityId": "lwc_01J00000000000000000000000",
    "grantEpoch": 4,
    "rootIdentityHash": "sha256:4444444444444444444444444444444444444444444444444444444444444444",
    "filesystemCapabilityHash": "sha256:5555555555555555555555555555555555555555555555555555555555555555",
    "baseCheckpointId": "lcp_01J00000000000000000000000",
    "workspaceManifestHash": "sha256:6666666666666666666666666666666666666666666666666666666666666666"
  },
  "executorAudience": {
    "audienceKind": "windows_local_host",
    "installationId": "install_01J00000000000000000000000",
    "hostBuildId": "desktop-0.1.0+20260710.1",
    "hostBinarySha256": "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    "runnerProfile": "standard_user_job",
    "containment": {
      "appFileApi": "selected_root_enforced",
      "processTree": "job_object_controlled",
      "childProcessIdentity": "signed_in_user",
      "childFilesystemEnforcement": "not_enforced",
      "childNetworkEnforcement": "declared_only",
      "measuredProfileHash": "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
    }
  },
  "command": {
    "resolvedExecutable": {
      "displayName": "dotnet",
      "encryptedPathRef": "local-encrypted://executables/dotnet",
      "fileIdentityHash": "sha256:1212121212121212121212121212121212121212121212121212121212121212",
      "contentHash": "sha256:1313131313131313131313131313131313131313131313131313131313131313",
      "signatureStatus": "valid",
      "signerThumbprintHash": "sha256:1414141414141414141414141414141414141414141414141414141414141414"
    },
    "argv": ["dotnet", "test", "--no-restore"],
    "cwd": ".",
    "environment": {
      "allowedNames": ["DOTNET_CLI_HOME", "PATH", "TEMP", "TMP"],
      "valueSourceHash": "sha256:1515151515151515151515151515151515151515151515151515151515151515"
    },
    "scriptInputs": [],
    "commandSpecHash": "sha256:1616161616161616161616161616161616161616161616161616161616161616"
  },
  "candidateHash": "sha256:1717171717171717171717171717171717171717171717171717171717171717"
}
```

`candidateHash` covers every candidate property except `candidateHash` itself. Executable resolution, `argv`, cwd, environment names/source, script hashes, expected writes, containment, target, and limits therefore cannot drift after approval.

## 7. Approval, immutable spec, and separate consumption

### 7.1 `ApprovedExecutionSpec`

```ts
interface ApprovedExecutionSpec {
  schemaVersion: "sapphirus.approved-execution-spec.v1";
  specId: ContractId;
  deliveryModel: DeliveryModel;
  authorityRef: AuthorityRef;
  ownerScopeRef: ContractId;
  projectId: ContractId;
  runId: ContractId;
  proposalId: ContractId;
  proposalHash: Sha256;
  candidateId: ContractId;
  candidateHash: Sha256;
  approvalId: ContractId;
  approvalDecisionHash: Sha256;
  policyVersion: string;
  policyHash: Sha256;
  workspaceTargetHash: Sha256;
  mutableInputSetHash: Sha256;
  executorAudience: ExecutorAudience;
  issuedAt: UtcInstant;
  expiresAt: UtcInstant;
  singleUseNonceHash: Sha256;
  specHash: Sha256;
}
```

The issuing authority revalidates the current candidate hash before minting the spec. Issuance MAY add only the approval, policy, audience, issue/expiry, and nonce bindings. It MUST NOT expand the action, target, inputs, outputs, network intent, rollback class, or limits.

The spec is immutable. Fields such as `consumed`, `consumedAt`, `result`, or `remainingUses` are prohibited.

### 7.2 `SpecConsumptionRecord`

```ts
interface SpecConsumptionRecord {
  schemaVersion: "sapphirus.spec-consumption.v1";
  consumptionId: ContractId;
  deliveryModel: DeliveryModel;
  authorityRef: AuthorityRef;
  specId: ContractId;
  specHash: Sha256;
  candidateHash: Sha256;
  singleUseNonceHash: Sha256;
  executorAudienceHash: Sha256;
  executionId: ContractId;
  attemptNumber: number;           // integer >= 1
  consumedAt: UtcInstant;
  consumptionHash: Sha256;
}
```

Only a successful compare-and-swap consume creates this immutable record. Failed or stale consumption attempts emit denial evidence but do not create a consumption record. Each authority enforces uniqueness on `(specHash, singleUseNonceHash, executorAudienceHash)`.

Example pair:

```json
{
  "spec": {
    "schemaVersion": "sapphirus.approved-execution-spec.v1",
    "specId": "spec_01J0000000000000000000000",
    "deliveryModel": "windows_local",
    "authorityRef": {
      "authorityKind": "desktop_local_store",
      "authorityId": "authority_01J0000000000000000000000",
      "installationId": "install_01J00000000000000000000000",
      "localStoreId": "store_01J000000000000000000000000",
      "authorityEpoch": 1
    },
    "ownerScopeRef": "ownerscope_01J00000000000000000000",
    "projectId": "project_01J0000000000000000000000",
    "runId": "run_01J00000000000000000000000",
    "proposalId": "proposal_01J000000000000000000000",
    "proposalHash": "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
    "candidateId": "candidate_01J000000000000000000000",
    "candidateHash": "sha256:1717171717171717171717171717171717171717171717171717171717171717",
    "approvalId": "approval_01J000000000000000000000",
    "approvalDecisionHash": "sha256:1818181818181818181818181818181818181818181818181818181818181818",
    "policyVersion": "desktop-airlock.2026-07-10",
    "policyHash": "sha256:1919191919191919191919191919191919191919191919191919191919191919",
    "workspaceTargetHash": "sha256:2020202020202020202020202020202020202020202020202020202020202020",
    "mutableInputSetHash": "sha256:2121212121212121212121212121212121212121212121212121212121212121",
    "executorAudience": {
      "audienceKind": "windows_local_host",
      "installationId": "install_01J00000000000000000000000",
      "hostBuildId": "desktop-0.1.0+20260710.1",
      "hostBinarySha256": "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
      "runnerProfile": "standard_user_job",
      "containment": {
        "appFileApi": "selected_root_enforced",
        "processTree": "job_object_controlled",
        "childProcessIdentity": "signed_in_user",
        "childFilesystemEnforcement": "not_enforced",
        "childNetworkEnforcement": "declared_only",
        "measuredProfileHash": "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
      }
    },
    "issuedAt": "2026-07-10T10:03:00.000Z",
    "expiresAt": "2026-07-10T10:13:00.000Z",
    "singleUseNonceHash": "sha256:2222222222222222222222222222222222222222222222222222222222222222",
    "specHash": "sha256:2323232323232323232323232323232323232323232323232323232323232323"
  },
  "consumption": {
    "schemaVersion": "sapphirus.spec-consumption.v1",
    "consumptionId": "consume_01J000000000000000000000",
    "deliveryModel": "windows_local",
    "authorityRef": {
      "authorityKind": "desktop_local_store",
      "authorityId": "authority_01J0000000000000000000000",
      "installationId": "install_01J00000000000000000000000",
      "localStoreId": "store_01J000000000000000000000000",
      "authorityEpoch": 1
    },
    "specId": "spec_01J0000000000000000000000",
    "specHash": "sha256:2323232323232323232323232323232323232323232323232323232323232323",
    "candidateHash": "sha256:1717171717171717171717171717171717171717171717171717171717171717",
    "singleUseNonceHash": "sha256:2222222222222222222222222222222222222222222222222222222222222222",
    "executorAudienceHash": "sha256:2424242424242424242424242424242424242424242424242424242424242424",
    "executionId": "execution_01J00000000000000000000",
    "attemptNumber": 1,
    "consumedAt": "2026-07-10T10:04:00.000Z",
    "consumptionHash": "sha256:2525252525252525252525252525252525252525252525252525252525252525"
  }
}
```

## 8. Execution result manifest union

### 8.1 Shared manifest fields

```ts
interface ExecutionResultCommon {
  schemaVersion: "sapphirus.execution-result-manifest.v1";
  manifestId: ContractId;
  authorityRef: AuthorityRef;
  ownerScopeRef: ContractId;
  projectId: ContractId;
  runId: ContractId;
  executionId: ContractId;
  candidateId: ContractId;
  candidateHash: Sha256;
  specId: ContractId;
  specHash: Sha256;
  policyHash: Sha256;
  approvalId: ContractId;
  consumptionId: ContractId;
  consumptionHash: Sha256;
  workspaceTargetHash: Sha256;
  executorAudienceHash: Sha256;
  startedAt: UtcInstant;
  completedAt: UtcInstant;
  status: "succeeded" | "failed" | "cancelled" | "timed_out";
  redactedLogRefs: ContentRef[];
  outputArtifacts: ArtifactRef[];
  validationSummaryHash: Sha256 | null;
  failureClassification: string | null;
  manifestHash: Sha256;
}

interface ContentRef {
  ref: OpaqueRef;
  contentHash: Sha256;
  byteLength: number;
  mediaType: string;
}

interface ArtifactRef extends ContentRef {
  artifactId: ContractId;
  classification: "public" | "internal" | "confidential" | "restricted";
}
```

### 8.2 Exact result union

```ts
type ExecutionResultManifest = WebWorkerResultManifest | WindowsLocalExecutionResultManifest;

interface WebWorkerResultManifest extends ExecutionResultCommon {
  deliveryModel: "web_managed";
  manifestKind: "web_worker_result";
  workItemId: ContractId;
  attemptId: ContractId;
  leaseId: ContractId;
  completionNonceHash: Sha256;
  jobTemplateId: string;
  workerImageDigest: Sha256;
  cloudSnapshotId: ContractId;
  cloudSnapshotHash: Sha256;
  commandResults: WebCommandResult[];
  checkpointCandidateRef: OpaqueRef | null;
}

interface WebCommandResult {
  commandSpecHash: Sha256;
  exitCode: number | null;
  startedAt: UtcInstant;
  completedAt: UtcInstant;
}

interface WindowsLocalExecutionResultManifest extends ExecutionResultCommon {
  deliveryModel: "windows_local";
  manifestKind: "windows_local_result";
  installationId: ContractId;
  hostBuildId: string;
  hostBinarySha256: Sha256;
  workspaceCapabilityId: ContractId;
  grantEpoch: number;
  rootIdentityHashBefore: Sha256;
  rootIdentityHashAfter: Sha256;
  effectJournalId: ContractId;
  preWriteCheckpointId: ContractId;
  observedEffect: LocalObservedEffect;
  changedFiles: LocalFileChange[];
  rollbackPlanId: ContractId | null;
  recoveryDisposition: "clean" | "recovered" | "manual_review";
}

type LocalObservedEffect =
  | {
      observedKind: "patch";
      patchHash: Sha256;
    }
  | {
      observedKind: "command";
      executableFileIdentityHash: Sha256;
      executableContentHash: Sha256;
      argvHash: Sha256;
      commandSpecHash: Sha256;
      exitCode: number | null;
      processTreeDisposition: "exited" | "cancelled" | "terminated" | "cleanup_incomplete";
    }
  | {
      observedKind: "rollback";
      rollbackPlanHash: Sha256;
    };

interface LocalFileChange {
  relativePath: RelativeWorkspacePath;
  operation: "created" | "modified" | "deleted";
  preFileIdentityHash: Sha256 | null;
  preContentHash: Sha256 | null;
  postFileIdentityHash: Sha256 | null;
  postContentHash: Sha256 | null;
  declared: boolean;
}
```

Example local result:

```json
{
  "schemaVersion": "sapphirus.execution-result-manifest.v1",
  "manifestId": "manifest_01J00000000000000000000",
  "deliveryModel": "windows_local",
  "manifestKind": "windows_local_result",
  "authorityRef": {
    "authorityKind": "desktop_local_store",
    "authorityId": "authority_01J0000000000000000000000",
    "installationId": "install_01J00000000000000000000000",
    "localStoreId": "store_01J000000000000000000000000",
    "authorityEpoch": 1
  },
  "ownerScopeRef": "ownerscope_01J00000000000000000000",
  "projectId": "project_01J0000000000000000000000",
  "runId": "run_01J00000000000000000000000",
  "executionId": "execution_01J00000000000000000000",
  "candidateId": "candidate_01J000000000000000000000",
  "candidateHash": "sha256:1717171717171717171717171717171717171717171717171717171717171717",
  "specId": "spec_01J0000000000000000000000",
  "specHash": "sha256:2323232323232323232323232323232323232323232323232323232323232323",
  "policyHash": "sha256:1919191919191919191919191919191919191919191919191919191919191919",
  "approvalId": "approval_01J000000000000000000000",
  "consumptionId": "consume_01J000000000000000000000",
  "consumptionHash": "sha256:2525252525252525252525252525252525252525252525252525252525252525",
  "workspaceTargetHash": "sha256:2020202020202020202020202020202020202020202020202020202020202020",
  "executorAudienceHash": "sha256:2424242424242424242424242424242424242424242424242424242424242424",
  "startedAt": "2026-07-10T10:04:00.000Z",
  "completedAt": "2026-07-10T10:04:12.000Z",
  "status": "succeeded",
  "redactedLogRefs": [],
  "outputArtifacts": [],
  "validationSummaryHash": "sha256:2626262626262626262626262626262626262626262626262626262626262626",
  "failureClassification": null,
  "installationId": "install_01J00000000000000000000000",
  "hostBuildId": "desktop-0.1.0+20260710.1",
  "hostBinarySha256": "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  "workspaceCapabilityId": "lwc_01J00000000000000000000000",
  "grantEpoch": 4,
  "rootIdentityHashBefore": "sha256:4444444444444444444444444444444444444444444444444444444444444444",
  "rootIdentityHashAfter": "sha256:4444444444444444444444444444444444444444444444444444444444444444",
  "effectJournalId": "journal_01J00000000000000000000",
  "preWriteCheckpointId": "lcp_01J00000000000000000000000",
  "observedEffect": {
    "observedKind": "command",
    "executableFileIdentityHash": "sha256:1212121212121212121212121212121212121212121212121212121212121212",
    "executableContentHash": "sha256:1313131313131313131313131313131313131313131313131313131313131313",
    "argvHash": "sha256:2727272727272727272727272727272727272727272727272727272727272727",
    "commandSpecHash": "sha256:1616161616161616161616161616161616161616161616161616161616161616",
    "exitCode": 0,
    "processTreeDisposition": "exited"
  },
  "changedFiles": [],
  "rollbackPlanId": null,
  "recoveryDisposition": "clean",
  "manifestHash": "sha256:2828282828282828282828282828282828282828282828282828282828282828"
}
```

Authority rules:

- A web worker writes a manifest and bounded payloads only. The .NET Runtime API authenticates/imports it and owns web lifecycle transitions.
- The Rust host observes local effects, creates the local manifest, and commits local lifecycle/evidence. A child process cannot mint or authenticate the manifest.
- Neither result variant is accepted by the other delivery authority as an execution completion.
- A remote result can be retained as verified input to `RemoteJobHandoff`; it cannot complete a local execution or apply files.

## 9. Evidence event and hash chain

```ts
type EvidenceActor =
  | { actorKind: "user"; subjectId: string }
  | { actorKind: "service"; serviceId: string }
  | { actorKind: "remote_worker"; workerIdentityHash: Sha256 }
  | { actorKind: "desktop_host"; installationId: ContractId; hostBinarySha256: Sha256 }
  | { actorKind: "local_process"; executableContentHash: Sha256 };

interface EvidenceEvent {
  schemaVersion: "sapphirus.evidence-event.v2";
  eventId: ContractId;
  deliveryModel: DeliveryModel;
  authorityRef: AuthorityRef;
  streamId: string;
  sequence: number;                // integer >= 1, contiguous within stream
  eventType: string;
  ownerScopeRef: ContractId;
  projectId: ContractId;
  runId: ContractId | null;
  actor: EvidenceActor;
  correlationId: string;
  causationId: string | null;
  occurredAt: UtcInstant;
  payloadHash: Sha256;
  payloadRef: OpaqueRef | null;
  redactionLevel: "summary" | "redacted" | "privileged";
  retentionClass: "operational" | "evidence" | "debug" | "privileged";
  previousEventHash: Sha256 | null;
  eventHash: Sha256;
}
```

Hash-chain rules:

1. `sequence = 1` requires `previousEventHash = null`.
2. `sequence > 1` requires `previousEventHash` equal to the accepted previous event's `eventHash` in the same `(authorityRef, streamId)`.
3. `eventHash` covers every property except `eventHash` itself using purpose `evidence-event` and schema major `v2`.
4. Sequence allocation, state transition, event append, and outbox append commit together in the owning authority's transaction.
5. A hash chain is tamper-evident ordering. It is not non-repudiation against an administrator or user controlling the authority.
6. Replication preserves the source chain; the replica never rewrites sequence, previous hash, actor, time, or authority.

```json
{
  "schemaVersion": "sapphirus.evidence-event.v2",
  "eventId": "event_01J0000000000000000000000",
  "deliveryModel": "windows_local",
  "authorityRef": {
    "authorityKind": "desktop_local_store",
    "authorityId": "authority_01J0000000000000000000000",
    "installationId": "install_01J00000000000000000000000",
    "localStoreId": "store_01J000000000000000000000000",
    "authorityEpoch": 1
  },
  "streamId": "run:run_01J00000000000000000000000",
  "sequence": 42,
  "eventType": "execution.completed",
  "ownerScopeRef": "ownerscope_01J00000000000000000000",
  "projectId": "project_01J0000000000000000000000",
  "runId": "run_01J00000000000000000000000",
  "actor": {
    "actorKind": "desktop_host",
    "installationId": "install_01J00000000000000000000000",
    "hostBinarySha256": "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
  },
  "correlationId": "corr-01J00000000000000000000000",
  "causationId": "consume_01J000000000000000000000",
  "occurredAt": "2026-07-10T10:04:13.000Z",
  "payloadHash": "sha256:2828282828282828282828282828282828282828282828282828282828282828",
  "payloadRef": "cas://sha256/28/28",
  "redactionLevel": "redacted",
  "retentionClass": "evidence",
  "previousEventHash": "sha256:2929292929292929292929292929292929292929292929292929292929292929",
  "eventHash": "sha256:3030303030303030303030303030303030303030303030303030303030303030"
}
```

Event types use the canonical catalog in `53`. Delivery-specific additions are added to that catalog before use; arbitrary UI event names are not persisted as evidence types.

## 10. Sync envelope and replica semantics

Sync is an explicit replication protocol, not distributed lifecycle authority.

```ts
type ReplicaClass =
  | "evidence_replica"
  | "collaboration_input"
  | "package_metadata"
  | "user_setting"
  | "telemetry_record"
  | "remote_job_link";

type SyncOperation = "upsert_immutable" | "append" | "tombstone";

interface SyncEnvelope {
  schemaVersion: "sapphirus.sync-envelope.v1";
  syncEnvelopeId: ContractId;
  sourceAuthority: AuthorityRef;
  sourceSequence: number;          // integer >= 1, unique per authority epoch
  sourceDeliveryModel: DeliveryModel;
  entityType: string;
  entityId: ContractId;
  entitySchemaVersion: string;
  entityVersion: string;
  replicaClass: ReplicaClass;
  operation: SyncOperation;
  sourceContentHash: Sha256;
  payloadRef: OpaqueRef | null;
  classification: "public" | "internal" | "confidential" | "restricted";
  consent: {
    consentKind: "not_required" | "workspace_upload" | "evidence_sync" | "collaboration_sync" | "telemetry";
    consentRecordId: ContractId | null;
    consentPolicyHash: Sha256;
  };
  encryption: {
    mode: "transport_only" | "recipient_encrypted";
    keyId: string | null;
    algorithm: string | null;
  };
  createdAt: UtcInstant;
  envelopeHash: Sha256;
  signature: {
    algorithm: "ed25519" | "ecdsa-p256-sha256";
    keyId: string;
    signature: Base64Url;
  };
}
```

The signature covers `envelopeHash`; `envelopeHash` covers all properties except `envelopeHash` and `signature`.

### 10.1 Merge rules

The following high-integrity entity types **MUST NOT** use last-writer-wins, field merge, receiver timestamps, or receiver-issued versions:

- `policy_decision`;
- `approval`;
- `approved_execution_spec`;
- `spec_consumption`;
- `execution_result_manifest`;
- `evidence_event`;
- `checkpoint` and `rollback_plan`;
- `remote_job_handoff`; and
- package signature or compatibility records.

They are append-only or immutable and deduplicated by source authority, source sequence, entity ID, and content hash. The same identity with a different hash is a visible conflict/security finding. A cloud acknowledgement means only that a replica was received.

For lower-integrity settings, a product-specific merge strategy MAY be defined, but it must be explicit in the entity schema. `tombstone` never deletes local workspace files, local authoritative evidence, an approval, a spec, a consumption, or a checkpoint.

Source code is not an automatic sync entity. Workspace upload requires a separate exact upload manifest and consent record.

```json
{
  "schemaVersion": "sapphirus.sync-envelope.v1",
  "syncEnvelopeId": "sync_01J00000000000000000000000",
  "sourceAuthority": {
    "authorityKind": "desktop_local_store",
    "authorityId": "authority_01J0000000000000000000000",
    "installationId": "install_01J00000000000000000000000",
    "localStoreId": "store_01J000000000000000000000000",
    "authorityEpoch": 1
  },
  "sourceSequence": 81,
  "sourceDeliveryModel": "windows_local",
  "entityType": "evidence_event",
  "entityId": "event_01J0000000000000000000000",
  "entitySchemaVersion": "sapphirus.evidence-event.v2",
  "entityVersion": "42",
  "replicaClass": "evidence_replica",
  "operation": "append",
  "sourceContentHash": "sha256:3030303030303030303030303030303030303030303030303030303030303030",
  "payloadRef": "recipient-encrypted://sync/01J00000000000000000000000",
  "classification": "confidential",
  "consent": {
    "consentKind": "evidence_sync",
    "consentRecordId": "consent_01J00000000000000000000",
    "consentPolicyHash": "sha256:3131313131313131313131313131313131313131313131313131313131313131"
  },
  "encryption": {
    "mode": "recipient_encrypted",
    "keyId": "sync-key-2026-01",
    "algorithm": "xchacha20-poly1305"
  },
  "createdAt": "2026-07-10T10:05:00.000Z",
  "envelopeHash": "sha256:3232323232323232323232323232323232323232323232323232323232323232",
  "signature": {
    "algorithm": "ed25519",
    "keyId": "desktop-sync-01",
    "signature": "ZXhhbXBsZS1zaWduYXR1cmU"
  }
}
```

## 11. Explicit remote-job handoff

`RemoteJobHandoff` links, but does not merge, a local authority and a cloud authority.

```ts
interface RemoteJobHandoffCommon {
  schemaVersion: "sapphirus.remote-job-handoff.v1";
  handoffId: ContractId;
  sourceAuthority: DesktopLocalStoreAuthorityRef;
  sourceProjectId: ContractId;
  sourceRunId: ContractId;
  sourceCheckpointId: ContractId;
  sourceWorkspaceManifestHash: Sha256;
  handoffVersion: number;          // integer >= 1
  previousHandoffHash: Sha256 | null;
  createdAt: UtcInstant;
  handoffHash: Sha256;
}

interface UploadPreview {
  uploadManifestRef: OpaqueRef;
  uploadManifestHash: Sha256;
  selectedEntryCount: number;
  selectedByteCount: number;
  redactionSummaryHash: Sha256;
  retentionPolicyHash: Sha256;
}

interface LocalHandoffAuthorization {
  candidateId: ContractId;
  candidateHash: Sha256;
  approvalId: ContractId;
  specId: ContractId;
  specHash: Sha256;
  consumptionId: ContractId;
}

interface CloudWorkLink {
  targetAuthority: AzureControlPlaneAuthorityRef;
  targetProjectId: ContractId;
  targetRunId: ContractId;
  targetWorkItemId: ContractId;
}

interface RemoteResultLink {
  remoteManifestRef: OpaqueRef;
  remoteManifestHash: Sha256;
  remoteEvidenceRangeHash: Sha256;
  cannotApplyDirectly: true;
}

type RemoteJobHandoff =
  | (RemoteJobHandoffCommon & { state: "draft" })
  | (RemoteJobHandoffCommon & { state: "upload_previewed"; uploadPreview: UploadPreview })
  | (RemoteJobHandoffCommon & { state: "locally_approved" | "uploading"; uploadPreview: UploadPreview; localAuthorization: LocalHandoffAuthorization })
  | (RemoteJobHandoffCommon & { state: "cloud_accepted" | "remote_running"; uploadPreview: UploadPreview; localAuthorization: LocalHandoffAuthorization; cloudWork: CloudWorkLink })
  | (RemoteJobHandoffCommon & { state: "result_available" | "result_verified"; uploadPreview: UploadPreview; localAuthorization: LocalHandoffAuthorization; cloudWork: CloudWorkLink; remoteResult: RemoteResultLink })
  | (RemoteJobHandoffCommon & { state: "imported_as_local_proposal" | "closed"; uploadPreview: UploadPreview; localAuthorization: LocalHandoffAuthorization; cloudWork: CloudWorkLink; remoteResult: RemoteResultLink; importedProposalId: ContractId; importedProposalHash: Sha256 })
  | (RemoteJobHandoffCommon & { state: "cancelled" | "expired" | "upload_failed" | "remote_failed" | "result_rejected"; failureCode: string; failureEvidenceHash: Sha256 });
```

Each accepted handoff transition appends a new immutable version with an incremented `handoffVersion` and the prior version's `handoffHash`. Version 1 requires `previousHandoffHash = null`. The current handoff state is a projection over that authority-owned chain, not a last-writer-wins row received from sync.

Example final handoff:

```json
{
  "schemaVersion": "sapphirus.remote-job-handoff.v1",
  "handoffId": "handoff_01J000000000000000000000",
  "sourceAuthority": {
    "authorityKind": "desktop_local_store",
    "authorityId": "authority_01J0000000000000000000000",
    "installationId": "install_01J00000000000000000000000",
    "localStoreId": "store_01J000000000000000000000000",
    "authorityEpoch": 1
  },
  "sourceProjectId": "project_01J0000000000000000000000",
  "sourceRunId": "run_01J00000000000000000000000",
  "sourceCheckpointId": "lcp_01J00000000000000000000000",
  "sourceWorkspaceManifestHash": "sha256:6666666666666666666666666666666666666666666666666666666666666666",
  "handoffVersion": 8,
  "previousHandoffHash": "sha256:3232323232323232323232323232323232323232323232323232323232323232",
  "createdAt": "2026-07-10T10:06:00.000Z",
  "state": "imported_as_local_proposal",
  "uploadPreview": {
    "uploadManifestRef": "cas://sha256/33/33",
    "uploadManifestHash": "sha256:3333333333333333333333333333333333333333333333333333333333333333",
    "selectedEntryCount": 120,
    "selectedByteCount": 870000,
    "redactionSummaryHash": "sha256:3434343434343434343434343434343434343434343434343434343434343434",
    "retentionPolicyHash": "sha256:3535353535353535353535353535353535353535353535353535353535353535"
  },
  "localAuthorization": {
    "candidateId": "candidate_01J000000000000000000000",
    "candidateHash": "sha256:3636363636363636363636363636363636363636363636363636363636363636",
    "approvalId": "approval_01J000000000000000000000",
    "specId": "spec_01J0000000000000000000000",
    "specHash": "sha256:3737373737373737373737373737373737373737373737373737373737373737",
    "consumptionId": "consume_01J000000000000000000000"
  },
  "cloudWork": {
    "targetAuthority": {
      "authorityKind": "azure_control_plane",
      "authorityId": "authority_01J0000000000000000000001",
      "tenantId": "tenant_01J00000000000000000000000",
      "controlPlaneInstanceId": "controlplane_01J0000000000000000",
      "authorityEpoch": 1,
      "region": "westeurope"
    },
    "targetProjectId": "project_01J0000000000000000000001",
    "targetRunId": "run_01J00000000000000000000001",
    "targetWorkItemId": "work_01J0000000000000000000000"
  },
  "remoteResult": {
    "remoteManifestRef": "azure-blob://remote-results/manifest.json",
    "remoteManifestHash": "sha256:3838383838383838383838383838383838383838383838383838383838383838",
    "remoteEvidenceRangeHash": "sha256:3939393939393939393939393939393939393939393939393939393939393939",
    "cannotApplyDirectly": true
  },
  "importedProposalId": "proposal_01J000000000000000000001",
  "importedProposalHash": "sha256:4040404040404040404040404040404040404040404040404040404040404040",
  "handoffHash": "sha256:4141414141414141414141414141414141414141414141414141414141414141"
}
```

The imported proposal is evaluated against the **current** local grant, checkpoint, workspace manifest, policy, and containment. Applying it requires a fresh local candidate, approval, spec, consumption, checkpoint, journal, result, and evidence chain.

## 12. Signed BMAD package compatibility

Shared BMAD packages can target both products, but package distribution and activation do not create workspace or execution authority.

```ts
interface PackageCompatibility {
  schemaVersion: "sapphirus.package-compatibility.v1";
  packageId: string;
  packageVersion: string;          // SemVer 2.0.0
  packageDigest: Sha256;
  packageManifestSchemaVersion: string;
  bmadRuntimeRange: string;
  contractEpoch: {
    minimum: number;
    maximum: number;
  };
  supportedDeliveryModels: DeliveryModel[];
  runtimeRanges: {
    webDotnet: string | null;
    desktopRustHost: string | null;
    typescriptUi: string;
  };
  requiredCapabilities: string[];
  optionalCapabilities: string[];
  forbiddenCapabilities: string[];
  conformanceBundle: {
    fixtureSetId: string;
    fixtureSetHash: Sha256;
    minimumConformanceLevel: "schema" | "semantic" | "execution_rehearsal";
  };
  revocationPolicyId: string;
  issuedAt: UtcInstant;
  expiresAt: UtcInstant | null;
  signedPayloadHash: Sha256;
  signature: {
    algorithm: "ed25519" | "ecdsa-p256-sha256";
    keyId: string;
    certificateChainRef: OpaqueRef | null;
    signature: Base64Url;
  };
}
```

Canonicalization rules sort and deduplicate `supportedDeliveryModels`, `requiredCapabilities`, `optionalCapabilities`, and `forbiddenCapabilities` before hashing. Overlap between required, optional, and forbidden capability sets is invalid.

```json
{
  "schemaVersion": "sapphirus.package-compatibility.v1",
  "packageId": "bmad.core.developer",
  "packageVersion": "1.4.0",
  "packageDigest": "sha256:4242424242424242424242424242424242424242424242424242424242424242",
  "packageManifestSchemaVersion": "sapphirus.bmad-package.v2",
  "bmadRuntimeRange": ">=1.4.0 <2.0.0",
  "contractEpoch": {
    "minimum": 1,
    "maximum": 1
  },
  "supportedDeliveryModels": ["web_managed", "windows_local"],
  "runtimeRanges": {
    "webDotnet": ">=1.0.0 <2.0.0",
    "desktopRustHost": ">=0.1.0 <1.0.0",
    "typescriptUi": ">=1.0.0 <2.0.0"
  },
  "requiredCapabilities": ["bmad.method_state.v1", "typed_model_output.v1"],
  "optionalCapabilities": ["remote_job_handoff.v1"],
  "forbiddenCapabilities": ["raw_shell.v1"],
  "conformanceBundle": {
    "fixtureSetId": "bmad-core-developer-1.4.0",
    "fixtureSetHash": "sha256:4343434343434343434343434343434343434343434343434343434343434343",
    "minimumConformanceLevel": "semantic"
  },
  "revocationPolicyId": "shared-packages-2026-01",
  "issuedAt": "2026-07-10T10:07:00.000Z",
  "expiresAt": null,
  "signedPayloadHash": "sha256:4444444444444444444444444444444444444444444444444444444444444444",
  "signature": {
    "algorithm": "ed25519",
    "keyId": "shared-package-signing-2026-01",
    "certificateChainRef": null,
    "signature": "ZXhhbXBsZS1wYWNrYWdlLXNpZ25hdHVyZQ"
  }
}
```

Activation requires signature/revocation validation, contract-epoch compatibility, runtime-range compatibility, capability satisfaction, and the package policy/rehearsal required by the target delivery model. A package that supports both models carries shared semantics, not shared executor code.

## 13. Schema evolution and compatibility

### 13.1 Version identifiers

- Schema identifiers use `sapphirus.<type>.v<major>`.
- The schema `$id` includes the immutable major version.
- Contract packages use SemVer.
- `contractEpoch` increments only for a coordinated semantic break spanning schemas, hashing, authority rules, or discriminator meaning.
- Event upcasters preserve source version and source hash in migration evidence.

### 13.2 Compatible and breaking changes

Within one schema major, a change is compatible only when all of the following are true:

- it does not change a required field, discriminator, enum meaning, canonicalization, hash basis, authority rule, or security default;
- any added optional field has an unambiguous absent meaning and is excluded from older senders by negotiated projection;
- retained durable versions have deterministic, side-effect-free upcasters; and
- C#, Rust, and TypeScript conformance suites pass the same fixtures.

The following require a new schema major and normally a contract-epoch review:

- adding or changing a delivery model, authority kind, target kind, audience kind, action kind, result kind, or high-integrity merge behavior;
- changing a hash purpose, canonical sort key, field name, field type, nullability, or timestamp precision;
- weakening a containment, consent, signature, approval, or spec-consumption invariant; and
- interpreting an old field with a new security meaning.

Unknown schema majors fail closed. Unknown discriminator or security-critical enum values fail closed. There is no generic `other` executor, authority, target, or effect variant.

### 13.3 Durable upcasting

Upcasters MUST be pure, deterministic, and side-effect free. They MAY create a current in-memory projection; they MUST NOT rewrite the original immutable object or its hash. If a current semantic projection cannot be produced without guessing, the object remains readable as evidence but is incompatible with execution.

## 14. Canonicalization and hashing

### 14.1 Canonical JSON

All contract hashes use RFC 8785 JSON Canonicalization Scheme behavior:

1. validate the exact schema first;
2. validate set ordering/deduplication and path normalization rules;
3. remove only the hash/signature fields explicitly excluded by that schema;
4. serialize canonical JSON as UTF-8 with no byte-order mark; and
5. apply purpose separation.

Hash formula:

```text
sha256(UTF8("sapphirus:" + purpose + ":" + schemaMajor + "\n" + JCS(value)))
```

Serialized form is lowercase `sha256:` followed by exactly 64 lowercase hexadecimal characters.

Required purposes include:

| Object | Purpose | Excluded field(s) |
|---|---|---|
| Durable payload | `contract-object` | envelope `contentHash` is outside payload |
| Candidate | `candidate-action` | `candidateHash` |
| Approved spec | `approved-execution-spec` | `specHash` |
| Spec consumption | `spec-consumption` | `consumptionHash` |
| Result manifest | `execution-result-manifest` | `manifestHash` |
| Evidence event | `evidence-event` | `eventHash` |
| Sync envelope | `sync-envelope` | `envelopeHash`, `signature` |
| Handoff | `remote-job-handoff` | `handoffHash` |
| Package compatibility | `package-compatibility` | `signedPayloadHash`, `signature` |

### 14.2 Golden hash vectors

These vectors are mandatory in every implementation. `canonicalJson` below is already canonical and the preimage contains one LF after the version token.

| Purpose/version | Canonical JSON | Expected hash |
|---|---|---|
| `contract-object/v1` | `{"deliveryModel":"windows_local","objectId":"run_01J00000000000000000000000"}` | `sha256:cee935f23a73790e45e1226e6f6c2ad8dd74f33059dc77e646863db08327d9b4` |
| `evidence-event/v2` | `{"eventId":"evt_01J00000000000000000000000","previousEventHash":null,"sequence":1}` | `sha256:80dcb430cf9139ad48f2c869d2476560fa4e4e84d5a61b6438eefecd2d19f9e1` |
| `mutable-input-set/v1` | `[]` | `sha256:2ca367f19010d684123667da585b3c4e2ecbbb86e2b69a3e58765485d14acfef` |

Golden fixtures also include nested candidates, specs, manifests, sync signatures, Unicode strings, integer boundaries, member-order permutations, and negative duplicate-key inputs.

## 15. Identifiers, time, paths, and errors

### 15.1 Identifiers

New durable IDs use a lowercase semantic prefix and an uppercase Crockford Base32 payload. Producers SHOULD use a 26-character ULID payload when offline generation and time-sortable diagnostics are useful, but consumers treat the payload as opaque:

```regex
^[a-z][a-z0-9_]{1,31}_[0-9A-HJKMNP-TV-Z]{16,64}$
```

IDs are globally collision resistant and can be generated offline. Sortability is operational convenience, not causal order or authority. External provider IDs are stored in specifically named external-reference fields and do not replace contract IDs.

### 15.2 Time

- Canonical serialization is UTC RFC 3339 with exactly millisecond precision: `YYYY-MM-DDTHH:mm:ss.sssZ`.
- Leap-second strings are rejected unless a future contract major defines handling.
- State guards use the owning authority's trusted clock policy; a client-rendered time cannot approve, consume, expire, or order an object.
- Causality and evidence ordering use sequence/version, not wall-clock comparison.

### 15.3 Relative paths

Wire paths use `/`, NFC normalization, no leading slash, no `.`/`..` segments except the whole cwd value `"."`, and no NUL. This representation is for review and hashing. The Windows Workspace Broker still performs handle/file-identity/reparse validation; a normalized string is never proof of containment.

### 15.4 Error envelope

```ts
interface ContractError {
  schemaVersion: "sapphirus.error.v1";
  errorId: ContractId;
  code:
    | "SCHEMA_UNSUPPORTED"
    | "SCHEMA_INVALID"
    | "DELIVERY_MODEL_MISMATCH"
    | "AUTHORITY_MISMATCH"
    | "WORKSPACE_TARGET_MISMATCH"
    | "EXECUTOR_AUDIENCE_MISMATCH"
    | "HASH_MISMATCH"
    | "STALE_MUTABLE_INPUT"
    | "SPEC_EXPIRED"
    | "SPEC_ALREADY_CONSUMED"
    | "SYNC_CONFLICT"
    | "SIGNATURE_INVALID"
    | "COMPATIBILITY_BLOCKED";
  message: string;
  correlationId: string;
  retryable: boolean;
  detailsRef: OpaqueRef | null;
}
```

Messages are safe for the target UI and never contain access tokens, encryption keys, absolute local paths, raw prompts, secret values, SQL details, or stack traces. Implementations may define more codes through the governed catalog; they may not map an authority or hash mismatch to a generic successful fallback.

## 16. Code generation and implementation boundaries

### 16.1 Schema-first artifacts

The reviewed JSON Schemas are the source of truth. Publish one versioned contract release into:

```text
packages/contracts/schemas/              # reviewed JSON Schema source
packages/contracts/fixtures/             # language-neutral valid/invalid/golden vectors
packages/contracts/generated/dotnet/     # C# wire records and serializers
packages/contracts/generated/rust/       # Rust serde wire structs/enums
packages/contracts/generated/typescript/ # TypeScript types and runtime validators
packages/contracts/schema-lock.json      # IDs, hashes, compatibility classification
```

Recommended package identities:

- NuGet: `Sapphirus.Contracts`;
- Rust crate: `sapphirus-contracts`;
- npm: `@sapphirus/contracts`.

Generated files are reproducible and are never hand-edited. CI regenerates them and fails on an unexplained diff.

### 16.2 What may be generated

| Generate | Keep handwritten |
|---|---|
| Wire records/enums | Domain aggregates and state machines |
| JSON serializers/deserializers | Authority resolution and authorization |
| Structural validators | Candidate normalization and Airlock policy |
| Discriminator visitors | Hash-purpose selection and consumption transaction |
| OpenAPI/IPC DTO projections | Workspace path/handle validation |
| Fixture loaders | Executor dispatch, journaling, recovery, storage adapters |

Generated structural validation is necessary but not sufficient. Each native authority runs semantic validators after schema validation and before persistence, approval, consumption, execution, sync import, or package activation.

### 16.3 Runtime-specific rules

| Runtime | Required behavior | Forbidden behavior |
|---|---|---|
| C#/.NET Web Runtime | Accept web contracts, validate replicas/handoffs/packages, own Azure lifecycle transitions | Consume a desktop local spec, address a local folder, or treat a replica as SQL authority |
| Rust/Tauri Local Runtime | Accept desktop contracts, own local hashes/specs/journals/evidence, verify remote replicas/results/packages | Accept renderer-minted authority fields, consume an Azure job spec locally, or expose local-store/file handles |
| TypeScript UI | Render discriminated projections, construct typed user commands/decisions, display exact hash/risk/containment summaries | Mint candidates/specs/evidence, compute authoritative lifecycle state, open DB/CAS, or bypass native/server validation |

The Tauri IPC command envelope is a delivery-specific transport around shared IDs/projections; it is not an Azure API contract. OpenAPI is a web/support-plane transport; it is not a local filesystem API.

## 17. Conformance suite

### 17.1 Fixture layout

```text
packages/contracts/fixtures/
  valid/
    shared/
    web_managed/
    windows_local/
    sync/
    packages/
  invalid/
    schema/
    discriminator_mismatch/
    authority_mismatch/
    hash_mismatch/
    spec_consumption/
    sync_conflict/
    remote_handoff/
  golden/
    canonical-json/
    hashes/
    signatures/
    upcasts/
  compatibility/
    retained-schema-majors/
    previous-runtime-release/
```

Every fixture contains the JSON document, expected validation result, expected normalized discriminator, expected hash/signature where applicable, and stable reason code for invalid fixtures.

### 17.2 Required tests in all three languages

1. Validate every valid fixture and reject every invalid fixture with the same reason category.
2. Deserialize and reserialize without changing canonical JSON or hashes.
3. Match every golden hash and signature vector byte for byte.
4. Reject all four mismatches: delivery/authority, delivery/target, delivery/audience, and source/replica authority.
5. Reject unknown schema majors, discriminators, security enums, duplicate members, unknown fields, and noncanonical set order.
6. Prove an approved spec remains byte-identical after consumption; only `SpecConsumptionRecord` is added.
7. Prove a second concurrent consumption of the same nonce cannot succeed.
8. Prove a remote result cannot deserialize as a local result, local spec, patch candidate, or file-apply command.
9. Prove high-integrity sync conflicts surface and never resolve through last-writer-wins.
10. Prove package compatibility rejects an unsupported runtime, contract epoch, delivery model, capability, signature, or revocation state.

### 17.3 Property and mutation tests

- Arbitrary JSON member permutations produce the same JCS hash.
- Any mutation of a hashed semantic field changes the hash.
- Excluded hash/signature fields do not create a self-reference.
- Round-tripping preserves large integers, Unicode, `null`, empty arrays, and boolean values exactly.
- Generated delivery unions accept exactly one branch; removing or adding branch-specific properties fails.
- A `Project` generator never produces a child with a different delivery model.
- Evidence sequences detect gap, duplicate, reordering, changed previous hash, and changed payload hash.
- Sync delivery under duplicate, delayed, and out-of-order transport is idempotent and does not advance source lifecycle state.
- Remote-handoff state generation permits only the required fields for that state.
- Candidate/spec mutation testing attempts every field removal, expansion, null substitution, enum substitution, and cross-branch transplant.

Path-resolution, NTFS reparse, Job Object, AppContainer, Azure job, database transaction, and crash-recovery tests are delivery-specific and remain in their owning suites. Shared conformance fixtures cannot prove an operating-system or cloud isolation claim.

### 17.4 Compatibility matrix

| Contract | .NET Web Runtime | Rust Local Runtime | TypeScript UI | Cross-authority rule |
|---|---|---|---|---|
| `DeliveryModel`, envelope, IDs, hashes | produce/consume | produce/consume | consume/render | Same semantics |
| Web candidate/spec/result | authoritative produce/import | verify only for handoff evidence | render web projection | Never executable locally |
| Desktop candidate/spec/result | reject as web authority; replica/support inspection only | authoritative produce/consume | render desktop projection | Never executable in Azure |
| Evidence event | authoritative web append; verify replica | authoritative local append; verify replica | render projection only | Source authority/hash preserved |
| Sync envelope | verify/import replica | sign/export or verify/import | settings/status projection only | Acknowledgement is not transition |
| Remote-job handoff | create linked web work and return result | own local handoff and proposal import | render consent/diff states | Two authorities, two runs, two approvals |
| Package compatibility | verify and activate web package | verify/cache/activate desktop package | render metadata | Shared package, separate adapter/executor |

### 17.5 Cross-language release fixture

At least one end-to-end fixture per release performs:

```text
Rust creates local candidate + spec + consumption + result + evidence
-> C# verifies a sync replica without changing source authority
-> TypeScript renders the projection and exact containment claim

C# creates web candidate + spec + worker result + evidence
-> Rust verifies it only as remote-handoff evidence
-> TypeScript renders the remote result as cannotApplyDirectly
```

No step is allowed to reserialize with a different hash or convert one delivery variant into the other.

## 18. Version negotiation

```ts
interface ContractCapabilities {
  schemaVersion: "sapphirus.contract-capabilities.v1";
  runtimeKind: "dotnet_web_runtime" | "rust_desktop_host" | "typescript_web_ui" | "typescript_desktop_ui";
  runtimeVersion: string;
  contractPackageVersion: string;
  contractEpoch: number;
  supportedSchemas: {
    schemaId: string;
    minimumMajor: number;
    maximumMajor: number;
  }[];
  supportedCapabilities: string[];
  capabilitiesHash: Sha256;
}
```

Negotiation rules:

1. Peers exchange authenticated capabilities before sending durable or authority-bearing payloads.
2. They require the same contract epoch and choose the highest mutually supported schema major for each exchanged type.
3. The sender projects to that exact version; the receiver does not guess from missing fields.
4. No mutual major produces `SCHEMA_UNSUPPORTED`, not a lossy downgrade.
5. Desktop UI/host versions are bundled and checked at boot. A mismatched renderer is not allowed to issue mutating IPC commands.
6. Cloud sync may queue an opaque encrypted envelope when the entity version is temporarily unreadable, but it cannot apply, acknowledge semantic import, or change lifecycle state until a supported verifier exists.
7. Package activation negotiates package compatibility separately and cannot expand runtime contract capabilities.

Support-window policy MUST name the oldest durable schema major readable by each release, the oldest peer version supported for sync/collaboration, and whether downgrade is safe after store/database migration.

## 19. Release gates

A release that changes any contract in this document is blocked until all applicable gates pass:

- **CONTRACT-01 — Schema lock:** reviewed schemas, `$id`s, generated artifacts, and `schema-lock.json` agree with no unexplained diff.
- **CONTRACT-02 — Three-language conformance:** C#, Rust, and TypeScript pass the same valid, invalid, canonicalization, hash, signature, and upcast fixtures.
- **CONTRACT-03 — Authority separation:** negative tests reject every web/desktop discriminator, authority, target, audience, spec, result, and persistence transplant.
- **CONTRACT-04 — Immutable approval:** candidate drift voids approval; spec issuance cannot expand effect; consumption is a separate unique record.
- **CONTRACT-05 — Evidence integrity:** state/evidence/outbox atomicity passes in each authority and hash-chain gap/tamper tests pass.
- **CONTRACT-06 — Replica safety:** duplicate/out-of-order/conflicting/tombstone sync tests prove no LWW for high-integrity objects and no local file deletion.
- **CONTRACT-07 — Remote handoff:** exact consent/upload, separate cloud run, `cannotApplyDirectly: true`, result verification, fresh local proposal, and fresh local approval are demonstrated end to end.
- **CONTRACT-08 — Package compatibility:** signature, revocation, contract epoch, delivery target, runtime range, capabilities, and fixture bundle are enforced.
- **CONTRACT-09 — Upgrade window:** retained durable schemas upcast deterministically; unsupported newer stores/databases fail safely; downgrade constraints are documented.
- **CONTRACT-10 — UI non-authority:** both TypeScript surfaces display exact hashes/containment/egress and cannot mint or mutate native/server authority records.

Desktop execution additionally remains gated by the DESK-01 containment decision in `95`; shared schema conformance cannot waive that security decision.

## 20. Explicit unresolved decisions

The following are **UNRESOLVED** and require an ADR or recorded spike result before their dependent feature ships:

| ID | Decision | Blocking scope |
|---|---|---|
| `CONTRACT-U01` | Select the production signature algorithm, trust root, rotation, revocation, and offline-verification model for sync envelopes and BMAD packages. | Sync/package production release |
| `CONTRACT-U02` | Select the exact schema/code-generation toolchain and prove stable union/nullability output for C#, Rust, and TypeScript. | First generated contract release |
| `CONTRACT-U03` | Define the supported schema/runtime window and emergency rollback policy for independently updated cloud and desktop clients. | Public beta/update service |
| `CONTRACT-U04` | Decide whether and how local evidence-chain heads are cloud-anchored or device-signed for higher assurance. Local chains remain tamper-evident, not non-repudiable, until then. | Regulated assurance tier only |
| `CONTRACT-U05` | Select sync payload encryption/key-sharing and recovery semantics, including multi-device collaboration and enterprise key escrow policy. | Confidential evidence sync |
| `CONTRACT-U06` | Define authority-epoch restore/link rules and the user-visible behavior when a desktop backup is restored beside an existing replica. | Backup/restore plus sync |
| `CONTRACT-U07` | Resolve DESK-01: whether arbitrary child tools must be filesystem/network confined, and whether AppContainer/brokered execution is sufficiently compatible. | Claim that commands can access only selected folders |
| `CONTRACT-U08` | Decide whether project-visible `.sapphirus/` metadata is supported and, if so, define a non-authoritative signed schema with privacy rules. | Workspace metadata feature |
| `CONTRACT-U09` | Define which collaboration inputs may be mutable and their explicit merge algorithms; approvals/specs/results/evidence/checkpoints remain excluded. | Multi-user collaboration editing |
| `CONTRACT-U10` | Select package capability vocabulary ownership and the compatibility-rehearsal level required for each package risk class. | Third-party package ecosystem |

Until resolved, implementations fail closed or keep the feature disabled. An unresolved item is never represented by a permissive default or an `unknown` authority variant.

## 21. Definition of done

- [ ] Every durable/shared schema has an exact discriminator and `additionalProperties: false`.
- [ ] Project/run delivery model is persisted and immutable in both authority stores.
- [ ] Authority, workspace target, executor audience, candidate, spec, result, and evidence variants cannot be mixed.
- [ ] The approved spec is immutable and consumption is a separate CAS-protected record.
- [ ] Web and local manifests share semantic evidence links but remain distinct result variants.
- [ ] Evidence hashes and sequences verify in C#, Rust, and TypeScript fixtures.
- [ ] Sync preserves source authority and does not use LWW for high-integrity objects.
- [ ] Remote results always carry `cannotApplyDirectly: true` and require fresh local proposal/approval/apply.
- [ ] Signed BMAD compatibility declares delivery/runtime/capability support and passes conformance fixtures.
- [ ] Code generation is reproducible and domain/security behavior remains handwritten in the owning runtime.
- [ ] Golden vectors, property tests, compatibility tests, and release gates pass in CI.
- [ ] Every unresolved shipping dependency is either resolved by ADR/evidence or remains disabled.
