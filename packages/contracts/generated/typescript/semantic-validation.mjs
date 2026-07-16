// @generated from handwritten semantic validators; DO NOT EDIT.
function isStrictlySortedUnique(values) {
  let previous;
  for (const value of values) {
    if (previous !== undefined && previous >= value) {
      return false;
    }
    previous = value;
  }
  return true;
}

function validateInstant(value, field, errors) {
  const epochMilliseconds = Date.parse(value);
  if (
    !Number.isFinite(epochMilliseconds)
    || new Date(epochMilliseconds).toISOString() !== value
  ) {
    errors.push({ code: "INVALID_UTC_INSTANT", field });
  }
}

function structurallyEqual(left, right) {
  if (Object.is(left, right)) return true;
  if (left === null || right === null || typeof left !== typeof right) return false;
  if (Array.isArray(left) || Array.isArray(right)) {
    return Array.isArray(left)
      && Array.isArray(right)
      && left.length === right.length
      && left.every((value, index) => structurallyEqual(value, right[index]));
  }
  if (typeof left !== "object") return false;
  const leftKeys = Object.keys(left).sort();
  const rightKeys = Object.keys(right).sort();
  return leftKeys.length === rightKeys.length
    && leftKeys.every(
      (key, index) => key === rightKeys[index]
        && structurallyEqual(left[key], right[key]),
    );
}

const INVISIBLE_OR_CONTROL_CHARACTER = /\p{C}/u;
const DRIVE_ROOTED_PATH = /(?<![A-Za-z0-9])[A-Za-z]:[\\/]/u;
const FILE_URI = /(?<![A-Za-z0-9])file:\/\//iu;
const ROOTED_DOUBLE_SLASH_PATH = /(?<![A-Za-z0-9:])\/\/(?!\s)/u;
const ROOTED_POSIX_PATH = /(?<![\p{L}\p{N}/:])\/(?=[\p{L}\p{N}._~-])/u;

function containsLocalPathShape(value) {
  return value.includes("\\")
    || DRIVE_ROOTED_PATH.test(value)
    || FILE_URI.test(value)
    || ROOTED_DOUBLE_SLASH_PATH.test(value)
    || ROOTED_POSIX_PATH.test(value);
}

export function validateContractErrorSemantics(document) {
  const errors = [];
  if (INVISIBLE_OR_CONTROL_CHARACTER.test(document.message)) {
    errors.push({
      code: "ERROR_MESSAGE_CONTROL_CHARACTER",
      field: "message",
    });
  }
  if (containsLocalPathShape(document.message)) {
    errors.push({
      code: "ERROR_MESSAGE_PATH_DISCLOSURE",
      field: "message",
    });
  }
  if (document.detailsRef !== null) {
    if (INVISIBLE_OR_CONTROL_CHARACTER.test(document.detailsRef)) {
      errors.push({
        code: "ERROR_DETAILS_REF_CONTROL_CHARACTER",
        field: "detailsRef",
      });
    }
    if (containsLocalPathShape(document.detailsRef)) {
      errors.push({
        code: "ERROR_DETAILS_REF_LOCAL_PATH",
        field: "detailsRef",
      });
    }
  }
  return errors;
}

export function validatePackageCompatibilitySemantics(document) {
  const errors = [];
  validateInstant(document.issuedAt, "issuedAt", errors);
  if (document.expiresAt !== null) {
    validateInstant(document.expiresAt, "expiresAt", errors);
    if (Date.parse(document.expiresAt) <= Date.parse(document.issuedAt)) {
      errors.push({ code: "PACKAGE_EXPIRY_INVALID", field: "expiresAt" });
    }
  }
  if (document.contractEpoch.minimum > document.contractEpoch.maximum) {
    errors.push({ code: "CONTRACT_EPOCH_RANGE_INVALID", field: "contractEpoch" });
  }
  if (!isStrictlySortedUnique(document.supportedDeliveryModels)) {
    errors.push({
      code: "DELIVERY_MODEL_SET_NOT_CANONICAL",
      field: "supportedDeliveryModels",
    });
  }

  const capabilityOwners = new Map();
  for (const field of [
    "requiredCapabilities",
    "optionalCapabilities",
    "forbiddenCapabilities",
  ]) {
    if (!isStrictlySortedUnique(document[field])) {
      errors.push({ code: "CAPABILITY_SET_NOT_CANONICAL", field });
    }
    for (const capability of document[field]) {
      const previousOwner = capabilityOwners.get(capability);
      if (previousOwner !== undefined && previousOwner !== field) {
        errors.push({
          code: "CAPABILITY_SET_OVERLAP",
          field,
          capability,
          conflictingField: previousOwner,
        });
      } else {
        capabilityOwners.set(capability, field);
      }
    }
  }
  return errors;
}

export function validateRemoteJobHandoffSemantics(document) {
  const errors = [];
  validateInstant(document.createdAt, "createdAt", errors);
  if (document.handoffVersion === 1 && document.previousHandoffHash !== null) {
    errors.push({
      code: "HANDOFF_GENESIS_PREVIOUS_HASH",
      field: "previousHandoffHash",
    });
  }
  if (document.handoffVersion > 1 && document.previousHandoffHash === null) {
    errors.push({
      code: "HANDOFF_CHAIN_PREVIOUS_HASH_REQUIRED",
      field: "previousHandoffHash",
    });
  }
  return errors;
}

export function validateRemoteJobHandoffTransition(previous, current) {
  const errors = [];
  if (
    previous.schemaVersion !== "sapphirus.remote-job-handoff.v1"
    || current.schemaVersion !== "sapphirus.remote-job-handoff.v1"
  ) {
    return [{ code: "HANDOFF_CHAIN_SCHEMA_MISMATCH", field: "schemaVersion" }];
  }

  for (const field of [
    "handoffId",
    "sourceAuthority",
    "sourceProjectId",
    "sourceRunId",
    "sourceCheckpointId",
    "sourceWorkspaceManifestHash",
  ]) {
    if (!structurallyEqual(previous[field], current[field])) {
      errors.push({ code: "HANDOFF_CHAIN_IDENTITY_MISMATCH", field });
    }
  }
  if (current.handoffVersion !== previous.handoffVersion + 1) {
    errors.push({ code: "HANDOFF_VERSION_NOT_INCREMENTAL", field: "handoffVersion" });
  }
  if (current.previousHandoffHash !== previous.handoffHash) {
    errors.push({
      code: "HANDOFF_PREVIOUS_HASH_MISMATCH",
      field: "previousHandoffHash",
    });
  }
  return errors;
}

import { canonicalHash } from "./canonical-json.mjs";

const BMAD_BUILDER_LIMIT_PROFILE = "sapphirus.bmad-builder-limits.v1";
const BMAD_GRAPH_KINDS = Object.freeze([
  "compatibility_yaml",
  "method_central_toml",
  "skill_customization_toml",
]);
const AGENT_LENSES = Object.freeze([
  "leanness",
  "architecture",
  "determinism",
  "customization",
  "enhancement",
  "agent-cohesion",
]);
const WORKFLOW_LENSES = Object.freeze(AGENT_LENSES.slice(0, 5));
const EXPECTED_BMM_MODULE_HASH =
  "sha256:5a2a4ff761b3a4f92730442386486f32318152fc0dfdd225dc6765a3bc2ec100";
const EXPECTED_BMM_AGENTS = Object.freeze([
  ["bmad-agent-analyst", "Mary", "Business Analyst", "📊", "software-development", "Channels Porter's strategic rigor and Minto's Pyramid Principle, grounds every finding in verifiable evidence, represents every stakeholder voice. Speaks like a treasure hunter narrating the find: thrilled by every clue, precise once the pattern emerges.", "sha256:fca38a404e79508ebc39235d0162bcb475b02407a913e3e5bd8c5526b2b261a6", "sha256:7191f4a60ada7dbabe083699b2461f35778af7688924dc6ce05911cf1ffc9054"],
  ["bmad-agent-architect", "Winston", "System Architect", "🏗️", "software-development", "Favors boring technology for stability, developer productivity as architecture, ties every decision to business value. Speaks like a seasoned engineer at the whiteboard: measured, always laying out trade-offs rather than verdicts.", "sha256:82bbf22ff3a4571741c9339ae6b7c35676a6e26a3237ceca487abe85369e91a0", "sha256:d9763009d7c20246119c24bcea5eacebd21ad60c22ab191b74c9a5fb6e5f57ad"],
  ["bmad-agent-dev", "Amelia", "Senior Software Engineer", "💻", "software-development", "Test-first discipline (red, green, refactor), 100% pass before review, no fluff all precision. Speaks like a terminal prompt: exact file paths, AC IDs, and commit-message brevity — every statement citable.", "sha256:54ab6d9d60d7ed7e84d6510cf325734b66feb1b419131436672559584b4bf508", "sha256:01a45ac2420f920cfc934de7b1a884b04017c85641085c93d9aada45d2dccf17"],
  ["bmad-agent-pm", "John", "Product Manager", "📋", "software-development", "Drives Jobs-to-be-Done over template filling, user value first, technical feasibility is a constraint not the driver. Speaks like a detective interrogating a cold case: short questions, sharper follow-ups, every 'why?' tightening the net.", "sha256:e2ebea0bf6ee158fcb9e62731a185983f2ac59f664940f4f4320cf10e1bbdf92", "sha256:afc2250de25241ac3577c9827e1ee62d8c7a1913de8eb23aadfffb0aadbadc37"],
  ["bmad-agent-tech-writer", "Paige", "Technical Writer", "📚", "software-development", "Master of CommonMark, DITA, and OpenAPI; turns complex concepts into accessible structured docs, favors diagrams over walls of text, every word earning its place. Speaks like the patient teacher you wish you'd had, using analogies that make complex things feel simple.", "sha256:7544242ff68d0ddfd466274f40ae9f3b0ea06ee5f5f938f68f953e19b3598cd7", "sha256:bb828f2d26a136870099226f07c61297ce88ddd335823b7549592932bbe14a2e"],
  ["bmad-agent-ux-designer", "Sally", "UX Designer", "🎨", "software-development", "Balances empathy with edge-case rigor, starts simple and evolves through feedback, every decision serves a genuine user need. Speaks like a filmmaker pitching the scene before the code exists, painting user stories that make you feel the problem.", "sha256:d780ae59b9fb7e3a94c5ffe4ec75f90942d4a9a3944af4b36c2d279fdd7801db", "sha256:3df7da44945f9df34dbb3a64de086c756e6c298e73078e388e8f9a4f57cbf7ab"],
]);
const EXPECTED_BMM_MENUS = Object.freeze({
  "bmad-agent-analyst": [["BP", "bmad-brainstorming", null], ["MR", "bmad-market-research", null], ["DR", "bmad-domain-research", null], ["TR", "bmad-technical-research", null], ["CB", "bmad-product-brief", null], ["WB", "bmad-prfaq", null], ["DP", "bmad-document-project", null]],
  "bmad-agent-architect": [["CA", "bmad-architecture", "create"], ["IR", "bmad-check-implementation-readiness", null]],
  "bmad-agent-dev": [["DS", "bmad-dev-story", null], ["QD", "bmad-quick-dev", null], ["QA", "bmad-qa-generate-e2e-tests", null], ["CR", "bmad-code-review", null], ["SP", "bmad-sprint-planning", null], ["CS", "bmad-create-story", null], ["ER", "bmad-retrospective", null]],
  "bmad-agent-pm": [["PRD", "bmad-prd", null], ["CE", "bmad-create-epics-and-stories", null], ["IR", "bmad-check-implementation-readiness", null], ["CC", "bmad-correct-course", null]],
  "bmad-agent-tech-writer": [["DP", "bmad-document-project", null]],
  "bmad-agent-ux-designer": [["CU", "bmad-ux", null]],
});
const EXPECTED_BMM_AGENT_RECORD_HASHES = Object.freeze({
  "bmad-agent-analyst": "sha256:6b37055d48b0b5a8186d4bac5986aefc68f30ca168124f0d101b6539c21adce9",
  "bmad-agent-architect": "sha256:4dc48526aac64c60d15a389f707189ac313cfdf3c69290860790b0272c5f1d20",
  "bmad-agent-dev": "sha256:00b6cd96945f5563f446e09f8cb5e5dc1c3cb11a2059e42555044d47f308f54f",
  "bmad-agent-pm": "sha256:ee14a413e53a6f4f52d9ca83e24babe32ba7f5cd8d2324ef921cddeb89c24869",
  "bmad-agent-tech-writer": "sha256:dbd78337564afb6d7b142c2ea3188f3b1eec3250d9ba8b64281bc016325f74bf",
  "bmad-agent-ux-designer": "sha256:bc39797efddbbf455b30c3de5e4b67f5df1bd9d0d4417567ab3cb109f98fcfd5",
});
const WINDOWS_RESERVED_SEGMENT = /^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\..*)?$/iu;
const BMAD_UTF8_ENCODER = new TextEncoder();

function bmadUtf8Length(value) {
  return BMAD_UTF8_ENCODER.encode(value).byteLength;
}

function bmadIssue(code, field, details = {}) {
  return Object.freeze({ code, field, ...details });
}

function bmadIsSafeText(value) {
  if (typeof value !== "string") return false;
  for (const scalar of value) {
    const codePoint = scalar.codePointAt(0);
    if (
      codePoint <= 0x001f
      || codePoint === 0x007f
      || (codePoint >= 0xd800 && codePoint <= 0xdfff)
      || codePoint === 0x061c
      || codePoint === 0x200e
      || codePoint === 0x200f
      || (codePoint >= 0x202a && codePoint <= 0x202e)
      || (codePoint >= 0x2066 && codePoint <= 0x2069)
    ) {
      return false;
    }
  }
  return true;
}

function bmadValidateStandaloneHash(document, purpose, hashField, errors) {
  let expected;
  try {
    expected = canonicalHash({
      purpose,
      schemaMajor: "v1",
      value: document,
      excludedFields: [hashField],
    }).serializedHash;
  } catch {
    expected = undefined;
  }
  if (document?.[hashField] !== expected) {
    errors.push(bmadIssue("HASH_MISMATCH", hashField));
  }
}

function bmadValidateInstant(value, field, errors) {
  const epochMilliseconds = Date.parse(value);
  if (!Number.isFinite(epochMilliseconds)
    || new Date(epochMilliseconds).toISOString() !== value) {
    errors.push(bmadIssue("INVALID_UTC_INSTANT", field));
  }
}

export function validateMethodHelpProposalSemantics(document) {
  const errors = [];
  if (document?.proposalKind === "recommended_capability"
    && !bmadIsSafeText(document.rationaleSummary)) {
    errors.push(bmadIssue("BMAD_UNSAFE_TEXT", "rationaleSummary"));
  }
  return errors;
}

export function validateMethodHelpRecommendationSemantics(document) {
  const errors = [];
  if (document?.recommendationKind === "recommended_capability"
    && !bmadIsSafeText(document.rationaleSummary)) {
    errors.push(bmadIssue("BMAD_UNSAFE_TEXT", "rationaleSummary"));
  }
  bmadValidateStandaloneHash(
    document,
    "bmad-method-help-recommendation",
    "recommendationHash",
    errors,
  );
  bmadValidateInstant(document?.createdAt, "createdAt", errors);
  return errors;
}

export function validateMethodAdvanceResultSemantics(document) {
  const errors = [];
  if ((document?.resultKind === "refusal" || document?.resultKind === "incomplete")
    && !bmadIsSafeText(document.safeMessage)) {
    errors.push(bmadIssue("BMAD_UNSAFE_TEXT", "safeMessage"));
  }
  bmadValidateStandaloneHash(
    document,
    "bmad-method-canonical-advance-result",
    "resultHash",
    errors,
  );
  bmadValidateInstant(document?.receivedAt, "receivedAt", errors);
  return errors;
}

function bmadTuple(values) {
  if (values.some((value) => typeof value !== "string" || value.includes("\0"))) {
    return null;
  }
  return values.join("\0");
}

function bmadCapabilityKey(value) {
  return bmadTuple([
    value.packageVersionId,
    value.moduleCode,
    value.skillName,
    value.normalizedAction ?? "",
  ]);
}

function bmadScopeKey(value) {
  return bmadTuple([
    value.graphKind,
    value.scope.packageVersionId,
    value.scope.moduleCode ?? "",
    value.scope.skillName ?? "",
  ]);
}

function bmadIsStrictlySortedUnique(values, keySelector) {
  let previous;
  for (const value of values) {
    const current = keySelector(value);
    if (current === null || (previous !== undefined && previous >= current)) return false;
    previous = current;
  }
  return true;
}

function bmadSameCapability(left, right) {
  return bmadCapabilityKey(left) === bmadCapabilityKey(right);
}

function bmadSameModelBinding(left, right) {
  return [
    "bindingKind",
    "providerId",
    "modelId",
    "deploymentId",
    "modelProfileHash",
    "modelCapabilityHash",
    "contextWindowProfileHash",
    "egressProfileHash",
    "requestSchemaHash",
    "responseSchemaHash",
    "bindingHash",
  ].every((field) => left?.[field] === right?.[field]);
}

function bmadSetEquals(left, right) {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function bmadFindDescriptorResource(descriptor, path, hash) {
  return descriptor?.resourceInventory?.some(
    (resource) => resource.path === path && resource.contentHash === hash,
  ) === true;
}

function bmadAgentRecordHashIsExact(agent) {
  const expected = EXPECTED_BMM_AGENT_RECORD_HASHES[agent.agentCode];
  if (expected === undefined) return false;
  const recordHash = canonicalHash({
    purpose: "bmad-agent-record",
    schemaMajor: "v1",
    value: {
      moduleCode: agent.moduleCode,
      agentCode: agent.agentCode,
      name: agent.name,
      title: agent.title,
      icon: agent.icon,
      team: agent.team,
      description: agent.description,
      personaSourceHash: agent.personaSourceHash,
      customizationSourceHash: agent.customizationSourceHash,
      menuItems: agent.menuItems,
    },
    excludedFields: [],
  }).serializedHash;
  const menuGraphHash = canonicalHash({
    purpose: "bmad-agent-menu-graph",
    schemaMajor: "v1",
    value: agent.menuItems,
    excludedFields: [],
  }).serializedHash;
  return recordHash === expected
    && agent.agentRecordHash === expected
    && agent.menuGraphHash === menuGraphHash
    && agent.personaCustomizationGraphHash === agent.customizationSourceHash;
}

function validateDescriptor(document, errors) {
  if (
    document.sourceIdentity.packageName !== document.packageName
    || document.sourceIdentity.packageVersion !== document.packageVersion
  ) {
    errors.push(bmadIssue("BMAD_SOURCE_IDENTITY_MISMATCH", "sourceIdentity"));
  }
  if (
    document.packageName === "bmad-method"
    && (
      document.packageVersion !== "6.10.0"
      || document.sourceIdentity.moduleVersion !== null
      || document.sourceIdentity.sourceFormatVersion !== null
      || document.sourceIdentity.archiveArtifactLabel !== "BMAD-METHOD-main.zip"
      || document.sourceIdentity.archiveSha256 !== "sha256:a7c049038099b99081fbd03d22c6a5180edd88dee656bb37c4276b1cc31b4a32"
      || document.sourceIdentity.runtimeCompatibility.length !== 1
      || document.sourceIdentity.runtimeCompatibility[0].runtime !== "node"
      || document.sourceIdentity.runtimeCompatibility[0].versionRange !== ">=20.12.0"
    )
  ) {
    errors.push(bmadIssue("BMAD_METHOD_SOURCE_IDENTITY_MISMATCH", "sourceIdentity"));
  }
  const graphKinds = [...new Set(document.configGraphs.map((graph) => graph.graphKind))].sort();
  if (!bmadSetEquals(graphKinds, BMAD_GRAPH_KINDS)) {
    errors.push(bmadIssue("BMAD_CONFIG_GRAPHS_INCOMPLETE", "configGraphs"));
  }
  if (!bmadIsStrictlySortedUnique(document.configGraphs, bmadScopeKey)) {
    errors.push(bmadIssue("BMAD_CONFIG_GRAPH_NOT_CANONICAL", "configGraphs"));
  }
  if (!bmadIsStrictlySortedUnique(document.configResolutions, bmadScopeKey)) {
    errors.push(bmadIssue("BMAD_CONFIG_RESOLUTION_NOT_CANONICAL", "configResolutions"));
  }

  const graphKeys = new Set(document.configGraphs.map(bmadScopeKey));
  for (const resolution of document.configResolutions) {
    const graph = document.configGraphs.find((candidate) =>
      bmadScopeKey(candidate) === bmadScopeKey(resolution));
    if (!graphKeys.has(bmadScopeKey(resolution))) {
      errors.push(bmadIssue("BMAD_CONFIG_RESOLUTION_ORPHAN", "configResolutions"));
    } else if (
      graph.graphHash !== resolution.graphHash
      || !bmadSetEquals(
        resolution.orderedLayerHashes,
        graph.layers.map((layer) => layer.sourceHash),
      )
    ) {
      errors.push(bmadIssue("BMAD_CONFIG_RESOLUTION_BINDING_MISMATCH", "configResolutions"));
    }
  }
  for (const graph of document.configGraphs) {
    const { moduleCode, skillName } = graph.scope;
    const allowedLayerKinds = {
      method_central_toml: new Set([
        "installer_team", "installer_user", "custom_team", "custom_user",
      ]),
      skill_customization_toml: new Set([
        "packaged_default", "team_override", "user_override",
      ]),
      compatibility_yaml: new Set([
        "method_module_yaml", "builder_root_yaml", "builder_user_yaml",
      ]),
    }[graph.graphKind];
    if (
      graph.scope.packageVersionId !== document.packageVersionId
      ||
      (graph.graphKind === "method_central_toml" && (moduleCode !== null || skillName !== null))
      || (graph.graphKind === "skill_customization_toml"
        && (moduleCode === null || skillName === null))
      || (graph.graphKind === "compatibility_yaml" && moduleCode === null)
    ) {
      errors.push(bmadIssue("BMAD_CONFIG_SCOPE_INVALID", "configGraphs.scope"));
    }
    if (
      !bmadIsStrictlySortedUnique(graph.layers, (layer) =>
        bmadTuple([String(layer.ordinal).padStart(8, "0"), layer.sourcePath]))
      || graph.layers.some((layer) =>
        layer.graphKind !== graph.graphKind || !allowedLayerKinds.has(layer.layerKind))
    ) {
      errors.push(bmadIssue("BMAD_CONFIG_LAYER_INVALID", "configGraphs.layers"));
    }
  }

  if (!bmadIsStrictlySortedUnique(document.modules, (module) => module.moduleCode)) {
    errors.push(bmadIssue("BMAD_MODULE_SET_NOT_CANONICAL", "modules"));
  }
  if (!bmadIsStrictlySortedUnique(document.skills, (skill) =>
    bmadTuple([skill.moduleCode, skill.skillName]))) {
    errors.push(bmadIssue("BMAD_SKILL_SET_NOT_CANONICAL", "skills"));
  }
  if (!bmadIsStrictlySortedUnique(document.resourceInventory, (resource) => resource.path)) {
    errors.push(bmadIssue("BMAD_RESOURCE_SET_NOT_CANONICAL", "resourceInventory"));
  }
  if (!bmadIsStrictlySortedUnique(
    document.instructionProjections,
    (projection) => projection.projectionId,
  )) {
    errors.push(bmadIssue("BMAD_INSTRUCTION_PROJECTION_SET_NOT_CANONICAL", "instructionProjections"));
  }
  const projectionHashes = new Set();
  for (const projection of document.instructionProjections) {
    const expectedProjectionHash = canonicalHash({
      purpose: "bmad-instruction-projection",
      schemaMajor: "v1",
      value: projection,
      excludedFields: ["projectionHash"],
    }).serializedHash;
    if (projection.projectionHash !== expectedProjectionHash) {
      errors.push(bmadIssue(
        "BMAD_INSTRUCTION_PROJECTION_HASH_MISMATCH",
        "instructionProjections.projectionHash",
      ));
    }
    if (
      projection.sourceIdentityHash !== document.sourceSnapshotHash
      || projectionHashes.has(projection.projectionHash)
    ) {
      errors.push(bmadIssue("BMAD_INSTRUCTION_PROJECTION_IDENTITY_MISMATCH", "instructionProjections"));
    }
    projectionHashes.add(projection.projectionHash);
    if (!bmadIsStrictlySortedUnique(projection.sourceResources, (source) => source.path)) {
      errors.push(bmadIssue("BMAD_INSTRUCTION_PROJECTION_SOURCE_NOT_CANONICAL", "instructionProjections.sourceResources"));
    }
    for (const source of [projection.sourceEntrypoint, ...projection.sourceResources]) {
      const inventory = document.resourceInventory.find((resource) => resource.path === source.path);
      if (
        inventory === undefined
        || inventory.locationKind !== "source_tree"
        || inventory.contentHash !== source.contentHash
        || inventory.treatment !== source.treatment
      ) {
        errors.push(bmadIssue("BMAD_INSTRUCTION_PROJECTION_SOURCE_TRANSPLANT", "instructionProjections"));
      }
    }
    const managed = document.resourceInventory.find((resource) =>
      resource.path === projection.managedInstruction.path);
    if (
      managed === undefined
      || managed.locationKind !== "managed_projection"
      || managed.contentRole !== "managed_instruction"
      || managed.runtimeUse !== "instruction_data"
      || managed.contentHash !== projection.managedInstruction.contentHash
    ) {
      errors.push(bmadIssue("BMAD_MANAGED_INSTRUCTION_TRANSPLANT", "instructionProjections.managedInstruction"));
    }
  }
  const moduleCodes = new Set(document.modules.map((module) => module.moduleCode));
  for (const skill of document.skills) {
    if (!moduleCodes.has(skill.moduleCode)) {
      errors.push(bmadIssue("BMAD_SKILL_MODULE_ORPHAN", "skills.moduleCode"));
    }
    if (!bmadFindDescriptorResource(
      document,
      skill.sourceEntrypointPath,
      skill.sourceEntrypointHash,
    )) {
      errors.push(bmadIssue("BMAD_SKILL_SOURCE_TRANSPLANT", "skills.sourceEntrypointHash"));
    }
    const projection = document.instructionProjections.find((candidate) =>
      candidate.projectionHash === skill.instructionProjectionHash);
    if (
      projection === undefined
      || projection.sourceEntrypoint.path !== skill.sourceEntrypointPath
      || projection.sourceEntrypoint.contentHash !== skill.sourceEntrypointHash
    ) {
      errors.push(bmadIssue("BMAD_SKILL_PROJECTION_TRANSPLANT", "skills.instructionProjectionHash"));
    }
  }
}

function validateCatalog(document, errors, context) {
  if (context.descriptor !== undefined) {
    if (
      document.packageVersionId !== context.descriptor.packageVersionId
      || document.descriptorHash !== context.descriptor.descriptorHash
      || document.packageSourceHash !== context.descriptor.sourceSnapshotHash
    ) {
      errors.push(bmadIssue("BMAD_CATALOG_DESCRIPTOR_BINDING_MISMATCH", "descriptorHash"));
    }
  }
  const installedKeys = new Set();
  if (!bmadIsStrictlySortedUnique(document.installedSkills, (skill) =>
    bmadTuple([skill.moduleCode, skill.skillName]))) {
    errors.push(bmadIssue("BMAD_INSTALLED_SKILL_SET_NOT_CANONICAL", "installedSkills"));
  }
  for (const skill of document.installedSkills) {
    if (!bmadIsStrictlySortedUnique(skill.capabilityKeys, bmadCapabilityKey)) {
      errors.push(bmadIssue("BMAD_CAPABILITY_SET_NOT_CANONICAL", "installedSkills.capabilityKeys"));
    }
    const actions = skill.capabilityKeys.map((key) => key.normalizedAction);
    if (
      (skill.actionCardinality === "single_action" && actions.length !== 1)
      || (skill.actionCardinality === "multi_action"
        && (actions.length < 2 || actions.some((action) => action === null)))
    ) {
      errors.push(bmadIssue("BMAD_CAPABILITY_CARDINALITY_INVALID", "installedSkills.capabilityKeys"));
    }
    for (const key of skill.capabilityKeys) {
      const encoded = bmadCapabilityKey(key);
      if (
        key.packageVersionId !== document.packageVersionId
        || key.moduleCode !== skill.moduleCode
        || key.skillName !== skill.skillName
        || installedKeys.has(encoded)
      ) {
        errors.push(bmadIssue("BMAD_CAPABILITY_KEY_COLLISION", "installedSkills.capabilityKeys"));
      }
      installedKeys.add(encoded);
    }
    if (context.descriptor !== undefined) {
      const descriptorSkill = context.descriptor.skills.find((candidate) =>
        candidate.moduleCode === skill.moduleCode && candidate.skillName === skill.skillName);
      if (
        descriptorSkill === undefined
        || descriptorSkill.sourceEntrypointHash !== skill.sourceEntrypointHash
        || descriptorSkill.resourceSetHash !== skill.resourceSetHash
        || descriptorSkill.skillDescriptorHash !== skill.skillDescriptorHash
        || descriptorSkill.executionProfile.profileHash !== skill.executionProfileHash
        || descriptorSkill.instructionProjectionHash !== skill.instructionProjectionHash
        || descriptorSkill.distributionProfile !== skill.distributionProfile
        || descriptorSkill.installProfile !== skill.installProfile
        || descriptorSkill.executionProfile.entrypointKind !== skill.entrypointKind
        || descriptorSkill.executionProfile.validationProfile !== skill.validationProfile
      ) {
        errors.push(bmadIssue("BMAD_INSTALLED_SKILL_TRANSPLANT", "installedSkills"));
      }
    }
  }

  const dependencyKeys = new Set();
  if (!bmadIsStrictlySortedUnique(
    document.dependencyAvailability,
    (dependency) => bmadCapabilityKey(dependency.capabilityKey),
  )) {
    errors.push(bmadIssue("BMAD_DEPENDENCY_SET_NOT_CANONICAL", "dependencyAvailability"));
  }
  for (const dependency of document.dependencyAvailability) {
    const encoded = bmadCapabilityKey(dependency.capabilityKey);
    if (installedKeys.has(encoded) || dependencyKeys.has(encoded)) {
      errors.push(bmadIssue("BMAD_CAPABILITY_KEY_COLLISION", "dependencyAvailability"));
    }
    dependencyKeys.add(encoded);
  }

  if (!bmadIsStrictlySortedUnique(
    document.helpActionGraph.actions,
    (action) => bmadCapabilityKey(action.capabilityKey),
  )) {
    errors.push(bmadIssue("BMAD_HELP_ACTION_SET_NOT_CANONICAL", "helpActionGraph.actions"));
  }
  if (
    document.helpActionGraph.packageVersionId !== document.packageVersionId
    || document.agentRoster.packageVersionId !== document.packageVersionId
  ) {
    errors.push(bmadIssue("BMAD_CATALOG_PACKAGE_BINDING_MISMATCH", "packageVersionId"));
  }
  for (const action of document.helpActionGraph.actions) {
    const encoded = bmadCapabilityKey(action.capabilityKey);
    if (!installedKeys.has(encoded) && !dependencyKeys.has(encoded)) {
      errors.push(bmadIssue("BMAD_HELP_ORPHAN", "helpActionGraph.actions"));
    }
  }

  if (!bmadIsStrictlySortedUnique(
    document.agentRoster.agents,
    (agent) => bmadTuple([agent.moduleCode, agent.agentCode]),
  )) {
    errors.push(bmadIssue("BMAD_AGENT_ROSTER_NOT_CANONICAL", "agentRoster.agents"));
  }
  const actualRoster = document.agentRoster.agents.map((agent) => [
    agent.agentCode,
    agent.name,
    agent.title,
    agent.icon,
    agent.team,
    agent.description,
    agent.personaSourceHash,
    agent.customizationSourceHash,
  ]);
  if (
    actualRoster.length !== EXPECTED_BMM_AGENTS.length
    || actualRoster.some((record) => {
      const expected = EXPECTED_BMM_AGENTS.find(([agentCode]) => agentCode === record[0]);
      return expected === undefined
        || record.some((value, field) => value !== expected[field]);
    })
    || document.agentRoster.agents.some((agent) => !bmadAgentRecordHashIsExact(agent))
  ) {
    errors.push(bmadIssue("BMAD_AGENT_ROSTER_BINDING_MISMATCH", "agentRoster.agents"));
  }
  for (const agent of document.agentRoster.agents) {
    const menuCodes = new Set();
    let lastOrdinal = -1;
    for (const item of agent.menuItems) {
      if (menuCodes.has(item.menuCode) || item.sourceOrdinal <= lastOrdinal) {
        errors.push(bmadIssue("BMAD_MENU_SCOPE_AMBIGUOUS", "agentRoster.agents.menuItems"));
      }
      menuCodes.add(item.menuCode);
      lastOrdinal = item.sourceOrdinal;
      const target = item.target;
      if (target.sourceCustomizationGraphHash !== agent.personaCustomizationGraphHash) {
        errors.push(bmadIssue("BMAD_MENU_TARGET_TRANSPLANT", "agentRoster.agents.menuItems.target"));
      }
      if (target.targetKind === "skill_target") {
        const encoded = bmadCapabilityKey(target.capabilityKey);
        if (!installedKeys.has(encoded) && !dependencyKeys.has(encoded)) {
          errors.push(bmadIssue("BMAD_AGENT_MENU_ORPHAN", "agentRoster.agents.menuItems.target"));
        }
      } else if (
        target.targetKind === "prompt_reference"
        && context.descriptor !== undefined
        && !bmadFindDescriptorResource(
          context.descriptor,
          target.sourceLocalMemberLabel,
          target.sourceMemberHash,
        )
      ) {
        errors.push(bmadIssue("BMAD_PROMPT_REFERENCE_TRANSPLANT", "agentRoster.agents.menuItems.target"));
      }
    }
    if (context.descriptor !== undefined) {
      const module = context.descriptor.modules.find((candidate) =>
        candidate.moduleCode === agent.moduleCode);
      if (module?.metadataSourceHash !== agent.moduleSourceHash) {
        errors.push(bmadIssue("BMAD_AGENT_MODULE_HASH_MISMATCH", "agentRoster.agents.moduleSourceHash"));
      }
      if (!context.descriptor.resourceInventory.some((resource) =>
        resource.contentHash === agent.personaSourceHash)) {
        errors.push(bmadIssue("BMAD_PERSONA_HASH_MISMATCH", "agentRoster.agents.personaSourceHash"));
      }
      if (!context.descriptor.resourceInventory.some((resource) =>
        resource.contentHash === agent.customizationSourceHash)) {
        errors.push(bmadIssue("BMAD_CUSTOMIZATION_HASH_MISMATCH", "agentRoster.agents.customizationSourceHash"));
      }
    }
    const expectedMenu = EXPECTED_BMM_MENUS[agent.agentCode];
    const skillTargets = agent.menuItems
      .filter((item) => item.target.targetKind === "skill_target")
      .map((item) => [
        item.menuCode,
        item.target.capabilityKey.skillName,
        item.target.capabilityKey.normalizedAction,
      ]);
    if (
      expectedMenu !== undefined
      && (
        skillTargets.length !== expectedMenu.length
        || skillTargets.some((record, index) =>
          record.some((value, field) => value !== expectedMenu[index][field]))
      )
    ) {
      errors.push(bmadIssue("BMAD_AGENT_MENU_BINDING_MISMATCH", "agentRoster.agents.menuItems"));
    }
  }
}

function validateMethodSession(document, errors, context) {
  const envelope = context.envelope;
  const localAuthority = envelope?.authorityRef?.authorityKind === "desktop_local_store";
  const managedAuthority = envelope?.authorityRef?.authorityKind === "azure_control_plane";
  if (
    envelope === undefined
    || envelope.objectId !== document.sessionId
    || (envelope.deliveryModel === "windows_local" && !localAuthority)
    || (envelope.deliveryModel === "web_managed" && !managedAuthority)
  ) {
    errors.push(bmadIssue("BMAD_METHOD_ENVELOPE_BINDING_MISMATCH", "envelope"));
  }
  if (
    document.executionProfile.profileHash !== document.executionProfileHash
    || document.executionProfile.validationProfile !== document.validationProfile
  ) {
    errors.push(bmadIssue("BMAD_METHOD_PROFILE_BINDING_MISMATCH", "executionProfile"));
  }
  const isHelp = document.methodShape === "no_agent_direct";
  if (isHelp) {
    if (
      document.capabilityKey.moduleCode !== "core"
      || document.capabilityKey.skillName !== "bmad-help"
      || document.capabilityKey.normalizedAction !== null
      || document.agentBinding.bindingKind !== "no_agent"
      || document.agentRosterHash !== null
      || document.executionProfile.entrypointKind !== "direct"
      || document.executionProfile.invocationModes.actions.length !== 0
      || document.validationProfile !== "MethodOfficialSkillV6"
    ) {
      errors.push(bmadIssue("BMAD_HELP_BINDING_MISMATCH", "capabilityKey"));
    }
  } else if (
    document.capabilityKey.moduleCode !== "bmm"
    || document.capabilityKey.skillName !== "bmad-architecture"
    || document.capabilityKey.normalizedAction !== "create"
    || document.executionProfile.entrypointKind !== "step_jit"
    || !bmadSetEquals(document.executionProfile.invocationModes.actions, ["create"])
    || document.executionProfile.resourcePolicy.resourceTiming !== "current_step_only"
    || document.validationProfile !== "MethodStepWorkflowV6"
    || document.agentBinding.rosterHash !== document.agentRosterHash
    || document.agentBinding.moduleSourceHash !== EXPECTED_BMM_MODULE_HASH
    || document.agentBinding.personaHash !== "sha256:6d3512c6f9014a2344418ce0b53b1c9ed8521e6bf8b337f2a802ade6307146e4"
    || document.agentBinding.customizationGraphHash !== "sha256:d9763009d7c20246119c24bcea5eacebd21ad60c22ab191b74c9a5fb6e5f57ad"
    || !bmadSameCapability(document.agentBinding.menuCapabilityKey, document.capabilityKey)
  ) {
    errors.push(bmadIssue("BMAD_ARCHITECT_BINDING_MISMATCH", "agentBinding"));
  }
  if (context.catalog !== undefined) {
    const catalog = context.catalog;
    const installed = catalog.installedSkills.find((skill) =>
      skill.capabilityKeys.some((key) => bmadSameCapability(key, document.capabilityKey)));
    if (
      catalog.packageVersionId !== document.packageVersionId
      || catalog.descriptorHash !== document.packageDescriptorHash
      || catalog.packageSourceHash !== document.packageSourceHash
      || catalog.catalogHash !== document.capabilityCatalogHash
      || installed === undefined
      || installed.instructionProjectionHash !== document.instructionProjectionHash
      || installed.resourceSetHash !== document.resourceSetHash
      || installed.executionProfileHash !== document.executionProfileHash
      || installed.validationProfile !== document.validationProfile
      || installed.distributionProfile !== document.distributionProfile
      || installed.installProfile !== document.installProfile
    ) {
      errors.push(bmadIssue("BMAD_METHOD_CATALOG_BINDING_MISMATCH", "capabilityCatalogHash"));
    }
    if (!isHelp) {
      const agent = catalog.agentRoster.agents.find((candidate) =>
        candidate.agentCode === document.agentBinding.agentCode);
      const menuItem = agent?.menuItems.find((item) => item.menuCode === document.agentBinding.menuCode);
      if (
        catalog.agentRoster.rosterHash !== document.agentRosterHash
        || agent === undefined
        || menuItem === undefined
        || agent.agentRecordHash !== document.agentBinding.agentRecordHash
        || agent.moduleSourceHash !== document.agentBinding.moduleSourceHash
        || agent.name !== document.agentBinding.agentName
        || agent.title !== document.agentBinding.agentTitle
        || agent.personaCustomizationGraphHash !== document.agentBinding.customizationGraphHash
        || menuItem.sourceMenuItemHash !== document.agentBinding.menuItemHash
        || menuItem.target.targetKind !== "skill_target"
        || !bmadSameCapability(menuItem.target.capabilityKey, document.capabilityKey)
      ) {
        errors.push(bmadIssue("BMAD_METHOD_AGENT_CATALOG_TRANSPLANT", "agentBinding"));
      }
    }
  }
  if (
    document.contextLedger.sessionId !== document.sessionId
    || document.contextLedger.entries.length !== document.decisionConsumptions.length
    || document.checkpoints.length !== document.decisionConsumptions.length
  ) {
    errors.push(bmadIssue("BMAD_CONTEXT_LEDGER_BINDING_MISMATCH", "contextLedger"));
  }
  let previousOrdinal = 0;
  const checkpointIds = new Set();
  const checkpointDecisions = new Set();
  for (const checkpoint of document.checkpoints) {
    if (
      checkpoint.sessionId !== document.sessionId
      || checkpoint.turnOrdinal !== previousOrdinal + 1
      || checkpointIds.has(checkpoint.checkpointId)
      || checkpointDecisions.has(checkpoint.contextDecisionId)
      || !bmadSameCapability(checkpoint.capabilityKey, document.capabilityKey)
      || checkpoint.contextDigest !== document.contextDigest
      || checkpoint.modelBindingHash !== document.modelBinding.bindingHash
    ) {
      errors.push(bmadIssue("BMAD_TURN_ORDINAL_INVALID", "checkpoints"));
    }
    previousOrdinal = checkpoint.turnOrdinal;
    checkpointIds.add(checkpoint.checkpointId);
    checkpointDecisions.add(checkpoint.contextDecisionId);
  }

  const ledgerByDecision = new Map();
  let reviewOrdinal = 0;
  for (const entry of document.contextLedger.entries) {
    if (
      entry.reviewOrdinal !== reviewOrdinal + 1
      || ledgerByDecision.has(entry.contextDecisionId)
      || entry.contextDigest !== document.contextDigest
      || entry.resourceSetHash !== document.resourceSetHash
      || entry.packageDescriptorHash !== document.packageDescriptorHash
      || entry.instructionProjectionHash !== document.instructionProjectionHash
      || entry.configResolutionHash !== document.configResolutionHash
      || entry.customizationHash !== document.customizationHash
      || entry.modelBindingHash !== document.modelBinding.bindingHash
      || entry.methodSchemaHash !== document.methodSchemaHash
      || entry.executionProfileHash !== document.executionProfileHash
      || entry.validationProfileHash !== document.validationProfileHash
    ) {
      errors.push(bmadIssue("BMAD_CONTEXT_LEDGER_BINDING_MISMATCH", "contextLedger.entries"));
    }
    reviewOrdinal = entry.reviewOrdinal;
    ledgerByDecision.set(entry.contextDecisionId, entry);
  }
  const decisions = new Set();
  const invocations = new Set();
  for (const consumption of document.decisionConsumptions) {
    const entry = ledgerByDecision.get(consumption.decisionId);
    const exactBinding =
      consumption.sessionId === document.sessionId
      && entry !== undefined
      && checkpointDecisions.has(consumption.decisionId)
      && entry.manifestHash === consumption.manifestHash
      && entry.consentHash === consumption.consentHash
      && consumption.packageDescriptorHash === document.packageDescriptorHash
      && consumption.packageSourceHash === document.packageSourceHash
      && consumption.instructionProjectionHash === document.instructionProjectionHash
      && consumption.capabilityCatalogHash === document.capabilityCatalogHash
      && bmadSameCapability(consumption.capabilityKey, document.capabilityKey)
      && consumption.contextDigest === document.contextDigest
      && consumption.distributionProfile === document.distributionProfile
      && consumption.installProfile === document.installProfile
      && consumption.executionProfileHash === document.executionProfileHash
      && consumption.validationProfileHash === document.validationProfileHash
      && consumption.configResolutionHash === document.configResolutionHash
      && consumption.customizationHash === document.customizationHash
      && consumption.resourceSetHash === document.resourceSetHash
      && bmadSameModelBinding(consumption.modelBinding, document.modelBinding)
      && consumption.methodSchemaHash === document.methodSchemaHash;
    if (
      decisions.has(consumption.decisionId)
      || invocations.has(consumption.invocationId)
      || !exactBinding
    ) {
      errors.push(bmadIssue("BMAD_CONTEXT_DECISION_REUSED", "decisionConsumptions"));
    }
    decisions.add(consumption.decisionId);
    invocations.add(consumption.invocationId);
  }
}

function validateBuilderPath(path, errors) {
  const bytes = bmadUtf8Length(path);
  const segments = path.split("/");
  if (
    path !== path.normalize("NFC")
    || bytes > 240
    || segments.length > 16
    || segments.some((segment) =>
      bmadUtf8Length(segment) > 120
      || segment.endsWith(".")
      || segment.endsWith(" ")
      || WINDOWS_RESERVED_SEGMENT.test(segment))
  ) {
    errors.push(bmadIssue("BMAD_BUILDER_PATH_INVALID", "proposedFileSet.files.path"));
  }
}

function validateBuilderInventory(document, errors) {
  const fileSet = document.proposedFileSet;
  if (fileSet.limitProfile !== BMAD_BUILDER_LIMIT_PROFILE) {
    errors.push(bmadIssue("BMAD_BUILDER_LIMIT_PROFILE_MISMATCH", "proposedFileSet.limitProfile"));
  }
  let totalBytes = 0;
  const caseFoldedPaths = new Set();
  const paths = [];
  for (const file of fileSet.files) {
    validateBuilderPath(file.path, errors);
    const pathKey = file.path.toLowerCase();
    if (caseFoldedPaths.has(pathKey)) {
      errors.push(bmadIssue("BMAD_BUILDER_PATH_COLLISION", "proposedFileSet.files.path"));
    }
    caseFoldedPaths.add(pathKey);
    const fileBytes = bmadUtf8Length(file.content);
    if (fileBytes > 262_144) {
      errors.push(bmadIssue("BMAD_BUILDER_FILE_TOO_LARGE", "proposedFileSet.files.content"));
    }
    totalBytes += fileBytes;
    paths.push(file.path);
  }
  if (totalBytes > 1_048_576) {
    errors.push(bmadIssue("BMAD_BUILDER_TOTAL_TOO_LARGE", "proposedFileSet.files"));
  }

  const sortedPaths = [...paths].sort();
  if (document.builderKind === "workflow") {
    if (!bmadSetEquals(sortedPaths, ["SKILL.md"])) {
      errors.push(bmadIssue("BMAD_BUILDER_INVENTORY_INVALID", "proposedFileSet.files"));
    }
  } else {
    const mandatory = new Set([
      "SKILL.md",
      "customize.toml",
      "references/prompt-quality-canon.md",
    ]);
    const capabilityReferences = paths.filter((path) =>
      path.startsWith("references/") && path !== "references/prompt-quality-canon.md");
    if (
      [...mandatory].some((path) => !paths.includes(path))
      || paths.some((path) =>
        !mandatory.has(path) && !/^references\/[a-z][a-z0-9-]*\.md$/u.test(path))
      || capabilityReferences.length > 13
    ) {
      errors.push(bmadIssue("BMAD_BUILDER_INVENTORY_INVALID", "proposedFileSet.files"));
    }
  }
}

function validateBuilder(document, errors) {
  const expectedProfile = document.builderKind === "agent"
    ? "BuilderAgentV2Stateless"
    : "BuilderOutcomeSkillV2";
  if (document.validationProfile !== expectedProfile) {
    errors.push(bmadIssue("BMAD_PROFILE_AMBIGUOUS", "validationProfile"));
  }
  if (
    document.authoringAction !== undefined
    && (
      document.authoringAction.builderKind !== document.builderKind
      || !({
        draft: {
          agent: ["create_rebuild"],
          workflow: ["build"],
        },
        revision: {
          agent: ["create_rebuild", "edit"],
          workflow: ["build", "edit"],
        },
      }[document.objectKind]?.[document.builderKind] ?? []).includes(document.authoringAction.action)
    )
  ) {
    errors.push(bmadIssue("BMAD_ACTION_UNSUPPORTED", "authoringAction"));
  }
  if (document.objectKind === "revision") validateBuilderInventory(document, errors);
  if (document.objectKind !== "analysis") return;

  const totalFindings = document.deterministicFindings.length
    + (document.modelLensResults ?? []).reduce((total, lens) => total + lens.findings.length, 0);
  if (totalFindings > 512) {
    errors.push(bmadIssue("BMAD_BUILDER_FINDING_LIMIT_EXCEEDED", "deterministicFindings"));
  }
  if (document.analysisKind === "model_lens") {
    const expectedLenses = document.builderKind === "agent" ? AGENT_LENSES : WORKFLOW_LENSES;
    const actualLenses = document.modelLensResults.map((result) => result.lens);
    if (!bmadSetEquals(actualLenses, expectedLenses)) {
      errors.push(bmadIssue("BMAD_MODEL_LENS_SET_INVALID", "modelLensResults"));
    }
    for (const result of document.modelLensResults) {
      if (
        result.builderKind !== document.builderKind
        || result.revisionId !== document.revisionId
        || result.revisionHash !== document.revisionHash
        || result.sourceMemberSetHash !== document.sourceMemberSetHash
        || result.instructionProjectionSetHash !== document.instructionProjectionSetHash
        || result.deterministicFactsHash !== document.deterministicFactsHash
        || result.modelHash !== document.modelBinding.modelHash
        || result.deploymentHash !== document.modelBinding.deploymentHash
        || result.modelProfileHash !== document.modelBinding.modelProfileHash
        || result.schemaHash !== document.modelBinding.schemaHash
        || result.consentHash !== document.modelBinding.consentHash
        || result.contextDecisionConsumptionHash
          !== document.modelBinding.contextDecisionConsumptionHash
      ) {
        errors.push(bmadIssue("BMAD_MODEL_LENS_BINDING_MISMATCH", "modelLensResults"));
      }
    }
  }
}

/**
 * Runs repository-owned BMAD semantic validation after strict parsing and
 * structural validation. The optional descriptor supplies cross-record source
 * closure; it never grants authority.
 */
export function validateBmadSemantics(document, context = {}) {
  const errors = [];
  if (document.schemaVersion === "sapphirus.bmad-package-descriptor.v1") {
    validateDescriptor(document, errors);
  } else if (document.schemaVersion === "sapphirus.bmad-capability-catalog.v1") {
    validateCatalog(document, errors, context);
  } else if (document.schemaVersion === "sapphirus.bmad-method-session.v1") {
    validateMethodSession(document, errors, context);
  } else if (
    document.schemaVersion === "sapphirus.bmad-builder-authoring.v1"
    || document.schemaVersion === "sapphirus.bmad-builder-revision.v1"
    || document.schemaVersion === "sapphirus.bmad-builder-analysis.v1"
  ) {
    validateBuilder(document, errors);
  }
  return errors.filter((error, index) =>
    errors.findIndex((candidate) => candidate.code === error.code) === index);
}

export function bmadContextDecisionUniquenessKey(document) {
  if (document.recordKind !== "context_decision_consumption") {
    throw new Error("BMAD context decision key requires a consumption record.");
  }
  const key = bmadTuple([document.decisionId]);
  if (key === null) throw new Error("BMAD consumption key fields must be NUL-free strings.");
  return key;
}
