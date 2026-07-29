import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";
import path from "node:path";
import { fileURLToPath } from "node:url";
import Ajv2020 from "ajv/dist/2020.js";
import { parseStrictJson } from "../scripts/lib/strict-json.mjs";

const packageRoot = fileURLToPath(new URL("../", import.meta.url));
const schemaRoot = path.join(packageRoot, "schemas");
const goldenRoot = path.join(packageRoot, "tests", "fixtures", "bmad");
const requiredSchemas = [
  "bmad-source-identity.schema.json",
  "bmad-source-accounting.schema.json",
  "bmad-capability-catalog.schema.json",
  "bmad-skill-package.schema.json",
  "bmad-session.schema.json",
  "bmad-artifact.schema.json",
  "bmad-action-plan.schema.json",
  "bmad-builder-authoring.schema.json",
  "bmad-builder-promotion.schema.json",
  "bmad-eval.schema.json",
  "bmad-module.schema.json",
  "bmad-script-plan.schema.json",
  "bmad-config.schema.json",
  "bmad-customization.schema.json",
  "bmad-invocation.schema.json",
  "bmad-review-result.schema.json",
  "bmad-problem.schema.json",
  "bmad-context-contribution.schema.json",
  "bmad-memory.schema.json",
  "bmad-upgrade.schema.json",
];

async function readJson(filePath) {
  return parseStrictJson(await readFile(filePath, "utf8"));
}

async function createRegistry() {
  const ajv = new Ajv2020({
    allErrors: true,
    allowUnionTypes: false,
    strict: true,
    validateFormats: false,
  });
  const names = (await readdir(schemaRoot))
    .filter((name) => name.endsWith(".schema.json"))
    .sort();
  const schemas = new Map();
  for (const name of names) {
    const schema = await readJson(path.join(schemaRoot, name));
    schemas.set(name, schema);
    ajv.addSchema(schema);
  }
  return { ajv, schemas };
}

const { ajv, schemas } = await createRegistry();

function validatorFor(name) {
  const schema = schemas.get(name);
  assert.ok(schema, `Missing schema ${name}.`);
  const validate = ajv.getSchema(schema.$id);
  assert.equal(typeof validate, "function", `Missing validator for ${name}.`);
  return validate;
}

function migrateBuilderDraftV1(input, manifest) {
  assert.equal(input.schemaVersion, manifest.fromVersion);
  const profiles = {
    BuilderAgentV2Stateless: "stateless",
    BuilderAgentV2Memory: "memory",
    BuilderAgentV2AutonomousPulse: "autonomous_pulse",
  };
  const output = {
    objectKind: input.objectKind,
    schemaVersion: manifest.toVersion,
    recordId: input.draftId,
    builderKind: input.builderKind,
    action: input.authoringAction.action,
    agentProfile: profiles[input.validationProfile] ?? "not_applicable",
    mode: "interactive",
    lifecycle: "draft",
    sourceHash: input.sourceIdentityHash,
    payloadHash: input.instructionProjectionSetHash,
    migration: structuredClone(manifest),
  };
  return output;
}

test("canonical BMAD schema family is complete and golden documents validate", async () => {
  for (const name of requiredSchemas) {
    assert.ok(schemas.has(name), `${name} is required by SRC-07.`);
  }
  const golden = await readJson(path.join(goldenRoot, "contract-family-v1.json"));
  for (const entry of golden.documents) {
    const validate = validatorFor(entry.schema);
    assert.equal(validate(entry.value), true, `${entry.schema}: ${ajv.errorsText(validate.errors)}`);
  }
});

test("closed objects and discriminators reject unknown fields and states", async () => {
  const golden = await readJson(path.join(goldenRoot, "contract-family-v1.json"));
  for (const entry of golden.documents) {
    const validate = validatorFor(entry.schema);
    const withUnknownField = { ...structuredClone(entry.value), futureAuthority: true };
    assert.equal(validate(withUnknownField), false, `${entry.schema} accepted an unknown field.`);
  }

  const session = structuredClone(
    golden.documents.find((entry) => entry.schema === "bmad-session.schema.json").value,
  );
  session.state = "telepathic";
  assert.equal(validatorFor("bmad-session.schema.json")(session), false);

  const promotion = structuredClone(
    golden.documents.find(
      (entry) => entry.schema === "bmad-builder-promotion.schema.json",
    ).value,
  );
  promotion.toState = "force_activated";
  assert.equal(validatorFor("bmad-builder-promotion.schema.json")(promotion), false);
  promotion.toState = "draft";
  assert.equal(validatorFor("bmad-builder-promotion.schema.json")(promotion), false);
});

test("current Builder v1 drafts migrate deterministically to the closed v2 contract", async () => {
  const migration = await readJson(
    path.join(goldenRoot, "builder-draft-v1-to-v2.json"),
  );
  const input = await readJson(path.join(packageRoot, migration.inputFixture));
  const first = migrateBuilderDraftV1(input, migration.manifest);
  const second = migrateBuilderDraftV1(structuredClone(input), migration.manifest);
  assert.equal(JSON.stringify(first), JSON.stringify(migration.expected));
  assert.equal(JSON.stringify(second), JSON.stringify(migration.expected));
  assert.equal(validatorFor("bmad-upgrade.schema.json")(migration.manifest), true);
  const validateAuthoring = validatorFor("bmad-builder-authoring.schema.json");
  assert.equal(
    validateAuthoring(first),
    true,
    ajv.errorsText(validateAuthoring.errors),
  );
});

test("Builder v2 covers every required kind, action, profile, and execution mode", async () => {
  const migration = await readJson(
    path.join(goldenRoot, "builder-draft-v1-to-v2.json"),
  );
  const base = migration.expected;
  const kinds = ["agent", "workflow", "module", "setup", "conversion", "evaluation", "promotion", "installation", "rollback"];
  const actions = ["create_rebuild", "build", "edit", "analyze", "convert", "evaluate", "promote", "install", "rollback", "configure"];
  const profiles = ["stateless", "memory", "autonomous_pulse", "not_applicable"];
  const modes = ["interactive", "headless"];
  const validate = validatorFor("bmad-builder-authoring.schema.json");
  for (const [field, values] of Object.entries({ builderKind: kinds, action: actions, agentProfile: profiles, mode: modes })) {
    for (const value of values) {
      assert.equal(validate({ ...base, [field]: value }), true, `${field}=${value}: ${ajv.errorsText(validate.errors)}`);
    }
  }
});
