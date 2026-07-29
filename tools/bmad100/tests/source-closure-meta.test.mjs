import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, mkdir, readFile, writeFile } from "node:fs/promises";
import { homedir, tmpdir } from "node:os";
import { dirname, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

const repositoryRoot = resolve(import.meta.dirname, "../../..");
const verifier = resolve(repositoryRoot, "tools/bmad100/verify-source-closure.mjs");
const foundationRoot = resolve(repositoryRoot, "packages/bmad-foundation");
const methodArchive =
  process.env.BMAD_METHOD_ARCHIVE ??
  resolve(homedir(), "Downloads", "BMAD-METHOD-main.zip");
const builderArchive =
  process.env.BMAD_BUILDER_ARCHIVE ??
  resolve(homedir(), "Downloads", "bmad-builder-main.zip");
const canonicalResult = resolve(repositoryRoot, "artifacts/bmad100/source-closure.json");
const canonicalReport = resolve(
  repositoryRoot,
  "artifacts/bmad100/quality/qal-01/report.json",
);
const canonicalMetaLog = resolve(
  repositoryRoot,
  "artifacts/bmad100/quality/qal-01/logs/meta-tests.json",
);
const publisher = resolve(
  repositoryRoot,
  "tools/bmad100/publish-source-closure-receipt.mjs",
);

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function stableJson(value) {
  return `${JSON.stringify(value, null, 2)}\n`;
}

function requireMatchingReceiptCandidate(expected, actual) {
  assert.equal(
    actual.candidate_sha256,
    expected.candidate_sha256,
    "meta receipt candidate does not match the fresh closure candidate",
  );
  assert.deepEqual(
    actual,
    expected,
    "meta receipt identity fields differ from the fresh closure candidate",
  );
}

async function temporaryCase() {
  const root = await mkdtemp(resolve(tmpdir(), "bmad-source-closure-"));
  return {
    root,
    manifestDir: resolve(root, "manifests"),
    report: resolve(root, "report.json"),
    result: resolve(root, "source-closure.json"),
    overlay: resolve(root, "overlay"),
  };
}

function run(args) {
  return spawnSync(
    process.execPath,
    [
      verifier,
      "--strict",
      "--foundation-root",
      foundationRoot,
      "--method-archive",
      methodArchive,
      "--builder-archive",
      builderArchive,
      ...args,
    ],
    { cwd: repositoryRoot, encoding: "utf8" },
  );
}

async function mutateJson(source, destination, mutate) {
  const value = JSON.parse(await readFile(source, "utf8"));
  mutate(value);
  await mkdir(dirname(destination), { recursive: true });
  await writeFile(destination, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

test("canonical pinned source closure is complete", async () => {
  const result = run([]);
  assert.equal(result.status, 0, result.stderr || result.stdout);
  const report = JSON.parse(await readFile(canonicalResult, "utf8"));
  assert.equal(report.complete, true);
  const taskReport = JSON.parse(await readFile(canonicalReport, "utf8"));
  assert.equal(taskReport.complete, false);
  assert.equal(taskReport.status, "pending_meta_tests");
  await assert.rejects(
    readFile(canonicalMetaLog, "utf8"),
    (error) => error?.code === "ENOENT",
    "a closure rerun must remove the prior meta receipt before promotion",
  );
  assert.deepEqual(report.denominators, {
    archive_members: 896,
    method_archive_members: 597,
    builder_archive_members: 299,
    method_primary_skills: 48,
    builder_primary_skills: 5,
    builder_public_actions: 9,
    builder_internal_actions: 1,
  });
});

test("deleting a source denominator row fails closed", async () => {
  const fixture = await temporaryCase();
  const source = resolve(foundationRoot, "source-manifest.json");
  const mutated = resolve(fixture.root, "source-manifest.json");
  await mutateJson(source, mutated, (value) => value.members.splice(0, 1));
  const result = run([
    "--source-manifest",
    mutated,
    "--json",
    fixture.result,
    "--report",
    fixture.report,
    "--manifest-dir",
    fixture.manifestDir,
  ]);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /member|denominator|count|missing/iu);
});

test("changing one required source byte fails its pinned hash", async () => {
  const fixture = await temporaryCase();
  const member = "src/bmm-skills/1-analysis/bmad-agent-analyst/SKILL.md";
  const source = resolve(foundationRoot, "source/method/6.10.0", member);
  const target = resolve(fixture.overlay, "method", member);
  await mkdir(dirname(target), { recursive: true });
  await writeFile(target, `${await readFile(source, "utf8")}\nmutated\n`, "utf8");
  const result = run([
    "--source-overlay",
    fixture.overlay,
    "--json",
    fixture.result,
    "--report",
    fixture.report,
    "--manifest-dir",
    fixture.manifestDir,
  ]);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /hash|byte|drift/iu);
});

test("removing a Builder action fails catalog closure", async () => {
  const fixture = await temporaryCase();
  const source = resolve(foundationRoot, "source-accounting.json");
  const mutated = resolve(fixture.root, "source-accounting.json");
  await mutateJson(source, mutated, (value) => value.builder_actions.pop());
  const result = run([
    "--source-accounting",
    mutated,
    "--json",
    fixture.result,
    "--report",
    fixture.report,
    "--manifest-dir",
    fixture.manifestDir,
  ]);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /action|catalog|count/iu);
});

test("removing a Builder sample row fails complete classification", async () => {
  const fixture = await temporaryCase();
  const source = resolve(foundationRoot, "builder-source-ledger.json");
  const mutated = resolve(fixture.root, "builder-source-ledger.json");
  await mutateJson(source, mutated, (value) => {
    const index = value.members.findIndex((member) => member.classification === "sample_fixture");
    assert.notEqual(index, -1);
    value.members.splice(index, 1);
  });
  const result = run([
    "--builder-ledger",
    mutated,
    "--json",
    fixture.result,
    "--report",
    fixture.report,
    "--manifest-dir",
    fixture.manifestDir,
  ]);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /sample|ledger|member|classification/iu);
});

test("deleting a Builder resource edge fails fresh graph closure", async () => {
  const fixture = await temporaryCase();
  const source = resolve(foundationRoot, "builder-source-ledger.json");
  const mutated = resolve(fixture.root, "builder-source-ledger.json");
  await mutateJson(source, mutated, (value) => value.resource_edges.pop());
  const result = run([
    "--builder-ledger",
    mutated,
    "--json",
    fixture.result,
    "--report",
    fixture.report,
    "--manifest-dir",
    fixture.manifestDir,
  ]);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /graph|edge|ledger|drift/iu);
});

test("modifying a Builder resource edge target fails resolution", async () => {
  const fixture = await temporaryCase();
  const source = resolve(foundationRoot, "builder-source-ledger.json");
  const mutated = resolve(fixture.root, "builder-source-ledger.json");
  await mutateJson(source, mutated, (value) => {
    value.resource_edges[0].to = "skills/missing-resource.md";
  });
  const result = run([
    "--builder-ledger",
    mutated,
    "--json",
    fixture.result,
    "--report",
    fixture.report,
    "--manifest-dir",
    fixture.manifestDir,
  ]);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /graph|edge|ledger|missing|unresolved/iu);
});

test("mutating a generated-package target fails its reviewed inventory contract", async () => {
  const fixture = await temporaryCase();
  const source = resolve(
    repositoryRoot,
    "tools/bmad100/manifests/generated-output-targets.json",
  );
  const mutated = resolve(fixture.root, "generated-output-targets.json");
  await mutateJson(source, mutated, (value) => {
    value.targets[0].target_uri =
      "generated-package://bmad-agent-builder/references/unknown.md";
    value.targets[0].target_kind = "file";
    value.targets[0].target_identity_sha256 = sha256(value.targets[0].target_uri);
  });
  const result = run([
    "--generated-output-inventory",
    mutated,
    "--json",
    fixture.result,
    "--report",
    fixture.report,
    "--manifest-dir",
    fixture.manifestDir,
  ]);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /generated-package|target|inventory|contract|orphan/iu);
});

test("breaking a Builder consumer fails resolved consumer closure", async () => {
  const fixture = await temporaryCase();
  const source = resolve(foundationRoot, "builder-source-ledger.json");
  const mutated = resolve(fixture.root, "builder-source-ledger.json");
  await mutateJson(source, mutated, (value) => {
    const member = value.members.find((row) => row.classification === "production_source");
    member.consumers = ["unresolved_consumer"];
  });
  const result = run([
    "--builder-ledger",
    mutated,
    "--json",
    fixture.result,
    "--report",
    fixture.report,
    "--manifest-dir",
    fixture.manifestDir,
  ]);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /consumer|graph|ledger/iu);
});

test("duplicating a Builder member fails canonical identity closure", async () => {
  const fixture = await temporaryCase();
  const source = resolve(foundationRoot, "builder-source-ledger.json");
  const mutated = resolve(fixture.root, "builder-source-ledger.json");
  await mutateJson(source, mutated, (value) => value.members.push(value.members[0]));
  const result = run([
    "--builder-ledger",
    mutated,
    "--json",
    fixture.result,
    "--report",
    fixture.report,
    "--manifest-dir",
    fixture.manifestDir,
  ]);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /duplicate|member|ledger|graph/iu);
});

test("falsely production-counting a sample fails accounting closure", async () => {
  const fixture = await temporaryCase();
  const source = resolve(foundationRoot, "source-accounting.json");
  const mutated = resolve(fixture.root, "source-accounting.json");
  await mutateJson(source, mutated, (value) => {
    const member = value.members.find(
      (row) => row.source_id === "builder" && row.path.startsWith("samples/"),
    );
    member.runtime_denominator = true;
    member.target_capability = "builder:false-production-row";
    member.implementation_task = "QAL-01";
  });
  const result = run([
    "--source-accounting",
    mutated,
    "--json",
    fixture.result,
    "--report",
    fixture.report,
    "--manifest-dir",
    fixture.manifestDir,
  ]);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /false production|production-counted|accounting/iu);
});

test("orphaning an accounting member fails archive closure", async () => {
  const fixture = await temporaryCase();
  const source = resolve(foundationRoot, "source-accounting.json");
  const mutated = resolve(fixture.root, "source-accounting.json");
  await mutateJson(source, mutated, (value) => value.members.pop());
  const result = run([
    "--source-accounting",
    mutated,
    "--json",
    fixture.result,
    "--report",
    fixture.report,
    "--manifest-dir",
    fixture.manifestDir,
  ]);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /accounting|member|orphan|count/iu);
});

test("a stale or mismatched meta receipt cannot promote QAL-01", async () => {
  const closure = JSON.parse(await readFile(canonicalResult, "utf8"));
  const stale = structuredClone(closure.receipt_candidate);
  stale.qal_input_tree_sha256 = "0".repeat(64);
  stale.candidate_sha256 = sha256(
    stableJson(
      Object.fromEntries(
        Object.entries(stale).filter(([key]) => key !== "candidate_sha256"),
      ),
    ),
  );
  assert.throws(
    () => requireMatchingReceiptCandidate(closure.receipt_candidate, stale),
    /does not match|differ/iu,
  );
});

test("a failed aggregate cannot publish a QAL-01 completion receipt", async () => {
  const fixture = await temporaryCase();
  const results = resolve(fixture.root, "meta-test-results.json");
  const receipt = resolve(fixture.root, "meta-tests.json");
  const report = resolve(fixture.root, "report.json");
  const verificationLog = resolve(fixture.root, "verification.json");
  await writeFile(
    results,
    stableJson({
      schema_version: "sapphirus.bmad100.meta-test-results/v1",
      aggregate: { total: 14, passed: 13, failed: 1, exit_code: 1 },
      tests: [],
    }),
    "utf8",
  );
  await writeFile(
    report,
    stableJson({ status: "pending_meta_tests", complete: false }),
    "utf8",
  );
  await writeFile(verificationLog, stableJson({ task_complete: false }), "utf8");
  await writeFile(receipt, stableJson({ stale: true }), "utf8");
  const result = spawnSync(
    process.execPath,
    [
      publisher,
      "--results",
      results,
      "--closure",
      resolve(fixture.root, "unused-closure.json"),
      "--report",
      report,
      "--verification-log",
      verificationLog,
      "--receipt",
      receipt,
    ],
    { cwd: repositoryRoot, encoding: "utf8" },
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /aggregate|14\/14/iu);
  const failedReport = JSON.parse(await readFile(report, "utf8"));
  assert.equal(failedReport.complete, false);
  assert.equal(failedReport.status, "pending_meta_tests");
  await assert.rejects(
    readFile(receipt, "utf8"),
    (error) => error?.code === "ENOENT",
    "a failed aggregate must not leave a completion receipt",
  );
});
