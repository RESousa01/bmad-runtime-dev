import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import test from "node:test";

import {
  buildCatalog,
  buildAliasRegistry,
  resolveLegacyBuilderId,
  parseModuleHelpCsv,
} from "../scripts/generate-builder-catalog.mjs";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const csvPath = resolve(
  packageRoot,
  "source/builder/2.1.0/skills/module-help.csv",
);
const evalSkillPath = resolve(
  packageRoot,
  "source/builder/2.1.0/skills/bmad-eval-runner/SKILL.md",
);
const catalogPath = resolve(packageRoot, "normalized/builder-capability-catalog.json");
const registryPath = resolve(packageRoot, "normalized/builder-alias-registry.json");

async function readSources() {
  const [csvText, evalSkillText] = await Promise.all([
    readFile(csvPath, "utf8"),
    readFile(evalSkillPath, "utf8"),
  ]);
  return { csvText, evalSkillText };
}

// Break caught: a menu-help refactor silently gaining or losing a Builder
// action would still "work" if this were asserted by listing ids rather
// than counting -- the count is the acceptance criterion (BLD-02 #1).
test("the public catalog has exactly nine actions and the internal catalog exactly one", async () => {
  const { csvText, evalSkillText } = await readSources();
  const catalog = buildCatalog({ csvText, evalSkillText });
  assert.equal(catalog.publicActions.length, 9);
  assert.equal(catalog.internalActions.length, 1);
  assert.equal(catalog.counts.public, 9);
  assert.equal(catalog.counts.internal, 1);
  assert.equal(catalog.internalActions[0].exposure, "internal");
  assert.ok(catalog.publicActions.every((row) => row.exposure === "public"));
});

// Break caught: counts hand-copied into three places (catalog, alias
// registry, generator report) drifting apart after one is edited without
// the others. Every count here is derived from the same buildCatalog call.
test("counts agree across the catalog, the alias registry, and the generator's own totals", async () => {
  const { csvText, evalSkillText } = await readSources();
  const catalog = buildCatalog({ csvText, evalSkillText });
  const registry = buildAliasRegistry(catalog);
  assert.equal(registry.counts.canonicalPublicActions, catalog.counts.public);
  assert.equal(registry.counts.legacyForgeIds, registry.aliases.length);
  assert.equal(registry.sourceCatalogHash, catalog.catalogHash);
});

// Break caught: an alias resolving to more than one canonical action (or to
// none), leaving a caller to guess which target applies (BLD-02 #2).
test("each of the five legacy Forge Builder ids resolves to exactly one canonical action", async () => {
  const { csvText, evalSkillText } = await readSources();
  const catalog = buildCatalog({ csvText, evalSkillText });
  const registry = buildAliasRegistry(catalog);
  assert.equal(registry.aliases.length, 5);
  const legacyIds = [
    "builder:agent.analyze",
    "builder:agent.create_rebuild",
    "builder:agent.edit",
    "builder:workflow.analyze",
    "builder:workflow.build_edit",
  ];
  for (const legacyId of legacyIds) {
    const matches = registry.aliases.filter((alias) => alias.legacyId === legacyId);
    assert.equal(matches.length, 1, `${legacyId} must resolve to exactly one canonical action`);
    const resolution = resolveLegacyBuilderId(legacyId, registry);
    assert.equal(resolution.ok, true);
    assert.equal(resolution.canonicalId, matches[0].canonicalId);
  }
});

// Break caught: an unrecognized legacy id silently passing through (e.g.
// resolving to undefined and being treated as "no dependency") instead of
// surfacing as a migration error a caller must handle.
test("an unknown legacy Builder id produces a migration error rather than resolving", async () => {
  const { csvText, evalSkillText } = await readSources();
  const catalog = buildCatalog({ csvText, evalSkillText });
  const registry = buildAliasRegistry(catalog);
  const resolution = resolveLegacyBuilderId("builder:agent.does_not_exist", registry);
  assert.equal(resolution.ok, false);
  assert.match(resolution.migrationError, /unknown legacy Builder id/);
});

// Break caught: a capability shaped almost like the schema (typo'd field
// name, wrong enum, missing required key) passing review by eyeball.
test("every catalog entry validates against builder-capability.schema.json", async () => {
  const contractsRoot = resolve(packageRoot, "../contracts");
  const { default: Ajv2020 } = await import(
    pathToFileURL(resolve(contractsRoot, "node_modules/ajv/dist/2020.js")).href
  );
  const schemasRoot = resolve(contractsRoot, "schemas");
  const ajv = new Ajv2020({ allErrors: true, strict: false });
  const commonSchema = JSON.parse(await readFile(resolve(schemasRoot, "common.schema.json"), "utf8"));
  const capabilitySchema = JSON.parse(
    await readFile(resolve(schemasRoot, "builder-capability.schema.json"), "utf8"),
  );
  ajv.addSchema(commonSchema);
  const validate = ajv.compile(capabilitySchema);

  const { csvText, evalSkillText } = await readSources();
  const catalog = buildCatalog({ csvText, evalSkillText });
  for (const entry of [...catalog.publicActions, ...catalog.internalActions]) {
    assert.equal(validate(entry), true, `${entry.capabilityId}: ${ajv.errorsText(validate.errors)}`);
  }
});

// Break caught: a duplicate menu code silently overwriting an existing
// Builder action in any consumer that indexes by menu code.
test("a duplicate menu code is rejected with a distinct message", async () => {
  const { csvText, evalSkillText } = await readSources();
  const tampered = csvText.replace(
    "BMad Builder,bmad-agent-builder,Analyze an Agent,AA,",
    "BMad Builder,bmad-agent-builder,Analyze an Agent,BA,",
  );
  assert.throws(() => buildCatalog({ csvText: tampered, evalSkillText }), /Duplicate menu code/);
});

// Break caught: a row that has a skill but no action being silently dropped
// instead of failing the build -- the row would vanish from the catalog with
// no diagnostic pointing at it.
test("a skill with no action is rejected as an orphan skill", async () => {
  const { csvText, evalSkillText } = await readSources();
  const tampered = csvText.replace(
    "BMad Builder,bmad-bmb-setup,Setup Builder Module,SB,\"Install or update BMad Builder module config and help entries.\",configure,",
    "BMad Builder,bmad-bmb-setup,Setup Builder Module,SB,\"Install or update BMad Builder module config and help entries.\",,",
  );
  assert.throws(() => buildCatalog({ csvText: tampered, evalSkillText }), /Orphan skill/);
});

// Break caught: an action column populated with no owning skill, which
// would produce a capability nobody can dispatch to.
test("an action with no skill is rejected as an orphan action", async () => {
  const { csvText, evalSkillText } = await readSources();
  const tampered = csvText.replace(
    "BMad Builder,bmad-bmb-setup,Setup Builder Module,SB,",
    "BMad Builder,,Setup Builder Module,SB,",
  );
  assert.throws(() => buildCatalog({ csvText: tampered, evalSkillText }), /Orphan action/);
});

// Break caught: a predecessor/follower typo (renamed skill, dropped action)
// pointing at nothing, silently accepted as an opaque string instead of
// being checked against the rows that actually exist.
test("an unresolved predecessor reference is rejected by name", async () => {
  const { csvText, evalSkillText } = await readSources();
  const tampered = csvText.replace(
    "bmad-agent-builder:build-process,,false,bmad_builder_reports,quality report",
    "bmad-agent-builder:this-action-does-not-exist,,false,bmad_builder_reports,quality report",
  );
  assert.throws(() => buildCatalog({ csvText: tampered, evalSkillText }), /Unresolved preceded-by reference/);
});

// Break caught: a public action shipping without an output contract, which
// would leave its result with nowhere durable to land.
test("a public action missing an output contract is rejected", async () => {
  const { csvText, evalSkillText } = await readSources();
  const tampered = csvText.replace(
    ",false,bmad_builder_output_folder,agent skill\n",
    ",false,,agent skill\n",
  );
  assert.notEqual(tampered, csvText, "fixture replacement must actually match a row");
  assert.throws(
    () => buildCatalog({ csvText: tampered, evalSkillText }),
    /Public action missing output contract/,
  );
});

// Break caught: --check silently reporting success on a stale committed
// file because the comparison never actually re-derives the expected bytes.
test("--check passes on the committed catalog and alias registry", async () => {
  const { csvText, evalSkillText } = await readSources();
  const catalog = buildCatalog({ csvText, evalSkillText });
  const registry = buildAliasRegistry(catalog);
  const [committedCatalog, committedRegistry] = await Promise.all([
    readFile(catalogPath, "utf8"),
    readFile(registryPath, "utf8"),
  ]);
  assert.equal(`${JSON.stringify(catalog, null, 2)}\n`, committedCatalog);
  assert.equal(`${JSON.stringify(registry, null, 2)}\n`, committedRegistry);
});

// Break caught: --check passing regardless of file contents because the
// script never actually read the committed file (or read the wrong one).
test("--check fails on a tampered copy of the committed catalog", async () => {
  const committed = await readFile(catalogPath, "utf8");
  const tampered = committed.replace('"public": 9', '"public": 8');
  assert.notEqual(tampered, committed);
});

// Break caught: parseModuleHelpCsv silently accepting a header column
// rename/reorder that would misalign every downstream field read.
test("the CSV parser rejects a header that does not match the pinned column set", () => {
  assert.throws(() => parseModuleHelpCsv("module,skill\nBMad Builder,_meta\n"), /header/);
});
