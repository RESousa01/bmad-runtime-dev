import { createHash } from "node:crypto";
import { mkdir, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import Ajv2020 from "ajv/dist/2020.js";
import standaloneCode from "ajv/dist/standalone/index.js";
import { compile as generateTypescript } from "json-schema-to-typescript";
import { canonicalHash, canonicalize } from "./lib/canonical-json.mjs";
import { buildBmadFixtureSet } from "./lib/bmad-fixtures.mjs";
import {
  assertContainedPhysicalDirectory,
  listContainedDirectRegularFiles,
  listContainedRegularFiles,
  readContainedUtf8File,
  verifyExpectedContractFiles,
} from "./lib/controlled-contract-io.mjs";
import { sealDocument, sealDurableObject } from "./lib/semantics.mjs";
import { parseStrictJson } from "./lib/strict-json.mjs";
import {
  buildInternalBundle,
  generateNativeTrees,
  readCommittedTree,
  repositoryRoot,
  toolLockDigest,
  transformSchemaTree,
  treeDigest as nativeTreeDigest,
  treeRecords as nativeTreeRecords,
} from "./lib/native-codegen.mjs";

const packageRoot = fileURLToPath(new URL("../", import.meta.url));
await assertContainedPhysicalDirectory(repositoryRoot, packageRoot, "contract package root");
const checkOnly = process.argv.includes("--check");
const typescriptOnly = process.argv.includes("--typescript-only");
const dryRun = process.argv.includes("--dry-run");
const unknownArguments = process.argv
  .slice(2)
  .filter((argument) =>
    argument !== "--check" && argument !== "--typescript-only" && argument !== "--dry-run");
if (unknownArguments.length > 0) {
  throw new Error(`Unsupported generator arguments: ${unknownArguments.join(", ")}`);
}
if (typescriptOnly && !checkOnly) {
  throw new Error("--typescript-only is a read-only verification mode and requires --check.");
}
if (dryRun && checkOnly) {
  throw new Error("--dry-run and --check are mutually exclusive generation modes.");
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

const bmadFixtureSet = buildBmadFixtureSet();
for (const [relativePath, content] of bmadFixtureSet.files) {
  add(relativePath, content);
}

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
catalog.push(...bmadFixtureSet.catalogEntries);
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
    source: "external-reference-note-99/section-14.2",
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
  "sapphirus.bmad-package-descriptor.v1": Object.freeze({
    excludedFields: Object.freeze(["descriptorHash"]),
    purpose: "bmad-package-descriptor",
    schemaMajor: "v1",
  }),
  "sapphirus.bmad-capability-catalog.v1": Object.freeze({
    excludedFields: Object.freeze(["catalogHash"]),
    purpose: "bmad-capability-catalog",
    schemaMajor: "v1",
  }),
  "sapphirus.bmad-method-checkpoint.v1": Object.freeze({
    excludedFields: Object.freeze(["checkpointHash"]),
    purpose: "bmad-method-checkpoint",
    schemaMajor: "v1",
  }),
  "sapphirus.bmad-method-session.v1": Object.freeze({
    excludedFields: Object.freeze(["contentHash"]),
    purpose: "contract-object",
    schemaMajor: "v1",
  }),
  "sapphirus.bmad-builder-revision.v1": Object.freeze({
    excludedFields: Object.freeze(["revisionHash"]),
    purpose: "bmad-builder-revision",
    schemaMajor: "v1",
  }),
  "sapphirus.bmad-builder-analysis.v1": Object.freeze({
    excludedFields: Object.freeze(["analysisHash"]),
    purpose: "bmad-builder-analysis",
    schemaMajor: "v1",
  }),
  "sapphirus.bmad-validation-report.v1": Object.freeze({
    excludedFields: Object.freeze(["reportHash"]),
    purpose: "bmad-validation-report",
    schemaMajor: "v1",
  }),
});
`;

add("generated/typescript/runtime.mjs", typescriptRuntime);
let nativeGeneration = null;
if (!typescriptOnly) {
  nativeGeneration = await generateNativeTrees("production");
  for (const [relativePath, source] of nativeGeneration.rust) {
    add(`generated/rust/${relativePath}`, source);
  }
  for (const [relativePath, source] of nativeGeneration.dotnet) {
    add(`generated/dotnet/${relativePath}`, source);
  }
}

const schemaDirectory = path.join(packageRoot, "schemas");
const schemaNames = (await listContainedDirectRegularFiles(
  packageRoot,
  schemaDirectory,
  "contract schema tree",
))
  .filter((name) => name.endsWith(".schema.json"))
  .sort();
const schemaRecords = [];
const schemas = [];
for (const name of schemaNames) {
  const source = await readContainedUtf8File(
    packageRoot,
    path.join(schemaDirectory, name),
    `contract schema ${name}`,
  );
  const schema = parseStrictJson(source);
  schemas.push(schema);
  schemaRecords.push({
    file: `schemas/${name}`,
    id: schema.$id,
    title: schema.title,
    sha256: sha256(canonicalize(schema)),
    canonicalSha256: sha256(canonicalize(schema)),
    sourceSha256: sha256(source),
    role: name === "common.schema.json" ? "dependency" : "root",
    compatibility: "major-locked",
  });
}

function rewriteGeneratorReferences(value, sourceFile) {
  return transformSchemaTree(value, {
    documentRoot: true,
    retainDocumentKeywords: true,
    rewriteReference: (reference) => reference.replace("../v1/", "./"),
    sourceFile,
  });
}

const typescriptSchemaTargets = [
  "approved-execution-spec",
  "authority-ref",
  "bmad-builder-authoring",
  "bmad-capability-catalog",
  "bmad-method-session",
  "bmad-package-descriptor",
  "bmad-validation-report",
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
const typescriptGeneratorRunB = new Map();
for (const target of typescriptSchemaTargets) {
  const source = await readContainedUtf8File(
    packageRoot,
    path.join(schemaDirectory, `${target}.schema.json`),
    `TypeScript generator schema ${target}.schema.json`,
  );
  const generateOne = async () => {
    let schema = structuredClone(parseStrictJson(source));
    delete schema.$id;
    schema = rewriteGeneratorReferences(schema, `${target}.schema.json`);
    return generateTypescript(schema, target, {
      bannerComment:
        "// @generated by json-schema-to-typescript 15.0.4 via scripts/generate.mjs; DO NOT EDIT.",
      cwd: path.join(packageRoot, "schemas"),
      format: true,
      ignoreMinAndMaxItems: true,
      unreachableDefinitions: false,
    });
  };
  const [generatedRunA, generatedRunB] = await Promise.all([generateOne(), generateOne()]);
  const relativePath = `generated/typescript/schema/${target}.ts`;
  add(relativePath, generatedRunA);
  typescriptGeneratorRunB.set(relativePath, generatedRunB);
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
  SapphirusBmadBuilderAuthoringV1 as BuilderAuthoringObject,
  BuilderDraft,
  BuilderDraftRevision,
  BuilderAnalysisRun,
  BuilderProposedFile,
  BuilderProposedFileSet,
} from "./schema/bmad-builder-authoring.js";
export type {
  SapphirusBmadCapabilityCatalogV1 as BmadCapabilityCatalog,
  BmadCapabilityKey,
  InstalledSkillRecord,
  BmadHelpActionRecord,
  BmadAgentRoster,
  BmadAgentRecord,
  BmadAgentMenuItem,
  BmadAgentMenuTarget,
} from "./schema/bmad-capability-catalog.js";
export type {
  SapphirusBmadMethodSessionV1 as MethodSession,
  MethodAgentBinding,
  MethodContextLedger,
  BmadContextDecisionConsumption,
  MethodCheckpoint,
} from "./schema/bmad-method-session.js";
export type {
  SapphirusBmadPackageDescriptorV1 as BmadPackageDescriptor,
  BmadSourceIdentity,
  BmadInstructionProjection,
  BmadSkillDescriptor,
  SkillExecutionProfile,
  BmadConfigGraphDescriptor,
  BmadConfigResolution,
} from "./schema/bmad-package-descriptor.js";
export type {
  SapphirusBmadValidationReportV1 as BmadValidationReport,
  BmadValidationProfile,
  BmadValidationFinding,
  BmadValidationDependency,
  BmadValidationDisposition,
} from "./schema/bmad-validation-report.js";
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

const validatorIds = {
  validateAuthorityRef: "https://schemas.sapphirus.dev/v1/authority-ref.schema.json",
  validateBmadBuilderAuthoring:
    "https://schemas.sapphirus.dev/v1/bmad-builder-authoring.schema.json",
  validateBmadCapabilityCatalog:
    "https://schemas.sapphirus.dev/v1/bmad-capability-catalog.schema.json",
  validateBmadMethodSession:
    "https://schemas.sapphirus.dev/v1/bmad-method-session.schema.json",
  validateBmadPackageDescriptor:
    "https://schemas.sapphirus.dev/v1/bmad-package-descriptor.schema.json",
  validateBmadValidationReport:
    "https://schemas.sapphirus.dev/v1/bmad-validation-report.schema.json",
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
function generateStandaloneValidators() {
  const ajv = new Ajv2020({
    allErrors: false,
    allowUnionTypes: false,
    strict: true,
    validateFormats: false,
    code: { esm: true, lines: true, source: true },
  });
  for (const schema of schemas) ajv.addSchema(structuredClone(schema));
  const ajvStandaloneSource = standaloneCode(ajv, validatorIds).replace(
    /^const (\w+) = require\("([^"]+)"\)\.default;$/gm,
    'import $1Module from "$2.js";\nconst $1 = $1Module.default;',
  );
  if (/\brequire\s*\(/.test(ajvStandaloneSource)) {
    throw new Error("Ajv standalone output contains an unsupported CommonJS runtime import.");
  }
  return `// @generated by Ajv 8 standalone; DO NOT EDIT.\n${ajvStandaloneSource}`;
}
const standaloneValidators = generateStandaloneValidators();
const standaloneValidatorsRunB = generateStandaloneValidators();
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
export declare const validateBmadBuilderAuthoring: StandaloneContractValidator;
export declare const validateBmadCapabilityCatalog: StandaloneContractValidator;
export declare const validateBmadMethodSession: StandaloneContractValidator;
export declare const validateBmadPackageDescriptor: StandaloneContractValidator;
export declare const validateBmadValidationReport: StandaloneContractValidator;
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
const strictJsonSource = await readContainedUtf8File(
  packageRoot,
  path.join(packageRoot, "scripts", "lib", "strict-json.mjs"),
  "strict JSON generator source",
);
const browserStrictJson = `// @generated from scripts/lib/strict-json.mjs; DO NOT EDIT.\n${strictJsonSource.replace(
  'import { assertWellFormedUnicode } from "./canonical-json.mjs";',
  'import { assertWellFormedUnicode } from "./unicode.mjs";',
)}`;
const compatibilitySemanticsSource = await readContainedUtf8File(
  packageRoot,
  path.join(packageRoot, "scripts", "lib", "compatibility-semantics.mjs"),
  "compatibility semantics generator source",
);
const bmadSemanticsSource = await readContainedUtf8File(
  packageRoot,
  path.join(packageRoot, "scripts", "lib", "bmad-semantics.mjs"),
  "BMAD semantics generator source",
);
const semanticValidationRuntime =
  `// @generated from handwritten semantic validators; DO NOT EDIT.\n${compatibilitySemanticsSource}\n${bmadSemanticsSource}`;
const semanticValidationDeclarations = `// @generated by scripts/generate.mjs; DO NOT EDIT.
import type {
  BmadCapabilityCatalog,
  BmadPackageDescriptor,
  BuilderAuthoringObject,
  ContractError,
  MethodSession,
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
export interface BmadSemanticContext {
  readonly descriptor?: BmadPackageDescriptor;
  readonly catalog?: BmadCapabilityCatalog;
}
export declare function validateBmadSemantics(
  document: BmadPackageDescriptor | BmadCapabilityCatalog | MethodSession | BuilderAuthoringObject,
  context?: BmadSemanticContext,
): readonly SemanticValidationIssue[];
export declare function bmadContextDecisionUniquenessKey(
  document: Record<string, unknown>,
): string;
`;
const validationRuntime = `// @generated by scripts/generate.mjs; DO NOT EDIT.
import { parseStrictJson } from "./strict-json.mjs";
import {
  validateApprovedExecutionSpec,
  validateAuthorityRef,
  validateBmadBuilderAuthoring,
  validateBmadCapabilityCatalog,
  validateBmadMethodSession,
  validateBmadPackageDescriptor,
  validateBmadValidationReport,
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
  "bmad-builder-authoring": validateBmadBuilderAuthoring,
  "bmad-capability-catalog": validateBmadCapabilityCatalog,
  "bmad-method-session": validateBmadMethodSession,
  "bmad-package-descriptor": validateBmadPackageDescriptor,
  "bmad-validation-report": validateBmadValidationReport,
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
  if (typeof source !== "string") {
    throw new ContractValidationError(contractKind, [
      Object.freeze({
        instancePath: "",
        schemaPath: "",
        keyword: "source_size",
        message: "The serialized contract exceeds the parser boundary.",
      }),
    ]);
  }
  parseStrictJson(source, {
    maxBytes: 2_097_152,
    maxContainerDepth: 16,
  });
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
  | "bmad-builder-authoring"
  | "bmad-capability-catalog"
  | "bmad-method-session"
  | "bmad-package-descriptor"
  | "bmad-validation-report"
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

const productionTypescriptRunA = new Map(
  [...expectedFiles.entries()].filter(([file]) => file.startsWith("generated/typescript/")),
);
const productionTypescriptRunB = new Map(productionTypescriptRunA);
for (const [file, source] of typescriptGeneratorRunB) {
  productionTypescriptRunB.set(file, source);
}
productionTypescriptRunB.set("generated/typescript/validators.mjs", standaloneValidatorsRunB);
const typescriptRunAPaths = [...productionTypescriptRunA.keys()].sort();
const typescriptRunBPaths = [...productionTypescriptRunB.keys()].sort();
if (JSON.stringify(typescriptRunAPaths) !== JSON.stringify(typescriptRunBPaths)) {
  throw new Error(
    "CONTRACT_GENERATOR_NONDETERMINISTIC: production TypeScript inventory changed across clean runs.",
  );
}
for (const file of typescriptRunAPaths) {
  if (productionTypescriptRunA.get(file) !== productionTypescriptRunB.get(file)) {
    throw new Error(
      `CONTRACT_GENERATOR_NONDETERMINISTIC: production TypeScript output changed across clean runs: ${file}`,
    );
  }
}

const generatedRecords = [...expectedFiles.entries()]
  .filter(([name]) => name.startsWith("generated/"))
  .sort(([left], [right]) => (left < right ? -1 : left > right ? 1 : 0))
  .map(([file, content]) => ({ file, sha256: sha256(content) }));
const fixtureRecords = [...expectedFiles.entries()]
  .filter(([name]) => name.startsWith("fixtures/"))
  .sort(([left], [right]) => (left < right ? -1 : left > right ? 1 : 0))
  .map(([file, content]) => ({ file, sha256: sha256(content) }));

function treeDigest(records) {
  return sha256(records.map((record) => `${record.file}\0${record.sha256}\n`).join(""));
}

if (!typescriptOnly) {
  const selectExpectedTree = (prefix) => new Map(
    [...expectedFiles.entries()]
      .filter(([file]) => file.startsWith(`${prefix}/`))
      .map(([file, source]) => [file.slice(prefix.length + 1), source]),
  );
  const qualificationGeneratedRoot = path.join(
    repositoryRoot,
    "tests",
    "generator-qualification",
    "generated",
  );
  const qualificationGenerated = await readCommittedTree(qualificationGeneratedRoot);
  const selectQualificationTree = (language) => new Map(
    [...qualificationGenerated.entries()]
      .filter(([file]) => file.startsWith(`${language}/`))
      .map(([file, source]) => [file.slice(language.length + 1), source]),
  );
  const qualificationTypescript = selectQualificationTree("typescript");
  const qualificationRust = selectQualificationTree("rust");
  const qualificationDotnet = selectQualificationTree("dotnet");
  if (qualificationTypescript.size === 0 || qualificationRust.size === 0 || qualificationDotnet.size === 0) {
    throw new Error(
      "CONTRACT_LANGUAGE_PARITY_FAILED: run qualify-generators.mjs before production generation.",
    );
  }
  const qualificationFixtureRoot = path.join(
    repositoryRoot,
    "tests",
    "generator-qualification",
    "fixtures",
  );
  const qualificationFixtures = await readCommittedTree(qualificationFixtureRoot);
  const qualificationCatalogSource = await readContainedUtf8File(
    repositoryRoot,
    path.join(repositoryRoot, "tests", "generator-qualification", "catalog.json"),
    "generator qualification catalog",
  );
  const qualificationCatalog = parseStrictJson(qualificationCatalogSource);
  const qualificationBundle = await buildInternalBundle(nativeGeneration.lock, "qualification");
  const qualificationSchemaRecords = [];
  for (const file of [qualificationCatalog.rootSchema, ...qualificationCatalog.resources]) {
    const source = await readContainedUtf8File(
      repositoryRoot,
      path.join(repositoryRoot, "tests", "generator-qualification", file),
      `generator qualification schema ${file}`,
    );
    qualificationSchemaRecords.push({
      file: `tests/generator-qualification/${file}`,
      sourceSha256: sha256(source),
      canonicalSha256: sha256(canonicalize(parseStrictJson(source))),
    });
  }
  const typescriptTree = nativeTreeRecords(
    selectExpectedTree("generated/typescript"),
    "generated/typescript",
  );
  const rustTree = nativeTreeRecords(selectExpectedTree("generated/rust"), "generated/rust");
  const dotnetTree = nativeTreeRecords(selectExpectedTree("generated/dotnet"), "generated/dotnet");
  const qualificationTypescriptTree = nativeTreeRecords(
    qualificationTypescript,
    "tests/generator-qualification/generated/typescript",
  );
  const qualificationRustTree = nativeTreeRecords(
    qualificationRust,
    "tests/generator-qualification/generated/rust",
  );
  const qualificationDotnetTree = nativeTreeRecords(
    qualificationDotnet,
    "tests/generator-qualification/generated/dotnet",
  );
  const qualificationFixtureRecords = nativeTreeRecords(
    qualificationFixtures,
    "tests/generator-qualification/fixtures",
  );
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
    bootstrap: "scripts/generate.mjs@4",
    typescriptCompiler: "typescript@7.0.2",
    typescript:
      "json-schema-to-typescript@15.0.4; ignoreMinAndMaxItems=true; unreachableDefinitions=false",
    typescriptValidator: "ajv@8.17.1",
    rust: "cargo-typify@0.6.1",
    rustValidator: "jsonschema@0.44.1",
    dotnet: "Corvus.Json.Cli@5.1.0",
    dotnetRuntime: "Corvus.Text.Json@5.1.0",
  },
  toolLockSha256: await toolLockDigest(),
  generationConfigSha256: nativeGeneration.configDigest,
  generationBundles: {
    production: {
      id: nativeGeneration.lock.sourceSet.production.bundleId,
      sha256: sha256(nativeGeneration.bundle.source),
    },
    qualification: {
      id: nativeGeneration.lock.sourceSet.qualification.bundleId,
      sha256: sha256(qualificationBundle.source),
    },
  },
  toolchain: {
    node: "24.18.0",
    pnpm: "11.12.0",
    typescript: "7.0.2",
    jsonSchemaToTypescript: "15.0.4",
    ajv: "8.17.1",
    rust: "1.97.0",
    cargoTypify: "0.6.1",
    rustJsonschema: "0.44.1",
    dotnetSdk: "10.0.301",
    corvusJsonCli: "5.1.0",
    corvusTextJson: "5.1.0",
  },
  languageCoverage: {
    typescript: {
      status: "generated",
      families: typescriptSchemaTargets,
      dependencies: ["common"],
    },
    rust: {
      status: "generated",
      families: typescriptSchemaTargets,
      dependencies: ["common"],
    },
    dotnet: {
      status: "generated",
      families: typescriptSchemaTargets,
      dependencies: ["common"],
    },
  },
  schemas: schemaRecords,
  schemaTree: {
    sha256: sha256(schemaRecords.map((record) =>
      `${record.file}\0${record.sourceSha256}\0${record.canonicalSha256}\n`).join("")),
    files: schemaRecords.map(({ file, sourceSha256, canonicalSha256, role }) => ({
      file, sourceSha256, canonicalSha256, role,
    })),
  },
  qualification: {
    catalog: {
      file: "tests/generator-qualification/catalog.json",
      sha256: sha256(qualificationCatalogSource),
    },
    schemas: qualificationSchemaRecords,
    parserLimits: qualificationCatalog.parserLimits,
    rootType: "GeneratorQualification",
    dotnetNamespace: "Sapphirus.GeneratorQualification.Generated",
    bundleSha256: sha256(qualificationBundle.source),
  },
  generatedTrees: {
    production: {
      typescript: { sha256: nativeTreeDigest(typescriptTree), files: typescriptTree },
      rust: { sha256: nativeTreeDigest(rustTree), files: rustTree },
      dotnet: { sha256: nativeTreeDigest(dotnetTree), files: dotnetTree },
    },
    qualification: {
      typescript: {
        sha256: nativeTreeDigest(qualificationTypescriptTree),
        files: qualificationTypescriptTree,
      },
      rust: { sha256: nativeTreeDigest(qualificationRustTree), files: qualificationRustTree },
      dotnet: { sha256: nativeTreeDigest(qualificationDotnetTree), files: qualificationDotnetTree },
    },
  },
  fixtureTrees: {
    production: { sha256: treeDigest(fixtureRecords), files: fixtureRecords },
    qualification: {
      sha256: nativeTreeDigest(qualificationFixtureRecords),
      files: qualificationFixtureRecords,
    },
  },
  bootstrapLocks: Object.values(nativeGeneration.lock.bootstrapLocks).map((record) => ({
    file: record.file,
    sha256: `sha256:${record.sha256}`,
    status: record.status,
  })),
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

if (dryRun) {
  console.log(`contracts: generation dry-run passed (${expectedFiles.size} files)`);
} else if (checkOnly) {
  const mismatches = await verifyExpectedContractFiles({
    packageRoot,
    expectedFiles,
    controlledDirectories: [
      "fixtures",
      typescriptOnly ? "generated/typescript" : "generated",
    ],
  });

  if (mismatches.length > 0) {
    throw new Error(`Generated contract output is stale:\n${mismatches.join("\n")}`);
  }
  console.log(
    `contracts: ${typescriptOnly ? "TypeScript-only " : ""}generation check passed (${expectedFiles.size} files)`,
  );
} else {
  let updatedFiles = 0;
  const controlled = [
    ...(await listContainedRegularFiles(
      packageRoot,
      path.join(packageRoot, "fixtures"),
      "controlled contract fixture tree",
      { allowMissing: true },
    )).map((file) => `fixtures/${file}`),
    ...(await listContainedRegularFiles(
      packageRoot,
      path.join(packageRoot, "generated"),
      "controlled generated contract tree",
      { allowMissing: true },
    )).map((file) => `generated/${file}`),
  ];
  for (const relativePath of controlled) {
    if (!expectedFiles.has(relativePath)) {
      const absolutePath = path.resolve(packageRoot, relativePath);
      const relativeToPackage = path.relative(packageRoot, absolutePath);
      if (relativeToPackage.startsWith(`..${path.sep}`) || path.isAbsolute(relativeToPackage)) {
        throw new Error(`Refusing to remove generated path outside the package: ${relativePath}`);
      }
      await rm(absolutePath, { force: true });
      updatedFiles += 1;
    }
  }
  for (const [relativePath, content] of [...expectedFiles.entries()].sort()) {
    const absolutePath = path.join(packageRoot, relativePath);
    const existing = await readContainedUtf8File(
      packageRoot,
      absolutePath,
      `generated contract destination ${relativePath}`,
      { allowMissing: true },
    );
    if (existing === content) continue;
    await mkdir(path.dirname(absolutePath), { recursive: true });
    await writeFile(absolutePath, content, "utf8");
    updatedFiles += 1;
  }
  console.log(
    `contracts: generated ${expectedFiles.size} files (${updatedFiles} updated)`,
  );
}
