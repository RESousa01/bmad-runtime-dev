import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

const packageRoot = resolve(import.meta.dirname, "..");
const classifier = join(packageRoot, "scripts", "classify-scripts.mjs");

function writeJson(path, value) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function fixture(overrides = {}) {
  const root = mkdtempSync(join(tmpdir(), "bmad-script-inventory-"));
  const sourceRoot = join(root, "source");
  const runtimeRoot = join(root, "runtime");
  const manifestPath = join(root, "source-manifest.json");
  const inventoryPath = join(root, "script-inventory.json");
  const scriptPath = "skills/example/scripts/tool.py";
  const sha256 = "a".repeat(64);

  mkdirSync(join(sourceRoot, "method", dirname(scriptPath)), { recursive: true });
  mkdirSync(runtimeRoot, { recursive: true });
  writeFileSync(
    join(sourceRoot, "method", scriptPath),
    "#!/usr/bin/env python3\nprint('example')\n",
    "utf8",
  );
  writeJson(manifestPath, {
    schema_version: "sapphirus.bmad.source-manifest/v1",
    sources: [{ id: "method", version: "6.10.0", archive_sha256: "b".repeat(64) }],
    members: [
      {
        source_id: "method",
        path: scriptPath,
        sha256,
        class: "script_source",
        entry_type: "file",
        is_executable: true,
      },
    ],
  });
  writeJson(inventoryPath, {
    schema_version: "sapphirus.bmad.script-inventory/v1",
    discovery_policy_version: "1.0.0",
    entries: [],
    ...overrides.inventory,
  });
  if (overrides.runtimeReference) {
    writeFileSync(
      join(runtimeRoot, "catalog.json"),
      JSON.stringify({ helper: `packages/bmad-foundation/source/method/${scriptPath}` }),
      "utf8",
    );
  }

  return {
    root,
    sourceRoot,
    runtimeRoot,
    manifestPath,
    inventoryPath,
    scriptPath,
    sha256,
  };
}

function classify(paths) {
  return spawnSync(
    process.execPath,
    [
      classifier,
      "--strict",
      "--check",
      "--manifest",
      paths.manifestPath,
      "--inventory",
      paths.inventoryPath,
      "--source-root",
      paths.sourceRoot,
      "--runtime-root",
      paths.runtimeRoot,
    ],
    { cwd: packageRoot, encoding: "utf8" },
  );
}

test("strict classification rejects an executable source member without a reviewed treatment", () => {
  const paths = fixture();
  try {
    const result = classify(paths);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /unclassified executable or code-generating member/i);
    assert.match(result.stderr, /method:skills\/example\/scripts\/tool\.py/);
  } finally {
    rmSync(paths.root, { recursive: true, force: true });
  }
});

test("strict classification rejects a runtime reference to a rejected source script", () => {
  const paths = fixture({ runtimeReference: true });
  try {
    writeJson(paths.inventoryPath, {
      schema_version: "sapphirus.bmad.script-inventory/v1",
      discovery_policy_version: "1.0.0",
      entries: [
        {
          member_key: `method:${paths.scriptPath}`,
          source_id: "method",
          path: paths.scriptPath,
          sha256: paths.sha256,
          treatment: "reject",
          review: { status: "reviewed", owner: "Security", rationale: "Unsafe helper." },
        },
      ],
    });
    const result = classify(paths);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /runtime references rejected script/i);
    assert.match(result.stderr, /catalog\.json/);
  } finally {
    rmSync(paths.root, { recursive: true, force: true });
  }
});

test("strict classification rejects source-manifest hash drift", () => {
  const paths = fixture();
  try {
    writeJson(paths.inventoryPath, {
      schema_version: "sapphirus.bmad.script-inventory/v1",
      discovery_policy_version: "1.0.0",
      entries: [
        {
          member_key: `method:${paths.scriptPath}`,
          source_id: "method",
          path: paths.scriptPath,
          sha256: "c".repeat(64),
          treatment: "port",
          review: {
            status: "reviewed",
            owner: "Security",
            rationale: "Port behind a typed runtime boundary.",
          },
        },
      ],
    });
    const result = classify(paths);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /source hash drift/i);
    assert.match(result.stderr, /method:skills\/example\/scripts\/tool\.py/);
  } finally {
    rmSync(paths.root, { recursive: true, force: true });
  }
});

test("the pinned inventory is complete, reviewed, unique, and current", () => {
  const result = spawnSync(process.execPath, [classifier, "--strict", "--check"], {
    cwd: packageRoot,
    encoding: "utf8",
  });
  assert.equal(result.status, 0, result.stderr || result.stdout);

  const inventory = JSON.parse(
    readFileSync(join(packageRoot, "script-inventory.json"), "utf8"),
  );
  assert.equal(inventory.schema_version, "sapphirus.bmad.script-inventory/v1");
  assert.ok(inventory.entries.length > 0);
  assert.equal(
    new Set(inventory.entries.map((entry) => entry.member_key)).size,
    inventory.entries.length,
  );
  for (const entry of inventory.entries) {
    assert.match(entry.sha256, /^[a-f0-9]{64}$/);
    assert.ok(
      ["port", "sandbox", "fixture_only", "distribution_only", "reject"].includes(
        entry.treatment,
      ),
    );
    assert.equal(entry.review.status, "reviewed");
    assert.ok(entry.review.owner);
    assert.ok(entry.review.rationale);
  }
});
