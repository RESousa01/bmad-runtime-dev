#![allow(
    clippy::expect_used,
    reason = "invalid static fixtures should fail the test at construction"
)]

use desktop_egress::{
    ContextClassification, ContextEgressManifestDraft, EgressError, EgressLimits,
    PreparedContextItem, RetentionMode,
};
use desktop_runtime::{sha256_bytes, ContractId, RelativeWorkspacePath, UnixMillis};

fn fixture_item(label: &str, content: &str) -> PreparedContextItem {
    PreparedContextItem {
        client_item_id: ContractId::new(format!("item_{}", label.replace(['/', '.'], "_")))
            .expect("fixture item id"),
        relative_label: RelativeWorkspacePath::new(label).expect("fixture label"),
        semantic_role: "source".to_owned(),
        language: Some("rust".to_owned()),
        original_content_hash: sha256_bytes(content.as_bytes()),
        outbound_content_hash: sha256_bytes(content.as_bytes()),
        original_byte_count: content.len() as u64,
        outbound_byte_count: content.len() as u64,
        token_estimate: 4,
        classification: ContextClassification::Internal,
        redactions: Vec::new(),
        outbound_content: content.to_owned(),
    }
}

fn fixture_draft(items: Vec<PreparedContextItem>) -> ContextEgressManifestDraft {
    let total_outbound_bytes = items.iter().map(|item| item.outbound_byte_count).sum();
    let total_token_estimate = items.iter().map(|item| item.token_estimate).sum();
    ContextEgressManifestDraft {
        schema_version: "sapphirus.context-egress-manifest.v1".to_owned(),
        tenant_ref: ContractId::new("tenant_ref").expect("tenant ref"),
        project_ref: ContractId::new("project_ref").expect("project ref"),
        run_ref: ContractId::new("run_ref").expect("run ref"),
        purpose: "planning".to_owned(),
        model_role: "planner".to_owned(),
        canonical_output_schema_id: ContractId::new("planning_output_v1").expect("schema id"),
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
        items,
        exclusions: Vec::new(),
        secret_findings: Vec::new(),
        total_outbound_bytes,
        total_token_estimate,
    }
}

#[test]
fn manifest_seals_exact_outbound_bytes_and_projects_them_for_review() {
    let item = fixture_item("src/lib.rs", "fn main() {}\n");
    let manifest = fixture_draft(vec![item]).seal().expect("valid manifest");

    manifest.verify().expect("sealed manifest");
    let review = manifest.review_projection();

    assert_eq!(review.items[0].relative_label.as_str(), "src/lib.rs");
    assert_eq!(review.items[0].outbound_content, "fn main() {}\n");
    assert_eq!(review.manifest_hash, manifest.manifest_hash);
}

#[test]
fn manifest_rejects_an_outbound_hash_that_does_not_match_the_bytes() {
    let mut item = fixture_item("src/lib.rs", "safe\n");
    item.outbound_content_hash = sha256_bytes(b"different");

    let error = fixture_draft(vec![item])
        .seal()
        .expect_err("hash drift must fail");

    assert_eq!(error, EgressError::ContextDrift);
}

#[test]
fn manifest_hash_changes_when_item_order_changes() {
    let first = fixture_item("src/a.rs", "a");
    let second = fixture_item("src/b.rs", "b");
    let left = fixture_draft(vec![first.clone(), second.clone()])
        .seal()
        .expect("left manifest");
    let right = fixture_draft(vec![second, first])
        .seal()
        .expect("right manifest");

    assert_ne!(left.manifest_hash, right.manifest_hash);
}

#[test]
fn manifest_rejects_duplicate_item_identifiers() {
    let first = fixture_item("src/a.rs", "a");
    let mut second = fixture_item("src/b.rs", "b");
    second.client_item_id = first.client_item_id.clone();

    let error = fixture_draft(vec![first, second])
        .seal()
        .expect_err("duplicate identifiers must fail");

    assert_eq!(error, EgressError::DuplicateContextItem);
}

#[test]
fn manifest_rejects_expired_or_oversized_context() {
    let item = fixture_item("src/lib.rs", "safe\n");
    let mut expired = fixture_draft(vec![item.clone()]);
    expired.expires_at = expired.created_at;
    assert_eq!(expired.seal(), Err(EgressError::InvalidLifetime));

    let mut oversized = fixture_draft(vec![item]);
    oversized.limits.maximum_context_bytes = 1;
    assert_eq!(oversized.seal(), Err(EgressError::ContextBudgetExceeded));
}
