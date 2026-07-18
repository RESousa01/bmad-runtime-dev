import { createHash } from "node:crypto";
import { appendFileSync, lstatSync, readFileSync } from "node:fs";
import process from "node:process";
import { basename, isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const VERSION_PATTERN = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/u;
const SHA256_PATTERN = /^[0-9a-f]{64}$/u;

function readRegularFile(path) {
  const stat = lstatSync(path);
  if (!stat.isFile() || stat.isSymbolicLink()) {
    throw new Error(`release authority must be a regular file: ${path}`);
  }
  return readFileSync(path, "utf8");
}

function readJson(path) {
  return JSON.parse(readRegularFile(path));
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
}

function tomlString(source, section, key, path) {
  const escapedSection = escapeRegExp(section);
  const sectionMatch = source.match(new RegExp(
    String.raw`(?:^|\n)\[${escapedSection}\]\s*\n([\s\S]*?)(?=\n\[|$)`,
    "u",
  ));
  if (sectionMatch === null) {
    throw new Error(`${path}: missing [${section}] section`);
  }
  const keyMatch = sectionMatch[1].match(new RegExp(
    String.raw`^${escapeRegExp(key)}\s*=\s*"([^"]+)"\s*$`,
    "mu",
  ));
  if (keyMatch === null) {
    throw new Error(`${path}: missing ${section}.${key}`);
  }
  return keyMatch[1];
}

function sha256(source) {
  return createHash("sha256").update(source).digest("hex");
}

function requireVersion(value, label) {
  if (typeof value !== "string" || !VERSION_PATTERN.test(value)) {
    throw new Error(`${label} must be an exact product version`);
  }
  return value;
}

function requireExactString(value, label) {
  if (typeof value !== "string" || value.length === 0 || value.trim() !== value) {
    throw new Error(`${label} must be a non-empty exact string`);
  }
  return value;
}

export function resolveReleaseMetadata(rootPath = process.cwd()) {
  const root = resolve(rootPath);
  const rootPackage = readJson(join(root, "package.json"));
  const rendererPackage = readJson(join(root, "apps", "desktop-ui", "package.json"));
  const tauriConfig = readJson(join(root, "crates", "desktop-app", "tauri.conf.json"));
  const cargoManifestPath = join(root, "Cargo.toml");
  const cargoManifest = readRegularFile(cargoManifestPath);
  const rustToolchainPath = join(root, "rust-toolchain.toml");
  const rustToolchain = readRegularFile(rustToolchainPath);
  const nodeVersion = readRegularFile(join(root, ".node-version")).trim();
  const cargoLock = readRegularFile(join(root, "Cargo.lock"));
  const pnpmLock = readRegularFile(join(root, "pnpm-lock.yaml"));

  const versions = [
    requireVersion(rootPackage.version, "package.json version"),
    requireVersion(rendererPackage.version, "apps/desktop-ui/package.json version"),
    requireVersion(tauriConfig.version, "tauri.conf.json version"),
    requireVersion(tomlString(cargoManifest, "workspace.package", "version", cargoManifestPath), "Cargo workspace version"),
  ];
  if (new Set(versions).size !== 1) {
    throw new Error(`release versions disagree: ${[...new Set(versions)].join(", ")}`);
  }
  const productVersion = versions[0];

  const packageManager = requireExactString(rootPackage.packageManager, "package.json packageManager");
  const pnpmMatch = packageManager.match(/^pnpm@(\d+\.\d+\.\d+)$/u);
  if (pnpmMatch === null || rootPackage.engines?.pnpm !== pnpmMatch[1]) {
    throw new Error("package.json pnpm authority is inconsistent");
  }
  if (!VERSION_PATTERN.test(nodeVersion) || rootPackage.engines?.node !== nodeVersion) {
    throw new Error("Node authority is inconsistent between .node-version and package.json");
  }
  const cargoRust = tomlString(cargoManifest, "workspace.package", "rust-version", cargoManifestPath);
  const toolchainRust = tomlString(rustToolchain, "toolchain", "channel", rustToolchainPath);
  if (cargoRust !== toolchainRust) {
    throw new Error("Rust authority is inconsistent between Cargo.toml and rust-toolchain.toml");
  }

  const productName = requireExactString(tauriConfig.productName, "Tauri productName");
  const identifier = requireExactString(tauriConfig.identifier, "Tauri identifier");
  const mainBinaryName = requireExactString(tauriConfig.mainBinaryName, "Tauri mainBinaryName");
  if (!/^[0-9A-Za-z._ -]+$/u.test(productName) || !/^[a-z0-9.-]+$/u.test(identifier) || !/^[a-z0-9-]+$/u.test(mainBinaryName)) {
    throw new Error("Tauri release identity contains unsupported characters");
  }

  const metadata = {
    schemaVersion: 1,
    product: {
      name: productName,
      version: productVersion,
      identifier,
      applicationName: `${mainBinaryName}.exe`,
      installerName: `${productName}_${productVersion}_x64-setup.exe`,
      sbomName: `${productName}_${productVersion}.cdx.json`,
    },
    toolchain: {
      node: nodeVersion,
      pnpm: pnpmMatch[1],
      rust: cargoRust,
      typescript: requireExactString(rootPackage.devDependencies?.typescript, "TypeScript version"),
      tauriCli: requireExactString(rootPackage.devDependencies?.["@tauri-apps/cli"], "Tauri CLI version"),
    },
    locks: {
      cargo: { path: "Cargo.lock", sha256: sha256(cargoLock) },
      pnpm: { path: "pnpm-lock.yaml", sha256: sha256(pnpmLock) },
    },
  };
  for (const lock of Object.values(metadata.locks)) {
    if (!SHA256_PATTERN.test(lock.sha256)) {
      throw new Error(`invalid lock digest for ${lock.path}`);
    }
  }
  return metadata;
}

function parseArguments(argv) {
  let root = process.cwd();
  let githubOutput;
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    const value = argv[index + 1];
    if (argument === "--root" && value !== undefined) {
      root = value;
      index += 1;
    } else if (argument === "--github-output" && value !== undefined) {
      githubOutput = value;
      index += 1;
    } else {
      throw new Error(`unknown or incomplete argument: ${argument}`);
    }
  }
  return { root, githubOutput };
}

function appendGithubOutputs(path, metadata) {
  const absolutePath = isAbsolute(path) ? path : resolve(path);
  const stat = lstatSync(absolutePath);
  if (!stat.isFile() || stat.isSymbolicLink()) {
    throw new Error(`GitHub output must be an existing regular file: ${basename(absolutePath)}`);
  }
  appendFileSync(
    absolutePath,
    [
      `product_version=${metadata.product.version}`,
      `installer_name=${metadata.product.installerName}`,
      `sbom_name=${metadata.product.sbomName}`,
      "",
    ].join("\n"),
    "utf8",
  );
}

const invokedPath = process.argv[1] === undefined ? "" : resolve(process.argv[1]);
if (invokedPath === fileURLToPath(import.meta.url)) {
  const { root, githubOutput } = parseArguments(process.argv.slice(2));
  const metadata = resolveReleaseMetadata(root);
  if (githubOutput !== undefined) {
    appendGithubOutputs(githubOutput, metadata);
  }
  process.stdout.write(`${JSON.stringify(metadata, null, 2)}\n`);
}
