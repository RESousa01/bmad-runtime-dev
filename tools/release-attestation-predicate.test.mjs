import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";

import { createReleaseAttestationPredicate } from "./create-release-attestation-predicate.mjs";

const digest = (bytes) => createHash("sha256").update(bytes).digest("hex");
const installerBytes = Buffer.from("installer");
const applicationBytes = Buffer.from("application");
const sbomBytes = Buffer.from("sbom");
const build = {
  schemaVersion: 2,
  sourceRevision: "ab".repeat(20),
  sourceTreeState: "clean",
  productVersion: "1.2.3",
  releaseMetadata: {
    schemaVersion: 1,
    product: { name: "Sapphirus", version: "1.2.3", identifier: "dev.sapphirus", applicationName: "sapphirus.exe", installerName: "Sapphirus_1.2.3_x64-setup.exe", sbomName: "Sapphirus_1.2.3.cdx.json" },
    toolchain: { node: "24.18.0", pnpm: "11.12.0", rust: "1.97.0", typescript: "7.0.2", tauriCli: "2.11.4" },
    locks: { cargo: { path: "Cargo.lock", sha256: "11".repeat(32) }, pnpm: { path: "pnpm-lock.yaml", sha256: "22".repeat(32) } },
  },
  toolchain: { node: "24.18.0", pnpm: "11.12.0", rustc: "rustc 1.97.0 (test 2026-01-01)", tauriCli: "tauri-cli 2.11.4" },
  sbom: { fileName: "Sapphirus_1.2.3.cdx.json", sha256: digest(sbomBytes), format: "CycloneDX", specVersion: "1.6" },
  certificateThumbprint: "AB".repeat(20),
  certificateNotAfterUtc: "2027-01-01T00:00:00.000Z",
  timestampProtocol: "rfc3161",
  installer: { sha256: digest(installerBytes), authenticodeStatus: "Valid", timestamperThumbprint: "CD".repeat(20) },
  application: { sha256: digest(applicationBytes), authenticodeStatus: "Valid", timestamperThumbprint: "CD".repeat(20) },
};
const lifecycle = {
  schemaVersion: 1,
  generatedAtUtc: "2026-07-18T00:00:00.000Z",
  artifact: { fileName: "Sapphirus_1.2.3_x64-setup.exe", byteLength: installerBytes.length, sha256: digest(installerBytes), expectedVersion: "1.2.3", authenticodeStatus: "Valid", signerThumbprint: "AB".repeat(20), timestamperThumbprint: "CD".repeat(20) },
  priorArtifact: { fileName: "Sapphirus_1.2.2_x64-setup.exe", byteLength: 1, sha256: "33".repeat(32), expectedVersion: "1.2.2", authenticodeStatus: "Valid", signerThumbprint: "AB".repeat(20), timestamperThumbprint: "CD".repeat(20) },
  lifecycle: { freshInstall: true, upgradedFromVersion: "1.2.2", installedVersion: "1.2.3", installedExecutableSha256: digest(applicationBytes), installedExecutableAuthenticodeStatus: "Valid", installedExecutableSignerThumbprint: "AB".repeat(20), installedExecutableTimestamperThumbprint: "CD".repeat(20), launchSmoke: true, uninstall: true, installRootRemoved: true, uninstallRegistrationRemoved: true, residueScope: "install-root-and-uninstall-registration" },
  bundledFoundation: { exactFileCount: 10, missing: 0, unexpected: 0, hashMismatches: 0 },
};
const bytes = (value) => Buffer.from(JSON.stringify(value));
const expected = { expectedRevision: "ab".repeat(20), expectedVersion: "1.2.3", expectedPriorVersion: "1.2.2" };

test("predicate binds exact subjects, release identities, and qualification evidence", () => {
  const predicate = createReleaseAttestationPredicate({
    buildEvidenceBytes: bytes(build), lifecycleEvidenceBytes: bytes(lifecycle), installerBytes, applicationBytes, sbomBytes, ...expected,
  });
  assert.deepEqual(predicate.releaseMetadata.locks, build.releaseMetadata.locks);
  assert.deepEqual(predicate.observedToolchain, build.toolchain);
  assert.equal(predicate.subjects.installer.sha256, digest(installerBytes));
  assert.equal(predicate.evidence.build.sha256, digest(bytes(build)));
  assert.equal(predicate.evidence.lifecycle.sha256, digest(bytes(lifecycle)));
});

for (const [label, mutation] of [
  ["installer", { installerBytes: Buffer.from("substituted installer") }],
  ["application", { applicationBytes: Buffer.from("substituted application") }],
  ["SBOM", { sbomBytes: Buffer.from("substituted sbom") }],
  ["lifecycle", { lifecycleEvidenceBytes: bytes({ ...lifecycle, artifact: { sha256: "00".repeat(32) } }) }],
]) {
  test(`predicate rejects substituted ${label} evidence`, () => {
    assert.throws(() => createReleaseAttestationPredicate({
      buildEvidenceBytes: bytes(build), lifecycleEvidenceBytes: bytes(lifecycle), installerBytes, applicationBytes, sbomBytes, ...expected, ...mutation,
    }), /does not bind|missing field/u);
  });
}

for (const gate of ["freshInstall", "launchSmoke", "uninstall", "installRootRemoved", "uninstallRegistrationRemoved"]) {
  test(`predicate rejects false lifecycle gate ${gate}`, () => {
    const altered = structuredClone(lifecycle);
    altered.lifecycle[gate] = false;
    assert.throws(() => createReleaseAttestationPredicate({ buildEvidenceBytes: bytes(build), lifecycleEvidenceBytes: bytes(altered), installerBytes, applicationBytes, sbomBytes, ...expected }), /lifecycle gate/u);
  });
}

test("predicate rejects source revision, lock, publisher, and foundation drift", () => {
  assert.throws(() => createReleaseAttestationPredicate({ buildEvidenceBytes: bytes(build), lifecycleEvidenceBytes: bytes(lifecycle), installerBytes, applicationBytes, sbomBytes, ...expected, expectedRevision: "cd".repeat(20) }), /source or product/u);
  const missingLock = structuredClone(build); delete missingLock.releaseMetadata.locks.pnpm;
  assert.throws(() => createReleaseAttestationPredicate({ buildEvidenceBytes: bytes(missingLock), lifecycleEvidenceBytes: bytes(lifecycle), installerBytes, applicationBytes, sbomBytes, ...expected }), /missing field/u);
  const wrongPublisher = structuredClone(lifecycle); wrongPublisher.lifecycle.installedExecutableSignerThumbprint = "EF".repeat(20);
  assert.throws(() => createReleaseAttestationPredicate({ buildEvidenceBytes: bytes(build), lifecycleEvidenceBytes: bytes(wrongPublisher), installerBytes, applicationBytes, sbomBytes, ...expected }), /publisher/u);
  const foundationMismatch = structuredClone(lifecycle); foundationMismatch.bundledFoundation.hashMismatches = 1;
  assert.throws(() => createReleaseAttestationPredicate({ buildEvidenceBytes: bytes(build), lifecycleEvidenceBytes: bytes(foundationMismatch), installerBytes, applicationBytes, sbomBytes, ...expected }), /foundation/u);
});

test("predicate rejects incomplete toolchain, lifecycle schema, version, signature, and timestamp evidence", () => {
  const cases = [];
  const missingToolchain = structuredClone(build); delete missingToolchain.toolchain.pnpm;
  cases.push([missingToolchain, lifecycle]);
  const wrongSchema = structuredClone(lifecycle); wrongSchema.schemaVersion = 2;
  cases.push([build, wrongSchema]);
  const wrongVersion = structuredClone(lifecycle); wrongVersion.lifecycle.installedVersion = "1.2.4";
  cases.push([build, wrongVersion]);
  const invalidSignature = structuredClone(lifecycle); invalidSignature.artifact.authenticodeStatus = "NotSigned";
  cases.push([build, invalidSignature]);
  const missingTimestamp = structuredClone(lifecycle); missingTimestamp.priorArtifact.timestamperThumbprint = null;
  cases.push([build, missingTimestamp]);
  for (const [alteredBuild, alteredLifecycle] of cases) {
    assert.throws(() => createReleaseAttestationPredicate({ buildEvidenceBytes: bytes(alteredBuild), lifecycleEvidenceBytes: bytes(alteredLifecycle), installerBytes, applicationBytes, sbomBytes, ...expected }));
  }
});

test("predicate treats quote and newline prior-version probes as data and rejects them", () => {
  for (const expectedPriorVersion of ["1.2.2' && echo injected", "1.2.2\necho injected"]) {
    assert.throws(() => createReleaseAttestationPredicate({ buildEvidenceBytes: bytes(build), lifecycleEvidenceBytes: bytes(lifecycle), installerBytes, applicationBytes, sbomBytes, ...expected, expectedPriorVersion }), /version/u);
  }
});
