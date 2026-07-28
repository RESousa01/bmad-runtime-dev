/**
 * Tests for the BM-02 coverage matrix and its checker.
 *
 * These mutate the matrix and assert the checker notices. A checker only ever
 * run against a correct file proves nothing about what it would catch, and the
 * whole point of this matrix is that it cannot quietly disagree with the
 * capability closure.
 */

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { copyFileSync, readFileSync, unlinkSync, writeFileSync } from "node:fs";
import process from "node:process";
import test from "node:test";

const CHECKER = "tools/check-bmad-coverage.mjs";
const MATRIX = "docs/bmad/skill-coverage.json";

const run = () => spawnSync(process.execPath, [CHECKER, "--check"], { encoding: "utf8" });

function withMutatedMatrix(mutate, assertion) {
  const backup = `${MATRIX}.bak`;
  copyFileSync(MATRIX, backup);
  try {
    const matrix = JSON.parse(readFileSync(MATRIX, "utf8"));
    mutate(matrix);
    writeFileSync(MATRIX, `${JSON.stringify(matrix, null, 2)}\n`, "utf8");
    assertion(run());
  } finally {
    copyFileSync(backup, MATRIX);
    unlinkSync(backup);
  }
}

test("the matrix covers the pinned upstream denominator and passes its check", () => {
  const matrix = JSON.parse(readFileSync(MATRIX, "utf8"));
  assert.equal(matrix.skills.length, 48, "the pinned upstream discovered 48 skills");
  assert.equal(matrix.pinnedUpstream, "bmad-method 6.10.0");
  const result = run();
  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
});

test("every skill has exactly one explicit support state", () => {
  const matrix = JSON.parse(readFileSync(MATRIX, "utf8"));
  for (const skill of matrix.skills) {
    assert.ok(skill.runtimeState, `${skill.upstreamSkill} has no runtime state`);
    assert.ok(skill.snapshotState, `${skill.upstreamSkill} has no snapshot state`);
  }
  const names = matrix.skills.map((s) => s.upstreamSkill);
  assert.equal(new Set(names).size, names.length, "a skill appears twice");
});

test("a short denominator is reported, because coverage over the wrong base is worse than none", () => {
  withMutatedMatrix(
    (matrix) => {
      matrix.skills.pop();
      matrix.totals.upstreamSkills = matrix.skills.length;
    },
    (result) => {
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /pinned upstream discovered 48 skills/u);
    },
  );
});

test("a capability the closure ledger does not define is reported", () => {
  withMutatedMatrix(
    (matrix) => {
      matrix.skills.find((s) => s.capabilityId !== null).capabilityId = "bmm:not-real";
    },
    (result) => {
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /closure ledger does not define/u);
    },
  );
});

test("an active projection with no capability is reported", () => {
  withMutatedMatrix(
    (matrix) => {
      matrix.skills.find((s) => s.runtimeState === "not_projected").runtimeState = "active_projection";
    },
    (result) => {
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /active_projection with no capability/u);
    },
  );
});

test("an unprojected row that still names a capability is reported", () => {
  withMutatedMatrix(
    (matrix) => {
      matrix.skills.find((s) => s.runtimeState === "active_projection").runtimeState = "not_projected";
    },
    (result) => {
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /not_projected yet names a capability/u);
    },
  );
});

test("stale totals are reported rather than trusted", () => {
  withMutatedMatrix(
    (matrix) => { matrix.totals.snapshotState.identical += 1; },
    (result) => {
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /totals\.snapshotState\.identical/u);
    },
  );
});

test("an unknown state is reported rather than passed through", () => {
  withMutatedMatrix(
    (matrix) => { matrix.skills[0].runtimeState = "probably_fine"; },
    (result) => {
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /unknown runtimeState/u);
    },
  );
});

test("no row claims a governed decision, because BM-03 has not recorded one", () => {
  const matrix = JSON.parse(readFileSync(MATRIX, "utf8"));
  const decided = matrix.skills.filter((s) => s.decision !== null);
  assert.equal(
    decided.length,
    0,
    "a decision here asserts a compatibility position the ledger has not taken",
  );
});
