// Fidelity conformance over the vendored BMAD Method corpus.
//
// ADR-0010 withdrew the OpenCode sidecar: Forge implements its agent runtime
// natively and re-derives discovery semantics in Rust. That trade bought a real
// enforcement boundary and cost the fidelity the sidecar would have inherited
// for free — every drift between Forge's discovery and upstream's is a *silent*
// BMAD workflow failure, not a crash.
//
// Testing is therefore the thing that makes that decision safe. `verify.mjs`
// already checks the seal: hashes and manifests — that the bytes are the
// pinned bytes. This file checks the other half: that the complete upstream
// corpus still has the SHAPE Forge's native runtime is written against.
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
    48,
    `expected 48 primary SKILL.md in the vendored corpus, found ${skills.length}. ` +
      "A changed count means discovery, the capability denominator, and the " +
      "fidelity matrix are all measuring something different than before.",
  );

  const core = skills.filter((path) => path.startsWith("core-skills/"));
  const bmm = skills.filter((path) => path.startsWith("bmm-skills/"));
  assert.equal(core.length, 14, "core skill count changed");
  assert.equal(bmm.length, 34, "BMM skill count changed");

  // The four phase folders are load-bearing: upstream's ordering is the
  // Method's sequence, and Forge presents it. The pinned archive also retains
  // one explicitly named v6 compatibility-shim surface.
  const phases = new Set(bmm.map((path) => path.split("/")[1]));
  assert.deepEqual(
    [...phases].sort(),
    ["1-analysis", "2-plan-workflows", "3-solutioning", "4-implementation", "v6-shims"],
    "the BMM phase and compatibility folders changed",
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
    41,
    "the number of skills reading {project-root}/_bmad changed; the virtual " +
      "overlay in the governed-IO layer is sized against this",
  );
  assert.equal(
    referencesConfigYaml,
    28,
    "the number of skills reading config.yaml changed; that file is generated " +
      "by upstream's installer and must be synthesised by the overlay",
  );
});

test("the complete pinned Python surface remains explicit and inert", async () => {
  // The complete 6.10.0 archive includes the Python helpers referenced by
  // skills, additional skill-local helpers, and their upstream tests. Retaining
  // these exact untrusted bytes does not grant process-execution authority.
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
    vendored.sort(),
    [
      "bmm-skills/3-solutioning/bmad-architecture/scripts/lint_spine.py",
      "bmm-skills/3-solutioning/bmad-architecture/scripts/tests/test_lint_spine.py",
      "bmm-skills/4-implementation/bmad-quick-dev/render.py",
      "core-skills/bmad-advanced-elicitation/scripts/pick_methods.py",
      "core-skills/bmad-advanced-elicitation/scripts/tests/test_pick_methods.py",
      "core-skills/bmad-brainstorming/scripts/brain.py",
      "core-skills/bmad-brainstorming/scripts/tests/test_brain.py",
      "core-skills/bmad-customize/scripts/list_customizable_skills.py",
      "core-skills/bmad-customize/scripts/tests/test_list_customizable_skills.py",
      "core-skills/bmad-deep-recon/scripts/recon_kit.py",
      "core-skills/bmad-deep-recon/scripts/tests/test_recon_kit.py",
      "core-skills/bmad-forge-idea/scripts/resolve_personas.py",
      "core-skills/bmad-forge-idea/scripts/tests/test_resolve_personas.py",
      "core-skills/bmad-party-mode/scripts/resolve_party.py",
      "core-skills/bmad-party-mode/scripts/tests/test_resolve_party.py",
      "core-skills/bmad-review/scripts/tests/test_word_metrics.py",
      "core-skills/bmad-review/scripts/word_metrics.py",
      "scripts/memlog.py",
      "scripts/resolve_config.py",
      "scripts/resolve_customization.py",
      "scripts/tests/test_memlog.py",
      "scripts/tests/test_resolve_customization.py",
    ],
    "the exact Python surface in the pinned Method archive changed",
  );
});
