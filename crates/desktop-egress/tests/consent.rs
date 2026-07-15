#![allow(
    clippy::expect_used,
    reason = "invalid static fixtures should fail the test at construction"
)]

use desktop_egress::{
    ApproveDecisionInput, ConsentService, ConsumeDecisionInput, ContextCandidate,
    ContextClassification, ContextEgressManifest, ContextPreparer, EgressError, EgressLimits,
    MemoryDecisionLedger, ModelInvocationBindingDraft, PatternSecretScanner, PrepareContextInput,
    RetentionMode,
};
use desktop_runtime::{sha256_bytes, ContractId, RelativeWorkspacePath, UnixMillis};

fn id(value: &str) -> ContractId {
    ContractId::new(value).expect("valid fixture identifier")
}

fn fixture_manifest() -> ContextEgressManifest {
    ContextPreparer::new(PatternSecretScanner)
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
        .expect("fixture manifest")
}

fn fixture_binding(manifest: &ContextEgressManifest) -> ModelInvocationBindingDraft {
    ModelInvocationBindingDraft {
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
}

fn fixture_approval<'a>(
    manifest: &'a ContextEgressManifest,
    binding: &'a desktop_egress::ModelInvocationBinding,
) -> ApproveDecisionInput<'a> {
    ApproveDecisionInput {
        manifest,
        binding,
        decision_id: id("decision_001"),
        issued_at: UnixMillis(1_500),
        expires_at: UnixMillis(31_500),
    }
}

fn fixture_consumption<'a>(
    decision: &'a desktop_egress::PendingContextDecision,
    binding: &'a desktop_egress::ModelInvocationBinding,
) -> ConsumeDecisionInput<'a> {
    ConsumeDecisionInput {
        decision,
        binding,
        invocation_id: id("invocation_001"),
        consumed_at: UnixMillis(2_000),
    }
}

#[test]
fn one_decision_authorizes_one_exact_invocation() {
    let manifest = fixture_manifest();
    let binding = fixture_binding(&manifest).seal().expect("binding");
    let ledger = MemoryDecisionLedger::default();
    let service = ConsentService::new(&ledger);
    let decision = service
        .approve(fixture_approval(&manifest, &binding))
        .expect("decision");

    let consumed = service
        .consume(fixture_consumption(&decision, &binding))
        .expect("consumption");

    assert_eq!(consumed.decision_id, decision.decision_id);
    assert_eq!(consumed.binding_hash, binding.binding_hash);
    assert_eq!(
        service.consume(fixture_consumption(&decision, &binding)),
        Err(EgressError::DecisionAlreadyConsumed)
    );
}

#[test]
fn byte_identical_retry_still_requires_a_new_decision() {
    let manifest = fixture_manifest();
    let binding = fixture_binding(&manifest).seal().expect("binding");
    let ledger = MemoryDecisionLedger::default();
    let service = ConsentService::new(&ledger);
    let decision = service
        .approve(fixture_approval(&manifest, &binding))
        .expect("decision");
    service
        .consume(fixture_consumption(&decision, &binding))
        .expect("first consumption");

    let mut retry = fixture_consumption(&decision, &binding);
    retry.invocation_id = id("invocation_retry");

    assert_eq!(
        service.consume(retry),
        Err(EgressError::DecisionAlreadyConsumed)
    );
}

#[test]
fn drifted_region_is_rejected_without_consuming_the_decision() {
    let manifest = fixture_manifest();
    let binding = fixture_binding(&manifest).seal().expect("binding");
    let ledger = MemoryDecisionLedger::default();
    let service = ConsentService::new(&ledger);
    let decision = service
        .approve(fixture_approval(&manifest, &binding))
        .expect("decision");
    let mut drifted = fixture_binding(&manifest);
    drifted.region = "westus".to_owned();
    let drifted = drifted.seal().expect("drifted binding shape");

    assert_eq!(
        service.consume(fixture_consumption(&decision, &drifted)),
        Err(EgressError::DecisionBindingMismatch)
    );
    assert!(service
        .consume(fixture_consumption(&decision, &binding))
        .is_ok());
}

#[test]
fn binding_that_does_not_match_the_manifest_cannot_be_approved() {
    let manifest = fixture_manifest();
    let mut draft = fixture_binding(&manifest);
    draft.purpose = "analysis".to_owned();
    let binding = draft.seal().expect("binding shape");
    let ledger = MemoryDecisionLedger::default();
    let service = ConsentService::new(&ledger);

    assert_eq!(
        service.approve(fixture_approval(&manifest, &binding)),
        Err(EgressError::DecisionBindingMismatch)
    );
}

#[test]
fn expired_decision_is_terminal() {
    let manifest = fixture_manifest();
    let binding = fixture_binding(&manifest).seal().expect("binding");
    let ledger = MemoryDecisionLedger::default();
    let service = ConsentService::new(&ledger);
    let decision = service
        .approve(fixture_approval(&manifest, &binding))
        .expect("decision");
    let mut input = fixture_consumption(&decision, &binding);
    input.consumed_at = UnixMillis(decision.expires_at.0 + 1);

    assert_eq!(service.consume(input), Err(EgressError::DecisionExpired));
    assert_eq!(
        service.consume(fixture_consumption(&decision, &binding)),
        Err(EgressError::DecisionExpired)
    );
}

#[test]
fn duplicate_decision_identifier_is_rejected() {
    let manifest = fixture_manifest();
    let binding = fixture_binding(&manifest).seal().expect("binding");
    let ledger = MemoryDecisionLedger::default();
    let service = ConsentService::new(&ledger);
    service
        .approve(fixture_approval(&manifest, &binding))
        .expect("first decision");

    assert_eq!(
        service.approve(fixture_approval(&manifest, &binding)),
        Err(EgressError::DecisionAlreadyExists)
    );
}
