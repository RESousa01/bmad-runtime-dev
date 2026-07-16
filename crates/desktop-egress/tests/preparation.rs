#![allow(
    clippy::expect_used,
    reason = "invalid static fixtures should fail the test at construction"
)]

use desktop_egress::{
    ContextCandidate, ContextClassification, ContextPreparer, EgressError, EgressLimits,
    PatternSecretScanner, PrepareContextInput, RetentionMode,
};
use desktop_runtime::{sha256_bytes, ContractId, RelativeWorkspacePath, UnixMillis};

fn candidate(label: &str, content: &str) -> ContextCandidate {
    ContextCandidate {
        client_item_id: ContractId::new(format!("item_{}", label.replace(['/', '.'], "_")))
            .expect("fixture item id"),
        relative_label: RelativeWorkspacePath::new(label).expect("fixture label"),
        semantic_role: "source".to_owned(),
        language: Some("text".to_owned()),
        classification: ContextClassification::Internal,
        content: content.to_owned(),
    }
}

fn fixture_input(candidates: Vec<ContextCandidate>) -> PrepareContextInput {
    PrepareContextInput {
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
        candidates,
        exclusions: Vec::new(),
    }
}

#[test]
fn preparation_preserves_safe_bytes_and_binds_original_and_outbound_hashes() {
    let content = "safe context\n";
    let manifest = ContextPreparer::new(PatternSecretScanner)
        .prepare(fixture_input(vec![candidate("notes.txt", content)]))
        .expect("safe manifest");

    let item = &manifest.draft.items[0];
    assert_eq!(item.outbound_content, content);
    assert_eq!(item.original_content_hash, sha256_bytes(content.as_bytes()));
    assert_eq!(item.outbound_content_hash, sha256_bytes(content.as_bytes()));
    assert!(manifest.draft.secret_findings.is_empty());
}

#[test]
fn preparation_rejects_dotenv_before_scanning() {
    let input = fixture_input(vec![candidate(".env", "API_KEY=secret")]);

    let error = ContextPreparer::new(PatternSecretScanner)
        .prepare(input)
        .expect_err("dotenv is denied");

    assert_eq!(error, EgressError::DeniedContextLabel);
}

#[test]
fn preparation_rejects_nested_secret_bearing_filenames_case_insensitively() {
    let input = fixture_input(vec![candidate("config/CREDENTIALS", "secret")]);

    let error = ContextPreparer::new(PatternSecretScanner)
        .prepare(input)
        .expect_err("credential file is denied");

    assert_eq!(error, EgressError::DeniedContextLabel);
}

#[test]
fn preparation_denies_credential_stores_token_caches_and_authority_state() {
    for label in [
        ".git-credentials",
        ".netrc",
        ".pypirc",
        ".docker/config.json",
        ".azure/msal_token_cache.json",
        ".sapphirus/authority.db",
        ".kube/config",
        ".gnupg/private-keys-v1.d/key.key",
        "tls/server.key",
        "certs/client.pem",
        "keys/id_ecdsa",
        "keys/id_dsa",
        "keys/id_ed25519_sk",
        "certs/client.p12",
        "certs/client.pfx",
        "config/token-cache.bin",
    ] {
        let error = ContextPreparer::new(PatternSecretScanner)
            .prepare(fixture_input(vec![candidate(
                label,
                "innocent-looking bytes",
            )]))
            .expect_err("authority-bearing label is denied before scanning");
        assert_eq!(error, EgressError::DeniedContextLabel, "accepted {label}");
    }
}

#[test]
fn preparation_redacts_private_key_material_and_records_a_finding() {
    let source = "prefix -----BEGIN PRIVATE KEY----- value";
    let manifest = ContextPreparer::new(PatternSecretScanner)
        .prepare(fixture_input(vec![candidate("notes.txt", source)]))
        .expect("redacted manifest");

    let item = &manifest.draft.items[0];
    assert!(!item.outbound_content.contains("BEGIN PRIVATE KEY"));
    assert!(item.outbound_content.contains("[REDACTED:private_key]"));
    assert_eq!(item.redactions[0].kind, "private_key");
    assert_eq!(item.redactions[0].occurrence_count, 1);
    assert_eq!(manifest.draft.secret_findings[0].kind, "private_key");
    assert_eq!(manifest.draft.secret_findings[0].occurrence_count, 1);
}

#[test]
fn preparation_redacts_prefixed_tokens_without_retaining_the_secret() {
    let github_token = "ghp_abcdefghijklmnopqrstuvwxyz0123456789";
    let source = format!("token={github_token}");
    let manifest = ContextPreparer::new(PatternSecretScanner)
        .prepare(fixture_input(vec![candidate("notes.txt", &source)]))
        .expect("redacted manifest");

    let serialized = serde_json::to_string(&manifest.review_projection()).expect("projection json");
    assert!(!serialized.contains(github_token));
    assert!(serialized.contains("github_token"));
}

#[test]
fn preparation_enforces_the_outbound_byte_budget_after_redaction() {
    let mut input = fixture_input(vec![candidate("notes.txt", "12345")]);
    input.limits.maximum_context_bytes = 4;

    let result = ContextPreparer::new(PatternSecretScanner).prepare(input);

    assert_eq!(result, Err(EgressError::ContextBudgetExceeded));
}
