import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  bmadContextDecisionUniquenessKey,
  sealDocument,
  specConsumptionUniquenessKey,
  validateDurableObjectHash,
  validateRemoteJobHandoffTransition,
  validateSemantics,
} from "../scripts/lib/semantics.mjs";
import {
  loadSchemaRegistry,
  validateSchemaDocument,
} from "../scripts/lib/schema-validator.mjs";
import { parseStrictJson } from "../scripts/lib/strict-json.mjs";

const packageRoot = fileURLToPath(new URL("../", import.meta.url));
const fixtureRoot = path.join(packageRoot, "fixtures");
const registry = await loadSchemaRegistry(path.join(packageRoot, "schemas"));
const catalog = parseStrictJson(
  await readFile(path.join(fixtureRoot, "catalog.json"), "utf8"),
);

async function readFixture(relativePath) {
  return readFile(path.join(fixtureRoot, relativePath), "utf8");
}

const STRUCTURAL_REASON_PRIORITY = [
  "ONE_OF_MISMATCH",
  "ARRAY_TOO_SHORT",
  "ARRAY_TOO_LONG",
  "UNKNOWN_PROPERTY",
  "REQUIRED_PROPERTY_MISSING",
  "CONST_MISMATCH",
  "ENUM_MISMATCH",
  "PATTERN_MISMATCH",
  "TYPE_MISMATCH",
  "SCHEMA_INVALID",
];

function normalizedSchemaReason(errors) {
  const codes = new Set(errors.map((error) => error.code));
  return STRUCTURAL_REASON_PRIORITY.find((code) => codes.has(code)) ?? "SCHEMA_INVALID";
}

test("D3 fixtures contain no deferred runner or containment vocabulary", async () => {
  const forbidden = [
    "WindowsContainmentClaim",
    "WindowsLocalHostAudience",
    "runnerProfile",
    "runner_profile",
    "RunnerProfile",
    "job_object_controlled",
    "childProcess",
    "child_process",
    "ChildProcess",
    "networkIntent",
    "network_intent",
    "NetworkIntent",
    "standard_user_job",
    "windows_local_host",
    "command_run",
    "raw_shell",
    "run_shell",
    "process_spawn",
  ];

  for (const entry of catalog) {
    const source = await readFixture(entry.file);
    for (const token of forbidden) {
      assert.ok(!source.includes(token), `${entry.file} contains deferred token ${token}.`);
    }
  }
});

test("valid and adversarial fixtures produce stable reason categories", async () => {
  for (const entry of catalog) {
    const source = await readFixture(entry.file);
    if (entry.reasonCode === "DUPLICATE_MEMBER") {
      assert.throws(() => parseStrictJson(source), { code: entry.reasonCode });
      continue;
    }

    const document = parseStrictJson(source);
    const schemaErrors = validateSchemaDocument(registry, entry.schema, document);
    const semanticErrors = schemaErrors.length > 0
      ? []
      : entry.schema === "durable-object.schema.json"
        ? validateDurableObjectHash(document)
        : validateSemantics(document, entry.contextFile === undefined
          ? undefined
          : await (async () => {
              const context = parseStrictJson(await readFixture(entry.contextFile));
              return context.schemaVersion === "sapphirus.bmad-capability-catalog.v1"
                ? { catalog: context }
                : { descriptor: context };
            })());
    const reasonCodes = schemaErrors.length > 0
      ? entry.reasonCodes === undefined
        ? schemaErrors.map((error) => error.code)
        : [normalizedSchemaReason(schemaErrors)]
      : semanticErrors.map((error) => error.code);

    if (entry.valid) {
      assert.deepEqual(reasonCodes, [], `${entry.file}: ${JSON.stringify(reasonCodes)}`);
    } else {
      if (entry.reasonCodes !== undefined) {
        assert.deepEqual(reasonCodes, entry.reasonCodes, entry.file);
      } else {
        assert.ok(
          reasonCodes.includes(entry.reasonCode),
          `${entry.file}: expected ${entry.reasonCode}, got ${reasonCodes.join(", ")}`,
        );
      }
    }
  }
});

test("BMAD fixtures cover canonical catalog separation and single-use context decisions", async () => {
  const descriptor = parseStrictJson(
    await readFixture("valid/bmad/package-descriptor.json"),
  );
  const catalogFixture = parseStrictJson(
    await readFixture("valid/bmad/capability-catalog.json"),
  );
  const session = parseStrictJson(
    await readFixture("valid/bmad/method-architect-iterative.json"),
  );

  assert.deepEqual(validateSemantics(descriptor), []);
  assert.deepEqual(validateSemantics(catalogFixture, { descriptor }), []);
  assert.equal(catalogFixture.agentRoster.agents.length, 6);
  assert.deepEqual(
    catalogFixture.agentRoster.agents.map((agent) => agent.agentCode),
    [
      "bmad-agent-analyst",
      "bmad-agent-architect",
      "bmad-agent-dev",
      "bmad-agent-pm",
      "bmad-agent-tech-writer",
      "bmad-agent-ux-designer",
    ],
  );
  const singleAction = catalogFixture.installedSkills.find(
    (skill) => skill.skillName === "bmad-help",
  );
  assert.equal(singleAction.actionCardinality, "single_action");
  assert.equal(singleAction.capabilityKeys[0].normalizedAction, null);
  const architecture = catalogFixture.installedSkills.find(
    (skill) => skill.skillName === "bmad-architecture",
  );
  assert.equal(architecture.actionCardinality, "single_action");
  assert.equal(architecture.capabilityKeys[0].normalizedAction, "create");
  assert.equal(Object.hasOwn(singleAction, "rawRow"), false);
  assert.equal(Object.hasOwn(catalogFixture.helpActionGraph.actions[0], "capabilityKeys"), false);
  assert.deepEqual(
    Object.keys(catalogFixture.helpActionGraph.actions[0].rawRow),
    [
      "module",
      "skill",
      "display-name",
      "menu-code",
      "description",
      "action",
      "args",
      "phase",
      "preceded-by",
      "followed-by",
      "required",
      "output-location",
      "outputs",
    ],
  );

  const paige = catalogFixture.agentRoster.agents.find(
    (agent) => agent.agentCode === "bmad-agent-tech-writer",
  );
  assert.ok(paige);
  assert.deepEqual(
    paige.menuItems.map((item) => [item.menuCode, item.target.targetKind]),
    [
      ["DP", "skill_target"],
      ["WD", "prompt_reference"],
      ["MG", "prompt_reference"],
      ["VD", "prompt_reference"],
      ["EC", "prompt_reference"],
    ],
  );
  assert.equal(
    catalogFixture.agentRoster.agents
      .flatMap((agent) => agent.menuItems)
      .filter((item) => item.menuCode === "DP").length,
    2,
    "DP may repeat across agent scopes but not within one agent",
  );
  for (const item of paige.menuItems.slice(1)) {
    assert.equal(item.target.availability, "unavailable_source_prompt");
    assert.equal(Object.hasOwn(item.target, "body"), false);
    assert.equal(Object.hasOwn(item.target, "prompt"), false);
    assert.equal(Object.hasOwn(item.target, "capabilityKey"), false);
  }
  assert.equal(session.payload.checkpoints.length, 2);
  assert.equal(session.payload.decisionConsumptions.length, 2);
  assert.equal(
    new Set(session.payload.decisionConsumptions.map(bmadContextDecisionUniquenessKey)).size,
    2,
  );
  const replay = structuredClone(session.payload.decisionConsumptions[0]);
  replay.invocationId = "invoke_01J00000000000000000000009";
  assert.equal(
    bmadContextDecisionUniquenessKey(replay),
    bmadContextDecisionUniquenessKey(session.payload.decisionConsumptions[0]),
    "the uniqueness key is decision-scoped, not invocation-scoped",
  );
});

test("candidate, approval, consumption, result, and evidence remain linked", async () => {
  const load = async (name) => parseStrictJson(await readFixture(`valid/${name}.json`));
  const candidate = await load("windows-local-candidate");
  const spec = await load("approved-execution-spec");
  const consumption = await load("spec-consumption");
  const result = await load("execution-result-manifest");
  const event = await load("evidence-event");

  assert.equal(spec.candidateId, candidate.candidateId);
  assert.equal(spec.candidateHash, candidate.candidateHash);
  assert.equal(consumption.specId, spec.specId);
  assert.equal(consumption.specHash, spec.specHash);
  assert.equal(consumption.singleUseNonceHash, spec.singleUseNonceHash);
  assert.equal(result.consumptionHash, consumption.consumptionHash);
  assert.equal(event.payloadHash, result.manifestHash);
  assert.equal(Object.hasOwn(spec, "consumed"), false);
  assert.equal(Object.hasOwn(spec, "consumedAt"), false);
  assert.equal(candidate.executorAudience.audienceKind, "native_patch_engine");
  assert.deepEqual(
    Object.keys(candidate.executorAudience).sort(),
    [
      "audienceKind",
      "hostBinarySha256",
      "hostBuildId",
      "installationId",
      "patchEngineProfileHash",
    ].sort(),
  );
  assert.deepEqual(spec.executorAudience, candidate.executorAudience);
  assert.equal(Object.hasOwn(candidate, "networkIntent"), false);
  assert.equal(candidate.limits.timeoutSeconds, 0);
  assert.equal(candidate.limits.maxOutputBytes, 0);
  assert.equal(candidate.limits.maxProcessCount, 0);
});

test("the one-time spec uniqueness tuple rejects a replay", async () => {
  const record = parseStrictJson(await readFixture("valid/spec-consumption.json"));
  const consumed = new Set();
  const consumeOnce = (value) => {
    const key = specConsumptionUniquenessKey(value);
    if (consumed.has(key)) return false;
    consumed.add(key);
    return true;
  };

  assert.equal(consumeOnce(record), true);
  assert.equal(consumeOnce(structuredClone(record)), false);
});

test("cross-authority transplantation fails closed", async () => {
  const transplanted = parseStrictJson(
    await readFixture("invalid/authority-mismatch.json"),
  );
  const errors = validateSchemaDocument(
    registry,
    "candidate-action.schema.json",
    transplanted,
  );
  assert.ok(errors.some((error) => error.code === "CONST_MISMATCH"));
});

test("package capability sets are canonical, disjoint, and epoch ordered", async () => {
  const valid = parseStrictJson(
    await readFixture("valid/package-compatibility.json"),
  );
  assert.deepEqual(validateSemantics(valid), []);

  const unsorted = structuredClone(valid);
  unsorted.requiredCapabilities.reverse();
  assert.ok(
    validateSemantics(sealDocument(unsorted)).some(
      (error) => error.code === "CAPABILITY_SET_NOT_CANONICAL",
    ),
  );

  const overlapping = structuredClone(valid);
  overlapping.optionalCapabilities.push(valid.requiredCapabilities[0]);
  overlapping.optionalCapabilities.sort();
  assert.ok(
    validateSemantics(sealDocument(overlapping)).some(
      (error) => error.code === "CAPABILITY_SET_OVERLAP",
    ),
  );
});

test("remote handoff transitions require one version step and the exact prior hash", async () => {
  const fixture = parseStrictJson(await readFixture("valid/remote-job-handoff.json"));
  const previous = sealDocument({
    ...structuredClone(fixture),
    handoffVersion: 7,
    previousHandoffHash: "sha256:1111111111111111111111111111111111111111111111111111111111111111",
  });
  const current = sealDocument({
    ...structuredClone(fixture),
    handoffVersion: 8,
    previousHandoffHash: previous.handoffHash,
  });
  assert.deepEqual(validateRemoteJobHandoffTransition(previous, current), []);

  const skippedVersion = sealDocument({
    ...structuredClone(current),
    handoffVersion: 9,
  });
  assert.ok(
    validateRemoteJobHandoffTransition(previous, skippedVersion).some(
      (error) => error.code === "HANDOFF_VERSION_NOT_INCREMENTAL",
    ),
  );

  const wrongHash = sealDocument({
    ...structuredClone(current),
    previousHandoffHash: "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
  });
  assert.ok(
    validateRemoteJobHandoffTransition(previous, wrongHash).some(
      (error) => error.code === "HANDOFF_PREVIOUS_HASH_MISMATCH",
    ),
  );
});
