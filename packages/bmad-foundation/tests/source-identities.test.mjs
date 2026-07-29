import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { resolve } from "node:path";
import test from "node:test";

const packageRoot = resolve(import.meta.dirname, "..");
const verifier = resolve(packageRoot, "scripts", "verify-source-identities.mjs");

test("the source lock and semantic ledger agree on both pinned identities", () => {
  const result = spawnSync(process.execPath, [verifier, "--check", "--metadata-only"], {
    cwd: packageRoot,
    encoding: "utf8",
  });

  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
  assert.match(
    result.stdout,
    /source identities verified: method=6\.10\.0 builder=2\.1\.0/u,
  );
});
