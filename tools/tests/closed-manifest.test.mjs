import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { canonicalTextBytes, verifyClosedManifest } from "../lib/closed-manifest.mjs";

const hash = (payload) => createHash("sha256").update(payload).digest("hex");

test("canonical text bytes make LF and CRLF vault notes equivalent", () => {
  assert.deepEqual(canonicalTextBytes(Buffer.from("one\r\ntwo\r\n")), Buffer.from("one\ntwo\n"));
});

async function fixture(t) {
  const root = await mkdtemp(join(tmpdir(), "living-manifest-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  await mkdir(join(root, "current"));
  await mkdir(join(root, "evidence"));
  const payload = Buffer.from("authority\n");
  await writeFile(join(root, "current", "state.md"), payload);
  return {
    root,
    manifest: {
      schemaVersion: "sapphirus.living-knowledge-manifest.v1",
      files: [{ path: "current/state.md", bytes: payload.length, sha256: hash(payload) }],
    },
  };
}

test("closed living manifest accepts its exact file set", async (t) => {
  const value = await fixture(t);
  await verifyClosedManifest({ ...value, directories: ["current", "evidence"] });
});

test("closed living manifest rejects mutated, added, and removed files", async (t) => {
  await t.test("mutation", async (t) => {
    const value = await fixture(t);
    await writeFile(join(value.root, "current", "state.md"), "changed\n");
    await assert.rejects(
      verifyClosedManifest({ ...value, directories: ["current", "evidence"] }),
      /differs from its living manifest record/,
    );
  });
  await t.test("addition", async (t) => {
    const value = await fixture(t);
    await writeFile(join(value.root, "evidence", "extra.json"), "{}\n");
    await assert.rejects(
      verifyClosedManifest({ ...value, directories: ["current", "evidence"] }),
      /file set drifted/,
    );
  });
  await t.test("removal", async (t) => {
    const value = await fixture(t);
    await rm(join(value.root, "current", "state.md"));
    await assert.rejects(
      verifyClosedManifest({ ...value, directories: ["current", "evidence"] }),
      /file set drifted/,
    );
  });
});
