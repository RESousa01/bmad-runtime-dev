import { existsSync } from "node:fs";
import { resolve } from "node:path";
import {
  fail,
  parseArgs,
  parseTomlValue,
  readJson,
  readText,
  rootFrom,
  writeGenerated,
} from "./lib.mjs";

function required(path, label) {
  if (!existsSync(path)) throw new Error(`missing pin owner: ${label}`);
  return path;
}

export function generatePins(root) {
  const nodePath = existsSync(resolve(root, ".node-version"))
    ? resolve(root, ".node-version")
    : required(resolve(root, ".nvmrc"), ".node-version or .nvmrc");
  const node = readText(nodePath).trim();
  const nvmPath = resolve(root, ".nvmrc");
  if (existsSync(nvmPath) && readText(nvmPath).trim() !== node) {
    throw new Error(".node-version and .nvmrc disagree");
  }
  const packageJson = readJson(required(resolve(root, "package.json"), "package.json"));
  const ui = readJson(
    required(resolve(root, "apps/desktop-ui/package.json"), "apps/desktop-ui/package.json"),
  );
  const cargoPath = required(resolve(root, "Cargo.toml"), "Cargo.toml");
  const rustPath = required(resolve(root, "rust-toolchain.toml"), "rust-toolchain.toml");
  const globalPath = required(resolve(root, "global.json"), "global.json");
  const tauriPath = required(
    resolve(root, "crates/desktop-app/tauri.conf.json"),
    "crates/desktop-app/tauri.conf.json",
  );
  const cargo = readText(cargoPath);
  const rust = readText(rustPath);
  const globalJson = readJson(globalPath);
  const tauri = readJson(tauriPath);
  const packageManager = packageJson.packageManager?.match(/^pnpm@(.+)$/)?.[1];
  const pins = [
    { id: "node", observed: node, sourcePaths: [nodePath.endsWith(".nvmrc") ? ".nvmrc" : ".node-version"] },
    { id: "pnpm", observed: packageManager, sourcePaths: ["package.json#/packageManager"] },
    {
      id: "rust",
      observed: parseTomlValue(rust, "toolchain", "channel"),
      sourcePaths: ["rust-toolchain.toml#/toolchain/channel"],
    },
    {
      id: "dotnet",
      observed: globalJson.sdk?.version,
      sourcePaths: ["global.json#/sdk/version"],
    },
    {
      id: "vite",
      observed: ui.devDependencies?.vite ?? ui.dependencies?.vite,
      sourcePaths: ["apps/desktop-ui/package.json#/devDependencies/vite"],
    },
    {
      id: "typescript",
      observed: ui.devDependencies?.typescript ?? ui.dependencies?.typescript,
      sourcePaths: ["apps/desktop-ui/package.json#/devDependencies/typescript"],
    },
    {
      id: "react",
      observed: ui.dependencies?.react,
      sourcePaths: ["apps/desktop-ui/package.json#/dependencies/react"],
    },
    {
      id: "workspace",
      observed: parseTomlValue(cargo, "workspace.package", "version"),
      sourcePaths: ["Cargo.toml#/workspace.package/version"],
    },
    {
      id: "desktop",
      observed: tauri.version,
      sourcePaths: ["crates/desktop-app/tauri.conf.json#/version"],
    },
  ].filter((pin) => pin.observed !== undefined);
  return { schemaVersion: "forge.knowledge.v2", generatedBy: "tools/knowledge/generate-pins.mjs", pins };
}

try {
  const options = parseArgs(process.argv.slice(2));
  const root = rootFrom(options);
  const output = resolve(root, options.output ?? "docs/_meta/pins.generated.json");
  writeGenerated(output, generatePins(root), options.check, "generated pin projection is stale");
  process.stdout.write(`${options.check ? "Verified" : "Generated"} ${output}\n`);
} catch (error) {
  fail(error);
}

