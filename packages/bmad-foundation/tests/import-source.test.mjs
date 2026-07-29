import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

const packageRoot = resolve(import.meta.dirname, "..");
const repositoryRoot = resolve(packageRoot, "..", "..");
const importer = resolve(packageRoot, "scripts", "import-source.mjs");

test("the retained Method and Builder trees exactly match the source manifest", () => {
  const result = spawnSync(process.execPath, [importer, "--check"], {
    cwd: packageRoot,
    encoding: "utf8",
  });

  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
  assert.match(result.stdout, /source import verified: method=597 builder=299/u);
});

test("Git preserves retained upstream source bytes without text normalization", () => {
  const attributes = readFileSync(resolve(repositoryRoot, ".gitattributes"), "utf8");
  assert.match(
    attributes,
    /^packages\/bmad-foundation\/source\/\*\* -text$/mu,
    "the pinned source roots must remain byte-identical after staging and checkout",
  );
});
