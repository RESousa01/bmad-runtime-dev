import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { canonicalHash } from "../scripts/lib/canonical-json.mjs";
import {
  loadSchemaRegistry,
  validateSchemaDocument,
} from "../scripts/lib/schema-validator.mjs";
import { parseStrictJson } from "../scripts/lib/strict-json.mjs";

const packageRoot = fileURLToPath(new URL("../", import.meta.url));
const repositoryRoot = path.resolve(packageRoot, "..", "..");
const fixtureRoot = path.join(packageRoot, "fixtures");
const registry = await loadSchemaRegistry(path.join(packageRoot, "schemas"));

async function readFixture(relativePath) {
  return parseStrictJson(await readFile(path.join(fixtureRoot, relativePath), "utf8"));
}

const compatibility = await readFixture("valid/model-access-compatibility.json");
const legacyRequest = await readFixture("valid/model-access-request.json");
const legacyReceipt = await readFixture("valid/model-access-receipt.json");
const currentRequest = await readFixture("valid/model-access-request-v2.json");
const currentReceipt = await readFixture("valid/model-access-receipt-v2.json");

const rustAdapter = await readFile(
  path.join(repositoryRoot, "crates", "desktop-cloud", "src", "model_access_compatibility.rs"),
  "utf8",
);
const dotnetAdapter = await readFile(
  path.join(
    repositoryRoot,
    "services",
    "desktop-support-api",
    "Model",
    "ModelAccessCompatibility.cs",
  ),
  "utf8",
);

/**
 * The canonical digest of the shipped version one receipt, computed with the
 * same `sapphirus:model-access-receipt:v1` preimage the support plane signs
 * with. Pinned so that if version two ever perturbed how a version one receipt
 * is hashed, every historical signature would break and this assertion would
 * break first. The same literal is pinned in the .NET replay test at
 * services/desktop-support-api.Tests/Model/ModelAccessCompatibilityTests.cs.
 */
const HISTORICAL_RECEIPT_DIGEST =
  "sha256:b30e1bebf1e57ea255397b185e90f8bb52cdabe2686ad9f168a13da7cba029b0";

function receiptSignedPayload(receipt) {
  const draft = { ...receipt };
  delete draft.receiptHash;
  delete draft.proof;
  return draft;
}

test("the compatibility policy is a one-way lift onto a single named legacy profile", () => {
  assert.deepEqual(
    validateSchemaDocument(registry, "model-access-compatibility.schema.json", compatibility),
    [],
  );
  assert.equal(compatibility.adapterDirection, "v1_to_v2_only");
  assert.equal(compatibility.legacyProfile.deploymentResolution, "server_side_only");
  assert.equal(compatibility.legacyProfile.clientSelectable, false);
  assert.equal(compatibility.legacyProfile.requiresCatalogVersion, false);
  assert.equal(compatibility.legacyProfile.variantPolicy, "profile_default_only");
  assert.equal(
    Object.hasOwn(compatibility.legacyProfile, "deploymentName"),
    false,
    "the legacy profile binding must not be able to name a deployment",
  );
});

test("both request versions are accepted and the removal gate is fully stated", () => {
  assert.deepEqual(compatibility.acceptedRequestSchemaVersions, [
    "desktop-model-access-request.v1",
    "desktop-model-access-request.v2",
  ]);
  assert.equal(compatibility.currentRequestSchemaVersion, "desktop-model-access-request.v2");
  assert.equal(compatibility.minimumDesktopContractEpoch, 1);
  assert.ok(
    compatibility.removalGate.removalContractEpoch > compatibility.minimumDesktopContractEpoch,
    "version one cannot be removed at the epoch that still has to speak it",
  );
  assert.deepEqual(compatibility.removalGate.conditions, [
    "catalog_published_for_every_tenant",
    "every_installation_at_removal_epoch",
    "legacy_profile_retired_from_catalog",
    "historical_receipt_replay_verified",
  ]);
});

test("the three implementations of the policy pin the same constants", () => {
  const shared = {
    legacyProfileId: compatibility.legacyProfile.profileId,
    legacyRequestSchemaVersion: "desktop-model-access-request.v1",
    currentRequestSchemaVersion: compatibility.currentRequestSchemaVersion,
    legacyReceiptSchemaVersion: "sapphirus.model-access-receipt.v1",
    currentReceiptSchemaVersion: "sapphirus.model-access-receipt.v2",
    legacyRequestMode: compatibility.legacyProfile.requestMode,
  };
  for (const value of Object.values(shared)) {
    assert.ok(
      rustAdapter.includes(`"${value}"`),
      `the Rust adapter must pin ${value}`,
    );
    assert.ok(
      dotnetAdapter.includes(`"${value}"`),
      `the support plane adapter must pin ${value}`,
    );
  }
  for (const reason of compatibility.rejectionReasons) {
    assert.ok(rustAdapter.includes(`"${reason}"`), `the Rust adapter must name ${reason}`);
    assert.ok(dotnetAdapter.includes(`"${reason}"`), `the support plane must name ${reason}`);
  }
  assert.ok(
    rustAdapter.includes(
      `MINIMUM_DESKTOP_CONTRACT_EPOCH: u32 = ${compatibility.minimumDesktopContractEpoch}`,
    ),
  );
  assert.ok(
    rustAdapter.includes(
      `REMOVAL_CONTRACT_EPOCH: u32 = ${compatibility.removalGate.removalContractEpoch}`,
    ),
  );
  assert.ok(
    dotnetAdapter.includes(
      `MinimumDesktopContractEpoch = ${compatibility.minimumDesktopContractEpoch}`,
    ),
  );
  assert.ok(
    dotnetAdapter.includes(
      `RemovalContractEpoch = ${compatibility.removalGate.removalContractEpoch}`,
    ),
  );
});

test("a version one request still validates after version two lands", () => {
  assert.deepEqual(
    validateSchemaDocument(registry, "model-access-request.schema.json", legacyRequest),
    [],
  );
  assert.equal(legacyRequest.schemaVersion, "desktop-model-access-request.v1");
  assert.deepEqual(
    validateSchemaDocument(registry, "model-access-request-v2.schema.json", currentRequest),
    [],
  );
  assert.equal(currentRequest.schemaVersion, "desktop-model-access-request.v2");
});

test("a version one request cannot carry a version two capability", () => {
  for (const [member, value] of [
    ["modelProfileId", "coding-balanced"],
    ["requestedVariantId", "deep"],
    ["catalogVersion", "v1.0123456789abcdef"],
    ["requestMode", "streaming_chat"],
    ["tools", []],
    ["attachments", []],
  ]) {
    const reaching = { ...legacyRequest, [member]: value };
    const errors = validateSchemaDocument(
      registry,
      "model-access-request.schema.json",
      reaching,
    );
    assert.ok(
      errors.some((error) => error.code === "UNKNOWN_PROPERTY"),
      `a version one request carrying ${member} must be rejected as an unknown property`,
    );
  }
});

test("the adapter cannot run backwards: a version two request is not a version one request", () => {
  const lowered = validateSchemaDocument(
    registry,
    "model-access-request.schema.json",
    currentRequest,
  );
  assert.ok(lowered.length > 0, "a version two request must not validate as version one");
  const raised = validateSchemaDocument(
    registry,
    "model-access-request-v2.schema.json",
    legacyRequest,
  );
  assert.ok(
    raised.length > 0,
    "a version one request must not validate as version two without adaptation",
  );
});

test("a receipt signed before version two still verifies against its historical digest", () => {
  assert.deepEqual(
    validateSchemaDocument(registry, "model-access-receipt.schema.json", legacyReceipt),
    [],
  );
  const replayed = canonicalHash({
    purpose: "model-access-receipt",
    schemaMajor: "v1",
    value: receiptSignedPayload(legacyReceipt),
  });
  assert.equal(replayed.serializedHash, HISTORICAL_RECEIPT_DIGEST);
  assert.equal(
    replayed.preimage.startsWith("sapphirus:model-access-receipt:v1\n"),
    true,
    "the version one receipt preimage must be unchanged by version two",
  );
  assert.deepEqual(
    compatibility.historicalReceiptPolicy.verifiableReceiptSchemaVersions,
    ["sapphirus.model-access-receipt.v1", "sapphirus.model-access-receipt.v2"],
  );
  assert.equal(compatibility.historicalReceiptPolicy.version1DigestUnchanged, true);
  assert.equal(compatibility.historicalReceiptPolicy.reverificationRequired, false);
});

test("the two receipt families are separate documents, not a restatement", () => {
  assert.deepEqual(
    validateSchemaDocument(registry, "model-access-receipt-v2.schema.json", currentReceipt),
    [],
  );
  assert.equal(currentReceipt.schemaVersion, "sapphirus.model-access-receipt.v2");
  assert.ok(
    validateSchemaDocument(registry, "model-access-receipt.schema.json", currentReceipt).length > 0,
    "a version two receipt must not pass as a version one receipt",
  );
  assert.ok(
    validateSchemaDocument(registry, "model-access-receipt-v2.schema.json", legacyReceipt).length
      > 0,
    "a version one receipt must not pass as a version two receipt",
  );
  assert.equal(
    Object.hasOwn(currentReceipt, "deploymentHash"),
    false,
    "the version two receipt replaces the bare deployment hash with a resolved pair",
  );
  assert.equal(typeof currentReceipt.resolvedDeploymentHash, "string");
  assert.equal(typeof currentReceipt.deploymentInventoryHash, "string");
});
