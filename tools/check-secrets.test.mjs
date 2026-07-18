import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const scannerPath = join(dirname(fileURLToPath(import.meta.url)), "check-secrets.mjs");
const scanRoots = [
  ".github",
  "apps",
  "crates",
  "docs",
  "helpers",
  "packages",
  "services",
  "tests",
  "tools",
];
const rootFiles = [
  ".editorconfig",
  ".gitattributes",
  ".gitignore",
  ".npmrc",
  ".node-version",
  ".nvmrc",
  "Cargo.lock",
  "Cargo.toml",
  "README.md",
  "deny.toml",
  "package.json",
  "pnpm-lock.yaml",
  "pnpm-workspace.yaml",
  "rust-toolchain.toml",
  "rustfmt.toml",
  "tsconfig.base.json",
];

async function createFixtureRoot() {
  const root = await mkdtemp(join(tmpdir(), "check-secrets-"));
  for (const directory of scanRoots) {
    await mkdir(join(root, directory), { recursive: true });
  }
  for (const file of rootFiles) {
    await writeFile(join(root, file), "# fixture\n");
  }
  return root;
}

function runScanner(root) {
  return spawnSync(process.execPath, [scannerPath], { cwd: root, encoding: "utf8" });
}

test("scanner passes a clean fixture tree", async () => {
  const root = await createFixtureRoot();
  try {
    await writeFile(join(root, "apps", "clean.tsx"), 'const key = `${a}\\u0000${b}`;\n');
    const result = runScanner(root);
    assert.equal(result.status, 0, result.stderr);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("scanner rejects a literal NUL byte in text source", async () => {
  const root = await createFixtureRoot();
  try {
    await writeFile(join(root, "apps", "nul.tsx"), Buffer.from("const key = `a\0b`;\n", "latin1"));
    const result = runScanner(root);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /NUL byte in text source/);
    assert.match(result.stderr, /nul\.tsx:1/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("scanner rejects other literal C0 control bytes in text source", async () => {
  const root = await createFixtureRoot();
  try {
    await writeFile(join(root, "apps", "unit-separator.ts"), Buffer.from("const key = `a\x1fb`;\n", "latin1"));
    const result = runScanner(root);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /C0 control byte in text source/);
    assert.match(result.stderr, /unit-separator\.ts:1/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("scanner rejects a literal secret assignment", async () => {
  const root = await createFixtureRoot();
  try {
    // Assembled at runtime so this test file does not itself trip the scanner.
    const probe = `const leak = { ${["client", "secret"].join("_")}: "hunter2-hunter2" };\n`;
    await writeFile(join(root, "packages", "leak.ts"), probe);
    const result = runScanner(root);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /literal secret assignment/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("scanner covers first-party C sharp sources", async () => {
  const root = await createFixtureRoot();
  try {
    const probe = `var ${["client", "secret"].join("_")} = "hunter2-hunter2";\n`;
    const serviceRoot = join(root, "services", "desktop-support-api");
    await mkdir(serviceRoot, { recursive: true });
    await writeFile(join(serviceRoot, "Leak.cs"), probe);
    const result = runScanner(root);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /literal secret assignment/);
    assert.match(result.stderr, /Leak\.cs:1/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
