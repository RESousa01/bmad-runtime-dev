import { spawnSync } from "node:child_process";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import {
  EXPECTED_META_TEST_COUNT,
  META_TEST_CASES,
  requireExactMetaInventory,
  sha256,
  stableJson,
} from "./lib/qal-01-meta-contract.mjs";

const repositoryRoot = resolve(import.meta.dirname, "../..");
const defaults = {
  closure: resolve(repositoryRoot, "artifacts/bmad100/source-closure.json"),
  report: resolve(repositoryRoot, "artifacts/bmad100/quality/qal-01/report.json"),
  verificationLog: resolve(
    repositoryRoot,
    "artifacts/bmad100/quality/qal-01/logs/verification.json",
  ),
  results: resolve(
    repositoryRoot,
    "artifacts/bmad100/quality/qal-01/logs/meta-test-results.json",
  ),
  receipt: resolve(
    repositoryRoot,
    "artifacts/bmad100/quality/qal-01/logs/meta-tests.json",
  ),
};

function parseArgs(argv) {
  const args = { ...defaults };
  const names = new Map([
    ["--closure", "closure"],
    ["--report", "report"],
    ["--verification-log", "verificationLog"],
    ["--results", "results"],
    ["--receipt", "receipt"],
  ]);
  for (let index = 0; index < argv.length; index += 2) {
    const key = names.get(argv[index]);
    if (!key || !argv[index + 1]) {
      throw new Error(`unknown or incomplete argument ${argv[index] ?? "<missing>"}`);
    }
    args[key] = resolve(argv[index + 1]);
  }
  return args;
}

async function readJson(path, label) {
  try {
    return JSON.parse(await readFile(path, "utf8"));
  } catch (error) {
    throw new Error(`cannot read ${label} at ${path}: ${error.message}`);
  }
}

function command(name, args) {
  const result = spawnSync(name, args, { cwd: repositoryRoot, encoding: "utf8" });
  if (result.status !== 0) {
    throw new Error(result.stderr || result.stdout || `${name} failed`);
  }
  return result.stdout.trim();
}

async function failClosed(args) {
  await rm(args.receipt, { force: true });
  try {
    const report = await readJson(args.report, "QAL-01 report");
    report.status = "pending_meta_tests";
    report.complete = false;
    delete report.meta_tests;
    delete report.command_results;
    await writeFile(args.report, stableJson(report), "utf8");
  } catch {
    // A missing or malformed report is itself fail-closed.
  }
}

async function currentReceiptCandidate(closure) {
  const rows = [];
  for (const row of closure.freshness?.verification_input_tree?.inputs ?? []) {
    if (row.path.startsWith("archive:")) {
      const sourceId = row.path.slice("archive:".length);
      const source = closure.pinned_sources.find((item) => item.id === sourceId);
      if (!source) throw new Error(`missing archive evidence for ${sourceId}`);
      rows.push({ path: row.path, sha256: sha256(await readFile(source.archive_path)) });
    } else {
      rows.push({
        path: row.path,
        sha256: sha256(await readFile(resolve(repositoryRoot, row.path))),
      });
    }
  }
  const archives = [];
  for (const source of closure.pinned_sources ?? []) {
    archives.push([source.id, sha256(await readFile(source.archive_path))]);
  }
  archives.sort(([left], [right]) => left.localeCompare(right));
  const pnpm =
    process.platform === "win32"
      ? command("cmd.exe", ["/d", "/s", "/c", "pnpm --version"])
      : command("pnpm", ["--version"]);
  const body = {
    schema_version: "sapphirus.bmad100.qal-01-receipt-candidate/v1",
    head_commit: command("git", ["rev-parse", "HEAD"]),
    qal_input_tree_sha256: sha256(stableJson(rows)),
    toolchain: {
      node: process.version.replace(/^v/u, ""),
      pnpm,
    },
    archives: Object.fromEntries(archives),
    archive_manifests: {
      method: sha256(
        await readFile(resolve(repositoryRoot, "tools/bmad100/manifests/method-6.10.0.json")),
      ),
      builder: sha256(
        await readFile(resolve(repositoryRoot, "tools/bmad100/manifests/builder-2.1.0.json")),
      ),
    },
    verifier_sha256: sha256(
      await readFile(resolve(repositoryRoot, "tools/bmad100/verify-source-closure.mjs")),
    ),
    meta_test_sha256: sha256(
      await readFile(resolve(repositoryRoot, "tools/bmad100/tests/source-closure-meta.test.mjs")),
    ),
  };
  return { ...body, candidate_sha256: sha256(stableJson(body)) };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  try {
    const results = await readJson(args.results, "meta-test result manifest");
    requireExactMetaInventory(results);
    if (
      results.schema_version !== "sapphirus.bmad100.meta-test-results/v1" ||
      results.command !== "node --test --test-reporter=tap tools/bmad100/tests/source-closure-meta.test.mjs"
    ) {
      throw new Error("meta-test result manifest identity is invalid");
    }
    const testPath = resolve(repositoryRoot, "tools/bmad100/tests/source-closure-meta.test.mjs");
    if (results.meta_test_sha256 !== sha256(await readFile(testPath))) {
      throw new Error("meta-test result manifest is stale");
    }
    if (
      results.tap?.path !== "artifacts/bmad100/quality/qal-01/logs/meta-tests.tap"
    ) {
      throw new Error("meta-test TAP evidence path is not canonical");
    }
    const tapBytes = await readFile(resolve(repositoryRoot, results.tap.path));
    if (results.tap.sha256 !== sha256(tapBytes)) {
      throw new Error("meta-test TAP evidence hash differs from the result manifest");
    }
    const tap = tapBytes.toString("utf8");
    const tapNames = [...tap.matchAll(/^# Subtest: (.+)$/gmu)].map(
      (match) => match[1],
    );
    const expectedNames = META_TEST_CASES.map(({ name }) => name);
    const tapSummary = Object.fromEntries(
      ["tests", "pass", "fail"].map((label) => {
        const match = tap.match(new RegExp(`^# ${label} (\\d+)$`, "mu"));
        return [label, match ? Number.parseInt(match[1], 10) : undefined];
      }),
    );
    if (
      tapSummary.tests !== EXPECTED_META_TEST_COUNT ||
      tapSummary.pass !== EXPECTED_META_TEST_COUNT ||
      tapSummary.fail !== 0 ||
      JSON.stringify(tapNames) !== JSON.stringify(expectedNames)
    ) {
      throw new Error("TAP evidence does not independently prove the exact 14/14 inventory");
    }

    const closure = await readJson(args.closure, "source closure");
    if (closure.complete !== true) throw new Error("source closure is incomplete");
    const currentCandidate = await currentReceiptCandidate(closure);
    if (
      currentCandidate.candidate_sha256 !== closure.receipt_candidate?.candidate_sha256 ||
      stableJson(currentCandidate) !== stableJson(closure.receipt_candidate)
    ) {
      throw new Error("fresh receipt identity differs from the source-closure candidate");
    }
    if (stableJson(results.toolchain) !== stableJson(currentCandidate.toolchain)) {
      throw new Error("meta-test result toolchain differs from the fresh closure candidate");
    }
    const report = await readJson(args.report, "QAL-01 report");
    const verificationLog = await readJson(args.verificationLog, "QAL-01 verification log");
    if (report.complete !== false || report.status !== "pending_meta_tests") {
      throw new Error("QAL-01 report was not in the required fail-closed pending state");
    }
    if (verificationLog.task_complete !== false) {
      throw new Error("QAL-01 verification log was already promoted");
    }

    const receipt = {
      schema_version: "sapphirus.bmad100.meta-test-log/v1",
      task_id: "QAL-01",
      command: "pnpm verify:bmad:source-meta-tests",
      exit_code: 0,
      started_at: results.started_at,
      ended_at: results.ended_at,
      toolchain: results.toolchain,
      aggregate: results.aggregate,
      tests: results.tests,
      results_manifest: {
        path: "artifacts/bmad100/quality/qal-01/logs/meta-test-results.json",
        sha256: sha256(await readFile(args.results)),
      },
      receipt_candidate: currentCandidate,
      promotion: {
        exact_inventory_match: true,
        identity_match: true,
      },
    };
    await mkdir(dirname(args.receipt), { recursive: true });
    await writeFile(args.receipt, stableJson(receipt), "utf8");

    const runState = await readJson(
      resolve(repositoryRoot, ".bmad100/run-state.json"),
      "BMAD-100 run state",
    );
    report.schema_version = "sapphirus.bmad.gate-evidence/v1";
    report.gate_ids = ["source_identity", "source_closure"];
    report.commit = currentCandidate.head_commit;
    report.source_tree = runState.repository?.worktree_tree;
    report.status = "pass";
    report.complete = true;
    report.commands = [
      {
        command:
          "node tools/bmad100/verify-source-closure.mjs --strict --json artifacts/bmad100/source-closure.json",
        exit_code: 0,
        required_for_pass: true,
      },
      {
        command: "pnpm verify:bmad:source-meta-tests",
        exit_code: 0,
        required_for_pass: true,
      },
    ];
    report.evidence_hashes = {
      source_closure: sha256(await readFile(args.closure)),
      meta_test_receipt: sha256(await readFile(args.receipt)),
    };
    report.meta_tests = {
      status: "pass",
      total: 14,
      passed: 14,
      failed: 0,
      evidence: "artifacts/bmad100/quality/qal-01/logs/meta-tests.json",
      results: "artifacts/bmad100/quality/qal-01/logs/meta-test-results.json",
      receipt_candidate_sha256: currentCandidate.candidate_sha256,
    };
    report.command_results = [
      {
        command: "pnpm verify:bmad:source-closure",
        exit_code: 0,
        evidence: "artifacts/bmad100/source-closure.json",
      },
      {
        command: "pnpm verify:bmad:source-meta-tests",
        exit_code: 0,
        evidence: "artifacts/bmad100/quality/qal-01/logs/meta-tests.json",
      },
    ];
    report.evidence = [
      ...new Set([
        ...(report.evidence ?? []),
        "artifacts/bmad100/quality/qal-01/logs/meta-test-results.json",
        "artifacts/bmad100/quality/qal-01/logs/meta-tests.json",
      ]),
    ];
    await writeFile(args.report, stableJson(report), "utf8");

    verificationLog.command_results = [
      ...(verificationLog.command_results ?? []).filter(
        ({ command: value }) => value !== "pnpm verify:bmad:source-meta-tests",
      ),
      {
        command: "pnpm verify:bmad:source-meta-tests",
        exit_code: 0,
        evidence: "artifacts/bmad100/quality/qal-01/logs/meta-tests.json",
      },
    ];
    verificationLog.task_complete = true;
    delete verificationLog.pending_required_command;
    await writeFile(args.verificationLog, stableJson(verificationLog), "utf8");
    process.stdout.write("QAL-01 meta-test receipt published from exact aggregate 14/14.\n");
  } catch (error) {
    await failClosed(args);
    throw error;
  }
}

main().catch((error) => {
  process.stderr.write(`QAL-01 receipt publication failed: ${error.message}\n`);
  process.exitCode = 1;
});
