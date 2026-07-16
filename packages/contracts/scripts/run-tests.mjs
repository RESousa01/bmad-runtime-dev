import { spawnSync } from "node:child_process";
import { readdir } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath, pathToFileURL } from "node:url";

const packageRoot = path.resolve(fileURLToPath(new URL("..", import.meta.url)));
const testsRoot = path.join(packageRoot, "tests");
const nativeArtifactTest = "native-codegen-lock.test.mjs";

export function selectContractTestFiles(entries, { includeNative = false } = {}) {
  return entries
    .filter((entry) => entry.endsWith(".test.mjs"))
    .filter((entry) => includeNative || entry !== nativeArtifactTest)
    .sort();
}

async function main() {
  const arguments_ = process.argv.slice(2);
  const includeNative = arguments_.includes("--include-native");
  const unknown = arguments_.filter((argument) => argument !== "--include-native");
  if (unknown.length > 0) {
    throw new Error(`Unknown contract test runner arguments: ${unknown.join(", ")}`);
  }

  const entries = await readdir(testsRoot);
  const files = selectContractTestFiles(entries, { includeNative }).map((entry) =>
    path.join(testsRoot, entry),
  );
  if (files.length === 0) {
    throw new Error("No contract tests were selected.");
  }

  const result = spawnSync(
    process.execPath,
    ["--test", "--test-isolation=none", ...files],
    {
      cwd: packageRoot,
      env: process.env,
      stdio: "inherit",
      windowsHide: true,
    },
  );
  if (result.error) {
    throw result.error;
  }
  process.exitCode = result.status ?? 1;
}

const invokedPath = process.argv[1]
  ? pathToFileURL(path.resolve(process.argv[1])).href
  : "";
if (import.meta.url === invokedPath) {
  await main();
}
