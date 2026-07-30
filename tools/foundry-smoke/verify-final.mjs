#!/usr/bin/env node

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const defaultRoot = resolve(scriptDirectory, "..", "..");
const argumentsList = process.argv.slice(2);
const strict = argumentsList.includes("--strict");
const rootIndex = argumentsList.indexOf("--root");
const outputIndex = argumentsList.indexOf("--output");
const repositoryRoot =
  rootIndex >= 0 ? resolve(argumentsList[rootIndex + 1]) : defaultRoot;
const outputPath =
  outputIndex >= 0
    ? resolve(repositoryRoot, argumentsList[outputIndex + 1])
    : join(repositoryRoot, "artifacts", "foundry", "final-scorecard.json");

const taskIds = [
  "FND-001", "FND-002", "FND-003",
  "FND-010", "FND-011", "FND-012", "FND-013",
  "FND-020", "FND-021", "FND-022", "FND-023", "FND-024", "FND-025",
  "FND-026", "FND-027", "FND-028", "FND-029", "FND-030", "FND-031",
  "FND-040", "FND-041", "FND-042", "FND-043", "FND-044", "FND-045",
  "FND-046", "FND-050", "FND-051", "FND-052", "FND-053",
  "FND-060", "FND-061", "FND-062", "FND-063", "FND-064", "FND-065",
];

const evidenceFiles = {
  runState: ".foundry-connection/run-state.json",
  prerequisites: "artifacts/foundry/prerequisites.json",
  localMatrix: "artifacts/foundry/qualification/local-matrix.json",
  deployedSmoke: "artifacts/foundry/qualification/deployed-smoke.json",
  privacyInspection: "artifacts/foundry/qualification/privacy-inspection.json",
  independentReview: "artifacts/foundry/independent-review.json",
};

function digest(content) {
  return `sha256:${createHash("sha256").update(content).digest("hex")}`;
}

function readJson(relativePath, blockers, code) {
  const fullPath = join(repositoryRoot, relativePath);
  try {
    const raw = readFileSync(fullPath);
    if (raw.length > 8 * 1024 * 1024) {
      blockers.push({ code: `${code}_too_large`, evidencePath: relativePath });
      return null;
    }
    return { value: JSON.parse(raw.toString("utf8")), hash: digest(raw) };
  } catch (error) {
    blockers.push({
      code,
      evidencePath: relativePath,
      detail: error instanceof SyntaxError ? "invalid_json" : "missing",
    });
    return null;
  }
}

function isSha256(value) {
  return typeof value === "string" && /^sha256:[0-9a-f]{64}$/.test(value);
}

function repositoryCommit() {
  try {
    return execFileSync("git", ["-C", repositoryRoot, "rev-parse", "HEAD"], {
      encoding: "utf8",
      windowsHide: true,
    }).trim();
  } catch {
    return null;
  }
}

const blockers = [];
const evidenceHashes = {};
const loaded = {};
for (const [name, relativePath] of Object.entries(evidenceFiles)) {
  const result = readJson(relativePath, blockers, `${name}_evidence_unavailable`);
  loaded[name] = result?.value ?? null;
  if (result) {
    evidenceHashes[name] = result.hash;
  }
}

const commit = repositoryCommit();
if (!commit) {
  blockers.push({ code: "repository_commit_unavailable" });
}

const taskStates = loaded.runState?.taskStates ?? {};
for (const taskId of taskIds) {
  if (taskStates[taskId]?.state !== "complete") {
    blockers.push({
      code: "task_not_complete",
      taskId,
      state: taskStates[taskId]?.state ?? "missing",
    });
  }
}

const prerequisite = loaded.prerequisites;
if (
  prerequisite?.schemaVersion !== "forge.foundry-prerequisites.v2" ||
  prerequisite?.ready !== true ||
  prerequisite?.credential?.mechanism !== "api_key" ||
  prerequisite?.checks?.foundryEntraTokenAcquired !== false ||
  prerequisite?.checks?.responsesCompatibleRouteProved !== true ||
  !["configured_and_probed", "openai_models", "project_deployments_api_key"]
    .includes(prerequisite?.selectedInventoryMode) ||
  !isSha256(prerequisite?.projectEndpointHash) ||
  !isSha256(prerequisite?.routeEvidenceHash)
) {
  blockers.push({
    code: "prerequisites_not_ready",
    evidencePath: evidenceFiles.prerequisites,
  });
}

const localMatrix = loaded.localMatrix;
if (
  localMatrix?.ready !== true ||
  localMatrix?.repositoryCommit !== commit ||
  Number(localMatrix?.summary?.failed ?? 1) !== 0 ||
  Number(localMatrix?.summary?.unavailableMandatory ?? 1) !== 0
) {
  blockers.push({
    code: "local_matrix_not_current_and_ready",
    evidencePath: evidenceFiles.localMatrix,
  });
}

const deployedSmoke = loaded.deployedSmoke;
if (
  deployedSmoke?.ready !== true ||
  deployedSmoke?.final !== true ||
  deployedSmoke?.authenticationMechanism !== "api_key" ||
  deployedSmoke?.keyRotation?.passed !== true ||
  deployedSmoke?.rollback?.passed !== true
) {
  blockers.push({
    code: "deployed_smoke_not_final",
    evidencePath: evidenceFiles.deployedSmoke,
  });
}

if (
  loaded.privacyInspection?.passed !== true ||
  Number(loaded.privacyInspection?.forbiddenFindingCount ?? 1) !== 0
) {
  blockers.push({
    code: "privacy_inspection_not_clean",
    evidencePath: evidenceFiles.privacyInspection,
  });
}

if (
  loaded.independentReview?.verdict !== "looks_good" ||
  Number(loaded.independentReview?.blockers?.length ?? 1) !== 0
) {
  blockers.push({
    code: "independent_review_not_approved",
    evidencePath: evidenceFiles.independentReview,
  });
}

const uniqueBlockers = [
  ...new Map(
    blockers.map((blocker) => [
      JSON.stringify([blocker.code, blocker.taskId, blocker.evidencePath]),
      blocker,
    ]),
  ).values(),
];

const scorecard = {
  schemaVersion: "forge.foundry-final-scorecard.v1",
  generatedAt: new Date().toISOString(),
  repositoryCommit: commit,
  complete: uniqueBlockers.length === 0,
  authenticationMechanism: "api_key",
  foundryEntraRequired: false,
  inventoryMode: prerequisite?.selectedInventoryMode ?? null,
  projectEndpointHash: prerequisite?.projectEndpointHash ?? null,
  prerequisiteEvidenceHash: evidenceHashes.prerequisites ?? null,
  deploymentSnapshotHash:
    deployedSmoke?.deploymentSnapshotHash ??
    prerequisite?.routeEvidenceHash ??
    null,
  credentialVersionBinding:
    deployedSmoke?.credentialVersionBinding ??
    prerequisite?.credential?.versionHash ??
    null,
  evidenceHashes,
  blockers: uniqueBlockers,
  honesty: [
    "complete is true only with current local evidence, real deployed API-key evidence, key rotation, privacy inspection, rollback, and independent review.",
    "Fake-server or source-only evidence cannot satisfy deployed qualification.",
  ],
};

mkdirSync(dirname(outputPath), { recursive: true });
writeFileSync(outputPath, `${JSON.stringify(scorecard, null, 2)}\n`, "utf8");
process.stdout.write(
  `Foundry final verification: complete=${scorecard.complete} blockers=${uniqueBlockers.length}\n`,
);
process.stdout.write(`Scorecard: ${outputPath}\n`);

if (strict && !scorecard.complete) {
  process.exitCode = 2;
}
