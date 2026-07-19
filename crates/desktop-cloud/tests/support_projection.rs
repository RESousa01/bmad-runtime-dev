#![allow(
    clippy::expect_used,
    reason = "invalid static fixtures should fail the test at construction"
)]

//! Conformance tests for the canonical support-plane transport projection.

use std::num::NonZeroU64;

use desktop_cloud::{
    project_model_access_request, AuthorizedModelRequest, CanonicalModelAccessRequest,
    CanonicalProjectionInputs, CloudError, ConsentProfile, ConsentSubject, ConsentWindow,
    InstallationConsentSigner, LeaseBinding, RegistrationBinding, TenantPolicyBinding,
};
use desktop_egress::{
    ApproveDecisionInput, ConsentService, ConsumeDecisionInput, ContextCandidate,
    ContextClassification, ContextPreparer, EgressLimits, MemoryDecisionLedger,
    ModelInvocationBindingDraft, PatternSecretScanner, PrepareContextInput, RetentionMode,
};
use desktop_runtime::{sha256_bytes, ContractId, RelativeWorkspacePath, Sha256Digest, UnixMillis};
use serde_json::Value;

fn id(value: &str) -> ContractId {
    ContractId::new(value).expect("valid fixture identifier")
}

struct FakeInstallationSigner;

impl InstallationConsentSigner for FakeInstallationSigner {
    #[expect(
        clippy::unnecessary_literal_bound,
        reason = "the trait signature fixes the return lifetime"
    )]
    fn key_id(&self) -> &str {
        "installation-key-1"
    }

    fn sign(&self, signed_payload_hash: &Sha256Digest) -> Result<String, CloudError> {
        let mut rendered = signed_payload_hash.to_string();
        rendered = rendered.replace(':', "_");
        Ok(format!("sig_{rendered}"))
    }
}

struct FailingSigner;

impl InstallationConsentSigner for FailingSigner {
    #[expect(
        clippy::unnecessary_literal_bound,
        reason = "the trait signature fixes the return lifetime"
    )]
    fn key_id(&self) -> &str {
        "installation-key-1"
    }

    fn sign(&self, _signed_payload_hash: &Sha256Digest) -> Result<String, CloudError> {
        Err(CloudError::IdentityUnavailable)
    }
}

fn authorized_fixture(language: Option<&str>) -> AuthorizedModelRequest {
    let manifest = ContextPreparer::new(PatternSecretScanner)
        .prepare(PrepareContextInput {
            tenant_ref: id("tenant_01J0000000000000"),
            project_ref: id("project_01J0000000000000"),
            run_ref: id("run_01J0000000000000"),
            purpose: "bmad_help".to_owned(),
            model_role: "planner".to_owned(),
            canonical_output_schema_id: id("sapphirus.bmad-method-help-proposal.v1"),
            canonical_output_schema_hash: sha256_bytes(b"schema"),
            provider_profile_hash: sha256_bytes(b"provider-profile"),
            model_profile_hash: sha256_bytes(b"model-profile"),
            deployment_hash: sha256_bytes(b"deployment"),
            policy_hash: sha256_bytes(b"policy"),
            region: "westeurope".to_owned(),
            retention_mode: RetentionMode::TransientNoStore,
            created_at: UnixMillis(1_000),
            expires_at: UnixMillis(61_000),
            limits: EgressLimits {
                maximum_context_items: 8,
                maximum_context_bytes: 64 * 1024,
                maximum_token_estimate: 16_000,
            },
            candidates: vec![ContextCandidate {
                client_item_id: id("item_01J0000000000000"),
                relative_label: RelativeWorkspacePath::new("notes.txt").expect("fixture path"),
                semantic_role: "source".to_owned(),
                language: language.map(str::to_owned),
                classification: ContextClassification::Internal,
                content: "safe context".to_owned(),
            }],
            exclusions: Vec::new(),
        })
        .expect("manifest");
    let binding = ModelInvocationBindingDraft {
        schema_version: "sapphirus.model-invocation-binding.v1".to_owned(),
        request_id: id("request_01J0000000000000"),
        tenant_ref: manifest.draft.tenant_ref.clone(),
        project_ref: manifest.draft.project_ref.clone(),
        run_ref: manifest.draft.run_ref.clone(),
        installation_id: id("install_01J0000000000000"),
        session_authority_hash: sha256_bytes(b"session-authority"),
        manifest_hash: manifest.manifest_hash,
        purpose: manifest.draft.purpose.clone(),
        model_role: manifest.draft.model_role.clone(),
        canonical_output_schema_id: manifest.draft.canonical_output_schema_id.clone(),
        canonical_output_schema_hash: manifest.draft.canonical_output_schema_hash,
        provider_profile_hash: manifest.draft.provider_profile_hash,
        model_profile_hash: manifest.draft.model_profile_hash,
        deployment_hash: manifest.draft.deployment_hash,
        policy_hash: manifest.draft.policy_hash,
        region: manifest.draft.region.clone(),
        retention_mode: manifest.draft.retention_mode,
        consent_disclosure_hash: sha256_bytes(b"consent-disclosure-v1"),
    }
    .seal()
    .expect("binding");
    let ledger = MemoryDecisionLedger::default();
    let service = ConsentService::new(&ledger);
    let decision = service
        .approve(ApproveDecisionInput {
            manifest: &manifest,
            binding: &binding,
            decision_id: id("decision_01J0000000000000"),
            issued_at: UnixMillis(1_500),
            expires_at: UnixMillis(31_500),
        })
        .expect("decision");
    let consumption = service
        .consume(ConsumeDecisionInput {
            decision: &decision,
            binding: &binding,
            invocation_id: id("invoke_01J0000000000000"),
            consumed_at: UnixMillis(2_000),
        })
        .expect("consumption");

    AuthorizedModelRequest::new(&manifest, &binding, consumption).expect("authorized request")
}

fn registration() -> RegistrationBinding {
    RegistrationBinding {
        registration_id: id("dreg_01J0000000000000"),
        installation_public_key_hash: sha256_bytes(b"installation-public-key"),
    }
}

fn lease() -> LeaseBinding {
    LeaseBinding {
        lease_id: id("lease_01J0000000000000"),
        lease_hash: sha256_bytes(b"entitlement-lease"),
    }
}

fn policy(policy_hash: Sha256Digest) -> TenantPolicyBinding {
    TenantPolicyBinding {
        policy_id: id("policy_01J0000000000000"),
        policy_version: NonZeroU64::new(7).expect("nonzero"),
        policy_hash,
    }
}

fn subject() -> ConsentSubject {
    ConsentSubject {
        tenant_hash: sha256_bytes(b"tenant"),
        subject_hash: sha256_bytes(b"subject"),
    }
}

fn window() -> ConsentWindow {
    ConsentWindow {
        issued_at: UnixMillis(2_000),
        not_before: UnixMillis(2_000),
        expires_at: UnixMillis(32_000),
        nonce_hash: sha256_bytes(b"nonce"),
    }
}

fn profile() -> ConsentProfile {
    ConsentProfile {
        model_capability_hash: sha256_bytes(b"model-capability"),
        budget_class: "standard".to_owned(),
    }
}

#[test]
fn projection_emits_only_the_canonical_public_envelope() {
    let request = authorized_fixture(Some("text"));
    let projected = project_model_access_request(
        request,
        &CanonicalProjectionInputs {
            registration: &registration(),
            lease: &lease(),
            policy: &policy(sha256_bytes(b"policy")),
            subject: &subject(),
            window: &window(),
            profile: &profile(),
            signer: &FakeInstallationSigner,
        },
        )
    .expect("projection");

    let serialized = serde_json::to_value(&projected).expect("serialize");
    let rendered = serialized.to_string();
    for forbidden in [
        "projectRef",
        "runRef",
        "tokenEstimate",
        "redactions",
        "totalOutboundBytes",
        "decisionHash",
        "installationId",
        "sessionAuthorityHash",
    ] {
        assert!(
            !rendered.contains(forbidden),
            "canonical envelope must not leak local-only field {forbidden}"
        );
    }

    assert_eq!(
        serialized["schemaVersion"],
        "desktop-model-access-request.v1"
    );
    assert_eq!(serialized["deliveryModel"], "windows_local");
    assert_eq!(serialized["retentionMode"], "transient_no_store");
    assert_eq!(serialized["registrationId"], "dreg_01J0000000000000");
    assert_eq!(
        serialized["consent"]["proof"]["proofType"],
        "installation_signature"
    );
    assert_eq!(serialized["consent"]["proof"]["algorithm"], "ES256");
    assert_eq!(serialized["consent"]["tenantPolicyVersion"], 7);
    assert_eq!(serialized["items"][0]["classification"], "internal");
    assert_eq!(
        serialized["consent"]["proof"]["signedPayloadHash"],
        serialized["consent"]["consentEnvelopeHash"],
        "the installation proof must sign exactly the consent envelope hash"
    );
    assert_eq!(
        serialized["consent"]["issuedAt"],
        "1970-01-01T00:00:02.000Z"
    );

    let round_tripped: CanonicalModelAccessRequest =
        serde_json::from_value(serialized.clone()).expect("canonical round trip");
    let reserialized = serde_json::to_value(&round_tripped).expect("reserialize");
    assert_eq!(
        reserialized, serialized,
        "canonical serialization must be stable"
    );
}

#[test]
fn projection_rejects_a_policy_binding_that_does_not_match_the_request() {
    let request = authorized_fixture(Some("text"));
    let error = project_model_access_request(
        request,
        &CanonicalProjectionInputs {
            registration: &registration(),
            lease: &lease(),
            policy: &policy(sha256_bytes(b"different-policy")),
            subject: &subject(),
            window: &window(),
            profile: &profile(),
            signer: &FakeInstallationSigner,
        },
        )
    .expect_err("policy drift must fail closed");
    assert_eq!(error, CloudError::CanonicalProjectionInvalid);
}

#[test]
fn projection_rejects_items_without_an_explicit_language() {
    let request = authorized_fixture(None);
    let error = project_model_access_request(
        request,
        &CanonicalProjectionInputs {
            registration: &registration(),
            lease: &lease(),
            policy: &policy(sha256_bytes(b"policy")),
            subject: &subject(),
            window: &window(),
            profile: &profile(),
            signer: &FakeInstallationSigner,
        },
        )
    .expect_err("missing language must fail closed");
    assert_eq!(error, CloudError::CanonicalProjectionInvalid);
}

#[test]
fn projection_rejects_an_inverted_consent_window() {
    let request = authorized_fixture(Some("text"));
    let inverted = ConsentWindow {
        issued_at: UnixMillis(2_000),
        not_before: UnixMillis(1_000),
        expires_at: UnixMillis(32_000),
        nonce_hash: sha256_bytes(b"nonce"),
    };
    let error = project_model_access_request(
        request,
        &CanonicalProjectionInputs {
            registration: &registration(),
            lease: &lease(),
            policy: &policy(sha256_bytes(b"policy")),
            subject: &subject(),
            window: &inverted,
            profile: &profile(),
            signer: &FakeInstallationSigner,
        },
        )
    .expect_err("inverted window must fail closed");
    assert_eq!(error, CloudError::CanonicalProjectionInvalid);
}

#[test]
fn projection_surfaces_signer_failures_without_emitting_an_envelope() {
    let request = authorized_fixture(Some("text"));
    let error = project_model_access_request(
        request,
        &CanonicalProjectionInputs {
            registration: &registration(),
            lease: &lease(),
            policy: &policy(sha256_bytes(b"policy")),
            subject: &subject(),
            window: &window(),
            profile: &profile(),
            signer: &FailingSigner,
        },
        )
    .expect_err("signer failure must propagate");
    assert_eq!(error, CloudError::IdentityUnavailable);
}

#[test]
fn canonical_valid_fixture_parses_into_the_projection_type() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../packages/contracts/fixtures/valid/model-access-request.json"
    ))
    .expect("valid fixture JSON");
    let parsed: CanonicalModelAccessRequest =
        serde_json::from_value(fixture.clone()).expect("canonical fixture must deserialize");
    let reserialized = serde_json::to_value(&parsed).expect("reserialize");
    assert_eq!(reserialized, fixture, "fixture round trip must be lossless");
}
