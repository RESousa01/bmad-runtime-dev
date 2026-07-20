import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import {
  lstat,
  mkdir,
  readFile,
  readdir,
  realpath,
  rm,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { inflateRawSync } from "node:zlib";
import Ajv2020 from "ajv/dist/2020.js";
import { parseStrictJson } from "./strict-json.mjs";

export const FAILURE_CODES = Object.freeze({
  fallback: "CONTRACT_GENERATOR_FALLBACK_FORBIDDEN",
  lock: "CONTRACT_LOCK_BOOTSTRAP_UNREVIEWED",
  nondeterministic: "CONTRACT_GENERATOR_NONDETERMINISTIC",
  parity: "CONTRACT_LANGUAGE_PARITY_FAILED",
  version: "CONTRACT_GENERATOR_VERSION_MISMATCH",
});

const packageRoot = fileURLToPath(new URL("../../", import.meta.url));
const nativeCodegenPath = fileURLToPath(import.meta.url);
export const repositoryRoot = path.resolve(packageRoot, "..", "..");
const toolLockPath = path.join(repositoryRoot, "tools", "contract-codegen", "tool-lock.json");
const manifestPath = path.join(repositoryRoot, ".config", "dotnet-tools.json");
const targetRoot = path.join(repositoryRoot, "target", "c");
const maximumOutputBytes = 1_048_576;
const decoder = new TextDecoder("utf-8", { fatal: true });
const trustedPowerShellPath = "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe";
const trustedPowerShellSecurityModulePath = "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\Modules\\Microsoft.PowerShell.Security\\Microsoft.PowerShell.Security.psd1";
const trustedPowerShellUtilityModulePath = "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\Modules\\Microsoft.PowerShell.Utility\\Microsoft.PowerShell.Utility.psd1";
const forbiddenNativeEnvironmentPrefixes = Object.freeze([
  "CORECLR_",
  "COREHOST_",
  "COMPLUS_",
  "COR_",
  "MSBUILD",
]);
const forbiddenNativeEnvironmentNames = new Set(["DEVPATH"]);
const allowedDotnetEnvironmentNames = new Set([
  "DOTNET_CLI_HOME",
  "DOTNET_CLI_TELEMETRY_OPTOUT",
  "DOTNET_NOLOGO",
  "DOTNET_SKIP_FIRST_TIME_EXPERIENCE",
]);
const canonicalDotnetRoot = "c:\\program files\\dotnet";
const lockedDotnetChildEnvironment = Object.freeze({
  DOTNET_EnableDiagnostics: "0",
  DOTNET_EnableDiagnostics_Debugger: "0",
  DOTNET_EnableDiagnostics_IPC: "0",
  DOTNET_EnableDiagnostics_Profiler: "0",
});
const lockedPowerShellChildEnvironment = Object.freeze({
  PSModulePath: "C:\\__sapphirus_no_module_search__",
});
const lockedRustChildEnvironment = Object.freeze({});

const productionRoots = Object.freeze([
  ["approved-execution-spec.schema.json", "ApprovedExecutionSpec"],
  ["authority-ref.schema.json", "AuthorityRef"],
  ["bmad-builder-authoring.schema.json", "BuilderAuthoringObject"],
  ["bmad-capability-catalog.schema.json", "BmadCapabilityCatalog"],
  ["bmad-capability-result.schema.json", "BmadCapabilityResult"],
  ["bmad-capability-run.schema.json", "BmadCapabilityRun"],
  ["bmad-method-advance-result.schema.json", "MethodAdvanceResult"],
  ["bmad-method-help-proposal.schema.json", "MethodHelpProposal"],
  ["bmad-method-help-recommendation.schema.json", "MethodHelpRecommendation"],
  ["bmad-method-session.schema.json", "MethodSession"],
  ["bmad-package-descriptor.schema.json", "BmadPackageDescriptor"],
  ["bmad-validation-report.schema.json", "BmadValidationReport"],
  ["candidate-action.schema.json", "CandidateAction"],
  ["contract-error.schema.json", "ContractError"],
  ["desktop-device-registration.schema.json", "DesktopDeviceRegistration"],
  ["desktop-entitlement-lease.schema.json", "DesktopEntitlementLease"],
  ["desktop-policy.schema.json", "DesktopPolicy"],
  ["durable-object.schema.json", "DurableObject"],
  ["evidence-event.schema.json", "EvidenceEvent"],
  ["execution-result-manifest.schema.json", "ExecutionResultManifest"],
  ["filesystem-capability.schema.json", "FilesystemCapabilitySnapshot"],
  ["model-access-receipt.schema.json", "ModelAccessReceipt"],
  ["model-access-request.schema.json", "ModelAccessRequest"],
  ["model-access-result.schema.json", "ModelAccessResult"],
  ["model-context-consent.schema.json", "ModelContextConsent"],
  ["package-compatibility.schema.json", "PackageCompatibility"],
  ["remote-job-handoff.schema.json", "RemoteJobHandoff"],
  ["spec-consumption.schema.json", "SpecConsumptionRecord"],
]);
const expectedFailureCodes = Object.freeze(Object.values(FAILURE_CODES).sort());
const expectedPackageIntegrities = Object.freeze({
  "json-schema-to-typescript@15.0.4":
    "sha512-Su9oK8DR4xCmDsLlyvadkXzX6+GGXJpbhwoLtOGArAG61dvbW4YQmSEno2y66ahpIdmLMg6YUf/QHLgiwvkrHQ==",
  "ajv@8.20.0":
    "sha512-Thbli+OlOj+iMPYFBVBfJ3OmCAnaSyNn4M1vz9T6Gka5Jt9ba/HIR56joy65tY6kx/FCF5VXNB819Y7/GUrBGA==",
  "typescript@7.0.2":
    "sha512-8FYau96o3NKOhbjKi/qNvG/W5jhzxkbdm5sj9AbZ/5T5sWqn3hJgLfGx27sRKZWTvyzCP8dLRBTf5tBTSRVUNA==",
});
const expectedRustArguments = Object.freeze(["typify", "{input}", "--output", "{output}"]);
const expectedBootstrapLocks = Object.freeze({
  pnpm: {
    file: "pnpm-lock.yaml",
    sha256: "28fa1837a685e17c9b696e761fd0f86173a09ed22c666c88378910792ae58a93",
    status: "reviewed",
  },
  cargo: {
    file: "Cargo.lock",
    sha256: "49ad67660adbbebcf3653ada3a472cadaed3c777bdeefe89a637f5f43b0f5c85",
    status: "reviewed",
  },
  dotnetTools: {
    file: ".config/dotnet-tools.json",
    sha256: "a951b939e6d946e93d33c112969fd2500f2cb77190367fb466a247d8a209582c",
    status: "reviewed",
  },
  dotnetPackages: {
    file: "tests/generator-qualification/dotnet/packages.lock.json",
    sha256: "4166588aeb1745851eb694eea48b493693909860fd66394b1a457639eea22b4a",
    status: "reviewed",
  },
});
const expectedDotnetArguments = Object.freeze([
  "jsonschema", "{input}", "--engine", "V5", "--useSchema", "Draft202012",
  "--useUnixLineEndings", "true", "--optionalAsNullable", "None", "--assertFormat",
  "false", "--disableOptionalNamingHeuristics", "true", "--rootNamespace", "{namespace}",
  "--outputRootTypeName", "{rootType}", "--outputRootAccessibility", "Public",
  "--defaultAccessibility", "Public", "--outputPath", "{output}",
]);

function fail(code, message) {
  throw new Error(`${code}: ${message}`);
}

export function assertNoInheritedNativeToolInjection(environment = process.env) {
  for (const [name, value] of Object.entries(environment)) {
    const normalizedName = name.toUpperCase();
    let forbidden = forbiddenNativeEnvironmentNames.has(normalizedName)
      || forbiddenNativeEnvironmentPrefixes.some((prefix) => normalizedName.startsWith(prefix));
    if (normalizedName.startsWith("DOTNET_ROOT")) {
      const normalizedRoot = path.win32.normalize(String(value)).replace(/[\\/]+$/u, "").toLowerCase();
      forbidden = normalizedRoot !== canonicalDotnetRoot;
    } else if (normalizedName.startsWith("DOTNET_")) {
      forbidden = !allowedDotnetEnvironmentNames.has(normalizedName);
    }
    if (forbidden) {
      fail(
        FAILURE_CODES.lock,
        `${normalizedName} is a forbidden inherited native-tool injection variable.`,
      );
    }
  }
}

export function createLockedNativeChildEnvironment(kind) {
  switch (kind) {
    case "dotnet": return lockedDotnetChildEnvironment;
    case "powershell": return lockedPowerShellChildEnvironment;
    case "rust": return lockedRustChildEnvironment;
    default: fail(FAILURE_CODES.lock, `Unknown native child environment: ${String(kind)}`);
  }
}

function equalJson(actual, expected, code, label) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    fail(code, `${label} does not match the reviewed contract-codegen configuration.`);
  }
}

function assertExactKeys(value, expected, code, label) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    fail(code, `${label} must be an object.`);
  }
  equalJson(Object.keys(value).sort(), [...expected].sort(), code, `${label} keys`);
}

function normalizeRelative(value) {
  return value.replaceAll("\\", "/");
}

function assertContained(root, candidate, code = FAILURE_CODES.parity) {
  const relative = path.relative(root, candidate);
  if (relative === "" || relative.startsWith(`..${path.sep}`) || relative === ".." || path.isAbsolute(relative)) {
    fail(code, `Path escapes its controlled root: ${normalizeRelative(candidate)}`);
  }
  return candidate;
}

function repositoryPath(relativePath) {
  if (typeof relativePath !== "string" || path.isAbsolute(relativePath)) {
    fail(FAILURE_CODES.lock, `Repository path is not relative: ${String(relativePath)}`);
  }
  const resolved = path.resolve(repositoryRoot, relativePath);
  assertContained(repositoryRoot, resolved, FAILURE_CODES.lock);
  return resolved;
}

async function physicalRepositoryPath(candidate, label, code = FAILURE_CODES.lock) {
  const lexical = path.resolve(candidate);
  assertContained(repositoryRoot, lexical, code);
  const relative = path.relative(repositoryRoot, lexical);
  let cursor = repositoryRoot;
  for (const segment of relative.split(path.sep)) {
    cursor = path.join(cursor, segment);
    let stats;
    try {
      stats = await lstat(cursor);
    } catch (error) {
      if (error?.code === "ENOENT") fail(code, `${label} is missing.`);
      throw error;
    }
    if (stats.isSymbolicLink()) {
      fail(code, `${label} traverses a symbolic-link or junction path: ${normalizeRelative(cursor)}`);
    }
  }
  const [physicalRoot, physicalCandidate] = await Promise.all([
    realpath(repositoryRoot),
    realpath(lexical),
  ]);
  assertContained(physicalRoot, physicalCandidate, code);
  return physicalCandidate;
}

async function safeRepositoryDestination(candidate, label, code = FAILURE_CODES.parity) {
  const lexical = path.resolve(candidate);
  assertContained(repositoryRoot, lexical, code);
  const relative = path.relative(repositoryRoot, lexical);
  let cursor = repositoryRoot;
  let lastExisting = repositoryRoot;
  for (const segment of relative.split(path.sep)) {
    cursor = path.join(cursor, segment);
    try {
      const stats = await lstat(cursor);
      if (stats.isSymbolicLink()) {
        fail(code, `${label} traverses a symbolic-link or junction path: ${normalizeRelative(cursor)}`);
      }
      lastExisting = cursor;
    } catch (error) {
      if (error?.code === "ENOENT") break;
      throw error;
    }
  }
  const [physicalRoot, physicalAncestor] = await Promise.all([
    realpath(repositoryRoot),
    realpath(lastExisting),
  ]);
  if (physicalAncestor !== physicalRoot) {
    assertContained(physicalRoot, physicalAncestor, code);
  }
  return lexical;
}

function ordinalCompare(left, right) {
  return Buffer.compare(Buffer.from(left, "utf8"), Buffer.from(right, "utf8"));
}

export function stableJson(value) {
  return `${JSON.stringify(value, null, 2)}\n`;
}

export function sha256(value) {
  return `sha256:${createHash("sha256").update(value).digest("hex")}`;
}

export function canonicalDotnetOutputRoot(runRoot, mode, stagingPolicy) {
  if (!path.isAbsolute(runRoot) || !["production", "qualification"].includes(mode)) {
    fail(FAILURE_CODES.nondeterministic, "Corvus staging requires an absolute reviewed run root.");
  }
  const targetLength = stagingPolicy?.dotnetOutputPathLengths?.[mode];
  if (!Number.isSafeInteger(targetLength) || targetLength <= 0) {
    fail(FAILURE_CODES.nondeterministic, "Corvus staging output length is not reviewed.");
  }
  const directoryLength = targetLength - runRoot.length - path.sep.length;
  if (directoryLength < "dotnet".length) {
    fail(
      FAILURE_CODES.nondeterministic,
      `The checkout path is too long for the reviewed ${mode} Corvus staging root.`,
    );
  }
  const outputRoot = path.join(runRoot, "dotnet".padEnd(directoryLength, "-"));
  if (outputRoot.length !== targetLength) {
    fail(FAILURE_CODES.nondeterministic, "Corvus staging output length could not be fixed.");
  }
  return outputRoot;
}

function requireBufferRange(buffer, offset, length, label) {
  if (!Number.isSafeInteger(offset) || !Number.isSafeInteger(length)
    || offset < 0 || length < 0 || offset + length > buffer.length) {
    fail(FAILURE_CODES.version, `${label} is outside the reviewed binary structure.`);
  }
}

function mapPortableExecutableRva(sections, rva, size, label) {
  for (const section of sections) {
    const relative = rva - section.virtualAddress;
    if (relative >= 0 && relative + size <= section.rawSize) {
      return section.rawOffset + relative;
    }
  }
  fail(FAILURE_CODES.version, `${label} is not backed by a regular PE section.`);
}

export function normalizedPortableExecutableSha256(source) {
  const normalized = Buffer.from(source);
  requireBufferRange(normalized, 0, 0x40, "cargo-typify DOS header");
  if (normalized.subarray(0, 2).toString("ascii") !== "MZ") {
    fail(FAILURE_CODES.version, "cargo-typify is not a reviewed PE executable.");
  }
  const peOffset = normalized.readUInt32LE(0x3c);
  requireBufferRange(normalized, peOffset, 24, "cargo-typify PE header");
  if (!normalized.subarray(peOffset, peOffset + 4).equals(Buffer.from("PE\0\0", "binary"))) {
    fail(FAILURE_CODES.version, "cargo-typify has an invalid PE signature.");
  }
  const coffOffset = peOffset + 4;
  const machine = normalized.readUInt16LE(coffOffset);
  const sectionCount = normalized.readUInt16LE(coffOffset + 2);
  const optionalHeaderSize = normalized.readUInt16LE(coffOffset + 16);
  if (machine !== 0x8664 || sectionCount === 0 || sectionCount > 96) {
    fail(FAILURE_CODES.version, "cargo-typify PE machine or section inventory changed.");
  }
  normalized.fill(0, coffOffset + 4, coffOffset + 8);

  const optionalOffset = coffOffset + 20;
  requireBufferRange(normalized, optionalOffset, optionalHeaderSize, "cargo-typify optional header");
  if (optionalHeaderSize < 112 || normalized.readUInt16LE(optionalOffset) !== 0x20b) {
    fail(FAILURE_CODES.version, "cargo-typify is not a PE32+ executable.");
  }
  const dataDirectoryCount = normalized.readUInt32LE(optionalOffset + 108);
  if (dataDirectoryCount < 7 || optionalHeaderSize < 112 + 7 * 8) {
    fail(FAILURE_CODES.version, "cargo-typify PE debug directory is unavailable.");
  }
  const debugDirectoryRva = normalized.readUInt32LE(optionalOffset + 112 + 6 * 8);
  const debugDirectorySize = normalized.readUInt32LE(optionalOffset + 112 + 6 * 8 + 4);
  if (debugDirectoryRva === 0 || debugDirectorySize === 0 || debugDirectorySize % 28 !== 0) {
    fail(FAILURE_CODES.version, "cargo-typify PE debug directory changed shape.");
  }

  const sectionTableOffset = optionalOffset + optionalHeaderSize;
  requireBufferRange(normalized, sectionTableOffset, sectionCount * 40, "cargo-typify section table");
  const sections = [];
  for (let index = 0; index < sectionCount; index += 1) {
    const sectionOffset = sectionTableOffset + index * 40;
    const virtualAddress = normalized.readUInt32LE(sectionOffset + 12);
    const rawSize = normalized.readUInt32LE(sectionOffset + 16);
    const rawOffset = normalized.readUInt32LE(sectionOffset + 20);
    requireBufferRange(normalized, rawOffset, rawSize, `cargo-typify section ${index}`);
    sections.push({ rawOffset, rawSize, virtualAddress });
  }
  const debugOffset = mapPortableExecutableRva(
    sections,
    debugDirectoryRva,
    debugDirectorySize,
    "cargo-typify PE debug directory",
  );
  requireBufferRange(normalized, debugOffset, debugDirectorySize, "cargo-typify PE debug directory");
  let codeViewCount = 0;
  for (let offset = debugOffset; offset < debugOffset + debugDirectorySize; offset += 28) {
    normalized.fill(0, offset + 4, offset + 8);
    const debugType = normalized.readUInt32LE(offset + 12);
    if (debugType !== 2) continue;
    const dataSize = normalized.readUInt32LE(offset + 16);
    const dataOffset = normalized.readUInt32LE(offset + 24);
    requireBufferRange(normalized, dataOffset, dataSize, "cargo-typify CodeView record");
    if (dataSize < 24
      || normalized.subarray(dataOffset, dataOffset + 4).toString("ascii") !== "RSDS") {
      fail(FAILURE_CODES.version, "cargo-typify CodeView record changed shape.");
    }
    normalized.fill(0, dataOffset + 4, dataOffset + 20);
    codeViewCount += 1;
  }
  if (codeViewCount !== 1) {
    fail(FAILURE_CODES.version, "cargo-typify must contain one reviewed CodeView identity.");
  }
  return createHash("sha256").update(normalized).digest("hex");
}

function findZipEndOfCentralDirectory(archive) {
  const minimumOffset = Math.max(0, archive.length - 65_557);
  for (let offset = archive.length - 22; offset >= minimumOffset; offset -= 1) {
    if (archive.readUInt32LE(offset) === 0x06054b50) return offset;
  }
  fail(FAILURE_CODES.version, "The reviewed Corvus nupkg has no ZIP central directory.");
}

function decodeZipPath(bytes) {
  let value;
  try {
    value = decoder.decode(bytes);
  } catch (error) {
    fail(FAILURE_CODES.version, `The reviewed Corvus nupkg has a non-UTF-8 path: ${error.message}`);
  }
  if (value.includes("\\") || value.includes("\0") || value.startsWith("/")
    || value.split("/").some((segment) => segment === "." || segment === "..")) {
    fail(FAILURE_CODES.version, "The reviewed Corvus nupkg contains an unsafe path.");
  }
  return value;
}

function readZipClosure(archive, prefix) {
  if (!Buffer.isBuffer(archive) || !prefix.endsWith("/") || prefix.includes("\\")) {
    fail(FAILURE_CODES.version, "The Corvus nupkg closure prefix is invalid.");
  }
  const eocdOffset = findZipEndOfCentralDirectory(archive);
  requireBufferRange(archive, eocdOffset, 22, "Corvus nupkg end record");
  const commentLength = archive.readUInt16LE(eocdOffset + 20);
  if (eocdOffset + 22 + commentLength !== archive.length
    || archive.readUInt16LE(eocdOffset + 4) !== 0
    || archive.readUInt16LE(eocdOffset + 6) !== 0) {
    fail(FAILURE_CODES.version, "The reviewed Corvus nupkg uses an unsupported ZIP layout.");
  }
  const diskEntries = archive.readUInt16LE(eocdOffset + 8);
  const entryCount = archive.readUInt16LE(eocdOffset + 10);
  const centralSize = archive.readUInt32LE(eocdOffset + 12);
  const centralOffset = archive.readUInt32LE(eocdOffset + 16);
  if (entryCount === 0xffff || centralSize === 0xffffffff || centralOffset === 0xffffffff
    || diskEntries !== entryCount || centralOffset + centralSize !== eocdOffset) {
    fail(FAILURE_CODES.version, "The reviewed Corvus nupkg ZIP inventory changed shape.");
  }
  requireBufferRange(archive, centralOffset, centralSize, "Corvus nupkg central directory");
  const entries = new Map();
  let cursor = centralOffset;
  for (let index = 0; index < entryCount; index += 1) {
    requireBufferRange(archive, cursor, 46, "Corvus nupkg central entry");
    if (archive.readUInt32LE(cursor) !== 0x02014b50) {
      fail(FAILURE_CODES.version, "The reviewed Corvus nupkg central entry is malformed.");
    }
    const flags = archive.readUInt16LE(cursor + 8);
    const method = archive.readUInt16LE(cursor + 10);
    const compressedSize = archive.readUInt32LE(cursor + 20);
    const uncompressedSize = archive.readUInt32LE(cursor + 24);
    const nameLength = archive.readUInt16LE(cursor + 28);
    const extraLength = archive.readUInt16LE(cursor + 30);
    const entryCommentLength = archive.readUInt16LE(cursor + 32);
    const diskStart = archive.readUInt16LE(cursor + 34);
    const localOffset = archive.readUInt32LE(cursor + 42);
    const recordLength = 46 + nameLength + extraLength + entryCommentLength;
    requireBufferRange(archive, cursor, recordLength, "Corvus nupkg central entry");
    if ((flags & 1) !== 0 || ![0, 8].includes(method) || diskStart !== 0
      || [compressedSize, uncompressedSize, localOffset].includes(0xffffffff)) {
      fail(FAILURE_CODES.version, "The reviewed Corvus nupkg entry encoding is unsupported.");
    }
    const archivePath = decodeZipPath(archive.subarray(cursor + 46, cursor + 46 + nameLength));
    cursor += recordLength;
    if (!archivePath.startsWith(prefix) || archivePath.endsWith("/")) continue;
    const relativePath = archivePath.slice(prefix.length);
    if (relativePath === "" || relativePath.split("/").some((segment) => segment === "")) {
      fail(FAILURE_CODES.version, "The reviewed Corvus closure contains an invalid file path.");
    }
    if (entries.has(relativePath)) {
      fail(FAILURE_CODES.version, `The reviewed Corvus closure duplicates ${relativePath}.`);
    }
    requireBufferRange(archive, localOffset, 30, `Corvus nupkg local entry ${relativePath}`);
    if (archive.readUInt32LE(localOffset) !== 0x04034b50
      || archive.readUInt16LE(localOffset + 6) !== flags
      || archive.readUInt16LE(localOffset + 8) !== method) {
      fail(FAILURE_CODES.version, `The reviewed Corvus local entry changed for ${relativePath}.`);
    }
    const localNameLength = archive.readUInt16LE(localOffset + 26);
    const localExtraLength = archive.readUInt16LE(localOffset + 28);
    const dataOffset = localOffset + 30 + localNameLength + localExtraLength;
    requireBufferRange(archive, localOffset + 30, localNameLength, `Corvus local path ${relativePath}`);
    if (!archive.subarray(localOffset + 30, localOffset + 30 + localNameLength)
      .equals(Buffer.from(archivePath, "utf8"))) {
      fail(FAILURE_CODES.version, `The reviewed Corvus local path changed for ${relativePath}.`);
    }
    requireBufferRange(archive, dataOffset, compressedSize, `Corvus nupkg data ${relativePath}`);
    entries.set(relativePath, {
      compressedSize,
      dataOffset,
      method,
      uncompressedSize,
    });
  }
  if (cursor !== centralOffset + centralSize) {
    fail(FAILURE_CODES.version, "The reviewed Corvus nupkg central directory has trailing data.");
  }
  return entries;
}

function extractZipEntry(archive, entry, relativePath) {
  const compressed = archive.subarray(entry.dataOffset, entry.dataOffset + entry.compressedSize);
  let content;
  try {
    content = entry.method === 0
      ? Buffer.from(compressed)
      : inflateRawSync(compressed, { maxOutputLength: entry.uncompressedSize });
  } catch (error) {
    fail(FAILURE_CODES.version, `The reviewed Corvus entry ${relativePath} cannot be expanded: ${error.message}`);
  }
  if (content.length !== entry.uncompressedSize) {
    fail(FAILURE_CODES.version, `The reviewed Corvus entry ${relativePath} changed size.`);
  }
  return content;
}

function closureDigest(records) {
  const hash = createHash("sha256");
  hash.update("sapphirus.nuget-tool-closure.v1\n", "utf8");
  for (const { path: relativePath, size, sha256: fileSha256 } of records) {
    hash.update(`${Buffer.byteLength(relativePath, "utf8")}:${relativePath}\0${size}:${fileSha256}\n`, "utf8");
  }
  return hash.digest("hex");
}

export function nugetArchiveClosureIdentity(source, prefix) {
  const archive = Buffer.from(source);
  const entries = readZipClosure(archive, prefix);
  const records = [...entries.entries()]
    .sort(([left], [right]) => ordinalCompare(left, right))
    .map(([relativePath, entry]) => {
      const content = extractZipEntry(archive, entry, relativePath);
      return {
        path: relativePath,
        sha256: createHash("sha256").update(content).digest("hex"),
        size: content.length,
      };
    });
  return { fileCount: records.length, treeSha256: closureDigest(records) };
}

async function hashFile(filePath) {
  const hash = createHash("sha256");
  await new Promise((resolve, reject) => {
    const stream = createReadStream(filePath);
    stream.on("data", (chunk) => hash.update(chunk));
    stream.on("error", reject);
    stream.on("end", resolve);
  });
  return hash.digest("hex");
}

async function readJson(filePath, code, label) {
  let source;
  try {
    source = await readFile(filePath, "utf8");
  } catch (error) {
    if (error?.code === "ENOENT") fail(code, `${label} is missing.`);
    throw error;
  }
  try {
    return parseStrictJson(source);
  } catch (error) {
    fail(code, `${label} is not strict JSON: ${error.message}`);
  }
}

export async function loadAndValidateToolLock() {
  const physicalToolLock = await physicalRepositoryPath(
    toolLockPath,
    "tool-lock.json",
    FAILURE_CODES.lock,
  );
  const lock = await readJson(physicalToolLock, FAILURE_CODES.lock, "tool-lock.json");
  return validateToolLock(lock);
}

export function validateToolLock(lock) {
  assertExactKeys(lock, [
    "schemaVersion", "lockStatus", "bootstrapLocks", "sourceSet", "tools", "invocations",
    "stagingPolicy", "normalizationPolicy", "failureCodes",
  ], FAILURE_CODES.lock, "tool-lock.json");
  if (lock.schemaVersion !== "sapphirus.contract-codegen-tool-lock.v1"
    || lock.lockStatus !== "reviewed_bootstrap") {
    fail(FAILURE_CODES.lock, "The contract-codegen bootstrap lock is not reviewed.");
  }

  const rust = lock.tools?.rust;
  const dotnet = lock.tools?.dotnet;
  const typescript = lock.tools?.typescript;
  assertExactKeys(lock.bootstrapLocks, Object.keys(expectedBootstrapLocks),
    FAILURE_CODES.lock, "bootstrapLocks");
  for (const [name, record] of Object.entries(lock.bootstrapLocks)) {
    assertExactKeys(record, ["file", "sha256", "status"], FAILURE_CODES.lock,
      `${name} bootstrap lock`);
  }
  equalJson(lock.bootstrapLocks, expectedBootstrapLocks, FAILURE_CODES.lock, "bootstrap locks");
  assertExactKeys(lock.sourceSet, ["production", "qualification"], FAILURE_CODES.lock, "sourceSet");
  assertExactKeys(lock.sourceSet.production,
    ["directory", "roots", "dependencies", "bundleId", "rootType"],
    FAILURE_CODES.lock, "production sourceSet");
  for (const root of lock.sourceSet.production.roots ?? []) {
    assertExactKeys(root, ["file", "id", "typeName"], FAILURE_CODES.lock, "production root");
  }
  for (const dependency of lock.sourceSet.production.dependencies ?? []) {
    assertExactKeys(dependency, ["file", "id", "prefix"], FAILURE_CODES.lock,
      "production dependency");
  }
  assertExactKeys(lock.sourceSet.qualification,
    ["directory", "root", "resources", "bundleId", "rootType"],
    FAILURE_CODES.lock, "qualification sourceSet");
  assertExactKeys(lock.sourceSet.qualification.root, ["file", "id", "typeName"],
    FAILURE_CODES.lock, "qualification root");
  for (const resource of lock.sourceSet.qualification.resources ?? []) {
    assertExactKeys(resource, ["file", "id", "prefix"], FAILURE_CODES.lock,
      "qualification resource");
  }
  assertExactKeys(lock.tools, ["typescript", "rust", "dotnet"], FAILURE_CODES.lock, "tools");
  assertExactKeys(typescript,
    ["identity", "validator", "compiler", "packageSource", "packageIntegrities", "license"],
    FAILURE_CODES.lock, "TypeScript tool lock");
  assertExactKeys(typescript.packageIntegrities, Object.keys(expectedPackageIntegrities),
    FAILURE_CODES.lock, "TypeScript package integrities");
  assertExactKeys(rust, [
    "identity", "packageId", "version", "command", "resolvedExecutable", "packageSource",
    "packageSha256", "executableIdentity", "installMetadata", "versionArguments", "versionExitCode",
    "versionOutput", "generationArguments", "installArguments", "compiler", "runtimeValidator",
    "license",
  ], FAILURE_CODES.lock, "Rust tool lock");
  assertExactKeys(rust.executableIdentity, ["algorithm", "fileSize", "normalizedSha256"],
    FAILURE_CODES.lock, "Rust executable identity");
  assertExactKeys(rust.installMetadata, [
    "path", "sha256", "installKey", "versionRequirement", "bins", "features", "allFeatures",
    "noDefaultFeatures", "profile", "target", "rustc",
  ], FAILURE_CODES.lock, "Rust install metadata");
  assertExactKeys(dotnet, [
    "identity", "packageId", "version", "command", "toolManifest", "packageSource",
    "packageSha256", "packageSha512", "versionArguments", "versionExitCode", "versionOutput",
    "generationArguments", "restoreArguments", "sdk", "sdkExecutable", "sdkAuthenticode",
    "toolClosure", "runtime", "license",
  ], FAILURE_CODES.lock, ".NET tool lock");
  assertExactKeys(dotnet.sdkAuthenticode, [
    "status", "signatureType", "signerSubject", "signerThumbprint", "originalFilename",
  ], FAILURE_CODES.lock, ".NET SDK Authenticode identity");
  assertExactKeys(dotnet.toolClosure, [
    "archivePrefix", "entryPoint", "fileCount", "treeSha256",
  ], FAILURE_CODES.lock, "Corvus expanded closure identity");
  assertExactKeys(lock.invocations, ["rust", "dotnet", "production", "qualification"],
    FAILURE_CODES.lock, "invocations");
  assertExactKeys(lock.invocations.rust, ["shell", "maximumCapturedBytes"],
    FAILURE_CODES.lock, "Rust invocation");
  assertExactKeys(lock.invocations.dotnet, ["shell", "maximumCapturedBytes"],
    FAILURE_CODES.lock, ".NET invocation");
  assertExactKeys(lock.invocations.production, ["rustOutput", "dotnetOutput", "dotnetNamespace"],
    FAILURE_CODES.lock, "production invocation");
  assertExactKeys(lock.invocations.qualification,
    ["typescriptOutput", "rustOutput", "dotnetOutput", "dotnetNamespace"],
    FAILURE_CODES.lock, "qualification invocation");
  assertExactKeys(lock.stagingPolicy, [
    "strategy", "runRoots", "modeDirectories", "dotnetOutputPathLengths",
    "outputPathLengthStrategy", "transformVersion", "documentRootKeywordsRemoved",
    "nestedScopeKeywordsRejected", "unsupportedReferenceKeywordsRejected",
    "definitionExtraction", "optionalNullablePresenceTransform", "productionOptionalNullablePolicy",
    "declaredReferencesOnly", "rejectDefinitionCollisions", "rejectSourceSetDrift",
  ], FAILURE_CODES.lock, "stagingPolicy");
  assertExactKeys(lock.stagingPolicy.modeDirectories, ["production", "qualification"],
    FAILURE_CODES.lock, "stagingPolicy modeDirectories");
  assertExactKeys(lock.stagingPolicy.dotnetOutputPathLengths, ["production", "qualification"],
    FAILURE_CODES.lock, "stagingPolicy dotnetOutputPathLengths");
  assertExactKeys(lock.normalizationPolicy, [
    "encoding", "decode", "lineEndings", "terminalLineFeedCount", "pathOrder", "rejectNul",
    "rejectEmbeddedBom", "rejectRunPathLeaks",
  ], FAILURE_CODES.lock, "normalizationPolicy");
  if (typescript?.identity !== "json-schema-to-typescript@15.0.4"
    || typescript?.validator !== "ajv@8.20.0"
    || typescript?.compiler !== "typescript@7.0.2"
    || typescript?.packageSource !== "pnpm-lock.yaml"
    || typescript?.license !== "MIT") {
    fail(FAILURE_CODES.version, "The TypeScript generator/runtime identity changed.");
  }
  equalJson(typescript.packageIntegrities, expectedPackageIntegrities, FAILURE_CODES.version,
    "TypeScript package integrities");
  equalJson({
    identity: rust?.identity,
    packageId: rust?.packageId,
    version: rust?.version,
    command: rust?.command,
    resolvedExecutable: rust?.resolvedExecutable,
    packageSource: rust?.packageSource,
    packageSha256: rust?.packageSha256,
    executableIdentity: rust?.executableIdentity,
    installMetadata: rust?.installMetadata,
    versionArguments: rust?.versionArguments,
    versionExitCode: rust?.versionExitCode,
    versionOutput: rust?.versionOutput,
    generationArguments: rust?.generationArguments,
    installArguments: rust?.installArguments,
    compiler: rust?.compiler,
    runtimeValidator: rust?.runtimeValidator,
    license: rust?.license,
  }, {
    identity: "cargo-typify@0.6.1",
    packageId: "cargo-typify",
    version: "0.6.1",
    command: "cargo-typify.exe",
    resolvedExecutable: "target/contract-tools/bin/cargo-typify.exe",
    packageSource: "crates.io",
    packageSha256: "dacf8eaa5f73f53e96392b36723d37e110c51f0e596c5e158a16c37190c5f7ee",
    executableIdentity: {
      algorithm: "pe-coff-debug-normalized-sha256-v1",
      fileSize: 8167936,
      normalizedSha256: "29eee6240f4657e66504be3e1195366a8fc201085c41a53af9e3d9ea556ee56d",
    },
    installMetadata: {
      path: "target/contract-tools/.crates2.json",
      sha256: "bff12916265ca0beb4d568168b8b9e1e931211063d1a024b9294d791d77f84f9",
      installKey: "cargo-typify 0.6.1 (registry+https://github.com/rust-lang/crates.io-index)",
      versionRequirement: "=0.6.1",
      bins: ["cargo-typify.exe"],
      features: [],
      allFeatures: false,
      noDefaultFeatures: false,
      profile: "release",
      target: "x86_64-pc-windows-msvc",
      rustc: "rustc 1.97.0 (2d8144b78 2026-07-07)\n"
        + "binary: rustc\n"
        + "commit-hash: 2d8144b7880597b6e6d3dfd63a9a9efae3f533d3\n"
        + "commit-date: 2026-07-07\n"
        + "host: x86_64-pc-windows-msvc\n"
        + "release: 1.97.0\n"
        + "LLVM version: 22.1.6\n",
    },
    versionArguments: ["typify", "--version"],
    versionExitCode: 0,
    versionOutput: "cargo-typify 0.6.1",
    generationArguments: [...expectedRustArguments],
    installArguments: [
      "install", "--locked", "--version", "0.6.1", "--root", "target/contract-tools",
      "cargo-typify",
    ],
    compiler: "rustc@1.97.0",
    runtimeValidator: "jsonschema@0.44.1",
    license: "Apache-2.0",
  }, FAILURE_CODES.version, "Rust generator lock");
  equalJson({
    identity: dotnet?.identity,
    packageId: dotnet?.packageId,
    version: dotnet?.version,
    command: dotnet?.command,
    toolManifest: dotnet?.toolManifest,
    packageSource: dotnet?.packageSource,
    packageSha256: dotnet?.packageSha256,
    packageSha512: dotnet?.packageSha512,
    versionArguments: dotnet?.versionArguments,
    versionExitCode: dotnet?.versionExitCode,
    versionOutput: dotnet?.versionOutput,
    generationArguments: dotnet?.generationArguments,
    sdk: dotnet?.sdk,
    sdkExecutable: dotnet?.sdkExecutable,
    sdkAuthenticode: dotnet?.sdkAuthenticode,
    toolClosure: dotnet?.toolClosure,
    restoreArguments: dotnet?.restoreArguments,
    runtime: dotnet?.runtime,
    license: dotnet?.license,
  }, {
    identity: "Corvus.Json.Cli@5.2.7",
    packageId: "Corvus.Json.Cli",
    version: "5.2.7",
    command: "corvusjson",
    toolManifest: ".config/dotnet-tools.json",
    packageSource: "nuget.org",
    packageSha256: "dfda21e11bc03c3d28ecb818ff6634247e149119b1cf9408d49010c7f2416c57",
    packageSha512: "Gok55yzooHkoEpz8pSWqjUVI1RKf7v93FgZx3dv4pnsN23fLCGGPRNJ1pl33B0ZhzbD361/uEsMOhXaCrnG21g==",
    versionArguments: ["version"],
    versionExitCode: 1,
    versionOutput: "Version: 5.2.7 Build: 43d84f4dbbb7f9bc4be6dd871b547277470512b1",
    generationArguments: [...expectedDotnetArguments],
    sdk: "10.0.302",
    sdkExecutable: "C:/Program Files/dotnet/dotnet.exe",
    sdkAuthenticode: {
      status: "Valid",
      signatureType: "Authenticode",
      signerSubject: "CN=.NET, O=Microsoft Corporation, L=Redmond, S=Washington, C=US",
      signerThumbprint: "BB793DB742624269BB5F4515BBE9A3DF418F588D",
      originalFilename: ".NET Host",
    },
    toolClosure: {
      archivePrefix: "tools/net10.0/any/",
      entryPoint: "Corvus.Json.Cli.dll",
      fileCount: 359,
      treeSha256: "3b4d31d9bdeef8592efe37470d7d0c34340f11ec987d99a72d35c5688107d426",
    },
    restoreArguments: ["tool", "restore", "--tool-manifest", ".config/dotnet-tools.json"],
    runtime: "Corvus.Text.Json@5.2.7",
    license: "Apache-2.0",
  }, FAILURE_CODES.version, ".NET generator lock");
  equalJson([...lock.failureCodes].sort(), expectedFailureCodes, FAILURE_CODES.lock, "failure code set");

  const production = lock.sourceSet?.production;
  equalJson(production?.roots?.map(({ file, typeName }) => [file, typeName]), productionRoots,
    FAILURE_CODES.lock, "production source set");
  equalJson(production?.roots?.map(({ file, id }) => [file, id]), productionRoots.map(([file]) => [
    file,
    `https://schemas.sapphirus.dev/${file === "evidence-event.schema.json" ? "v2" : "v1"}/${file}`,
  ]), FAILURE_CODES.lock, "production canonical IDs");
  equalJson(production?.dependencies?.map(({ file, prefix }) => [file, prefix]),
    [["common.schema.json", "Common"]], FAILURE_CODES.lock, "production dependencies");
  equalJson(production?.dependencies?.map(({ file, id }) => [file, id]), [[
    "common.schema.json", "https://schemas.sapphirus.dev/v1/common.schema.json",
  ]], FAILURE_CODES.lock, "production dependency IDs");
  equalJson({
    directory: production?.directory,
    bundleId: production?.bundleId,
    rootType: production?.rootType,
  }, {
    directory: "packages/contracts/schemas",
    bundleId: "https://schemas.sapphirus.dev/codegen/contracts-catalog.schema.json",
    rootType: "SapphirusContractsCatalog",
  }, FAILURE_CODES.lock, "production bundle configuration");
  equalJson(lock.sourceSet?.qualification, {
    directory: "tests/generator-qualification/schemas",
    root: {
      file: "qualification.schema.json",
      id: "https://schemas.sapphirus.dev/generator-qualification/v1/qualification.schema.json",
      typeName: "GeneratorQualification",
    },
    resources: [{
      file: "qualification-external.schema.json",
      id: "https://schemas.sapphirus.dev/generator-qualification/v1/qualification-external.schema.json",
      prefix: "QualificationExternal",
    }],
    bundleId: "https://schemas.sapphirus.dev/codegen/generator-qualification.schema.json",
    rootType: "GeneratorQualification",
  }, FAILURE_CODES.lock, "qualification source set");
  equalJson(lock.invocations, {
    rust: { shell: false, maximumCapturedBytes: maximumOutputBytes },
    dotnet: { shell: false, maximumCapturedBytes: maximumOutputBytes },
    production: {
      rustOutput: "packages/contracts/generated/rust/contracts.rs",
      dotnetOutput: "packages/contracts/generated/dotnet",
      dotnetNamespace: "Sapphirus.Contracts.Generated",
    },
    qualification: {
      typescriptOutput: "tests/generator-qualification/generated/typescript/qualification.ts",
      rustOutput: "tests/generator-qualification/generated/rust/qualification.rs",
      dotnetOutput: "tests/generator-qualification/generated/dotnet",
      dotnetNamespace: "Sapphirus.GeneratorQualification.Generated",
    },
  }, FAILURE_CODES.lock, "invocation configuration");
  equalJson(lock.stagingPolicy, {
    strategy: "internal-$defs-bundle-v1",
    runRoots: ["target/c/a", "target/c/b"],
    modeDirectories: { production: "p", qualification: "q" },
    dotnetOutputPathLengths: { production: 86, qualification: 89 },
    outputPathLengthStrategy: "fixed-absolute-corvus-output-root-v1",
    transformVersion: "internal-$defs-bundle-v1",
    documentRootKeywordsRemoved: ["$id", "$schema", "$defs"],
    nestedScopeKeywordsRejected: ["$id", "$schema", "$defs"],
    unsupportedReferenceKeywordsRejected: [
      "$anchor", "$dynamicAnchor", "$dynamicRef", "$recursiveAnchor", "$recursiveRef",
    ],
    definitionExtraction: "prefix-and-promote-to-bundle-$defs",
    optionalNullablePresenceTransform: "partition-qualification-root-oneOf-v1",
    productionOptionalNullablePolicy: "fail-closed-v1",
    declaredReferencesOnly: true,
    rejectDefinitionCollisions: true,
    rejectSourceSetDrift: true,
  }, FAILURE_CODES.lock, "staging policy");
  equalJson(lock.normalizationPolicy, {
    encoding: "UTF-8",
    decode: "fatal",
    lineEndings: "LF",
    terminalLineFeedCount: 1,
    pathOrder: "utf8-byte-ordinal",
    rejectNul: true,
    rejectEmbeddedBom: true,
    rejectRunPathLeaks: true,
  }, FAILURE_CODES.lock, "normalization policy");
  return lock;
}

async function validateManifest() {
  const physicalManifest = await physicalRepositoryPath(
    manifestPath,
    ".NET tool manifest",
    FAILURE_CODES.lock,
  );
  const manifest = await readJson(physicalManifest, FAILURE_CODES.lock, ".NET tool manifest");
  equalJson(manifest, {
    version: 1,
    isRoot: true,
    tools: {
      "corvus.json.cli": {
        version: "5.2.7",
        commands: ["corvusjson"],
        rollForward: false,
      },
    },
  }, FAILURE_CODES.lock, ".NET tool manifest");
}

async function validateBootstrapLocks(lock) {
  for (const [name, record] of Object.entries(lock.bootstrapLocks)) {
    if (record.status !== "reviewed") {
      fail(FAILURE_CODES.lock, `${name} bootstrap lock is not reviewed.`);
    }
    const physical = await physicalRepositoryPath(
      repositoryPath(record.file),
      `${name} bootstrap lock`,
      FAILURE_CODES.lock,
    );
    if (await hashFile(physical) !== record.sha256) {
      fail(FAILURE_CODES.lock, `${name} bootstrap lock hash changed.`);
    }
  }
}

async function runProcess(executable, argumentsList, {
  acceptedExitCodes = [0],
  beforeSpawn,
  environmentKind,
  label,
}) {
  assertNoInheritedNativeToolInjection();
  if (beforeSpawn !== undefined) await beforeSpawn();
  assertNoInheritedNativeToolInjection();
  const environment = createLockedNativeChildEnvironment(environmentKind);
  return await new Promise((resolve, reject) => {
    const child = spawn(executable, argumentsList, {
      cwd: repositoryRoot,
      env: environment,
      shell: false,
      windowsHide: true,
      stdio: ["ignore", "pipe", "pipe"],
    });
    const stdout = [];
    const stderr = [];
    let captured = 0;
    let overflow = false;
    const capture = (target) => (chunk) => {
      captured += chunk.length;
      if (captured > maximumOutputBytes) {
        overflow = true;
        child.kill();
        return;
      }
      target.push(chunk);
    };
    child.stdout.on("data", capture(stdout));
    child.stderr.on("data", capture(stderr));
    child.on("error", (error) => reject(error));
    child.on("close", (exitCode) => {
      const stdoutOutput = Buffer.concat(stdout).toString("utf8").trim();
      const stderrOutput = Buffer.concat(stderr).toString("utf8").trim();
      const output = [stdoutOutput, stderrOutput].filter((value) => value !== "").join("\n");
      if (overflow) {
        reject(new Error(`${label} exceeded the ${maximumOutputBytes}-byte diagnostic limit.`));
      } else if (!acceptedExitCodes.includes(exitCode)) {
        reject(new Error(`${label} exited ${exitCode}: ${redact(output)}`));
      } else {
        resolve({ exitCode, output, stderr: stderrOutput, stdout: stdoutOutput });
      }
    });
  });
}

function redact(value) {
  return value
    .replaceAll(repositoryRoot, "<repository>")
    .replaceAll(normalizeRelative(repositoryRoot), "<repository>");
}

function activeCacheRoot(environmentName, fallback, label) {
  const configured = process.env[environmentName];
  const selected = configured === undefined || configured === "" ? fallback : configured;
  if (selected === "" || !path.isAbsolute(selected)) {
    fail(FAILURE_CODES.lock, `${label} must resolve to an absolute active cache path.`);
  }
  return path.resolve(selected);
}

async function physicalContainedPath(root, candidate, label, code = FAILURE_CODES.version) {
  if (!path.isAbsolute(root) || !path.isAbsolute(candidate)) {
    fail(code, `${label} must use absolute physical paths.`);
  }
  const rootPath = path.resolve(root);
  const candidatePath = path.resolve(candidate);
  assertContained(rootPath, candidatePath, code);
  const rootStats = await lstat(rootPath);
  if (rootStats.isSymbolicLink()) fail(code, `${label} root is a substituted link.`);
  let cursor = rootPath;
  for (const segment of path.relative(rootPath, candidatePath).split(path.sep)) {
    cursor = path.join(cursor, segment);
    const stats = await lstat(cursor);
    if (stats.isSymbolicLink()) fail(code, `${label} traverses a substituted link.`);
  }
  const [physicalRoot, physicalCandidate] = await Promise.all([
    realpath(rootPath),
    realpath(candidatePath),
  ]);
  assertContained(physicalRoot, physicalCandidate, code);
  return physicalCandidate;
}

async function listExpandedClosure(root, prefix = "", files = new Map()) {
  const entries = await readdir(root, { withFileTypes: true });
  entries.sort((left, right) => ordinalCompare(left.name, right.name));
  for (const entry of entries) {
    const absolutePath = path.join(root, entry.name);
    const relativePath = prefix === "" ? entry.name : `${prefix}/${entry.name}`;
    const stats = await lstat(absolutePath);
    if (stats.isSymbolicLink()) {
      fail(FAILURE_CODES.lock, `The expanded Corvus closure contains a substituted link: ${relativePath}`);
    }
    if (stats.isDirectory()) {
      await listExpandedClosure(absolutePath, relativePath, files);
    } else if (stats.isFile()) {
      files.set(relativePath, { absolutePath, size: stats.size });
    } else {
      fail(FAILURE_CODES.lock, `The expanded Corvus closure contains a non-file entry: ${relativePath}`);
    }
  }
  return files;
}

async function validateCorvusExpandedClosure(packagePath, closureRoot, identity) {
  const archive = await readFile(packagePath);
  const entries = readZipClosure(archive, identity.archivePrefix);
  const expanded = await listExpandedClosure(closureRoot);
  const archivePaths = [...entries.keys()].sort(ordinalCompare);
  const expandedPaths = [...expanded.keys()].sort(ordinalCompare);
  if (archivePaths.length !== identity.fileCount
    || JSON.stringify(archivePaths) !== JSON.stringify(expandedPaths)) {
    fail(FAILURE_CODES.version, "The expanded Corvus closure inventory changed.");
  }
  const records = [];
  for (const relativePath of archivePaths) {
    const expected = extractZipEntry(archive, entries.get(relativePath), relativePath);
    const actualRecord = expanded.get(relativePath);
    if (actualRecord.size !== expected.length) {
      fail(FAILURE_CODES.version, `The expanded Corvus closure changed size at ${relativePath}.`);
    }
    const actual = await readFile(actualRecord.absolutePath);
    if (!actual.equals(expected)) {
      fail(FAILURE_CODES.version, `The expanded Corvus closure changed bytes at ${relativePath}.`);
    }
    records.push({
      path: relativePath,
      sha256: createHash("sha256").update(expected).digest("hex"),
      size: expected.length,
    });
  }
  if (closureDigest(records) !== identity.treeSha256) {
    fail(FAILURE_CODES.version, "The reviewed Corvus closure digest changed.");
  }
  const entryPoint = expanded.get(identity.entryPoint)?.absolutePath;
  if (entryPoint === undefined) {
    fail(FAILURE_CODES.version, "The reviewed Corvus closure entry point is unavailable.");
  }
  return entryPoint;
}

async function validateCargoExecutableIdentity(executable, identity) {
  const stats = await lstat(executable);
  if (!stats.isFile() || stats.isSymbolicLink() || stats.size !== identity.fileSize) {
    fail(FAILURE_CODES.version, "The cargo-typify executable identity changed size or type.");
  }
  const digest = normalizedPortableExecutableSha256(await readFile(executable));
  if (digest !== identity.normalizedSha256) {
    fail(FAILURE_CODES.version, "The cargo-typify normalized executable digest changed.");
  }
}

async function validateDotnetHostAuthenticode(executable, identity) {
  const [verifier, securityModule, utilityModule] = await Promise.all([
    physicalContainedPath(
      "C:\\Windows",
      trustedPowerShellPath,
      "trusted Windows Authenticode verifier",
      FAILURE_CODES.version,
    ),
    physicalContainedPath(
      "C:\\Windows",
      trustedPowerShellSecurityModulePath,
      "trusted Windows PowerShell security module",
      FAILURE_CODES.version,
    ),
    physicalContainedPath(
      "C:\\Windows",
      trustedPowerShellUtilityModulePath,
      "trusted Windows PowerShell utility module",
      FAILURE_CODES.version,
    ),
  ]);
  const escapedPath = executable.replaceAll("'", "''");
  const escapedSecurityModule = securityModule.replaceAll("'", "''");
  const escapedUtilityModule = utilityModule.replaceAll("'", "''");
  const script = [
    "$ErrorActionPreference = 'Stop'",
    "$PSModuleAutoLoadingPreference = 'None'",
    "Set-StrictMode -Version Latest",
    `Import-Module -Name '${escapedSecurityModule}' -Force -ErrorAction Stop`,
    `Import-Module -Name '${escapedUtilityModule}' -Force -ErrorAction Stop`,
    `$target = '${escapedPath}'`,
    "$signature = Microsoft.PowerShell.Security\\Get-AuthenticodeSignature -LiteralPath $target",
    "$version = [Diagnostics.FileVersionInfo]::GetVersionInfo($target)",
    "[ordered]@{status=$signature.Status.ToString();signatureType=$signature.SignatureType.ToString();signerSubject=$signature.SignerCertificate.Subject;signerThumbprint=$signature.SignerCertificate.Thumbprint;originalFilename=$version.OriginalFilename} | Microsoft.PowerShell.Utility\\ConvertTo-Json -Compress",
  ].join("; ");
  const encoded = Buffer.from(script, "utf16le").toString("base64");
  let probe;
  try {
    probe = await runProcess(verifier, [
      "-NoLogo", "-NoProfile", "-NonInteractive", "-EncodedCommand", encoded,
    ], {
      beforeSpawn: async () => {
        const [currentVerifier, currentSecurityModule, currentUtilityModule] = await Promise.all([
          physicalContainedPath(
            "C:\\Windows",
            trustedPowerShellPath,
            "trusted Windows Authenticode verifier",
            FAILURE_CODES.version,
          ),
          physicalContainedPath(
            "C:\\Windows",
            trustedPowerShellSecurityModulePath,
            "trusted Windows PowerShell security module",
            FAILURE_CODES.version,
          ),
          physicalContainedPath(
            "C:\\Windows",
            trustedPowerShellUtilityModulePath,
            "trusted Windows PowerShell utility module",
            FAILURE_CODES.version,
          ),
        ]);
        if (path.win32.normalize(currentVerifier).toLowerCase()
            !== path.win32.normalize(verifier).toLowerCase()
          || path.win32.normalize(currentSecurityModule).toLowerCase()
            !== path.win32.normalize(securityModule).toLowerCase()
          || path.win32.normalize(currentUtilityModule).toLowerCase()
            !== path.win32.normalize(utilityModule).toLowerCase()) {
          fail(FAILURE_CODES.version, "The trusted Windows Authenticode verifier closure changed.");
        }
      },
      environmentKind: "powershell",
      label: ".NET host Authenticode probe",
    });
  } catch (error) {
    if (error?.code === "ENOENT") {
      fail(FAILURE_CODES.fallback, "The trusted Windows Authenticode verifier is unavailable.");
    }
    throw error;
  }
  let actual;
  if (probe.stderr !== "" && (!probe.stderr.startsWith("#< CLIXML")
    || probe.stderr.includes(' S="Error"'))) {
    fail(FAILURE_CODES.version, "The .NET host Authenticode verifier emitted diagnostics.");
  }
  try {
    actual = parseStrictJson(probe.stdout);
  } catch (error) {
    fail(FAILURE_CODES.version, `The .NET host Authenticode evidence is malformed: ${error.message}`);
  }
  equalJson(actual, identity, FAILURE_CODES.version, ".NET host Authenticode identity");
}

async function findCargoArchive(cargoHome) {
  const cacheRoot = path.join(cargoHome, "registry", "cache");
  let registries;
  try {
    registries = await readdir(cacheRoot, { withFileTypes: true });
  } catch (error) {
    if (error?.code === "ENOENT" || error?.code === "EACCES") return null;
    throw error;
  }
  const matches = [];
  for (const entry of registries.sort((a, b) => ordinalCompare(a.name, b.name))) {
    if (entry.isSymbolicLink()) {
      fail(FAILURE_CODES.lock, "The active Cargo registry cache contains a substituted directory.");
    }
    if (!entry.isDirectory()) continue;
    const candidate = path.join(cacheRoot, entry.name, "cargo-typify-0.6.1.crate");
    try {
      const stats = await lstat(candidate);
      if (stats.isSymbolicLink()) {
        fail(FAILURE_CODES.lock, "The cargo-typify source archive is a substituted link.");
      }
      if (stats.isFile()) matches.push(candidate);
    } catch (error) {
      if (error?.code !== "ENOENT") throw error;
    }
  }
  if (matches.length > 1) {
    fail(FAILURE_CODES.lock, "The active Cargo cache contains ambiguous cargo-typify archives.");
  }
  return matches[0] ?? null;
}

async function validateNodeToolchain(lock) {
  const pnpmLockSource = (await readFile(path.join(repositoryRoot, "pnpm-lock.yaml"), "utf8"))
    .replaceAll("\r\n", "\n");
  for (const [identity, integrity] of Object.entries(lock.tools.typescript.packageIntegrities)) {
    const exactResolution = `  ${identity}:\n    resolution: {integrity: ${integrity}}`;
    if (!pnpmLockSource.includes(exactResolution)) {
      fail(FAILURE_CODES.version, `${identity} is not bound to its reviewed pnpm integrity.`);
    }
  }
  for (const [packageName, version] of [
    ["json-schema-to-typescript", "15.0.4"],
    ["ajv", "8.20.0"],
    ["typescript", "7.0.2"],
  ]) {
    const manifest = await readJson(
      path.join(packageRoot, "node_modules", packageName, "package.json"),
      FAILURE_CODES.fallback,
      `${packageName} package manifest`,
    );
    if (manifest.name !== packageName || manifest.version !== version) {
      fail(FAILURE_CODES.version, `Expected ${packageName}@${version}.`);
    }
  }
}

export async function resolveRepoLocalExecutable(relativePath) {
  const executable = repositoryPath(relativePath);
  try {
    await lstat(executable);
  } catch (error) {
    if (error?.code === "ENOENT") {
      fail(FAILURE_CODES.fallback, `Missing repo-local ${relativePath}.`);
    }
    throw error;
  }
  return physicalRepositoryPath(
    executable,
    `repo-local executable ${relativePath}`,
    FAILURE_CODES.version,
  );
}

export async function preflightNativeTools(lock) {
  assertNoInheritedNativeToolInjection();
  lock ??= await loadAndValidateToolLock();
  await validateBootstrapLocks(lock);
  await validateManifest();
  await validateNodeToolchain(lock);

  const rustRealPath = await resolveRepoLocalExecutable(lock.tools.rust.resolvedExecutable);
  await validateCargoExecutableIdentity(rustRealPath, lock.tools.rust.executableIdentity);
  const installMetadataPath = await physicalRepositoryPath(
    repositoryPath(lock.tools.rust.installMetadata.path),
    "cargo install metadata",
    FAILURE_CODES.lock,
  );
  if (await hashFile(installMetadataPath) !== lock.tools.rust.installMetadata.sha256) {
    fail(FAILURE_CODES.version, "The cargo install metadata checksum changed.");
  }
  const installMetadata = await readJson(
    installMetadataPath,
    FAILURE_CODES.lock,
    "cargo install metadata",
  );
  const metadata = lock.tools.rust.installMetadata;
  equalJson(installMetadata, {
    installs: {
      [metadata.installKey]: {
        version_req: metadata.versionRequirement,
        bins: metadata.bins,
        features: metadata.features,
        all_features: metadata.allFeatures,
        no_default_features: metadata.noDefaultFeatures,
        profile: metadata.profile,
        target: metadata.target,
        rustc: metadata.rustc,
      },
    },
  }, FAILURE_CODES.version, "cargo install metadata");

  const userProfile = process.env.USERPROFILE ?? "";
  const cargoHome = activeCacheRoot(
    "CARGO_HOME",
    userProfile === "" ? "" : path.join(userProfile, ".cargo"),
    "CARGO_HOME",
  );
  const cargoArchive = await findCargoArchive(cargoHome);
  if (cargoArchive === null) {
    fail(FAILURE_CODES.lock, "The reviewed cargo-typify source archive is unavailable.");
  }
  if (await hashFile(cargoArchive) !== lock.tools.rust.packageSha256) {
    fail(FAILURE_CODES.version, "The cargo-typify source archive checksum changed.");
  }

  const nugetPackages = activeCacheRoot(
    "NUGET_PACKAGES",
    userProfile === "" ? "" : path.join(userProfile, ".nuget", "packages"),
    "NUGET_PACKAGES",
  );
  const nugetRoot = path.join(nugetPackages, "corvus.json.cli", "5.2.7");
  const nugetPackage = path.join(nugetRoot, "corvus.json.cli.5.2.7.nupkg");
  const nugetSha512 = path.join(nugetRoot, "corvus.json.cli.5.2.7.nupkg.sha512");
  let physicalPackage;
  try {
    const [packageStats, checksumStats] = await Promise.all([
      lstat(nugetPackage),
      lstat(nugetSha512),
    ]);
    if (!packageStats.isFile() || packageStats.isSymbolicLink()
      || !checksumStats.isFile() || checksumStats.isSymbolicLink()) {
      fail(FAILURE_CODES.lock, "The active Corvus package artifacts are substituted links.");
    }
    const [physicalCache, resolvedPackage, physicalChecksum] = await Promise.all([
      realpath(nugetPackages),
      realpath(nugetPackage),
      realpath(nugetSha512),
    ]);
    physicalPackage = resolvedPackage;
    assertContained(physicalCache, physicalPackage, FAILURE_CODES.lock);
    assertContained(physicalCache, physicalChecksum, FAILURE_CODES.lock);
    if ((await readFile(physicalChecksum, "utf8")).trim() !== lock.tools.dotnet.packageSha512
      || await hashFile(physicalPackage) !== lock.tools.dotnet.packageSha256) {
      fail(FAILURE_CODES.version, "The Corvus.Json.Cli package checksum changed.");
    }
  } catch (error) {
    if (error?.code === "ENOENT") {
      fail(FAILURE_CODES.lock, "The reviewed Corvus.Json.Cli package archive is unavailable.");
    }
    throw error;
  }

  const closureCandidate = path.join(
    nugetRoot,
    ...lock.tools.dotnet.toolClosure.archivePrefix.split("/").filter(Boolean),
  );
  let physicalClosure;
  try {
    physicalClosure = await physicalContainedPath(
      nugetPackages,
      closureCandidate,
      "expanded Corvus closure",
      FAILURE_CODES.lock,
    );
  } catch (error) {
    if (error?.code === "ENOENT") {
      fail(FAILURE_CODES.lock, "The reviewed expanded Corvus closure is unavailable.");
    }
    throw error;
  }
  const dotnetToolAssembly = await validateCorvusExpandedClosure(
    physicalPackage,
    physicalClosure,
    lock.tools.dotnet.toolClosure,
  );

  if (process.platform !== "win32") {
    fail(FAILURE_CODES.fallback, "The pinned native generator toolchain requires Windows.");
  }
  const dotnetCandidate = path.win32.normalize(lock.tools.dotnet.sdkExecutable);
  const trustedProgramFiles = path.win32.dirname(path.win32.dirname(dotnetCandidate));
  const configuredProgramFiles = process.env.ProgramFiles;
  if (configuredProgramFiles !== undefined
    && path.win32.normalize(configuredProgramFiles).toLowerCase()
      !== trustedProgramFiles.toLowerCase()) {
    fail(FAILURE_CODES.lock, "The Program Files root is environment-substituted.");
  }
  let dotnetExecutable;
  try {
    dotnetExecutable = await physicalContainedPath(
      trustedProgramFiles,
      dotnetCandidate,
      "pinned .NET SDK executable",
      FAILURE_CODES.version,
    );
  } catch (error) {
    if (error?.code === "ENOENT") fail(FAILURE_CODES.fallback, "The pinned .NET SDK is missing.");
    throw error;
  }
  await validateDotnetHostAuthenticode(dotnetExecutable, lock.tools.dotnet.sdkAuthenticode);

  const executionIdentities = {
    corvusClosureRoot: physicalClosure,
    corvusPackage: physicalPackage,
    dotnetExecutable,
    dotnetToolAssembly,
    lock,
    rustExecutable: rustRealPath,
  };

  const rustVersion = await runProcess(rustRealPath, lock.tools.rust.versionArguments, {
    acceptedExitCodes: [lock.tools.rust.versionExitCode],
    beforeSpawn: () => validateCargoExecutableIdentity(
      rustRealPath,
      lock.tools.rust.executableIdentity,
    ),
    environmentKind: "rust",
    label: "cargo-typify version probe",
  });
  if (rustVersion.output !== lock.tools.rust.versionOutput) {
    fail(FAILURE_CODES.version, `Unexpected cargo-typify version output: ${rustVersion.output}`);
  }
  let dotnetVersion;
  try {
    dotnetVersion = await runProcess(dotnetExecutable, ["--version"], {
      beforeSpawn: () => validateDotnetHostAuthenticode(
        dotnetExecutable,
        lock.tools.dotnet.sdkAuthenticode,
      ),
      environmentKind: "dotnet",
      label: ".NET SDK probe",
    });
  } catch (error) {
    if (error?.code === "ENOENT") fail(FAILURE_CODES.fallback, "The pinned .NET SDK is missing.");
    throw error;
  }
  if (dotnetVersion.output !== lock.tools.dotnet.sdk) {
    fail(FAILURE_CODES.version, `Expected .NET ${lock.tools.dotnet.sdk}; received ${dotnetVersion.output}.`);
  }
  const dotnetVersionProbe = await runProcess(dotnetExecutable, [
    dotnetToolAssembly, ...lock.tools.dotnet.versionArguments,
  ], {
    acceptedExitCodes: [lock.tools.dotnet.versionExitCode],
    beforeSpawn: () => revalidateCorvusExecutionIdentities(executionIdentities),
    environmentKind: "dotnet",
    label: "Corvus version probe",
  });
  if (dotnetVersionProbe.output !== lock.tools.dotnet.versionOutput) {
    fail(FAILURE_CODES.version, `Unexpected Corvus version output: ${dotnetVersionProbe.output}`);
  }
  return executionIdentities;
}

async function revalidateCorvusExecutionIdentities(preflight) {
  const entryPoint = await validateCorvusExpandedClosure(
    preflight.corvusPackage,
    preflight.corvusClosureRoot,
    preflight.lock.tools.dotnet.toolClosure,
  );
  if (path.resolve(entryPoint).toLowerCase()
    !== path.resolve(preflight.dotnetToolAssembly).toLowerCase()) {
    fail(FAILURE_CODES.version, "The selected Corvus entry point changed after preflight.");
  }
  await validateDotnetHostAuthenticode(
    preflight.dotnetExecutable,
    preflight.lock.tools.dotnet.sdkAuthenticode,
  );
}

function decodePointerToken(value) {
  return decodeURIComponent(value).replaceAll("~1", "/").replaceAll("~0", "~");
}

function encodePointerToken(value) {
  return value.replaceAll("~", "~0").replaceAll("/", "~1");
}

function propertyName(typeName) {
  return `${typeName[0].toLowerCase()}${typeName.slice(1)}`;
}

function createStrictAjv() {
  return new Ajv2020({
    allErrors: true,
    strict: true,
    strictTypes: false,
    validateFormats: false,
  });
}

function validatorAtReference(ajv, reference, label) {
  try {
    return ajv.getSchema(reference) ?? ajv.compile({ $ref: reference });
  } catch (error) {
    fail(FAILURE_CODES.parity, `${label} cannot be compiled for null-presence analysis: ${error.message}`);
  }
}

export function partitionOptionalNullableRoot(schema, rootType, definitions) {
  if (schema.type !== "object" || schema.additionalProperties !== false
    || schema.properties === null || typeof schema.properties !== "object") {
    return schema;
  }
  const probeId = "urn:sapphirus:generator-qualification:null-presence-probe";
  const probe = {
    $schema: "https://json-schema.org/draft/2020-12/schema",
    $id: probeId,
    ...structuredClone(schema),
    $defs: structuredClone(definitions),
  };
  const ajv = createStrictAjv();
  try {
    ajv.addSchema(probe, probeId);
  } catch (error) {
    fail(
      FAILURE_CODES.parity,
      `Qualification schema cannot be compiled for null-presence analysis: ${error.message}`,
    );
  }
  const acceptsNull = (name) => validatorAtReference(
    ajv,
    `${probeId}#/properties/${encodePointerToken(name)}`,
    `Qualification property ${name}`,
  )(null);
  const required = new Set(schema.required ?? []);
  const optionalNullable = Object.entries(schema.properties)
    .filter(([name]) => !required.has(name) && acceptsNull(name))
    .map(([name]) => name)
    .sort(ordinalCompare);
  if (optionalNullable.length === 0) return schema;
  if (optionalNullable.length > 4) {
    fail(FAILURE_CODES.parity, "Optional-nullable presence partition exceeds its reviewed bound.");
  }
  const variants = [];
  const variantCount = 2 ** optionalNullable.length;
  for (let mask = 0; mask < variantCount; mask += 1) {
    const variant = structuredClone(schema);
    delete variant.title;
    const labels = [];
    const variantRequired = new Set(variant.required ?? []);
    for (const [index, name] of optionalNullable.entries()) {
      if ((mask & (1 << index)) !== 0) {
        variantRequired.add(name);
        labels.push(`With${name[0].toUpperCase()}${name.slice(1)}`);
      } else {
        delete variant.properties[name];
        labels.push(`Without${name[0].toUpperCase()}${name.slice(1)}`);
      }
    }
    variant.required = [...variantRequired];
    variant.title = `${rootType}${labels.join("And")}`;
    variants.push(variant);
  }
  return { oneOf: variants };
}

export function findOptionalNullableProperties(schema, acceptsNull, pointer = "#", findings = []) {
  if (schema === null || typeof schema !== "object") return findings;
  if (Array.isArray(schema)) return findings;
  if (schema.properties !== null && typeof schema.properties === "object"
    && !Array.isArray(schema.properties)) {
    const required = new Set(schema.required ?? []);
    for (const [name, propertySchema] of Object.entries(schema.properties)) {
      const propertyPointer = `${pointer}/properties/${encodePointerToken(name)}`;
      if (!required.has(name) && acceptsNull(propertyPointer)) {
        findings.push(propertyPointer);
      }
    }
  }
  for (const [key, value] of Object.entries(schema)) {
    const memberPointer = `${pointer}/${encodePointerToken(key)}`;
    if (key === "$defs" || schemaMapKeywords.has(key)) {
      if (value !== null && typeof value === "object" && !Array.isArray(value)) {
        for (const [name, child] of Object.entries(value)) {
          findOptionalNullableProperties(
            child,
            acceptsNull,
            `${memberPointer}/${encodePointerToken(name)}`,
            findings,
          );
        }
      }
    } else if (schemaArrayKeywords.has(key)) {
      if (Array.isArray(value)) {
        value.forEach((child, index) => findOptionalNullableProperties(
          child,
          acceptsNull,
          `${memberPointer}/${index}`,
          findings,
        ));
      }
    } else if (schemaValueKeywords.has(key)) {
      findOptionalNullableProperties(value, acceptsNull, memberPointer, findings);
    }
  }
  return findings;
}

async function loadSourceDocuments(configuration, mode) {
  const directory = await physicalRepositoryPath(
    repositoryPath(configuration.directory),
    `${mode} schema source directory`,
    FAILURE_CODES.parity,
  );
  const expectedFiles = mode === "production"
    ? [...configuration.roots.map(({ file }) => file), ...configuration.dependencies.map(({ file }) => file)]
    : [configuration.root.file, ...configuration.resources.map(({ file }) => file)];
  const schemaEntries = (await readdir(directory, { withFileTypes: true }))
    .filter((entry) => entry.name.endsWith(".schema.json"));
  for (const entry of schemaEntries) {
    if (!entry.isFile() || entry.isSymbolicLink()) {
      fail(FAILURE_CODES.parity, `${entry.name} is not a regular schema source.`);
    }
  }
  const actualFiles = schemaEntries.map((entry) => entry.name).sort(ordinalCompare);
  equalJson(actualFiles, [...expectedFiles].sort(ordinalCompare), FAILURE_CODES.parity,
    `${mode} schema source set`);
  const descriptors = mode === "production"
    ? [
      ...configuration.roots.map((entry) => ({ ...entry, prefix: entry.typeName, role: "root" })),
      ...configuration.dependencies.map((entry) => ({ ...entry, role: "dependency" })),
    ]
    : [
      { ...configuration.root, prefix: configuration.root.typeName, role: "root" },
      ...configuration.resources.map((entry) => ({ ...entry, role: "dependency" })),
    ];
  const documents = [];
  for (const descriptor of descriptors) {
    const filePath = await physicalRepositoryPath(
      assertContained(directory, path.resolve(directory, descriptor.file)),
      `${mode} schema source ${descriptor.file}`,
      FAILURE_CODES.parity,
    );
    const fileInfo = await lstat(filePath);
    if (!fileInfo.isFile() || fileInfo.isSymbolicLink()) {
      fail(FAILURE_CODES.parity, `${descriptor.file} must be a regular schema file.`);
    }
    const source = await readFile(filePath, "utf8");
    const schema = parseStrictJson(source);
    if (schema.$id !== descriptor.id || !schema.$id.startsWith("https://")) {
      fail(FAILURE_CODES.parity, `${descriptor.file} has an undeclared canonical $id.`);
    }
    documents.push({ ...descriptor, schema, source });
  }
  const ids = documents.map(({ id }) => id);
  if (new Set(ids).size !== ids.length) fail(FAILURE_CODES.parity, "Schema $id collision detected.");
  if (mode === "production") {
    const ajv = createStrictAjv();
    try {
      for (const document of documents) ajv.addSchema(document.schema, document.id);
    } catch (error) {
      fail(
        FAILURE_CODES.parity,
        `Production schema cannot be compiled for null-presence analysis: ${error.message}`,
      );
    }
    for (const document of documents) {
      const optionalNullable = findOptionalNullableProperties(
        document.schema,
        (propertyPointer) => validatorAtReference(
          ajv,
          `${document.id}${propertyPointer}`,
          `${document.file}${propertyPointer}`,
        )(null),
      );
      if (optionalNullable.length > 0) {
        fail(
          FAILURE_CODES.parity,
          `${document.file} contains optional-nullable properties that cargo-typify would collapse: `
            + optionalNullable.join(", "),
        );
      }
    }
  }
  return documents;
}

export function assertInternalReferenceClosure(schema) {
  if (schema === null || typeof schema !== "object" || Array.isArray(schema)
    || schema.$defs === null || typeof schema.$defs !== "object" || Array.isArray(schema.$defs)) {
    fail(FAILURE_CODES.parity, "Generated bundle must contain an object-valued $defs registry.");
  }
  const validateReference = (member, memberPointer) => {
    if (typeof member !== "string") {
      fail(FAILURE_CODES.parity, `${memberPointer} is not a string reference.`);
    }
    const match = /^#\/\$defs\/([^/]+)$/u.exec(member);
    if (match === null) {
      fail(
        FAILURE_CODES.parity,
        `${memberPointer} is not a closed local definition reference: ${member}`,
      );
    }
    const definitionName = decodePointerToken(match[1]);
    if (!Object.hasOwn(schema.$defs, definitionName)) {
      fail(FAILURE_CODES.parity, `${memberPointer} targets missing definition ${definitionName}.`);
    }
  };
  const visit = (value, pointer = "#") => {
    if (typeof value === "boolean") return;
    if (value === null || typeof value !== "object" || Array.isArray(value)) {
      fail(FAILURE_CODES.parity, `${pointer} is not a schema object or boolean.`);
    }
    for (const [key, member] of Object.entries(value)) {
      const memberPointer = `${pointer}/${encodePointerToken(key)}`;
      if (key === "$ref") {
        validateReference(member, memberPointer);
      } else if (key === "$dynamicRef" || key === "$recursiveRef") {
        fail(FAILURE_CODES.parity, `${memberPointer} is forbidden in the closed generator bundle.`);
      } else if (key === "$defs" || schemaMapKeywords.has(key)) {
        for (const [name, child] of Object.entries(member)) {
          visit(child, `${memberPointer}/${encodePointerToken(name)}`);
        }
      } else if (schemaArrayKeywords.has(key)) {
        member.forEach((child, index) => visit(child, `${memberPointer}/${index}`));
      } else if (schemaValueKeywords.has(key)) {
        visit(member, memberPointer);
      }
    }
  };
  visit(schema);
}

const schemaMapKeywords = new Set([
  "properties", "patternProperties", "dependentSchemas",
]);
const schemaArrayKeywords = new Set([
  "allOf", "anyOf", "oneOf", "prefixItems",
]);
const schemaValueKeywords = new Set([
  "additionalProperties", "contains", "contentSchema", "else", "if", "items", "not",
  "propertyNames", "then", "unevaluatedItems", "unevaluatedProperties",
]);
const dataValueKeywords = new Set([
  "$comment", "const", "default", "dependentRequired", "deprecated", "description", "enum",
  "examples", "exclusiveMaximum", "exclusiveMinimum", "format", "maxContains", "maximum",
  "maxItems", "maxLength", "maxProperties", "minContains", "minimum", "minItems", "minLength",
  "minProperties", "multipleOf", "pattern", "readOnly", "required", "title", "type", "uniqueItems",
  "writeOnly",
]);

export function transformSchemaTree(value, {
  documentRoot = false,
  retainDocumentKeywords = false,
  rewriteReference = (reference) => reference,
  sourceFile = "<schema>",
} = {}) {
  const visit = (schema, isDocumentRoot, pointer) => {
    if (typeof schema === "boolean") return schema;
    if (schema === null || typeof schema !== "object" || Array.isArray(schema)) {
      fail(FAILURE_CODES.parity, `${sourceFile}${pointer} is not a schema object or boolean.`);
    }
    const result = {};
    for (const [key, member] of Object.entries(schema)) {
      const memberPointer = `${pointer}/${encodePointerToken(key)}`;
      if (key === "$id" || key === "$schema") {
        if (!isDocumentRoot) {
          fail(FAILURE_CODES.parity, `${sourceFile}${memberPointer} is unsupported nested schema scope.`);
        }
        if (retainDocumentKeywords) result[key] = structuredClone(member);
        continue;
      }
      if (key === "$defs") {
        if (!isDocumentRoot) {
          fail(FAILURE_CODES.parity, `${sourceFile}${memberPointer} is unsupported nested schema scope.`);
        }
        if (retainDocumentKeywords) {
          if (member === null || typeof member !== "object" || Array.isArray(member)) {
            fail(FAILURE_CODES.parity, `${sourceFile}${memberPointer} must be a schema map.`);
          }
          result[key] = Object.fromEntries(Object.entries(member).map(([name, child]) => [
            name,
            visit(child, false, `${memberPointer}/${encodePointerToken(name)}`),
          ]));
        }
        continue;
      }
      if (key === "$anchor" || key === "$dynamicAnchor" || key === "$dynamicRef"
        || key === "$recursiveAnchor" || key === "$recursiveRef") {
        fail(FAILURE_CODES.parity, `${sourceFile}${memberPointer} is unsupported by the closed bundle transform.`);
      }
      if (key === "$ref") {
        if (typeof member !== "string") {
          fail(FAILURE_CODES.parity, `${sourceFile}${memberPointer} must be a string.`);
        }
        result[key] = rewriteReference(member);
      } else if (schemaMapKeywords.has(key)) {
        if (member === null || typeof member !== "object" || Array.isArray(member)) {
          fail(FAILURE_CODES.parity, `${sourceFile}${memberPointer} must be a schema map.`);
        }
        result[key] = Object.fromEntries(Object.entries(member).map(([name, child]) => [
          name,
          visit(child, false, `${memberPointer}/${encodePointerToken(name)}`),
        ]));
      } else if (schemaArrayKeywords.has(key)) {
        if (!Array.isArray(member)) {
          fail(FAILURE_CODES.parity, `${sourceFile}${memberPointer} must be a schema array.`);
        }
        result[key] = member.map((child, index) =>
          visit(child, false, `${memberPointer}/${index}`));
      } else if (schemaValueKeywords.has(key)) {
        result[key] = visit(member, false, memberPointer);
      } else if (dataValueKeywords.has(key)) {
        result[key] = structuredClone(member);
      } else if (member !== null && typeof member === "object") {
        fail(
          FAILURE_CODES.parity,
          `${sourceFile}${memberPointer} is an unsupported object-valued schema keyword.`,
        );
      } else {
        result[key] = member;
      }
    }
    return result;
  };
  return visit(value, documentRoot, "#");
}

export async function buildInternalBundle(lock, mode) {
  if (mode !== "production" && mode !== "qualification") {
    fail(FAILURE_CODES.parity, `Unsupported bundle mode ${mode}.`);
  }
  const configuration = lock.sourceSet[mode];
  const documents = await loadSourceDocuments(configuration, mode);
  const byId = new Map(documents.map((document) => [document.id, document]));
  const definitionNames = new Map();
  const bundleDefinitions = {};
  for (const document of documents) {
    const localDefinitions = document.schema.$defs ?? {};
    for (const name of Object.keys(localDefinitions).sort(ordinalCompare)) {
      const qualified = `${document.prefix}${name}`;
      if (definitionNames.has(qualified)) {
        fail(FAILURE_CODES.parity, `Bundled definition collision: ${qualified}.`);
      }
      definitionNames.set(`${document.id}#/$defs/${name}`, qualified);
      definitionNames.set(qualified, document.file);
    }
    if (document.role === "root") {
      if (definitionNames.has(document.typeName)) {
        fail(FAILURE_CODES.parity, `Bundled root collision: ${document.typeName}.`);
      }
      definitionNames.set(`${document.id}#`, document.typeName);
      definitionNames.set(document.typeName, document.file);
    }
  }

  const rewriteReference = (reference, document) => {
    let resolved;
    try {
      resolved = new URL(reference, document.id);
    } catch {
      fail(FAILURE_CODES.parity, `${document.file} contains an invalid $ref: ${reference}`);
    }
    const targetId = `${resolved.origin}${resolved.pathname}${resolved.search}`;
    const target = byId.get(targetId);
    if (target === undefined) {
      fail(FAILURE_CODES.parity, `${document.file} contains an undeclared external $ref: ${reference}`);
    }
    if (resolved.hash === "" || resolved.hash === "#") {
      if (mode === "qualification" && target.role === "root") return "#";
      const rootName = definitionNames.get(`${target.id}#`);
      if (rootName === undefined) {
        fail(FAILURE_CODES.parity, `${reference} targets a dependency document root.`);
      }
      return `#/$defs/${rootName}`;
    }
    const match = /^#\/\$defs\/([^/]+)$/u.exec(resolved.hash);
    if (match === null) {
      fail(FAILURE_CODES.parity, `${document.file} contains an unsupported $ref fragment: ${reference}`);
    }
    const localName = decodePointerToken(match[1]);
    const qualified = definitionNames.get(`${target.id}#/$defs/${localName}`);
    if (qualified === undefined) {
      fail(FAILURE_CODES.parity, `${reference} targets an undeclared definition.`);
    }
    return `#/$defs/${qualified}`;
  };

  const transform = (value, document, documentRoot = false) => transformSchemaTree(value, {
    documentRoot,
    rewriteReference: (reference) => rewriteReference(reference, document),
    sourceFile: document.file,
  });

  for (const document of documents) {
    for (const [name, definition] of Object.entries(document.schema.$defs ?? {}).sort(
      ([left], [right]) => ordinalCompare(left, right),
    )) {
      const qualified = definitionNames.get(`${document.id}#/$defs/${name}`);
      if (Object.hasOwn(bundleDefinitions, qualified)) {
        fail(FAILURE_CODES.parity, `Bundled definition collision: ${qualified}.`);
      }
      bundleDefinitions[qualified] = transform(definition, document);
    }
  }

  const rootDocuments = documents.filter(({ role }) => role === "root");
  if (mode === "production") {
    for (const document of rootDocuments) {
      bundleDefinitions[document.typeName] = transform(document.schema, document, true);
    }
  }
  const orderedDefinitions = Object.fromEntries(
    Object.entries(bundleDefinitions).sort(([left], [right]) => ordinalCompare(left, right)),
  );
  let bundle;
  if (mode === "production") {
    bundle = {
      $schema: "https://json-schema.org/draft/2020-12/schema",
      $id: configuration.bundleId,
      title: configuration.rootType,
      type: "object",
      additionalProperties: false,
      properties: Object.fromEntries(rootDocuments.map((document) => [
        propertyName(document.typeName), { $ref: `#/$defs/${document.typeName}` },
      ])),
      $defs: orderedDefinitions,
    };
  } else {
    const root = rootDocuments[0];
    const transformedRoot = partitionOptionalNullableRoot(
      transform(root.schema, root, true),
      configuration.rootType,
      orderedDefinitions,
    );
    bundle = {
      $schema: "https://json-schema.org/draft/2020-12/schema",
      $id: configuration.bundleId,
      ...transformedRoot,
      title: configuration.rootType,
      $defs: orderedDefinitions,
    };
  }
  assertInternalReferenceClosure(bundle);
  return { bundle, documents, source: stableJson(bundle) };
}

export function prepareRustCodegenBundle(bundle) {
  const rustBundle = structuredClone(bundle);
  const action = rustBundle.$defs?.BuilderAuthoringObjectBuilderAuthoringAction;
  const configLayer = rustBundle.$defs?.BmadPackageDescriptorBmadConfigLayer;
  if (!Array.isArray(action?.oneOf) || action.oneOf.length !== 2
    || !Array.isArray(configLayer?.oneOf) || configLayer.oneOf.length !== 3) {
    fail(FAILURE_CODES.parity, "Rust BMAD codegen relaxation targets changed shape.");
  }

  action.oneOf[0].properties.action.enum = ["create_rebuild", "edit", "analyze", "build"];
  action.oneOf[1].properties.action.enum = ["create_rebuild", "build", "edit", "analyze"];
  const allLayerKinds = [
    "installer_team",
    "installer_user",
    "custom_team",
    "custom_user",
    "packaged_default",
    "team_override",
    "user_override",
    "method_module_yaml",
    "builder_root_yaml",
    "builder_user_yaml",
  ];
  for (const branch of configLayer.oneOf) {
    branch.properties.layerKind.enum = [...allLayerKinds];
  }

  const actionReference = "#/$defs/BuilderAuthoringObjectBuilderAuthoringAction";
  const lensReference = "#/$defs/BuilderAuthoringObjectBuilderModelLensResult";
  const visit = (value) => {
    if (value === null || typeof value !== "object") return;
    if (Array.isArray(value)) {
      value.forEach(visit);
      return;
    }
    const authoringAction = value.properties?.authoringAction;
    if (authoringAction?.type === "object"
      && typeof authoringAction.properties?.builderKind?.const === "string") {
      value.properties.authoringAction = { $ref: actionReference };
    }
    const modelLensResults = value.properties?.modelLensResults;
    if (Array.isArray(modelLensResults?.prefixItems)) {
      delete modelLensResults.prefixItems;
      modelLensResults.items = { $ref: lensReference };
    }
    Object.values(value).forEach(visit);
  };
  visit(rustBundle);
  assertInternalReferenceClosure(rustBundle);
  return rustBundle;
}

function substitute(argumentsTemplate, replacements) {
  return argumentsTemplate.map((argument) => replacements[argument] ?? argument);
}

function normalizeGeneratedSource(buffer, relativePath, runRoot) {
  let source;
  try {
    source = decoder.decode(buffer);
  } catch (error) {
    fail(FAILURE_CODES.parity, `${relativePath} is not valid UTF-8: ${error.message}`);
  }
  if (source.includes("\0") || source.includes("\uFEFF")) {
    fail(FAILURE_CODES.parity, `${relativePath} contains a forbidden NUL or BOM.`);
  }
  source = source.replaceAll("\r\n", "\n").replaceAll("\r", "\n").replace(/\n*$/u, "\n");
  const forbiddenPaths = [repositoryRoot, normalizeRelative(repositoryRoot), runRoot, normalizeRelative(runRoot)];
  for (const forbidden of forbiddenPaths) {
    if (source.includes(forbidden)) {
      fail(FAILURE_CODES.nondeterministic, `${relativePath} embeds a repository or run path.`);
    }
  }
  if (source.includes("target/contract-codegen/")
    || source.includes("target\\contract-codegen\\")
    || /\b20\d{2}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z\b/u.test(source)
    || /\b[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\b/iu.test(source)) {
    fail(FAILURE_CODES.nondeterministic, `${relativePath} embeds staging or run-specific metadata.`);
  }
  return source;
}

async function readGeneratedTree(root, runRoot) {
  const physicalRoot = await physicalRepositoryPath(
    root,
    "native generator output root",
    FAILURE_CODES.parity,
  );
  const files = new Map();
  const visit = async (directory, prefix = "") => {
    const entries = (await readdir(directory, { withFileTypes: true }))
      .sort((left, right) => ordinalCompare(left.name, right.name));
    for (const entry of entries) {
      const absolute = path.join(directory, entry.name);
      const relative = prefix === "" ? entry.name : `${prefix}/${entry.name}`;
      if (entry.isSymbolicLink()) fail(FAILURE_CODES.parity, `${relative} is a generated symlink.`);
      if (entry.isDirectory()) await visit(absolute, relative);
      else if (entry.isFile()) {
        files.set(relative, normalizeGeneratedSource(await readFile(absolute), relative, runRoot));
      } else fail(FAILURE_CODES.parity, `${relative} is not a regular generated file.`);
    }
  };
  await visit(physicalRoot);
  if (files.size === 0) fail(FAILURE_CODES.parity, "Native generator produced an empty tree.");
  return files;
}

function compareTrees(left, right, label) {
  const leftNames = [...left.keys()].sort(ordinalCompare);
  const rightNames = [...right.keys()].sort(ordinalCompare);
  if (JSON.stringify(leftNames) !== JSON.stringify(rightNames)) {
    fail(FAILURE_CODES.nondeterministic, `${label} output inventories differ across clean runs.`);
  }
  for (const name of leftNames) {
    if (left.get(name) !== right.get(name)) {
      fail(FAILURE_CODES.nondeterministic, `${label} output bytes differ for ${name}.`);
    }
  }
}

async function runOneGeneration(preflight, mode, runName, bundleSource, rustBundleSource) {
  const runIndex = runName === "run-a" ? 0 : runName === "run-b" ? 1 : -1;
  if (runIndex < 0) fail(FAILURE_CODES.parity, `Unsupported native run ${runName}.`);
  const reviewedRunRoot = repositoryPath(preflight.lock.stagingPolicy.runRoots[runIndex]);
  assertContained(targetRoot, reviewedRunRoot, FAILURE_CODES.parity);
  const modeDirectory = preflight.lock.stagingPolicy.modeDirectories[mode];
  const runRoot = await safeRepositoryDestination(
    assertContained(targetRoot, path.join(reviewedRunRoot, modeDirectory)),
    `${mode} ${runName} staging root`,
    FAILURE_CODES.parity,
  );
  await rm(runRoot, { recursive: true, force: true });
  const input = path.join(runRoot, "input", `${mode}.schema.json`);
  const rustInput = path.join(runRoot, "input", `${mode}.rust.schema.json`);
  const rustRoot = path.join(runRoot, "rust");
  const dotnetRoot = canonicalDotnetOutputRoot(
    runRoot,
    mode,
    preflight.lock.stagingPolicy,
  );
  await mkdir(path.dirname(input), { recursive: true });
  await mkdir(rustRoot, { recursive: true });
  await mkdir(dotnetRoot, { recursive: true });
  await physicalRepositoryPath(runRoot, `${mode} ${runName} staging root`, FAILURE_CODES.parity);
  await writeFile(input, bundleSource, "utf8");
  await writeFile(rustInput, rustBundleSource, "utf8");
  const rustOutput = path.join(rustRoot, mode === "production" ? "contracts.rs" : "qualification.rs");
  const sourceSet = preflight.lock.sourceSet[mode];
  const namespace = preflight.lock.invocations[mode].dotnetNamespace;
  try {
    await Promise.all([
      runProcess(preflight.rustExecutable, substitute(expectedRustArguments, {
        "{input}": rustInput,
        "{output}": rustOutput,
      }), {
        beforeSpawn: () => validateCargoExecutableIdentity(
          preflight.rustExecutable,
          preflight.lock.tools.rust.executableIdentity,
        ),
        environmentKind: "rust",
        label: `${mode} cargo-typify generation`,
      }),
      runProcess(preflight.dotnetExecutable, [
        preflight.dotnetToolAssembly,
        ...substitute(expectedDotnetArguments, {
          "{input}": input,
          "{namespace}": namespace,
          "{rootType}": sourceSet.rootType,
          "{output}": dotnetRoot,
        }),
      ], {
        beforeSpawn: () => revalidateCorvusExecutionIdentities(preflight),
        environmentKind: "dotnet",
        label: `${mode} Corvus generation`,
      }),
    ]);
  } catch (error) {
    if (error?.code === "ENOENT") {
      fail(FAILURE_CODES.fallback, error.message);
    }
    fail(FAILURE_CODES.parity, redact(error.message));
  }
  return {
    dotnet: await readGeneratedTree(dotnetRoot, runRoot),
    rust: await readGeneratedTree(rustRoot, runRoot),
  };
}

export async function generateNativeTrees(mode) {
  const lock = await loadAndValidateToolLock();
  const preflight = await preflightNativeTools(lock);
  const bundle = await buildInternalBundle(lock, mode);
  const rustBundleSource = mode === "production"
    ? stableJson(prepareRustCodegenBundle(bundle.bundle))
    : bundle.source;
  await mkdir(targetRoot, { recursive: true });
  const [runA, runB] = await Promise.all([
    runOneGeneration(preflight, mode, "run-a", bundle.source, rustBundleSource),
    runOneGeneration(preflight, mode, "run-b", bundle.source, rustBundleSource),
  ]);
  compareTrees(runA.rust, runB.rust, `${mode} Rust`);
  compareTrees(runA.dotnet, runB.dotnet, `${mode} C#`);
  const orchestratorSources = {};
  for (const relativePath of [
    "packages/contracts/scripts/generate-all.mjs",
    "packages/contracts/scripts/generate.mjs",
    "packages/contracts/scripts/qualify-generators.mjs",
  "packages/contracts/scripts/lib/bmad-semantics.mjs",
  "packages/contracts/scripts/lib/bmad-fixtures.mjs",
    "packages/contracts/scripts/lib/controlled-contract-io.mjs",
    "packages/contracts/scripts/lib/generation-transaction.mjs",
    "packages/contracts/scripts/lib/native-codegen.mjs",
  ]) {
    const absolutePath = relativePath.endsWith("native-codegen.mjs")
      ? nativeCodegenPath
      : path.join(repositoryRoot, relativePath);
    orchestratorSources[relativePath] = sha256(await readFile(absolutePath));
  }
  return {
    bundle,
    configDigest: sha256(stableJson({
      sourceSet: lock.sourceSet,
      invocations: lock.invocations,
      stagingPolicy: lock.stagingPolicy,
      normalizationPolicy: lock.normalizationPolicy,
      rustArguments: lock.tools.rust.generationArguments,
      dotnetArguments: lock.tools.dotnet.generationArguments,
      orchestratorSources,
    })),
    dotnet: runA.dotnet,
    lock,
    rust: runA.rust,
  };
}

export function treeRecords(tree, prefix) {
  return [...tree.entries()]
    .sort(([left], [right]) => ordinalCompare(left, right))
    .map(([relativePath, source]) => ({
      file: `${prefix}/${relativePath}`,
      sha256: sha256(source),
    }));
}

export function treeDigest(records) {
  return sha256(records.map(({ file, sha256: digest }) => `${file}\0${digest}\n`).join(""));
}

async function listFilesRecursive(root, prefix = "") {
  let entries;
  try {
    entries = await readdir(root, { withFileTypes: true });
  } catch (error) {
    if (error?.code === "ENOENT") return [];
    throw error;
  }
  const files = [];
  for (const entry of entries.sort((left, right) => ordinalCompare(left.name, right.name))) {
    const relative = prefix === "" ? entry.name : `${prefix}/${entry.name}`;
    const absolute = path.join(root, entry.name);
    if (entry.isDirectory()) files.push(...await listFilesRecursive(absolute, relative));
    else if (entry.isFile() && !entry.isSymbolicLink()) files.push(relative);
    else fail(FAILURE_CODES.parity, `${relative} is not a regular file.`);
  }
  return files;
}

export async function listFiles(root) {
  const resolvedRoot = await safeRepositoryDestination(
    root,
    "controlled generated tree",
    FAILURE_CODES.parity,
  );
  return listFilesRecursive(resolvedRoot);
}

export async function synchronizeTree(root, expected, checkOnly) {
  const resolvedRoot = await safeRepositoryDestination(
    root,
    "controlled generated tree",
    FAILURE_CODES.parity,
  );
  const mismatches = [];
  for (const [relativePath, source] of [...expected.entries()].sort(
    ([left], [right]) => ordinalCompare(left, right),
  )) {
    const target = await safeRepositoryDestination(
      assertContained(resolvedRoot, path.resolve(resolvedRoot, relativePath)),
      `controlled generated file ${relativePath}`,
      FAILURE_CODES.parity,
    );
    try {
      if (await readFile(target, "utf8") !== source) mismatches.push(`${relativePath}: content differs`);
    } catch (error) {
      if (error?.code === "ENOENT") mismatches.push(`${relativePath}: missing`);
      else throw error;
    }
  }
  for (const relativePath of await listFiles(resolvedRoot)) {
    if (!expected.has(relativePath)) mismatches.push(`${relativePath}: unexpected generated file`);
  }
  if (checkOnly && mismatches.length > 0) {
    fail(FAILURE_CODES.parity, `Generated output is stale:\n${mismatches.join("\n")}`);
  }
  if (!checkOnly) {
    for (const relativePath of await listFiles(resolvedRoot)) {
      if (!expected.has(relativePath)) {
        const stalePath = await safeRepositoryDestination(
          assertContained(resolvedRoot, path.resolve(resolvedRoot, relativePath)),
          `stale generated file ${relativePath}`,
          FAILURE_CODES.parity,
        );
        await rm(stalePath, { force: true });
      }
    }
    for (const [relativePath, source] of expected) {
      const target = await safeRepositoryDestination(
        assertContained(resolvedRoot, path.resolve(resolvedRoot, relativePath)),
        `controlled generated file ${relativePath}`,
        FAILURE_CODES.parity,
      );
      await mkdir(path.dirname(target), { recursive: true });
      await writeFile(target, source, "utf8");
    }
  }
  return mismatches;
}

export async function readCommittedTree(root) {
  const resolvedRoot = await safeRepositoryDestination(
    root,
    "committed generated tree",
    FAILURE_CODES.parity,
  );
  const tree = new Map();
  for (const relativePath of await listFiles(resolvedRoot)) {
    const committedPath = await safeRepositoryDestination(
      path.join(resolvedRoot, relativePath),
      `committed generated file ${relativePath}`,
      FAILURE_CODES.parity,
    );
    tree.set(relativePath, await readFile(committedPath, "utf8"));
  }
  return tree;
}

export async function toolLockDigest() {
  const source = await readFile(await physicalRepositoryPath(
    toolLockPath,
    "tool-lock.json",
    FAILURE_CODES.lock,
  ), "utf8");
  return sha256(source);
}
