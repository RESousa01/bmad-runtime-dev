import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { pathToFileURL } from "node:url";

export const SYNTHETIC_BEARER_MARKER = "forge-foundry-fake-marker-v1";

const fixtureUrl = new URL(
  "./fixtures/foundry-fixtures.v1.json",
  import.meta.url,
);
const allowedEnvironments = new Set(["Development", "Test"]);
const allowedResponseHeaders = new Set([
  "retry-after",
  "x-ms-error-code",
]);
const deploymentPath =
  /^\/api\/projects\/[^/?#]+\/deployments$/u;
const responsesPath =
  /^\/api\/projects\/[^/?#]+\/openai\/v1\/responses$/u;

export class FakeFoundryClock {
  #epochMilliseconds;

  constructor(value = "2000-01-01T00:00:00.000Z") {
    const parsed = new Date(value).getTime();
    if (!Number.isFinite(parsed)) {
      throw new TypeError("fake_foundry_clock_invalid");
    }
    this.#epochMilliseconds = parsed;
  }

  now() {
    return new Date(this.#epochMilliseconds).toISOString();
  }

  advance(milliseconds) {
    if (!Number.isSafeInteger(milliseconds) || milliseconds < 0) {
      throw new TypeError("fake_foundry_clock_advance_invalid");
    }
    this.#epochMilliseconds += milliseconds;
  }
}

export async function loadFixtureCatalog() {
  const text = await readFile(fixtureUrl, "utf8");
  const catalog = JSON.parse(text);
  validateCatalog(catalog);
  return catalog;
}

export async function createFakeFoundryServer({
  environment,
  clock = new FakeFoundryClock(),
  catalog: suppliedCatalog,
  port = 0,
} = {}) {
  const productionProcess = [
    process.env.NODE_ENV,
    process.env.ASP_NET_CORE_ENVIRONMENT,
    process.env.ASPNETCORE_ENVIRONMENT,
    process.env.DOTNET_ENVIRONMENT,
    process.env.FORGE_ENVIRONMENT,
  ].some((value) => value?.toLowerCase() === "production");
  if (productionProcess || !allowedEnvironments.has(environment)) {
    throw new Error("fake_foundry_environment_forbidden");
  }
  if (!(clock instanceof FakeFoundryClock)) {
    throw new TypeError("fake_foundry_clock_required");
  }
  if (
    !Number.isSafeInteger(port)
    || port < 0
    || port > 65_535
  ) {
    throw new TypeError("fake_foundry_port_invalid");
  }

  const catalog = structuredClone(
    suppliedCatalog ?? await loadFixtureCatalog(),
  );
  validateCatalog(catalog);
  deepFreeze(catalog);
  const state = {
    catalog,
    clock,
    origin: undefined,
    queue: [],
    receipts: [],
    sockets: new Set(),
    stopped: false,
  };
  const server = createServer((request, response) => {
    handleRequest(state, request, response).catch(() => {
      if (!response.headersSent && !response.destroyed) {
        writeJsonError(response, 500, "fake_foundry_internal_error");
      } else if (!response.destroyed) {
        response.destroy();
      }
    });
  });
  server.on("connection", (socket) => {
    state.sockets.add(socket);
    socket.once("close", () => state.sockets.delete(socket));
  });
  await new Promise((resolve, reject) => {
    const onError = (error) => {
      server.off("listening", onListening);
      reject(error);
    };
    const onListening = () => {
      server.off("error", onError);
      resolve();
    };
    server.once("error", onError);
    server.once("listening", onListening);
    server.listen({ host: "127.0.0.1", port });
  });
  const address = server.address();
  if (address === null || typeof address === "string") {
    await closeServer(server, state);
    throw new Error("fake_foundry_address_unavailable");
  }
  state.origin = `http://127.0.0.1:${address.port}`;

  return Object.freeze({
    get origin() {
      return state.origin;
    },
    get pendingResponseCount() {
      return state.queue.length;
    },
    get requestReceipts() {
      return state.receipts.map((receipt) => ({
        ...receipt,
        headerNames: [...receipt.headerNames],
      }));
    },
    async scriptScenario(group, name) {
      const { route, scenarios } = resolveScenarioGroup(catalog, group);
      const scenario = scenarios.find((candidate) => candidate.name === name);
      if (scenario === undefined) {
        throw new Error("fake_foundry_scenario_unknown");
      }
      if (
        state.queue.length + scenario.responses.length
        > catalog.limits.maximumQueueDepth
      ) {
        throw new Error("fake_foundry_queue_limit_exceeded");
      }
      for (const response of scenario.responses) {
        state.queue.push({ route, response });
      }
    },
    async stop() {
      await closeServer(server, state);
    },
  });
}

function validateCatalog(catalog) {
  if (
    catalog === null
    || typeof catalog !== "object"
    || catalog.schemaVersion !== "forge.foundry-fake-fixtures.v1"
    || catalog.syntheticBearerMarker !== SYNTHETIC_BEARER_MARKER
  ) {
    throw new Error("fake_foundry_catalog_invalid");
  }
  const limits = catalog.limits;
  if (
    limits === null
    || typeof limits !== "object"
    || !Number.isSafeInteger(limits.maximumFixtureBodyBytes)
    || limits.maximumFixtureBodyBytes < 1
    || limits.maximumFixtureBodyBytes > 2 * 1024 * 1024
    || !Number.isSafeInteger(limits.maximumRequestBodyBytes)
    || limits.maximumRequestBodyBytes < 1
    || limits.maximumRequestBodyBytes > 1024 * 1024
    || !Number.isSafeInteger(limits.maximumQueueDepth)
    || limits.maximumQueueDepth < 1
    || limits.maximumQueueDepth > 64
  ) {
    throw new Error("fake_foundry_catalog_limits_invalid");
  }
  for (const group of [
    catalog.deploymentScenarios,
    catalog.responseScenarios,
  ]) {
    if (!Array.isArray(group) || group.length === 0) {
      throw new Error("fake_foundry_catalog_scenarios_invalid");
    }
    const names = new Set();
    for (const scenario of group) {
      if (
        scenario === null
        || typeof scenario !== "object"
        || typeof scenario.name !== "string"
        || !/^[a-z0-9_]{1,64}$/u.test(scenario.name)
        || names.has(scenario.name)
        || !Array.isArray(scenario.responses)
        || scenario.responses.length === 0
        || scenario.responses.length > limits.maximumQueueDepth
      ) {
        throw new Error("fake_foundry_catalog_scenario_invalid");
      }
      names.add(scenario.name);
      for (const response of scenario.responses) {
        validateFixtureResponse(response, limits.maximumFixtureBodyBytes);
      }
    }
  }
}

function deepFreeze(value) {
  if (
    value === null
    || typeof value !== "object"
    || Object.isFrozen(value)
  ) {
    return value;
  }
  for (const child of Object.values(value)) {
    deepFreeze(child);
  }
  return Object.freeze(value);
}

function validateFixtureResponse(response, maximumBytes) {
  if (
    response === null
    || typeof response !== "object"
    || !Number.isSafeInteger(response.status)
    || response.status < 100
    || response.status > 599
    || !["application/json", "text/event-stream"].includes(
      response.contentType,
    )
  ) {
    throw new Error("fake_foundry_fixture_response_invalid");
  }
  const bodyKinds = [
    Object.hasOwn(response, "jsonBody"),
    Object.hasOwn(response, "body"),
    Object.hasOwn(response, "repeatBody"),
    response.mode === "wait_for_cancellation",
  ].filter(Boolean);
  if (bodyKinds.length !== 1) {
    throw new Error("fake_foundry_fixture_body_invalid");
  }
  if (fixtureBodyByteLength(response) > maximumBytes) {
    throw new Error("fake_foundry_fixture_body_too_large");
  }
  if (response.headers !== undefined) {
    if (
      response.headers === null
      || Array.isArray(response.headers)
      || typeof response.headers !== "object"
    ) {
      throw new Error("fake_foundry_fixture_headers_invalid");
    }
    for (const [name, value] of Object.entries(response.headers)) {
      if (
        !allowedResponseHeaders.has(name)
        || typeof value !== "string"
        || value.length > 128
        || [...value].some((character) => {
          const codePoint = character.codePointAt(0);
          return codePoint <= 31 || codePoint === 127;
        })
      ) {
        throw new Error("fake_foundry_fixture_headers_invalid");
      }
    }
  }
}

function fixtureBodyByteLength(response) {
  if (Object.hasOwn(response, "jsonBody")) {
    return Buffer.byteLength(JSON.stringify(response.jsonBody), "utf8");
  }
  if (typeof response.body === "string") {
    return Buffer.byteLength(response.body, "utf8");
  }
  if (response.repeatBody !== undefined) {
    const { value, count } = response.repeatBody;
    if (
      typeof value !== "string"
      || value.length === 0
      || value.length > 16
      || !Number.isSafeInteger(count)
      || count < 0
    ) {
      throw new Error("fake_foundry_fixture_repeat_invalid");
    }
    const valueBytes = Buffer.byteLength(value, "utf8");
    if (count > Math.floor(Number.MAX_SAFE_INTEGER / valueBytes)) {
      return Number.POSITIVE_INFINITY;
    }
    return valueBytes * count;
  }
  return 0;
}

function renderFixtureBody(response, sourceOrigin, targetOrigin) {
  let body;
  if (Object.hasOwn(response, "jsonBody")) {
    body = JSON.stringify(response.jsonBody);
  } else if (typeof response.body === "string") {
    body = response.body;
  } else if (response.repeatBody !== undefined) {
    const { value, count } = response.repeatBody;
    body = value.repeat(count);
  } else {
    return undefined;
  }
  if (
    typeof sourceOrigin === "string"
    && typeof targetOrigin === "string"
  ) {
    body = body.replaceAll(sourceOrigin, targetOrigin);
  }
  return Buffer.from(body, "utf8");
}

async function handleRequest(state, request, response) {
  const evidence = await readRequestEvidence(
    request,
    state.catalog.limits.maximumRequestBodyBytes,
  );
  const route = classifyRoute(request);
  const requestTargetSha256 = createHash("sha256")
    .update(request.url ?? "", "utf8")
    .digest("hex");
  const receipt = Object.freeze({
    sequence: state.receipts.length + 1,
    receivedAt: state.clock.now(),
    method: request.method ?? "",
    route: route ?? "unrecognized",
    requestTargetSha256: `sha256:${requestTargetSha256}`,
    bodySha256: evidence.bodySha256,
    bodyTruncated: evidence.bodyTruncated,
    headerNames: Object.freeze([
      ...new Set(Object.keys(request.headers)),
    ].sort()),
  });
  state.receipts.push(receipt);

  if (evidence.bodyTruncated) {
    writeJsonError(response, 413, "fake_foundry_request_body_too_large");
    return;
  }
  if (!isAuthorized(request.headers.authorization)) {
    writeJsonError(response, 401, "fake_foundry_unauthorized");
    return;
  }
  if (route === undefined) {
    writeJsonError(response, 404, "fake_foundry_route_not_found");
    return;
  }
  const queued = state.queue[0];
  if (queued === undefined) {
    writeJsonError(response, 409, "fake_foundry_queue_empty");
    return;
  }
  if (queued.route !== route) {
    writeJsonError(response, 409, "fake_foundry_script_mismatch");
    return;
  }
  state.queue.shift();
  if (queued.response.mode === "wait_for_cancellation") {
    await waitForClientCancellation(request, response);
    return;
  }

  const body = renderFixtureBody(
    queued.response,
    state.catalog.baseOrigin,
    state.origin,
  );
  if (
    body === undefined
    || body.byteLength > state.catalog.limits.maximumFixtureBodyBytes
  ) {
    throw new Error("fake_foundry_rendered_body_invalid");
  }
  response.statusCode = queued.response.status;
  response.setHeader("content-type", queued.response.contentType);
  response.setHeader("content-length", body.byteLength);
  for (const [name, value] of Object.entries(
    queued.response.headers ?? {},
  )) {
    response.setHeader(name, value);
  }
  response.end(body);
}

async function readRequestEvidence(request, maximumBytes) {
  const hash = createHash("sha256");
  let observedBytes = 0;
  let bodyTruncated = false;
  for await (const rawChunk of request) {
    const chunk = Buffer.isBuffer(rawChunk)
      ? rawChunk
      : Buffer.from(rawChunk);
    const remaining = maximumBytes + 1 - observedBytes;
    if (remaining <= 0) {
      bodyTruncated = true;
      break;
    }
    const observed = chunk.subarray(0, remaining);
    hash.update(observed);
    observedBytes += observed.byteLength;
    if (
      observedBytes > maximumBytes
      || observed.byteLength < chunk.byteLength
    ) {
      bodyTruncated = true;
      break;
    }
  }
  return {
    bodySha256: `sha256:${hash.digest("hex")}`,
    bodyTruncated,
  };
}

function isAuthorized(value) {
  return value === `Bearer ${SYNTHETIC_BEARER_MARKER}`;
}

function classifyRoute(request) {
  const parsed = new URL(request.url ?? "/", "http://fake.invalid");
  if (
    request.method === "GET"
    && deploymentPath.test(parsed.pathname)
  ) {
    const allowed = new Set([
      "api-version",
      "continuationToken",
      "deploymentType",
    ]);
    if (
      [...parsed.searchParams.keys()].some((key) => !allowed.has(key))
      || parsed.searchParams.getAll("api-version").length !== 1
      || parsed.searchParams.get("api-version") !== "v1"
      || parsed.searchParams.getAll("deploymentType").length > 1
      || (
        parsed.searchParams.has("deploymentType")
        && parsed.searchParams.get("deploymentType") !== "ModelDeployment"
      )
      || parsed.searchParams.getAll("continuationToken").length > 1
    ) {
      return undefined;
    }
    return "deployment";
  }
  if (
    request.method === "POST"
    && responsesPath.test(parsed.pathname)
    && parsed.search.length === 0
  ) {
    return "responses";
  }
  return undefined;
}

function resolveScenarioGroup(catalog, group) {
  if (group === "deployment") {
    return {
      route: "deployment",
      scenarios: catalog.deploymentScenarios,
    };
  }
  if (group === "responses") {
    return {
      route: "responses",
      scenarios: catalog.responseScenarios,
    };
  }
  throw new Error("fake_foundry_scenario_group_invalid");
}

function writeJsonError(response, status, code) {
  const body = Buffer.from(JSON.stringify({ error: { code } }), "utf8");
  response.statusCode = status;
  response.setHeader("content-type", "application/json");
  response.setHeader("content-length", body.byteLength);
  response.end(body);
}

function waitForClientCancellation(request, response) {
  return new Promise((resolve) => {
    const finish = () => {
      request.off("aborted", finish);
      response.off("close", finish);
      resolve();
    };
    request.once("aborted", finish);
    response.once("close", finish);
  });
}

async function closeServer(server, state) {
  if (state.stopped) {
    return;
  }
  state.stopped = true;
  for (const socket of state.sockets) {
    socket.destroy();
  }
  await new Promise((resolve, reject) => {
    server.close((error) => {
      if (error === undefined) {
        resolve();
      } else {
        reject(error);
      }
    });
  });
}

function readCliArguments(arguments_) {
  const values = new Map();
  for (let index = 0; index < arguments_.length; index += 2) {
    const name = arguments_[index];
    const value = arguments_[index + 1];
    if (
      typeof name !== "string"
      || !name.startsWith("--")
      || value === undefined
    ) {
      throw new Error("fake_foundry_cli_arguments_invalid");
    }
    values.set(name.slice(2), value);
  }
  return values;
}

async function runCli() {
  const arguments_ = readCliArguments(process.argv.slice(2));
  const environment = arguments_.get("environment");
  const portValue = arguments_.get("port") ?? "0";
  if (!/^\d{1,5}$/u.test(portValue)) {
    throw new Error("fake_foundry_cli_port_invalid");
  }
  const fake = await createFakeFoundryServer({
    environment,
    port: Number(portValue),
  });
  const group = arguments_.get("scenario-group");
  const scenario = arguments_.get("scenario");
  if ((group === undefined) !== (scenario === undefined)) {
    await fake.stop();
    throw new Error("fake_foundry_cli_scenario_invalid");
  }
  if (group !== undefined) {
    await fake.scriptScenario(group, scenario);
  }
  process.stdout.write(`${JSON.stringify({
    schemaVersion: "forge.foundry-fake-started.v1",
    origin: fake.origin,
    pendingResponseCount: fake.pendingResponseCount,
  })}\n`);
  const stop = async () => {
    await fake.stop();
    process.exitCode = 0;
  };
  process.once("SIGINT", stop);
  process.once("SIGTERM", stop);
}

const invokedPath = process.argv[1] === undefined
  ? undefined
  : pathToFileURL(process.argv[1]).href;
if (invokedPath === import.meta.url) {
  runCli().catch((error) => {
    process.stderr.write(
      `${error instanceof Error ? error.message : "fake_foundry_cli_failed"}\n`,
    );
    process.exitCode = 1;
  });
}
