import { existsSync } from "node:fs";
import { resolve } from "node:path";
import {
  fail,
  parseArgs,
  readJson,
  rootFrom,
  sha256File,
} from "./lib.mjs";

try {
  const options = parseArgs(process.argv.slice(2));
  const root = rootFrom(options);
  const manifestPath = resolve(
    root,
    options.manifest ?? "bmad-runtime-lib/archive-manifest.json",
  );
  if (!existsSync(manifestPath)) throw new Error("archive manifest is missing");
  const manifest = readJson(manifestPath);
  const seenOriginal = new Set();
  for (const file of manifest.files ?? []) {
    if (seenOriginal.has(file.originalPath)) {
      throw new Error(`duplicate archive original path: ${file.originalPath}`);
    }
    seenOriginal.add(file.originalPath);
    if (!/^[0-9a-f]{64}$/.test(file.sha256)) {
      throw new Error(`${file.originalPath}: invalid archive SHA-256`);
    }
    const archived = resolve(root, file.archivePath);
    if (!existsSync(archived)) throw new Error(`${file.originalPath}: archived file is missing`);
    if (sha256File(archived) !== file.sha256) {
      throw new Error(`${file.originalPath}: archive hash mismatch`);
    }
  }
  process.stdout.write(`Archive manifest verified (${seenOriginal.size} files)\n`);
} catch (error) {
  fail(error);
}

