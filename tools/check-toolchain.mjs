import assert from "node:assert/strict";
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

// pnpm records the exact invoking package-manager version in every package
// script. Reading that metadata avoids executing a platform-specific command
// shim (which Node intentionally refuses to spawn directly on Windows).
const packageManagerAgent = process.env.npm_config_user_agent ?? "";
const childPnpm = /(?:^|\s)pnpm\/([^\s]+)/u.exec(packageManagerAgent)?.[1];
assert.ok(
  childPnpm,
  "Toolchain checks must run through pnpm so the invoking version can be verified.",
);
assert.equal(
  childPnpm,
  expectedPnpm,
  `Child processes resolve pnpm ${childPnpm}, but the pinned toolchain requires ${expectedPnpm}.`,
);

console.log(`Toolchain check passed: node ${actualNode}, pnpm ${childPnpm}.`);
