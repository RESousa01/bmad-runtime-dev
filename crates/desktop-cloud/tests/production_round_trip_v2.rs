//! FND-043: the catalog gate in front of the production round trip.
//!
//! Two orderings are asserted. The catalog is fetched and the selection is
//! resolved *before* review, so the user approves bytes going to a model that
//! was actually consulted about whether it can take them. And the catalog and
//! profile are revalidated *immediately before* dispatch, so a catalog that
//! expired or a profile that was withdrawn during review stops the call instead
//! of spending a single-use approval on a model the organization no longer
//! offers.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "a fixture that does not hold is a broken test, and it must stop the run"
)]

use std::collections::VecDeque;
use std::sync::Mutex;

use async_trait::async_trait;
use desktop_cloud::{
    resolve_model_selection, revalidate_before_dispatch, CloudError, HttpExecutor, HttpResponse,
    ModelCatalogRequest, ModelSelectionOutcome, OutboundHttpRequest, PersistedModelSelection,
    ResolvedModelSelection, SupportApiOrigin, SupportApiTransport, VerifiedModelCatalog,
};
use desktop_runtime::UnixMillis;
use serde_json::{json, Value};
use zeroize::Zeroizing;

const PROFILE_ID: &str = "fixture-mid";
const VARIANT_ID: &str = "balanced";
const CATALOG_VERSION: &str = "v7.00112233445566aa";

/// Records the URL of every request so the ordering claim is checked rather
/// than assumed.
struct RecordingExecutor {
    responses: Mutex<VecDeque<HttpResponse>>,
    seen: Mutex<Vec<String>>,
}

#[async_trait]
impl HttpExecutor for RecordingExecutor {
    async fn execute(&self, request: OutboundHttpRequest) -> Result<HttpResponse, CloudError> {
        self.seen
            .lock()
            .expect("url log")
            .push(request.url().path().to_owned());
        self.responses
            .lock()
            .expect("response lock")
            .pop_front()
            .ok_or(CloudError::TransportFailed)
    }
}

fn catalog_body(expires_at: &str, profiles: &[Value], default_profile_id: &str) -> String {
    serde_json::to_string(&json!({
        "schemaVersion": "sapphirus.model-catalog.v1",
        "catalogVersion": CATALOG_VERSION,
        "catalogHash": format!("sha256:{}", "1".repeat(64)),
        "catalogState": "fresh",
        "policyVersion": 3,
        "policyHash": "sha256:2222222222222222222222222222222222222222222222222222222222222222",
        "deploymentSnapshotHash":
            "sha256:3333333333333333333333333333333333333333333333333333333333333333",
        "generatedAt": "2026-07-28T00:00:00.000Z",
        "expiresAt": expires_at,
        "defaultProfileId": default_profile_id,
        "profiles": profiles,
        "unavailableProfiles": []
    }))
    .expect("catalog serializes")
}

fn profile(profile_id: &str, variant_ids: &[&str]) -> Value {
    json!({
        "profileId": profile_id,
        "displayName": "Fixture Profile",
        "description": "A synthetic fixture profile. It names no deployment.",
        "family": null,
        "provider": "microsoft_foundry",
        "availability": "available",
        "statusDetail": null,
        "capabilities": {
            "textInput": true,
            "imageInput": false,
            "pdfInput": false,
            "attachments": false,
            "toolCalling": true,
            "structuredOutput": true,
            "reasoning": true
        },
        "limits": {
            "contextTokens": 128_000,
            "maxInputTokens": 120_000,
            "maxOutputTokens": 8_000,
            "maxAttachmentBytes": null
        },
        "variants": variant_ids.iter().map(|variant_id| json!({
            "variantId": variant_id,
            "displayName": "Variant",
            "reasoningEffort": "medium",
            "maxOutputTokens": 8_000
        })).collect::<Vec<_>>(),
        "defaultVariantId": variant_ids.first().expect("at least one variant"),
        "qualifiedAt": "2026-07-28T00:00:00.000Z"
    })
}

async fn fetched(body: String) -> (VerifiedModelCatalog, Vec<String>) {
    let executor = RecordingExecutor {
        responses: Mutex::new(VecDeque::from(vec![HttpResponse::new(
            200,
            Some("application/json".to_owned()),
            Some(body.len() as u64),
            body.into_bytes(),
        )])),
        seen: Mutex::new(Vec::new()),
    };
    let transport = SupportApiTransport::new(
        SupportApiOrigin::new("https://support.example.test").expect("origin"),
        executor,
    );
    let catalog = transport
        .fetch_model_catalog(
            Zeroizing::new("token".to_owned()),
            &ModelCatalogRequest {
                subject_hash: "sha256:fixture-subject",
                cached: None,
                idempotency_key: "round-trip-fixture",
            },
            UnixMillis(1),
        )
        .await
        .expect("catalog verifies")
        .cached()
        .catalog()
        .clone();
    let seen = transport
        .executor_for_test()
        .seen
        .lock()
        .expect("url log")
        .clone();
    (catalog, seen)
}

fn persisted(profile_id: &str, variant_id: &str) -> PersistedModelSelection {
    PersistedModelSelection {
        profile_id: profile_id.to_owned(),
        variant_id: variant_id.to_owned(),
        catalog_version: CATALOG_VERSION.to_owned(),
        catalog_hash: format!("sha256:{}", "1".repeat(64)),
        selected_at: UnixMillis(10),
    }
}

fn resolved(catalog: &VerifiedModelCatalog) -> ResolvedModelSelection {
    match resolve_model_selection(catalog, Some(&persisted(PROFILE_ID, VARIANT_ID)), &[]) {
        ModelSelectionOutcome::Selected { selection, .. } => selection,
        other => panic!("the fixture selection must resolve, got {other:?}"),
    }
}

/// The catalog is fetched from the one reviewed catalog route, and the selection
/// is resolved against it — both before any review surface exists.
#[tokio::test]
async fn the_catalog_is_fetched_and_resolved_before_review() {
    let (catalog, seen) = fetched(catalog_body(
        "2026-07-28T00:05:00.000Z",
        &[profile(PROFILE_ID, &[VARIANT_ID, "thorough"])],
        PROFILE_ID,
    ))
    .await;

    assert_eq!(seen, ["/desktop/v1/model-catalog"]);
    let selection = resolved(&catalog);
    assert_eq!(selection.profile_id(), PROFILE_ID);
    assert_eq!(selection.variant_id(), VARIANT_ID);
    assert_eq!(selection.catalog_version(), catalog.catalog_version());
}

/// The gate immediately before dispatch. Each refusal here is an approval that
/// is *not* spent.
#[tokio::test]
async fn the_catalog_and_profile_are_revalidated_before_dispatch() {
    let (catalog, _) = fetched(catalog_body(
        "2026-07-28T00:05:00.000Z",
        &[profile(PROFILE_ID, &[VARIANT_ID, "thorough"])],
        PROFILE_ID,
    ))
    .await;
    let selection = resolved(&catalog);

    // Inside the catalog's own validity window: the dispatch may proceed.
    let before_expiry = UnixMillis(catalog.expires_at().0 - 1);
    revalidate_before_dispatch(&catalog, &selection, before_expiry)
        .expect("a live catalog and an offered profile pass the gate");

    // At expiry, and after it, the catalog is no longer authority.
    assert_eq!(
        revalidate_before_dispatch(&catalog, &selection, catalog.expires_at()),
        Err(CloudError::ModelCatalogUnavailable),
    );
    assert_eq!(
        revalidate_before_dispatch(&catalog, &selection, UnixMillis(catalog.expires_at().0 + 1)),
        Err(CloudError::ModelCatalogUnavailable),
    );

    // A profile withdrawn between review and dispatch stops the call.
    let (withdrawn, _) = fetched(catalog_body(
        "2026-07-28T00:05:00.000Z",
        &[profile("fixture-alpha", &["fast"])],
        "fixture-alpha",
    ))
    .await;
    assert_eq!(
        revalidate_before_dispatch(&withdrawn, &selection, before_expiry),
        Err(CloudError::ModelCatalogIntegrity),
    );

    // So does a variant that disappeared while the profile survived.
    let (narrowed, _) = fetched(catalog_body(
        "2026-07-28T00:05:00.000Z",
        &[profile(PROFILE_ID, &["thorough"])],
        PROFILE_ID,
    ))
    .await;
    assert_eq!(
        revalidate_before_dispatch(&narrowed, &selection, before_expiry),
        Err(CloudError::ModelCatalogIntegrity),
    );
}

/// A selection resolved against one catalog cannot be dispatched against
/// another, even when the profile happens to exist in both.
#[tokio::test]
async fn a_selection_from_another_catalog_epoch_is_refused() {
    let (catalog, _) = fetched(catalog_body(
        "2026-07-28T00:05:00.000Z",
        &[profile(PROFILE_ID, &[VARIANT_ID])],
        PROFILE_ID,
    ))
    .await;
    let selection = resolved(&catalog);

    let newer_body = catalog_body(
        "2026-07-28T00:05:00.000Z",
        &[profile(PROFILE_ID, &[VARIANT_ID])],
        PROFILE_ID,
    )
    .replace(&"1".repeat(64), &"9".repeat(64));
    let (newer, _) = fetched(newer_body).await;

    assert_eq!(
        revalidate_before_dispatch(&newer, &selection, UnixMillis(2)),
        Err(CloudError::ModelCatalogIntegrity),
    );
}
