import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import path from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = fileURLToPath(new URL("../", import.meta.url));
const repositoryRoot = path.resolve(packageRoot, "..", "..");
const workflow = await readFile(
  path.join(repositoryRoot, ".github", "workflows", "contracts.yml"),
  "utf8",
);

function stepSource(name) {
  const start = workflow.indexOf(`      - name: ${name}\n`);
  assert.notEqual(start, -1, `Missing workflow step: ${name}`);
  const next = workflow.indexOf("      - ", start + 8);
  return workflow.slice(start, next === -1 ? workflow.length : next);
}

test("native generator restore and verification share an isolated Cargo home", () => {
  const isolatedCargoHome = /\n        env:\n          CARGO_HOME: \$\{\{ runner\.temp \}\}\\sapphirus-contract-cargo-home\n/u;

  assert.match(stepSource("Restore reviewed cargo-typify generator"), isolatedCargoHome);
  assert.match(stepSource("Verify cross-language generators"), isolatedCargoHome);
});
