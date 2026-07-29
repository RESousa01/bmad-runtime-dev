import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const root = new URL("../", import.meta.url);

async function readJson(path) {
  return JSON.parse(await readFile(new URL(path, root), "utf8"));
}

test("canonical catalog accounts for every pinned Method and Builder row", async () => {
  const [catalog, accounting] = await Promise.all([
    readJson("capability-catalog.json"),
    readJson("source-accounting.json"),
  ]);

  const methodRows = catalog.capabilities.filter((row) => row.source.id === "method");
  const builderRows = catalog.capabilities.filter((row) => row.source.id === "builder");

  assert.equal(methodRows.length, 48);
  assert.equal(builderRows.length, 10);
  assert.equal(methodRows.length, accounting.totals.method_primary_skills);
  assert.equal(builderRows.filter((row) => row.visibility === "public").length, 9);
  assert.equal(builderRows.filter((row) => row.visibility === "internal").length, 1);
});

test("aliases are unambiguous, governed migrations to catalog implementations", async () => {
  const [catalog, aliasCatalog] = await Promise.all([
    readJson("capability-catalog.json"),
    readJson("compatibility-aliases.json"),
  ]);
  const canonicalIds = new Set(catalog.capabilities.map((row) => row.canonicalId));
  const aliases = new Set();

  for (const alias of aliasCatalog.aliases) {
    assert.ok(alias.alias);
    assert.ok(alias.target);
    assert.ok(alias.migrationPolicy);
    assert.ok(!aliases.has(alias.alias), `duplicate alias ${alias.alias}`);
    assert.ok(canonicalIds.has(alias.target), `unknown target ${alias.target}`);
    aliases.add(alias.alias);
  }
});

test("generated Rust and TypeScript projections are current", async () => {
  const catalog = await readJson("capability-catalog.json");
  const [rust, typescript] = await Promise.all([
    readFile(new URL("../../crates/desktop-app/src/generated/bmad_capabilities.rs", root), "utf8"),
    readFile(new URL("../../apps/desktop-ui/src/generated/bmadCapabilities.ts", root), "utf8"),
  ]);

  assert.match(rust, new RegExp(`CAPABILITY_ROW_COUNT: usize = ${catalog.capabilities.length}`));
  assert.match(typescript, new RegExp(`capabilityRowCount = ${catalog.capabilities.length}`));
  assert.match(typescript, /builderPublicActionRowCount = 9/);
  assert.match(typescript, /builderInternalActionRowCount = 1/);
});
