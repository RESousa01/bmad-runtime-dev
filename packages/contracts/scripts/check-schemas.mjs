import assert from "node:assert/strict";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  loadSchemaRegistry,
  resolveReference,
} from "./lib/schema-validator.mjs";

const packageRoot = fileURLToPath(new URL("../", import.meta.url));
const registry = await loadSchemaRegistry(path.join(packageRoot, "schemas"));
const expectedDraft = "https://json-schema.org/draft/2020-12/schema";
const seenIds = new Set();
let objectSchemaCount = 0;
let referenceCount = 0;

function inspect(node, documentName, pointer) {
  if (node === null || typeof node !== "object") return;
  if (Array.isArray(node)) {
    node.forEach((item, index) => inspect(item, documentName, `${pointer}/${index}`));
    return;
  }

  if (node.type === "object") {
    objectSchemaCount += 1;
    assert.equal(
      node.additionalProperties,
      false,
      `${documentName}${pointer} must close object properties.`,
    );
  }
  if (typeof node.$ref === "string") {
    referenceCount += 1;
    resolveReference(registry, node.$ref, documentName);
  }

  for (const [key, value] of Object.entries(node)) {
    inspect(value, documentName, `${pointer}/${key}`);
  }
}

for (const [documentName, schema] of registry.documents) {
  assert.equal(schema.$schema, expectedDraft, `${documentName} must use draft 2020-12.`);
  assert.match(
    schema.$id,
    /^https:\/\/schemas\.sapphirus\.dev\/v[1-9][0-9]*\/[a-z0-9-]+\.schema\.json$/,
    `${documentName} has a noncanonical schema ID.`,
  );
  assert.ok(!seenIds.has(schema.$id), `${documentName} repeats schema ID ${schema.$id}.`);
  seenIds.add(schema.$id);
  assert.match(schema.title, /^sapphirus\.[a-z0-9-]+\.v[1-9][0-9]*$/);
  const idMajor = /\/v([1-9][0-9]*)\//.exec(schema.$id)?.[1];
  const titleMajor = /\.v([1-9][0-9]*)$/.exec(schema.title)?.[1];
  assert.equal(idMajor, titleMajor, `${documentName} title and $id major differ.`);
  inspect(schema, documentName, "#");
}

assert.ok(objectSchemaCount >= 20, "Expected representative closed object schemas.");
assert.ok(referenceCount >= 40, "Expected schemas to reuse reviewed primitive definitions.");
console.log(
  `contracts: ${registry.documents.size} schemas, ${objectSchemaCount} closed objects, ${referenceCount} resolved refs`,
);
