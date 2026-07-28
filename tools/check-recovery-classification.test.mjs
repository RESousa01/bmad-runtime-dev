import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import process from "node:process";
import test from "node:test";

const CHECKER = "tools/check-recovery-classification.mjs";

/**
 * A throwaway source tree.
 *
 * Running the checker against the repository proves nothing on its own: one
 * that matched nothing would also report zero offenders.
 */
function fixture(files) {
  const dir = mkdtempSync(path.join(tmpdir(), "recovery-class-"));
  for (const [name, source] of Object.entries(files)) {
    const full = path.join(dir, name);
    mkdirSync(path.dirname(full), { recursive: true });
    writeFileSync(full, source, "utf8");
  }
  return {
    cleanup: () => rmSync(dir, { force: true, recursive: true }),
    run: () => {
      const result = spawnSync(
        process.execPath,
        [CHECKER, "--json", "--root", dir],
        { encoding: "utf8" },
      );
      return { ...result, report: JSON.parse(result.stdout) };
    },
  };
}

test("flags a re-implemented classification", () => {
  const f = fixture({
    "feature.ts":
      'if (error.details.code === "recovery_required") enterRecovery();\n',
  });
  try {
    const { report, status } = f.run();
    assert.equal(status, 1);
    assert.equal(report.offenders.length, 1);
    assert.equal(report.offenders[0].lines[0].line, 1);
  } finally {
    f.cleanup();
  }
});

test("flags the second code too", () => {
  const f = fixture({
    "feature.ts": 'const bad = code === "integrity_failure";\n',
  });
  try {
    assert.equal(f.run().status, 1);
  } finally {
    f.cleanup();
  }
});

test("flags a negated comparison", () => {
  // `!==` decides just as much as `===`.
  const f = fixture({ "feature.ts": 'if (code !== "recovery_required") retry();\n' });
  try {
    assert.equal(f.run().status, 1);
  } finally {
    f.cleanup();
  }
});

test("allows the owning module", () => {
  const f = fixture({
    "app/hostFailure.ts":
      'export const is = (c) => c === "recovery_required" || c === "integrity_failure";\n',
  });
  try {
    const { report, status } = f.run();
    assert.equal(status, 0);
    assert.deepEqual(report.offenders, []);
  } finally {
    f.cleanup();
  }
});

test("allows a declaration that only lists the codes", () => {
  // `contracts.ts` declares the union of every host code and
  // `workspaceProtocol.ts` parses it. Both name these two legitimately and
  // neither decides anything with them.
  const f = fixture({
    "contracts.ts":
      'export type Code = "conflict" | "recovery_required" | "integrity_failure";\n',
    "parser.ts":
      'const codes = new Set(["recovery_required", "integrity_failure"]);\n',
  });
  try {
    const { report, status } = f.run();
    assert.equal(status, 0);
    assert.deepEqual(report.offenders, []);
  } finally {
    f.cleanup();
  }
});

test("allows a test to construct the codes", () => {
  // Tests build these to prove the classifier works.
  const f = fixture({
    "feature.test.ts": 'expect(code === "recovery_required").toBe(true);\n',
  });
  try {
    assert.equal(f.run().status, 0);
  } finally {
    f.cleanup();
  }
});

test("passes on a tree that never mentions them", () => {
  const f = fixture({ "feature.ts": "export const a = 1;\n" });
  try {
    const { report, status } = f.run();
    assert.equal(status, 0);
    assert.equal(report.scanned, 1);
  } finally {
    f.cleanup();
  }
});

test("reports every offending line, not just the first file", () => {
  const f = fixture({
    "a.ts": 'if (code === "recovery_required") x();\n',
    "b.ts": 'if (code === "integrity_failure") y();\n',
  });
  try {
    const { report } = f.run();
    assert.equal(report.offenders.length, 2);
  } finally {
    f.cleanup();
  }
});
