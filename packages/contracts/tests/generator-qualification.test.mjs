import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  canonicalHash,
  canonicalize,
} from "../scripts/lib/canonical-json.mjs";
import { createQualificationValidator } from "../scripts/lib/qualification-validator.mjs";
import { parseStrictJson } from "../scripts/lib/strict-json.mjs";

const packageRoot = fileURLToPath(new URL("../", import.meta.url));
const repositoryRoot = path.resolve(packageRoot, "..", "..");
const qualificationRoot = path.join(repositoryRoot, "tests", "generator-qualification");
const catalog = parseStrictJson(
  await readFile(path.join(qualificationRoot, "catalog.json"), "utf8"),
);

function assertExactKeys(value, expected, label) {
  assert.deepEqual(Object.keys(value).sort(), [...expected].sort(), `${label} has open fields.`);
}

function resolveQualificationPath(relativePath) {
  assert.equal(path.posix.isAbsolute(relativePath), false, `${relativePath} must be relative.`);
  assert.equal(relativePath.includes("\\"), false, `${relativePath} must use forward slashes.`);
  assert.equal(
    relativePath.split("/").includes(".."),
    false,
    `${relativePath} must not traverse directories.`,
  );
  const resolved = path.resolve(qualificationRoot, ...relativePath.split("/"));
  assert.ok(
    resolved.startsWith(`${qualificationRoot}${path.sep}`),
    `${relativePath} escapes the qualification root.`,
  );
  return resolved;
}

async function readQualificationFile(relativePath) {
  return readFile(resolveQualificationPath(relativePath), "utf8");
}

test("strict JSON parsing rejects a container beyond the configured depth", () => {
  assert.throws(
    () => parseStrictJson('{"nested":{}}', { maxContainerDepth: 1 }),
    { code: "MAX_DEPTH_EXCEEDED" },
  );
});

test("strict JSON parsing enforces a configured UTF-8 byte limit", () => {
  assert.throws(
    () => parseStrictJson('"é"', { maxBytes: 3 }),
    { code: "MAX_BYTES_EXCEEDED" },
  );
});

test("strict JSON parsing preserves unlimited defaults for existing callers", () => {
  const source = `{"value":${"[".repeat(9)}null${"]".repeat(9)}}`;
  assert.deepEqual(parseStrictJson(source).value.flat(9), [null]);
  assert.throws(() => parseStrictJson(source, { maxContainerDepth: 8 }), {
    code: "MAX_DEPTH_EXCEEDED",
  });
});

test("strict JSON parsing rejects non-finite and precision-losing numeric tokens", () => {
  assert.throws(() => parseStrictJson("9007199254740991.1"), {
    code: "INTEGER_OUT_OF_RANGE",
  });
  assert.throws(() => parseStrictJson("1e309"), {
    code: "NON_FINITE_NUMBER",
  });

  for (const source of ["0.1", "-1.5", "9007199254740991", "-9007199254740991"]) {
    assert.equal(parseStrictJson(source), Number(source));
  }
});

test("qualification validation rejects duplicate members before invoking Ajv", async () => {
  let validatorInvocations = 0;
  const qualification = createQualificationValidator({
    rootSchema: {
      $schema: "https://json-schema.org/draft/2020-12/schema",
      $id: "https://schemas.sapphirus.dev/generator-qualification/test/root.schema.json",
      type: "object",
      additionalProperties: false,
      required: ["value"],
      properties: { value: { type: "integer" } },
    },
    resources: [],
    parserLimits: { maxBytes: 262144, maxContainerDepth: 8 },
    reasonPriority: ["DUPLICATE_MEMBER", "SCHEMA_INVALID"],
    onValidatorInvoke: () => {
      validatorInvocations += 1;
    },
  });

  assert.deepEqual(qualification.validateSource('{"value":1,"value":2}'), {
    accepted: false,
    reasonCategory: "DUPLICATE_MEMBER",
    rejectionStage: "strict_parser",
    validatorInvoked: false,
    discriminator: null,
  });
  assert.equal(validatorInvocations, 0);
});

test("qualification catalog is closed, complete, and defect-isolated", async () => {
  assertExactKeys(
    catalog,
    [
      "schemaVersion",
      "rootSchema",
      "resources",
      "parserLimits",
      "reasonPriority",
      "fixtures",
    ],
    "catalog",
  );
  assert.equal(
    catalog.schemaVersion,
    "sapphirus.generator-qualification.catalog.v1",
  );
  assert.deepEqual({ ...catalog.parserLimits }, {
    maxBytes: 262144,
    maxContainerDepth: 8,
  });
  assert.equal(new Set(catalog.reasonPriority).size, catalog.reasonPriority.length);
  assert.equal(catalog.fixtures.length, 25);

  const ids = new Set();
  const files = new Set();
  const coverage = new Set();
  for (const entry of catalog.fixtures) {
    assertExactKeys(
      entry,
      [
        "id",
        "file",
        "expected",
        "reasonCategory",
        "rejectionStage",
        "validatorInvoked",
        "covers",
      ],
      `fixture ${entry.id}`,
    );
    assert.match(entry.id, /^[a-z][a-z0-9-]*$/u);
    assert.equal(ids.has(entry.id), false, `Duplicate fixture id ${entry.id}.`);
    assert.equal(files.has(entry.file), false, `Duplicate fixture file ${entry.file}.`);
    ids.add(entry.id);
    files.add(entry.file);
    assert.ok(entry.covers.length > 0, `${entry.id} must declare coverage.`);
    for (const item of entry.covers) {
      assert.match(item, /^[a-z][a-z0-9_]*$/u);
      coverage.add(item);
    }

    if (entry.expected === "accept") {
      assert.equal(entry.reasonCategory, null, entry.id);
      assert.equal(entry.rejectionStage, "none", entry.id);
      assert.equal(entry.validatorInvoked, true, entry.id);
      assert.ok(entry.file.startsWith("fixtures/valid/"), entry.id);
    } else {
      assert.equal(entry.expected, "reject", entry.id);
      assert.ok(catalog.reasonPriority.includes(entry.reasonCategory), entry.id);
      assert.ok(
        ["strict_parser", "structural_validator"].includes(entry.rejectionStage),
        entry.id,
      );
      assert.equal(
        entry.validatorInvoked,
        entry.rejectionStage === "structural_validator",
        entry.id,
      );
      assert.ok(entry.file.startsWith("fixtures/invalid/"), entry.id);
    }
    assert.ok((await readQualificationFile(entry.file)).length > 0, entry.file);
  }

  for (const requiredCoverage of [
    "closed_object",
    "one_of_discriminator",
    "required_null",
    "optional_absent",
    "optional_null",
    "local_external_ref",
    "signed_minimum",
    "signed_maximum",
    "unsigned_zero",
    "unsigned_maximum",
    "interoperable_minimum",
    "interoperable_maximum",
    "unicode_nfc",
    "unicode_nfd",
    "unicode_non_normalization",
    "empty_string",
    "empty_array",
    "recursive_depth_exact_limit",
    "recursive_depth_overflow",
    "duplicate_key",
    "decoded_duplicate_key",
    "unknown_property",
    "unknown_discriminator",
    "union_transplant",
    "unsafe_integer",
    "numeric_precision_loss",
    "non_finite_number",
    "unpaired_unicode",
  ]) {
    assert.ok(coverage.has(requiredCoverage), `Missing coverage ${requiredCoverage}.`);
  }

  const rootSchema = parseStrictJson(await readQualificationFile(catalog.rootSchema));
  assert.equal(rootSchema.$schema, "https://json-schema.org/draft/2020-12/schema");
  assert.equal(
    rootSchema.$id,
    "https://schemas.sapphirus.dev/generator-qualification/v1/qualification.schema.json",
  );
  for (const resourcePath of catalog.resources) {
    const resource = parseStrictJson(await readQualificationFile(resourcePath));
    assert.equal(typeof resource.$id, "string", resourcePath);
  }
});

test("Ajv qualification returns the cataloged stage and stable reason for all fixtures", async () => {
  const rootSchema = parseStrictJson(await readQualificationFile(catalog.rootSchema));
  const resources = await Promise.all(
    catalog.resources.map(async (resourcePath) =>
      parseStrictJson(await readQualificationFile(resourcePath))),
  );
  let validatorInvocations = 0;
  const qualification = createQualificationValidator({
    rootSchema,
    resources,
    parserLimits: catalog.parserLimits,
    reasonPriority: catalog.reasonPriority,
    onValidatorInvoke: () => {
      validatorInvocations += 1;
    },
  });

  for (const entry of catalog.fixtures) {
    const source = await readQualificationFile(entry.file);
    const invocationsBefore = validatorInvocations;
    const actual = qualification.validateSource(source);
    assert.deepEqual(
      {
        accepted: actual.accepted,
        reasonCategory: actual.reasonCategory,
        rejectionStage: actual.rejectionStage,
        validatorInvoked: actual.validatorInvoked,
      },
      {
        accepted: entry.expected === "accept",
        reasonCategory: entry.reasonCategory,
        rejectionStage: entry.rejectionStage,
        validatorInvoked: entry.validatorInvoked,
      },
      entry.id,
    );
    assert.equal(
      validatorInvocations - invocationsBefore,
      entry.validatorInvoked ? 1 : 0,
      `${entry.id} validator invocation count`,
    );

    if (entry.expected === "accept") {
      const parsed = parseStrictJson(source, catalog.parserLimits);
      const canonicalBefore = canonicalize(parsed);
      const serialized = JSON.stringify(parsed);
      const reparsed = parseStrictJson(serialized, catalog.parserLimits);
      assert.equal(canonicalize(reparsed), canonicalBefore, `${entry.id} round trip`);
      assert.equal(qualification.validateSource(serialized).accepted, true, entry.id);
    } else if (entry.rejectionStage === "strict_parser") {
      assert.equal(actual.discriminator, null, entry.id);
    }
  }
});

test("accepted null and absence semantics survive serialization", async () => {
  const absent = parseStrictJson(
    await readQualificationFile("fixtures/valid/text-null-empty.json"),
    catalog.parserLimits,
  );
  assert.equal(absent.nullableValue, null);
  assert.equal(Object.hasOwn(absent, "optionalValue"), false);

  const explicitNull = parseStrictJson(
    await readQualificationFile("fixtures/valid/count-optional-null.json"),
    catalog.parserLimits,
  );
  assert.equal(explicitNull.optionalValue, null);
  const roundTrip = parseStrictJson(JSON.stringify(explicitNull), catalog.parserLimits);
  assert.equal(Object.hasOwn(roundTrip, "optionalValue"), true);
  assert.equal(roundTrip.optionalValue, null);
});

test("NFC and NFD remain canonically byte-distinct and hash-distinct", async () => {
  const nfc = parseStrictJson(
    await readQualificationFile("fixtures/valid/unicode-nfc.json"),
    catalog.parserLimits,
  );
  const nfd = parseStrictJson(
    await readQualificationFile("fixtures/valid/unicode-nfd.json"),
    catalog.parserLimits,
  );
  assert.equal(nfc.label.normalize("NFD"), nfd.label);
  assert.notEqual(nfc.label, nfd.label);

  const nfcWithoutLabel = structuredClone(nfc);
  const nfdWithoutLabel = structuredClone(nfd);
  delete nfcWithoutLabel.label;
  delete nfdWithoutLabel.label;
  assert.equal(canonicalize(nfcWithoutLabel), canonicalize(nfdWithoutLabel));

  const nfcCanonical = canonicalize(nfc);
  const nfdCanonical = canonicalize(nfd);
  assert.notEqual(
    Buffer.compare(Buffer.from(nfcCanonical, "utf8"), Buffer.from(nfdCanonical, "utf8")),
    0,
  );
  assert.notEqual(
    canonicalHash({
      purpose: "contract-object",
      schemaMajor: "v1",
      value: nfc,
    }).serializedHash,
    canonicalHash({
      purpose: "contract-object",
      schemaMajor: "v1",
      value: nfd,
    }).serializedHash,
  );
});
