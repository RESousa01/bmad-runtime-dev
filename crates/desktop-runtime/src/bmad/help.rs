use std::collections::BTreeSet;

use serde::Serialize;

use crate::{ContractId, Sha256Digest};

use super::{
    BmadCatalog, BmadCatalogAvailability, BmadHelpAction, BmadHelpActionKey, BmadKernelError,
    BmadKernelErrorCode, MethodSession, MethodState,
};

const MAX_INTENT_BYTES: usize = 4_096;

#[derive(Clone, Debug)]
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
