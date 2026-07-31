import assert from "node:assert/strict";
import test from "node:test";
import { fixtureRoot, runScript, writeJson } from "./test-helpers.mjs";

test("a reference source lock with an invalid SHA-256 fails", () => {
  const root = fixtureRoot("sources-hash");
  writeJson(root, "reference-sources.lock.json", {
    schemaVersion: "forge.reference-sources.v1",
    sources: [
      {
        id: "example",
        purpose: "historical comparison",
        repositoryUrl: "https://example.com/repo.git",
        version: "1.0.0",
        commit: "1111111111111111111111111111111111111111",
        archiveSha256: "bad",
        license: "MIT",
        acquiredAt: "2026-07-31T00:00:00Z",
        storageLocator: "artifact://approved/example",
        includedPaths: ["src"],
        excludedPaths: ["website"],
        releaseDecision: "reference_only",
      },
    ],
  });

  const result = runScript("verify-reference-sources.mjs", ["--root", root, "--offline"]);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /invalid archive sha-256/i);
});

