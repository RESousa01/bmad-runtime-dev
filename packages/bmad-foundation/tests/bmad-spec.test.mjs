import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const packageRoot = fileURLToPath(new URL("../", import.meta.url));
const readJson = async (relativePath) =>
  JSON.parse(await readFile(`${packageRoot}/${relativePath}`, "utf8"));

test("bmad-spec is exposed from the canonical catalog without aliases", async () => {
  const catalog = await readJson("capability-catalog.json");
  const spec = catalog.capabilities.find(
    (capability) => capability.canonicalId === "bmm:bmad-spec",
  );
  assert.equal(spec.supportState, "adapted");
  assert.equal(spec.uiState, "planned");
  assert.deepEqual(spec.aliases, []);
  assert.deepEqual(spec.modes, ["interactive", "headless"]);
});

test("bmad-spec normalized projection closes all five pinned source resources", async () => {
  const projection = await readJson("normalized/bmad-spec.package.json");
  assert.equal(projection.lifecycleState, "sealed_read_only");
  assert.equal(projection.capability.skillName, "bmad-spec");
  assert.equal(projection.instructionProjection.sourceEntrypoint.treatment, "adapt");
  assert.equal(projection.instructionProjection.sourceResources.length, 4);
  assert.deepEqual(
    projection.instructionProjection.sourceResources.map(({ path }) => path),
    [
      "src/bmm-skills/2-plan-workflows/bmad-spec/customize.toml",
      "src/bmm-skills/2-plan-workflows/bmad-spec/assets/headless-schemas.md",
      "src/bmm-skills/2-plan-workflows/bmad-spec/assets/spec-template.md",
      "src/bmm-skills/2-plan-workflows/bmad-spec/assets/stories-schema.md",
    ],
  );
  assert.match(projection.projectionEnvelopeHash, /^sha256:[0-9a-f]{64}$/u);
});

test("bmad-spec managed instructions retain source branches and authority boundaries", async () => {
  const instructions = await readFile(
    `${packageRoot}/runtime/method/6.10.0/spec.instructions.md`,
    "utf8",
  );
  for (const required of [
    "insufficient_intent",
    "missing_slug",
    "existing slug is an update",
    "no change signal",
    "interactive-only",
    "coherence, then preservation",
    "grants no filesystem, process, model, network",
  ]) {
    assert.match(instructions, new RegExp(required, "iu"));
  }
});

test("bmad-spec is preserved but absent from advertised generated consumers", async () => {
  const [help, compatibility, adoption, rust, typescript] = await Promise.all([
    readJson("normalized/bmad-help-capability-graph.json"),
    readJson("compatibility-ledger.json"),
    readJson("adoption-ledger.json"),
    readFile(`${packageRoot}/../../crates/desktop-app/src/generated/bmad_capabilities.rs`, "utf8"),
    readFile(`${packageRoot}/../../apps/desktop-ui/src/generated/bmadCapabilities.ts`, "utf8"),
  ]);
  const helpNode = help.nodes.find((capability) => capability.id === "bmm:bmad-spec");
  assert.deepEqual(
    {
      uiState: helpNode?.uiState,
      predecessorIds: helpNode?.predecessorIds,
      followerIds: helpNode?.followerIds,
      outputLocations: helpNode?.outputLocations,
    },
    {
      uiState: "planned",
      predecessorIds: [],
      followerIds: [],
      outputLocations: ["governed_artifact"],
    },
  );
  const compatibilityRow = compatibility.rows.find(
    (row) => row.rowId === "method:src/bmm-skills/2-plan-workflows/bmad-spec/SKILL.md",
  );
  assert.equal(compatibilityRow?.decision, "adapt");
  const managedOutput = adoption.runtimeProjections.find(
    (output) => output.path === "runtime/method/6.10.0/spec.instructions.md",
  );
  assert.equal(managedOutput?.authority, "none");
  assert.match(rust, /canonical_id: "bmm:bmad-spec"/u);
  assert.match(typescript, /"canonicalId": "bmm:bmad-spec"/u);
  assert.doesNotMatch(
    rust.split("pub const UI_ADVERTISED_CAPABILITIES", 2)[1],
    /canonical_id: "bmm:bmad-spec"/u,
  );
});
