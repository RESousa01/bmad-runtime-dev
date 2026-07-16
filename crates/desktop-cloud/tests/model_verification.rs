#![allow(
    clippy::expect_used,
    reason = "invalid static fixtures should fail the test at construction"
)]

use async_trait::async_trait;
#[cfg(feature = "deterministic-fake")]
use desktop_cloud::DeterministicModelTransport;
use desktop_cloud::{
    verify_dispatched_model_response, AuthorizedModelRequest, BrokerToken,
    CanonicalOutputValidator, CloudError, CloudSession, DispatchedModelRequest, EntitlementLease,
    EntitlementProofVerifier, EntitlementVerifier, HttpExecutor, HttpResponse, IdentityBroker,
    ModelAccessReceipt, ModelReceiptStatus, OfflineModelTransport, OutboundHttpRequest,
    RawModelOutput, ReceiptClock, ReceiptProofVerifier, ReceiptVerifier, ReplaySafeReceiptVerifier,
    SupportApiOrigin, SupportApiTransport, VerifiedEntitlement,
};
use desktop_egress::{
    ApproveDecisionInput, ConsentService, ConsumeDecisionInput, ContextCandidate,
    ContextClassification, ContextPreparer, EgressLimits, MemoryDecisionLedger,
    ModelInvocationBindingDraft, PatternSecretScanner, PrepareContextInput, RetentionMode,
};
use desktop_runtime::{sha256_bytes, ContractId, RelativeWorkspacePath, Sha256Digest, UnixMillis};
use semver::Version;
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

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

fn raw_response_for(request: &AuthorizedModelRequest) -> RawModelOutput {
    let payload_json = r#"{"summary":"bounded plan"}"#.to_owned();
    let payload_hash = sha256_bytes(payload_json.as_bytes());
    RawModelOutput {
        request_id: request.request_id().clone(),
        output_schema_id: request.canonical_output_schema_id().clone(),
        payload_json,
        payload_hash,
        receipt: ModelAccessReceipt {
            schema_version: "sapphirus.model-access-receipt.v1".to_owned(),
            receipt_id: id("receipt_001"),
            request_id: request.request_id().clone(),
            request_hash: request.request_hash(),
            result_hash: payload_hash,
            manifest_hash: request.manifest_hash(),
            binding_hash: request.binding_hash(),
            consumption_hash: request.consumption_hash(),
            consent_disclosure_hash: request.consent_disclosure_hash(),
            provider_profile_hash: request.provider_profile_hash(),
            model_profile_hash: request.model_profile_hash(),
            deployment_hash: request.deployment_hash(),
            retention_mode: request.retention_mode(),
            region: request.region().to_owned(),
            input_bytes: request.total_outbound_bytes(),
            output_bytes: 26,
            started_at: UnixMillis(2_100),
            completed_at: UnixMillis(2_200),
            status: ModelReceiptStatus::Succeeded,
            proof: "test-proof".to_owned(),
        },
    }
}

struct StaticBroker;

#[async_trait]
impl IdentityBroker for StaticBroker {
    async fn acquire_token(&self) -> Result<BrokerToken, CloudError> {
        BrokerToken::new(
            "transport-secret".to_owned(),
            id("tenant_ref"),
            id("account_ref"),
            UnixMillis(60_000),
        )
    }

    async fn sign_out(&self) -> Result<(), CloudError> {
        Ok(())
    }
}

struct KnownEntitlementProof;

impl EntitlementProofVerifier for KnownEntitlementProof {
    fn verify(&self, lease: &EntitlementLease) -> Result<(), CloudError> {
        if lease.signature == "valid-proof" {
            Ok(())
        } else {
            Err(CloudError::EntitlementUnavailable)
        }
    }
}

fn verified_entitlement() -> VerifiedEntitlement {
    let policy_hash = sha256_bytes(b"policy");
    EntitlementVerifier::new(
        KnownEntitlementProof,
        "dreg_0123456789abcdef0123456789",
        sha256_bytes(b"subject"),
        policy_hash,
        "model_access",
        Version::parse("0.1.0-beta.1").expect("version"),
    )
    .expect("entitlement verifier")
    .verify(
        &EntitlementLease {
            schema_version: "desktop-entitlement-lease.v1".to_owned(),
            lease_id: "lease_001".to_owned(),
            registration_id: "dreg_0123456789abcdef0123456789".to_owned(),
            subject_hash: sha256_bytes(b"subject").to_string(),
            delivery_model: "windows_local".to_owned(),
            issued_at: "1970-01-01T00:00:00Z".to_owned(),
            not_before: "1970-01-01T00:00:00Z".to_owned(),
            expires_at: "1970-01-02T00:00:00Z".to_owned(),
            offline_grace_ends_at: "1970-01-03T00:00:00Z".to_owned(),
            features: vec!["model_access".to_owned()],
            tenant_policy_hash: policy_hash.to_string(),
            minimum_client_version: "0.1.0-beta.1".to_owned(),
            key_id: "test-key".to_owned(),
            signature: "valid-proof".to_owned(),
        },
        UnixMillis(1_000),
    )
    .expect("verified entitlement")
}

struct StaticResponseExecutor(RawModelOutput);

#[async_trait]
impl HttpExecutor for StaticResponseExecutor {
    async fn execute(&self, _request: OutboundHttpRequest) -> Result<HttpResponse, CloudError> {
        let body = serde_json::to_vec(&self.0).map_err(|_| CloudError::TransportFailed)?;
        Ok(HttpResponse::new(
            200,
            Some("application/json".to_owned()),
            Some(u64::try_from(body.len()).map_err(|_| CloudError::TransportFailed)?),
            body,
        ))
    }
}

async fn dispatch(
    request: AuthorizedModelRequest,
    response: RawModelOutput,
) -> (DispatchedModelRequest, RawModelOutput) {
    let transport = SupportApiTransport::new(
        SupportApiOrigin::new("https://support.example.com").expect("origin"),
        StaticResponseExecutor(response),
    );
    let session = CloudSession::new(StaticBroker, id("tenant_ref"));
    let access = session
        .acquire_access(UnixMillis(1_000))
        .await
        .expect("access");
    transport
        .send(
            &session,
            &access,
            &verified_entitlement(),
            request,
            UnixMillis(1_000),
        )
        .await
        .expect("dispatch")
}

async fn dispatched_response(
    mutate: impl FnOnce(&mut RawModelOutput),
) -> (DispatchedModelRequest, RawModelOutput) {
    let request = authorized_fixture();
    let mut response = raw_response_for(&request);
    mutate(&mut response);
    dispatch(request, response).await
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

#[cfg(feature = "deterministic-fake")]
struct FakeReceipt;

#[cfg(feature = "deterministic-fake")]
impl ReceiptVerifier for FakeReceipt {
    fn verify(&self, receipt: &ModelAccessReceipt) -> Result<(), CloudError> {
        if receipt.proof == "deterministic-fake-no-trust" {
            Ok(())
        } else {
            Err(CloudError::ReceiptInvalid)
        }
    }
}

struct KnownReceiptProof;

impl ReceiptProofVerifier for KnownReceiptProof {
    fn verify_proof(&self, receipt: &ModelAccessReceipt) -> Result<(), CloudError> {
        if receipt.proof == "test-proof" {
            Ok(())
        } else {
            Err(CloudError::ReceiptInvalid)
        }
    }
}

struct FixedReceiptClock(UnixMillis);

impl ReceiptClock for FixedReceiptClock {
    fn now(&self) -> Result<UnixMillis, CloudError> {
        Ok(self.0)
    }
}

struct SharedReceiptClock(Arc<AtomicU64>);

impl ReceiptClock for SharedReceiptClock {
    fn now(&self) -> Result<UnixMillis, CloudError> {
        Ok(UnixMillis(self.0.load(Ordering::SeqCst)))
    }
}

#[test]
fn authorized_request_contains_only_consumed_outbound_context() {
    let request = authorized_fixture();

    assert_eq!(request.request_id(), &id("request_001"));
    assert_eq!(request.items()[0].relative_label.as_str(), "notes.txt");
    assert_eq!(request.items()[0].content, "safe context");
    assert_eq!(
        request.items()[0].content_hash,
        sha256_bytes(b"safe context")
    );
}

#[test]
fn offline_composition_never_dispatches_or_falls_back() {
    assert!(matches!(
        OfflineModelTransport.send(authorized_fixture()),
        Err(CloudError::Offline)
    ));
}

#[cfg(feature = "deterministic-fake")]
#[test]
fn deterministic_composition_is_explicit_and_returns_a_dispatched_capability() {
    let request = authorized_fixture();
    let (dispatched, response) = DeterministicModelTransport
        .send(request, UnixMillis(2_200))
        .expect("explicit deterministic output");

    let verified =
        verify_dispatched_model_response(dispatched, response, &KnownSchema, &FakeReceipt)
            .expect("verified deterministic output");
    assert_eq!(
        verified.payload["summary"],
        "Deterministic planning preview"
    );
}

#[tokio::test]
async fn valid_typed_response_and_receipt_are_verified() {
    let request = authorized_fixture();
    let response = raw_response_for(&request);
    let expected_request_id = request.request_id().clone();
    let (dispatched, response) = dispatch(request, response).await;

    let output =
        verify_dispatched_model_response(dispatched, response, &KnownSchema, &KnownReceipt)
            .expect("verified output");

    assert_eq!(output.request_id, expected_request_id);
    assert_eq!(output.payload["summary"], "bounded plan");
}

#[tokio::test]
async fn request_payload_and_receipt_substitutions_fail_closed() {
    let (dispatched, wrong_request) =
        dispatched_response(|response| response.request_id = id("request_other")).await;
    assert!(matches!(
        verify_dispatched_model_response(dispatched, wrong_request, &KnownSchema, &KnownReceipt),
        Err(CloudError::ResponseBindingMismatch)
    ));

    let (dispatched, changed_payload) = dispatched_response(|response| {
        response.payload_json = r#"{"summary":"changed"}"#.to_owned();
    })
    .await;
    assert!(matches!(
        verify_dispatched_model_response(dispatched, changed_payload, &KnownSchema, &KnownReceipt),
        Err(CloudError::InvalidModelOutput)
    ));

    let (dispatched, wrong_schema) = dispatched_response(|response| {
        response.output_schema_id = id("other_schema");
    })
    .await;
    assert!(matches!(
        verify_dispatched_model_response(dispatched, wrong_schema, &KnownSchema, &KnownReceipt),
        Err(CloudError::ResponseBindingMismatch)
    ));

    let (dispatched, changed_manifest) = dispatched_response(|response| {
        response.receipt.manifest_hash = sha256_bytes(b"other-manifest");
    })
    .await;
    assert!(matches!(
        verify_dispatched_model_response(dispatched, changed_manifest, &KnownSchema, &KnownReceipt),
        Err(CloudError::ResponseBindingMismatch)
    ));

    let (dispatched, bad_proof) = dispatched_response(|response| {
        response.receipt.proof = "forged".to_owned();
    })
    .await;
    assert!(matches!(
        verify_dispatched_model_response(dispatched, bad_proof, &KnownSchema, &KnownReceipt),
        Err(CloudError::ReceiptInvalid)
    ));
}

#[tokio::test]
async fn receipt_profile_region_retention_and_consumption_drift_fail_closed() {
    let (dispatched, profile) = dispatched_response(|response| {
        response.receipt.model_profile_hash = sha256_bytes(b"other-profile");
    })
    .await;
    assert!(matches!(
        verify_dispatched_model_response(dispatched, profile, &KnownSchema, &KnownReceipt),
        Err(CloudError::ResponseBindingMismatch)
    ));

    let (dispatched, provider) = dispatched_response(|response| {
        response.receipt.provider_profile_hash = sha256_bytes(b"other-provider");
    })
    .await;
    assert!(matches!(
        verify_dispatched_model_response(dispatched, provider, &KnownSchema, &KnownReceipt),
        Err(CloudError::ResponseBindingMismatch)
    ));

    let (dispatched, deployment) = dispatched_response(|response| {
        response.receipt.deployment_hash = sha256_bytes(b"other-deployment");
    })
    .await;
    assert!(matches!(
        verify_dispatched_model_response(dispatched, deployment, &KnownSchema, &KnownReceipt),
        Err(CloudError::ResponseBindingMismatch)
    ));

    let (dispatched, region) = dispatched_response(|response| {
        response.receipt.region = "westus".to_owned();
    })
    .await;
    assert!(matches!(
        verify_dispatched_model_response(dispatched, region, &KnownSchema, &KnownReceipt),
        Err(CloudError::ResponseBindingMismatch)
    ));

    let (dispatched, consumption) = dispatched_response(|response| {
        response.receipt.consumption_hash = sha256_bytes(b"other-consumption");
    })
    .await;
    assert!(matches!(
        verify_dispatched_model_response(dispatched, consumption, &KnownSchema, &KnownReceipt),
        Err(CloudError::ResponseBindingMismatch)
    ));

    let request = authorized_fixture();
    let serialized = serde_json::to_string(&raw_response_for(&request)).expect("response json");
    let invalid_retention = serialized.replace("transient_no_store", "provider_store");
    assert!(serde_json::from_str::<RawModelOutput>(&invalid_retention).is_err());
}

#[test]
fn receipt_freshness_and_replay_are_enforced_atomically() {
    let request = authorized_fixture();
    let response = raw_response_for(&request);
    let now = Arc::new(AtomicU64::new(2_300));
    let verifier = ReplaySafeReceiptVerifier::new(
        KnownReceiptProof,
        SharedReceiptClock(Arc::clone(&now)),
        1_000,
        100,
    )
    .expect("receipt verifier");

    verifier.verify(&response.receipt).expect("fresh receipt");
    assert_eq!(
        verifier.verify(&response.receipt),
        Err(CloudError::ReceiptInvalid)
    );

    now.store(4_000, Ordering::SeqCst);
    let mut reused_after_window = response.clone();
    reused_after_window.receipt.started_at = UnixMillis(3_900);
    reused_after_window.receipt.completed_at = UnixMillis(3_950);
    verifier
        .verify(&reused_after_window.receipt)
        .expect("expired replay entry was pruned before bounded insertion");

    let expired = ReplaySafeReceiptVerifier::new(
        KnownReceiptProof,
        FixedReceiptClock(UnixMillis(10_000)),
        1_000,
        100,
    )
    .expect("receipt verifier");
    assert_eq!(
        expired.verify(&response.receipt),
        Err(CloudError::ReceiptInvalid)
    );

    let mut future_receipt = response.receipt;
    future_receipt.started_at = UnixMillis(3_000);
    future_receipt.completed_at = UnixMillis(3_100);
    let future = ReplaySafeReceiptVerifier::new(
        KnownReceiptProof,
        FixedReceiptClock(UnixMillis(2_300)),
        1_000,
        100,
    )
    .expect("receipt verifier");
    assert_eq!(
        future.verify(&future_receipt),
        Err(CloudError::ReceiptInvalid)
    );
}
