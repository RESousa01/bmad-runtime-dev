#!/usr/bin/env node
// Machine-enforced 100-percent readiness scorecard (readiness program Task 2).
//
// A capability may be `complete` only when every required evidence kind is
// attached, every evidence record is bound to the same immutable source
// revision, and — where artifact identities appear — every record names the
// same installer hash and container digest. The checker never prints paths,
// secrets, tokens, or raw provider content: evidence references are bounded
// opaque strings.

import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

export const READINESS_CAPABILITIES = Object.freeze([
  "bmad_foundation",
  "full_bmad_breadth",
  "offline_developer_checkout",
  "reproducible_installable_offline_prototype",
  "complete_current_source_installable_exe",
  "deterministic_help_backend",
  "user_facing_deterministic_help",
  "production_model_backed_help",
  "d3_governed_edits_backend",
  "user_facing_governed_edits",
  "integrated_d2_d3_desktop",
  "first_honest_ai_desktop_prototype",
  "horizontal_governed_foundation",
  "internal_pilot_readiness",
]);

const SCORECARD_SCHEMA = "sapphirus.readiness-scorecard.v1";
const REVISION_PATTERN = /^[0-9a-f]{40}$/u;
const SHA256_PATTERN = /^[0-9a-f]{64}$/u;
const CONTAINER_DIGEST_PATTERN = /^sha256:[0-9a-f]{64}$/u;
const BOUNDED_REF_PATTERN = /^[\x20-\x7e]{1,512}$/u;
const EVIDENCE_KIND_PATTERN = /^[a-z][a-z0-9_]{0,63}$/u;

function violation(message) {
  throw new Error(message);
}

function requireKeys(record, keys, optionalKeys, label) {
  const allowed = new Set([...keys, ...optionalKeys]);
  for (const key of Object.keys(record)) {
    if (!allowed.has(key)) violation(`${label}: unknown key ${key}`);
  }
  for (const key of keys) {
    if (!(key in record)) violation(`${label}: missing key ${key}`);
  }
}

function validateEvidence(evidence, index, capability, expectedRevision, now) {
  if (typeof evidence !== "object" || evidence === null || Array.isArray(evidence)) {
    violation(`${capability}: evidence ${index} is not a record`);
  }
  requireKeys(
    evidence,
    ["kind", "sourceRevision", "observedAt", "urlOrRef"],
    ["installerSha256", "containerDigest", "expiresAt"],
    `${capability}: evidence ${index}`,
  );
  if (!EVIDENCE_KIND_PATTERN.test(String(evidence.kind))) {
    violation(`${capability}: evidence ${index} has an invalid kind`);
  }
  if (!REVISION_PATTERN.test(String(evidence.sourceRevision))) {
    violation(`${capability}: evidence ${index} has an invalid source revision`);
  }
  if (evidence.sourceRevision !== expectedRevision) {
    violation(`${capability}: evidence ${index} source revision mismatch`);
  }
  if (!Number.isSafeInteger(evidence.observedAt) || evidence.observedAt <= 0) {
    violation(`${capability}: evidence ${index} has an invalid observedAt`);
  }
  if (evidence.observedAt > now) {
    violation(`${capability}: evidence ${index} is observed in the future`);
  }
  if ("expiresAt" in evidence) {
    if (!Number.isSafeInteger(evidence.expiresAt) || evidence.expiresAt <= 0) {
      violation(`${capability}: evidence ${index} has an invalid expiresAt`);
    }
    if (evidence.expiresAt <= now) {
      violation(`${capability}: evidence ${index} is expired`);
    }
  }
  if (!BOUNDED_REF_PATTERN.test(String(evidence.urlOrRef))) {
    violation(`${capability}: evidence ${index} reference is not a bounded printable string`);
  }
  if ("installerSha256" in evidence && !SHA256_PATTERN.test(String(evidence.installerSha256))) {
    violation(`${capability}: evidence ${index} has an invalid installer hash`);
  }
  if ("containerDigest" in evidence && !CONTAINER_DIGEST_PATTERN.test(String(evidence.containerDigest))) {
    violation(`${capability}: evidence ${index} has an invalid container digest`);
  }
}

export function validateReadinessScorecard(scorecard, expectedRevision, now = Date.now()) {
  if (typeof scorecard !== "object" || scorecard === null || Array.isArray(scorecard)) {
    violation("scorecard is not a record");
  }
  requireKeys(scorecard, ["schemaVersion", "sourceRevision", "capabilities"], [], "scorecard");
  if (scorecard.schemaVersion !== SCORECARD_SCHEMA) {
    violation("scorecard schema version mismatch");
  }
  if (!REVISION_PATTERN.test(String(scorecard.sourceRevision))) {
    violation("scorecard source revision is not a 40-character commit");
  }
  if (typeof expectedRevision === "string" && scorecard.sourceRevision !== expectedRevision) {
    violation("scorecard source revision mismatch");
  }
  if (!Array.isArray(scorecard.capabilities)) {
    violation("scorecard capabilities is not an array");
  }

  const seen = new Set();
  const missing = [];
  let installerSha256 = null;
  let containerDigest = null;

  for (const record of scorecard.capabilities) {
    if (typeof record !== "object" || record === null || Array.isArray(record)) {
      violation("capability record is not a record");
    }
    requireKeys(
      record,
      ["capability", "status", "percentage", "requiredEvidenceKinds", "evidence"],
      [],
      String(record.capability ?? "capability"),
    );
    const capability = String(record.capability);
    if (!READINESS_CAPABILITIES.includes(capability)) {
      violation(`unknown capability ${capability}`);
    }
    if (seen.has(capability)) violation(`duplicate capability ${capability}`);
    seen.add(capability);

    if (record.status !== "incomplete" && record.status !== "complete") {
      violation(`${capability}: status must be incomplete or complete`);
    }
    if (
      !Number.isSafeInteger(record.percentage)
      || record.percentage < 0
      || record.percentage > 100
    ) {
      violation(`${capability}: percentage must be an integer between 0 and 100`);
    }
    if (record.status === "complete" && record.percentage !== 100) {
      violation(`${capability}: complete requires percentage 100`);
    }
    if (record.status === "incomplete" && record.percentage === 100) {
      violation(`${capability}: percentage 100 requires status complete`);
    }
    if (!Array.isArray(record.requiredEvidenceKinds) || record.requiredEvidenceKinds.length === 0) {
      violation(`${capability}: requiredEvidenceKinds must be a non-empty array`);
    }
    for (const kind of record.requiredEvidenceKinds) {
      if (!EVIDENCE_KIND_PATTERN.test(String(kind))) {
        violation(`${capability}: invalid required evidence kind`);
      }
    }
    if (!Array.isArray(record.evidence)) {
      violation(`${capability}: evidence must be an array`);
    }

    const kinds = new Set();
    record.evidence.forEach((evidence, index) => {
      validateEvidence(evidence, index, capability, scorecard.sourceRevision, now);
      if (kinds.has(evidence.kind)) {
        violation(`${capability}: duplicate evidence kind ${evidence.kind}`);
      }
      kinds.add(evidence.kind);
      if ("installerSha256" in evidence) {
        if (installerSha256 !== null && installerSha256 !== evidence.installerSha256) {
          violation(`${capability}: installer hash mismatch across evidence`);
        }
        installerSha256 = evidence.installerSha256;
      }
      if ("containerDigest" in evidence) {
        if (containerDigest !== null && containerDigest !== evidence.containerDigest) {
          violation(`${capability}: container digest mismatch across evidence`);
        }
        containerDigest = evidence.containerDigest;
      }
    });

    const absentKinds = record.requiredEvidenceKinds.filter((kind) => !kinds.has(kind));
    if (record.status === "complete" && absentKinds.length > 0) {
      violation(`${capability}: complete without required evidence ${absentKinds.join(", ")}`);
    }
    if (record.status !== "complete") {
      missing.push({ capability, percentage: record.percentage, absentKinds });
    }
  }

  for (const capability of READINESS_CAPABILITIES) {
    if (!seen.has(capability)) violation(`missing capability ${capability}`);
  }

  return { releaseReady: missing.length === 0, missing };
}

function runCli() {
  const repoRoot = process.cwd();
  const scorecardPath = path.join(repoRoot, "docs", "readiness", "100-percent-scorecard.json");
  const scorecard = JSON.parse(readFileSync(scorecardPath, "utf8"));
  const head = execFileSync("git", ["rev-parse", "HEAD"], { cwd: repoRoot, encoding: "utf8" }).trim();

  let result;
  try {
    result = validateReadinessScorecard(scorecard, null);
  } catch (error) {
    console.error(`Readiness scorecard invalid: ${error.message}`);
    process.exit(1);
  }

  const lines = [];
  if (scorecard.sourceRevision !== head) {
    lines.push(
      `- scorecard is bound to revision ${scorecard.sourceRevision.slice(0, 12)}…, HEAD is ${head.slice(0, 12)}…`,
    );
  }
  for (const entry of result.missing) {
    const detail = entry.absentKinds.length > 0
      ? ` (missing evidence: ${entry.absentKinds.join(", ")})`
      : "";
    lines.push(`- ${entry.capability}: ${entry.percentage}%${detail}`);
  }

  if (lines.length === 0 && result.releaseReady) {
    console.log("Readiness scorecard: 14/14 capabilities complete on the bound revision.");
    return;
  }
  console.error("Readiness scorecard is not release-ready:\n" + lines.join("\n"));
  process.exit(1);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  runCli();
}
