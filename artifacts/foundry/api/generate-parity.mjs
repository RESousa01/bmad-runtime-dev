import { canonicalize, canonicalHash } from "../../../packages/contracts/scripts/lib/canonical-json.mjs";
import {
  capabilitySurfaceOf,
  SCHEMA_MAJOR,
  PROFILE_HASH_PURPOSE,
  REGISTRY_HASH_PURPOSE,
  CAPABILITY_HASH_PURPOSE,
} from "../../../tools/foundry-profiles/validate-profiles.mjs";

function profile(id, priority, deployment) {
  return {
    schemaVersion: "sapphirus.model-profile.v1",
    profileId: id,
    displayName: `Synthetic ${id}`,
    description: `Synthetic parity fixture for ${id}.`,
    family: "synthetic-family",
    priority,
    enabled: true,
    visible: true,
    deploymentName: deployment,
    expectedModel: {
      publisher: "SyntheticPublisher",
      name: "synthetic-model",
      version: "0000-00-00",
    },
    purposeAllowlist: ["ordinary_chat", "bmad_help"],
    modelRoleAllowlist: ["planner", "assistant"],
    requestModeAllowlist: ["structured_response", "streaming_chat"],
    canonicalOutputSchemaAllowlist: ["sapphirus.bmad-method-help-proposal.v1"],
    supportedModalities: ["text"],
    requiredCapabilities: {
      textInput: true,
      imageInput: false,
      pdfInput: false,
      attachments: false,
      toolCalling: true,
      structuredOutput: true,
      reasoning: true,
      streaming: true,
    },
    limits: {
      contextTokens: 256000,
      maxInputTokens: 220000,
      maxOutputTokens: 32000,
      maxAttachmentBytes: 0,
      maxOutputBytes: 262144,
    },
    variants: [
      {
        variantId: "balanced",
        displayName: "Balanced",
        reasoningEffort: "medium",
        verbosity: "medium",
        maxOutputTokens: 16000,
      },
      {
        variantId: "deep",
        displayName: "Deep",
        reasoningEffort: "high",
        verbosity: "high",
        maxOutputTokens: 32000,
      },
    ],
    retryPolicy: {
      maximumAttempts: 3,
      initialBackoffMilliseconds: 250,
      maximumBackoffMilliseconds: 4000,
    },
    retentionMode: "transient_no_store",
    allowedRegions: ["testregion"],
    qualification: {
      status: "qualified",
      evidenceHash:
        "sha256:1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
      qualifiedAt: "2026-07-28T00:00:00.000Z",
    },
    priceProfile: {
      version: "synthetic-price-2026-07",
      currency: "EUR",
      inputMicrounitsPerThousand: 2750,
      outputMicrounitsPerThousand: 11000,
      reasoningMicrounitsPerThousand: 11000,
      cacheReadMicrounitsPerThousand: 275,
    },
  };
}

const profiles = [
  profile("synthetic-alpha", 10, "synthetic-deployment-alpha"),
  profile("synthetic-beta", 20, "synthetic-deployment-beta"),
];

const planned = profiles.map((entry) => {
  const hash = canonicalHash({
    purpose: PROFILE_HASH_PURPOSE,
    schemaMajor: SCHEMA_MAJOR,
    value: entry,
  });
  return { profileId: entry.profileId, valueHash: hash.serializedHash, canonicalJson: hash.canonicalJson };
});

const registryVersion = 7;
const registryHash = canonicalHash({
  purpose: REGISTRY_HASH_PURPOSE,
  schemaMajor: SCHEMA_MAJOR,
  value: {
    registryVersion,
    profiles: planned.map((entry) => ({
      profileId: entry.profileId,
      profileHash: entry.valueHash,
    })),
  },
}).serializedHash;

const capabilityHash = canonicalHash({
  purpose: CAPABILITY_HASH_PURPOSE,
  schemaMajor: SCHEMA_MAJOR,
  value: capabilitySurfaceOf(profiles),
}).serializedHash;

const registryValue = canonicalize({
  schemaVersion: "forge.model-profile-registry.v1",
  registryVersion,
  registryHash,
  capabilityHash,
  profileIds: planned.map((entry) => entry.profileId),
});

const out = {
  profileValues: Object.fromEntries(planned.map((e) => [e.profileId, e.canonicalJson])),
  profileHashes: Object.fromEntries(planned.map((e) => [e.profileId, e.valueHash])),
  registryVersion,
  registryHash,
  capabilityHash,
  registryValue,
};
console.log(JSON.stringify(out, null, 2));
