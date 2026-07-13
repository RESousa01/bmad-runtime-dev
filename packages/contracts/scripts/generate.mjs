import { createHash } from "node:crypto";
import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import Ajv2020 from "ajv/dist/2020.js";
import standaloneCode from "ajv/dist/standalone/index.js";
import { compile as generateTypescript } from "json-schema-to-typescript";
import { canonicalHash, canonicalize } from "./lib/canonical-json.mjs";
import { sealDocument, sealDurableObject } from "./lib/semantics.mjs";
import { parseStrictJson } from "./lib/strict-json.mjs";

const packageRoot = fileURLToPath(new URL("../", import.meta.url));
const checkOnly = process.argv.includes("--check");
const typescriptOnly = process.argv.includes("--typescript-only");
const unknownArguments = process.argv
  .slice(2)
  .filter((argument) => argument !== "--check" && argument !== "--typescript-only");
if (unknownArguments.length > 0) {
  throw new Error(`Unsupported generator arguments: ${unknownArguments.join(", ")}`);
}
if (typescriptOnly && !checkOnly) {
  throw new Error("--typescript-only is a read-only verification mode and requires --check.");
}
const expectedFiles = new Map();

function stableJson(value) {
  return `${JSON.stringify(value, null, 2)}\n`;
}

function sha256(value) {
  return `sha256:${createHash("sha256").update(value, "utf8").digest("hex")}`;
}

function id(prefix, fill = "0") {
  return `${prefix}_01J${fill.repeat(23)}`;
}

function hash(fill) {
  return `sha256:${fill.repeat(64)}`;
}

function add(relativePath, content) {
  expectedFiles.set(relativePath.replaceAll("\\", "/"), content);
}

const authority = {
  authorityKind: "desktop_local_store",
  authorityId: id("authority"),
  installationId: id("install"),
  localStoreId: id("store"),
  authorityEpoch: 1,
};

const workspaceTarget = {
  targetKind: "local_folder_capability",
  workspaceCapabilityId: id("lwc"),
  grantEpoch: 1,
  rootIdentityHash: hash("1"),
  filesystemCapabilityHash: hash("2"),
  baseCheckpointId: id("lcp"),
  workspaceManifestHash: hash("3"),
};

const executorAudience = {
  audienceKind: "native_patch_engine",
  installationId: id("install"),
  hostBuildId: "desktop-0.1.0-beta.1",
  hostBinarySha256: hash("4"),
  patchEngineProfileHash: hash("5"),
};

const candidate = sealDocument({
  schemaVersion: "sapphirus.candidate-action.v1",
  candidateId: id("candidate"),
  projectId: id("project"),
  runId: id("run"),
  proposalId: id("proposal"),
  proposalHash: hash("6"),
  deliveryModel: "windows_local",
  actionKind: "patch_apply",
  authorityRef: authority,
  ownerScopeRef: id("ownerscope"),
  policyContextHash: hash("7"),
  workspaceTarget,
  executorAudience,
  mutableInputs: [
    {
      inputKind: "path_preimage",
      inputId: "README.md",
      contentHash: hash("8"),
    },
    {
      inputKind: "workspace_manifest",
      inputId: id("manifest"),
      contentHash: hash("9"),
    },
  ],
  declaredWrites: [
    {
      pathPattern: "README.md",
      operation: "modify",
      preimageHash: hash("8"),
    },
    {
      pathPattern: "src/app.ts",
      operation: "create",
      preimageHash: null,
    },
  ],
  limits: {
    timeoutSeconds: 0,
    maxOutputBytes: 0,
    maxChangedFiles: 20,
    maxChangedBytes: 1048576,
    maxProcessCount: 0,
  },
  rollbackClass: "file_tracked",
  patchRef: "cas://sha256/patch-fixture",
  patchHash: hash("a"),
  preimages: [
    {
      relativePath: "README.md",
      exists: true,
      fileIdentityHash: hash("b"),
      contentHash: hash("8"),
      metadataHash: hash("c"),
    },
    {
      relativePath: "src/app.ts",
      exists: false,
      fileIdentityHash: null,
      contentHash: null,
      metadataHash: null,
    },
  ],
  createdAt: "2026-07-13T10:00:00.000Z",
  expiresAt: "2026-07-13T10:15:00.000Z",
  candidateHash: hash("0"),
});

const workspaceTargetHash = canonicalHash({
  purpose: "workspace-target",
  schemaMajor: "v1",
  value: workspaceTarget,
}).serializedHash;
const mutableInputSetHash = canonicalHash({
  purpose: "mutable-input-set",
  schemaMajor: "v1",
  value: candidate.mutableInputs,
}).serializedHash;
const executorAudienceHash = canonicalHash({
  purpose: "executor-audience",
  schemaMajor: "v1",
  value: executorAudience,
}).serializedHash;

const spec = sealDocument({
  schemaVersion: "sapphirus.approved-execution-spec.v1",
  specId: id("spec"),
  deliveryModel: "windows_local",
  authorityRef: authority,
  ownerScopeRef: id("ownerscope"),
  projectId: id("project"),
  runId: id("run"),
  proposalId: id("proposal"),
  proposalHash: candidate.proposalHash,
  candidateId: candidate.candidateId,
  candidateHash: candidate.candidateHash,
  approvalId: id("approval"),
  approvalDecisionHash: hash("d"),
  policyVersion: "desktop-airlock.2026-07-13",
  policyHash: hash("e"),
  workspaceTargetHash,
  mutableInputSetHash,
  executorAudience,
  issuedAt: "2026-07-13T10:01:00.000Z",
  expiresAt: "2026-07-13T10:11:00.000Z",
  singleUseNonceHash: hash("f"),
  specHash: hash("0"),
});

const consumption = sealDocument({
  schemaVersion: "sapphirus.spec-consumption.v1",
  consumptionId: id("consume"),
  deliveryModel: "windows_local",
  authorityRef: authority,
  specId: spec.specId,
  specHash: spec.specHash,
  candidateHash: candidate.candidateHash,
  singleUseNonceHash: spec.singleUseNonceHash,
  executorAudienceHash,
  executionId: id("execution"),
  attemptNumber: 1,
  consumedAt: "2026-07-13T10:02:00.000Z",
  consumptionHash: hash("0"),
});

const result = sealDocument({
  schemaVersion: "sapphirus.execution-result-manifest.v1",
  manifestId: id("manifest"),
  deliveryModel: "windows_local",
  manifestKind: "windows_local_result",
  authorityRef: authority,
  ownerScopeRef: id("ownerscope"),
  projectId: id("project"),
  runId: id("run"),
  executionId: consumption.executionId,
  candidateId: candidate.candidateId,
  candidateHash: candidate.candidateHash,
  specId: spec.specId,
  specHash: spec.specHash,
  policyHash: spec.policyHash,
  approvalId: spec.approvalId,
  consumptionId: consumption.consumptionId,
  consumptionHash: consumption.consumptionHash,
  workspaceTargetHash,
  executorAudienceHash,
  startedAt: "2026-07-13T10:02:01.000Z",
  completedAt: "2026-07-13T10:02:02.000Z",
  status: "succeeded",
  redactedLogRefs: [],
  outputArtifacts: [],
  validationSummaryHash: hash("1"),
  failureClassification: null,
  installationId: authority.installationId,
  hostBuildId: executorAudience.hostBuildId,
  hostBinarySha256: executorAudience.hostBinarySha256,
  workspaceCapabilityId: workspaceTarget.workspaceCapabilityId,
  grantEpoch: workspaceTarget.grantEpoch,
  rootIdentityHashBefore: workspaceTarget.rootIdentityHash,
  rootIdentityHashAfter: workspaceTarget.rootIdentityHash,
  effectJournalId: id("journal"),
  preWriteCheckpointId: workspaceTarget.baseCheckpointId,
  observedEffect: {
    observedKind: "patch",
    patchHash: candidate.patchHash,
  },
  changedFiles: [
    {
      relativePath: "README.md",
      operation: "modified",
      preFileIdentityHash: hash("b"),
      preContentHash: hash("8"),
      postFileIdentityHash: hash("b"),
      postContentHash: hash("2"),
      declared: true,
    },
    {
      relativePath: "src/app.ts",
      operation: "created",
      preFileIdentityHash: null,
      preContentHash: null,
      postFileIdentityHash: hash("3"),
      postContentHash: hash("4"),
      declared: true,
    },
  ],
  rollbackPlanId: id("rollback"),
  recoveryDisposition: "clean",
  manifestHash: hash("0"),
});

const evidenceEvent = sealDocument({
  schemaVersion: "sapphirus.evidence-event.v2",
  eventId: id("event"),
  deliveryModel: "windows_local",
  authorityRef: authority,
  streamId: `run:${id("run")}`,
  sequence: 1,
  eventType: "execution.completed",
  ownerScopeRef: id("ownerscope"),
  projectId: id("project"),
  runId: id("run"),
  actor: {
    actorKind: "desktop_host",
    installationId: authority.installationId,
    hostBinarySha256: executorAudience.hostBinarySha256,
  },
  correlationId: "corr-fixture-0001",
  causationId: consumption.consumptionId,
  occurredAt: "2026-07-13T10:02:03.000Z",
  payloadHash: result.manifestHash,
  payloadRef: "cas://sha256/result-fixture",
  redactionLevel: "redacted",
  retentionClass: "evidence",
  previousEventHash: null,
  eventHash: hash("0"),
});

const durableObject = sealDurableObject({
  envelope: {
    schemaVersion: "sapphirus.durable-object.v1",
    objectType: "execution_spec_candidate",
    objectId: id("object"),
    deliveryModel: "windows_local",
    authorityRef: authority,
    ownerScopeRef: id("ownerscope"),
    projectId: id("project"),
    runId: id("run"),
    createdAt: "2026-07-13T10:00:01.000Z",
    contentHash: hash("0"),
  },
  payload: {
    candidateId: candidate.candidateId,
  },
});

const filesystemCapability = {
  schemaVersion: "sapphirus.filesystem-capability.v1",
  snapshotId: id("fsc"),
  workspaceCapabilityId: workspaceTarget.workspaceCapabilityId,
  grantEpoch: workspaceTarget.grantEpoch,
  capturedAt: "2026-07-13T10:00:02.000Z",
  filesystemKind: "ntfs",
  locationClass: "fixed_local",
  volumeIdentityHash: hash("7"),
  rootFileIdentityHash: hash("8"),
  caseSensitiveDirectory: false,
  cloudPlaceholderState: "none",
  capabilities: {
    stableFileIds: "verified",
    reparseInspection: "verified",
    hardlinkCount: "verified",
    perFileAtomicReplace: "verified",
    durableFileFlush: "verified",
    durableDirectoryFlush: "unknown",
  },
  supportTier: "supported_writable",
  policyVersion: "desktop-filesystem.2026-07-13",
  policyHash: hash("9"),
  snapshotHash: hash("a"),
};

const contractError = {
  schemaVersion: "sapphirus.error.v1",
  errorId: id("error"),
  code: "COMPATIBILITY_BLOCKED",
  message: "This package is not compatible with the current desktop policy.",
  correlationId: "corr-fixture-0002",
  retryable: false,
  detailsRef: null,
};

const packageCompatibility = sealDocument({
  schemaVersion: "sapphirus.package-compatibility.v1",
  packageId: "bmad.core.developer",
  packageVersion: "1.4.0",
  packageDigest: hash("4"),
  packageManifestSchemaVersion: "sapphirus.bmad-package.v2",
  bmadRuntimeRange: ">=1.4.0 <2.0.0",
  contractEpoch: {
    minimum: 1,
    maximum: 1,
  },
  supportedDeliveryModels: ["web_managed", "windows_local"],
  runtimeRanges: {
    webDotnet: ">=1.0.0 <2.0.0",
    desktopRustHost: ">=0.1.0 <1.0.0",
    typescriptUi: ">=1.0.0 <2.0.0",
  },
  requiredCapabilities: ["bmad.method_state.v1", "typed_model_output.v1"],
  optionalCapabilities: ["remote_job_handoff.v1"],
  forbiddenCapabilities: ["untrusted_package_assets.v1"],
  conformanceBundle: {
    fixtureSetId: "bmad-core-developer-1.4.0",
    fixtureSetHash: hash("5"),
    minimumConformanceLevel: "semantic",
  },
  revocationPolicyId: "shared-packages-2026-01",
  issuedAt: "2026-07-13T10:07:00.000Z",
  expiresAt: null,
  signedPayloadHash: hash("0"),
  signature: {
    algorithm: "ed25519",
    keyId: "shared-package-signing-2026-01",
    certificateChainRef: null,
    signature: "ZXhhbXBsZS1wYWNrYWdlLXNpZ25hdHVyZQ",
  },
});

const remoteJobHandoff = sealDocument({
  schemaVersion: "sapphirus.remote-job-handoff.v1",
  handoffId: id("handoff"),
  sourceAuthority: authority,
  sourceProjectId: id("project"),
  sourceRunId: id("run"),
  sourceCheckpointId: workspaceTarget.baseCheckpointId,
  sourceWorkspaceManifestHash: workspaceTarget.workspaceManifestHash,
  handoffVersion: 8,
  previousHandoffHash: hash("2"),
  createdAt: "2026-07-13T10:06:00.000Z",
  state: "imported_as_local_proposal",
  uploadPreview: {
    uploadManifestRef: "cas://sha256/33/33",
    uploadManifestHash: hash("3"),
    selectedEntryCount: 120,
    selectedByteCount: 870000,
    redactionSummaryHash: hash("4"),
    retentionPolicyHash: hash("5"),
  },
  localAuthorization: {
    candidateId: candidate.candidateId,
    candidateHash: candidate.candidateHash,
    approvalId: spec.approvalId,
    specId: spec.specId,
    specHash: spec.specHash,
    consumptionId: consumption.consumptionId,
  },
  cloudWork: {
    targetAuthority: {
      authorityKind: "azure_control_plane",
      authorityId: id("authority", "1"),
      tenantId: id("tenant"),
      controlPlaneInstanceId: id("controlplane"),
      authorityEpoch: 1,
      region: "westeurope",
    },
    targetProjectId: id("project", "1"),
    targetRunId: id("run", "1"),
    targetWorkItemId: id("work"),
  },
  remoteResult: {
    remoteManifestRef: "azure-blob://remote-results/manifest.json",
    remoteManifestHash: hash("8"),
    remoteEvidenceRangeHash: hash("9"),
    cannotApplyDirectly: true,
  },
  importedProposalId: id("proposal", "1"),
  importedProposalHash: hash("4"),
  handoffHash: hash("0"),
});

add("fixtures/valid/windows-local-candidate.json", stableJson(candidate));
add("fixtures/valid/approved-execution-spec.json", stableJson(spec));
add("fixtures/valid/spec-consumption.json", stableJson(consumption));
add("fixtures/valid/execution-result-manifest.json", stableJson(result));
add("fixtures/valid/evidence-event.json", stableJson(evidenceEvent));
add("fixtures/valid/durable-object.json", stableJson(durableObject));
add("fixtures/valid/filesystem-capability.json", stableJson(filesystemCapability));
add("fixtures/valid/contract-error.json", stableJson(contractError));
add("fixtures/valid/package-compatibility.json", stableJson(packageCompatibility));
add("fixtures/valid/remote-job-handoff.json", stableJson(remoteJobHandoff));

const unknownDiscriminator = structuredClone(candidate);
unknownDiscriminator.actionKind = "unsupported_action";
add("fixtures/invalid/unknown-discriminator.json", stableJson(unknownDiscriminator));

const authorityMismatch = structuredClone(candidate);
authorityMismatch.authorityRef = {
  authorityKind: "azure_control_plane",
  authorityId: id("authority", "1"),
  tenantId: id("tenant"),
  controlPlaneInstanceId: id("controlplane"),
  authorityEpoch: 1,
  region: "westeurope",
};
add("fixtures/invalid/authority-mismatch.json", stableJson(authorityMismatch));

const targetMismatch = structuredClone(candidate);
targetMismatch.workspaceTarget = {
  targetKind: "cloud_snapshot",
  workspaceId: id("workspace"),
  snapshotId: id("snapshot"),
  snapshotHash: hash("1"),
  snapshotObjectRef: "azure-blob://fixture/snapshot",
  baseCheckpointId: null,
  checkoutPolicyHash: hash("2"),
};
add(
  "fixtures/invalid/workspace-target-mismatch.json",
  stableJson(sealDocument(targetMismatch)),
);

const audienceMismatch = structuredClone(candidate);
audienceMismatch.executorAudience = {
  audienceKind: "unsupported_executor",
  installationId: id("install"),
  hostBuildId: "desktop-0.1.0-beta.1",
  hostBinarySha256: hash("4"),
  patchEngineProfileHash: hash("5"),
};
add(
  "fixtures/invalid/executor-audience-mismatch.json",
  stableJson(sealDocument(audienceMismatch)),
);

const hashMismatch = structuredClone(candidate);
hashMismatch.candidateHash = hash("f");
add("fixtures/invalid/hash-mismatch.json", stableJson(hashMismatch));

const unsortedInputs = structuredClone(candidate);
unsortedInputs.mutableInputs.reverse();
add("fixtures/invalid/unsorted-inputs.json", stableJson(sealDocument(unsortedInputs)));

const unknownProperty = structuredClone(candidate);
unknownProperty.rendererApproved = true;
add(
  "fixtures/invalid/unknown-property.json",
  stableJson(sealDocument(unknownProperty)),
);

const traversalPath = structuredClone(candidate);
traversalPath.declaredWrites[0].pathPattern = "../README.md";
add("fixtures/invalid/traversal-path.json", stableJson(sealDocument(traversalPath)));

const mutableSpec = structuredClone(spec);
mutableSpec.consumed = true;
add("fixtures/invalid/mutable-spec-state.json", stableJson(sealDocument(mutableSpec)));

const brokenEvidenceChain = structuredClone(evidenceEvent);
brokenEvidenceChain.sequence = 2;
add(
  "fixtures/invalid/evidence-chain-gap.json",
  stableJson(sealDocument(brokenEvidenceChain)),
);

const invalidTimestamp = structuredClone(evidenceEvent);
invalidTimestamp.occurredAt = "2026-02-31T10:02:03.000Z";
add(
  "fixtures/invalid/invalid-timestamp.json",
  stableJson(sealDocument(invalidTimestamp)),
);

const unknownMajor = structuredClone(candidate);
unknownMajor.schemaVersion = "sapphirus.candidate-action.v9";
add("fixtures/invalid/unknown-schema-major.json", stableJson(unknownMajor));

const filesystemUnknownProperty = structuredClone(filesystemCapability);
filesystemUnknownProperty.unreviewedCapability = true;
add(
  "fixtures/invalid/filesystem-capability-unknown-property.json",
  stableJson(filesystemUnknownProperty),
);

const contractErrorControlCharacter = structuredClone(contractError);
contractErrorControlCharacter.message = "The request failed.\nContact the workspace owner.";
add(
  "fixtures/invalid/contract-error-control-character.json",
  stableJson(contractErrorControlCharacter),
);

const contractErrorPathDisclosure = structuredClone(contractError);
contractErrorPathDisclosure.message =
  "The request failed while reading C:/Users/example/source/file.ts.";
add(
  "fixtures/invalid/contract-error-path-disclosure.json",
  stableJson(contractErrorPathDisclosure),
);

const contractErrorDetailsRefControl = structuredClone(contractError);
contractErrorDetailsRefControl.detailsRef = "cas://errors/line\nbreak";
add(
  "fixtures/invalid/contract-error-details-ref-control-character.json",
  stableJson(contractErrorDetailsRefControl),
);

const contractErrorDetailsRefLocalPath = structuredClone(contractError);
contractErrorDetailsRefLocalPath.detailsRef = "C:/Users/example/source/error.json";
add(
  "fixtures/invalid/contract-error-details-ref-local-path.json",
  stableJson(contractErrorDetailsRefLocalPath),
);

const handoffUnknownState = structuredClone(remoteJobHandoff);
handoffUnknownState.state = "applied_to_local_workspace";
add(
  "fixtures/invalid/remote-handoff-unknown-state.json",
  stableJson(sealDocument(handoffUnknownState)),
);

const handoffDirectApply = structuredClone(remoteJobHandoff);
handoffDirectApply.remoteResult.cannotApplyDirectly = false;
add(
  "fixtures/invalid/remote-handoff-direct-apply.json",
  stableJson(sealDocument(handoffDirectApply)),
);

const overlappingCapabilities = structuredClone(packageCompatibility);
overlappingCapabilities.optionalCapabilities = [
  "remote_job_handoff.v1",
  "typed_model_output.v1",
];
add(
  "fixtures/invalid/package-capability-overlap.json",
  stableJson(sealDocument(overlappingCapabilities)),
);

const unsortedCapabilities = structuredClone(packageCompatibility);
unsortedCapabilities.requiredCapabilities.reverse();
add(
  "fixtures/invalid/package-capability-unsorted.json",
  stableJson(sealDocument(unsortedCapabilities)),
);

const invertedContractEpoch = structuredClone(packageCompatibility);
invertedContractEpoch.contractEpoch = { minimum: 2, maximum: 1 };
add(
  "fixtures/invalid/package-contract-epoch-inverted.json",
  stableJson(sealDocument(invertedContractEpoch)),
);

const invalidGenesisHandoff = structuredClone(remoteJobHandoff);
invalidGenesisHandoff.handoffVersion = 1;
add(
  "fixtures/invalid/remote-handoff-genesis-previous-hash.json",
  stableJson(sealDocument(invalidGenesisHandoff)),
);

const missingPreviousHandoff = structuredClone(remoteJobHandoff);
missingPreviousHandoff.previousHandoffHash = null;
add(
  "fixtures/invalid/remote-handoff-missing-previous-hash.json",
  stableJson(sealDocument(missingPreviousHandoff)),
);

add(
  "fixtures/invalid/duplicate-member.json",
  '{"schemaVersion":"sapphirus.candidate-action.v1","schemaVersion":"sapphirus.candidate-action.v1"}\n',
);

const catalog = [
  ["valid/windows-local-candidate.json", "candidate-action.schema.json", true, null],
  ["valid/approved-execution-spec.json", "approved-execution-spec.schema.json", true, null],
  ["valid/spec-consumption.json", "spec-consumption.schema.json", true, null],
  ["valid/execution-result-manifest.json", "execution-result-manifest.schema.json", true, null],
  ["valid/evidence-event.json", "evidence-event.schema.json", true, null],
  ["valid/durable-object.json", "durable-object.schema.json", true, null],
  ["valid/filesystem-capability.json", "filesystem-capability.schema.json", true, null],
  ["valid/contract-error.json", "contract-error.schema.json", true, null],
  ["valid/package-compatibility.json", "package-compatibility.schema.json", true, null],
  ["valid/remote-job-handoff.json", "remote-job-handoff.schema.json", true, null],
  ["invalid/unknown-discriminator.json", "candidate-action.schema.json", false, "CONST_MISMATCH"],
  ["invalid/authority-mismatch.json", "candidate-action.schema.json", false, "CONST_MISMATCH"],
  ["invalid/workspace-target-mismatch.json", "candidate-action.schema.json", false, "CONST_MISMATCH"],
  ["invalid/executor-audience-mismatch.json", "candidate-action.schema.json", false, "CONST_MISMATCH"],
  ["invalid/hash-mismatch.json", "candidate-action.schema.json", false, "HASH_MISMATCH"],
  ["invalid/unsorted-inputs.json", "candidate-action.schema.json", false, "MUTABLE_INPUT_SET_NOT_CANONICAL"],
  ["invalid/unknown-property.json", "candidate-action.schema.json", false, "UNKNOWN_PROPERTY"],
  ["invalid/traversal-path.json", "candidate-action.schema.json", false, "PATTERN_MISMATCH"],
  ["invalid/mutable-spec-state.json", "approved-execution-spec.schema.json", false, "UNKNOWN_PROPERTY"],
  ["invalid/evidence-chain-gap.json", "evidence-event.schema.json", false, "EVIDENCE_CHAIN_PREVIOUS_HASH_REQUIRED"],
  ["invalid/invalid-timestamp.json", "evidence-event.schema.json", false, "INVALID_UTC_INSTANT"],
  ["invalid/unknown-schema-major.json", "candidate-action.schema.json", false, "CONST_MISMATCH"],
  ["invalid/filesystem-capability-unknown-property.json", "filesystem-capability.schema.json", false, "UNKNOWN_PROPERTY"],
  ["invalid/contract-error-control-character.json", "contract-error.schema.json", false, "ERROR_MESSAGE_CONTROL_CHARACTER"],
  ["invalid/contract-error-path-disclosure.json", "contract-error.schema.json", false, "ERROR_MESSAGE_PATH_DISCLOSURE"],
  ["invalid/contract-error-details-ref-control-character.json", "contract-error.schema.json", false, "ERROR_DETAILS_REF_CONTROL_CHARACTER"],
  ["invalid/contract-error-details-ref-local-path.json", "contract-error.schema.json", false, "ERROR_DETAILS_REF_LOCAL_PATH"],
  ["invalid/remote-handoff-unknown-state.json", "remote-job-handoff.schema.json", false, "ONE_OF_MISMATCH"],
  ["invalid/remote-handoff-direct-apply.json", "remote-job-handoff.schema.json", false, "ONE_OF_MISMATCH"],
  ["invalid/package-capability-overlap.json", "package-compatibility.schema.json", false, "CAPABILITY_SET_OVERLAP"],
  ["invalid/package-capability-unsorted.json", "package-compatibility.schema.json", false, "CAPABILITY_SET_NOT_CANONICAL"],
  ["invalid/package-contract-epoch-inverted.json", "package-compatibility.schema.json", false, "CONTRACT_EPOCH_RANGE_INVALID"],
  ["invalid/remote-handoff-genesis-previous-hash.json", "remote-job-handoff.schema.json", false, "HANDOFF_GENESIS_PREVIOUS_HASH"],
  ["invalid/remote-handoff-missing-previous-hash.json", "remote-job-handoff.schema.json", false, "HANDOFF_CHAIN_PREVIOUS_HASH_REQUIRED"],
  ["invalid/duplicate-member.json", null, false, "DUPLICATE_MEMBER"],
].map(([file, schema, valid, reasonCode]) => ({ file, schema, valid, reasonCode }));
add("fixtures/catalog.json", stableJson(catalog));

const requiredGoldenVectors = [
  {
    purpose: "contract-object",
    schemaMajor: "v1",
    value: {
      deliveryModel: "windows_local",
      objectId: "run_01J00000000000000000000000",
    },
    canonicalJson:
      '{"deliveryModel":"windows_local","objectId":"run_01J00000000000000000000000"}',
    expectedHash:
      "sha256:cee935f23a73790e45e1226e6f6c2ad8dd74f33059dc77e646863db08327d9b4",
  },
  {
    purpose: "evidence-event",
    schemaMajor: "v2",
    value: {
      eventId: "evt_01J00000000000000000000000",
      previousEventHash: null,
      sequence: 1,
    },
    canonicalJson:
      '{"eventId":"evt_01J00000000000000000000000","previousEventHash":null,"sequence":1}',
    expectedHash:
      "sha256:80dcb430cf9139ad48f2c869d2476560fa4e4e84d5a61b6438eefecd2d19f9e1",
  },
  {
    purpose: "mutable-input-set",
    schemaMajor: "v1",
    value: [],
    canonicalJson: "[]",
    expectedHash:
      "sha256:2ca367f19010d684123667da585b3c4e2ecbbb86e2b69a3e58765485d14acfef",
  },
];

const supplementalGoldenValues = [
  {
    name: "unicode-and-member-order",
    purpose: "contract-object",
    schemaMajor: "v1",
    value: { z: "é", a: "😀", nested: { beta: 2, alpha: 1 } },
  },
  {
    name: "integer-boundaries",
    purpose: "contract-object",
    schemaMajor: "v1",
    value: { maximum: 9007199254740991, minimum: -9007199254740991 },
  },
].map((vector) => {
  const computed = canonicalHash(vector);
  return {
    ...vector,
    canonicalJson: computed.canonicalJson,
    expectedHash: computed.serializedHash,
  };
});

add(
  "fixtures/golden/hash-vectors.json",
  stableJson({
    source: "bmad-runtime-lib/99 section 14.2",
    required: requiredGoldenVectors,
    supplemental: supplementalGoldenValues,
  }),
);

const typescriptRuntime = `// @generated by scripts/generate.mjs; DO NOT EDIT.

export const CONTRACT_EPOCH = 1;

export const HASH_RULES = Object.freeze({
  "sapphirus.candidate-action.v1": Object.freeze({
    excludedFields: Object.freeze(["candidateHash"]),
    purpose: "candidate-action",
    schemaMajor: "v1",
  }),
  "sapphirus.approved-execution-spec.v1": Object.freeze({
    excludedFields: Object.freeze(["specHash"]),
    purpose: "approved-execution-spec",
    schemaMajor: "v1",
  }),
  "sapphirus.spec-consumption.v1": Object.freeze({
    excludedFields: Object.freeze(["consumptionHash"]),
    purpose: "spec-consumption",
    schemaMajor: "v1",
  }),
  "sapphirus.execution-result-manifest.v1": Object.freeze({
    excludedFields: Object.freeze(["manifestHash"]),
    purpose: "execution-result-manifest",
    schemaMajor: "v1",
  }),
  "sapphirus.evidence-event.v2": Object.freeze({
    excludedFields: Object.freeze(["eventHash"]),
    purpose: "evidence-event",
    schemaMajor: "v2",
  }),
  "sapphirus.remote-job-handoff.v1": Object.freeze({
    excludedFields: Object.freeze(["handoffHash"]),
    purpose: "remote-job-handoff",
    schemaMajor: "v1",
  }),
  "sapphirus.package-compatibility.v1": Object.freeze({
    excludedFields: Object.freeze(["signedPayloadHash", "signature"]),
    purpose: "package-compatibility",
    schemaMajor: "v1",
  }),
});
`;

function getRustContracts() {
  return `// @generated by scripts/generate.mjs; DO NOT EDIT.

use serde::{Deserialize, Serialize};

pub type Sha256 = String;
pub type ContractId = String;
pub type UtcInstant = String;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryModel {
    WebManaged,
    WindowsLocal,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "authorityKind", deny_unknown_fields)]
pub enum AuthorityRef {
    #[serde(rename = "azure_control_plane")]
    AzureControlPlane {
        #[serde(rename = "authorityId")]
        authority_id: ContractId,
        #[serde(rename = "tenantId")]
        tenant_id: ContractId,
        #[serde(rename = "controlPlaneInstanceId")]
        control_plane_instance_id: ContractId,
        #[serde(rename = "authorityEpoch")]
        authority_epoch: u64,
        region: String,
    },
    #[serde(rename = "desktop_local_store")]
    DesktopLocalStore {
        #[serde(rename = "authorityId")]
        authority_id: ContractId,
        #[serde(rename = "installationId")]
        installation_id: ContractId,
        #[serde(rename = "localStoreId")]
        local_store_id: ContractId,
        #[serde(rename = "authorityEpoch")]
        authority_epoch: u64,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalFolderCapabilityTarget {
    pub target_kind: String,
    pub workspace_capability_id: ContractId,
    pub grant_epoch: u64,
    pub root_identity_hash: Sha256,
    pub filesystem_capability_hash: Sha256,
    pub base_checkpoint_id: ContractId,
    pub workspace_manifest_hash: Sha256,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativePatchEngineAudience {
    pub audience_kind: String,
    pub installation_id: ContractId,
    pub host_build_id: String,
    pub host_binary_sha256: Sha256,
    pub patch_engine_profile_hash: Sha256,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MutableInputBinding {
    pub input_kind: String,
    pub input_id: String,
    pub content_hash: Sha256,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeclaredWrite {
    pub path_pattern: String,
    pub operation: String,
    pub preimage_hash: Option<Sha256>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionLimits {
    pub timeout_seconds: u64,
    pub max_output_bytes: u64,
    pub max_changed_files: u64,
    pub max_changed_bytes: u64,
    pub max_process_count: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalPathPreimage {
    pub relative_path: String,
    pub exists: bool,
    pub file_identity_hash: Option<Sha256>,
    pub content_hash: Option<Sha256>,
    pub metadata_hash: Option<Sha256>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CandidateAction {
    pub schema_version: String,
    pub candidate_id: ContractId,
    pub project_id: ContractId,
    pub run_id: ContractId,
    pub proposal_id: ContractId,
    pub proposal_hash: Sha256,
    pub delivery_model: DeliveryModel,
    pub action_kind: String,
    pub authority_ref: AuthorityRef,
    pub owner_scope_ref: ContractId,
    pub policy_context_hash: Sha256,
    pub workspace_target: LocalFolderCapabilityTarget,
    pub executor_audience: NativePatchEngineAudience,
    pub mutable_inputs: Vec<MutableInputBinding>,
    pub declared_writes: Vec<DeclaredWrite>,
    pub limits: ExecutionLimits,
    pub rollback_class: String,
    pub patch_ref: String,
    pub patch_hash: Sha256,
    pub preimages: Vec<LocalPathPreimage>,
    pub created_at: UtcInstant,
    pub expires_at: UtcInstant,
    pub candidate_hash: Sha256,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovedExecutionSpec {
    pub schema_version: String,
    pub spec_id: ContractId,
    pub delivery_model: DeliveryModel,
    pub authority_ref: AuthorityRef,
    pub owner_scope_ref: ContractId,
    pub project_id: ContractId,
    pub run_id: ContractId,
    pub proposal_id: ContractId,
    pub proposal_hash: Sha256,
    pub candidate_id: ContractId,
    pub candidate_hash: Sha256,
    pub approval_id: ContractId,
    pub approval_decision_hash: Sha256,
    pub policy_version: String,
    pub policy_hash: Sha256,
    pub workspace_target_hash: Sha256,
    pub mutable_input_set_hash: Sha256,
    pub executor_audience: NativePatchEngineAudience,
    pub issued_at: UtcInstant,
    pub expires_at: UtcInstant,
    pub single_use_nonce_hash: Sha256,
    pub spec_hash: Sha256,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecConsumptionRecord {
    pub schema_version: String,
    pub consumption_id: ContractId,
    pub delivery_model: DeliveryModel,
    pub authority_ref: AuthorityRef,
    pub spec_id: ContractId,
    pub spec_hash: Sha256,
    pub candidate_hash: Sha256,
    pub single_use_nonce_hash: Sha256,
    pub executor_audience_hash: Sha256,
    pub execution_id: ContractId,
    pub attempt_number: u64,
    pub consumed_at: UtcInstant,
    pub consumption_hash: Sha256,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentRef {
    #[serde(rename = "ref")]
    pub reference: String,
    pub content_hash: Sha256,
    pub byte_length: u64,
    pub media_type: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactRef {
    #[serde(rename = "ref")]
    pub reference: String,
    pub content_hash: Sha256,
    pub byte_length: u64,
    pub media_type: String,
    pub artifact_id: ContractId,
    pub classification: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalObservedEffect {
    pub observed_kind: String,
    pub patch_hash: Sha256,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalFileChange {
    pub relative_path: String,
    pub operation: String,
    pub pre_file_identity_hash: Option<Sha256>,
    pub pre_content_hash: Option<Sha256>,
    pub post_file_identity_hash: Option<Sha256>,
    pub post_content_hash: Option<Sha256>,
    pub declared: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionResultManifest {
    pub schema_version: String,
    pub manifest_id: ContractId,
    pub delivery_model: DeliveryModel,
    pub manifest_kind: String,
    pub authority_ref: AuthorityRef,
    pub owner_scope_ref: ContractId,
    pub project_id: ContractId,
    pub run_id: ContractId,
    pub execution_id: ContractId,
    pub candidate_id: ContractId,
    pub candidate_hash: Sha256,
    pub spec_id: ContractId,
    pub spec_hash: Sha256,
    pub policy_hash: Sha256,
    pub approval_id: ContractId,
    pub consumption_id: ContractId,
    pub consumption_hash: Sha256,
    pub workspace_target_hash: Sha256,
    pub executor_audience_hash: Sha256,
    pub started_at: UtcInstant,
    pub completed_at: UtcInstant,
    pub status: String,
    pub redacted_log_refs: Vec<ContentRef>,
    pub output_artifacts: Vec<ArtifactRef>,
    pub validation_summary_hash: Option<Sha256>,
    pub failure_classification: Option<String>,
    pub installation_id: ContractId,
    pub host_build_id: String,
    pub host_binary_sha256: Sha256,
    pub workspace_capability_id: ContractId,
    pub grant_epoch: u64,
    pub root_identity_hash_before: Sha256,
    pub root_identity_hash_after: Sha256,
    pub effect_journal_id: ContractId,
    pub pre_write_checkpoint_id: ContractId,
    pub observed_effect: LocalObservedEffect,
    pub changed_files: Vec<LocalFileChange>,
    pub rollback_plan_id: Option<ContractId>,
    pub recovery_disposition: String,
    pub manifest_hash: Sha256,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "actorKind", deny_unknown_fields)]
pub enum EvidenceActor {
    #[serde(rename = "user")]
    User {
        #[serde(rename = "subjectId")]
        subject_id: String,
    },
    #[serde(rename = "service")]
    Service {
        #[serde(rename = "serviceId")]
        service_id: String,
    },
    #[serde(rename = "desktop_host")]
    DesktopHost {
        #[serde(rename = "installationId")]
        installation_id: ContractId,
        #[serde(rename = "hostBinarySha256")]
        host_binary_sha256: Sha256,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceEvent {
    pub schema_version: String,
    pub event_id: ContractId,
    pub delivery_model: DeliveryModel,
    pub authority_ref: AuthorityRef,
    pub stream_id: String,
    pub sequence: u64,
    pub event_type: String,
    pub owner_scope_ref: ContractId,
    pub project_id: ContractId,
    pub run_id: Option<ContractId>,
    pub actor: EvidenceActor,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub occurred_at: UtcInstant,
    pub payload_hash: Sha256,
    pub payload_ref: Option<String>,
    pub redaction_level: String,
    pub retention_class: String,
    pub previous_event_hash: Option<Sha256>,
    pub event_hash: Sha256,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CandidatePointer {
    pub candidate_id: ContractId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DurableObjectEnvelope {
    pub schema_version: String,
    pub object_type: String,
    pub object_id: ContractId,
    pub delivery_model: DeliveryModel,
    pub authority_ref: AuthorityRef,
    pub owner_scope_ref: ContractId,
    pub project_id: ContractId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<ContractId>,
    pub created_at: UtcInstant,
    pub content_hash: Sha256,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DurableObject {
    pub envelope: DurableObjectEnvelope,
    pub payload: CandidatePointer,
}
`;
}

function getDotnetContracts() {
  return `// <auto-generated by="scripts/generate.mjs" />
#nullable enable
using System;
using System.Collections.Generic;
using System.Text.Json.Serialization;

namespace Sapphirus.Contracts.Generated;

[JsonPolymorphic(TypeDiscriminatorPropertyName = "authorityKind")]
[JsonDerivedType(typeof(AzureControlPlaneAuthorityRef), "azure_control_plane")]
[JsonDerivedType(typeof(DesktopLocalStoreAuthorityRef), "desktop_local_store")]
public abstract record AuthorityRef;

public sealed record AzureControlPlaneAuthorityRef(
    string AuthorityId,
    string TenantId,
    string ControlPlaneInstanceId,
    long AuthorityEpoch,
    string Region) : AuthorityRef;

public sealed record DesktopLocalStoreAuthorityRef(
    string AuthorityId,
    string InstallationId,
    string LocalStoreId,
    long AuthorityEpoch) : AuthorityRef;

public sealed record LocalFolderCapabilityTarget(
    string TargetKind,
    string WorkspaceCapabilityId,
    long GrantEpoch,
    string RootIdentityHash,
    string FilesystemCapabilityHash,
    string BaseCheckpointId,
    string WorkspaceManifestHash);

public sealed record NativePatchEngineAudience(
    string AudienceKind,
    string InstallationId,
    string HostBuildId,
    string HostBinarySha256,
    string PatchEngineProfileHash);

public sealed record MutableInputBinding(
    string InputKind,
    string InputId,
    string ContentHash);

public sealed record DeclaredWrite(
    string PathPattern,
    string Operation,
    string? PreimageHash);

public sealed record ExecutionLimits(
    long TimeoutSeconds,
    long MaxOutputBytes,
    long MaxChangedFiles,
    long MaxChangedBytes,
    long MaxProcessCount);

public sealed record LocalPathPreimage(
    string RelativePath,
    bool Exists,
    string? FileIdentityHash,
    string? ContentHash,
    string? MetadataHash);

public sealed record CandidateAction(
    string SchemaVersion,
    string CandidateId,
    string ProjectId,
    string RunId,
    string ProposalId,
    string ProposalHash,
    string DeliveryModel,
    string ActionKind,
    DesktopLocalStoreAuthorityRef AuthorityRef,
    string OwnerScopeRef,
    string PolicyContextHash,
    LocalFolderCapabilityTarget WorkspaceTarget,
    NativePatchEngineAudience ExecutorAudience,
    IReadOnlyList<MutableInputBinding> MutableInputs,
    IReadOnlyList<DeclaredWrite> DeclaredWrites,
    ExecutionLimits Limits,
    string RollbackClass,
    string PatchRef,
    string PatchHash,
    IReadOnlyList<LocalPathPreimage> Preimages,
    DateTimeOffset CreatedAt,
    DateTimeOffset ExpiresAt,
    string CandidateHash);

public sealed record ApprovedExecutionSpec(
    string SchemaVersion,
    string SpecId,
    string DeliveryModel,
    DesktopLocalStoreAuthorityRef AuthorityRef,
    string OwnerScopeRef,
    string ProjectId,
    string RunId,
    string ProposalId,
    string ProposalHash,
    string CandidateId,
    string CandidateHash,
    string ApprovalId,
    string ApprovalDecisionHash,
    string PolicyVersion,
    string PolicyHash,
    string WorkspaceTargetHash,
    string MutableInputSetHash,
    NativePatchEngineAudience ExecutorAudience,
    DateTimeOffset IssuedAt,
    DateTimeOffset ExpiresAt,
    string SingleUseNonceHash,
    string SpecHash);

public sealed record SpecConsumptionRecord(
    string SchemaVersion,
    string ConsumptionId,
    string DeliveryModel,
    DesktopLocalStoreAuthorityRef AuthorityRef,
    string SpecId,
    string SpecHash,
    string CandidateHash,
    string SingleUseNonceHash,
    string ExecutorAudienceHash,
    string ExecutionId,
    long AttemptNumber,
    DateTimeOffset ConsumedAt,
    string ConsumptionHash);

public record ContentRef(
    [property: JsonPropertyName("ref")] string Reference,
    string ContentHash,
    long ByteLength,
    string MediaType);

public sealed record ArtifactRef(
    [property: JsonPropertyName("ref")] string Reference,
    string ContentHash,
    long ByteLength,
    string MediaType,
    string ArtifactId,
    string Classification);

public sealed record LocalObservedEffect(
    string ObservedKind,
    string PatchHash);

public sealed record LocalFileChange(
    string RelativePath,
    string Operation,
    string? PreFileIdentityHash,
    string? PreContentHash,
    string? PostFileIdentityHash,
    string? PostContentHash,
    bool Declared);

public sealed record ExecutionResultManifest(
    string SchemaVersion,
    string ManifestId,
    string DeliveryModel,
    string ManifestKind,
    DesktopLocalStoreAuthorityRef AuthorityRef,
    string OwnerScopeRef,
    string ProjectId,
    string RunId,
    string ExecutionId,
    string CandidateId,
    string CandidateHash,
    string SpecId,
    string SpecHash,
    string PolicyHash,
    string ApprovalId,
    string ConsumptionId,
    string ConsumptionHash,
    string WorkspaceTargetHash,
    string ExecutorAudienceHash,
    DateTimeOffset StartedAt,
    DateTimeOffset CompletedAt,
    string Status,
    IReadOnlyList<ContentRef> RedactedLogRefs,
    IReadOnlyList<ArtifactRef> OutputArtifacts,
    string? ValidationSummaryHash,
    string? FailureClassification,
    string InstallationId,
    string HostBuildId,
    string HostBinarySha256,
    string WorkspaceCapabilityId,
    long GrantEpoch,
    string RootIdentityHashBefore,
    string RootIdentityHashAfter,
    string EffectJournalId,
    string PreWriteCheckpointId,
    LocalObservedEffect ObservedEffect,
    IReadOnlyList<LocalFileChange> ChangedFiles,
    string? RollbackPlanId,
    string RecoveryDisposition,
    string ManifestHash);

[JsonPolymorphic(TypeDiscriminatorPropertyName = "actorKind")]
[JsonDerivedType(typeof(UserEvidenceActor), "user")]
[JsonDerivedType(typeof(ServiceEvidenceActor), "service")]
[JsonDerivedType(typeof(DesktopHostEvidenceActor), "desktop_host")]
public abstract record EvidenceActor;

public sealed record UserEvidenceActor(string SubjectId) : EvidenceActor;

public sealed record ServiceEvidenceActor(string ServiceId) : EvidenceActor;

public sealed record DesktopHostEvidenceActor(
    string InstallationId,
    string HostBinarySha256) : EvidenceActor;

public sealed record EvidenceEvent(
    string SchemaVersion,
    string EventId,
    string DeliveryModel,
    DesktopLocalStoreAuthorityRef AuthorityRef,
    string StreamId,
    long Sequence,
    string EventType,
    string OwnerScopeRef,
    string ProjectId,
    string? RunId,
    EvidenceActor Actor,
    string CorrelationId,
    string? CausationId,
    DateTimeOffset OccurredAt,
    string PayloadHash,
    string? PayloadRef,
    string RedactionLevel,
    string RetentionClass,
    string? PreviousEventHash,
    string EventHash);

public sealed record CandidatePointer(string CandidateId);

public sealed record DurableObjectEnvelope(
    string SchemaVersion,
    string ObjectType,
    string ObjectId,
    string DeliveryModel,
    DesktopLocalStoreAuthorityRef AuthorityRef,
    string OwnerScopeRef,
    string ProjectId,
    [property: JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] string? RunId,
    DateTimeOffset CreatedAt,
    string ContentHash);

public sealed record DurableObject(
    DurableObjectEnvelope Envelope,
    CandidatePointer Payload);

[JsonSourceGenerationOptions(
    PropertyNamingPolicy = JsonKnownNamingPolicy.CamelCase,
    UnmappedMemberHandling = JsonUnmappedMemberHandling.Disallow)]
[JsonSerializable(typeof(AuthorityRef))]
[JsonSerializable(typeof(CandidateAction))]
[JsonSerializable(typeof(ApprovedExecutionSpec))]
[JsonSerializable(typeof(SpecConsumptionRecord))]
[JsonSerializable(typeof(ExecutionResultManifest))]
[JsonSerializable(typeof(EvidenceEvent))]
[JsonSerializable(typeof(DurableObject))]
public partial class ContractsJsonContext : JsonSerializerContext
{
}
`;
}

add("generated/typescript/runtime.mjs", typescriptRuntime);
if (!typescriptOnly) {
  add("generated/rust/contracts.rs", getRustContracts());
  add("generated/dotnet/Contracts.g.cs", getDotnetContracts());
}

const schemaNames = (await readdir(path.join(packageRoot, "schemas")))
  .filter((name) => name.endsWith(".schema.json"))
  .sort();
const schemaRecords = [];
const schemas = [];
for (const name of schemaNames) {
  const source = await readFile(path.join(packageRoot, "schemas", name), "utf8");
  const schema = parseStrictJson(source);
  schemas.push(schema);
  schemaRecords.push({
    file: `schemas/${name}`,
    id: schema.$id,
    title: schema.title,
    sha256: sha256(canonicalize(schema)),
    compatibility: "major-locked",
  });
}

function rewriteGeneratorReferences(value) {
  if (Array.isArray(value)) {
    value.forEach(rewriteGeneratorReferences);
    return;
  }
  if (value === null || typeof value !== "object") return;
  for (const [key, member] of Object.entries(value)) {
    if (key === "$ref" && typeof member === "string") {
      value[key] = member.replace("../v1/", "./");
    } else {
      rewriteGeneratorReferences(member);
    }
  }
}

const typescriptSchemaTargets = [
  "approved-execution-spec",
  "authority-ref",
  "candidate-action",
  "contract-error",
  "durable-object",
  "evidence-event",
  "execution-result-manifest",
  "filesystem-capability",
  "package-compatibility",
  "remote-job-handoff",
  "spec-consumption",
];
for (const target of typescriptSchemaTargets) {
  const source = await readFile(
    path.join(packageRoot, "schemas", `${target}.schema.json`),
    "utf8",
  );
  const schema = structuredClone(parseStrictJson(source));
  delete schema.$id;
  rewriteGeneratorReferences(schema);
  const generated = await generateTypescript(schema, target, {
    bannerComment:
      "// @generated by json-schema-to-typescript 15.0.4 via scripts/generate.mjs; DO NOT EDIT.",
    cwd: path.join(packageRoot, "schemas"),
    format: true,
    ignoreMinAndMaxItems: true,
    unreachableDefinitions: false,
  });
  add(`generated/typescript/schema/${target}.ts`, generated);
}

const typescriptBarrel = `// @generated by scripts/generate.mjs; DO NOT EDIT.
export type Sha256 = \`sha256:\${string}\`;
export type ContractId = string;
export type UtcInstant = string;
export type DeliveryModel = "web_managed" | "windows_local";
export interface ContractHashRule {
  readonly excludedFields: readonly string[];
  readonly purpose: string;
  readonly schemaMajor: string;
}
export declare const CONTRACT_EPOCH: 1;
export declare const HASH_RULES: Readonly<Record<string, ContractHashRule>>;

export type {
  SapphirusAuthorityRefV1 as AuthorityRef,
  AzureControlPlaneAuthorityRef,
  DesktopLocalStoreAuthorityRef,
} from "./schema/authority-ref.js";
export type {
  SapphirusCandidateActionV1 as CandidateAction,
  LocalFolderCapabilityTarget,
  NativePatchEngineAudience,
  MutableInputBinding,
  DeclaredWrite,
  LocalPathPreimage,
} from "./schema/candidate-action.js";
export type {
  SapphirusApprovedExecutionSpecV1 as ApprovedExecutionSpec,
} from "./schema/approved-execution-spec.js";
export type {
  SapphirusSpecConsumptionV1 as SpecConsumptionRecord,
} from "./schema/spec-consumption.js";
export type {
  SapphirusExecutionResultManifestV1 as ExecutionResultManifest,
  ContentRef,
  ArtifactRef,
} from "./schema/execution-result-manifest.js";
export type {
  SapphirusEvidenceEventV2 as EvidenceEvent,
} from "./schema/evidence-event.js";
export type {
  SapphirusDurableObjectV1 as DurableObject,
  WebManagedObject,
  WindowsLocalObject,
  CandidatePointer,
} from "./schema/durable-object.js";
export type {
  SapphirusFilesystemCapabilityV1 as FilesystemCapabilitySnapshot,
} from "./schema/filesystem-capability.js";
export type {
  SapphirusContractErrorV1 as ContractError,
} from "./schema/contract-error.js";
export type {
  SapphirusPackageCompatibilityV1 as PackageCompatibility,
} from "./schema/package-compatibility.js";
export type {
  SapphirusRemoteJobHandoffV1 as RemoteJobHandoff,
} from "./schema/remote-job-handoff.js";

import type { SapphirusEvidenceEventV2 } from "./schema/evidence-event.js";
import type { SapphirusExecutionResultManifestV1 } from "./schema/execution-result-manifest.js";
import type { SapphirusDurableObjectV1 } from "./schema/durable-object.js";

export type EvidenceActor = SapphirusEvidenceEventV2["actor"];
export type LocalObservedEffect = SapphirusExecutionResultManifestV1["observedEffect"];
export type LocalFileChange = SapphirusExecutionResultManifestV1["changedFiles"][number];
export type DurableObjectEnvelope = SapphirusDurableObjectV1["envelope"];
`;
add("generated/typescript/contracts.ts", typescriptBarrel);

const ajv = new Ajv2020({
  allErrors: false,
  allowUnionTypes: false,
  strict: true,
  validateFormats: false,
  code: { esm: true, lines: true, source: true },
});
for (const schema of schemas) ajv.addSchema(schema);

const validatorIds = {
  validateAuthorityRef: "https://schemas.sapphirus.dev/v1/authority-ref.schema.json",
  validateCandidateAction: "https://schemas.sapphirus.dev/v1/candidate-action.schema.json",
  validateApprovedExecutionSpec:
    "https://schemas.sapphirus.dev/v1/approved-execution-spec.schema.json",
  validateSpecConsumption: "https://schemas.sapphirus.dev/v1/spec-consumption.schema.json",
  validateExecutionResultManifest:
    "https://schemas.sapphirus.dev/v1/execution-result-manifest.schema.json",
  validateEvidenceEvent: "https://schemas.sapphirus.dev/v2/evidence-event.schema.json",
  validateDurableObject: "https://schemas.sapphirus.dev/v1/durable-object.schema.json",
  validateFilesystemCapability:
    "https://schemas.sapphirus.dev/v1/filesystem-capability.schema.json",
  validateContractError: "https://schemas.sapphirus.dev/v1/contract-error.schema.json",
  validatePackageCompatibility:
    "https://schemas.sapphirus.dev/v1/package-compatibility.schema.json",
  validateRemoteJobHandoff:
    "https://schemas.sapphirus.dev/v1/remote-job-handoff.schema.json",
};
const ajvStandaloneSource = standaloneCode(ajv, validatorIds).replace(
  /^const (\w+) = require\("([^"]+)"\)\.default;$/gm,
  'import $1Module from "$2.js";\nconst $1 = $1Module.default;',
);
if (/\brequire\s*\(/.test(ajvStandaloneSource)) {
  throw new Error("Ajv standalone output contains an unsupported CommonJS runtime import.");
}
const standaloneValidators = `// @generated by Ajv 8 standalone; DO NOT EDIT.\n${ajvStandaloneSource}`;
const validatorDeclarations = `// @generated by scripts/generate.mjs; DO NOT EDIT.
export interface ContractValidationIssue {
  readonly instancePath: string;
  readonly schemaPath: string;
  readonly keyword: string;
  readonly message?: string;
}
export interface StandaloneContractValidator {
  (data: unknown): boolean;
  readonly errors?: readonly ContractValidationIssue[] | null;
}
export declare const validateAuthorityRef: StandaloneContractValidator;
export declare const validateCandidateAction: StandaloneContractValidator;
export declare const validateApprovedExecutionSpec: StandaloneContractValidator;
export declare const validateSpecConsumption: StandaloneContractValidator;
export declare const validateExecutionResultManifest: StandaloneContractValidator;
export declare const validateEvidenceEvent: StandaloneContractValidator;
export declare const validateDurableObject: StandaloneContractValidator;
export declare const validateFilesystemCapability: StandaloneContractValidator;
export declare const validateContractError: StandaloneContractValidator;
export declare const validatePackageCompatibility: StandaloneContractValidator;
export declare const validateRemoteJobHandoff: StandaloneContractValidator;
`;
const unicodeRuntime = `// @generated by scripts/generate.mjs; DO NOT EDIT.
export class UnicodeValidationError extends Error {
  constructor(message) {
    super(message);
    this.name = "UnicodeValidationError";
  }
}

export function assertWellFormedUnicode(value, label = "string") {
  for (let index = 0; index < value.length; index += 1) {
    const codeUnit = value.charCodeAt(index);
    if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
      const next = value.charCodeAt(index + 1);
      if (!(next >= 0xdc00 && next <= 0xdfff)) {
        throw new UnicodeValidationError(
          \`\${label} contains an unpaired high surrogate at UTF-16 index \${index}.\`,
        );
      }
      index += 1;
    } else if (codeUnit >= 0xdc00 && codeUnit <= 0xdfff) {
      throw new UnicodeValidationError(
        \`\${label} contains an unpaired low surrogate at UTF-16 index \${index}.\`,
      );
    }
  }
}
`;
const strictJsonSource = await readFile(
  path.join(packageRoot, "scripts", "lib", "strict-json.mjs"),
  "utf8",
);
const browserStrictJson = `// @generated from scripts/lib/strict-json.mjs; DO NOT EDIT.\n${strictJsonSource.replace(
  'import { assertWellFormedUnicode } from "./canonical-json.mjs";',
  'import { assertWellFormedUnicode } from "./unicode.mjs";',
)}`;
const compatibilitySemanticsSource = await readFile(
  path.join(packageRoot, "scripts", "lib", "compatibility-semantics.mjs"),
  "utf8",
);
const semanticValidationRuntime =
  `// @generated from scripts/lib/compatibility-semantics.mjs; DO NOT EDIT.\n${compatibilitySemanticsSource}`;
const semanticValidationDeclarations = `// @generated by scripts/generate.mjs; DO NOT EDIT.
import type {
  ContractError,
  PackageCompatibility,
  RemoteJobHandoff,
} from "./contracts.js";

export interface SemanticValidationIssue {
  readonly code: string;
  readonly field: string;
  readonly capability?: string;
  readonly conflictingField?: string;
}
export declare function validateContractErrorSemantics(
  document: ContractError,
): readonly SemanticValidationIssue[];
export declare function validatePackageCompatibilitySemantics(
  document: PackageCompatibility,
): readonly SemanticValidationIssue[];
export declare function validateRemoteJobHandoffSemantics(
  document: RemoteJobHandoff,
): readonly SemanticValidationIssue[];
export declare function validateRemoteJobHandoffTransition(
  previous: RemoteJobHandoff,
  current: RemoteJobHandoff,
): readonly SemanticValidationIssue[];
`;
const validationRuntime = `// @generated by scripts/generate.mjs; DO NOT EDIT.
import { parseStrictJson } from "./strict-json.mjs";
import {
  validateApprovedExecutionSpec,
  validateAuthorityRef,
  validateCandidateAction,
  validateContractError,
  validateDurableObject,
  validateEvidenceEvent,
  validateExecutionResultManifest,
  validateFilesystemCapability,
  validatePackageCompatibility,
  validateRemoteJobHandoff,
  validateSpecConsumption,
} from "./validators.mjs";

export const CONTRACT_VALIDATORS = Object.freeze({
  "approved-execution-spec": validateApprovedExecutionSpec,
  "authority-ref": validateAuthorityRef,
  "candidate-action": validateCandidateAction,
  "contract-error": validateContractError,
  "durable-object": validateDurableObject,
  "evidence-event": validateEvidenceEvent,
  "execution-result-manifest": validateExecutionResultManifest,
  "filesystem-capability": validateFilesystemCapability,
  "package-compatibility": validatePackageCompatibility,
  "remote-job-handoff": validateRemoteJobHandoff,
  "spec-consumption": validateSpecConsumption,
});

export class ContractValidationError extends Error {
  constructor(contractKind, issues) {
    super(\`Contract \${contractKind} failed structural validation.\`);
    this.name = "ContractValidationError";
    this.contractKind = contractKind;
    this.issues = Object.freeze(issues);
  }
}

export function parseAndValidateContract(source, contractKind) {
  const validate = CONTRACT_VALIDATORS[contractKind];
  if (validate === undefined) {
    throw new ContractValidationError(contractKind, [
      Object.freeze({
        instancePath: "",
        schemaPath: "",
        keyword: "unknown_contract_kind",
        message: "The contract kind is not supported by this build.",
      }),
    ]);
  }
  if (typeof source !== "string" || source.length > 2_097_152) {
    throw new ContractValidationError(contractKind, [
      Object.freeze({
        instancePath: "",
        schemaPath: "",
        keyword: "source_size",
        message: "The serialized contract exceeds the parser boundary.",
      }),
    ]);
  }
  parseStrictJson(source);
  const value = JSON.parse(source);
  if (!validate(value)) {
    const issues = (validate.errors ?? []).map((issue) =>
      Object.freeze({
        instancePath: issue.instancePath,
        schemaPath: issue.schemaPath,
        keyword: issue.keyword,
        message: issue.message,
      }),
    );
    throw new ContractValidationError(contractKind, issues);
  }
  return value;
}
`;
const validationDeclarations = `// @generated by scripts/generate.mjs; DO NOT EDIT.
import type { StandaloneContractValidator, ContractValidationIssue } from "./validators.mjs";

export type ContractKind =
  | "approved-execution-spec"
  | "authority-ref"
  | "candidate-action"
  | "contract-error"
  | "durable-object"
  | "evidence-event"
  | "execution-result-manifest"
  | "filesystem-capability"
  | "package-compatibility"
  | "remote-job-handoff"
  | "spec-consumption";

export declare const CONTRACT_VALIDATORS: Readonly<Record<ContractKind, StandaloneContractValidator>>;
export declare class ContractValidationError extends Error {
  readonly contractKind: string;
  readonly issues: readonly ContractValidationIssue[];
}
export declare function parseAndValidateContract<T = unknown>(source: string, contractKind: ContractKind): T;
`;
add("generated/typescript/validators.mjs", standaloneValidators);
add("generated/typescript/validators.d.mts", validatorDeclarations);
add("generated/typescript/unicode.mjs", unicodeRuntime);
add("generated/typescript/strict-json.mjs", browserStrictJson);
add("generated/typescript/validation.mjs", validationRuntime);
add("generated/typescript/validation.d.mts", validationDeclarations);
add("generated/typescript/semantic-validation.mjs", semanticValidationRuntime);
add("generated/typescript/semantic-validation.d.mts", semanticValidationDeclarations);

const generatedRecords = [...expectedFiles.entries()]
  .filter(([name]) => name.startsWith("generated/"))
  .sort(([left], [right]) => left.localeCompare(right))
  .map(([file, content]) => ({ file, sha256: sha256(content) }));
const fixtureRecords = [...expectedFiles.entries()]
  .filter(([name]) => name.startsWith("fixtures/"))
  .sort(([left], [right]) => left.localeCompare(right))
  .map(([file, content]) => ({ file, sha256: sha256(content) }));

function treeDigest(records) {
  return sha256(records.map((record) => `${record.file}\0${record.sha256}\n`).join(""));
}

if (!typescriptOnly) {
  const schemaLock = {
  schemaVersion: "sapphirus.schema-lock.v1",
  packageVersion: "0.1.0-beta.1",
  contractEpoch: 1,
  jsonSchemaDraft: "2020-12",
  compatibility: {
    writes: ["v1", "evidence-event/v2"],
    reads: ["v1", "evidence-event/v2"],
    unknownMajors: "fail_closed",
  },
  generators: {
    bootstrap: "scripts/generate.mjs@3",
    typescriptCompiler: "typescript@7.0.2",
    typescript:
      "json-schema-to-typescript@15.0.4; ignoreMinAndMaxItems=true; unreachableDefinitions=false",
    typescriptValidator: "ajv@8.17.1",
    rust: "typify-cli@0.6.1",
    rustValidator: "jsonschema@0.44.1",
    dotnet: "Corvus.Json.CodeGeneration.Cli@5.1.0",
    dotnetRuntime: "Corvus.Text.Json@5.1.0",
  },
  languageCoverage: {
    typescript: {
      status: "generated",
      families: typescriptSchemaTargets,
    },
    rust: {
      status: "deferred_for_new_families",
      deferredFamilies: [
        "contract-error",
        "filesystem-capability",
        "package-compatibility",
        "remote-job-handoff",
      ],
    },
    dotnet: {
      status: "deferred_for_new_families",
      deferredFamilies: [
        "contract-error",
        "filesystem-capability",
        "package-compatibility",
        "remote-job-handoff",
      ],
    },
  },
  schemas: schemaRecords,
  generatedTree: {
    sha256: treeDigest(generatedRecords),
    files: generatedRecords,
  },
  fixtureTree: {
    sha256: treeDigest(fixtureRecords),
    files: fixtureRecords,
  },
  };
  add("schema-lock.json", stableJson(schemaLock));
}

async function listControlledFiles(directory) {
  const absolute = path.join(packageRoot, directory);
  let entries;
  try {
    entries = await readdir(absolute, { withFileTypes: true });
  } catch (error) {
    if (error.code === "ENOENT") return [];
    throw error;
  }

  const files = [];
  for (const entry of entries) {
    const relative = `${directory}/${entry.name}`;
    if (entry.isDirectory()) {
      files.push(...(await listControlledFiles(relative)));
    } else if (entry.isFile()) {
      files.push(relative.replaceAll("\\", "/"));
    }
  }
  return files.sort();
}

if (checkOnly) {
  const mismatches = [];
  for (const [relativePath, expected] of [...expectedFiles.entries()].sort()) {
    try {
      const actual = await readFile(path.join(packageRoot, relativePath), "utf8");
      if (actual !== expected) mismatches.push(`${relativePath}: content differs`);
    } catch (error) {
      if (error.code === "ENOENT") mismatches.push(`${relativePath}: missing`);
      else throw error;
    }
  }

  const controlled = [
    ...(await listControlledFiles("fixtures")),
    ...(await listControlledFiles(
      typescriptOnly ? "generated/typescript" : "generated",
    )),
  ];
  for (const relativePath of controlled) {
    if (!expectedFiles.has(relativePath)) {
      mismatches.push(`${relativePath}: unexpected generated file`);
    }
  }

  if (mismatches.length > 0) {
    throw new Error(`Generated contract output is stale:\n${mismatches.join("\n")}`);
  }
  console.log(
    `contracts: ${typescriptOnly ? "TypeScript-only " : ""}generation check passed (${expectedFiles.size} files)`,
  );
} else {
  let updatedFiles = 0;
  for (const [relativePath, content] of [...expectedFiles.entries()].sort()) {
    const absolutePath = path.join(packageRoot, relativePath);
    try {
      if ((await readFile(absolutePath, "utf8")) === content) continue;
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
    }
    await mkdir(path.dirname(absolutePath), { recursive: true });
    await writeFile(absolutePath, content, "utf8");
    updatedFiles += 1;
  }
  console.log(
    `contracts: generated ${expectedFiles.size} files (${updatedFiles} updated)`,
  );
}
