use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::{Map, Value};

use crate::{canonical_hash, generated_contracts, ContractId, RelativeWorkspacePath, Sha256Digest};

use super::{BmadEntrypointKind, BmadKernelError, BmadKernelErrorCode, BmadLoadedPackage};

const HELP_HEADER: [&str; 13] = [
    "module",
    "skill",
    "display-name",
    "menu-code",
    "description",
    "action",
    "args",
    "phase",
    "preceded-by",
    "followed-by",
    "required",
    "output-location",
    "outputs",
];
const MAX_CATALOG_BYTES: usize = 1_048_576;
const MAX_CATALOG_ROWS: usize = 4_096;
const MAX_CELL_BYTES: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadCatalogAvailability {
    Available,
    CapabilityDisabled,
    DependencyUnavailable,
    OrphanSkill,
    NetworkUnavailable,
    SourcePromptUnavailable,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct BmadHelpActionKey {
    pub package_version_id: ContractId,
    pub module_code: String,
    pub skill_name: String,
    pub action: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BmadInstalledSkillRecord {
    pub module_code: String,
    pub skill_name: String,
    pub display_name: String,
    pub description: String,
    pub entrypoint_kind: BmadEntrypointKind,
    pub actions: Vec<String>,
    pub distribution_profile: String,
    pub install_profile: String,
    pub validation_profile: String,
    #[serde(skip_serializing)]
    pub execution_profile_hash: Sha256Digest,
    pub capability_enabled: bool,
    pub structurally_eligible: bool,
    pub hidden_from_help: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct BmadHelpAction {
    pub key: BmadHelpActionKey,
    pub module_code: String,
    pub skill_name: String,
    pub display_name: String,
    pub menu_code: Option<String>,
    pub description: String,
    pub action: Option<String>,
    #[serde(skip_serializing)]
    pub args: Option<String>,
    #[serde(skip_serializing)]
    pub phase: Option<String>,
    #[serde(skip_serializing)]
    pub preceded_by: Option<String>,
    #[serde(skip_serializing)]
    pub followed_by: Option<String>,
    pub required: bool,
    pub output_locations: Vec<String>,
    pub expected_outputs: Vec<String>,
    pub availability: BmadCatalogAvailability,
    pub network_reference_present: bool,
    pub source_ordinal: u64,
    #[serde(skip_serializing)]
    pub source_row_hash: Sha256Digest,
    #[serde(skip_serializing)]
    source_row: [String; 13],
}

impl BmadHelpAction {
    #[must_use]
    pub fn raw_source_row(&self) -> &[String; 13] {
        &self.source_row
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct BmadCatalog {
    pub installed_skills: Vec<BmadInstalledSkillRecord>,
    pub help_actions: Vec<BmadHelpAction>,
}

#[derive(Clone, Debug)]
pub struct BmadHelpCatalogSource {
    module_code: String,
    contents: String,
}

impl BmadHelpCatalogSource {
    /// Creates one bounded, module-scoped help catalog source.
    ///
    /// # Errors
    ///
    /// Returns [`BmadKernelErrorCode::HelpCatalogInvalid`] for an invalid module
    /// code, non-canonical controls, or oversized bytes.
    pub fn new(
        module_code: impl Into<String>,
        contents: impl Into<String>,
    ) -> Result<Self, BmadKernelError> {
        let module_code = module_code.into();
        let contents = contents.into();
        if !valid_slug(&module_code, 64)
            || contents.len() > MAX_CATALOG_BYTES
            || contents.contains('\0')
            || contents.chars().any(is_directional_control)
        {
            return Err(BmadKernelErrorCode::HelpCatalogInvalid.into());
        }
        Ok(Self {
            module_code,
            contents,
        })
    }

    /// Creates a bounded catalog source from normalized raw 13-column rows.
    ///
    /// This is used by the sealed foundation adapter so production does not
    /// need source-tree CSV bytes. The raw row values remain data; the same CSV
    /// parser and semantic checks used for reviewed sources still run.
    ///
    /// # Errors
    ///
    /// Returns [`BmadKernelErrorCode::HelpCatalogInvalid`] when the row count,
    /// column count, encoded size, or a field violates catalog limits.
    pub fn from_rows(
        module_code: impl Into<String>,
        rows: &[Vec<String>],
    ) -> Result<Self, BmadKernelError> {
        if rows.len() > MAX_CATALOG_ROWS || rows.iter().any(|row| row.len() != HELP_HEADER.len()) {
            return Err(BmadKernelErrorCode::HelpCatalogInvalid.into());
        }
        let mut contents = HELP_HEADER.join(",");
        contents.push('\n');
        for row in rows {
            for (index, field) in row.iter().enumerate() {
                if index > 0 {
                    contents.push(',');
                }
                encode_csv_field(field, &mut contents);
            }
            contents.push('\n');
        }
        Self::new(module_code, contents)
    }
}

fn encode_csv_field(field: &str, output: &mut String) {
    if field.contains([',', '"', '\r', '\n']) {
        output.push('"');
        for character in field.chars() {
            if character == '"' {
                output.push('"');
            }
            output.push(character);
        }
        output.push('"');
    } else {
        output.push_str(field);
    }
}

pub struct BmadCatalogBuilder;

impl BmadCatalogBuilder {
    /// Builds separate installed-skill and help-action projections.
    ///
    /// # Errors
    ///
    /// Fails closed for malformed CSV, duplicate identities, same-module menu
    /// aliases, or authority-bearing text.
    pub fn build(
        package: &BmadLoadedPackage,
        sources: &[BmadHelpCatalogSource],
    ) -> Result<BmadCatalog, BmadKernelError> {
        let mut installed_skills = package
            .skills
            .iter()
            .map(|skill| BmadInstalledSkillRecord {
                module_code: skill.module_code.clone(),
                skill_name: skill.skill_name.clone(),
                display_name: skill.display_name.clone(),
                description: skill.description.clone(),
                entrypoint_kind: skill.entrypoint_kind,
                actions: skill.actions.clone(),
                distribution_profile: skill.distribution_profile.clone(),
                install_profile: skill.install_profile.clone(),
                validation_profile: skill.validation_profile.clone(),
                execution_profile_hash: skill.execution_profile_hash,
                capability_enabled: skill.capability_enabled,
                structurally_eligible: skill.structurally_eligible,
                hidden_from_help: true,
            })
            .collect::<Vec<_>>();
        installed_skills.sort_by(|left, right| {
            (&left.module_code, &left.skill_name).cmp(&(&right.module_code, &right.skill_name))
        });

        let mut source_modules = BTreeSet::new();
        let mut identities = BTreeSet::new();
        let mut menu_codes = BTreeSet::new();
        let mut help_actions = Vec::new();
        for source in sources {
            if !source_modules.insert(source.module_code.clone()) {
                return Err(BmadKernelErrorCode::HelpCatalogInvalid.into());
            }
            for (source_ordinal, row) in parse_help_catalog(&source.contents)?
                .into_iter()
                .enumerate()
            {
                let source_ordinal = u64::try_from(source_ordinal)
                    .map_err(|_| BmadKernelErrorCode::HelpCatalogInvalid)?;
                let mut action = normalize_help_row(
                    &package.package_version_id,
                    &source.module_code,
                    source_ordinal,
                    row,
                )?;
                infer_single_action(&installed_skills, &mut action);
                if !identities.insert(action.key.clone()) {
                    return Err(BmadKernelErrorCode::HelpCatalogInvalid.into());
                }
                if action.menu_code.as_ref().is_some_and(|menu_code| {
                    !menu_codes.insert((source.module_code.clone(), menu_code.clone()))
                }) {
                    return Err(BmadKernelErrorCode::MenuCodeAmbiguous.into());
                }
                if action.skill_name != "_meta" {
                    if let Some(installed) = installed_skills.iter_mut().find(|installed| {
                        installed.module_code == action.module_code
                            && installed.skill_name == action.skill_name
                    }) {
                        installed.hidden_from_help = false;
                    }
                }
                help_actions.push(action);
            }
        }

        apply_availability(&installed_skills, &mut help_actions);
        help_actions.sort_by(|left, right| left.key.cmp(&right.key));
        Ok(BmadCatalog {
            installed_skills,
            help_actions,
        })
    }
}

fn infer_single_action(installed: &[BmadInstalledSkillRecord], action: &mut BmadHelpAction) {
    if action.action.is_some() {
        return;
    }
    let Some(installed_action) = installed
        .iter()
        .find(|skill| {
            skill.module_code == action.module_code
                && skill.skill_name == action.skill_name
                && skill.actions.len() == 1
        })
        .and_then(|skill| skill.actions.first())
    else {
        return;
    };
    action.action = Some(installed_action.clone());
    action.key.action = Some(installed_action.clone());
}

fn parse_help_catalog(contents: &str) -> Result<Vec<Vec<String>>, BmadKernelError> {
    let rows = parse_csv(contents)?;
    if rows.is_empty()
        || rows.len() > MAX_CATALOG_ROWS + 1
        || rows[0].iter().map(String::as_str).ne(HELP_HEADER)
    {
        return Err(BmadKernelErrorCode::HelpCatalogInvalid.into());
    }
    let data = rows.into_iter().skip(1).collect::<Vec<_>>();
    if data.iter().any(|row| row.len() != HELP_HEADER.len()) {
        return Err(BmadKernelErrorCode::HelpCatalogInvalid.into());
    }
    Ok(data)
}

fn parse_csv(contents: &str) -> Result<Vec<Vec<String>>, BmadKernelError> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut chars = contents.chars().peekable();
    let mut quoted = false;
    let mut closed_quote = false;
    while let Some(character) = chars.next() {
        if quoted {
            if character == '"' {
                if chars.peek() == Some(&'"') {
                    let _ = chars.next();
                    field.push('"');
                } else {
                    quoted = false;
                    closed_quote = true;
                }
            } else {
                field.push(character);
            }
            if field.len() > MAX_CELL_BYTES {
                return Err(BmadKernelErrorCode::HelpCatalogInvalid.into());
            }
            continue;
        }
        if closed_quote {
            match character {
                ',' => {
                    row.push(std::mem::take(&mut field));
                    closed_quote = false;
                }
                '\r' => {
                    if chars.peek() == Some(&'\n') {
                        let _ = chars.next();
                    }
                    finish_csv_row(&mut rows, &mut row, &mut field);
                    closed_quote = false;
                }
                '\n' => {
                    finish_csv_row(&mut rows, &mut row, &mut field);
                    closed_quote = false;
                }
                _ => return Err(BmadKernelErrorCode::HelpCatalogInvalid.into()),
            }
            continue;
        }
        match character {
            '"' if field.is_empty() => quoted = true,
            '"' => return Err(BmadKernelErrorCode::HelpCatalogInvalid.into()),
            ',' => row.push(std::mem::take(&mut field)),
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    let _ = chars.next();
                }
                finish_csv_row(&mut rows, &mut row, &mut field);
            }
            '\n' => finish_csv_row(&mut rows, &mut row, &mut field),
            _ => field.push(character),
        }
        if field.len() > MAX_CELL_BYTES {
            return Err(BmadKernelErrorCode::HelpCatalogInvalid.into());
        }
    }
    if quoted {
        return Err(BmadKernelErrorCode::HelpCatalogInvalid.into());
    }
    if !row.is_empty() || !field.is_empty() {
        finish_csv_row(&mut rows, &mut row, &mut field);
    }
    Ok(rows)
}

fn finish_csv_row(rows: &mut Vec<Vec<String>>, row: &mut Vec<String>, field: &mut String) {
    row.push(std::mem::take(field));
    rows.push(std::mem::take(row));
}

fn normalize_help_row(
    package_version_id: &ContractId,
    module_code: &str,
    source_ordinal: u64,
    row: Vec<String>,
) -> Result<BmadHelpAction, BmadKernelError> {
    if row
        .iter()
        .any(|cell| !valid_display_cell(cell) || contains_authority_directive(cell))
    {
        return Err(BmadKernelErrorCode::HelpCatalogInvalid.into());
    }
    let source_row: [String; 13] = row
        .try_into()
        .map_err(|_| BmadKernelErrorCode::HelpCatalogInvalid)?;
    let source_row_hash = canonical_hash("bmad-help-source-row", 1, &source_row)
        .map_err(|_| BmadKernelErrorCode::HelpCatalogInvalid)?;
    let [module, skill_name, display_name, menu_code, description, action, args, phase, preceded_by, followed_by, required, output_location, outputs] =
        source_row.clone();
    if module.is_empty()
        || (!valid_slug(&skill_name, 128) && skill_name != "_meta")
        || (!action.is_empty() && !valid_slug(&action, 128))
        || (!menu_code.is_empty() && !valid_menu_code(&menu_code))
        || !valid_source_reference(&args, SourceReferenceKind::Args)
        || !valid_source_reference(&phase, SourceReferenceKind::Phase)
        || !valid_source_reference(&preceded_by, SourceReferenceKind::Dependency)
        || !valid_source_reference(&followed_by, SourceReferenceKind::Dependency)
    {
        return Err(BmadKernelErrorCode::HelpCatalogInvalid.into());
    }
    let required = match required.as_str() {
        "true" => true,
        "false" => false,
        _ => return Err(BmadKernelErrorCode::HelpCatalogInvalid.into()),
    };
    let network_reference_present = skill_name == "_meta" && !output_location.is_empty();
    if (!network_reference_present
        && !valid_source_reference(&output_location, SourceReferenceKind::OutputHint))
        || !valid_source_reference(&outputs, SourceReferenceKind::OutputHint)
    {
        return Err(BmadKernelErrorCode::HelpCatalogInvalid.into());
    }
    let output_locations = if network_reference_present {
        Vec::new()
    } else {
        split_alternatives(&output_location)
    };
    let expected_outputs = split_alternatives(&outputs);
    let action = nonempty(action);
    let key = BmadHelpActionKey {
        package_version_id: package_version_id.clone(),
        module_code: module_code.to_owned(),
        skill_name: skill_name.clone(),
        action: action.clone(),
    };
    Ok(BmadHelpAction {
        key,
        module_code: module_code.to_owned(),
        skill_name,
        display_name,
        menu_code: nonempty(menu_code),
        description,
        action,
        args: nonempty(args),
        phase: nonempty(phase),
        preceded_by: nonempty(preceded_by),
        followed_by: nonempty(followed_by),
        required,
        output_locations,
        expected_outputs,
        availability: BmadCatalogAvailability::DependencyUnavailable,
        network_reference_present,
        source_ordinal,
        source_row_hash,
        source_row,
    })
}

fn apply_availability(installed: &[BmadInstalledSkillRecord], actions: &mut [BmadHelpAction]) {
    let installed_by_key = installed
        .iter()
        .map(|skill| {
            (
                (skill.module_code.as_str(), skill.skill_name.as_str()),
                skill,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let action_skills = installed
        .iter()
        .map(|skill| skill.skill_name.clone())
        .chain(
            actions
                .iter()
                .filter(|action| action.skill_name != "_meta")
                .map(|action| action.skill_name.clone()),
        )
        .collect::<BTreeSet<_>>();
    for action in actions {
        action.availability = if action.skill_name == "_meta" {
            BmadCatalogAvailability::NetworkUnavailable
        } else if !dependency_available(action, &action_skills)
            || action
                .phase
                .as_ref()
                .is_some_and(|phase| contains_authority_directive(phase))
        {
            BmadCatalogAvailability::DependencyUnavailable
        } else if let Some(skill) =
            installed_by_key.get(&(action.module_code.as_str(), action.skill_name.as_str()))
        {
            if skill.capability_enabled {
                BmadCatalogAvailability::Available
            } else {
                BmadCatalogAvailability::CapabilityDisabled
            }
        } else {
            BmadCatalogAvailability::OrphanSkill
        };
    }
}

fn dependency_available(action: &BmadHelpAction, action_skills: &BTreeSet<String>) -> bool {
    action.preceded_by.as_ref().is_none_or(|dependency| {
        dependency
            .split('|')
            .map(str::trim)
            .filter(|skill| !skill.is_empty())
            .map(|reference| {
                reference
                    .split_once(':')
                    .map_or(reference, |(skill, _)| skill)
            })
            .all(|skill| action_skills.contains(skill))
    })
}

fn split_alternatives(value: &str) -> Vec<String> {
    value
        .split('|')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect()
}

fn nonempty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

fn valid_display_cell(value: &str) -> bool {
    value.len() <= MAX_CELL_BYTES
        && !value.contains('\0')
        && !value.chars().any(is_directional_control)
        && !value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\t'))
}

fn valid_slug(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || (index > 0 && byte == b'-')
        })
}

fn valid_menu_code(value: &str) -> bool {
    value.len() <= 16
        && value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'-')
}

fn contains_authority_directive(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("airlock")
        || lower.contains("grant permission")
        || lower.contains("execute command")
        || lower.contains("<script")
}

#[derive(Clone, Copy)]
enum SourceReferenceKind {
    Args,
    Phase,
    Dependency,
    OutputHint,
}

fn valid_source_reference(value: &str, kind: SourceReferenceKind) -> bool {
    if value.is_empty() {
        return true;
    }
    if value.contains(['\0', '\\', '<', '>'])
        || value.contains("://")
        || value.starts_with('/')
        || value.split('/').any(|segment| segment == "..")
    {
        return false;
    }
    match kind {
        SourceReferenceKind::Args => value.len() <= 512,
        SourceReferenceKind::Phase => value.len() <= 128,
        SourceReferenceKind::Dependency => value.len() <= 1_024,
        SourceReferenceKind::OutputHint => value.len() <= 2_048,
    }
}

fn is_directional_control(character: char) -> bool {
    matches!(
        character,
        '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadMenuTargetKind {
    SkillTarget,
    PromptReference,
}

#[derive(Clone, Debug)]
struct BmadAgentMenuSource {
    code: String,
    description: String,
    target: Value,
}

#[derive(Clone, Debug)]
pub struct BmadAgentSource {
    module_code: String,
    agent_code: String,
    display_name: String,
    title: String,
    icon: String,
    team: String,
    description: String,
    module_source_hash: Sha256Digest,
    entrypoint_hash: Sha256Digest,
    customization_hash: Sha256Digest,
    persona_graph_hash: Sha256Digest,
    source_member_ids: Vec<String>,
    menus: Vec<BmadAgentMenuSource>,
}

impl BmadAgentSource {
    /// Parses bounded inert agent display/menu data; target semantics are
    /// validated later against host-owned catalog and reviewed references.
    ///
    /// # Errors
    ///
    /// Returns [`BmadKernelErrorCode::AgentMenuTargetInvalid`] for malformed or
    /// authority-bearing roster data.
    pub fn from_value(value: &Value) -> Result<Self, BmadKernelError> {
        let object = value
            .as_object()
            .ok_or(BmadKernelErrorCode::AgentMenuTargetInvalid)?;
        exact_keys(
            object,
            &[
                "moduleCode",
                "agentCode",
                "displayName",
                "title",
                "icon",
                "team",
                "description",
                "moduleSourceHash",
                "entrypointHash",
                "customizationHash",
                "personaGraphHash",
                "sourceMemberIds",
                "menus",
            ],
        )?;
        let module_code = map_string(object, "moduleCode")?.to_owned();
        let agent_code = map_string(object, "agentCode")?.to_owned();
        let display_name = map_string(object, "displayName")?.to_owned();
        let title = map_string(object, "title")?.to_owned();
        let icon = map_string(object, "icon")?.to_owned();
        let team = map_string(object, "team")?.to_owned();
        let description = map_string(object, "description")?.to_owned();
        if !valid_slug(&module_code, 64)
            || !valid_slug(&agent_code, 128)
            || !valid_display_cell(&display_name)
            || display_name.is_empty()
            || !valid_display_cell(&title)
            || title.is_empty()
            || icon.is_empty()
            || icon.len() > 64
            || !valid_display_cell(&icon)
            || !valid_slug(&team, 128)
            || description.is_empty()
            || description.len() > 4_096
            || !valid_display_cell(&description)
            || [&display_name, &title, &icon, &description]
                .into_iter()
                .any(|value| contains_authority_directive(value))
        {
            return Err(BmadKernelErrorCode::AgentMenuTargetInvalid.into());
        }
        let module_source_hash = map_digest(object, "moduleSourceHash")?;
        let entrypoint_hash = map_digest(object, "entrypointHash")?;
        let customization_hash = map_digest(object, "customizationHash")?;
        let persona_graph_hash = map_digest(object, "personaGraphHash")?;
        let source_member_ids = object
            .get("sourceMemberIds")
            .and_then(Value::as_array)
            .ok_or(BmadKernelErrorCode::AgentMenuTargetInvalid)?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .filter(|id| valid_source_member_id(id))
                    .map(str::to_owned)
                    .ok_or_else(|| BmadKernelErrorCode::AgentMenuTargetInvalid.into())
            })
            .collect::<Result<Vec<_>, BmadKernelError>>()?;
        if source_member_ids.is_empty() || source_member_ids.len() > 32 {
            return Err(BmadKernelErrorCode::AgentMenuTargetInvalid.into());
        }
        if source_member_ids.iter().collect::<BTreeSet<_>>().len() != source_member_ids.len() {
            return Err(BmadKernelErrorCode::AgentMenuTargetInvalid.into());
        }
        let menu_values = object
            .get("menus")
            .and_then(Value::as_array)
            .ok_or(BmadKernelErrorCode::AgentMenuTargetInvalid)?;
        if menu_values.len() > 64 {
            return Err(BmadKernelErrorCode::AgentMenuTargetInvalid.into());
        }
        let menus = menu_values
            .iter()
            .map(parse_agent_menu_source)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            module_code,
            agent_code,
            display_name,
            title,
            icon,
            team,
            description,
            module_source_hash,
            entrypoint_hash,
            customization_hash,
            persona_graph_hash,
            source_member_ids,
            menus,
        })
    }
}

fn parse_agent_menu_source(value: &Value) -> Result<BmadAgentMenuSource, BmadKernelError> {
    let object = value
        .as_object()
        .ok_or(BmadKernelErrorCode::AgentMenuTargetInvalid)?;
    exact_keys(object, &["code", "description", "target"])?;
    let code = map_string(object, "code")?.to_owned();
    let description = map_string(object, "description")?.to_owned();
    let target = object
        .get("target")
        .filter(|target| target.is_object())
        .ok_or(BmadKernelErrorCode::AgentMenuTargetInvalid)?
        .clone();
    if !valid_menu_code(&code)
        || description.is_empty()
        || !valid_display_cell(&description)
        || contains_authority_directive(&description)
    {
        return Err(BmadKernelErrorCode::AgentMenuTargetInvalid.into());
    }
    Ok(BmadAgentMenuSource {
        code,
        description,
        target,
    })
}

#[derive(Clone, Debug)]
pub struct BmadReviewedPromptReference {
    owner_module_code: String,
    owner_agent_code: String,
    id: String,
    label: RelativeWorkspacePath,
    hash: Sha256Digest,
}

impl BmadReviewedPromptReference {
    /// Binds a source-local prompt member label to reviewed bytes.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid IDs or member labels.
    pub fn new(
        owner_module_code: impl Into<String>,
        owner_agent_code: impl Into<String>,
        member_id: impl Into<String>,
        member_label: impl Into<String>,
        member_hash: Sha256Digest,
    ) -> Result<Self, BmadKernelError> {
        let owner_module_code = owner_module_code.into();
        let owner_agent_code = owner_agent_code.into();
        let member_id = member_id.into();
        if !valid_slug(&owner_module_code, 64)
            || !valid_slug(&owner_agent_code, 128)
            || !valid_source_member_id(&member_id)
        {
            return Err(BmadKernelErrorCode::AgentMenuTargetInvalid.into());
        }
        let member_label = member_label.into();
        if member_label.chars().any(is_directional_control) {
            return Err(BmadKernelErrorCode::AgentMenuTargetInvalid.into());
        }
        let member_label = RelativeWorkspacePath::new(member_label)
            .map_err(|_| BmadKernelErrorCode::AgentMenuTargetInvalid)?;
        Ok(Self {
            owner_module_code,
            owner_agent_code,
            id: member_id,
            label: member_label,
            hash: member_hash,
        })
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct BmadAgentMenuRecord {
    pub code: String,
    pub description: String,
    pub target_kind: BmadMenuTargetKind,
    pub display_label: String,
    pub availability: BmadCatalogAvailability,
}

#[derive(Clone, Debug, Serialize)]
pub struct BmadAgentRecord {
    pub module_code: String,
    pub agent_code: String,
    pub display_name: String,
    pub title: String,
    pub icon: String,
    pub team: String,
    pub description: String,
    pub available: bool,
    pub source_evidence_count: usize,
    pub menus: Vec<BmadAgentMenuRecord>,
    #[serde(skip_serializing)]
    record_hash: Sha256Digest,
}

impl BmadAgentRecord {
    #[must_use]
    pub const fn record_hash(&self) -> Sha256Digest {
        self.record_hash
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct BmadAgentRoster {
    pub agents: Vec<BmadAgentRecord>,
    #[serde(skip_serializing)]
    roster_hash: Sha256Digest,
}

impl BmadAgentRoster {
    #[must_use]
    pub const fn roster_hash(&self) -> Sha256Digest {
        self.roster_hash
    }

    /// Loads the sealed normalized roster into display-only native records.
    ///
    /// The generated contract closes the source shape. This adapter then
    /// verifies the package binding and every roster/agent graph self-hash,
    /// resolves availability exclusively from the native catalog, and drops
    /// source-member labels and authority-only hashes.
    ///
    /// # Errors
    ///
    /// Returns [`BmadKernelErrorCode::AgentMenuTargetInvalid`] for a malformed,
    /// transplanted, non-canonical, or self-hash-invalid roster.
    pub fn load_normalized(
        bytes: &[u8],
        catalog: &BmadCatalog,
        expected_package_version_id: &ContractId,
    ) -> Result<Self, BmadKernelError> {
        if bytes.len() > MAX_CATALOG_BYTES {
            return Err(BmadKernelErrorCode::AgentMenuTargetInvalid.into());
        }
        let generated = serde_json::from_slice::<
            generated_contracts::BmadCapabilityCatalogBmadAgentRoster,
        >(bytes)
        .map_err(|_| BmadKernelErrorCode::AgentMenuTargetInvalid)?;
        drop(generated);
        let value: Value = serde_json::from_slice(bytes)
            .map_err(|_| BmadKernelErrorCode::AgentMenuTargetInvalid)?;
        let object = value
            .as_object()
            .ok_or(BmadKernelErrorCode::AgentMenuTargetInvalid)?;
        if map_string(object, "packageVersionId")? != expected_package_version_id.as_str() {
            return Err(BmadKernelErrorCode::AgentMenuTargetInvalid.into());
        }
        let values = object
            .get("agents")
            .and_then(Value::as_array)
            .ok_or(BmadKernelErrorCode::AgentMenuTargetInvalid)?;
        let mut identities = BTreeSet::new();
        let mut previous_identity: Option<(String, String)> = None;
        let mut record_hashes = Vec::with_capacity(values.len());
        let mut agents = Vec::with_capacity(values.len());
        for value in values {
            let agent = load_normalized_agent(value, catalog, expected_package_version_id)?;
            let identity = (agent.module_code.clone(), agent.agent_code.clone());
            if previous_identity
                .as_ref()
                .is_some_and(|previous| previous >= &identity)
                || !identities.insert(identity.clone())
            {
                return Err(BmadKernelErrorCode::AgentMenuTargetInvalid.into());
            }
            previous_identity = Some(identity);
            record_hashes.push(agent.record_hash());
            agents.push(agent);
        }
        let roster_hash = map_digest(object, "rosterHash")?;
        let computed = canonical_hash("bmad-agent-roster", 1, &record_hashes)
            .map_err(|_| BmadKernelErrorCode::AgentMenuTargetInvalid)?;
        if roster_hash != computed {
            return Err(BmadKernelErrorCode::AgentMenuTargetInvalid.into());
        }
        Ok(Self {
            agents,
            roster_hash,
        })
    }
}

fn load_normalized_agent(
    value: &Value,
    catalog: &BmadCatalog,
    expected_package_version_id: &ContractId,
) -> Result<BmadAgentRecord, BmadKernelError> {
    let agent = value
        .as_object()
        .ok_or(BmadKernelErrorCode::AgentMenuTargetInvalid)?;
    let menu_values = agent
        .get("menuItems")
        .and_then(Value::as_array)
        .ok_or(BmadKernelErrorCode::AgentMenuTargetInvalid)?;
    verify_normalized_agent_hashes(agent, menu_values)?;
    let menus = load_normalized_menus(menu_values, catalog, expected_package_version_id)?;
    let available = menus
        .iter()
        .any(|menu| menu.availability == BmadCatalogAvailability::Available);
    Ok(BmadAgentRecord {
        module_code: map_string(agent, "moduleCode")?.to_owned(),
        agent_code: map_string(agent, "agentCode")?.to_owned(),
        display_name: map_string(agent, "name")?.to_owned(),
        title: map_string(agent, "title")?.to_owned(),
        icon: map_string(agent, "icon")?.to_owned(),
        team: map_string(agent, "team")?.to_owned(),
        description: map_string(agent, "description")?.to_owned(),
        available,
        source_evidence_count: 0,
        menus,
        record_hash: map_digest(agent, "agentRecordHash")?,
    })
}

fn load_normalized_menus(
    values: &[Value],
    catalog: &BmadCatalog,
    expected_package_version_id: &ContractId,
) -> Result<Vec<BmadAgentMenuRecord>, BmadKernelError> {
    let mut menu_codes = BTreeSet::new();
    values
        .iter()
        .map(|value| {
            let menu = value
                .as_object()
                .ok_or(BmadKernelErrorCode::AgentMenuTargetInvalid)?;
            let code = map_string(menu, "menuCode")?.to_owned();
            if !menu_codes.insert(code.clone()) {
                return Err(BmadKernelErrorCode::AgentMenuTargetInvalid.into());
            }
            let target = menu
                .get("target")
                .and_then(Value::as_object)
                .ok_or(BmadKernelErrorCode::AgentMenuTargetInvalid)?;
            let (target_kind, availability) =
                load_normalized_menu_target(target, catalog, expected_package_version_id)?;
            Ok(BmadAgentMenuRecord {
                code,
                description: map_string(menu, "description")?.to_owned(),
                target_kind,
                display_label: map_string(menu, "displayName")?.to_owned(),
                availability,
            })
        })
        .collect()
}

fn load_normalized_menu_target(
    target: &Map<String, Value>,
    catalog: &BmadCatalog,
    expected_package_version_id: &ContractId,
) -> Result<(BmadMenuTargetKind, BmadCatalogAvailability), BmadKernelError> {
    match map_string(target, "targetKind")? {
        "skill_target" => Ok((
            BmadMenuTargetKind::SkillTarget,
            normalized_skill_availability(target, catalog, expected_package_version_id)?,
        )),
        "prompt_reference" => {
            if map_string(target, "availability")? != "unavailable_source_prompt" {
                return Err(BmadKernelErrorCode::AgentMenuTargetInvalid.into());
            }
            Ok((
                BmadMenuTargetKind::PromptReference,
                BmadCatalogAvailability::SourcePromptUnavailable,
            ))
        }
        _ => Err(BmadKernelErrorCode::AgentMenuTargetInvalid.into()),
    }
}

fn verify_normalized_agent_hashes(
    agent: &Map<String, Value>,
    menu_values: &[Value],
) -> Result<(), BmadKernelError> {
    let menu_hash = canonical_hash("bmad-agent-menu-graph", 1, &menu_values)
        .map_err(|_| BmadKernelErrorCode::AgentMenuTargetInvalid)?;
    if map_digest(agent, "menuGraphHash")? != menu_hash
        || map_string(agent, "personaCustomizationGraphHash")?
            != map_string(agent, "customizationSourceHash")?
    {
        return Err(BmadKernelErrorCode::AgentMenuTargetInvalid.into());
    }
    let record = serde_json::json!({
        "moduleCode": required_value(agent, "moduleCode")?,
        "agentCode": required_value(agent, "agentCode")?,
        "name": required_value(agent, "name")?,
        "title": required_value(agent, "title")?,
        "icon": required_value(agent, "icon")?,
        "team": required_value(agent, "team")?,
        "description": required_value(agent, "description")?,
        "personaSourceHash": required_value(agent, "personaSourceHash")?,
        "customizationSourceHash": required_value(agent, "customizationSourceHash")?,
        "menuItems": menu_values,
    });
    let record_hash = canonical_hash("bmad-agent-record", 1, &record)
        .map_err(|_| BmadKernelErrorCode::AgentMenuTargetInvalid)?;
    if map_digest(agent, "agentRecordHash")? != record_hash {
        return Err(BmadKernelErrorCode::AgentMenuTargetInvalid.into());
    }
    Ok(())
}

fn normalized_skill_availability(
    target: &Map<String, Value>,
    catalog: &BmadCatalog,
    expected_package_version_id: &ContractId,
) -> Result<BmadCatalogAvailability, BmadKernelError> {
    let key = target
        .get("capabilityKey")
        .and_then(Value::as_object)
        .ok_or(BmadKernelErrorCode::AgentMenuTargetInvalid)?;
    if map_string(key, "packageVersionId")? != expected_package_version_id.as_str() {
        return Err(BmadKernelErrorCode::AgentMenuTargetInvalid.into());
    }
    let module_code = map_string(key, "moduleCode")?;
    let skill_name = map_string(key, "skillName")?;
    let normalized_action = key
        .get("normalizedAction")
        .and_then(|value| {
            if value.is_null() {
                Some(None)
            } else {
                value.as_str().map(Some)
            }
        })
        .ok_or(BmadKernelErrorCode::AgentMenuTargetInvalid)?;
    let Some(skill) = catalog
        .installed_skills
        .iter()
        .find(|skill| skill.module_code == module_code && skill.skill_name == skill_name)
    else {
        return Ok(BmadCatalogAvailability::DependencyUnavailable);
    };
    let action_known = normalized_action.map_or_else(
        || skill.actions.is_empty(),
        |action| skill.actions.iter().any(|candidate| candidate == action),
    );
    if !action_known {
        return Err(BmadKernelErrorCode::AgentMenuTargetInvalid.into());
    }
    Ok(if skill.capability_enabled {
        BmadCatalogAvailability::Available
    } else {
        BmadCatalogAvailability::CapabilityDisabled
    })
}

fn required_value<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a Value, BmadKernelError> {
    object
        .get(field)
        .ok_or_else(|| BmadKernelErrorCode::AgentMenuTargetInvalid.into())
}

#[derive(Clone, Debug)]
pub struct BmadUnavailableDependency {
    module_code: String,
    skill_name: String,
    reason: String,
}

impl BmadUnavailableDependency {
    /// Declares one reviewed, explicit unavailable skill dependency.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed identities or unsafe reasons.
    pub fn new(
        module_code: impl Into<String>,
        skill_name: impl Into<String>,
        reason: impl Into<String>,
    ) -> Result<Self, BmadKernelError> {
        let module_code = module_code.into();
        let skill_name = skill_name.into();
        let reason = reason.into();
        if !valid_slug(&module_code, 64)
            || !valid_slug(&skill_name, 128)
            || reason.is_empty()
            || reason.len() > 512
            || !valid_display_cell(&reason)
            || contains_authority_directive(&reason)
        {
            return Err(BmadKernelErrorCode::AgentMenuTargetInvalid.into());
        }
        Ok(Self {
            module_code,
            skill_name,
            reason,
        })
    }
}

pub struct BmadAgentRosterBuilder;

impl BmadAgentRosterBuilder {
    /// Builds a display-only roster with closed skill/prompt target variants.
    ///
    /// # Errors
    ///
    /// Fails closed for duplicate identities/menu codes, prompt transplants,
    /// ambiguous target shapes, or missing reviewed prompt bindings.
    pub fn build(
        catalog: &BmadCatalog,
        sources: &[BmadAgentSource],
        reviewed_prompts: &BTreeMap<String, BmadReviewedPromptReference>,
        unavailable_dependencies: &[BmadUnavailableDependency],
    ) -> Result<BmadAgentRoster, BmadKernelError> {
        let mut unavailable = BTreeMap::new();
        for dependency in unavailable_dependencies {
            if unavailable
                .insert(
                    (
                        dependency.module_code.as_str(),
                        dependency.skill_name.as_str(),
                    ),
                    dependency.reason.as_str(),
                )
                .is_some()
            {
                return Err(BmadKernelErrorCode::AgentMenuTargetInvalid.into());
            }
        }
        let mut agent_codes = BTreeSet::new();
        let mut agents = Vec::with_capacity(sources.len());
        for source in sources {
            if !agent_codes.insert((source.module_code.clone(), source.agent_code.clone())) {
                return Err(BmadKernelErrorCode::AgentMenuTargetInvalid.into());
            }
            let mut menu_codes = BTreeSet::new();
            let mut menus = Vec::with_capacity(source.menus.len());
            for menu in &source.menus {
                if !menu_codes.insert(menu.code.clone()) {
                    return Err(BmadKernelErrorCode::AgentMenuTargetInvalid.into());
                }
                menus.push(resolve_menu_target(
                    catalog,
                    source,
                    menu,
                    reviewed_prompts,
                    &unavailable,
                )?);
            }
            let record_hash = canonical_hash(
                "bmad-agent-record",
                1,
                &serde_json::json!({
                    "moduleCode": source.module_code,
                    "agentCode": source.agent_code,
                    "displayName": source.display_name,
                    "title": source.title,
                    "icon": source.icon,
                    "team": source.team,
                    "description": source.description,
                    "moduleSourceHash": source.module_source_hash,
                    "entrypointHash": source.entrypoint_hash,
                    "customizationHash": source.customization_hash,
                    "personaGraphHash": source.persona_graph_hash,
                    "sourceMemberIds": source.source_member_ids,
                    "menus": source.menus.iter().map(|menu| serde_json::json!({
                        "code": menu.code,
                        "description": menu.description,
                        "target": menu.target,
                    })).collect::<Vec<_>>(),
                }),
            )
            .map_err(|_| BmadKernelErrorCode::AgentMenuTargetInvalid)?;
            agents.push(BmadAgentRecord {
                module_code: source.module_code.clone(),
                agent_code: source.agent_code.clone(),
                display_name: source.display_name.clone(),
                title: source.title.clone(),
                icon: source.icon.clone(),
                team: source.team.clone(),
                description: source.description.clone(),
                available: false,
                source_evidence_count: source.source_member_ids.len(),
                menus,
                record_hash,
            });
        }
        agents.sort_by(|left, right| {
            (&left.module_code, &left.agent_code).cmp(&(&right.module_code, &right.agent_code))
        });
        let record_hashes = agents
            .iter()
            .map(BmadAgentRecord::record_hash)
            .collect::<Vec<_>>();
        let roster_hash = canonical_hash("bmad-agent-roster", 1, &record_hashes)
            .map_err(|_| BmadKernelErrorCode::AgentMenuTargetInvalid)?;
        Ok(BmadAgentRoster {
            agents,
            roster_hash,
        })
    }
}

fn resolve_menu_target(
    catalog: &BmadCatalog,
    source: &BmadAgentSource,
    menu: &BmadAgentMenuSource,
    reviewed_prompts: &BTreeMap<String, BmadReviewedPromptReference>,
    unavailable: &BTreeMap<(&str, &str), &str>,
) -> Result<BmadAgentMenuRecord, BmadKernelError> {
    let target = menu
        .target
        .as_object()
        .ok_or(BmadKernelErrorCode::AgentMenuTargetInvalid)?;
    match map_string(target, "targetKind")? {
        "skill_target" => resolve_skill_target(catalog, menu, target, unavailable),
        "prompt_reference" => resolve_prompt_target(source, menu, target, reviewed_prompts),
        _ => Err(BmadKernelErrorCode::AgentMenuTargetInvalid.into()),
    }
}

fn resolve_skill_target(
    catalog: &BmadCatalog,
    menu: &BmadAgentMenuSource,
    target: &Map<String, Value>,
    unavailable: &BTreeMap<(&str, &str), &str>,
) -> Result<BmadAgentMenuRecord, BmadKernelError> {
    exact_keys(target, &["targetKind", "moduleCode", "skillName", "action"])?;
    let module_code = map_string(target, "moduleCode")?;
    let skill_name = map_string(target, "skillName")?;
    let action = target
        .get("action")
        .and_then(|value| {
            if value.is_null() {
                Some(None)
            } else {
                value.as_str().map(Some)
            }
        })
        .ok_or(BmadKernelErrorCode::AgentMenuTargetInvalid)?;
    if !valid_slug(module_code, 64)
        || !valid_slug(skill_name, 128)
        || action.is_some_and(|value| !valid_slug(value, 128))
    {
        return Err(BmadKernelErrorCode::AgentMenuTargetInvalid.into());
    }
    let skill = catalog
        .installed_skills
        .iter()
        .find(|skill| skill.module_code == module_code && skill.skill_name == skill_name);
    let action_known = action.is_none()
        || catalog.help_actions.iter().any(|candidate| {
            candidate.module_code == module_code
                && candidate.skill_name == skill_name
                && candidate.action.as_deref() == action
        });
    let explicitly_unavailable = unavailable.contains_key(&(module_code, skill_name));
    let availability = match skill {
        Some(skill) if action_known && skill.capability_enabled => {
            BmadCatalogAvailability::Available
        }
        Some(_) if action_known => BmadCatalogAvailability::CapabilityDisabled,
        None if explicitly_unavailable => BmadCatalogAvailability::DependencyUnavailable,
        _ => return Err(BmadKernelErrorCode::AgentMenuTargetInvalid.into()),
    };
    Ok(BmadAgentMenuRecord {
        code: menu.code.clone(),
        description: menu.description.clone(),
        target_kind: BmadMenuTargetKind::SkillTarget,
        display_label: skill_name.to_owned(),
        availability,
    })
}

fn resolve_prompt_target(
    source: &BmadAgentSource,
    menu: &BmadAgentMenuSource,
    target: &Map<String, Value>,
    reviewed_prompts: &BTreeMap<String, BmadReviewedPromptReference>,
) -> Result<BmadAgentMenuRecord, BmadKernelError> {
    exact_keys(
        target,
        &["targetKind", "sourceMemberId", "sourceMemberHash"],
    )?;
    let member_id = map_string(target, "sourceMemberId")?;
    let member_hash = Sha256Digest::parse(map_string(target, "sourceMemberHash")?)
        .map_err(|_| BmadKernelErrorCode::AgentMenuTargetInvalid)?;
    let reviewed = reviewed_prompts
        .get(member_id)
        .filter(|reviewed| {
            reviewed.id == member_id
                && reviewed.hash == member_hash
                && !reviewed.label.as_str().is_empty()
                && reviewed.owner_module_code == source.module_code
                && reviewed.owner_agent_code == source.agent_code
        })
        .ok_or(BmadKernelErrorCode::AgentMenuTargetInvalid)?;
    let _ = reviewed;
    Ok(BmadAgentMenuRecord {
        code: menu.code.clone(),
        description: menu.description.clone(),
        target_kind: BmadMenuTargetKind::PromptReference,
        display_label: "Source prompt reference".to_owned(),
        availability: BmadCatalogAvailability::SourcePromptUnavailable,
    })
}

fn exact_keys(object: &Map<String, Value>, expected: &[&str]) -> Result<(), BmadKernelError> {
    if object.len() == expected.len() && expected.iter().all(|key| object.contains_key(*key)) {
        Ok(())
    } else {
        Err(BmadKernelErrorCode::AgentMenuTargetInvalid.into())
    }
}

fn map_string<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, BmadKernelError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| BmadKernelErrorCode::AgentMenuTargetInvalid.into())
}

fn map_digest(object: &Map<String, Value>, key: &str) -> Result<Sha256Digest, BmadKernelError> {
    Sha256Digest::parse(map_string(object, key)?)
        .map_err(|_| BmadKernelErrorCode::AgentMenuTargetInvalid.into())
}

fn valid_source_member_id(value: &str) -> bool {
    let Some((prefix, suffix)) = value.split_once('-') else {
        return false;
    };
    valid_slug(prefix, 64) && suffix.len() == 3 && suffix.bytes().all(|byte| byte.is_ascii_digit())
}
