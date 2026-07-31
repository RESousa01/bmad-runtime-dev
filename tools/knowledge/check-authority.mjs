import { resolve } from "node:path";
import {
  documentRecord,
  fail,
  manifestDocumentPaths,
  normalizedRelative,
  parseArgs,
  readJson,
  rootFrom,
} from "./lib.mjs";

function activeBody(record) {
  return record.body;
}

try {
  const options = parseArgs(process.argv.slice(2));
  const root = rootFrom(options);
  const manifest = readJson(resolve(root, "docs/_meta/knowledge-manifest.json"));
  const documents = manifestDocumentPaths(manifest).map((path) => documentRecord(root, path));
  const entrypoint = normalizedRelative(manifest.entrypoint ?? "");
  const productState = normalizedRelative(manifest.productState ?? "");
  if (!entrypoint || documents.filter((record) => record.path === entrypoint).length !== 1) {
    throw new Error("exactly one team entrypoint is required");
  }
  if (!productState || documents.filter((record) => record.path === productState).length !== 1) {
    throw new Error("exactly one current product-state document is required");
  }
  for (const record of documents) {
    if (
      record.data.status === "current" &&
      !record.path.startsWith("docs/current/") &&
      record.path !== entrypoint
    ) {
      throw new Error(`${record.path}: status current is not allowed outside docs/current`);
    }
    if (["plan", "review"].includes(record.data.doc_type) && record.data.status === "current") {
      throw new Error(`${record.path}: plan or review cannot claim current authority`);
    }
    const body = activeBody(record);
    if (/\b(?:Sapphirus Desktop|Develop Sapphirus|Sapphirus is)\b/i.test(body)) {
      throw new Error(`${record.path}: stale product name`);
    }
    for (const line of body.split(/\r?\n/)) {
      if (!/sapphirus/i.test(line)) continue;
      if (/legacy identifier allowlist|legacy compatibility|`sapphirus[.*_]/i.test(line)) continue;
      throw new Error(`${record.path}: former-name text is outside the legacy identifier allowlist`);
    }
    if (/\b(?:Entra\s+is\s+required|required[^.\n]*\bEntra|must\s+use\s+Entra)\b/i.test(body)) {
      throw new Error(`${record.path}: Entra requirement is not current authority`);
    }
    if (
      record.path.startsWith("docs/current/") ||
      record.path.startsWith("docs/architecture/")
    ) {
      if (/\bCortex\b/i.test(body)) {
        throw new Error(`${record.path}: Cortex is not current Forge architecture`);
      }
    }
    if (/\]\([^)]*bmad-runtime-lib[^)]*\)/i.test(body)) {
      throw new Error(`${record.path}: active document links to a legacy note as authority`);
    }
  }
  process.stdout.write("Documentation authority verified\n");
} catch (error) {
  fail(error);
}

