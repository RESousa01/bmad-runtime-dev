import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import process from "node:process";
import test from "node:test";

const CHECKER = "tools/check-css-conflicts.mjs";

/**
 * A throwaway CSS tree the real checker can be pointed at. Returns a `run`
 * that can be called more than once, so the ratchet -- record a baseline, then
 * verify against it -- can actually be exercised rather than assumed.
 */
function fixture(files) {
  const dir = mkdtempSync(path.join(tmpdir(), "css-conflicts-"));
  const root = path.join(dir, "src");
  mkdirSync(root, { recursive: true });
  const write = (name, css) => {
    const full = path.join(root, name);
    mkdirSync(path.dirname(full), { recursive: true });
    writeFileSync(full, css, "utf8");
  };
  for (const [name, css] of Object.entries(files)) write(name, css);

  const baselinePath = path.join(dir, "baseline.json");
  writeFileSync(baselinePath, JSON.stringify({ conflicts: [] }), "utf8");

  return {
    write,
    dispose: () => rmSync(dir, { recursive: true, force: true }),
    run: (...argv) => spawnSync(process.execPath, [CHECKER, ...argv], {
      encoding: "utf8",
      env: {
        ...process.env,
        CSS_CONFLICT_ROOTS: root,
        CSS_CONFLICT_BASELINE: baselinePath,
      },
    }),
  };
}

/** Single-shot form for the cases that do not need a second run. */
function runAgainst(files, argv = []) {
  const f = fixture(files);
  try {
    return f.run(...argv);
  } finally {
    f.dispose();
  }
}

test("reports the same property declared on the same selector by two files", () => {
  const result = runAgainst({
    "a.css": ".row { padding: 4px; }",
    "b.css": ".row { padding: 8px; }",
  });
  assert.notEqual(result.status, 0, "A cross-file conflict must fail the check.");
  assert.match(result.stderr, /\.row \{ padding \}/u);
  assert.match(result.stderr, /a\.css/u);
  assert.match(result.stderr, /b\.css/u);
});

test("accepts the same selector when the two files set different properties", () => {
  const result = runAgainst({
    "a.css": ".row { padding: 4px; }",
    "b.css": ".row { color: red; }",
  });
  assert.equal(result.status, 0, result.stderr);
});

test("accepts one file declaring a property twice, which the cascade resolves locally", () => {
  const result = runAgainst({
    "a.css": ".row { padding: 4px; }\n.row { padding: 8px; }",
  });
  assert.equal(result.status, 0, result.stderr);
});

test("ignores custom properties, because redefining a token per scope is the contract working", () => {
  const result = runAgainst({
    "a.css": ":root { --color-canvas: #fff; }",
    "b.css": ":root[data-theme='dark'] { --color-canvas: #000; }\n.x { --color-canvas: #111; }",
  });
  assert.equal(result.status, 0, result.stderr);
});

test("ignores declarations inside comments", () => {
  const result = runAgainst({
    "a.css": ".row { padding: 4px; }",
    "b.css": "/* .row { padding: 8px; } */\n.other { padding: 2px; }",
  });
  assert.equal(result.status, 0, result.stderr);
});

test("the ratchet holds: a recorded conflict passes, a new one still fails", () => {
  const f = fixture({
    "a.css": ".row { padding: 4px; }",
    "b.css": ".row { padding: 8px; }",
  });
  try {
    assert.notEqual(f.run().status, 0, "The conflict must fail before it is recorded.");

    const written = f.run("--write-baseline");
    assert.equal(written.status, 0, written.stderr);

    assert.equal(f.run().status, 0, "A recorded conflict must stop failing.");

    // A second, unrecorded conflict must still be caught -- otherwise the
    // baseline would be silencing the check rather than bounding it.
    f.write("c.css", ".row { color: red; }");
    f.write("d.css", ".row { color: blue; }");
    const after = f.run();
    assert.notEqual(after.status, 0, "A new conflict must fail even with a baseline.");
    assert.match(after.stderr, /\.row \{ color \}/u);
    assert.doesNotMatch(
      after.stderr,
      /\{ padding \}/u,
      "The already-recorded conflict must not be reported again.",
    );
  } finally {
    f.dispose();
  }
});

test("the repository itself has no unbaselined conflicts", () => {
  const result = spawnSync(process.execPath, [CHECKER], { encoding: "utf8" });
  assert.equal(
    result.status,
    0,
    `${result.stdout}\n${result.stderr}`,
  );
});
