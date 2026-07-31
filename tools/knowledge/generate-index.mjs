import { resolve } from "node:path";
import {
  documentRecord,
  fail,
  manifestDocumentPaths,
  parseArgs,
  readJson,
  rootFrom,
  writeGenerated,
} from "./lib.mjs";

export function generateIndex(root) {
  const manifest = readJson(resolve(root, "docs/_meta/knowledge-manifest.json"));
  const documents = manifestDocumentPaths(manifest)
    .map((path) => documentRecord(root, path))
    .map(({ path, data }) => ({
      path,
      title: data.title,
      docType: data.doc_type,
      status: data.status,
      ownerRole: data.owner_role,
      lastVerified: data.last_verified,
      reviewDue: data.review_due,
    }))
    .sort((left, right) => left.path.localeCompare(right.path));
  const currentTitles = new Map();
  for (const document of documents.filter((entry) => entry.status === "current")) {
    if (currentTitles.has(document.title)) {
      throw new Error(
        `duplicate current title: ${document.title} (${currentTitles.get(document.title)}, ${document.path})`,
      );
    }
    currentTitles.set(document.title, document.path);
  }
  return {
    schemaVersion: "forge.knowledge.v2",
    generatedBy: "tools/knowledge/generate-index.mjs",
    documents,
  };
}

try {
  const options = parseArgs(process.argv.slice(2));
  const root = rootFrom(options);
  const output = resolve(root, options.output ?? "docs/_meta/note-index.generated.json");
  writeGenerated(output, generateIndex(root), options.check, "generated note index is stale");
  process.stdout.write(`${options.check ? "Verified" : "Generated"} ${output}\n`);
} catch (error) {
  fail(error);
}

