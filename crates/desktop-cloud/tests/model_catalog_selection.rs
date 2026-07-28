//! FND-041: which model answers is never changed behind the user's back.
//!
//! The invariant under test is that resolution has exactly one shape: the
//! user's own current selection wins if it is still available, and if it is
//! *not*, resolution stops and demands an explicit choice rather than quietly
//! falling through to some other model. Only when there is no current selection
//! at all does the fallback order run — most recent still-valid selection, then
//! the catalog's default, then the deterministic first selectable profile.
//!
//! A persisted selection also carries nothing but stable identifiers and the
//! identity of the catalog it was made from, so local state can never
//! reconstitute a deployment name or an endpoint.
#![allow(
    clippy::expect_used,
    reason = "a fixture that does not hold is a broken test, and it must stop the run"
)]

use std::collections::VecDeque;
use std::sync::Mutex;

use async_trait::async_trait;
use desktop_cloud::{
    resolve_model_selection, CloudError, HttpExecutor, HttpResponse, ModelCatalogRequest,
    ModelSelectionBlocked, ModelSelectionOutcome, ModelSelectionSource, OutboundHttpRequest,
    PersistedModelSelection, ResolvedModelSelection, SupportApiOrigin, SupportApiTransport,
    VerifiedModelCatalog,
};
use desktop_runtime::generated_contracts as canonical;
use desktop_runtime::UnixMillis;
use serde_json::{json, Value};
use zeroize::Zeroizing;

struct ScriptedExecutor {
    responses: Mutex<VecDeque<HttpResponse>>,
}

#[async_trait]
impl HttpExecutor for ScriptedExecutor {
    async fn execute(&self, _request: OutboundHttpRequest) -> Result<HttpResponse, CloudError> {
        self.responses
            .lock()
            .expect("response lock")
            .pop_front()
            .ok_or(CloudError::TransportFailed)
    }
}

fn profile(profile_id: &str, variant_ids: &[&str]) -> Value {
    let variants = variant_ids
        .iter()
        .map(|variant_id| {
            json!({
                "variantId": variant_id,
                "displayName": "Variant",
                "reasoningEffort": "medium",
                "maxOutputTokens": 8_000
            })
        })
        .collect::<Vec<_>>();
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
        "variants": variants,
        "defaultVariantId": variant_ids.first().expect("at least one variant"),
        "qualifiedAt": "2026-07-28T00:00:00.000Z"
    })
}

fn catalog_value(version: &str, hash_nibble: char) -> Value {
    json!({
        "schemaVersion": "sapphirus.model-catalog.v1",
        "catalogVersion": version,
        "catalogHash": format!("sha256:{}", String::from(hash_nibble).repeat(64)),
        "catalogState": "fresh",
        "policyVersion": 3,
        "policyHash": "sha256:2222222222222222222222222222222222222222222222222222222222222222",
        "deploymentSnapshotHash":
            "sha256:3333333333333333333333333333333333333333333333333333333333333333",
        "generatedAt": "2026-07-28T00:00:00.000Z",
        "expiresAt": "2026-07-28T00:05:00.000Z",
        "defaultProfileId": "fixture-mid",
        "profiles": [
            profile("fixture-mid", &["balanced", "thorough"]),
            profile("fixture-alpha", &["fast"])
        ],
        "unavailableProfiles": []
    })
}

/// Verifies one catalog body through the only path that produces authority.
async fn verified(catalog: &Value) -> VerifiedModelCatalog {
    let body = serde_json::to_string(catalog).expect("fixture catalog serializes");
    let origin = SupportApiOrigin::new("https://support.example.test").expect("synthetic origin");
    let transport = SupportApiTransport::new(
        origin,
        ScriptedExecutor {
            responses: Mutex::new(VecDeque::from(vec![HttpResponse::new(
                200,
                Some("application/json".to_owned()),
                Some(body.len() as u64),
                body.as_bytes().to_vec(),
            )])),
        },
    );
    transport
        .fetch_model_catalog(
            Zeroizing::new("token".to_owned()),
            &ModelCatalogRequest {
                subject_hash: "sha256:fixture-subject",
                cached: None,
                idempotency_key: "selection-fixture",
            },
            UnixMillis(1),
        )
        .await
        .expect("fixture catalog verifies")
        .cached()
        .catalog()
        .clone()
}

fn persisted(profile_id: &str, variant_id: &str, selected_at: u64) -> PersistedModelSelection {
    PersistedModelSelection {
        profile_id: profile_id.to_owned(),
        variant_id: variant_id.to_owned(),
        catalog_version: "v7.00112233445566aa".to_owned(),
        catalog_hash: format!("sha256:{}", "1".repeat(64)),
        selected_at: UnixMillis(selected_at),
    }
}

/// The `(profile, variant, rule)` a resolution settled on, or `None` if it did
/// not settle on one.
fn settled(outcome: &ModelSelectionOutcome) -> Option<(&str, &str, ModelSelectionSource)> {
    match outcome {
        ModelSelectionOutcome::Selected { selection, source } => {
            Some((selection.profile_id(), selection.variant_id(), *source))
        }
        _ => None,
    }
}

fn resolved(outcome: ModelSelectionOutcome) -> Option<ResolvedModelSelection> {
    match outcome {
        ModelSelectionOutcome::Selected { selection, .. } => Some(selection),
        _ => None,
    }
}

fn refused(
    outcome: &ModelSelectionOutcome,
) -> Option<(&PersistedModelSelection, ModelSelectionBlocked)> {
    match outcome {
        ModelSelectionOutcome::RequiresExplicitChoice { previous, blocked } => {
            Some((previous, *blocked))
        }
        _ => None,
    }
}

#[tokio::test]
async fn a_still_available_current_selection_is_carried_forward_unchanged() {
    let catalog = verified(&catalog_value("v7.00112233445566aa", '1')).await;
    let current = persisted("fixture-mid", "thorough", 10);

    let outcome = resolve_model_selection(&catalog, Some(&current), &[]);

    assert_eq!(
        settled(&outcome).expect("a resolved selection"),
        (
            "fixture-mid",
            "thorough",
            ModelSelectionSource::CurrentSelection
        )
    );
}

#[tokio::test]
async fn a_removed_current_profile_requires_an_explicit_choice_and_never_switches_silently() {
    let catalog = verified(&catalog_value("v7.00112233445566aa", '1')).await;
    let current = persisted("fixture-gone", "balanced", 10);
    // A perfectly good recent selection *and* a catalog default both exist. If
    // resolution ever fell through to either, the user would be answered by a
    // model they never chose.
    let recent = vec![persisted("fixture-alpha", "fast", 9)];

    let outcome = resolve_model_selection(&catalog, Some(&current), &recent);

    let (previous, blocked) = refused(&outcome).expect("an explicit choice is required");
    assert_eq!(*previous, current);
    assert_eq!(blocked, ModelSelectionBlocked::ProfileRemoved);
    assert!(settled(&outcome).is_none());
}

#[tokio::test]
async fn an_unavailable_current_profile_surfaces_the_catalogs_own_reason_and_recovery_action() {
    let mut value = catalog_value("v7.00112233445566aa", '1');
    value["unavailableProfiles"] = json!([
        {
            "profileId": "fixture-blocked",
            "displayName": "Fixture Blocked",
            "availability": "deployment_missing",
            "recoveryAction": "contact_administrator"
        }
    ]);
    let catalog = verified(&value).await;
    let current = persisted("fixture-blocked", "balanced", 10);

    let outcome = resolve_model_selection(&catalog, Some(&current), &[]);

    let (_, blocked) = refused(&outcome).expect("an explicit choice is required");
    assert_eq!(
        blocked,
        ModelSelectionBlocked::ProfileUnavailable {
            availability: canonical::ModelCatalogBlockedAvailability::DeploymentMissing,
            recovery_action: canonical::ModelCatalogModelRecoveryAction::ContactAdministrator,
        }
    );
}

#[tokio::test]
async fn a_surviving_profile_whose_variant_disappeared_still_requires_an_explicit_choice() {
    let catalog = verified(&catalog_value("v7.00112233445566aa", '1')).await;
    let current = persisted("fixture-alpha", "thorough", 10);

    let outcome = resolve_model_selection(&catalog, Some(&current), &[]);

    let (_, blocked) = refused(&outcome).expect("an explicit choice is required");
    assert_eq!(blocked, ModelSelectionBlocked::VariantRemoved);
}

#[tokio::test]
async fn with_no_current_selection_the_most_recent_still_valid_one_is_restored() {
    let catalog = verified(&catalog_value("v7.00112233445566aa", '1')).await;
    let recent = vec![
        persisted("fixture-mid", "balanced", 5),
        // Newest *and* still valid, so it wins over both the older entry and
        // the catalog default.
        persisted("fixture-alpha", "fast", 20),
        // Newer still, but no longer in the catalog, so it is skipped rather
        // than blocking resolution: the user is not mid-choice here.
        persisted("fixture-gone", "balanced", 30),
    ];

    let outcome = resolve_model_selection(&catalog, None, &recent);

    assert_eq!(
        settled(&outcome).expect("a resolved selection"),
        (
            "fixture-alpha",
            "fast",
            ModelSelectionSource::RecentSelection
        )
    );
}

#[tokio::test]
async fn recent_selections_recorded_in_the_same_millisecond_resolve_deterministically() {
    let catalog = verified(&catalog_value("v7.00112233445566aa", '1')).await;
    let forward = vec![
        persisted("fixture-mid", "balanced", 7),
        persisted("fixture-alpha", "fast", 7),
    ];
    let reversed = vec![
        persisted("fixture-alpha", "fast", 7),
        persisted("fixture-mid", "balanced", 7),
    ];

    let first = resolve_model_selection(&catalog, None, &forward);
    let second = resolve_model_selection(&catalog, None, &reversed);

    assert_eq!(
        settled(&first).expect("a resolved selection"),
        settled(&second).expect("a resolved selection")
    );
    assert_eq!(
        settled(&first).expect("a resolved selection").0,
        "fixture-alpha"
    );
}

#[tokio::test]
async fn with_no_usable_history_the_catalog_default_and_its_own_default_variant_are_adopted() {
    let catalog = verified(&catalog_value("v7.00112233445566aa", '1')).await;
    let recent = vec![persisted("fixture-gone", "balanced", 30)];

    let outcome = resolve_model_selection(&catalog, None, &recent);

    assert_eq!(
        settled(&outcome).expect("a resolved selection"),
        (
            "fixture-mid",
            "balanced",
            ModelSelectionSource::CatalogDefault
        )
    );
}

#[tokio::test]
async fn without_a_server_default_the_first_selectable_profile_is_chosen_deterministically() {
    let mut value = catalog_value("v7.00112233445566aa", '1');
    // A catalog may legitimately publish no default. The array is ordered so
    // that "first" cannot accidentally mean "array position".
    value["defaultProfileId"] = Value::Null;
    value["profiles"] = json!([
        profile("fixture-zulu", &["fast"]),
        profile("fixture-alpha", &["fast"])
    ]);
    let catalog = verified(&value).await;

    let outcome = resolve_model_selection(&catalog, None, &[]);

    assert_eq!(
        settled(&outcome).expect("a resolved selection"),
        (
            "fixture-alpha",
            "fast",
            ModelSelectionSource::FirstSelectableProfile
        )
    );
}

#[tokio::test]
async fn a_catalog_with_nothing_selectable_resolves_to_no_selection_at_all() {
    let mut value = catalog_value("v7.00112233445566aa", '1');
    value["defaultProfileId"] = Value::Null;
    value["profiles"] = json!([]);
    let catalog = verified(&value).await;

    // A user who had chosen a model is told their model is gone, not handed a
    // different one.
    let with_history = resolve_model_selection(
        &catalog,
        Some(&persisted("fixture-mid", "balanced", 1)),
        &[],
    );
    assert_eq!(
        refused(&with_history)
            .expect("an explicit choice is required")
            .1,
        ModelSelectionBlocked::ProfileRemoved
    );

    assert_eq!(
        resolve_model_selection(&catalog, None, &[]),
        ModelSelectionOutcome::NoneSelectable
    );
}

#[tokio::test]
async fn a_catalog_profile_or_variant_change_invalidates_everything_prepared_under_the_old_one() {
    let first = verified(&catalog_value("v7.00112233445566aa", '1')).await;
    // The same profiles under a different catalog identity.
    let second = verified(&catalog_value("v8.00112233445566bb", '5')).await;
    let current = persisted("fixture-mid", "balanced", 10);

    let before = resolved(resolve_model_selection(&first, Some(&current), &[]))
        .expect("a resolved selection");
    let after = resolved(resolve_model_selection(&second, Some(&current), &[]))
        .expect("a resolved selection");

    assert_eq!(before.profile_id(), after.profile_id());
    assert_eq!(before.variant_id(), after.variant_id());
    // The same model under a different catalog: context estimates, consent, and
    // prepared requests were all sized against the old catalog and are stale.
    assert!(before.invalidates_prepared_work(&after));
    assert!(!before.invalidates_prepared_work(&before.clone()));

    // A variant change under one catalog invalidates just the same.
    let other_variant = resolved(resolve_model_selection(
        &first,
        Some(&persisted("fixture-mid", "thorough", 11)),
        &[],
    ))
    .expect("the other variant is selectable");
    assert!(before.invalidates_prepared_work(&other_variant));

    // A different profile, likewise.
    let other_profile = resolved(resolve_model_selection(
        &first,
        Some(&persisted("fixture-alpha", "fast", 12)),
        &[],
    ))
    .expect("the other profile is selectable");
    assert!(before.invalidates_prepared_work(&other_profile));
}

#[tokio::test]
async fn a_persisted_selection_carries_only_stable_identifiers_and_catalog_identity() {
    let catalog = verified(&catalog_value("v7.00112233445566aa", '1')).await;
    let current = persisted("fixture-mid", "balanced", 10);

    let selection = resolved(resolve_model_selection(&catalog, Some(&current), &[]))
        .expect("the current selection is available");

    // Everything the desktop persists or resolves is a stable identifier or a
    // catalog digest. Nothing here can name a deployment or an endpoint.
    let rendered = format!("{selection:?} {current:?}").to_lowercase();
    for forbidden in [
        "endpoint",
        "deployment",
        "subscription",
        "resource",
        "api-key",
        "azure",
    ] {
        assert!(!rendered.contains(forbidden), "{rendered}");
    }
    assert_eq!(selection.catalog_version(), catalog.catalog_version());
    assert_eq!(selection.catalog_hash(), catalog.catalog_hash());
}
