//! Scripted-server production round-trip lifecycle (readiness Task 9).
//!
//! The scripted executor plays the deployed support plane: it asserts the
//! exact reviewed stage order, serves signed documents minted with a test
//! proof key, and returns a receipt whose proof binds to the exact
//! request. Fail-closed variants prove that a tampered policy stops the
//! sequence before the lease, and that a replayed receipt is rejected.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
#![cfg(windows)]

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use desktop_cloud::{
    AuthorizedModelRequest, BrokerToken, CanonicalReceiptProof, CloudError, CloudSession,
    HttpExecutor, HttpResponse, IdentityBroker, InstallationConsentSigner as _,
    OutboundHttpRequest, PinnedProofKey, ProductionRoundTrip, ProductionSupportClient,
    ProductionSupportConfig, ProofKeyRing, RegistrationMaterial, SignedStateStore,
    SupportApiOrigin, SupportApiTransport, WindowsInstallationIdentity,
};
use desktop_egress::{
    ApproveDecisionInput, ConsentService, ConsumeDecisionInput, ContextCandidate,
    ContextClassification, ContextPreparer, EgressLimits, MemoryDecisionLedger,
    ModelInvocationBindingDraft, PatternSecretScanner, PrepareContextInput, RetentionMode,
};
use desktop_runtime::{
    canonical_hash, canonical_hash_without_field, sha256_bytes, ContractId, UnixMillis,
};
use serde_json::json;

const TENANT: &str = "11111111-1111-1111-1111-111111111111";
const CLIENT: &str = "22222222-2222-2222-2222-222222222222";
const SCOPE: &str = "api://22222222-2222-2222-2222-222222222222/.default";
const ORIGIN: &str = "https://support.example.test";
const ISSUER: &str = "https://login.microsoftonline.com/11111111-1111-1111-1111-111111111111/v2.0";
const AUDIENCE: &str = "api://22222222-2222-2222-2222-222222222222";
const REGISTRATION_ID: &str = "dreg_0123456789abcdef0123456789";

fn id(value: &str) -> ContractId {
    ContractId::new(value).expect("fixture identifier")
}

#[derive(Default)]
struct MemoryStateStore {
    values: Mutex<std::collections::HashMap<String, String>>,
}

impl SignedStateStore for MemoryStateStore {
    fn load(&self, name: &str) -> Option<String> {
        self.values.lock().expect("state lock").get(name).cloned()
    }

    fn save(&self, name: &str, value: &str) {
        self.values
            .lock()
            .expect("state lock")
            .insert(name.to_owned(), value.to_owned());
    }

    fn clear(&self) {
        self.values.lock().expect("state lock").clear();
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
            UnixMillis(1_790_000_000_000),
        )
    }

    async fn sign_out(&self) -> Result<(), CloudError> {
        Ok(())
    }
}

struct SignerFixture {
    identity: Arc<WindowsInstallationIdentity>,
    key_id: String,
}

impl SignerFixture {
    fn create() -> Self {
        let identity = WindowsInstallationIdentity::open_or_create(&format!(
            "sapphirus-test-roundtrip-{:016x}",
            rand::random::<u64>(),
        ))
        .expect("test proof key");
        let key_id = identity.key_id().to_owned();
        Self {
            identity: Arc::new(identity),
            key_id,
        }
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
        ProductionSupportConfig::new(
            TENANT,
            CLIENT,
            SCOPE,
            ORIGIN,
            "westeurope",
            self.ring(),
            self.ring(),
            ISSUER,
            AUDIENCE,
        )
        .expect("production configuration")
    }

    fn policy_json(&self) -> serde_json::Value {
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
            .expect("sign policy");
        json!({
            "schemaVersion": "desktop-policy.v1",
            "policyId": "policy_01J00000000000000000000000",
            "policyVersion": 7,
            "policyHash": digest.to_string(),
            "systemBrowserFallbackAllowed": false,
            "maximumContextBytes": 524_288,
            "maximumContextItems": 64,
            "allowedRegions": ["westeurope"],
            "keyId": self.key_id,
            "signature": signature,
        })
    }

    fn lease_json(&self, policy_hash: &str, subject_hash: &str) -> serde_json::Value {
        let draft = json!({
            "schemaVersion": "desktop-entitlement-lease.v1",
            "leaseId": "lease_01J00000000000000000000000",
            "registrationId": REGISTRATION_ID,
            "subjectHash": subject_hash,
            "deliveryModel": "windows_local",
            "issuedAt": "2026-07-20T10:00:00.000Z",
            "notBefore": "2026-07-20T09:58:00.000Z",
            "expiresAt": "2026-07-21T10:00:00.000Z",
            "offlineGraceEndsAt": "2026-07-24T10:00:00.000Z",
            "features": ["local_runtime", "model_access"],
            "tenantPolicyHash": policy_hash,
            "minimumClientVersion": "0.1.0-beta.1",
        });
        let digest = canonical_hash("entitlement-lease", 1, &draft).expect("lease digest");
        let signature = self
            .identity
            .sign_digest(digest.as_bytes())
            .expect("sign lease");
        json!({
            "schemaVersion": "desktop-entitlement-lease.v1",
            "leaseId": "lease_01J00000000000000000000000",
            "registrationId": REGISTRATION_ID,
            "subjectHash": subject_hash,
            "deliveryModel": "windows_local",
            "issuedAt": "2026-07-20T10:00:00+00:00",
            "notBefore": "2026-07-20T09:58:00+00:00",
            "expiresAt": "2026-07-21T10:00:00+00:00",
            "offlineGraceEndsAt": "2026-07-24T10:00:00+00:00",
            "features": ["local_runtime", "model_access"],
            "tenantPolicyHash": policy_hash,
            "minimumClientVersion": "0.1.0-beta.1",
            "keyId": self.key_id,
            "signature": signature,
        })
    }

    fn delete(self) {
        if let Ok(identity) = Arc::try_unwrap(self.identity) {
            identity.delete().expect("delete test key");
        }
    }
}

fn authorized_fixture(policy_hash: desktop_runtime::Sha256Digest) -> AuthorizedModelRequest {
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
            policy_hash,
            region: "westeurope".to_owned(),
            retention_mode: RetentionMode::TransientNoStore,
            created_at: UnixMillis(1_000),
            expires_at: UnixMillis(601_000),
            limits: EgressLimits {
                maximum_context_items: 8,
                maximum_context_bytes: 64 * 1024,
                maximum_token_estimate: 16_000,
            },
            candidates: vec![ContextCandidate {
                client_item_id: id("item_notes"),
                relative_label: desktop_runtime::RelativeWorkspacePath::new("notes.txt")
                    .expect("fixture path"),
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
            expires_at: UnixMillis(301_500),
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

/// Plays the deployed support plane: serves each stage in the reviewed
/// order and signs the receipt for the exact incoming request.
type ReceiptSigner = Box<dyn Fn(&[u8; 32]) -> String + Send + Sync>;

struct ScriptedServer {
    signer_key_id: String,
    sign: ReceiptSigner,
    policy_json: serde_json::Value,
    lease_json: serde_json::Value,
    calls: Mutex<Vec<String>>,
    tamper_policy: bool,
}

#[async_trait]
impl HttpExecutor for ScriptedServer {
    async fn execute(&self, request: OutboundHttpRequest) -> Result<HttpResponse, CloudError> {
        let path = request.url().path().to_owned();
        self.calls.lock().expect("calls lock").push(path.clone());
        let body = match path.as_str() {
            "/desktop/v1/bootstrap" => json!({
                "schemaVersion": "sapphirus.desktop-bootstrap.v1",
                "region": "westeurope",
                "contractEpoch": "1",
                "minimumClientContractEpoch": "1",
                "capabilities": ["windows_local", "transient_no_store"],
                "serverTime": "2026-07-20T10:00:00.000Z",
            }),
            "/desktop/v1/devices/registrations" => json!({
                "schemaVersion": "desktop-device-registration.v1",
                "registrationId": REGISTRATION_ID,
                "status": "active",
                "createdAt": "2026-07-20T10:00:00.000Z",
            }),
            "/desktop/v1/policy/current" => {
                let mut policy = self.policy_json.clone();
                if self.tamper_policy {
                    policy["maximumContextItems"] = json!(999);
                }
                policy
            }
            "/desktop/v1/entitlements/leases" => self.lease_json.clone(),
            "/desktop/v1/model-access/calls" => {
                let request_value: serde_json::Value =
                    serde_json::from_slice(request.body()).expect("model request body");
                let payload_json = "{\"summary\":\"scripted result\"}".to_owned();
                let payload_hash = sha256_bytes(payload_json.as_bytes());
                let mut receipt = json!({
                    "schemaVersion": "sapphirus.model-access-receipt.v1",
                    "receiptId": "receipt_01J00000000000000000000001",
                    "requestId": request_value["requestId"],
                    "requestHash": request_value["requestHash"],
                    "resultHash": payload_hash.to_string(),
                    "manifestHash": request_value["manifestHash"],
                    "bindingHash": request_value["bindingHash"],
                    "consumptionHash": request_value["consumptionHash"],
                    "consentDisclosureHash": request_value["consentDisclosureHash"],
                    "providerProfileHash": request_value["providerProfileHash"],
                    "modelProfileHash": request_value["modelProfileHash"],
                    "deploymentHash": request_value["deploymentHash"],
                    "retentionMode": request_value["retentionMode"],
                    "region": request_value["region"],
                    "inputBytes": request_value["totalOutboundBytes"],
                    "outputBytes": payload_json.len(),
                    "startedAt": 10_000,
                    "completedAt": 11_000,
                    "status": "succeeded",
                    "proof": "",
                });
                let digest =
                    canonical_hash_without_field("model-access-receipt", 1, &receipt, "proof")
                        .expect("receipt digest");
                let proof = CanonicalReceiptProof {
                    proof_type: "support_plane_signature".to_owned(),
                    algorithm: "ES256".to_owned(),
                    issuer: ISSUER.to_owned(),
                    audience: AUDIENCE.to_owned(),
                    key_id: self.signer_key_id.clone(),
                    signed_payload_hash: digest.to_string(),
                    signature: (self.sign)(digest.as_bytes()),
                };
                receipt["proof"] = json!(serde_json::to_string(&proof).expect("proof json"));
                json!({
                    "requestId": request_value["requestId"],
                    "outputSchemaId": request_value["canonicalOutputSchemaId"],
                    "payloadJson": payload_json,
                    "payloadHash": payload_hash.to_string(),
                    "receipt": receipt,
                })
            }
            other => panic!("unexpected route {other}"),
        };
        let bytes = serde_json::to_vec(&body).expect("scripted body");
        Ok(HttpResponse::new(
            200,
            Some("application/json".to_owned()),
            Some(bytes.len() as u64),
            bytes,
        ))
    }
}

fn round_trip(
    fixture: &SignerFixture,
    tamper_policy: bool,
) -> (
    ProductionRoundTrip<StaticBroker, ScriptedServer>,
    desktop_runtime::Sha256Digest,
) {
    let policy_json = fixture.policy_json();
    let policy_hash_text = policy_json["policyHash"].as_str().expect("policy hash");
    let policy_hash =
        desktop_runtime::Sha256Digest::parse(policy_hash_text).expect("policy digest");
    let config = fixture.config();
    let subject_hash = sha256_bytes(b"subject").to_string();
    let lease_json = fixture.lease_json(policy_hash_text, &subject_hash);
    let signer = Arc::clone(&fixture.identity);
    let server = ScriptedServer {
        signer_key_id: fixture.key_id.clone(),
        sign: Box::new(move |digest| signer.sign_digest(digest).expect("sign receipt")),
        policy_json,
        lease_json,
        calls: Mutex::new(Vec::new()),
        tamper_policy,
    };
    let session = CloudSession::new(StaticBroker, id("tenant_ref"));
    let client = ProductionSupportClient::new(config, Box::new(MemoryStateStore::default()));
    let transport =
        SupportApiTransport::new(SupportApiOrigin::new(ORIGIN).expect("origin"), server);
    let registration = RegistrationMaterial {
        installation_public_key: fixture.identity.public_key_base64url(),
        installation_public_key_hash: sha256_bytes(fixture.identity.public_key_spki()).to_string(),
        client_release: "0.1.0-beta.1".to_owned(),
        platform: "windows".to_owned(),
        architecture: "x86_64".to_owned(),
        tenant_policy_version: 7,
    };
    (
        ProductionRoundTrip::new(session, client, transport, registration),
        policy_hash,
    )
}

#[tokio::test]
async fn the_full_lifecycle_completes_in_the_reviewed_order() {
    let fixture = SignerFixture::create();
    let (round_trip, policy_hash) = round_trip(&fixture, false);
    let request = authorized_fixture(policy_hash);

    let (_dispatched, output) = round_trip
        .send(request, UnixMillis(1_784_543_400_000))
        .await
        .expect("full round trip");
    assert_eq!(output.payload_json, "{\"summary\":\"scripted result\"}");

    let calls = round_trip
        .transport_for_test()
        .executor_for_test()
        .calls
        .lock()
        .expect("calls")
        .clone();
    assert_eq!(
        calls,
        vec![
            "/desktop/v1/bootstrap".to_owned(),
            "/desktop/v1/devices/registrations".to_owned(),
            "/desktop/v1/policy/current".to_owned(),
            "/desktop/v1/entitlements/leases".to_owned(),
            "/desktop/v1/model-access/calls".to_owned(),
        ],
    );
    fixture.delete();
}

#[tokio::test]
async fn a_tampered_policy_stops_the_sequence_before_the_lease() {
    let fixture = SignerFixture::create();
    let (round_trip, policy_hash) = round_trip(&fixture, true);
    let request = authorized_fixture(policy_hash);

    assert!(round_trip
        .send(request, UnixMillis(1_784_543_400_000))
        .await
        .is_err());
    let calls = round_trip
        .transport_for_test()
        .executor_for_test()
        .calls
        .lock()
        .expect("calls")
        .clone();
    assert_eq!(
        calls,
        vec![
            "/desktop/v1/bootstrap".to_owned(),
            "/desktop/v1/devices/registrations".to_owned(),
            "/desktop/v1/policy/current".to_owned(),
        ],
        "no lease or model call may follow a rejected policy"
    );
    fixture.delete();
}

#[tokio::test]
async fn a_replayed_receipt_id_is_rejected() {
    let fixture = SignerFixture::create();
    let (round_trip, policy_hash) = round_trip(&fixture, false);

    let first = authorized_fixture(policy_hash);
    round_trip
        .send(first, UnixMillis(1_784_543_400_000))
        .await
        .expect("first round trip");
    // The scripted server reuses the same receipt id; the client's replay
    // protection must reject the second completion.
    let second = authorized_fixture(policy_hash);
    assert!(matches!(
        round_trip.send(second, UnixMillis(1_784_543_401_000)).await,
        Err(CloudError::ReceiptInvalid)
    ));
    fixture.delete();
}
