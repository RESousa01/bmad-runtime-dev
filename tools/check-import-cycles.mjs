#!/usr/bin/env node
/**
 * Fails when the renderer's modules import each other in a cycle.
 *
 * FE-10 asks for App.tsx to become a composition root, and its acceptance
 * criteria include "no circular dependencies are introduced". Decomposing a
 * god component is exactly the change that introduces them: a controller
 * pulled out of App reaches back into App for one type, App imports the
 * controller, and the cycle is invisible in review because each import looks
 * reasonable on its own.
 *
 * A cycle is not merely untidy here. Module initialisation order inside one
 * becomes dependent on which member is imported first, so a `const` declared at
 * module scope can be `undefined` when a sibling reads it -- a failure that
 * appears only in the build, only sometimes, and never in a unit test that
 * imports the modules in the lucky order.
 *
 * Type-only imports are excluded. `import type` is erased before the bundler
 * sees it, so it cannot produce an initialisation-order problem, and forbidding
 * it would push code towards duplicating types instead of sharing them.
 *
 * Usage:
 *   node tools/check-import-cycles.mjs [--json] [--root <dir>]...
 *
 * `--root` replaces the default roots, so the checker can be pointed at a
 * fixture tree and proved to actually detect a cycle rather than passing
 * because it found nothing to look at.
 */

import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");

/** Roots to scan. Each is a self-contained module graph. */
const ROOTS = [
  "apps/desktop-ui/src",
  "packages/ui/src",
];

const SOURCE_EXTENSIONS = [".ts", ".tsx"];

function listSourceFiles(dir) {
  const found = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) {
      if (entry === "node_modules" || entry === "dist") continue;
      found.push(...listSourceFiles(full));
      continue;
    }
    if (!SOURCE_EXTENSIONS.some((ext) => entry.endsWith(ext))) continue;
    // Tests may legitimately import a module that imports them back through a
    // barrel; they are leaves of the graph and never ship.
    if (/\.(test|spec)\.tsx?$/u.test(entry)) continue;
    found.push(full);
  }
  return found;
}

/*
 * Value imports only.
 *
 * `import type { X }` and `import { type X }` are erased, so neither can create
 * an initialisation-order problem. The first is dropped by the `type` in the
 * pattern; the second still counts as a value import if any specifier in the
 * clause lacks `type`, which is the correct reading -- one value binding is
 * enough to make the import real.
 */
const IMPORT_PATTERN =
  /(?:^|\n)\s*(?:import|export)\s+(?!type\s)(?:[^'"]*?\sfrom\s+)?["']([^"']+)["']/gu;

function valueImports(file) {
  const source = readFileSync(file, "utf8");
  const specifiers = [];
  for (const match of source.matchAll(IMPORT_PATTERN)) {
    const specifier = match[1];
    if (specifier === undefined) continue;
    if (!specifier.startsWith(".")) continue;
    specifiers.push(specifier);
  }
  return specifiers;
}

/**
 * Resolution order for an extensionless specifier, matching the bundler's.
 *
 * The index candidates are built with `join`, not by appending "/index.ts":
 * string concatenation produces a mixed-separator path on Windows, which never
 * matches the `join`-produced paths in the file list, so every directory import
 * silently resolved to nothing and its edges vanished from the graph. A
 * checker that resolves no imports reports no cycles and looks like it passed.
 */
function resolveCandidates(base) {
  return [
    base,
    `${base}.ts`,
    `${base}.tsx`,
    join(base, "index.ts"),
    join(base, "index.tsx"),
  ];
}

function resolveSpecifier(fromFile, specifier) {
  const base = resolve(dirname(fromFile), specifier);
  for (const candidate of resolveCandidates(base)) {
    try {
      if (statSync(candidate).isFile()) return candidate;
    } catch {
      // Not this shape; try the next.
    }
  }
  return null;
}

function buildGraph(files) {
  const graph = new Map();
  for (const file of files) {
    const edges = [];
    for (const specifier of valueImports(file)) {
      const target = resolveSpecifier(file, specifier);
      // Unresolved means it left the scanned roots -- a package import written
      // relatively, or a non-source asset. Neither can form a cycle here.
      if (target !== null && files.includes(target)) edges.push(target);
    }
    graph.set(file, edges);
  }
  return graph;
}

/** Tarjan, so every cycle is reported once rather than once per entry point. */
function findCycles(graph) {
  const index = new Map();
  const low = new Map();
  const onStack = new Set();
  const stack = [];
  const cycles = [];
  let counter = 0;

  function strongConnect(node) {
    index.set(node, counter);
    low.set(node, counter);
    counter += 1;
    stack.push(node);
    onStack.add(node);

    for (const next of graph.get(node) ?? []) {
      if (!index.has(next)) {
        strongConnect(next);
        low.set(node, Math.min(low.get(node), low.get(next)));
      } else if (onStack.has(next)) {
        low.set(node, Math.min(low.get(node), index.get(next)));
      }
    }

    if (low.get(node) !== index.get(node)) return;
    const component = [];
    for (;;) {
      const member = stack.pop();
      onStack.delete(member);
      component.push(member);
      if (member === node) break;
    }
    // A single module is a cycle only if it imports itself.
    const selfEdge = component.length === 1
      && (graph.get(node) ?? []).includes(node);
    if (component.length > 1 || selfEdge) cycles.push(component.sort());
  }

  for (const node of graph.keys()) {
    if (!index.has(node)) strongConnect(node);
  }
  return cycles;
}

function parseRoots(argv) {
  const roots = [];
  for (let i = 0; i < argv.length; i += 1) {
    if (argv[i] !== "--root") continue;
    const value = argv[i + 1];
    if (value === undefined) throw new Error("--root needs a directory");
    roots.push(value);
  }
  return roots.length > 0 ? roots : ROOTS;
}

function main() {
  const asJson = process.argv.includes("--json");
  const files = [];
  for (const root of parseRoots(process.argv.slice(2))) {
    const full = resolve(REPO_ROOT, root);
    try {
      files.push(...listSourceFiles(full));
    } catch {
      // A root that does not exist is not an error: this check should keep
      // working while the workspace layout moves.
    }
  }

  const cycles = findCycles(buildGraph(files));
  const report = cycles.map((cycle) =>
    cycle.map((file) => relative(REPO_ROOT, file).replaceAll("\\", "/")));

  if (asJson) {
    process.stdout.write(`${JSON.stringify({ cycles: report, scanned: files.length }, null, 2)}\n`);
  } else if (report.length === 0) {
    process.stdout.write(`Import cycle check passed: ${files.length} modules, no cycles.\n`);
  } else {
    process.stdout.write(`Import cycles found (${report.length}):\n`);
    for (const cycle of report) {
      process.stdout.write(`\n  ${cycle.join("\n  → ")}\n`);
    }
    process.stdout.write(
      "\nA cycle makes module initialisation order depend on which member is\n"
      + "imported first, so a module-scope const can read as undefined. Break it\n"
      + "by moving the shared value into a module both can depend on, or by making\n"
      + "one side of the edge `import type`.\n",
    );
  }
  process.exitCode = report.length === 0 ? 0 : 1;
}

main();
