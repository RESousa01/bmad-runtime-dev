import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import test from "node:test";

const root = new URL("../", import.meta.url);

// This module deliberately does NOT read normalized/bmad-help-action-graph.json:
// that file is a sealed production fixture (its `sources`/`graphHash` shape is
// consumed directly by BmadCatalogBuilder::build_bound in
// crates/desktop-runtime/tests/{bmad_catalog,bmad_help_run,bmad_help_materialization,
// bmad_method_session}.rs, and its graphHash is a canonical_hash pinned to the
// exact `sources` bytes). Overwriting it with a different schema would break
// those Rust integration tests. The durable node/edge graph this test file
// covers instead lives in normalized/bmad-help-capability-graph.json.

async function readJson(path) {
  return JSON.parse(await readFile(new URL(path, root), "utf8"));
}

const HASH_PATTERN = /^sha256:[0-9a-f]{64}$/;

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function stableJson(value) {
  return `${JSON.stringify(value, null, 2)}\n`;
}

test("BMAD Help capability graph is an exact generated catalog-and-alias projection", async () => {
  const [catalogText, aliasCatalogText, graph] = await Promise.all([
    readFile(new URL("capability-catalog.json", root), "utf8"),
    readFile(new URL("compatibility-aliases.json", root), "utf8"),
    readJson("normalized/bmad-help-capability-graph.json"),
  ]);
  const catalog = JSON.parse(catalogText);
  const aliases = JSON.parse(aliasCatalogText);
  const groupedRows = new Map();
  for (const row of catalog.capabilities) {
    const rows = groupedRows.get(row.canonicalId) ?? [];
    rows.push(row);
    groupedRows.set(row.canonicalId, rows);
  }
  const nodes = [...groupedRows.entries()]
    .map(([id, rows]) => {
      const sourceIds = [...new Set(rows.map((row) => row.source.id))];
      const runtimeRows = rows.filter((row) => !row.source.path.includes("/v6-shims/"));
      assert.equal(sourceIds.length, 1, `${id} has one derived source`);
      assert.equal(runtimeRows.length, 1, `${id} has one canonical non-shim runtime row`);
      const canonical = runtimeRows[0];
      const phase = canonical.source.id === "builder" || canonical.source.path.includes("/core-skills/")
        ? "anytime"
        : canonical.source.path.includes("/1-analysis/")
          ? "analysis"
          : canonical.source.path.includes("/2-plan-workflows/")
            ? "planning"
            : canonical.source.path.includes("/3-solutioning/")
              ? "solutioning"
              : "implementation";
      const body = {
        id,
        sourceId: sourceIds[0],
        phase,
        predecessorIds: [...new Set(rows.flatMap((row) => row.predecessorIds ?? []))].sort(),
        followerIds: [...new Set(rows.flatMap((row) => row.followerIds ?? []))].sort(),
        outputLocations: [...new Set(rows.flatMap((row) => row.outputs))].sort(),
        supportState: canonical.supportState,
        uiState: canonical.uiState,
        rowIds: rows.map((row) => row.rowId).sort(),
      };
      return { ...body, nodeHash: `sha256:${sha256(stableJson(body))}` };
    })
    .sort((left, right) => left.id.localeCompare(right.id));
  const aliasEdges = aliases.aliases
    .map((alias) => ({
      aliasId: alias.alias,
      canonicalId: alias.target,
      deprecated: alias.deprecatedSince !== undefined,
    }))
    .sort((left, right) => left.aliasId.localeCompare(right.aliasId));
  const body = {
    schemaVersion: "sapphirus.bmad-help-capability-graph.v1",
    generatedBy: "packages/bmad-foundation/scripts/generate-catalog.mjs",
    sourceCatalogSha256: sha256(catalogText),
    sourceAliasCatalogSha256: sha256(aliasCatalogText),
    nodeCount: nodes.length,
    nodes,
    aliasEdges,
  };

  // Break caught: a plausible-looking hand-authored graph no longer tracks the
  // canonical catalog, aliases, rows, or support states.
  assert.deepEqual(graph, {
    ...body,
    graphHash: `sha256:${sha256(stableJson(body))}`,
  });
});

test("BMAD Help capability graph has one node per canonical capability in the catalog", async () => {
  const [catalog, graph] = await Promise.all([
    readJson("capability-catalog.json"),
    readJson("normalized/bmad-help-capability-graph.json"),
  ]);

  const canonicalIds = new Set(catalog.capabilities.map((row) => row.canonicalId));
  assert.equal(graph.nodeCount, canonicalIds.size);
  assert.equal(graph.nodes.length, canonicalIds.size);

  // Break caught: a regeneration that drops or duplicates a capability while
  // leaving nodeCount untouched. Every canonical id from the catalog must be
  // present in the graph, and the graph must name nothing the catalog does not.
  const graphIds = graph.nodes.map((node) => node.id);
  assert.deepEqual([...graphIds].sort(), [...canonicalIds].sort());
});

// Break caught: a current canonical PRD row can still be hidden behind a
// stale Help node, while its pending v6 compatibility rows are accidentally
// promoted with it. The node describes the canonical implementation; alias
// edges retain the deprecated shim identities separately.
test("BMAD Help exposes canonical PRD exact runtime availability without promoting aliases", async () => {
  const graph = await readJson("normalized/bmad-help-capability-graph.json");
  const prd = graph.nodes.find((node) => node.id === "bmm:bmad-prd");

  assert.equal(prd?.supportState, "exact");
  assert.equal(prd?.uiState, "runtime_available");
  assert.deepEqual(prd?.rowIds, [
    "method:src/bmm-skills/2-plan-workflows/bmad-prd/SKILL.md",
    "method:src/bmm-skills/v6-shims/bmad-create-prd/SKILL.md",
    "method:src/bmm-skills/v6-shims/bmad-edit-prd/SKILL.md",
    "method:src/bmm-skills/v6-shims/bmad-validate-prd/SKILL.md",
  ]);
  assert.deepEqual(
    graph.aliasEdges
      .filter((edge) => edge.canonicalId === "bmm:bmad-prd")
      .map((edge) => [edge.aliasId, edge.deprecated]),
    [
      ["bmm:bmad-create-prd", true],
      ["bmm:bmad-edit-prd", true],
      ["bmm:bmad-validate-prd", true],
    ],
  );
});

test("BMAD Help and UI retain the source-derived UX actions followers and outputs", async () => {
  const [catalog, graph, typescript] = await Promise.all([
    readJson("capability-catalog.json"),
    readJson("normalized/bmad-help-capability-graph.json"),
    readFile(new URL("../../apps/desktop-ui/src/generated/bmadCapabilities.ts", root), "utf8"),
  ]);
  const row = catalog.capabilities.find(
    (candidate) =>
      candidate.rowId ===
      "method:src/bmm-skills/2-plan-workflows/bmad-ux/SKILL.md",
  );
  const node = graph.nodes.find((candidate) => candidate.id === "bmm:bmad-ux");

  assert.deepEqual(row?.declaredActions, ["create", "update", "validate"]);
  assert.deepEqual(node?.followerIds, [
    "bmm:bmad-architecture",
    "bmm:bmad-create-epics-and-stories",
    "bmm:bmad-dev-story",
  ]);
  assert.deepEqual(node?.outputLocations, [
    "planning_artifacts/DESIGN.md",
    "planning_artifacts/EXPERIENCE.md",
    "planning_artifacts/validation-report.{md,html}",
  ]);
  assert.match(typescript, /"declaredActions": \[\s*"create",\s*"update",\s*"validate"/);
});

test("BMAD Help action graph nodes are unique and canonically ordered", async () => {
  const graph = await readJson("normalized/bmad-help-capability-graph.json");
  const ids = graph.nodes.map((node) => node.id);
  assert.equal(new Set(ids).size, ids.length, "duplicate node id in the action graph");
  const sorted = [...ids].sort();
  assert.deepEqual(ids, sorted, "nodes must be stored in canonical (sorted) order for a stable diff");
});

test("BMAD Help action graph has no orphan predecessor/follower references", async () => {
  const graph = await readJson("normalized/bmad-help-capability-graph.json");
  const nodeIds = new Set(graph.nodes.map((node) => node.id));
  for (const node of graph.nodes) {
    for (const predecessorId of node.predecessorIds) {
      assert.ok(nodeIds.has(predecessorId), `unknown predecessor ${predecessorId} on ${node.id}`);
    }
    for (const followerId of node.followerIds) {
      assert.ok(nodeIds.has(followerId), `unknown follower ${followerId} on ${node.id}`);
    }
  }
});

test("BMAD Help action graph alias edges resolve to a real node and carry well-formed hashes", async () => {
  const graph = await readJson("normalized/bmad-help-capability-graph.json");
  const nodeIds = new Set(graph.nodes.map((node) => node.id));
  const seenAliases = new Set();

  for (const edge of graph.aliasEdges) {
    assert.ok(nodeIds.has(edge.canonicalId), `alias ${edge.aliasId} targets unknown node ${edge.canonicalId}`);
    assert.ok(!seenAliases.has(edge.aliasId), `duplicate alias id ${edge.aliasId}`);
    seenAliases.add(edge.aliasId);
    assert.equal(typeof edge.deprecated, "boolean");
  }

  for (const node of graph.nodes) {
    assert.match(node.nodeHash, HASH_PATTERN, `malformed nodeHash on ${node.id}`);
  }
  assert.match(graph.graphHash, HASH_PATTERN, "malformed top-level graphHash");
  assert.match(graph.sourceCatalogSha256, /^[0-9a-f]{64}$/);
});

test("BMAD Help action graph alias edges are canonically ordered", async () => {
  const graph = await readJson("normalized/bmad-help-capability-graph.json");
  const ids = graph.aliasEdges.map((edge) => edge.aliasId);
  assert.deepEqual(ids, [...ids].sort());
});
