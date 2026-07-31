import assert from "node:assert/strict";
import test from "node:test";
import { fixtureRoot, maintainedDoc, runScript, write, writeJson } from "./test-helpers.mjs";

function authorityFixture(body) {
  const root = fixtureRoot("authority");
  write(root, "docs/README.md", maintainedDoc({ title: "Forge team documentation" }));
  write(root, "docs/current/product-state.md", maintainedDoc({ title: "Forge product state", body }));
  writeJson(root, "docs/_meta/knowledge-manifest.json", {
    schemaVersion: "forge.knowledge.v2",
    entrypoint: "docs/README.md",
    productState: "docs/current/product-state.md",
    ownerRoles: ["forge-maintainers"],
    documents: ["docs/README.md", "docs/current/product-state.md"],
  });
  return root;
}

test("current prose using the stale product name fails authority validation", () => {
  const result = runScript("check-authority.mjs", [
    "--root",
    authorityFixture("Sapphirus Desktop is the current product."),
  ]);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /stale product name/i);
});

test("current prose making Entra required fails authority validation", () => {
  const result = runScript("check-authority.mjs", [
    "--root",
    authorityFixture("Entra is required for every developer."),
  ]);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /entra requirement/i);
});

test("an explicit legacy identifier table passes", () => {
  const result = runScript("check-authority.mjs", [
    "--root",
    authorityFixture("Legacy identifier allowlist: `sapphirus.contract.v1` remains compatibility data."),
  ]);
  assert.equal(result.status, 0, result.stderr);
});

