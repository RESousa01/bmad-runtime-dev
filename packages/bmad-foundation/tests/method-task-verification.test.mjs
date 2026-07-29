import assert from "node:assert/strict";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { fileURLToPath } from "node:url";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = resolve(packageRoot, "..", "..");

function run(script, args) {
  return spawnSync(process.execPath, [resolve(repositoryRoot, script), ...args], {
    cwd: repositoryRoot,
    encoding: "utf8",
  });
}

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

test("Method task verification resolves short skill ids and writes bound receipts", () => {
  const root = mkdtempSync(resolve(tmpdir(), "bmad-method-receipts-"));
  try {
    const closurePath = resolve(root, "source-closure.json");
    const conformancePath = resolve(root, "conformance.json");
    const authorityPath = resolve(root, "authority.json");

    const closure = run("packages/bmad-foundation/scripts/resource-closure.mjs", [
      "--source",
      "method@6.10.0",
      "--skill",
      "bmad-prd",
      "--check",
      "--json",
      closurePath,
    ]);
    assert.equal(closure.status, 0, closure.stderr);
    const closureReceipt = readJson(closurePath);
    assert.equal(closureReceipt.packages.length, 1);
    assert.equal(closureReceipt.packages[0].production_skills.length, 1);
    assert.equal(
      closureReceipt.packages[0].production_skills[0].root,
      "src/bmm-skills/2-plan-workflows/bmad-prd/SKILL.md",
    );
    assert.ok(closureReceipt.packages[0].member_accounting.length > 1);
    assert.ok(
      closureReceipt.packages[0].member_accounting.every(
        (member) => member.path.includes("/bmad-prd/") || member.path.endsWith("/bmad-prd/SKILL.md"),
      ),
    );

    const conformance = run("packages/bmad-foundation/scripts/conformance.mjs", [
      "--skill",
      "bmad-prd",
      "--all-modes",
      "--strict",
      "--json",
      conformancePath,
    ]);
    assert.equal(conformance.status, 0, conformance.stderr);
    assert.deepEqual(readJson(conformancePath).skills, [
      "method:src/bmm-skills/2-plan-workflows/bmad-prd/SKILL.md",
    ]);

    const authority = run("tools/bmad100/verify-authority.mjs", [
      "--skill",
      "bmad-prd",
      "--json",
      authorityPath,
    ]);
    assert.equal(authority.status, 0, authority.stderr);
    assert.deepEqual(readJson(authorityPath).skills, [
      "method:src/bmm-skills/2-plan-workflows/bmad-prd/SKILL.md",
    ]);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("failed Method verification removes a stale receipt", () => {
  const root = mkdtempSync(resolve(tmpdir(), "bmad-method-stale-"));
  try {
    const receiptPath = resolve(root, "conformance.json");
    writeFileSync(receiptPath, "{\"stale\":true}\n", "utf8");
    const result = run("packages/bmad-foundation/scripts/conformance.mjs", [
      "--skill",
      "not-a-real-skill",
      "--all-modes",
      "--strict",
      "--json",
      receiptPath,
    ]);
    assert.equal(result.status, 1);
    assert.equal(existsSync(receiptPath), false);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
