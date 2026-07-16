import { createHash } from "node:crypto";
import { readFile, readdir } from "node:fs/promises";
import { join, relative, resolve, sep } from "node:path";

function sha256(payload) {
  return createHash("sha256").update(payload).digest("hex");
}

async function walk(directory) {
  const files = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) files.push(...(await walk(path)));
    else if (entry.isFile()) files.push(path);
  }
  return files;
}

export async function verifyClosedManifest({ root, manifest, directories }) {
  const rootPath = resolve(root);
  const records = new Map();
  if (manifest?.schemaVersion !== "sapphirus.living-knowledge-manifest.v1" || !Array.isArray(manifest.files)) {
    throw new Error("living manifest has unsupported or malformed top-level data");
  }
  for (const record of manifest.files) {
    if (
      typeof record?.path !== "string" ||
      !Number.isSafeInteger(record?.bytes) ||
      record.bytes < 0 ||
      !/^[0-9a-f]{64}$/.test(record?.sha256 ?? "") ||
      records.has(record.path)
    ) {
      throw new Error("living manifest contains a malformed or duplicate record");
    }
    const target = resolve(rootPath, ...record.path.split("/"));
    if (target !== rootPath && !target.startsWith(`${rootPath}${sep}`)) {
      throw new Error("living manifest record escapes its root");
    }
    records.set(record.path, record);
  }

  const actualPaths = [];
  for (const directory of directories) {
    for (const path of await walk(join(rootPath, directory))) {
      actualPaths.push(relative(rootPath, path).split(sep).join("/"));
    }
  }
  actualPaths.sort();
  if (actualPaths.length !== records.size || actualPaths.some((path) => !records.has(path))) {
    throw new Error("living manifest file set drifted");
  }
  for (const path of actualPaths) {
    const payload = await readFile(join(rootPath, ...path.split("/")));
    const expected = records.get(path);
    if (payload.length !== expected.bytes || sha256(payload) !== expected.sha256) {
      throw new Error(`${path} differs from its living manifest record`);
    }
  }
}
