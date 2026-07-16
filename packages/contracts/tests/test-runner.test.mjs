import assert from "node:assert/strict";
import test from "node:test";

import { selectContractTestFiles } from "../scripts/run-tests.mjs";

test("the TypeScript lane excludes only native generator artifact tests", () => {
  const files = selectContractTestFiles([
    "schema-closure.test.mjs",
    "native-codegen-lock.test.mjs",
    "README.md",
    "generator-provenance.test.mjs",
  ]);

  assert.deepEqual(files, [
    "generator-provenance.test.mjs",
    "schema-closure.test.mjs",
  ]);
});

test("the cross-language lane includes native generator artifact tests", () => {
  const files = selectContractTestFiles(
    ["schema-closure.test.mjs", "native-codegen-lock.test.mjs"],
    { includeNative: true },
  );

  assert.deepEqual(files, [
    "native-codegen-lock.test.mjs",
    "schema-closure.test.mjs",
  ]);
});
