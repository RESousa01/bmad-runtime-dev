#![allow(
    clippy::expect_used,
    reason = "invalid static fixtures should fail the test at construction"
)]

#[cfg(feature = "deterministic-fake")]
use desktop_cloud::{
    verify_dispatched_model_response, CanonicalOutputValidator, DeterministicModelTransport,
    ModelAccessReceipt, ReceiptVerifier,
};
use desktop_cloud::{AuthorizedModelRequest, CloudError, OfflineModelTransport};
use desktop_egress::{
    ApproveDecisionInput, ConsentService, ConsumeDecisionInput, ContextCandidate,
    ContextClassification, ContextPreparer, EgressLimits, MemoryDecisionLedger,
    ModelInvocationBindingDraft, PatternSecretScanner, PrepareContextInput, RetentionMode,
};
#[cfg(feature = "deterministic-fake")]
use desktop_runtime::Sha256Digest;
use desktop_runtime::{sha256_bytes, ContractId, RelativeWorkspacePath, UnixMillis};
#[cfg(feature = "deterministic-fake")]
use serde_json::Value;

fn id(value: &str) -> ContractId {
    ContractId::new(value).expect("valid fixture identifier")
}

fn authorized_fixture() -> AuthorizedModelRequest {
    let manifest = ContextPreparer::new(PatternSecretScanner)
        .prepare(PrepareContextInput {
            tenant_ref: id("tenant_ref"),
            project_ref: id("project_ref"),
            run_ref: id("run_ref"),
            purpose: "planning".to_owned(),
            model_role: "planner".to_owned(),
            canonical_output_schema_id: id("planning_output_v1"),
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
                client_item_id: id("item_notes"),
                relative_label: RelativeWorkspacePath::new("notes.txt").expect("fixture path"),
                semantic_role: "source".to_owned(),
                language: Some("text".to_owned()),
                classification: ContextClassification::Internal,
                content: "safe context".to_owned(),
            }],
            exclusions: Vec::new(),
        })
        .expect("manifest");
    let binding = ModelInvocationBindingDraft {
        schema_version: "sapphirus.model-invocation-binding.v1".to_owned(),
        request_id: id("request_001"),
        tenant_ref: manifest.draft.tenant_ref.clone(),
        project_ref: manifest.draft.project_ref.clone(),
        run_ref: manifest.draft.run_ref.clone(),
        installation_id: id("installation_001"),
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
            decision_id: id("decision_001"),
            issued_at: UnixMillis(1_500),
            expires_at: UnixMillis(31_500),
        })
        .expect("decision");
    let consumption = service
        .consume(ConsumeDecisionInput {
            decision: &decision,
            binding: &binding,
            invocation_id: id("invocation_001"),
            consumed_at: UnixMillis(2_000),
        })
        .expect("consumption");

    AuthorizedModelRequest::new(&manifest, &binding, consumption).expect("authorized request")
}

#[test]
fn offline_composition_remains_fail_closed_without_a_fallback() {
    assert!(matches!(
        OfflineModelTransport.send(authorized_fixture()),
        Err(CloudError::Offline)
    ));
}

#[cfg(feature = "deterministic-fake")]
struct KnownSchema;

#[cfg(feature = "deterministic-fake")]
impl CanonicalOutputValidator for KnownSchema {
    fn validate(
        &self,
        _schema_id: &ContractId,
        _schema_hash: Sha256Digest,
        payload: &Value,
    ) -> Result<(), CloudError> {
        if payload.is_object() {
            Ok(())
        } else {
            Err(CloudError::InvalidModelOutput)
        }
    }
}

#[cfg(feature = "deterministic-fake")]
struct DeterministicReceipt;

#[cfg(feature = "deterministic-fake")]
impl ReceiptVerifier for DeterministicReceipt {
    fn verify(&self, receipt: &ModelAccessReceipt) -> Result<(), CloudError> {
        if receipt.proof == "deterministic-fake-no-trust" {
            Ok(())
        } else {
            Err(CloudError::ReceiptInvalid)
        }
    }
}

#[cfg(feature = "deterministic-fake")]
#[test]
fn deterministic_fixture_is_caller_supplied_bound_and_normally_verified() {
    let fixture = "{\n  \"second\": \"\\u00e9\",\n  \"first\": \"é\"\n}";
    let (dispatched, response) = DeterministicModelTransport
        .send_fixture(authorized_fixture(), fixture.to_owned(), UnixMillis(2_200))
        .expect("bounded deterministic fixture");

    assert_eq!(response.payload_json, fixture);
    assert_eq!(response.payload_hash, sha256_bytes(fixture.as_bytes()));
    assert_eq!(response.receipt.result_hash, response.payload_hash);
    assert_eq!(
        response.receipt.output_bytes,
        u64::try_from(fixture.len()).expect("bounded fixture length")
    );

    let verified =
        verify_dispatched_model_response(dispatched, response, &KnownSchema, &DeterministicReceipt)
            .expect("normal verification path");
    assert_eq!(verified.payload_bytes(), fixture.as_bytes());
    assert_eq!(verified.payload()["second"], "é");
}

#[cfg(feature = "deterministic-fake")]
#[test]
fn deterministic_fixture_rejects_an_output_over_the_response_bound() {
    let oversized = "x".repeat(1024 * 1024 + 1);

    assert!(matches!(
        DeterministicModelTransport.send_fixture(
            authorized_fixture(),
            oversized,
            UnixMillis(2_200)
        ),
        Err(CloudError::InvalidModelOutput)
    ));
}
