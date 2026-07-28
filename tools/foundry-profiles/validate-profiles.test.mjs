import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import process from "node:process";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { canonicalize } from "../../packages/contracts/scripts/lib/canonical-json.mjs";
import { parseStrictJson } from "../../packages/contracts/scripts/lib/strict-json.mjs";
import {
  loadSchemaRegistry,
  resolveReference,
} from "../../packages/contracts/scripts/lib/schema-validator.mjs";
import { assessDeploymentNameSafety } from "./validate-profiles.mjs";

const toolDirectory = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(toolDirectory, "..", "..");
const VALIDATOR = path.join(toolDirectory, "validate-profiles.mjs");
const EXAMPLE = path.join(
  repositoryRoot,
  "infra",
  "desktop-support",
  "model-profiles.example.json",
);
const READY_DEPLOYMENTS = path.join(toolDirectory, "fixtures", "ready-deployments.json");
const BLOCKED_DEPLOYMENTS = path.join(
  repositoryRoot,
  "artifacts",
  "foundry",
  "prerequisites.json",
);

function readJson(filePath) {
  return JSON.parse(readFileSync(filePath, "utf8"));
}

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

/**
 * Runs the validator exactly as an operator would, so exit codes and the
 * printed report are covered rather than only the exported functions.
 */
function runValidator({ profiles, deployments = READY_DEPLOYMENTS, current = null }) {
  const directory = mkdtempSync(path.join(tmpdir(), "forge-profiles-"));
  try {
    let profilesPath = profiles;
    if (typeof profiles !== "string") {
      profilesPath = path.join(directory, "profiles.json");
      writeFileSync(profilesPath, JSON.stringify(profiles, null, 2), "utf8");
    }
    let currentPath = null;
    if (current !== null) {
      currentPath = path.join(directory, "current.json");
      writeFileSync(currentPath, JSON.stringify(current, null, 2), "utf8");
    }

    const argv = [
      VALIDATOR,
      "--profiles",
      profilesPath,
      "--deployments",
      deployments,
      "--quiet",
    ];
    if (currentPath !== null) {
      argv.push("--current", currentPath);
    }
    const result = spawnSync(process.execPath, argv, { encoding: "utf8" });
    let report = null;
    if (result.stdout.trim().length > 0) {
      try {
        report = JSON.parse(result.stdout);
      } catch {
        report = null;
      }
    }
    return { status: result.status, stderr: result.stderr, report };
  } finally {
    rmSync(directory, { force: true, recursive: true });
  }
}

function violationCodes(report) {
  return report.violations.map((entry) => entry.code);
}

function exampleDocument() {
  return readJson(EXAMPLE);
}

function profileByIdOf(document, profileId) {
  const profile = document.profiles.find((entry) => entry.profileId === profileId);
  assert.ok(profile !== undefined, `fixture is missing profile ${profileId}`);
  return profile;
}

// --- The mirrored schema tracks the canonical contract --------------------

// $defs is dropped from the comparison because every reference into it has
// already been inlined; resolution still reads the registry's original
// documents, so pruning the copy cannot hide a difference.
const STRIPPED_SCHEMA_KEYS = new Set([
  "$schema",
  "$id",
  "$defs",
  "title",
  "description",
]);

function inlineReferences(registry, documentName, node, depth = 0) {
  assert.ok(depth < 32, "schema nesting exceeded the comparison depth bound");
  if (Array.isArray(node)) {
    return node.map((item) => inlineReferences(registry, documentName, item, depth + 1));
  }
  if (node === null || typeof node !== "object") return node;

  if (typeof node.$ref === "string") {
    assert.equal(
      Object.keys(node).length,
      1,
      "a $ref with sibling keywords cannot be compared structurally",
    );
    const resolved = resolveReference(registry, node.$ref, documentName);
    return inlineReferences(registry, resolved.documentName, resolved.schema, depth + 1);
  }

  const result = {};
  for (const [key, value] of Object.entries(node)) {
    if (STRIPPED_SCHEMA_KEYS.has(key)) continue;
    result[key] = inlineReferences(registry, documentName, value, depth + 1);
  }
  return result;
}

test("the operator document schema mirrors the canonical model profile contract", async () => {
  const canonicalRegistry = await loadSchemaRegistry(
    path.join(repositoryRoot, "packages", "contracts", "schemas"),
  );
  const toolRegistry = await loadSchemaRegistry(toolDirectory);

  const canonicalProfile = inlineReferences(
    canonicalRegistry,
    "model-profile.schema.json",
    canonicalRegistry.documents.get("model-profile.schema.json"),
  );
  const mirroredProfile = inlineReferences(
    toolRegistry,
    "profile.schema.json",
    toolRegistry.documents.get("profile.schema.json").$defs.ModelProfile,
  );

  assert.equal(
    canonicalize(mirroredProfile),
    canonicalize(canonicalProfile),
    "tools/foundry-profiles/profile.schema.json has drifted from the canonical contract",
  );
});

test("the document schema is closed at every object", async () => {
  const registry = await loadSchemaRegistry(toolDirectory);
  const document = registry.documents.get("profile.schema.json");
  const open = [];
  const walk = (node, pointer) => {
    if (Array.isArray(node)) {
      node.forEach((item, index) => walk(item, `${pointer}/${index}`));
      return;
    }
    if (node === null || typeof node !== "object") return;
    if (Object.hasOwn(node, "properties") && node.additionalProperties !== false) {
      open.push(pointer);
    }
    for (const [key, value] of Object.entries(node)) {
      walk(value, `${pointer}/${key}`);
    }
  };
  walk(document, "");
  assert.deepEqual(open, []);
});

// --- The example and the real, unready inventory --------------------------

test("the shipped example validates against the synthetic ready inventory", () => {
  const { status, report } = runValidator({ profiles: EXAMPLE });
  assert.equal(status, 0, JSON.stringify(report?.violations ?? [], null, 2));
  assert.equal(report.valid, true);
  assert.equal(report.profiles.length, 4);
  assert.equal(report.registryVersion, 1);
  assert.match(report.registryHash, /^sha256:[0-9a-f]{64}$/u);
  assert.match(report.capabilityHash, /^sha256:[0-9a-f]{64}$/u);
  assert.deepEqual(report.warnings, []);
});

test("a not-ready deployment inventory is refused, and says why", () => {
  const { status, report } = runValidator({
    profiles: EXAMPLE,
    deployments: BLOCKED_DEPLOYMENTS,
  });
  assert.equal(status, 2);
  assert.deepEqual(violationCodes(report), ["deployment_inventory_not_ready"]);
  assert.equal(report.deploymentInventory.ready, false);
  assert.deepEqual(report.violations[0].inventoryBlockers, [
    "azure_cli_authentication_missing",
    "container_app_resource_id_missing",
    "foundry_role_scope_missing",
    "intended_deployments_missing",
    "managed_identity_client_id_missing",
    "project_endpoint_missing",
  ]);
});

test("an unrecognized deployments file is refused rather than assumed empty", () => {
  const directory = mkdtempSync(path.join(tmpdir(), "forge-inventory-"));
  try {
    const inventoryPath = path.join(directory, "inventory.json");
    writeFileSync(inventoryPath, JSON.stringify({ deployments: [] }), "utf8");
    const { status, report } = runValidator({
      profiles: EXAMPLE,
      deployments: inventoryPath,
    });
    assert.equal(status, 2);
    assert.ok(
      violationCodes(report).includes("deployment_inventory_shape_unrecognized"),
    );
  } finally {
    rmSync(directory, { force: true, recursive: true });
  }
});

// --- Deployment cross-checks ----------------------------------------------

test("a deployment that is not in the inventory is rejected", () => {
  const document = exampleDocument();
  profileByIdOf(document, "fast-response").deploymentName = "example-absent-do-not-deploy";
  const { status, report } = runValidator({ profiles: document });
  assert.equal(status, 2);
  assert.ok(violationCodes(report).includes("deployment_unknown"));
});

test("a deployment name that differs only by case is rejected as a case mismatch", () => {
  const document = exampleDocument();
  profileByIdOf(document, "fast-response").deploymentName = "Example-Fast-Do-Not-Deploy";
  const { status, report } = runValidator({ profiles: document });
  assert.equal(status, 2);
  assert.ok(violationCodes(report).includes("deployment_name_case_mismatch"));
});

test("a deployment serving a different model identity is rejected", () => {
  const document = exampleDocument();
  profileByIdOf(document, "fast-response").expectedModel.version = "0000-00-99";
  const { status, report } = runValidator({ profiles: document });
  assert.equal(status, 2);
  const mismatch = report.violations.find(
    (entry) => entry.code === "model_identity_mismatch",
  );
  assert.ok(mismatch !== undefined);
  assert.match(mismatch.message, /version/u);
});

test("two profiles on one deployment must expect the same model", () => {
  const document = exampleDocument();
  profileByIdOf(document, "deep-reasoning").expectedModel.name = "example-model-other";
  const { status, report } = runValidator({ profiles: document });
  assert.equal(status, 2);
  assert.ok(violationCodes(report).includes("deployment_model_identity_conflict"));
});

test("unsafe deployment names never reach a request path", () => {
  assert.equal(assessDeploymentNameSafety("example-fast-do-not-deploy"), null);
  for (const unsafe of [
    "",
    "a",
    "-leading-hyphen",
    "trailing-hyphen-",
    "has space",
    "has/slash",
    "has?query",
    "has:colon",
    "..",
    "café-deployment",
    "name\u0000",
    "name ",
    "a".repeat(129),
  ]) {
    assert.equal(
      assessDeploymentNameSafety(unsafe),
      "deployment_name_unsafe",
      `expected ${JSON.stringify(unsafe)} to be refused`,
    );
  }
});

test("a document carrying an unsafe deployment name is refused", () => {
  const document = exampleDocument();
  profileByIdOf(document, "fast-response").deploymentName = "has space";
  const { status, report } = runValidator({ profiles: document });
  assert.equal(status, 2);
  // The canonical DeploymentName pattern is the first gate, so the refusal is
  // attributed to the schema; the safety assessment above is defence in depth.
  assert.ok(violationCodes(report).includes("document_schema_invalid"));
});

test("a canonical deployment inventory enforces capability narrowing", () => {
  const directory = mkdtempSync(path.join(tmpdir(), "forge-canonical-inv-"));
  try {
    const inventoryPath = path.join(directory, "inventory.json");
    writeFileSync(
      inventoryPath,
      JSON.stringify({
        schemaVersion: "sapphirus.foundry-deployment.v1",
        snapshotHash: `sha256:${"1".repeat(64)}`,
        retrievedAt: "2026-07-28T00:00:00.000Z",
        deployments: [
          {
            deploymentName: "example-fast-do-not-deploy",
            deploymentType: "ModelDeployment",
            modelPublisher: "ExamplePublisher",
            modelName: "example-model-fast",
            modelVersion: "0000-00-01",
            sku: { name: "ExampleStandard", tier: null, family: null, size: null, capacity: 1 },
            capabilities: {
              textInput: true,
              imageInput: false,
              pdfInput: false,
              attachments: false,
              toolCalling: false,
              structuredOutput: true,
              reasoning: false,
              streaming: true,
            },
            deploymentHash: `sha256:${"2".repeat(64)}`,
          },
        ],
      }),
      "utf8",
    );

    const document = exampleDocument();
    document.profiles = [profileByIdOf(document, "fast-response")];
    const { status, report } = runValidator({
      profiles: document,
      deployments: inventoryPath,
    });
    assert.equal(status, 2);
    assert.equal(report.deploymentInventory.capabilitiesKnown, true);
    const narrowed = report.violations.find(
      (entry) => entry.code === "capability_not_advertised",
    );
    assert.ok(narrowed !== undefined);
    assert.equal(narrowed.path, "profiles/0/requiredCapabilities/toolCalling");
  } finally {
    rmSync(directory, { force: true, recursive: true });
  }
});

// --- Document-wide uniqueness ---------------------------------------------

test("a duplicate profile identifier is rejected", () => {
  const document = exampleDocument();
  document.profiles.push(clone(profileByIdOf(document, "fast-response")));
  const { status, report } = runValidator({ profiles: document });
  assert.equal(status, 2);
  assert.ok(violationCodes(report).includes("profile_id_duplicate"));
});

test("two visible profiles may not share a display name", () => {
  const document = exampleDocument();
  profileByIdOf(document, "deep-reasoning").displayName = "  balanced ";
  const { status, report } = runValidator({ profiles: document });
  assert.equal(status, 2);
  assert.ok(violationCodes(report).includes("display_name_duplicate"));
});

test("a hidden evaluation profile may reuse a visible display name", () => {
  const document = exampleDocument();
  const hidden = profileByIdOf(document, "evaluation-harness");
  assert.equal(hidden.visible, false);
  hidden.displayName = "Balanced";

  const allowed = runValidator({ profiles: document });
  assert.equal(allowed.status, 0, JSON.stringify(allowed.report?.violations ?? []));
  const published = allowed.report.profiles.find(
    (entry) => entry.profileId === "evaluation-harness",
  );
  assert.equal(published.visible, false);
  assert.equal(published.enabled, true);
  assert.equal(published.key, "desktop-support:model-profile:evaluation-harness");

  // The same name becomes a violation the moment the profile becomes visible.
  hidden.visible = true;
  const refused = runValidator({ profiles: document });
  assert.equal(refused.status, 2);
  assert.ok(violationCodes(refused.report).includes("display_name_duplicate"));
});

test("a duplicate variant identifier inside one profile is rejected", () => {
  const document = exampleDocument();
  const profile = profileByIdOf(document, "fast-response");
  profile.variants[1].variantId = profile.variants[0].variantId;
  const { status, report } = runValidator({ profiles: document });
  assert.equal(status, 2);
  assert.ok(violationCodes(report).includes("variant_id_duplicate"));
});

// --- Limits, retention, qualification -------------------------------------

test("a non-positive limit is rejected", () => {
  const document = exampleDocument();
  profileByIdOf(document, "fast-response").limits.maxOutputTokens = 0;
  const { status, report } = runValidator({ profiles: document });
  assert.equal(status, 2);
  const codes = violationCodes(report);
  assert.ok(codes.includes("document_schema_invalid"));

  // Defence in depth: the semantic check fires on its own when the schema
  // bound is satisfied but the value is still not a usable budget.
  const negative = exampleDocument();
  profileByIdOf(negative, "fast-response").limits.maxAttachmentBytes = -1;
  const negativeRun = runValidator({ profiles: negative });
  assert.equal(negativeRun.status, 2);
  assert.ok(violationCodes(negativeRun.report).includes("document_schema_invalid"));
});

test("an input budget larger than the context window is rejected", () => {
  const document = exampleDocument();
  const profile = profileByIdOf(document, "fast-response");
  profile.limits.maxInputTokens = profile.limits.contextTokens + 1;
  const { status, report } = runValidator({ profiles: document });
  assert.equal(status, 2);
  assert.ok(violationCodes(report).includes("limit_exceeds_context"));
});

test("a variant may narrow the output budget but never widen it", () => {
  const document = exampleDocument();
  const profile = profileByIdOf(document, "fast-response");
  profile.variants[0].maxOutputTokens = profile.limits.maxOutputTokens + 1;
  const { status, report } = runValidator({ profiles: document });
  assert.equal(status, 2);
  assert.ok(violationCodes(report).includes("variant_output_exceeds_profile_limit"));
});

test("an unsupported retention mode is rejected", () => {
  const document = exampleDocument();
  profileByIdOf(document, "fast-response").retentionMode = "retain_for_training";
  const { status, report } = runValidator({ profiles: document });
  assert.equal(status, 2);
  assert.ok(violationCodes(report).includes("document_schema_invalid"));
});

test("a profile without qualification evidence is rejected", () => {
  const placeholder = exampleDocument();
  profileByIdOf(placeholder, "fast-response").qualification.evidenceHash =
    `sha256:${"0".repeat(64)}`;
  const placeholderRun = runValidator({ profiles: placeholder });
  assert.equal(placeholderRun.status, 2);
  assert.ok(
    violationCodes(placeholderRun.report).includes("qualification_evidence_placeholder"),
  );

  const unqualified = exampleDocument();
  profileByIdOf(unqualified, "fast-response").qualification.status = "expired";
  const unqualifiedRun = runValidator({ profiles: unqualified });
  assert.equal(unqualifiedRun.status, 2);
  assert.ok(
    violationCodes(unqualifiedRun.report).includes("qualification_not_qualified"),
  );

  const undated = exampleDocument();
  profileByIdOf(undated, "fast-response").qualification.qualifiedAt = null;
  const undatedRun = runValidator({ profiles: undated });
  assert.equal(undatedRun.status, 2);
  assert.ok(
    violationCodes(undatedRun.report).includes("qualification_timestamp_missing"),
  );
});

test("a request mode without its capability or output schema is rejected", () => {
  const noSchema = exampleDocument();
  profileByIdOf(noSchema, "fast-response").canonicalOutputSchemaAllowlist = [];
  const noSchemaRun = runValidator({ profiles: noSchema });
  assert.equal(noSchemaRun.status, 2);
  assert.ok(
    violationCodes(noSchemaRun.report).includes("structured_mode_without_output_schema"),
  );

  const noCapability = exampleDocument();
  profileByIdOf(noCapability, "fast-response").requiredCapabilities.streaming = false;
  const noCapabilityRun = runValidator({ profiles: noCapability });
  assert.equal(noCapabilityRun.status, 2);
  assert.ok(
    violationCodes(noCapabilityRun.report).includes("capability_contradicts_request_mode"),
  );
});

test("a modality without its input capability is rejected", () => {
  const document = exampleDocument();
  profileByIdOf(document, "deep-reasoning").requiredCapabilities.pdfInput = false;
  const { status, report } = runValidator({ profiles: document });
  assert.equal(status, 2);
  assert.ok(violationCodes(report).includes("modality_contradicts_capability"));
});

// --- Strict parsing --------------------------------------------------------

test("a duplicate JSON member is refused before any validation", () => {
  const directory = mkdtempSync(path.join(tmpdir(), "forge-strict-"));
  try {
    const profilesPath = path.join(directory, "profiles.json");
    writeFileSync(
      profilesPath,
      '{"schemaVersion":"forge.model-profile-document.v1",' +
        '"registryVersion":1,"registryVersion":2,"profiles":[]}',
      "utf8",
    );
    const result = spawnSync(
      process.execPath,
      [VALIDATOR, "--profiles", profilesPath, "--deployments", READY_DEPLOYMENTS, "--quiet"],
      { encoding: "utf8" },
    );
    assert.equal(result.status, 1);
    assert.match(result.stderr, /not strict JSON/u);
    assert.match(result.stderr, /Duplicate JSON member/u);
  } finally {
    rmSync(directory, { force: true, recursive: true });
  }
});

test("missing arguments fail with a usage error, not a partial run", () => {
  const result = spawnSync(process.execPath, [VALIDATOR, "--profiles", EXAMPLE], {
    encoding: "utf8",
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /--deployments is required/u);
});

// --- Current state, diff, and downgrade ------------------------------------

function currentStateFrom(report, { registryVersion, mutate = () => {} }) {
  const profiles = report.profiles.map((entry) => ({
    profileId: entry.profileId,
    valueHash: entry.valueHash,
  }));
  const state = {
    schemaVersion: "forge.model-profile-store-state.v1",
    label: "production",
    registryVersion,
    profiles,
  };
  mutate(state);
  return state;
}

test("the diff against the current store state is redacted", () => {
  const baseline = runValidator({ profiles: EXAMPLE });
  assert.equal(baseline.status, 0);

  const current = currentStateFrom(baseline.report, {
    registryVersion: 1,
    mutate: (state) => {
      // One profile changed, one is published but no longer authored.
      state.profiles[0].valueHash = `sha256:${"9".repeat(64)}`;
      state.profiles.push({
        profileId: "retired-profile",
        valueHash: `sha256:${"8".repeat(64)}`,
      });
    },
  });

  const document = exampleDocument();
  document.registryVersion = 2;
  const { status, report } = runValidator({ profiles: document, current });
  assert.equal(status, 0, JSON.stringify(report.violations));

  assert.deepEqual(
    report.diff.changed.map((entry) => entry.profileId),
    ["balanced-default"],
  );
  assert.deepEqual(
    report.diff.removed.map((entry) => entry.profileId),
    ["retired-profile"],
  );
  assert.equal(report.diff.added.length, 0);
  assert.equal(report.diff.unchanged.length, 3);
  assert.ok(
    report.warnings.some((entry) => entry.code === "profile_removed"),
    "a published profile that disappeared must be surfaced",
  );

  const serializedDiff = JSON.stringify(report.diff);
  for (const secret of [
    "example-fast-do-not-deploy",
    "example-shared-do-not-deploy",
    "ExamplePublisher",
    "example-model-reasoning",
    "exampleregion",
  ]) {
    assert.ok(
      !serializedDiff.includes(secret),
      `the diff leaked ${secret}; it must carry identifiers and digests only`,
    );
  }
  for (const entry of [...report.diff.changed, ...report.diff.removed]) {
    for (const [key, value] of Object.entries(entry)) {
      if (key === "profileId") continue;
      assert.match(value, /^sha256:[0-9a-f]{64}$/u);
    }
  }
});

test("a registry version that does not advance is rejected", () => {
  const baseline = runValidator({ profiles: EXAMPLE });
  const current = currentStateFrom(baseline.report, { registryVersion: 5 });

  const equal = exampleDocument();
  equal.registryVersion = 5;
  const equalRun = runValidator({ profiles: equal, current });
  assert.equal(equalRun.status, 2);
  assert.ok(violationCodes(equalRun.report).includes("registry_version_not_increasing"));

  const lower = exampleDocument();
  lower.registryVersion = 4;
  const lowerRun = runValidator({ profiles: lower, current });
  assert.equal(lowerRun.status, 2);
  assert.ok(violationCodes(lowerRun.report).includes("registry_version_not_increasing"));

  const higher = exampleDocument();
  higher.registryVersion = 6;
  const higherRun = runValidator({ profiles: higher, current });
  assert.equal(higherRun.status, 0, JSON.stringify(higherRun.report.violations));
});

// --- Hash behaviour --------------------------------------------------------

test("hashes are stable under key reordering and sensitive to content", () => {
  const baseline = runValidator({ profiles: EXAMPLE });
  assert.equal(baseline.status, 0);

  const reordered = exampleDocument();
  reordered.profiles = reordered.profiles.map((profile) =>
    Object.fromEntries(Object.entries(profile).reverse()),
  );
  const reorderedRun = runValidator({ profiles: reordered });
  assert.equal(reorderedRun.status, 0);
  assert.equal(reorderedRun.report.registryHash, baseline.report.registryHash);
  assert.equal(reorderedRun.report.capabilityHash, baseline.report.capabilityHash);
  assert.deepEqual(
    reorderedRun.report.profiles.map((entry) => entry.valueHash),
    baseline.report.profiles.map((entry) => entry.valueHash),
  );

  // A price change moves the registry hash but not the capability surface.
  const repriced = exampleDocument();
  profileByIdOf(repriced, "fast-response").priceProfile.inputMicrounitsPerThousand = 101;
  const repricedRun = runValidator({ profiles: repriced });
  assert.equal(repricedRun.status, 0);
  assert.notEqual(repricedRun.report.registryHash, baseline.report.registryHash);
  assert.equal(repricedRun.report.capabilityHash, baseline.report.capabilityHash);

  // Widening what the organization permits moves both.
  const widened = exampleDocument();
  profileByIdOf(widened, "fast-response").purposeAllowlist.push("planning");
  const widenedRun = runValidator({ profiles: widened });
  assert.equal(widenedRun.status, 0);
  assert.notEqual(widenedRun.report.registryHash, baseline.report.registryHash);
  assert.notEqual(widenedRun.report.capabilityHash, baseline.report.capabilityHash);

  // The registry version is part of the registry hash.
  const rolled = exampleDocument();
  rolled.registryVersion = 2;
  const rolledRun = runValidator({ profiles: rolled });
  assert.equal(rolledRun.status, 0);
  assert.notEqual(rolledRun.report.registryHash, baseline.report.registryHash);
  assert.equal(rolledRun.report.capabilityHash, baseline.report.capabilityHash);
});

test("the emitted plan carries canonical values whose digests match the report", () => {
  const directory = mkdtempSync(path.join(tmpdir(), "forge-plan-"));
  try {
    const planPath = path.join(directory, "plan.json");
    const result = spawnSync(
      process.execPath,
      [
        VALIDATOR,
        "--profiles",
        EXAMPLE,
        "--deployments",
        READY_DEPLOYMENTS,
        "--emit-plan",
        planPath,
        "--quiet",
      ],
      { encoding: "utf8" },
    );
    assert.equal(result.status, 0);
    const report = JSON.parse(result.stdout);
    const plan = readJson(planPath);

    assert.equal(plan.registryHash, report.registryHash);
    assert.equal(plan.capabilityHash, report.capabilityHash);
    assert.equal(plan.keyPrefix, "desktop-support:model-profile:");
    assert.equal(
      plan.registryVersionKey,
      "desktop-support:model-profile-registry:version",
    );
    assert.equal(plan.profiles.length, report.profiles.length);

    for (const [index, entry] of plan.profiles.entries()) {
      assert.equal(entry.valueHash, report.profiles[index].valueHash);
      // The stored value is the canonical form, which is what lets the
      // PowerShell publisher re-derive the digest without a canonicalizer.
      const parsed = parseStrictJson(entry.value);
      assert.equal(canonicalize(parsed), entry.value);
      assert.equal(parsed.profileId, entry.profileId);
    }

    const registryRecord = parseStrictJson(plan.registryValue);
    assert.equal(canonicalize(registryRecord), plan.registryValue);
    assert.equal(registryRecord.schemaVersion, "forge.model-profile-registry.v1");
    assert.deepEqual(
      registryRecord.profileIds,
      plan.profiles.map((entry) => entry.profileId),
    );
  } finally {
    rmSync(directory, { force: true, recursive: true });
  }
});

test("no plan is emitted for a document that is refused", () => {
  const directory = mkdtempSync(path.join(tmpdir(), "forge-plan-refused-"));
  try {
    const planPath = path.join(directory, "plan.json");
    const result = spawnSync(
      process.execPath,
      [
        VALIDATOR,
        "--profiles",
        EXAMPLE,
        "--deployments",
        BLOCKED_DEPLOYMENTS,
        "--emit-plan",
        planPath,
        "--quiet",
      ],
      { encoding: "utf8" },
    );
    assert.equal(result.status, 2);
    assert.throws(() => readFileSync(planPath, "utf8"));
    assert.equal(JSON.parse(result.stdout).valid, false);
  } finally {
    rmSync(directory, { force: true, recursive: true });
  }
});

// --- The example itself ----------------------------------------------------

test("the shipped example names nothing that could be mistaken for real", () => {
  const source = readFileSync(EXAMPLE, "utf8");
  const document = exampleDocument();

  for (const profile of document.profiles) {
    assert.match(
      profile.deploymentName,
      /do-not-deploy$/u,
      "example deployment names must be obviously synthetic",
    );
    assert.match(profile.expectedModel.publisher, /^Example/u);
    assert.match(profile.expectedModel.name, /^example-/u);
    assert.equal(profile.priceProfile.currency, "XTS");
    assert.equal(profile.retentionMode, "transient_no_store");
  }

  assert.ok(!source.includes("azure.com"), "no endpoint may appear in the example");
  assert.ok(!source.includes("azconfig.io"), "no endpoint may appear in the example");
  assert.ok(
    !/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/iu.test(source),
    "no subscription, tenant, or client identifier may appear in the example",
  );

  const shared = document.profiles.filter(
    (profile) => profile.deploymentName === "example-shared-do-not-deploy",
  );
  assert.equal(shared.length, 3, "the example must show profiles sharing one deployment");
  const budgets = new Set(
    shared.flatMap((profile) =>
      profile.variants.map(
        (variant) => `${variant.reasoningEffort}:${variant.maxOutputTokens}`,
      ),
    ),
  );
  assert.ok(
    budgets.size >= 4,
    "shared-deployment profiles must differ in reasoning effort and output budget",
  );
});
