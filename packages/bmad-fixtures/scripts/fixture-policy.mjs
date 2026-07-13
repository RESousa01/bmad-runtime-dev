import { Buffer } from "node:buffer";
import { createHash } from "node:crypto";
import { lstat, readFile, readdir, realpath } from "node:fs/promises";
import path from "node:path";
import { TextDecoder } from "node:util";
import { fileURLToPath } from "node:url";

const defaultPackageRoot = fileURLToPath(new URL("../", import.meta.url));
const defaultRepositoryRoot = fileURLToPath(new URL("../../../", import.meta.url));
const MAX_DESCRIPTOR_BYTES = 64 * 1024;
const MAX_JSON_DEPTH = 32;

const DESCRIPTOR_FIELDS = Object.freeze([
  "schemaVersion",
  "fixtureId",
  "fixtureKind",
  "distributionProfile",
  "validationProfile",
  "executionProfile",
  "activationState",
  "builderActions",
  "scriptExecution",
  "networkAccess",
  "source",
  "payload",
  "assertions",
]);
const SOURCE_FIELDS = Object.freeze([
  "project",
  "packageVersion",
  "license",
  "relativePath",
  "byteLength",
  "sha256",
]);
const PAYLOAD_FIELDS = Object.freeze([
  "relativePath",
  "byteLength",
  "sha256",
]);

const LOCKED_FIXTURES = Object.freeze({
  method_direct_skill: Object.freeze({
    descriptorName: "sealed-method-bmad-help.json",
    fixtureId: "bmad_method_help_v6",
    distributionProfile: "method_source_tree",
    validationProfile: "MethodOfficialSkillV6",
    executionProfile: "direct",
    activationState: "sealed_read_only",
    builderActions: Object.freeze([]),
    source: Object.freeze({
      project: "BMAD-METHOD",
      packageVersion: "6.10.0",
      license: "MIT",
      relativePath:
        "bmad-runtime-lib/_source_review/BMAD-METHOD-main/BMAD-METHOD-main/src/core-skills/bmad-help/SKILL.md",
      byteLength: 4617,
      sha256:
        "718077d741e20d9c94f3c2b7827047f2d18a90b85c3cc2eecd449e28b7b0d642",
    }),
    payload: null,
    assertions: Object.freeze({
      sourceDerived: true,
      officialPrefix: true,
    }),
  }),
  builder_stateless_agent: Object.freeze({
    descriptorName: "inactive-stateless-agent.json",
    fixtureId: "bmad_builder_stateless_agent_v1",
    distributionProfile: "builder_source_tree",
    validationProfile: "BuilderAgentV2Stateless",
    executionProfile: "direct",
    activationState: "not_active",
    builderActions: Object.freeze(["Build", "Edit", "Analyze"]),
    source: Object.freeze({
      project: "bmad-builder",
      packageVersion: "2.1.0",
      license: "MIT",
      relativePath:
        "bmad-runtime-lib/_source_review/bmad-builder-main/bmad-builder-main/skills/bmad-agent-builder/SKILL.md",
      byteLength: 5686,
      sha256:
        "806ea0a5c3bd9d4ef5dfa2e0beb37490b0fb3faef848ac493db2db0e99f32dda",
    }),
    payload: Object.freeze({
      relativePath:
        "packages/bmad-fixtures/fixtures/payloads/stateless-agent/SKILL.md",
      byteLength: 625,
      sha256:
        "eddd5da0ba4e2ace5825e6e8578631230254387583a50a4e0889ab1eddb79317",
    }),
    assertions: Object.freeze({
      agentType: "stateless",
      sanctumPresent: false,
      usesReservedPrefix: false,
    }),
  }),
  builder_simple_workflow: Object.freeze({
    descriptorName: "inactive-simple-workflow.json",
    fixtureId: "bmad_builder_simple_workflow_v1",
    distributionProfile: "builder_source_tree",
    validationProfile: "BuilderOutcomeSkillV2",
    executionProfile: "inline",
    activationState: "not_active",
    builderActions: Object.freeze(["Build", "Edit", "Analyze"]),
    source: Object.freeze({
      project: "bmad-builder",
      packageVersion: "2.1.0",
      license: "MIT",
      relativePath:
        "bmad-runtime-lib/_source_review/bmad-builder-main/bmad-builder-main/skills/bmad-workflow-builder/SKILL.md",
      byteLength: 4528,
      sha256:
        "ed28d89b38b1821fce92e09845e94300a4b3d2ec94e8ce7e86e8fa6fe170a644",
    }),
    payload: Object.freeze({
      relativePath:
        "packages/bmad-fixtures/fixtures/payloads/simple-workflow/SKILL.md",
      byteLength: 628,
      sha256:
        "a49b5d2a4e31429766e9513ff08b67e3bdf92f18e399ae251c0d8b620319432c",
    }),
    assertions: Object.freeze({
      inlineFirst: true,
      usesReservedPrefix: false,
    }),
  }),
});

export const EXPECTED_DESCRIPTOR_NAMES = Object.freeze(
  Object.values(LOCKED_FIXTURES)
    .map((fixture) => fixture.descriptorName)
    .sort(),
);

const AUTHORITY_KEY_PATTERNS = Object.freeze([
  /^(argv|argumentvector|command|commands|commandline|commandargs?)$/,
  /^(env|environment|environments|environmentvariables?|envpassthrough)$/,
  /hook/,
  /process/,
  /shell/,
  /spawn/,
  /runner/,
  /^run/,
  /execution/,
  /executable/,
  /^(cwd|workingdirectory)$/,
  /^(stdin|stdout|stderr)$/,
  /network/,
  /egress/,
  /socket/,
  /endpoint/,
  /^(http|https|url|urls)$/,
  /grant/,
  /permission/,
  /privilege/,
  /authority/,
  /capabilit/,
  /credential/,
  /secret/,
  /token/,
  /tool/,
  /filesystem/,
  /fileaccess/,
  /write/,
  /delete/,
  /cleanup/,
  /convert/,
  /eval/,
  /rehears/,
  /script/,
  /activat/,
  /promot/,
  /publish/,
  /install/,
]);

const SAFE_ROOT_AUTHORITY_FIELDS = Object.freeze({
  activationstate: Object.freeze(["sealed_read_only", "not_active"]),
  executionprofile: Object.freeze(["direct", "inline"]),
  networkaccess: Object.freeze(["blocked"]),
  scriptexecution: Object.freeze(["blocked"]),
});

export class FixtureValidationError extends Error {
  constructor(code, location, message, options) {
    super(`${code} at ${location}: ${message}`, options);
    this.name = "FixtureValidationError";
    this.code = code;
    this.location = location;
  }
}

function fail(code, location, message, options) {
  throw new FixtureValidationError(code, location, message, options);
}

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function requireRecord(value, location) {
  if (!isRecord(value)) {
    fail("SCHEMA_TYPE", location, "expected an object");
  }
}

function requireExactFields(value, fields, location) {
  requireRecord(value, location);
  const expected = new Set(fields);

  for (const field of fields) {
    if (!Object.hasOwn(value, field)) {
      fail("SCHEMA_MISSING_FIELD", location, `missing required field ${field}`);
    }
  }
  for (const field of Object.keys(value)) {
    if (!expected.has(field)) {
      fail("SCHEMA_UNKNOWN_FIELD", `${location}.${field}`, "field is not allowed");
    }
  }
}

function normalizeKey(key) {
  return key.toLowerCase().replace(/[^a-z0-9]/gu, "");
}

function rejectAuthorityBearingKeys(value, location = "$", isRoot = true) {
  if (Array.isArray(value)) {
    value.forEach((item, index) =>
      rejectAuthorityBearingKeys(item, `${location}[${index}]`, false),
    );
    return;
  }
  if (!isRecord(value)) {
    return;
  }

  for (const [key, child] of Object.entries(value)) {
    const normalized = normalizeKey(key);
    const hasSafeRootRule =
      isRoot && Object.hasOwn(SAFE_ROOT_AUTHORITY_FIELDS, normalized);
    if (hasSafeRootRule) {
      const safeValues = SAFE_ROOT_AUTHORITY_FIELDS[normalized];
      if (!safeValues.includes(child)) {
        fail(
          "AUTHORITY_BEARING_FIELD",
          `${location}.${key}`,
          `field is descriptive or deny-only and must be one of ${safeValues.join(", ")}`,
        );
      }
    } else if (AUTHORITY_KEY_PATTERNS.some((pattern) => pattern.test(normalized))) {
      fail(
        "AUTHORITY_BEARING_FIELD",
        `${location}.${key}`,
        "descriptor keys cannot carry operational authority or deferred capability claims",
      );
    }

    rejectAuthorityBearingKeys(child, `${location}.${key}`, false);
  }
}

function requireExactValue(actual, expected, location) {
  if (!Object.is(actual, expected)) {
    fail(
      "IDENTITY_MISMATCH",
      location,
      `expected ${JSON.stringify(expected)}, received ${JSON.stringify(actual)}`,
    );
  }
}

function requireString(value, location) {
  if (typeof value !== "string" || value.length === 0) {
    fail("SCHEMA_TYPE", location, "expected a non-empty string");
  }
}

function requirePortableRepositoryPath(value, location) {
  requireString(value, location);
  const segments = value.split("/");
  if (
    value.includes("\\") ||
    value.includes("\0") ||
    value.startsWith("/") ||
    /^[a-zA-Z]:/u.test(value) ||
    segments.some((segment) => segment === "" || segment === "." || segment === "..")
  ) {
    fail(
      "SCHEMA_PATH",
      location,
      "expected a normalized repository-relative path with forward slashes",
    );
  }
}

function requireDigest(value, location) {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/u.test(value)) {
    fail("SCHEMA_DIGEST", location, "expected a lowercase SHA-256 digest");
  }
}

function requireByteLength(value, location) {
  if (!Number.isSafeInteger(value) || value <= 0) {
    fail("SCHEMA_BYTE_LENGTH", location, "expected a positive safe integer");
  }
}

function validateLockedRecord(actual, expected, fields, location) {
  requireExactFields(actual, fields, location);
  for (const field of fields) {
    requireExactValue(actual[field], expected[field], `${location}.${field}`);
  }
}

function validateContentBinding(actual, expected, location) {
  const fields = expected.project === undefined ? PAYLOAD_FIELDS : SOURCE_FIELDS;
  requireExactFields(actual, fields, location);

  if (expected.project !== undefined) {
    requireString(actual.project, `${location}.project`);
    requireString(actual.packageVersion, `${location}.packageVersion`);
    requireExactValue(actual.license, "MIT", `${location}.license`);
  }
  requirePortableRepositoryPath(actual.relativePath, `${location}.relativePath`);
  requireByteLength(actual.byteLength, `${location}.byteLength`);
  requireDigest(actual.sha256, `${location}.sha256`);

  validateLockedRecord(actual, expected, fields, location);
}

function validateBuilderActions(actual, expected, location) {
  if (!Array.isArray(actual)) {
    fail("SCHEMA_TYPE", location, "expected an array");
  }
  if (actual.length !== expected.length) {
    fail(
      "SEMANTIC_ACTIONS",
      location,
      `expected exactly ${JSON.stringify(expected)}`,
    );
  }
  for (let index = 0; index < expected.length; index += 1) {
    if (actual[index] !== expected[index]) {
      fail(
        "SEMANTIC_ACTIONS",
        `${location}[${index}]`,
        `expected ${JSON.stringify(expected[index])}`,
      );
    }
  }
}

function validateFixtureDescriptor(
  descriptor,
  descriptorName = "<inline>",
) {
  requireRecord(descriptor, descriptorName);
  rejectAuthorityBearingKeys(descriptor);
  requireExactFields(descriptor, DESCRIPTOR_FIELDS, descriptorName);

  requireExactValue(
    descriptor.schemaVersion,
    "sapphirus.bmad-fixture.v1",
    `${descriptorName}.schemaVersion`,
  );
  requireString(descriptor.fixtureKind, `${descriptorName}.fixtureKind`);
  if (!Object.hasOwn(LOCKED_FIXTURES, descriptor.fixtureKind)) {
    fail(
      "SCHEMA_DISCRIMINATOR",
      `${descriptorName}.fixtureKind`,
      `unknown fixture kind ${JSON.stringify(descriptor.fixtureKind)}`,
    );
  }
  const locked = LOCKED_FIXTURES[descriptor.fixtureKind];
  requireExactValue(descriptorName, locked.descriptorName, "descriptorName");

  for (const field of [
    "fixtureId",
    "distributionProfile",
    "validationProfile",
    "executionProfile",
    "activationState",
  ]) {
    requireString(descriptor[field], `${descriptorName}.${field}`);
    requireExactValue(
      descriptor[field],
      locked[field],
      `${descriptorName}.${field}`,
    );
  }
  requireExactValue(
    descriptor.scriptExecution,
    "blocked",
    `${descriptorName}.scriptExecution`,
  );
  requireExactValue(
    descriptor.networkAccess,
    "blocked",
    `${descriptorName}.networkAccess`,
  );
  validateBuilderActions(
    descriptor.builderActions,
    locked.builderActions,
    `${descriptorName}.builderActions`,
  );
  validateContentBinding(descriptor.source, locked.source, `${descriptorName}.source`);

  if (locked.payload === null) {
    requireExactValue(descriptor.payload, null, `${descriptorName}.payload`);
  } else {
    if (descriptor.payload === null) {
      fail(
        "SEMANTIC_PAYLOAD",
        `${descriptorName}.payload`,
        "inactive Builder fixtures require a source-bound draft payload",
      );
    }
    validateContentBinding(
      descriptor.payload,
      locked.payload,
      `${descriptorName}.payload`,
    );
  }

  validateLockedRecord(
    descriptor.assertions,
    locked.assertions,
    Object.keys(locked.assertions),
    `${descriptorName}.assertions`,
  );
  return descriptor;
}

function parseJsonRejectingDuplicateKeys(text, sourceName) {
  if (typeof text !== "string") {
    fail("JSON_INPUT", sourceName, "expected UTF-8 JSON text");
  }
  const descriptorBytes = Buffer.byteLength(text, "utf8");
  if (descriptorBytes > MAX_DESCRIPTOR_BYTES) {
    fail(
      "JSON_SIZE_LIMIT",
      sourceName,
      `descriptor exceeds the ${MAX_DESCRIPTOR_BYTES}-byte limit`,
    );
  }

  let index = 0;

  function syntax(message) {
    fail("JSON_SYNTAX", `${sourceName}:${index}`, message);
  }

  function skipWhitespace() {
    while (
      index < text.length &&
      (text[index] === " " ||
        text[index] === "\t" ||
        text[index] === "\r" ||
        text[index] === "\n")
    ) {
      index += 1;
    }
  }

  function parseString() {
    if (text[index] !== '"') {
      syntax("expected a JSON string");
    }
    const start = index;
    index += 1;
    while (index < text.length) {
      const character = text[index];
      if (character === '"') {
        index += 1;
        let value;
        try {
          value = JSON.parse(text.slice(start, index));
        } catch (error) {
          fail(
            "JSON_SYNTAX",
            `${sourceName}:${start}`,
            "invalid JSON string escape",
            { cause: error },
          );
        }
        for (let offset = 0; offset < value.length; offset += 1) {
          const codeUnit = value.charCodeAt(offset);
          if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
            const nextCodeUnit = value.charCodeAt(offset + 1);
            if (!(nextCodeUnit >= 0xdc00 && nextCodeUnit <= 0xdfff)) {
              fail(
                "JSON_UNPAIRED_SURROGATE",
                `${sourceName}:${start}`,
                "JSON strings must contain well-formed Unicode",
              );
            }
            offset += 1;
          } else if (codeUnit >= 0xdc00 && codeUnit <= 0xdfff) {
            fail(
              "JSON_UNPAIRED_SURROGATE",
              `${sourceName}:${start}`,
              "JSON strings must contain well-formed Unicode",
            );
          }
        }
        return value;
      }
      if (character === "\\") {
        index += 2;
        continue;
      }
      if (character.charCodeAt(0) < 0x20) {
        syntax("unescaped control character in JSON string");
      }
      index += 1;
    }
    syntax("unterminated JSON string");
  }

  function parseLiteral(literal, value) {
    if (!text.startsWith(literal, index)) {
      syntax(`expected ${literal}`);
    }
    index += literal.length;
    return value;
  }

  function parseNumber() {
    const match = /^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?/u.exec(
      text.slice(index),
    );
    if (match === null) {
      syntax("invalid JSON number");
    }
    index += match[0].length;
    const value = Number(match[0]);
    if (!Number.isFinite(value)) {
      syntax("JSON number exceeds the finite numeric range");
    }
    if (Number.isInteger(value) && !Number.isSafeInteger(value)) {
      fail(
        "JSON_INTEGER_RANGE",
        `${sourceName}:${index - match[0].length}`,
        "integral JSON numbers must be within the safe integer range",
      );
    }
    return value;
  }

  function parseArray(depth) {
    const array = [];
    index += 1;
    skipWhitespace();
    if (text[index] === "]") {
      index += 1;
      return array;
    }
    while (index < text.length) {
      array.push(parseValue(depth + 1));
      skipWhitespace();
      if (text[index] === "]") {
        index += 1;
        return array;
      }
      if (text[index] !== ",") {
        syntax("expected ',' or ']' in array");
      }
      index += 1;
      skipWhitespace();
    }
    syntax("unterminated JSON array");
  }

  function parseObject(depth) {
    const object = Object.create(null);
    const keys = new Set();
    index += 1;
    skipWhitespace();
    if (text[index] === "}") {
      index += 1;
      return object;
    }
    while (index < text.length) {
      const keyOffset = index;
      const key = parseString();
      if (keys.has(key)) {
        fail(
          "JSON_DUPLICATE_KEY",
          `${sourceName}:${keyOffset}`,
          `duplicate object key ${JSON.stringify(key)}`,
        );
      }
      keys.add(key);
      skipWhitespace();
      if (text[index] !== ":") {
        syntax("expected ':' after object key");
      }
      index += 1;
      object[key] = parseValue(depth + 1);
      skipWhitespace();
      if (text[index] === "}") {
        index += 1;
        return object;
      }
      if (text[index] !== ",") {
        syntax("expected ',' or '}' in object");
      }
      index += 1;
      skipWhitespace();
    }
    syntax("unterminated JSON object");
  }

  function parseValue(depth) {
    if (depth > MAX_JSON_DEPTH) {
      fail(
        "JSON_DEPTH_LIMIT",
        `${sourceName}:${index}`,
        `descriptor exceeds the ${MAX_JSON_DEPTH}-level nesting limit`,
      );
    }
    skipWhitespace();
    const character = text[index];
    if (character === "{") {
      return parseObject(depth);
    }
    if (character === "[") {
      return parseArray(depth);
    }
    if (character === '"') {
      return parseString();
    }
    if (character === "t") {
      return parseLiteral("true", true);
    }
    if (character === "f") {
      return parseLiteral("false", false);
    }
    if (character === "n") {
      return parseLiteral("null", null);
    }
    if (character === "-" || (character >= "0" && character <= "9")) {
      return parseNumber();
    }
    syntax("expected a JSON value");
  }

  const value = parseValue(0);
  skipWhitespace();
  if (index !== text.length) {
    syntax("unexpected trailing content");
  }
  return value;
}

export function parseFixtureDescriptor(text, descriptorName) {
  requireString(descriptorName, "descriptorName");
  const descriptor = parseJsonRejectingDuplicateKeys(text, descriptorName);
  return validateFixtureDescriptor(descriptor, descriptorName);
}

export function decodeDescriptorBytes(bytes, sourceName) {
  requireString(sourceName, "descriptorName");
  if (!(bytes instanceof Uint8Array)) {
    fail("JSON_INPUT", sourceName, "expected raw descriptor bytes");
  }
  if (bytes.byteLength > MAX_DESCRIPTOR_BYTES) {
    fail(
      "JSON_SIZE_LIMIT",
      sourceName,
      `descriptor exceeds the ${MAX_DESCRIPTOR_BYTES}-byte limit`,
    );
  }
  try {
    return new TextDecoder("utf-8", {
      fatal: true,
      ignoreBOM: true,
    }).decode(bytes);
  } catch (error) {
    fail("JSON_UTF8", sourceName, "descriptor is not valid UTF-8", {
      cause: error,
    });
  }
}

export function parseFixtureDescriptorBytes(bytes, descriptorName) {
  return parseFixtureDescriptor(
    decodeDescriptorBytes(bytes, descriptorName),
    descriptorName,
  );
}

export function assertContentMatchesBinding(binding, content, location) {
  if (!(content instanceof Uint8Array)) {
    fail("CONTENT_TYPE", location, "expected raw file bytes");
  }
  const actual = {
    byteLength: content.byteLength,
    sha256: createHash("sha256").update(content).digest("hex"),
  };
  if (
    actual.byteLength !== binding.byteLength ||
    actual.sha256 !== binding.sha256
  ) {
    fail(
      "CONTENT_DIGEST_MISMATCH",
      location,
      `expected ${binding.byteLength} bytes/${binding.sha256}, received ${actual.byteLength} bytes/${actual.sha256}`,
    );
  }
  return actual;
}

function isContained(root, candidate) {
  const relative = path.relative(root, candidate);
  return (
    relative !== "" &&
    !relative.startsWith(`..${path.sep}`) &&
    relative !== ".." &&
    !path.isAbsolute(relative)
  );
}

async function verifyRepositoryFile(binding, repositoryRoot, location) {
  const resolvedRoot = path.resolve(repositoryRoot);
  const resolvedFile = path.resolve(resolvedRoot, ...binding.relativePath.split("/"));
  if (!isContained(resolvedRoot, resolvedFile)) {
    fail("CONTENT_PATH_ESCAPE", location, "path resolves outside the repository root");
  }

  let fileStats;
  let realRoot;
  let realFile;
  try {
    [fileStats, realRoot, realFile] = await Promise.all([
      lstat(resolvedFile),
      realpath(resolvedRoot),
      realpath(resolvedFile),
    ]);
  } catch (error) {
    fail("CONTENT_IO", location, "bound file is unavailable", { cause: error });
  }
  if (!fileStats.isFile() || fileStats.isSymbolicLink()) {
    fail("CONTENT_FILE_TYPE", location, "bound path must be a regular file");
  }
  if (!isContained(realRoot, realFile)) {
    fail("CONTENT_PATH_ESCAPE", location, "real path escapes the repository root");
  }

  let content;
  try {
    content = await readFile(resolvedFile);
  } catch (error) {
    fail("CONTENT_IO", location, "bound file could not be read", { cause: error });
  }
  return assertContentMatchesBinding(binding, content, location);
}

export async function verifyFixtureSet({
  packageRoot = defaultPackageRoot,
  repositoryRoot = defaultRepositoryRoot,
} = {}) {
  const fixtureDirectory = path.join(packageRoot, "fixtures");
  const fixtureEntries = await readdir(fixtureDirectory, { withFileTypes: true });
  const fixtureEntryNames = fixtureEntries.map((entry) => entry.name).sort();
  const expectedFixtureEntryNames = [...EXPECTED_DESCRIPTOR_NAMES, "payloads"].sort();
  if (
    JSON.stringify(fixtureEntryNames) !==
    JSON.stringify(expectedFixtureEntryNames)
  ) {
    fail(
      "FIXTURE_TREE_MISMATCH",
      fixtureDirectory,
      `expected exactly ${JSON.stringify(expectedFixtureEntryNames)}, received ${JSON.stringify(fixtureEntryNames)}`,
    );
  }

  const descriptorEntries = fixtureEntries.filter((entry) =>
    entry.name.toLowerCase().endsWith(".json"),
  );
  if (descriptorEntries.some((entry) => !entry.isFile() || entry.isSymbolicLink())) {
    fail(
      "CONTENT_FILE_TYPE",
      fixtureDirectory,
      "fixture descriptors must be regular files",
    );
  }
  const descriptorNames = descriptorEntries
    .map((entry) => entry.name)
    .sort();

  if (JSON.stringify(descriptorNames) !== JSON.stringify(EXPECTED_DESCRIPTOR_NAMES)) {
    fail(
      "FIXTURE_SET_MISMATCH",
      fixtureDirectory,
      `expected exactly ${JSON.stringify(EXPECTED_DESCRIPTOR_NAMES)}, received ${JSON.stringify(descriptorNames)}`,
    );
  }

  const payloadDirectoryEntry = fixtureEntries.find(
    (entry) => entry.name === "payloads",
  );
  if (
    payloadDirectoryEntry === undefined ||
    !payloadDirectoryEntry.isDirectory() ||
    payloadDirectoryEntry.isSymbolicLink()
  ) {
    fail(
      "CONTENT_FILE_TYPE",
      path.join(fixtureDirectory, "payloads"),
      "payloads must be a regular directory",
    );
  }
  const payloadDirectory = path.join(fixtureDirectory, "payloads");
  const payloadKinds = await readdir(payloadDirectory, { withFileTypes: true });
  const expectedPayloadKinds = ["simple-workflow", "stateless-agent"];
  if (
    JSON.stringify(payloadKinds.map((entry) => entry.name).sort()) !==
    JSON.stringify(expectedPayloadKinds)
  ) {
    fail(
      "FIXTURE_TREE_MISMATCH",
      payloadDirectory,
      `expected exactly ${JSON.stringify(expectedPayloadKinds)}`,
    );
  }
  for (const payloadKind of payloadKinds) {
    if (!payloadKind.isDirectory() || payloadKind.isSymbolicLink()) {
      fail(
        "CONTENT_FILE_TYPE",
        path.join(payloadDirectory, payloadKind.name),
        "payload kind must be a regular directory",
      );
    }
    const payloadEntries = await readdir(
      path.join(payloadDirectory, payloadKind.name),
      { withFileTypes: true },
    );
    if (
      payloadEntries.length !== 1 ||
      payloadEntries[0].name !== "SKILL.md" ||
      !payloadEntries[0].isFile() ||
      payloadEntries[0].isSymbolicLink()
    ) {
      fail(
        "FIXTURE_TREE_MISMATCH",
        path.join(payloadDirectory, payloadKind.name),
        "payload kind must contain only a regular SKILL.md",
      );
    }
  }

  const descriptors = [];
  for (const descriptorName of descriptorNames) {
    const bytes = await readFile(path.join(fixtureDirectory, descriptorName));
    const descriptor = parseFixtureDescriptorBytes(bytes, descriptorName);
    await verifyRepositoryFile(
      descriptor.source,
      repositoryRoot,
      `${descriptorName}.source`,
    );
    if (descriptor.payload !== null) {
      await verifyRepositoryFile(
        descriptor.payload,
        repositoryRoot,
        `${descriptorName}.payload`,
      );
    }
    descriptors.push(descriptor);
  }

  const fixtureIds = descriptors.map((descriptor) => descriptor.fixtureId);
  if (new Set(fixtureIds).size !== fixtureIds.length) {
    fail("FIXTURE_ID_COLLISION", fixtureDirectory, "fixture IDs must be unique");
  }

  return Object.freeze({
    descriptorCount: descriptors.length,
    fixtureIds: Object.freeze([...fixtureIds].sort()),
  });
}
