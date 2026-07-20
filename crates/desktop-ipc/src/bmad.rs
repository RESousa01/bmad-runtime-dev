use std::collections::BTreeSet;

use desktop_runtime::{
    BmadAgentMenuRecord, BmadAgentRecord, BmadAgentRoster, BmadCatalog, BmadCatalogAvailability,
    BmadEntrypointKind, BmadHelpAction, BmadInstalledSkillRecord, BmadLibraryProjectionScope,
    BmadLoadedPackage, BmadMenuTargetKind,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MAX_BMAD_LIBRARY_PROJECTION_BYTES: usize = 256 * 1024;
const MAX_SKILLS: usize = 64;
const MAX_HELP_ACTIONS: usize = 64;
const MAX_AGENTS: usize = 16;
const MAX_MENUS_PER_AGENT: usize = 32;
const MAX_ACTIONS_PER_SKILL: usize = 16;
const MAX_EXPECTED_ARTIFACTS: usize = 16;
const MAX_IDENTIFIER_BYTES: usize = 256;
const MAX_DESCRIPTION_BYTES: usize = 2_048;
const MAX_ICON_BYTES: usize = 64;
pub(crate) const MAX_BMAD_CURSOR_BYTES: usize = 256;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct BmadLibrarySnapshotPayload {
    pub(crate) scope: BmadLibraryProjectionScope,
    pub(crate) cursor: Option<String>,
}

pub(crate) const MAX_PERSONA_MARKDOWN_BYTES: usize = 16_384;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct BmadPersonaViewPayload {
    pub(crate) agent_code: String,
}

/// One roster agent's sealed persona perspective for read-only renderer
/// display. Carries only repository-authored instruction text and its
/// hash — never source bodies, paths, or authority material.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadPersonaPerspectiveProjection {
    pub schema_version: String,
    pub agent_code: String,
    pub name: String,
    pub title: String,
    pub icon: String,
    pub instruction_markdown: String,
    pub instruction_hash: String,
}

/// Builds the bounded persona perspective projection.
///
/// # Errors
///
/// Returns [`BmadProjectionError::Unavailable`] when any field exceeds its
/// bound or the markdown is empty or oversized.
pub fn project_bmad_persona_perspective(
    agent_code: &str,
    name: &str,
    title: &str,
    icon: &str,
    instruction_markdown: &str,
    instruction_hash: &str,
) -> Result<BmadPersonaPerspectiveProjection, BmadProjectionError> {
    if instruction_markdown.is_empty()
        || instruction_markdown.len() > MAX_PERSONA_MARKDOWN_BYTES
        || !instruction_hash.starts_with("sha256:")
    {
        return Err(BmadProjectionError::Unavailable);
    }
    Ok(BmadPersonaPerspectiveProjection {
        schema_version: "sapphirus.bmad-persona-perspective.v1".to_owned(),
        agent_code: bounded_identifier(agent_code)?,
        name: bounded_text(name, MAX_IDENTIFIER_BYTES)?,
        title: bounded_text(title, MAX_IDENTIFIER_BYTES)?,
        icon: bounded_text(icon, MAX_ICON_BYTES)?,
        instruction_markdown: instruction_markdown.to_owned(),
        instruction_hash: instruction_hash.to_owned(),
    })
}

#[must_use]
pub(crate) fn valid_bmad_cursor(cursor: &str) -> bool {
    !cursor.is_empty()
        && cursor.len() <= MAX_BMAD_CURSOR_BYTES
        && cursor.bytes().all(|byte| matches!(byte, 0x21..=0x7e))
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadProjectionAvailability {
    Available,
    CapabilityDisabled,
    DependencyUnavailable,
    OrphanSkill,
    NetworkUnavailable,
    SourcePromptUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadProjectionBlockerCode {
    BmadCapabilityDisabled,
    BmadDependencyUnavailable,
    BmadHelpCatalogOrphan,
    BmadNetworkReferenceUnavailable,
    BmadSourcePromptUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadAgentMenuTargetProjection {
    SkillTarget,
    PromptReference,
}

impl BmadAgentMenuTargetProjection {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SkillTarget => "skill_target",
            Self::PromptReference => "prompt_reference",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadLibrarySourceKind {
    SealedFoundation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadLibrarySourceProjection {
    pub source_kind: BmadLibrarySourceKind,
    pub package_name: String,
    pub package_version: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadInstalledSkillProjection {
    pub module_code: String,
    pub skill_name: String,
    pub display_name: String,
    pub description: String,
    pub actions: Vec<String>,
    pub entrypoint_kind: BmadEntrypointKind,
    pub distribution_profile: String,
    pub install_profile: String,
    pub validation_profile: String,
    pub availability: BmadProjectionAvailability,
    pub blocker_codes: Vec<BmadProjectionBlockerCode>,
    pub hidden_from_help: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadHelpActionProjection {
    pub module_code: String,
    pub skill_name: String,
    pub action: Option<String>,
    pub display_name: String,
    pub menu_code: Option<String>,
    pub description: String,
    pub required_guidance: bool,
    pub expected_artifacts: Vec<String>,
    pub availability: BmadProjectionAvailability,
    pub blocker_codes: Vec<BmadProjectionBlockerCode>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadAgentMenuProjection {
    pub code: String,
    pub description: String,
    pub target_kind: BmadAgentMenuTargetProjection,
    pub display_label: String,
    pub availability: BmadProjectionAvailability,
    pub availability_reason: Option<BmadProjectionBlockerCode>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadAgentProjection {
    pub module_code: String,
    pub agent_code: String,
    pub name: String,
    pub title: String,
    pub icon: String,
    pub team: String,
    pub description: String,
    pub availability: BmadProjectionAvailability,
    pub blocker_codes: Vec<BmadProjectionBlockerCode>,
    pub menus: Vec<BmadAgentMenuProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadLibrarySnapshotProjection {
    pub schema_version: String,
    pub scope: BmadLibraryProjectionScope,
    pub source: BmadLibrarySourceProjection,
    pub installed_skills: Vec<BmadInstalledSkillProjection>,
    pub help_actions: Vec<BmadHelpActionProjection>,
    pub method_agents: Vec<BmadAgentProjection>,
    pub builder_packages: Vec<BmadBuilderPackageProjection>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadBuilderPackageKind {
    Agent,
    Workflow,
}

/// Display-only projection of an installed Builder package. Activation stays a
/// gated local decision (Note 14); this surface can never claim authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadBuilderPackageProjection {
    pub package_name: String,
    pub package_version: String,
    pub package_kind: BmadBuilderPackageKind,
    pub display_name: String,
    pub activation_state: String,
    pub resource_count: u32,
    pub descriptor_digest: String,
    pub blocker_codes: Vec<String>,
}

const MAX_BUILDER_PACKAGES: usize = 8;

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum BmadProjectionError {
    #[error("the BMAD projection is unavailable")]
    Unavailable,
    #[error("the BMAD projection cursor is stale")]
    Gap,
}

impl BmadProjectionError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Unavailable => "bmad_projection_unavailable",
            Self::Gap => "bmad_projection_gap",
        }
    }
}

/// Produces the bounded, display-only Method-library projection.
///
/// # Errors
///
/// Returns [`BmadProjectionError::Gap`] for any v1 continuation cursor and
/// [`BmadProjectionError::Unavailable`] if trusted records exceed the closed
/// projection limits, contain unsafe display text, are not bound to the same
/// catalog, or serialize beyond the response cap.
pub fn project_bmad_library(
    package: &BmadLoadedPackage,
    catalog: &BmadCatalog,
    roster: &BmadAgentRoster,
    scope: BmadLibraryProjectionScope,
    cursor: Option<&str>,
) -> Result<BmadLibrarySnapshotProjection, BmadProjectionError> {
    project_bmad_library_with_activations(package, catalog, roster, scope, cursor, &[], Vec::new())
}

/// Produces the bounded Method-library projection with an explicit
/// product-owned availability overlay. The sealed catalog remains immutable;
/// overlays can only promote exact structurally eligible installed skills.
///
/// # Errors
///
/// Returns [`BmadProjectionError::Unavailable`] for duplicate, unknown, or
/// structurally ineligible activations in addition to the baseline projection
/// failures.
pub fn project_bmad_library_with_activations(
    package: &BmadLoadedPackage,
    catalog: &BmadCatalog,
    roster: &BmadAgentRoster,
    scope: BmadLibraryProjectionScope,
    cursor: Option<&str>,
    activations: &[(&str, &str)],
    builder_packages: Vec<BmadBuilderPackageProjection>,
) -> Result<BmadLibrarySnapshotProjection, BmadProjectionError> {
    if cursor.is_some() {
        return Err(BmadProjectionError::Gap);
    }
    if catalog.capability_catalog_hash() != roster.capability_catalog_hash()
        || catalog.installed_skills.len() > MAX_SKILLS
        || catalog.help_actions.len() > MAX_HELP_ACTIONS
        || roster.agents.len() > MAX_AGENTS
        || builder_packages.len() > MAX_BUILDER_PACKAGES
        || builder_packages.iter().any(|builder| {
            builder.activation_state != "installed_inactive"
                || builder.package_name.len() > MAX_IDENTIFIER_BYTES
                || builder.package_version.len() > MAX_IDENTIFIER_BYTES
                || builder.display_name.len() > MAX_IDENTIFIER_BYTES
                || builder.blocker_codes != ["builder_engine_gated"]
        })
    {
        return Err(BmadProjectionError::Unavailable);
    }
    let activated = validated_activations(catalog, activations)?;
    let projection = BmadLibrarySnapshotProjection {
        schema_version: "bmad-library-snapshot.v2".to_owned(),
        scope,
        source: BmadLibrarySourceProjection {
            source_kind: BmadLibrarySourceKind::SealedFoundation,
            package_name: bounded_text(&package.package_name, MAX_IDENTIFIER_BYTES)?,
            package_version: bounded_text(&package.package_version, MAX_IDENTIFIER_BYTES)?,
        },
        installed_skills: catalog
            .installed_skills
            .iter()
            .map(|skill| {
                project_installed_skill(
                    skill,
                    activated.contains(&(skill.module_code.as_str(), skill.skill_name.as_str())),
                )
            })
            .collect::<Result<_, _>>()?,
        help_actions: catalog
            .help_actions
            .iter()
            .filter(|action| action.skill_name != "_meta")
            .map(|action| {
                project_help_action(
                    action,
                    activated.contains(&(action.module_code.as_str(), action.skill_name.as_str())),
                )
            })
            .collect::<Result<_, _>>()?,
        method_agents: roster
            .agents
            .iter()
            .map(project_agent)
            .collect::<Result<_, _>>()?,
        builder_packages,
        next_cursor: None,
    };
    if serde_json::to_vec(&projection)
        .map_err(|_| BmadProjectionError::Unavailable)?
        .len()
        > MAX_BMAD_LIBRARY_PROJECTION_BYTES
    {
        return Err(BmadProjectionError::Unavailable);
    }
    Ok(projection)
}

fn validated_activations<'a>(
    catalog: &BmadCatalog,
    activations: &[(&'a str, &'a str)],
) -> Result<BTreeSet<(&'a str, &'a str)>, BmadProjectionError> {
    let mut activated = BTreeSet::new();
    for &(module_code, skill_name) in activations {
        let eligible = catalog.installed_skills.iter().any(|skill| {
            skill.module_code == module_code
                && skill.skill_name == skill_name
                && skill.structurally_eligible
        });
        if !eligible || !activated.insert((module_code, skill_name)) {
            return Err(BmadProjectionError::Unavailable);
        }
    }
    Ok(activated)
}

fn project_installed_skill(
    skill: &BmadInstalledSkillRecord,
    activated: bool,
) -> Result<BmadInstalledSkillProjection, BmadProjectionError> {
    if skill.actions.len() > MAX_ACTIONS_PER_SKILL {
        return Err(BmadProjectionError::Unavailable);
    }
    let availability = if skill.capability_enabled || activated {
        BmadCatalogAvailability::Available
    } else {
        BmadCatalogAvailability::CapabilityDisabled
    };
    Ok(BmadInstalledSkillProjection {
        module_code: bounded_identifier(&skill.module_code)?,
        skill_name: bounded_identifier(&skill.skill_name)?,
        display_name: bounded_text(&skill.display_name, MAX_IDENTIFIER_BYTES)?,
        description: bounded_text(&skill.description, MAX_DESCRIPTION_BYTES)?,
        actions: skill
            .actions
            .iter()
            .map(|action| bounded_identifier(action))
            .collect::<Result<_, _>>()?,
        entrypoint_kind: skill.entrypoint_kind,
        distribution_profile: bounded_identifier(&skill.distribution_profile)?,
        install_profile: bounded_identifier(&skill.install_profile)?,
        validation_profile: bounded_identifier(&skill.validation_profile)?,
        availability: project_availability(availability),
        blocker_codes: blocker_codes(availability),
        hidden_from_help: skill.hidden_from_help,
    })
}

fn project_help_action(
    action: &BmadHelpAction,
    activated: bool,
) -> Result<BmadHelpActionProjection, BmadProjectionError> {
    if action.expected_outputs.len() > MAX_EXPECTED_ARTIFACTS {
        return Err(BmadProjectionError::Unavailable);
    }
    Ok(BmadHelpActionProjection {
        module_code: bounded_identifier(&action.module_code)?,
        skill_name: bounded_identifier(&action.skill_name)?,
        action: action
            .action
            .as_deref()
            .map(bounded_identifier)
            .transpose()?,
        display_name: bounded_text(&action.display_name, MAX_IDENTIFIER_BYTES)?,
        menu_code: action
            .menu_code
            .as_deref()
            .map(bounded_identifier)
            .transpose()?,
        description: bounded_text(&action.description, MAX_DESCRIPTION_BYTES)?,
        required_guidance: action.required,
        expected_artifacts: action
            .expected_outputs
            .iter()
            .map(|artifact| bounded_text(artifact, MAX_IDENTIFIER_BYTES))
            .collect::<Result<_, _>>()?,
        availability: project_availability(activated_availability(action.availability, activated)),
        blocker_codes: blocker_codes(activated_availability(action.availability, activated)),
    })
}

const fn activated_availability(
    availability: BmadCatalogAvailability,
    activated: bool,
) -> BmadCatalogAvailability {
    if activated && matches!(availability, BmadCatalogAvailability::CapabilityDisabled) {
        BmadCatalogAvailability::Available
    } else {
        availability
    }
}

fn project_agent(agent: &BmadAgentRecord) -> Result<BmadAgentProjection, BmadProjectionError> {
    if agent.menus.len() > MAX_MENUS_PER_AGENT {
        return Err(BmadProjectionError::Unavailable);
    }
    let menus = agent
        .menus
        .iter()
        .map(project_agent_menu)
        .collect::<Result<Vec<_>, _>>()?;
    let blockers = agent
        .menus
        .iter()
        .filter_map(|menu| blocker_code(menu.availability))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let availability = if agent.available {
        BmadProjectionAvailability::Available
    } else if agent
        .menus
        .iter()
        .any(|menu| menu.availability == BmadCatalogAvailability::CapabilityDisabled)
    {
        BmadProjectionAvailability::CapabilityDisabled
    } else {
        BmadProjectionAvailability::DependencyUnavailable
    };
    Ok(BmadAgentProjection {
        module_code: bounded_identifier(&agent.module_code)?,
        agent_code: bounded_identifier(&agent.agent_code)?,
        name: bounded_text(&agent.display_name, MAX_IDENTIFIER_BYTES)?,
        title: bounded_text(&agent.title, MAX_IDENTIFIER_BYTES)?,
        icon: bounded_text(&agent.icon, MAX_ICON_BYTES)?,
        team: bounded_identifier(&agent.team)?,
        description: bounded_text(&agent.description, MAX_DESCRIPTION_BYTES)?,
        availability,
        blocker_codes: blockers,
        menus,
    })
}

fn project_agent_menu(
    menu: &BmadAgentMenuRecord,
) -> Result<BmadAgentMenuProjection, BmadProjectionError> {
    Ok(BmadAgentMenuProjection {
        code: bounded_identifier(&menu.code)?,
        description: bounded_text(&menu.description, MAX_DESCRIPTION_BYTES)?,
        target_kind: match menu.target_kind {
            BmadMenuTargetKind::SkillTarget => BmadAgentMenuTargetProjection::SkillTarget,
            BmadMenuTargetKind::PromptReference => BmadAgentMenuTargetProjection::PromptReference,
        },
        display_label: bounded_text(&menu.display_label, MAX_IDENTIFIER_BYTES)?,
        availability: project_availability(menu.availability),
        availability_reason: blocker_code(menu.availability),
    })
}

const fn project_availability(value: BmadCatalogAvailability) -> BmadProjectionAvailability {
    match value {
        BmadCatalogAvailability::Available => BmadProjectionAvailability::Available,
        BmadCatalogAvailability::CapabilityDisabled => {
            BmadProjectionAvailability::CapabilityDisabled
        }
        BmadCatalogAvailability::DependencyUnavailable => {
            BmadProjectionAvailability::DependencyUnavailable
        }
        BmadCatalogAvailability::OrphanSkill => BmadProjectionAvailability::OrphanSkill,
        BmadCatalogAvailability::NetworkUnavailable => {
            BmadProjectionAvailability::NetworkUnavailable
        }
        BmadCatalogAvailability::SourcePromptUnavailable => {
            BmadProjectionAvailability::SourcePromptUnavailable
        }
    }
}

fn blocker_codes(value: BmadCatalogAvailability) -> Vec<BmadProjectionBlockerCode> {
    blocker_code(value).into_iter().collect()
}

const fn blocker_code(value: BmadCatalogAvailability) -> Option<BmadProjectionBlockerCode> {
    match value {
        BmadCatalogAvailability::Available => None,
        BmadCatalogAvailability::CapabilityDisabled => {
            Some(BmadProjectionBlockerCode::BmadCapabilityDisabled)
        }
        BmadCatalogAvailability::DependencyUnavailable => {
            Some(BmadProjectionBlockerCode::BmadDependencyUnavailable)
        }
        BmadCatalogAvailability::OrphanSkill => {
            Some(BmadProjectionBlockerCode::BmadHelpCatalogOrphan)
        }
        BmadCatalogAvailability::NetworkUnavailable => {
            Some(BmadProjectionBlockerCode::BmadNetworkReferenceUnavailable)
        }
        BmadCatalogAvailability::SourcePromptUnavailable => {
            Some(BmadProjectionBlockerCode::BmadSourcePromptUnavailable)
        }
    }
}

fn bounded_identifier(value: &str) -> Result<String, BmadProjectionError> {
    if value.is_empty()
        || value.len() > MAX_IDENTIFIER_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(BmadProjectionError::Unavailable);
    }
    Ok(value.to_owned())
}

fn bounded_text(value: &str, max_bytes: usize) -> Result<String, BmadProjectionError> {
    if value.len() > max_bytes
        || value.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '\u{061c}'
                        | '\u{200e}'
                        | '\u{200f}'
                        | '\u{202a}'..='\u{202e}'
                        | '\u{2066}'..='\u{2069}'
                )
        })
    {
        return Err(BmadProjectionError::Unavailable);
    }
    Ok(value.to_owned())
}
