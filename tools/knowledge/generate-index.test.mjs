import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";
import { fixtureRoot, maintainedDoc, runScript, write, writeJson } from "./test-helpers.mjs";

test("adding a maintained note requires manifest data but no generator source change", () => {
  const root = fixtureRoot("index");
  write(root, "docs/current/one.md", maintainedDoc({ title: "One" }));
  write(root, "docs/current/two.md", maintainedDoc({ title: "Two" }));
  writeJson(root, "docs/_meta/knowledge-manifest.json", {
    schemaVersion: "forge.knowledge.v2",
    ownerRoles: ["forge-maintainers"],
    documents: ["docs/current/one.md", "docs/current/two.md"],
  });
  const output = join(root, "docs/_meta/note-index.generated.json");

  const result = runScript("generate-index.mjs", ["--root", root, "--output", output]);
  assert.equal(result.status, 0, result.stderr);
  const index = JSON.parse(readFileSync(output, "utf8"));
  assert.deepEqual(
    index.documents.map((document) => document.title),
    ["One", "Two"],
  );
});

test("index generation rejects duplicate current titles", () => {
  const root = fixtureRoot("index-duplicate");
  write(root, "docs/current/one.md", maintainedDoc({ title: "Duplicate" }));
  write(root, "docs/current/two.md", maintainedDoc({ title: "Duplicate" }));
  writeJson(root, "docs/_meta/knowledge-manifest.json", {
    schemaVersion: "forge.knowledge.v2",
    ownerRoles: ["forge-maintainers"],
    documents: ["docs/current/one.md", "docs/current/two.md"],
  });

  const result = runScript("generate-index.mjs", ["--root", root]);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /duplicate current title/i);
});

