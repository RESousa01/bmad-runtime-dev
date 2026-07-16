import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { SCHEMA_CLOSURES } from "../generated/typescript/runtime.mjs";
import { canonicalHash, canonicalize } from "../scripts/lib/canonical-json.mjs";
import { buildSchemaClosureManifest } from "../scripts/lib/schema-closure.mjs";
import { parseStrictJson } from "../scripts/lib/strict-json.mjs";

const schemaRoot = fileURLToPath(new URL("../schemas/", import.meta.url));
const generatedRoot = fileURLToPath(new URL("../generated/", import.meta.url));

async function readSchemas() {
  const names = (await readdir(schemaRoot)).filter((name) => name.endsWith(".schema.json")).sort();
  return Promise.all(names.map(async (name) =>
    parseStrictJson(await readFile(path.join(schemaRoot, name), "utf8"))));
}

const cases = Object.freeze([
  {
    rootSchemaId: "https://schemas.sapphirus.dev/v1/bmad-method-advance-result.schema.json",
    members: [
      "https://schemas.sapphirus.dev/v1/bmad-method-advance-result.schema.json",
      "https://schemas.sapphirus.dev/v1/bmad-method-session.schema.json",
      "https://schemas.sapphirus.dev/v1/common.schema.json",
    ],
  },
  {
    rootSchemaId: "https://schemas.sapphirus.dev/v1/bmad-method-help-proposal.schema.json",
    members: [
      "https://schemas.sapphirus.dev/v1/bmad-capability-catalog.schema.json",
      "https://schemas.sapphirus.dev/v1/bmad-method-help-proposal.schema.json",
      "https://schemas.sapphirus.dev/v1/common.schema.json",
    ],
  },
  {
    rootSchemaId: "https://schemas.sapphirus.dev/v1/bmad-method-help-recommendation.schema.json",
    members: [
      "https://schemas.sapphirus.dev/v1/bmad-capability-catalog.schema.json",
      "https://schemas.sapphirus.dev/v1/bmad-method-help-recommendation.schema.json",
      "https://schemas.sapphirus.dev/v1/bmad-method-session.schema.json",
      "https://schemas.sapphirus.dev/v1/common.schema.json",
    ],
  },
]);

test("sealed Help schema closures are fragment-aware, sorted, and self-verifying", async () => {
  const schemas = await readSchemas();
  const closureHashes = [];
  for (const expected of cases) {
    const closure = buildSchemaClosureManifest({
      rootSchemaId: expected.rootSchemaId,
      schemas,
    });
    assert.equal(closure.rootSchemaId, expected.rootSchemaId);
    assert.deepEqual(closure.members.map(({ schemaId }) => schemaId), expected.members);
    assert.ok(closure.members.every(({ canonicalSha256 }) => /^sha256:[a-f0-9]{64}$/.test(canonicalSha256)));
    const preimage = canonicalize({
      rootSchemaId: closure.rootSchemaId,
      members: closure.members,
    });
    const expectedHash = `sha256:${createHash("sha256").update(preimage, "utf8").digest("hex")}`;
    assert.equal(closure.closureSha256, expectedHash);
    closureHashes.push(expectedHash);
    assert.notEqual(closure.closureSha256, canonicalHash({
      purpose: "schema-closure",
      schemaMajor: "v1",
      value: { rootSchemaId: closure.rootSchemaId, members: closure.members },
    }).serializedHash);

    const driftedSchemas = structuredClone(schemas);
    const driftedRoot = driftedSchemas.find((schema) => schema.$id === expected.rootSchemaId);
    driftedRoot.title = `${driftedRoot.title}-drift`;
    assert.notEqual(buildSchemaClosureManifest({
      rootSchemaId: expected.rootSchemaId,
      schemas: driftedSchemas,
    }).closureSha256, closure.closureSha256);
    assert.deepEqual(SCHEMA_CLOSURES[expected.rootSchemaId], closure);

    const constantName = expected.rootSchemaId
      .slice(expected.rootSchemaId.lastIndexOf("/") + 1, -".schema.json".length)
      .replaceAll("-", "_")
      .toUpperCase() + "_SCHEMA_CLOSURE_SHA256";
    const rust = await readFile(path.join(generatedRoot, "rust/contracts.rs"), "utf8");
    const dotnet = await readFile(path.join(generatedRoot, "dotnet/BmadSchemaClosures.g.cs"), "utf8");
    assert.ok(rust.includes(`pub const ${constantName}: &str =\n    "${expectedHash}";`));
    assert.ok(dotnet.includes(`public const string ${constantName} = "${expectedHash}";`));
  }
  assert.equal(new Set(closureHashes).size, cases.length);
});
