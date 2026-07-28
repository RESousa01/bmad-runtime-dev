//! Immutable, content-addressed resolved BMAD configuration snapshots.
//!
//! A session records the runtime resolver's `sessionConfigHash`; it never asks
//! this store to update an older snapshot.  Configuration changes therefore
//! create another CAS value, making stale previews detectable while preserving
//! the exact configuration an earlier session used.

use desktop_runtime::canonical_hash;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

use super::{LocalStore, PayloadRef, StoreError};

const CONFIG_SNAPSHOT_KIND: &str = "bmad_config_snapshot";
const CONFIG_SNAPSHOT_SCHEMA: &str = "sapphirus.bmad-config.v1";

/// A resolved configuration suitable for durable session provenance.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BmadConfigSnapshot {
    pub config_id: String,
    pub resolution_hash: String,
    pub resolved_config: Value,
}

impl LocalStore {
    /// Stores one immutable resolved configuration in the encrypted CAS.
    ///
    /// The document must be the resolver's redacted projection: secret
    /// references may be retained, secret values may not. CAS addresses the
    /// whole snapshot by content, so writing a changed configuration cannot
    /// overwrite a configuration already bound to a session.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Inconsistent`] for an unbound, malformed, or
    /// secret-bearing snapshot and [`StoreError::Serialization`] for a value
    /// that cannot be represented as JSON.
    pub fn store_bmad_config_snapshot(
        &self,
        snapshot: &BmadConfigSnapshot,
    ) -> Result<PayloadRef, StoreError> {
        validate_snapshot(snapshot)?;
        let bytes = serde_json::to_vec(snapshot)?;
        self.put_payload(CONFIG_SNAPSHOT_KIND, CONFIG_SNAPSHOT_SCHEMA, &bytes)
    }

    /// Loads one immutable configuration snapshot from its CAS reference.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Inconsistent`] if the reference does not name a
    /// configuration snapshot or the recovered contents violate redaction and
    /// session-binding invariants.
    pub fn load_bmad_config_snapshot(
        &self,
        reference: &PayloadRef,
    ) -> Result<BmadConfigSnapshot, StoreError> {
        if reference.kind != CONFIG_SNAPSHOT_KIND
            || reference.schema_version != CONFIG_SNAPSHOT_SCHEMA
        {
            return Err(StoreError::Inconsistent);
        }
        let bytes = self.get_payload(reference)?;
        let snapshot = serde_json::from_slice(&bytes)?;
        validate_snapshot(&snapshot)?;
        Ok(snapshot)
    }
}

fn validate_snapshot(snapshot: &BmadConfigSnapshot) -> Result<(), StoreError> {
    if !valid_contract_id(&snapshot.config_id) || !valid_sha256(&snapshot.resolution_hash) {
        return Err(StoreError::Inconsistent);
    }
    let object = snapshot
        .resolved_config
        .as_object()
        .ok_or(StoreError::Inconsistent)?;
    if object.len() != 10
        || !object.keys().all(|key| {
            matches!(
                key.as_str(),
                "schemaVersion"
                    | "configId"
                    | "layers"
                    | "effectiveValues"
                    | "secretReferences"
                    | "requirements"
                    | "conflicts"
                    | "resolvedHash"
                    | "provenanceHash"
                    | "sessionConfigHash"
            )
        })
        || object.get("schemaVersion").and_then(Value::as_str) != Some(CONFIG_SNAPSHOT_SCHEMA)
        || object.get("configId").and_then(Value::as_str) != Some(snapshot.config_id.as_str())
        || object.get("resolvedHash").and_then(Value::as_str)
            != Some(snapshot.resolution_hash.as_str())
        || object.get("sessionConfigHash").and_then(Value::as_str)
            != Some(snapshot.resolution_hash.as_str())
        || has_unapproved_secret_material(&snapshot.resolved_config, false)
        || !valid_layers(object.get("layers"))
        || !valid_effective_values(object.get("effectiveValues"))
        || !valid_secret_references(object.get("secretReferences"))
        || !valid_requirements(object.get("requirements"))
        || !valid_conflicts(object.get("conflicts"))
    {
        return Err(StoreError::Inconsistent);
    }
    let provenance_hash = canonical_provenance_hash(&snapshot.resolved_config)?;
    let resolution_hash = canonical_resolution_hash(&snapshot.resolved_config)?;
    if object.get("provenanceHash").and_then(Value::as_str) != Some(provenance_hash.as_str())
        || snapshot.resolution_hash != resolution_hash
    {
        return Err(StoreError::Inconsistent);
    }
    Ok(())
}

fn canonical_provenance_hash(document: &Value) -> Result<String, StoreError> {
    let mut identity = document.clone();
    let object = identity.as_object_mut().ok_or(StoreError::Inconsistent)?;
    for field in [
        "configId",
        "provenanceHash",
        "resolvedHash",
        "sessionConfigHash",
    ] {
        object.remove(field);
    }
    canonical_hash("bmad-config-provenance", 1, &identity)
        .map(|hash| hash.to_string())
        .map_err(|_| StoreError::Inconsistent)
}

fn canonical_resolution_hash(document: &Value) -> Result<String, StoreError> {
    let mut identity = document.clone();
    let object = identity.as_object_mut().ok_or(StoreError::Inconsistent)?;
    object.remove("resolvedHash");
    object.remove("sessionConfigHash");
    canonical_hash("bmad-config-resolution", 1, &identity)
        .map(|hash| hash.to_string())
        .map_err(|_| StoreError::Inconsistent)
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|character| character.is_ascii_hexdigit())
}

fn valid_contract_id(value: &str) -> bool {
    let Some((prefix, suffix)) = value.split_once('_') else {
        return false;
    };
    !prefix.is_empty()
        && prefix.len() <= 31
        && prefix
            .bytes()
            .enumerate()
            .all(|(index, character)| {
                if index == 0 {
                    character.is_ascii_lowercase()
                } else {
                    character.is_ascii_lowercase() || character.is_ascii_digit() || character == b'_'
                }
            })
        && (16..=64).contains(&suffix.len())
        && suffix.bytes().all(|character| {
            character.is_ascii_digit() || matches!(character, b'A'..=b'H' | b'J' | b'K' | b'M' | b'N' | b'P'..=b'T' | b'V'..=b'Z')
        })
}

fn valid_layers(value: Option<&Value>) -> bool {
    let Some(layers) = value.and_then(Value::as_array) else {
        return false;
    };
    if layers.is_empty() || layers.len() > 5 {
        return false;
    }
    let mut seen = BTreeSet::new();
    layers.iter().all(|layer| {
        let Some(object) = layer.as_object() else {
            return false;
        };
        if object.len() != 4
            || !object
                .keys()
                .all(|key| matches!(key.as_str(), "kind" | "ordinal" | "contentHash" | "version"))
        {
            return false;
        }
        let Some(kind) = object.get("kind").and_then(Value::as_str) else {
            return false;
        };
        let Some(ordinal) = object.get("ordinal").and_then(Value::as_u64) else {
            return false;
        };
        let expected = match kind {
            "defaults" => 0,
            "bundled_module" => 1,
            "project" => 2,
            "user" => 3,
            "invocation" => 4,
            _ => return false,
        };
        ordinal == expected
            && object
                .get("version")
                .and_then(Value::as_u64)
                .is_some_and(|version| version > 0 && u32::try_from(version).is_ok())
            && object
                .get("contentHash")
                .and_then(Value::as_str)
                .is_some_and(valid_sha256)
            && seen.insert(ordinal)
    })
}

fn valid_effective_values(value: Option<&Value>) -> bool {
    let Some(values) = value.and_then(Value::as_array) else {
        return false;
    };
    let mut seen = BTreeSet::new();
    values.len() <= 5
        && values.iter().all(|entry| {
            let Some(object) = entry.as_object() else {
                return false;
            };
            if object.len() != 7
                || !object.keys().all(|key| {
                    matches!(
                        key.as_str(),
                        "key"
                            | "value"
                            | "sourceLayer"
                            | "sourceHash"
                            | "validation"
                            | "redaction"
                            | "version"
                    )
                })
                || object.get("redaction").and_then(Value::as_str) != Some("none")
                || object.get("validation").and_then(Value::as_str) != Some("valid")
                || !valid_provenance(object)
            {
                return false;
            }
            let Some(key) = object.get("key").and_then(Value::as_str) else {
                return false;
            };
            seen.insert(key)
                && match key {
                    "language" => object
                        .get("value")
                        .and_then(Value::as_str)
                        .is_some_and(valid_language),
                    "personalization" => {
                        object
                            .get("value")
                            .and_then(Value::as_str)
                            .is_some_and(|text| {
                                !text.trim().is_empty()
                                    && text.len() <= 4_096
                                    && !contains_sensitive_value_assignment(text)
                            })
                    }
                    "output_folder" | "report_folder" => object
                        .get("value")
                        .and_then(Value::as_str)
                        .is_some_and(valid_relative_folder),
                    "model_preference" => valid_model_preference(object.get("value")),
                    _ => false,
                }
        })
}

fn valid_secret_references(value: Option<&Value>) -> bool {
    let Some(references) = value.and_then(Value::as_array) else {
        return false;
    };
    let mut seen = BTreeSet::new();
    references.len() <= 64
        && references.iter().all(|entry| {
            let Some(object) = entry.as_object() else {
                return false;
            };
            object.len() == 7
                && object.keys().all(|key| {
                    matches!(
                        key.as_str(),
                        "key"
                            | "reference"
                            | "sourceLayer"
                            | "sourceHash"
                            | "validation"
                            | "redaction"
                            | "version"
                    )
                })
                && object.get("validation").and_then(Value::as_str) == Some("valid")
                && object.get("redaction").and_then(Value::as_str) == Some("reference_only")
                && object
                    .get("key")
                    .and_then(Value::as_str)
                    .is_some_and(valid_secret_key)
                && object
                    .get("reference")
                    .and_then(Value::as_str)
                    .is_some_and(valid_secret_reference)
                && valid_provenance(object)
                && seen.insert(
                    object
                        .get("key")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                )
        })
}

fn valid_requirements(value: Option<&Value>) -> bool {
    value.and_then(Value::as_array).is_some_and(|requirements| {
        requirements.len() <= 5
            && requirements.iter().all(|entry| {
                let Some(object) = entry.as_object() else {
                    return false;
                };
                object.len() == 2
                    && object.get("reason").and_then(Value::as_str)
                        == Some("missing_required_headless_value")
                    && matches!(
                        object.get("key").and_then(Value::as_str),
                        Some("language" | "output_folder" | "report_folder" | "model_preference")
                    )
            })
    })
}

fn valid_conflicts(value: Option<&Value>) -> bool {
    value.and_then(Value::as_array).is_some_and(|conflicts| {
        conflicts.len() <= 128
            && conflicts.iter().all(|entry| {
                let Some(object) = entry.as_object() else {
                    return false;
                };
                object.len() == 4
                    && matches!(
                        object.get("code").and_then(Value::as_str),
                        Some("config_value_overridden" | "config_secret_reference_overridden")
                    )
                    && object
                        .get("key")
                        .and_then(Value::as_str)
                        .is_some_and(|key| !key.is_empty() && key.len() <= 128)
                    && object
                        .get("previousSourceLayer")
                        .and_then(Value::as_str)
                        .is_some_and(valid_layer_kind)
                    && object
                        .get("nextSourceLayer")
                        .and_then(Value::as_str)
                        .is_some_and(valid_layer_kind)
            })
    })
}

fn valid_provenance(object: &serde_json::Map<String, Value>) -> bool {
    object
        .get("sourceLayer")
        .and_then(Value::as_str)
        .is_some_and(valid_layer_kind)
        && object
            .get("sourceHash")
            .and_then(Value::as_str)
            .is_some_and(valid_sha256)
        && object
            .get("version")
            .and_then(Value::as_u64)
            .is_some_and(|version| version > 0 && u32::try_from(version).is_ok())
}

fn valid_layer_kind(value: &str) -> bool {
    matches!(
        value,
        "defaults" | "bundled_module" | "project" | "user" | "invocation"
    )
}

fn valid_language(value: &str) -> bool {
    let mut parts = value.split('-');
    let Some(primary) = parts.next() else {
        return false;
    };
    (2..=3).contains(&primary.len())
        && primary
            .bytes()
            .all(|character| character.is_ascii_alphabetic())
        && parts.all(|part| {
            (2..=8).contains(&part.len())
                && part
                    .bytes()
                    .all(|character| character.is_ascii_alphanumeric())
        })
}

fn contains_sensitive_value_assignment(value: &str) -> bool {
    value.char_indices().any(|(index, character)| {
        matches!(character, '=' | ':') && is_sensitive_assignment_key(value[..index].trim_end())
    }) || contains_credential_uri_userinfo(value)
}

fn is_sensitive_assignment_key(prefix: &str) -> bool {
    let key = prefix
        .rsplit(|character: char| !is_key_character(character))
        .next()
        .unwrap_or_default();
    let normalized = key
        .bytes()
        .filter(|byte| !matches!(byte, b'_' | b'-'))
        .map(|byte| char::from(byte.to_ascii_lowercase()))
        .collect::<String>();
    matches!(
        normalized.as_str(),
        "password"
            | "passwd"
            | "apikey"
            | "secret"
            | "token"
            | "accesstoken"
            | "clientsecret"
            | "dbpassword"
            | "privatekey"
            | "connectionstring"
    )
}

fn contains_credential_uri_userinfo(value: &str) -> bool {
    value.match_indices("://").any(|(offset, _)| {
        let authority = value[offset + 3..]
            .split(|character: char| {
                character.is_whitespace() || matches!(character, '/' | '?' | '#')
            })
            .next()
            .unwrap_or_default();
        authority.split_once('@').is_some_and(|(userinfo, _)| {
            userinfo
                .split_once(':')
                .is_some_and(|(user, password)| !user.is_empty() && !password.is_empty())
        })
    })
}

fn is_key_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
}

fn valid_relative_folder(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && !value.contains('\\')
        && !value.starts_with('/')
        && !value.contains(':')
        && value.split('/').all(valid_windows_path_segment)
}

fn valid_windows_path_segment(segment: &str) -> bool {
    if segment.is_empty()
        || matches!(segment, "." | "..")
        || segment.ends_with('.')
        || segment.ends_with(' ')
        || segment.chars().any(|character| {
            character.is_control() || matches!(character, '<' | '>' | '"' | '|' | '?' | '*')
        })
    {
        return false;
    }
    let device_name = segment
        .split_once('.')
        .map_or(segment, |(base, _)| base)
        .to_ascii_uppercase();
    !matches!(
        device_name.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

fn valid_model_preference(value: Option<&Value>) -> bool {
    let Some(object) = value.and_then(Value::as_object) else {
        return false;
    };
    object.len() == 2
        && object
            .get("provider")
            .and_then(Value::as_str)
            .is_some_and(valid_model_identifier)
        && object
            .get("model")
            .and_then(Value::as_str)
            .is_some_and(valid_model_identifier)
}

fn valid_model_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, b'.' | b'_' | b'-' | b'/')
        })
}

fn valid_secret_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, b'_' | b'-'))
}

fn valid_secret_reference(value: &str) -> bool {
    let Some(reference) = value.strip_prefix("secret://") else {
        return false;
    };
    !reference.is_empty() && reference.len() <= 512 && reference.split('/').all(valid_secret_key)
}

fn has_unapproved_secret_material(value: &Value, inside_secret_reference: bool) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, value)| {
            if key == "secretReferences" {
                return has_unapproved_secret_material(value, true);
            }
            let allowed_reference = key == "reference" && inside_secret_reference;
            is_secret_key(key) && !allowed_reference
                || has_unapproved_secret_material(value, inside_secret_reference)
        }),
        Value::Array(items) => items
            .iter()
            .any(|item| has_unapproved_secret_material(item, inside_secret_reference)),
        Value::String(text) => !inside_secret_reference && text.starts_with("secret://"),
        _ => false,
    }
}

fn is_secret_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|character| !matches!(character, '_' | '-'))
        .flat_map(char::to_lowercase)
        .collect::<String>();
    normalized.contains("secret")
        || normalized.contains("password")
        || normalized.contains("credential")
        || normalized.contains("token")
        || normalized.contains("apikey")
}
