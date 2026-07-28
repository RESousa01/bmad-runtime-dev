import { spawnSync } from "node:child_process";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const packageRoot = fileURLToPath(new URL("../", import.meta.url));
const arguments_ = process.argv.slice(2);
const unknown = arguments_.filter((argument) => argument !== "--check");
if (unknown.length > 0) {
  throw new Error(`Unsupported BMAD generator arguments: ${unknown.join(", ")}`);
}
const result = spawnSync(
  process.execPath,
  [path.join(packageRoot, "scripts", "generate.mjs"), ...arguments_],
  {
    cwd: packageRoot,
    env: process.env,
    stdio: "inherit",
    windowsHide: true,
  },
);
if (result.error) throw result.error;
process.exitCode = result.status ?? 1;
