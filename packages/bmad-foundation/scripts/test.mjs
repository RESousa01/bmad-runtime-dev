#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { readdirSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const args = process.argv.slice(2).filter((arg) => arg !== "--runInBand");
const skill = args.length === 1 ? args[0] : null;
if (args.length > 1 || (skill !== null && skill !== "bmad-deep-recon")) {
  process.stderr.write(
    "usage: node scripts/test.mjs [--runInBand bmad-deep-recon]\n",
  );
  process.exitCode = 1;
} else {
  const tests = readdirSync(resolve(packageRoot, "tests"))
    .filter((name) => name.endsWith(".test.mjs"))
    .map((name) => resolve(packageRoot, "tests", name));
  const nodeArgs = ["--test"];
  if (skill !== null) {
    nodeArgs.push("--test-name-pattern", "(?:Deep Recon|bmad-deep-recon)");
  }
  nodeArgs.push(...tests);
  const result = spawnSync(process.execPath, nodeArgs, {
    cwd: packageRoot,
    stdio: "inherit",
  });
  process.exitCode = result.status ?? 1;
}
