use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Map, Value};

use crate::{canonical_hash, Sha256Digest};

use super::kernel_error::{BmadKernelError, BmadKernelErrorCode};

const MAX_CONFIG_BYTES: usize = 1_048_576;
const MAX_CONFIG_LAYERS: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadConfigGraphKind {
    MethodCentralToml,
    SkillCustomizationToml,
    CompatibilityYaml,
    LayeredRuntime,
}

impl BmadConfigGraphKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MethodCentralToml => "method_central_toml",
            Self::SkillCustomizationToml => "skill_customization_toml",
            Self::CompatibilityYaml => "compatibility_yaml",
            Self::LayeredRuntime => "layered_runtime",
        }
    }
}

#[derive(Clone, Debug)]
enum BmadConfigLayerPayload {
    Valid(Value),
    Invalid(String),
}

#[derive(Clone, Debug)]
pub struct BmadConfigLayer {
    graph_kind: BmadConfigGraphKind,
    layer_kind: String,
    ordinal: u8,
    required: bool,
    payload: BmadConfigLayerPayload,
}

impl BmadConfigLayer {
    /// Creates an already-parsed, untrusted config layer.
    ///
    /// # Errors
    ///
    /// Returns [`BmadKernelErrorCode::ConfigGraphInvalid`] when the layer kind,
    /// ordinal, payload shape, or bounded size is invalid.
    pub fn valid(
        graph_kind: BmadConfigGraphKind,
        layer_kind: impl Into<String>,
        ordinal: u8,
        required: bool,
        entries: Value,
    ) -> Result<Self, BmadKernelError> {
        let layer_kind = layer_kind.into();
        validate_layer_identity(graph_kind, &layer_kind, ordinal)?;
        if !entries.is_object()
            || serde_json::to_vec(&entries)
                .map_err(|_| BmadKernelErrorCode::ConfigGraphInvalid)?
                .len()
                > MAX_CONFIG_BYTES
        {
            return Err(BmadKernelErrorCode::ConfigGraphInvalid.into());
        }
        Ok(Self {
            graph_kind,
            layer_kind,
            ordinal,
            required,
            payload: BmadConfigLayerPayload::Valid(entries),
        })
    }

    /// Records a parser failure without accepting partially parsed config.
    ///
    /// # Errors
    ///
    /// Returns [`BmadKernelErrorCode::ConfigGraphInvalid`] for an invalid layer
    /// identity or an empty/oversized diagnostic.
    pub fn invalid(
        graph_kind: BmadConfigGraphKind,
        layer_kind: impl Into<String>,
        ordinal: u8,
        required: bool,
        diagnostic: impl Into<String>,
    ) -> Result<Self, BmadKernelError> {
        let layer_kind = layer_kind.into();
        let diagnostic = diagnostic.into();
        validate_layer_identity(graph_kind, &layer_kind, ordinal)?;
        if diagnostic.is_empty()
            || diagnostic.len() > 4_096
            || diagnostic.chars().any(char::is_control)
        {
            return Err(BmadKernelErrorCode::ConfigGraphInvalid.into());
        }
        Ok(Self {
            graph_kind,
            layer_kind,
            ordinal,
            required,
            payload: BmadConfigLayerPayload::Invalid(diagnostic),
        })
    }
}

#[derive(Clone, Debug)]
pub struct BmadConfigGraph {
    kind: BmadConfigGraphKind,
    layers: Vec<BmadConfigLayer>,
}

impl BmadConfigGraph {
    /// Constructs an ordered config graph without combining graph families.
    ///
    /// # Errors
    ///
    /// Returns [`BmadKernelErrorCode::ConfigGraphInvalid`] when layer ownership
    /// or canonical ordering is invalid.
    pub fn new(
        kind: BmadConfigGraphKind,
        layers: Vec<BmadConfigLayer>,
    ) -> Result<Self, BmadKernelError> {
        if layers.len() > MAX_CONFIG_LAYERS
            || layers.iter().any(|layer| layer.graph_kind != kind)
            || layers
                .windows(2)
                .any(|pair| pair[0].ordinal >= pair[1].ordinal)
        {
            return Err(BmadKernelErrorCode::ConfigGraphInvalid.into());
        }
        Ok(Self { kind, layers })
    }

    #[must_use]
    pub const fn kind(&self) -> BmadConfigGraphKind {
        self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BmadConfigWarning {
    pub code: String,
    pub layer_kind: String,
}

#[derive(Clone, Debug)]
pub struct BmadConfigResolution {
    pub graph_kind: BmadConfigGraphKind,
    pub value: Value,
    pub warnings: Vec<BmadConfigWarning>,
    pub resolution_hash: Sha256Digest,
}

#[derive(Clone, Debug)]
pub struct BmadResolvedConfig {
    pub central: BmadConfigResolution,
    pub skill: BmadConfigResolution,
    pub compatibility: BmadConfigResolution,
}

pub struct BmadConfigResolver;

impl BmadConfigResolver {
    /// Resolves the three independent Method config families.
    ///
    /// # Errors
    ///
    /// Fails closed if a graph is supplied under the wrong family or any graph
    /// contains required invalid data or policy-bearing keys.
    pub fn resolve(
        central: &BmadConfigGraph,
        skill: &BmadConfigGraph,
        compatibility: &BmadConfigGraph,
    ) -> Result<BmadResolvedConfig, BmadKernelError> {
        if central.kind != BmadConfigGraphKind::MethodCentralToml
            || skill.kind != BmadConfigGraphKind::SkillCustomizationToml
            || compatibility.kind != BmadConfigGraphKind::CompatibilityYaml
        {
            return Err(BmadKernelErrorCode::ConfigGraphInvalid.into());
        }
        Ok(BmadResolvedConfig {
            central: Self::resolve_graph(central)?,
            skill: Self::resolve_graph(skill)?,
            compatibility: Self::resolve_graph(compatibility)?,
        })
    }

    /// Applies the source-defined merge semantics to one config family.
    ///
    /// # Errors
    ///
    /// Fails closed for required invalid layers or policy-bearing data.
    pub fn resolve_graph(graph: &BmadConfigGraph) -> Result<BmadConfigResolution, BmadKernelError> {
        let mut value = Value::Object(Map::new());
        let mut warnings = Vec::new();
        for layer in &graph.layers {
            match &layer.payload {
                BmadConfigLayerPayload::Invalid(diagnostic) if layer.required => {
                    let _ = diagnostic;
                    return Err(BmadKernelErrorCode::ConfigMergeConflict.into());
                }
                BmadConfigLayerPayload::Invalid(diagnostic) => {
                    let _ = diagnostic;
                    warnings.push(BmadConfigWarning {
                        code: "config_optional_layer_invalid".to_owned(),
                        layer_kind: layer.layer_kind.clone(),
                    });
                }
                BmadConfigLayerPayload::Valid(entries) => {
                    if contains_forbidden_policy(entries) {
                        return Err(BmadKernelErrorCode::ConfigPolicyForbidden.into());
                    }
                    merge_value(&mut value, entries, &layer.layer_kind, &mut warnings);
                }
            }
        }

        let resolution_hash = canonical_hash(
            "bmad-config-resolution",
            1,
            &serde_json::json!({
                "graphKind": graph.kind.as_str(),
                "value": value,
                "warnings": warnings,
            }),
        )
        .map_err(|_| BmadKernelErrorCode::ConfigMergeConflict)?;
        Ok(BmadConfigResolution {
            graph_kind: graph.kind,
            value,
            warnings,
            resolution_hash,
        })
    }

    /// Resolves the closed, host-owned runtime configuration vocabulary.
    /// Caller order never affects the result: defaults < bundled module <
    /// project < user < invocation. Configuration remains data only; policy
    /// and raw secrets fail closed rather than becoming runtime authority.
    ///
    /// The returned `sessionConfigHash` is an immutable session binding. A
    /// changed hash makes a preview stale but has no path to mutate history.
    ///
    /// # Errors
    ///
    /// Returns [`BmadKernelErrorCode::ConfigGraphInvalid`] for malformed,
    /// duplicate, unknown, unsafe, or secret-bearing documents.
    pub fn resolve_layered(
        documents: &[Value],
        headless: bool,
    ) -> Result<BmadConfigResolution, BmadKernelError> {
        let layers = parse_runtime_layers(documents)?;
        let merged = merge_runtime_layers(&layers);
        build_runtime_resolution(&layers, &merged, headless)
    }

    /// Returns whether a preview must be regenerated for a new immutable
    /// resolved-config hash. It deliberately never changes the prior hash.
    #[must_use]
    pub fn preview_is_stale(
        preview_config_hash: &Sha256Digest,
        current_config_hash: &Sha256Digest,
    ) -> bool {
        preview_config_hash != current_config_hash
    }
}

fn config_id_from_identity(identity: &Value) -> Result<String, BmadKernelError> {
    let hash = canonical_hash("bmad-config-id", 1, identity)
        .map_err(|_| BmadKernelErrorCode::ConfigGraphInvalid)?
        .to_string();
    let identifier = hash
        .get(7..23)
        .ok_or(BmadKernelErrorCode::ConfigGraphInvalid)?
        .to_ascii_uppercase();
    Ok(format!("config_{identifier}"))
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum RuntimeLayerKind {
    Defaults,
    BundledModule,
    Project,
    User,
    Invocation,
}

impl RuntimeLayerKind {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "defaults" => Some(Self::Defaults),
            "bundled_module" => Some(Self::BundledModule),
            "project" => Some(Self::Project),
            "user" => Some(Self::User),
            "invocation" => Some(Self::Invocation),
            _ => None,
        }
    }

    const fn ordinal(self) -> u8 {
        match self {
            Self::Defaults => 0,
            Self::BundledModule => 1,
            Self::Project => 2,
            Self::User => 3,
            Self::Invocation => 4,
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Defaults => "defaults",
            Self::BundledModule => "bundled_module",
            Self::Project => "project",
            Self::User => "user",
            Self::Invocation => "invocation",
        }
    }
}

#[derive(Clone, Debug)]
struct RuntimeLayer {
    kind: RuntimeLayerKind,
    version: u32,
    values: BTreeMap<String, Value>,
    secret_references: BTreeMap<String, String>,
    source_hash: Sha256Digest,
}

#[derive(Clone, Debug)]
struct RuntimeEffectiveValue {
    value: Value,
    source_layer: &'static str,
    source_hash: Sha256Digest,
    version: u32,
}

#[derive(Clone, Debug)]
struct RuntimeEffectiveSecret {
    reference: String,
    source_layer: &'static str,
    source_hash: Sha256Digest,
    version: u32,
}

struct RuntimeMerge {
    values: BTreeMap<String, RuntimeEffectiveValue>,
    secrets: BTreeMap<String, RuntimeEffectiveSecret>,
    warnings: Vec<BmadConfigWarning>,
    conflicts: Vec<Value>,
}

fn parse_runtime_layers(
    documents: &[Value],
) -> Result<BTreeMap<u8, RuntimeLayer>, BmadKernelError> {
    if documents.is_empty() || documents.len() > 5 {
        return Err(BmadKernelErrorCode::ConfigGraphInvalid.into());
    }
    let mut layers = BTreeMap::new();
    for document in documents {
        let layer = parse_runtime_layer(document)?;
        if layers.insert(layer.kind.ordinal(), layer).is_some() {
            return Err(BmadKernelErrorCode::ConfigGraphInvalid.into());
        }
    }
    Ok(layers)
}

fn merge_runtime_layers(layers: &BTreeMap<u8, RuntimeLayer>) -> RuntimeMerge {
    let mut merged = RuntimeMerge {
        values: BTreeMap::new(),
        secrets: BTreeMap::new(),
        warnings: Vec::new(),
        conflicts: Vec::new(),
    };
    for layer in layers.values() {
        merge_runtime_values(layer, &mut merged);
        merge_runtime_secrets(layer, &mut merged);
    }
    merged
}

fn merge_runtime_values(layer: &RuntimeLayer, merged: &mut RuntimeMerge) {
    for (key, value) in &layer.values {
        let replacement = RuntimeEffectiveValue {
            value: value.clone(),
            source_layer: layer.kind.as_str(),
            source_hash: layer.source_hash,
            version: layer.version,
        };
        if let Some(previous) = merged.values.insert(key.clone(), replacement) {
            if previous.value != *value {
                push_conflict(
                    merged,
                    "config_value_overridden",
                    key,
                    previous.source_layer,
                    layer.kind.as_str(),
                );
            }
        }
    }
}

fn merge_runtime_secrets(layer: &RuntimeLayer, merged: &mut RuntimeMerge) {
    for (key, reference) in &layer.secret_references {
        let replacement = RuntimeEffectiveSecret {
            reference: reference.clone(),
            source_layer: layer.kind.as_str(),
            source_hash: layer.source_hash,
            version: layer.version,
        };
        if let Some(previous) = merged.secrets.insert(key.clone(), replacement) {
            if previous.reference != *reference {
                push_conflict(
                    merged,
                    "config_secret_reference_overridden",
                    key,
                    previous.source_layer,
                    layer.kind.as_str(),
                );
            }
        }
    }
}

fn push_conflict(
    merged: &mut RuntimeMerge,
    code: &str,
    key: &str,
    previous_source_layer: &str,
    next_source_layer: &str,
) {
    merged.warnings.push(BmadConfigWarning {
        code: code.to_owned(),
        layer_kind: next_source_layer.to_owned(),
    });
    merged.conflicts.push(serde_json::json!({
        "code": code,
        "key": key,
        "previousSourceLayer": previous_source_layer,
        "nextSourceLayer": next_source_layer,
    }));
}

fn build_runtime_resolution(
    layers: &BTreeMap<u8, RuntimeLayer>,
    merged: &RuntimeMerge,
    headless: bool,
) -> Result<BmadConfigResolution, BmadKernelError> {
    let requirements = if headless {
        required_headless_values(&merged.values)
    } else {
        Vec::new()
    };
    let effective_values = merged
        .values
        .iter()
        .map(|(key, value)| effective_value_json(key, value))
        .collect::<Vec<_>>();
    let secret_references = merged
        .secrets
        .iter()
        .map(|(key, secret)| secret_reference_json(key, secret))
        .collect::<Vec<_>>();
    let layer_records = layers.values().map(runtime_layer_json).collect::<Vec<_>>();
    let identity = serde_json::json!({
        "schemaVersion": "sapphirus.bmad-config.v1",
        "layers": layer_records,
        "effectiveValues": effective_values,
        "secretReferences": secret_references,
        "requirements": requirements,
        "conflicts": merged.conflicts,
    });
    let (value, resolution_hash) = seal_runtime_config(identity)?;
    Ok(BmadConfigResolution {
        graph_kind: BmadConfigGraphKind::LayeredRuntime,
        value,
        warnings: merged.warnings.clone(),
        resolution_hash,
    })
}

fn effective_value_json(key: &str, value: &RuntimeEffectiveValue) -> Value {
    serde_json::json!({
        "key": key,
        "value": value.value,
        "sourceLayer": value.source_layer,
        "sourceHash": value.source_hash.to_string(),
        "validation": "valid",
        "redaction": "none",
        "version": value.version,
    })
}

fn secret_reference_json(key: &str, secret: &RuntimeEffectiveSecret) -> Value {
    serde_json::json!({
        "key": key,
        "reference": secret.reference,
        "sourceLayer": secret.source_layer,
        "sourceHash": secret.source_hash.to_string(),
        "validation": "valid",
        "redaction": "reference_only",
        "version": secret.version,
    })
}

fn runtime_layer_json(layer: &RuntimeLayer) -> Value {
    serde_json::json!({
        "kind": layer.kind.as_str(),
        "ordinal": layer.kind.ordinal(),
        "contentHash": layer.source_hash.to_string(),
        "version": layer.version,
    })
}

fn seal_runtime_config(mut identity: Value) -> Result<(Value, Sha256Digest), BmadKernelError> {
    let config_id = config_id_from_identity(&identity)?;
    let provenance_hash = canonical_hash("bmad-config-provenance", 1, &identity)
        .map_err(|_| BmadKernelErrorCode::ConfigGraphInvalid)?;
    let object = identity
        .as_object_mut()
        .ok_or(BmadKernelErrorCode::ConfigGraphInvalid)?;
    object.insert("configId".to_owned(), Value::String(config_id));
    object.insert(
        "provenanceHash".to_owned(),
        Value::String(provenance_hash.to_string()),
    );
    let resolution_hash = canonical_hash("bmad-config-resolution", 1, &identity)
        .map_err(|_| BmadKernelErrorCode::ConfigGraphInvalid)?;
    let object = identity
        .as_object_mut()
        .ok_or(BmadKernelErrorCode::ConfigGraphInvalid)?;
    object.insert(
        "resolvedHash".to_owned(),
        Value::String(resolution_hash.to_string()),
    );
    object.insert(
        "sessionConfigHash".to_owned(),
        Value::String(resolution_hash.to_string()),
    );
    Ok((identity, resolution_hash))
}

fn parse_runtime_layer(document: &Value) -> Result<RuntimeLayer, BmadKernelError> {
    let object = document
        .as_object()
        .ok_or(BmadKernelErrorCode::ConfigGraphInvalid)?;
    if object.len() > 4
        || object.keys().any(|key| {
            !matches!(
                key.as_str(),
                "kind" | "version" | "values" | "secretReferences"
            )
        })
    {
        return Err(BmadKernelErrorCode::ConfigGraphInvalid.into());
    }
    let kind = object
        .get("kind")
        .and_then(Value::as_str)
        .and_then(RuntimeLayerKind::parse)
        .ok_or(BmadKernelErrorCode::ConfigGraphInvalid)?;
    let version = object
        .get("version")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or(BmadKernelErrorCode::ConfigGraphInvalid)?;
    let values = object
        .get("values")
        .and_then(Value::as_object)
        .ok_or(BmadKernelErrorCode::ConfigGraphInvalid)?;
    if serde_json::to_vec(document)
        .map_err(|_| BmadKernelErrorCode::ConfigGraphInvalid)?
        .len()
        > MAX_CONFIG_BYTES
        || contains_forbidden_policy(document)
    {
        return Err(BmadKernelErrorCode::ConfigGraphInvalid.into());
    }
    let mut validated_values = BTreeMap::new();
    for (key, value) in values {
        validate_runtime_value(key, value)?;
        validated_values.insert(key.clone(), value.clone());
    }
    let mut validated_secrets = BTreeMap::new();
    if let Some(secret_references) = object.get("secretReferences") {
        for (key, reference) in secret_references
            .as_object()
            .ok_or(BmadKernelErrorCode::ConfigGraphInvalid)?
        {
            let reference = reference
                .as_str()
                .filter(|reference| valid_secret_key(key) && valid_secret_reference(reference))
                .ok_or(BmadKernelErrorCode::ConfigGraphInvalid)?;
            validated_secrets.insert(key.clone(), reference.to_owned());
        }
    }
    let source_hash = canonical_hash(
        "bmad-config-layer",
        1,
        &serde_json::json!({
            "kind": kind.as_str(),
            "version": version,
            "values": validated_values,
            "secretReferences": validated_secrets,
        }),
    )
    .map_err(|_| BmadKernelErrorCode::ConfigGraphInvalid)?;
    Ok(RuntimeLayer {
        kind,
        version,
        values: validated_values,
        secret_references: validated_secrets,
        source_hash,
    })
}

fn validate_runtime_value(key: &str, value: &Value) -> Result<(), BmadKernelError> {
    match key {
        "language" => value
            .as_str()
            .filter(|value| valid_language(value))
            .map(|_| ())
            .ok_or(BmadKernelErrorCode::ConfigGraphInvalid.into()),
        "personalization" => value
            .as_str()
            .filter(|value| {
                !value.trim().is_empty()
                    && value.len() <= 4_096
                    && !contains_sensitive_value_assignment(value)
            })
            .map(|_| ())
            .ok_or(BmadKernelErrorCode::ConfigGraphInvalid.into()),
        "output_folder" | "report_folder" => value
            .as_str()
            .filter(|value| valid_relative_folder(value))
            .map(|_| ())
            .ok_or(BmadKernelErrorCode::ConfigGraphInvalid.into()),
        "model_preference" => valid_model_preference(value),
        _ => Err(BmadKernelErrorCode::ConfigGraphInvalid.into()),
    }
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

fn valid_model_preference(value: &Value) -> Result<(), BmadKernelError> {
    let Some(object) = value.as_object() else {
        return Err(BmadKernelErrorCode::ConfigGraphInvalid.into());
    };
    if object.len() != 2 || !object.contains_key("provider") || !object.contains_key("model") {
        return Err(BmadKernelErrorCode::ConfigGraphInvalid.into());
    }
    if object
        .values()
        .any(|value| !value.as_str().is_some_and(valid_model_identifier))
    {
        return Err(BmadKernelErrorCode::ConfigGraphInvalid.into());
    }
    Ok(())
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

fn required_headless_values(values: &BTreeMap<String, RuntimeEffectiveValue>) -> Vec<Value> {
    ["model_preference", "output_folder", "report_folder"]
        .into_iter()
        .filter(|key| !values.contains_key(*key))
        .map(|key| {
            serde_json::json!({
                "key": key,
                "reason": "missing_required_headless_value",
            })
        })
        .collect()
}

fn validate_layer_identity(
    graph_kind: BmadConfigGraphKind,
    layer_kind: &str,
    ordinal: u8,
) -> Result<(), BmadKernelError> {
    let expected = match graph_kind {
        BmadConfigGraphKind::MethodCentralToml => [
            "installer_team",
            "installer_user",
            "custom_team",
            "custom_user",
        ]
        .get(usize::from(ordinal))
        .copied(),
        BmadConfigGraphKind::SkillCustomizationToml => {
            ["packaged_default", "team_override", "user_override"]
                .get(usize::from(ordinal))
                .copied()
        }
        BmadConfigGraphKind::CompatibilityYaml => [
            "method_module_yaml",
            "builder_root_yaml",
            "builder_user_yaml",
        ]
        .get(usize::from(ordinal))
        .copied(),
        BmadConfigGraphKind::LayeredRuntime => None,
    };
    if expected == Some(layer_kind) {
        Ok(())
    } else {
        Err(BmadKernelErrorCode::ConfigGraphInvalid.into())
    }
}

fn merge_value(
    target: &mut Value,
    incoming: &Value,
    layer_kind: &str,
    warnings: &mut Vec<BmadConfigWarning>,
) {
    match (target, incoming) {
        (_, Value::Null) => warnings.push(BmadConfigWarning {
            code: "config_deletion_unsupported".to_owned(),
            layer_kind: layer_kind.to_owned(),
        }),
        (Value::Object(target_map), Value::Object(incoming_map)) => {
            for (key, incoming_value) in incoming_map {
                if incoming_value.is_null() {
                    warnings.push(BmadConfigWarning {
                        code: "config_deletion_unsupported".to_owned(),
                        layer_kind: layer_kind.to_owned(),
                    });
                } else if let Some(target_value) = target_map.get_mut(key) {
                    merge_value(target_value, incoming_value, layer_kind, warnings);
                } else {
                    target_map.insert(key.clone(), incoming_value.clone());
                }
            }
        }
        (Value::Array(target_array), Value::Array(incoming_array)) => {
            if let Some(key) = common_array_key(target_array, incoming_array) {
                merge_keyed_array(target_array, incoming_array, key, layer_kind, warnings);
            } else {
                target_array.extend(incoming_array.iter().cloned());
            }
        }
        (target_value, incoming_value) => *target_value = incoming_value.clone(),
    }
}

fn common_array_key<'a>(target: &'a [Value], incoming: &'a [Value]) -> Option<&'static str> {
    ["code", "id"].into_iter().find(|key| {
        !target.is_empty()
            && !incoming.is_empty()
            && target
                .iter()
                .chain(incoming)
                .all(|value| value.get(*key).and_then(Value::as_str).is_some())
    })
}

fn merge_keyed_array(
    target: &mut Vec<Value>,
    incoming: &[Value],
    key: &str,
    layer_kind: &str,
    warnings: &mut Vec<BmadConfigWarning>,
) {
    let mut positions = target
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            item.get(key)
                .and_then(Value::as_str)
                .map(|value| (value.to_owned(), index))
        })
        .collect::<BTreeMap<_, _>>();
    for (item_key, incoming_item) in incoming.iter().filter_map(|item| {
        item.get(key)
            .and_then(Value::as_str)
            .map(|item_key| (item_key, item))
    }) {
        if let Some(index) = positions.get(item_key).copied() {
            merge_value(&mut target[index], incoming_item, layer_kind, warnings);
        } else {
            positions.insert(item_key.to_owned(), target.len());
            target.push(incoming_item.clone());
        }
    }
}

fn contains_forbidden_policy(value: &Value) -> bool {
    match value {
        Value::Object(entries) => entries
            .iter()
            .any(|(key, value)| is_forbidden_policy_key(key) || contains_forbidden_policy(value)),
        Value::Array(items) => items.iter().any(contains_forbidden_policy),
        _ => false,
    }
}

fn is_forbidden_policy_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|character| !matches!(character, '_' | '-'))
        .flat_map(char::to_lowercase)
        .collect::<String>();
    matches!(
        normalized.as_str(),
        "airlock"
            | "approval"
            | "approvals"
            | "authority"
            | "capabilityenabled"
            | "command"
            | "commands"
            | "executionpolicy"
            | "permission"
            | "permissions"
            | "policy"
            | "process"
            | "shell"
            | "tool"
            | "tools"
    )
}
