import assert from "node:assert/strict";
import test from "node:test";
import { fixtureRoot, runScript, write, writeJson } from "./test-helpers.mjs";

test("a plan alone cannot prove an implemented claim", () => {
  const root = fixtureRoot("claims-plan");
  write(root, "docs/plans/active/plan.md", "# Plan\n");
  writeJson(root, "docs/_meta/claims.json", {
    schemaVersion: "forge.knowledge.v2",
    claims: [
      {
        id: "FORGE-TEST-001",
        subject: "feature.test",
        class: "implemented",
        statement: "The feature is implemented.",
        status: "current",
        ownerRole: "forge-maintainers",
        verifiedAt: "2026-07-31",
        reviewDue: "2099-01-01",
        evidence: [{ kind: "repository", path: "docs/plans/active/plan.md" }],
        supersedes: [],
      },
    ],
  });

  const result = runScript("verify-claims.mjs", ["--root", root, "--offline"]);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /implemented claim relies only on plan or review prose/i);
});

test("archive mode reports repository commits as not verified instead of missing source", () => {
  const root = fixtureRoot("claims-archive");
  write(root, "src/proof.txt", "proof\n");
  writeJson(root, "docs/_meta/claims.json", {
    schemaVersion: "forge.knowledge.v2",
    claims: [
      {
        id: "FORGE-TEST-002",
        subject: "feature.test",
        class: "implemented",
        statement: "The feature is implemented.",
        status: "current",
        ownerRole: "forge-maintainers",
        verifiedAt: "2026-07-31",
        reviewDue: "2099-01-01",
        evidence: [
          {
            kind: "repository",
            path: "src/proof.txt",
            commit: "1111111111111111111111111111111111111111"
          }
        ],
        supersedes: [],
      },
    ],
  });

  const result = runScript("verify-claims.mjs", [
    "--root",
    root,
    "--offline",
    "--archive-mode",
  ]);
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /not_verified_in_archive/i);
  assert.doesNotMatch(result.stdout, /missing_source/i);
});

test("gate evidence must use a manifest-allowlisted command", () => {
  const root = fixtureRoot("claims-gate");
  writeJson(root, "docs/_meta/knowledge-manifest.json", {
    schemaVersion: "forge.knowledge.v2",
    allowedGates: ["node safe-check.mjs"],
  });
  writeJson(root, "docs/_meta/claims.json", {
    schemaVersion: "forge.knowledge.v2",
    claims: [
      {
        id: "FORGE-TEST-003",
        subject: "feature.test",
        class: "implemented",
        statement: "The feature is implemented.",
        status: "current",
        ownerRole: "forge-maintainers",
        verifiedAt: "2026-07-31",
        reviewDue: "2099-01-01",
        evidence: [{ kind: "gate", command: "node unsafe-check.mjs", expectedExit: 0 }],
        supersedes: [],
      },
    ],
  });

  const result = runScript("verify-claims.mjs", ["--root", root, "--offline"]);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /gate command is not allowlisted/i);
});
