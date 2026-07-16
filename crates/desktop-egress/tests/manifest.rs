use desktop_egress::manifest::{
    ContextClassification, ContextEgressManifestDraft, EgressError, EgressLimits,
    PreparedContextItem, RetentionMode,
};
use desktop_runtime::{sha256_bytes, ContractId, RelativeWorkspacePath, UnixMillis};

fn fixture_item(
    label: &str,
    content: &str,
) -> Result<PreparedContextItem, Box<dyn std::error::Error>> {
    Ok(PreparedContextItem {
        item_id: ContractId::new(format!("item-{}", label.replace('/', "-")))?,
        relative_label: RelativeWorkspacePath::new(label)?,
        classification: ContextClassification::Workspace,
        original_content: content.to_owned(),
        outbound_content: content.to_owned(),
        original_byte_count: content.len() as u64,
        outbound_byte_count: content.len() as u64,
        original_content_hash: sha256_bytes(content.as_bytes()),
        outbound_content_hash: sha256_bytes(content.as_bytes()),
        token_count: 1,
        redactions: Vec::new(),
    })
}

fn fixture_draft(items: Vec<PreparedContextItem>) -> ContextEgressManifestDraft {
    let total_byte_count = items.iter().map(|item| item.outbound_byte_count).sum();
    let total_token_count = items.iter().map(|item| item.token_count).sum();
    ContextEgressManifestDraft {
        schema_version: 1,
        created_at: UnixMillis(1_000),
        expires_at: UnixMillis(2_000),
        items,
        total_byte_count,
        total_token_count,
        limits: EgressLimits {
            maximum_items: 10,
            maximum_context_bytes: 1_000,
            maximum_tokens: 100,
        },
        retention_mode: RetentionMode::TransientNoStore,
    }
}

#[test]
fn manifest_seals_exact_outbound_bytes_and_projects_them_for_review() -> Result<(), EgressError> {
    let item =
        fixture_item("src/lib.rs", "fn main() {}\n").map_err(|_| EgressError::ContextDrift)?;
    let manifest = fixture_draft(vec![item]).seal()?;
    manifest.verify()?;
    let review = manifest.review_projection();
    assert_eq!(review.items[0].relative_label.as_str(), "src/lib.rs");
    assert_eq!(review.items[0].outbound_content, "fn main() {}\n");
    assert_eq!(review.manifest_hash, manifest.manifest_hash);
    Ok(())
}

#[test]
fn manifest_rejects_an_outbound_hash_that_does_not_match_the_bytes(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut item = fixture_item("src/lib.rs", "safe\n")?;
    item.outbound_content_hash = sha256_bytes(b"different");
    assert!(matches!(
        fixture_draft(vec![item]).seal(),
        Err(EgressError::ContextDrift)
    ));
    Ok(())
}

#[test]
fn manifest_hash_changes_when_item_order_changes() -> Result<(), EgressError> {
    let first = fixture_item("src/a.rs", "a").map_err(|_| EgressError::ContextDrift)?;
    let second = fixture_item("src/b.rs", "b").map_err(|_| EgressError::ContextDrift)?;
    let left = fixture_draft(vec![first.clone(), second.clone()]).seal()?;
    let right = fixture_draft(vec![second, first]).seal()?;
    assert_ne!(left.manifest_hash, right.manifest_hash);
    Ok(())
}

#[test]
fn malformed_manifest_values_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
    let mut empty = fixture_draft(Vec::new());
    assert!(matches!(empty.clone().seal(), Err(EgressError::EmptyItems)));

    let mut invalid_schema = fixture_draft(vec![fixture_item("src/a.rs", "a")?]);
    invalid_schema.schema_version = 2;
    assert_eq!(
        invalid_schema.seal(),
        Err(EgressError::UnsupportedSchemaVersion)
    );

    let mut invalid_time = fixture_draft(vec![fixture_item("src/a.rs", "a")?]);
    invalid_time.expires_at = invalid_time.created_at;
    assert_eq!(invalid_time.seal(), Err(EgressError::InvalidTimeRange));

    let mut invalid_count = fixture_draft(vec![fixture_item("src/a.rs", "a")?]);
    invalid_count.total_byte_count += 1;
    assert_eq!(invalid_count.seal(), Err(EgressError::ContextDrift));

    let mut invalid_limit = fixture_draft(vec![fixture_item("src/a.rs", "a")?]);
    invalid_limit.limits.maximum_items = 0;
    assert_eq!(invalid_limit.seal(), Err(EgressError::InvalidLimits));

    let mut invalid_retention = fixture_draft(vec![fixture_item("src/a.rs", "a")?]);
    invalid_retention.retention_mode = RetentionMode::Persistent;
    assert_eq!(
        invalid_retention.seal(),
        Err(EgressError::UnsupportedRetention)
    );

    empty.items = vec![
        fixture_item("src/a.rs", "a")?,
        fixture_item("src/a.rs", "a")?,
    ];
    empty.total_byte_count = 2;
    empty.total_token_count = 2;
    assert!(matches!(empty.seal(), Err(EgressError::DuplicateItemId)));
    Ok(())
}
