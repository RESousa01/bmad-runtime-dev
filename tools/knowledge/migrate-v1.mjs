import { resolve } from "node:path";
import {
  fail,
  parseArgs,
  readJson,
  rootFrom,
  writeGenerated,
} from "./lib.mjs";

try {
  const options = parseArgs(process.argv.slice(2));
  const root = rootFrom(options);
  const evidenceRoot = resolve(root, "bmad-runtime-lib/knowledge-base/evidence");
  const outputDir = resolve(root, options["output-dir"] ?? "docs/_meta");
  const oldClaims = readJson(resolve(evidenceRoot, "claims.json"));
  const oldSources = readJson(resolve(evidenceRoot, "sources.json"));
  const oldCatalog = readJson(resolve(evidenceRoot, "note-catalog.json"));
  const claims = {
    schemaVersion: "forge.knowledge.v2",
    migrationSource: "sapphirus.living-knowledge.v1",
    claims: (oldClaims.claims ?? []).map((claim) => ({
      id: claim.id,
      subject: claim.subject,
      class: "unknown",
      statement: claim.statement,
      status: "needs_reverification",
      ownerRole: "forge-maintainers",
      verifiedAt: null,
      reviewDue: null,
      evidence: [],
      supersedes: [],
      legacyRecord: claim,
    })),
  };
  const sources = {
    schemaVersion: "forge.knowledge.v2",
    migrationSource: "sapphirus.living-knowledge.v1",
    sources: (oldSources.sources ?? []).map((source) => ({
      ...source,
      status: "needs_reverification",
    })),
  };
  const catalog = {
    schemaVersion: "forge.knowledge.v2",
    migrationSource: "sapphirus.living-knowledge.v1",
    notes: oldCatalog.notes ?? [],
  };
  writeGenerated(resolve(outputDir, "claims.migrated.json"), claims, false);
  writeGenerated(resolve(outputDir, "sources.migrated.json"), sources, false);
  writeGenerated(resolve(outputDir, "note-catalog.migrated.json"), catalog, false);
  process.stdout.write(`Migrated v1 evidence to ${outputDir} without promoting current claims\n`);
} catch (error) {
  fail(error);
}

