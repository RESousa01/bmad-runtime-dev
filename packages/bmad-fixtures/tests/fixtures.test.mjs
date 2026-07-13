import assert from "node:assert/strict";
import { cp, mkdtemp, readFile, readdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import test from "node:test";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import {
  EXPECTED_DESCRIPTOR_NAMES,
  FixtureValidationError,
  assertContentMatchesBinding,
  decodeDescriptorBytes,
  parseFixtureDescriptor,
  parseFixtureDescriptorBytes,
  verifyFixtureSet,
} from "../scripts/fixture-policy.mjs";

const packageRoot = fileURLToPath(new URL("../", import.meta.url));
const fixtureDirectory = path.join(packageRoot, "fixtures");

async function readDescriptor(name) {
  const bytes = await readFile(path.join(fixtureDirectory, name));
  return decodeDescriptorBytes(bytes, name);
}

async function loadDescriptor(name) {
  const bytes = await readFile(path.join(fixtureDirectory, name));
  return parseFixtureDescriptorBytes(bytes, name);
}

function mutateDescriptor(text, mutate) {
  const descriptor = JSON.parse(text);
  mutate(descriptor);
  return JSON.stringify(descriptor);
}

function hasCode(code) {
  return (error) =>
    error instanceof FixtureValidationError && error.code === code;
}

test("sealed descriptor set and repository-owned payload bytes verify", async () => {
  const result = await verifyFixtureSet();
  assert.equal(result.descriptorCount, 3);
  assert.deepEqual(EXPECTED_DESCRIPTOR_NAMES, [
    "inactive-simple-workflow.json",
    "inactive-stateless-agent.json",
    "sealed-method-bmad-help.json",
  ]);
  assert.deepEqual(result.fixtureIds, [
    "bmad_builder_simple_workflow_v1",
    "bmad_builder_stateless_agent_v1",
    "bmad_method_help_v6",
  ]);
});

test("Method proof is exact, provenance-bound, and read-only", async () => {
  const fixture = await loadDescriptor("sealed-method-bmad-help.json");
  assert.equal(fixture.fixtureKind, "method_direct_skill");
  assert.equal(fixture.source.project, "BMAD-METHOD");
  assert.equal(
    fixture.source.archiveSha256,
    "a7c049038099b99081fbd03d22c6a5180edd88dee656bb37c4276b1cc31b4a32",
  );
  assert.equal(
    fixture.source.archivePath,
    "BMAD-METHOD-main/src/core-skills/bmad-help/SKILL.md",
  );
  assert.equal(fixture.executionProfile, "direct");
  assert.equal(fixture.activationState, "sealed_read_only");
  assert.deepEqual(fixture.builderActions, []);
  assert.equal(fixture.payload, null);
  assert.equal(fixture.scriptExecution, "blocked");
  assert.equal(fixture.networkAccess, "blocked");
});

test("Builder examples expose only inactive Build, Edit, and Analyze drafts", async () => {
  for (const name of [
    "inactive-stateless-agent.json",
    "inactive-simple-workflow.json",
  ]) {
    const fixture = await loadDescriptor(name);
    assert.equal(fixture.activationState, "not_active");
    assert.deepEqual(fixture.builderActions, ["Build", "Edit", "Analyze"]);
    assert.equal(
      fixture.executionProfile,
      name === "inactive-simple-workflow.json" ? "inline" : "direct",
    );
    assert.notEqual(fixture.payload, null);
    assert.equal(fixture.scriptExecution, "blocked");
    assert.equal(fixture.networkAccess, "blocked");
  }
});

test("JSON parser rejects malformed input and duplicate decoded keys", async (t) => {
  await t.test("trailing comma", () => {
    assert.throws(
      () => parseFixtureDescriptor('{"fixtureKind":"method_direct_skill",}', "sealed-method-bmad-help.json"),
      hasCode("JSON_SYNTAX"),
    );
  });
  await t.test("duplicate root key", () => {
    assert.throws(
      () =>
        parseFixtureDescriptor(
          '{"fixtureKind":"method_direct_skill","fixtureKind":"method_direct_skill"}',
          "sealed-method-bmad-help.json",
        ),
      hasCode("JSON_DUPLICATE_KEY"),
    );
  });
  await t.test("duplicate escaped key", () => {
    assert.throws(
      () =>
        parseFixtureDescriptor(
          '{"source":{"project":1,"\\u0070roject":2}}',
          "sealed-method-bmad-help.json",
        ),
      hasCode("JSON_DUPLICATE_KEY"),
    );
  });
  for (const [label, input] of [
    ["unpaired high surrogate", '{"value":"\\ud800"}'],
    ["unpaired low surrogate", '{"value":"\\udc00"}'],
  ]) {
    await t.test(label, () => {
      assert.throws(
        () => parseFixtureDescriptor(input, "sealed-method-bmad-help.json"),
        hasCode("JSON_UNPAIRED_SURROGATE"),
      );
    });
  }
  await t.test("valid escaped scalar", async () => {
    const source = await readDescriptor("sealed-method-bmad-help.json");
    const input = source.replace(
      '"bmad_method_help_v6"',
      '"bmad_method_help_v\\u0036"',
    );
    assert.doesNotThrow(() =>
      parseFixtureDescriptor(input, "sealed-method-bmad-help.json"),
    );
  });
  await t.test("valid surrogate pair reaches schema validation", () => {
    assert.throws(
      () =>
        parseFixtureDescriptor(
          '{"\\ud83d\\ude00":true}',
          "sealed-method-bmad-help.json",
        ),
      hasCode("SCHEMA_MISSING_FIELD"),
    );
  });
  await t.test("non-finite numeric representation", () => {
    assert.throws(
      () => parseFixtureDescriptor('{"value":1e400}', "sealed-method-bmad-help.json"),
      hasCode("JSON_SYNTAX"),
    );
  });
  for (const value of ["9007199254740992", "-9007199254740992", "1e20"]) {
    await t.test(`unsafe integral number ${value}`, () => {
      assert.throws(
        () =>
          parseFixtureDescriptor(
            `{"value":${value}}`,
            "sealed-method-bmad-help.json",
          ),
        hasCode("JSON_INTEGER_RANGE"),
      );
    });
  }
  await t.test("excessive nesting", () => {
    const input = `${"[".repeat(34)}null${"]".repeat(34)}`;
    assert.throws(
      () => parseFixtureDescriptor(input, "sealed-method-bmad-help.json"),
      hasCode("JSON_DEPTH_LIMIT"),
    );
  });
  await t.test("oversized descriptor", () => {
    const input = `{"padding":"${"x".repeat(64 * 1024)}"}`;
    assert.throws(
      () => parseFixtureDescriptor(input, "sealed-method-bmad-help.json"),
      hasCode("JSON_SIZE_LIMIT"),
    );
  });
  await t.test("malformed UTF-8 bytes", () => {
    assert.throws(
      () =>
        parseFixtureDescriptorBytes(
          Uint8Array.from([0x7b, 0x22, 0xc3, 0x28, 0x22, 0x7d]),
          "sealed-method-bmad-help.json",
        ),
      hasCode("JSON_UTF8"),
    );
  });
  await t.test("UTF-8 BOM is not silently discarded", () => {
    assert.throws(
      () =>
        parseFixtureDescriptorBytes(
          Uint8Array.from([0xef, 0xbb, 0xbf, 0x7b, 0x7d]),
          "sealed-method-bmad-help.json",
        ),
      hasCode("JSON_SYNTAX"),
    );
  });
});

test("closed schemas reject unknown fields and fixture kinds", async (t) => {
  const source = await readDescriptor("sealed-method-bmad-help.json");

  await t.test("unknown root field", () => {
    const input = mutateDescriptor(source, (fixture) => {
      fixture.futureField = true;
    });
    assert.throws(
      () => parseFixtureDescriptor(input, "sealed-method-bmad-help.json"),
      hasCode("SCHEMA_UNKNOWN_FIELD"),
    );
  });
  await t.test("unknown nested source field", () => {
    const input = mutateDescriptor(source, (fixture) => {
      fixture.source.futureField = true;
    });
    assert.throws(
      () => parseFixtureDescriptor(input, "sealed-method-bmad-help.json"),
      hasCode("SCHEMA_UNKNOWN_FIELD"),
    );
  });
  await t.test("source filesystem path field", () => {
    const input = mutateDescriptor(source, (fixture) => {
      fixture.source.relativePath = "reviewed/source/SKILL.md";
    });
    assert.throws(
      () => parseFixtureDescriptor(input, "sealed-method-bmad-help.json"),
      hasCode("SCHEMA_UNKNOWN_FIELD"),
    );
  });
  await t.test("unknown kind-specific assertion", () => {
    const input = mutateDescriptor(source, (fixture) => {
      fixture.assertions.futureSemantic = false;
    });
    assert.throws(
      () => parseFixtureDescriptor(input, "sealed-method-bmad-help.json"),
      hasCode("SCHEMA_UNKNOWN_FIELD"),
    );
  });
  await t.test("unknown discriminator", () => {
    const input = mutateDescriptor(source, (fixture) => {
      fixture.fixtureKind = "future_fixture";
    });
    assert.throws(
      () => parseFixtureDescriptor(input, "sealed-method-bmad-help.json"),
      hasCode("SCHEMA_DISCRIMINATOR"),
    );
  });
  for (const discriminator of ["__proto__", "constructor", "toString"]) {
    await t.test(`prototype-like discriminator ${discriminator}`, () => {
      const input = mutateDescriptor(source, (fixture) => {
        fixture.fixtureKind = discriminator;
      });
      assert.throws(
        () => parseFixtureDescriptor(input, "sealed-method-bmad-help.json"),
        hasCode("SCHEMA_DISCRIMINATOR"),
      );
    });
  }
  await t.test("missing field", () => {
    const input = mutateDescriptor(source, (fixture) => {
      delete fixture.source;
    });
    assert.throws(
      () => parseFixtureDescriptor(input, "sealed-method-bmad-help.json"),
      hasCode("SCHEMA_MISSING_FIELD"),
    );
  });
  await t.test("invalid source type", () => {
    const input = mutateDescriptor(source, (fixture) => {
      fixture.source = [];
    });
    assert.throws(
      () => parseFixtureDescriptor(input, "sealed-method-bmad-help.json"),
      hasCode("SCHEMA_TYPE"),
    );
  });
  await t.test("invalid digest encoding", () => {
    const input = mutateDescriptor(source, (fixture) => {
      fixture.source.sha256 = "A".repeat(64);
    });
    assert.throws(
      () => parseFixtureDescriptor(input, "sealed-method-bmad-help.json"),
      hasCode("SCHEMA_DIGEST"),
    );
  });
  await t.test("unsafe byte length", () => {
    const input = mutateDescriptor(source, (fixture) => {
      fixture.source.byteLength = Number.MAX_SAFE_INTEGER + 1;
    });
    assert.throws(
      () => parseFixtureDescriptor(input, "sealed-method-bmad-help.json"),
      hasCode("JSON_INTEGER_RANGE"),
    );
  });
  await t.test("archive path traversal", () => {
    const input = mutateDescriptor(source, (fixture) => {
      fixture.source.archivePath = "../outside/SKILL.md";
    });
    assert.throws(
      () => parseFixtureDescriptor(input, "sealed-method-bmad-help.json"),
      hasCode("SCHEMA_PATH"),
    );
  });
  for (const [label, archivePath] of [
    ["absolute archive path", "/outside/SKILL.md"],
    ["Windows archive path", "outside\\SKILL.md"],
    ["drive-qualified archive path", "C:/outside/SKILL.md"],
  ]) {
    await t.test(label, () => {
      const input = mutateDescriptor(source, (fixture) => {
        fixture.source.archivePath = archivePath;
      });
      assert.throws(
        () => parseFixtureDescriptor(input, "sealed-method-bmad-help.json"),
        hasCode("SCHEMA_PATH"),
      );
    });
  }
  await t.test("uppercase archive digest", () => {
    const input = mutateDescriptor(source, (fixture) => {
      fixture.source.archiveSha256 = fixture.source.archiveSha256.toUpperCase();
    });
    assert.throws(
      () => parseFixtureDescriptor(input, "sealed-method-bmad-help.json"),
      hasCode("SCHEMA_DIGEST"),
    );
  });
  await t.test("well-formed but untrusted archive digest", () => {
    const input = mutateDescriptor(source, (fixture) => {
      fixture.source.archiveSha256 = "0".repeat(64);
    });
    assert.throws(
      () => parseFixtureDescriptor(input, "sealed-method-bmad-help.json"),
      hasCode("IDENTITY_MISMATCH"),
    );
  });
});

test("authority-bearing keys are rejected recursively", async (t) => {
  const source = await readDescriptor("inactive-stateless-agent.json");
  const cases = [
    ["argv", ["tool.exe"]],
    ["command", "tool.exe --unsafe"],
    ["environment", { SECRET: "value" }],
    ["hooks", ["post-install"]],
    ["process", { executable: "tool.exe" }],
    ["shell", "powershell"],
    ["networkGrants", ["internet"]],
    ["filesystemWriteGrant", true],
    ["convertAction", "Convert"],
    ["evaluationClaim", "passed"],
    ["activationRequest", true],
    ["scripts", ["prepare.js"]],
  ];

  for (const [key, value] of cases) {
    await t.test(key, () => {
      const input = mutateDescriptor(source, (fixture) => {
        fixture.assertions.nested = { safe: [{ [key]: value }] };
      });
      assert.throws(
        () => parseFixtureDescriptor(input, "inactive-stateless-agent.json"),
        hasCode("AUTHORITY_BEARING_FIELD"),
      );
    });
  }

  await t.test("authority key at descriptor root", () => {
    const input = mutateDescriptor(source, (fixture) => {
      fixture.command = "tool.exe";
    });
    assert.throws(
      () => parseFixtureDescriptor(input, "inactive-stateless-agent.json"),
      hasCode("AUTHORITY_BEARING_FIELD"),
    );
  });

  await t.test("deny-only key is not accepted below the root", () => {
    const input = mutateDescriptor(source, (fixture) => {
      fixture.assertions.nested = { networkAccess: "blocked" };
    });
    assert.throws(
      () => parseFixtureDescriptor(input, "inactive-stateless-agent.json"),
      hasCode("AUTHORITY_BEARING_FIELD"),
    );
  });
});

test("deny-only fields cannot become capabilities", async (t) => {
  const source = await readDescriptor("inactive-simple-workflow.json");
  for (const [field, value] of [
    ["activationState", "active"],
    ["executionProfile", "shell"],
    ["scriptExecution", "allowed"],
    ["networkAccess", "allowed"],
  ]) {
    await t.test(field, () => {
      const input = mutateDescriptor(source, (fixture) => {
        fixture[field] = value;
      });
      assert.throws(
        () => parseFixtureDescriptor(input, "inactive-simple-workflow.json"),
        hasCode("AUTHORITY_BEARING_FIELD"),
      );
    });
  }
});

test("fixture-kind semantics and action enums fail closed", async (t) => {
  const methodSource = await readDescriptor("sealed-method-bmad-help.json");
  const builderSource = await readDescriptor("inactive-simple-workflow.json");

  await t.test("Method cannot expose Builder actions", () => {
    const input = mutateDescriptor(methodSource, (fixture) => {
      fixture.builderActions = ["Build"];
    });
    assert.throws(
      () => parseFixtureDescriptor(input, "sealed-method-bmad-help.json"),
      hasCode("SEMANTIC_ACTIONS"),
    );
  });
  await t.test("Builder cannot expose Convert", () => {
    const input = mutateDescriptor(builderSource, (fixture) => {
      fixture.builderActions.push("Convert");
    });
    assert.throws(
      () => parseFixtureDescriptor(input, "inactive-simple-workflow.json"),
      hasCode("SEMANTIC_ACTIONS"),
    );
  });
  await t.test("Builder action order is canonical", () => {
    const input = mutateDescriptor(builderSource, (fixture) => {
      fixture.builderActions = ["Edit", "Build", "Analyze"];
    });
    assert.throws(
      () => parseFixtureDescriptor(input, "inactive-simple-workflow.json"),
      hasCode("SEMANTIC_ACTIONS"),
    );
  });
  await t.test("Builder draft payload is required", () => {
    const input = mutateDescriptor(builderSource, (fixture) => {
      fixture.payload = null;
    });
    assert.throws(
      () => parseFixtureDescriptor(input, "inactive-simple-workflow.json"),
      hasCode("SEMANTIC_PAYLOAD"),
    );
  });
});

test("descriptor names and source/payload identities are immutable", async (t) => {
  const source = await readDescriptor("inactive-stateless-agent.json");

  await t.test("descriptor name", () => {
    assert.throws(
      () => parseFixtureDescriptor(source, "inactive-simple-workflow.json"),
      hasCode("IDENTITY_MISMATCH"),
    );
  });
  for (const [label, mutate] of [
    ["archive path", (fixture) => {
      fixture.source.archivePath = "bmad-builder-main/skills/other/SKILL.md";
    }],
    ["archive hash", (fixture) => {
      fixture.source.archiveSha256 = "1".repeat(64);
    }],
    ["source hash", (fixture) => {
      fixture.source.sha256 = "0".repeat(64);
    }],
    ["payload hash", (fixture) => {
      fixture.payload.sha256 = "0".repeat(64);
    }],
    ["fixture id", (fixture) => {
      fixture.fixtureId = "replacement";
    }],
  ]) {
    await t.test(label, () => {
      const input = mutateDescriptor(source, mutate);
      assert.throws(
        () => parseFixtureDescriptor(input, "inactive-stateless-agent.json"),
        hasCode("IDENTITY_MISMATCH"),
      );
    });
  }
});

test("bound-content verification detects byte tampering", async () => {
  const fixture = await loadDescriptor("inactive-simple-workflow.json");
  const payload = await readFile(
    path.join(packageRoot, ...fixture.payload.relativePath.split("/")),
  );
  assert.deepEqual(
    assertContentMatchesBinding(fixture.payload, payload, "payload"),
    {
      byteLength: fixture.payload.byteLength,
      sha256: fixture.payload.sha256,
    },
  );

  const tampered = Buffer.from(payload);
  tampered[0] ^= 0xff;
  assert.throws(
    () => assertContentMatchesBinding(fixture.payload, tampered, "payload"),
    hasCode("CONTENT_DIGEST_MISMATCH"),
  );
});

test("package allowlist contains only conformance assets", async () => {
  const packageManifest = JSON.parse(
    await readFile(path.join(packageRoot, "package.json"), "utf8"),
  );
  assert.equal(packageManifest.private, true);
  assert.deepEqual(packageManifest.files, [
    "fixtures",
    "scripts",
    "tests",
    "README.md",
  ]);
});

test("verification succeeds in a minimal checkout containing only this package", async () => {
  const isolatedRoot = await mkdtemp(
    path.join(tmpdir(), "sapphirus-bmad-checkout-"),
  );
  const isolatedPackageRoot = path.join(
    isolatedRoot,
    "packages",
    "bmad-fixtures",
  );

  try {
    await cp(packageRoot, isolatedPackageRoot, { recursive: true });
    assert.deepEqual(await readdir(isolatedRoot), ["packages"]);
    const isolatedPolicyUrl = pathToFileURL(
      path.join(isolatedPackageRoot, "scripts", "fixture-policy.mjs"),
    );
    const { verifyFixtureSet: verifyIsolatedFixtureSet } = await import(
      isolatedPolicyUrl.href
    );
    assert.deepEqual(
      await verifyIsolatedFixtureSet(),
      {
        descriptorCount: 3,
        fixtureIds: [
          "bmad_builder_simple_workflow_v1",
          "bmad_builder_stateless_agent_v1",
          "bmad_method_help_v6",
        ],
      },
    );
  } finally {
    await rm(isolatedRoot, { recursive: true, force: true });
  }
});
