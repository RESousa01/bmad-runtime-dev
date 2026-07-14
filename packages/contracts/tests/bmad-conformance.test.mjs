import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { canonicalHash } from "../scripts/lib/canonical-json.mjs";
import { isDiscriminatorRefinement } from "../scripts/lib/schema-validator.mjs";
import {
  sealDocument,
  sealDurableObject,
  validateSemantics,
} from "../scripts/lib/semantics.mjs";
import { parseStrictJson } from "../scripts/lib/strict-json.mjs";

const packageRoot = fileURLToPath(new URL("../", import.meta.url));
const schemaRoot = path.join(packageRoot, "schemas");
const fixtureRoot = path.join(packageRoot, "fixtures");

const family = Object.freeze([
  {
    file: "bmad-package-descriptor.schema.json",
    id: "https://schemas.sapphirus.dev/v1/bmad-package-descriptor.schema.json",
    title: "sapphirus.bmad-package-descriptor.v1",
    definitions: [
      "BmadSourceIdentity",
      "BmadDistributionProfile",
      "BmadInstallProfile",
      "BmadMetadataOrigin",
      "BmadSourceTreatment",
      "BmadInstructionProjection",
      "BmadResourceInventoryEntry",
      "BmadSkillDescriptor",
      "SkillExecutionProfile",
      "BmadConfigLayer",
      "BmadConfigGraphDescriptor",
      "BmadConfigResolution",
    ],
  },
  {
    file: "bmad-capability-catalog.schema.json",
    id: "https://schemas.sapphirus.dev/v1/bmad-capability-catalog.schema.json",
    title: "sapphirus.bmad-capability-catalog.v1",
    definitions: [
      "BmadCapabilityKey",
      "InstalledSkillRecord",
      "BmadHelpActionRecord",
      "BmadHelpActionGraph",
      "BmadDependencyAvailability",
      "BmadAgentRecord",
      "BmadAgentRoster",
      "BmadAgentMenuItem",
      "BmadAgentMenuTarget",
      "BmadAgentSkillTarget",
      "BmadAgentPromptReferenceTarget",
    ],
  },
  {
    file: "bmad-method-session.schema.json",
    id: "https://schemas.sapphirus.dev/v1/bmad-method-session.schema.json",
    title: "sapphirus.bmad-method-session.v1",
    definitions: [
      "MethodInvocationBinding",
      "MethodAgentBinding",
      "MethodModelBinding",
      "MethodContextLedger",
      "BmadContextDecisionConsumption",
      "MethodCheckpoint",
      "MethodArtifactExpectation",
      "MethodAdvanceRequest",
      "MethodAdvanceResult",
      "MethodHelpRecommendation",
      "ArchitectureSpineDraft",
      "ArchitectureReviewResult",
    ],
  },
  {
    file: "bmad-builder-authoring.schema.json",
    id: "https://schemas.sapphirus.dev/v1/bmad-builder-authoring.schema.json",
    title: "sapphirus.bmad-builder-authoring.v1",
    definitions: [
      "BuilderAuthoringAction",
      "BuilderProposedFile",
      "BuilderProposedFileSet",
      "BuilderDraft",
      "BuilderDraftRevision",
      "BuilderAnalysisRun",
      "BuilderAnalysisKind",
      "BuilderModelLensResult",
    ],
  },
  {
    file: "bmad-validation-report.schema.json",
    id: "https://schemas.sapphirus.dev/v1/bmad-validation-report.schema.json",
    title: "sapphirus.bmad-validation-report.v1",
    definitions: [
      "BmadValidationProfile",
      "BmadValidationFinding",
      "BmadValidationDependency",
      "BmadValidationEvidenceRef",
      "BmadValidationDisposition",
    ],
  },
]);

async function readSchema(file) {
  return parseStrictJson(await readFile(path.join(schemaRoot, file), "utf8"));
}

async function readFixture(file) {
  return parseStrictJson(await readFile(path.join(fixtureRoot, file), "utf8"));
}

function visit(node, callback, pointer = "#") {
  if (node === null || typeof node !== "object") return;
  callback(node, pointer);
  if (Array.isArray(node)) {
    node.forEach((value, index) => visit(value, callback, `${pointer}/${index}`));
    return;
  }
  for (const [key, value] of Object.entries(node)) {
    visit(value, callback, `${pointer}/${key}`);
  }
}

function localRef(schema, reference) {
  if (!reference.startsWith("#/")) return undefined;
  let value = schema;
  for (const token of reference
    .slice(2)
    .split("/")
    .map((part) => part.replaceAll("~1", "/").replaceAll("~0", "~"))) {
    value = value?.[token];
  }
  return value;
}

function collectRootVersions(schema, node, versions = new Set(), seen = new Set()) {
  if (node === null || typeof node !== "object") return versions;
  if (typeof node.$ref === "string") {
    if (seen.has(node.$ref)) return versions;
    seen.add(node.$ref);
    const resolved = localRef(schema, node.$ref);
    if (resolved !== undefined) collectRootVersions(schema, resolved, versions, seen);
  }
  const version = node.properties?.schemaVersion?.const;
  if (typeof version === "string") versions.add(version);
  for (const branch of [...(node.oneOf ?? []), ...(node.allOf ?? [])]) {
    collectRootVersions(schema, branch, versions, seen);
  }
  return versions;
}

function discriminatorFacts(schema, node, seen = new Set()) {
  if (node === null || typeof node !== "object") return new Map();
  if (typeof node.$ref === "string" && node.$ref.startsWith("#/")) {
    if (seen.has(node.$ref)) return new Map();
    const nextSeen = new Set(seen).add(node.$ref);
    return discriminatorFacts(schema, localRef(schema, node.$ref), nextSeen);
  }

  const facts = new Map();
  for (const [name, property] of Object.entries(node.properties ?? {})) {
    if (property !== null && typeof property === "object" && Object.hasOwn(property, "const")) {
      facts.set(name, property.const);
    }
  }
  for (const component of node.allOf ?? []) {
    for (const [name, value] of discriminatorFacts(schema, component, seen)) facts.set(name, value);
  }
  if (Array.isArray(node.oneOf) && node.oneOf.length > 0) {
    const alternatives = node.oneOf.map((branch) => discriminatorFacts(schema, branch, seen));
    for (const [name, value] of alternatives[0]) {
      if (alternatives.every((alternative) => Object.is(alternative.get(name), value))) {
        facts.set(name, value);
      }
    }
  }
  return facts;
}

function assertExplicitUnion(schema, branches, pointer) {
  const nullBranches = branches.filter((branch) => branch?.type === "null");
  if (nullBranches.length > 0) {
    assert.equal(branches.length, 2, `${pointer} nullable union must have exactly two branches.`);
    assert.equal(nullBranches.length, 1, `${pointer} nullable union must have one null branch.`);
    return;
  }
  const directTypes = branches.map((branch) => branch?.type).filter((type) => type !== undefined);
  if (directTypes.length === branches.length && new Set(directTypes).size === branches.length) return;

  const branchFacts = branches.map((branch) => discriminatorFacts(schema, branch));
  const discriminators = [...branchFacts[0].keys()].filter((name) => {
    const values = branchFacts.map((facts) => facts.get(name));
    return values.every((value) => value !== undefined) && new Set(values).size === branches.length;
  });
  const compositeSignatures = branchFacts.map((facts) =>
    JSON.stringify([...facts].sort(([left], [right]) => (left < right ? -1 : left > right ? 1 : 0))));
  assert.ok(
    discriminators.length > 0
      || (branchFacts.every((facts) => facts.size > 0)
        && new Set(compositeSignatures).size === branches.length),
    `${pointer} must use a unique const discriminator or an explicit nullable type branch.`,
  );
}

test("the closed BMAD v1 family has the exact canonical roots and named definitions", async () => {
  for (const expected of family) {
    const schema = await readSchema(expected.file);
    assert.equal(schema.$schema, "https://json-schema.org/draft/2020-12/schema");
    assert.equal(schema.$id, expected.id);
    assert.equal(schema.title, expected.title);
    if (schema.properties?.schemaVersion !== undefined) {
      assert.equal(schema.properties.schemaVersion.const, expected.title);
    } else {
      const versions = collectRootVersions(schema, schema);
      collectRootVersions(schema, schema.properties?.payload, versions);
      const rootVersions = [...versions];
      assert.ok(rootVersions.length > 0, `${expected.file} must discriminate root versions.`);
      assert.ok(rootVersions.includes(expected.title), `${expected.file} lacks its current root major.`);
    }
    assert.deepEqual(
      expected.definitions.filter((name) => !Object.hasOwn(schema.$defs ?? {}, name)),
      [],
      `${expected.file} is missing required named definitions.`,
    );
    visit(schema, (node, pointer) => {
      if (node.type === "object") {
        if (isDiscriminatorRefinement(node)) return;
        assert.equal(
          node.additionalProperties,
          false,
          `${expected.file}${pointer} must reject unknown properties.`,
        );
      }
    });
  }
});

test("Builder authoring keeps Agent and Workflow action vocabularies disjoint", async () => {
  const schema = await readSchema("bmad-builder-authoring.schema.json");
  const branches = schema.$defs.BuilderAuthoringAction.oneOf;
  assert.deepEqual(branches.map((branch) => branch.properties.builderKind.const), [
    "agent",
    "workflow",
  ]);
  assert.deepEqual(branches[0].properties.action.enum, ["create_rebuild", "edit", "analyze"]);
  assert.deepEqual(branches[1].properties.action.enum, ["build", "edit", "analyze"]);
});

test("config-layer vocabularies remain disjoint across the three source graphs", async () => {
  const schema = await readSchema("bmad-package-descriptor.schema.json");
  const branches = schema.$defs.BmadConfigLayer.oneOf;
  assert.deepEqual(branches.map((branch) => branch.properties.layerKind.enum), [
    ["installer_team", "installer_user", "custom_team", "custom_user"],
    ["packaged_default", "team_override", "user_override"],
    ["method_module_yaml", "builder_root_yaml", "builder_user_yaml"],
  ]);
});

test("catalog and instruction projections reject resealed nested-hash substitution", async () => {
  const descriptor = await readFixture("valid/bmad/package-descriptor.json");
  const catalog = await readFixture("valid/bmad/capability-catalog.json");
  const forgedHash = `sha256:${"f".repeat(64)}`;

  for (const mutate of [
    (value) => { value.agentRoster.agents[0].agentRecordHash = forgedHash; },
    (value) => { value.agentRoster.agents[0].menuItems[0].sourceMenuItemHash = forgedHash; },
  ]) {
    const value = structuredClone(catalog);
    mutate(value);
    const reasons = validateSemantics(sealDocument(value), { descriptor })
      .map((issue) => issue.code);
    assert.deepEqual(reasons, ["BMAD_AGENT_ROSTER_BINDING_MISMATCH"]);
  }

  const managedSubstitution = structuredClone(descriptor);
  const managed = managedSubstitution.instructionProjections[0].managedInstruction;
  managed.contentHash = forgedHash;
  managedSubstitution.resourceInventory.find((resource) => resource.path === managed.path)
    .contentHash = forgedHash;
  assert.deepEqual(
    validateSemantics(sealDocument(managedSubstitution)).map((issue) => issue.code),
    ["BMAD_INSTRUCTION_PROJECTION_HASH_MISMATCH"],
  );

  const projectionSubstitution = structuredClone(descriptor);
  const projection = projectionSubstitution.instructionProjections[0];
  const originalProjectionHash = projection.projectionHash;
  projection.projectionHash = forgedHash;
  projectionSubstitution.skills
    .filter((skill) => skill.instructionProjectionHash === originalProjectionHash)
    .forEach((skill) => { skill.instructionProjectionHash = forgedHash; });
  assert.deepEqual(
    validateSemantics(sealDocument(projectionSubstitution)).map((issue) => issue.code),
    ["BMAD_INSTRUCTION_PROJECTION_HASH_MISMATCH"],
  );
});

test("Method sessions bind the catalog's exact descriptor and package source", async () => {
  const descriptor = await readFixture("valid/bmad/package-descriptor.json");
  const catalog = await readFixture("valid/bmad/capability-catalog.json");
  const session = await readFixture("valid/bmad/method-architect-iterative.json");
  assert.equal(catalog.packageSourceHash, descriptor.sourceSnapshotHash);

  const forgedHash = `sha256:${"f".repeat(64)}`;
  const descriptorSubstitution = structuredClone(session);
  descriptorSubstitution.payload.packageDescriptorHash = forgedHash;
  descriptorSubstitution.payload.contextLedger.entries.forEach((entry) => {
    entry.packageDescriptorHash = forgedHash;
  });
  descriptorSubstitution.payload.decisionConsumptions.forEach((consumption) => {
    consumption.packageDescriptorHash = forgedHash;
  });
  assert.deepEqual(
    validateSemantics(sealDurableObject(descriptorSubstitution), { catalog })
      .map((issue) => issue.code),
    ["BMAD_METHOD_CATALOG_BINDING_MISMATCH"],
  );

  const sourceSubstitution = structuredClone(session);
  sourceSubstitution.payload.packageSourceHash = forgedHash;
  sourceSubstitution.payload.decisionConsumptions.forEach((consumption) => {
    consumption.packageSourceHash = forgedHash;
  });
  assert.deepEqual(
    validateSemantics(sealDurableObject(sourceSubstitution), { catalog })
      .map((issue) => issue.code),
    ["BMAD_METHOD_CATALOG_BINDING_MISMATCH"],
  );
});

test("Method durability uses the standard envelope and hashes only the payload", async () => {
  const schema = await readSchema("bmad-method-session.schema.json");
  assert.deepEqual(schema.required, ["envelope", "payload"]);
  const session = await readFixture("valid/bmad/method-architect-iterative.json");
  assert.equal(session.envelope.schemaVersion, "sapphirus.durable-object.v1");
  assert.equal(session.envelope.objectType, "bmad_method_session");
  assert.equal(session.envelope.objectId, session.payload.sessionId);
  assert.equal(Object.hasOwn(session.payload, "contentHash"), false);
  assert.equal(Object.hasOwn(session.payload, "authorityRef"), false);
  assert.equal(
    session.envelope.contentHash,
    canonicalHash({
      purpose: "contract-object",
      schemaMajor: "v1",
      value: session.payload,
      excludedFields: [],
    }).serializedHash,
  );
});

test("every BMAD oneOf is explicitly discriminated or nullable", async () => {
  for (const { file } of family) {
    const schema = await readSchema(file);
    visit(schema, (node, pointer) => {
      if (Array.isArray(node.oneOf)) assertExplicitUnion(schema, node.oneOf, `${file}${pointer}`);
    });
  }
});

test("agent menu targets are explicit, prompt references are inert, and no prompt body exists", async () => {
  const schema = await readSchema("bmad-capability-catalog.schema.json");
  const target = schema.$defs.BmadAgentMenuTarget;
  assert.deepEqual(
    target.oneOf.map((branch) => branch.$ref),
    ["#/$defs/BmadAgentSkillTarget", "#/$defs/BmadAgentPromptReferenceTarget"],
  );
  assert.equal(schema.$defs.BmadAgentSkillTarget.properties.targetKind.const, "skill_target");
  const prompt = schema.$defs.BmadAgentPromptReferenceTarget;
  assert.equal(prompt.properties.targetKind.const, "prompt_reference");
  assert.equal(prompt.properties.availability.const, "unavailable_source_prompt");
  assert.equal(Object.hasOwn(prompt.properties, "prompt"), false);
  assert.equal(Object.hasOwn(prompt.properties, "body"), false);
  assert.equal(Object.hasOwn(prompt.properties, "capabilityKey"), false);
});

test("early BMAD schemas do not model deferred Builder lifecycle objects", async () => {
  const deferredTypeNames = new Set([
    "SkillPackageCandidate",
    "SkillPackageVersion",
    "PackageRegistration",
    "PackagePublication",
    "InstallRehearsalRun",
    "InvocationRehearsalRun",
    "EvaluationRun",
    "PackagePromotionRequest",
    "PackageActivation",
    "PackageRollback",
    "BuilderModule",
    "BuilderRegistration",
    "BuilderRehearsal",
    "BuilderEvaluation",
    "BuilderPublication",
    "BuilderPromotion",
    "BuilderActivation",
    "BuilderRollback",
    "BuilderMemoryAgent",
    "BuilderAutonomousAgent",
  ]);
  for (const { file } of family) {
    const schema = await readSchema(file);
    const definitions = new Set(Object.keys(schema.$defs ?? {}));
    assert.deepEqual([...definitions].filter((name) => deferredTypeNames.has(name)), [], file);
  }
});

test("the six BMAD self-hash purposes exclude only their reviewed self-field", () => {
  const cases = [
    ["sapphirus.bmad-package-descriptor.v1", "descriptorHash", "bmad-package-descriptor"],
    ["sapphirus.bmad-capability-catalog.v1", "catalogHash", "bmad-capability-catalog"],
    ["sapphirus.bmad-method-checkpoint.v1", "checkpointHash", "bmad-method-checkpoint"],
    ["sapphirus.bmad-builder-revision.v1", "revisionHash", "bmad-builder-revision"],
    ["sapphirus.bmad-builder-analysis.v1", "analysisHash", "bmad-builder-analysis"],
    ["sapphirus.bmad-validation-report.v1", "reportHash", "bmad-validation-report"],
  ];
  for (const [schemaVersion, hashField, purpose] of cases) {
    const value = { schemaVersion, objectId: "fixture", payload: { alpha: 1 }, [hashField]: null };
    const sealed = sealDocument(value);
    const expected = canonicalHash({
      purpose,
      schemaMajor: "v1",
      value: sealed,
      excludedFields: [hashField],
    }).serializedHash;
    assert.equal(sealed[hashField], expected);

    const selfFieldMutation = { ...sealed, [hashField]: `sha256:${"f".repeat(64)}` };
    assert.equal(
      canonicalHash({ purpose, schemaMajor: "v1", value: selfFieldMutation, excludedFields: [hashField] })
        .serializedHash,
      expected,
    );
    const semanticMutation = { ...sealed, payload: { alpha: 2 } };
    assert.notEqual(
      canonicalHash({ purpose, schemaMajor: "v1", value: semanticMutation, excludedFields: [hashField] })
        .serializedHash,
      expected,
    );
  }
});

test("the committed BMAD golden vectors agree and semantic mutations drift every purpose", async () => {
  const golden = parseStrictJson(
    await readFile(path.join(fixtureRoot, "golden/bmad/hash-vectors.json"), "utf8"),
  );
  assert.equal(golden.vectors.length, 6);
  for (const vector of golden.vectors) {
    const computed = canonicalHash({
      purpose: vector.purpose,
      schemaMajor: vector.schemaMajor,
      value: vector.value,
      excludedFields: vector.excludedFields,
    });
    assert.equal(computed.canonicalJson, vector.canonicalJson, vector.name);
    assert.equal(computed.serializedHash, vector.expectedHash, vector.name);

    const selfField = vector.excludedFields[0];
    const selfMutation = { ...vector.value, [selfField]: `sha256:${"f".repeat(64)}` };
    assert.equal(
      canonicalHash({ ...vector, value: selfMutation }).serializedHash,
      vector.expectedHash,
      `${vector.name} self-field exclusion`,
    );
    const semanticMutation = { ...vector.value, fixtureMutation: true };
    assert.notEqual(
      canonicalHash({ ...vector, value: semanticMutation }).serializedHash,
      vector.expectedHash,
      `${vector.name} semantic mutation`,
    );
  }
});
