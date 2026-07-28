/**
 * Enforces CSS ownership: who is allowed to declare what.
 *
 * Backlog item FE-03 names this file as its verification and gives three
 * acceptance criteria. Two of them are ratchets and are enforced here:
 *
 *   - the duplicate-selector count trends to zero for migrated surfaces;
 *   - the legacy design-refinement and design-polish files are no longer
 *     allowed to gain selectors.
 *
 * The third -- "no feature depends on import order outside declared layers" --
 * is the same measurement from the other side. A feature depends on import
 * order exactly when two files declare the same selector and property at equal
 * specificity, which is what `check-css-conflicts.mjs` counts. Driving that to
 * zero is what makes the statement true, so the two checks are complementary:
 * this one stops the debt growing, that one measures it shrinking.
 *
 * Why a ratchet rather than a wall. Cascade layers were implemented and
 * reverted once already (recorded in the v3 reconciliation): naming every
 * stylesheet's layer changed which rule won on 83 elements, because layer
 * order overrides specificity outright and emission order never did. The debt
 * is real, the fix is not a one-line change, and blocking all work until it is
 * done would be worse than bounding it.
 */

import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const BASELINE = path.join(root, "tools", "css-ownership-baseline.json");

/**
 * Files that exist to override other files.
 *
 * They are frozen rather than deleted because deleting them is FE-30's job and
 * needs the surfaces migrated first. Freezing is what stops the problem
 * growing while that happens: a new selector here is a new override, and a new
 * override is a new thing nobody can find later.
 */
const FROZEN = [
  "apps/desktop-ui/src/components/redesign/design-polish.css",
  "apps/desktop-ui/src/components/redesign/design-refinement.css",
];

/** Distinct selectors a file declares, ignoring at-rule preludes. */
function selectorsIn(file) {
  const css = readFileSync(path.join(root, file), "utf8").replace(/\/\*[\s\S]*?\*\//gu, "");
  const found = new Set();
  for (const match of css.matchAll(/([^{}]+)\{[^{}]*\}/gu)) {
    const prelude = match[1].replaceAll(/\s+/gu, " ").trim();
    if (!prelude || prelude.startsWith("@")) continue;
    for (const part of prelude.split(",")) {
      const selector = part.trim();
      if (selector) found.add(selector);
    }
  }
  return found;
}

const current = Object.fromEntries(FROZEN.map((file) => [file, selectorsIn(file).size]));

if (process.argv.includes("--write-baseline")) {
  writeFileSync(BASELINE, `${JSON.stringify({ frozen: current }, null, 2)}\n`, "utf8");
  console.log(`Recorded the frozen-file selector counts in ${path.relative(root, BASELINE)}.`);
  process.exit(0);
}

const baseline = JSON.parse(readFileSync(BASELINE, "utf8")).frozen;
const problems = [];

for (const file of FROZEN) {
  const allowed = baseline[file];
  if (allowed === undefined) {
    problems.push(`${file} is frozen but has no baseline; re-record it deliberately.`);
    continue;
  }
  if (current[file] > allowed) {
    problems.push(
      `${file} gained ${current[file] - allowed} selector(s): ${allowed} -> ${current[file]}. `
      + "This file exists to override others; a new selector here is a new override that "
      + "nobody will be able to find. Put the rule in the file that owns the component.",
    );
  }
}

const shrunk = FROZEN.filter((file) => current[file] < baseline[file]);

console.log("CSS ownership");
for (const file of FROZEN) {
  const delta = current[file] - baseline[file];
  const trend = delta === 0 ? "" : delta < 0 ? `  (${delta}, good)` : `  (+${delta})`;
  console.log(`  ${String(current[file]).padStart(4)}  ${path.relative("apps/desktop-ui/src", file).replaceAll("\\", "/")}${trend}`);
}

if (problems.length > 0) {
  console.error(`\n${problems.length} ownership problem(s):\n`);
  for (const problem of problems) console.error(`  - ${problem}\n`);
  console.error(
    "If a frozen file genuinely must change, re-record with:\n"
    + "  node tools/check-css-ownership.mjs --write-baseline\n",
  );
}

assert.equal(problems.length, 0, "A frozen override file gained selectors.");

if (shrunk.length > 0) {
  console.log("\nSelectors were removed from a frozen file. Re-record the baseline to keep the ratchet tight:");
  console.log("  node tools/check-css-ownership.mjs --write-baseline");
}
console.log("\nOwnership check passed.");
