//! Canonical, deterministic BMAD Help.
//!
//! Everything below `BmadHelpQuery` answers availability, prerequisite,
//! next-step, alias, and support-state questions purely from the generated
//! catalog and caller-supplied project state. Without this module, Help would
//! need a live model call to answer "can I run this" or "what's next" -- which
//! would make Help itself gated on the same chat capability it is supposed to
//! remain available without. Nothing here reads a file, calls a port, or
//! carries a model/chat type; the whole surface is data in, typed answer out.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

use crate::{ContractId, Sha256Digest};

use super::{
    BmadCatalog, BmadCatalogAvailability, BmadHelpAction, BmadHelpActionKey, BmadKernelError,
    BmadKernelErrorCode, MethodSession, MethodState,
};

const MAX_INTENT_BYTES: usize = 4_096;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct BmadHelpIntent(String);

impl BmadHelpIntent {
    /// Creates bounded, inert intent text for catalog ranking.
    ///
    /// # Errors
    ///
    /// Returns [`BmadKernelErrorCode::HelpEvidenceInsufficient`] for empty,
    /// oversized, control-bearing, or bidirectional text.
    pub fn new(value: impl Into<String>) -> Result<Self, BmadKernelError> {
        let value = value.into();
        if value.trim().is_empty()
            || value.len() > MAX_INTENT_BYTES
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
            return Err(BmadKernelErrorCode::HelpEvidenceInsufficient.into());
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for BmadHelpIntent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BmadArtifactEvidenceKind {
    Unknown,
    ConversationStatement,
    FuzzyArtifactMatch,
    UserBoundImport,
    RecordedSuccessfulRun,
}

#[derive(Clone, Debug)]
pub struct BmadArtifactEvidence {
    action: BmadHelpActionKey,
    kind: BmadArtifactEvidenceKind,
    evidence_ref_hash: Sha256Digest,
}

impl BmadArtifactEvidence {
    /// Creates authoritative evidence only from a completed, exact-bound Method
    /// invocation for the same capability key.
    ///
    /// # Errors
    ///
    /// Returns an error when the session is incomplete, the capability differs,
    /// or the invocation has no aggregate-owned provenance.
    pub fn from_completed_session(
        action: BmadHelpActionKey,
        session: &MethodSession,
        invocation_id: &ContractId,
    ) -> Result<Self, BmadKernelError> {
        let binding = session
            .current_binding()
            .map_err(|_| BmadKernelErrorCode::HelpEvidenceInsufficient)?;
        if session.state() != MethodState::Completed
            || binding.capability_catalog_hash != action.capability_catalog_hash
            || binding.capability_key.package_version_id != action.package_version_id
            || binding.capability_key.module_code != action.module_code
            || binding.capability_key.skill_name != action.skill_name
            || binding.capability_key.normalized_action != action.action
        {
            return Err(BmadKernelErrorCode::HelpEvidenceInsufficient.into());
        }
        let provenance = session
            .artifact_provenance_for(invocation_id)
            .map_err(|_| BmadKernelErrorCode::HelpEvidenceInsufficient)?;
        let current_binding_hash = binding
            .binding_hash()
            .map_err(|_| BmadKernelErrorCode::HelpEvidenceInsufficient)?;
        if provenance.binding_hash != current_binding_hash {
            return Err(BmadKernelErrorCode::HelpEvidenceInsufficient.into());
        }
        Ok(Self {
            action,
            kind: BmadArtifactEvidenceKind::RecordedSuccessfulRun,
            evidence_ref_hash: provenance.binding_hash,
        })
    }

    /// Records a host-observed fuzzy artifact match. This produces only
    /// heuristic confidence and never a completion claim.
    #[must_use]
    pub const fn heuristic(action: BmadHelpActionKey, artifact_hash: Sha256Digest) -> Self {
        Self {
            action,
            kind: BmadArtifactEvidenceKind::FuzzyArtifactMatch,
            evidence_ref_hash: artifact_hash,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadHelpConfidence {
    Authoritative,
    UserAsserted,
    Heuristic,
    Contextual,
    Unknown,
}

#[derive(Clone, Debug, Serialize)]
pub struct BmadHelpSourceRef {
    pub capability_catalog_hash: Sha256Digest,
    pub package_version_id: ContractId,
    pub module_code: String,
    pub skill_name: String,
    pub action: Option<String>,
    pub source_ordinal: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct BmadHelpRecommendation {
    pub action: BmadHelpActionKey,
    pub display_name: String,
    pub reason: String,
    pub required_guidance: bool,
    pub confidence: BmadHelpConfidence,
    pub availability: BmadCatalogAvailability,
    pub expected_outputs: Vec<String>,
    pub source_refs: Vec<BmadHelpSourceRef>,
    pub blocker_codes: Vec<String>,
    pub alternatives: Vec<BmadHelpActionKey>,
    pub completion_claimed: bool,
}

pub struct BmadHelpAdvisor;

impl BmadHelpAdvisor {
    /// Ranks catalog evidence and returns a non-executable recommendation.
    ///
    /// # Errors
    ///
    /// Returns [`BmadKernelErrorCode::HelpEvidenceInsufficient`] when no
    /// user-facing action exists. It never turns evidence into completion.
    pub fn recommend(
        catalog: &BmadCatalog,
        intent: &BmadHelpIntent,
        evidence: &[BmadArtifactEvidence],
    ) -> Result<BmadHelpRecommendation, BmadKernelError> {
        let action =
            select_action(catalog, intent).ok_or(BmadKernelErrorCode::HelpEvidenceInsufficient)?;
        let evidence_kind = evidence
            .iter()
            .filter(|item| item.action == action.key)
            .max_by_key(|item| (item.kind, item.evidence_ref_hash))
            .map_or(BmadArtifactEvidenceKind::Unknown, |item| item.kind);
        let confidence = match evidence_kind {
            BmadArtifactEvidenceKind::RecordedSuccessfulRun => BmadHelpConfidence::Authoritative,
            BmadArtifactEvidenceKind::UserBoundImport => BmadHelpConfidence::UserAsserted,
            BmadArtifactEvidenceKind::FuzzyArtifactMatch => BmadHelpConfidence::Heuristic,
            BmadArtifactEvidenceKind::ConversationStatement => BmadHelpConfidence::Contextual,
            BmadArtifactEvidenceKind::Unknown => BmadHelpConfidence::Unknown,
        };
        Ok(BmadHelpRecommendation {
            action: action.key.clone(),
            display_name: action.display_name.clone(),
            reason: recommendation_reason(action, confidence),
            required_guidance: action.required,
            confidence,
            availability: action.availability,
            expected_outputs: action.expected_outputs.clone(),
            source_refs: vec![BmadHelpSourceRef {
                capability_catalog_hash: action.key.capability_catalog_hash,
                package_version_id: action.key.package_version_id.clone(),
                module_code: action.module_code.clone(),
                skill_name: action.skill_name.clone(),
                action: action.action.clone(),
                source_ordinal: action.source_ordinal,
            }],
            blocker_codes: blocker_codes(action.availability),
            alternatives: alternatives(catalog, action),
            completion_claimed: false,
        })
    }
}

fn blocker_codes(availability: BmadCatalogAvailability) -> Vec<String> {
    let code = match availability {
        BmadCatalogAvailability::Available => return Vec::new(),
        BmadCatalogAvailability::CapabilityDisabled => "bmad_capability_disabled",
        BmadCatalogAvailability::DependencyUnavailable => "bmad_dependency_unavailable",
        BmadCatalogAvailability::OrphanSkill => "bmad_help_catalog_orphan",
        BmadCatalogAvailability::NetworkUnavailable => "bmad_network_reference_unavailable",
        BmadCatalogAvailability::SourcePromptUnavailable => "bmad_source_prompt_unavailable",
    };
    vec![code.to_owned()]
}

fn alternatives(catalog: &BmadCatalog, selected: &BmadHelpAction) -> Vec<BmadHelpActionKey> {
    catalog
        .help_actions
        .iter()
        .filter(|action| {
            action.skill_name != "_meta"
                && action.key != selected.key
                && action.module_code == selected.module_code
        })
        .take(3)
        .map(|action| action.key.clone())
        .collect()
}

fn select_action<'a>(
    catalog: &'a BmadCatalog,
    intent: &BmadHelpIntent,
) -> Option<&'a BmadHelpAction> {
    let intent_tokens = tokens(&intent.0);
    catalog
        .help_actions
        .iter()
        .filter(|action| action.skill_name != "_meta")
        .filter_map(|action| {
            let mut candidate = tokens(&action.skill_name);
            candidate.extend(tokens(&action.display_name));
            candidate.extend(tokens(&action.description));
            let relevance = intent_tokens.intersection(&candidate).count();
            (relevance > 0).then_some((action, relevance, usize::from(action.required)))
        })
        .max_by_key(|(_, relevance, required)| (*relevance, *required))
        .map(|(action, _, _)| action)
}

fn tokens(value: &str) -> BTreeSet<String> {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .map(str::to_ascii_lowercase)
        .filter(|token| token.len() >= 3)
        .collect()
}

fn recommendation_reason(action: &BmadHelpAction, confidence: BmadHelpConfidence) -> String {
    match confidence {
        BmadHelpConfidence::Authoritative => {
            "A recorded local run is bound to this catalog action.".to_owned()
        }
        BmadHelpConfidence::UserAsserted => {
            "A user-bound import references this catalog action.".to_owned()
        }
        BmadHelpConfidence::Heuristic => {
            "Artifact names heuristically resemble this action's expected output.".to_owned()
        }
        BmadHelpConfidence::Contextual => {
            "Conversation context mentions this catalog action.".to_owned()
        }
        BmadHelpConfidence::Unknown => format!(
            "The current intent most closely matches the catalog entry {}.",
            action.display_name
        ),
    }
}

// ---------------------------------------------------------------------------
// Deterministic availability, next-step, alias, support-state, and projection
// answers. Everything below reads only the catalog and caller-supplied
// project/support state; nothing here touches a model port, a chat type, or
// any I/O.
// ---------------------------------------------------------------------------

/// Errors from the deterministic Help query surface.
///
/// This is deliberately its own type rather than a new [`BmadKernelErrorCode`]
/// variant: a removed alias is not "insufficient evidence" or an "invalid
/// catalog" -- collapsing it into an existing code would erase the very
/// distinction acceptance criterion (4) requires (an explicit diagnostic, not
/// a folded-in miss).
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum BmadHelpQueryError {
    #[error("BMAD Help has no catalog entry for capability {0:?}")]
    UnknownCapability(String),
    #[error("BMAD Help alias {0:?} is not registered")]
    AliasUnknown(String),
    #[error("BMAD Help alias {0:?} has been removed and no longer resolves")]
    AliasRemoved(String),
}

/// Whether a catalog entry can be invoked given the current project state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadHelpInvocability {
    Invokable,
    Blocked,
}

/// How closely a capability's runtime behavior matches its reviewed source.
///
/// This is orthogonal to [`BmadCatalogAvailability`]: availability answers
/// "does this exist and is it enabled," support state answers "how faithfully
/// does the shipped implementation match the pinned source." A capability can
/// be available and still be a `Shim`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadHelpSupportState {
    Exact,
    Adapted,
    Shim,
}

/// Which capabilities a project has already completed.
///
/// Help has no persistence port, so it never derives this itself -- the host
/// supplies it from whatever session/artifact record it already maintains.
/// Without this boundary, "is this capability done" would require Help to
/// read files or call a store, which is exactly the I/O this module refuses.
#[derive(Clone, Debug, Default)]
pub struct BmadHelpProjectState {
    completed: BTreeSet<String>,
}

impl BmadHelpProjectState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_completed(mut self, capability: impl Into<String>) -> Self {
        self.completed.insert(capability.into());
        self
    }

    #[must_use]
    pub fn is_completed(&self, capability: &str) -> bool {
        self.completed.contains(capability)
    }
}

/// Host-supplied exact/adapted/shim classification, keyed by `skill_name`.
///
/// A capability absent from the table defaults to `Exact`: the catalog itself
/// carries no support-state column, so an entry the host has not classified
/// is treated as matching its reviewed source until told otherwise.
#[derive(Clone, Debug, Default)]
pub struct BmadHelpSupportStateTable {
    states: BTreeMap<String, BmadHelpSupportState>,
}

impl BmadHelpSupportStateTable {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_state(
        mut self,
        capability: impl Into<String>,
        state: BmadHelpSupportState,
    ) -> Self {
        self.states.insert(capability.into(), state);
        self
    }

    #[must_use]
    pub fn state_for(&self, capability: &str) -> BmadHelpSupportState {
        self.states
            .get(capability)
            .copied()
            .unwrap_or(BmadHelpSupportState::Exact)
    }
}

/// A legacy-id-to-canonical-capability route.
#[derive(Clone, Debug)]
struct BmadHelpAliasRoute {
    canonical_skill_name: String,
    deprecated: bool,
    removed: bool,
}

/// Host-supplied alias/deprecation table, keyed by legacy id.
///
/// Kept separate from [`BmadCatalog`] because aliases are a migration
/// concern layered over the catalog, not a catalog fact: the same catalog
/// build can be paired with a growing set of retired legacy ids over time.
#[derive(Clone, Debug, Default)]
pub struct BmadHelpAliasTable {
    routes: BTreeMap<String, BmadHelpAliasRoute>,
}

impl BmadHelpAliasTable {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_alias(
        mut self,
        legacy_id: impl Into<String>,
        canonical_skill_name: impl Into<String>,
        deprecated: bool,
    ) -> Self {
        self.routes.insert(
            legacy_id.into(),
            BmadHelpAliasRoute {
                canonical_skill_name: canonical_skill_name.into(),
                deprecated,
                removed: false,
            },
        );
        self
    }

    #[must_use]
    pub fn with_removed(mut self, legacy_id: impl Into<String>) -> Self {
        self.routes.insert(
            legacy_id.into(),
            BmadHelpAliasRoute {
                canonical_skill_name: String::new(),
                deprecated: true,
                removed: true,
            },
        );
        self
    }
}

/// Non-prose answer to "can I invoke this, and if not, what's missing."
#[derive(Clone, Debug, Serialize)]
pub struct BmadHelpAvailabilityReport {
    pub capability: BmadHelpActionKey,
    pub invocability: BmadHelpInvocability,
    /// Unmet prerequisite ids (catalog `skill_name`s), never prose. Empty
    /// exactly when `invocability` is `Invokable`.
    pub unmet_prerequisites: Vec<String>,
    pub support_state: BmadHelpSupportState,
}

/// One entry in the deterministic next-step list.
///
/// A completed capability never appears here (the caller filters it before
/// construction). A `blocked` entry is still reported -- so the caller can
/// show *why* a capability isn't ready -- but must not be offered as a ready
/// action.
#[derive(Clone, Debug, Serialize)]
pub struct BmadHelpNextStep {
    pub capability: BmadHelpActionKey,
    pub display_name: String,
    pub phase: Option<String>,
    pub blocked: bool,
    pub unmet_prerequisites: Vec<String>,
}

/// Resolved identity and deprecation status for a legacy id.
#[derive(Clone, Debug, Serialize)]
pub struct BmadHelpAliasResolution {
    pub requested_id: String,
    pub canonical: BmadHelpActionKey,
    pub deprecated: bool,
}

/// One row of the structured projection consumed by UI and the command
/// palette. Every field is a typed id or enum, never a formatted string, so a
/// renderer cannot accidentally parse prose to recover structure.
#[derive(Clone, Debug, Serialize)]
pub struct BmadHelpCapabilityProjection {
    pub capability: BmadHelpActionKey,
    pub display_name: String,
    pub description: String,
    pub phase: Option<String>,
    pub invocability: BmadHelpInvocability,
    pub unmet_prerequisites: Vec<String>,
    pub support_state: BmadHelpSupportState,
}

/// Catalog totals, always derived from the live catalog rather than a pinned
/// constant, so acceptance criterion (3) cannot regress by someone hand-
/// editing a hardcoded number that drifts from the real row count.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct BmadHelpCatalogCounts {
    pub total_capabilities: usize,
    pub invokable_now: usize,
    pub blocked: usize,
}

/// The deterministic Help query surface. Every method takes the catalog plus
/// caller-supplied state and returns a typed answer; none of them accept, or
/// need, a model or chat capability.
pub struct BmadHelpQuery;

impl BmadHelpQuery {
    /// Reports whether `capability` is invokable now, and if not, exactly
    /// which prerequisite ids block it.
    ///
    /// # Errors
    ///
    /// Returns [`BmadHelpQueryError::UnknownCapability`] when no catalog row
    /// has this `skill_name`.
    pub fn availability(
        catalog: &BmadCatalog,
        project_state: &BmadHelpProjectState,
        support_states: &BmadHelpSupportStateTable,
        capability: &str,
    ) -> Result<BmadHelpAvailabilityReport, BmadHelpQueryError> {
        let action = find_action(catalog, capability)
            .ok_or_else(|| BmadHelpQueryError::UnknownCapability(capability.to_owned()))?;
        let unmet = unmet_prerequisites(project_state, action);
        Ok(BmadHelpAvailabilityReport {
            capability: action.key.clone(),
            invocability: invocability(&unmet),
            unmet_prerequisites: unmet,
            support_state: support_states.state_for(capability),
        })
    }

    /// Lists every non-meta catalog capability not yet completed, each
    /// carrying its own blocked/invokable status and unmet prerequisites.
    ///
    /// A completed capability is omitted outright rather than included and
    /// marked done, so a caller cannot accidentally re-offer it by skipping a
    /// status check.
    #[must_use]
    pub fn next_steps(
        catalog: &BmadCatalog,
        project_state: &BmadHelpProjectState,
    ) -> Vec<BmadHelpNextStep> {
        catalog
            .help_actions
            .iter()
            .filter(|action| action.skill_name != "_meta")
            .filter(|action| !project_state.is_completed(&action.skill_name))
            .map(|action| {
                let unmet = unmet_prerequisites(project_state, action);
                BmadHelpNextStep {
                    capability: action.key.clone(),
                    display_name: action.display_name.clone(),
                    phase: action.phase.clone(),
                    blocked: !unmet.is_empty(),
                    unmet_prerequisites: unmet,
                }
            })
            .collect()
    }

    /// Resolves a legacy id to its canonical capability and deprecation
    /// status.
    ///
    /// # Errors
    ///
    /// Returns [`BmadHelpQueryError::AliasRemoved`] for a legacy id the table
    /// marks removed, and [`BmadHelpQueryError::AliasUnknown`] for one the
    /// table has never registered. These are kept distinct on purpose: a
    /// removed alias is a governed migration outcome, not an unrecognized
    /// string.
    pub fn resolve_alias(
        catalog: &BmadCatalog,
        alias_table: &BmadHelpAliasTable,
        legacy_id: &str,
    ) -> Result<BmadHelpAliasResolution, BmadHelpQueryError> {
        let route = alias_table
            .routes
            .get(legacy_id)
            .ok_or_else(|| BmadHelpQueryError::AliasUnknown(legacy_id.to_owned()))?;
        if route.removed {
            return Err(BmadHelpQueryError::AliasRemoved(legacy_id.to_owned()));
        }
        let action = find_action(catalog, &route.canonical_skill_name).ok_or_else(|| {
            BmadHelpQueryError::UnknownCapability(route.canonical_skill_name.clone())
        })?;
        Ok(BmadHelpAliasResolution {
            requested_id: legacy_id.to_owned(),
            canonical: action.key.clone(),
            deprecated: route.deprecated,
        })
    }

    /// Projects every non-meta catalog capability into typed UI/command-
    /// palette rows.
    #[must_use]
    pub fn project_capabilities(
        catalog: &BmadCatalog,
        project_state: &BmadHelpProjectState,
        support_states: &BmadHelpSupportStateTable,
    ) -> Vec<BmadHelpCapabilityProjection> {
        catalog
            .help_actions
            .iter()
            .filter(|action| action.skill_name != "_meta")
            .map(|action| {
                let unmet = unmet_prerequisites(project_state, action);
                BmadHelpCapabilityProjection {
                    capability: action.key.clone(),
                    display_name: action.display_name.clone(),
                    description: action.description.clone(),
                    phase: action.phase.clone(),
                    invocability: invocability(&unmet),
                    support_state: support_states.state_for(&action.skill_name),
                    unmet_prerequisites: unmet,
                }
            })
            .collect()
    }

    /// Catalog totals derived from the live catalog: total non-meta
    /// capabilities, how many are invokable now, and how many are blocked.
    #[must_use]
    pub fn catalog_counts(
        catalog: &BmadCatalog,
        project_state: &BmadHelpProjectState,
    ) -> BmadHelpCatalogCounts {
        let mut counts = BmadHelpCatalogCounts {
            total_capabilities: 0,
            invokable_now: 0,
            blocked: 0,
        };
        for action in catalog
            .help_actions
            .iter()
            .filter(|action| action.skill_name != "_meta")
        {
            counts.total_capabilities += 1;
            if unmet_prerequisites(project_state, action).is_empty() {
                counts.invokable_now += 1;
            } else {
                counts.blocked += 1;
            }
        }
        counts
    }
}

fn invocability(unmet_prerequisites: &[String]) -> BmadHelpInvocability {
    if unmet_prerequisites.is_empty() {
        BmadHelpInvocability::Invokable
    } else {
        BmadHelpInvocability::Blocked
    }
}

fn find_action<'a>(catalog: &'a BmadCatalog, skill_name: &str) -> Option<&'a BmadHelpAction> {
    catalog
        .help_actions
        .iter()
        .find(|action| action.skill_name == skill_name)
}

/// A capability's single predecessor, by `skill_name`, when it is not yet
/// complete. `preceded_by` carries at most one dependency (the catalog's CSV
/// column is single-valued), so this returns zero or one id.
fn unmet_prerequisites(
    project_state: &BmadHelpProjectState,
    action: &BmadHelpAction,
) -> Vec<String> {
    action
        .preceded_by
        .iter()
        .filter(|dependency| !project_state.is_completed(dependency))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        sha256_bytes, BmadCatalogBuilder, BmadEntrypointKind, BmadHelpCatalogSource,
        BmadLoadedPackage, BmadLoadedSkill,
    };

    const HEADER: &str = "module,skill,display-name,menu-code,description,action,args,phase,preceded-by,followed-by,required,output-location,outputs";

    fn package() -> BmadLoadedPackage {
        BmadLoadedPackage {
            package_name: "bmad-method".to_owned(),
            package_version: "6.10.0".to_owned(),
            package_version_id: ContractId::new(
                "pkgver_8B33C55A4D67D0B258FEDBB75D1CB09DBC7F711BC9BDC794D8B052B31FCE6D86",
            )
            .expect("package version id"),
            descriptor_hash: sha256_bytes(b"descriptor"),
            observed_inventory_hash: sha256_bytes(b"inventory"),
            skills: vec![
                BmadLoadedSkill {
                    module_code: "bmm".to_owned(),
                    skill_name: "bmad-product-brief".to_owned(),
                    display_name: "Product Brief".to_owned(),
                    description: "Draft a product brief.".to_owned(),
                    entrypoint_kind: BmadEntrypointKind::Direct,
                    actions: Vec::new(),
                    distribution_profile: "sapphirus_package".to_owned(),
                    install_profile: "SapphirusManagedV1".to_owned(),
                    validation_profile: "MethodOfficialSkillV6".to_owned(),
                    execution_profile_hash: sha256_bytes(b"brief profile"),
                    capability_enabled: true,
                    structurally_eligible: true,
                },
                BmadLoadedSkill {
                    module_code: "bmm".to_owned(),
                    skill_name: "bmad-prd".to_owned(),
                    display_name: "PRD".to_owned(),
                    description: "Write the PRD.".to_owned(),
                    entrypoint_kind: BmadEntrypointKind::Direct,
                    actions: Vec::new(),
                    distribution_profile: "sapphirus_package".to_owned(),
                    install_profile: "SapphirusManagedV1".to_owned(),
                    validation_profile: "MethodOfficialSkillV6".to_owned(),
                    execution_profile_hash: sha256_bytes(b"prd profile"),
                    capability_enabled: true,
                    structurally_eligible: true,
                },
            ],
        }
    }

    fn catalog() -> BmadCatalog {
        let rows = [
            "BMad Method,bmad-product-brief,Product Brief,PB,Draft a product brief.,,,1-analysis,,,true,planning_artifacts,brief",
            "BMad Method,bmad-prd,PRD,PR,Write the PRD.,,,2-plan-workflows,bmad-product-brief,,true,planning_artifacts,prd",
        ];
        let source = BmadHelpCatalogSource::new("bmm", format!("{HEADER}\n{}\n", rows.join("\n")))
            .expect("catalog source");
        BmadCatalogBuilder::build(&package(), &[source]).expect("catalog")
    }

    #[test]
    // Break caught: a later edit wiring Help's deterministic query surface to
    // a model port, provider client, or chat type -- which would make Help
    // itself gated on the capability it exists to remain available without.
    // The source scan fails loudly on any real (non-comment) reference.
    fn bmad_help_answers_with_no_model_or_chat_capability_present() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bmad/help.rs"),
        )
        .expect("help.rs source");
        // Scan only the module proper, not this test's own assertion strings
        // (which necessarily mention the forbidden tokens as literals).
        let module_source = source
            .split_once("#[cfg(test)]")
            .map_or(source.as_str(), |(module, _)| module);
        for (index, line) in module_source.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            assert!(
                !code.contains("model_port")
                    && !code.contains("ModelPort")
                    && !code.contains("chat::")
                    && !code.contains("ChatSession")
                    && !code.contains("ChatTurn"),
                "help.rs line {} references a model/chat capability: {line:?}",
                index + 1,
            );
        }

        // Functional half: a full availability answer with only the catalog
        // and inert project/support state -- no model or chat value exists
        // anywhere in this call.
        let catalog = catalog();
        let report = BmadHelpQuery::availability(
            &catalog,
            &BmadHelpProjectState::new(),
            &BmadHelpSupportStateTable::new(),
            "bmad-product-brief",
        )
        .expect("availability answer");
        assert!(matches!(
            report.invocability,
            BmadHelpInvocability::Invokable
        ));
    }

    #[test]
    // Break caught: reporting a blocked capability's reason as prose (or
    // omitting it), which would violate acceptance (2) -- a suggested action
    // must be genuinely invokable or come with the exact unmet id list.
    fn bmad_help_blocked_capability_reports_unmet_ids_invokable_reports_none() {
        let catalog = catalog();
        let project_state = BmadHelpProjectState::new();
        let support_states = BmadHelpSupportStateTable::new();

        let prd =
            BmadHelpQuery::availability(&catalog, &project_state, &support_states, "bmad-prd")
                .expect("prd availability");
        assert!(matches!(prd.invocability, BmadHelpInvocability::Blocked));
        assert_eq!(
            prd.unmet_prerequisites,
            vec!["bmad-product-brief".to_owned()]
        );

        let brief = BmadHelpQuery::availability(
            &catalog,
            &project_state,
            &support_states,
            "bmad-product-brief",
        )
        .expect("brief availability");
        assert!(matches!(
            brief.invocability,
            BmadHelpInvocability::Invokable
        ));
        assert!(brief.unmet_prerequisites.is_empty());
    }

    #[test]
    // Break caught: re-suggesting a capability the project already completed
    // (a stale prompt loop), and separately, silently dropping a blocked
    // capability instead of reporting it as blocked.
    fn bmad_help_completed_capability_is_not_resuggested_blocked_one_is_reported() {
        let catalog = catalog();
        let project_state = BmadHelpProjectState::new().with_completed("bmad-product-brief");

        let steps = BmadHelpQuery::next_steps(&catalog, &project_state);
        assert!(
            !steps
                .iter()
                .any(|step| step.capability.skill_name == "bmad-product-brief"),
            "a completed capability must not be re-suggested"
        );
        let prd_step = steps
            .iter()
            .find(|step| step.capability.skill_name == "bmad-prd")
            .expect("prd is still incomplete");
        assert!(!prd_step.blocked, "prd's predecessor is now complete");
        assert!(prd_step.unmet_prerequisites.is_empty());
    }

    #[test]
    // Break caught: a removed alias silently resolving to nothing (or to a
    // stale target) instead of surfacing as a distinct, explicit diagnostic.
    fn bmad_help_legacy_alias_resolves_removed_alias_yields_explicit_diagnostic() {
        let catalog = catalog();
        let alias_table = BmadHelpAliasTable::new()
            .with_alias("bmad-create-prd", "bmad-prd", true)
            .with_removed("bmad-legacy-brief");

        let resolved = BmadHelpQuery::resolve_alias(&catalog, &alias_table, "bmad-create-prd")
            .expect("legacy alias resolves");
        assert_eq!(resolved.canonical.skill_name, "bmad-prd");
        assert!(resolved.deprecated);

        let removed = BmadHelpQuery::resolve_alias(&catalog, &alias_table, "bmad-legacy-brief");
        assert!(matches!(removed, Err(BmadHelpQueryError::AliasRemoved(_))));

        let unknown = BmadHelpQuery::resolve_alias(&catalog, &alias_table, "never-registered");
        assert!(matches!(unknown, Err(BmadHelpQueryError::AliasUnknown(_))));
    }

    #[test]
    // Break caught: collapsing exact/adapted/shim into a boolean (or two of
    // the three states), which would silently hide a shim from the projection
    // a user relies on to know what they are really running.
    fn bmad_help_support_state_exact_adapted_shim_all_representable() {
        let support_states = BmadHelpSupportStateTable::new()
            .with_state("bmad-prd", BmadHelpSupportState::Adapted)
            .with_state("bmad-product-brief", BmadHelpSupportState::Shim);
        assert!(matches!(
            support_states.state_for("bmad-prd"),
            BmadHelpSupportState::Adapted
        ));
        assert!(matches!(
            support_states.state_for("bmad-product-brief"),
            BmadHelpSupportState::Shim
        ));
        // Absent from the table defaults to Exact -- proving all three states
        // are reachable, including the unclassified default.
        assert!(matches!(
            support_states.state_for("bmad-unclassified"),
            BmadHelpSupportState::Exact
        ));
    }

    #[test]
    // Break caught: a hardcoded total that drifts from the catalog's real row
    // count the moment a capability is added or removed -- acceptance (3)
    // requires the count to be derived, not pinned.
    fn bmad_help_reported_counts_equal_catalog_count_derived_not_hardcoded() {
        let catalog = catalog();
        let counts = BmadHelpQuery::catalog_counts(&catalog, &BmadHelpProjectState::new());
        let expected = catalog
            .help_actions
            .iter()
            .filter(|action| action.skill_name != "_meta")
            .count();
        assert_eq!(counts.total_capabilities, expected);
        assert_eq!(counts.invokable_now + counts.blocked, expected);
        assert_eq!(counts.invokable_now, 1, "only the brief has no predecessor");
        assert_eq!(counts.blocked, 1, "the prd is blocked on the brief");
    }

    #[test]
    fn bmad_help_project_capabilities_is_typed_not_prose() {
        let catalog = catalog();
        let rows = BmadHelpQuery::project_capabilities(
            &catalog,
            &BmadHelpProjectState::new(),
            &BmadHelpSupportStateTable::new(),
        );
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| !row.capability.skill_name.is_empty()));
    }
}
