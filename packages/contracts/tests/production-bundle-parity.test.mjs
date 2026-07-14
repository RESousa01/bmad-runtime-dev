import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import Ajv2020 from "ajv/dist/2020.js";
import {
  buildInternalBundle,
  loadAndValidateToolLock,
} from "../scripts/lib/native-codegen.mjs";

const packageRoot = fileURLToPath(new URL("../", import.meta.url));

test("production bundle preserves structural acceptance for every cataloged schema fixture", async () => {
  const lock = await loadAndValidateToolLock();
  const { bundle } = await buildInternalBundle(lock, "production");
  const originalAjv = new Ajv2020({ allErrors: true, strict: true, validateFormats: false });
  const schemas = new Map();
  for (const entry of [...lock.sourceSet.production.roots, ...lock.sourceSet.production.dependencies]) {
    const schema = JSON.parse(await readFile(path.join(packageRoot, "schemas", entry.file), "utf8"));
    schemas.set(entry.file, schema);
    originalAjv.addSchema(schema, entry.id);
  }
  const bundleAjv = new Ajv2020({ allErrors: true, strict: true, validateFormats: false });
  bundleAjv.addSchema(bundle, bundle.$id);
  const typeNameByFile = new Map(lock.sourceSet.production.roots.map((root) => [root.file, root.typeName]));
  const catalog = JSON.parse(await readFile(path.join(packageRoot, "fixtures", "catalog.json"), "utf8"));
  let compared = 0;
  for (const fixture of catalog) {
    if (fixture.schema === null) continue;
    const value = JSON.parse(await readFile(path.join(packageRoot, "fixtures", fixture.file), "utf8"));
    const original = originalAjv.getSchema(schemas.get(fixture.schema).$id);
    const bundled = bundleAjv.getSchema(
      `${bundle.$id}#/$defs/${typeNameByFile.get(fixture.schema)}`,
    );
    assert.equal(typeof original, "function", fixture.schema);
    assert.equal(typeof bundled, "function", fixture.schema);
    assert.equal(bundled(value), original(value), fixture.file);
    compared += 1;
  }
  assert.equal(compared, catalog.filter((fixture) => fixture.schema !== null).length);
});
