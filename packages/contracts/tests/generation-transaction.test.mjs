import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { runGenerationTransaction } from "../scripts/lib/generation-transaction.mjs";

const packageRoot = fileURLToPath(new URL("../", import.meta.url));

test("generation transaction restores controlled outputs after a later-stage failure", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "sapphirus-generation-transaction-"));
  const generated = path.join(root, "generated");
  const lock = path.join(root, "schema-lock.json");
  await mkdir(generated);
  await writeFile(path.join(generated, "before.txt"), "before\n");
  await writeFile(lock, "{\"before\":true}\n");
  try {
    await assert.rejects(
      runGenerationTransaction({
        root,
        targets: [generated, lock],
        execute: async () => {
          await writeFile(path.join(generated, "before.txt"), "changed\n");
          await writeFile(path.join(generated, "partial.txt"), "partial\n");
          await writeFile(lock, "{\"partial\":true}\n");
          throw new Error("simulated later-stage failure");
        },
      }),
      /simulated later-stage failure/u,
    );
    assert.equal(await readFile(path.join(generated, "before.txt"), "utf8"), "before\n");
    await assert.rejects(readFile(path.join(generated, "partial.txt")), { code: "ENOENT" });
    assert.equal(await readFile(lock, "utf8"), "{\"before\":true}\n");
  } finally {
    await rm(root, { force: true, recursive: true });
  }
});

test("package generation is transaction-orchestrated and config-digests both transaction scripts", async () => {
  const packageManifest = JSON.parse(await readFile(path.join(packageRoot, "package.json"), "utf8"));
  assert.equal(packageManifest.scripts.generate, "node ./scripts/generate-all.mjs");
  const nativeCodegen = await readFile(
    path.join(packageRoot, "scripts", "lib", "native-codegen.mjs"),
    "utf8",
  );
  assert.match(nativeCodegen, /packages\/contracts\/scripts\/generate-all\.mjs/u);
  assert.match(nativeCodegen, /packages\/contracts\/scripts\/lib\/generation-transaction\.mjs/u);
  assert.match(nativeCodegen, /packages\/contracts\/scripts\/lib\/controlled-contract-io\.mjs/u);
});
