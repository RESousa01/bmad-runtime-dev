import { createHash } from "node:crypto";
import { readFile, readdir } from "node:fs/promises";
import { join, relative } from "node:path";
import process from "node:process";

import { canonicalTextBytes, verifyClosedManifest } from "./lib/closed-manifest.mjs";

const root = process.cwd();
const vaultRoot = join(root, "bmad-runtime-lib");
const recordPath = join(root, "docs", "provenance", "vault-validation.json");

function fail(message) {
  console.error(`Reference vault verification failed: ${message}`);
  process.exit(1);
}

async function bytes(path) {
  try {
    return await readFile(path);
  } catch {
    fail(`${relative(root, path)} is missing or unreadable`);
  }
}

function sha256(payload) {
  return createHash("sha256").update(payload).digest("hex");
}

function lineCount(payload) {
  const text = payload.toString("utf8").replace(/^\uFEFF/, "");
  if (text.length === 0) return 0;
  const breaks = text.match(/\r\n|[\n\r\v\f\x1c-\x1e\x85\u2028\u2029]/g)?.length ?? 0;
  return /(?:\r\n|[\n\r\v\f\x1c-\x1e\x85\u2028\u2029])$/.test(text) ? breaks : breaks + 1;
}

function parseJson(payload, displayPath) {
  try {
    return JSON.parse(payload.toString("utf8"));
  } catch {
    fail(`${displayPath} is not valid JSON`);
  }
}

const recordBytes = await bytes(recordPath);
const record = parseJson(recordBytes, relative(root, recordPath));
if (
  record.schemaVersion !== "sapphirus.reference-vault-validation.v1" ||
  record.manifestVerified !== true ||
  !Array.isArray(record.errors) ||
  record.errors.length !== 0 ||
  !Array.isArray(record.warnings) ||
  record.warnings.length !== 0
) {
  fail("the recorded validation result is not a clean, supported verification record");
}

const expectedValidator = "bmad-runtime-lib/_source_review/validate_library.py";
const expectedManifest = "bmad-runtime-lib/manifest.json";
const expectedLivingValidator = "bmad-runtime-lib/_source_review/living_knowledge.py";
const expectedLivingManifest = "bmad-runtime-lib/knowledge-base/manifest.json";
if (
  record.validator !== expectedValidator ||
  record.manifest !== expectedManifest ||
  record.livingValidator !== expectedLivingValidator ||
  record.livingManifest !== expectedLivingManifest
) {
  fail("the verification record points outside the reviewed validator and manifest paths");
}

const validatorBytes = await bytes(join(root, ...record.validator.split("/")));
const manifestBytes = await bytes(join(root, ...record.manifest.split("/")));
const livingValidatorBytes = await bytes(join(root, ...record.livingValidator.split("/")));
const livingManifestBytes = await bytes(join(root, ...record.livingManifest.split("/")));
if (sha256(validatorBytes) !== record.validatorSha256) fail("the reviewed validator hash drifted");
if (sha256(manifestBytes) !== record.manifestSha256) fail("the frozen manifest hash drifted");
if (sha256(livingValidatorBytes) !== record.livingValidatorSha256) {
  fail("the living-knowledge validator hash drifted");
}
if (sha256(livingManifestBytes) !== record.livingManifestSha256) {
  fail("the living-knowledge manifest hash drifted");
}

const manifest = parseJson(manifestBytes, record.manifest);
const livingManifest = parseJson(livingManifestBytes, record.livingManifest);
try {
  await verifyClosedManifest({
    root: join(vaultRoot, "knowledge-base"),
    manifest: livingManifest,
    directories: ["current", "evidence"],
  });
} catch (error) {
  fail(error instanceof Error ? error.message : "living manifest verification failed");
}
if (!Array.isArray(manifest.files) || typeof manifest.metrics !== "object" || manifest.metrics === null) {
  fail("manifest.json does not have the required closed top-level data");
}

const entries = await readdir(vaultRoot, { withFileTypes: true });
const markdownNames = entries
  .filter((entry) => entry.isFile() && entry.name.toLowerCase().endsWith(".md"))
  .map((entry) => entry.name)
  .sort((left, right) => left.localeCompare(right, "en", { sensitivity: "base" }));
const records = new Map();
for (const item of manifest.files) {
  if (
    typeof item?.name !== "string" ||
    typeof item?.lines !== "number" ||
    typeof item?.bytes !== "number" ||
    !/^[0-9a-f]{64}$/.test(item?.sha256 ?? "") ||
    records.has(item.name)
  ) {
    fail("manifest.json contains a malformed or duplicate file record");
  }
  records.set(item.name, item);
}

if (
  markdownNames.length !== records.size ||
  markdownNames.some((name) => !records.has(name)) ||
  !records.has("100 - BMAD Method and Builder Deep Comprehension Audit.md")
) {
  fail("the root Markdown set differs from the frozen manifest or omits note 100");
}

let totalLines = 0;
let totalBytes = 0;
for (const name of markdownNames) {
  const payload = canonicalTextBytes(await bytes(join(vaultRoot, name)));
  const actual = { lines: lineCount(payload), bytes: payload.length, sha256: sha256(payload) };
  const expected = records.get(name);
  if (
    actual.lines !== expected.lines ||
    actual.bytes !== expected.bytes ||
    actual.sha256 !== expected.sha256
  ) {
    fail(`${name} differs from its frozen manifest record`);
  }
  totalLines += actual.lines;
  totalBytes += actual.bytes;
}

const metrics = manifest.metrics;
if (
  metrics.markdown_files !== markdownNames.length ||
  metrics.markdown_lines !== totalLines ||
  metrics.markdown_bytes !== totalBytes ||
  record.rootMarkdownFiles !== markdownNames.length
) {
  fail("aggregate Markdown metrics differ from the frozen validation record");
}

console.log(
  `Reference vault verified: ${markdownNames.length} root Markdown files; root and living validator/manifest hashes unchanged.`,
);
