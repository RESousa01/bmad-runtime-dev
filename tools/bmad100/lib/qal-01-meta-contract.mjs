import { createHash } from "node:crypto";

export const META_TEST_CASES = Object.freeze([
  { name: "canonical pinned source closure is complete", category: "canonical" },
  { name: "deleting a source denominator row fails closed", category: "negative" },
  { name: "changing one required source byte fails its pinned hash", category: "negative" },
  { name: "removing a Builder action fails catalog closure", category: "negative" },
  { name: "removing a Builder sample row fails complete classification", category: "negative" },
  { name: "deleting a Builder resource edge fails fresh graph closure", category: "negative" },
  { name: "modifying a Builder resource edge target fails resolution", category: "negative" },
  {
    name: "mutating a generated-package target fails its reviewed inventory contract",
    category: "negative",
  },
  { name: "breaking a Builder consumer fails resolved consumer closure", category: "negative" },
  { name: "duplicating a Builder member fails canonical identity closure", category: "negative" },
  { name: "falsely production-counting a sample fails accounting closure", category: "negative" },
  { name: "orphaning an accounting member fails archive closure", category: "negative" },
  { name: "a stale or mismatched meta receipt cannot promote QAL-01", category: "negative" },
  {
    name: "a failed aggregate cannot publish a QAL-01 completion receipt",
    category: "publisher_guard",
  },
]);

export const EXPECTED_META_TEST_COUNT = 14;

if (
  META_TEST_CASES.length !== EXPECTED_META_TEST_COUNT ||
  new Set(META_TEST_CASES.map(({ name }) => name)).size !== EXPECTED_META_TEST_COUNT
) {
  throw new Error("QAL-01 meta-test contract must define 14 unique cases");
}

export function stableJson(value) {
  return `${JSON.stringify(value, null, 2)}\n`;
}

export function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

export function requireExactMetaInventory(result) {
  const expectedNames = META_TEST_CASES.map(({ name }) => name);
  const actualNames = result.tests?.map(({ name }) => name);
  if (
    result.aggregate?.total !== EXPECTED_META_TEST_COUNT ||
    result.aggregate?.passed !== EXPECTED_META_TEST_COUNT ||
    result.aggregate?.failed !== 0 ||
    result.aggregate?.exit_code !== 0
  ) {
    throw new Error("QAL-01 meta-test aggregate is not exactly 14/14 with exit code 0");
  }
  if (JSON.stringify(actualNames) !== JSON.stringify(expectedNames)) {
    throw new Error("QAL-01 meta-test result inventory differs from the governed 14-case contract");
  }
  for (let index = 0; index < META_TEST_CASES.length; index += 1) {
    const expected = META_TEST_CASES[index];
    const actual = result.tests[index];
    if (actual.category !== expected.category || actual.passed !== true) {
      throw new Error(`QAL-01 meta-test result is invalid for ${expected.name}`);
    }
  }
}
