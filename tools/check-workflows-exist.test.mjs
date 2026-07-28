/**
 * Tests for check-workflows.mjs, which enforces QR-03's acceptance criteria:
 * CI calls only commands and paths that exist, and a Windows artifact is
 * proven on a Windows runner.
 *
 * Named `check-workflows-exist` because `check-workflows.test.mjs` already
 * exists and covers a different concern -- CI hygiene, not CI reachability.
 */

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { copyFileSync, readFileSync, writeFileSync } from "node:fs";
import process from "node:process";
import test from "node:test";

const CHECKER = "tools/check-workflows.mjs";

const run = () => spawnSync(process.execPath, [CHECKER], { encoding: "utf8" });

/** Edits a workflow, runs the checker, then puts the file back. */
function withMutation(workflow, mutate, assertion) {
  const path = `.github/workflows/${workflow}`;
  const backup = `${path}.bak`;
  copyFileSync(path, backup);
  try {
    writeFileSync(path, mutate(readFileSync(path, "utf8")), "utf8");
    assertion(run());
  } finally {
    copyFileSync(backup, path);
    spawnSync(process.execPath, ["-e", `require("node:fs").unlinkSync(${JSON.stringify(backup)})`]);
  }
}

test("the repository's workflows pass", () => {
  const result = run();
  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
});

test("a pnpm script that does not exist is reported", () => {
  withMutation(
    "desktop.yml",
    (source) => source.replace("pnpm verify:boundaries", "pnpm verify:boundariez"),
    (result) => {
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /verify:boundariez.*not a root script/u);
    },
  );
});

test("a node path that does not exist is reported, including inside a block scalar", () => {
  // contracts.yml calls node from inside a `run: |` block. An earlier version
  // of the checker captured such a block as the single command "|" and read
  // nothing inside it, so this case is the regression guard for that.
  withMutation(
    "contracts.yml",
    (source) => source.replace(
      "node tools/contract-codegen/native-generator.mjs",
      "node tools/contract-codegen/native-generatorz.mjs",
    ),
    (result) => {
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /native-generatorz\.mjs.*does not exist/u);
    },
  );
});

test("a Windows artifact built without a Windows runner is reported", () => {
  withMutation(
    "release-windows-signed.yml",
    (source) => source.replaceAll(/runs-on: \[self-hosted, windows, x64, [^\]]+\]/gu, "runs-on: ubuntu-24.04"),
    (result) => {
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /no job declares a Windows runner/u);
    },
  );
});

test("a script that exists in the filtered package is accepted", () => {
  withMutation(
    "desktop.yml",
    (source) => `${source}\n# pnpm --filter @forge/desktop-ui typecheck\n`,
    (result) => assert.equal(result.status, 0, result.stderr),
  );
});
