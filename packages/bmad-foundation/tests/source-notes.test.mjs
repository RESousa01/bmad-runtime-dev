import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const packageRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const verifier = resolve(packageRoot, "scripts", "verify-source-notes.mjs");

test("Builder source notes bind compatibility decisions to the pinned source", () => {
  const result = spawnSync(process.execPath, [verifier], {
    cwd: packageRoot,
    encoding: "utf8",
  });

  assert.equal(
    result.status,
    0,
    `source-note verification failed:\n${result.stdout}\n${result.stderr}`,
  );
  assert.match(result.stdout, /Builder 2\.1\.0 source notes verified/u);
  assert.match(result.stdout, /9 public actions \+ 1 internal Eval Runner/u);
});

test("Builder source-note verification fails closed on a changed authority decision", async () => {
  const temporaryRoot = await mkdtemp(join(tmpdir(), "bmad-source-notes-"));
  const canonicalPath = resolve(
    packageRoot,
    "source",
    "builder",
    "2.1.0",
    "source-notes.json",
  );
  const tamperedPath = join(temporaryRoot, "source-notes.json");
  try {
    const tampered = JSON.parse(await readFile(canonicalPath, "utf8"));
    tampered.runtime_authority = "authoritative_runtime_instruction";
    await writeFile(tamperedPath, `${JSON.stringify(tampered, null, 2)}\n`, "utf8");

    const result = spawnSync(
      process.execPath,
      [verifier, "--source-notes", tamperedPath],
      { cwd: packageRoot, encoding: "utf8" },
    );

    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /runtime_authority/u);
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
});
