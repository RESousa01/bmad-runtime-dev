import assert from "node:assert/strict";
import test from "node:test";
import { fixtureRoot, maintainedDoc, runScript, write, writeJson } from "./test-helpers.mjs";

test("a broken standard Markdown link in the team entrypoint fails", () => {
  const root = fixtureRoot("docs-link");
  write(
    root,
    "docs/README.md",
    maintainedDoc({ title: "Forge team documentation", body: "[Missing](current/missing.md)" }),
  );
  writeJson(root, "docs/_meta/knowledge-manifest.json", {
    schemaVersion: "forge.knowledge.v2",
    ownerRoles: ["forge-maintainers"],
    documents: ["docs/README.md"],
  });

  const result = runScript("check-docs.mjs", ["--root", root]);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /broken markdown link/i);
});

test("archived original-language prose is excluded from current-language checks", () => {
  const root = fixtureRoot("docs-archive");
  write(root, "docs/archive/old.md", "# Sapphirus historical note\n\nOriginal text.\n");
  writeJson(root, "docs/_meta/knowledge-manifest.json", {
    schemaVersion: "forge.knowledge.v2",
    ownerRoles: ["forge-maintainers"],
    documents: [],
    archives: [{ path: "docs/archive/old.md", sha256: "ignored-by-doc-check" }],
  });

  const result = runScript("check-docs.mjs", ["--root", root]);
  assert.equal(result.status, 0, result.stderr);
});

test("a source_commit_from pointer must resolve to an exact commit", () => {
  const root = fixtureRoot("docs-pointer");
  write(
    root,
    "docs/current/state.md",
    maintainedDoc({
      title: "State",
      extra:
        'source_commit_from: "docs/provenance/baseline.json#/implementationWorktree/currentHead"\n',
    }).replace('source_commit: "1111111111111111111111111111111111111111"\n', ""),
  );
  writeJson(root, "docs/provenance/baseline.json", {
    implementationWorktree: { currentHead: "not-a-commit" },
  });
  writeJson(root, "docs/_meta/knowledge-manifest.json", {
    schemaVersion: "forge.knowledge.v2",
    ownerRoles: ["forge-maintainers"],
    documents: ["docs/current/state.md"],
  });

  const result = runScript("check-docs.mjs", ["--root", root]);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /does not resolve to an exact commit/i);
});

test("a superseded document path must remain recoverable", () => {
  const root = fixtureRoot("docs-supersedes");
  write(
    root,
    "docs/current/state.md",
    maintainedDoc({ title: "State" }).replace(
      "supersedes: []",
      "supersedes:\n  - docs/archive/missing.md",
    ),
  );
  writeJson(root, "docs/_meta/knowledge-manifest.json", {
    schemaVersion: "forge.knowledge.v2",
    ownerRoles: ["forge-maintainers"],
    documents: ["docs/current/state.md"],
  });

  const result = runScript("check-docs.mjs", ["--root", root]);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /broken supersedes path/i);
});
