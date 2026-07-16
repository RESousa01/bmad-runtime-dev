import { lstat, mkdir, readFile, readdir, realpath, rm, writeFile } from "node:fs/promises";
import path from "node:path";

function assertContained(root, candidate) {
  const relative = path.relative(root, candidate);
  if (relative === "" || relative === ".." || relative.startsWith(`..${path.sep}`)
    || path.isAbsolute(relative)) {
    throw new Error(`Generation transaction path escapes its root: ${candidate}`);
  }
}

async function assertPhysicalTarget(root, candidate) {
  const rootPath = path.resolve(root);
  const targetPath = path.resolve(candidate);
  assertContained(rootPath, targetPath);
  const rootRealPath = await realpath(rootPath);
  let cursor = rootPath;
  let lastExisting = rootPath;
  for (const segment of path.relative(rootPath, targetPath).split(path.sep)) {
    cursor = path.join(cursor, segment);
    try {
      const stats = await lstat(cursor);
      if (stats.isSymbolicLink()) {
        throw new Error(`Generation transaction refuses a linked path segment: ${cursor}`);
      }
      lastExisting = cursor;
    } catch (error) {
      if (error?.code === "ENOENT") break;
      throw error;
    }
  }
  const ancestorRealPath = await realpath(lastExisting);
  if (ancestorRealPath !== rootRealPath) assertContained(rootRealPath, ancestorRealPath);
  return targetPath;
}

async function captureDirectory(directory, relative = "", files = new Map()) {
  for (const entry of await readdir(path.join(directory, relative), { withFileTypes: true })) {
    const entryRelative = path.join(relative, entry.name);
    if (entry.isSymbolicLink()) {
      throw new Error(`Generation transaction refuses a linked entry: ${entryRelative}`);
    }
    if (entry.isDirectory()) {
      await captureDirectory(directory, entryRelative, files);
    } else if (entry.isFile()) {
      files.set(entryRelative, await readFile(path.join(directory, entryRelative)));
    } else {
      throw new Error(`Generation transaction refuses a non-regular entry: ${entryRelative}`);
    }
  }
  return files;
}

export async function captureGenerationState(root, targets) {
  const rootPath = path.resolve(root);
  const snapshots = [];
  for (const target of targets) {
    const targetPath = await assertPhysicalTarget(rootPath, target);
    let stats;
    try {
      stats = await lstat(targetPath);
    } catch (error) {
      if (error?.code === "ENOENT") {
        snapshots.push({ kind: "missing", path: targetPath });
        continue;
      }
      throw error;
    }
    if (stats.isSymbolicLink()) {
      throw new Error(`Generation transaction refuses a linked target: ${targetPath}`);
    }
    if (stats.isDirectory()) {
      snapshots.push({ kind: "directory", path: targetPath, files: await captureDirectory(targetPath) });
    } else if (stats.isFile()) {
      snapshots.push({ kind: "file", path: targetPath, source: await readFile(targetPath) });
    } else {
      throw new Error(`Generation transaction refuses a non-regular target: ${targetPath}`);
    }
  }
  return snapshots;
}

export async function restoreGenerationState(root, snapshots) {
  const rootPath = path.resolve(root);
  for (const snapshot of snapshots) {
    await assertPhysicalTarget(rootPath, snapshot.path);
    await rm(snapshot.path, { force: true, recursive: true });
    if (snapshot.kind === "missing") continue;
    if (snapshot.kind === "file") {
      await mkdir(path.dirname(snapshot.path), { recursive: true });
      await writeFile(snapshot.path, snapshot.source);
      continue;
    }
    await mkdir(snapshot.path, { recursive: true });
    for (const [relative, source] of snapshot.files) {
      const destination = path.resolve(snapshot.path, relative);
      assertContained(snapshot.path, destination);
      await assertPhysicalTarget(rootPath, destination);
      await mkdir(path.dirname(destination), { recursive: true });
      await writeFile(destination, source);
    }
  }
}

export async function runGenerationTransaction({ root, targets, execute }) {
  const snapshots = await captureGenerationState(root, targets);
  try {
    return await execute();
  } catch (error) {
    try {
      await restoreGenerationState(root, snapshots);
    } catch (rollbackError) {
      throw new AggregateError(
        [error, rollbackError],
        "Generation failed and its controlled-output rollback also failed.",
      );
    }
    throw error;
  }
}
