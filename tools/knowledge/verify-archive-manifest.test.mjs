import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";
import { fixtureRoot, runScript, write, writeJson } from "./test-helpers.mjs";

test("archive verification detects a changed preserved note", () => {
  const root = fixtureRoot("archive");
  const original = "preserved bytes\n";
  write(root, "docs/archive/preserved/note.md", original);
  writeJson(root, "bmad-runtime-lib/archive-manifest.json", {
    schemaVersion: "forge.archive-manifest.v1",
    files: [
      {
        originalPath: "note.md",
        archivePath: "docs/archive/preserved/note.md",
        sha256: createHash("sha256").update(original).digest("hex"),
      },
    ],
  });

  let result = runScript("verify-archive-manifest.mjs", ["--root", root]);
  assert.equal(result.status, 0, result.stderr);

  write(root, "docs/archive/preserved/note.md", "changed bytes\n");
  result = runScript("verify-archive-manifest.mjs", ["--root", root]);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /archive hash mismatch/i);
});

