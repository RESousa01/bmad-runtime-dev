use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Map, Value};

use crate::{canonical_hash, Sha256Digest};

use super::{BmadKernelError, BmadKernelErrorCode};

const MAX_CONFIG_BYTES: usize = 1_048_576;
const MAX_CONFIG_LAYERS: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadConfigGraphKind {
    MethodCentralToml,
    SkillCustomizationToml,
    CompatibilityYaml,
}

impl BmadConfigGraphKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MethodCentralToml => "method_central_toml",
            Self::SkillCustomizationToml => "skill_customization_toml",
            Self::CompatibilityYaml => "compatibility_yaml",
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
