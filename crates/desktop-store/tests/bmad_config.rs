#![allow(clippy::expect_used)]

use desktop_runtime::{canonical_hash, BmadConfigResolver};
use desktop_store::{BmadConfigSnapshot, KeyProtector, LocalStore, StoreError};
use serde_json::json;

#[derive(Debug)]
struct TestProtector;

impl KeyProtector for TestProtector {
    fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, StoreError> {
        Ok(plaintext.to_vec())
    }

    fn unprotect(&self, protected: &[u8]) -> Result<Vec<u8>, StoreError> {
        Ok(protected.to_vec())
    }
}

fn snapshot(language: &str) -> BmadConfigSnapshot {
    let resolution = BmadConfigResolver::resolve_layered(
        &[json!({
            "kind": "defaults",
            "version": 1,
            "values": {
                "language": language,
                "personalization": "Explain why password rotation matters without including one.",
                "output_folder": "out",
                "report_folder": "reports",
                "model_preference": {"provider": "azure", "model": "gpt-5"}
            },
            "secretReferences": {"model_api_key": "secret://workspace/model-api-key"}
        })],
        false,
    )
    .expect("valid resolved configuration");
    BmadConfigSnapshot {
        config_id: resolution.value["configId"]
            .as_str()
            .expect("config id")
            .to_owned(),
        resolution_hash: resolution.resolution_hash.to_string(),
        resolved_config: resolution.value,
    }
}

#[test]
fn bmad_config_snapshots_are_append_only_and_old_session_bindings_remain_readable_after_a_change(
) -> Result<(), Box<dyn std::error::Error>> {
    // Break caught: overwriting a prior resolution lets a resumed session silently use changed
    // configuration instead of the immutable hash it was created with.
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let original = snapshot("en");
    let changed = snapshot("nl");

    let original_ref = store.store_bmad_config_snapshot(&original)?;
    let changed_ref = store.store_bmad_config_snapshot(&changed)?;

    assert_ne!(original_ref.content_hash, changed_ref.content_hash);
    assert_eq!(store.load_bmad_config_snapshot(&original_ref)?, original);
    assert_eq!(store.load_bmad_config_snapshot(&changed_ref)?, changed);
    Ok(())
}

#[test]
fn bmad_config_snapshot_rejects_nested_secret_material_malformed_references_and_hash_tampering(
) -> Result<(), Box<dyn std::error::Error>> {
    // Break caught: persisting credential material or a different session hash turns a durable
    // config snapshot into a secret store or breaks replay provenance.
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;

    let mut secret = snapshot("en");
    secret.resolved_config["effectiveValues"][0]["value"] =
        json!({"nested": {"api_key": "plaintext"}});
    assert!(matches!(
        store.store_bmad_config_snapshot(&secret),
        Err(StoreError::Inconsistent)
    ));

    let mut malformed_reference = snapshot("en");
    malformed_reference.resolved_config["secretReferences"][0]["reference"] =
        json!("vault://secret");
    assert!(matches!(
        store.store_bmad_config_snapshot(&malformed_reference),
        Err(StoreError::Inconsistent)
    ));

    let mut tampered = snapshot("en");
    tampered.resolved_config["effectiveValues"][0]["value"] = json!("nl");
    assert!(matches!(
        store.store_bmad_config_snapshot(&tampered),
        Err(StoreError::Inconsistent)
    ));
    Ok(())
}

#[test]
fn bmad_config_store_rejects_resealed_credential_text_from_direct_callers(
) -> Result<(), Box<dyn std::error::Error>> {
    // Break caught: a direct store caller that recomputes the public hashes can otherwise embed
    // a credential assignment in a personalization value that runtime validation would reject.
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let mut direct = snapshot("en");
    let personalization = direct.resolved_config["effectiveValues"]
        .as_array_mut()
        .expect("effective values")
        .iter_mut()
        .find(|entry| entry["key"] == "personalization")
        .expect("personalization value");
    personalization["value"] = json!("nested note: client_secret = leaked-value");
    reseal(&mut direct)?;

    assert!(matches!(
        store.store_bmad_config_snapshot(&direct),
        Err(StoreError::Inconsistent)
    ));
    Ok(())
}

#[test]
fn bmad_config_store_rejects_resealed_camel_case_credential_text_from_direct_callers(
) -> Result<(), Box<dyn std::error::Error>> {
    // Break caught: direct callers can reseal a snapshot, so the store must reject the same
    // camel/PascalCase credential assignments that the runtime boundary rejects.
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    for text in [
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
    ] {
        let mut direct = snapshot("en");
        let personalization = direct.resolved_config["effectiveValues"]
            .as_array_mut()
            .expect("effective values")
            .iter_mut()
            .find(|entry| entry["key"] == "personalization")
            .expect("personalization value");
        personalization["value"] = json!(text);
        reseal(&mut direct)?;

        assert!(matches!(
            store.store_bmad_config_snapshot(&direct),
            Err(StoreError::Inconsistent)
        ));
    }
    Ok(())
}

fn reseal(snapshot: &mut BmadConfigSnapshot) -> Result<(), Box<dyn std::error::Error>> {
    let mut provenance_input = snapshot.resolved_config.clone();
    let provenance = provenance_input.as_object_mut().expect("config object");
    for key in [
        "configId",
        "provenanceHash",
        "resolvedHash",
        "sessionConfigHash",
    ] {
        provenance.remove(key);
    }
    let provenance_hash = canonical_hash("bmad-config-provenance", 1, &provenance_input)?;
    snapshot.resolved_config["provenanceHash"] = json!(provenance_hash.to_string());

    let mut resolution_input = snapshot.resolved_config.clone();
    let resolution = resolution_input.as_object_mut().expect("config object");
    resolution.remove("resolvedHash");
    resolution.remove("sessionConfigHash");
    let resolution_hash = canonical_hash("bmad-config-resolution", 1, &resolution_input)?;
    snapshot.resolution_hash = resolution_hash.to_string();
    snapshot.resolved_config["resolvedHash"] = json!(snapshot.resolution_hash);
    snapshot.resolved_config["sessionConfigHash"] = json!(snapshot.resolution_hash);
    Ok(())
}
