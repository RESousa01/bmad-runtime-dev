import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { parseStrictJson } from "./lib/strict-json.mjs";

const packageRoot = fileURLToPath(new URL("../", import.meta.url));
const generatedRoot = path.join(packageRoot, "generated", "typescript");
const schemaTargets = [
  "approved-execution-spec",
  "authority-ref",
  "bmad-builder-authoring",
  "bmad-capability-catalog",
  "bmad-method-session",
  "bmad-package-descriptor",
  "bmad-validation-report",
  "candidate-action",
  "contract-error",
  "durable-object",
  "evidence-event",
  "execution-result-manifest",
  "filesystem-capability",
  "package-compatibility",
  "remote-job-handoff",
  "spec-consumption",
];
const expectedFiles = [
  "canonical-json.mjs",
  "contracts.ts",
  "runtime.mjs",
  ...schemaTargets.map((target) => `schema/${target}.ts`),
  "semantic-validation.d.mts",
  "semantic-validation.mjs",
  "sha256.mjs",
  "strict-json.mjs",
  "unicode.mjs",
  "validation.d.mts",
  "validation.mjs",
  "validators.d.mts",
  "validators.mjs",
].sort();

async function listRegularFiles(directory, prefix = "") {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const relativePath = prefix === "" ? entry.name : `${prefix}/${entry.name}`;
    const absolutePath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await listRegularFiles(absolutePath, relativePath)));
    } else {
      assert.ok(entry.isFile(), `Generated TypeScript entry is not a regular file: ${relativePath}`);
      files.push(relativePath);
    }
  }
  return files.sort();
}

const actualFiles = await listRegularFiles(generatedRoot);
assert.deepEqual(
  actualFiles,
  expectedFiles,
  "Generated TypeScript tree does not match the reviewed file inventory.",
);

const sources = new Map();
const digestRecords = [];
for (const relativePath of actualFiles) {
  const source = await readFile(path.join(generatedRoot, relativePath), "utf8");
  assert.match(source, /generated/i, `${relativePath} must carry a generated marker.`);
  sources.set(relativePath, source);
  digestRecords.push({
    file: `generated/typescript/${relativePath}`,
    sha256: `sha256:${createHash("sha256").update(source, "utf8").digest("hex")}`,
  });
}

const typeSources = actualFiles
  .filter((relativePath) => relativePath.endsWith(".ts") || relativePath.endsWith(".d.mts"))
  .map((relativePath) => sources.get(relativePath))
  .join("\n");
for (const symbol of [
  "AuthorityRef",
  "BmadPackageDescriptor",
  "BmadCapabilityCatalog",
  "MethodSession",
  "BuilderAuthoringObject",
  "BmadValidationReport",
  "CandidateAction",
  "ApprovedExecutionSpec",
  "SpecConsumptionRecord",
  "ExecutionResultManifest",
  "EvidenceEvent",
  "ContractError",
  "FilesystemCapabilitySnapshot",
  "PackageCompatibility",
  "RemoteJobHandoff",
]) {
  assert.ok(typeSources.includes(symbol), `TypeScript bindings are missing ${symbol}.`);
}

assert.ok(!/\bany\b/.test(typeSources), "Generated TypeScript declarations must not use any.");
assert.ok(
  typeSources.includes("NativePatchEngineAudience"),
  "TypeScript bindings must expose the governed D3 patch-engine audience.",
);

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
const deferredBmadVocabulary = [
  "SkillPackageCandidate",
  "SkillPackageVersion",
  "PackageRegistration",
  "PackagePublication",
  "InstallRehearsalRun",
  "InvocationRehearsalRun",
  "EvaluationRun",
  "PackagePromotionRequest",
  "PackageActivation",
  "PackageRollback",
  "BuilderModule",
  "BuilderRegistration",
  "BuilderRehearsal",
  "BuilderEvaluation",
  "BuilderPublication",
  "BuilderPromotion",
  "BuilderActivation",
  "BuilderRollback",
  "BuilderMemoryAgent",
  "BuilderAutonomousAgent",
  "RegisterBuilder",
  "RehearseBuilder",
  "EvaluateBuilder",
  "PublishBuilder",
  "PromoteBuilder",
  "ActivateBuilder",
  "RollbackBuilder",
];
const allSources = [...sources.values()].join("\n");
for (const forbidden of deferredRunnerVocabulary) {
  assert.ok(
    !allSources.includes(forbidden),
    `Published TypeScript must not expose deferred runner vocabulary: ${forbidden}.`,
  );
}
for (const forbidden of deferredBmadVocabulary) {
  assert.ok(
    !allSources.includes(forbidden),
    `Published TypeScript must not expose deferred BMAD vocabulary: ${forbidden}.`,
  );
}

for (const target of [
  "candidate-action",
  "approved-execution-spec",
  "spec-consumption",
  "execution-result-manifest",
  "evidence-event",
]) {
  const schema = parseStrictJson(
    await readFile(path.join(packageRoot, "schemas", `${target}.schema.json`), "utf8"),
  );
  const generated = sources.get(`schema/${target}.ts`);
  assert.ok(generated !== undefined, `Generated TypeScript schema is missing ${target}.`);
  for (const propertyName of schema.required) {
    assert.ok(
      generated.includes(`${propertyName}:`),
      `TypeScript binding omits required ${target} property ${propertyName}.`,
    );
  }
}

const treeDigest = `sha256:${createHash("sha256")
  .update(
    digestRecords
      .map((record) => `${record.file}\0${record.sha256}\n`)
      .join(""),
    "utf8",
  )
  .digest("hex")}`;
console.log(
  `contracts: TypeScript binding checks passed (${digestRecords.length} files, ${treeDigest})`,
);
