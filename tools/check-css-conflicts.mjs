/**
 * Reports renderer CSS declarations that more than one file makes.
 *
 * Why this exists
 * ---------------
 * When two stylesheets declare the same property on the same selector, exactly
 * one of them takes effect and nothing in either file says which. The loser is
 * invisible: it reads as live code, it survives review, and it is only found
 * when someone edits the winner and the value does not move.
 *
 * Three defects were shipping in this renderer for exactly that reason:
 *
 *   - transcript prose was a size above the scale, because a later rule raised
 *     it and the earlier one still looked authoritative;
 *   - design-polish.css was re-adding a bubble, an avatar shadow and a font
 *     size that styles.css had removed;
 *   - roughly a third of design-polish.css never applied at all, because the
 *     files that emit after it declare the same pairs.
 *
 * None of that is visible in a diff, and no test caught any of it.
 *
 * How it fails
 * ------------
 * A ratchet, not a wall. The existing conflicts are recorded in
 * css-conflicts-baseline.json; this exits non-zero only for pairs that are not
 * in that file. Removing a conflict is always allowed, and the baseline can be
 * re-recorded with --write-baseline. The debt stays visible and bounded
 * without blocking work on it.
 */

import assert from "node:assert/strict";
import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";

// Overridable so the test can run this exact check against fixture trees
// rather than against a second copy of the logic.
const ROOTS = (process.env.CSS_CONFLICT_ROOTS ?? "apps/desktop-ui/src,packages/ui/src")
  .split(",")
  .map((root) => root.trim())
  .filter(Boolean);
const BASELINE = process.env.CSS_CONFLICT_BASELINE ?? "tools/css-conflicts-baseline.json";

/** Every .css file under the given roots, excluding build output. */
function styleSheets() {
  const found = [];
  const walk = (dir) => {
    for (const entry of readdirSync(dir)) {
      if (entry === "node_modules" || entry === "dist") continue;
      const full = path.join(dir, entry);
      if (statSync(full).isDirectory()) walk(full);
      else if (entry.endsWith(".css")) found.push(full.replaceAll("\\", "/"));
    }
  };
  for (const root of ROOTS) walk(root);
  return found.sort();
}

/**
 * Selector/property pairs declared by one file, as JSON keys.
 *
 * JSON rather than a joined string because descendant selectors contain
 * spaces and combinators contain most punctuation, so any single-character
 * separator would eventually split something like ".message p" in the wrong
 * place.
 *
 * The parse is deliberately shallow. Innermost brace blocks are matched, so a
 * rule nested in a media query is read as its own selector and the condition
 * is ignored. That over-reports slightly, since two files can declare the same
 * pair under conditions that never both apply, which is the right direction
 * for a report whose job is to make silent duplication visible.
 *
 * Custom properties are skipped: redefining a token per scope is the token
 * contract working, not a conflict.
 */
function declarations(css) {
  const pairs = new Set();
  const withoutComments = css.replace(/\/\*[\s\S]*?\*\//gu, "");
  for (const match of withoutComments.matchAll(/([^{}]+)\{([^{}]*)\}/gu)) {
    const selector = match[1].replaceAll(/\s+/gu, " ").trim();
    if (!selector || selector.startsWith("@")) continue;
    for (const declaration of match[2].split(";")) {
      const colon = declaration.indexOf(":");
      if (colon < 1) continue;
      const property = declaration.slice(0, colon).trim();
      if (!property || property.startsWith("--")) continue;
      pairs.add(JSON.stringify([selector, property]));
    }
  }
  return pairs;
}

function findConflicts() {
  const owners = new Map();
  for (const file of styleSheets()) {
    for (const pair of declarations(readFileSync(file, "utf8"))) {
      if (!owners.has(pair)) owners.set(pair, []);
      owners.get(pair).push(file);
    }
  }
  const conflicts = [];
  for (const [pair, files] of owners) {
    if (files.length < 2) continue;
    const [selector, property] = JSON.parse(pair);
    conflicts.push({ selector, property, files });
  }
  conflicts.sort(
    (a, b) => a.selector.localeCompare(b.selector) || a.property.localeCompare(b.property),
  );
  return conflicts;
}

/** Stable identity for a conflict, so re-ordering files is not a change. */
function conflictKey(conflict) {
  return JSON.stringify([conflict.selector, conflict.property, [...conflict.files].sort()]);
}

const conflicts = findConflicts();

if (process.argv.includes("--write-baseline")) {
  const recorded = { conflicts: conflicts.map(conflictKey).sort() };
  writeFileSync(BASELINE, `${JSON.stringify(recorded, null, 2)}\n`, "utf8");
  console.log(`Recorded ${conflicts.length} known CSS conflicts in ${BASELINE}.`);
  process.exit(0);
}

const baseline = new Set(JSON.parse(readFileSync(BASELINE, "utf8")).conflicts);
const added = conflicts.filter((conflict) => !baseline.has(conflictKey(conflict)));

if (added.length > 0) {
  console.error(`\n${added.length} new cross-file CSS conflict(s):\n`);
  for (const conflict of added) {
    console.error(`  ${conflict.selector} { ${conflict.property} }`);
    for (const file of conflict.files) console.error(`      ${file}`);
    console.error("");
  }
  console.error(
    "One of these declarations does nothing, and the file it lives in does not\n"
      + "say which. Put the property in one file, or re-record the baseline with:\n"
      + "  node tools/check-css-conflicts.mjs --write-baseline\n",
  );
}

assert.equal(added.length, 0, "New cross-file CSS conflicts were introduced.");

const resolved = baseline.size - (conflicts.length - added.length);
const trend = resolved > 0 ? `, ${resolved} fewer than the baseline` : "";
console.log(`CSS conflict check passed: ${conflicts.length} known conflicts${trend}.`);
