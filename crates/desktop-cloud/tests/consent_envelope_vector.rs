#![allow(
    clippy::expect_used,
    reason = "test fixtures fail loudly at construction"
)]
//! Golden vector for the canonical consent-envelope hash, shared verbatim
//! with the C# verifier tests (`InstallationConsentVerifierTests`). Both
//! sides must produce the same digest for this fixed draft.

use desktop_runtime::canonical_hash;
use serde_json::json;

const HASH: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

pub const EXPECTED_ENVELOPE_HASH: &str =
    "sha256:9789b78a496650d993bfd4d0595924117a9a7ba5beba6285a05608d37f298735";

#[test]
fn consent_envelope_hash_matches_the_shared_golden_vector() {
    let draft = json!({
        "schemaVersion": "sapphirus.model-context-consent.v1",
        "decisionId": "decision_01J00000000000000000000000",
        "requestId": "request_01J00000000000000000000000",
        "invocationId": "invoke_01J00000000000000000000000",
        "deliveryModel": "windows_local",
        "tenantHash": HASH,
        "subjectHash": HASH,
        "registrationId": "dreg_AAAAAAAAAAAAAAAAAAAAAAAAAA",
        "installationPublicKeyHash": HASH,
        "entitlementLeaseId": "lease_01J00000000000000000000000",
        "entitlementLeaseHash": HASH,
        "tenantPolicyId": "policy_01J00000000000000000000000",
        "tenantPolicyVersion": 7,
        "tenantPolicyHash": HASH,
        "purpose": "bmad_help",
        "modelRole": "planner",
        "canonicalOutputSchemaId": "sapphirus.bmad-method-help-proposal.v1",
        "canonicalOutputSchemaHash": HASH,
        "manifestHash": HASH,
        "invocationBindingHash": HASH,
        "consumptionHash": HASH,
        "consentDisclosureHash": HASH,
        "providerProfileHash": HASH,
        "modelProfileHash": HASH,
        "modelCapabilityHash": HASH,
        "deploymentHash": HASH,
        "region": "westeurope",
        "retentionMode": "transient_no_store",
        "budgetClass": "interactive-standard",
        "issuedAt": "2026-07-16T10:00:00.000Z",
        "notBefore": "2026-07-16T10:00:00.000Z",
        "expiresAt": "2026-07-16T10:05:00.000Z",
        "nonceHash": HASH,
    });
    let envelope_hash =
        canonical_hash("model-context-consent", 1, &draft).expect("canonical hash");
    assert_eq!(envelope_hash.to_string(), EXPECTED_ENVELOPE_HASH);
}
