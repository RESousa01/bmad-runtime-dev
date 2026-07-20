#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::unreadable_literal,
    clippy::too_many_lines,
    reason = "test fixtures fail loudly at construction"
)]

//! Host-only lifecycle tests for the production support composition:
//! fail-closed configuration, ES256 proof verification for policies,
//! leases, and receipts, downgrade/replay protection, cached-state
//! re-verification, and sign-out epoch invalidation.

use desktop_cloud::{
    verify_canonical_receipt_proof, CanonicalReceiptProof, CloudError, PinnedProofKey,
    ProductionSupportClient, ProductionSupportConfig, ProofKeyRing, SignedDesktopPolicyDoc,
    SignedEntitlementLeaseDoc, SignedStateStore, VerifiedSignedPolicy,
};
use desktop_runtime::{canonical_hash, ContractId, UnixMillis};
use parking_lot::Mutex;
use serde_json::json;
use std::collections::HashMap;

const TENANT: &str = "11111111-1111-1111-1111-111111111111";
const CLIENT: &str = "22222222-2222-2222-2222-222222222222";
const SCOPE: &str = "api://22222222-2222-2222-2222-222222222222/desktop.access";
const ORIGIN: &str = "https://support.sapphirus.example";
const ISSUER: &str = "https://login.microsoftonline.com/11111111-1111-1111-1111-111111111111/v2.0";
const AUDIENCE: &str = "api://22222222-2222-2222-2222-222222222222";

fn placeholder_ring() -> ProofKeyRing {
    ProofKeyRing::new(
        PinnedProofKey {
            key_id: "https://signing.vault.example/keys/proof/1111aaaa".to_owned(),
            public_key_spki: desktop_cloud::p256_spki_from_point(&[0x11; 32], &[0x22; 32]),
        },
        vec![],
    )
    .expect("placeholder ring")
}

fn config_with(ring: ProofKeyRing) -> ProductionSupportConfig {
    ProductionSupportConfig::new(
        TENANT,
        CLIENT,
        SCOPE,
        ORIGIN,
        "westeurope",
        ring.clone(),
        ring,
        ISSUER,
        AUDIENCE,
    )
    .expect("valid production configuration")
}

#[derive(Default)]
struct MemoryStateStore {
    values: Mutex<HashMap<String, String>>,
}

impl SignedStateStore for MemoryStateStore {
    fn load(&self, name: &str) -> Option<String> {
        self.values.lock().get(name).cloned()
    }

    fn save(&self, name: &str, value: &str) {
        self.values.lock().insert(name.to_owned(), value.to_owned());
    }

    fn clear(&self) {
        self.values.lock().clear();
    }
}

#[test]
fn production_cannot_start_without_exact_configuration() {
    let ring = placeholder_ring;
    let valid = |tenant: &str,
                 client: &str,
                 scope: &str,
                 origin: &str,
                 region: &str,
                 issuer: &str,
                 audience: &str| {
        ProductionSupportConfig::new(
            tenant,
            client,
            scope,
            origin,
            region,
            ring(),
            ring(),
            issuer,
            audience,
        )
    };
    assert!(valid(
        TENANT,
        CLIENT,
        SCOPE,
        ORIGIN,
        "westeurope",
        ISSUER,
        AUDIENCE
    )
    .is_ok());
    for broken in [
        valid(
            "not-a-guid",
            CLIENT,
            SCOPE,
            ORIGIN,
            "westeurope",
            ISSUER,
            AUDIENCE,
        ),
        valid(TENANT, "", SCOPE, ORIGIN, "westeurope", ISSUER, AUDIENCE),
        valid(
            TENANT,
            CLIENT,
            "desktop.access",
            ORIGIN,
            "westeurope",
            ISSUER,
            AUDIENCE,
        ),
        valid(
            TENANT,
            CLIENT,
            SCOPE,
            "http://support.sapphirus.example",
            "westeurope",
            ISSUER,
            AUDIENCE,
        ),
        valid(TENANT, CLIENT, SCOPE, ORIGIN, "", ISSUER, AUDIENCE),
        valid(
            TENANT,
            CLIENT,
            SCOPE,
            ORIGIN,
            "westeurope",
            "not-a-url",
            AUDIENCE,
        ),
        valid(TENANT, CLIENT, SCOPE, ORIGIN, "westeurope", ISSUER, ""),
    ] {
        assert!(
            broken.is_err(),
            "partial production configuration must fail closed"
        );
    }
}

#[test]
fn session_projection_is_bounded_and_sign_out_invalidates_epochs() {
    let client = ProductionSupportClient::new(
        config_with(placeholder_ring()),
        Box::new(MemoryStateStore::default()),
    );
    let projection = client.session_projection();
    let serialized = serde_json::to_string(&projection).expect("serialize projection");
    // Bounded IPC projection: status words and epoch only; no key, token,
    // proof, path, or origin material.
    assert_eq!(
        serialized,
        format!(
            "{{\"schemaVersion\":\"sapphirus.production-session.v1\",\"status\":\"configured\",\"sessionEpoch\":{},\"region\":\"westeurope\"}}",
            client.session_epoch(),
        ),
    );

    let epoch_before = client.session_epoch();
    assert!(client.require_session(epoch_before).is_ok());
    client.sign_out();
    assert_eq!(
        client.require_session(epoch_before),
        Err(CloudError::SessionInvalidated),
    );
    assert!(client.require_session(client.session_epoch()).is_ok());
}

#[cfg(windows)]
mod signed_documents {
    use desktop_cloud::{InstallationConsentSigner as _, WindowsInstallationIdentity};

    use super::*;

    struct SignerFixture {
        identity: WindowsInstallationIdentity,
        key_id: String,
    }

    impl SignerFixture {
        fn create() -> Self {
            let identity = WindowsInstallationIdentity::open_or_create(&format!(
                "sapphirus-test-proof-{:016x}",
                rand::random::<u64>(),
            ))
            .expect("create test proof key");
            let key_id = identity.key_id().to_owned();
            Self { identity, key_id }
        }

        fn ring(&self) -> ProofKeyRing {
            ProofKeyRing::new(
                PinnedProofKey {
                    key_id: self.key_id.clone(),
                    public_key_spki: self.identity.public_key_spki().to_vec(),
                },
                vec![],
            )
            .expect("test ring")
        }

        fn config(&self) -> ProductionSupportConfig {
            config_with(self.ring())
        }

        fn signed_policy(&self) -> SignedDesktopPolicyDoc {
            let draft = json!({
                "schemaVersion": "desktop-policy.v1",
                "policyId": "policy_01J00000000000000000000000",
                "policyVersion": 7,
                "systemBrowserFallbackAllowed": false,
                "maximumContextBytes": 524_288,
                "maximumContextItems": 64,
                "allowedRegions": ["westeurope"],
                "retentionMode": "transient_no_store",
            });
            let digest = canonical_hash("desktop-policy", 1, &draft).expect("policy digest");
            let signature = self
                .identity
                .sign_digest(digest.as_bytes())
                .expect("sign policy digest");
            SignedDesktopPolicyDoc {
                schema_version: "desktop-policy.v1".to_owned(),
                policy_id: "policy_01J00000000000000000000000".to_owned(),
                policy_version: 7,
                policy_hash: digest.to_string(),
                system_browser_fallback_allowed: false,
                maximum_context_bytes: 524_288,
                maximum_context_items: 64,
                allowed_regions: vec!["westeurope".to_owned()],
                key_id: self.key_id.clone(),
                signature,
            }
        }

        fn signed_lease(
            &self,
            policy: &SignedDesktopPolicyDoc,
            registration_id: &str,
        ) -> SignedEntitlementLeaseDoc {
            let draft = json!({
                "schemaVersion": "desktop-entitlement-lease.v1",
                "leaseId": "lease_01J00000000000000000000000",
                "registrationId": registration_id,
                "subjectHash": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "deliveryModel": "windows_local",
                "issuedAt": "2026-07-20T10:00:00.000Z",
                "notBefore": "2026-07-20T09:58:00.000Z",
                "expiresAt": "2026-07-21T10:00:00.000Z",
                "offlineGraceEndsAt": "2026-07-24T10:00:00.000Z",
                "features": ["local_runtime", "model_access"],
                "tenantPolicyHash": policy.policy_hash,
                "minimumClientVersion": "0.1.0-beta.1",
            });
            let digest = canonical_hash("entitlement-lease", 1, &draft).expect("lease digest");
            let signature = self
                .identity
                .sign_digest(digest.as_bytes())
                .expect("sign lease digest");
            SignedEntitlementLeaseDoc {
                schema_version: "desktop-entitlement-lease.v1".to_owned(),
                lease_id: "lease_01J00000000000000000000000".to_owned(),
                registration_id: registration_id.to_owned(),
                subject_hash:
                    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        .to_owned(),
                delivery_model: "windows_local".to_owned(),
                // The wire rendering intentionally differs from the canonical
                // rendering: verification must normalize before hashing.
                issued_at: "2026-07-20T10:00:00+00:00".to_owned(),
                not_before: "2026-07-20T09:58:00+00:00".to_owned(),
                expires_at: "2026-07-21T10:00:00+00:00".to_owned(),
                offline_grace_ends_at: "2026-07-24T10:00:00+00:00".to_owned(),
                features: vec!["local_runtime".to_owned(), "model_access".to_owned()],
                tenant_policy_hash: policy.policy_hash.clone(),
                minimum_client_version: "0.1.0-beta.1".to_owned(),
                key_id: self.key_id.clone(),
                signature,
            }
        }

        fn delete(self) {
            self.identity.delete().expect("delete test key");
        }
    }

    #[test]
    fn policy_lease_and_receipt_proofs_verify_and_every_tamper_fails_closed() {
        let fixture = SignerFixture::create();
        let config = fixture.config();
        let registration =
            ContractId::new("dreg_AAAAAAAAAAAAAAAAAAAAAAAAAA").expect("registration id");
        // Policy verifies; canonical hash equals the served policy hash.
        let policy_doc = fixture.signed_policy();
        let policy = VerifiedSignedPolicy::verify(policy_doc.clone(), &config, None)
            .expect("valid signed policy");
        assert_eq!(policy.canonical_hash.to_string(), policy_doc.policy_hash);

        // Policy tampering matrix.
        let mut wrong_region = policy_doc.clone();
        wrong_region.allowed_regions = vec!["eastus".to_owned()];
        assert!(VerifiedSignedPolicy::verify(wrong_region, &config, None).is_err());
        let mut wrong_signature = policy_doc.clone();
        wrong_signature.signature = "AAAA".repeat(22);
        assert!(VerifiedSignedPolicy::verify(wrong_signature, &config, None).is_err());
        let mut unknown_key = policy_doc.clone();
        unknown_key.key_id = "https://signing.vault.example/keys/proof/retired".to_owned();
        assert!(VerifiedSignedPolicy::verify(unknown_key, &config, None).is_err());
        let mut mutated_limit = policy_doc.clone();
        mutated_limit.maximum_context_bytes = 1;
        assert!(
            VerifiedSignedPolicy::verify(mutated_limit, &config, None).is_err(),
            "mutating a covered field breaks the canonical hash",
        );
        assert!(
            VerifiedSignedPolicy::verify(policy_doc.clone(), &config, Some(9)).is_err(),
            "version downgrade must fail closed",
        );
        assert!(
            serde_json::from_value::<SignedDesktopPolicyDoc>(json!({
                "schemaVersion": "desktop-policy.v1",
                "policyId": "policy_01J00000000000000000000000",
                "policyVersion": 7,
                "policyHash": policy_doc.policy_hash,
                "systemBrowserFallbackAllowed": false,
                "maximumContextBytes": 524288,
                "maximumContextItems": 64,
                "allowedRegions": ["westeurope"],
                "keyId": policy_doc.key_id,
                "signature": policy_doc.signature,
                "injectedField": true,
            }))
            .is_err(),
            "unknown policy fields must fail closed",
        );

        // Lease verifies inside its window with wire-format instants.
        let lease_doc = fixture.signed_lease(&policy_doc, registration.as_str());
        let in_window = UnixMillis(1784548800000); // 2026-07-20T12:00:00Z
        let lease = desktop_cloud::VerifiedLease::verify(
            lease_doc.clone(),
            &config,
            &policy,
            &registration,
            in_window,
        );
        assert!(
            lease.is_ok(),
            "lease with normalized instants verifies: {lease:?}"
        );

        // Lease tampering matrix.
        let other_registration =
            ContractId::new("dreg_BBBBBBBBBBBBBBBBBBBBBBBBBB").expect("other registration");
        assert!(desktop_cloud::VerifiedLease::verify(
            lease_doc.clone(),
            &config,
            &policy,
            &other_registration,
            in_window,
        )
        .is_err());
        let expired = UnixMillis(1784721600000); // 2026-07-22T12:00:00Z
        assert!(desktop_cloud::VerifiedLease::verify(
            lease_doc.clone(),
            &config,
            &policy,
            &registration,
            expired,
        )
        .is_err());
        let mut wrong_policy_hash = lease_doc.clone();
        wrong_policy_hash.tenant_policy_hash =
            "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned();
        assert!(desktop_cloud::VerifiedLease::verify(
            wrong_policy_hash,
            &config,
            &policy,
            &registration,
            in_window,
        )
        .is_err());

        // Receipt proof verifies and replays fail closed.
        let receipt_hash_digest =
            canonical_hash("model-access-receipt", 1, &json!({"fixture": "receipt"}))
                .expect("receipt digest");
        let receipt_hash = receipt_hash_digest.to_string();
        let signature = fixture
            .identity
            .sign_digest(receipt_hash_digest.as_bytes())
            .expect("sign receipt digest");
        let proof = CanonicalReceiptProof {
            proof_type: "support_plane_signature".to_owned(),
            algorithm: "ES256".to_owned(),
            issuer: ISSUER.to_owned(),
            audience: AUDIENCE.to_owned(),
            key_id: fixture.key_id.clone(),
            signed_payload_hash: receipt_hash.clone(),
            signature,
        };
        assert!(verify_canonical_receipt_proof(&proof, &receipt_hash, &config).is_ok());
        let mut wrong_issuer = proof.clone();
        wrong_issuer.issuer = "https://evil.example/".to_owned();
        assert!(verify_canonical_receipt_proof(&wrong_issuer, &receipt_hash, &config).is_err());
        let mut wrong_audience = proof.clone();
        wrong_audience.audience = "api://other".to_owned();
        assert!(verify_canonical_receipt_proof(&wrong_audience, &receipt_hash, &config).is_err());
        let mut wrong_algorithm = proof.clone();
        wrong_algorithm.algorithm = "ES384".to_owned();
        assert!(verify_canonical_receipt_proof(&wrong_algorithm, &receipt_hash, &config).is_err());

        let client =
            ProductionSupportClient::new(fixture.config(), Box::new(MemoryStateStore::default()));
        assert!(client
            .accept_receipt_proof("receipt_REPLAYTEST", &proof, &receipt_hash)
            .is_ok());
        assert_eq!(
            client.accept_receipt_proof("receipt_REPLAYTEST", &proof, &receipt_hash),
            Err(CloudError::ReceiptInvalid),
            "a replayed receipt id must fail closed",
        );

        fixture.delete();
    }

    #[test]
    fn tampered_last_known_valid_cache_is_discarded_not_trusted() {
        let fixture = SignerFixture::create();
        let store = MemoryStateStore::default();
        store.save(
            "policy.last-known-valid",
            "{\"schemaVersion\":\"desktop-policy.v1\",\"forged\":true}",
        );
        let client = ProductionSupportClient::new(fixture.config(), Box::new(store));
        assert!(client.last_known_policy().is_none());

        let accepted = client
            .accept_policy(fixture.signed_policy())
            .expect("accept valid policy");
        let cached = client.last_known_policy().expect("cache round-trips");
        assert_eq!(
            cached.document.policy_version,
            accepted.document.policy_version
        );

        let mut downgraded = fixture.signed_policy();
        downgraded.policy_version = 3;
        // Downgrade against the cached version 7 must fail even though the
        // document itself is... unsigned-for-3, so both hash and downgrade fail.
        assert!(client.accept_policy(downgraded).is_err());

        fixture.delete();
    }
}
