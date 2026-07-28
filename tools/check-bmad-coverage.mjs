/**
 * Checks the upstream-to-runtime BMAD skill coverage matrix against what the
 * foundation package actually exposes.
 *
 * Backlog item BM-02 asks for a complete matrix: all 48 upstream directories
 * containing a real SKILL.md, each ending in one explicit state. Audit finding
 * F-19 is why -- the embedded snapshot holds 228 files against 597 upstream, so
 * "BMAD 6.10.0" cannot be claimed from a version string or a folder name. F-21
 * is the sharper form: without a checked matrix the UI can overstate what is
 * available, or route a capability to a legacy name, and nothing notices.
 *
 * This decides nothing. Deciding is BM-03, which records an
 * adopt/adapt/shim/defer/reject ledger with owners and semantic tests. This
 * only refuses to let the matrix and the capability closure drift apart in
 * silence.
 *
 * Why it lives in tools/ rather than the path BM-02 names
 * ------------------------------------------------------
 * BM-02 lists `packages/bmad-foundation/scripts/coverage.mjs`. That package is
 * sealed: `foundation.test.mjs` asserts its exact root entry list and that
 * `package.json.files` deep-equals a reviewed constant, so adding any file to
 * it fails by design. That seal is the point -- it stops casual additions to a
 * hash-locked corpus -- and breaking it to satisfy a path would trade a real
 * invariant for a cosmetic one. The matrix is *about* the foundation, not part
 * of the imported material, so the checker sits outside and reads in. The
 * human-readable half is at the path BM-02 does name, `docs/bmad/skill-coverage.md`.
 */

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const read = (relative) => JSON.parse(readFileSync(path.join(root, relative), "utf8"));

const matrix = read("docs/bmad/skill-coverage.json");
const closure = read("packages/bmad-foundation/capability-closure-ledger.json");

const SNAPSHOT_STATES = new Set(["identical", "modified", "relocated_or_legacy", "missing"]);
const RUNTIME_STATES = new Set([
  "active_projection",
  "compatibility_shim",
  "legacy_split_active",
  "normalized_not_in_closure",
  "not_projected",
  "upstream_shim_unprojected",
]);
// BM-03 fills these in. Null until then, and null is a state, not an omission.
const DECISIONS = new Set(["adopt", "adapt", "shim", "defer", "reject", null]);

// BM-02's first acceptance criterion: the denominator is the pinned upstream
// discovery result, not however many rows someone happened to write.
const PINNED_UPSTREAM_SKILL_COUNT = 48;

const problems = [];
const capabilityIds = new Set(closure.capabilities.map((c) => c.capabilityId));
const activeCapabilityIds = new Set(
  closure.capabilities.filter((c) => c.activationStatus === "active").map((c) => c.capabilityId),
);

if (matrix.skills.length !== PINNED_UPSTREAM_SKILL_COUNT) {
  problems.push(
    `the matrix has ${matrix.skills.length} rows; the pinned upstream discovered `
    + `${PINNED_UPSTREAM_SKILL_COUNT} skills. A coverage figure over the wrong denominator is worse `
    + "than none.",
  );
}

const seen = new Set();
for (const skill of matrix.skills) {
  const where = `row ${skill.index} (${skill.upstreamSkill})`;
  if (seen.has(skill.upstreamSkill)) problems.push(`${where}: duplicate upstream skill`);
  seen.add(skill.upstreamSkill);

  if (!skill.upstreamPath?.endsWith("SKILL.md")) {
    problems.push(`${where}: upstreamPath must point at a SKILL.md`);
  }
  if (!SNAPSHOT_STATES.has(skill.snapshotState)) {
    problems.push(`${where}: unknown snapshotState "${skill.snapshotState}"`);
  }
  if (!RUNTIME_STATES.has(skill.runtimeState)) {
    problems.push(`${where}: unknown runtimeState "${skill.runtimeState}"`);
  }
  if (!DECISIONS.has(skill.decision)) {
    problems.push(`${where}: unknown decision "${skill.decision}"`);
  }

  if (skill.capabilityId !== null && !capabilityIds.has(skill.capabilityId)) {
    problems.push(
      `${where}: names capability "${skill.capabilityId}", which the closure ledger does not define`,
    );
  }
  if (skill.runtimeState === "active_projection") {
    if (skill.capabilityId === null) {
      problems.push(`${where}: is an active_projection with no capability`);
    } else if (!activeCapabilityIds.has(skill.capabilityId)) {
      problems.push(
        `${where}: is an active_projection, but "${skill.capabilityId}" is not active in the closure ledger`,
      );
    }
  }
  if (skill.runtimeState === "not_projected" && skill.capabilityId !== null) {
    problems.push(`${where}: is not_projected yet names a capability`);
  }
}

const tally = (key) => matrix.skills.reduce((acc, s) => {
  const value = s[key] ?? "—";
  acc[value] = (acc[value] ?? 0) + 1;
  return acc;
}, {});

if (matrix.totals.upstreamSkills !== matrix.skills.length) {
  problems.push(
    `totals.upstreamSkills says ${matrix.totals.upstreamSkills}, the matrix has ${matrix.skills.length}`,
  );
}
for (const key of ["snapshotState", "runtimeState", "auditPriority"]) {
  const recomputed = tally(key);
  const recorded = matrix.totals[key] ?? {};
  for (const [state, count] of Object.entries(recomputed)) {
    if (recorded[state] !== count) {
      problems.push(`totals.${key}.${state} says ${recorded[state] ?? 0}, the rows give ${count}`);
    }
  }
  for (const state of Object.keys(recorded)) {
    if (!(state in recomputed)) problems.push(`totals.${key}.${state} counts rows that do not exist`);
  }
}

const count = (state) => matrix.skills.filter((s) => s.runtimeState === state).length;
console.log(`BMAD skill coverage against ${matrix.pinnedUpstream}`);
console.log(`  upstream skills discovered : ${matrix.skills.length}`);
console.log(`  active projections         : ${count("active_projection")}`);
console.log(`  compatibility shims        : ${count("compatibility_shim")}`);
console.log(`  legacy split, active       : ${count("legacy_split_active")}`);
console.log(`  normalized, not in closure : ${count("normalized_not_in_closure")}`);
console.log(`  not projected              : ${count("not_projected")}`);
console.log(`  upstream shim, unprojected : ${count("upstream_shim_unprojected")}`);
console.log(`  awaiting a BM-03 decision  : ${matrix.skills.filter((s) => s.decision === null).length}`);

if (problems.length > 0) {
  console.error(`\n${problems.length} coverage problem(s):\n`);
  for (const problem of problems) console.error(`  - ${problem}`);
  console.error("");
}

if (process.argv.includes("--check")) {
  assert.equal(problems.length, 0, "The skill coverage matrix disagrees with the capability closure.");
  console.log("\nCoverage check passed.");
}
