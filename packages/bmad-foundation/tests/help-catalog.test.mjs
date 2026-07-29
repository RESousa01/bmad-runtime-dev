import assert from "node:assert/strict";
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
