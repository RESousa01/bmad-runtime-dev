// Merges per-skill compatibility decisions into the shared ledger.
//
// Forty-eight MTH tasks each own exactly one Method skill, but they all
// record their decision in one `compatibility-ledger.json`. Concurrent
// writers to a single JSON file lose updates, so each task instead writes a
// fragment named for its own row and this script folds them in. The ledger
// stays the single artifact everything else reads; only the write path is
// partitioned.
//
// A fragment may only modify the row it is named for, and may only set the
// decision fields. Identity — rowId, canonicalId, sourcePath, sourceSha256 —
// comes from the catalog-seeded ledger and a fragment cannot rewrite it,
// because a task that could restate its own source hash could also hide a
// drift from the pinned source.

import { readFile, readdir, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const ledgerPath = path.join(packageRoot, "compatibility-ledger.json");
const fragmentsRoot = path.join(packageRoot, "ledger-fragments");

const DECISION_FIELDS = new Set([
  "decision",
  "supportState",
  "owner",
  "rationale",
  "semanticDiffRef",
  "supportedVersions",
  "migrationPolicy",
  "deprecationPolicy",
  "canonicalTarget",
]);

const DECISIONS = new Set(["adopt", "adapt", "shim", "pending"]);
const SUPPORT_STATES = new Set(["exact", "adapted", "shim", "unavailable"]);

function fail(message) {
  process.stderr.write(`merge-ledger-fragments: ${message}\n`);
  process.exit(1);
}

async function readJson(file) {
  return JSON.parse(await readFile(file, "utf8"));
}

/// A fragment must justify itself: an `adapt` or `shim` decision without an
/// owner and rationale is an undocumented divergence from the pinned source,
/// which is exactly what the ledger exists to prevent.
function validateFragment(fragment, row) {
  const problems = [];
  if (!DECISIONS.has(fragment.decision)) {
    problems.push(`decision ${String(fragment.decision)} is not a known decision`);
  }
  if (!SUPPORT_STATES.has(fragment.supportState)) {
    problems.push(`supportState ${String(fragment.supportState)} is not a known state`);
  }
  if (fragment.decision === "adopt" && fragment.supportState !== "exact") {
    problems.push("an adopted skill must report support state exact");
  }
  if (fragment.decision === "shim" && fragment.supportState !== "shim") {
    problems.push("a shim must report support state shim");
  }
  if (fragment.decision === "shim" && !fragment.canonicalTarget) {
    problems.push("a shim must name the canonical target it forwards to");
  }
  if ((fragment.decision === "adapt" || fragment.decision === "shim")
    && (!fragment.owner || !fragment.rationale)) {
    problems.push(`a ${fragment.decision} decision requires an owner and a rationale`);
  }
  for (const key of Object.keys(fragment)) {
    if (key !== "rowId" && !DECISION_FIELDS.has(key)) {
      problems.push(`fragment may not set ${key}; identity comes from the catalog`);
    }
  }
  if (fragment.rowId !== row.rowId) {
    problems.push(`fragment rowId does not match the row it names`);
  }
  return problems;
}

async function main() {
  const check = process.argv.slice(2).includes("--check");
  const ledger = await readJson(ledgerPath);
  const byRowId = new Map(ledger.rows.map((row) => [row.rowId, row]));

  if (!existsSync(fragmentsRoot)) {
    process.stdout.write("no ledger fragments present; ledger unchanged\n");
    return;
  }

  const files = (await readdir(fragmentsRoot)).filter((name) => name.endsWith(".json")).sort();
  const problems = [];
  let applied = 0;

  for (const file of files) {
    const fragment = await readJson(path.join(fragmentsRoot, file));
    const row = byRowId.get(fragment.rowId);
    if (!row) {
      problems.push(`${file}: rowId ${String(fragment.rowId)} is not in the ledger`);
      continue;
    }
    const fragmentProblems = validateFragment(fragment, row);
    if (fragmentProblems.length > 0) {
      problems.push(...fragmentProblems.map((problem) => `${file}: ${problem}`));
      continue;
    }
    for (const key of Object.keys(fragment)) {
      if (key === "rowId") continue;
      row[key] = fragment[key];
    }
    applied += 1;
  }

  if (problems.length > 0) {
    fail(`${problems.length} fragment problem(s):\n  ${problems.join("\n  ")}`);
  }

  const pending = ledger.rows.filter((row) => row.decision === "pending").length;
  const serialized = `${JSON.stringify(ledger, null, 2)}\n`;

  if (check) {
    const current = await readFile(ledgerPath, "utf8");
    if (current !== serialized) {
      fail("ledger is stale; run without --check to fold the fragments in");
    }
    process.stdout.write(`ledger current: ${applied} fragment(s), ${pending} row(s) still pending\n`);
    return;
  }

  await writeFile(ledgerPath, serialized);
  process.stdout.write(`ledger updated: ${applied} fragment(s) applied, ${pending} row(s) still pending\n`);
}

await main();
