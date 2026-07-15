use std::{collections::BTreeSet, fmt, sync::Arc};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    canonical_hash, canonical_json_bytes, generated_contracts, sha256_bytes, ContractId,
    Sha256Digest, UnixMillis,
};

use super::{
    BmadCapabilityKey, BmadCatalogAvailability, BmadCompiledHelpInvocation, BmadHelpAction,
    BmadHelpActionKey, BmadKernelError, BmadKernelErrorCode, MethodAdvanceDisposition,
    MethodAdvanceReceipt, MethodAdvanceResult, MethodCanonicalAdvanceResultData, MethodSession,
    MethodVerifiedAdvanceResult, MethodVerifiedResultBindingData,
};

const MAX_HELP_PROPOSAL_BYTES: usize = 65_536;
const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;
const JSON_MEDIA_TYPE: &str = "application/json";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadArtifactClassification {
    Public,
    Internal,
    Confidential,
    Restricted,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadArtifactReference {
    #[serde(rename = "ref")]
    pub ref_: String,
    pub content_hash: Sha256Digest,
    pub byte_length: u64,
    pub media_type: String,
    pub artifact_id: ContractId,
    pub classification: BmadArtifactClassification,
}

impl BmadArtifactReference {
    /// Creates a host-owned, content-addressed evidence reference.
    ///
    /// # Errors
    ///
    /// Rejects references that are not exact local SHA-256 CAS identities or
    /// do not satisfy the generated `ArtifactRef` contract.
    pub fn new(
        artifact_id: ContractId,
        ref_: impl Into<String>,
        content_hash: Sha256Digest,
        byte_length: u64,
        media_type: impl Into<String>,
        classification: BmadArtifactClassification,
    ) -> Result<Self, BmadKernelError> {
        let value = Self {
            ref_: ref_.into(),
            content_hash,
            byte_length,
            media_type: media_type.into(),
            artifact_id,
            classification,
        };
        let expected_ref = format!("cas://sha256/{}", value.content_hash.hex_value());
        if value.ref_ != expected_ref
            || value.byte_length > MAX_SAFE_JSON_INTEGER
            || !safe_text(&value.ref_)
            || !safe_text(&value.media_type)
        {
            return Err(proposal_invalid());
        }
        let shape = serde_json::to_value(&value).map_err(|_| proposal_invalid())?;
        serde_json::from_value::<generated_contracts::CommonArtifactRef>(shape)
            .map_err(|_| proposal_invalid())?;
        Ok(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadHelpEvidenceClass {
    Authoritative,
    UserAsserted,
    Heuristic,
    Contextual,
    Unknown,
}

#[derive(Clone)]
pub struct BmadHelpEvidenceToken {
    token_id: ContractId,
    capability: BmadHelpActionKey,
    evidence_class: BmadHelpEvidenceClass,
    artifact_ref: BmadArtifactReference,
}

impl fmt::Debug for BmadHelpEvidenceToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BmadHelpEvidenceToken")
            .field("token_id", &self.token_id)
            .field("capability", &self.capability)
            .field("evidence_class", &self.evidence_class)
            .field("artifact_ref", &"<redacted>")
            .finish()
    }
}

impl BmadHelpEvidenceToken {
    /// Binds one opaque token ID to an exact host artifact and catalog action.
    ///
    /// # Errors
    ///
    /// Rejects `_meta` capabilities and unknown evidence classes.
    pub fn from_host_fact(
        token_id: ContractId,
        capability: BmadHelpActionKey,
        evidence_class: BmadHelpEvidenceClass,
        artifact_ref: BmadArtifactReference,
    ) -> Result<Self, BmadKernelError> {
        if capability.skill_name == "_meta" || evidence_class == BmadHelpEvidenceClass::Unknown {
            return Err(proposal_invalid());
        }
        Ok(Self {
            token_id,
            capability,
            evidence_class,
            artifact_ref,
        })
    }

    #[must_use]
    pub const fn token_id(&self) -> &ContractId {
        &self.token_id
    }

    pub(super) const fn capability(&self) -> &BmadHelpActionKey {
        &self.capability
    }

    pub(super) const fn evidence_class(&self) -> BmadHelpEvidenceClass {
        self.evidence_class
    }

    pub(super) const fn artifact_ref(&self) -> &BmadArtifactReference {
        &self.artifact_ref
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadContentReference {
    #[serde(rename = "ref")]
    pub ref_: String,
    pub content_hash: Sha256Digest,
    pub byte_length: u64,
    pub media_type: String,
}

impl BmadContentReference {
    pub(crate) fn verify_local_json(&self) -> Result<(), BmadKernelError> {
        let expected_ref = format!("cas://sha256/{}", self.content_hash.hex_value());
        if self.ref_ != expected_ref
            || self.byte_length > MAX_SAFE_JSON_INTEGER
            || self.media_type != JSON_MEDIA_TYPE
            || !safe_text(&self.ref_)
        {
            return Err(proposal_invalid());
        }
        let shape = serde_json::to_value(self).map_err(|_| proposal_invalid())?;
        serde_json::from_value::<generated_contracts::CommonContentRef>(shape)
            .map_err(|_| proposal_invalid())?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(
    tag = "recommendationKind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum BmadMethodHelpRecommendation {
    RecommendedCapability {
        recommendation_id: ContractId,
        session_id: ContractId,
        capability_key: BmadCapabilityKey,
        evidence_class: BmadHelpEvidenceClass,
        evidence_refs: Vec<BmadArtifactReference>,
        guidance_required: bool,
        rationale_summary: String,
        recommendation_hash: Sha256Digest,
        #[serde(serialize_with = "crate::ids::serialize_utc_instant")]
        created_at: UnixMillis,
    },
    NoRecommendation {
        recommendation_id: ContractId,
        session_id: ContractId,
        evidence_class: BmadHelpEvidenceClass,
        reason_code: BmadHelpNoRecommendationReason,
        recommendation_hash: Sha256Digest,
        #[serde(serialize_with = "crate::ids::serialize_utc_instant")]
        created_at: UnixMillis,
    },
}

impl BmadMethodHelpRecommendation {
    #[must_use]
    pub const fn recommendation_hash(&self) -> Sha256Digest {
        match self {
            Self::RecommendedCapability {
                recommendation_hash,
                ..
            }
            | Self::NoRecommendation {
                recommendation_hash,
                ..
            } => *recommendation_hash,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadHelpNoRecommendationReason {
    CatalogEvidenceAbsent,
    CompletionEvidenceAmbiguous,
    DependencyUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadCanonicalAdvanceResult {
    pub result_kind: String,
    pub result_id: ContractId,
    pub request_id: ContractId,
    pub invocation_id: ContractId,
    pub response_schema_hash: Sha256Digest,
    pub response_content_ref: BmadContentReference,
    pub produced_artifacts: Vec<BmadArtifactReference>,
    pub unresolved_open_item_count: u64,
    pub result_hash: Sha256Digest,
    #[serde(serialize_with = "crate::ids::serialize_utc_instant")]
    pub received_at: UnixMillis,
}

impl BmadCanonicalAdvanceResult {
    pub(crate) fn verify(&self) -> Result<(), BmadKernelError> {
        self.response_content_ref.verify_local_json()?;
        let value = serde_json::to_value(self).map_err(|_| proposal_invalid())?;
        let expected = crate::canonical_hash_without_field(
            "bmad-method-canonical-advance-result",
            1,
            &value,
            "resultHash",
        )
        .map_err(|_| proposal_invalid())?;
        if self.result_kind != "completion_candidate"
            || !self.produced_artifacts.is_empty()
            || self.unresolved_open_item_count != 0
            || self.result_hash != expected
        {
            return Err(proposal_invalid());
        }
        serde_json::from_value::<generated_contracts::MethodAdvanceResult>(value)
            .map_err(|_| proposal_invalid())?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct BmadHelpRecordIds {
    pub recommendation_id: ContractId,
    pub result_id: ContractId,
}

/// Opaque exact D2 output and already-verified receipt assertion.
///
/// This wrapper is intentionally not proof that production receipt
/// verification ran; the D2 composition gate remains responsible for that.
#[derive(Clone)]
pub struct BmadVerifiedHelpProposal {
    raw_bytes: Arc<[u8]>,
    receipt: MethodAdvanceReceipt,
    model_receipt_evidence_hash: Sha256Digest,
}

impl fmt::Debug for BmadVerifiedHelpProposal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BmadVerifiedHelpProposal")
            .field(
                "raw_bytes",
                &format_args!("<redacted:{} bytes>", self.raw_bytes.len()),
            )
            .field("model_request_id", &self.receipt.model_request_id)
            .finish_non_exhaustive()
    }
}

impl BmadVerifiedHelpProposal {
    /// Retains exact raw proposal bytes plus a trusted-host receipt assertion.
    ///
    /// # Errors
    ///
    /// Rejects empty or over-64-KiB payloads before parsing.
    pub fn from_trusted_host_evidence(
        raw_bytes: impl Into<Arc<[u8]>>,
        receipt: MethodAdvanceReceipt,
        model_receipt_evidence_hash: Sha256Digest,
    ) -> Result<Self, BmadKernelError> {
        let raw_bytes = raw_bytes.into();
        if raw_bytes.is_empty() || raw_bytes.len() > MAX_HELP_PROPOSAL_BYTES {
            return Err(proposal_invalid());
        }
        Ok(Self {
            raw_bytes,
            receipt,
            model_receipt_evidence_hash,
        })
    }
}

#[derive(Clone)]
pub struct BmadCanonicalHelpRecords {
    raw_proposal_bytes: Arc<[u8]>,
    model_response_payload_hash: Sha256Digest,
    recommendation: BmadMethodHelpRecommendation,
    recommendation_bytes: Arc<[u8]>,
    recommendation_content_ref: BmadContentReference,
    canonical_result: BmadCanonicalAdvanceResult,
    canonical_result_bytes: Arc<[u8]>,
    verified_result: MethodVerifiedAdvanceResult,
}

impl fmt::Debug for BmadCanonicalHelpRecords {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BmadCanonicalHelpRecords")
            .field("raw_proposal_bytes", &"<redacted>")
            .field("recommendation_bytes", &"<redacted>")
            .field("canonical_result_bytes", &"<redacted>")
            .field(
                "model_response_payload_hash",
                &self.model_response_payload_hash,
            )
            .finish_non_exhaustive()
    }
}

impl BmadCanonicalHelpRecords {
    #[must_use]
    pub fn raw_proposal_bytes(&self) -> &[u8] {
        &self.raw_proposal_bytes
    }

    #[must_use]
    pub const fn model_response_payload_hash(&self) -> Sha256Digest {
        self.model_response_payload_hash
    }

    #[must_use]
    pub const fn recommendation(&self) -> &BmadMethodHelpRecommendation {
        &self.recommendation
    }

    #[must_use]
    pub fn recommendation_bytes(&self) -> &[u8] {
        &self.recommendation_bytes
    }

    #[must_use]
    pub const fn recommendation_content_ref(&self) -> &BmadContentReference {
        &self.recommendation_content_ref
    }

    #[must_use]
    pub const fn canonical_result(&self) -> &BmadCanonicalAdvanceResult {
        &self.canonical_result
    }

    #[must_use]
    pub fn canonical_result_bytes(&self) -> &[u8] {
        &self.canonical_result_bytes
    }

    #[must_use]
    pub const fn verified_result(&self) -> &MethodVerifiedAdvanceResult {
        &self.verified_result
    }
}

pub struct BmadHelpMaterializer;

impl BmadHelpMaterializer {
    /// Strictly validates a verified proposal and materializes both canonical
    /// host records without mutating the Method aggregate.
    ///
    /// # Errors
    ///
    /// Rejects structural, semantic, catalog, token, hash, or lineage drift.
    pub fn materialize(
        compiled: &BmadCompiledHelpInvocation,
        session: &MethodSession,
        proposal: &BmadVerifiedHelpProposal,
        ids: BmadHelpRecordIds,
        now: UnixMillis,
    ) -> Result<BmadCanonicalHelpRecords, BmadKernelError> {
        let parsed = parse_proposal(&proposal.raw_bytes)?;
        let recommendation = materialize_recommendation(
            compiled,
            parsed,
            ids.recommendation_id,
            session.session_id(),
            now,
        )?;
        let recommendation_value =
            serde_json::to_value(&recommendation).map_err(|_| proposal_invalid())?;
        serde_json::from_value::<generated_contracts::MethodHelpRecommendation>(
            recommendation_value.clone(),
        )
        .map_err(|_| proposal_invalid())?;
        verify_recommendation_hash(&recommendation, &recommendation_value)?;
        let recommendation_bytes = Arc::<[u8]>::from(
            canonical_json_bytes(&recommendation_value).map_err(|_| proposal_invalid())?,
        );
        let recommendation_content_ref = content_ref(&recommendation_bytes)?;
        let canonical_result = seal_canonical_result(
            ids.result_id,
            proposal.receipt.model_request_id.clone(),
            proposal.receipt.invocation_id.clone(),
            compiled.recommendation_schema_closure_hash(),
            recommendation_content_ref.clone(),
            now,
        )?;
        canonical_result.verify()?;
        let canonical_result_value =
            serde_json::to_value(&canonical_result).map_err(|_| proposal_invalid())?;
        let canonical_result_bytes = Arc::<[u8]>::from(
            canonical_json_bytes(&canonical_result_value).map_err(|_| proposal_invalid())?,
        );
        let accepted_result = MethodAdvanceResult {
            disposition: MethodAdvanceDisposition::Completed,
            current_step_key: "recommend".to_owned(),
            next_step_key: None,
            working_artifact_refs: Vec::new(),
        };
        let binding = MethodVerifiedResultBindingData {
            invocation_id: proposal.receipt.invocation_id.clone(),
            decision_id: proposal.receipt.decision_id.clone(),
            decision_consumption_hash: proposal.receipt.decision_consumption_hash,
            model_request_id: proposal.receipt.model_request_id.clone(),
            model_request_hash: proposal.receipt.model_request_hash,
            session_authority_hash: proposal.receipt.session_authority_hash,
            d2_model_invocation_binding_hash: proposal.receipt.d2_model_invocation_binding_hash,
            model_bridge_binding_hash: proposal.receipt.model_bridge_binding_hash,
            method_binding_hash: compiled
                .exact_binding()
                .binding_hash()
                .map_err(|_| proposal_invalid())?,
            model_binding_hash: compiled.exact_binding().model_binding_hash,
            response_schema_hash: compiled.proposal_schema_closure_hash(),
            model_response_payload_hash: sha256_bytes(&proposal.raw_bytes),
            accepted_method_result_hash: canonical_hash(
                "bmad-method-advance-result",
                1,
                &accepted_result,
            )
            .map_err(|_| proposal_invalid())?,
            model_receipt_evidence_hash: proposal.model_receipt_evidence_hash,
            canonical_advance_result: Some(MethodCanonicalAdvanceResultData {
                recommendation_schema_hash: canonical_result.response_schema_hash,
                result_schema_hash: compiled.result_schema_closure_hash(),
                result_id: canonical_result.result_id.clone(),
                recommendation_content_ref: canonical_result.response_content_ref.clone(),
                received_at: canonical_result.received_at,
            }),
            canonical_advance_result_hash: Some(canonical_result.result_hash),
        };
        let verified_result =
            MethodVerifiedAdvanceResult::from_trusted_host_evidence(accepted_result, binding)
                .map_err(|_| proposal_invalid())?;
        session
            .validate_result(proposal.receipt.aggregate_version, &verified_result)
            .map_err(|_| proposal_invalid())?;

        Ok(BmadCanonicalHelpRecords {
            raw_proposal_bytes: Arc::clone(&proposal.raw_bytes),
            model_response_payload_hash: sha256_bytes(&proposal.raw_bytes),
            recommendation,
            recommendation_bytes,
            recommendation_content_ref,
            canonical_result,
            canonical_result_bytes,
            verified_result,
        })
    }
}

enum ParsedHelpProposal {
    Recommended {
        capability_key: BmadCapabilityKey,
        evidence_token_ids: Vec<ContractId>,
        rationale_summary: String,
    },
    None(BmadHelpNoRecommendationReason),
}

fn parse_proposal(bytes: &[u8]) -> Result<ParsedHelpProposal, BmadKernelError> {
    let value =
        crate::strict_json::parse_strict_json_value(bytes).map_err(|_| proposal_invalid())?;
    serde_json::from_value::<generated_contracts::MethodHelpProposal>(value.clone())
        .map_err(|_| proposal_invalid())?;
    let object = value.as_object().ok_or_else(proposal_invalid)?;
    match object.get("proposalKind").and_then(Value::as_str) {
        Some("recommended_capability") => {
            let capability = object
                .get("capabilityKey")
                .and_then(Value::as_object)
                .ok_or_else(proposal_invalid)?;
            let capability_key = BmadCapabilityKey {
                package_version_id: ContractId::new(required_string(
                    capability,
                    "packageVersionId",
                )?)
                .map_err(|_| proposal_invalid())?,
                module_code: required_string(capability, "moduleCode")?.to_owned(),
                skill_name: required_string(capability, "skillName")?.to_owned(),
                normalized_action: capability
                    .get("normalizedAction")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            };
            let ids = object
                .get("evidenceTokenIds")
                .and_then(Value::as_array)
                .ok_or_else(proposal_invalid)?
                .iter()
                .map(|value| {
                    ContractId::new(value.as_str().ok_or_else(proposal_invalid)?)
                        .map_err(|_| proposal_invalid())
                })
                .collect::<Result<Vec<_>, _>>()?;
            if ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
                return Err(proposal_invalid());
            }
            let rationale_summary = required_string(object, "rationaleSummary")?.to_owned();
            if !safe_text(&rationale_summary) {
                return Err(proposal_invalid());
            }
            Ok(ParsedHelpProposal::Recommended {
                capability_key,
                evidence_token_ids: ids,
                rationale_summary,
            })
        }
        Some("no_recommendation") => {
            let reason = match required_string(object, "reasonCode")? {
                "catalog_evidence_absent" => BmadHelpNoRecommendationReason::CatalogEvidenceAbsent,
                "completion_evidence_ambiguous" => {
                    BmadHelpNoRecommendationReason::CompletionEvidenceAmbiguous
                }
                "dependency_unavailable" => BmadHelpNoRecommendationReason::DependencyUnavailable,
                _ => return Err(proposal_invalid()),
            };
            Ok(ParsedHelpProposal::None(reason))
        }
        _ => Err(proposal_invalid()),
    }
}

fn materialize_recommendation(
    compiled: &BmadCompiledHelpInvocation,
    proposal: ParsedHelpProposal,
    recommendation_id: ContractId,
    session_id: ContractId,
    created_at: UnixMillis,
) -> Result<BmadMethodHelpRecommendation, BmadKernelError> {
    match proposal {
        ParsedHelpProposal::Recommended {
            capability_key,
            evidence_token_ids,
            rationale_summary,
        } => {
            let action = exact_action(compiled, &capability_key)?;
            let mut expected_tokens = compiled
                .evidence_tokens()
                .iter()
                .filter(|token| token_matches_capability(token, &capability_key))
                .collect::<Vec<_>>();
            expected_tokens.sort_by(|left, right| left.token_id().cmp(right.token_id()));
            let proposed_ids = evidence_token_ids.iter().collect::<BTreeSet<_>>();
            let expected_ids = expected_tokens
                .iter()
                .map(|token| token.token_id())
                .collect::<BTreeSet<_>>();
            if expected_tokens.is_empty() || proposed_ids != expected_ids {
                return Err(proposal_invalid());
            }
            let evidence_class = expected_tokens
                .iter()
                .map(|token| token.evidence_class())
                .max()
                .ok_or_else(proposal_invalid)?;
            let mut evidence_refs = expected_tokens
                .into_iter()
                .map(|token| token.artifact_ref().clone())
                .collect::<Vec<_>>();
            evidence_refs.sort();
            let draft = RecommendedDraft {
                recommendation_kind: "recommended_capability",
                recommendation_id: &recommendation_id,
                session_id: &session_id,
                capability_key: &capability_key,
                evidence_class,
                evidence_refs: &evidence_refs,
                guidance_required: action.required,
                rationale_summary: &rationale_summary,
                created_at,
            };
            let recommendation_hash = canonical_hash("bmad-method-help-recommendation", 1, &draft)
                .map_err(|_| proposal_invalid())?;
            Ok(BmadMethodHelpRecommendation::RecommendedCapability {
                recommendation_id,
                session_id,
                capability_key,
                evidence_class,
                evidence_refs,
                guidance_required: action.required,
                rationale_summary,
                recommendation_hash,
                created_at,
            })
        }
        ParsedHelpProposal::None(reason_code) => {
            if !no_recommendation_is_proven(compiled, reason_code) {
                return Err(proposal_invalid());
            }
            let draft = NoRecommendationDraft {
                recommendation_kind: "no_recommendation",
                recommendation_id: &recommendation_id,
                session_id: &session_id,
                evidence_class: BmadHelpEvidenceClass::Unknown,
                reason_code,
                created_at,
            };
            let recommendation_hash = canonical_hash("bmad-method-help-recommendation", 1, &draft)
                .map_err(|_| proposal_invalid())?;
            Ok(BmadMethodHelpRecommendation::NoRecommendation {
                recommendation_id,
                session_id,
                evidence_class: BmadHelpEvidenceClass::Unknown,
                reason_code,
                recommendation_hash,
                created_at,
            })
        }
    }
}

fn exact_action<'a>(
    compiled: &'a BmadCompiledHelpInvocation,
    key: &BmadCapabilityKey,
) -> Result<&'a BmadHelpAction, BmadKernelError> {
    compiled
        .catalog_candidates()
        .iter()
        .find(|action| {
            action.skill_name != "_meta"
                && action.key.package_version_id == key.package_version_id
                && action.key.module_code == key.module_code
                && action.key.skill_name == key.skill_name
                && action.key.action == key.normalized_action
        })
        .ok_or_else(proposal_invalid)
}

fn token_matches_capability(token: &BmadHelpEvidenceToken, key: &BmadCapabilityKey) -> bool {
    let capability = token.capability();
    capability.package_version_id == key.package_version_id
        && capability.module_code == key.module_code
        && capability.skill_name == key.skill_name
        && capability.action == key.normalized_action
}

fn no_recommendation_is_proven(
    compiled: &BmadCompiledHelpInvocation,
    reason: BmadHelpNoRecommendationReason,
) -> bool {
    match reason {
        BmadHelpNoRecommendationReason::CatalogEvidenceAbsent => {
            compiled.evidence_tokens().is_empty()
        }
        BmadHelpNoRecommendationReason::CompletionEvidenceAmbiguous => {
            compiled
                .evidence_tokens()
                .iter()
                .map(BmadHelpEvidenceToken::capability)
                .collect::<BTreeSet<_>>()
                .len()
                > 1
        }
        BmadHelpNoRecommendationReason::DependencyUnavailable => {
            let candidates = compiled
                .catalog_candidates()
                .iter()
                .filter(|action| action.skill_name != "_meta")
                .collect::<Vec<_>>();
            !candidates.is_empty()
                && candidates.iter().all(|action| {
                    matches!(
                        action.availability,
                        BmadCatalogAvailability::DependencyUnavailable
                            | BmadCatalogAvailability::OrphanSkill
                            | BmadCatalogAvailability::NetworkUnavailable
                            | BmadCatalogAvailability::SourcePromptUnavailable
                    )
                })
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecommendedDraft<'a> {
    recommendation_kind: &'static str,
    recommendation_id: &'a ContractId,
    session_id: &'a ContractId,
    capability_key: &'a BmadCapabilityKey,
    evidence_class: BmadHelpEvidenceClass,
    evidence_refs: &'a [BmadArtifactReference],
    guidance_required: bool,
    rationale_summary: &'a str,
    #[serde(serialize_with = "crate::ids::serialize_utc_instant")]
    created_at: UnixMillis,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NoRecommendationDraft<'a> {
    recommendation_kind: &'static str,
    recommendation_id: &'a ContractId,
    session_id: &'a ContractId,
    evidence_class: BmadHelpEvidenceClass,
    reason_code: BmadHelpNoRecommendationReason,
    #[serde(serialize_with = "crate::ids::serialize_utc_instant")]
    created_at: UnixMillis,
}

fn verify_recommendation_hash(
    recommendation: &BmadMethodHelpRecommendation,
    value: &Value,
) -> Result<(), BmadKernelError> {
    let expected = crate::canonical_hash_without_field(
        "bmad-method-help-recommendation",
        1,
        value,
        "recommendationHash",
    )
    .map_err(|_| proposal_invalid())?;
    if recommendation.recommendation_hash() != expected {
        return Err(proposal_invalid());
    }
    Ok(())
}

fn content_ref(bytes: &[u8]) -> Result<BmadContentReference, BmadKernelError> {
    let content_hash = sha256_bytes(bytes);
    let byte_length = u64::try_from(bytes.len()).map_err(|_| proposal_invalid())?;
    let value = BmadContentReference {
        ref_: format!("cas://sha256/{}", content_hash.hex_value()),
        content_hash,
        byte_length,
        media_type: JSON_MEDIA_TYPE.to_owned(),
    };
    value.verify_local_json()?;
    Ok(value)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalResultDraft<'a> {
    result_kind: &'static str,
    result_id: &'a ContractId,
    request_id: &'a ContractId,
    invocation_id: &'a ContractId,
    response_schema_hash: Sha256Digest,
    response_content_ref: &'a BmadContentReference,
    produced_artifacts: &'a [BmadArtifactReference],
    unresolved_open_item_count: u64,
    #[serde(serialize_with = "crate::ids::serialize_utc_instant")]
    received_at: UnixMillis,
}

fn seal_canonical_result(
    result_id: ContractId,
    request_id: ContractId,
    invocation_id: ContractId,
    response_schema_hash: Sha256Digest,
    response_content_ref: BmadContentReference,
    received_at: UnixMillis,
) -> Result<BmadCanonicalAdvanceResult, BmadKernelError> {
    let draft = CanonicalResultDraft {
        result_kind: "completion_candidate",
        result_id: &result_id,
        request_id: &request_id,
        invocation_id: &invocation_id,
        response_schema_hash,
        response_content_ref: &response_content_ref,
        produced_artifacts: &[],
        unresolved_open_item_count: 0,
        received_at,
    };
    let result_hash = canonical_hash("bmad-method-canonical-advance-result", 1, &draft)
        .map_err(|_| proposal_invalid())?;
    Ok(BmadCanonicalAdvanceResult {
        result_kind: "completion_candidate".to_owned(),
        result_id,
        request_id,
        invocation_id,
        response_schema_hash,
        response_content_ref,
        produced_artifacts: Vec::new(),
        unresolved_open_item_count: 0,
        result_hash,
        received_at,
    })
}

fn required_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    field: &str,
) -> Result<&'a str, BmadKernelError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(proposal_invalid)
}

fn safe_text(value: &str) -> bool {
    !value.chars().any(|character| {
        matches!(
            character,
            '\u{0000}'..='\u{001f}'
                | '\u{007f}'
                | '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        )
    })
}

const fn proposal_invalid() -> BmadKernelError {
    BmadKernelError::new(BmadKernelErrorCode::HelpProposalInvalid)
}
