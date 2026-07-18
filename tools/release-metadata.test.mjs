import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { resolveReleaseMetadata } from "./resolve-release-metadata.mjs";

function createFixture() {
  const root = mkdtempSync(join(tmpdir(), "sapphirus-release-metadata-"));
  mkdirSync(join(root, "apps", "desktop-ui"), { recursive: true });
  mkdirSync(join(root, "crates", "desktop-app"), { recursive: true });
  writeFileSync(join(root, ".node-version"), "24.18.0\n");
  writeFileSync(join(root, "rust-toolchain.toml"), '[toolchain]\nchannel = "1.97.0"\n');
  writeFileSync(
    join(root, "Cargo.toml"),
    '[workspace.package]\nversion = "1.2.3"\nrust-version = "1.97.0"\n',
  );
  writeFileSync(join(root, "Cargo.lock"), "version = 4\n");
  writeFileSync(join(root, "pnpm-lock.yaml"), "lockfileVersion: '9.0'\n");
  writeFileSync(
    join(root, "package.json"),
    `${JSON.stringify({
      name: "sapphirus-desktop",
      version: "1.2.3",
      packageManager: "pnpm@11.12.0",
      engines: { node: "24.18.0", pnpm: "11.12.0" },
      devDependencies: { "@tauri-apps/cli": "2.11.4", typescript: "7.0.2" },
    })}\n`,
  );
  writeFileSync(
    join(root, "apps", "desktop-ui", "package.json"),
    `${JSON.stringify({ name: "@sapphirus/desktop-ui", version: "1.2.3" })}\n`,
  );
  writeFileSync(
    join(root, "crates", "desktop-app", "tauri.conf.json"),
    `${JSON.stringify({
      productName: "Sapphirus",
      version: "1.2.3",
      identifier: "com.sapphirus.desktop",
      mainBinaryName: "sapphirus",
    })}\n`,
  );
  return root;
}

test("release metadata centralizes product, toolchain, and lock identities", () => {
  const root = createFixture();
  try {
    const metadata = resolveReleaseMetadata(root);
    assert.deepEqual(metadata.product, {
      name: "Sapphirus",
      version: "1.2.3",
      identifier: "com.sapphirus.desktop",
      applicationName: "sapphirus.exe",
      installerName: "Sapphirus_1.2.3_x64-setup.exe",
      sbomName: "Sapphirus_1.2.3.cdx.json",
    });
    assert.deepEqual(metadata.toolchain, {
      node: "24.18.0",
      pnpm: "11.12.0",
      rust: "1.97.0",
      typescript: "7.0.2",
      tauriCli: "2.11.4",
    });
    assert.match(metadata.locks.cargo.sha256, /^[0-9a-f]{64}$/u);
    assert.match(metadata.locks.pnpm.sha256, /^[0-9a-f]{64}$/u);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("release metadata rejects version drift", () => {
  const root = createFixture();
  try {
    const rendererPath = join(root, "apps", "desktop-ui", "package.json");
    const renderer = JSON.parse(readFileSync(rendererPath, "utf8"));
    renderer.version = "1.2.4";
    writeFileSync(rendererPath, `${JSON.stringify(renderer)}\n`);
    assert.throws(() => resolveReleaseMetadata(root), /release versions disagree/u);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("metadata CLI writes GitHub outputs from the same authority", () => {
  const root = createFixture();
  const outputPath = join(root, "github-output.txt");
  writeFileSync(outputPath, "");
  try {
    execFileSync(
      process.execPath,
      [
        fileURLToPath(new URL("./resolve-release-metadata.mjs", import.meta.url)),
        "--root",
        root,
        "--github-output",
        outputPath,
      ],
      { encoding: "utf8" },
    );
    assert.equal(
      readFileSync(outputPath, "utf8"),
      "product_version=1.2.3\ninstaller_name=Sapphirus_1.2.3_x64-setup.exe\nsbom_name=Sapphirus_1.2.3.cdx.json\n",
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
