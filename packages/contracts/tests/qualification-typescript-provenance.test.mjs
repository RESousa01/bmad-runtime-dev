import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const packageRoot = fileURLToPath(new URL("../", import.meta.url));
const repositoryRoot = path.resolve(packageRoot, "..", "..");
const qualificationProject = "../../tests/generator-qualification/typescript/tsconfig.json";

test("BMAD-G0 compiles the generated qualification TypeScript in cross-language verification", async () => {
  const packageManifest = JSON.parse(await readFile(path.join(packageRoot, "package.json"), "utf8"));
  const expectedCommand = `tsc --project ${qualificationProject} --noEmit`;
  assert.equal(packageManifest.scripts["qualify:typescript"], expectedCommand);
  assert.match(packageManifest.scripts["qualify:generators"], new RegExp(expectedCommand.replaceAll(".", "\\."), "u"));
  assert.match(packageManifest.scripts["verify:cross-language"], new RegExp(expectedCommand.replaceAll(".", "\\."), "u"));

  const tsconfig = JSON.parse(await readFile(
    path.join(repositoryRoot, "tests", "generator-qualification", "typescript", "tsconfig.json"),
    "utf8",
  ));
  assert.equal(tsconfig.compilerOptions.noEmit, true);
  assert.equal(tsconfig.compilerOptions.rootDir, "../generated/typescript");
  assert.deepEqual(tsconfig.include, ["../generated/typescript/**/*.ts"]);
});
