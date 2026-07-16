import { lstat, readFile, readdir, realpath } from "node:fs/promises";
import path from "node:path";
import { parseStrictJson } from "./strict-json.mjs";

const CONTAINMENT_CODE = "CONTRACT_PATH_CONTAINMENT";

function fail(message) {
  throw new Error(`${CONTAINMENT_CODE}: ${message}`);
}

function normalized(value) {
  return value.replaceAll("\\", "/");
}

function assertLexicallyContained(root, candidate, label) {
  const relative = path.relative(root, candidate);
  if (relative === ".." || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative)) {
    fail(`${label} escapes its controlled root: ${normalized(candidate)}`);
  }
}

function assertPhysicallyContained(root, candidate, label) {
  const relative = path.relative(root, candidate);
  if (relative === ".." || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative)) {
    fail(`${label} resolves outside its controlled root: ${normalized(candidate)}`);
  }
}

function assertExpectedType(stats, expectedType, candidate, label) {
  if (expectedType === "file" && !stats.isFile()) {
    fail(`${label} is not a regular file: ${normalized(candidate)}`);
  }
  if (expectedType === "directory" && !stats.isDirectory()) {
    fail(`${label} is not a directory: ${normalized(candidate)}`);
  }
}

async function inspectContainedPath(controlRoot, candidate, {
  allowMissing = false,
  expectedType,
  label = "controlled path",
} = {}) {
  const root = path.resolve(controlRoot);
  const target = path.resolve(candidate);
  assertLexicallyContained(root, target, label);

  let rootStats;
  try {
    rootStats = await lstat(root);
  } catch (error) {
    if (allowMissing && error?.code === "ENOENT") return null;
    throw error;
  }
  if (rootStats.isSymbolicLink()) {
    fail(`${label} traverses a symbolic link or junction: ${normalized(root)}`);
  }
  if (!rootStats.isDirectory()) {
    fail(`controlled root is not a directory: ${normalized(root)}`);
  }

  const rootPhysical = await realpath(root);
  let cursor = root;
  let stats = rootStats;
  const relative = path.relative(root, target);
  const segments = relative === "" ? [] : relative.split(path.sep);
  for (const [index, segment] of segments.entries()) {
    cursor = path.join(cursor, segment);
    try {
      stats = await lstat(cursor);
    } catch (error) {
      if (allowMissing && error?.code === "ENOENT") return null;
      throw error;
    }
    if (stats.isSymbolicLink()) {
      fail(`${label} traverses a symbolic link or junction: ${normalized(cursor)}`);
    }
    if (stats.isFile() && stats.nlink !== 1) {
      fail(`${label} is a multiply-linked regular file: ${normalized(cursor)}`);
    }
    if (index < segments.length - 1 && !stats.isDirectory()) {
      fail(`${label} traverses a non-directory path segment: ${normalized(cursor)}`);
    }
  }

  assertExpectedType(stats, expectedType, target, label);
  const targetPhysical = await realpath(target);
  assertPhysicallyContained(rootPhysical, targetPhysical, label);
  return { path: target, stats };
}

export async function assertContainedPhysicalDirectory(controlRoot, candidate, label) {
  await inspectContainedPath(controlRoot, candidate, {
    expectedType: "directory",
    label,
  });
}

export async function readContainedUtf8File(controlRoot, candidate, label, {
  allowMissing = false,
} = {}) {
  const inspected = await inspectContainedPath(controlRoot, candidate, {
    allowMissing,
    expectedType: "file",
    label,
  });
  if (inspected === null) return undefined;
  return readFile(inspected.path, "utf8");
}

export async function listContainedRegularFiles(controlRoot, directory, label, {
  allowMissing = false,
} = {}) {
  const inspected = await inspectContainedPath(controlRoot, directory, {
    allowMissing,
    expectedType: "directory",
    label,
  });
  if (inspected === null) return [];

  const files = [];
  const visit = async (current, prefix = "") => {
    const entries = (await readdir(current, { withFileTypes: true }))
      .sort((left, right) => (left.name < right.name ? -1 : left.name > right.name ? 1 : 0));
    for (const entry of entries) {
      const absolute = path.join(current, entry.name);
      const relative = prefix === "" ? entry.name : `${prefix}/${entry.name}`;
      const child = await inspectContainedPath(controlRoot, absolute, {
        label: `${label} entry ${relative}`,
      });
      if (child.stats.isDirectory()) {
        await visit(child.path, relative);
      } else if (child.stats.isFile()) {
        files.push(relative);
      } else {
        fail(`${label} entry ${relative} is not a regular file or directory`);
      }
    }
  };
  await visit(inspected.path);
  return files;
}

export async function listContainedDirectRegularFiles(controlRoot, directory, label) {
  const inspected = await inspectContainedPath(controlRoot, directory, {
    expectedType: "directory",
    label,
  });
  const files = [];
  const entries = (await readdir(inspected.path, { withFileTypes: true }))
    .sort((left, right) => (left.name < right.name ? -1 : left.name > right.name ? 1 : 0));
  for (const entry of entries) {
    const absolute = path.join(inspected.path, entry.name);
    const child = await inspectContainedPath(controlRoot, absolute, {
      label: `${label} entry ${entry.name}`,
    });
    if (!child.stats.isFile()) {
      fail(`${label} entry ${entry.name} is not a regular file`);
    }
    files.push(entry.name);
  }
  return files;
}

export async function verifyExpectedContractFiles({
  packageRoot,
  expectedFiles,
  controlledDirectories,
}) {
  await inspectContainedPath(packageRoot, packageRoot, {
    expectedType: "directory",
    label: "contract package root",
  });
  const mismatches = [];
  for (const [relativePath, expected] of [...expectedFiles.entries()]
    .sort(([left], [right]) => (left < right ? -1 : left > right ? 1 : 0))) {
    const actual = await readContainedUtf8File(
      packageRoot,
      path.resolve(packageRoot, relativePath),
      `contract check file ${normalized(relativePath)}`,
      { allowMissing: true },
    );
    if (actual === undefined) mismatches.push(`${relativePath}: missing`);
    else if (actual !== expected) mismatches.push(`${relativePath}: content differs`);
  }

  for (const directory of controlledDirectories) {
    const normalizedDirectory = normalized(directory).replace(/\/$/u, "");
    const files = await listContainedRegularFiles(
      packageRoot,
      path.resolve(packageRoot, directory),
      `controlled contract tree ${normalizedDirectory}`,
      { allowMissing: true },
    );
    for (const file of files) {
      const relativePath = `${normalizedDirectory}/${file}`;
      if (!expectedFiles.has(relativePath)) {
        mismatches.push(`${relativePath}: unexpected generated file`);
      }
    }
  }
  return mismatches;
}

export async function loadBindingCheckInputs({ packageRoot, repositoryRoot }) {
  await inspectContainedPath(repositoryRoot, packageRoot, {
    expectedType: "directory",
    label: "contract package root",
  });

  const dotnetRoot = path.join(packageRoot, "generated", "dotnet");
  const dotnetFiles = await listContainedRegularFiles(
    packageRoot,
    dotnetRoot,
    "generated C# tree",
  );
  const dotnetFileSources = await Promise.all(dotnetFiles.map((file) =>
    readContainedUtf8File(packageRoot, path.join(dotnetRoot, file), `generated C# file ${file}`)));
  const rustSource = await readContainedUtf8File(
    packageRoot,
    path.join(packageRoot, "generated", "rust", "contracts.rs"),
    "generated Rust binding",
  );
  const typescriptSource = await readContainedUtf8File(
    packageRoot,
    path.join(packageRoot, "generated", "typescript", "contracts.ts"),
    "generated TypeScript binding",
  );
  const typescriptSchemaDirectory = path.join(
    packageRoot,
    "generated",
    "typescript",
    "schema",
  );
  const typescriptSchemaFiles = (await listContainedDirectRegularFiles(
    packageRoot,
    typescriptSchemaDirectory,
    "generated TypeScript schema tree",
  )).filter((name) => name.endsWith(".ts"));
  const typescriptSchemaSources = await Promise.all(typescriptSchemaFiles.map((name) =>
    readContainedUtf8File(
      packageRoot,
      path.join(typescriptSchemaDirectory, name),
      `generated TypeScript schema ${name}`,
    )));

  const lockSource = await readContainedUtf8File(
    packageRoot,
    path.join(packageRoot, "schema-lock.json"),
    "schema-lock.json",
  );
  const lock = parseStrictJson(lockSource);
  const toolLock = parseStrictJson(await readContainedUtf8File(
    repositoryRoot,
    path.join(repositoryRoot, "tools", "contract-codegen", "tool-lock.json"),
    "tool-lock.json",
  ));
  const schemaDirectory = path.join(packageRoot, "schemas");
  const schemaNames = (await listContainedDirectRegularFiles(
    packageRoot,
    schemaDirectory,
    "contract schema tree",
  )).filter((name) => name.endsWith(".schema.json"));
  const schemaSources = new Map(await Promise.all(schemaNames.map(async (name) => [
    name,
    await readContainedUtf8File(
      packageRoot,
      path.join(schemaDirectory, name),
      `contract schema ${name}`,
    ),
  ])));
  const lockedGeneratedSources = new Map(await Promise.all(
    lock.generatedTree.files.map(async ({ file }) => [
      file,
      await readContainedUtf8File(
        packageRoot,
        path.resolve(packageRoot, file),
        `schema-lock generated file ${file}`,
      ),
    ]),
  ));
  const packageManifestSource = await readContainedUtf8File(
    packageRoot,
    path.join(packageRoot, "package.json"),
    "contract package manifest",
  );
  const ipcEnvelopeSource = await readContainedUtf8File(
    repositoryRoot,
    path.join(repositoryRoot, "crates", "desktop-ipc", "src", "envelope.rs"),
    "desktop IPC envelope source",
  );
  const hostClientSource = await readContainedUtf8File(
    repositoryRoot,
    path.join(repositoryRoot, "apps", "desktop-ui", "src", "lib", "hostClient.ts"),
    "desktop UI IPC client source",
  );
  const desktopAppRegistrationSource = await readContainedUtf8File(
    repositoryRoot,
    path.join(repositoryRoot, "crates", "desktop-app", "src", "lib.rs"),
    "desktop application command registration source",
  );
  const desktopAppCommandCatalogSource = await readContainedUtf8File(
    repositoryRoot,
    path.join(repositoryRoot, "crates", "desktop-app", "src", "commands.rs"),
    "desktop application command catalog source",
  );
  const desktopRuntimeCommandSource = await readContainedUtf8File(
    repositoryRoot,
    path.join(repositoryRoot, "crates", "desktop-runtime", "src", "command.rs"),
    "desktop runtime command union source",
  );

  return {
    desktopAppCommandCatalogSource,
    desktopAppRegistrationSource,
    desktopRuntimeCommandSource,
    dotnetFiles,
    dotnetFileSources,
    lock,
    lockSource,
    lockedGeneratedSources,
    packageManifestSource,
    ipcEnvelopeSource,
    hostClientSource,
    rustSource,
    schemaSources,
    toolLock,
    typescriptSchemaFiles,
    typescriptSchemaSources,
    typescriptSource,
  };
}
