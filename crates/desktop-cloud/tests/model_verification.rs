#![allow(
    clippy::expect_used,
    reason = "invalid static fixtures should fail the test at construction"
)]

use desktop_cloud::{
    verify_model_response, AuthorizedModelRequest, CanonicalOutputValidator, CloudError,
    ModelAccessReceipt, ModelReceiptStatus, RawModelOutput, ReceiptVerifier,
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

    AuthorizedModelRequest::new(&manifest, &binding, &consumption).expect("authorized request")
}

fn raw_response_for(request: &AuthorizedModelRequest) -> RawModelOutput {
    let payload_json = r#"{"summary":"bounded plan"}"#.to_owned();
    let payload_hash = sha256_bytes(payload_json.as_bytes());
    RawModelOutput {
        request_id: request.request_id.clone(),
        output_schema_id: request.canonical_output_schema_id.clone(),
        payload_json,
        payload_hash,
        receipt: ModelAccessReceipt {
            schema_version: "sapphirus.model-access-receipt.v1".to_owned(),
            receipt_id: id("receipt_001"),
            request_id: request.request_id.clone(),
            request_hash: request.request_hash,
            result_hash: payload_hash,
            manifest_hash: request.manifest_hash,
            binding_hash: request.binding_hash,
            consumption_hash: request.consumption_hash,
            consent_disclosure_hash: request.consent_disclosure_hash,
            provider_profile_hash: request.provider_profile_hash,
            model_profile_hash: request.model_profile_hash,
            deployment_hash: request.deployment_hash,
            retention_mode: request.retention_mode,
            region: request.region.clone(),
            input_bytes: request.total_outbound_bytes,
            output_bytes: 26,
            started_at: UnixMillis(2_100),
            completed_at: UnixMillis(2_200),
            status: ModelReceiptStatus::Succeeded,
            proof: "test-proof".to_owned(),
        },
    }
}

struct KnownSchema;

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

struct KnownReceipt;

impl ReceiptVerifier for KnownReceipt {
    fn verify(&self, receipt: &ModelAccessReceipt) -> Result<(), CloudError> {
        if receipt.proof == "test-proof" {
            Ok(())
        } else {
            Err(CloudError::ReceiptInvalid)
        }
    }
}

#[test]
fn authorized_request_contains_only_consumed_outbound_context() {
    let request = authorized_fixture();

    assert_eq!(request.request_id, id("request_001"));
    assert_eq!(request.items[0].relative_label.as_str(), "notes.txt");
    assert_eq!(request.items[0].content, "safe context");
    assert_eq!(request.items[0].content_hash, sha256_bytes(b"safe context"));
}

#[test]
fn valid_typed_response_and_receipt_are_verified() {
    let request = authorized_fixture();
    let response = raw_response_for(&request);

    let output = verify_model_response(&request, response, &KnownSchema, &KnownReceipt)
        .expect("verified output");

    assert_eq!(output.request_id, request.request_id);
    assert_eq!(output.payload["summary"], "bounded plan");
}

#[test]
fn request_payload_and_receipt_substitutions_fail_closed() {
    let request = authorized_fixture();

    let mut wrong_request = raw_response_for(&request);
    wrong_request.request_id = id("request_other");
    assert!(matches!(
        verify_model_response(&request, wrong_request, &KnownSchema, &KnownReceipt),
        Err(CloudError::ResponseBindingMismatch)
    ));

    let mut changed_payload = raw_response_for(&request);
    changed_payload.payload_json = r#"{"summary":"changed"}"#.to_owned();
    assert!(matches!(
        verify_model_response(&request, changed_payload, &KnownSchema, &KnownReceipt),
        Err(CloudError::InvalidModelOutput)
    ));

    let mut wrong_schema = raw_response_for(&request);
    wrong_schema.output_schema_id = id("other_schema");
    assert!(matches!(
        verify_model_response(&request, wrong_schema, &KnownSchema, &KnownReceipt),
        Err(CloudError::ResponseBindingMismatch)
    ));

    let mut changed_manifest = raw_response_for(&request);
    changed_manifest.receipt.manifest_hash = sha256_bytes(b"other-manifest");
    assert!(matches!(
        verify_model_response(&request, changed_manifest, &KnownSchema, &KnownReceipt),
        Err(CloudError::ResponseBindingMismatch)
    ));

    let mut bad_proof = raw_response_for(&request);
    bad_proof.receipt.proof = "forged".to_owned();
    assert!(matches!(
        verify_model_response(&request, bad_proof, &KnownSchema, &KnownReceipt),
        Err(CloudError::ReceiptInvalid)
    ));
}

#[test]
fn receipt_profile_region_retention_and_consumption_drift_fail_closed() {
    let request = authorized_fixture();

    let mut profile = raw_response_for(&request);
    profile.receipt.model_profile_hash = sha256_bytes(b"other-profile");
    assert!(matches!(
        verify_model_response(&request, profile, &KnownSchema, &KnownReceipt),
        Err(CloudError::ResponseBindingMismatch)
    ));

    let mut provider = raw_response_for(&request);
    provider.receipt.provider_profile_hash = sha256_bytes(b"other-provider");
    assert!(matches!(
        verify_model_response(&request, provider, &KnownSchema, &KnownReceipt),
        Err(CloudError::ResponseBindingMismatch)
    ));

    let mut deployment = raw_response_for(&request);
    deployment.receipt.deployment_hash = sha256_bytes(b"other-deployment");
    assert!(matches!(
        verify_model_response(&request, deployment, &KnownSchema, &KnownReceipt),
        Err(CloudError::ResponseBindingMismatch)
    ));

    let mut region = raw_response_for(&request);
    region.receipt.region = "westus".to_owned();
    assert!(matches!(
        verify_model_response(&request, region, &KnownSchema, &KnownReceipt),
        Err(CloudError::ResponseBindingMismatch)
    ));

    let mut consumption = raw_response_for(&request);
    consumption.receipt.consumption_hash = sha256_bytes(b"other-consumption");
    assert!(matches!(
        verify_model_response(&request, consumption, &KnownSchema, &KnownReceipt),
        Err(CloudError::ResponseBindingMismatch)
    ));

    let serialized = serde_json::to_string(&raw_response_for(&request)).expect("response json");
    let invalid_retention = serialized.replace("transient_no_store", "provider_store");
    assert!(serde_json::from_str::<RawModelOutput>(&invalid_retention).is_err());
}
