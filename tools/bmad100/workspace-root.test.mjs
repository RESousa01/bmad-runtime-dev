import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  cpSync,
  mkdtempSync,
  mkdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";

const repositoryRoot = resolve(import.meta.dirname, "..", "..");
const verifier = resolve(import.meta.dirname, "verify-workspace.mjs");

function runVerifier(root) {
  return spawnSync(process.execPath, [verifier, "--root", root], {
    encoding: "utf8",
  });
}

test("the authoritative root enumerates members without promoting retained source data", () => {
  const result = runVerifier(repositoryRoot);

  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
  assert.match(result.stdout, /workspace verified: pnpm=5 cargo=12 dotnet=5/u);
});

test("an undeclared package manifest is rejected", () => {
  const fixture = mkdtempSync(join(tmpdir(), "sapphirus-workspace-"));
  try {
    for (const path of [
      "package.json",
      "pnpm-workspace.yaml",
      "Cargo.toml",
      "global.json",
      "Sapphirus.slnx",
      ".nvmrc",
      "rust-toolchain.toml",
      ".gitignore",
    ]) {
      const destination = join(fixture, path);
      mkdirSync(resolve(destination, ".."), { recursive: true });
      cpSync(join(repositoryRoot, path), destination);
    }

    for (const directory of ["apps", "packages", "crates", "helpers", "services", "tests"]) {
      cpSync(join(repositoryRoot, directory), join(fixture, directory), {
        recursive: true,
        filter: (source) =>
          !source.includes("node_modules") &&
          !source.includes("target") &&
          !source.includes("generated"),
      });
    }

    mkdirSync(join(fixture, "extensions", "undeclared"), { recursive: true });
    writeFileSync(
      join(fixture, "extensions", "undeclared", "package.json"),
      '{"name":"@sapphirus/undeclared","private":true}\n',
      "utf8",
    );

    const result = runVerifier(fixture);

    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /undeclared package manifest.*extensions\/undeclared\/package\.json/u);
  } finally {
    rmSync(fixture, { recursive: true, force: true });
  }
});
