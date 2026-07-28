#![allow(
    clippy::expect_used,
    reason = "static fixture contract failures should stop the test"
)]

use serde_json::Value;
use std::collections::BTreeSet;

const CANONICAL_FIXTURES: &str =
    include_str!("../../../tools/foundry-fake/fixtures/foundry-fixtures.v1.json");
const RUST_FIXTURES: &str = include_str!("fixtures/foundry/foundry-fixtures.v1.json");

#[test]
fn foundry_fixture_contract_catalog_is_complete_bounded_and_mirrored() {
    assert_eq!(
        CANONICAL_FIXTURES, RUST_FIXTURES,
        "Rust fixture mirror must be byte-identical to the canonical fake catalog"
    );
    let catalog: Value =
        serde_json::from_str(CANONICAL_FIXTURES).expect("valid Foundry fixture JSON");
    assert_eq!(catalog["schemaVersion"], "forge.foundry-fake-fixtures.v1");
    assert_eq!(
        catalog["syntheticBearerMarker"],
        "forge-foundry-fake-marker-v1"
    );

    let deployment_names = scenario_names(&catalog["deploymentScenarios"]);
    let expected_deployments = BTreeSet::from([
        "deployment_removal",
        "duplicate_deployments",
        "empty_inventory",
        "malformed_json",
        "multiple_pages",
        "one_page",
        "oversized_body",
        "unknown_fields",
        "wrong_next_link_origin",
    ]);
    assert_eq!(deployment_names, expected_deployments);

    let response_names = scenario_names(&catalog["responseScenarios"]);
    let expected_responses = BTreeSet::from([
        "client_cancellation",
        "content_filter",
        "deployment_missing_404",
        "forbidden_403",
        "malformed_sse",
        "rate_limited_429",
        "refusal",
        "server_error_500",
        "streaming_text_deltas",
        "streaming_tool_call_deltas",
        "streaming_usage",
        "structured_output",
        "timeout",
        "truncated_stream",
    ]);
    assert_eq!(response_names, expected_responses);

    let maximum_body_bytes = catalog["limits"]["maximumFixtureBodyBytes"]
        .as_u64()
        .expect("fixture body limit");
    assert_eq!(maximum_body_bytes, 1_048_577);
    for group_name in ["deploymentScenarios", "responseScenarios"] {
        let scenarios = catalog[group_name].as_array().expect("scenario array");
        for scenario in scenarios {
            let responses = scenario["responses"].as_array().expect("response queue");
            assert!(
                !responses.is_empty() && responses.len() <= 64,
                "every scenario queue is finite and non-empty"
            );
            for response in responses {
                let body_size = fixture_body_size(response);
                assert!(
                    body_size <= maximum_body_bytes,
                    "fixture body exceeds declared hard bound"
                );
            }
        }
    }

    let wrong_origin = scenario(&catalog["deploymentScenarios"], "wrong_next_link_origin");
    assert!(
        wrong_origin
            .to_string()
            .contains("https://attacker.invalid/"),
        "wrong-origin fixture must exercise cross-origin rejection"
    );
    assert!(
        !CANONICAL_FIXTURES.contains("Authorization:"),
        "fixture catalog must not persist authorization header values"
    );
    assert!(!CANONICAL_FIXTURES.contains("client_secret"));
    assert!(!CANONICAL_FIXTURES.contains("api-key"));
}

fn scenario_names(value: &Value) -> BTreeSet<&str> {
    value
        .as_array()
        .expect("scenario array")
        .iter()
        .map(|scenario| scenario["name"].as_str().expect("scenario name"))
        .collect()
}

fn scenario<'a>(value: &'a Value, name: &str) -> &'a Value {
    value
        .as_array()
        .expect("scenario array")
        .iter()
        .find(|candidate| candidate["name"] == name)
        .expect("named scenario")
}

fn fixture_body_size(response: &Value) -> u64 {
    if let Some(json_body) = response.get("jsonBody") {
        let bytes = serde_json::to_vec(json_body).expect("serializable JSON fixture");
        return u64::try_from(bytes.len()).expect("JSON body length fits u64");
    }
    if let Some(body) = response["body"].as_str() {
        return u64::try_from(body.len()).expect("body length fits u64");
    }
    if let Some(repeat) = response.get("repeatBody") {
        let value = repeat["value"].as_str().expect("repeat value");
        let count = repeat["count"].as_u64().expect("repeat count");
        let value_bytes = u64::try_from(value.len()).expect("repeat length fits u64");
        return value_bytes
            .checked_mul(count)
            .expect("bounded fixture multiplication");
    }
    assert_eq!(
        response["mode"], "wait_for_cancellation",
        "bodyless fixture must be an explicit waiting response"
    );
    0
}
