use std::collections::BTreeSet;

use desktop_runtime::{
    BmadCatalogAvailability, BmadHelpConfidence, BmadHelpRecommendation, BmadLoadedPackage,
};
use serde::Serialize;
use thiserror::Error;

use crate::bmad::{
    BmadLibrarySourceKind, BmadLibrarySourceProjection, BmadProjectionAvailability,
    BmadProjectionBlockerCode,
};

pub const MAX_BMAD_HELP_RECOMMENDATION_BYTES: usize = 64 * 1_024;
const MAX_IDENTIFIER_BYTES: usize = 256;
const MAX_LABEL_BYTES: usize = 256;
const MAX_REASON_BYTES: usize = 4_096;
const MAX_EXPECTED_ARTIFACTS: usize = 16;
const MAX_BLOCKER_CODES: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadHelpConfidenceProjection {
    Authoritative,
    UserAsserted,
    Heuristic,
    Contextual,
    Unknown,
}

/// Renderer-safe, non-executable view of one host-grounded Help recommendation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadHelpRecommendationProjection {
    pub schema_version: &'static str,
    pub display_name: String,
    pub module_code: String,
    pub skill_name: String,
    pub action: Option<String>,
    pub source: BmadLibrarySourceProjection,
    pub confidence: BmadHelpConfidenceProjection,
    pub reason: String,
    pub required_guidance: bool,
    pub expected_artifacts: Vec<String>,
    pub availability: BmadProjectionAvailability,
    pub blocker_codes: Vec<BmadProjectionBlockerCode>,
    pub completion_claimed: bool,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum BmadHelpProjectionError {
    #[error("the BMAD Help recommendation projection is unavailable")]
    Unavailable,
}

impl BmadHelpProjectionError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Unavailable => "bmad_projection_unavailable",
        }
    }
}

/// Produces a bounded, explicitly inert Help recommendation for the renderer.
///
/// Internal capability identities, hashes, source references, alternatives,
/// and execution context are deliberately not part of this projection.
///
/// # Errors
///
/// Returns [`BmadHelpProjectionError::Unavailable`] rather than truncating when
/// any projected value is malformed, unsafe, inconsistent, or exceeds a
/// closed field/response limit. An internal completion claim is also rejected.
pub fn project_bmad_help_recommendation(
    package: &BmadLoadedPackage,
    recommendation: &BmadHelpRecommendation,
) -> Result<BmadHelpRecommendationProjection, BmadHelpProjectionError> {
    if recommendation.completion_claimed
        || recommendation.expected_outputs.len() > MAX_EXPECTED_ARTIFACTS
        || recommendation.blocker_codes.len() > MAX_BLOCKER_CODES
        || recommendation.action.package_version_id != package.package_version_id
        || recommendation.source_refs.is_empty()
        || recommendation.source_refs.iter().any(|source| {
            source.package_version_id != recommendation.action.package_version_id
                || source.capability_catalog_hash != recommendation.action.capability_catalog_hash
                || source.module_code != recommendation.action.module_code
                || source.skill_name != recommendation.action.skill_name
                || source.action != recommendation.action.action
        })
    {
        return Err(BmadHelpProjectionError::Unavailable);
    }

    let availability = project_availability(recommendation.availability);
    let blocker_codes =
        project_blocker_codes(recommendation.availability, &recommendation.blocker_codes)?;
    let projection = BmadHelpRecommendationProjection {
        schema_version: "bmad-help-recommendation.v1",
        display_name: bounded_nonempty_text(&recommendation.display_name, MAX_LABEL_BYTES)?,
        module_code: bounded_identifier(&recommendation.action.module_code)?,
        skill_name: bounded_identifier(&recommendation.action.skill_name)?,
        action: recommendation
            .action
            .action
            .as_deref()
            .map(bounded_identifier)
            .transpose()?,
        source: BmadLibrarySourceProjection {
            source_kind: BmadLibrarySourceKind::SealedFoundation,
            package_name: bounded_nonempty_text(&package.package_name, MAX_IDENTIFIER_BYTES)?,
            package_version: bounded_nonempty_text(&package.package_version, MAX_IDENTIFIER_BYTES)?,
        },
        confidence: project_confidence(recommendation.confidence),
        reason: bounded_nonempty_text(&recommendation.reason, MAX_REASON_BYTES)?,
        required_guidance: recommendation.required_guidance,
        expected_artifacts: recommendation
            .expected_outputs
            .iter()
            .map(|artifact| bounded_nonempty_text(artifact, MAX_LABEL_BYTES))
            .collect::<Result<_, _>>()?,
        availability,
        blocker_codes,
        completion_claimed: false,
    };

    if serde_json::to_vec(&projection)
        .map_err(|_| BmadHelpProjectionError::Unavailable)?
        .len()
        > MAX_BMAD_HELP_RECOMMENDATION_BYTES
    {
        return Err(BmadHelpProjectionError::Unavailable);
    }
    Ok(projection)
}

const fn project_confidence(value: BmadHelpConfidence) -> BmadHelpConfidenceProjection {
    match value {
        BmadHelpConfidence::Authoritative => BmadHelpConfidenceProjection::Authoritative,
        BmadHelpConfidence::UserAsserted => BmadHelpConfidenceProjection::UserAsserted,
        BmadHelpConfidence::Heuristic => BmadHelpConfidenceProjection::Heuristic,
        BmadHelpConfidence::Contextual => BmadHelpConfidenceProjection::Contextual,
        BmadHelpConfidence::Unknown => BmadHelpConfidenceProjection::Unknown,
    }
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

fn project_blocker_codes(
    availability: BmadCatalogAvailability,
    values: &[String],
) -> Result<Vec<BmadProjectionBlockerCode>, BmadHelpProjectionError> {
    let mut unique = BTreeSet::new();
    let projected = values
        .iter()
        .map(|value| {
            let blocker = match value.as_str() {
                "bmad_capability_disabled" => BmadProjectionBlockerCode::BmadCapabilityDisabled,
                "bmad_dependency_unavailable" => {
                    BmadProjectionBlockerCode::BmadDependencyUnavailable
                }
                "bmad_help_catalog_orphan" => BmadProjectionBlockerCode::BmadHelpCatalogOrphan,
                "bmad_network_reference_unavailable" => {
                    BmadProjectionBlockerCode::BmadNetworkReferenceUnavailable
                }
                "bmad_source_prompt_unavailable" => {
                    BmadProjectionBlockerCode::BmadSourcePromptUnavailable
                }
                _ => return Err(BmadHelpProjectionError::Unavailable),
            };
            if !unique.insert(blocker) {
                return Err(BmadHelpProjectionError::Unavailable);
            }
            Ok(blocker)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let expected = expected_blocker_code(availability);
    if projected.as_slice() != expected.as_slice() {
        return Err(BmadHelpProjectionError::Unavailable);
    }
    Ok(projected)
}

fn expected_blocker_code(availability: BmadCatalogAvailability) -> Vec<BmadProjectionBlockerCode> {
    let code = match availability {
        BmadCatalogAvailability::Available => return Vec::new(),
        BmadCatalogAvailability::CapabilityDisabled => {
            BmadProjectionBlockerCode::BmadCapabilityDisabled
        }
        BmadCatalogAvailability::DependencyUnavailable => {
            BmadProjectionBlockerCode::BmadDependencyUnavailable
        }
        BmadCatalogAvailability::OrphanSkill => BmadProjectionBlockerCode::BmadHelpCatalogOrphan,
        BmadCatalogAvailability::NetworkUnavailable => {
            BmadProjectionBlockerCode::BmadNetworkReferenceUnavailable
        }
        BmadCatalogAvailability::SourcePromptUnavailable => {
            BmadProjectionBlockerCode::BmadSourcePromptUnavailable
        }
    };
    vec![code]
}

fn bounded_identifier(value: &str) -> Result<String, BmadHelpProjectionError> {
    if value.is_empty()
        || value.len() > MAX_IDENTIFIER_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(BmadHelpProjectionError::Unavailable);
    }
    Ok(value.to_owned())
}

fn bounded_nonempty_text(value: &str, max_bytes: usize) -> Result<String, BmadHelpProjectionError> {
    if value.trim().is_empty()
        || value.len() > max_bytes
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
        return Err(BmadHelpProjectionError::Unavailable);
    }
    Ok(value.to_owned())
}
