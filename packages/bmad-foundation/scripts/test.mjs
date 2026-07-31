#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { readdirSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const args = process.argv.slice(2).filter((arg) => arg !== "--runInBand");
const skill = args.length === 1 ? args[0] : null;
const focusedPatterns = new Map([
  ["bmad-prd", "(?:\\bPRD\\b|bmad-prd)"],
  [
    "bmad-check-implementation-readiness",
    "(?:implementation readiness|implementation_readiness|bmad-check-implementation-readiness)",
  ],
  ["bmad-deep-recon", "(?:Deep Recon|deep_recon|bmad-deep-recon)"],
  ["script-inventory", "(?:script inventory|classification|inventory)"],
  ["builder-source-closure", "(?:Builder.*closure|builder source closure|archive members)"],
]);
if (args.length > 1 || (skill !== null && !focusedPatterns.has(skill))) {
  process.stderr.write(
    `usage: node scripts/test.mjs [--runInBand ${
      [...focusedPatterns.keys()].join("|")
    }]\n`,
  );
  process.exitCode = 1;
} else {
  const tests = readdirSync(resolve(packageRoot, "tests"))
    .filter((name) => name.endsWith(".test.mjs"))
    .map((name) => resolve(packageRoot, "tests", name));
  const nodeArgs = ["--test"];
  if (skill !== null) {
    nodeArgs.push("--test-name-pattern", focusedPatterns.get(skill));
  }
  nodeArgs.push(...tests);
  const result = spawnSync(process.execPath, nodeArgs, {
    cwd: packageRoot,
    stdio: "inherit",
  });
  process.exitCode = result.status ?? 1;
}
