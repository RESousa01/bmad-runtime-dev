#!/usr/bin/env node
/**
 * Deterministic validator for operator-authored model profile documents.
 *
 * The document is checked against tools/foundry-profiles/profile.schema.json,
 * cross-checked against a Foundry deployment inventory, and reduced to stable
 * canonical hashes. Nothing here contacts Azure: the deployment inventory is a
 * file produced by tools/foundry-smoke/check-prerequisites.ps1, and the
 * optional current-state file is a redacted hash summary produced by
 * tools/foundry-profiles/publish-profiles.ps1.
 *
 * Exit codes
 *   0  the document is publishable against the supplied inventory
 *   1  usage, I/O, or malformed-input failure
 *   2  the document was read but refused; every reason is in report.violations
 */

import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import {
  canonicalHash,
  canonicalize,
} from "../../packages/contracts/scripts/lib/canonical-json.mjs";
import { parseStrictJson } from "../../packages/contracts/scripts/lib/strict-json.mjs";
import {
  loadSchemaRegistry,
  validateSchemaDocument,
} from "../../packages/contracts/scripts/lib/schema-validator.mjs";

const toolDirectory = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(toolDirectory, "..", "..");

export const DOCUMENT_SCHEMA_FILE = "profile.schema.json";
export const KEY_PREFIX = "desktop-support:model-profile:";
export const REGISTRY_VERSION_KEY = "desktop-support:model-profile-registry:version";
export const PROFILE_HASH_PURPOSE = "model-profile";
export const REGISTRY_HASH_PURPOSE = "model-profile-registry";
export const CAPABILITY_HASH_PURPOSE = "model-profile-capability";
export const SCHEMA_MAJOR = "v1";

const MAXIMUM_DOCUMENT_BYTES = 1024 * 1024;
const MAXIMUM_INVENTORY_BYTES = 4 * 1024 * 1024;
const MAXIMUM_CONTAINER_DEPTH = 32;

/**
 * Azure App Configuration rejects a key-value whose key, label, and value
 * exceed its per-item budget. The bound is enforced here so an oversized
 * profile is refused before any write is attempted rather than failing
 * halfway through a set.
 */
const MAXIMUM_STORE_VALUE_BYTES = 10000;

const POSITIVE_LIMIT_FIELDS = [
  "contextTokens",
  "maxInputTokens",
  "maxOutputTokens",
  "maxOutputBytes",
];

class UsageError extends Error {
  constructor(message) {
    super(message);
    this.name = "UsageError";
  }
}

function parseArguments(argv) {
  const options = {
    profiles: null,
    deployments: null,
    current: null,
    out: null,
    emitPlan: null,
    quiet: false,
  };
  const flags = new Map([
    ["--profiles", "profiles"],
    ["--deployments", "deployments"],
    ["--current", "current"],
    ["--out", "out"],
    ["--emit-plan", "emitPlan"],
  ]);

  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (token === "--quiet") {
      options.quiet = true;
      continue;
    }
    const field = flags.get(token);
    if (field === undefined) {
      throw new UsageError(`Unknown argument ${token}.`);
    }
    const value = argv[index + 1];
    if (value === undefined || value.startsWith("--")) {
      throw new UsageError(`${token} requires a path.`);
    }
    if (options[field] !== null) {
      throw new UsageError(`${token} was supplied more than once.`);
    }
    options[field] = value;
    index += 1;
  }

  if (options.profiles === null) throw new UsageError("--profiles is required.");
  if (options.deployments === null) throw new UsageError("--deployments is required.");
  return options;
}

function readJsonFile(filePath, label, maximumBytes) {
  let source;
  try {
    source = readFileSync(filePath, "utf8");
  } catch (error) {
    throw new UsageError(`Cannot read the ${label} file: ${error.code ?? error.message}.`);
  }
  try {
    return parseStrictJson(source, {
      maxBytes: maximumBytes,
      maxContainerDepth: MAXIMUM_CONTAINER_DEPTH,
    });
  } catch (error) {
    throw new UsageError(`The ${label} file is not strict JSON: ${error.message}`);
  }
}

function isPlainObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function repositoryRelative(filePath) {
  const resolved = path.resolve(filePath);
  const relative = path.relative(repositoryRoot, resolved);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    return path.basename(resolved);
  }
  return relative.split(path.sep).join("/");
}

/**
 * Reads either shape of deployment inventory this repository produces.
 *
 * `forge.foundry-prerequisites.v1` and `.v2` are reports written by
 * tools/foundry-smoke/check-prerequisites.ps1. It carries a readiness verdict
 * and a redacted projection in which capabilities are reduced to sorted key
 * names, so capability narrowing cannot be checked from it.
 *
 * `sapphirus.foundry-deployment.v1` is the normalized contract inventory. It
 * carries capability booleans, so narrowing is checked when it is supplied.
 */
export function readDeploymentInventory(document) {
  if (!isPlainObject(document)) {
    return {
      recognized: false,
      ready: false,
      blockers: ["deployment_inventory_shape_unrecognized"],
      capabilitiesKnown: false,
      deployments: new Map(),
      source: "unknown",
    };
  }

  const schemaVersion = document.schemaVersion;
  if (
    schemaVersion === "forge.foundry-prerequisites.v1" ||
    schemaVersion === "forge.foundry-prerequisites.v2"
  ) {
    const inventory = isPlainObject(document.deploymentInventory)
      ? document.deploymentInventory
      : null;
    const records = Array.isArray(inventory?.deployments) ? inventory.deployments : [];
    const blockers = Array.isArray(document.blockers)
      ? document.blockers.filter((entry) => typeof entry === "string")
      : [];
    return {
      recognized: true,
      ready: document.ready === true,
      blockers,
      capabilitiesKnown: false,
      deployments: indexDeployments(records),
      source: schemaVersion,
    };
  }

  if (schemaVersion === "sapphirus.foundry-deployment.v1") {
    const records = Array.isArray(document.deployments) ? document.deployments : [];
    return {
      recognized: true,
      // A normalized contract inventory carries no readiness verdict; being a
      // valid snapshot of what exists is the verdict. An empty snapshot still
      // fails every deployment cross-check below.
      ready: records.length > 0,
      blockers: records.length > 0 ? [] : ["deployment_inventory_empty"],
      capabilitiesKnown: true,
      deployments: indexDeployments(records),
      source: "sapphirus.foundry-deployment.v1",
    };
  }

  return {
    recognized: false,
    ready: false,
    blockers: ["deployment_inventory_shape_unrecognized"],
    capabilitiesKnown: false,
    deployments: new Map(),
    source: "unknown",
  };
}

function indexDeployments(records) {
  const deployments = new Map();
  for (const record of records) {
    if (!isPlainObject(record)) continue;
    const name = record.deploymentName;
    if (typeof name !== "string" || name.length === 0) continue;
    deployments.set(name, {
      deploymentName: name,
      modelPublisher: typeof record.modelPublisher === "string" ? record.modelPublisher : null,
      modelName: typeof record.modelName === "string" ? record.modelName : null,
      modelVersion: typeof record.modelVersion === "string" ? record.modelVersion : null,
      capabilities: isPlainObject(record.capabilities) ? record.capabilities : null,
    });
  }
  return deployments;
}

/**
 * A deployment name reaches an Azure request path and an App Configuration
 * value. Anything outside the canonical contract alphabet is refused here
 * rather than being escaped and forwarded.
 */
export function assessDeploymentNameSafety(name) {
  if (typeof name !== "string") return "deployment_name_unsafe";
  if (!/^[A-Za-z0-9][A-Za-z0-9_-]{0,126}[A-Za-z0-9]$/u.test(name)) {
    return "deployment_name_unsafe";
  }
  if (name !== name.normalize("NFC")) return "deployment_name_unsafe";
  if (/[^\x20-\x7e]/u.test(name)) return "deployment_name_unsafe";
  return null;
}

function isDegenerateDigest(value) {
  const match = /^sha256:([0-9a-f]{64})$/u.exec(value ?? "");
  if (match === null) return true;
  return new Set(match[1]).size === 1;
}

function normalizeDisplayName(value) {
  return value.trim().replace(/\s+/gu, " ").toLowerCase();
}

function profileHashOf(profile) {
  return canonicalHash({
    purpose: PROFILE_HASH_PURPOSE,
    schemaMajor: SCHEMA_MAJOR,
    value: profile,
  });
}

/**
 * The capability surface is the part of the registry that decides what the
 * organization permits, as opposed to what it costs or how large a response
 * may be. Publishing a price change therefore moves the registry hash but not
 * the capability hash, and a reader can tell the two apart without diffing.
 */
export function capabilitySurfaceOf(profiles) {
  return [...profiles]
    .map((profile) => ({
      profileId: profile.profileId,
      enabled: profile.enabled,
      visible: profile.visible,
      deploymentName: profile.deploymentName,
      expectedModel: {
        publisher: profile.expectedModel.publisher,
        name: profile.expectedModel.name,
        version: profile.expectedModel.version,
      },
      purposeAllowlist: [...profile.purposeAllowlist].sort(),
      modelRoleAllowlist: [...profile.modelRoleAllowlist].sort(),
      requestModeAllowlist: [...profile.requestModeAllowlist].sort(),
      canonicalOutputSchemaAllowlist: [...profile.canonicalOutputSchemaAllowlist].sort(),
      supportedModalities: [...profile.supportedModalities].sort(),
      requiredCapabilities: { ...profile.requiredCapabilities },
      retentionMode: profile.retentionMode,
      allowedRegions: [...profile.allowedRegions].sort(),
      qualificationStatus: profile.qualification.status,
    }))
    .sort((left, right) => (left.profileId < right.profileId ? -1 : 1));
}

function readCurrentState(document) {
  if (!isPlainObject(document)) {
    return { recognized: false, registryVersion: null, profiles: new Map() };
  }

  if (document.schemaVersion === "forge.model-profile-store-state.v1") {
    const profiles = new Map();
    for (const entry of Array.isArray(document.profiles) ? document.profiles : []) {
      if (!isPlainObject(entry)) continue;
      if (typeof entry.profileId !== "string") continue;
      if (typeof entry.valueHash !== "string") continue;
      profiles.set(entry.profileId, entry.valueHash);
    }
    return {
      recognized: true,
      registryVersion:
        Number.isSafeInteger(document.registryVersion) ? document.registryVersion : null,
      label: typeof document.label === "string" ? document.label : null,
      profiles,
    };
  }

  if (document.schemaVersion === "forge.model-profile-document.v1") {
    const profiles = new Map();
    for (const profile of Array.isArray(document.profiles) ? document.profiles : []) {
      if (!isPlainObject(profile) || typeof profile.profileId !== "string") continue;
      profiles.set(profile.profileId, profileHashOf(profile).serializedHash);
    }
    return {
      recognized: true,
      registryVersion:
        Number.isSafeInteger(document.registryVersion) ? document.registryVersion : null,
      label: null,
      profiles,
    };
  }

  return { recognized: false, registryVersion: null, profiles: new Map() };
}

/**
 * A redacted diff: profile identifiers and digests only. Deployment names,
 * limits, prices, and qualification evidence never leave this function.
 */
export function buildRedactedDiff(currentState, plannedProfiles) {
  const planned = new Map(plannedProfiles.map((entry) => [entry.profileId, entry.valueHash]));
  const added = [];
  const changed = [];
  const unchanged = [];
  const removed = [];

  for (const [profileId, valueHash] of planned) {
    if (!currentState.profiles.has(profileId)) {
      added.push({ profileId, valueHash });
      continue;
    }
    const previous = currentState.profiles.get(profileId);
    if (previous === valueHash) {
      unchanged.push({ profileId, valueHash });
    } else {
      changed.push({ profileId, previousValueHash: previous, valueHash });
    }
  }
  for (const [profileId, valueHash] of currentState.profiles) {
    if (!planned.has(profileId)) {
      removed.push({ profileId, previousValueHash: valueHash });
    }
  }

  const byProfileId = (left, right) => (left.profileId < right.profileId ? -1 : 1);
  return {
    added: added.sort(byProfileId),
    changed: changed.sort(byProfileId),
    removed: removed.sort(byProfileId),
    unchanged: unchanged.sort(byProfileId),
  };
}

function crossCheckProfile({ profile, index, inventory, violations }) {
  const at = (field) => `profiles/${index}/${field}`;
  const add = (code, field, message) =>
    violations.push({ code, path: at(field), profileId: profile.profileId, message });

  const unsafe = assessDeploymentNameSafety(profile.deploymentName);
  if (unsafe !== null) {
    add(unsafe, "deploymentName", "The deployment name is outside the canonical alphabet.");
    return;
  }

  const deployment = inventory.deployments.get(profile.deploymentName);
  if (deployment === undefined) {
    const caseInsensitive = [...inventory.deployments.keys()].find(
      (name) => name.toLowerCase() === profile.deploymentName.toLowerCase(),
    );
    if (caseInsensitive !== undefined) {
      add(
        "deployment_name_case_mismatch",
        "deploymentName",
        "The inventory holds this deployment under a different case; names are compared ordinally.",
      );
      return;
    }
    add(
      "deployment_unknown",
      "deploymentName",
      "No deployment with this name exists in the supplied inventory.",
    );
    return;
  }

  const expected = profile.expectedModel;
  const mismatched = [];
  if (deployment.modelPublisher !== expected.publisher) mismatched.push("publisher");
  if (deployment.modelName !== expected.name) mismatched.push("name");
  if (deployment.modelVersion !== expected.version) mismatched.push("version");
  if (mismatched.length > 0) {
    add(
      "model_identity_mismatch",
      "expectedModel",
      `The deployment serves a different model identity (${mismatched.join(", ")}).`,
    );
  }

  if (inventory.capabilitiesKnown && deployment.capabilities !== null) {
    for (const [capability, required] of Object.entries(profile.requiredCapabilities)) {
      if (required === true && deployment.capabilities[capability] !== true) {
        add(
          "capability_not_advertised",
          `requiredCapabilities/${capability}`,
          "The deployment does not advertise a capability this profile requires.",
        );
      }
    }
  }
}

function checkProfileInternals({ profile, index, violations, warnings }) {
  const at = (field) => `profiles/${index}/${field}`;
  const add = (code, field, message) =>
    violations.push({ code, path: at(field), profileId: profile.profileId, message });

  if (profile.retentionMode !== "transient_no_store") {
    add("retention_mode_unsupported", "retentionMode", "Only transient_no_store is supported.");
  }

  for (const field of POSITIVE_LIMIT_FIELDS) {
    const value = profile.limits[field];
    if (!Number.isSafeInteger(value) || value < 1) {
      add("limit_not_positive", `limits/${field}`, "This limit must be a positive integer.");
    }
  }
  if (!Number.isSafeInteger(profile.limits.maxAttachmentBytes) || profile.limits.maxAttachmentBytes < 0) {
    add("limit_not_positive", "limits/maxAttachmentBytes", "This limit must not be negative.");
  }
  if (
    Number.isSafeInteger(profile.limits.maxInputTokens) &&
    Number.isSafeInteger(profile.limits.contextTokens) &&
    profile.limits.maxInputTokens > profile.limits.contextTokens
  ) {
    add(
      "limit_exceeds_context",
      "limits/maxInputTokens",
      "The input budget exceeds the declared context window.",
    );
  }
  if (profile.requiredCapabilities.attachments === true && profile.limits.maxAttachmentBytes < 1) {
    add(
      "attachment_budget_missing",
      "limits/maxAttachmentBytes",
      "The profile requires attachments but permits zero attachment bytes.",
    );
  }

  const qualification = profile.qualification;
  if (qualification.status !== "qualified") {
    add(
      "qualification_not_qualified",
      "qualification/status",
      `A profile whose qualification status is ${qualification.status} is never selectable.`,
    );
  }
  if (isDegenerateDigest(qualification.evidenceHash)) {
    add(
      "qualification_evidence_placeholder",
      "qualification/evidenceHash",
      "The qualification evidence digest is a placeholder constant, not evidence.",
    );
  }
  if (qualification.status === "qualified" && qualification.qualifiedAt === null) {
    add(
      "qualification_timestamp_missing",
      "qualification/qualifiedAt",
      "A qualified profile must record when it was qualified.",
    );
  }

  const seenVariantIds = new Set();
  const seenVariantNames = new Set();
  profile.variants.forEach((variant, variantIndex) => {
    if (seenVariantIds.has(variant.variantId)) {
      add(
        "variant_id_duplicate",
        `variants/${variantIndex}/variantId`,
        `Variant ${variant.variantId} is declared more than once in this profile.`,
      );
    }
    seenVariantIds.add(variant.variantId);

    const normalized = normalizeDisplayName(variant.displayName);
    if (seenVariantNames.has(normalized)) {
      add(
        "variant_display_name_duplicate",
        `variants/${variantIndex}/displayName`,
        "Two variants of this profile are indistinguishable to a user.",
      );
    }
    seenVariantNames.add(normalized);

    if (variant.maxOutputTokens > profile.limits.maxOutputTokens) {
      add(
        "variant_output_exceeds_profile_limit",
        `variants/${variantIndex}/maxOutputTokens`,
        "A variant may narrow the profile output budget; it may never widen it.",
      );
    }
  });

  const allowsStructured = profile.requestModeAllowlist.includes("structured_response");
  const allowsStreaming = profile.requestModeAllowlist.includes("streaming_chat");
  if (allowsStructured && profile.canonicalOutputSchemaAllowlist.length === 0) {
    add(
      "structured_mode_without_output_schema",
      "canonicalOutputSchemaAllowlist",
      "structured_response is allowed but no canonical output schema is permitted.",
    );
  }
  if (allowsStructured && profile.requiredCapabilities.structuredOutput !== true) {
    add(
      "capability_contradicts_request_mode",
      "requiredCapabilities/structuredOutput",
      "structured_response is allowed but structured output is not a required capability.",
    );
  }
  if (allowsStreaming && profile.requiredCapabilities.streaming !== true) {
    add(
      "capability_contradicts_request_mode",
      "requiredCapabilities/streaming",
      "streaming_chat is allowed but streaming is not a required capability.",
    );
  }
  if (!allowsStructured && profile.canonicalOutputSchemaAllowlist.length > 0) {
    warnings.push({
      code: "output_schema_without_structured_mode",
      path: at("canonicalOutputSchemaAllowlist"),
      profileId: profile.profileId,
      message: "Canonical output schemas are listed but structured_response is not allowed.",
    });
  }

  const modalityCapability = { text: "textInput", image: "imageInput", pdf: "pdfInput" };
  for (const modality of profile.supportedModalities) {
    const capability = modalityCapability[modality];
    if (capability !== undefined && profile.requiredCapabilities[capability] !== true) {
      add(
        "modality_contradicts_capability",
        `requiredCapabilities/${capability}`,
        `Modality ${modality} is supported but the matching input capability is not required.`,
      );
    }
  }

  if (profile.retryPolicy.initialBackoffMilliseconds > profile.retryPolicy.maximumBackoffMilliseconds) {
    add(
      "retry_backoff_inverted",
      "retryPolicy",
      "The initial backoff exceeds the maximum backoff.",
    );
  }
}

function checkDocumentWideRules({ profiles, violations }) {
  const seenProfileIds = new Map();
  const seenVisibleNames = new Map();
  const seenDeploymentIdentity = new Map();
  const seenDeploymentFold = new Map();

  profiles.forEach((profile, index) => {
    const at = (field) => `profiles/${index}/${field}`;
    const add = (code, field, message) =>
      violations.push({ code, path: at(field), profileId: profile.profileId, message });

    if (seenProfileIds.has(profile.profileId)) {
      add(
        "profile_id_duplicate",
        "profileId",
        `Profile ${profile.profileId} is declared more than once.`,
      );
    } else {
      seenProfileIds.set(profile.profileId, index);
    }

    if (profile.visible === true) {
      const normalized = normalizeDisplayName(profile.displayName);
      if (seenVisibleNames.has(normalized)) {
        add(
          "display_name_duplicate",
          "displayName",
          "Two visible profiles are indistinguishable to a user.",
        );
      } else {
        seenVisibleNames.set(normalized, index);
      }
    }

    if (assessDeploymentNameSafety(profile.deploymentName) !== null) return;

    const folded = profile.deploymentName.toLowerCase();
    const priorFold = seenDeploymentFold.get(folded);
    if (priorFold !== undefined && priorFold !== profile.deploymentName) {
      add(
        "deployment_name_ambiguous",
        "deploymentName",
        "Two profiles name the same deployment with different casing.",
      );
    } else {
      seenDeploymentFold.set(folded, profile.deploymentName);
    }

    const identity = canonicalize({
      publisher: profile.expectedModel.publisher,
      name: profile.expectedModel.name,
      version: profile.expectedModel.version,
    });
    const priorIdentity = seenDeploymentIdentity.get(profile.deploymentName);
    if (priorIdentity !== undefined && priorIdentity !== identity) {
      add(
        "deployment_model_identity_conflict",
        "expectedModel",
        "Two profiles share a deployment but expect different model identities.",
      );
    } else {
      seenDeploymentIdentity.set(profile.deploymentName, identity);
    }
  });
}

export function validate({ profileDocument, deploymentDocument, currentDocument, registry }) {
  const violations = [];
  const warnings = [];

  const schemaErrors = validateSchemaDocument(registry, DOCUMENT_SCHEMA_FILE, profileDocument);
  for (const error of schemaErrors) {
    violations.push({
      code: "document_schema_invalid",
      path: error.path,
      schemaCode: error.code,
      message: error.message,
    });
  }

  const inventory = readDeploymentInventory(deploymentDocument);
  if (!inventory.recognized) {
    violations.push({
      code: "deployment_inventory_shape_unrecognized",
      path: "$",
      message:
        "The deployments file is neither forge.foundry-prerequisites.v1/v2 nor sapphirus.foundry-deployment.v1.",
    });
  } else if (!inventory.ready) {
    violations.push({
      code: "deployment_inventory_not_ready",
      path: "$",
      message:
        "The deployment inventory does not assert readiness; no profile may be validated against it.",
      inventoryBlockers: [...inventory.blockers].sort(),
    });
  }

  // Semantic checks run only on a document that already matches the schema.
  // Running them on an off-schema document would report imagined problems.
  const profiles = schemaErrors.length === 0 ? profileDocument.profiles : [];
  const readyInventory = inventory.recognized && inventory.ready;

  profiles.forEach((profile, index) => {
    checkProfileInternals({ profile, index, violations, warnings });
    if (readyInventory) {
      crossCheckProfile({ profile, index, inventory, violations });
    }
  });
  checkDocumentWideRules({ profiles, violations });

  const plannedProfiles = profiles.map((profile) => {
    const hash = profileHashOf(profile);
    return {
      profileId: profile.profileId,
      key: `${KEY_PREFIX}${profile.profileId}`,
      visible: profile.visible === true,
      enabled: profile.enabled === true,
      valueHash: hash.serializedHash,
      value: hash.canonicalJson,
      valueBytes: Buffer.byteLength(hash.canonicalJson, "utf8"),
    };
  });

  for (const planned of plannedProfiles) {
    if (planned.valueBytes > MAXIMUM_STORE_VALUE_BYTES) {
      violations.push({
        code: "profile_value_too_large",
        path: `profiles/${planned.profileId}`,
        profileId: planned.profileId,
        message: `The canonical profile value is ${planned.valueBytes} bytes; the configured bound is ${MAXIMUM_STORE_VALUE_BYTES}.`,
      });
    }
  }

  const orderedPlan = [...plannedProfiles].sort((left, right) =>
    left.profileId < right.profileId ? -1 : 1,
  );

  const registryVersion =
    schemaErrors.length === 0 ? profileDocument.registryVersion : null;

  let registryHash = null;
  let capabilityHash = null;
  let registryValue = null;
  if (schemaErrors.length === 0) {
    registryHash = canonicalHash({
      purpose: REGISTRY_HASH_PURPOSE,
      schemaMajor: SCHEMA_MAJOR,
      value: {
        registryVersion,
        profiles: orderedPlan.map((entry) => ({
          profileId: entry.profileId,
          profileHash: entry.valueHash,
        })),
      },
    }).serializedHash;
    capabilityHash = canonicalHash({
      purpose: CAPABILITY_HASH_PURPOSE,
      schemaMajor: SCHEMA_MAJOR,
      value: capabilitySurfaceOf(profiles),
    }).serializedHash;
    registryValue = canonicalize({
      schemaVersion: "forge.model-profile-registry.v1",
      registryVersion,
      registryHash,
      capabilityHash,
      profileIds: orderedPlan.map((entry) => entry.profileId),
    });
  }

  const currentState =
    currentDocument === undefined || currentDocument === null
      ? null
      : readCurrentState(currentDocument);
  if (currentState !== null && !currentState.recognized) {
    violations.push({
      code: "current_state_shape_unrecognized",
      path: "$",
      message:
        "The current-state file is neither forge.model-profile-store-state.v1 nor forge.model-profile-document.v1.",
    });
  }

  let diff = null;
  if (currentState !== null && currentState.recognized) {
    diff = buildRedactedDiff(currentState, orderedPlan);
    for (const entry of diff.removed) {
      warnings.push({
        code: "profile_removed",
        path: `profiles/${entry.profileId}`,
        profileId: entry.profileId,
        message: "This profile is published today and is absent from the document.",
      });
    }
    if (
      currentState.registryVersion !== null &&
      registryVersion !== null &&
      registryVersion <= currentState.registryVersion
    ) {
      violations.push({
        code: "registry_version_not_increasing",
        path: "registryVersion",
        message: `The document publishes registry version ${registryVersion} over the current ${currentState.registryVersion}.`,
      });
    }
  }

  const orderedViolations = [...violations].sort((left, right) => {
    if (left.code !== right.code) return left.code < right.code ? -1 : 1;
    return left.path < right.path ? -1 : 1;
  });

  return {
    valid: orderedViolations.length === 0,
    violations: orderedViolations,
    warnings,
    inventory,
    registryVersion,
    registryHash,
    capabilityHash,
    registryValue,
    plan: orderedPlan,
    diff,
  };
}

function buildReport({ options, outcome }) {
  return {
    schemaVersion: "forge.model-profile-validation.v1",
    generatedAt: new Date().toISOString().replace(/\.\d{3}Z$/u, ".000Z"),
    valid: outcome.valid,
    profilesPath: repositoryRelative(options.profiles),
    deploymentsPath: repositoryRelative(options.deployments),
    currentStatePath: options.current === null ? null : repositoryRelative(options.current),
    keyPrefix: KEY_PREFIX,
    registryVersionKey: REGISTRY_VERSION_KEY,
    deploymentInventory: {
      schemaVersion: outcome.inventory.source,
      ready: outcome.inventory.ready,
      capabilitiesKnown: outcome.inventory.capabilitiesKnown,
      deploymentCount: outcome.inventory.deployments.size,
      blockers: [...outcome.inventory.blockers].sort(),
    },
    registryVersion: outcome.registryVersion,
    registryHash: outcome.registryHash,
    capabilityHash: outcome.capabilityHash,
    profiles: outcome.plan.map((entry) => ({
      profileId: entry.profileId,
      key: entry.key,
      visible: entry.visible,
      enabled: entry.enabled,
      valueHash: entry.valueHash,
      valueBytes: entry.valueBytes,
    })),
    diff: outcome.diff,
    warnings: outcome.warnings,
    violations: outcome.violations,
  };
}

function writeJsonFile(filePath, value) {
  const resolved = path.resolve(filePath);
  mkdirSync(path.dirname(resolved), { recursive: true });
  writeFileSync(resolved, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

export async function main(argv) {
  const options = parseArguments(argv);

  const profileDocument = readJsonFile(options.profiles, "profiles", MAXIMUM_DOCUMENT_BYTES);
  const deploymentDocument = readJsonFile(
    options.deployments,
    "deployments",
    MAXIMUM_INVENTORY_BYTES,
  );
  const currentDocument =
    options.current === null
      ? null
      : readJsonFile(options.current, "current state", MAXIMUM_DOCUMENT_BYTES);

  const registry = await loadSchemaRegistry(toolDirectory);
  const outcome = validate({
    profileDocument,
    deploymentDocument,
    currentDocument,
    registry,
  });
  const report = buildReport({ options, outcome });

  if (options.out !== null) {
    writeJsonFile(options.out, report);
  }
  // A plan is emitted only for a document that validates. A refused document
  // still prints its full report, so the caller can act on the violations
  // rather than on a bare exit code.
  if (options.emitPlan !== null && outcome.valid) {
    writeJsonFile(options.emitPlan, {
      schemaVersion: "forge.model-profile-plan.v1",
      keyPrefix: KEY_PREFIX,
      registryVersionKey: REGISTRY_VERSION_KEY,
      registryVersion: outcome.registryVersion,
      registryHash: outcome.registryHash,
      capabilityHash: outcome.capabilityHash,
      registryValue: outcome.registryValue,
      profiles: outcome.plan.map((entry) => ({
        profileId: entry.profileId,
        key: entry.key,
        value: entry.value,
        valueHash: entry.valueHash,
      })),
    });
  }

  process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
  if (!options.quiet) {
    const summary = outcome.valid
      ? `model-profile document is publishable: ${report.profiles.length} profiles, registry version ${report.registryVersion}`
      : `model-profile document refused: ${outcome.violations.length} violations (${[
          ...new Set(outcome.violations.map((entry) => entry.code)),
        ].join(", ")})`;
    process.stderr.write(`${summary}\n`);
  }
  return outcome.valid ? 0 : 2;
}

const invokedDirectly =
  process.argv[1] !== undefined &&
  path.resolve(process.argv[1]) === path.resolve(fileURLToPath(import.meta.url));

if (invokedDirectly) {
  try {
    process.exitCode = await main(process.argv.slice(2));
  } catch (error) {
    if (error instanceof UsageError) {
      process.stderr.write(`${error.message}\n`);
      process.exitCode = 1;
    } else {
      process.stderr.write(`${error.name}: ${error.message}\n`);
      process.exitCode = 1;
    }
  }
}
