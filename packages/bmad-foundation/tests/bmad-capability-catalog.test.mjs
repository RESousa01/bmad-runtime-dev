import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import test from "node:test";

const root = new URL("../", import.meta.url);

async function readJson(path) {
  return JSON.parse(await readFile(new URL(path, root), "utf8"));
}

test("canonical catalog accounts for every pinned Method and Builder row", async () => {
  const [catalog, accounting] = await Promise.all([
    readJson("capability-catalog.json"),
    readJson("source-accounting.json"),
  ]);

  const methodRows = catalog.capabilities.filter((row) => row.source.id === "method");
  const builderRows = catalog.capabilities.filter((row) => row.source.id === "builder");

  assert.equal(methodRows.length, 48);
  assert.equal(builderRows.length, 10);
  assert.equal(methodRows.length, accounting.totals.method_primary_skills);
  assert.equal(builderRows.filter((row) => row.visibility === "public").length, 9);
  assert.equal(builderRows.filter((row) => row.visibility === "internal").length, 1);
});

test("aliases are unambiguous, governed migrations to catalog implementations", async () => {
  const [catalog, aliasCatalog] = await Promise.all([
    readJson("capability-catalog.json"),
    readJson("compatibility-aliases.json"),
  ]);
  const canonicalIds = new Set(catalog.capabilities.map((row) => row.canonicalId));
  const aliases = new Set();

  for (const alias of aliasCatalog.aliases) {
    assert.ok(alias.alias);
    assert.ok(alias.target);
    assert.ok(alias.migrationPolicy);
    assert.ok(!aliases.has(alias.alias), `duplicate alias ${alias.alias}`);
    assert.ok(canonicalIds.has(alias.target), `unknown target ${alias.target}`);
    aliases.add(alias.alias);
  }
});

test("generated Rust and TypeScript projections are current", async () => {
  const catalog = await readJson("capability-catalog.json");
  const [rust, typescript] = await Promise.all([
    readFile(new URL("../../crates/desktop-app/src/generated/bmad_capabilities.rs", root), "utf8"),
    readFile(new URL("../../apps/desktop-ui/src/generated/bmadCapabilities.ts", root), "utf8"),
  ]);

  assert.match(rust, new RegExp(`CAPABILITY_ROW_COUNT: usize = ${catalog.capabilities.length}`));
  assert.match(typescript, new RegExp(`capabilityRowCount = ${catalog.capabilities.length}`));
  assert.match(typescript, /builderPublicActionRowCount = 9/);
  assert.match(typescript, /builderInternalActionRowCount = 1/);
});

// Break caught: a canonical row can be qualified in source accounting and the
// compatibility ledger while presentation code still advertises every public
// row, including pending legacy PRD shims.
test("generated UI projections advertise only the qualified canonical PRD row", async () => {
  const catalog = await readJson("capability-catalog.json");
  const [rust, typescript] = await Promise.all([
    readFile(new URL("../../crates/desktop-app/src/generated/bmad_capabilities.rs", root), "utf8"),
    readFile(new URL("../../apps/desktop-ui/src/generated/bmadCapabilities.ts", root), "utf8"),
  ]);
  const prdRows = catalog.capabilities.filter((row) => row.canonicalId === "bmm:bmad-prd");
  const qualified = prdRows.filter(
    (row) =>
      row.visibility === "public"
      && row.uiState === "runtime_available"
      && row.supportState !== "unavailable",
  );

  assert.equal(prdRows.length, 4);
  assert.deepEqual(
    qualified.map((row) => row.rowId),
    ["method:src/bmm-skills/2-plan-workflows/bmad-prd/SKILL.md"],
  );
  assert.match(
    rust,
    /pub const UI_ADVERTISED_CAPABILITIES[\s\S]*row_id: "method:src\/bmm-skills\/2-plan-workflows\/bmad-prd\/SKILL\.md"/,
  );
  assert.doesNotMatch(
    rust.split("pub const UI_ADVERTISED_CAPABILITIES", 2)[1],
    /v6-shims\/bmad-(create|edit|validate)-prd/,
  );
  assert.match(typescript, /row\.uiState === "runtime_available"/);
  assert.match(typescript, /row\.supportState !== "unavailable"/);
});

const readinessRowId =
  "method:src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/SKILL.md";
const readinessCanonicalId = "bmm:bmad-check-implementation-readiness";
const readinessResources = [
  "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/customize.toml",
  "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/steps/step-01-document-discovery.md",
  "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/steps/step-02-prd-analysis.md",
  "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/steps/step-03-epic-coverage-validation.md",
  "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/steps/step-04-ux-alignment.md",
  "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/steps/step-05-epic-quality-review.md",
  "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/steps/step-06-final-assessment.md",
  "src/bmm-skills/3-solutioning/bmad-check-implementation-readiness/templates/readiness-report-template.md",
];

test("readiness normalized envelope closes the exact pinned micro-file package", async () => {
  const envelope = await readJson(
    "normalized/bmad-check-implementation-readiness.package.json",
  );
  assert.equal(envelope.capability.skillName, "bmad-check-implementation-readiness");
  assert.equal(envelope.instructionProjection.sourceEntrypoint.treatment, "adopt");
  assert.deepEqual(
    envelope.instructionProjection.sourceResources.map(({ path }) => path),
    readinessResources,
  );
  for (const resource of envelope.instructionProjection.sourceResources) {
    const bytes = await readFile(new URL(`source/method/6.10.0/${resource.path}`, root));
    const hash = createHash("sha256").update(bytes).digest("hex");
    assert.equal(resource.contentHash, `sha256:${hash}`);
    assert.equal(resource.treatment, "adopt");
  }
});

test("readiness catalog Help UI Rust and runtime manifest derive one runtime state", async () => {
  const [catalog, ledger, help, manifest, typescript, rust, normalizedBytes] =
    await Promise.all([
      readJson("capability-catalog.json"),
      readJson("compatibility-ledger.json"),
      readJson("normalized/bmad-help-capability-graph.json"),
      readJson("runtime-manifest.json"),
      readFile(new URL("../../apps/desktop-ui/src/generated/bmadCapabilities.ts", root), "utf8"),
      readFile(
        new URL("../../crates/desktop-app/src/generated/bmad_capabilities.rs", root),
        "utf8",
      ),
      readFile(
        new URL("normalized/bmad-check-implementation-readiness.package.json", root),
      ),
    ]);
  const catalogRow = catalog.capabilities.find((row) => row.rowId === readinessRowId);
  const ledgerRow = ledger.rows.find((row) => row.rowId === readinessRowId);
  const helpNode = help.nodes.find((node) => node.id === readinessCanonicalId);
  const manifestResource = manifest.resources.find(
    (resource) =>
      resource.path === "normalized/bmad-check-implementation-readiness.package.json",
  );
  const normalizedHash = createHash("sha256").update(normalizedBytes).digest("hex");

  assert.equal(ledgerRow.decision, "adopt");
  assert.equal(ledgerRow.supportState, "exact");
  assert.equal(catalogRow.supportState, "exact");
  assert.equal(catalogRow.uiState, "runtime_available");
  assert.equal(helpNode.supportState, "exact");
  assert.equal(helpNode.uiState, "runtime_available");
  assert.match(typescript, /bmm:bmad-check-implementation-readiness/);
  assert.match(rust, /bmm:bmad-check-implementation-readiness/);
  assert.equal(manifestResource.contentHash, `sha256:${normalizedHash}`);
});

test("readiness source-derived goldens prove parity and durable lifecycle branches", async () => {
  const golden = await readJson(
    "../../artifacts/bmad100/method/bmad-check-implementation-readiness/golden/source-derived-cases.json",
  );
  assert.deepEqual(golden.declaredModes, ["interactive", "headless"]);
  assert.deepEqual(golden.requiredOutputs.interactive, golden.requiredOutputs.headless);
  assert.deepEqual(golden.workflow.steps, [
    "document_discovery",
    "prd_analysis",
    "epic_coverage_validation",
    "ux_alignment",
    "epic_quality_review",
    "final_assessment",
  ]);
  assert.deepEqual(golden.lifecycleCases, [
    "cancel",
    "fail",
    "retry",
    "repair",
    "restart",
    "resume",
    "upgrade",
  ]);
});

test("generated UI projections advertise one canonical Deep Recon row and retain governed aliases", async () => {
  const catalog = await readJson("capability-catalog.json");
  const aliases = await readJson("compatibility-aliases.json");
  const [rust, typescript] = await Promise.all([
    readFile(new URL("../../crates/desktop-app/src/generated/bmad_capabilities.rs", root), "utf8"),
    readFile(new URL("../../apps/desktop-ui/src/generated/bmadCapabilities.ts", root), "utf8"),
  ]);
  const rows = catalog.capabilities.filter(
    (row) => row.canonicalId === "core:bmad-deep-recon",
  );
  const qualified = rows.filter(
    (row) =>
      row.visibility === "public"
      && row.uiState === "runtime_available"
      && row.supportState !== "unavailable",
  );

  assert.equal(rows.length, 4);
  assert.deepEqual(
    qualified.map((row) => row.rowId),
    ["method:src/core-skills/bmad-deep-recon/SKILL.md"],
  );
  assert.deepEqual(
    aliases.aliases
      .filter((alias) => alias.target === "core:bmad-deep-recon")
      .map((alias) => alias.alias),
    [
      "bmm:bmad-domain-research",
      "bmm:bmad-market-research",
      "bmm:bmad-technical-research",
    ],
  );
  assert.match(rust, /core:bmad-deep-recon/u);
  assert.match(typescript, /core:bmad-deep-recon/u);
});
