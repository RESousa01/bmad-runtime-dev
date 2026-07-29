import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const currentRoot = path.join(packageRoot, "fixtures", "current");
const expectedFiles = [
  "fixture-migration-map.json",
  "normalized-packages.json",
  "runtime-outcomes.json",
  "source-and-catalog.json",
];

async function fixture(name) {
  return JSON.parse(await readFile(path.join(currentRoot, name), "utf8"));
}

function exactKeys(value, keys, location) {
  assert.deepEqual(Object.keys(value).sort(), [...keys].sort(), `${location} keys`);
}

test("SRC-05-NODE-TREE-001 historical evidence has a closed four-file tree", async () => {
  const entries = await readdir(currentRoot, { withFileTypes: true });
  assert.deepEqual(entries.map((entry) => entry.name).sort(), expectedFiles);
  assert.ok(entries.every((entry) => entry.isFile() && !entry.isSymbolicLink()));
});

test("SRC-05-NODE-SOURCE-001 freezes the baseline ledgers and capability denominator", async () => {
  const evidence = await fixture("source-and-catalog.json");
  exactKeys(
    evidence,
    ["schemaVersion", "baselineTree", "sourceLedger", "closureLedger"],
    "source-and-catalog",
  );
  assert.equal(evidence.schemaVersion, "sapphirus.bmad.current-source-catalog/v1");
  assert.equal(
    evidence.baselineTree,
    "c0af6450ef0e8b3cd9e67043e0c753e1956528fe",
  );
  assert.deepEqual(evidence.sourceLedger, {
    schemaVersion: "sapphirus.bmad.semantic-source-ledger/v1",
    sourceMemberCount: 294,
    managedOutputCount: 40,
    adoptedSourceCount: 228,
    contentHash:
      "sha256:f5599c0ccbd727472101aab6e27c269a0902c9a78d4e3b28c15ae110d7afae71",
  });
  assert.equal(evidence.closureLedger.schemaVersion, "sapphirus.bmad-capability-closure.v1");
  assert.equal(evidence.closureLedger.capabilityIds.length, 38);
  assert.deepEqual(evidence.closureLedger.builderCapabilityIds, [
    "builder:agent.analyze",
    "builder:agent.create_rebuild",
    "builder:agent.edit",
    "builder:workflow.analyze",
    "builder:workflow.build_edit",
  ]);
  assert.deepEqual(
    evidence.closureLedger.capabilityIds.filter((id) => id.startsWith("builder:")),
    evidence.closureLedger.builderCapabilityIds,
  );
});

test("SRC-05-NODE-PACKAGE-001 freezes normalized Help and inactive Builder packages", async () => {
  const evidence = await fixture("normalized-packages.json");
  assert.equal(evidence.schemaVersion, "sapphirus.bmad.current-normalized-packages/v1");
  assert.equal(evidence.normalizedInventory.contractCount, 35);
  assert.equal(evidence.normalizedInventory.paths.length, 35);
  assert.equal(new Set(evidence.normalizedInventory.paths).size, 35);
  assert.equal(
    evidence.normalizedInventory.runtimeManifestHash,
    "sha256:52a21580d10277e8670cec85a6684c3f1089dcd474eb44a856694a22b3b77021",
  );
  assert.equal(evidence.packages.length, 3);
  assert.deepEqual(
    evidence.packages.map(({ path, contentHash }) => [path, contentHash]),
    [
      [
        "normalized/bmad-help.package.json",
        "sha256:db10c735500110d971229aa0c5c09a5320ffcf290abdd3bba7118979e84dd95d",
      ],
      [
        "normalized/builder-agent.package.json",
        "sha256:9ee163d13df2020d4b351b376ee41fa615e6d770e67fb61af125a13353c46add",
      ],
      [
        "normalized/builder-workflow.package.json",
        "sha256:30cbe39a06007654663529740de2c7c77ea3278e968081a1e2758367566bc30b",
      ],
    ],
  );
  assert.deepEqual(evidence.packages[1].actions, ["create_rebuild", "edit", "analyze"]);
  assert.deepEqual(evidence.packages[2].actions, ["build", "edit", "analyze"]);
  assert.ok(evidence.packages.slice(1).every((item) =>
    item.lifecycleState === "inactive_data" && item.activationAuthority === "none"));
});

test("SRC-05-NODE-HELP-001 freezes Help, failures, cancellation, resume, and inert projections", async () => {
  const evidence = await fixture("runtime-outcomes.json");
  assert.equal(evidence.schemaVersion, "sapphirus.bmad.current-runtime-outcomes/v1");
  assert.deepEqual(evidence.helpRecommendation, {
    confidence: "heuristic",
    completionClaimed: false,
    availability: "capability_disabled",
    expectedOutputs: ["architecture"],
    requiredGuidance: true,
    blockerCodes: ["bmad_capability_disabled"],
  });
  assert.deepEqual(evidence.terminalStates, ["cancelled", "failed", "refused"]);
  assert.deepEqual(evidence.unsupportedStates, [
    "capability_disabled",
    "dependency_unavailable",
    "source_prompt_unavailable",
  ]);
  assert.equal(evidence.resumeBehavior, "read_only_projection");
  assert.deepEqual(evidence.builderProjection, {
    activationState: "installed_inactive",
    blockerCodes: ["builder_engine_gated"],
    exposesDraftFileBytes: false,
  });
  assert.deepEqual(evidence.knownDefects.ordinaryChatBmadCoupling, {
    historicalDefect: "chat_unavailable_when_bmad_commands_missing",
    baselineActual: "chat_available_when_chat_send_and_model_route_are_ready",
    targetExpected: "chat_available_when_chat_send_and_model_route_are_ready",
    status: "already_repaired",
    replacementTestId: "SRC-05-UI-COUPLING-001",
  });
  assert.deepEqual(evidence.knownDefects.inertBuilder, {
    historicalActual: "five_capability_ids_project_inactive_drafts_only",
    targetExpected: "migration_must_preserve_drafts_before_new_lifecycle_authority",
    replacementTestId: "SRC-05-RUST-BUILDER-001",
  });
});

test("SRC-05-NODE-MIGRATION-001 maps every legacy fixture to replacement evidence", async () => {
  const evidence = await fixture("fixture-migration-map.json");
  assert.equal(evidence.schemaVersion, "sapphirus.bmad.current-fixture-migration/v1");
  assert.deepEqual(
    evidence.fixtures.map(({ path, disposition, replacementTestId }) => ({
      path,
      disposition,
      replacementTestId,
    })),
    [
      {
        path: "fixtures/sealed-method-bmad-help.json",
        disposition: "retained",
        replacementTestId: "SRC-05-NODE-HELP-001",
      },
      {
        path: "fixtures/inactive-stateless-agent.json",
        disposition: "migrated",
        replacementTestId: "SRC-05-RUST-BUILDER-001",
      },
      {
        path: "fixtures/inactive-simple-workflow.json",
        disposition: "migrated",
        replacementTestId: "SRC-05-RUST-BUILDER-001",
      },
    ],
  );
  assert.ok(evidence.fixtures.every((item) =>
    ["retained", "migrated", "removed"].includes(item.disposition)
    && /^SRC-05-[A-Z]+-[A-Z]+-[0-9]{3}$/u.test(item.replacementTestId)));
});
