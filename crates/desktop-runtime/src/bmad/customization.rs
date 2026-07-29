//! Deterministic, inert BMAD customization overlays.
//!
//! Customization is untrusted data. This module parses the reviewed TOML
//! subset, applies Method's structural merge rules, and projects declarations
//! into text, dependencies, or typed proposals. It deliberately has no
//! filesystem, process, network, tool, model, or persistence port.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::{Map, Value};
use thiserror::Error;

use crate::{canonical_hash, sha256_bytes, Sha256Digest};

use super::{parse_toml_subset, TomlSubsetError};

const MAX_LAYERS: usize = 4;
const MAX_LABEL_BYTES: usize = 512;
const MAX_DECLARATIONS: usize = 512;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadCustomizationLayerKind {
    Defaults,
    Team,
    User,
    Invocation,
}

impl BmadCustomizationLayerKind {
    const fn ordinal(self) -> u8 {
        match self {
            Self::Defaults => 0,
            Self::Team => 1,
            Self::User => 2,
            Self::Invocation => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadCustomizationMode {
    Interactive,
    Headless,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadCustomizationDeclarationKind {
    Dependency,
    ToolIntent,
    ActivationProposal,
    OutputProposal,
    ForbiddenEffect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadCustomizationStatus {
    Available,
    Proposed,
    Unavailable,
    Forbidden,
}

#[derive(Clone, Debug)]
pub struct BmadCustomizationLayer {
    pub kind: BmadCustomizationLayerKind,
    pub provenance_label: String,
    pub version: u32,
    pub content_hash: Sha256Digest,
    value: Value,
}

impl BmadCustomizationLayer {
    /// Parses one exact customization source without accepting partial TOML.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed input, unsafe provenance, or version
    /// zero.
    pub fn parse(
        kind: BmadCustomizationLayerKind,
        provenance_label: impl Into<String>,
        version: u32,
        source: &str,
    ) -> Result<Self, BmadCustomizationError> {
        let provenance_label = provenance_label.into();
        if version == 0
            || provenance_label.is_empty()
            || provenance_label.len() > MAX_LABEL_BYTES
            || provenance_label.chars().any(char::is_control)
        {
            return Err(BmadCustomizationError::InvalidLayer);
        }
        let value = parse_toml_subset(source).map_err(BmadCustomizationError::Toml)?;
        Ok(Self {
            kind,
            provenance_label,
            version,
            content_hash: sha256_bytes(source.as_bytes()),
            value,
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct BmadCustomizationAvailability {
    available_skills: BTreeSet<String>,
    available_tools: BTreeSet<String>,
}

impl BmadCustomizationAvailability {
    #[must_use]
    pub fn new<S, T, SI, TI>(skills: SI, tools: TI) -> Self
    where
        S: Into<String>,
        T: Into<String>,
        SI: IntoIterator<Item = S>,
        TI: IntoIterator<Item = T>,
    {
        Self {
            available_skills: skills.into_iter().map(Into::into).collect(),
            available_tools: tools.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct BmadCustomizationRequest {
    customization_id: String,
    capability_id: String,
    session_id: String,
    evidence_id: String,
    mode: BmadCustomizationMode,
    layers: Vec<BmadCustomizationLayer>,
    availability: BmadCustomizationAvailability,
}

impl BmadCustomizationRequest {
    /// Creates a bound customization request.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid identities, duplicate layer kinds, or an
    /// empty/oversized layer set.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        customization_id: impl Into<String>,
        capability_id: impl Into<String>,
        session_id: impl Into<String>,
        evidence_id: impl Into<String>,
        mode: BmadCustomizationMode,
        layers: Vec<BmadCustomizationLayer>,
        availability: BmadCustomizationAvailability,
    ) -> Result<Self, BmadCustomizationError> {
        let customization_id = customization_id.into();
        let capability_id = capability_id.into();
        let session_id = session_id.into();
        let evidence_id = evidence_id.into();
        for identity in [&customization_id, &capability_id, &session_id, &evidence_id] {
            if !valid_contract_id(identity) {
                return Err(BmadCustomizationError::InvalidIdentity);
            }
        }
        if layers.is_empty() || layers.len() > MAX_LAYERS {
            return Err(BmadCustomizationError::InvalidLayer);
        }
        let unique = layers
            .iter()
            .map(|layer| layer.kind)
            .collect::<BTreeSet<_>>();
        if unique.len() != layers.len() {
            return Err(BmadCustomizationError::DuplicateLayer);
        }
        Ok(Self {
            customization_id,
            capability_id,
            session_id,
            evidence_id,
            mode,
            layers,
            availability,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadCustomizationLayerRecord {
    pub kind: BmadCustomizationLayerKind,
    pub ordinal: u8,
    pub provenance_label: String,
    pub content_hash: Sha256Digest,
    pub version: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadCustomizationTextOverlay {
    pub key: String,
    pub value: Value,
    pub source_layer: BmadCustomizationLayerKind,
    pub provenance_label: String,
    pub source_hash: Sha256Digest,
    pub overlay_hash: Sha256Digest,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadCustomizationDeclaration {
    pub kind: BmadCustomizationDeclarationKind,
    pub status: BmadCustomizationStatus,
    pub code: String,
    pub key: String,
    pub literal: String,
    pub source_layer: BmadCustomizationLayerKind,
    pub provenance_label: String,
    pub source_hash: Sha256Digest,
    pub declaration_hash: Sha256Digest,
    #[serde(skip)]
    sequence: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadCustomizationConflict {
    pub code: String,
    pub key: String,
    pub previous_source_layer: BmadCustomizationLayerKind,
    pub next_source_layer: BmadCustomizationLayerKind,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadCustomizationResolution {
    pub schema_version: String,
    pub customization_id: String,
    pub capability_id: String,
    pub session_id: String,
    pub evidence_id: String,
    pub mode: BmadCustomizationMode,
    pub layers: Vec<BmadCustomizationLayerRecord>,
    pub text_overlays: Vec<BmadCustomizationTextOverlay>,
    pub dependencies: Vec<BmadCustomizationDeclaration>,
    pub tool_intents: Vec<BmadCustomizationDeclaration>,
    pub activation_proposals: Vec<BmadCustomizationDeclaration>,
    pub output_proposals: Vec<BmadCustomizationDeclaration>,
    pub forbidden_effects: Vec<BmadCustomizationDeclaration>,
    pub conflicts: Vec<BmadCustomizationConflict>,
    pub activation_allowed: bool,
    pub resolved_hash: Sha256Digest,
    pub provenance_hash: Sha256Digest,
    pub session_customization_hash: Sha256Digest,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum BmadCustomizationError {
    #[error("customization TOML is invalid")]
    Toml(TomlSubsetError),
    #[error("customization layer is invalid")]
    InvalidLayer,
    #[error("customization layer kind is duplicated")]
    DuplicateLayer,
    #[error("customization binding identity is invalid")]
    InvalidIdentity,
    #[error("customization exceeds its bounded declaration count")]
    BoundsViolation,
    #[error("customization hash could not be computed")]
    Hash,
}

pub struct BmadCustomizationResolver;

impl BmadCustomizationResolver {
    /// Resolves text and inert declarations with no effect-capable port.
    ///
    /// # Errors
    ///
    /// Returns an error only for malformed/bounds/hash failures. Unsafe
    /// declarations are retained as explicit forbidden records so interactive
    /// and headless callers receive the same deterministic diagnostics.
    pub fn resolve(
        request: &BmadCustomizationRequest,
    ) -> Result<BmadCustomizationResolution, BmadCustomizationError> {
        let mut ordered = request.layers.clone();
        ordered.sort_by_key(|layer| layer.kind.ordinal());

        let mut merged = Value::Object(Map::new());
        let mut origins = BTreeMap::new();
        let mut conflicts = Vec::new();
        for layer in &ordered {
            merge_value(
                &mut merged,
                &layer.value,
                "",
                layer.kind,
                &mut origins,
                &mut conflicts,
            );
        }

        let source_by_kind = ordered
            .iter()
            .map(|layer| (layer.kind, layer))
            .collect::<BTreeMap<_, _>>();
        let mut classified = Classified::default();
        classify_value(
            &merged,
            "",
            &origins,
            &source_by_kind,
            request,
            &mut classified,
        )?;
        classified.sort();

        let layers = ordered
            .iter()
            .map(|layer| BmadCustomizationLayerRecord {
                kind: layer.kind,
                ordinal: layer.kind.ordinal(),
                provenance_label: layer.provenance_label.clone(),
                content_hash: layer.content_hash,
                version: layer.version,
            })
            .collect::<Vec<_>>();
        let provenance_hash = hash(
            "bmad-customization-provenance",
            &serde_json::json!({ "layers": layers }),
        )?;
        let resolved_identity = serde_json::json!({
            "capabilityId": request.capability_id,
            "layers": layers,
            "textOverlays": classified.text,
            "dependencies": classified.dependencies,
            "toolIntents": classified.tools,
            "activationProposals": classified.activations,
            "outputProposals": classified.outputs,
            "forbiddenEffects": classified.forbidden,
            "conflicts": conflicts,
        });
        let resolved_hash = hash("bmad-customization-resolution", &resolved_identity)?;
        let session_customization_hash = hash(
            "bmad-session-customization",
            &serde_json::json!({
                "customizationId": request.customization_id,
                "capabilityId": request.capability_id,
                "sessionId": request.session_id,
                "evidenceId": request.evidence_id,
                "mode": request.mode,
                "resolvedHash": resolved_hash,
                "provenanceHash": provenance_hash,
            }),
        )?;
        let activation_allowed = classified.forbidden.is_empty()
            && classified
                .dependencies
                .iter()
                .chain(&classified.tools)
                .all(|item| item.status != BmadCustomizationStatus::Unavailable);

        Ok(BmadCustomizationResolution {
            schema_version: "sapphirus.bmad-customization.v1".to_owned(),
            customization_id: request.customization_id.clone(),
            capability_id: request.capability_id.clone(),
            session_id: request.session_id.clone(),
            evidence_id: request.evidence_id.clone(),
            mode: request.mode,
            layers,
            text_overlays: classified.text,
            dependencies: classified.dependencies,
            tool_intents: classified.tools,
            activation_proposals: classified.activations,
            output_proposals: classified.outputs,
            forbidden_effects: classified.forbidden,
            conflicts,
            activation_allowed,
            resolved_hash,
            provenance_hash,
            session_customization_hash,
        })
    }
}

#[derive(Default)]
struct Classified {
    next_sequence: usize,
    text: Vec<BmadCustomizationTextOverlay>,
    dependencies: Vec<BmadCustomizationDeclaration>,
    tools: Vec<BmadCustomizationDeclaration>,
    activations: Vec<BmadCustomizationDeclaration>,
    outputs: Vec<BmadCustomizationDeclaration>,
    forbidden: Vec<BmadCustomizationDeclaration>,
}

impl Classified {
    fn len(&self) -> usize {
        self.text.len()
            + self.dependencies.len()
            + self.tools.len()
            + self.activations.len()
            + self.outputs.len()
            + self.forbidden.len()
    }

    fn take_sequence(&mut self) -> usize {
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        sequence
    }

    fn with_sequence(
        &mut self,
        mut declaration: BmadCustomizationDeclaration,
    ) -> BmadCustomizationDeclaration {
        declaration.sequence = self.take_sequence();
        declaration
    }

    fn sort(&mut self) {
        self.text.sort_by(|left, right| left.key.cmp(&right.key));
        for declarations in [
            &mut self.dependencies,
            &mut self.tools,
            &mut self.activations,
            &mut self.outputs,
            &mut self.forbidden,
        ] {
            declarations.sort_by_key(|declaration| declaration.sequence);
        }
    }
}

fn merge_value(
    base: &mut Value,
    overlay: &Value,
    path: &str,
    source: BmadCustomizationLayerKind,
    origins: &mut BTreeMap<String, BmadCustomizationLayerKind>,
    conflicts: &mut Vec<BmadCustomizationConflict>,
) {
    match (base, overlay) {
        (Value::Object(base), Value::Object(overlay)) => {
            for (key, next) in overlay {
                let child = join_path(path, key);
                if let Some(previous) = base.get_mut(key) {
                    merge_value(previous, next, &child, source, origins, conflicts);
                } else {
                    base.insert(key.clone(), next.clone());
                    label_origins(next, &child, source, origins);
                }
            }
        }
        (Value::Array(base), Value::Array(overlay)) => {
            record_conflict(path, source, origins, conflicts);
            merge_arrays(base, overlay, path, source, origins);
            origins.insert(path.to_owned(), source);
        }
        (base, overlay) => {
            record_conflict(path, source, origins, conflicts);
            *base = overlay.clone();
            label_origins(overlay, path, source, origins);
        }
    }
}

fn merge_arrays(
    base: &mut Vec<Value>,
    overlay: &[Value],
    path: &str,
    source: BmadCustomizationLayerKind,
    origins: &mut BTreeMap<String, BmadCustomizationLayerKind>,
) {
    let combined = base.iter().chain(overlay).collect::<Vec<_>>();
    let keyed_by = ["code", "id"].into_iter().find(|candidate| {
        !combined.is_empty()
            && combined.iter().all(|item| {
                item.as_object()
                    .and_then(|object| object.get(*candidate))
                    .is_some()
            })
    });
    let Some(keyed_by) = keyed_by else {
        for item in overlay {
            let index = base.len();
            base.push(item.clone());
            label_origins(item, &array_item_path(path, index), source, origins);
        }
        return;
    };
    let mut indexes = base
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            item.get(keyed_by)
                .map(canonical_key)
                .map(|key| (key, index))
        })
        .collect::<BTreeMap<_, _>>();
    for item in overlay {
        let key = canonical_key(&item[keyed_by]);
        if let Some(index) = indexes.get(&key).copied() {
            base[index] = item.clone();
            label_origins(item, &array_item_path(path, index), source, origins);
        } else {
            let index = base.len();
            indexes.insert(key, index);
            base.push(item.clone());
            label_origins(item, &array_item_path(path, index), source, origins);
        }
    }
}

fn canonical_key(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}

fn record_conflict(
    path: &str,
    source: BmadCustomizationLayerKind,
    origins: &BTreeMap<String, BmadCustomizationLayerKind>,
    conflicts: &mut Vec<BmadCustomizationConflict>,
) {
    if let Some(previous) = origins.get(path).copied() {
        conflicts.push(BmadCustomizationConflict {
            code: "customization_value_overridden".to_owned(),
            key: path.to_owned(),
            previous_source_layer: previous,
            next_source_layer: source,
        });
    }
}

fn label_origins(
    value: &Value,
    path: &str,
    source: BmadCustomizationLayerKind,
    origins: &mut BTreeMap<String, BmadCustomizationLayerKind>,
) {
    origins.insert(path.to_owned(), source);
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                label_origins(child, &join_path(path, key), source, origins);
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                label_origins(child, &array_item_path(path, index), source, origins);
            }
        }
        _ => {}
    }
}

fn classify_value(
    value: &Value,
    path: &str,
    origins: &BTreeMap<String, BmadCustomizationLayerKind>,
    source_by_kind: &BTreeMap<BmadCustomizationLayerKind, &BmadCustomizationLayer>,
    request: &BmadCustomizationRequest,
    output: &mut Classified,
) -> Result<(), BmadCustomizationError> {
    if output.len() > MAX_DECLARATIONS {
        return Err(BmadCustomizationError::BoundsViolation);
    }
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                classify_value(
                    child,
                    &join_path(path, key),
                    origins,
                    source_by_kind,
                    request,
                    output,
                )?;
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                let item_path = array_item_path(path, index);
                if let Some(literal) = item.as_str() {
                    classify_literal(
                        &item_path,
                        literal,
                        origins,
                        source_by_kind,
                        request,
                        output,
                    )?;
                } else if item.is_object() {
                    if let Some(inert) = classify_nested_value(
                        item,
                        &item_path,
                        origins,
                        source_by_kind,
                        request,
                        output,
                    )? {
                        add_text_overlay(&inert, &item_path, origins, source_by_kind, output)?;
                    }
                } else {
                    add_text_overlay(item, &item_path, origins, source_by_kind, output)?;
                }
            }
        }
        Value::String(literal) => {
            classify_literal(path, literal, origins, source_by_kind, request, output)?;
        }
        _ => add_text_overlay(value, path, origins, source_by_kind, output)?,
    }
    Ok(())
}

fn classify_literal(
    path: &str,
    literal: &str,
    origins: &BTreeMap<String, BmadCustomizationLayerKind>,
    source_by_kind: &BTreeMap<BmadCustomizationLayerKind, &BmadCustomizationLayer>,
    request: &BmadCustomizationRequest,
    output: &mut Classified,
) -> Result<(), BmadCustomizationError> {
    let leaf = path
        .rsplit('.')
        .next()
        .unwrap_or(path)
        .split('[')
        .next()
        .unwrap_or(path);
    if classify_literal_declaration(
        path,
        leaf,
        literal,
        origins,
        source_by_kind,
        request,
        output,
    )? {
        return Ok(());
    }
    add_text_overlay(
        &Value::String(literal.to_owned()),
        path,
        origins,
        source_by_kind,
        output,
    )
}

fn classify_literal_declaration(
    path: &str,
    leaf: &str,
    literal: &str,
    origins: &BTreeMap<String, BmadCustomizationLayerKind>,
    source_by_kind: &BTreeMap<BmadCustomizationLayerKind, &BmadCustomizationLayer>,
    request: &BmadCustomizationRequest,
    output: &mut Classified,
) -> Result<bool, BmadCustomizationError> {
    if literal.is_empty() {
        return Ok(false);
    }
    if let Some(code) = forbidden_literal_code(leaf, literal) {
        let item = declaration(
            BmadCustomizationDeclarationKind::ForbiddenEffect,
            BmadCustomizationStatus::Forbidden,
            code,
            path,
            literal,
            origins,
            source_by_kind,
        )?;
        let item = output.with_sequence(item);
        output.forbidden.push(item);
        return Ok(true);
    }
    classify_declared_literal(
        path,
        leaf,
        literal,
        origins,
        source_by_kind,
        request,
        output,
    )
}

fn classify_nested_value(
    value: &Value,
    path: &str,
    origins: &BTreeMap<String, BmadCustomizationLayerKind>,
    source_by_kind: &BTreeMap<BmadCustomizationLayerKind, &BmadCustomizationLayer>,
    request: &BmadCustomizationRequest,
    output: &mut Classified,
) -> Result<Option<Value>, BmadCustomizationError> {
    match value {
        Value::Object(object) => {
            let mut inert = Map::new();
            for (key, child) in object {
                if let Some(value) = classify_nested_value(
                    child,
                    &join_path(path, key),
                    origins,
                    source_by_kind,
                    request,
                    output,
                )? {
                    inert.insert(key.clone(), value);
                }
            }
            Ok((!inert.is_empty()).then_some(Value::Object(inert)))
        }
        Value::Array(items) => {
            let mut inert = Vec::new();
            for (index, child) in items.iter().enumerate() {
                if let Some(value) = classify_nested_value(
                    child,
                    &array_item_path(path, index),
                    origins,
                    source_by_kind,
                    request,
                    output,
                )? {
                    inert.push(value);
                }
            }
            Ok((!inert.is_empty()).then_some(Value::Array(inert)))
        }
        Value::String(literal) => {
            let leaf = path
                .rsplit('.')
                .next()
                .unwrap_or(path)
                .split('[')
                .next()
                .unwrap_or(path);
            if classify_literal_declaration(
                path,
                leaf,
                literal,
                origins,
                source_by_kind,
                request,
                output,
            )? {
                Ok(None)
            } else {
                Ok(Some(value.clone()))
            }
        }
        _ => Ok(Some(value.clone())),
    }
}

fn forbidden_literal_code(key: &str, literal: &str) -> Option<&'static str> {
    if sensitive_key(key) || looks_like_secret(literal) {
        Some("customization_secret_forbidden")
    } else if is_network_literal(literal) {
        Some("customization_network_effect_forbidden")
    } else if looks_like_path_escape(literal) {
        Some("customization_path_escape")
    } else if injection_literal(literal) || is_process_effect_field(key) {
        Some("customization_injection_forbidden")
    } else {
        None
    }
}

fn classify_declared_literal(
    path: &str,
    leaf: &str,
    literal: &str,
    origins: &BTreeMap<String, BmadCustomizationLayerKind>,
    source_by_kind: &BTreeMap<BmadCustomizationLayerKind, &BmadCustomizationLayer>,
    request: &BmadCustomizationRequest,
    output: &mut Classified,
) -> Result<bool, BmadCustomizationError> {
    if let Some(skill) = literal
        .strip_prefix("skill:")
        .or_else(|| (leaf == "skill").then_some(literal))
    {
        let skill = skill.split_whitespace().next().unwrap_or(skill);
        let (status, code) = skill_status(skill, request);
        let item = declaration(
            if status == BmadCustomizationStatus::Forbidden {
                BmadCustomizationDeclarationKind::ForbiddenEffect
            } else {
                BmadCustomizationDeclarationKind::Dependency
            },
            status,
            code,
            path,
            literal,
            origins,
            source_by_kind,
        )?;
        let item = output.with_sequence(item);
        if status == BmadCustomizationStatus::Forbidden {
            output.forbidden.push(item);
        } else {
            output.dependencies.push(item);
        }
    } else if literal.starts_with("file:") || is_file_field(leaf, literal) {
        let item = declaration(
            BmadCustomizationDeclarationKind::Dependency,
            BmadCustomizationStatus::Available,
            "customization_file_dependency",
            path,
            literal,
            origins,
            source_by_kind,
        )?;
        let item = output.with_sequence(item);
        output.dependencies.push(item);
    } else if let Some(tool) = literal
        .strip_prefix("tool:")
        .or_else(|| (leaf == "tool").then_some(literal))
    {
        let tool = tool.split_whitespace().next().unwrap_or(tool);
        let status = if request.availability.available_tools.contains(tool) {
            BmadCustomizationStatus::Proposed
        } else {
            BmadCustomizationStatus::Unavailable
        };
        let item = declaration(
            BmadCustomizationDeclarationKind::ToolIntent,
            status,
            if status == BmadCustomizationStatus::Proposed {
                "customization_tool_proposed"
            } else {
                "customization_tool_unavailable"
            },
            path,
            literal,
            origins,
            source_by_kind,
        )?;
        let item = output.with_sequence(item);
        output.tools.push(item);
    } else if is_output_field(leaf) {
        let item = declaration(
            BmadCustomizationDeclarationKind::OutputProposal,
            BmadCustomizationStatus::Proposed,
            "customization_output_proposed",
            path,
            literal,
            origins,
            source_by_kind,
        )?;
        let item = output.with_sequence(item);
        output.outputs.push(item);
    } else if is_activation_field(leaf) {
        let item = declaration(
            BmadCustomizationDeclarationKind::ActivationProposal,
            BmadCustomizationStatus::Proposed,
            "customization_activation_proposed",
            path,
            literal,
            origins,
            source_by_kind,
        )?;
        let item = output.with_sequence(item);
        output.activations.push(item);
    } else {
        return Ok(false);
    }
    Ok(true)
}

fn skill_status(
    skill: &str,
    request: &BmadCustomizationRequest,
) -> (BmadCustomizationStatus, &'static str) {
    if skill == request.capability_id {
        (
            BmadCustomizationStatus::Forbidden,
            "customization_dependency_cycle",
        )
    } else if request.availability.available_skills.contains(skill) {
        (
            BmadCustomizationStatus::Available,
            "customization_skill_dependency",
        )
    } else {
        (
            BmadCustomizationStatus::Unavailable,
            "customization_skill_unavailable",
        )
    }
}

fn add_text_overlay(
    value: &Value,
    path: &str,
    origins: &BTreeMap<String, BmadCustomizationLayerKind>,
    source_by_kind: &BTreeMap<BmadCustomizationLayerKind, &BmadCustomizationLayer>,
    output: &mut Classified,
) -> Result<(), BmadCustomizationError> {
    let source = source_for(path, origins, source_by_kind)?;
    output.text.push(BmadCustomizationTextOverlay {
        key: path.to_owned(),
        value: value.clone(),
        source_layer: source.kind,
        provenance_label: source.provenance_label.clone(),
        source_hash: source.content_hash,
        overlay_hash: hash(
            "bmad-customization-text-overlay",
            &serde_json::json!({
                "key": path,
                "value": value,
                "sourceLayer": source.kind,
                "provenanceLabel": source.provenance_label,
                "sourceHash": source.content_hash,
            }),
        )?,
    });
    Ok(())
}

fn declaration(
    kind: BmadCustomizationDeclarationKind,
    status: BmadCustomizationStatus,
    code: &str,
    key: &str,
    literal: &str,
    origins: &BTreeMap<String, BmadCustomizationLayerKind>,
    source_by_kind: &BTreeMap<BmadCustomizationLayerKind, &BmadCustomizationLayer>,
) -> Result<BmadCustomizationDeclaration, BmadCustomizationError> {
    let source = source_for(key, origins, source_by_kind)?;
    let identity = serde_json::json!({
        "kind": kind,
        "status": status,
        "code": code,
        "key": key,
        "literal": literal,
        "sourceLayer": source.kind,
        "provenanceLabel": source.provenance_label,
        "sourceHash": source.content_hash,
    });
    Ok(BmadCustomizationDeclaration {
        kind,
        status,
        code: code.to_owned(),
        key: key.to_owned(),
        literal: literal.to_owned(),
        source_layer: source.kind,
        provenance_label: source.provenance_label.clone(),
        source_hash: source.content_hash,
        declaration_hash: hash("bmad-customization-declaration", &identity)?,
        sequence: 0,
    })
}

fn source_for<'a>(
    path: &str,
    origins: &BTreeMap<String, BmadCustomizationLayerKind>,
    source_by_kind: &'a BTreeMap<BmadCustomizationLayerKind, &BmadCustomizationLayer>,
) -> Result<&'a BmadCustomizationLayer, BmadCustomizationError> {
    let mut candidate = path;
    loop {
        if let Some(kind) = origins.get(candidate) {
            return source_by_kind
                .get(kind)
                .copied()
                .ok_or(BmadCustomizationError::InvalidLayer);
        }
        let Some((parent, _)) = candidate.rsplit_once('.') else {
            return Err(BmadCustomizationError::InvalidLayer);
        };
        candidate = parent;
    }
}

fn hash(purpose: &str, value: &Value) -> Result<Sha256Digest, BmadCustomizationError> {
    canonical_hash(purpose, 1, value).map_err(|_| BmadCustomizationError::Hash)
}

fn join_path(parent: &str, key: &str) -> String {
    if parent.is_empty() {
        key.to_owned()
    } else {
        format!("{parent}.{key}")
    }
}

fn array_item_path(parent: &str, index: usize) -> String {
    format!("{parent}[{index}]")
}

fn valid_contract_id(value: &str) -> bool {
    let Some((prefix, suffix)) = value.split_once('_') else {
        return false;
    };
    (2..=32).contains(&prefix.len())
        && prefix
            .bytes()
            .enumerate()
            .all(|(index, byte)| byte.is_ascii_lowercase() || index > 0 && byte.is_ascii_digit())
        && (16..=64).contains(&suffix.len())
        && suffix.bytes().all(|byte| {
            byte.is_ascii_digit()
                || matches!(
                    byte,
                    b'A'..=b'H' | b'J'..=b'K' | b'M'..=b'N' | b'P'..=b'T' | b'V'..=b'Z'
                )
        })
}

fn is_activation_field(value: &str) -> bool {
    matches!(
        value,
        "activation_steps_prepend" | "activation_steps_append" | "on_complete" | "prompt"
    ) || value.starts_with("on_")
}

fn is_process_effect_field(value: &str) -> bool {
    matches!(
        value,
        "command" | "exec" | "process" | "script" | "shell" | "system"
    )
}

fn is_output_field(value: &str) -> bool {
    value.ends_with("_output_path")
        || matches!(value, "output_dir" | "memory_dir" | "output_folder_name")
}

fn is_file_field(key: &str, value: &str) -> bool {
    (key.ends_with("_template")
        || key.ends_with("_file")
        || matches!(key, "pack" | "style_guide" | "brain_methods"))
        && (value.contains('/') || value.contains('{'))
}

fn looks_like_path_escape(value: &str) -> bool {
    let path = value.strip_prefix("file:").unwrap_or(value);
    let path = path
        .replace("{project-root}/", "")
        .replace("{skill-root}/", "");
    path.starts_with('/')
        || path.starts_with('\\')
        || path.contains('\\')
        || path.split('/').any(|component| component == "..")
        || {
            let bytes = path.as_bytes();
            bytes.len() > 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
        }
}

fn is_network_literal(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://") || value.starts_with("webhook:")
}

fn injection_literal(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "ignore previous instructions",
        "ignore all previous",
        "system prompt",
        "<script",
        "powershell -",
        "cmd.exe",
        "bash -c",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn sensitive_key(value: &str) -> bool {
    let normalized = value
        .bytes()
        .filter(|byte| !matches!(byte, b'_' | b'-'))
        .map(|byte| byte.to_ascii_lowercase())
        .collect::<Vec<_>>();
    matches!(
        normalized.as_slice(),
        b"apikey"
            | b"accesstoken"
            | b"clientsecret"
            | b"dbpassword"
            | b"privatekey"
            | b"connectionstring"
            | b"password"
            | b"secret"
            | b"token"
    )
}

fn looks_like_secret(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.starts_with("sk-")
        || lower.starts_with("ghp_")
        || lower.contains("://")
            && value
                .split_once("://")
                .and_then(|(_, rest)| rest.split_once('@'))
                .is_some_and(|(userinfo, _)| userinfo.contains(':'))
}
