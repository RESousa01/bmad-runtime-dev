#![allow(
    clippy::expect_used,
    reason = "invalid static fixtures should fail the test at construction"
)]

use desktop_cloud::{CloudError, EntitlementLease, EntitlementProofVerifier, EntitlementVerifier};
use desktop_runtime::{sha256_bytes, UnixMillis};
use semver::Version;

fn lease() -> EntitlementLease {
    EntitlementLease {
        schema_version: "desktop-entitlement-lease.v1".to_owned(),
        lease_id: "lease_001".to_owned(),
        registration_id: "dreg_0123456789abcdef0123456789".to_owned(),
        subject_hash: sha256_bytes(b"subject").to_string(),
        delivery_model: "windows_local".to_owned(),
        issued_at: "2026-07-15T12:00:00Z".to_owned(),
        not_before: "2026-07-15T11:58:00Z".to_owned(),
        expires_at: "2026-07-16T12:00:00Z".to_owned(),
        offline_grace_ends_at: "2026-07-19T12:00:00Z".to_owned(),
        features: vec!["local_runtime".to_owned(), "model_access".to_owned()],
        tenant_policy_hash: sha256_bytes(b"policy").to_string(),
        minimum_client_version: "0.1.0-beta.1".to_owned(),
        key_id: "test-key".to_owned(),
        signature: "valid-proof".to_owned(),
    }
}

struct KnownProof;

impl EntitlementProofVerifier for KnownProof {
    fn verify(&self, lease: &EntitlementLease) -> Result<(), CloudError> {
        if lease.signature == "valid-proof" && lease.key_id == "test-key" {
            Ok(())
        } else {
            Err(CloudError::EntitlementUnavailable)
        }
    }
}

fn verifier() -> EntitlementVerifier<KnownProof> {
    EntitlementVerifier::new(
        KnownProof,
        "dreg_0123456789abcdef0123456789",
        sha256_bytes(b"subject"),
        sha256_bytes(b"policy"),
        "model_access",
        Version::parse("0.1.0-beta.1").expect("version"),
    )
    .expect("verifier")
}

#[test]
fn valid_audience_bound_lease_enables_only_the_verified_feature_window() {
    let verified = verifier()
        .verify(&lease(), UnixMillis(1_784_116_800_000))
        .expect("verified entitlement");

    assert_eq!(
        verified.registration_id(),
        "dreg_0123456789abcdef0123456789"
    );
    assert_eq!(verified.required_feature(), "model_access");
    assert_eq!(verified.expires_at(), UnixMillis(1_784_203_200_000));
}

#[test]
fn registration_subject_policy_and_feature_substitution_fail_closed() {
    let mut changed = lease();
    changed.registration_id = "dreg_ffffffffffffffffffffffffff".to_owned();
    assert!(matches!(
        verifier().verify(&changed, UnixMillis(1_784_116_800_000)),
        Err(CloudError::EntitlementUnavailable)
    ));

    let mut changed = lease();
    changed.subject_hash = sha256_bytes(b"other-subject").to_string();
    assert!(verifier()
        .verify(&changed, UnixMillis(1_784_116_800_000))
        .is_err());

    let mut changed = lease();
    changed.tenant_policy_hash = sha256_bytes(b"other-policy").to_string();
    assert!(verifier()
        .verify(&changed, UnixMillis(1_784_116_800_000))
        .is_err());

    let mut changed = lease();
    changed.features.retain(|feature| feature != "model_access");
    assert!(matches!(
        verifier().verify(&changed, UnixMillis(1_784_116_800_000)),
        Err(CloudError::FeatureDisabled)
    ));
}

#[test]
fn not_yet_valid_expired_and_incoherent_windows_fail_closed() {
    assert!(verifier()
        .verify(&lease(), UnixMillis(1_784_116_620_000))
        .is_err());
    assert!(verifier()
        .verify(&lease(), UnixMillis(1_784_203_200_000))
        .is_err());

    let mut incoherent = lease();
    incoherent.offline_grace_ends_at = "2026-07-15T12:01:00Z".to_owned();
    assert!(verifier()
        .verify(&incoherent, UnixMillis(1_784_116_800_000))
        .is_err());
}

#[test]
fn minimum_client_version_and_proof_are_enforced() {
    let mut update_required = lease();
    update_required.minimum_client_version = "0.2.0".to_owned();
    assert!(matches!(
        verifier().verify(&update_required, UnixMillis(1_784_116_800_000)),
        Err(CloudError::EntitlementUnavailable)
    ));

    let mut forged = lease();
    forged.signature = "forged".to_owned();
    assert!(verifier()
        .verify(&forged, UnixMillis(1_784_116_800_000))
        .is_err());
}

#[test]
fn duplicate_features_unknown_fields_and_malformed_hashes_are_rejected() {
    let mut duplicate = lease();
    duplicate.features.push("model_access".to_owned());
    assert!(verifier()
        .verify(&duplicate, UnixMillis(1_784_116_800_000))
        .is_err());

    let mut malformed = lease();
    malformed.subject_hash = "not-a-hash".to_owned();
    assert!(verifier()
        .verify(&malformed, UnixMillis(1_784_116_800_000))
        .is_err());

    let mut value = serde_json::to_value(lease()).expect("lease value");
    value["unexpected"] = serde_json::json!(true);
    assert!(serde_json::from_value::<EntitlementLease>(value).is_err());
}
