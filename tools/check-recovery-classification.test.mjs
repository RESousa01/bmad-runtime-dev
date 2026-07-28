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

test("flags a switch arm", () => {
  // A switch is as much a branch as a comparison. The first version of this
  // checker matched only `===`, so this form passed silently -- which is the
  // failure the checker exists to prevent, occurring in the checker itself.
  const f = fixture({
    "feature.ts": [
      "switch (code) {",
      '  case "recovery_required":',
      '  case "integrity_failure":',
      "    return true;",
      "}",
    ].join("\n"),
  });
  try {
    const { report, status } = f.run();
    assert.equal(status, 1);
    assert.equal(report.offenders[0].lines.length, 2);
  } finally {
    f.cleanup();
  }
});

test("flags a list holding only the two codes", () => {
  // However it is later consulted -- `.includes`, `Set.has`, `.some`.
  const f = fixture({
    "feature.ts": [
      'const R = ["recovery_required", "integrity_failure"];',
      "export const is = (c) => R.includes(c);",
    ].join("\n"),
  });
  try {
    assert.equal(f.run().status, 1);
  } finally {
    f.cleanup();
  }
});

test("allows a list carrying the two codes among the others", () => {
  // That is the parser describing every host code, not a classifier.
  const f = fixture({
    "parser.ts":
      'const codes = ["conflict", "recovery_required", "integrity_failure", "internal"];\n',
  });
  try {
    assert.equal(f.run().status, 0);
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

test("allows a declaration that lists the codes among the others", () => {
  /*
   * `contracts.ts` declares the union of every host code and
   * `workspaceProtocol.ts` validates against all of them. Both name these two
   * legitimately and neither decides anything with them.
   *
   * The parser fixture deliberately carries other codes. An earlier version of
   * this test used a `Set` of *only* these two and asserted it was allowed --
   * but that is a classifier, whatever it is called, and the rule that flags it
   * is correct. The fixture was the thing that was wrong.
   */
  const f = fixture({
    "contracts.ts":
      'export type Code = "conflict" | "recovery_required" | "integrity_failure";\n',
    "parser.ts":
      'const codes = new Set(["conflict", "recovery_required", "integrity_failure", "internal"]);\n',
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
