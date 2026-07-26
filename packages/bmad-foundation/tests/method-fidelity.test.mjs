// Fidelity conformance over the vendored BMAD Method corpus.
//
// ADR-0010 withdrew the OpenCode sidecar: Forge implements its agent runtime
// natively and re-derives discovery semantics in Rust. That trade bought a real
// enforcement boundary and cost the fidelity the sidecar would have inherited
// for free — every drift between Forge's discovery and upstream's is a *silent*
// BMAD workflow failure, not a crash.
//
// Testing is therefore the thing that makes that decision safe. `verify.mjs`
// already checks the seal: hashes, manifest, treatments — that the bytes are
// the bytes. This file checks the other half: that the corpus still has the
// SHAPE Forge's native runtime is written against.
//
// These assertions are deliberately about the corpus, not about Rust. They fail
// when the vendored Method changes underneath an implementation that assumed
// otherwise — which is the failure mode with no other alarm on it.

import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const packageRoot = fileURLToPath(new URL("..", import.meta.url));
const corpusRoot = join(packageRoot, "source", "method", "6.10.0", "src");

/** Every `SKILL.md` in the vendored corpus, as repo-relative POSIX paths. */
async function skillFiles() {
  const found = [];
  async function walk(directory, prefix) {
    const entries = await readdir(directory, { withFileTypes: true });
    for (const entry of entries) {
      const next = prefix ? `${prefix}/${entry.name}` : entry.name;
      if (entry.isDirectory()) {
        await walk(join(directory, entry.name), next);
      } else if (entry.name === "SKILL.md") {
        found.push(next);
      }
    }
  }
  await walk(corpusRoot, "");
  return found.sort();
}

test("the corpus still holds exactly the skills the runtime is written against", async () => {
  const skills = await skillFiles();
  assert.equal(
    skills.length,
    47,
    `expected 47 SKILL.md in the vendored corpus, found ${skills.length}. ` +
      "A changed count means discovery, the capability denominator, and the " +
      "fidelity matrix are all measuring something different than before.",
  );

  const core = skills.filter((path) => path.startsWith("core-skills/"));
  const bmm = skills.filter((path) => path.startsWith("bmm-skills/"));
  assert.equal(core.length, 14, "core skill count changed");
  assert.equal(bmm.length, 33, "BMM skill count changed");

  // The four phase folders are load-bearing: upstream's ordering is the
  // Method's sequence, and Forge presents it.
  const phases = new Set(bmm.map((path) => path.split("/")[1]));
  assert.deepEqual(
    [...phases].sort(),
    ["1-analysis", "2-plan-workflows", "3-solutioning", "4-implementation"],
    "the BMM phase folders changed",
  );
});

test("every skill exposes the frontmatter discovery reads", async () => {
  const skills = await skillFiles();
  const missing = [];
  for (const relative of skills) {
    const text = await readFile(join(corpusRoot, relative), "utf8");
    // Upstream skills open with YAML frontmatter carrying at least `name` and
    // `description`. Discovery keys off both: the name is the invocation
    // identity, the description is what a router matches against.
    const frontmatter = /^---\r?\n([\s\S]*?)\r?\n---/u.exec(text);
    if (!frontmatter) {
      missing.push(`${relative}: no frontmatter block`);
      continue;
    }
    const body = frontmatter[1];
    if (!/^name:\s*\S/mu.test(body)) missing.push(`${relative}: no name`);
    if (!/^description:\s*\S/mu.test(body)) {
      missing.push(`${relative}: no description`);
    }
  }
  assert.deepEqual(missing, [], `skills missing discovery frontmatter:\n${missing.join("\n")}`);
});

test("the _bmad overlay surface is the size the design assumes", async () => {
  // D6a sized the virtual `_bmad/` overlay from these counts. If they move, the
  // overlay's required surface moved with them and the design note is stale.
  const skills = await skillFiles();
  let referencesBmadTree = 0;
  let referencesConfigYaml = 0;
  for (const relative of skills) {
    const text = await readFile(join(corpusRoot, relative), "utf8");
    if (text.includes("{project-root}/_bmad")) referencesBmadTree += 1;
    if (text.includes("config.yaml")) referencesConfigYaml += 1;
  }
  assert.equal(
    referencesBmadTree,
    39,
    "the number of skills reading {project-root}/_bmad changed; the virtual " +
      "overlay in the governed-IO layer is sized against this",
  );
  assert.equal(
    referencesConfigYaml,
    36,
    "the number of skills reading config.yaml changed; that file is generated " +
      "by upstream's installer and must be synthesised by the overlay",
  );
});

test("the script dependency the corpus declares is still unvendored", async () => {
  // 39 skills shell out to `uv run {project-root}/_bmad/scripts/*.py` and the
  // corpus vendors none of those scripts. This is an open decision (bundle
  // Python, reimplement natively, or degrade those skills), and this test
  // exists so it is not silently *resolved* by someone dropping files in.
  const skills = await skillFiles();
  const invoked = new Set();
  for (const relative of skills) {
    const text = await readFile(join(corpusRoot, relative), "utf8");
    for (const match of text.matchAll(/_bmad\/scripts\/([A-Za-z0-9_]+\.py)/gu)) {
      invoked.add(match[1]);
    }
  }
  assert.deepEqual(
    [...invoked].sort(),
    ["memlog.py", "resolve_config.py", "resolve_customization.py"],
    "the set of Python scripts the corpus invokes changed",
  );

  // None of them are vendored. When that stops being true, this test should be
  // updated deliberately — it is the marker for an unresolved design decision.
  const vendored = [];
  async function walk(directory, prefix) {
    const entries = await readdir(directory, { withFileTypes: true });
    for (const entry of entries) {
      const next = prefix ? `${prefix}/${entry.name}` : entry.name;
      if (entry.isDirectory()) {
        await walk(join(directory, entry.name), next);
      } else if (entry.name.endsWith(".py")) {
        vendored.push(next);
      }
    }
  }
  await walk(corpusRoot, "");
  assert.deepEqual(
    vendored,
    [],
    "Python scripts are now vendored; the uv/Python decision has been made " +
      "in code and the packet must be updated to match",
  );
});
