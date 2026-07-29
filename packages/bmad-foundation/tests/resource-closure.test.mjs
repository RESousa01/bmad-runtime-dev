import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

import {
  evaluateResourceReferences,
  evaluateSkillResourceClosure,
} from "../scripts/resource-closure.mjs";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const scriptPath = resolve(packageRoot, "scripts", "resource-closure.mjs");
const source = {
  id: "method",
  package_name: "bmad-method",
  version: "6.10.0",
};

function member(path, body) {
  const bytes = Buffer.from(body, "utf8");
  return {
    path,
    bytes,
    contentHash: createHash("sha256").update(bytes).digest("hex"),
  };
}

function seal(memberRecord, literal, disposition) {
  return {
    source_id: source.id,
    package_name: source.package_name,
    version: source.version,
    member_path: memberRecord.path,
    member_sha256: memberRecord.contentHash,
    literal,
    disposition,
  };
}

test("resource closure self-test covers nested Markdown, unpinned remotes, and mandatory-edge denominator", () => {
  const result = spawnSync(process.execPath, [scriptPath, "--self-test"], {
    cwd: packageRoot,
    encoding: "utf8",
    env: { ...process.env, NO_COLOR: "1" },
  });

  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(result.stdout, /resource-closure self-test passed/u);
});

test("valid exact generated-output disposition preserves output evidence outside package closure", () => {
  const skill = member("skills/docs/SKILL.md", "[report](./report.md)\n");
  const result = evaluateResourceReferences({
    source,
    members: [skill],
    referencePaths: [skill.path],
    dispositions: [
      {
        ...seal(skill, "./report.md", "generated_output"),
        output_identity: "docs:report.md",
      },
    ],
  });

  assert.deepEqual(result.missing, []);
  assert.deepEqual(result.invalid, []);
  assert.deepEqual(result.activationBlockers, []);
  assert.deepEqual(
    result.directedEdges.map((edge) => ({
      disposition: edge.disposition,
      output_identity: edge.output_identity,
      to: edge.to,
    })),
    [
      {
        disposition: "generated_output",
        output_identity: "docs:report.md",
        to: null,
      },
    ],
  );
});

test("sealed dispositions reject declaring-member hash and literal drift", () => {
  const skill = member("skills/docs/SKILL.md", "[report](./report.md)\n");
  assert.throws(
    () =>
      evaluateResourceReferences({
        source,
        members: [skill],
        referencePaths: [skill.path],
        dispositions: [
          {
            ...seal(skill, "./report.md", "generated_output"),
            member_sha256: "0".repeat(64),
            output_identity: "docs:report.md",
          },
        ],
      }),
    /member hash drift/u,
  );
  assert.throws(
    () =>
      evaluateResourceReferences({
        source,
        members: [skill],
        referencePaths: [skill.path],
        dispositions: [
          {
            ...seal(skill, "./renamed.md", "generated_output"),
            output_identity: "docs:report.md",
          },
        ],
      }),
    /literal drift/u,
  );
});

test("unknown generated output remains a missing mandatory package resource", () => {
  const skill = member("skills/docs/SKILL.md", "[report](./unknown.md)\n");
  const result = evaluateResourceReferences({
    source,
    members: [skill],
    referencePaths: [skill.path],
  });

  assert.equal(
    result.directedEdges[0].disposition,
    "package_resource_mandatory",
  );
  assert.deepEqual(result.missing, [
    {
      from: skill.path,
      to: "skills/docs/unknown.md",
      literal: "./unknown.md",
      disposition: "package_resource_mandatory",
    },
  ]);
});

test("rejected external authority is evidenced as an activation blocker, not local closure", () => {
  const skill = member(
    "skills/ux/SKILL.md",
    "[authority](https://example.invalid/design.md)\n",
  );
  const result = evaluateResourceReferences({
    source,
    members: [skill],
    referencePaths: [skill.path],
    dispositions: [
      {
        ...seal(
          skill,
          "https://example.invalid/design.md",
          "external_authority_rejected",
        ),
        authority_identity: "example:design.md",
        governance_owner: "MTH-10",
      },
    ],
  });

  assert.equal(result.missing.length, 0);
  assert.equal(result.invalid.length, 0);
  assert.equal(result.activationBlockers.length, 1);
  assert.equal(
    result.activationBlockers[0].disposition,
    "external_authority_rejected",
  );
  assert.equal(result.activationBlockers[0].to, null);
});

test("mandatory denominator includes local package edges but excludes generated outputs and rejected authorities", () => {
  const skill = member(
    "skills/mixed/SKILL.md",
    "[local](./local.md)\n[output](./report.md)\n[authority](https://example.invalid/spec.md)\n",
  );
  const local = member("skills/mixed/local.md", "local\n");
  const result = evaluateResourceReferences({
    source,
    members: [skill, local],
    referencePaths: [skill.path, local.path],
    dispositions: [
      {
        ...seal(skill, "./report.md", "generated_output"),
        output_identity: "mixed:report.md",
      },
      {
        ...seal(
          skill,
          "https://example.invalid/spec.md",
          "external_authority_rejected",
        ),
        authority_identity: "example:spec.md",
        governance_owner: "MTH-10",
      },
    ],
  });

  assert.equal(
    result.directedEdges.filter(
      (edge) => edge.disposition === "package_resource_mandatory",
    ).length,
    1,
  );
  assert.equal(result.missing.length, 0);
  assert.equal(result.activationBlockers.length, 1);
});

test("nested missing and unsealed remote references fail independently", () => {
  const skill = member("skills/nested/SKILL.md", "[step](./step.md)\n");
  const step = member(
    "skills/nested/step.md",
    "[missing](./missing.md)\n[remote](https://example.invalid/remote.md)\n",
  );
  const result = evaluateResourceReferences({
    source,
    members: [skill, step],
    referencePaths: [skill.path, step.path],
  });

  assert.deepEqual(
    result.missing.map((entry) => entry.literal),
    ["./missing.md"],
  );
  assert.deepEqual(
    result.invalid.map((entry) => entry.literal),
    ["https://example.invalid/remote.md"],
  );
});

test("unsealed mandatory remote enters the affected skill denominator and blocks activation", () => {
  const skill = member(
    "skills/remote/SKILL.md",
    "[remote](https://example.invalid/remote.md)\n",
  );
  const classified = evaluateResourceReferences({
    source,
    members: [skill],
    referencePaths: [skill.path],
  });
  const closure = evaluateSkillResourceClosure({
    source,
    root: skill.path,
    memberPaths: [skill.path],
    directedEdges: classified.directedEdges,
  });

  assert.equal(classified.invalid[0].requirement, "mandatory");
  assert.equal(closure.required_mandatory_edge_count, 2);
  assert.equal(closure.closed_mandatory_edge_count, 1);
  assert.equal(closure.closure_percent, 50);
  assert.equal(closure.activation_allowed, false);
  assert.deepEqual(
    closure.invalid_references.map((entry) => ({
      literal: entry.literal,
      requirement: entry.requirement,
    })),
    [
      {
        literal: "https://example.invalid/remote.md",
        requirement: "mandatory",
      },
    ],
  );
});

test("an exact seal cannot bypass the same literal from a different member", () => {
  const skill = member(
    "skills/no-bypass/SKILL.md",
    "[step](./step.md)\n[authority](https://example.invalid/spec.md)\n",
  );
  const step = member(
    "skills/no-bypass/step.md",
    "[authority](https://example.invalid/spec.md)\n",
  );
  const result = evaluateResourceReferences({
    source,
    members: [skill, step],
    referencePaths: [skill.path, step.path],
    dispositions: [
      {
        ...seal(
          skill,
          "https://example.invalid/spec.md",
          "external_authority_rejected",
        ),
        authority_identity: "example:spec.md",
        governance_owner: "MTH-10",
      },
    ],
  });

  assert.equal(result.activationBlockers.length, 1);
  assert.deepEqual(
    result.invalid.map((entry) => ({
      from: entry.from,
      literal: entry.literal,
    })),
    [
      {
        from: step.path,
        literal: "https://example.invalid/spec.md",
      },
    ],
  );
});

test("package-wide check reports blockers separately and never marks bmad-ux invokable", () => {
  const result = spawnSync(process.execPath, [scriptPath, "--check"], {
    cwd: packageRoot,
    encoding: "utf8",
    env: { ...process.env, NO_COLOR: "1" },
  });

  assert.equal(result.status, 0, result.stderr || result.stdout);
  const summary = JSON.parse(result.stdout);
  const method = summary.packages.find(
    (entry) => entry.source.id === "method",
  );
  const ux = method.production_skills.find(
    (skill) =>
      skill.root ===
      "src/bmm-skills/2-plan-workflows/bmad-ux/SKILL.md",
  );
  assert.equal(ux.closure_percent, 100);
  assert.equal(ux.activation_allowed, false);
  assert.ok(
    ux.activation_blockers.every(
      (blocker) =>
        blocker.disposition === "external_authority_rejected" &&
        blocker.governance_owner === "MTH-10",
    ),
  );
  assert.ok(
    method.edge_dispositions.some(
      (edge) =>
        edge.disposition === "generated_output" &&
        edge.output_identity === "bmad-document-project:project-overview.md",
    ),
  );
});
