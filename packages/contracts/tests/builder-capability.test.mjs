import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import test from "node:test";
import Ajv2020 from "ajv/dist/2020.js";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const foundationRoot = resolve(packageRoot, "../bmad-foundation");
const schemasRoot = resolve(packageRoot, "schemas");

async function readJson(path) {
  return JSON.parse(await readFile(path, "utf8"));
}

async function loadGenerator() {
  return import(
    pathToFileURL(resolve(foundationRoot, "scripts/generate-builder-catalog.mjs")).href
  );
}

async function createValidator() {
  const ajv = new Ajv2020({ allErrors: true, strict: false });
  const commonSchema = await readJson(resolve(schemasRoot, "common.schema.json"));
  const capabilitySchema = await readJson(resolve(schemasRoot, "builder-capability.schema.json"));
  ajv.addSchema(commonSchema);
  return ajv.compile(capabilitySchema);
}

// Break caught: the committed builder-capability-catalog.json drifting from
// the schema that packages/contracts declares final, without any test in
// the contracts package itself ever loading and validating it.
test("the committed Builder capability catalog validates against builder-capability.schema.json", async () => {
  const validate = await createValidator();
  const catalog = await readJson(
    resolve(foundationRoot, "normalized/builder-capability-catalog.json"),
  );
  assert.equal(catalog.counts.public, 9);
  assert.equal(catalog.counts.internal, 1);
  for (const entry of [...catalog.publicActions, ...catalog.internalActions]) {
    assert.equal(validate(entry), true, `${entry.capabilityId}: ${validate.errors ? JSON.stringify(validate.errors) : ""}`);
  }
});

// Break caught: the schema silently accepting an unknown field on a
// capability object, which would let a future generator bug through
// undetected (the schema is closed via additionalProperties: false).
test("builder-capability.schema.json rejects an unknown field on a real catalog entry", async () => {
  const validate = await createValidator();
  const catalog = await readJson(
    resolve(foundationRoot, "normalized/builder-capability-catalog.json"),
  );
  const withUnknownField = { ...catalog.publicActions[0], notPartOfTheContract: true };
  assert.equal(validate(withUnknownField), false);
});

// Break caught: an internal (Eval Runner) capability slipping through with
// exposure "public", which would advertise it on the public Builder menu.
test("the Eval Runner capability is exposure internal and is the only internal action", async () => {
  const catalog = await readJson(
    resolve(foundationRoot, "normalized/builder-capability-catalog.json"),
  );
  assert.equal(catalog.internalActions.length, 1);
  assert.equal(catalog.internalActions[0].action, "run-evals");
  assert.equal(catalog.internalActions[0].exposure, "internal");
});

// Break caught: the alias registry and the capability catalog disagreeing
// about which capability id a legacy Forge Builder id resolves to -- each
// alias's canonicalId must actually name a capability present in the
// catalog, not a stale or renamed one.
test("every alias registry canonicalId names a real capability in the catalog", async () => {
  const catalog = await readJson(
    resolve(foundationRoot, "normalized/builder-capability-catalog.json"),
  );
  const registry = await readJson(
    resolve(foundationRoot, "normalized/builder-alias-registry.json"),
  );
  const capabilityIds = new Set(catalog.publicActions.map((entry) => entry.capabilityId));
  assert.equal(registry.aliases.length, 5);
  for (const alias of registry.aliases) {
    assert.ok(capabilityIds.has(alias.canonicalId), `${alias.legacyId} -> unknown ${alias.canonicalId}`);
  }
});

// Break caught: generate-builder-catalog.mjs --check silently drifting from
// what packages/contracts asserts is valid -- rerunning the generator here
// (via its exported buildCatalog) proves the committed file is exactly what
// the generator would produce today, not a stale hand-edit.
test("regenerating the catalog in memory reproduces the committed file byte-for-byte", async () => {
  const { buildCatalog } = await loadGenerator();
  const [csvText, evalSkillText] = await Promise.all([
    readFile(
      resolve(foundationRoot, "source/builder/2.1.0/skills/module-help.csv"),
      "utf8",
    ),
    readFile(
      resolve(foundationRoot, "source/builder/2.1.0/skills/bmad-eval-runner/SKILL.md"),
      "utf8",
    ),
  ]);
  const regenerated = buildCatalog({ csvText, evalSkillText });
  const committed = await readFile(
    resolve(foundationRoot, "normalized/builder-capability-catalog.json"),
    "utf8",
  );
  assert.equal(`${JSON.stringify(regenerated, null, 2)}\n`, committed);
});
