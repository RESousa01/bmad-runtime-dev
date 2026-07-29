import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";
import path from "node:path";
import { fileURLToPath } from "node:url";
import Ajv2020 from "ajv/dist/2020.js";
import { canonicalHash, canonicalize } from "../scripts/lib/canonical-json.mjs";
import { parseStrictJson } from "../scripts/lib/strict-json.mjs";

const packageRoot = fileURLToPath(new URL("../", import.meta.url));
const fixtureRoot = path.join(packageRoot, "fixtures", "bmad");

async function readJson(relativePath) {
  return JSON.parse(await readFile(path.join(packageRoot, relativePath), "utf8"));
}

test("SRC-08 generated surfaces cover every canonical BMAD schema version", async () => {
  const catalog = await readJson("fixtures/bmad/catalog.json");
  const [typescript, rust, csharp] = await Promise.all([
    readFile(path.join(packageRoot, "generated/typescript/bmad.ts"), "utf8"),
    readFile(path.join(packageRoot, "generated/rust/bmad.rs"), "utf8"),
    readFile(path.join(packageRoot, "generated/csharp/BmadContracts.cs"), "utf8"),
  ]);
  assert.equal(catalog.schemaVersion, "sapphirus.bmad-cross-language-fixtures.v1");
  // Reviewed count. Seven BMAD schemas were added by ENG-06, ENG-08, ENG-12,
  // ENG-13, ENG-14, ENG-16 and BLD-02; this guard exists so an addition is a
  // deliberate act rather than a silent one.
  assert.equal(catalog.schemas.length, 22);
  for (const schema of catalog.schemas) {
    assert.match(typescript, new RegExp(schema.protocolVersion.replaceAll(".", "\\."), "u"));
    assert.match(rust, new RegExp(schema.rustType, "u"));
    assert.match(csharp, new RegExp(schema.csharpType, "u"));
  }
});

test("valid fixture branches validate and preserve canonical bytes and hashes in both directions", async () => {
  const catalog = await readJson("fixtures/bmad/catalog.json");
  const ajv = new Ajv2020({ allErrors: true, strict: true, validateFormats: false });
  for (const name of (await readdir(path.join(packageRoot, "schemas"))).sort()) {
    if (name.endsWith(".schema.json")) ajv.addSchema(await readJson(`schemas/${name}`));
  }
  assert.deepEqual(catalog.roundTripOrder, [
    "typescript", "rust", "csharp", "rust", "typescript",
  ]);
  assert.ok(catalog.validFixtures.length >= catalog.schemas.length);
  for (const fixturePath of catalog.validFixtures) {
    const fixture = await readJson(fixturePath);
    const validate = ajv.getSchema(fixture.schemaId);
    assert.equal(typeof validate, "function", fixturePath);
    assert.equal(validate(fixture.document), true, ajv.errorsText(validate.errors));
    let value = structuredClone(fixture.document);
    for (const _language of catalog.roundTripOrder) {
      value = parseStrictJson(canonicalize(value));
    }
    assert.equal(canonicalize(value), fixture.canonicalJson, fixturePath);
    assert.equal(canonicalHash({
      purpose: "bmad-cross-language-fixture",
      schemaMajor: "v1",
      value,
    }).serializedHash, fixture.expectedHash, fixturePath);
  }
});

test("rejection fixtures cover every closed root plus forbidden lifecycle transitions", async () => {
  const catalog = await readJson("fixtures/bmad/catalog.json");
  const ajv = new Ajv2020({ allErrors: true, strict: true, validateFormats: false });
  for (const name of (await readdir(path.join(packageRoot, "schemas"))).sort()) {
    if (name.endsWith(".schema.json")) ajv.addSchema(await readJson(`schemas/${name}`));
  }
  const rejections = await Promise.all(catalog.rejectionFixtures.map(readJson));
  const unknownRoots = new Set(
    rejections.filter(({ reason }) => reason === "unknown_property").map(({ schemaId }) => schemaId),
  );
  assert.equal(unknownRoots.size, catalog.schemas.length);
  assert.ok(rejections.some(({ reason }) => reason === "forbidden_transition"));
  for (const fixture of rejections) {
    const validate = ajv.getSchema(fixture.schemaId);
    assert.equal(validate(fixture.document), false, fixture.id);
  }
});

test("fixture tree contains no undeclared JSON output", async () => {
  const catalog = await readJson("fixtures/bmad/catalog.json");
  const declared = new Set([
    "catalog.json",
    ...catalog.validFixtures.map((file) => path.relative(fixtureRoot, path.join(packageRoot, file)).replaceAll("\\", "/")),
    ...catalog.rejectionFixtures.map((file) => path.relative(fixtureRoot, path.join(packageRoot, file)).replaceAll("\\", "/")),
  ]);
  const actual = [];
  async function walk(directory, prefix = "") {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const relative = prefix === "" ? entry.name : `${prefix}/${entry.name}`;
      if (entry.isDirectory()) await walk(path.join(directory, entry.name), relative);
      else actual.push(relative);
    }
  }
  await walk(fixtureRoot);
  assert.deepEqual(actual.sort(), [...declared].sort());
});
