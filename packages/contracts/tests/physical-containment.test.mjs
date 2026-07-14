import assert from "node:assert/strict";
import {
  link,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  symlink,
  writeFile,
} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  loadBindingCheckInputs,
  readContainedUtf8File,
  verifyExpectedContractFiles,
} from "../scripts/lib/controlled-contract-io.mjs";

const packageRoot = fileURLToPath(new URL("../", import.meta.url));

async function withTemporaryRoot(prefix, execute) {
  const root = await mkdtemp(path.join(os.tmpdir(), prefix));
  try {
    await execute(root);
  } finally {
    await rm(root, { force: true, recursive: true });
  }
}

async function createDirectoryLink(target, linkPath, context) {
  try {
    await symlink(target, linkPath, process.platform === "win32" ? "junction" : "dir");
    return true;
  } catch (error) {
    if (["EPERM", "EACCES", "ENOSYS"].includes(error?.code)) {
      context.skip(`directory links are unavailable: ${error.code}`);
      return false;
    }
    throw error;
  }
}

async function createFileLink(target, linkPath, context) {
  try {
    await symlink(target, linkPath, "file");
    return true;
  } catch (error) {
    if (["EPERM", "EACCES", "ENOSYS"].includes(error?.code)) {
      context.skip(`file links are unavailable: ${error.code}`);
      return false;
    }
    throw error;
  }
}

for (const mode of [
  {
    name: "generate --check",
    controlledDirectories: ["fixtures", "generated"],
    linkedRoot: "generated",
  },
  {
    name: "generate --check --typescript-only",
    controlledDirectories: ["fixtures", "generated/typescript"],
    linkedRoot: "generated/typescript",
  },
]) {
  test(`${mode.name} rejects a linked controlled root`, async (context) => {
    await withTemporaryRoot("sapphirus-contract-check-", async (temporaryRoot) => {
      const packageRoot = path.join(temporaryRoot, "package");
      const externalRoot = path.join(temporaryRoot, "external");
      await mkdir(path.dirname(path.join(packageRoot, mode.linkedRoot)), { recursive: true });
      await mkdir(path.join(packageRoot, "fixtures"), { recursive: true });
      await mkdir(externalRoot, { recursive: true });
      await writeFile(path.join(externalRoot, "escaped.txt"), "escaped\n");
      if (!await createDirectoryLink(
        externalRoot,
        path.join(packageRoot, mode.linkedRoot),
        context,
      )) return;

      await assert.rejects(
        verifyExpectedContractFiles({
          packageRoot,
          expectedFiles: new Map(),
          controlledDirectories: mode.controlledDirectories,
        }),
        /CONTRACT_PATH_CONTAINMENT.*symbolic link or junction/iu,
      );
    });
  });
}

test("generate checks reject a nested junction before reading redirected files", async (context) => {
  await withTemporaryRoot("sapphirus-contract-nested-link-", async (temporaryRoot) => {
    const packageRoot = path.join(temporaryRoot, "package");
    const schemaRoot = path.join(packageRoot, "generated", "typescript", "schema");
    const externalRoot = path.join(temporaryRoot, "external");
    await mkdir(path.dirname(schemaRoot), { recursive: true });
    await mkdir(path.join(packageRoot, "fixtures"), { recursive: true });
    await mkdir(externalRoot);
    await writeFile(path.join(externalRoot, "escaped.ts"), "export {};\n");
    if (!await createDirectoryLink(externalRoot, schemaRoot, context)) return;

    await assert.rejects(
      verifyExpectedContractFiles({
        packageRoot,
        expectedFiles: new Map(),
        controlledDirectories: ["fixtures", "generated/typescript"],
      }),
      /CONTRACT_PATH_CONTAINMENT.*symbolic link or junction/iu,
    );
  });
});

test("contained reads reject a linked file", async (context) => {
  await withTemporaryRoot("sapphirus-contract-file-link-", async (temporaryRoot) => {
    const packageRoot = path.join(temporaryRoot, "package");
    const externalFile = path.join(temporaryRoot, "external.json");
    const linkedFile = path.join(packageRoot, "schema-lock.json");
    await mkdir(packageRoot);
    await writeFile(externalFile, "{}\n");
    if (!await createFileLink(externalFile, linkedFile, context)) return;

    await assert.rejects(
      readContainedUtf8File(packageRoot, linkedFile, "schema-lock.json"),
      /CONTRACT_PATH_CONTAINMENT.*symbolic link or junction/iu,
    );
  });
});

test("contained reads reject a multiply-linked regular file", async () => {
  await withTemporaryRoot("sapphirus-contract-hard-link-", async (temporaryRoot) => {
    const packageRoot = path.join(temporaryRoot, "package");
    const externalFile = path.join(temporaryRoot, "external.json");
    const linkedFile = path.join(packageRoot, "schema-lock.json");
    await mkdir(packageRoot);
    await writeFile(externalFile, "{}\n");
    await link(externalFile, linkedFile);

    await assert.rejects(
      readContainedUtf8File(packageRoot, linkedFile, "schema-lock.json"),
      /CONTRACT_PATH_CONTAINMENT.*multiply-linked regular file/iu,
    );
  });
});

test("contained reads reject a directory where a regular file is required", async () => {
  await withTemporaryRoot("sapphirus-contract-nonregular-", async (temporaryRoot) => {
    const packageRoot = path.join(temporaryRoot, "package");
    const expectedFile = path.join(packageRoot, "generated", "typescript", "contracts.ts");
    await mkdir(expectedFile, { recursive: true });

    await assert.rejects(
      readContainedUtf8File(packageRoot, expectedFile, "generated TypeScript binding"),
      /CONTRACT_PATH_CONTAINMENT.*not a regular file/iu,
    );
  });
});

test("binding input loading rejects a nested generated-tree junction", async (context) => {
  await withTemporaryRoot("sapphirus-binding-check-link-", async (temporaryRoot) => {
    const repositoryRoot = path.join(temporaryRoot, "repository");
    const packageRoot = path.join(repositoryRoot, "packages", "contracts");
    const dotnetRoot = path.join(packageRoot, "generated", "dotnet");
    const externalRoot = path.join(temporaryRoot, "external-dotnet");
    await mkdir(path.dirname(dotnetRoot), { recursive: true });
    await mkdir(externalRoot);
    await writeFile(path.join(externalRoot, "Escaped.cs"), "// <auto-generated>\n");
    if (!await createDirectoryLink(externalRoot, dotnetRoot, context)) return;

    await assert.rejects(
      loadBindingCheckInputs({ packageRoot, repositoryRoot }),
      /CONTRACT_PATH_CONTAINMENT.*symbolic link or junction/iu,
    );
  });
});

test("generator and binding entry points delegate check reads to controlled I/O", async () => {
  const [generatorSource, bindingSource] = await Promise.all([
    readFile(path.join(packageRoot, "scripts", "generate.mjs"), "utf8"),
    readFile(path.join(packageRoot, "scripts", "check-generated-bindings.mjs"), "utf8"),
  ]);
  assert.match(generatorSource, /assertContainedPhysicalDirectory\s*\(/u);
  assert.match(generatorSource, /verifyExpectedContractFiles\s*\(/u);
  assert.doesNotMatch(generatorSource, /async function listControlledFiles\s*\(/u);
  assert.match(bindingSource, /loadBindingCheckInputs\s*\(/u);
  assert.doesNotMatch(bindingSource, /from "node:fs\/promises"/u);
});
