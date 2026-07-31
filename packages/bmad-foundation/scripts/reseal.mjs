#!/usr/bin/env node
// Reseals the foundation's computed identity chain after a reviewed edit.
//
// Several foundation values are hashes over other foundation values: the help
// package descriptor hashes its own body (including `sourceIdentity`), the
// runtime manifest hashes the ledger bytes and every resource, and each
// instruction-projection envelope hashes its projection. Editing any sealed
// byte by hand therefore breaks a chain that spans four files.
//
// This script recomputes that chain with exactly the algorithm
// `verify.mjs` checks it with (`canonicalize` + the `sapphirus:<purpose>:v1`
// preimage), so a reviewed change is resealed reproducibly instead of by
// transcribing digests. It never *decides* anything: it only recomputes what
// the current bytes imply. `verify.mjs` remains the gate.
//
// Usage: node scripts/reseal.mjs [--check]
//   --check  report what would change and exit non-zero, without writing.

import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const checkOnly = process.argv.includes("--check");

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

// Byte-identical to verify.mjs `canonicalize`.
function canonicalize(value) {
  if (value === null || typeof value === "boolean" || typeof value === "string") {
    return JSON.stringify(value);
  }
  if (typeof value === "number" && Number.isFinite(value)) {
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map(canonicalize).join(",")}]`;
  }
  if (typeof value === "object") {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonicalize(value[key])}`)
      .join(",")}}`;
  }
  throw new Error("non-JSON value in hash preimage");
}

function canonicalValueHash(purpose, value) {
  return `sha256:${sha256(`sapphirus:${purpose}:v1\n${canonicalize(value)}`)}`;
}

function canonicalDocumentHash(purpose, value, excludedField) {
  return canonicalValueHash(
    purpose,
    Object.fromEntries(Object.entries(value).filter(([key]) => key !== excludedField)),
  );
}

async function readJson(relativePath) {
  return JSON.parse(await readFile(join(packageRoot, relativePath), "utf8"));
}

const changes = [];

/// Writes `value` as the package's canonical two-space JSON with a trailing
/// newline — the shape every existing foundation file already uses.
async function writeJson(relativePath, value) {
  const absolute = join(packageRoot, relativePath);
  const next = `${JSON.stringify(value, null, 2)}\n`;
  const current = await readFile(absolute, "utf8").catch(() => null);
  if (current === next) return false;
  changes.push(relativePath);
  if (!checkOnly) await writeFile(absolute, next, "utf8");
  return true;
}

/// Rebinds each managed output's recorded bytes in the semantic ledger. Runs
/// first: the runtime manifest hashes the ledger's bytes, so the ledger must
/// already be settled when the manifest is sealed.
async function resealManagedOutputs() {
  const path = "semantic-source-ledger.json";
  const ledger = await readJson(path);
  const sourceManifest = await readJson("source-manifest.json");
  const sourceHashes = new Map(
    sourceManifest.members.map((member) => [
      `${member.source_id}:${member.path}`,
      member.sha256,
    ]),
  );
  for (const member of ledger.sourceMembers ?? []) {
    const currentHash = sourceHashes.get(`${member.sourceId}:${member.member}`);
    if (currentHash !== undefined) member.sha256 = currentHash;
  }
  for (const output of ledger.managedOutputs ?? []) {
    const bytes = await readFile(join(packageRoot, output.path));
    output.byteLength = bytes.byteLength;
    output.sha256 = sha256(bytes);
  }
  await writeJson(path, ledger);
}

/// Reseals the help package descriptor: its `descriptorHash` covers its own
/// body, so any reviewed change to `sourceIdentity` lands here first.
async function resealHelpDescriptor() {
  const path = "normalized/bmad-help.package.json";
  const descriptor = await readJson(path);
  const methodSourceRoot = join(packageRoot, "source", "method", "6.10.0");
  const sourceManifest = await readJson("source-manifest.json");
  const methodPaths = sourceManifest.members
    .filter((member) => member.source_id === "method")
    .map((member) => member.path);
  const resolveCurrentSource = async (source) => {
    let bytes = await readFile(join(methodSourceRoot, source.path)).catch(() => null);
    if (bytes !== null) return bytes;
    const suffix = source.path.split("/").slice(-2).join("/");
    const candidates = methodPaths.filter((candidate) => candidate.endsWith(`/${suffix}`));
    if (candidates.length !== 1) return null;
    source.path = candidates[0];
    bytes = await readFile(join(methodSourceRoot, source.path)).catch(() => null);
    return bytes;
  };
  const retainedResourceInventory = [];
  for (const entry of descriptor.resourceInventory ?? []) {
    if (entry.locationKind !== "source_tree") continue;
    const bytes = await resolveCurrentSource(entry);
    if (bytes === null) continue;
    entry.contentHash = `sha256:${sha256(bytes)}`;
    entry.byteLength = bytes.byteLength;
    retainedResourceInventory.push(entry);
  }
  descriptor.resourceInventory = [
    ...(descriptor.resourceInventory ?? []).filter((entry) => entry.locationKind !== "source_tree"),
    ...retainedResourceInventory,
  ];
  // The descriptor also carries a global inventory binding every managed
  // projection by content hash, which `verify.mjs` checks against the real
  // bytes. Refreshing only the projection envelopes left that inventory stale,
  // so a reviewed edit to any instruction file still failed the gate.
  for (const entry of descriptor.resourceInventory ?? []) {
    if (entry.locationKind !== "managed_projection") continue;
    const bytes = await readFile(join(packageRoot, entry.path));
    entry.contentHash = `sha256:${sha256(bytes)}`;
    entry.byteLength = bytes.byteLength;
  }
  // …and a composite hash over that same inventory, which is likewise derived
  // from the real bytes rather than declared.
  if (descriptor.finalCompositeInventoryHash !== undefined) {
    const inventory = (descriptor.resourceInventory ?? [])
      .filter(({ locationKind }) => locationKind === "managed_projection")
      .map(({ path: resourcePath, contentHash, byteLength }) => ({
        path: resourcePath,
        locationKind: "managed_projection",
        contentHash,
        byteLength,
      }));
    descriptor.finalCompositeInventoryHash = canonicalValueHash(
      "bmad-final-composite-inventory",
      inventory,
    );
  }
  for (const projection of descriptor.instructionProjections ?? []) {
    if (projection.sourceEntrypoint?.path !== undefined) {
      const bytes = await resolveCurrentSource(projection.sourceEntrypoint);
      if (bytes === null) throw new Error(`missing projection entrypoint ${projection.sourceEntrypoint.path}`);
      projection.sourceEntrypoint.contentHash = `sha256:${sha256(bytes)}`;
    }
    const retainedSourceResources = [];
    for (const source of projection.sourceResources ?? []) {
      const bytes = await resolveCurrentSource(source);
      if (bytes === null) continue;
      source.contentHash = `sha256:${sha256(bytes)}`;
      retainedSourceResources.push(source);
    }
    projection.sourceResources = retainedSourceResources;
    // The projection binds its managed instruction by content hash, and its
    // identifier is derived from that same hash. Both follow the bytes.
    const managed = projection.managedInstruction;
    if (managed?.path !== undefined) {
      const bytes = await readFile(join(packageRoot, managed.path));
      const digest = sha256(bytes);
      managed.contentHash = `sha256:${digest}`;
      projection.projectionId = `projection_${digest.toUpperCase()}`;
    }
    projection.projectionHash = canonicalDocumentHash(
      "bmad-instruction-projection",
      projection,
      "projectionHash",
    );
  }
  descriptor.descriptorHash = canonicalDocumentHash(
    "bmad-package-descriptor",
    descriptor,
    "descriptorHash",
  );
  await writeJson(path, descriptor);
}

/// Reseals every Method projection envelope (`normalized/*.package.json` that
/// carries `instructionProjection` rather than a full descriptor).
async function resealProjectionEnvelopes() {
  const manifest = await readJson("runtime-manifest.json");
  const envelopes = manifest.resources
    .map(({ path }) => path)
    .filter((path) => path.startsWith("normalized/") && path !== "normalized/bmad-help.package.json");
  for (const path of envelopes) {
    const envelope = await readJson(path);
    if (envelope.instructionProjection === undefined) continue;
    const managed = envelope.instructionProjection.managedInstruction;
    if (managed !== undefined) {
      const bytes = await readFile(join(packageRoot, managed.path));
      managed.contentHash = `sha256:${sha256(bytes)}`;
    }
    envelope.instructionProjection.projectionHash = canonicalDocumentHash(
      "bmad-instruction-projection",
      envelope.instructionProjection,
      "projectionHash",
    );
    envelope.projectionEnvelopeHash = canonicalDocumentHash(
      "bmad-foundation-method-projection",
      envelope,
      "projectionEnvelopeHash",
    );
    await writeJson(path, envelope);
  }
}

/// Reseals the runtime manifest last: it hashes the semantic ledger bytes and
/// every resource, so it must observe the already-resealed files.
async function resealRuntimeManifest() {
  const path = "runtime-manifest.json";
  const manifest = await readJson(path);
  const semanticBytes = await readFile(join(packageRoot, "semantic-source-ledger.json"));
  manifest.semanticLedgerHash = `sha256:${sha256(semanticBytes)}`;
  for (const resource of manifest.resources) {
    const bytes = await readFile(join(packageRoot, resource.path));
    resource.contentHash = `sha256:${sha256(bytes)}`;
    resource.byteLength = bytes.byteLength;
  }
  manifest.resources.sort((left, right) => (left.path < right.path ? -1 : left.path > right.path ? 1 : 0));
  manifest.manifestHash = canonicalDocumentHash(
    "bmad-foundation-runtime-manifest",
    manifest,
    "manifestHash",
  );
  await writeJson(path, manifest);
}

await resealManagedOutputs();
await resealHelpDescriptor();
await resealProjectionEnvelopes();
await resealRuntimeManifest();

// The reviewed semantic lock in verify.mjs stays a *manual* edit by design —
// the gate refuses automatic lock updates, because the lock is the human
// checkpoint on ledger content. Report the value instead of writing it.
const semanticBytes = await readFile(join(packageRoot, "semantic-source-ledger.json"));
const lock = `  byteLength: ${semanticBytes.byteLength},\n  sha256: "${sha256(semanticBytes)}",`;

if (changes.length === 0) {
  process.stdout.write("foundation identity chain already sealed\n");
} else if (checkOnly) {
  process.stderr.write(`foundation identity chain is stale:\n  ${changes.join("\n  ")}\n`);
  process.exitCode = 1;
} else {
  process.stdout.write(
    `resealed foundation identity chain:\n  ${changes.join("\n  ")}\n\n`
      + `reviewed semantic lock for scripts/verify.mjs (apply by hand):\n${lock}\n`,
  );
}
