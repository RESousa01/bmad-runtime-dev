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

test("Rust codegen relaxation is private and leaves the canonical Builder schema strict", async () => {
  const nativeCodegen = await import("../scripts/lib/native-codegen.mjs");
  assert.equal(typeof nativeCodegen.prepareRustCodegenBundle, "function");

  const lock = await loadAndValidateToolLock();
  const { bundle } = await buildInternalBundle(lock, "production");
  const rustBundle = nativeCodegen.prepareRustCodegenBundle(bundle);
  assert.notStrictEqual(rustBundle, bundle);

  const canonicalAjv = new Ajv2020({ allErrors: true, strict: true, validateFormats: false });
  canonicalAjv.addSchema(bundle, bundle.$id);
  const rustAjv = new Ajv2020({ allErrors: true, strict: true, validateFormats: false });
  rustAjv.addSchema(rustBundle, rustBundle.$id);
  const canonicalBuilder = canonicalAjv.getSchema(`${bundle.$id}#/$defs/BuilderAuthoringObject`);
  const rustBuilder = rustAjv.getSchema(`${rustBundle.$id}#/$defs/BuilderAuthoringObject`);
  const agentRevision = JSON.parse(await readFile(
    path.join(packageRoot, "fixtures", "valid", "bmad", "builder-agent-revision.json"),
    "utf8",
  ));
  agentRevision.authoringAction.action = "build";

  assert.equal(canonicalBuilder(agentRevision), false);
  assert.equal(rustBuilder(agentRevision), true);

  const modelLens = JSON.parse(await readFile(
    path.join(packageRoot, "fixtures", "valid", "bmad", "builder-agent-analysis-model-lens.json"),
    "utf8",
  ));
  [modelLens.modelLensResults[0], modelLens.modelLensResults[1]] =
    [modelLens.modelLensResults[1], modelLens.modelLensResults[0]];
  assert.equal(canonicalBuilder(modelLens), false);
  assert.equal(rustBuilder(modelLens), true);

  const canonicalDescriptor = canonicalAjv.getSchema(`${bundle.$id}#/$defs/BmadPackageDescriptor`);
  const rustDescriptor = rustAjv.getSchema(`${rustBundle.$id}#/$defs/BmadPackageDescriptor`);
  const descriptor = JSON.parse(await readFile(
    path.join(packageRoot, "fixtures", "valid", "bmad", "package-descriptor.json"),
    "utf8",
  ));
  descriptor.configGraphs
    .find((graph) => graph.graphKind === "method_central_toml")
    .layers[0].layerKind = "packaged_default";
  assert.equal(canonicalDescriptor(descriptor), false);
  assert.equal(rustDescriptor(descriptor), true);
  assert.equal(
    bundle.$defs.BuilderAuthoringObjectBuilderAuthoringAction
      .oneOf[0].properties.action.enum.includes("build"),
    false,
  );
});
