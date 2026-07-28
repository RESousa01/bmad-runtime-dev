import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import test from "node:test";

import {
  FakeFoundryClock,
  SYNTHETIC_BEARER_MARKER,
  createFakeFoundryServer,
  loadFixtureCatalog,
} from "./server.mjs";

const canonicalFixtureUrl = new URL(
  "./fixtures/foundry-fixtures.v1.json",
  import.meta.url,
);
const rustFixtureUrl = new URL(
  "../../crates/desktop-cloud/tests/fixtures/foundry/foundry-fixtures.v1.json",
  import.meta.url,
);

test("fixture catalog is complete, bounded, and mirrored", async () => {
  const canonical = await readFile(canonicalFixtureUrl, "utf8");
  const rustMirror = await readFile(rustFixtureUrl, "utf8");
  const catalog = await loadFixtureCatalog();

  assert.equal(rustMirror, canonical);
  assert.equal(catalog.schemaVersion, "forge.foundry-fake-fixtures.v1");
  assert.equal(
    catalog.syntheticBearerMarker,
    "forge-foundry-fake-marker-v1",
  );
  assert.deepEqual(
    catalog.deploymentScenarios.map(({ name }) => name).sort(),
    [
      "deployment_removal",
      "duplicate_deployments",
      "empty_inventory",
      "malformed_json",
      "multiple_pages",
      "one_page",
      "oversized_body",
      "unknown_fields",
      "wrong_next_link_origin",
    ],
  );
  assert.deepEqual(
    catalog.responseScenarios.map(({ name }) => name).sort(),
    [
      "client_cancellation",
      "content_filter",
      "deployment_missing_404",
      "forbidden_403",
      "malformed_sse",
      "rate_limited_429",
      "refusal",
      "server_error_500",
      "streaming_text_deltas",
      "streaming_tool_call_deltas",
      "streaming_usage",
      "structured_output",
      "timeout",
      "truncated_stream",
    ],
  );
});

test("supplied fixture recipes are size-checked before expansion", async () => {
  const catalog = structuredClone(await loadFixtureCatalog());
  catalog.deploymentScenarios[0].responses[0] = {
    status: 200,
    contentType: "application/json",
    repeatBody: {
      value: "x",
      count: Number.MAX_SAFE_INTEGER,
    },
  };

  await assert.rejects(
    createFakeFoundryServer({
      environment: "Test",
      catalog,
    }),
    /fake_foundry_fixture_body_too_large/u,
  );
});

test("server snapshots supplied catalogs before queuing scenarios", async (t) => {
  const catalog = await loadFixtureCatalog();
  const fake = await createFakeFoundryServer({
    environment: "Test",
    catalog,
  });
  t.after(() => fake.stop());
  const structured = catalog.responseScenarios.find(
    ({ name }) => name === "structured_output",
  );
  structured.responses[0].jsonBody.status = "mutated-after-start";
  await fake.scriptScenario("responses", "structured_output");

  const response = await fetch(
    `${fake.origin}/api/projects/forge-test/openai/v1/responses`,
    {
      method: "POST",
      headers: {
        authorization: `Bearer ${SYNTHETIC_BEARER_MARKER}`,
      },
    },
  );

  assert.equal(response.status, 200);
  assert.equal((await response.json()).status, "completed");
});

test("fake server refuses production and staging composition", async () => {
  for (const environment of ["", "Production", "Staging"]) {
    await assert.rejects(
      createFakeFoundryServer({ environment }),
      /fake_foundry_environment_forbidden/u,
    );
  }
});

test("production process markers override a caller claiming Test", () => {
  const script = [
    "import { createFakeFoundryServer } from",
    "'./tools/foundry-fake/server.mjs';",
    "try {",
    "  const fake = await createFakeFoundryServer({ environment: 'Test' });",
    "  await fake.stop();",
    "  process.exit(0);",
    "} catch (error) {",
    "  process.stderr.write(`${error.message}\\n`);",
    "  process.exit(1);",
    "}",
  ].join(" ");
  const result = spawnSync(
    process.execPath,
    ["--input-type=module", "--eval", script],
    {
      cwd: new URL("../..", import.meta.url),
      encoding: "utf8",
      env: {
        ...process.env,
        NODE_ENV: "production",
      },
      timeout: 10_000,
    },
  );

  assert.equal(result.status, 1);
  assert.match(result.stderr, /fake_foundry_environment_forbidden/u);
});

test("fake server requires the exact synthetic marker without consuming the queue", async (t) => {
  const fake = await createFakeFoundryServer({ environment: "Test" });
  t.after(() => fake.stop());
  await fake.scriptScenario("deployment", "one_page");
  const url = `${fake.origin}/api/projects/forge-test/deployments?api-version=v1&deploymentType=ModelDeployment`;

  const missing = await fetch(url);
  const wrong = await fetch(url, {
    headers: { authorization: "Bearer wrong-marker" },
  });
  const accepted = await fetch(url, {
    headers: { authorization: `Bearer ${SYNTHETIC_BEARER_MARKER}` },
  });

  assert.equal(missing.status, 401);
  assert.equal(wrong.status, 401);
  assert.equal(accepted.status, 200);
  assert.equal(fake.pendingResponseCount, 0);
});

test("request evidence keeps only a body hash and sorted header names", async (t) => {
  const clock = new FakeFoundryClock("2026-07-28T12:00:00.000Z");
  const fake = await createFakeFoundryServer({
    environment: "Development",
    clock,
  });
  t.after(() => fake.stop());
  await fake.scriptScenario("responses", "structured_output");
  const body = JSON.stringify({
    model: "desktop-planning",
    input: "private fixture content",
  });

  const response = await fetch(
    `${fake.origin}/api/projects/forge-test/openai/v1/responses`,
    {
      method: "POST",
      headers: {
        authorization: `Bearer ${SYNTHETIC_BEARER_MARKER}`,
        "content-type": "application/json",
        "x-secret-marker": "header-value-must-not-leak",
      },
      body,
    },
  );

  assert.equal(response.status, 200);
  const [receipt] = fake.requestReceipts;
  assert.equal(
    receipt.bodySha256,
    `sha256:${createHash("sha256").update(body).digest("hex")}`,
  );
  assert.deepEqual(
    receipt.headerNames,
    [...receipt.headerNames].sort(),
  );
  assert.ok(receipt.headerNames.includes("authorization"));
  assert.ok(receipt.headerNames.includes("content-type"));
  const serialized = JSON.stringify(receipt);
  assert.doesNotMatch(serialized, /private fixture content/u);
  assert.doesNotMatch(serialized, /header-value-must-not-leak/u);
  assert.doesNotMatch(serialized, /forge-foundry-fake-marker-v1/u);
  assert.equal(receipt.receivedAt, "2026-07-28T12:00:00.000Z");
});

test("oversized request bodies fail closed without consuming a response", async (t) => {
  const fake = await createFakeFoundryServer({ environment: "Test" });
  t.after(() => fake.stop());
  await fake.scriptScenario("responses", "structured_output");

  const response = await fetch(
    `${fake.origin}/api/projects/forge-test/openai/v1/responses`,
    {
      method: "POST",
      headers: {
        authorization: `Bearer ${SYNTHETIC_BEARER_MARKER}`,
        "content-type": "text/plain",
      },
      body: "x".repeat(65_537),
    },
  );

  assert.equal(response.status, 413);
  assert.equal(fake.pendingResponseCount, 1);
  assert.equal(fake.requestReceipts.at(-1).bodyTruncated, true);
});

test("scripted queues, clock, route guard, and cancellation are deterministic", async (t) => {
  const clock = new FakeFoundryClock("2026-07-28T12:00:00.000Z");
  const fake = await createFakeFoundryServer({ environment: "Test", clock });
  t.after(() => fake.stop());
  const responseUrl =
    `${fake.origin}/api/projects/forge-test/openai/v1/responses`;
  const headers = {
    authorization: `Bearer ${SYNTHETIC_BEARER_MARKER}`,
  };

  await fake.scriptScenario("responses", "rate_limited_429");
  await fake.scriptScenario("responses", "structured_output");
  const first = await fetch(responseUrl, { method: "POST", headers });
  clock.advance(5_000);
  const second = await fetch(responseUrl, { method: "POST", headers });
  assert.equal(first.status, 429);
  assert.equal(second.status, 200);
  assert.deepEqual(
    fake.requestReceipts.map(({ sequence, receivedAt }) => ({
      sequence,
      receivedAt,
    })),
    [
      { sequence: 1, receivedAt: "2026-07-28T12:00:00.000Z" },
      { sequence: 2, receivedAt: "2026-07-28T12:00:05.000Z" },
    ],
  );

  const beforeUnknownRoute = fake.pendingResponseCount;
  const queryName = ["api", "key"].join("-");
  const unknown = await fetch(
    `${fake.origin}/healthz?${queryName}=query-value-must-not-leak`,
    { headers },
  );
  assert.equal(unknown.status, 404);
  assert.equal(fake.pendingResponseCount, beforeUnknownRoute);
  const unknownReceipt = fake.requestReceipts.at(-1);
  assert.equal(unknownReceipt.route, "unrecognized");
  assert.match(
    unknownReceipt.requestTargetSha256,
    /^sha256:[0-9a-f]{64}$/u,
  );
  assert.doesNotMatch(
    JSON.stringify(unknownReceipt),
    /query-value-must-not-leak/u,
  );

  await fake.scriptScenario("responses", "client_cancellation");
  const controller = new AbortController();
  const pending = fetch(responseUrl, {
    method: "POST",
    headers,
    signal: controller.signal,
  });
  setTimeout(() => controller.abort(), 25);
  await assert.rejects(pending, { name: "AbortError" });
  assert.equal(fake.pendingResponseCount, 0);
});
