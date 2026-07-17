import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  loadSchemaRegistry,
  validateSchemaDocument,
} from "../scripts/lib/schema-validator.mjs";
import { CONTRACT_VALIDATORS } from "../generated/typescript/validation.mjs";

const packageRoot = fileURLToPath(new URL("../", import.meta.url));
const registry = await loadSchemaRegistry(path.join(packageRoot, "schemas"));
const hash = (digit) => `sha256:${digit.repeat(64)}`;

const validConsent = {
  schemaVersion: "sapphirus.model-context-consent.v1",
  decisionId: "decision_01J00000000000000000000000",
  requestId: "request_01J00000000000000000000000",
  invocationId: "invoke_01J00000000000000000000000",
  deliveryModel: "windows_local",
  tenantHash: hash("1"),
  subjectHash: hash("2"),
  registrationId: "dreg_01J00000000000000000000000",
  installationPublicKeyHash: hash("3"),
  entitlementLeaseId: "lease_01J00000000000000000000000",
  entitlementLeaseHash: hash("4"),
  tenantPolicyId: "policy_01J00000000000000000000000",
  tenantPolicyVersion: 7,
  tenantPolicyHash: hash("5"),
  purpose: "bmad_help",
  modelRole: "planner",
  canonicalOutputSchemaId: "sapphirus.bmad-method-help-proposal.v1",
  canonicalOutputSchemaHash: hash("6"),
  manifestHash: hash("7"),
  invocationBindingHash: hash("8"),
  consumptionHash: hash("9"),
  consentDisclosureHash: hash("a"),
  providerProfileHash: hash("b"),
  modelProfileHash: hash("c"),
  modelCapabilityHash: hash("d"),
  deploymentHash: hash("e"),
  region: "westeurope",
  retentionMode: "transient_no_store",
  budgetClass: "interactive-standard",
  issuedAt: "2026-07-16T10:00:00.000Z",
  notBefore: "2026-07-16T10:00:00.000Z",
  expiresAt: "2026-07-16T10:05:00.000Z",
  nonceHash: hash("f"),
  consentEnvelopeHash: hash("0"),
  proof: {
    proofType: "installation_signature",
    algorithm: "ES256",
    keyId: "installation-key-2026-07",
    signedPayloadHash: hash("0"),
    signature: "ZXhhbXBsZS1kZXZpY2Utc2lnbmF0dXJl",
  },
};

const validReceipt = {
  schemaVersion: "sapphirus.model-access-receipt.v1",
  receiptId: "receipt_01J00000000000000000000000",
  requestId: validConsent.requestId,
  requestHash: hash("1"),
  resultHash: hash("2"),
  deliveryModel: "windows_local",
  tenantHash: validConsent.tenantHash,
  subjectHash: validConsent.subjectHash,
  registrationId: validConsent.registrationId,
  manifestHash: validConsent.manifestHash,
  invocationBindingHash: validConsent.invocationBindingHash,
  consumptionHash: validConsent.consumptionHash,
  consentEnvelopeHash: validConsent.consentEnvelopeHash,
  consentDisclosureHash: validConsent.consentDisclosureHash,
  providerProfileHash: validConsent.providerProfileHash,
  modelProfileHash: validConsent.modelProfileHash,
  modelCapabilityHash: validConsent.modelCapabilityHash,
  deploymentHash: validConsent.deploymentHash,
  canonicalOutputSchemaId: validConsent.canonicalOutputSchemaId,
  canonicalOutputSchemaHash: validConsent.canonicalOutputSchemaHash,
  providerSchemaProjectionHash: hash("3"),
  credentialBindingHash: hash("4"),
  retentionMode: "transient_no_store",
  region: validConsent.region,
  inputBytes: 8421,
  outputBytes: 1200,
  usage: {
    inputTokens: 2100,
    outputTokens: 300,
    costMicrounits: 7200,
    currency: "EUR",
  },
  retryCount: 0,
  fallbackEvents: [],
  providerRequestId: "provider-request-opaque",
  startedAt: "2026-07-16T10:00:01.000Z",
  completedAt: "2026-07-16T10:00:02.000Z",
  terminalStatus: "succeeded",
  receiptHash: hash("5"),
  proof: {
    proofType: "support_plane_signature",
    algorithm: "ES256",
    issuer: "https://support.sapphirus.example/",
    audience: "sapphirus-desktop",
    keyId: "model-receipt-key-2026-07",
    signedPayloadHash: hash("5"),
    signature: "ZXhhbXBsZS1zdXBwb3J0LXBsYW5lLXNpZ25hdHVyZQ",
  },
};

test("canonical D2 consent and receipt contracts bind both authorities", () => {
  assert.deepEqual(
    validateSchemaDocument(registry, "model-context-consent.schema.json", validConsent),
    [],
  );
  assert.deepEqual(
    validateSchemaDocument(registry, "model-access-receipt.schema.json", validReceipt),
    [],
  );
});

test("D2 contracts reject delivery-model drift and receipt content leakage", () => {
  const driftedConsent = structuredClone(validConsent);
  driftedConsent.deliveryModel = "web_managed";
  assert.ok(
    validateSchemaDocument(
      registry,
      "model-context-consent.schema.json",
      driftedConsent,
    ).some((error) => error.code === "CONST_MISMATCH"),
  );

  const leakingReceipt = structuredClone(validReceipt);
  leakingReceipt.rawPrompt = "must never be durable receipt content";
  assert.ok(
    validateSchemaDocument(
      registry,
      "model-access-receipt.schema.json",
      leakingReceipt,
    ).some((error) => error.code === "UNKNOWN_PROPERTY"),
  );
});

test("generated TypeScript validators expose both D2 authority contracts", () => {
  assert.equal(CONTRACT_VALIDATORS["model-context-consent"](validConsent), true);
  assert.equal(CONTRACT_VALIDATORS["model-access-receipt"](validReceipt), true);
});
