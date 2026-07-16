import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { runGenerationTransaction } from "./lib/generation-transaction.mjs";

if (process.argv.length !== 2) throw new Error("generate-all.mjs does not accept arguments.");

const packageRoot = fileURLToPath(new URL("../", import.meta.url));
const repositoryRoot = path.resolve(packageRoot, "..", "..");
const typescriptCompiler = path.join(packageRoot, "node_modules", "typescript", "bin", "tsc");

function runNode(script, argumentsList = []) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [script, ...argumentsList], {
      cwd: packageRoot,
      shell: false,
      stdio: "inherit",
    });
    child.on("error", reject);
    child.on("close", (exitCode) => {
      if (exitCode === 0) resolve();
      else reject(new Error(`${path.basename(script)} exited with code ${exitCode}.`));
    });
  });
}

const qualify = path.join(packageRoot, "scripts", "qualify-generators.mjs");
const generate = path.join(packageRoot, "scripts", "generate.mjs");
const checkSchemas = path.join(packageRoot, "scripts", "check-schemas.mjs");
const checkBindings = path.join(packageRoot, "scripts", "check-generated-bindings.mjs");

// Prove both complete generator paths before touching committed outputs.
await runNode(qualify, ["--dry-run"]);
await runNode(generate, ["--dry-run"]);

await runGenerationTransaction({
  root: repositoryRoot,
  targets: [
    path.join(repositoryRoot, "tests", "generator-qualification", "generated"),
    path.join(packageRoot, "generated"),
    path.join(packageRoot, "fixtures"),
    path.join(packageRoot, "schema-lock.json"),
  ],
  execute: async () => {
    await runNode(qualify, ["--write"]);
    await runNode(generate);
    await runNode(checkSchemas);
    await runNode(typescriptCompiler, ["--project", path.join(packageRoot, "tsconfig.json"), "--noEmit"]);
    await runNode(typescriptCompiler, [
      "--project",
      path.join(repositoryRoot, "tests", "generator-qualification", "typescript", "tsconfig.json"),
      "--noEmit",
    ]);
    await runNode(checkBindings);
  },
});
