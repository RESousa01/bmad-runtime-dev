import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { parseStrictJson } from "./lib/strict-json.mjs";

const packageRoot = fileURLToPath(new URL("../", import.meta.url));
const files = {
  dotnet: "generated/dotnet/Contracts.g.cs",
  rust: "generated/rust/contracts.rs",
  typescript: "generated/typescript/contracts.ts",
};
const sources = Object.fromEntries(
  await Promise.all(
    Object.entries(files).map(async ([runtime, file]) => [
      runtime,
      await readFile(path.join(packageRoot, file), "utf8"),
    ]),
  ),
);
const typescriptSchemaDirectory = path.join(
  packageRoot,
  "generated",
  "typescript",
  "schema",
);
const generatedTypescriptSchemas = (
  await readdir(typescriptSchemaDirectory)
).filter((name) => name.endsWith(".ts"));
sources.typescript = [
  sources.typescript,
  ...(await Promise.all(
    generatedTypescriptSchemas.map((name) =>
      readFile(path.join(typescriptSchemaDirectory, name), "utf8"),
    ),
  )),
].join("\n");
const deferredRunnerVocabulary = [
  "WindowsContainmentClaim",
  "WindowsLocalHostAudience",
  "runnerProfile",
  "runner_profile",
  "RunnerProfile",
  "job_object_controlled",
  "childProcess",
  "child_process",
  "ChildProcess",
  "networkIntent",
  "network_intent",
  "NetworkIntent",
  "standard_user_job",
  "windows_local_host",
  "command_run",
  "raw_shell",
  "run_shell",
  "process_spawn",
];

for (const [runtime, source] of Object.entries(sources)) {
  assert.match(source, /generated/i, `${runtime} binding must carry a generated marker.`);
  for (const symbol of [
    "AuthorityRef",
    "CandidateAction",
    "ApprovedExecutionSpec",
    "SpecConsumptionRecord",
    "ExecutionResultManifest",
    "EvidenceEvent",
  ]) {
    assert.ok(source.includes(symbol), `${runtime} binding is missing ${symbol}.`);
  }
  assert.ok(!source.includes("CommandCandidate"), `${runtime} must not expose D4 commands.`);
  assert.ok(
    source.includes("NativePatchEngineAudience"),
    `${runtime} binding must expose the D3 native patch-engine audience.`,
  );
  for (const forbidden of deferredRunnerVocabulary) {
    assert.ok(
      !source.includes(forbidden),
      `${runtime} binding must not expose deferred runner vocabulary: ${forbidden}.`,
    );
  }
}

for (const symbol of [
  "ContractError",
  "FilesystemCapabilitySnapshot",
  "PackageCompatibility",
  "RemoteJobHandoff",
]) {
  assert.ok(
    sources.typescript.includes(symbol),
    `TypeScript binding is missing schema-first family ${symbol}.`,
  );
}

const schemaNames = (await readdir(path.join(packageRoot, "schemas")))
  .filter((name) => name.endsWith(".schema.json"));
for (const schemaName of schemaNames) {
  const source = await readFile(path.join(packageRoot, "schemas", schemaName), "utf8");
  for (const forbidden of deferredRunnerVocabulary) {
    assert.ok(
      !source.includes(forbidden),
      `${schemaName} must not expose deferred runner vocabulary: ${forbidden}.`,
    );
  }
}

assert.ok(!/\bany\b/.test(sources.typescript), "Generated TypeScript must not use any.");
assert.ok(sources.rust.includes("deny_unknown_fields"));
assert.ok(sources.dotnet.includes("JsonPolymorphic"));

const lock = parseStrictJson(
  await readFile(path.join(packageRoot, "schema-lock.json"), "utf8"),
);
for (const record of lock.generatedTree.files) {
  const source = await readFile(path.join(packageRoot, record.file), "utf8");
  const actual = `sha256:${createHash("sha256").update(source, "utf8").digest("hex")}`;
  assert.equal(actual, record.sha256, `${record.file} differs from schema-lock.json.`);
}

function toSnakeCase(value) {
  return value.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`);
}

function toPascalCase(value) {
  return `${value[0].toUpperCase()}${value.slice(1)}`;
}

for (const schemaName of [
  "candidate-action.schema.json",
  "approved-execution-spec.schema.json",
  "spec-consumption.schema.json",
  "execution-result-manifest.schema.json",
  "evidence-event.schema.json",
]) {
  const schema = parseStrictJson(
    await readFile(path.join(packageRoot, "schemas", schemaName), "utf8"),
  );
  for (const propertyName of schema.required) {
    assert.ok(
      sources.typescript.includes(`${propertyName}:`),
      `TypeScript binding omits required ${schemaName} property ${propertyName}.`,
    );
    assert.ok(
      sources.rust.includes(`pub ${toSnakeCase(propertyName)}:`),
      `Rust binding omits required ${schemaName} property ${propertyName}.`,
    );
    assert.ok(
      sources.dotnet.includes(toPascalCase(propertyName)),
      `C# binding omits required ${schemaName} property ${propertyName}.`,
    );
  }
}

console.log(
  "contracts: baseline cross-language and extended TypeScript binding checks passed",
);
