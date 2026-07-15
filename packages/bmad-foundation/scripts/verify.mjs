import { createHash } from "node:crypto";
import { lstat, readFile, readdir, realpath } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { TextDecoder } from "node:util";
import { fileURLToPath, pathToFileURL } from "node:url";

const packageRoot = fileURLToPath(new URL("../", import.meta.url));
const lockedSemanticLedger = Object.freeze({
  byteLength: 48709,
  sha256: "574ab4d79a8f954c9743741cf9912f5283a255b88a80b07550ed379865d8cc4f",
});
const runtimePaths = Object.freeze([
  "runtime/builder/2.1.0/agent-analyze.instructions.md",
  "runtime/builder/2.1.0/agent-create-rebuild.instructions.md",
  "runtime/builder/2.1.0/agent-edit.instructions.md",
  "runtime/builder/2.1.0/workflow-analyze.instructions.md",
  "runtime/builder/2.1.0/workflow-build-edit.instructions.md",
  "runtime/method/6.10.0/architect-persona.instructions.md",
  "runtime/method/6.10.0/architecture-create.instructions.md",
  "runtime/method/6.10.0/bmad-help.instructions.md",
]);
const methodRuntimePaths = Object.freeze(
  runtimePaths.filter((relativePath) => relativePath.startsWith("runtime/method/")),
);
const normalizedPaths = Object.freeze([
  "normalized/bmad-architect.package.json",
  "normalized/bmad-architecture.package.json",
  "normalized/bmad-help.package.json",
  "normalized/bmm-agent-roster.json",
  "normalized/builder-agent.package.json",
  "normalized/builder-workflow.package.json",
]);
const runtimeResourcePaths = Object.freeze([
  "NOTICE.md",
  "adoption-ledger.json",
  "licenses/BMAD-BUILDER-MIT.txt",
  "licenses/BMAD-METHOD-MIT.txt",
  ...normalizedPaths,
  ...runtimePaths,
  "semantic-source-ledger.json",
].sort());
const managedOutputPaths = Object.freeze([
  "NOTICE.md",
  "adoption-ledger.json",
  "licenses/BMAD-BUILDER-MIT.txt",
  "licenses/BMAD-METHOD-MIT.txt",
  ...runtimePaths,
].sort());
const packageFiles = Object.freeze([
  "NOTICE.md",
  "README.md",
  "adoption-ledger.json",
  "licenses/BMAD-BUILDER-MIT.txt",
  "licenses/BMAD-METHOD-MIT.txt",
  ...normalizedPaths,
  "package.json",
  ...runtimePaths,
  "runtime-manifest.json",
  "scripts/verify.mjs",
  "semantic-source-ledger.json",
  "tests/foundation.test.mjs",
].sort());
const packageDirectories = Object.freeze([
  "licenses",
  "normalized",
  "runtime",
  "runtime/builder",
  "runtime/builder/2.1.0",
  "runtime/method",
  "runtime/method/6.10.0",
  "scripts",
  "tests",
].sort());
const packageDistributionFiles = Object.freeze([
  "adoption-ledger.json",
  "semantic-source-ledger.json",
  "NOTICE.md",
  "licenses",
  "normalized",
  "runtime",
  "runtime-manifest.json",
  "scripts",
  "tests",
  "README.md",
]);
const contextMarkers = Object.freeze([
  ["bmad", "runtime", "lib"].join("-"),
  ["", "source", "review"].join("_"),
]);
const sha256Pattern = /^[0-9a-f]{64}$/u;
const placeholderDigestPattern = /sha256:([0-9a-f])\1{63}/u;
const executableRuntimeName =
  /(?:\.(?:bat|cjs|cmd|dll|exe|js|mjs|ps1|py|ts)|(?:^|[-_.])(?:cleanup|eval|hook|install|render|setup|wake)(?:[-_.]|$))/iu;
const executableRuntimeContent =
  /(?:^#!|```\s*(?:bash|cmd|javascript|js|node|powershell|python|sh|typescript)|\b(?:child_process|npm\s+install|pnpm\s+install|python\s+-m|uv\s+run)\b)/imu;
const authorityProjectionPattern =
  /\b(?:activation|child_process|command|evaluation|network|promotion|registration|script)\b/iu;
const allowedTreatments = new Set(["adopt", "adapt", "defer", "reject"]);
const maxJsonBytes = 512 * 1024;
const maxJsonDepth = 64;

function fail(code, location, message) {
  throw new Error(`[${code}] ${location}: ${message}`);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function canonicalize(value) {
  if (value === null || typeof value === "boolean" || typeof value === "string") {
    return JSON.stringify(value);
  }
  if (typeof value === "number" && Number.isFinite(value)) {
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map(canonicalize).join(",")}]`;
  }
  if (typeof value === "object") {
    return `{${Object.keys(value).sort().map((key) =>
      `${JSON.stringify(key)}:${canonicalize(value[key])}`).join(",")}}`;
  }
  fail("foundation_hash_mismatch", "canonical-json", "non-JSON value in hash preimage");
}

function canonicalDocumentHash(purpose, value, excludedField) {
  const projected = Object.fromEntries(
    Object.entries(value).filter(([key]) => key !== excludedField),
  );
  return canonicalValueHash(purpose, projected);
}

function canonicalValueHash(purpose, value) {
  const preimage = `sapphirus:${purpose}:v1\n${canonicalize(value)}`;
  return `sha256:${sha256(preimage)}`;
}

function sameValues(actual, expected) {
  return Array.isArray(actual)
    && actual.length === expected.length
    && actual.every((value, index) => value === expected[index]);
}

function exactKeys(value, expected, location) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    fail("foundation_hash_mismatch", location, "expected an object");
  }
  const actual = Object.keys(value).sort();
  const locked = [...expected].sort();
  if (!sameValues(actual, locked)) {
    fail(
      "foundation_hash_mismatch",
      location,
      `expected keys ${JSON.stringify(locked)}, received ${JSON.stringify(actual)}`,
    );
  }
}

function requireRecoveryPath(record, field, code, location, message) {
  if (
    record === null
    || typeof record !== "object"
    || Array.isArray(record)
    || typeof record[field] !== "string"
    || record[field].trim() === ""
  ) {
    fail(code, location, message);
  }
  return record[field];
}

function normalizedPath(value) {
  return value.replaceAll("\\", "/");
}

function assertSafeRelativePath(value, location) {
  if (
    typeof value !== "string"
    || value.length === 0
    || /[\u0000-\u001f]/u.test(value)
  ) {
    fail("foundation_reference_escape", location, "path must be a non-empty string");
  }
  if (
    value.includes("\\")
    || value.includes(":")
    || value.startsWith("/")
    || value.startsWith("//")
  ) {
    fail("foundation_reference_escape", location, "absolute, drive, UNC, and alternate-stream paths are forbidden");
  }
  const segments = value.split("/");
  if (
    segments.some(
      (segment) =>
        segment === ""
        || segment === "."
        || segment === ".."
        || /[. ]$/u.test(segment)
        || /^(?:aux|con|nul|prn|com[1-9]|lpt[1-9])(?:\..*)?$/iu.test(segment),
    )
  ) {
    fail("foundation_reference_escape", location, "empty and traversal segments are forbidden");
  }
  return value;
}

async function readRegularBytes(relativePath) {
  assertSafeRelativePath(relativePath, relativePath);
  const absolutePath = path.join(packageRoot, ...relativePath.split("/"));
  let metadata;
  try {
    metadata = await lstat(absolutePath);
  } catch {
    fail("foundation_hash_mismatch", relativePath, "required file is missing or unreadable");
  }
  if (!metadata.isFile() || metadata.isSymbolicLink()) {
    fail("foundation_reference_escape", relativePath, "expected a regular file, not a link");
  }
  const resolvedRoot = await realpath(packageRoot);
  const resolvedPath = await realpath(absolutePath);
  const relativeResolved = path.relative(resolvedRoot, resolvedPath);
  if (relativeResolved.startsWith("..") || path.isAbsolute(relativeResolved)) {
    fail("foundation_reference_escape", relativePath, "resolved path escapes the package");
  }
  return await readFile(absolutePath);
}

function decodeText(bytes, location) {
  try {
    const source = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    if (source.includes("\0")) {
      fail("foundation_executable_content", location, "NUL bytes are forbidden");
    }
    return source;
  } catch (error) {
    if (error instanceof Error && error.message.startsWith("[foundation_")) throw error;
    fail("foundation_hash_mismatch", location, "text must be valid UTF-8");
  }
}

function parseJson(bytes, location) {
  if (bytes.byteLength > maxJsonBytes) {
    fail("foundation_hash_mismatch", location, "JSON exceeds the bounded verification size");
  }
  if (bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf) {
    fail("foundation_hash_mismatch", location, "UTF-8 BOM is forbidden");
  }
  const source = decodeText(bytes, location);
  let index = 0;

  function syntax(message) {
    fail("foundation_hash_mismatch", location, message);
  }

  function skipWhitespace() {
    while (
      index < source.length
      && [" ", "\t", "\r", "\n"].includes(source[index])
    ) {
      index += 1;
    }
  }

  function parseString() {
    if (source[index] !== '"') syntax("expected a JSON string");
    const start = index;
    index += 1;
    while (index < source.length) {
      const character = source[index];
      if (character === '"') {
        index += 1;
        let value;
        try {
          value = JSON.parse(source.slice(start, index));
        } catch {
          syntax("invalid JSON string escape");
        }
        for (let offset = 0; offset < value.length; offset += 1) {
          const codeUnit = value.charCodeAt(offset);
          if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
            const nextCodeUnit = value.charCodeAt(offset + 1);
            if (!(nextCodeUnit >= 0xdc00 && nextCodeUnit <= 0xdfff)) {
              syntax("JSON strings must contain well-formed Unicode");
            }
            offset += 1;
          } else if (codeUnit >= 0xdc00 && codeUnit <= 0xdfff) {
            syntax("JSON strings must contain well-formed Unicode");
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
    if (!source.startsWith(literal, index)) syntax(`expected ${literal}`);
    index += literal.length;
    return value;
  }

  function parseNumber() {
    const match = /^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?/u.exec(
      source.slice(index),
    );
    if (match === null) syntax("invalid JSON number");
    index += match[0].length;
    const value = Number(match[0]);
    if (!Number.isFinite(value)) syntax("JSON number exceeds the finite numeric range");
    if (Number.isInteger(value) && !Number.isSafeInteger(value)) {
      syntax("integral JSON numbers must be within the safe integer range");
    }
    return value;
  }

  function parseArray(depth) {
    const value = [];
    index += 1;
    skipWhitespace();
    if (source[index] === "]") {
      index += 1;
      return value;
    }
    while (index < source.length) {
      value.push(parseValue(depth + 1));
      skipWhitespace();
      if (source[index] === "]") {
        index += 1;
        return value;
      }
      if (source[index] !== ",") syntax("expected ',' or ']' in array");
      index += 1;
      skipWhitespace();
    }
    syntax("unterminated JSON array");
  }

  function parseObject(depth) {
    const value = Object.create(null);
    const keys = new Set();
    index += 1;
    skipWhitespace();
    if (source[index] === "}") {
      index += 1;
      return value;
    }
    while (index < source.length) {
      const key = parseString();
      if (keys.has(key)) {
        fail("foundation_hash_mismatch", location, "duplicate decoded object key");
      }
      keys.add(key);
      skipWhitespace();
      if (source[index] !== ":") syntax("expected ':' after object key");
      index += 1;
      value[key] = parseValue(depth + 1);
      skipWhitespace();
      if (source[index] === "}") {
        index += 1;
        return value;
      }
      if (source[index] !== ",") syntax("expected ',' or '}' in object");
      index += 1;
      skipWhitespace();
    }
    syntax("unterminated JSON object");
  }

  function parseValue(depth) {
    if (depth > maxJsonDepth) syntax("JSON exceeds the bounded nesting depth");
    skipWhitespace();
    const character = source[index];
    if (character === "{") return parseObject(depth);
    if (character === "[") return parseArray(depth);
    if (character === '"') return parseString();
    if (character === "t") return parseLiteral("true", true);
    if (character === "f") return parseLiteral("false", false);
    if (character === "n") return parseLiteral("null", null);
    if (character === "-" || (character >= "0" && character <= "9")) {
      return parseNumber();
    }
    syntax("expected a JSON value");
  }

  const value = parseValue(0);
  skipWhitespace();
  if (index !== source.length) syntax("unexpected trailing JSON content");
  return value;
}

function validateTreatments(treatments, location) {
  if (!Array.isArray(treatments) || treatments.length === 0) {
    fail("foundation_hash_mismatch", location, "at least one treatment is required");
  }
  for (const [index, treatment] of treatments.entries()) {
    exactKeys(treatment, ["decision", "rationale"], `${location}[${index}]`);
    if (!allowedTreatments.has(treatment.decision)) {
      fail("foundation_hash_mismatch", `${location}[${index}].decision`, "unknown treatment");
    }
    if (typeof treatment.rationale !== "string" || treatment.rationale.trim() === "") {
      fail("foundation_hash_mismatch", `${location}[${index}].rationale`, "rationale is required");
    }
  }
}

export function classifyProjectionFromSourceIdentity(sourceIdentity) {
  if (
    sourceIdentity?.sourceId === "method"
    && sourceIdentity.profile === "MethodOfficialSkillV6"
    && ["bmad-help", "bmad-agent-architect", "bmad-architecture"].includes(sourceIdentity.skill)
  ) {
    return "method";
  }
  if (
    sourceIdentity?.sourceId === "builder"
    && sourceIdentity.profile === "BuilderAgentV2Stateless"
    && sourceIdentity.skill === "bmad-agent-builder"
  ) {
    return "builder_agent";
  }
  if (
    sourceIdentity?.sourceId === "builder"
    && sourceIdentity.profile === "BuilderOutcomeSkillV2"
    && sourceIdentity.skill === "bmad-workflow-builder"
  ) {
    return "builder_workflow";
  }
  throw new Error("unknown BMAD source identity");
}

async function verifyTree() {
  const files = [];
  const directories = [];
  const resolvedRoot = await realpath(packageRoot);
  async function walk(directory) {
    const relativeDirectory = normalizedPath(path.relative(packageRoot, directory));
    const location = relativeDirectory || "package";
    const metadata = await lstat(directory);
    if (!metadata.isDirectory() || metadata.isSymbolicLink()) {
      fail("foundation_reference_escape", location, "linked package directories are forbidden");
    }
    const resolvedDirectory = await realpath(directory);
    const resolvedRelative = path.relative(resolvedRoot, resolvedDirectory);
    if (resolvedRelative.startsWith("..") || path.isAbsolute(resolvedRelative)) {
      fail("foundation_reference_escape", location, "resolved directory escapes the package");
    }
    if (relativeDirectory !== "") directories.push(relativeDirectory);
    const entries = await readdir(directory, { withFileTypes: true });
    for (const entry of entries) {
      const absolutePath = path.join(directory, entry.name);
      const relativePath = normalizedPath(path.relative(packageRoot, absolutePath));
      if (entry.isSymbolicLink()) {
        fail("foundation_reference_escape", relativePath, "linked package entries are forbidden");
      }
      if (entry.isDirectory()) await walk(absolutePath);
      else if (entry.isFile()) files.push(relativePath);
      else fail("foundation_reference_escape", relativePath, "unsupported package entry type");
    }
  }
  await walk(packageRoot);
  files.sort();
  directories.sort();

  for (const licensePath of [
    "licenses/BMAD-BUILDER-MIT.txt",
    "licenses/BMAD-METHOD-MIT.txt",
  ]) {
    if (!files.includes(licensePath)) {
      fail(
        "foundation_license_decision_missing",
        licensePath,
        "required reviewed license artifact is missing",
      );
    }
  }

  for (const relativePath of files) {
    const name = path.posix.basename(relativePath);
    const isExpected = packageFiles.includes(relativePath);
    if (!isExpected && executableRuntimeName.test(name)) {
      fail(
        "foundation_executable_content",
        relativePath,
        "unexpected executable or reserved package content is forbidden",
      );
    }
    const source = decodeText(await readRegularBytes(relativePath), relativePath);
    if (relativePath.startsWith("normalized/") && placeholderDigestPattern.test(source)) {
      fail(
        "foundation_hash_mismatch",
        relativePath,
        "placeholder digests are forbidden in normalized runtime data",
      );
    }
    if (relativePath.startsWith("runtime/") && executableRuntimeContent.test(source)) {
      fail(
        "foundation_executable_content",
        relativePath,
        "runtime instruction contains executable content",
      );
    }
    for (const marker of contextMarkers) {
      if (source.includes(marker)) {
        fail(
          "foundation_external_context_dependency",
          relativePath,
          "external context-library marker is forbidden",
        );
      }
    }
  }

  if (!sameValues(files, packageFiles)) {
    fail(
      "foundation_hash_mismatch",
      "package",
      `file allowlist mismatch: ${JSON.stringify(files)}`,
    );
  }
  if (!sameValues(directories, packageDirectories)) {
    fail(
      "foundation_reference_escape",
      "package",
      `directory allowlist mismatch: ${JSON.stringify(directories)}`,
    );
  }

}

function verifyManifest(manifest) {
  if (manifest === null || typeof manifest !== "object" || Array.isArray(manifest)) {
    fail("foundation_hash_mismatch", "package.json", "expected an object");
  }
  for (const dependencyField of [
    "dependencies",
    "devDependencies",
    "optionalDependencies",
    "peerDependencies",
  ]) {
    if (manifest[dependencyField] !== undefined) {
      fail("foundation_external_context_dependency", `package.json.${dependencyField}`, "dependencies are forbidden");
    }
  }
  if (Array.isArray(manifest.files)) {
    for (const [index, entry] of manifest.files.entries()) {
      assertSafeRelativePath(entry, `package.json.files[${index}]`);
    }
  }
  exactKeys(
    manifest,
    ["name", "version", "description", "private", "type", "files", "scripts", "engines"],
    "package.json",
  );
  if (
    manifest.name !== "@sapphirus/bmad-foundation"
    || manifest.version !== "0.1.0-beta.1"
    || manifest.private !== true
    || manifest.type !== "module"
  ) {
    fail("foundation_hash_mismatch", "package.json", "package identity drifted");
  }
  if (
    manifest.engines?.node !== "24.18.0"
    || manifest.engines?.pnpm !== "11.12.0"
  ) {
    fail("foundation_hash_mismatch", "package.json", "toolchain identity drifted");
  }
  const expectedScripts = {
    check: "node ./scripts/verify.mjs",
    lint: "node ./scripts/verify.mjs",
    test: "node --test --test-isolation=none ./tests/*.test.mjs",
    verify: "node ./scripts/verify.mjs && node --test --test-isolation=none ./tests/*.test.mjs",
  };
  if (JSON.stringify(manifest.scripts) !== JSON.stringify(expectedScripts)) {
    fail("foundation_hash_mismatch", "package.json.scripts", "verification scripts drifted");
  }
  if (!sameValues(manifest.files, packageDistributionFiles)) {
    fail("foundation_hash_mismatch", "package.json.files", "package distribution allowlist drifted");
  }
}

function preflightSemanticRecovery(semantic) {
  if (!Array.isArray(semantic?.sources) || semantic.sources.length !== 2) {
    fail("foundation_source_identity_incomplete", "sources", "exact source identity records are required");
  }
  if (!Array.isArray(semantic.sourceMembers)) {
    fail("foundation_source_identity_incomplete", "sourceMembers", "source member records are required");
  }
  if (!Array.isArray(semantic.identityEvidence)) {
    fail("foundation_source_identity_incomplete", "identityEvidence", "source identity evidence is required");
  }
  if (!Array.isArray(semantic.legalEvidence) || !Array.isArray(semantic.licenses)) {
    fail("foundation_license_decision_missing", "legalEvidence", "license and trademark evidence is required");
  }

  for (const [index, source] of semantic.sources.entries()) {
    const location = `sources[${index}]`;
    const artifactLabel = requireRecoveryPath(
      source,
      "archiveArtifactLabel",
      "foundation_source_identity_incomplete",
      location,
      "source identity record is malformed",
    );
    assertSafeRelativePath(artifactLabel, `${location}.archiveArtifactLabel`);
  }
  for (const [index, item] of semantic.identityEvidence.entries()) {
    const location = `identityEvidence[${index}]`;
    const member = requireRecoveryPath(
      item,
      "member",
      "foundation_source_identity_incomplete",
      location,
      "source identity evidence record is malformed",
    );
    assertSafeRelativePath(member, `${location}.member`);
  }
  for (const [index, item] of semantic.legalEvidence.entries()) {
    const location = `legalEvidence[${index}]`;
    const member = requireRecoveryPath(
      item,
      "member",
      "foundation_license_decision_missing",
      location,
      "license or trademark evidence record is malformed",
    );
    assertSafeRelativePath(member, `${location}.member`);
  }
  for (const [index, item] of semantic.licenses.entries()) {
    const location = `licenses[${index}]`;
    const licensePath = requireRecoveryPath(
      item,
      "path",
      "foundation_license_decision_missing",
      location,
      "license record is malformed",
    );
    assertSafeRelativePath(licensePath, `${location}.path`);
  }
  for (const [index, item] of semantic.sourceMembers.entries()) {
    const location = `sourceMembers[${index}]`;
    const member = requireRecoveryPath(
      item,
      "member",
      "foundation_source_identity_incomplete",
      location,
      "source member record is malformed",
    );
    assertSafeRelativePath(member, `${location}.member`);
  }
  if (Array.isArray(semantic.managedOutputs)) {
    for (const [index, item] of semantic.managedOutputs.entries()) {
      const location = `managedOutputs[${index}]`;
      const outputPath = requireRecoveryPath(
        item,
        "path",
        "foundation_hash_mismatch",
        location,
        "managed output record is malformed",
      );
      assertSafeRelativePath(outputPath, `${location}.path`);
    }
  }

  const expectedIdentityEvidence = [
    "builder:package.json:9072e665e63c05e5701387c4ff8d0ba1489518b59307a92a475686a019439ead",
    "method:package.json:7b4f67e25fb6ed90136d9e6214e9e67373c109bfa64b341a484ae6cc9dc09d83",
  ];
  const evidenceKey = (item) => `${item.sourceId}:${item.member}:${item.sha256}`;
  if (!sameValues(semantic.identityEvidence.map(evidenceKey), expectedIdentityEvidence)) {
    fail("foundation_source_identity_incomplete", "identityEvidence", "source identity evidence drifted");
  }
  for (const source of semantic.sources) {
    if (
      source.gitIdentity !== null
      || source.exception !== null
      || source.promotionEligibility !== "blocked_provenance"
      || !sameValues(source.missingImmutableIdentity, ["upstream commit or tag", "release signature"])
    ) {
      fail(
        "foundation_source_identity_incomplete",
        `sources.${source.id ?? "unknown"}`,
        "missing immutable identity must remain explicit and promotion-blocking",
      );
    }
  }

  const expectedLegalEvidence = [
    "builder:LICENSE:0aa79baf6328b4a1e694ce10a12ffc36d7666554da128dff0e8fcda0fc536a66",
    "builder:TRADEMARK.md:ce57ad749e43277c6021e5d5085980b33c9bf8f67a070bbbf07e041ccdddc58b",
    "method:LICENSE:0aa79baf6328b4a1e694ce10a12ffc36d7666554da128dff0e8fcda0fc536a66",
    "method:TRADEMARK.md:ce57ad749e43277c6021e5d5085980b33c9bf8f67a070bbbf07e041ccdddc58b",
  ];
  const expectedLicenses = [
    "builder:licenses/BMAD-BUILDER-MIT.txt:0aa79baf6328b4a1e694ce10a12ffc36d7666554da128dff0e8fcda0fc536a66",
    "method:licenses/BMAD-METHOD-MIT.txt:0aa79baf6328b4a1e694ce10a12ffc36d7666554da128dff0e8fcda0fc536a66",
  ];
  if (!sameValues(semantic.legalEvidence.map(evidenceKey), expectedLegalEvidence)) {
    fail("foundation_license_decision_missing", "legalEvidence", "license or trademark evidence drifted");
  }
  if (
    !sameValues(
      semantic.licenses.map((item) => `${item.sourceId}:${item.path}:${item.sha256}`),
      expectedLicenses,
    )
  ) {
    fail("foundation_license_decision_missing", "licenses", "exact license records are required");
  }
}

function preflightAdoptionRecovery(adoption) {
  if (!Array.isArray(adoption?.licenseDecisions)) {
    fail("foundation_license_decision_missing", "licenseDecisions", "license decisions are required");
  }
  for (const [index, item] of adoption.licenseDecisions.entries()) {
    const location = `licenseDecisions[${index}]`;
    const licensePath = requireRecoveryPath(
      item,
      "path",
      "foundation_license_decision_missing",
      location,
      "license decision record is malformed",
    );
    assertSafeRelativePath(licensePath, `${location}.path`);
  }
  if (Array.isArray(adoption.runtimeProjections)) {
    for (const [index, projection] of adoption.runtimeProjections.entries()) {
      const location = `runtimeProjections[${index}]`;
      const projectionPath = requireRecoveryPath(
        projection,
        "path",
        "foundation_hash_mismatch",
        location,
        "runtime projection record is malformed",
      );
      assertSafeRelativePath(projectionPath, `${location}.path`);
      if (authorityProjectionPattern.test(JSON.stringify(projection))) {
        fail(
          "foundation_executable_content",
          location,
          "authority-bearing projection field is forbidden",
        );
      }
    }
  }
  const expectedLicenseDecisions = [
    { sourceId: "builder", spdx: "MIT", path: "licenses/BMAD-BUILDER-MIT.txt", decision: "retain_exact_text" },
    { sourceId: "method", spdx: "MIT", path: "licenses/BMAD-METHOD-MIT.txt", decision: "retain_exact_text" },
  ];
  if (
    JSON.stringify(adoption.licenseDecisions) !== JSON.stringify(expectedLicenseDecisions)
    || adoption.trademarkDecision?.status !== "product_naming_not_approved"
    || typeof adoption.trademarkDecision?.rationale !== "string"
    || adoption.trademarkDecision.rationale.trim() === ""
  ) {
    fail("foundation_license_decision_missing", "licenseDecisions", "exact license and trademark decisions are required");
  }
  if (adoption.operationalAuthority !== "none") {
    fail("foundation_executable_content", "operationalAuthority", "operational authority is forbidden");
  }
}

function verifySourceFacts(semantic) {
  exactKeys(
    semantic,
    [
      "schemaVersion",
      "sources",
      "identityEvidence",
      "legalEvidence",
      "licenses",
      "sourceMembers",
      "managedOutputs",
    ],
    "semantic-source-ledger.json",
  );
  if (semantic.schemaVersion !== "sapphirus.bmad.semantic-source-ledger/v1") {
    fail("foundation_hash_mismatch", "semantic-source-ledger.json.schemaVersion", "unknown schema version");
  }
  const sources = new Map(semantic.sources.map((source) => [source.id, source]));
  const lockedSources = {
    builder: {
      packageName: "bmad-builder",
      packageVersion: "2.1.0",
      moduleVersion: "1.0.0",
      sourceFormatVersion: null,
      sourceFormatVersionEvidence: "not_declared",
      runtimeCompatibility: { node: ">=22.0.0" },
      archiveArtifactLabel: "bmad-builder-main.zip",
      archiveSha256: "d3c70744a9875623b01856cc907cf558324bacc920f0d860c36ad2788a4d2852",
      sourceDistributionProfile: "builder_source_tree",
    },
    method: {
      packageName: "bmad-method",
      packageVersion: "6.10.0",
      moduleVersion: null,
      sourceFormatVersion: null,
      sourceFormatVersionEvidence: "not_declared",
      runtimeCompatibility: { node: ">=20.12.0" },
      archiveArtifactLabel: "BMAD-METHOD-main.zip",
      archiveSha256: "a7c049038099b99081fbd03d22c6a5180edd88dee656bb37c4276b1cc31b4a32",
      sourceDistributionProfile: "method_source_tree",
    },
  };
  if (!sameValues([...sources.keys()].sort(), ["builder", "method"])) {
    fail("foundation_source_identity_incomplete", "semantic-source-ledger.json.sources", "exact sources are required");
  }
  for (const [id, locked] of Object.entries(lockedSources)) {
    const source = sources.get(id);
    for (const [field, value] of Object.entries(locked)) {
      if (JSON.stringify(source[field]) !== JSON.stringify(value)) {
        fail("foundation_source_identity_incomplete", `sources.${id}.${field}`, "source identity drifted");
      }
    }
    if (
      source.archiveEvidence !== "reviewed_snapshot_only"
      || source.gitIdentity !== null
      || source.exception !== null
      || source.promotionEligibility !== "blocked_provenance"
      || !Array.isArray(source.missingImmutableIdentity)
      || source.missingImmutableIdentity.length === 0
    ) {
      fail(
        "foundation_source_identity_incomplete",
        `sources.${id}`,
        "missing immutable identity must remain explicit and promotion-blocking",
      );
    }
  }

  const evidenceKey = (item) => `${item.sourceId}:${item.member}:${item.sha256}`;
  const lockedIdentityEvidence = [
    "builder:package.json:9072e665e63c05e5701387c4ff8d0ba1489518b59307a92a475686a019439ead",
    "method:package.json:7b4f67e25fb6ed90136d9e6214e9e67373c109bfa64b341a484ae6cc9dc09d83",
  ];
  const lockedLegalEvidence = [
    "builder:LICENSE:0aa79baf6328b4a1e694ce10a12ffc36d7666554da128dff0e8fcda0fc536a66",
    "builder:TRADEMARK.md:ce57ad749e43277c6021e5d5085980b33c9bf8f67a070bbbf07e041ccdddc58b",
    "method:LICENSE:0aa79baf6328b4a1e694ce10a12ffc36d7666554da128dff0e8fcda0fc536a66",
    "method:TRADEMARK.md:ce57ad749e43277c6021e5d5085980b33c9bf8f67a070bbbf07e041ccdddc58b",
  ];
  if (!sameValues(semantic.identityEvidence.map(evidenceKey), lockedIdentityEvidence)) {
    fail("foundation_source_identity_incomplete", "identityEvidence", "package identity evidence drifted");
  }
  if (!sameValues(semantic.legalEvidence.map(evidenceKey), lockedLegalEvidence)) {
    fail("foundation_license_decision_missing", "legalEvidence", "license or trademark evidence drifted");
  }

  const lockedLicenses = [
    "builder:licenses/BMAD-BUILDER-MIT.txt:0aa79baf6328b4a1e694ce10a12ffc36d7666554da128dff0e8fcda0fc536a66",
    "method:licenses/BMAD-METHOD-MIT.txt:0aa79baf6328b4a1e694ce10a12ffc36d7666554da128dff0e8fcda0fc536a66",
  ];
  if (
    !sameValues(
      semantic.licenses.map((item) => `${item.sourceId}:${item.path}:${item.sha256}`),
      lockedLicenses,
    )
  ) {
    fail("foundation_license_decision_missing", "licenses", "exact license artifacts are required");
  }

  if (!Array.isArray(semantic.sourceMembers) || semantic.sourceMembers.length !== 76) {
    fail("foundation_source_identity_incomplete", "sourceMembers", "exactly 76 reviewed members are required");
  }
  const ids = new Set();
  const identities = new Set();
  for (const member of semantic.sourceMembers) {
    exactKeys(member, ["id", "sourceId", "member", "sha256", "treatments"], `sourceMembers.${member?.id ?? "unknown"}`);
    if (ids.has(member.id)) fail("foundation_source_identity_incomplete", member.id, "duplicate member id");
    ids.add(member.id);
    const identity = `${member.sourceId}:${member.member}`;
    if (identities.has(identity)) {
      fail("foundation_source_identity_incomplete", identity, "duplicate source/member identity");
    }
    identities.add(identity);
    assertSafeRelativePath(member.member, `sourceMembers.${member.id}.member`);
    if (!sha256Pattern.test(member.sha256)) {
      fail("foundation_hash_mismatch", `sourceMembers.${member.id}.sha256`, "invalid digest");
    }
    validateTreatments(member.treatments, `sourceMembers.${member.id}.treatments`);
  }
  const expectedIds = [
    ...Array.from({ length: 29 }, (_, index) => `method-${String(index + 1).padStart(3, "0")}`),
    ...Array.from({ length: 47 }, (_, index) => `builder-${String(index + 1).padStart(3, "0")}`),
  ];
  if (!sameValues([...ids], expectedIds)) {
    fail("foundation_source_identity_incomplete", "sourceMembers", "source member sequence drifted");
  }
  return { ids, memberById: new Map(semantic.sourceMembers.map((member) => [member.id, member])) };
}

async function verifyManagedOutputs(semantic) {
  if (!Array.isArray(semantic.managedOutputs)) {
    fail("foundation_hash_mismatch", "managedOutputs", "managed output lock is required");
  }
  const outputs = [...semantic.managedOutputs].sort((left, right) =>
    left.path < right.path ? -1 : left.path > right.path ? 1 : 0,
  );
  if (!sameValues(outputs.map((output) => output.path), managedOutputPaths)) {
    fail("foundation_hash_mismatch", "managedOutputs", "managed output allowlist drifted");
  }
  for (const output of outputs) {
    exactKeys(output, ["path", "byteLength", "sha256"], `managedOutputs.${output.path}`);
    const bytes = await readRegularBytes(output.path);
    if (bytes.byteLength !== output.byteLength || sha256(bytes) !== output.sha256) {
      fail("foundation_hash_mismatch", output.path, "managed output bytes differ from the reviewed lock");
    }
  }
}

function verifyAdoption(adoption, semanticState) {
  if (
    adoption.schemaVersion !== "sapphirus.bmad.adoption-ledger/v1"
    || adoption.operationalAuthority !== "none"
    || adoption.promotionEligibility !== "blocked_provenance"
  ) {
    fail("foundation_hash_mismatch", "adoption-ledger.json", "authority or promotion posture drifted");
  }
  if (
    adoption.trademarkDecision?.status !== "product_naming_not_approved"
    || typeof adoption.trademarkDecision?.rationale !== "string"
    || adoption.trademarkDecision.rationale.trim() === ""
  ) {
    fail("foundation_license_decision_missing", "trademarkDecision", "separate product-naming decision is required");
  }
  if (!Array.isArray(adoption.licenseDecisions) || adoption.licenseDecisions.length !== 2) {
    fail("foundation_license_decision_missing", "licenseDecisions", "two license decisions are required");
  }

  const decisions = new Map(
    adoption.sourceDecisions.map((decision) => [decision.sourceMemberId, decision]),
  );
  if (!sameValues([...decisions.keys()].sort(), [...semanticState.ids].sort())) {
    fail("foundation_source_identity_incomplete", "sourceDecisions", "decision set does not close source members");
  }
  for (const [id, decision] of decisions) {
    validateTreatments(decision.treatments, `sourceDecisions.${id}.treatments`);
    if (
      JSON.stringify(decision.treatments)
      !== JSON.stringify(semanticState.memberById.get(id).treatments)
    ) {
      fail("foundation_hash_mismatch", `sourceDecisions.${id}`, "semantic and adoption treatments disagree");
    }
  }

  const expectedRoster = [
    ["bmad-agent-analyst", "Mary", "Business Analyst", "display_only_unavailable", ["method-004", "method-006", "method-007"]],
    ["bmad-agent-tech-writer", "Paige", "Technical Writer", "display_only_unavailable", ["method-004", "method-008", "method-009"]],
    ["bmad-agent-pm", "John", "Product Manager", "display_only_unavailable", ["method-004", "method-014", "method-015"]],
    ["bmad-agent-ux-designer", "Sally", "UX Designer", "display_only_unavailable", ["method-004", "method-016", "method-017"]],
    ["bmad-agent-architect", "Winston", "System Architect", "managed_projection_inactive", ["method-004", "method-018", "method-019"]],
    ["bmad-agent-dev", "Amelia", "Senior Software Engineer", "display_only_unavailable", ["method-004", "method-020", "method-021"]],
  ];
  if (!Array.isArray(adoption.methodRoster) || adoption.methodRoster.length !== expectedRoster.length) {
    fail("foundation_source_identity_incomplete", "methodRoster", "exact six-record roster is required");
  }
  for (const [index, expected] of expectedRoster.entries()) {
    const entry = adoption.methodRoster[index];
    if (!sameValues([entry.code, entry.name, entry.title, entry.state], expected.slice(0, 4))) {
      fail("foundation_source_identity_incomplete", `methodRoster[${index}]`, "roster identity drifted");
    }
    if (
      !Array.isArray(entry.sourceMemberIds)
      || !sameValues(entry.sourceMemberIds, expected[4])
      || entry.sourceMemberIds.some((id) => !semanticState.ids.has(id))
    ) {
      fail("foundation_source_identity_incomplete", `methodRoster[${index}]`, "roster references are not closed");
    }
  }

  const expectedPrompts = [
    ["WD", "method-010"],
    ["MG", "method-011"],
    ["VD", "method-012"],
    ["EC", "method-013"],
  ];
  if (!Array.isArray(adoption.promptReferenceBindings) || adoption.promptReferenceBindings.length !== 4) {
    fail("foundation_source_identity_incomplete", "promptReferenceBindings", "Paige reference closure is incomplete");
  }
  for (const [index, expected] of expectedPrompts.entries()) {
    const binding = adoption.promptReferenceBindings[index];
    if (
      binding.agentId !== "bmad-agent-tech-writer"
      || binding.menuCode !== expected[0]
      || binding.sourceMemberId !== expected[1]
      || binding.availability !== "unavailable_reference_only"
    ) {
      fail("foundation_source_identity_incomplete", `promptReferenceBindings[${index}]`, "prompt binding drifted");
    }
  }

  const expectedProjections = new Map([
    ["runtime/method/6.10.0/bmad-help.instructions.md", {
      classification: "method",
      state: "sealed_read_only",
      skill: "bmad-help",
      profile: "MethodOfficialSkillV6",
      actions: [],
      action: null,
      entrypointKind: "direct",
      sourceMemberIds: ["method-001", "method-002", "method-003", "method-004", "method-005"],
    }],
    ["runtime/method/6.10.0/architect-persona.instructions.md", {
      classification: "method",
      state: "sealed_read_only",
      skill: "bmad-agent-architect",
      profile: "MethodOfficialSkillV6",
      actions: [],
      action: null,
      entrypointKind: null,
      sourceMemberIds: ["method-004", "method-018", "method-019"],
    }],
    ["runtime/method/6.10.0/architecture-create.instructions.md", {
      classification: "method",
      state: "sealed_read_only",
      skill: "bmad-architecture",
      profile: "MethodOfficialSkillV6",
      actions: [],
      action: "create",
      entrypointKind: "step_jit",
      sourceMemberIds: [
        "method-018", "method-019", "method-022", "method-023", "method-024",
        "method-025", "method-026", "method-027", "method-028", "method-029",
      ],
    }],
    ["runtime/builder/2.1.0/agent-create-rebuild.instructions.md", {
      classification: "builder_agent",
      state: "inactive_data",
      skill: "bmad-agent-builder",
      profile: "BuilderAgentV2Stateless",
      actions: ["create_rebuild"],
      entrypointKind: "direct",
      sourceMemberIds: [
        "builder-003", "builder-004", "builder-005", "builder-008", "builder-009",
        "builder-010", "builder-011", "builder-013", "builder-014", "builder-015",
        "builder-016", "builder-038", "builder-039", "builder-040", "builder-041",
        "builder-042", "builder-043",
      ],
    }],
    ["runtime/builder/2.1.0/agent-edit.instructions.md", {
      classification: "builder_agent",
      state: "inactive_data",
      skill: "bmad-agent-builder",
      profile: "BuilderAgentV2Stateless",
      actions: ["edit"],
      entrypointKind: "direct",
      sourceMemberIds: [
        "builder-003", "builder-004", "builder-006", "builder-008", "builder-009",
        "builder-010", "builder-011", "builder-013", "builder-014", "builder-016",
      ],
    }],
    ["runtime/builder/2.1.0/agent-analyze.instructions.md", {
      classification: "builder_agent",
      state: "inactive_data",
      skill: "bmad-agent-builder",
      profile: "BuilderAgentV2Stateless",
      actions: ["analyze"],
      entrypointKind: "direct",
      sourceMemberIds: [
        "builder-003", "builder-004", "builder-007", "builder-009", "builder-011",
        "builder-012", "builder-027", "builder-028", "builder-029", "builder-030",
        "builder-031", "builder-032", "builder-040", "builder-041",
      ],
    }],
    ["runtime/builder/2.1.0/workflow-build-edit.instructions.md", {
      classification: "builder_workflow",
      state: "inactive_data",
      skill: "bmad-workflow-builder",
      profile: "BuilderOutcomeSkillV2",
      actions: ["build", "edit"],
      entrypointKind: "inline",
      sourceMemberIds: [
        "builder-017", "builder-018", "builder-019", "builder-021", "builder-022",
        "builder-023", "builder-025", "builder-026", "builder-044", "builder-045",
        "builder-046",
      ],
    }],
    ["runtime/builder/2.1.0/workflow-analyze.instructions.md", {
      classification: "builder_workflow",
      state: "inactive_data",
      skill: "bmad-workflow-builder",
      profile: "BuilderOutcomeSkillV2",
      actions: ["analyze"],
      entrypointKind: "inline",
      sourceMemberIds: [
        "builder-017", "builder-018", "builder-020", "builder-021", "builder-023",
        "builder-024", "builder-033", "builder-034", "builder-035", "builder-036",
        "builder-037", "builder-046", "builder-047",
      ],
    }],
  ]);
  if (!Array.isArray(adoption.runtimeProjections) || adoption.runtimeProjections.length !== expectedProjections.size) {
    fail("foundation_hash_mismatch", "runtimeProjections", "exact projection set is required");
  }
  for (const projection of adoption.runtimeProjections) {
    const expected = expectedProjections.get(projection.path);
    if (!expected) fail("foundation_hash_mismatch", projection.path, "unexpected runtime projection");
    const classification = classifyProjectionFromSourceIdentity(projection.sourceIdentity);
    if (
      classification !== expected.classification
      || projection.classification !== expected.classification
      || projection.state !== expected.state
      || projection.authority !== "none"
      || projection.distributionProfile !== "sapphirus_package"
      || projection.installProfile !== "SapphirusManagedV1"
      || projection.sourceIdentity.skill !== expected.skill
      || projection.sourceIdentity.profile !== expected.profile
      || projection.validationProfile !== expected.profile
      || projection.entrypointKind !== expected.entrypointKind
      || !sameValues(projection.actions, expected.actions)
      || (Object.hasOwn(expected, "action") && projection.action !== expected.action)
    ) {
      fail("foundation_hash_mismatch", projection.path, "projection semantics drifted");
    }
    if (
      !sameValues(projection.sourceMemberIds, expected.sourceMemberIds)
      || projection.sourceMemberIds.some((id) => !semanticState.ids.has(id))
    ) {
      fail("foundation_source_identity_incomplete", projection.path, "projection references are not closed");
    }
    if (authorityProjectionPattern.test(JSON.stringify(projection))) {
      fail("foundation_executable_content", projection.path, "authority-bearing projection field is forbidden");
    }
  }
}

async function verifyRuntime() {
  for (const relativePath of runtimePaths) {
    const name = path.posix.basename(relativePath);
    if (executableRuntimeName.test(name)) {
      fail("foundation_executable_content", relativePath, "runtime filename is executable or reserved");
    }
    const source = decodeText(await readRegularBytes(relativePath), relativePath);
    if (executableRuntimeContent.test(source)) {
      fail("foundation_executable_content", relativePath, "runtime instruction contains executable content");
    }
  }
}

async function verifyRuntimeManifest(semanticBytes) {
  const manifest = parseJson(
    await readRegularBytes("runtime-manifest.json"),
    "runtime-manifest.json",
  );
  exactKeys(
    manifest,
    ["schemaVersion", "foundationVersion", "semanticLedgerHash", "resources", "manifestHash"],
    "runtime-manifest.json",
  );
  if (
    manifest.schemaVersion !== "sapphirus.bmad-foundation-runtime-manifest.v1"
    || manifest.foundationVersion !== "0.1.0-beta.1"
    || manifest.semanticLedgerHash !== `sha256:${sha256(semanticBytes)}`
    || manifest.manifestHash
      !== canonicalDocumentHash(
        "bmad-foundation-runtime-manifest",
        manifest,
        "manifestHash",
      )
  ) {
    fail("foundation_hash_mismatch", "runtime-manifest.json", "runtime manifest identity or hash drifted");
  }
  if (!Array.isArray(manifest.resources)) {
    fail("foundation_hash_mismatch", "runtime-manifest.json.resources", "resource inventory is required");
  }
  const resources = new Map();
  let previousPath = "";
  for (const [index, resource] of manifest.resources.entries()) {
    const location = `runtime-manifest.json.resources[${index}]`;
    exactKeys(resource, ["path", "contentHash", "byteLength", "contentKind"], location);
    assertSafeRelativePath(resource.path, `${location}.path`);
    if (
      resource.path <= previousPath
      || resources.has(resource.path)
      || !resource.contentHash.startsWith("sha256:")
      || !sha256Pattern.test(resource.contentHash.slice(7))
      || !Number.isSafeInteger(resource.byteLength)
      || resource.byteLength < 0
      || ![
        "legal_notice",
        "managed_instruction",
        "normalized_contract",
        "provenance_ledger",
      ].includes(resource.contentKind)
    ) {
      fail("foundation_hash_mismatch", location, "resource inventory is non-canonical");
    }
    previousPath = resource.path;
    const bytes = await readRegularBytes(resource.path);
    if (
      bytes.byteLength !== resource.byteLength
      || `sha256:${sha256(bytes)}` !== resource.contentHash
    ) {
      fail("foundation_hash_mismatch", resource.path, "runtime resource bytes drifted");
    }
    resources.set(resource.path, resource);
  }
  if (!sameValues([...resources.keys()], runtimeResourcePaths)) {
    fail("foundation_hash_mismatch", "runtime-manifest.json.resources", "runtime resource allowlist drifted");
  }
  return resources;
}

function verifyProjectionEnvelope(envelope, resources, expectedPath, expectedSkill, expectedAction) {
  exactKeys(
    envelope,
    [
      "schemaVersion",
      "packageVersionId",
      "lifecycleState",
      "capability",
      "instructionProjection",
      "projectionEnvelopeHash",
    ],
    expectedPath,
  );
  const projection = envelope.instructionProjection;
  if (
    envelope.schemaVersion !== "sapphirus.bmad-foundation-method-projection.v1"
    || envelope.lifecycleState !== "sealed_read_only"
    || envelope.capability?.moduleCode !== "bmm"
    || envelope.capability?.skillName !== expectedSkill
    || envelope.capability?.normalizedAction !== expectedAction
    || projection?.managedInstruction?.path !== expectedPath
    || projection.managedInstruction.contentHash !== resources.get(expectedPath)?.contentHash
    || projection.projectionHash
      !== canonicalDocumentHash("bmad-instruction-projection", projection, "projectionHash")
    || envelope.projectionEnvelopeHash
      !== canonicalDocumentHash(
        "bmad-foundation-method-projection",
        envelope,
        "projectionEnvelopeHash",
      )
  ) {
    fail("foundation_hash_mismatch", expectedPath, "Method projection envelope drifted");
  }
}

function verifyBuilderPackage(value, resources, expectedKind, expectedProfile, expectedPaths) {
  exactKeys(
    value,
    [
      "schemaVersion",
      "packageName",
      "packageVersion",
      "authoringKind",
      "lifecycleState",
      "activationAuthority",
      "validationProfile",
      "resources",
      "packageHash",
    ],
    `normalized/${expectedKind}.package.json`,
  );
  if (
    value.schemaVersion !== "sapphirus.bmad-foundation-builder-package.v1"
    || value.packageName !== "bmad-builder"
    || value.packageVersion !== "2.1.0"
    || value.authoringKind !== expectedKind
    || value.lifecycleState !== "inactive_data"
    || value.activationAuthority !== "none"
    || value.validationProfile !== expectedProfile
    || value.packageHash
      !== canonicalDocumentHash("bmad-foundation-builder-package", value, "packageHash")
    || !Array.isArray(value.resources)
    || !sameValues(value.resources.map((resource) => resource.path), expectedPaths)
  ) {
    fail("foundation_hash_mismatch", `normalized/${expectedKind}.package.json`, "Builder package drifted");
  }
  for (const resource of value.resources) {
    if (
      resource.contentHash !== resources.get(resource.path)?.contentHash
      || resource.byteLength !== resources.get(resource.path)?.byteLength
      || !Array.isArray(resource.actions)
      || !Array.isArray(resource.sourceMemberIds)
      || resource.sourceMemberIds.length === 0
    ) {
      fail("foundation_hash_mismatch", resource.path, "Builder resource binding drifted");
    }
  }
}

async function verifyNormalizedArtifacts(resources) {
  const descriptor = parseJson(
    await readRegularBytes("normalized/bmad-help.package.json"),
    "normalized/bmad-help.package.json",
  );
  if (
    descriptor.schemaVersion !== "sapphirus.bmad-package-descriptor.v1"
    || descriptor.packageName !== "bmad-method"
    || descriptor.packageVersion !== "6.10.0"
    || descriptor.installProfile !== "SapphirusManagedV1"
    || descriptor.descriptorHash
      !== canonicalDocumentHash("bmad-package-descriptor", descriptor, "descriptorHash")
  ) {
    fail("foundation_hash_mismatch", "normalized/bmad-help.package.json", "Method descriptor drifted");
  }
  const managedInventory = descriptor.resourceInventory.filter(({ locationKind }) =>
    locationKind === "managed_projection");
  if (!sameValues(managedInventory.map(({ path: resourcePath }) => resourcePath), methodRuntimePaths)) {
    fail("foundation_hash_mismatch", "normalized/bmad-help.package.json.resourceInventory", "managed resource set drifted");
  }
  for (const entry of managedInventory) {
    if (
      entry.contentHash !== resources.get(entry.path)?.contentHash
      || entry.byteLength !== resources.get(entry.path)?.byteLength
    ) {
      fail("foundation_hash_mismatch", entry.path, "managed resource binding drifted");
    }
  }
  const finalInventory = methodRuntimePaths.map((resourcePath) => ({
    path: resourcePath,
    locationKind: "managed_projection",
    contentHash: resources.get(resourcePath).contentHash,
    byteLength: resources.get(resourcePath).byteLength,
  }));
  if (
    descriptor.finalCompositeInventoryHash
    !== canonicalValueHash("bmad-final-composite-inventory", finalInventory)
    || !sameValues(
      descriptor.skills.map(({ moduleCode, skillName }) => `${moduleCode}:${skillName}`),
      ["bmm:bmad-architecture", "core:bmad-help"],
    )
  ) {
    fail("foundation_hash_mismatch", "normalized/bmad-help.package.json", "Method capability inventory drifted");
  }
  for (const projection of descriptor.instructionProjections) {
    if (
      projection.projectionHash
      !== canonicalDocumentHash("bmad-instruction-projection", projection, "projectionHash")
    ) {
      fail("foundation_hash_mismatch", projection.managedInstruction.path, "instruction projection hash drifted");
    }
  }

  const roster = parseJson(
    await readRegularBytes("normalized/bmm-agent-roster.json"),
    "normalized/bmm-agent-roster.json",
  );
  if (
    !Array.isArray(roster.agents)
    || !sameValues(
      roster.agents.map(({ agentCode }) => agentCode),
      [
        "bmad-agent-analyst",
        "bmad-agent-architect",
        "bmad-agent-dev",
        "bmad-agent-pm",
        "bmad-agent-tech-writer",
        "bmad-agent-ux-designer",
      ],
    )
  ) {
    fail("foundation_hash_mismatch", "normalized/bmm-agent-roster.json", "exact Method roster drifted");
  }

  verifyProjectionEnvelope(
    parseJson(
      await readRegularBytes("normalized/bmad-architect.package.json"),
      "normalized/bmad-architect.package.json",
    ),
    resources,
    "runtime/method/6.10.0/architect-persona.instructions.md",
    "bmad-agent-architect",
    null,
  );
  verifyProjectionEnvelope(
    parseJson(
      await readRegularBytes("normalized/bmad-architecture.package.json"),
      "normalized/bmad-architecture.package.json",
    ),
    resources,
    "runtime/method/6.10.0/architecture-create.instructions.md",
    "bmad-architecture",
    "create",
  );
  verifyBuilderPackage(
    parseJson(
      await readRegularBytes("normalized/builder-agent.package.json"),
      "normalized/builder-agent.package.json",
    ),
    resources,
    "stateless_agent",
    "BuilderAgentV2Stateless",
    [
      "runtime/builder/2.1.0/agent-analyze.instructions.md",
      "runtime/builder/2.1.0/agent-create-rebuild.instructions.md",
      "runtime/builder/2.1.0/agent-edit.instructions.md",
    ],
  );
  verifyBuilderPackage(
    parseJson(
      await readRegularBytes("normalized/builder-workflow.package.json"),
      "normalized/builder-workflow.package.json",
    ),
    resources,
    "simple_inline_workflow",
    "BuilderOutcomeSkillV2",
    [
      "runtime/builder/2.1.0/workflow-analyze.instructions.md",
      "runtime/builder/2.1.0/workflow-build-edit.instructions.md",
    ],
  );
}

export async function verifyFoundation() {
  await verifyTree();
  verifyManifest(parseJson(await readRegularBytes("package.json"), "package.json"));

  const semanticBytes = await readRegularBytes("semantic-source-ledger.json");
  const semantic = parseJson(semanticBytes, "semantic-source-ledger.json");
  preflightSemanticRecovery(semantic);
  if (
    semanticBytes.byteLength !== lockedSemanticLedger.byteLength
    || sha256(semanticBytes) !== lockedSemanticLedger.sha256
  ) {
    fail(
      "foundation_hash_mismatch",
      "semantic-source-ledger.json",
      "reviewed semantic lock changed; automatic lock updates are forbidden",
    );
  }
  const semanticState = verifySourceFacts(semantic);
  const adoption = parseJson(
    await readRegularBytes("adoption-ledger.json"),
    "adoption-ledger.json",
  );
  preflightAdoptionRecovery(adoption);
  const runtimeResources = await verifyRuntimeManifest(semanticBytes);
  await verifyManagedOutputs(semantic);
  verifyAdoption(adoption, semanticState);
  await verifyRuntime();
  await verifyNormalizedArtifacts(runtimeResources);
  return {
    sourceMemberCount: semantic.sourceMembers.length,
    managedOutputCount: semantic.managedOutputs.length,
    semanticLedgerSha256: lockedSemanticLedger.sha256,
  };
}

function isMainModule() {
  if (!process.argv[1]) return false;
  return import.meta.url.split("?")[0] === pathToFileURL(path.resolve(process.argv[1])).href;
}

if (isMainModule()) {
  try {
    const result = await verifyFoundation();
    console.log(
      `bmad-foundation: verified ${result.sourceMemberCount} source members, `
      + `${result.managedOutputCount} managed outputs, sha256:${result.semanticLedgerSha256}`,
    );
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
