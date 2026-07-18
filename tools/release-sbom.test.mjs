import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";

import {
  createCycloneDxSbom,
  serializeCycloneDxSbom,
  verifyCycloneDxSbom,
} from "./generate-release-sbom.mjs";

const sha512Bytes = Buffer.alloc(64, 0x2a);
const npmIntegrity = `sha512-${sha512Bytes.toString("base64")}`;
const cargoChecksum = "11".repeat(32);
const releaseMetadata = {
  schemaVersion: 1,
  product: {
    name: "Sapphirus",
    version: "1.2.3",
    identifier: "com.sapphirus.desktop",
    applicationName: "sapphirus.exe",
    installerName: "Sapphirus_1.2.3_x64-setup.exe",
    sbomName: "Sapphirus_1.2.3.cdx.json",
  },
  toolchain: {
    node: "24.18.0",
    pnpm: "11.12.0",
    rust: "1.97.0",
    typescript: "7.0.2",
    tauriCli: "2.11.4",
  },
  locks: {
    cargo: { path: "Cargo.lock", sha256: "aa".repeat(32) },
    pnpm: { path: "pnpm-lock.yaml", sha256: "bb".repeat(32) },
  },
};

const pnpmLock = `lockfileVersion: '9.0'

packages:

  '@scope/example@1.2.3':
    resolution: {integrity: ${npmIntegrity}}

  react@19.2.7:
    resolution: {integrity: ${npmIntegrity}}

snapshots:
`;

const cargoLock = `version = 4

[[package]]
name = "serde"
version = "1.0.228"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "${cargoChecksum}"

[[package]]
name = "desktop-app"
version = "1.2.3"
`;

test("SBOM deterministically inventories pnpm and Cargo locks", () => {
  const first = createCycloneDxSbom({ releaseMetadata, pnpmLock, cargoLock });
  const second = createCycloneDxSbom({ releaseMetadata, pnpmLock, cargoLock });
  assert.deepEqual(first, second);
  assert.equal(first.bomFormat, "CycloneDX");
  assert.equal(first.specVersion, "1.6");
  assert.equal(first.metadata.component.version, "1.2.3");
  assert.deepEqual(
    first.components.map((component) => component.purl),
    [
      "pkg:cargo/desktop-app@1.2.3",
      "pkg:cargo/serde@1.0.228",
      "pkg:npm/%40scope/example@1.2.3",
      "pkg:npm/react@19.2.7",
    ],
  );
  const npmComponent = first.components.find((component) => component.purl.includes("react"));
  assert.equal(npmComponent.hashes[0].alg, "SHA-512");
  assert.equal(npmComponent.hashes[0].content, sha512Bytes.toString("hex"));
  const cargoComponent = first.components.find((component) => component.purl.includes("serde"));
  assert.equal(cargoComponent.hashes[0].content, cargoChecksum);
});

test("SBOM records exact lock digests and has stable serialized bytes", () => {
  const sbom = createCycloneDxSbom({ releaseMetadata, pnpmLock, cargoLock });
  assert.deepEqual(sbom.metadata.properties, [
    { name: "sapphirus:lock:Cargo.lock:sha256", value: "aa".repeat(32) },
    { name: "sapphirus:lock:pnpm-lock.yaml:sha256", value: "bb".repeat(32) },
    { name: "sapphirus:scope", value: "complete-build-lock-inventory" },
  ]);
  const bytes = `${JSON.stringify(sbom, null, 2)}\n`;
  assert.equal(createHash("sha256").update(bytes).digest("hex").length, 64);
});

test("SBOM rejects lock entries whose package identity cannot be resolved", () => {
  assert.throws(
    () => createCycloneDxSbom({
      releaseMetadata,
      pnpmLock: "lockfileVersion: '9.0'\npackages:\n  malformed-key:\n    resolution: {}\n",
      cargoLock,
    }),
    /invalid pnpm package key/u,
  );
});

test("SBOM verification rejects an altered component inventory", () => {
  const sbom = createCycloneDxSbom({ releaseMetadata, pnpmLock, cargoLock });
  sbom.components.pop();
  assert.throws(
    () => verifyCycloneDxSbom({
      releaseMetadata,
      pnpmLock,
      cargoLock,
      candidate: serializeCycloneDxSbom(sbom),
    }),
    /SBOM bytes disagree/u,
  );
});

test("SBOM verification rejects stale lock metadata", () => {
  const staleMetadata = structuredClone(releaseMetadata);
  staleMetadata.locks.pnpm.sha256 = "cc".repeat(32);
  const staleSbom = serializeCycloneDxSbom(createCycloneDxSbom({
    releaseMetadata: staleMetadata,
    pnpmLock,
    cargoLock,
  }));
  assert.throws(
    () => verifyCycloneDxSbom({ releaseMetadata, pnpmLock, cargoLock, candidate: staleSbom }),
    /SBOM bytes disagree/u,
  );
});

test("SBOM verification rejects a current SBOM after a lock mutation", () => {
  const candidate = serializeCycloneDxSbom(createCycloneDxSbom({ releaseMetadata, pnpmLock, cargoLock }));
  assert.throws(
    () => verifyCycloneDxSbom({
      releaseMetadata,
      pnpmLock: pnpmLock.replace("react@19.2.7", "react@19.2.8"),
      cargoLock,
      candidate,
    }),
    /SBOM bytes disagree/u,
  );
});
