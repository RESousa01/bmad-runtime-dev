import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import process from "node:process";

const expectedNode = readFileSync(".node-version", "utf8").trim();
const rootManifest = JSON.parse(readFileSync("package.json", "utf8"));
const packageManager = String(rootManifest.packageManager ?? "");
const expectedPnpm = packageManager.replace(/^pnpm@/u, "");
assert.notEqual(expectedPnpm, packageManager, "package.json packageManager must pin pnpm@<version>.");
assert.equal(
  rootManifest.engines?.node,
  expectedNode,
  "package.json engines.node must match .node-version.",
);

const actualNode = process.version.replace(/^v/u, "");
assert.equal(
  actualNode,
  expectedNode,
  `Node ${actualNode} is running, but the pinned toolchain requires ${expectedNode}.`,
);

// Child processes must inherit the same pnpm the pin names, not whatever
// launcher happens to shadow it on PATH.
const childPnpm = execFileSync("pnpm", ["--version"], { encoding: "utf8", shell: true }).trim();
assert.equal(
  childPnpm,
  expectedPnpm,
  `Child processes resolve pnpm ${childPnpm}, but the pinned toolchain requires ${expectedPnpm}.`,
);

console.log(`Toolchain check passed: node ${actualNode}, pnpm ${childPnpm}.`);
