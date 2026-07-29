import assert from "node:assert/strict";
import {
  existsSync,
  readFileSync,
  readdirSync,
} from "node:fs";
import { relative, resolve, sep } from "node:path";
import process from "node:process";

const excludedDirectories = new Set([
  ".git",
  ".worktrees",
  "artifacts",
  "bmad-runtime-lib",
  "library",
  "node_modules",
  "target",
  "tmp",
]);
const excludedPathPrefixes = Object.freeze([
  "packages/bmad-foundation/source",
]);

function normalize(path) {
  return path.split(sep).join("/");
}

function read(root, path) {
  return readFileSync(resolve(root, path), "utf8");
}

function discover(root, matches) {
  const found = [];
  const visit = (directory) => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      if (entry.isDirectory() && excludedDirectories.has(entry.name)) continue;
      const path = resolve(directory, entry.name);
      if (entry.isDirectory()) {
        const relativePath = normalize(relative(root, path));
        if (excludedPathPrefixes.some(
          (prefix) => relativePath === prefix || relativePath.startsWith(`${prefix}/`),
        )) {
          continue;
        }
        visit(path);
      } else if (entry.isFile() && matches(entry.name)) {
        found.push(normalize(relative(root, path)));
      }
    }
  };
  visit(root);
  return found.sort();
}

function parsePnpmPatterns(source) {
  const patterns = [];
  let inPackages = false;
  for (const line of source.split(/\r?\n/u)) {
    if (line === "packages:") {
      inPackages = true;
      continue;
    }
    if (inPackages && /^\S/u.test(line)) break;
    const match = /^\s*-\s*["']?([^"']+?)["']?\s*$/u.exec(line);
    if (inPackages && match) patterns.push(match[1].trim());
  }
  return patterns;
}

function expandPnpmPatterns(root, patterns) {
  const members = [];
  for (const pattern of patterns) {
    assert.match(pattern, /^[^*]+(?:\/\*)?$/u, `unsupported pnpm workspace pattern: ${pattern}`);
    if (!pattern.endsWith("/*")) {
      members.push(`${pattern}/package.json`);
      continue;
    }
    const parent = resolve(root, pattern.slice(0, -2));
    for (const entry of readdirSync(parent, { withFileTypes: true })) {
      if (entry.isDirectory() && existsSync(resolve(parent, entry.name, "package.json"))) {
        members.push(`${pattern.slice(0, -1)}${entry.name}/package.json`);
      }
    }
  }
  return members.sort();
}

function parseCargoMembers(source) {
  const members = /\bmembers\s*=\s*\[([\s\S]*?)\]/mu.exec(source)?.[1] ?? "";
  return [...members.matchAll(/"([^"]+)"/gu)]
    .map((match) => `${match[1]}/Cargo.toml`)
    .sort();
}

function parseDotnetMembers(source) {
  return [...source.matchAll(/<Project\s+Path="([^"]+\.csproj)"\s*\/>/gu)]
    .map((match) => match[1].replaceAll("\\", "/"))
    .sort();
}

function assertExactMembers(kind, declared, discovered) {
  const undeclared = discovered.filter((path) => !declared.includes(path));
  const missing = declared.filter((path) => !discovered.includes(path));
  assert.deepEqual(
    { undeclared, missing },
    { undeclared: [], missing: [] },
    [
      `${kind} workspace membership is incomplete.`,
      ...undeclared.map((path) => `undeclared ${kind} manifest: ${path}`),
      ...missing.map((path) => `declared ${kind} manifest is missing: ${path}`),
    ].join("\n"),
  );
}

function assertToolchains(root, manifest, cargoSource) {
  const nodeVersion = read(root, ".node-version").trim();
  const nvmVersion = read(root, ".nvmrc").trim();
  assert.equal(nvmVersion, nodeVersion, ".nvmrc and .node-version must pin the same Node version.");
  assert.equal(manifest.engines?.node, nodeVersion, "package.json engines.node must match Node pins.");

  const pnpmVersion = String(manifest.packageManager ?? "").replace(/^pnpm@/u, "");
  assert.notEqual(pnpmVersion, manifest.packageManager, "packageManager must pin pnpm@<version>.");
  assert.equal(manifest.engines?.pnpm, pnpmVersion, "packageManager and engines.pnpm must match.");

  const [nodeMajor] = nodeVersion.split(".").map(Number);
  assert.ok(nodeMajor >= 22, "Node must satisfy the Builder >=22.0.0 runtime floor.");

  const rustVersion = /\brust-version\s*=\s*"([^"]+)"/u.exec(cargoSource)?.[1];
  const rustChannel = /\bchannel\s*=\s*"([^"]+)"/u.exec(read(root, "rust-toolchain.toml"))?.[1];
  assert.equal(rustChannel, rustVersion, "Cargo rust-version and rust-toolchain channel must match.");

  const dotnetVersion = JSON.parse(read(root, "global.json")).sdk?.version;
  assert.match(dotnetVersion ?? "", /^\d+\.\d+\.\d+$/u, "global.json must pin an exact .NET SDK.");
}

function assertRootCommands(manifest) {
  const required = [
    "install:locked",
    "generate",
    "lint",
    "typecheck",
    "test:unit",
    "test:integration",
    "test:desktop-e2e",
    "security",
    "package",
    "verify:workspace",
    "verify:bmad:100",
  ];
  const missing = required.filter((name) => !manifest.scripts?.[name]);
  assert.deepEqual(missing, [], `missing root scripts: ${missing.join(", ")}`);
}

function assertGeneratedOutputsIgnored(root, dotnetMembers) {
  const ignore = read(root, ".gitignore");
  for (const path of ["node_modules/", "dist/", "coverage/", "target/"]) {
    assert.match(ignore, new RegExp(`^${path.replace("/", "\\/")}$`, "mu"), `${path} must be ignored.`);
  }
  for (const member of dotnetMembers) {
    const directory = member.slice(0, member.lastIndexOf("/"));
    for (const output of ["bin", "obj"]) {
      assert.ok(
        ignore.includes(`${directory}/${output}/`) || ignore.includes(`**/${output}/`),
        `${directory}/${output}/ must be ignored.`,
      );
    }
  }
}

function assertCiContract(root) {
  const workflowPath = resolve(root, ".github", "workflows", "bmad-100.yml");
  assert.ok(existsSync(workflowPath), ".github/workflows/bmad-100.yml is required.");
  const workflow = readFileSync(workflowPath, "utf8");
  for (const command of [
    "pnpm install --frozen-lockfile",
    "pnpm verify:workspace",
    "cargo metadata --locked --no-deps",
    "dotnet restore Sapphirus.slnx --locked-mode",
  ]) {
    assert.ok(workflow.includes(command), `BMAD-100 CI must call: ${command}`);
  }
}

export function verifyWorkspace(root) {
  const manifest = JSON.parse(read(root, "package.json"));
  const cargoSource = read(root, "Cargo.toml");
  const pnpmMembers = expandPnpmPatterns(root, parsePnpmPatterns(read(root, "pnpm-workspace.yaml")));
  const cargoMembers = parseCargoMembers(cargoSource);
  const dotnetMembers = parseDotnetMembers(read(root, "Sapphirus.slnx"));

  assertExactMembers(
    "package",
    pnpmMembers,
    discover(root, (name) => name === "package.json").filter((path) => path !== "package.json"),
  );
  assertExactMembers(
    "Cargo",
    cargoMembers,
    discover(root, (name) => name === "Cargo.toml").filter((path) => path !== "Cargo.toml"),
  );
  assertExactMembers(".NET", dotnetMembers, discover(root, (name) => name.endsWith(".csproj")));

  assertToolchains(root, manifest, cargoSource);
  assertRootCommands(manifest);
  assertGeneratedOutputsIgnored(root, dotnetMembers);
  assertCiContract(root);

  return {
    pnpm: pnpmMembers.length,
    cargo: cargoMembers.length,
    dotnet: dotnetMembers.length,
  };
}

function main() {
  const rootFlag = process.argv.indexOf("--root");
  const root = resolve(rootFlag >= 0 ? process.argv[rootFlag + 1] : ".");
  try {
    const result = verifyWorkspace(root);
    console.log(
      `workspace verified: pnpm=${result.pnpm} cargo=${result.cargo} dotnet=${result.dotnet}`,
    );
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(import.meta.filename)) {
  main();
}
