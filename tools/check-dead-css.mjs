#!/usr/bin/env node
/**
 * Finds CSS class selectors no source file can produce.
 *
 * Phase 5, `FE-30`. The renderer carries 8,476 lines of CSS across nine files,
 * and every one of the 67 recorded conflicts involves `design-polish.css` --
 * imported last, so it wins by source order rather than by owning anything.
 * Deleting that cascade starts with knowing which of it is load-bearing, and
 * the answer is not obvious by reading: a selector that matches nothing looks
 * exactly like one that matches something rarely.
 *
 * jsdom loads no CSS, so no test can catch a dead rule. This can.
 *
 * What it proves, and what it does not
 * -----------------------------------
 * A class is "reachable" if its literal text appears in the renderer
 * TypeScript, or -- for a BEM modifier -- if the stem it would be built from
 * appears there. That is deliberately generous, because `className={`row
 * row--${kind}`}` builds names no scan can see, so a reported class is a
 * *candidate*, not a verdict. Every deletion still has to be argued.
 *
 * The generosity runs one way only. It never reports a class that is used, so a
 * clean run means nothing was missed; it can only fail to report one that is
 * dead. A checker that occasionally deleted live styles would be worse than no
 * checker at all.
 *
 * The baseline works like the CSS conflict gate's: known-dead classes are
 * recorded, the count is expected to fall, and a *new* one fails the build.
 */

import { readFileSync, writeFileSync } from "node:fs";
import { globSync } from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(here, "..");
const BASELINE = path.join(repoRoot, "tools/dead-css-baseline.json");

const cssGlobs = [
  "apps/desktop-ui/src/**/*.css",
  "packages/ui/src/**/*.css",
];
const sourceGlobs = [
  "apps/desktop-ui/src/**/*.ts",
  "apps/desktop-ui/src/**/*.tsx",
  "packages/ui/src/**/*.ts",
  "packages/ui/src/**/*.tsx",
];

function collect(globs) {
  const files = [];
  for (const pattern of globs) {
    for (const file of globSync(pattern, { cwd: repoRoot })) {
      files.push(path.join(repoRoot, file));
    }
  }
  return files.sort();
}

/** Class names a stylesheet declares, with where each was declared. */
function declaredClasses(cssFiles) {
  const declared = new Map();
  for (const file of cssFiles) {
    const css = readFileSync(file, "utf8").replace(/\/\*[\s\S]*?\*\//gu, "");
    // Selectors only: everything before a `{`, excluding at-rule preludes,
    // which carry no class of their own.
    for (const block of css.matchAll(/([^{}]+)\{/gu)) {
      const prelude = block[1];
      if (prelude.trimStart().startsWith("@")) continue;
      for (const found of prelude.matchAll(/\.(-?[_a-zA-Z][\w-]*)/gu)) {
        const name = found[1];
        if (!declared.has(name)) declared.set(name, new Set());
        declared.get(name).add(path.relative(repoRoot, file).replaceAll("\\", "/"));
      }
    }
  }
  return declared;
}

/**
 * Every class name any source file could produce.
 *
 * Whole-text search rather than parsing `className` expressions: a name built
 * by template literal, held in a constant, or passed as a prop is still a name
 * that appears in the text, and missing one would mean deleting a live rule.
 *
 * Stylesheets are deliberately excluded. A class mentioned only by CSS is
 * mentioned only by the rules being audited, so including them would let every
 * dead rule vouch for itself.
 */
function reachableText(sourceFiles) {
  return sourceFiles.map((file) => readFileSync(file, "utf8")).join("\n");
}

const cssFiles = collect(cssGlobs);
const sourceFiles = collect(sourceGlobs);
const declared = declaredClasses(cssFiles);
const haystack = reachableText(sourceFiles);

/**
 * Whether a source file could produce this class.
 *
 * The literal name, or -- for a BEM modifier -- the stem it would be built
 * from. `context-drawer--overlay` never appears in the renderer, because
 * `ContextDrawer` writes `` `context-drawer--${presentation}` ``; reporting it
 * would have been a live rule flagged for deletion, which is the one outcome
 * this tool must never produce.
 *
 * The stem check is why it stays generous. It cannot tell a constructed
 * `--overlay` from a constructed `--nonsense`, so it clears both, and a
 * genuinely dead modifier on a stem that is built elsewhere goes unreported.
 * Missing a dead rule costs a line of CSS; misreporting a live one costs the
 * user a broken screen.
 */
function reachable(name, sources) {
  if (sources.includes(name)) return true;
  const modifier = name.indexOf("--");
  return modifier > 0 && sources.includes(`${name.slice(0, modifier)}--`);
}

const dead = [];
for (const [name, files] of [...declared].sort(([a], [b]) => a.localeCompare(b))) {
  if (reachable(name, haystack)) continue;
  dead.push({ files: [...files].sort(), name });
}

const key = (entry) => `${entry.name} :: ${entry.files.join(", ")}`;

if (process.argv.includes("--write-baseline")) {
  writeFileSync(BASELINE, `${JSON.stringify({ dead: dead.map(key) }, null, 2)}\n`);
  console.log(`Recorded ${dead.length} known-dead CSS classes.`);
  process.exit(0);
}

const baseline = new Set(JSON.parse(readFileSync(BASELINE, "utf8")).dead);
const added = dead.filter((entry) => !baseline.has(key(entry)));
const removed = baseline.size - (dead.length - added.length);

if (added.length > 0) {
  console.error(
    `${added.length} CSS class${added.length === 1 ? "" : "es"} no source file can produce:\n`
    + added.map((entry) => `  .${entry.name}  (${entry.files.join(", ")})`).join("\n")
    + "\n\nDelete the rule, or if the name is built dynamically, re-record:\n"
    + "  node tools/check-dead-css.mjs --write-baseline\n",
  );
  process.exit(1);
}

console.log(
  `Dead CSS check passed: ${dead.length} known dead`
  + (removed > 0 ? `, ${removed} removed since the baseline.` : "."),
);
