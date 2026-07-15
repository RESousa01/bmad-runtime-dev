use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use crate::{
    canonical_hash, canonical_hash_without_field, canonical_json_bytes, sha256_bytes, AuthorityRef,
    ContractId, Sha256Digest,
};

const BUILDER_DRAFT_SCHEMA: &str = "sapphirus.bmad-builder-authoring.v1";
const BUILDER_REVISION_SCHEMA: &str = "sapphirus.bmad-builder-revision.v1";
const BUILDER_ANALYSIS_SCHEMA: &str = "sapphirus.bmad-builder-analysis.v1";
const BUILDER_LIMIT_PROFILE: &str = "sapphirus.bmad-builder-limits.v1";
const MAX_FILES: usize = 16;
const MAX_FILE_BYTES: usize = 262_144;
const MAX_TOTAL_BYTES: usize = 1_048_576;
const MAX_FINDINGS: usize = 512;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct BmadUtcInstant(String);

impl BmadUtcInstant {
    /// Preserves one canonical BMAD UTC instant.
    ///
    /// # Errors
    ///
    /// Returns `builder_payload_tampered` for a non-canonical instant.
    pub fn parse(value: impl Into<String>) -> Result<Self, BuilderError> {
        let value = Self(value.into());
        if value.valid() {
            Ok(value)
        } else {
            Err(BuilderError::new(BuilderErrorCode::BuilderPayloadTampered))
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn valid(&self) -> bool {
        let bytes = self.0.as_bytes();
        bytes.len() == 24
            && matches!(bytes.get(4), Some(b'-'))
            && matches!(bytes.get(7), Some(b'-'))
            && matches!(bytes.get(10), Some(b'T'))
            && matches!(bytes.get(13), Some(b':'))
            && matches!(bytes.get(16), Some(b':'))
            && matches!(bytes.get(19), Some(b'.'))
            && matches!(bytes.get(23), Some(b'Z'))
            && bytes.iter().enumerate().all(|(index, byte)| {
                matches!(index, 4 | 7 | 10 | 13 | 16 | 19 | 23) || byte.is_ascii_digit()
            })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BuilderKind {
    Agent,
    Workflow,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum BuilderValidationProfile {
    BuilderAgentV2Stateless,
    BuilderOutcomeSkillV2,
}

impl BuilderValidationProfile {
    const fn matches_kind(self, kind: BuilderKind) -> bool {
        matches!(
            (self, kind),
            (Self::BuilderAgentV2Stateless, BuilderKind::Agent)
                | (Self::BuilderOutcomeSkillV2, BuilderKind::Workflow)
        )
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BuilderActionName {
    CreateRebuild,
    Build,
    Edit,
    Analyze,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuilderAuthoringAction {
    pub builder_kind: BuilderKind,
    pub action: BuilderActionName,
}

impl BuilderAuthoringAction {
    #[must_use]
    pub const fn agent_create_rebuild() -> Self {
        Self {
            builder_kind: BuilderKind::Agent,
            action: BuilderActionName::CreateRebuild,
        }
    }

    #[must_use]
    pub const fn workflow_build() -> Self {
        Self {
            builder_kind: BuilderKind::Workflow,
            action: BuilderActionName::Build,
        }
    }

    #[must_use]
    pub const fn edit(kind: BuilderKind) -> Self {
        Self {
            builder_kind: kind,
            action: BuilderActionName::Edit,
        }
    }

    #[must_use]
    pub const fn analyze(kind: BuilderKind) -> Self {
        Self {
            builder_kind: kind,
            action: BuilderActionName::Analyze,
        }
    }

    const fn valid_for_draft(&self, kind: BuilderKind) -> bool {
        matches!(
            (kind, self.builder_kind, self.action),
            (
                BuilderKind::Agent,
                BuilderKind::Agent,
                BuilderActionName::CreateRebuild
            ) | (
                BuilderKind::Workflow,
                BuilderKind::Workflow,
                BuilderActionName::Build
            )
        )
    }

    fn valid_for_revision(&self, kind: BuilderKind, ordinal: u64) -> bool {
        if self.builder_kind != kind {
            return false;
        }
        matches!(
            (kind, self.action, ordinal),
            (BuilderKind::Agent, BuilderActionName::CreateRebuild, 1)
                | (BuilderKind::Workflow, BuilderActionName::Build, 1)
                | (
                    BuilderKind::Agent | BuilderKind::Workflow,
                    BuilderActionName::Edit,
                    2..
                )
        )
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuilderDraftRecord {
    pub object_kind: String,
    pub schema_version: String,
    pub draft_id: ContractId,
    pub owner_scope_ref: ContractId,
    pub project_id: ContractId,
    pub authoring_session_id: ContractId,
    pub builder_kind: BuilderKind,
    pub validation_profile: BuilderValidationProfile,
    pub authoring_action: BuilderAuthoringAction,
    pub source_identity_hash: Sha256Digest,
    pub instruction_projection_set_hash: Sha256Digest,
    pub created_at: BmadUtcInstant,
    pub draft_effect: String,
}

impl BuilderDraftRecord {
    fn validate(&self) -> Result<(), BuilderError> {
        if self.object_kind != "draft"
            || self.schema_version != BUILDER_DRAFT_SCHEMA
            || self.draft_effect != "none"
            || !self.created_at.valid()
            || !self.validation_profile.matches_kind(self.builder_kind)
            || !self.authoring_action.valid_for_draft(self.builder_kind)
        {
            return Err(BuilderError::new(
                BuilderErrorCode::BuilderActionInvalidForKind,
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuilderProposedFile {
    pub path: String,
    pub content: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuilderProposedFileSet {
    pub limit_profile: String,
    pub files: Vec<BuilderProposedFile>,
}

impl BuilderProposedFileSet {
    fn validate(&self, kind: BuilderKind) -> Result<(), BuilderError> {
        if self.limit_profile != BUILDER_LIMIT_PROFILE
            || self.files.is_empty()
            || self.files.len() > MAX_FILES
        {
            return Err(BuilderError::new(
                BuilderErrorCode::BuilderFutureCapabilityForbidden,
            ));
        }
        let mut total_bytes = 0_usize;
        let mut paths = BTreeSet::new();
        let mut folded_paths = BTreeSet::new();
        for file in &self.files {
            validate_builder_path(&file.path)?;
            let folded = file.path.to_lowercase();
            if !paths.insert(file.path.as_str()) || !folded_paths.insert(folded) {
                return Err(BuilderError::new(BuilderErrorCode::BuilderRevisionStale));
            }
            let bytes = file.content.len();
            if bytes > MAX_FILE_BYTES {
                return Err(BuilderError::new(
                    BuilderErrorCode::BuilderFutureCapabilityForbidden,
                ));
            }
            total_bytes = total_bytes.checked_add(bytes).ok_or_else(|| {
                BuilderError::new(BuilderErrorCode::BuilderFutureCapabilityForbidden)
            })?;
        }
        if total_bytes > MAX_TOTAL_BYTES || !valid_inventory(kind, &paths) {
            return Err(BuilderError::new(
                BuilderErrorCode::BuilderFutureCapabilityForbidden,
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuilderDraftRevision {
    pub object_kind: String,
    pub schema_version: String,
    pub revision_id: ContractId,
    pub draft_id: ContractId,
    pub builder_kind: BuilderKind,
    pub validation_profile: BuilderValidationProfile,
    pub authoring_action: BuilderAuthoringAction,
    pub ordinal: u64,
    pub parent_revision_hash: Option<Sha256Digest>,
    pub proposed_file_set: BuilderProposedFileSet,
    pub source_identity_hash: Sha256Digest,
    pub instruction_projection_set_hash: Sha256Digest,
    pub raw_result_hash: Sha256Digest,
    pub inventory_hash: Sha256Digest,
    pub created_at: BmadUtcInstant,
    pub revision_hash: Sha256Digest,
}

impl BuilderDraftRevision {
    /// Computes the host-owned deterministic hash of the exact proposed file inventory.
    ///
    /// # Errors
    ///
    /// Returns `builder_payload_tampered` if canonical serialization fails.
    pub fn host_inventory_hash(&self) -> Result<Sha256Digest, BuilderError> {
        Ok(canonical_hash(
            "bmad-builder-file-inventory",
            1,
            &self.proposed_file_set,
        )?)
    }

    fn validate(&self) -> Result<(), BuilderError> {
        if self.object_kind != "revision"
            || self.schema_version != BUILDER_REVISION_SCHEMA
            || self.ordinal == 0
            || !self.created_at.valid()
            || !self.validation_profile.matches_kind(self.builder_kind)
            || !self
                .authoring_action
                .valid_for_revision(self.builder_kind, self.ordinal)
        {
            return Err(BuilderError::new(
                BuilderErrorCode::BuilderActionInvalidForKind,
            ));
        }
        self.proposed_file_set.validate(self.builder_kind)?;
        let actual =
            canonical_hash_without_field("bmad-builder-revision", 1, self, "revisionHash")?;
        if actual != self.revision_hash {
            return Err(BuilderError::new(BuilderErrorCode::BuilderPayloadTampered));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BuilderFindingSeverity {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuilderDeterministicFinding {
    pub finding_id: ContractId,
    pub rule_id: String,
    pub severity: BuilderFindingSeverity,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relative_path: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BuilderModelLens {
    Leanness,
    Architecture,
    Determinism,
    Customization,
    Enhancement,
    AgentCohesion,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BuilderLensVerdict {
    Clear,
    FindingsPresent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuilderModelFinding {
    pub finding_id: ContractId,
    pub severity: BuilderFindingSeverity,
    pub title: String,
    pub location: String,
    pub evidence: String,
    pub recommendation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposed_smallest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub predicted_delta: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuilderAnalysisModelBinding {
    pub model_hash: Sha256Digest,
    pub deployment_hash: Sha256Digest,
    pub model_profile_hash: Sha256Digest,
    pub schema_hash: Sha256Digest,
    pub consent_hash: Sha256Digest,
    pub context_decision_id: ContractId,
    pub context_decision_consumption_hash: Sha256Digest,
    pub invocation_id: ContractId,
    pub result_hash: Sha256Digest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuilderModelAnalysisDecisionInput {
    pub decision_id: ContractId,
    pub invocation_id: ContractId,
    pub source_member_set_hash: Sha256Digest,
    pub deterministic_facts_hash: Sha256Digest,
    pub model_hash: Sha256Digest,
    pub deployment_hash: Sha256Digest,
    pub model_profile_hash: Sha256Digest,
    pub schema_hash: Sha256Digest,
    pub consent_hash: Sha256Digest,
    pub reviewed_at: BmadUtcInstant,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuilderAnalysisContextDecision {
    pub decision_id: ContractId,
    pub invocation_id: ContractId,
    pub draft_id: ContractId,
    pub revision_id: ContractId,
    pub revision_hash: Sha256Digest,
    pub scope_hash: Sha256Digest,
    pub source_member_set_hash: Sha256Digest,
    pub instruction_projection_set_hash: Sha256Digest,
    pub deterministic_facts_hash: Sha256Digest,
    pub model_hash: Sha256Digest,
    pub deployment_hash: Sha256Digest,
    pub model_profile_hash: Sha256Digest,
    pub schema_hash: Sha256Digest,
    pub consent_hash: Sha256Digest,
    pub reviewed_at: BmadUtcInstant,
    pub decision_hash: Sha256Digest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuilderAnalysisDecisionConsumption {
    pub consumption_id: ContractId,
    pub decision_id: ContractId,
    pub invocation_id: ContractId,
    pub draft_id: ContractId,
    pub revision_id: ContractId,
    pub analysis_id: ContractId,
    pub decision_hash: Sha256Digest,
    pub consumed_at: BmadUtcInstant,
    pub consumption_hash: Sha256Digest,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BuilderAnalysisDecisionInvalidationReason {
    RevisionChanged,
    RevisionSuperseded,
    AcceptedForReview,
    DraftBlocked,
    DraftAbandoned,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuilderAnalysisDecisionInvalidation {
    pub decision_id: ContractId,
    pub invocation_id: ContractId,
    pub draft_id: ContractId,
    pub revision_id: ContractId,
    pub decision_hash: Sha256Digest,
    pub reason: BuilderAnalysisDecisionInvalidationReason,
    pub aggregate_version: u64,
    pub invalidation_hash: Sha256Digest,
}

impl BuilderAnalysisContextDecision {
    /// Verifies the immutable host decision self-hash and canonical review time.
    ///
    /// # Errors
    ///
    /// Returns `builder_payload_tampered` when retained decision evidence drifted.
    pub fn validate_integrity(&self) -> Result<(), BuilderError> {
        let actual = canonical_hash_without_field(
            "bmad-builder-analysis-context-decision",
            1,
            self,
            "decisionHash",
        )?;
        if !self.reviewed_at.valid() || actual != self.decision_hash {
            return Err(BuilderError::new(BuilderErrorCode::BuilderPayloadTampered));
        }
        Ok(())
    }
}

impl BuilderAnalysisDecisionConsumption {
    /// Verifies the immutable host consumption receipt self-hash and canonical time.
    ///
    /// # Errors
    ///
    /// Returns `builder_payload_tampered` when retained consumption evidence drifted.
    pub fn validate_integrity(&self) -> Result<(), BuilderError> {
        let actual = canonical_hash_without_field(
            "bmad-builder-analysis-decision-consumption",
            1,
            self,
            "consumptionHash",
        )?;
        if !self.consumed_at.valid() || actual != self.consumption_hash {
            return Err(BuilderError::new(BuilderErrorCode::BuilderPayloadTampered));
        }
        Ok(())
    }
}

impl BuilderAnalysisDecisionInvalidation {
    /// Verifies the immutable host invalidation receipt self-hash.
    ///
    /// # Errors
    ///
    /// Returns `builder_payload_tampered` when retained invalidation evidence drifted.
    pub fn validate_integrity(&self) -> Result<(), BuilderError> {
        let actual = canonical_hash_without_field(
            "bmad-builder-analysis-decision-invalidation",
            1,
            self,
            "invalidationHash",
        )?;
        if self.aggregate_version == 0 || actual != self.invalidation_hash {
            return Err(BuilderError::new(BuilderErrorCode::BuilderPayloadTampered));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuilderModelLensResult {
    pub builder_kind: BuilderKind,
    pub lens: BuilderModelLens,
    pub revision_id: ContractId,
    pub revision_hash: Sha256Digest,
    pub source_member_set_hash: Sha256Digest,
    pub instruction_projection_set_hash: Sha256Digest,
    pub deterministic_facts_hash: Sha256Digest,
    pub model_hash: Sha256Digest,
    pub deployment_hash: Sha256Digest,
    pub model_profile_hash: Sha256Digest,
    pub schema_hash: Sha256Digest,
    pub consent_hash: Sha256Digest,
    pub context_decision_consumption_hash: Sha256Digest,
    pub verdict: BuilderLensVerdict,
    pub evaluation_claim: String,
    pub findings: Vec<BuilderModelFinding>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BuilderAnalysisKind {
    DeterministicStatic,
    ModelLens,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BuilderModelLensesNotPerformedReason {
    NotRequested,
    ConsentAbsent,
    ConnectivityAbsent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuilderAnalysisRun {
    pub object_kind: String,
    pub schema_version: String,
    pub analysis_id: ContractId,
    pub draft_id: ContractId,
    pub revision_id: ContractId,
    pub revision_hash: Sha256Digest,
    pub builder_kind: BuilderKind,
    pub validation_profile: BuilderValidationProfile,
    pub analysis_kind: BuilderAnalysisKind,
    pub source_member_set_hash: Sha256Digest,
    pub instruction_projection_set_hash: Sha256Digest,
    pub deterministic_facts_hash: Sha256Digest,
    pub deterministic_findings: Vec<BuilderDeterministicFinding>,
    pub model_lenses_performed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_lenses_not_performed_reason: Option<BuilderModelLensesNotPerformedReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_binding: Option<BuilderAnalysisModelBinding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_lens_results: Option<Vec<BuilderModelLensResult>>,
    pub evaluation_claim: String,
    pub created_at: BmadUtcInstant,
    pub analysis_hash: Sha256Digest,
}

impl BuilderAnalysisRun {
    fn validate(&self) -> Result<(), BuilderError> {
        if self.object_kind != "analysis"
            || self.schema_version != BUILDER_ANALYSIS_SCHEMA
            || self.evaluation_claim != "none"
            || !self.created_at.valid()
            || !self.validation_profile.matches_kind(self.builder_kind)
            || self.deterministic_findings.len() > MAX_FINDINGS
            || self.deterministic_findings.iter().any(|finding| {
                !valid_rule_id(&finding.rule_id)
                    || !valid_text(&finding.message, 4_096)
                    || finding
                        .relative_path
                        .as_deref()
                        .is_some_and(|path| validate_builder_path(path).is_err())
            })
        {
            return Err(BuilderError::new(
                BuilderErrorCode::BuilderAnalysisNotEvaluation,
            ));
        }
        match self.analysis_kind {
            BuilderAnalysisKind::DeterministicStatic => {
                if self.model_lenses_performed
                    || self.model_lenses_not_performed_reason.is_none()
                    || self.model_binding.is_some()
                    || self.model_lens_results.is_some()
                {
                    return Err(BuilderError::new(
                        BuilderErrorCode::BuilderAnalysisNotEvaluation,
                    ));
                }
            }
            BuilderAnalysisKind::ModelLens => self.validate_model_lenses()?,
        }
        let actual =
            canonical_hash_without_field("bmad-builder-analysis", 1, self, "analysisHash")?;
        if actual != self.analysis_hash {
            return Err(BuilderError::new(BuilderErrorCode::BuilderPayloadTampered));
        }
        Ok(())
    }

    fn validate_model_lenses(&self) -> Result<(), BuilderError> {
        let binding = self
            .model_binding
            .as_ref()
            .ok_or_else(|| BuilderError::new(BuilderErrorCode::BuilderAnalysisNotEvaluation))?;
        let results = self
            .model_lens_results
            .as_ref()
            .ok_or_else(|| BuilderError::new(BuilderErrorCode::BuilderAnalysisNotEvaluation))?;
        let expected = match self.builder_kind {
            BuilderKind::Agent => &[
                BuilderModelLens::Leanness,
                BuilderModelLens::Architecture,
                BuilderModelLens::Determinism,
                BuilderModelLens::Customization,
                BuilderModelLens::Enhancement,
                BuilderModelLens::AgentCohesion,
            ][..],
            BuilderKind::Workflow => &[
                BuilderModelLens::Leanness,
                BuilderModelLens::Architecture,
                BuilderModelLens::Determinism,
                BuilderModelLens::Customization,
                BuilderModelLens::Enhancement,
            ][..],
        };
        let total_findings = results
            .iter()
            .try_fold(self.deterministic_findings.len(), |total, result| {
                total.checked_add(result.findings.len())
            });
        if !self.model_lenses_performed
            || self.model_lenses_not_performed_reason.is_some()
            || results.len() != expected.len()
            || total_findings.is_none_or(|total| total > MAX_FINDINGS)
        {
            return Err(BuilderError::new(
                BuilderErrorCode::BuilderAnalysisNotEvaluation,
            ));
        }
        for (result, expected_lens) in results.iter().zip(expected) {
            if result.builder_kind != self.builder_kind
                || result.lens != *expected_lens
                || result.revision_id != self.revision_id
                || result.revision_hash != self.revision_hash
                || result.source_member_set_hash != self.source_member_set_hash
                || result.instruction_projection_set_hash != self.instruction_projection_set_hash
                || result.deterministic_facts_hash != self.deterministic_facts_hash
                || result.model_hash != binding.model_hash
                || result.deployment_hash != binding.deployment_hash
                || result.model_profile_hash != binding.model_profile_hash
                || result.schema_hash != binding.schema_hash
                || result.consent_hash != binding.consent_hash
                || result.context_decision_consumption_hash
                    != binding.context_decision_consumption_hash
                || result.evaluation_claim != "none"
                || result.findings.iter().any(|finding| !finding.valid())
            {
                return Err(BuilderError::new(
                    BuilderErrorCode::BuilderAnalysisNotEvaluation,
                ));
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn model_binding(&self) -> Option<&BuilderAnalysisModelBinding> {
        self.model_binding.as_ref()
    }
}

impl BuilderModelFinding {
    fn valid(&self) -> bool {
        valid_text(&self.title, 240)
            && valid_text(&self.location, 512)
            && valid_text(&self.evidence, 4_096)
            && valid_text(&self.recommendation, 4_096)
            && self
                .proposed_smallest
                .as_deref()
                .is_none_or(|value| valid_text(value, 4_096))
            && self
                .predicted_delta
                .as_deref()
                .is_none_or(|value| valid_text(value, 1_024))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuilderDraftScope {
    pub owner_scope_ref: ContractId,
    pub project_id: ContractId,
    pub authoring_session_id: ContractId,
    pub authority_ref: AuthorityRef,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuilderRendererProjection {
    pub draft_id: ContractId,
    pub builder_kind: BuilderKind,
    pub state: BuilderDraftState,
    pub version: u64,
    pub current_revision_id: Option<ContractId>,
    pub current_revision_hash: Option<Sha256Digest>,
    pub revision_count: u64,
    pub analysis_count: u64,
    pub draft_effect: String,
    pub evaluation_capability: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BuilderDraftState {
    Drafting,
    DraftReady,
    Analyzed,
    UserAccepted,
    Blocked,
    Abandoned,
    Superseded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuilderPersistenceEvent {
    RevisionAppended,
    AnalysisDecisionIssued,
    AnalysisRecorded,
    RevisionSuperseded,
    AcceptedForReview,
    Blocked,
    Abandoned,
}

impl BuilderPersistenceEvent {
    #[must_use]
    pub const fn event_type(self) -> &'static str {
        match self {
            Self::RevisionAppended => "bmad.builder.revision_appended",
            Self::AnalysisDecisionIssued => "bmad.builder.analysis_decision_issued",
            Self::AnalysisRecorded => "bmad.builder.analysis_recorded",
            Self::RevisionSuperseded => "bmad.builder.revision_superseded",
            Self::AcceptedForReview => "bmad.builder.accepted_for_review",
            Self::Blocked => "bmad.builder.blocked",
            Self::Abandoned => "bmad.builder.abandoned",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuilderCapabilityFact {
    EvaluationUnavailable,
    BuilderTargetDeferred,
}

impl BuilderCapabilityFact {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EvaluationUnavailable => "evaluation_unavailable",
            Self::BuilderTargetDeferred => "builder_target_deferred",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuilderDraft {
    record: BuilderDraftRecord,
    authority_ref: Option<AuthorityRef>,
    state: BuilderDraftState,
    version: u64,
    revisions: Vec<BuilderDraftRevision>,
    analyses: Vec<BuilderAnalysisRun>,
    #[serde(default)]
    pending_analysis_decision: Option<BuilderAnalysisContextDecision>,
    #[serde(default)]
    analysis_consumptions: Vec<BuilderAnalysisDecisionConsumption>,
    #[serde(default)]
    analysis_decision_invalidations: Vec<BuilderAnalysisDecisionInvalidation>,
}

impl BuilderDraft {
    /// Creates an inactive app-local Builder draft from an exact source record.
    ///
    /// # Errors
    ///
    /// Returns a closed Builder error for an invalid profile, action, or effect.
    pub fn create(record: BuilderDraftRecord) -> Result<Self, BuilderError> {
        record.validate()?;
        Ok(Self {
            record,
            authority_ref: None,
            state: BuilderDraftState::Drafting,
            version: 1,
            revisions: Vec::new(),
            analyses: Vec::new(),
            pending_analysis_decision: None,
            analysis_consumptions: Vec::new(),
            analysis_decision_invalidations: Vec::new(),
        })
    }

    /// Binds the host authority used for owner-scoped persistence.
    ///
    /// # Errors
    ///
    /// Returns a conflict if authority was already bound to a different value.
    pub fn bind_authority(&mut self, authority_ref: AuthorityRef) -> Result<(), BuilderError> {
        if self
            .authority_ref
            .as_ref()
            .is_some_and(|current| current != &authority_ref)
        {
            return Err(BuilderError::new(BuilderErrorCode::BuilderRevisionStale));
        }
        self.authority_ref = Some(authority_ref);
        Ok(())
    }

    #[must_use]
    pub const fn record(&self) -> &BuilderDraftRecord {
        &self.record
    }

    #[must_use]
    pub const fn state(&self) -> BuilderDraftState {
        self.state
    }

    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    #[must_use]
    pub fn scope(&self) -> Option<BuilderDraftScope> {
        self.authority_ref
            .as_ref()
            .map(|authority_ref| BuilderDraftScope {
                owner_scope_ref: self.record.owner_scope_ref.clone(),
                project_id: self.record.project_id.clone(),
                authoring_session_id: self.record.authoring_session_id.clone(),
                authority_ref: authority_ref.clone(),
            })
    }

    #[must_use]
    pub fn current_revision(&self) -> Option<&BuilderDraftRevision> {
        self.revisions.last()
    }

    #[must_use]
    pub fn revisions(&self) -> &[BuilderDraftRevision] {
        &self.revisions
    }

    #[must_use]
    pub fn analyses(&self) -> &[BuilderAnalysisRun] {
        &self.analyses
    }

    #[must_use]
    pub const fn pending_analysis_decision(&self) -> Option<&BuilderAnalysisContextDecision> {
        self.pending_analysis_decision.as_ref()
    }

    #[must_use]
    pub fn analysis_consumptions(&self) -> &[BuilderAnalysisDecisionConsumption] {
        &self.analysis_consumptions
    }

    #[must_use]
    pub fn analysis_decision_invalidations(&self) -> &[BuilderAnalysisDecisionInvalidation] {
        &self.analysis_decision_invalidations
    }

    #[must_use]
    pub fn renderer_projection(&self) -> BuilderRendererProjection {
        BuilderRendererProjection {
            draft_id: self.record.draft_id.clone(),
            builder_kind: self.record.builder_kind,
            state: self.state,
            version: self.version,
            current_revision_id: self
                .current_revision()
                .map(|revision| revision.revision_id.clone()),
            current_revision_hash: self
                .current_revision()
                .map(|revision| revision.revision_hash),
            revision_count: u64::try_from(self.revisions.len()).unwrap_or(u64::MAX),
            analysis_count: u64::try_from(self.analyses.len()).unwrap_or(u64::MAX),
            draft_effect: "none".to_owned(),
            evaluation_capability: BuilderCapabilityFact::EvaluationUnavailable
                .as_str()
                .to_owned(),
        }
    }

    /// Appends a new immutable file-set revision.
    ///
    /// # Errors
    ///
    /// Returns an action, profile, identity, or optimistic-version error.
    pub fn append_revision(
        &mut self,
        expected_version: u64,
        revision: BuilderDraftRevision,
    ) -> Result<(), BuilderError> {
        self.require_mutable(expected_version)?;
        revision.validate()?;
        let expected_ordinal = u64::try_from(self.revisions.len())
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| BuilderError::new(BuilderErrorCode::BuilderRevisionStale))?;
        let expected_parent = self.current_revision().map(|value| value.revision_hash);
        if revision.draft_id != self.record.draft_id
            || revision.builder_kind != self.record.builder_kind
            || revision.validation_profile != self.record.validation_profile
            || revision.source_identity_hash != self.record.source_identity_hash
            || revision.instruction_projection_set_hash
                != self.record.instruction_projection_set_hash
            || revision.ordinal != expected_ordinal
            || revision.parent_revision_hash != expected_parent
        {
            return Err(BuilderError::new(BuilderErrorCode::BuilderRevisionStale));
        }
        self.invalidate_pending_analysis_decision(
            BuilderAnalysisDecisionInvalidationReason::RevisionChanged,
        )?;
        self.revisions.push(revision);
        self.state = BuilderDraftState::DraftReady;
        self.advance_version()
    }

    /// Persists one host-reviewed model-analysis decision for the exact current revision.
    /// The decision grants no evaluation, registration, or execution capability.
    ///
    /// # Errors
    ///
    /// Returns a stable decision error for missing authority, replay, or revision drift.
    pub fn issue_model_analysis_decision(
        &mut self,
        expected_version: u64,
        input: BuilderModelAnalysisDecisionInput,
    ) -> Result<BuilderAnalysisContextDecision, BuilderError> {
        self.require_state(
            expected_version,
            &[BuilderDraftState::DraftReady, BuilderDraftState::Analyzed],
        )?;
        if !input.reviewed_at.valid()
            || self.pending_analysis_decision.is_some()
            || self.analysis_consumptions.iter().any(|consumption| {
                consumption.decision_id == input.decision_id
                    || consumption.invocation_id == input.invocation_id
            })
            || self
                .analysis_decision_invalidations
                .iter()
                .any(|invalidation| {
                    invalidation.decision_id == input.decision_id
                        || invalidation.invocation_id == input.invocation_id
                })
        {
            return Err(BuilderError::new(
                BuilderErrorCode::BuilderContextDecisionInvalid,
            ));
        }
        let revision = self
            .current_revision()
            .ok_or_else(|| BuilderError::new(BuilderErrorCode::BuilderRevisionStale))?;
        let scope = self
            .scope()
            .ok_or_else(|| BuilderError::new(BuilderErrorCode::BuilderContextDecisionInvalid))?;
        let scope_hash = canonical_hash("bmad-builder-draft-scope", 1, &scope)?;
        let mut decision = BuilderAnalysisContextDecision {
            decision_id: input.decision_id,
            invocation_id: input.invocation_id,
            draft_id: self.record.draft_id.clone(),
            revision_id: revision.revision_id.clone(),
            revision_hash: revision.revision_hash,
            scope_hash,
            source_member_set_hash: input.source_member_set_hash,
            instruction_projection_set_hash: self.record.instruction_projection_set_hash,
            deterministic_facts_hash: input.deterministic_facts_hash,
            model_hash: input.model_hash,
            deployment_hash: input.deployment_hash,
            model_profile_hash: input.model_profile_hash,
            schema_hash: input.schema_hash,
            consent_hash: input.consent_hash,
            reviewed_at: input.reviewed_at,
            decision_hash: sha256_bytes(b"pending-builder-analysis-decision"),
        };
        decision.decision_hash = canonical_hash_without_field(
            "bmad-builder-analysis-context-decision",
            1,
            &decision,
            "decisionHash",
        )?;
        self.pending_analysis_decision = Some(decision.clone());
        self.advance_version()?;
        Ok(decision)
    }

    /// Appends analysis evidence for the exact current revision without changing files.
    ///
    /// # Errors
    ///
    /// Returns a stale-revision, evaluation-smuggling, replay, or version error.
    pub fn record_analysis(
        &mut self,
        expected_version: u64,
        mut analysis: BuilderAnalysisRun,
    ) -> Result<(), BuilderError> {
        self.require_state(
            expected_version,
            &[BuilderDraftState::DraftReady, BuilderDraftState::Analyzed],
        )?;
        let revision = self
            .current_revision()
            .ok_or_else(|| BuilderError::new(BuilderErrorCode::BuilderRevisionStale))?;
        if analysis.draft_id != self.record.draft_id
            || analysis.revision_id != revision.revision_id
            || analysis.revision_hash != revision.revision_hash
            || analysis.builder_kind != self.record.builder_kind
            || analysis.validation_profile != self.record.validation_profile
            || analysis.instruction_projection_set_hash
                != self.record.instruction_projection_set_hash
            || self
                .analyses
                .iter()
                .any(|existing| existing.analysis_id == analysis.analysis_id)
        {
            return Err(BuilderError::new(BuilderErrorCode::BuilderRevisionStale));
        }
        let consumption = match analysis.analysis_kind {
            BuilderAnalysisKind::DeterministicStatic => None,
            BuilderAnalysisKind::ModelLens => Some(self.authorize_model_analysis(&mut analysis)?),
        };
        analysis.validate()?;
        self.analyses.push(analysis);
        if let Some(consumption) = consumption {
            self.pending_analysis_decision = None;
            self.analysis_consumptions.push(consumption);
        }
        self.state = BuilderDraftState::Analyzed;
        self.advance_version()
    }

    fn authorize_model_analysis(
        &self,
        analysis: &mut BuilderAnalysisRun,
    ) -> Result<BuilderAnalysisDecisionConsumption, BuilderError> {
        let decision = self
            .pending_analysis_decision
            .as_ref()
            .ok_or_else(|| BuilderError::new(BuilderErrorCode::BuilderContextDecisionMissing))?;
        let binding = analysis
            .model_binding
            .as_ref()
            .ok_or_else(|| BuilderError::new(BuilderErrorCode::BuilderContextDecisionInvalid))?;
        let scope = self
            .scope()
            .ok_or_else(|| BuilderError::new(BuilderErrorCode::BuilderContextDecisionInvalid))?;
        if decision.scope_hash != canonical_hash("bmad-builder-draft-scope", 1, &scope)?
            || decision.draft_id != analysis.draft_id
            || decision.revision_id != analysis.revision_id
            || decision.revision_hash != analysis.revision_hash
            || decision.source_member_set_hash != analysis.source_member_set_hash
            || decision.instruction_projection_set_hash != analysis.instruction_projection_set_hash
            || decision.deterministic_facts_hash != analysis.deterministic_facts_hash
            || decision.decision_id != binding.context_decision_id
            || decision.invocation_id != binding.invocation_id
            || decision.model_hash != binding.model_hash
            || decision.deployment_hash != binding.deployment_hash
            || decision.model_profile_hash != binding.model_profile_hash
            || decision.schema_hash != binding.schema_hash
            || decision.consent_hash != binding.consent_hash
            || analysis.created_at.as_str() < decision.reviewed_at.as_str()
        {
            return Err(BuilderError::new(
                BuilderErrorCode::BuilderContextDecisionInvalid,
            ));
        }
        let consumption_seed = canonical_hash(
            "bmad-builder-analysis-consumption-id",
            1,
            &(
                &decision.decision_hash,
                &decision.draft_id,
                &decision.revision_id,
                &analysis.analysis_id,
                &decision.invocation_id,
            ),
        )?;
        let consumption_id = ContractId::new(format!(
            "consume_{}",
            consumption_seed.to_string().trim_start_matches("sha256:")
        ))
        .map_err(|_| BuilderError::new(BuilderErrorCode::BuilderContextDecisionInvalid))?;
        let mut consumption = BuilderAnalysisDecisionConsumption {
            consumption_id,
            decision_id: decision.decision_id.clone(),
            invocation_id: decision.invocation_id.clone(),
            draft_id: decision.draft_id.clone(),
            revision_id: decision.revision_id.clone(),
            analysis_id: analysis.analysis_id.clone(),
            decision_hash: decision.decision_hash,
            consumed_at: analysis.created_at.clone(),
            consumption_hash: sha256_bytes(b"pending-builder-analysis-consumption"),
        };
        consumption.consumption_hash = canonical_hash_without_field(
            "bmad-builder-analysis-decision-consumption",
            1,
            &consumption,
            "consumptionHash",
        )?;
        if let Some(binding) = analysis.model_binding.as_mut() {
            binding.context_decision_consumption_hash = consumption.consumption_hash;
        }
        if let Some(results) = analysis.model_lens_results.as_mut() {
            for result in results {
                result.context_decision_consumption_hash = consumption.consumption_hash;
            }
        }
        analysis.analysis_hash =
            canonical_hash_without_field("bmad-builder-analysis", 1, analysis, "analysisHash")?;
        Ok(consumption)
    }

    /// Marks the current inactive revision as superseded while retaining history.
    ///
    /// # Errors
    ///
    /// Returns a stale-revision or optimistic-version error.
    pub fn supersede_revision(&mut self, expected_version: u64) -> Result<(), BuilderError> {
        self.require_mutable(expected_version)?;
        if self.current_revision().is_none() {
            return Err(BuilderError::new(BuilderErrorCode::BuilderRevisionStale));
        }
        self.invalidate_pending_analysis_decision(
            BuilderAnalysisDecisionInvalidationReason::RevisionSuperseded,
        )?;
        self.state = BuilderDraftState::Superseded;
        self.advance_version()
    }

    /// Records user acceptance for review; it grants no activation authority.
    ///
    /// # Errors
    ///
    /// Returns a stale-state or optimistic-version error.
    pub fn accept_for_review(&mut self, expected_version: u64) -> Result<(), BuilderError> {
        self.require_state(
            expected_version,
            &[BuilderDraftState::DraftReady, BuilderDraftState::Analyzed],
        )?;
        self.invalidate_pending_analysis_decision(
            BuilderAnalysisDecisionInvalidationReason::AcceptedForReview,
        )?;
        self.state = BuilderDraftState::UserAccepted;
        self.advance_version()
    }

    /// Records a stable authoring blocker without inventing a future target.
    ///
    /// # Errors
    ///
    /// Returns a terminal-state or optimistic-version error.
    pub fn block(&mut self, expected_version: u64) -> Result<(), BuilderError> {
        self.require_mutable(expected_version)?;
        self.invalidate_pending_analysis_decision(
            BuilderAnalysisDecisionInvalidationReason::DraftBlocked,
        )?;
        self.state = BuilderDraftState::Blocked;
        self.advance_version()
    }

    /// Abandons the inactive draft while preserving its immutable history.
    ///
    /// # Errors
    ///
    /// Returns a terminal-state or optimistic-version error.
    pub fn abandon(&mut self, expected_version: u64) -> Result<(), BuilderError> {
        self.require_mutable(expected_version)?;
        self.invalidate_pending_analysis_decision(
            BuilderAnalysisDecisionInvalidationReason::DraftAbandoned,
        )?;
        self.state = BuilderDraftState::Abandoned;
        self.advance_version()
    }

    /// Serializes the private authority aggregate canonically for authenticated CAS.
    ///
    /// # Errors
    ///
    /// Returns `builder_payload_tampered` if canonical serialization fails.
    pub fn to_persisted_json(&self) -> Result<String, BuilderError> {
        String::from_utf8(canonical_json_bytes(self)?)
            .map_err(|_| BuilderError::new(BuilderErrorCode::BuilderPayloadTampered))
    }

    /// Reconstructs and validates a private authority aggregate.
    ///
    /// # Errors
    ///
    /// Returns `builder_payload_tampered` for malformed or inconsistent history.
    pub fn from_persisted_json(source: &str) -> Result<Self, BuilderError> {
        let value: Self = serde_json::from_str(source)
            .map_err(|_| BuilderError::new(BuilderErrorCode::BuilderPayloadTampered))?;
        value.validate_restored()?;
        Ok(value)
    }

    fn validate_restored(&self) -> Result<(), BuilderError> {
        self.record.validate()?;
        if self.version == 0 {
            return Err(BuilderError::new(BuilderErrorCode::BuilderPayloadTampered));
        }
        self.validate_restored_revisions()?;
        let (mut decision_ids, mut invocation_ids) = self.validate_restored_analyses()?;
        self.validate_restored_consumptions()?;
        self.validate_restored_invalidations(&mut decision_ids, &mut invocation_ids)?;
        self.validate_pending_analysis_decision(&decision_ids, &invocation_ids)?;
        if !self.restored_state_shape_valid() {
            return Err(BuilderError::new(BuilderErrorCode::BuilderPayloadTampered));
        }
        Ok(())
    }

    fn validate_restored_revisions(&self) -> Result<(), BuilderError> {
        let mut revision_ids = BTreeSet::new();
        let mut revision_hashes = BTreeSet::new();
        for (index, revision) in self.revisions.iter().enumerate() {
            revision.validate()?;
            let expected_ordinal = u64::try_from(index)
                .ok()
                .and_then(|value| value.checked_add(1))
                .ok_or_else(|| BuilderError::new(BuilderErrorCode::BuilderPayloadTampered))?;
            let expected_parent = index
                .checked_sub(1)
                .and_then(|prior| self.revisions.get(prior))
                .map(|value| value.revision_hash);
            if revision.draft_id != self.record.draft_id
                || revision.builder_kind != self.record.builder_kind
                || revision.validation_profile != self.record.validation_profile
                || revision.ordinal != expected_ordinal
                || revision.parent_revision_hash != expected_parent
                || !revision_ids.insert(&revision.revision_id)
                || !revision_hashes.insert(revision.revision_hash)
            {
                return Err(BuilderError::new(BuilderErrorCode::BuilderPayloadTampered));
            }
        }
        Ok(())
    }

    fn validate_restored_analyses(
        &self,
    ) -> Result<(BTreeSet<String>, BTreeSet<String>), BuilderError> {
        let mut analysis_ids = BTreeSet::new();
        let mut decision_ids = BTreeSet::new();
        let mut invocation_ids = BTreeSet::new();
        let mut consumption_hashes = BTreeSet::new();
        for analysis in &self.analyses {
            analysis.validate()?;
            let revision_matches = self.revisions.iter().any(|revision| {
                revision.revision_id == analysis.revision_id
                    && revision.revision_hash == analysis.revision_hash
            });
            let unique_binding = analysis.model_binding().is_none_or(|binding| {
                decision_ids.insert(binding.context_decision_id.to_string())
                    && invocation_ids.insert(binding.invocation_id.to_string())
                    && consumption_hashes.insert(binding.context_decision_consumption_hash)
            });
            if analysis.draft_id != self.record.draft_id
                || !revision_matches
                || !analysis_ids.insert(&analysis.analysis_id)
                || !unique_binding
            {
                return Err(BuilderError::new(BuilderErrorCode::BuilderPayloadTampered));
            }
        }
        Ok((decision_ids, invocation_ids))
    }

    fn validate_restored_consumptions(&self) -> Result<(), BuilderError> {
        if self.analysis_consumptions.len()
            != self
                .analyses
                .iter()
                .filter(|analysis| analysis.analysis_kind == BuilderAnalysisKind::ModelLens)
                .count()
        {
            return Err(BuilderError::new(BuilderErrorCode::BuilderPayloadTampered));
        }
        for consumption in &self.analysis_consumptions {
            consumption.validate_integrity()?;
            let analysis = self
                .analyses
                .iter()
                .find(|analysis| analysis.analysis_id == consumption.analysis_id)
                .ok_or_else(|| BuilderError::new(BuilderErrorCode::BuilderPayloadTampered))?;
            let binding = analysis
                .model_binding()
                .ok_or_else(|| BuilderError::new(BuilderErrorCode::BuilderPayloadTampered))?;
            if consumption.draft_id != self.record.draft_id
                || consumption.revision_id != analysis.revision_id
                || consumption.decision_id != binding.context_decision_id
                || consumption.invocation_id != binding.invocation_id
                || consumption.consumption_hash != binding.context_decision_consumption_hash
            {
                return Err(BuilderError::new(BuilderErrorCode::BuilderPayloadTampered));
            }
        }
        Ok(())
    }

    fn validate_pending_analysis_decision(
        &self,
        decision_ids: &BTreeSet<String>,
        invocation_ids: &BTreeSet<String>,
    ) -> Result<(), BuilderError> {
        if let Some(decision) = &self.pending_analysis_decision {
            decision.validate_integrity()?;
            let revision = self
                .current_revision()
                .ok_or_else(|| BuilderError::new(BuilderErrorCode::BuilderPayloadTampered))?;
            let scope = self
                .scope()
                .ok_or_else(|| BuilderError::new(BuilderErrorCode::BuilderPayloadTampered))?;
            if decision.draft_id != self.record.draft_id
                || decision.revision_id != revision.revision_id
                || decision.revision_hash != revision.revision_hash
                || decision.instruction_projection_set_hash
                    != self.record.instruction_projection_set_hash
                || decision.scope_hash != canonical_hash("bmad-builder-draft-scope", 1, &scope)?
                || decision_ids.contains(decision.decision_id.as_str())
                || invocation_ids.contains(decision.invocation_id.as_str())
            {
                return Err(BuilderError::new(BuilderErrorCode::BuilderPayloadTampered));
            }
        }
        Ok(())
    }

    fn validate_restored_invalidations(
        &self,
        decision_ids: &mut BTreeSet<String>,
        invocation_ids: &mut BTreeSet<String>,
    ) -> Result<(), BuilderError> {
        for invalidation in &self.analysis_decision_invalidations {
            invalidation.validate_integrity()?;
            let revision_exists = self
                .revisions
                .iter()
                .any(|revision| revision.revision_id == invalidation.revision_id);
            if invalidation.draft_id != self.record.draft_id
                || !revision_exists
                || invalidation.aggregate_version > self.version
                || !decision_ids.insert(invalidation.decision_id.to_string())
                || !invocation_ids.insert(invalidation.invocation_id.to_string())
            {
                return Err(BuilderError::new(BuilderErrorCode::BuilderPayloadTampered));
            }
        }
        Ok(())
    }

    fn invalidate_pending_analysis_decision(
        &mut self,
        reason: BuilderAnalysisDecisionInvalidationReason,
    ) -> Result<(), BuilderError> {
        let Some(decision) = self.pending_analysis_decision.take() else {
            return Ok(());
        };
        let aggregate_version = self
            .version
            .checked_add(1)
            .ok_or_else(|| BuilderError::new(BuilderErrorCode::BuilderRevisionStale))?;
        let mut invalidation = BuilderAnalysisDecisionInvalidation {
            decision_id: decision.decision_id,
            invocation_id: decision.invocation_id,
            draft_id: decision.draft_id,
            revision_id: decision.revision_id,
            decision_hash: decision.decision_hash,
            reason,
            aggregate_version,
            invalidation_hash: sha256_bytes(b"pending-builder-analysis-invalidation"),
        };
        invalidation.invalidation_hash = canonical_hash_without_field(
            "bmad-builder-analysis-decision-invalidation",
            1,
            &invalidation,
            "invalidationHash",
        )?;
        self.analysis_decision_invalidations.push(invalidation);
        Ok(())
    }

    fn restored_state_shape_valid(&self) -> bool {
        match self.state {
            BuilderDraftState::Drafting => self.revisions.is_empty() && self.analyses.is_empty(),
            BuilderDraftState::DraftReady | BuilderDraftState::UserAccepted => {
                !self.revisions.is_empty()
            }
            BuilderDraftState::Analyzed => !self.revisions.is_empty() && !self.analyses.is_empty(),
            BuilderDraftState::Blocked | BuilderDraftState::Abandoned => true,
            BuilderDraftState::Superseded => !self.revisions.is_empty(),
        }
    }

    fn require_mutable(&self, expected_version: u64) -> Result<(), BuilderError> {
        if matches!(
            self.state,
            BuilderDraftState::UserAccepted
                | BuilderDraftState::Abandoned
                | BuilderDraftState::Superseded
        ) {
            return Err(BuilderError::new(BuilderErrorCode::BuilderRevisionStale));
        }
        self.require_version(expected_version)
    }

    fn require_state(
        &self,
        expected_version: u64,
        allowed: &[BuilderDraftState],
    ) -> Result<(), BuilderError> {
        self.require_version(expected_version)?;
        if !allowed.contains(&self.state) {
            return Err(BuilderError::new(BuilderErrorCode::BuilderRevisionStale));
        }
        Ok(())
    }

    fn require_version(&self, expected_version: u64) -> Result<(), BuilderError> {
        if self.version != expected_version {
            return Err(BuilderError::new(BuilderErrorCode::BuilderRevisionStale));
        }
        Ok(())
    }

    fn advance_version(&mut self) -> Result<(), BuilderError> {
        self.version = self
            .version
            .checked_add(1)
            .ok_or_else(|| BuilderError::new(BuilderErrorCode::BuilderRevisionStale))?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuilderErrorCode {
    BuilderActionInvalidForKind,
    BuilderRevisionStale,
    BuilderTargetDeferred,
    BuilderAnalysisNotEvaluation,
    BuilderFutureCapabilityForbidden,
    BuilderPayloadTampered,
    BuilderContextDecisionMissing,
    BuilderContextDecisionInvalid,
}

impl BuilderErrorCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BuilderActionInvalidForKind => "builder_action_invalid_for_kind",
            Self::BuilderRevisionStale => "builder_revision_stale",
            Self::BuilderTargetDeferred => "builder_target_deferred",
            Self::BuilderAnalysisNotEvaluation => "builder_analysis_not_evaluation",
            Self::BuilderFutureCapabilityForbidden => "builder_future_capability_forbidden",
            Self::BuilderPayloadTampered => "builder_payload_tampered",
            Self::BuilderContextDecisionMissing => "builder_context_decision_missing",
            Self::BuilderContextDecisionInvalid => "builder_context_decision_invalid",
        }
    }
}

#[derive(Debug)]
pub struct BuilderError {
    code: BuilderErrorCode,
}

impl BuilderError {
    #[must_use]
    pub const fn new(code: BuilderErrorCode) -> Self {
        Self { code }
    }

    #[must_use]
    pub const fn code(&self) -> BuilderErrorCode {
        self.code
    }
}

impl fmt::Display for BuilderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code.as_str())
    }
}

impl Error for BuilderError {}

impl From<crate::CanonicalHashError> for BuilderError {
    fn from(_: crate::CanonicalHashError) -> Self {
        Self::new(BuilderErrorCode::BuilderPayloadTampered)
    }
}

impl From<serde_json::Error> for BuilderError {
    fn from(_: serde_json::Error) -> Self {
        Self::new(BuilderErrorCode::BuilderPayloadTampered)
    }
}

fn validate_builder_path(path: &str) -> Result<(), BuilderError> {
    let segments = path.split('/').collect::<Vec<_>>();
    let valid = !path.is_empty()
        && path.len() <= 240
        && path.nfc().eq(path.chars())
        && !path.starts_with('/')
        && !path.contains(['\\', ':', '\0'])
        && segments.len() <= 16
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && segment != &"."
                && segment != &".."
                && segment.len() <= 120
                && !segment.ends_with(['.', ' '])
                && !is_windows_reserved(segment)
                && !segment.to_ascii_lowercase().starts_with("bmad-")
                && !segment.chars().any(char::is_control)
        });
    if valid {
        Ok(())
    } else {
        Err(BuilderError::new(
            BuilderErrorCode::BuilderFutureCapabilityForbidden,
        ))
    }
}

fn is_windows_reserved(segment: &str) -> bool {
    let stem = segment
        .split_once('.')
        .map_or(segment, |(candidate, _)| candidate)
        .to_ascii_lowercase();
    matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
        || stem
            .strip_prefix("com")
            .or_else(|| stem.strip_prefix("lpt"))
            .is_some_and(|suffix| suffix.len() == 1 && matches!(suffix.as_bytes()[0], b'1'..=b'9'))
}

fn valid_inventory(kind: BuilderKind, paths: &BTreeSet<&str>) -> bool {
    match kind {
        BuilderKind::Workflow => paths.len() == 1 && paths.contains("SKILL.md"),
        BuilderKind::Agent => {
            let mandatory = [
                "SKILL.md",
                "customize.toml",
                "references/prompt-quality-canon.md",
            ];
            mandatory.iter().all(|path| paths.contains(path))
                && paths.len() <= 16
                && paths.iter().all(|path| {
                    mandatory.contains(path)
                        || path
                            .strip_prefix("references/")
                            .and_then(|value| value.strip_suffix(".md"))
                            .is_some_and(valid_capability_name)
                })
        }
    }
}

fn valid_capability_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_lowercase()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn valid_rule_id(value: &str) -> bool {
    (1..=160).contains(&value.len())
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
}

fn valid_text(value: &str, max: usize) -> bool {
    (1..=max).contains(&value.len()) && !value.chars().any(|character| character == '\0')
}
