#![forbid(unsafe_code)]

//! Handwritten BMAD conformance rules layered over generated wire shapes.

use std::collections::{HashMap, HashSet};

use desktop_runtime::{canonical_hash, canonical_hash_without_field};
use serde_json::Value;

/// Exact early Builder limit-profile identifier shared by all runtimes.
pub const BUILDER_LIMIT_PROFILE: &str = "sapphirus.bmad-builder-limits.v1";

/// Canonical tuple separator. Contract string members are required to be NUL-free.
const TUPLE_SEPARATOR: char = '\0';

const EXPECTED_AGENT_RECORD_HASHES: [(&str, &str); 6] = [
    (
        "bmad-agent-analyst",
        "sha256:6b37055d48b0b5a8186d4bac5986aefc68f30ca168124f0d101b6539c21adce9",
    ),
    (
        "bmad-agent-architect",
        "sha256:4dc48526aac64c60d15a389f707189ac313cfdf3c69290860790b0272c5f1d20",
    ),
    (
        "bmad-agent-dev",
        "sha256:00b6cd96945f5563f446e09f8cb5e5dc1c3cb11a2059e42555044d47f308f54f",
    ),
    (
        "bmad-agent-pm",
        "sha256:ee14a413e53a6f4f52d9ca83e24babe32ba7f5cd8d2324ef921cddeb89c24869",
    ),
    (
        "bmad-agent-tech-writer",
        "sha256:dbd78337564afb6d7b142c2ea3188f3b1eec3250d9ba8b64281bc016325f74bf",
    ),
    (
        "bmad-agent-ux-designer",
        "sha256:bc39797efddbbf455b30c3de5e4b67f5df1bd9d0d4417567ab3cb109f98fcfd5",
    ),
];

/// Builds a collision-safe semantic tuple key for already validated strings.
///
/// # Errors
///
/// Returns `BMAD_SCHEMA_INVALID` if a tuple member is not a string or contains NUL.
pub fn tuple_key<'a>(values: impl IntoIterator<Item = &'a Value>) -> Result<String, &'static str> {
    let mut key = String::new();
    for (index, value) in values.into_iter().enumerate() {
        let text = value.as_str().ok_or("BMAD_SCHEMA_INVALID")?;
        if text.contains(TUPLE_SEPARATOR) {
            return Err("BMAD_SCHEMA_INVALID");
        }
        if index != 0 {
            key.push(TUPLE_SEPARATOR);
        }
        key.push_str(text);
    }
    Ok(key)
}

/// Returns true only when values are strictly increasing by an ordinal key.
pub fn is_strictly_sorted_unique<T>(values: &[T], key: impl Fn(&T) -> String) -> bool {
    values.windows(2).all(|pair| key(&pair[0]) < key(&pair[1]))
}

fn string<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field)?.as_str()
}

fn array<'a>(value: &'a Value, field: &str) -> Option<&'a Vec<Value>> {
    value.get(field)?.as_array()
}

fn nullable_string(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn capability_key(value: &Value) -> String {
    [
        nullable_string(value, "packageVersionId"),
        nullable_string(value, "moduleCode"),
        nullable_string(value, "skillName"),
        nullable_string(value, "normalizedAction"),
    ]
    .join("\0")
}

fn scope_key(value: &Value) -> String {
    let Some(scope) = value.get("scope") else {
        return String::new();
    };
    [
        nullable_string(value, "graphKind"),
        nullable_string(scope, "packageVersionId"),
        nullable_string(scope, "moduleCode"),
        nullable_string(scope, "skillName"),
    ]
    .join("\0")
}

fn push_once(errors: &mut Vec<String>, code: &str) {
    if !errors.iter().any(|candidate| candidate == code) {
        errors.push(code.to_owned());
    }
}

fn verify_hash(value: &Value, errors: &mut Vec<String>) {
    let Some(version) = string(value, "schemaVersion") else {
        return;
    };
    let rule = match version {
        "sapphirus.bmad-package-descriptor.v1" => {
            Some(("bmad-package-descriptor", "descriptorHash"))
        }
        "sapphirus.bmad-capability-catalog.v1" => Some(("bmad-capability-catalog", "catalogHash")),
        "sapphirus.bmad-method-checkpoint.v1" => Some(("bmad-method-checkpoint", "checkpointHash")),
        "sapphirus.bmad-method-session.v1" => Some(("contract-object", "contentHash")),
        "sapphirus.bmad-builder-revision.v1" => Some(("bmad-builder-revision", "revisionHash")),
        "sapphirus.bmad-builder-analysis.v1" => Some(("bmad-builder-analysis", "analysisHash")),
        "sapphirus.bmad-validation-report.v1" => Some(("bmad-validation-report", "reportHash")),
        _ => None,
    };
    let Some((purpose, field)) = rule else {
        return;
    };
    let Ok(expected) = canonical_hash_without_field(purpose, 1, value, field) else {
        push_once(errors, "BMAD_SCHEMA_INVALID");
        return;
    };
    let expected = expected.to_string();
    if string(value, field) != Some(expected.as_str()) {
        push_once(errors, "HASH_MISMATCH");
    }
}

fn validate_descriptor(value: &Value, errors: &mut Vec<String>) {
    let source = value.get("sourceIdentity");
    if source.and_then(|source| string(source, "packageName")) != string(value, "packageName")
        || source.and_then(|source| string(source, "packageVersion"))
            != string(value, "packageVersion")
    {
        push_once(errors, "BMAD_SOURCE_IDENTITY_MISMATCH");
    }
    if string(value, "packageName") == Some("bmad-method") {
        let runtime = source
            .and_then(|source| array(source, "runtimeCompatibility"))
            .and_then(|items| items.first());
        if string(value, "packageVersion") != Some("6.10.0")
            || source
                .and_then(|source| source.get("moduleVersion"))
                .is_none_or(|v| !v.is_null())
            || source
                .and_then(|source| source.get("sourceFormatVersion"))
                .is_none_or(|v| !v.is_null())
            || source.and_then(|source| string(source, "archiveArtifactLabel"))
                != Some("BMAD-METHOD-main.zip")
            || source.and_then(|source| string(source, "archiveSha256"))
                != Some("sha256:a7c049038099b99081fbd03d22c6a5180edd88dee656bb37c4276b1cc31b4a32")
            || runtime.and_then(|item| string(item, "runtime")) != Some("node")
            || runtime.and_then(|item| string(item, "versionRange")) != Some(">=20.12.0")
        {
            push_once(errors, "BMAD_METHOD_SOURCE_IDENTITY_MISMATCH");
        }
    }
    let Some(graphs) = array(value, "configGraphs") else {
        return;
    };
    let Some(resolutions) = array(value, "configResolutions") else {
        return;
    };
    let kinds: HashSet<_> = graphs
        .iter()
        .filter_map(|graph| string(graph, "graphKind"))
        .collect();
    if kinds
        != HashSet::from([
            "compatibility_yaml",
            "method_central_toml",
            "skill_customization_toml",
        ])
    {
        push_once(errors, "BMAD_CONFIG_GRAPHS_INCOMPLETE");
    }
    if !is_strictly_sorted_unique(graphs, scope_key) {
        push_once(errors, "BMAD_CONFIG_GRAPH_NOT_CANONICAL");
    }
    if !is_strictly_sorted_unique(resolutions, scope_key) {
        push_once(errors, "BMAD_CONFIG_RESOLUTION_NOT_CANONICAL");
    }
    let graph_by_key: HashMap<_, _> = graphs
        .iter()
        .map(|graph| (scope_key(graph), graph))
        .collect();
    for resolution in resolutions {
        let Some(graph) = graph_by_key.get(&scope_key(resolution)).copied() else {
            push_once(errors, "BMAD_CONFIG_RESOLUTION_ORPHAN");
            continue;
        };
        let ordered = array(resolution, "orderedLayerHashes");
        let expected: Option<Vec<_>> = array(graph, "layers").map(|layers| {
            layers
                .iter()
                .map(|layer| layer.get("sourceHash").cloned().unwrap_or(Value::Null))
                .collect()
        });
        if string(graph, "graphHash") != string(resolution, "graphHash")
            || ordered.cloned() != expected
        {
            push_once(errors, "BMAD_CONFIG_RESOLUTION_BINDING_MISMATCH");
        }
    }
    for graph in graphs {
        let kind = string(graph, "graphKind").unwrap_or_default();
        let Some(scope) = graph.get("scope") else {
            continue;
        };
        let module = scope.get("moduleCode").and_then(Value::as_str);
        let skill = scope.get("skillName").and_then(Value::as_str);
        if string(scope, "packageVersionId") != string(value, "packageVersionId")
            || (kind == "method_central_toml" && (module.is_some() || skill.is_some()))
            || (kind == "skill_customization_toml" && (module.is_none() || skill.is_none()))
            || (kind == "compatibility_yaml" && module.is_none())
        {
            push_once(errors, "BMAD_CONFIG_SCOPE_INVALID");
        }
        if array(graph, "layers").is_some_and(|layers| {
            !is_strictly_sorted_unique(layers, |layer| {
                format!(
                    "{:08}\0{}",
                    layer
                        .get("ordinal")
                        .and_then(Value::as_u64)
                        .unwrap_or_default(),
                    nullable_string(layer, "sourcePath")
                )
            }) || layers
                .iter()
                .any(|layer| string(layer, "graphKind") != Some(kind))
        }) {
            push_once(errors, "BMAD_CONFIG_LAYER_INVALID");
        }
    }
    for (field, code, key) in [
        ("modules", "BMAD_MODULE_SET_NOT_CANONICAL", "moduleCode"),
        (
            "resourceInventory",
            "BMAD_RESOURCE_SET_NOT_CANONICAL",
            "path",
        ),
    ] {
        if let Some(values) = array(value, field) {
            if !is_strictly_sorted_unique(values, |item| nullable_string(item, key)) {
                push_once(errors, code);
            }
        }
    }
    if let Some(skills) = array(value, "skills") {
        if !is_strictly_sorted_unique(skills, |skill| {
            format!(
                "{}\0{}",
                nullable_string(skill, "moduleCode"),
                nullable_string(skill, "skillName")
            )
        }) {
            push_once(errors, "BMAD_SKILL_SET_NOT_CANONICAL");
        }
    }
    let Some(resources) = array(value, "resourceInventory") else {
        return;
    };
    let Some(projections) = array(value, "instructionProjections") else {
        return;
    };
    if !is_strictly_sorted_unique(projections, |projection| {
        nullable_string(projection, "projectionId")
    }) {
        push_once(errors, "BMAD_INSTRUCTION_PROJECTION_SET_NOT_CANONICAL");
    }
    let mut projection_hashes = HashSet::new();
    for projection in projections {
        if string(projection, "sourceIdentityHash") != string(value, "sourceSnapshotHash")
            || !projection_hashes.insert(nullable_string(projection, "projectionHash"))
        {
            push_once(errors, "BMAD_INSTRUCTION_PROJECTION_IDENTITY_MISMATCH");
        }
        if let Some(sources) = array(projection, "sourceResources") {
            if !is_strictly_sorted_unique(sources, |source| nullable_string(source, "path")) {
                push_once(errors, "BMAD_INSTRUCTION_PROJECTION_SOURCE_NOT_CANONICAL");
            }
            for source in projection
                .get("sourceEntrypoint")
                .into_iter()
                .chain(sources.iter())
            {
                let found = resources.iter().any(|resource| {
                    string(resource, "path") == string(source, "path")
                        && string(resource, "contentHash") == string(source, "contentHash")
                        && string(resource, "treatment") == string(source, "treatment")
                        && string(resource, "locationKind") == Some("source_tree")
                });
                if !found {
                    push_once(errors, "BMAD_INSTRUCTION_PROJECTION_SOURCE_TRANSPLANT");
                }
            }
        }
        if let Some(managed) = projection.get("managedInstruction") {
            let found = resources.iter().any(|resource| {
                string(resource, "path") == string(managed, "path")
                    && string(resource, "contentHash") == string(managed, "contentHash")
                    && string(resource, "locationKind") == Some("managed_projection")
                    && string(resource, "contentRole") == Some("managed_instruction")
                    && string(resource, "runtimeUse") == Some("instruction_data")
            });
            if !found {
                push_once(errors, "BMAD_MANAGED_INSTRUCTION_TRANSPLANT");
            }
        }
    }
    if let Some(skills) = array(value, "skills") {
        let module_codes: HashSet<_> = array(value, "modules")
            .into_iter()
            .flatten()
            .filter_map(|module| string(module, "moduleCode"))
            .collect();
        for skill in skills {
            if !module_codes.contains(string(skill, "moduleCode").unwrap_or_default()) {
                push_once(errors, "BMAD_SKILL_MODULE_ORPHAN");
            }
            if !descriptor_has_resource(
                value,
                string(skill, "sourceEntrypointPath").unwrap_or_default(),
                string(skill, "sourceEntrypointHash").unwrap_or_default(),
            ) {
                push_once(errors, "BMAD_SKILL_SOURCE_TRANSPLANT");
            }
            let projection = projections.iter().find(|projection| {
                string(projection, "projectionHash") == string(skill, "instructionProjectionHash")
            });
            if projection.is_none_or(|projection| {
                let entry = projection.get("sourceEntrypoint");
                entry.and_then(|entry| string(entry, "path"))
                    != string(skill, "sourceEntrypointPath")
                    || entry.and_then(|entry| string(entry, "contentHash"))
                        != string(skill, "sourceEntrypointHash")
            }) {
                push_once(errors, "BMAD_SKILL_PROJECTION_TRANSPLANT");
            }
        }
    }
}

fn descriptor_has_resource(descriptor: &Value, path: &str, hash: &str) -> bool {
    array(descriptor, "resourceInventory").is_some_and(|resources| {
        resources.iter().any(|resource| {
            string(resource, "path") == Some(path) && string(resource, "contentHash") == Some(hash)
        })
    })
}

fn agent_record_hash_is_exact(agent: &Value) -> bool {
    let Some(expected) = EXPECTED_AGENT_RECORD_HASHES
        .iter()
        .find_map(|(code, hash)| (string(agent, "agentCode") == Some(*code)).then_some(*hash))
    else {
        return false;
    };
    let record = serde_json::json!({
        "moduleCode": agent.get("moduleCode"),
        "agentCode": agent.get("agentCode"),
        "name": agent.get("name"),
        "title": agent.get("title"),
        "icon": agent.get("icon"),
        "team": agent.get("team"),
        "description": agent.get("description"),
        "personaSourceHash": agent.get("personaSourceHash"),
        "customizationSourceHash": agent.get("customizationSourceHash"),
        "menuItems": agent.get("menuItems"),
    });
    let Some(record_hash) = canonical_hash("bmad-agent-record", 1, &record)
        .ok()
        .map(|hash| hash.to_string())
    else {
        return false;
    };
    let Some(menu_hash) = agent
        .get("menuItems")
        .and_then(|items| canonical_hash("bmad-agent-menu-graph", 1, items).ok())
        .map(|hash| hash.to_string())
    else {
        return false;
    };
    record_hash == expected
        && string(agent, "agentRecordHash") == Some(expected)
        && string(agent, "menuGraphHash") == Some(menu_hash.as_str())
        && string(agent, "personaCustomizationGraphHash")
            == string(agent, "customizationSourceHash")
}

fn validate_catalog(value: &Value, descriptor: Option<&Value>, errors: &mut Vec<String>) {
    if let Some(descriptor) = descriptor {
        if string(value, "packageVersionId") != string(descriptor, "packageVersionId")
            || string(value, "descriptorHash") != string(descriptor, "descriptorHash")
        {
            push_once(errors, "BMAD_CATALOG_DESCRIPTOR_BINDING_MISMATCH");
        }
    }
    let Some(skills) = array(value, "installedSkills") else {
        return;
    };
    if !is_strictly_sorted_unique(skills, |skill| {
        format!(
            "{}\0{}",
            nullable_string(skill, "moduleCode"),
            nullable_string(skill, "skillName")
        )
    }) {
        push_once(errors, "BMAD_INSTALLED_SKILL_SET_NOT_CANONICAL");
    }
    let mut installed = HashSet::new();
    for skill in skills {
        let Some(keys) = array(skill, "capabilityKeys") else {
            continue;
        };
        let cardinality_valid = match string(skill, "actionCardinality") {
            Some("single_action") => keys.len() == 1,
            Some("multi_action") => {
                keys.len() >= 2
                    && keys
                        .iter()
                        .all(|key| key.get("normalizedAction").is_some_and(Value::is_string))
            }
            _ => false,
        };
        if !cardinality_valid {
            push_once(errors, "BMAD_CAPABILITY_CARDINALITY_INVALID");
        }
        if !is_strictly_sorted_unique(keys, capability_key) {
            push_once(errors, "BMAD_CAPABILITY_SET_NOT_CANONICAL");
        }
        for key in keys {
            let encoded = capability_key(key);
            if string(key, "packageVersionId") != string(value, "packageVersionId")
                || string(key, "moduleCode") != string(skill, "moduleCode")
                || string(key, "skillName") != string(skill, "skillName")
                || !installed.insert(encoded)
            {
                push_once(errors, "BMAD_CAPABILITY_KEY_COLLISION");
            }
        }
        if let Some(descriptor) = descriptor {
            let descriptor_skill = array(descriptor, "skills").and_then(|descriptor_skills| {
                descriptor_skills.iter().find(|candidate| {
                    string(candidate, "moduleCode") == string(skill, "moduleCode")
                        && string(candidate, "skillName") == string(skill, "skillName")
                })
            });
            if descriptor_skill.is_none_or(|candidate| {
                string(candidate, "sourceEntrypointHash") != string(skill, "sourceEntrypointHash")
                    || string(candidate, "resourceSetHash") != string(skill, "resourceSetHash")
                    || string(candidate, "skillDescriptorHash")
                        != string(skill, "skillDescriptorHash")
                    || candidate
                        .get("executionProfile")
                        .and_then(|profile| string(profile, "profileHash"))
                        != string(skill, "executionProfileHash")
                    || string(candidate, "instructionProjectionHash")
                        != string(skill, "instructionProjectionHash")
                    || string(candidate, "distributionProfile")
                        != string(skill, "distributionProfile")
                    || string(candidate, "installProfile") != string(skill, "installProfile")
                    || candidate
                        .get("executionProfile")
                        .and_then(|profile| string(profile, "entrypointKind"))
                        != string(skill, "entrypointKind")
                    || candidate
                        .get("executionProfile")
                        .and_then(|profile| string(profile, "validationProfile"))
                        != string(skill, "validationProfile")
            }) {
                push_once(errors, "BMAD_INSTALLED_SKILL_TRANSPLANT");
            }
        }
    }
    let mut dependencies = HashSet::new();
    if let Some(values) = array(value, "dependencyAvailability") {
        if !is_strictly_sorted_unique(values, |dependency| {
            dependency
                .get("capabilityKey")
                .map_or_else(String::new, capability_key)
        }) {
            push_once(errors, "BMAD_DEPENDENCY_SET_NOT_CANONICAL");
        }
        for dependency in values {
            let Some(key) = dependency.get("capabilityKey") else {
                continue;
            };
            let encoded = capability_key(key);
            if installed.contains(&encoded) || !dependencies.insert(encoded) {
                push_once(errors, "BMAD_CAPABILITY_KEY_COLLISION");
            }
        }
    }
    if let Some(actions) = value
        .get("helpActionGraph")
        .and_then(|graph| array(graph, "actions"))
    {
        if !is_strictly_sorted_unique(actions, |action| {
            action
                .get("capabilityKey")
                .map_or_else(String::new, capability_key)
        }) {
            push_once(errors, "BMAD_HELP_ACTION_SET_NOT_CANONICAL");
        }
        for action in actions {
            let encoded = action
                .get("capabilityKey")
                .map_or_else(String::new, capability_key);
            if !installed.contains(&encoded) && !dependencies.contains(&encoded) {
                push_once(errors, "BMAD_HELP_ORPHAN");
            }
        }
    }
    if value
        .get("helpActionGraph")
        .and_then(|graph| string(graph, "packageVersionId"))
        != string(value, "packageVersionId")
        || value
            .get("agentRoster")
            .and_then(|roster| string(roster, "packageVersionId"))
            != string(value, "packageVersionId")
    {
        push_once(errors, "BMAD_CATALOG_PACKAGE_BINDING_MISMATCH");
    }
    let Some(agents) = value
        .get("agentRoster")
        .and_then(|roster| array(roster, "agents"))
    else {
        return;
    };
    if !is_strictly_sorted_unique(agents, |agent| {
        format!(
            "{}\0{}",
            nullable_string(agent, "moduleCode"),
            nullable_string(agent, "agentCode")
        )
    }) {
        push_once(errors, "BMAD_AGENT_ROSTER_NOT_CANONICAL");
    }
    if agents.len() != EXPECTED_AGENT_RECORD_HASHES.len()
        || agents
            .iter()
            .any(|agent| !agent_record_hash_is_exact(agent))
    {
        push_once(errors, "BMAD_AGENT_ROSTER_BINDING_MISMATCH");
    }
    for agent in agents {
        let mut menu_codes = HashSet::new();
        let mut previous_ordinal = None;
        let Some(menu_items) = array(agent, "menuItems") else {
            continue;
        };
        for item in menu_items {
            let code = string(item, "menuCode").unwrap_or_default();
            let ordinal = item
                .get("sourceOrdinal")
                .and_then(Value::as_u64)
                .unwrap_or_default();
            if !menu_codes.insert(code) || previous_ordinal.is_some_and(|last| ordinal <= last) {
                push_once(errors, "BMAD_MENU_SCOPE_AMBIGUOUS");
            }
            previous_ordinal = Some(ordinal);
            let Some(target) = item.get("target") else {
                continue;
            };
            if string(target, "sourceCustomizationGraphHash")
                != string(agent, "personaCustomizationGraphHash")
            {
                push_once(errors, "BMAD_MENU_TARGET_TRANSPLANT");
            }
            match string(target, "targetKind") {
                Some("skill_target") => {
                    let encoded = target
                        .get("capabilityKey")
                        .map_or_else(String::new, capability_key);
                    if !installed.contains(&encoded) && !dependencies.contains(&encoded) {
                        push_once(errors, "BMAD_AGENT_MENU_ORPHAN");
                    }
                }
                Some("prompt_reference") => {
                    if let Some(descriptor) = descriptor {
                        let path = string(target, "sourceLocalMemberLabel").unwrap_or_default();
                        let hash = string(target, "sourceMemberHash").unwrap_or_default();
                        if !descriptor_has_resource(descriptor, path, hash) {
                            push_once(errors, "BMAD_PROMPT_REFERENCE_TRANSPLANT");
                        }
                    }
                }
                _ => {}
            }
        }
        if let Some(descriptor) = descriptor {
            let module = array(descriptor, "modules").and_then(|modules| {
                modules
                    .iter()
                    .find(|module| string(module, "moduleCode") == string(agent, "moduleCode"))
            });
            if module.and_then(|module| string(module, "metadataSourceHash"))
                != string(agent, "moduleSourceHash")
            {
                push_once(errors, "BMAD_AGENT_MODULE_HASH_MISMATCH");
            }
            let persona = string(agent, "personaSourceHash").unwrap_or_default();
            if !array(descriptor, "resourceInventory").is_some_and(|resources| {
                resources
                    .iter()
                    .any(|resource| string(resource, "contentHash") == Some(persona))
            }) {
                push_once(errors, "BMAD_PERSONA_HASH_MISMATCH");
            }
            let customization = string(agent, "customizationSourceHash").unwrap_or_default();
            if !array(descriptor, "resourceInventory").is_some_and(|resources| {
                resources
                    .iter()
                    .any(|resource| string(resource, "contentHash") == Some(customization))
            }) {
                push_once(errors, "BMAD_CUSTOMIZATION_HASH_MISMATCH");
            }
        }
    }
}

fn same_capability(left: Option<&Value>, right: Option<&Value>) -> bool {
    left.is_some_and(|left| {
        right.is_some_and(|right| capability_key(left) == capability_key(right))
    })
}

fn validate_method(value: &Value, catalog: Option<&Value>, errors: &mut Vec<String>) {
    let profile = value.get("executionProfile");
    if profile.and_then(|profile| string(profile, "profileHash"))
        != string(value, "executionProfileHash")
        || profile.and_then(|profile| string(profile, "validationProfile"))
            != string(value, "validationProfile")
    {
        push_once(errors, "BMAD_METHOD_PROFILE_BINDING_MISMATCH");
    }
    let capability = value.get("capabilityKey");
    let is_help = string(value, "methodShape") == Some("no_agent_direct");
    if is_help {
        let actions = profile
            .and_then(|profile| profile.get("invocationModes"))
            .and_then(|modes| array(modes, "actions"));
        if capability.and_then(|key| string(key, "moduleCode")) != Some("core")
            || capability.and_then(|key| string(key, "skillName")) != Some("bmad-help")
            || capability
                .and_then(|key| key.get("normalizedAction"))
                .is_none_or(|action| !action.is_null())
            || value
                .get("agentBinding")
                .and_then(|binding| string(binding, "bindingKind"))
                != Some("no_agent")
            || value
                .get("agentRosterHash")
                .is_none_or(|hash| !hash.is_null())
            || profile.and_then(|profile| string(profile, "entrypointKind")) != Some("direct")
            || actions.is_none_or(|actions| !actions.is_empty())
            || string(value, "validationProfile") != Some("MethodOfficialSkillV6")
        {
            push_once(errors, "BMAD_HELP_BINDING_MISMATCH");
        }
    } else {
        let actions = profile
            .and_then(|profile| profile.get("invocationModes"))
            .and_then(|modes| array(modes, "actions"));
        let agent = value.get("agentBinding");
        if capability.and_then(|key| string(key, "moduleCode")) != Some("bmm")
            || capability.and_then(|key| string(key, "skillName")) != Some("bmad-architecture")
            || capability.and_then(|key| string(key, "normalizedAction")) != Some("create")
            || profile.and_then(|profile| string(profile, "entrypointKind")) != Some("step_jit")
            || actions.is_none_or(|actions| {
                actions.len() != 1 || actions.first().and_then(Value::as_str) != Some("create")
            })
            || profile
                .and_then(|profile| profile.get("resourcePolicy"))
                .and_then(|policy| string(policy, "resourceTiming"))
                != Some("current_step_only")
            || string(value, "validationProfile") != Some("MethodStepWorkflowV6")
            || agent.and_then(|agent| string(agent, "rosterHash"))
                != string(value, "agentRosterHash")
            || agent.and_then(|agent| string(agent, "moduleSourceHash"))
                != Some("sha256:5a2a4ff761b3a4f92730442386486f32318152fc0dfdd225dc6765a3bc2ec100")
            || agent.and_then(|agent| string(agent, "personaHash"))
                != Some("sha256:6d3512c6f9014a2344418ce0b53b1c9ed8521e6bf8b337f2a802ade6307146e4")
            || agent.and_then(|agent| string(agent, "customizationGraphHash"))
                != Some("sha256:d9763009d7c20246119c24bcea5eacebd21ad60c22ab191b74c9a5fb6e5f57ad")
            || !same_capability(
                agent.and_then(|agent| agent.get("menuCapabilityKey")),
                capability,
            )
        {
            push_once(errors, "BMAD_ARCHITECT_BINDING_MISMATCH");
        }
    }
    if let Some(catalog) = catalog.filter(|catalog| {
        string(catalog, "schemaVersion") == Some("sapphirus.bmad-capability-catalog.v1")
    }) {
        let installed = array(catalog, "installedSkills").and_then(|skills| {
            skills.iter().find(|skill| {
                array(skill, "capabilityKeys").is_some_and(|keys| {
                    keys.iter()
                        .any(|key| same_capability(Some(key), capability))
                })
            })
        });
        if string(catalog, "packageVersionId") != string(value, "packageVersionId")
            || string(catalog, "catalogHash") != string(value, "capabilityCatalogHash")
            || installed.is_none_or(|skill| {
                string(skill, "instructionProjectionHash")
                    != string(value, "instructionProjectionHash")
                    || string(skill, "resourceSetHash") != string(value, "resourceSetHash")
                    || string(skill, "executionProfileHash")
                        != string(value, "executionProfileHash")
                    || string(skill, "validationProfile") != string(value, "validationProfile")
                    || string(skill, "distributionProfile") != string(value, "distributionProfile")
                    || string(skill, "installProfile") != string(value, "installProfile")
            })
        {
            push_once(errors, "BMAD_METHOD_CATALOG_BINDING_MISMATCH");
        }
        if !is_help {
            let binding = value.get("agentBinding");
            let roster = catalog.get("agentRoster");
            let agent = roster
                .and_then(|roster| array(roster, "agents"))
                .and_then(|agents| {
                    agents.iter().find(|agent| {
                        string(agent, "agentCode")
                            == binding.and_then(|binding| string(binding, "agentCode"))
                    })
                });
            let menu = agent
                .and_then(|agent| array(agent, "menuItems"))
                .and_then(|items| {
                    items.iter().find(|item| {
                        string(item, "menuCode")
                            == binding.and_then(|binding| string(binding, "menuCode"))
                    })
                });
            if roster.and_then(|roster| string(roster, "rosterHash"))
                != string(value, "agentRosterHash")
                || agent.is_none_or(|agent| {
                    string(agent, "agentRecordHash")
                        != binding.and_then(|binding| string(binding, "agentRecordHash"))
                        || string(agent, "moduleSourceHash")
                            != binding.and_then(|binding| string(binding, "moduleSourceHash"))
                        || string(agent, "personaCustomizationGraphHash")
                            != binding.and_then(|binding| string(binding, "customizationGraphHash"))
                })
                || menu.is_none_or(|menu| {
                    string(menu, "sourceMenuItemHash")
                        != binding.and_then(|binding| string(binding, "menuItemHash"))
                        || !same_capability(
                            menu.get("target")
                                .and_then(|target| target.get("capabilityKey")),
                            capability,
                        )
                })
            {
                push_once(errors, "BMAD_METHOD_AGENT_CATALOG_TRANSPLANT");
            }
        }
    }
    let Some(checkpoints) = array(value, "checkpoints") else {
        return;
    };
    let mut ids = HashSet::new();
    let mut checkpoint_decisions = HashSet::new();
    for (index, checkpoint) in checkpoints.iter().enumerate() {
        if string(checkpoint, "sessionId") != string(value, "sessionId")
            || checkpoint.get("turnOrdinal").and_then(Value::as_u64)
                != u64::try_from(index + 1).ok()
            || !ids.insert(nullable_string(checkpoint, "checkpointId"))
            || !checkpoint_decisions.insert(nullable_string(checkpoint, "contextDecisionId"))
            || !same_capability(checkpoint.get("capabilityKey"), capability)
            || string(checkpoint, "contextDigest") != string(value, "contextDigest")
            || string(checkpoint, "modelBindingHash")
                != value
                    .get("modelBinding")
                    .and_then(|binding| string(binding, "bindingHash"))
        {
            push_once(errors, "BMAD_TURN_ORDINAL_INVALID");
        }
        verify_hash(checkpoint, errors);
    }
    let ledger = value.get("contextLedger");
    let entries = ledger.and_then(|ledger| array(ledger, "entries"));
    let consumptions = array(value, "decisionConsumptions");
    if ledger.and_then(|ledger| string(ledger, "sessionId")) != string(value, "sessionId")
        || entries.map(Vec::len) != consumptions.map(Vec::len)
        || checkpoints.len() != consumptions.map_or(usize::MAX, Vec::len)
    {
        push_once(errors, "BMAD_CONTEXT_LEDGER_BINDING_MISMATCH");
    }
    let mut ledger_by_decision = HashMap::new();
    if let Some(entries) = entries {
        for (index, entry) in entries.iter().enumerate() {
            let decision = nullable_string(entry, "contextDecisionId");
            if entry.get("reviewOrdinal").and_then(Value::as_u64) != u64::try_from(index + 1).ok()
                || ledger_by_decision.insert(decision, entry).is_some()
                || string(entry, "contextDigest") != string(value, "contextDigest")
                || string(entry, "resourceSetHash") != string(value, "resourceSetHash")
                || string(entry, "packageDescriptorHash") != string(value, "packageDescriptorHash")
                || string(entry, "instructionProjectionHash")
                    != string(value, "instructionProjectionHash")
                || string(entry, "configResolutionHash") != string(value, "configResolutionHash")
                || string(entry, "customizationHash") != string(value, "customizationHash")
                || string(entry, "modelBindingHash")
                    != value
                        .get("modelBinding")
                        .and_then(|binding| string(binding, "bindingHash"))
                || string(entry, "methodSchemaHash") != string(value, "methodSchemaHash")
                || string(entry, "executionProfileHash") != string(value, "executionProfileHash")
                || string(entry, "validationProfileHash") != string(value, "validationProfileHash")
            {
                push_once(errors, "BMAD_CONTEXT_LEDGER_BINDING_MISMATCH");
            }
        }
    }
    let mut decisions = HashSet::new();
    let mut invocations = HashSet::new();
    if let Some(consumptions) = consumptions {
        for consumption in consumptions {
            let decision = nullable_string(consumption, "decisionId");
            let entry = ledger_by_decision.get(&decision).copied();
            let exact = string(consumption, "sessionId") == string(value, "sessionId")
                && checkpoint_decisions.contains(&decision)
                && entry.is_some_and(|entry| {
                    string(entry, "manifestHash") == string(consumption, "manifestHash")
                        && string(entry, "consentHash") == string(consumption, "consentHash")
                })
                && string(consumption, "packageDescriptorHash")
                    == string(value, "packageDescriptorHash")
                && string(consumption, "packageSourceHash") == string(value, "packageSourceHash")
                && string(consumption, "instructionProjectionHash")
                    == string(value, "instructionProjectionHash")
                && string(consumption, "capabilityCatalogHash")
                    == string(value, "capabilityCatalogHash")
                && same_capability(consumption.get("capabilityKey"), capability)
                && string(consumption, "contextDigest") == string(value, "contextDigest")
                && string(consumption, "distributionProfile")
                    == string(value, "distributionProfile")
                && string(consumption, "installProfile") == string(value, "installProfile")
                && string(consumption, "executionProfileHash")
                    == string(value, "executionProfileHash")
                && string(consumption, "validationProfileHash")
                    == string(value, "validationProfileHash")
                && string(consumption, "configResolutionHash")
                    == string(value, "configResolutionHash")
                && string(consumption, "customizationHash") == string(value, "customizationHash")
                && string(consumption, "resourceSetHash") == string(value, "resourceSetHash")
                && consumption.get("modelBinding") == value.get("modelBinding")
                && string(consumption, "methodSchemaHash") == string(value, "methodSchemaHash");
            if !decisions.insert(decision)
                || !invocations.insert(nullable_string(consumption, "invocationId"))
                || !exact
            {
                push_once(errors, "BMAD_CONTEXT_DECISION_REUSED");
            }
        }
    }
}

fn validate_builder(value: &Value, errors: &mut Vec<String>) {
    let kind = string(value, "builderKind").unwrap_or_default();
    let expected_profile = if kind == "agent" {
        "BuilderAgentV2Stateless"
    } else {
        "BuilderOutcomeSkillV2"
    };
    if string(value, "validationProfile") != Some(expected_profile) {
        push_once(errors, "BMAD_PROFILE_AMBIGUOUS");
    }
    if string(value, "objectKind") == Some("revision") {
        let Some(file_set) = value.get("proposedFileSet") else {
            return;
        };
        if string(file_set, "limitProfile") != Some(BUILDER_LIMIT_PROFILE) {
            push_once(errors, "BMAD_BUILDER_LIMIT_PROFILE_MISMATCH");
        }
        let Some(files) = array(file_set, "files") else {
            return;
        };
        let mut folded = HashSet::new();
        let mut paths = Vec::new();
        let mut total_bytes = 0usize;
        for file in files {
            let path = string(file, "path").unwrap_or_default();
            if !folded.insert(path.to_lowercase()) {
                push_once(errors, "BMAD_BUILDER_PATH_COLLISION");
            }
            paths.push(path);
            total_bytes += string(file, "content").map_or(0, str::len);
        }
        if total_bytes > 1_048_576 {
            push_once(errors, "BMAD_BUILDER_TOTAL_TOO_LARGE");
        }
        paths.sort_unstable();
        if (kind == "workflow" && paths != ["SKILL.md"])
            || (kind == "agent"
                && ![
                    "SKILL.md",
                    "customize.toml",
                    "references/prompt-quality-canon.md",
                ]
                .iter()
                .all(|required| paths.contains(required)))
        {
            push_once(errors, "BMAD_BUILDER_INVENTORY_INVALID");
        }
    }
    if string(value, "objectKind") != Some("analysis") {
        return;
    }
    if string(value, "analysisKind") == Some("model_lens") {
        let expected: &[&str] = if kind == "agent" {
            &[
                "leanness",
                "architecture",
                "determinism",
                "customization",
                "enhancement",
                "agent-cohesion",
            ]
        } else {
            &[
                "leanness",
                "architecture",
                "determinism",
                "customization",
                "enhancement",
            ]
        };
        let Some(results) = array(value, "modelLensResults") else {
            return;
        };
        if results.len() != expected.len()
            || results
                .iter()
                .zip(expected)
                .any(|(result, expected)| string(result, "lens") != Some(expected))
        {
            push_once(errors, "BMAD_MODEL_LENS_SET_INVALID");
        }
        let binding = value.get("modelBinding");
        for result in results {
            if string(result, "builderKind") != string(value, "builderKind")
                || string(result, "revisionId") != string(value, "revisionId")
                || string(result, "revisionHash") != string(value, "revisionHash")
                || string(result, "sourceMemberSetHash") != string(value, "sourceMemberSetHash")
                || string(result, "instructionProjectionSetHash")
                    != string(value, "instructionProjectionSetHash")
                || string(result, "deterministicFactsHash")
                    != string(value, "deterministicFactsHash")
                || string(result, "modelHash")
                    != binding.and_then(|binding| string(binding, "modelHash"))
                || string(result, "deploymentHash")
                    != binding.and_then(|binding| string(binding, "deploymentHash"))
                || string(result, "modelProfileHash")
                    != binding.and_then(|binding| string(binding, "modelProfileHash"))
                || string(result, "schemaHash")
                    != binding.and_then(|binding| string(binding, "schemaHash"))
                || string(result, "consentHash")
                    != binding.and_then(|binding| string(binding, "consentHash"))
                || string(result, "contextDecisionConsumptionHash")
                    != binding.and_then(|binding| string(binding, "contextDecisionConsumptionHash"))
            {
                push_once(errors, "BMAD_MODEL_LENS_BINDING_MISMATCH");
            }
        }
    }
}

/// Performs handwritten semantic checks after strict parsing and structural validation.
#[must_use]
pub fn validate_bmad_semantics(value: &Value, descriptor: Option<&Value>) -> Vec<String> {
    let mut errors = Vec::new();
    match string(value, "schemaVersion") {
        Some("sapphirus.bmad-package-descriptor.v1") => validate_descriptor(value, &mut errors),
        Some("sapphirus.bmad-capability-catalog.v1") => {
            validate_catalog(value, descriptor, &mut errors);
        }
        Some("sapphirus.bmad-method-session.v1") => validate_method(value, descriptor, &mut errors),
        Some(
            "sapphirus.bmad-builder-authoring.v1"
            | "sapphirus.bmad-builder-revision.v1"
            | "sapphirus.bmad-builder-analysis.v1",
        ) => validate_builder(value, &mut errors),
        _ => {}
    }
    verify_hash(value, &mut errors);
    errors
}
