import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  canonicalHash,
  canonicalize,
} from "../scripts/lib/canonical-json.mjs";
import { HASH_RULES } from "../generated/typescript/runtime.mjs";
import { parseStrictJson } from "../scripts/lib/strict-json.mjs";

const packageRoot = fileURLToPath(new URL("../", import.meta.url));
const vectors = parseStrictJson(
  await readFile(path.join(packageRoot, "fixtures/golden/hash-vectors.json"), "utf8"),
);
const packageCompatibility = parseStrictJson(
  await readFile(
    path.join(packageRoot, "fixtures/valid/package-compatibility.json"),
    "utf8",
  ),
);
const remoteJobHandoff = parseStrictJson(
  await readFile(
    path.join(packageRoot, "fixtures/valid/remote-job-handoff.json"),
    "utf8",
  ),
);

test("matches every mandatory note 99 hash vector including the LF", () => {
  for (const vector of vectors.required) {
    const result = canonicalHash(vector);
    assert.equal(result.canonicalJson, vector.canonicalJson);
    assert.equal(result.serializedHash, vector.expectedHash);
    assert.equal(
      result.preimage,
      `sapphirus:${vector.purpose}:${vector.schemaMajor}\n${vector.canonicalJson}`,
    );
  }
});

test("canonical member order is independent of insertion order", () => {
  const left = { z: 1, nested: { beta: 2, alpha: 1 }, a: "é" };
  const right = { a: "é", nested: { alpha: 1, beta: 2 }, z: 1 };
  assert.equal(canonicalize(left), canonicalize(right));
  assert.equal(
    canonicalHash({ purpose: "contract-object", schemaMajor: "v1", value: left })
      .serializedHash,
    canonicalHash({ purpose: "contract-object", schemaMajor: "v1", value: right })
      .serializedHash,
  );
});

test("semantic mutation changes a purpose-separated hash", () => {
  const original = { deliveryModel: "windows_local", sequence: 1 };
  const mutated = { deliveryModel: "windows_local", sequence: 2 };
  assert.notEqual(
    canonicalHash({ purpose: "contract-object", schemaMajor: "v1", value: original })
      .serializedHash,
    canonicalHash({ purpose: "contract-object", schemaMajor: "v1", value: mutated })
      .serializedHash,
  );
});

test("package and handoff runtime hash rules match their sealed fixtures", () => {
  for (const [document, hashField] of [
    [packageCompatibility, "signedPayloadHash"],
    [remoteJobHandoff, "handoffHash"],
  ]) {
    const rule = HASH_RULES[document.schemaVersion];
    assert.ok(rule, document.schemaVersion);
    assert.equal(
      canonicalHash({ ...rule, value: document }).serializedHash,
      document[hashField],
    );
  }

  const alternateSignature = structuredClone(packageCompatibility);
  alternateSignature.signature.signature = "YWx0ZXJuYXRlLXNpZ25hdHVyZQ";
  const rule = HASH_RULES[packageCompatibility.schemaVersion];
  assert.equal(
    canonicalHash({ ...rule, value: alternateSignature }).serializedHash,
    packageCompatibility.signedPayloadHash,
  );
});

test("rejects non-I-JSON values before hashing", () => {
  assert.throws(() => canonicalize({ value: Number.POSITIVE_INFINITY }), {
    code: "NON_FINITE_NUMBER",
  });
  assert.throws(() => canonicalize({ value: 9007199254740992 }), {
    code: "INTEGER_OUT_OF_RANGE",
  });
  assert.throws(() => canonicalize({ value: "\ud800" }), {
    code: "INVALID_UNICODE",
  });
  assert.throws(() => canonicalize(new Array(1)), {
    code: "SPARSE_ARRAY",
  });
  const arrayWithProperty = [];
  arrayWithProperty.label = "not-json";
  assert.throws(() => canonicalize(arrayWithProperty), {
    code: "ARRAY_EXTRA_PROPERTY",
  });
});
