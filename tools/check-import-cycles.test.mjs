import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import process from "node:process";
import test from "node:test";

const CHECKER = "tools/check-import-cycles.mjs";

/**
 * A throwaway module tree the real checker can be pointed at.
 *
 * The checker is only worth having if it actually finds a cycle, and the only
 * way to know that is to give it one. Running it against the repository proves
 * nothing on its own: a checker that resolves no imports at all also reports
 * zero cycles.
 */
function fixture(files) {
  const dir = mkdtempSync(path.join(tmpdir(), "import-cycles-"));
  for (const [name, source] of Object.entries(files)) {
    const full = path.join(dir, name);
    mkdirSync(path.dirname(full), { recursive: true });
    writeFileSync(full, source, "utf8");
  }
  return {
    cleanup: () => rmSync(dir, { force: true, recursive: true }),
    run: () => {
      const result = spawnSync(
        process.execPath,
        [CHECKER, "--json", "--root", dir],
        { encoding: "utf8" },
      );
      return { ...result, report: JSON.parse(result.stdout) };
    },
  };
}

test("reports a two-module cycle", () => {
  const f = fixture({
    "a.ts": 'import { b } from "./b";\nexport const a = () => b();\n',
    "b.ts": 'import { a } from "./a";\nexport const b = () => a();\n',
  });
  try {
    const { report, status } = f.run();
    assert.equal(status, 1);
    assert.equal(report.cycles.length, 1);
    assert.equal(report.cycles[0].length, 2);
  } finally {
    f.cleanup();
  }
});

test("reports a cycle that runs through a third module", () => {
  // The shape a decomposition actually produces: App imports a controller, the
  // controller imports a helper, the helper reaches back into App.
  const f = fixture({
    "a.ts": 'import { b } from "./b";\nexport const a = () => b();\n',
    "b.ts": 'import { c } from "./c";\nexport const b = () => c();\n',
    "c.ts": 'import { a } from "./a";\nexport const c = () => a();\n',
  });
  try {
    const { report, status } = f.run();
    assert.equal(status, 1);
    assert.equal(report.cycles[0].length, 3);
  } finally {
    f.cleanup();
  }
});

test("passes on an acyclic graph", () => {
  const f = fixture({
    "a.ts": 'import { b } from "./b";\nexport const a = () => b();\n',
    "b.ts": "export const b = () => 1;\n",
    "c.ts": 'import { b } from "./b";\nexport const c = () => b();\n',
  });
  try {
    const { report, status } = f.run();
    assert.equal(status, 0);
    assert.deepEqual(report.cycles, []);
    assert.equal(report.scanned, 3);
  } finally {
    f.cleanup();
  }
});

test("ignores a type-only edge", () => {
  // `import type` is erased before the bundler sees it, so it cannot produce an
  // initialisation-order problem. Forbidding it would push code towards
  // duplicating types rather than sharing them.
  const f = fixture({
    "a.ts": 'import { b } from "./b";\nexport const a = () => b();\nexport type A = number;\n',
    "b.ts": 'import type { A } from "./a";\nexport const b = (): A => 1;\n',
  });
  try {
    const { report, status } = f.run();
    assert.equal(status, 0);
    assert.deepEqual(report.cycles, []);
  } finally {
    f.cleanup();
  }
});

test("counts an inline value specifier as a real import", () => {
  // `import { type X, y }` still binds `y` at runtime. One value binding is
  // enough to make the edge real.
  const f = fixture({
    "a.ts": 'import { type B, b } from "./b";\nexport const a = (): B => b();\nexport type A = number;\n',
    "b.ts": 'import { a } from "./a";\nexport type B = number;\nexport const b = () => a();\n',
  });
  try {
    const { status } = f.run();
    assert.equal(status, 1);
  } finally {
    f.cleanup();
  }
});

test("resolves a directory import through its index", () => {
  const f = fixture({
    "a.ts": 'import { b } from "./feature";\nexport const a = () => b();\n',
    "feature/index.ts": 'import { a } from "../a";\nexport const b = () => a();\n',
  });
  try {
    const { status } = f.run();
    assert.equal(status, 1);
  } finally {
    f.cleanup();
  }
});

test("does not treat a self-import-free module as a cycle", () => {
  const f = fixture({ "a.ts": "export const a = () => 1;\n" });
  try {
    const { report, status } = f.run();
    assert.equal(status, 0);
    assert.deepEqual(report.cycles, []);
  } finally {
    f.cleanup();
  }
});

test("skips test files, which are leaves that never ship", () => {
  const f = fixture({
    "a.ts": "export const a = () => 1;\n",
    "a.test.ts": 'import { a } from "./a";\nexport const t = a;\n',
  });
  try {
    const { report } = f.run();
    assert.equal(report.scanned, 1);
  } finally {
    f.cleanup();
  }
});

test("ignores imports that leave the scanned roots", () => {
  const f = fixture({
    "a.ts": 'import { x } from "react";\nimport { b } from "./b";\nexport const a = () => b() + x;\n',
    "b.ts": "export const b = () => 1;\n",
  });
  try {
    const { status } = f.run();
    assert.equal(status, 0);
  } finally {
    f.cleanup();
  }
});
