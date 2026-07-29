import { spawnSync } from "node:child_process";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import {
  EXPECTED_META_TEST_COUNT,
  META_TEST_CASES,
  sha256,
  stableJson,
} from "./lib/qal-01-meta-contract.mjs";

const repositoryRoot = resolve(import.meta.dirname, "../..");
const testPath = resolve(repositoryRoot, "tools/bmad100/tests/source-closure-meta.test.mjs");
const publisherPath = resolve(
  repositoryRoot,
  "tools/bmad100/publish-source-closure-receipt.mjs",
);
const reportPath = resolve(repositoryRoot, "artifacts/bmad100/quality/qal-01/report.json");
const receiptPath = resolve(
  repositoryRoot,
  "artifacts/bmad100/quality/qal-01/logs/meta-tests.json",
);
const resultsPath = resolve(
  repositoryRoot,
  "artifacts/bmad100/quality/qal-01/logs/meta-test-results.json",
);
const tapPath = resolve(
  repositoryRoot,
  "artifacts/bmad100/quality/qal-01/logs/meta-tests.tap",
);

async function invalidatePriorPromotion() {
  await rm(receiptPath, { force: true });
  await rm(resultsPath, { force: true });
  try {
    const report = JSON.parse(await readFile(reportPath, "utf8"));
    report.status = "pending_meta_tests";
    report.complete = false;
    delete report.meta_tests;
    delete report.command_results;
    await writeFile(reportPath, stableJson(report), "utf8");
  } catch {
    // The test suite will diagnose missing canonical closure evidence.
  }
}

function summaryValue(output, label) {
  const match = output.match(new RegExp(`^# ${label} (\\d+)$`, "mu"));
  return match ? Number.parseInt(match[1], 10) : undefined;
}

async function main() {
  await invalidatePriorPromotion();
  const startedAt = new Date().toISOString();
  const args = ["--test", "--test-reporter=tap", testPath];
  const suite = spawnSync(process.execPath, args, {
    cwd: repositoryRoot,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  });
  const output = `${suite.stdout ?? ""}${suite.stderr ?? ""}`;
  process.stdout.write(output);
  await mkdir(dirname(tapPath), { recursive: true });
  await writeFile(tapPath, output, "utf8");
  if (suite.status !== 0) {
    process.exitCode = suite.status ?? 1;
    return;
  }

  const names = [...output.matchAll(/^# Subtest: (.+)$/gmu)].map((match) => match[1]);
  const expectedNames = META_TEST_CASES.map(({ name }) => name);
  const total = summaryValue(output, "tests");
  const passed = summaryValue(output, "pass");
  const failed = summaryValue(output, "fail");
  if (
    total !== EXPECTED_META_TEST_COUNT ||
    passed !== EXPECTED_META_TEST_COUNT ||
    failed !== 0 ||
    JSON.stringify(names) !== JSON.stringify(expectedNames)
  ) {
    throw new Error("Node test output does not prove the exact governed 14/14 inventory");
  }

  const pnpmCommand =
    process.platform === "win32"
      ? ["cmd.exe", ["/d", "/s", "/c", "pnpm --version"]]
      : ["pnpm", ["--version"]];
  const pnpmResult = spawnSync(pnpmCommand[0], pnpmCommand[1], {
    cwd: repositoryRoot,
    encoding: "utf8",
  });
  if (pnpmResult.status !== 0) throw new Error("cannot resolve pnpm version");
  const endedAt = new Date().toISOString();
  const results = {
    schema_version: "sapphirus.bmad100.meta-test-results/v1",
    task_id: "QAL-01",
    command: "node --test --test-reporter=tap tools/bmad100/tests/source-closure-meta.test.mjs",
    started_at: startedAt,
    ended_at: endedAt,
    toolchain: {
      node: process.version.replace(/^v/u, ""),
      pnpm: pnpmResult.stdout.trim(),
    },
    aggregate: { total, passed, failed, exit_code: suite.status },
    tests: META_TEST_CASES.map((entry) => ({ ...entry, passed: true })),
    tap: {
      path: "artifacts/bmad100/quality/qal-01/logs/meta-tests.tap",
      sha256: sha256(output),
    },
    meta_test_sha256: sha256(await readFile(testPath)),
  };
  await writeFile(resultsPath, stableJson(results), "utf8");

  const publisher = spawnSync(process.execPath, [publisherPath, "--results", resultsPath], {
    cwd: repositoryRoot,
    encoding: "utf8",
  });
  process.stdout.write(publisher.stdout ?? "");
  process.stderr.write(publisher.stderr ?? "");
  if (publisher.status !== 0) process.exitCode = publisher.status ?? 1;
}

main().catch((error) => {
  process.stderr.write(`QAL-01 meta-test runner failed: ${error.message}\n`);
  process.exitCode = 1;
});
