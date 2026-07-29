import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const scriptPath = resolve(packageRoot, "scripts", "builder-resource-closure.mjs");
const ledgerPath = resolve(packageRoot, "builder-source-ledger.json");

function runStrict(extraArgs = []) {
  return spawnSync(
    process.execPath,
    [scriptPath, "--builder", "2.1.0", "--strict", ...extraArgs],
    {
      cwd: packageRoot,
      encoding: "utf8",
      env: { ...process.env, NO_COLOR: "1" },
    },
  );
}

function withChangedLedger(change, callback) {
  const directory = mkdtempSync(resolve(tmpdir(), "bmad-builder-ledger-"));
  try {
    const path = resolve(directory, "ledger.json");
    const ledger = JSON.parse(readFileSync(ledgerPath, "utf8"));
    change(ledger);
    writeFileSync(path, `${JSON.stringify(ledger, null, 2)}\n`, "utf8");
    callback(path);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

test("strict Builder closure accepts the pinned complete graph", () => {
  const result = runStrict();
  assert.equal(result.status, 0, result.stderr || result.stdout);

  const ledger = JSON.parse(readFileSync(ledgerPath, "utf8"));
  assert.equal(ledger.totals.archive_members, 299);
  assert.equal(ledger.totals.governance_sidecars, 2);
  assert.equal(ledger.totals.accounted_members, 301);
  assert.equal(ledger.totals.classified_members, 301);
  assert.equal(ledger.totals.production_skills, 5);
  assert.deepEqual(
    ledger.production_skills.map((skill) => skill.id),
    [
      "bmad-agent-builder",
      "bmad-bmb-setup",
      "bmad-eval-runner",
      "bmad-module-builder",
      "bmad-workflow-builder",
    ],
  );
  assert.ok(
    ledger.production_skills.every(
      (skill) =>
        skill.resource_member_count > 0 &&
        skill.closed_member_count === skill.resource_member_count &&
        skill.closure_percent === 100,
    ),
  );
  assert.equal(ledger.capabilities.production_skill_count, 5);
  assert.equal(ledger.capabilities.sample_skill_count, 0);
  assert.equal(ledger.capabilities.web_distribution_skill_count, 0);
});

test("strict Builder closure detects ledger drift", () => {
  const result = runStrict(["--check-ledger", "tests/fixtures/builder-source-closure/drifted-ledger.json"]);
  assert.notEqual(result.status, 0);
  assert.match(`${result.stderr}\n${result.stdout}`, /ledger|drift|mismatch/i);
});

test("strict Builder closure detects omitted source members", () => {
  const result = runStrict([
    "--check-ledger",
    "tests/fixtures/builder-source-closure/omitted-member-ledger.json",
  ]);
  assert.notEqual(result.status, 0);
  assert.match(`${result.stderr}\n${result.stdout}`, /omitted|member|mismatch/i);
});

test("strict Builder closure rejects duplicate canonical identities", () => {
  withChangedLedger(
    (ledger) => {
      ledger.members[1].id = ledger.members[0].id;
    },
    (path) => {
      const result = runStrict(["--check-ledger", path]);
      assert.notEqual(result.status, 0);
      assert.match(
        `${result.stderr}\n${result.stdout}`,
        /duplicate canonical identity/i,
      );
    },
  );
});

test("strict Builder closure rejects unclassified members", () => {
  withChangedLedger(
    (ledger) => {
      delete ledger.members[0].classification;
    },
    (path) => {
      const result = runStrict(["--check-ledger", path]);
      assert.notEqual(result.status, 0);
      assert.match(
        `${result.stderr}\n${result.stdout}`,
        /unclassified ledger member/i,
      );
    },
  );
});
