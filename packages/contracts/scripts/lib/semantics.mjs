import { canonicalHash } from "./canonical-json.mjs";
import {
  validateContractErrorSemantics,
  validatePackageCompatibilitySemantics,
  validateRemoteJobHandoffSemantics,
} from "./compatibility-semantics.mjs";
import {
  bmadContextDecisionUniquenessKey,
  validateBmadSemantics,
} from "./bmad-semantics.mjs";

export { validateRemoteJobHandoffTransition } from "./compatibility-semantics.mjs";
export { bmadContextDecisionUniquenessKey, validateBmadSemantics } from "./bmad-semantics.mjs";

const HASH_RULES = Object.freeze({
  "sapphirus.candidate-action.v1": {
    hashField: "candidateHash",
    excludedFields: ["candidateHash"],
    purpose: "candidate-action",
    schemaMajor: "v1",
  },
  "sapphirus.approved-execution-spec.v1": {
    hashField: "specHash",
    excludedFields: ["specHash"],
    purpose: "approved-execution-spec",
    schemaMajor: "v1",
  },
  "sapphirus.spec-consumption.v1": {
    hashField: "consumptionHash",
    excludedFields: ["consumptionHash"],
    purpose: "spec-consumption",
    schemaMajor: "v1",
  },
  "sapphirus.execution-result-manifest.v1": {
    hashField: "manifestHash",
    excludedFields: ["manifestHash"],
    purpose: "execution-result-manifest",
    schemaMajor: "v1",
  },
  "sapphirus.evidence-event.v2": {
    hashField: "eventHash",
    excludedFields: ["eventHash"],
    purpose: "evidence-event",
    schemaMajor: "v2",
  },
  "sapphirus.remote-job-handoff.v1": {
    hashField: "handoffHash",
    excludedFields: ["handoffHash"],
    purpose: "remote-job-handoff",
    schemaMajor: "v1",
  },
  "sapphirus.package-compatibility.v1": {
    hashField: "signedPayloadHash",
    excludedFields: ["signedPayloadHash", "signature"],
    purpose: "package-compatibility",
    schemaMajor: "v1",
  },
  "sapphirus.bmad-package-descriptor.v1": {
    hashField: "descriptorHash",
    excludedFields: ["descriptorHash"],
    purpose: "bmad-package-descriptor",
    schemaMajor: "v1",
  },
  "sapphirus.bmad-capability-catalog.v1": {
    hashField: "catalogHash",
    excludedFields: ["catalogHash"],
    purpose: "bmad-capability-catalog",
    schemaMajor: "v1",
  },
  "sapphirus.bmad-method-checkpoint.v1": {
    hashField: "checkpointHash",
    excludedFields: ["checkpointHash"],
    purpose: "bmad-method-checkpoint",
    schemaMajor: "v1",
  },
  "sapphirus.bmad-method-session.v1": {
    hashField: "contentHash",
    excludedFields: ["contentHash"],
    purpose: "contract-object",
    schemaMajor: "v1",
  },
  "sapphirus.bmad-builder-revision.v1": {
    hashField: "revisionHash",
    excludedFields: ["revisionHash"],
    purpose: "bmad-builder-revision",
    schemaMajor: "v1",
  },
  "sapphirus.bmad-builder-analysis.v1": {
    hashField: "analysisHash",
    excludedFields: ["analysisHash"],
    purpose: "bmad-builder-analysis",
    schemaMajor: "v1",
  },
  "sapphirus.bmad-validation-report.v1": {
    hashField: "reportHash",
    excludedFields: ["reportHash"],
    purpose: "bmad-validation-report",
    schemaMajor: "v1",
  },
});

function isStrictlySortedUnique(values, keySelector) {
  let previous;
  for (const value of values) {
    const current = keySelector(value);
    if (previous !== undefined && previous >= current) {
      return false;
    }
    previous = current;
  }
  return true;
}

function validateRelativePath(value, field, errors) {
  if (value !== value.normalize("NFC")) {
    errors.push({ code: "PATH_NOT_NFC", field });
  }
  if (
    value !== "." &&
    (value.startsWith("/") ||
      /^[A-Za-z]:/.test(value) ||
      value.includes(":") ||
      value.includes("\\") ||
      value.includes("\0") ||
      value.split("/").some((segment) => segment === "." || segment === ".."))
  ) {
    errors.push({ code: "PATH_NOT_RELATIVE_NORMALIZED", field });
  }
}

function validateInstant(value, field, errors) {
  const epochMilliseconds = Date.parse(value);
  if (
    !Number.isFinite(epochMilliseconds) ||
    new Date(epochMilliseconds).toISOString() !== value
  ) {
    errors.push({ code: "INVALID_UTC_INSTANT", field });
  }
}

function verifySelfHash(document, errors) {
  const rule = HASH_RULES[document.schemaVersion];
  if (rule === undefined) return;

  const expected = canonicalHash({
    purpose: rule.purpose,
    schemaMajor: rule.schemaMajor,
    value: document,
    excludedFields: rule.excludedFields,
  }).serializedHash;
  if (document[rule.hashField] !== expected) {
    errors.push({ code: "HASH_MISMATCH", field: rule.hashField, expected });
  }
}

export function validateSemantics(document, context = {}) {
  const errors = [];

  if (document.schemaVersion === "sapphirus.candidate-action.v1") {
    validateInstant(document.createdAt, "createdAt", errors);
    validateInstant(document.expiresAt, "expiresAt", errors);
    if (
      !isStrictlySortedUnique(
        document.mutableInputs,
        (item) => `${item.inputKind}\u0000${item.inputId}`,
      )
    ) {
      errors.push({ code: "MUTABLE_INPUT_SET_NOT_CANONICAL", field: "mutableInputs" });
    }
    if (
      !isStrictlySortedUnique(
        document.declaredWrites,
        (item) => `${item.pathPattern}\u0000${item.operation}`,
      )
    ) {
      errors.push({ code: "DECLARED_WRITE_SET_NOT_CANONICAL", field: "declaredWrites" });
    }
    if (
      !isStrictlySortedUnique(document.preimages, (item) => item.relativePath)
    ) {
      errors.push({ code: "PREIMAGE_SET_NOT_CANONICAL", field: "preimages" });
    }
    for (const item of document.declaredWrites) {
      validateRelativePath(item.pathPattern, "declaredWrites.pathPattern", errors);
    }
    for (const item of document.preimages) {
      validateRelativePath(item.relativePath, "preimages.relativePath", errors);
    }
    if (Date.parse(document.expiresAt) <= Date.parse(document.createdAt)) {
      errors.push({ code: "CANDIDATE_EXPIRY_INVALID", field: "expiresAt" });
    }
  }

  if (document.schemaVersion === "sapphirus.approved-execution-spec.v1") {
    validateInstant(document.issuedAt, "issuedAt", errors);
    validateInstant(document.expiresAt, "expiresAt", errors);
    if (Date.parse(document.expiresAt) <= Date.parse(document.issuedAt)) {
      errors.push({ code: "SPEC_EXPIRY_INVALID", field: "expiresAt" });
    }
    const forbiddenMutableFields = ["consumed", "consumedAt", "result", "remainingUses"];
    for (const field of forbiddenMutableFields) {
      if (Object.hasOwn(document, field)) {
        errors.push({ code: "MUTABLE_SPEC_FIELD_FORBIDDEN", field });
      }
    }
  }

  if (document.schemaVersion === "sapphirus.spec-consumption.v1") {
    validateInstant(document.consumedAt, "consumedAt", errors);
  }

  if (document.schemaVersion === "sapphirus.execution-result-manifest.v1") {
    validateInstant(document.startedAt, "startedAt", errors);
    validateInstant(document.completedAt, "completedAt", errors);
    if (
      !isStrictlySortedUnique(document.changedFiles, (item) => item.relativePath)
    ) {
      errors.push({ code: "CHANGED_FILE_SET_NOT_CANONICAL", field: "changedFiles" });
    }
    for (const item of document.changedFiles) {
      validateRelativePath(item.relativePath, "changedFiles.relativePath", errors);
    }
    if (Date.parse(document.completedAt) < Date.parse(document.startedAt)) {
      errors.push({ code: "RESULT_TIME_RANGE_INVALID", field: "completedAt" });
    }
  }

  if (document.schemaVersion === "sapphirus.evidence-event.v2") {
    validateInstant(document.occurredAt, "occurredAt", errors);
    if (document.sequence === 1 && document.previousEventHash !== null) {
      errors.push({ code: "EVIDENCE_GENESIS_PREVIOUS_HASH", field: "previousEventHash" });
    }
    if (document.sequence > 1 && document.previousEventHash === null) {
      errors.push({ code: "EVIDENCE_CHAIN_PREVIOUS_HASH_REQUIRED", field: "previousEventHash" });
    }
  }

  if (document.schemaVersion === "sapphirus.filesystem-capability.v1") {
    validateInstant(document.capturedAt, "capturedAt", errors);
  }

  if (document.schemaVersion === "sapphirus.error.v1") {
    errors.push(...validateContractErrorSemantics(document));
  }

  if (document.schemaVersion === "sapphirus.package-compatibility.v1") {
    errors.push(...validatePackageCompatibilitySemantics(document));
  }

  if (document.schemaVersion === "sapphirus.remote-job-handoff.v1") {
    errors.push(...validateRemoteJobHandoffSemantics(document));
  }

  if (typeof document.schemaVersion === "string" && document.schemaVersion.startsWith("sapphirus.bmad-")) {
    errors.push(...validateBmadSemantics(document, context));
    for (const checkpoint of document.checkpoints ?? []) verifySelfHash(checkpoint, errors);
  }

  verifySelfHash(document, errors);
  return errors;
}

export function sealDocument(document) {
  const rule = HASH_RULES[document.schemaVersion];
  if (rule === undefined) {
    throw new Error(`No self-hash rule for ${document.schemaVersion}.`);
  }
  const copy = structuredClone(document);
  copy[rule.hashField] = canonicalHash({
    purpose: rule.purpose,
    schemaMajor: rule.schemaMajor,
    value: copy,
    excludedFields: rule.excludedFields,
  }).serializedHash;
  return copy;
}

export function sealDurableObject(document) {
  const copy = structuredClone(document);
  copy.envelope.contentHash = canonicalHash({
    purpose: "contract-object",
    schemaMajor: "v1",
    value: copy.payload,
  }).serializedHash;
  return copy;
}

export function validateDurableObjectHash(document) {
  const expected = canonicalHash({
    purpose: "contract-object",
    schemaMajor: "v1",
    value: document.payload,
  }).serializedHash;
  return document.envelope.contentHash === expected
    ? []
    : [{ code: "HASH_MISMATCH", field: "envelope.contentHash", expected }];
}

export function specConsumptionUniquenessKey(document) {
  if (document.schemaVersion !== "sapphirus.spec-consumption.v1") {
    throw new Error("Consumption uniqueness keys require a v1 consumption record.");
  }
  return canonicalizeTuple([
    document.specHash,
    document.singleUseNonceHash,
    document.executorAudienceHash,
  ]);
}

export function contextDecisionUniquenessKey(document) {
  return bmadContextDecisionUniquenessKey(document);
}

function canonicalizeTuple(values) {
  for (const value of values) {
    if (typeof value !== "string" || value.includes("\u0000")) {
      throw new Error("Consumption key components must be NUL-free strings.");
    }
  }
  return values.join("\u0000");
}
