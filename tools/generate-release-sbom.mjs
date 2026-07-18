import { lstatSync, readFileSync, writeFileSync } from "node:fs";
import process from "node:process";
import { basename, dirname, isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { resolveReleaseMetadata } from "./resolve-release-metadata.mjs";

function packageIdentity(key) {
  const at = key.lastIndexOf("@");
  if (at <= 0 || at === key.length - 1) {
    throw new Error(`invalid pnpm package key: ${key}`);
  }
  const name = key.slice(0, at);
  const version = key.slice(at + 1).split("(", 1)[0];
  if (name.length === 0 || version.length === 0) {
    throw new Error(`invalid pnpm package key: ${key}`);
  }
  return { name, version };
}

function npmPurl(name, version) {
  if (name.startsWith("@")) {
    const slash = name.indexOf("/");
    if (slash <= 1 || slash === name.length - 1) {
      throw new Error(`invalid scoped npm package: ${name}`);
    }
    return `pkg:npm/${encodeURIComponent(name.slice(0, slash))}/${encodeURIComponent(name.slice(slash + 1))}@${encodeURIComponent(version)}`;
  }
  return `pkg:npm/${encodeURIComponent(name)}@${encodeURIComponent(version)}`;
}

function parsePnpmComponents(source) {
  const lines = source.replaceAll("\r\n", "\n").split("\n");
  const start = lines.findIndex((line) => line === "packages:");
  if (start < 0) {
    throw new Error("pnpm lock is missing the packages section");
  }
  const components = [];
  for (let index = start + 1; index < lines.length;) {
    if (/^[^\s]/u.test(lines[index]) && lines[index].length > 0) {
      break;
    }
    const match = lines[index].match(/^  (.+):\s*$/u);
    if (match === null) {
      index += 1;
      continue;
    }
    const rawKey = match[1];
    const key = rawKey.startsWith("'") && rawKey.endsWith("'")
      ? rawKey.slice(1, -1)
      : rawKey;
    const block = [];
    index += 1;
    while (index < lines.length && !/^  \S/u.test(lines[index]) && !/^[^\s]/u.test(lines[index])) {
      block.push(lines[index]);
      index += 1;
    }
    const { name, version } = packageIdentity(key);
    const integrityMatch = block.join("\n").match(/integrity:\s*(?:'([^']+)'|"([^"]+)"|([^,}\s]+))/u);
    const integrity = integrityMatch?.[1] ?? integrityMatch?.[2] ?? integrityMatch?.[3];
    const component = {
      type: "library",
      name,
      version,
      purl: npmPurl(name, version),
    };
    if (integrity !== undefined) {
      const digestMatch = integrity.match(/^sha512-([A-Za-z0-9+/]+={0,2})$/u);
      if (digestMatch === null) {
        throw new Error(`unsupported pnpm integrity for ${key}`);
      }
      const bytes = Buffer.from(digestMatch[1], "base64");
      if (bytes.length !== 64) {
        throw new Error(`invalid SHA-512 integrity for ${key}`);
      }
      component.hashes = [{ alg: "SHA-512", content: bytes.toString("hex") }];
    }
    components.push(component);
  }
  return components;
}

function parseCargoComponents(source) {
  const components = [];
  for (const block of source.replaceAll("\r\n", "\n").split(/^\[\[package\]\]\s*$/mu).slice(1)) {
    const name = block.match(/^name\s*=\s*"([^"]+)"\s*$/mu)?.[1];
    const version = block.match(/^version\s*=\s*"([^"]+)"\s*$/mu)?.[1];
    if (name === undefined || version === undefined) {
      throw new Error("Cargo lock contains an incomplete package identity");
    }
    const checksum = block.match(/^checksum\s*=\s*"([0-9a-f]{64})"\s*$/mu)?.[1];
    const component = {
      type: "library",
      name,
      version,
      purl: `pkg:cargo/${encodeURIComponent(name)}@${encodeURIComponent(version)}`,
    };
    if (checksum !== undefined) {
      component.hashes = [{ alg: "SHA-256", content: checksum }];
    }
    components.push(component);
  }
  return components;
}

function deduplicateComponents(components) {
  const byPurl = new Map();
  for (const component of components) {
    const previous = byPurl.get(component.purl);
    if (previous !== undefined && JSON.stringify(previous) !== JSON.stringify(component)) {
      throw new Error(`conflicting lock identities for ${component.purl}`);
    }
    byPurl.set(component.purl, component);
  }
  return [...byPurl.values()].sort((left, right) => left.purl < right.purl ? -1 : left.purl > right.purl ? 1 : 0);
}

export function createCycloneDxSbom({ releaseMetadata, pnpmLock, cargoLock }) {
  const components = deduplicateComponents([
    ...parseCargoComponents(cargoLock),
    ...parsePnpmComponents(pnpmLock),
  ]);
  return {
    $schema: "https://cyclonedx.org/schema/bom-1.6.schema.json",
    bomFormat: "CycloneDX",
    specVersion: "1.6",
    version: 1,
    metadata: {
      component: {
        type: "application",
        "bom-ref": `pkg:generic/sapphirus@${encodeURIComponent(releaseMetadata.product.version)}`,
        name: releaseMetadata.product.name,
        version: releaseMetadata.product.version,
        purl: `pkg:generic/sapphirus@${encodeURIComponent(releaseMetadata.product.version)}`,
      },
      properties: [
        { name: `sapphirus:lock:${releaseMetadata.locks.cargo.path}:sha256`, value: releaseMetadata.locks.cargo.sha256 },
        { name: `sapphirus:lock:${releaseMetadata.locks.pnpm.path}:sha256`, value: releaseMetadata.locks.pnpm.sha256 },
        { name: "sapphirus:scope", value: "complete-build-lock-inventory" },
      ],
    },
    components,
  };
}

export function serializeCycloneDxSbom(sbom) {
  return `${JSON.stringify(sbom, null, 2)}\n`;
}

export function verifyCycloneDxSbom({ releaseMetadata, pnpmLock, cargoLock, candidate }) {
  const expected = serializeCycloneDxSbom(createCycloneDxSbom({ releaseMetadata, pnpmLock, cargoLock }));
  if (!Buffer.from(candidate).equals(Buffer.from(expected))) {
    throw new Error("SBOM bytes disagree with the current release metadata and lock inventory");
  }
  return expected;
}

function parseArguments(argv) {
  let root = process.cwd();
  let output;
  let verify;
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    const value = argv[index + 1];
    if (argument === "--root" && value !== undefined) {
      root = value;
      index += 1;
    } else if (argument === "--output" && value !== undefined) {
      output = value;
      index += 1;
    } else if (argument === "--verify" && value !== undefined) {
      verify = value;
      index += 1;
    } else {
      throw new Error(`unknown or incomplete argument: ${argument}`);
    }
  }
  if ((output === undefined) === (verify === undefined)) {
    throw new Error("exactly one of --output or --verify is required");
  }
  return {
    root: resolve(root),
    output: output === undefined ? undefined : isAbsolute(output) ? output : resolve(output),
    verify: verify === undefined ? undefined : isAbsolute(verify) ? verify : resolve(verify),
  };
}

function writeSbom(root, output) {
  const releaseMetadata = resolveReleaseMetadata(root);
  if (basename(output) !== releaseMetadata.product.sbomName) {
    throw new Error(`SBOM output must be named ${releaseMetadata.product.sbomName}`);
  }
  const parent = lstatSync(dirname(output));
  if (!parent.isDirectory() || parent.isSymbolicLink()) {
    throw new Error("SBOM output parent must be an existing regular directory");
  }
  try {
    lstatSync(output);
    throw new Error("SBOM output must not already exist");
  } catch (error) {
    if (error?.code !== "ENOENT") {
      throw error;
    }
  }
  const sbom = createCycloneDxSbom({
    releaseMetadata,
    pnpmLock: readFileSync(join(root, "pnpm-lock.yaml"), "utf8"),
    cargoLock: readFileSync(join(root, "Cargo.lock"), "utf8"),
  });
  writeFileSync(output, serializeCycloneDxSbom(sbom), { encoding: "utf8", flag: "wx" });
  process.stdout.write(`${output}\n`);
}

function verifySbom(root, candidatePath) {
  const candidateStat = lstatSync(candidatePath);
  if (!candidateStat.isFile() || candidateStat.isSymbolicLink()) {
    throw new Error("SBOM candidate must be a regular file");
  }
  const releaseMetadata = resolveReleaseMetadata(root);
  verifyCycloneDxSbom({
    releaseMetadata,
    pnpmLock: readFileSync(join(root, "pnpm-lock.yaml"), "utf8"),
    cargoLock: readFileSync(join(root, "Cargo.lock"), "utf8"),
    candidate: readFileSync(candidatePath),
  });
  process.stdout.write(`${candidatePath}\n`);
}

const invokedPath = process.argv[1] === undefined ? "" : resolve(process.argv[1]);
if (invokedPath === fileURLToPath(import.meta.url)) {
  const { root, output, verify } = parseArguments(process.argv.slice(2));
  if (output !== undefined) {
    writeSbom(root, output);
  } else {
    verifySbom(root, verify);
  }
}
