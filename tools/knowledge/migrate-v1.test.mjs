import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";
import { fixtureRoot, runScript, writeJson } from "./test-helpers.mjs";

test("v1 claims are never promoted to current without fresh evidence", () => {
  const root = fixtureRoot("migrate");
  writeJson(root, "bmad-runtime-lib/knowledge-base/evidence/claims.json", {
    schemaVersion: "sapphirus.living-knowledge.v1",
    claims: [{ id: "OLD-1", subject: "product.name", statement: "Old statement" }],
  });
  writeJson(root, "bmad-runtime-lib/knowledge-base/evidence/sources.json", {
    schemaVersion: "sapphirus.living-knowledge.v1",
    sources: [],
  });
  writeJson(root, "bmad-runtime-lib/knowledge-base/evidence/note-catalog.json", {
    schemaVersion: "sapphirus.living-knowledge.v1",
    notes: [],
  });
  const outputDir = join(root, "docs/_meta");

  const result = runScript("migrate-v1.mjs", ["--root", root, "--output-dir", outputDir]);
  assert.equal(result.status, 0, result.stderr);
  const claims = JSON.parse(readFileSync(join(outputDir, "claims.migrated.json"), "utf8"));
  assert.equal(claims.schemaVersion, "forge.knowledge.v2");
  assert.equal(claims.claims[0].status, "needs_reverification");
  assert.equal(claims.claims[0].class, "unknown");
});

