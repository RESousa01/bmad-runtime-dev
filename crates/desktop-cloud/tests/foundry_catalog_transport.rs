//! FND-040: the model-catalog route is the only way a catalog enters the
//! desktop, and nothing that fails a projection gate ever becomes one.
//!
//! The invariant under test is that a catalog is authority only after it has
//! been fetched from the reviewed route under the fixed origin, decoded through
//! the generated contract, and found internally consistent and safe to render.
//! Cache metadata is a separate authority from catalog content: an `ETag` is
//! echoed, never derived from a body, and a `304` reuses a cached catalog only
//! when that catalog belongs to the current subject and has not expired.
#![allow(
    clippy::expect_used,
    reason = "a fixture or scripted response that does not hold is a broken test, and it must stop the run"
)]

use std::collections::VecDeque;
use std::sync::Mutex;

use async_trait::async_trait;
use desktop_cloud::{
    CachedModelCatalog, CloudError, HttpExecutor, HttpMethod, HttpResponse, ModelCatalogFetch,
    ModelCatalogRequest, OutboundHttpRequest, SupportApiOrigin, SupportApiTransport,
};
use desktop_runtime::UnixMillis;
use serde_json::{json, Value};
use zeroize::Zeroizing;

const SUBJECT: &str = "sha256:aaaa1111bbbb2222cccc3333dddd4444eeee5555ffff6666aaaa7777bbbb8888";
const OTHER_SUBJECT: &str =
    "sha256:9999888877776666555544443333222211110000aaaabbbbccccddddeeeeffff";
const CATALOG_ETAG: &str = "\"fixture-catalog-1\"";
const MAX_CATALOG_RESPONSE_BYTES: usize = 256 * 1024;

struct SeenRequest {
    method: HttpMethod,
    url: String,
    body_len: usize,
    if_none_match: Option<String>,
    debug: String,
}

struct ScriptedExecutor {
    seen: Mutex<Vec<SeenRequest>>,
    responses: Mutex<VecDeque<HttpResponse>>,
}

#[async_trait]
impl HttpExecutor for ScriptedExecutor {
    async fn execute(&self, request: OutboundHttpRequest) -> Result<HttpResponse, CloudError> {
        self.seen.lock().expect("seen lock").push(SeenRequest {
            method: request.method(),
            url: request.url().as_str().to_owned(),
            body_len: request.body().len(),
            if_none_match: request.if_none_match().map(str::to_owned),
            debug: format!("{request:?}"),
        });
        self.responses
            .lock()
            .expect("response lock")
            .pop_front()
            .ok_or(CloudError::TransportFailed)
    }
}

fn transport(responses: Vec<HttpResponse>) -> SupportApiTransport<ScriptedExecutor> {
    let origin = SupportApiOrigin::new("https://support.example.test").expect("synthetic origin");
    SupportApiTransport::new(
        origin,
        ScriptedExecutor {
            seen: Mutex::new(Vec::new()),
            responses: Mutex::new(responses.into()),
        },
    )
}

fn json_response(status: u16, body: &str, etag: Option<&str>) -> HttpResponse {
    HttpResponse::new(
        status,
        Some("application/json".to_owned()),
        Some(body.len() as u64),
        body.as_bytes().to_vec(),
    )
    .with_etag(etag.map(str::to_owned))
}

fn not_modified(etag: Option<&str>) -> HttpResponse {
    HttpResponse::new(304, None, Some(0), Vec::new()).with_etag(etag.map(str::to_owned))
}

fn status_only(status: u16) -> HttpResponse {
    HttpResponse::new(
        status,
        Some("application/json".to_owned()),
        Some(2),
        b"{}".to_vec(),
    )
}

fn profile(profile_id: &str, variant_id: &str) -> Value {
    json!({
        "profileId": profile_id,
        "displayName": "Fixture Balanced",
        "description": "A synthetic fixture profile. It names no deployment.",
        "family": "fixture-family",
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
        "variants": [
            {
                "variantId": variant_id,
                "displayName": "Balanced",
                "reasoningEffort": "medium",
                "maxOutputTokens": 8_000
            },
            {
                "variantId": "thorough",
                "displayName": "Thorough",
                "reasoningEffort": "high",
                "maxOutputTokens": 8_000
            }
        ],
        "defaultVariantId": variant_id,
        "qualifiedAt": "2026-07-28T00:00:00.000Z"
    })
}

fn base_catalog() -> Value {
    json!({
        "schemaVersion": "sapphirus.model-catalog.v1",
        "catalogVersion": "v7.00112233445566aa",
        "catalogHash": "sha256:1111111111111111111111111111111111111111111111111111111111111111",
        "catalogState": "fresh",
        "policyVersion": 3,
        "policyHash": "sha256:2222222222222222222222222222222222222222222222222222222222222222",
        "deploymentSnapshotHash":
            "sha256:3333333333333333333333333333333333333333333333333333333333333333",
        "generatedAt": "2026-07-28T00:00:00.000Z",
        "expiresAt": "2026-07-28T00:05:00.000Z",
        "defaultProfileId": "fixture-balanced",
        "profiles": [
            profile("fixture-balanced", "balanced"),
            profile("fixture-fast", "fast")
        ],
        "unavailableProfiles": [
            {
                "profileId": "fixture-restricted",
                "displayName": "Fixture Restricted",
                "availability": "not_entitled",
                "recoveryAction": "request_entitlement"
            }
        ]
    })
}

fn catalog_body(catalog: &Value) -> String {
    serde_json::to_string(catalog).expect("fixture catalog serializes")
}

fn request<'a>(
    subject_hash: &'a str,
    cached: Option<&'a CachedModelCatalog>,
) -> ModelCatalogRequest<'a> {
    ModelCatalogRequest {
        subject_hash,
        cached,
        idempotency_key: "catalog-fixture-1",
    }
}

/// Fetches once against a scripted 200 and returns the resulting cache entry.
async fn fetch_once() -> CachedModelCatalog {
    let transport = transport(vec![json_response(
        200,
        &catalog_body(&base_catalog()),
        Some(CATALOG_ETAG),
    )]);
    let fetched = transport
        .fetch_model_catalog(
            Zeroizing::new("token".to_owned()),
            &request(SUBJECT, None),
            UnixMillis(1),
        )
        .await
        .expect("scripted catalog fetch");
    fetched.cached().clone()
}

#[tokio::test]
async fn the_catalog_route_is_a_get_against_the_one_reviewed_path_under_the_fixed_origin() {
    let transport = transport(vec![json_response(
        200,
        &catalog_body(&base_catalog()),
        Some(CATALOG_ETAG),
    )]);
    transport
        .fetch_model_catalog(
            Zeroizing::new("token".to_owned()),
            &request(SUBJECT, None),
            UnixMillis(1),
        )
        .await
        .expect("catalog fetch");

    let seen = transport.executor_for_test().seen.lock().expect("seen");
    let observed = seen.first().expect("exactly one request");
    assert_eq!(seen.len(), 1);
    assert_eq!(observed.method, HttpMethod::Get);
    assert_eq!(
        observed.url,
        "https://support.example.test/desktop/v1/model-catalog"
    );
    // The catalog URL never inherits the model-access path, a query, or a
    // fragment, and a GET carries no body.
    assert!(!observed.url.contains("model-access"));
    assert!(!observed.url.contains('?'));
    assert!(!observed.url.contains('#'));
    assert_eq!(observed.body_len, 0);
    // No cached catalog was held, so nothing was revalidated.
    assert!(observed.if_none_match.is_none());
}

#[tokio::test]
async fn an_outbound_catalog_request_never_renders_its_bearer_or_its_body() {
    let transport = transport(vec![json_response(
        200,
        &catalog_body(&base_catalog()),
        Some(CATALOG_ETAG),
    )]);
    transport
        .fetch_model_catalog(
            Zeroizing::new("super-secret-bearer".to_owned()),
            &request(SUBJECT, None),
            UnixMillis(1),
        )
        .await
        .expect("catalog fetch");

    let seen = transport.executor_for_test().seen.lock().expect("seen");
    let debug = &seen.first().expect("one request").debug;
    assert!(!debug.contains("super-secret-bearer"), "{debug}");
    assert!(debug.contains("[REDACTED]"), "{debug}");
}

#[tokio::test]
async fn a_verified_catalog_carries_its_cache_metadata_beside_it_and_never_inside_it() {
    let cached = fetch_once().await;

    assert_eq!(cached.metadata().subject_hash(), SUBJECT);
    assert_eq!(cached.metadata().etag(), Some(CATALOG_ETAG));
    assert_eq!(cached.metadata().fetched_at(), UnixMillis(1));
    // Expiry is copied once from the validated body and then governs reuse on
    // its own; the catalog content is never re-read to decide freshness.
    assert_eq!(
        cached.metadata().expires_at(),
        cached.catalog().expires_at()
    );
    assert!(cached.catalog().expires_at().0 > cached.catalog().generated_at().0);

    let catalog = cached.catalog();
    assert_eq!(catalog.catalog_version(), "v7.00112233445566aa");
    assert_eq!(catalog.default_profile_id(), Some("fixture-balanced"));
    assert_eq!(
        catalog.selectable_profile_ids(),
        vec!["fixture-balanced", "fixture-fast"]
    );
    assert_eq!(
        catalog.variant_ids("fixture-balanced"),
        vec!["balanced", "thorough"]
    );
    assert!(!catalog.is_selectable("fixture-restricted"));
    // The Azure-free projection is the whole point: no rendered form of a
    // verified catalog may name a deployment or an endpoint.
    let rendered = format!("{catalog:?}");
    assert!(!rendered.contains("azure"), "{rendered}");
    assert!(rendered.contains("[REDACTED]"), "{rendered}");
}

#[tokio::test]
async fn a_held_catalog_is_offered_as_a_validator_and_a_matching_not_modified_reuses_it() {
    let cached = fetch_once().await;
    let transport = transport(vec![not_modified(Some(CATALOG_ETAG))]);
    let now = UnixMillis(cached.metadata().expires_at().0 - 1);

    let outcome = transport
        .fetch_model_catalog(
            Zeroizing::new("token".to_owned()),
            &request(SUBJECT, Some(&cached)),
            now,
        )
        .await
        .expect("revalidated catalog");

    assert!(matches!(outcome, ModelCatalogFetch::Revalidated(_)));
    assert_eq!(
        outcome.cached().catalog().catalog_hash(),
        cached.catalog().catalog_hash()
    );
    // A 304 does not extend validity: expiry came from a body, and no body was
    // received.
    assert_eq!(
        outcome.cached().metadata().expires_at(),
        cached.metadata().expires_at()
    );
    let seen = transport.executor_for_test().seen.lock().expect("seen");
    assert_eq!(
        seen.first().expect("one request").if_none_match.as_deref(),
        Some(CATALOG_ETAG)
    );
}

#[tokio::test]
async fn a_not_modified_over_an_expired_cache_reports_the_catalog_unavailable() {
    let cached = fetch_once().await;
    let transport = transport(vec![not_modified(Some(CATALOG_ETAG))]);
    // Exactly at the expiry instant the catalog is already unusable.
    let now = cached.metadata().expires_at();

    let outcome = transport
        .fetch_model_catalog(
            Zeroizing::new("token".to_owned()),
            &request(SUBJECT, Some(&cached)),
            now,
        )
        .await;

    assert!(matches!(outcome, Err(CloudError::ModelCatalogUnavailable)));
}

#[tokio::test]
async fn an_unsolicited_or_body_carrying_not_modified_is_never_treated_as_a_cache_hit() {
    // Nothing was revalidated, so a 304 is a protocol violation.
    let unsolicited = transport(vec![not_modified(Some(CATALOG_ETAG))]);
    assert!(matches!(
        unsolicited
            .fetch_model_catalog(
                Zeroizing::new("token".to_owned()),
                &request(SUBJECT, None),
                UnixMillis(1),
            )
            .await,
        Err(CloudError::TransportFailed)
    ));

    let cached = fetch_once().await;
    let now = UnixMillis(cached.metadata().expires_at().0 - 1);

    // A 304 carrying a representation is smuggling.
    let with_body = transport(vec![HttpResponse::new(
        304,
        Some("application/json".to_owned()),
        Some(2),
        b"{}".to_vec(),
    )
    .with_etag(Some(CATALOG_ETAG.to_owned()))]);
    assert!(matches!(
        with_body
            .fetch_model_catalog(
                Zeroizing::new("token".to_owned()),
                &request(SUBJECT, Some(&cached)),
                now,
            )
            .await,
        Err(CloudError::ModelCatalogIntegrity)
    ));

    // A 304 that answers a different validator than the one sent is not a hit.
    let wrong_validator = transport(vec![not_modified(Some("\"some-other-catalog\""))]);
    assert!(matches!(
        wrong_validator
            .fetch_model_catalog(
                Zeroizing::new("token".to_owned()),
                &request(SUBJECT, Some(&cached)),
                now,
            )
            .await,
        Err(CloudError::ModelCatalogIntegrity)
    ));
}

#[tokio::test]
async fn one_subjects_cached_catalog_is_never_offered_or_reused_for_another_subject() {
    let cached = fetch_once().await;
    let now = UnixMillis(cached.metadata().expires_at().0 - 1);
    let transport = transport(vec![not_modified(Some(CATALOG_ETAG))]);

    let outcome = transport
        .fetch_model_catalog(
            Zeroizing::new("token".to_owned()),
            &request(OTHER_SUBJECT, Some(&cached)),
            now,
        )
        .await;

    // The other subject's validator was never sent, so the scripted 304 is
    // unsolicited and cannot resolve to the first subject's catalog.
    let seen = transport.executor_for_test().seen.lock().expect("seen");
    assert!(seen.first().expect("one request").if_none_match.is_none());
    assert!(matches!(outcome, Err(CloudError::TransportFailed)));
}

#[tokio::test]
async fn a_catalog_version_that_reappears_with_a_different_hash_is_a_transplant() {
    let cached = fetch_once().await;
    let mut transplanted = base_catalog();
    transplanted["catalogHash"] =
        json!("sha256:4444444444444444444444444444444444444444444444444444444444444444");
    let transport = transport(vec![json_response(
        200,
        &catalog_body(&transplanted),
        Some("\"fixture-catalog-2\""),
    )]);

    let outcome = transport
        .fetch_model_catalog(
            Zeroizing::new("token".to_owned()),
            &request(SUBJECT, Some(&cached)),
            UnixMillis(2),
        )
        .await;

    assert!(matches!(outcome, Err(CloudError::ModelCatalogIntegrity)));
}

#[tokio::test]
async fn an_oversized_catalog_response_fails_before_anything_is_decoded() {
    let padding = "a".repeat(MAX_CATALOG_RESPONSE_BYTES);
    let body = format!("{{\"padding\":\"{padding}\"}}");
    assert!(body.len() > MAX_CATALOG_RESPONSE_BYTES);
    let transport = transport(vec![json_response(200, &body, Some(CATALOG_ETAG))]);

    let outcome = transport
        .fetch_model_catalog(
            Zeroizing::new("token".to_owned()),
            &request(SUBJECT, None),
            UnixMillis(1),
        )
        .await;

    assert!(matches!(outcome, Err(CloudError::TransportFailed)));
}

#[tokio::test]
async fn a_non_json_catalog_body_is_opaque() {
    let transport = transport(vec![HttpResponse::new(
        200,
        Some("text/html".to_owned()),
        Some(6),
        b"<html>".to_vec(),
    )]);

    assert!(matches!(
        transport
            .fetch_model_catalog(
                Zeroizing::new("token".to_owned()),
                &request(SUBJECT, None),
                UnixMillis(1),
            )
            .await,
        Err(CloudError::ModelCatalogIntegrity)
    ));
}

#[tokio::test]
async fn every_classified_catalog_status_maps_to_one_stable_secret_free_error() {
    for (status, expected) in [
        (401_u16, CloudError::ReauthenticationRequired),
        (403, CloudError::EntitlementUnavailable),
        (409, CloudError::ContractUpgradeRequired),
        (426, CloudError::ContractUpgradeRequired),
        (503, CloudError::ModelCatalogUnavailable),
        (500, CloudError::TransportFailed),
        (404, CloudError::TransportFailed),
        (302, CloudError::TransportFailed),
    ] {
        let transport = transport(vec![status_only(status)]);
        let outcome = transport
            .fetch_model_catalog(
                Zeroizing::new("token".to_owned()),
                &request(SUBJECT, None),
                UnixMillis(1),
            )
            .await;
        assert_eq!(outcome.err(), Some(expected), "status {status}");
    }
}

#[tokio::test]
async fn a_malformed_local_catalog_request_never_reaches_the_network() {
    for (subject, key) in [
        ("", "catalog-fixture-1"),
        (SUBJECT, ""),
        ("subject with spaces", "catalog-fixture-1"),
        ("subject\nwith-newline", "catalog-fixture-1"),
    ] {
        let transport = transport(vec![json_response(
            200,
            &catalog_body(&base_catalog()),
            Some(CATALOG_ETAG),
        )]);
        let outcome = transport
            .fetch_model_catalog(
                Zeroizing::new("token".to_owned()),
                &ModelCatalogRequest {
                    subject_hash: subject,
                    cached: None,
                    idempotency_key: key,
                },
                UnixMillis(1),
            )
            .await;
        assert!(matches!(outcome, Err(CloudError::TransportFailed)));
        assert!(transport
            .executor_for_test()
            .seen
            .lock()
            .expect("seen")
            .is_empty());
    }
}

/// Bodies whose shape is wrong: an unreviewed property, a mis-cased field, an
/// unrecognized schema, or a property the schema requires that was dropped
/// rather than nulled.
fn structural_cases() -> Vec<(&'static str, Value)> {
    let mut cases: Vec<(&str, Value)> = Vec::new();

    let mut unknown_property = base_catalog();
    unknown_property["deploymentName"] = json!("some-deployment");
    cases.push((
        "unknown property carrying azure authority",
        unknown_property,
    ));

    let mut wrong_case = base_catalog();
    let version = wrong_case["catalogVersion"].clone();
    wrong_case
        .as_object_mut()
        .expect("object")
        .remove("catalogVersion");
    wrong_case["catalogversion"] = version;
    cases.push(("field name in the wrong case", wrong_case));

    let mut wrong_schema = base_catalog();
    wrong_schema["schemaVersion"] = json!("sapphirus.model-catalog.v2");
    cases.push(("unrecognized schema version", wrong_schema));

    let mut absent_default = base_catalog();
    absent_default
        .as_object_mut()
        .expect("object")
        .remove("defaultProfileId");
    cases.push(("default property missing entirely", absent_default));

    // Every nullable-but-required property must be *present*, not merely
    // absent-and-therefore-null. Dropping `qualifiedAt` is how a profile's
    // qualification evidence would otherwise disappear with nothing failing.
    for property in ["family", "statusDetail", "qualifiedAt"] {
        let mut dropped = base_catalog();
        dropped["profiles"][0]
            .as_object_mut()
            .expect("object")
            .remove(property);
        cases.push(("profile property dropped rather than nulled", dropped));
    }
    for property in [
        "contextTokens",
        "maxInputTokens",
        "maxOutputTokens",
        "maxAttachmentBytes",
    ] {
        let mut dropped = base_catalog();
        dropped["profiles"][0]["limits"]
            .as_object_mut()
            .expect("object")
            .remove(property);
        cases.push(("limit dropped rather than nulled", dropped));
    }

    cases
}

/// Bodies whose content contradicts itself: impossible instants, dangling
/// defaults, duplicate identifiers, or bounds the schema does not admit.
fn coherence_cases() -> Vec<(&'static str, Value)> {
    let mut cases: Vec<(&str, Value)> = Vec::new();

    let mut impossible_date = base_catalog();
    impossible_date["generatedAt"] = json!("2026-02-31T00:00:00.000Z");
    cases.push(("calendar-impossible instant", impossible_date));

    let mut inverted_window = base_catalog();
    inverted_window["expiresAt"] = json!("2026-07-27T00:00:00.000Z");
    cases.push(("expiry at or before generation", inverted_window));

    let mut impossible_qualification = base_catalog();
    impossible_qualification["profiles"][0]["qualifiedAt"] = json!("2026-02-30T00:00:00.000Z");
    cases.push((
        "calendar-impossible qualification",
        impossible_qualification,
    ));

    let mut unknown_default = base_catalog();
    unknown_default["defaultProfileId"] = json!("fixture-absent");
    cases.push(("default naming no selectable profile", unknown_default));

    let mut blocked_default = base_catalog();
    blocked_default["defaultProfileId"] = json!("fixture-restricted");
    cases.push(("default naming an unavailable profile", blocked_default));

    let mut duplicate_profiles = base_catalog();
    duplicate_profiles["profiles"][1]["profileId"] = json!("fixture-balanced");
    cases.push(("duplicate selectable profile id", duplicate_profiles));

    let mut straddling_profile = base_catalog();
    straddling_profile["unavailableProfiles"][0]["profileId"] = json!("fixture-balanced");
    cases.push((
        "profile both selectable and unavailable",
        straddling_profile,
    ));

    let mut blocked_availability = base_catalog();
    blocked_availability["profiles"][0]["availability"] = json!("policy_blocked");
    cases.push((
        "blocked availability on a selectable profile",
        blocked_availability,
    ));

    let mut foreign_provider = base_catalog();
    foreign_provider["profiles"][0]["provider"] = json!("some_other_provider");
    cases.push(("profile naming a foreign provider", foreign_provider));

    let mut duplicate_variants = base_catalog();
    duplicate_variants["profiles"][0]["variants"][1]["variantId"] = json!("balanced");
    cases.push(("duplicate variant id", duplicate_variants));

    let mut absent_default_variant = base_catalog();
    absent_default_variant["profiles"][0]["defaultVariantId"] = json!("absent");
    cases.push((
        "default variant absent from variants",
        absent_default_variant,
    ));

    let mut no_variants = base_catalog();
    no_variants["profiles"][0]["variants"] = json!([]);
    cases.push(("profile with no variants", no_variants));

    let mut too_many_variants = base_catalog();
    let variant = too_many_variants["profiles"][0]["variants"][0].clone();
    let variants = (0..9)
        .map(|index| {
            let mut clone = variant.clone();
            clone["variantId"] = json!(format!("v{index}a"));
            clone
        })
        .collect::<Vec<_>>();
    too_many_variants["profiles"][0]["variants"] = json!(variants);
    too_many_variants["profiles"][0]["defaultVariantId"] = json!("v0a");
    cases.push(("more variants than the schema admits", too_many_variants));

    let mut too_many_profiles = base_catalog();
    let profiles = (0..65)
        .map(|index| profile(&format!("fixture-p{index}"), "balanced"))
        .collect::<Vec<_>>();
    too_many_profiles["profiles"] = json!(profiles);
    too_many_profiles["defaultProfileId"] = json!("fixture-p0");
    cases.push(("more profiles than the schema admits", too_many_profiles));

    cases
}

/// Bodies carrying display text that is unsafe to hand to the renderer.
///
/// The renderer is untrusted and these strings reach it, so display text is
/// treated as adversarial input rather than as data an organization vouched
/// for. The invisible ones matter most: they are how one profile name becomes a
/// visual twin of another next to a model the user is about to trust.
fn display_text_cases() -> Vec<(&'static str, Value)> {
    let mut cases: Vec<(&str, Value)> = Vec::new();

    let mut control_character = base_catalog();
    control_character["profiles"][0]["displayName"] = json!("Fixture\u{0007}Balanced");
    cases.push(("control character in a display name", control_character));

    let mut bidi_override = base_catalog();
    bidi_override["profiles"][0]["displayName"] = json!("\u{202e}decnalaB erutxiF");
    cases.push(("bidi override in a display name", bidi_override));

    let mut zero_width = base_catalog();
    zero_width["profiles"][0]["description"] = json!("Fixture\u{200b}description");
    cases.push(("zero-width character in a description", zero_width));

    let mut byte_order_mark = base_catalog();
    byte_order_mark["profiles"][0]["family"] = json!("\u{feff}fixture-family");
    cases.push(("byte order mark in a family name", byte_order_mark));

    let mut status_detail = base_catalog();
    status_detail["profiles"][0]["statusDetail"] = json!("degraded\u{2069}\u{2066}capacity");
    cases.push(("bidi isolate in a status detail", status_detail));

    let mut variant_name = base_catalog();
    variant_name["profiles"][0]["variants"][0]["displayName"] = json!("Bal\u{0000}anced");
    cases.push(("null byte in a variant display name", variant_name));

    let mut unavailable_name = base_catalog();
    unavailable_name["unavailableProfiles"][0]["displayName"] = json!("Restricted\u{202d}");
    cases.push((
        "bidi override in an unavailable display name",
        unavailable_name,
    ));

    cases
}

/// Every catalog body a hostile or broken support plane could return is
/// rejected outright as an integrity failure, never partially accepted.
#[tokio::test]
async fn every_malicious_or_incoherent_catalog_body_is_rejected_as_an_integrity_failure() {
    let mut cases = structural_cases();
    cases.append(&mut coherence_cases());
    cases.append(&mut display_text_cases());
    assert!(cases.len() > 25, "the rejection table must stay broad");

    for (label, catalog) in cases {
        let body = catalog_body(&catalog);
        let transport = transport(vec![json_response(200, &body, Some(CATALOG_ETAG))]);
        let outcome = transport
            .fetch_model_catalog(
                Zeroizing::new("token".to_owned()),
                &request(SUBJECT, None),
                UnixMillis(1),
            )
            .await;
        assert_eq!(
            outcome.err(),
            Some(CloudError::ModelCatalogIntegrity),
            "{label}"
        );
    }
}

#[tokio::test]
async fn a_catalog_body_carrying_a_lone_surrogate_never_becomes_a_display_string() {
    // Written as raw bytes: a lone surrogate cannot be constructed as a Rust
    // string, which is exactly why it has to be rejected at the JSON boundary
    // rather than after decoding.
    let body = catalog_body(&base_catalog())
        .replace("\"Fixture Balanced\"", "\"Fixture \\ud800 Balanced\"");
    let transport = transport(vec![json_response(200, &body, Some(CATALOG_ETAG))]);

    assert!(matches!(
        transport
            .fetch_model_catalog(
                Zeroizing::new("token".to_owned()),
                &request(SUBJECT, None),
                UnixMillis(1),
            )
            .await,
        Err(CloudError::ModelCatalogIntegrity)
    ));
}

#[tokio::test]
async fn a_catalog_with_nothing_selectable_carries_no_default_and_is_still_verifiable() {
    let mut empty = base_catalog();
    empty["profiles"] = json!([]);
    empty["defaultProfileId"] = Value::Null;
    let transport = transport(vec![json_response(
        200,
        &catalog_body(&empty),
        Some(CATALOG_ETAG),
    )]);

    let fetched = transport
        .fetch_model_catalog(
            Zeroizing::new("token".to_owned()),
            &request(SUBJECT, None),
            UnixMillis(1),
        )
        .await
        .expect("an empty catalog is a valid catalog");

    assert!(fetched.cached().catalog().default_profile_id().is_none());
    assert!(fetched
        .cached()
        .catalog()
        .selectable_profile_ids()
        .is_empty());
}

/// An explicit `null` default is the support plane declining to name one, not a
/// broken catalog: the schema admits it, and the desktop then resolves
/// deterministically from profiles the catalog already declared invocable. A
/// *dangling* default — one naming a profile that is absent or unavailable — is
/// the failure, and it is covered by the integrity table above.
#[tokio::test]
async fn a_catalog_that_declines_to_name_a_default_is_still_a_valid_catalog() {
    let mut undecided = base_catalog();
    undecided["defaultProfileId"] = Value::Null;
    let transport = transport(vec![json_response(
        200,
        &catalog_body(&undecided),
        Some(CATALOG_ETAG),
    )]);

    let fetched = transport
        .fetch_model_catalog(
            Zeroizing::new("token".to_owned()),
            &request(SUBJECT, None),
            UnixMillis(1),
        )
        .await
        .expect("a catalog with no named default still verifies");

    assert!(fetched.cached().catalog().default_profile_id().is_none());
    assert_eq!(
        fetched.cached().catalog().selectable_profile_ids(),
        vec!["fixture-balanced", "fixture-fast"]
    );
}

#[tokio::test]
async fn a_stale_usable_catalog_is_accepted_and_reports_its_own_staleness() {
    let mut stale = base_catalog();
    stale["catalogState"] = json!("stale_usable");
    let transport = transport(vec![json_response(
        200,
        &catalog_body(&stale),
        Some(CATALOG_ETAG),
    )]);

    let fetched = transport
        .fetch_model_catalog(
            Zeroizing::new("token".to_owned()),
            &request(SUBJECT, None),
            UnixMillis(1),
        )
        .await
        .expect("a stale-usable catalog is still a catalog");

    assert_eq!(
        fetched.cached().catalog().catalog_state(),
        desktop_runtime::generated_contracts::ModelCatalogCatalogState::StaleUsable
    );
}

#[tokio::test]
async fn an_unusable_etag_is_dropped_rather_than_cached_and_replayed() {
    let transport = transport(vec![json_response(
        200,
        &catalog_body(&base_catalog()),
        Some("bad etag with spaces"),
    )]);

    let fetched = transport
        .fetch_model_catalog(
            Zeroizing::new("token".to_owned()),
            &request(SUBJECT, None),
            UnixMillis(1),
        )
        .await
        .expect("catalog fetch");

    // The catalog is still authority; only the unusable validator is discarded,
    // so the next fetch is unconditional rather than failing.
    assert!(fetched.cached().metadata().etag().is_none());
}
