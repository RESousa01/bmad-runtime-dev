import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";
import path from "node:path";
import { fileURLToPath } from "node:url";
import Ajv2020 from "ajv/dist/2020.js";
import { parseStrictJson } from "../scripts/lib/strict-json.mjs";

const packageRoot = fileURLToPath(new URL("../", import.meta.url));
const schemaDirectory = path.join(packageRoot, "schemas");
const fixtureDirectory = path.join(packageRoot, "fixtures");
const schemaNames = (await readdir(schemaDirectory))
  .filter((name) => name.endsWith(".schema.json"))
  .sort();
const schemas = new Map();

const ajv = new Ajv2020({
  allErrors: true,
  allowUnionTypes: false,
  strict: true,
  validateFormats: false,
});
for (const name of schemaNames) {
  const source = await readFile(path.join(schemaDirectory, name), "utf8");
  parseStrictJson(source);
  const schema = JSON.parse(source);
  schemas.set(name, schema);
  ajv.addSchema(schema);
}

const catalog = parseStrictJson(
  await readFile(path.join(fixtureDirectory, "catalog.json"), "utf8"),
);
const structurallyInvalidReasons = new Set([
  "CONST_MISMATCH",
  "ONE_OF_MISMATCH",
  "PATTERN_MISMATCH",
  "UNKNOWN_PROPERTY",
]);

test("Ajv 2020-12 resolves every schema and validates structural fixtures", async () => {
  for (const entry of catalog) {
    if (entry.schema === null) continue;
    const schema = schemas.get(entry.schema);
    const validate = ajv.getSchema(schema.$id);
    assert.equal(typeof validate, "function", `Missing Ajv validator for ${entry.schema}.`);
    const source = await readFile(path.join(fixtureDirectory, entry.file), "utf8");
    parseStrictJson(source);
    const document = JSON.parse(source);
    const valid = validate(document);

    if (entry.valid) {
      assert.equal(valid, true, `${entry.file}: ${ajv.errorsText(validate.errors)}`);
    } else if (structurallyInvalidReasons.has(entry.reasonCode)) {
      assert.equal(valid, false, `${entry.file} should fail JSON Schema validation.`);
    }
  }
});
