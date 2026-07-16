#![allow(
    clippy::expect_used,
    reason = "invalid static fixtures should fail the test at construction"
)]

use std::sync::Arc;

use async_trait::async_trait;
use desktop_cloud::{
    AuthorizedModelRequest, BrokerToken, CloudError, CloudSession, EntitlementLease,
    EntitlementProofVerifier, EntitlementVerifier, HttpExecutor, HttpResponse, IdentityBroker,
    OutboundHttpRequest, RawModelOutput, SupportApiOrigin, SupportApiTransport,
    VerifiedEntitlement,
};
use desktop_egress::{
    ApproveDecisionInput, ConsentService, ConsumeDecisionInput, ContextCandidate,
    ContextClassification, ContextPreparer, EgressLimits, MemoryDecisionLedger,
    ModelInvocationBindingDraft, PatternSecretScanner, PrepareContextInput, RetentionMode,
};
use desktop_runtime::{sha256_bytes, ContractId, RelativeWorkspacePath, UnixMillis};
use parking_lot::Mutex;
use semver::Version;

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

fn entitlement_for(policy: &[u8], now: UnixMillis) -> VerifiedEntitlement {
    entitlement_with(policy, "model_access", "1970-01-02T00:00:00Z", now)
}

fn entitlement_with(
    policy: &[u8],
    feature: &str,
    expires_at: &str,
    now: UnixMillis,
) -> VerifiedEntitlement {
    let policy_hash = sha256_bytes(policy);
    let lease = EntitlementLease {
        schema_version: "desktop-entitlement-lease.v1".to_owned(),
        lease_id: "lease_001".to_owned(),
        registration_id: "dreg_0123456789abcdef0123456789".to_owned(),
        subject_hash: sha256_bytes(b"subject").to_string(),
        delivery_model: "windows_local".to_owned(),
        issued_at: "1970-01-01T00:00:00Z".to_owned(),
        not_before: "1970-01-01T00:00:00Z".to_owned(),
        expires_at: expires_at.to_owned(),
        offline_grace_ends_at: "1970-01-03T00:00:00Z".to_owned(),
        features: vec![feature.to_owned()],
        tenant_policy_hash: policy_hash.to_string(),
        minimum_client_version: "0.1.0-beta.1".to_owned(),
        key_id: "test-key".to_owned(),
        signature: "valid-proof".to_owned(),
    };
    EntitlementVerifier::new(
        KnownEntitlementProof,
        "dreg_0123456789abcdef0123456789",
        sha256_bytes(b"subject"),
        policy_hash,
        feature,
        Version::parse("0.1.0-beta.1").expect("version"),
    )
    .expect("verifier")
    .verify(&lease, now)
    .expect("verified entitlement")
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

#[derive(Clone, Debug)]
struct RecordedRequest {
    url: String,
    body: Vec<u8>,
    idempotency_key: String,
    safe_debug: String,
}

#[derive(Clone)]
struct RecordingExecutor {
    response: HttpResponse,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
}

#[async_trait]
impl HttpExecutor for RecordingExecutor {
    async fn execute(&self, request: OutboundHttpRequest) -> Result<HttpResponse, CloudError> {
        self.requests.lock().push(RecordedRequest {
            url: request.url().to_string(),
            body: request.body().to_vec(),
            idempotency_key: request.idempotency_key().to_owned(),
            safe_debug: format!("{request:?}"),
        });
        Ok(self.response.clone())
    }
}

fn executor(response: HttpResponse) -> RecordingExecutor {
    RecordingExecutor {
        response,
        requests: Arc::new(Mutex::new(Vec::new())),
    }
}

#[test]
fn support_origin_is_https_only_and_cannot_embed_request_controlled_components() {
    assert!(SupportApiOrigin::new("https://support.example.com").is_ok());
    for invalid in [
        "http://support.example.com",
        "https://user@support.example.com",
        "https://support.example.com/base",
        "https://support.example.com/?tenant=other",
        "https://support.example.com/#fragment",
    ] {
        assert!(
            SupportApiOrigin::new(invalid).is_err(),
            "accepted {invalid}"
        );
    }
}

#[tokio::test]
async fn authorized_request_uses_one_fixed_endpoint_and_secret_safe_boundary() {
    let request = authorized_fixture();
    let raw = serde_json::json!({
        "requestId": request.request_id(),
        "outputSchemaId": request.canonical_output_schema_id(),
        "payloadJson": "{}",
        "payloadHash": sha256_bytes(b"{}"),
        "receipt": {
            "schemaVersion": "sapphirus.model-access-receipt.v1",
            "receiptId": "receipt_001",
            "requestId": "request_001",
            "requestHash": request.request_hash(),
            "resultHash": sha256_bytes(b"{}"),
            "manifestHash": request.manifest_hash(),
            "bindingHash": request.binding_hash(),
            "consumptionHash": request.consumption_hash(),
            "consentDisclosureHash": request.consent_disclosure_hash(),
            "providerProfileHash": request.provider_profile_hash(),
            "modelProfileHash": request.model_profile_hash(),
            "deploymentHash": request.deployment_hash(),
            "retentionMode": "transient_no_store",
            "region": request.region(),
            "inputBytes": request.total_outbound_bytes(),
            "outputBytes": 2,
            "startedAt": 2100,
            "completedAt": 2200,
            "status": "succeeded",
            "proof": "proof"
        }
    });
    let body = serde_json::to_vec(&raw).expect("response fixture");
    let executor = executor(HttpResponse::new(
        200,
        Some("application/json".to_owned()),
        Some(u64::try_from(body.len()).expect("bounded")),
        body,
    ));
    let requests = Arc::clone(&executor.requests);
    let transport = SupportApiTransport::new(
        SupportApiOrigin::new("https://support.example.com").expect("origin"),
        executor,
    );
    let session = CloudSession::new(StaticBroker, id("tenant_ref"));
    let access = session
        .acquire_access(UnixMillis(1_000))
        .await
        .expect("access");
    let entitlement = entitlement_for(b"policy", UnixMillis(1_000));
    let expected_request_id = request.request_id().clone();
    let expected_body = serde_json::to_vec(&request).expect("request serialization");

    let (_dispatched, response): (_, RawModelOutput) = transport
        .send(&session, &access, &entitlement, request, UnixMillis(1_000))
        .await
        .expect("transport response");
    assert_eq!(response.request_id, expected_request_id);
    let recorded = requests.lock();
    assert_eq!(recorded.len(), 1);
    assert_eq!(
        recorded[0].url,
        "https://support.example.com/desktop/v1/model-access/calls"
    );
    assert_eq!(recorded[0].body, expected_body);
    assert_eq!(recorded[0].idempotency_key, "request_001");
    assert!(!recorded[0].safe_debug.contains("transport-secret"));
    assert!(!recorded[0].safe_debug.contains("safe context"));
}

#[tokio::test]
async fn stale_access_status_and_untrusted_body_fail_before_projection() {
    let request = authorized_fixture();
    let executor = executor(HttpResponse::new(
        500,
        Some("application/json".to_owned()),
        Some(2),
        b"{}".to_vec(),
    ));
    let requests = Arc::clone(&executor.requests);
    let transport = SupportApiTransport::new(
        SupportApiOrigin::new("https://support.example.com").expect("origin"),
        executor,
    );
    let session = CloudSession::new(StaticBroker, id("tenant_ref"));
    let access = session
        .acquire_access(UnixMillis(1_000))
        .await
        .expect("access");
    let entitlement = entitlement_for(b"policy", UnixMillis(1_000));
    assert!(matches!(
        transport
            .send(&session, &access, &entitlement, request, UnixMillis(1_000),)
            .await,
        Err(CloudError::TransportFailed)
    ));
    session.sign_out().await.expect("sign out");
    assert!(matches!(
        transport
            .send(
                &session,
                &access,
                &entitlement,
                authorized_fixture(),
                UnixMillis(1_000),
            )
            .await,
        Err(CloudError::SessionInvalidated)
    ));
    assert_eq!(requests.lock().len(), 1);
}

#[tokio::test]
async fn oversized_or_non_json_responses_fail_closed() {
    let session = CloudSession::new(StaticBroker, id("tenant_ref"));
    let access = session
        .acquire_access(UnixMillis(1_000))
        .await
        .expect("access");
    let entitlement = entitlement_for(b"policy", UnixMillis(1_000));
    for response in [
        HttpResponse::new(
            200,
            Some("application/json".to_owned()),
            Some(2 * 1024 * 1024),
            b"{}".to_vec(),
        ),
        HttpResponse::new(200, Some("text/html".to_owned()), Some(2), b"{}".to_vec()),
        HttpResponse::new(
            200,
            Some("application/json".to_owned()),
            Some(8),
            b"not-json".to_vec(),
        ),
        HttpResponse::new(
            200,
            Some("application/json".to_owned()),
            Some(99),
            b"{}".to_vec(),
        ),
    ] {
        let transport = SupportApiTransport::new(
            SupportApiOrigin::new("https://support.example.com").expect("origin"),
            executor(response),
        );
        assert!(transport
            .send(
                &session,
                &access,
                &entitlement,
                authorized_fixture(),
                UnixMillis(1_000),
            )
            .await
            .is_err());
    }
}

#[tokio::test]
async fn expired_access_is_rejected_before_any_context_leaves_the_process() {
    let request = authorized_fixture();
    let executor = executor(HttpResponse::new(
        200,
        Some("application/json".to_owned()),
        Some(2),
        b"{}".to_vec(),
    ));
    let requests = Arc::clone(&executor.requests);
    let transport = SupportApiTransport::new(
        SupportApiOrigin::new("https://support.example.com").expect("origin"),
        executor,
    );
    let session = CloudSession::new(StaticBroker, id("tenant_ref"));
    let access = session
        .acquire_access(UnixMillis(1_000))
        .await
        .expect("access");
    let entitlement = entitlement_for(b"policy", UnixMillis(1_000));

    assert!(matches!(
        transport
            .send(&session, &access, &entitlement, request, UnixMillis(60_000),)
            .await,
        Err(CloudError::ReauthenticationRequired)
    ));
    assert!(requests.lock().is_empty());
}

#[tokio::test]
async fn stale_or_wrong_policy_entitlement_blocks_egress() {
    let executor = executor(HttpResponse::new(
        200,
        Some("application/json".to_owned()),
        Some(2),
        b"{}".to_vec(),
    ));
    let requests = Arc::clone(&executor.requests);
    let transport = SupportApiTransport::new(
        SupportApiOrigin::new("https://support.example.com").expect("origin"),
        executor,
    );
    let session = CloudSession::new(StaticBroker, id("tenant_ref"));
    let access = session
        .acquire_access(UnixMillis(1_000))
        .await
        .expect("access");
    let entitlement = entitlement_for(b"other-policy", UnixMillis(1_000));

    assert!(matches!(
        transport
            .send(
                &session,
                &access,
                &entitlement,
                authorized_fixture(),
                UnixMillis(1_000),
            )
            .await,
        Err(CloudError::EntitlementUnavailable)
    ));

    let wrong_feature = entitlement_with(
        b"policy",
        "local_runtime",
        "1970-01-02T00:00:00Z",
        UnixMillis(1_000),
    );
    assert!(matches!(
        transport
            .send(
                &session,
                &access,
                &wrong_feature,
                authorized_fixture(),
                UnixMillis(1_000),
            )
            .await,
        Err(CloudError::EntitlementUnavailable)
    ));

    let expired = entitlement_with(
        b"policy",
        "model_access",
        "1970-01-01T00:00:10Z",
        UnixMillis(1_000),
    );
    assert!(matches!(
        transport
            .send(
                &session,
                &access,
                &expired,
                authorized_fixture(),
                UnixMillis(10_000),
            )
            .await,
        Err(CloudError::EntitlementUnavailable)
    ));
    assert!(requests.lock().is_empty());
}
