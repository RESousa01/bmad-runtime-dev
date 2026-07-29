import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import test from "node:test";

const packageRoot = resolve(import.meta.dirname, "..");
const verifier = resolve(packageRoot, "scripts", "source-accounting.mjs");
const accountingPath = resolve(packageRoot, "source-accounting.json");

function verifyWith(path) {
  return spawnSync(process.execPath, [verifier, "--check"], {
    cwd: packageRoot,
    encoding: "utf8",
    env: { ...process.env, BMAD_SOURCE_ACCOUNTING_PATH: path },
  });
}

test("every pinned source member has one governed source-accounting treatment", () => {
  const result = spawnSync(process.execPath, [verifier, "--check"], {
    cwd: packageRoot,
    encoding: "utf8",
  });

  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
  assert.match(
    result.stdout,
    /source accounting verified: members=896 primary_method=48 primary_builder=5/u,
  );
});

// Break caught: leaving the first completed Phase 2 Method row in the
// generator's blanket pending state makes every downstream projection stale
// again the next time source-accounting.json is regenerated.
test("qualified Method source rows are runtime-available while other rows stay pending", async () => {
  const accounting = JSON.parse(await readFile(accountingPath, "utf8"));
  const prd = accounting.members.find(
    (member) => member.path === "src/bmm-skills/2-plan-workflows/bmad-prd/SKILL.md",
  );
  const readiness = accounting.members.find(
    (member) =>
      member.path
      === "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/SKILL.md",
  );

  assert.equal(prd?.support_state, "exact");
  assert.equal(prd?.ui_state, "runtime_available");
  assert.equal(prd?.release_state, "qualified");
  assert.equal(readiness?.support_state, "exact");
  assert.equal(readiness?.ui_state, "runtime_available");
  assert.equal(readiness?.release_state, "qualified");
  assert.deepEqual(accounting.totals.runtime_support, {
    exact: 3,
    implementation_pending: 50,
  });
  assert.deepEqual(accounting.totals.ui_support, {
    planned: 50,
    runtime_available: 3,
  });
  assert.deepEqual(accounting.totals.release_state, {
    blocked_pending_conformance: 50,
    qualified: 3,
  });
});

test("the canonical Deep Recon source row is runtime-qualified", async () => {
  const accounting = JSON.parse(await readFile(accountingPath, "utf8"));
  const recon = accounting.members.find(
    (member) => member.path === "src/core-skills/bmad-deep-recon/SKILL.md",
  );

  assert.equal(recon?.support_state, "exact");
  assert.equal(recon?.ui_state, "runtime_available");
  assert.equal(recon?.release_state, "qualified");
});

test("missing and contradictory accounting rows fail closed", async () => {
  const temporaryRoot = await mkdtemp(resolve(tmpdir(), "bmad-source-accounting-"));
  try {
    const canonical = JSON.parse(await readFile(accountingPath, "utf8"));
    const cases = [
      {
        name: "missing-member.json",
        mutate: (value) => value.members.pop(),
        error: /896/u,
      },
      {
        name: "rejected-primary.json",
        mutate: (value) => {
          value.members.find((member) => member.runtime_denominator).treatment = "reject_nonruntime";
        },
        error: /primary rejected/u,
      },
      {
        name: "purposeless-nonruntime.json",
        mutate: (value) => {
          value.members.find((member) => !member.runtime_denominator).retained_purpose = "";
        },
        error: /retained purpose is required/u,
      },
    ];

    for (const testCase of cases) {
      const mutated = structuredClone(canonical);
      testCase.mutate(mutated);
      const path = resolve(temporaryRoot, testCase.name);
      await writeFile(path, `${JSON.stringify(mutated, null, 2)}\n`, "utf8");
      const result = verifyWith(path);
      assert.notEqual(result.status, 0, `${testCase.name} unexpectedly verified`);
      assert.match(`${result.stdout}\n${result.stderr}`, testCase.error);
    }
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
});
