import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import assert from "node:assert/strict";

const testsDir = dirname(fileURLToPath(import.meta.url));
const packageRoot = resolve(testsDir, "..");

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

const catalog = readJson(resolve(packageRoot, "capability-catalog.json"));
const ledger = readJson(resolve(packageRoot, "compatibility-ledger.json"));

function catalogMethodRows() {
  return catalog.capabilities.filter(
    (capability) => capability.source.class === "primary_method_skill",
  );
}

// The ledger is keyed by `rowId`, not `canonicalId`: several source
// SKILL.md files (a v6 shim and the skill it delegates to, for example)
// legitimately share one canonical capability, so `canonicalId` alone
// cannot identify one of the 48 rows the plan requires.
function rowKey(row) {
  return row.rowId;
}

// The same predicate `conformance.mjs --all` applies: a row the 48 MTH tasks
// have not yet decided must fail the gate. Kept inline (rather than
// imported) so this test proves the *ledger data* is honest independent of
// whatever the runner script currently does with it.
function rowPassesGate(row) {
  return row.decision !== "pending" && row.supportState !== "unavailable";
}

test("compatibility ledger has exactly 48 method rows matching the catalog", () => {
  const catalogIds = catalogMethodRows()
    .map((row) => row.rowId)
    .sort();
  assert.equal(catalog.counts.method, 48);
  assert.equal(ledger.rows.length, 48);
  assert.equal(new Set(ledger.rows.map(rowKey)).size, 48);
  const ledgerIds = ledger.rows.map(rowKey).sort();
  assert.deepEqual(ledgerIds, catalogIds);
});

test("every ledger row's source sha256 and canonicalId match the catalog's", () => {
  const byId = new Map(catalogMethodRows().map((row) => [row.rowId, row]));
  for (const row of ledger.rows) {
    const catalogRow = byId.get(row.rowId);
    assert.ok(catalogRow, `${row.rowId} is not a method capability in the catalog`);
    assert.equal(row.canonicalId, catalogRow.canonicalId);
    assert.equal(row.sourcePath, catalogRow.source.path);
    assert.equal(row.sourceSha256, `sha256:${catalogRow.source.sha256}`);
    assert.match(row.sourceSha256, /^sha256:[0-9a-f]{64}$/u);
  }
});

// Break caught: the ledger reporting Phase 2 complete while a row still
// carries the freshly-seeded `pending` decision -- the gate must refuse a
// `pending` row rather than silently treating "not yet decided" as passing.
test("a pending row fails the gate predicate and the seed carries no other decision", () => {
  for (const row of ledger.rows) {
    assert.equal(row.decision, "pending");
    assert.equal(row.supportState, "unavailable");
    assert.equal(rowPassesGate(row), false);
  }
});

test("ledger row ordering is canonical (ascending rowId, no duplicates)", () => {
  const ids = ledger.rows.map(rowKey);
  const sorted = [...ids].sort((left, right) => (left < right ? -1 : left > right ? 1 : 0));
  assert.deepEqual(ids, sorted);
  assert.equal(new Set(ids).size, ids.length);
});
