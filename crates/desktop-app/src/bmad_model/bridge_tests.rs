#![allow(
    clippy::expect_used,
    reason = "invalid static fixtures should fail the test at construction"
)]

use desktop_egress::{
    ApproveDecisionInput, ConsentService, ContextCandidate, ContextClassification,
    ContextEgressManifest, ContextPreparer, DecisionEvidenceInput, EgressLimits,
    MemoryDecisionLedger, ModelInvocationBinding, ModelInvocationBindingDraft,
    PatternSecretScanner, PendingContextDecision, PrepareContextInput, RetentionMode,
};
use desktop_runtime::{
    canonical_hash, sha256_bytes, BmadHelpBindingCompiler, BmadTrustedHelpModelProfile,
    BmadTrustedHelpModelProfileData, ContractId, MethodContextDecision, MethodExactBinding,
    RelativeWorkspacePath, Sha256Digest, UnixMillis,
};
use serde::Serialize;

use super::bridge::{
    bridge_method_context_decision, BmadHelpDecisionBridgeExpectation, BmadModelBridgeError,
};
use crate::bmad_foundation::load_bmad_foundation;

fn id(value: &str) -> ContractId {
    ContractId::new(value).expect("valid fixture identifier")
}

fn foundation_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/bmad-foundation")
}

fn compiled_method_binding(model_seed: u8) -> MethodExactBinding {
    let foundation = load_bmad_foundation(foundation_path()).expect("sealed foundation");
    let model = BmadTrustedHelpModelProfile::from_host_assertion(BmadTrustedHelpModelProfileData {
        provider_id: "azure-openai-managed".to_owned(),
        model_id: "gpt-5.2".to_owned(),
        deployment_id: "sapphirus-help".to_owned(),
        model_profile_hash: sha256_bytes(&[model_seed, 1]),
        model_capability_hash: sha256_bytes(&[model_seed, 2]),
        context_window_profile_hash: sha256_bytes(&[model_seed, 3]),
        egress_profile_hash: sha256_bytes(&[model_seed, 4]),
        request_schema_hash: sha256_bytes(&[model_seed, 5]),
    })
    .expect("trusted model profile");
    BmadHelpBindingCompiler::compile(foundation.help_invocation(), foundation.catalog(), &model)
        .expect("compiled Help invocation")
        .exact_binding()
        .clone()
}

fn manifest_with(contents: [&str; 2]) -> ContextEgressManifest {
    ContextPreparer::new(PatternSecretScanner)
        .prepare(PrepareContextInput {
            tenant_ref: id("tenant_ref"),
            project_ref: id("project_ref"),
            run_ref: id("run_ref"),
            purpose: "bmad_help".to_owned(),
            model_role: "method_help".to_owned(),
            canonical_output_schema_id: id("method_help_proposal_v1"),
            canonical_output_schema_hash: sha256_bytes(b"help proposal schema"),
            provider_profile_hash: sha256_bytes(b"provider profile"),
            model_profile_hash: sha256_bytes(b"model profile"),
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
            candidates: vec![
                ContextCandidate {
                    client_item_id: id("item_instruction"),
                    relative_label: RelativeWorkspacePath::new("instruction.txt")
                        .expect("fixture path"),
                    semantic_role: "sealed_instruction".to_owned(),
                    language: Some("text".to_owned()),
                    classification: ContextClassification::Internal,
                    content: contents[0].to_owned(),
                },
                ContextCandidate {
                    client_item_id: id("item_intent"),
                    relative_label: RelativeWorkspacePath::new("intent.txt").expect("fixture path"),
                    semantic_role: "current_intent".to_owned(),
                    language: Some("text".to_owned()),
                    classification: ContextClassification::Internal,
                    content: contents[1].to_owned(),
                },
            ],
            exclusions: Vec::new(),
        })
        .expect("fixture manifest")
}

fn d2_binding(
    manifest: &ContextEgressManifest,
    session_authority_hash: Sha256Digest,
) -> ModelInvocationBinding {
    ModelInvocationBindingDraft {
        schema_version: "sapphirus.model-invocation-binding.v1".to_owned(),
        request_id: id("request_001"),
        tenant_ref: manifest.draft.tenant_ref.clone(),
        project_ref: manifest.draft.project_ref.clone(),
        run_ref: manifest.draft.run_ref.clone(),
        installation_id: id("installation_001"),
        session_authority_hash,
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
        consent_disclosure_hash: sha256_bytes(b"reviewed disclosure"),
    }
    .seal()
    .expect("D2 binding")
}

struct BridgeFixture {
    manifest: ContextEgressManifest,
    d2_binding: ModelInvocationBinding,
    decision: PendingContextDecision,
    ledger: MemoryDecisionLedger,
    method_binding: MethodExactBinding,
    expectation: BmadHelpDecisionBridgeExpectation,
}

fn bridge_fixture() -> BridgeFixture {
    let method_binding = compiled_method_binding(7);
    let method_binding_hash = method_binding.binding_hash().expect("Method binding hash");
    let manifest = manifest_with(["sealed help instructions", "review architecture readiness"]);
    let session_authority_hash = sha256_bytes(b"Method session authority");
    let d2_binding = d2_binding(&manifest, session_authority_hash);
    let ledger = MemoryDecisionLedger::default();
    let decision = ConsentService::new(&ledger)
        .approve(ApproveDecisionInput {
            manifest: &manifest,
            binding: &d2_binding,
            decision_id: id("decision_001"),
            issued_at: UnixMillis(1_500),
            expires_at: UnixMillis(31_500),
        })
        .expect("pending decision");
    let expectation = BmadHelpDecisionBridgeExpectation {
        decision_id: id("decision_001"),
        manifest_hash: manifest.manifest_hash,
        d2_binding_hash: d2_binding.binding_hash,
        session_authority_hash,
        method_binding_hash,
        issued_at: UnixMillis(1_500),
        expires_at: UnixMillis(31_500),
        observed_at: UnixMillis(2_000),
    };
    BridgeFixture {
        manifest,
        d2_binding,
        decision,
        ledger,
        method_binding,
        expectation,
    }
}

fn bridge(
    fixture: &BridgeFixture,
    expectation: &BmadHelpDecisionBridgeExpectation,
) -> Result<MethodContextDecision, BmadModelBridgeError> {
    let service = ConsentService::new(&fixture.ledger);
    bridge_method_context_decision(
        service
            .evidence(DecisionEvidenceInput {
                decision: &fixture.decision,
                observed_at: fixture.expectation.observed_at,
            })
            .expect("live sealed evidence"),
        &fixture.manifest,
        &fixture.d2_binding,
        &fixture.method_binding,
        expectation,
    )
}

fn bridge_with(
    fixture: &BridgeFixture,
    manifest: &ContextEgressManifest,
    d2_binding: &ModelInvocationBinding,
    method_binding: &MethodExactBinding,
    expectation: &BmadHelpDecisionBridgeExpectation,
) -> Result<MethodContextDecision, BmadModelBridgeError> {
    let service = ConsentService::new(&fixture.ledger);
    bridge_method_context_decision(
        service
            .evidence(DecisionEvidenceInput {
                decision: &fixture.decision,
                observed_at: fixture.expectation.observed_at,
            })
            .expect("live sealed evidence"),
        manifest,
        d2_binding,
        method_binding,
        expectation,
    )
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExpectedReviewedContext<'a> {
    items: Vec<ExpectedReviewedContextItem<'a>>,
    manifest_hash: Sha256Digest,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExpectedReviewedContextItem<'a> {
    client_item_id: &'a ContractId,
    outbound_content_hash: Sha256Digest,
    outbound_byte_count: u64,
}

#[test]
fn bridge_produces_the_locked_method_context_mapping() {
    let fixture = bridge_fixture();
    let decision = bridge(&fixture, &fixture.expectation).expect("bridged Method decision");
    let expected_context_digest = canonical_hash(
        "bmad-help-reviewed-context",
        1,
        &ExpectedReviewedContext {
            items: fixture
                .manifest
                .draft
                .items
                .iter()
                .map(|item| ExpectedReviewedContextItem {
                    client_item_id: &item.client_item_id,
                    outbound_content_hash: item.outbound_content_hash,
                    outbound_byte_count: item.outbound_byte_count,
                })
                .collect(),
            manifest_hash: fixture.manifest.manifest_hash,
        },
    )
    .expect("reviewed context digest");

    assert_eq!(decision.decision_id, fixture.expectation.decision_id);
    assert_eq!(decision.manifest_hash, fixture.manifest.manifest_hash);
    assert_eq!(
        decision.consent_hash,
        fixture.d2_binding.draft.consent_disclosure_hash
    );
    assert_eq!(decision.context_digest, expected_context_digest);
    assert_eq!(
        decision.binding_hash,
        fixture.expectation.method_binding_hash
    );
    assert_eq!(decision.reviewed_at, fixture.expectation.issued_at);
}

#[test]
fn bridge_rejects_manifest_item_order_and_content_substitution() {
    let fixture = bridge_fixture();
    let mut reordered = fixture.manifest.clone();
    reordered.draft.items.swap(0, 1);
    assert_eq!(
        bridge_with(
            &fixture,
            &reordered,
            &fixture.d2_binding,
            &fixture.method_binding,
            &fixture.expectation,
        ),
        Err(BmadModelBridgeError::BindingMismatch)
    );

    let mut changed_content = fixture.manifest.clone();
    changed_content.draft.items[0]
        .outbound_content
        .push_str(" substituted");
    assert_eq!(
        bridge_with(
            &fixture,
            &changed_content,
            &fixture.d2_binding,
            &fixture.method_binding,
            &fixture.expectation,
        ),
        Err(BmadModelBridgeError::BindingMismatch)
    );
}

#[test]
fn bridge_rejects_disclosure_and_d2_binding_substitution() {
    let fixture = bridge_fixture();
    let mut disclosure = fixture.d2_binding.draft.clone();
    disclosure.consent_disclosure_hash = sha256_bytes(b"other disclosure");
    let disclosure = disclosure.seal().expect("substituted disclosure binding");
    assert_eq!(
        bridge_with(
            &fixture,
            &fixture.manifest,
            &disclosure,
            &fixture.method_binding,
            &fixture.expectation,
        ),
        Err(BmadModelBridgeError::BindingMismatch)
    );

    let mut request = fixture.d2_binding.draft.clone();
    request.request_id = id("request_substituted");
    let request = request.seal().expect("substituted D2 binding");
    assert_eq!(
        bridge_with(
            &fixture,
            &fixture.manifest,
            &request,
            &fixture.method_binding,
            &fixture.expectation,
        ),
        Err(BmadModelBridgeError::BindingMismatch)
    );
}

#[test]
fn bridge_rejects_session_and_method_binding_substitution() {
    let fixture = bridge_fixture();
    let mut session = fixture.d2_binding.draft.clone();
    session.session_authority_hash = sha256_bytes(b"other session authority");
    let session = session.seal().expect("substituted session binding");
    assert_eq!(
        bridge_with(
            &fixture,
            &fixture.manifest,
            &session,
            &fixture.method_binding,
            &fixture.expectation,
        ),
        Err(BmadModelBridgeError::BindingMismatch)
    );

    let other_method_binding = compiled_method_binding(8);
    assert_eq!(
        bridge_with(
            &fixture,
            &fixture.manifest,
            &fixture.d2_binding,
            &other_method_binding,
            &fixture.expectation,
        ),
        Err(BmadModelBridgeError::BindingMismatch)
    );
}

#[test]
fn bridge_rejects_every_expected_identity_and_timestamp_substitution() {
    let fixture = bridge_fixture();
    let mut substitutions = Vec::new();

    let mut decision_id = fixture.expectation.clone();
    decision_id.decision_id = id("decision_substituted");
    substitutions.push(decision_id);

    let mut manifest = fixture.expectation.clone();
    manifest.manifest_hash = sha256_bytes(b"other manifest");
    substitutions.push(manifest);

    let mut d2_binding = fixture.expectation.clone();
    d2_binding.d2_binding_hash = sha256_bytes(b"other D2 binding");
    substitutions.push(d2_binding);

    let mut session = fixture.expectation.clone();
    session.session_authority_hash = sha256_bytes(b"other session authority");
    substitutions.push(session);

    let mut method_binding = fixture.expectation.clone();
    method_binding.method_binding_hash = sha256_bytes(b"other Method binding");
    substitutions.push(method_binding);

    let mut issued = fixture.expectation.clone();
    issued.issued_at = UnixMillis(1_501);
    substitutions.push(issued);

    let mut expires = fixture.expectation.clone();
    expires.expires_at = UnixMillis(31_501);
    substitutions.push(expires);

    let mut observed = fixture.expectation.clone();
    observed.observed_at = UnixMillis(2_001);
    substitutions.push(observed);

    for substitution in substitutions {
        assert_eq!(
            bridge(&fixture, &substitution),
            Err(BmadModelBridgeError::BindingMismatch)
        );
    }
}
