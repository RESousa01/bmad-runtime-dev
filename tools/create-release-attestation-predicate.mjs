import { createHash } from "node:crypto";
import { lstatSync, readFileSync, writeFileSync } from "node:fs";
import process from "node:process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function regularBytes(path) {
  const stat = lstatSync(path);
  if (!stat.isFile() || stat.isSymbolicLink()) {
    throw new Error(`attestation input must be a regular file: ${path}`);
  }
  return readFileSync(path);
}

function parseJson(bytes, label) {
  try {
    return JSON.parse(bytes.toString("utf8"));
  } catch {
    throw new Error(`${label} must contain valid JSON`);
  }
}

function assertExactKeys(value, keys, label) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} must be an object`);
  }
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`${label} has an unexpected or missing field`);
  }
}

function requireDigest(value, label) {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/u.test(value)) {
    throw new Error(`${label} must be an exact SHA-256 digest`);
  }
}

function requireText(value, label) {
  if (typeof value !== "string" || value.length === 0 || value.trim() !== value) {
    throw new Error(`${label} must be an exact non-empty string`);
  }
}

function validateEvidence(build, lifecycle, expectedRevision, expectedVersion, expectedPriorVersion) {
  assertExactKeys(build, ["schemaVersion", "sourceRevision", "sourceTreeState", "productVersion", "releaseMetadata", "toolchain", "sbom", "certificateThumbprint", "certificateNotAfterUtc", "timestampProtocol", "application", "installer"], "build evidence");
  if (build.schemaVersion !== 2 || build.sourceTreeState !== "clean" || build.sourceRevision !== expectedRevision || build.productVersion !== expectedVersion) {
    throw new Error("build evidence source or product identity is invalid");
  }
  assertExactKeys(build.releaseMetadata, ["schemaVersion", "product", "toolchain", "locks"], "release metadata");
  assertExactKeys(build.releaseMetadata.locks, ["cargo", "pnpm"], "release locks");
  for (const [name, expectedPath] of [["cargo", "Cargo.lock"], ["pnpm", "pnpm-lock.yaml"]]) {
    assertExactKeys(build.releaseMetadata.locks[name], ["path", "sha256"], `${name} lock`);
    if (build.releaseMetadata.locks[name].path !== expectedPath) throw new Error(`${name} lock path is invalid`);
    requireDigest(build.releaseMetadata.locks[name].sha256, `${name} lock`);
  }
  const metadataToolchain = build.releaseMetadata.toolchain;
  assertExactKeys(metadataToolchain, ["node", "pnpm", "rust", "typescript", "tauriCli"], "release toolchain");
  for (const [key, value] of Object.entries(metadataToolchain)) requireText(value, `release toolchain ${key}`);
  assertExactKeys(build.toolchain, ["node", "pnpm", "rustc", "tauriCli"], "observed toolchain");
  if (
    build.toolchain.node !== metadataToolchain.node ||
    build.toolchain.pnpm !== metadataToolchain.pnpm ||
    !build.toolchain.rustc.startsWith(`rustc ${metadataToolchain.rust} `) ||
    build.toolchain.tauriCli !== `tauri-cli ${metadataToolchain.tauriCli}`
  ) throw new Error("observed toolchain disagrees with release metadata");
  if (build.releaseMetadata.schemaVersion !== 1 || build.releaseMetadata.product?.version !== expectedVersion) {
    throw new Error("release metadata product identity is invalid");
  }
  assertExactKeys(build.releaseMetadata.product, ["name", "version", "identifier", "applicationName", "installerName", "sbomName"], "release product");
  requireText(build.releaseMetadata.product.installerName, "installer name");
  requireText(build.releaseMetadata.product.applicationName, "application name");
  requireText(build.releaseMetadata.product.sbomName, "SBOM name");
  requireText(build.certificateThumbprint, "publisher thumbprint");
  if (!/^[0-9A-Fa-f]{40}$/u.test(build.certificateThumbprint)) throw new Error("publisher thumbprint is invalid");
  requireText(build.certificateNotAfterUtc, "certificate expiry");
  if (!Number.isFinite(Date.parse(build.certificateNotAfterUtc)) || !["rfc3161", "authenticode"].includes(build.timestampProtocol)) throw new Error("build timestamp policy is invalid");
  assertExactKeys(build.application, ["sha256", "authenticodeStatus", "timestamperThumbprint"], "application evidence");
  assertExactKeys(build.installer, ["sha256", "authenticodeStatus", "timestamperThumbprint"], "installer evidence");
  assertExactKeys(build.sbom, ["fileName", "sha256", "format", "specVersion"], "SBOM evidence");
  if (build.sbom.fileName !== build.releaseMetadata.product.sbomName || build.sbom.format !== "CycloneDX" || build.sbom.specVersion !== "1.6") throw new Error("SBOM identity is invalid");
  if (build.application.authenticodeStatus !== "Valid" || build.installer.authenticodeStatus !== "Valid") throw new Error("build artifact signature is invalid");
  requireText(build.application.timestamperThumbprint, "application timestamp");
  requireText(build.installer.timestamperThumbprint, "installer timestamp");

  assertExactKeys(lifecycle, ["schemaVersion", "generatedAtUtc", "artifact", "priorArtifact", "lifecycle", "bundledFoundation"], "lifecycle evidence");
  if (lifecycle.schemaVersion !== 1 || lifecycle.priorArtifact === null) throw new Error("lifecycle evidence schema or prior artifact is invalid");
  const artifactKeys = ["fileName", "byteLength", "sha256", "expectedVersion", "authenticodeStatus", "signerThumbprint", "timestamperThumbprint"];
  assertExactKeys(lifecycle.artifact, artifactKeys, "current lifecycle artifact");
  assertExactKeys(lifecycle.priorArtifact, artifactKeys, "prior lifecycle artifact");
  assertExactKeys(lifecycle.lifecycle, ["freshInstall", "upgradedFromVersion", "installedVersion", "installedExecutableSha256", "installedExecutableAuthenticodeStatus", "installedExecutableSignerThumbprint", "installedExecutableTimestamperThumbprint", "launchSmoke", "uninstall", "installRootRemoved", "uninstallRegistrationRemoved", "residueScope"], "lifecycle result");
  assertExactKeys(lifecycle.bundledFoundation, ["exactFileCount", "missing", "unexpected", "hashMismatches"], "bundled foundation result");
  const publisher = build.certificateThumbprint.toUpperCase();
  if (
    lifecycle.artifact.expectedVersion !== expectedVersion ||
    lifecycle.priorArtifact.expectedVersion !== expectedPriorVersion ||
    lifecycle.lifecycle.installedVersion !== expectedVersion ||
    lifecycle.lifecycle.upgradedFromVersion !== expectedPriorVersion ||
    lifecycle.artifact.authenticodeStatus !== "Valid" ||
    lifecycle.priorArtifact.authenticodeStatus !== "Valid" ||
    lifecycle.lifecycle.installedExecutableAuthenticodeStatus !== "Valid" ||
    lifecycle.artifact.signerThumbprint?.toUpperCase() !== publisher ||
    lifecycle.priorArtifact.signerThumbprint?.toUpperCase() !== publisher ||
    lifecycle.lifecycle.installedExecutableSignerThumbprint?.toUpperCase() !== publisher
  ) throw new Error("lifecycle version, signature, or publisher identity is invalid");
  for (const value of [lifecycle.artifact.timestamperThumbprint, lifecycle.priorArtifact.timestamperThumbprint, lifecycle.lifecycle.installedExecutableTimestamperThumbprint]) requireText(value, "lifecycle timestamp");
  for (const gate of ["freshInstall", "launchSmoke", "uninstall", "installRootRemoved", "uninstallRegistrationRemoved"]) {
    if (lifecycle.lifecycle[gate] !== true) throw new Error(`lifecycle gate ${gate} did not pass`);
  }
  if (lifecycle.lifecycle.residueScope !== "install-root-and-uninstall-registration") throw new Error("lifecycle residue scope is invalid");
  if (!Number.isInteger(lifecycle.bundledFoundation.exactFileCount) || lifecycle.bundledFoundation.exactFileCount <= 0 || lifecycle.bundledFoundation.missing !== 0 || lifecycle.bundledFoundation.unexpected !== 0 || lifecycle.bundledFoundation.hashMismatches !== 0) {
    throw new Error("bundled foundation qualification did not pass");
  }
}

export function createReleaseAttestationPredicate({
  buildEvidenceBytes,
  lifecycleEvidenceBytes,
  installerBytes,
  applicationBytes,
  sbomBytes,
  expectedRevision,
  expectedVersion,
  expectedPriorVersion,
}) {
  const build = parseJson(buildEvidenceBytes, "build evidence");
  const lifecycle = parseJson(lifecycleEvidenceBytes, "lifecycle evidence");
  const installerSha256 = sha256(installerBytes);
  const applicationSha256 = sha256(applicationBytes);
  const sbomSha256 = sha256(sbomBytes);
  validateEvidence(build, lifecycle, expectedRevision, expectedVersion, expectedPriorVersion);
  if (
    build.installer?.sha256 !== installerSha256 ||
    build.application?.sha256 !== applicationSha256 ||
    build.sbom?.sha256 !== sbomSha256 ||
    lifecycle.artifact?.sha256 !== installerSha256 ||
    lifecycle.lifecycle?.installedExecutableSha256 !== applicationSha256
  ) {
    throw new Error("qualification evidence does not bind the exact attestation subjects");
  }
  if (build.releaseMetadata?.product?.version !== build.productVersion) {
    throw new Error("release metadata does not bind the evidenced product version");
  }
  return {
    schemaVersion: 1,
    sourceRevision: build.sourceRevision,
    sourceTreeState: build.sourceTreeState,
    productVersion: build.productVersion,
    releaseMetadata: build.releaseMetadata,
    observedToolchain: build.toolchain,
    subjects: {
      installer: { sha256: installerSha256 },
      application: { sha256: applicationSha256 },
      sbom: { sha256: sbomSha256 },
    },
    evidence: {
      build: { sha256: sha256(buildEvidenceBytes) },
      lifecycle: { sha256: sha256(lifecycleEvidenceBytes) },
    },
    qualification: lifecycle,
  };
}

function parseArguments(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith("--") || value === undefined || values.has(key)) {
      throw new Error(`invalid or duplicate argument: ${key ?? ""}`);
    }
    values.set(key, value);
  }
  const required = ["--build-evidence", "--lifecycle-evidence", "--installer", "--application", "--sbom", "--output", "--expected-revision", "--expected-version", "--expected-prior-version"];
  for (const key of required) {
    if (!values.has(key)) throw new Error(`${key} is required`);
  }
  return values;
}

function main(argv) {
  const values = parseArguments(argv);
  const output = resolve(values.get("--output"));
  const parent = lstatSync(dirname(output));
  if (!parent.isDirectory() || parent.isSymbolicLink()) {
    throw new Error("predicate output parent must be a regular directory");
  }
  const predicate = createReleaseAttestationPredicate({
    buildEvidenceBytes: regularBytes(resolve(values.get("--build-evidence"))),
    lifecycleEvidenceBytes: regularBytes(resolve(values.get("--lifecycle-evidence"))),
    installerBytes: regularBytes(resolve(values.get("--installer"))),
    applicationBytes: regularBytes(resolve(values.get("--application"))),
    sbomBytes: regularBytes(resolve(values.get("--sbom"))),
    expectedRevision: values.get("--expected-revision"),
    expectedVersion: values.get("--expected-version"),
    expectedPriorVersion: values.get("--expected-prior-version"),
  });
  writeFileSync(output, `${JSON.stringify(predicate, null, 2)}\n`, { encoding: "utf8", flag: "wx" });
  process.stdout.write(`${output}\n`);
}

const invokedPath = process.argv[1] === undefined ? "" : resolve(process.argv[1]);
if (invokedPath === fileURLToPath(import.meta.url)) main(process.argv.slice(2));
