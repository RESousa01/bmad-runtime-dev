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
