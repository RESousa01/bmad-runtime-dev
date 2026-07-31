import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";
import { fixtureRoot, runScript, write, writeJson } from "./test-helpers.mjs";

test("generates pin values from owner manifests instead of a duplicated expectation", () => {
  const root = fixtureRoot("pins");
  write(root, ".node-version", "24.18.0\n");
  write(root, ".nvmrc", "24.18.0\n");
  writeJson(root, "package.json", {
    packageManager: "pnpm@11.15.1",
    engines: { node: "24.18.0", pnpm: "11.15.1" },
  });
  writeJson(root, "apps/desktop-ui/package.json", {
    dependencies: { react: "19.2.7" },
    devDependencies: { vite: "8.1.5", typescript: "7.0.2" },
  });
  write(root, "Cargo.toml", '[workspace.package]\nversion = "0.1.0"\n');
  write(root, "rust-toolchain.toml", '[toolchain]\nchannel = "1.97.0"\n');
  writeJson(root, "global.json", { sdk: { version: "10.0.302" } });
  writeJson(root, "crates/desktop-app/tauri.conf.json", { version: "0.1.0" });

  const output = join(root, "docs/_meta/pins.generated.json");
  const result = runScript("generate-pins.mjs", ["--root", root, "--output", output]);
  assert.equal(result.status, 0, result.stderr);

  const pins = JSON.parse(readFileSync(output, "utf8"));
  assert.deepEqual(
    Object.fromEntries(pins.pins.map((pin) => [pin.id, pin.observed])),
    {
      node: "24.18.0",
      pnpm: "11.15.1",
      rust: "1.97.0",
      dotnet: "10.0.302",
      vite: "8.1.5",
      typescript: "7.0.2",
      react: "19.2.7",
      workspace: "0.1.0",
      desktop: "0.1.0",
    },
  );
});

test("check mode rejects a stale generated pin projection", () => {
  const root = fixtureRoot("pins-check");
  write(root, ".node-version", "24.18.0\n");
  writeJson(root, "package.json", {
    packageManager: "pnpm@11.15.1",
    engines: { node: "24.18.0", pnpm: "11.15.1" },
  });
  writeJson(root, "apps/desktop-ui/package.json", { devDependencies: { vite: "8.1.5" } });
  write(root, "Cargo.toml", '[workspace.package]\nversion = "0.1.0"\n');
  write(root, "rust-toolchain.toml", '[toolchain]\nchannel = "1.97.0"\n');
  writeJson(root, "global.json", { sdk: { version: "10.0.302" } });
  writeJson(root, "crates/desktop-app/tauri.conf.json", { version: "0.1.0" });
  const output = write(root, "docs/_meta/pins.generated.json", "{}\n");

  const result = runScript("generate-pins.mjs", [
    "--root",
    root,
    "--output",
    output,
    "--check",
  ]);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /generated pin projection is stale/i);
});

