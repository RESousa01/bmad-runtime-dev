import { existsSync } from "node:fs";
import { resolve } from "node:path";
import {
  fail,
  parseArgs,
  readJson,
  rootFrom,
  sha256File,
} from "./lib.mjs";

const DECISIONS = new Set([
  "sealed_input",
  "reference_only",
  "clean_room_pattern_only",
  "excluded",
  "legal_review_required",
]);

try {
  const options = parseArgs(process.argv.slice(2));
  const root = rootFrom(options);
  const lockPath = resolve(root, options.lock ?? "reference-sources.lock.json");
  if (!existsSync(lockPath)) {
    const manifestPath = resolve(root, "docs/_meta/knowledge-manifest.json");
    const manifest = existsSync(manifestPath) ? readJson(manifestPath) : {};
    if (manifest.sourceLockRequired === false) {
      process.stdout.write("Reference source lock intentionally blocked by migration manifest\n");
      process.exit(0);
    }
    throw new Error("reference-sources.lock.json is missing");
  }
  const lock = readJson(lockPath);
  for (const source of lock.sources ?? []) {
    for (const field of [
      "id",
      "purpose",
      "repositoryUrl",
      "version",
      "commit",
      "archiveSha256",
      "license",
      "acquiredAt",
      "storageLocator",
      "includedPaths",
      "excludedPaths",
      "releaseDecision",
    ]) {
      if (source[field] === undefined || source[field] === "") {
        throw new Error(`${source.id ?? "<source>"}: missing ${field}`);
      }
    }
    if (!/^https:\/\/.+/.test(source.repositoryUrl)) {
      throw new Error(`${source.id}: repository URL must be canonical HTTPS`);
    }
    if (!/^[0-9a-f]{40}$/.test(source.commit)) {
      throw new Error(`${source.id}: invalid immutable commit`);
    }
    if (!/^[0-9a-f]{64}$/.test(source.archiveSha256)) {
      throw new Error(`${source.id}: invalid archive SHA-256`);
    }
    if (!DECISIONS.has(source.releaseDecision)) {
      throw new Error(`${source.id}: invalid release decision`);
    }
    if (/example|placeholder|changeme/i.test(source.storageLocator)) {
      throw new Error(`${source.id}: storage locator is not approved`);
    }
  }
  if (options.archive) {
    if (!options["source-id"]) throw new Error("--source-id is required with --archive");
    const source = (lock.sources ?? []).find((entry) => entry.id === options["source-id"]);
    if (!source) throw new Error(`unknown source id ${options["source-id"]}`);
    const actual = sha256File(resolve(options.archive));
    if (actual !== source.archiveSha256) throw new Error(`${source.id}: supplied archive hash mismatch`);
  }
  process.stdout.write(`Reference sources verified (${lock.sources?.length ?? 0})\n`);
} catch (error) {
  fail(error);
}

