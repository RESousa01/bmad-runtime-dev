#![allow(clippy::expect_used)]

use desktop_runtime::{BmadConfigResolver, BmadKernelErrorCode};
use serde_json::json;
use std::collections::BTreeSet;

const BMAD_CONFIG_SCHEMA: &str =
    include_str!("../../../packages/contracts/schemas/bmad-config.schema.json");

fn layer(kind: &str, version: u32, values: serde_json::Value) -> serde_json::Value {
    json!({"kind": kind, "version": version, "values": values})
}

#[test]
fn bmad_config_layered_config_uses_fixed_precedence_and_explains_every_effective_value() {
    // Break caught: accepting caller order, dropping provenance, or retaining a secret value
    // lets a lower-priority document silently control an invocation or leaks a credential.
    let resolution = BmadConfigResolver::resolve_layered(
        &[
            layer(
                "user",
                3,
                json!({
                    "language": "nl-BE",
                    "personalization": "Keep status updates concise.",
                    "output_folder": "user-output"
                }),
            ),
            layer(
                "defaults",
                1,
                json!({
                    "language": "en",
                    "output_folder": "artifacts",
                    "report_folder": "reports",
                    "model_preference": {"provider": "azure-openai", "model": "gpt-5"}
                }),
            ),
            json!({
                "kind": "invocation",
                "version": 7,
                "values": {"language": "fr-BE"},
                "secretReferences": {"model_api_key": "secret://workspace/model-api-key"}
            }),
            layer(
                "bundled_module",
                2,
                json!({"report_folder": "module-reports"}),
            ),
            layer("project", 5, json!({"output_folder": "project-output"})),
        ],
        false,
    )
    .expect("all supplied values are valid");

    let values = resolution.value["effectiveValues"]
        .as_array()
        .expect("explainable effective values");
    assert_eq!(values.len(), 5);
    assert_eq!(values[0]["key"], "language");
    assert_eq!(values[0]["value"], "fr-BE");
    assert_eq!(values[0]["sourceLayer"], "invocation");
    assert_eq!(values[0]["validation"], "valid");
    assert_eq!(values[0]["redaction"], "none");
    assert_eq!(values[0]["version"], 7);
    assert!(values[0]["sourceHash"]
        .as_str()
        .is_some_and(|hash| hash.starts_with("sha256:")));
    assert_eq!(values[1]["key"], "model_preference");
    assert_eq!(values[1]["sourceLayer"], "defaults");
    assert_eq!(values[2]["key"], "output_folder");
    assert_eq!(values[2]["value"], "user-output");
    assert_eq!(values[2]["sourceLayer"], "user");
    assert_eq!(values[3]["key"], "personalization");
    assert_eq!(values[4]["key"], "report_folder");
    assert_eq!(values[4]["value"], "module-reports");

    let top_level = resolution
        .value
        .as_object()
        .expect("closed config contract");
    assert_eq!(top_level.len(), 10);
    for required in [
        "schemaVersion",
        "configId",
        "layers",
        "effectiveValues",
        "secretReferences",
        "requirements",
        "conflicts",
        "resolvedHash",
        "provenanceHash",
        "sessionConfigHash",
    ] {
        assert!(
            top_level.contains_key(required),
            "runtime config misses {required}"
        );
    }
    assert_eq!(
        resolution.value["schemaVersion"],
        "sapphirus.bmad-config.v1"
    );
    assert_eq!(
        resolution.value["resolvedHash"],
        resolution.resolution_hash.to_string()
    );
    assert_eq!(
        resolution.value["sessionConfigHash"],
        resolution.resolution_hash.to_string()
    );
    assert!(resolution.value.get("warnings").is_none());

    let schema: serde_json::Value = serde_json::from_str(BMAD_CONFIG_SCHEMA).expect("schema JSON");
    assert_closed_schema_object(&schema, &resolution.value);
    let definitions = schema["$defs"].as_object().expect("schema definitions");
    for layer in resolution.value["layers"].as_array().expect("layers") {
        assert_closed_schema_object(&definitions["ConfigLayer"], layer);
    }
    for effective in resolution.value["effectiveValues"]
        .as_array()
        .expect("values")
    {
        assert_closed_schema_object(&definitions["EffectiveValue"], effective);
    }
    for secret in resolution.value["secretReferences"]
        .as_array()
        .expect("secret refs")
    {
        assert_closed_schema_object(&definitions["SecretReference"], secret);
    }

    assert_eq!(
        resolution.value["secretReferences"][0]["reference"],
        "secret://workspace/model-api-key"
    );
    assert_eq!(
        resolution.value["secretReferences"][0]["redaction"],
        "reference_only"
    );
    assert!(resolution.value.get("secretValues").is_none());
    assert!(resolution
        .warnings
        .iter()
        .any(|warning| warning.code == "config_value_overridden"));
    assert!(resolution.value["conflicts"]
        .as_array()
        .expect("deterministic override diagnostics")
        .iter()
        .any(|conflict| {
            conflict
                == &json!({
                    "code": "config_value_overridden",
                    "key": "language",
                    "previousSourceLayer": "user",
                    "nextSourceLayer": "invocation"
                })
        }));
}

fn assert_closed_schema_object(schema: &serde_json::Value, value: &serde_json::Value) {
    let object = value.as_object().expect("runtime projection is an object");
    let properties = schema["properties"]
        .as_object()
        .expect("closed schema properties");
    let required = schema["required"]
        .as_array()
        .expect("schema required fields");
    let actual = object.keys().cloned().collect::<BTreeSet<_>>();
    let allowed = properties.keys().cloned().collect::<BTreeSet<_>>();
    assert_eq!(
        actual, allowed,
        "runtime output must be closed by its schema"
    );
    for required in required {
        assert!(object.contains_key(required.as_str().expect("required field name")));
    }
}

#[test]
fn bmad_config_layered_config_rejects_unsafe_paths_unknown_values_and_secret_material() {
    // Break caught: treating an unsafe path, an unreviewed setting, or secret material as regular
    // configuration gives configuration documents authority beyond their declared data boundary.
    for values in [
        json!({"output_folder": "../outside", "report_folder": "reports", "language": "en", "model_preference": {"provider": "azure", "model": "gpt-5"}}),
        json!({"output_folder": "out", "report_folder": "reports", "language": "en", "model_preference": {"provider": "azure", "model": "gpt-5"}, "api_key": "not-allowed"}),
        json!({"output_folder": "out", "report_folder": "reports", "language": "en", "model_preference": {"provider": "azure", "model": "gpt-5"}, "unknown_setting": true}),
        json!({"output_folder": "out", "report_folder": "reports", "language": "english", "model_preference": {"provider": "azure", "model": "gpt-5"}}),
        json!({"output_folder": "out", "report_folder": "reports", "language": "en", "personalization": "   ", "model_preference": {"provider": "azure", "model": "gpt-5"}}),
        json!({"output_folder": "out", "report_folder": "reports", "language": "en", "model_preference": {"provider": "azure"}}),
        json!({"output_folder": "CON/reports", "report_folder": "reports", "language": "en", "model_preference": {"provider": "azure", "model": "gpt-5"}}),
        json!({"output_folder": "COM1", "report_folder": "reports", "language": "en", "model_preference": {"provider": "azure", "model": "gpt-5"}}),
        json!({"output_folder": "out. ", "report_folder": "reports", "language": "en", "model_preference": {"provider": "azure", "model": "gpt-5"}}),
        json!({"output_folder": "C:/out", "report_folder": "reports", "language": "en", "model_preference": {"provider": "azure", "model": "gpt-5"}}),
    ] {
        assert_eq!(
            BmadConfigResolver::resolve_layered(&[layer("defaults", 1, values)], false)
                .expect_err("unsafe configuration must fail closed")
                .code(),
            BmadKernelErrorCode::ConfigGraphInvalid
        );
    }
}

#[test]
fn bmad_config_rejects_credential_assignments_in_personalization_but_keeps_ordinary_prose() {
    // Break caught: accepting a credential assignment hidden in the only free-text field makes
    // the resolved configuration a durable secret carrier rather than a redacted projection.
    for personalization in [
        "password=hunter2",
        "api_key: sk-test",
        "Use client_secret = top-secret for deployment.",
        "private_key: -----BEGIN PRIVATE KEY-----",
        "connection_string=Server=example;Password=hunter2",
        "clientSecret=leaked-value",
        "accessToken: leaked-value",
        "apiKey = leaked-value",
        "dbPassword: leaked-value",
        "privateKey=-----BEGIN PRIVATE KEY-----",
        "connectionString = Server=example;Password=hunter2",
        "ClientSecret=leaked-value",
        "client-secret=leaked-value",
        "AccessToken: leaked-value",
        "access-token: leaked-value",
        "ApiKey = leaked-value",
        "api-key = leaked-value",
        "DbPassword: leaked-value",
        "db-password: leaked-value",
        "PrivateKey=-----BEGIN PRIVATE KEY-----",
        "private-key=-----BEGIN PRIVATE KEY-----",
        "ConnectionString = Server=example;Password=hunter2",
        "connection-string = Server=example;Password=hunter2",
        "https://alice:hunter2@example.invalid/service",
    ] {
        assert_eq!(
            BmadConfigResolver::resolve_layered(
                &[layer(
                    "defaults",
                    1,
                    json!({"language": "en", "personalization": personalization}),
                )],
                false,
            )
            .expect_err("credential-like free text must fail closed")
            .code(),
            BmadKernelErrorCode::ConfigGraphInvalid
        );
    }

    BmadConfigResolver::resolve_layered(
        &[layer(
            "defaults",
            1,
            json!({
                "language": "en",
                "personalization": "Explain why password rotation matters without including one."
            }),
        )],
        false,
    )
    .expect("ordinary prose mentioning password is not credential material");
}

#[test]
fn bmad_config_headless_missing_requirements_are_structured_and_preview_staleness_keeps_history_hashes(
) {
    // Break caught: allowing a headless execution without its required inputs, or mutating an old
    // session's hash during a config change, makes execution unreproducible.
    let incomplete = BmadConfigResolver::resolve_layered(
        &[layer("defaults", 1, json!({"language": "en"}))],
        true,
    )
    .expect("missing headless values are a structured result, not an implicit fallback");
    assert_eq!(
        incomplete.value["requirements"],
        json!([
            {"key": "model_preference", "reason": "missing_required_headless_value"},
            {"key": "output_folder", "reason": "missing_required_headless_value"},
            {"key": "report_folder", "reason": "missing_required_headless_value"}
        ])
    );

    let first = BmadConfigResolver::resolve_layered(
        &[layer(
            "defaults",
            1,
            json!({
                "language": "en",
                "output_folder": "out",
                "report_folder": "reports",
                "model_preference": {"provider": "azure", "model": "gpt-5"}
            }),
        )],
        false,
    )
    .expect("first configuration");
    let second = BmadConfigResolver::resolve_layered(
        &[layer(
            "defaults",
            2,
            json!({
                "language": "nl",
                "output_folder": "out",
                "report_folder": "reports",
                "model_preference": {"provider": "azure", "model": "gpt-5"}
            }),
        )],
        false,
    )
    .expect("changed configuration");

    assert_ne!(first.resolution_hash, second.resolution_hash);
    assert!(BmadConfigResolver::preview_is_stale(
        &first.resolution_hash,
        &second.resolution_hash
    ));
    assert!(!BmadConfigResolver::preview_is_stale(
        &first.resolution_hash,
        &first.resolution_hash
    ));
    assert_eq!(
        first.value["sessionConfigHash"],
        first.resolution_hash.to_string()
    );
}
