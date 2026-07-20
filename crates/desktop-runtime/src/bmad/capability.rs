//! Generic sealed BMAD capability runs (readiness Task 5, ADR-0005).
//!
//! One closed run record and one tagged result type serve every roster menu
//! capability and Builder authoring operation. Model output enters only as
//! validated data through these constructors: there is no field through
//! which a result can carry authority, an absolute path, a command, or an
//! approval claim, and a result whose archetype does not match the run's
//! declared output schema is rejected before it is ever stored.

use crate::{ContractId, RelativeWorkspacePath, Sha256Digest, UnixMillis};

pub const BMAD_CAPABILITY_RUN_SCHEMA: &str = "sapphirus.bmad-capability-run.v1";
pub const BMAD_DOCUMENT_ARTIFACT_SCHEMA: &str = "sapphirus.bmad-document-artifact.v1";
pub const BMAD_GOVERNED_CHANGE_SET_SCHEMA: &str = "sapphirus.bmad-governed-change-set.v1";
pub const BMAD_INACTIVE_BUILDER_DRAFT_SCHEMA: &str = "sapphirus.bmad-inactive-builder-draft.v1";

const TITLE_LIMIT: usize = 200;
const PROSE_LIMIT: usize = 32_768;
const SECTION_LIMIT: usize = 64;
const EVIDENCE_LIMIT: usize = 64;
const QUESTION_LIMIT: usize = 64;
const QUESTION_LENGTH_LIMIT: usize = 1_024;
const MERMAID_LIMIT: usize = 16_384;
const SUMMARY_LIMIT: usize = 4_096;
const CHANGE_LIMIT: usize = 128;
const CONTENT_LIMIT: usize = 262_144;
const FILE_LIMIT: usize = 64;
const REVISION_NOTE_LIMIT: usize = 2_048;
const CAPABILITY_SUFFIX_MIN: usize = 3;
const CAPABILITY_SUFFIX_MAX: usize = 81;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BmadCapabilityRunError {
    InvalidCapabilityId,
    UnknownOutputSchema,
    BoundsViolation,
    DuplicatePath,
    ResultArchetypeMismatch,
    ResultAlreadyRecorded,
}

/// A closure-ledger capability identifier: `bmm:<name>` or `builder:<name>`.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(transparent)]
pub struct BmadClosureCapabilityId(String);

impl BmadClosureCapabilityId {
    /// Validates the closed ADR-0005 capability identifier shape.
    ///
    /// # Errors
    ///
    /// Returns [`BmadCapabilityRunError::InvalidCapabilityId`] for any value
    /// outside `^(bmm|builder):[a-z][a-z0-9._-]{2,80}$`.
    pub fn new(value: &str) -> Result<Self, BmadCapabilityRunError> {
        let suffix = value
            .strip_prefix("bmm:")
            .or_else(|| value.strip_prefix("builder:"))
            .ok_or(BmadCapabilityRunError::InvalidCapabilityId)?;
        let mut characters = suffix.chars();
        let first = characters
            .next()
            .ok_or(BmadCapabilityRunError::InvalidCapabilityId)?;
        if !first.is_ascii_lowercase()
            || suffix.len() < CAPABILITY_SUFFIX_MIN
            || suffix.len() > CAPABILITY_SUFFIX_MAX
            || !characters.all(|c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-')
            })
        {
            return Err(BmadCapabilityRunError::InvalidCapabilityId);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadDocumentSection {
    pub heading: String,
    pub body: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadDocumentArtifact {
    schema_version: &'static str,
    pub title: String,
    pub sections: Vec<BmadDocumentSection>,
    pub evidence_refs: Vec<ContractId>,
    pub open_questions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mermaid_text: Option<String>,
}

impl BmadDocumentArtifact {
    /// Builds a bounded inert document artifact.
    ///
    /// # Errors
    ///
    /// Returns [`BmadCapabilityRunError::BoundsViolation`] when any field
    /// exceeds its reviewed bound or a required field is empty.
    pub fn new(
        title: String,
        sections: Vec<BmadDocumentSection>,
        evidence_refs: Vec<ContractId>,
        open_questions: Vec<String>,
        mermaid_text: Option<String>,
    ) -> Result<Self, BmadCapabilityRunError> {
        let bounded_title = !title.is_empty() && title.len() <= TITLE_LIMIT;
        let bounded_sections = !sections.is_empty()
            && sections.len() <= SECTION_LIMIT
            && sections.iter().all(|section| {
                !section.heading.is_empty()
                    && section.heading.len() <= TITLE_LIMIT
                    && !section.body.is_empty()
                    && section.body.len() <= PROSE_LIMIT
            });
        let bounded_questions = open_questions.len() <= QUESTION_LIMIT
            && open_questions
                .iter()
                .all(|question| !question.is_empty() && question.len() <= QUESTION_LENGTH_LIMIT);
        let bounded_mermaid = mermaid_text
            .as_ref()
            .is_none_or(|text| !text.is_empty() && text.len() <= MERMAID_LIMIT);
        if !bounded_title
            || !bounded_sections
            || evidence_refs.len() > EVIDENCE_LIMIT
            || !bounded_questions
            || !bounded_mermaid
        {
            return Err(BmadCapabilityRunError::BoundsViolation);
        }
        Ok(Self {
            schema_version: BMAD_DOCUMENT_ARTIFACT_SCHEMA,
            title,
            sections,
            evidence_refs,
            open_questions,
            mermaid_text,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum BmadCandidateChange {
    Create {
        path: RelativeWorkspacePath,
        content: String,
    },
    Replace {
        path: RelativeWorkspacePath,
        content: String,
        preimage_sha256: Sha256Digest,
    },
    Delete {
        path: RelativeWorkspacePath,
        preimage_sha256: Sha256Digest,
    },
}

impl BmadCandidateChange {
    #[must_use]
    pub const fn path(&self) -> &RelativeWorkspacePath {
        match self {
            Self::Create { path, .. } | Self::Replace { path, .. } | Self::Delete { path, .. } => {
                path
            }
        }
    }

    const fn content_length(&self) -> usize {
        match self {
            Self::Create { content, .. } | Self::Replace { content, .. } => content.len(),
            Self::Delete { .. } => 0,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadGovernedChangeSet {
    schema_version: &'static str,
    pub summary: String,
    pub changes: Vec<BmadCandidateChange>,
}

impl BmadGovernedChangeSet {
    /// Builds a candidate change set. The set carries no authority: it can
    /// only become a D3 proposal through the existing review path.
    ///
    /// # Errors
    ///
    /// Returns [`BmadCapabilityRunError::BoundsViolation`] on any exceeded
    /// bound and [`BmadCapabilityRunError::DuplicatePath`] when two changes
    /// target one path.
    pub fn new(
        summary: String,
        changes: Vec<BmadCandidateChange>,
    ) -> Result<Self, BmadCapabilityRunError> {
        if summary.is_empty()
            || summary.len() > SUMMARY_LIMIT
            || changes.is_empty()
            || changes.len() > CHANGE_LIMIT
            || changes
                .iter()
                .any(|change| change.content_length() > CONTENT_LIMIT)
        {
            return Err(BmadCapabilityRunError::BoundsViolation);
        }
        let mut seen = std::collections::BTreeSet::new();
        for change in &changes {
            if !seen.insert(change.path().as_str().to_owned()) {
                return Err(BmadCapabilityRunError::DuplicatePath);
            }
        }
        Ok(Self {
            schema_version: BMAD_GOVERNED_CHANGE_SET_SCHEMA,
            summary,
            changes,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadBuilderDraftKind {
    Agent,
    Workflow,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadBuilderDraftFile {
    pub path: RelativeWorkspacePath,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadInactiveBuilderDraft {
    schema_version: &'static str,
    pub draft_kind: BmadBuilderDraftKind,
    pub title: String,
    pub revision_note: String,
    pub files: Vec<BmadBuilderDraftFile>,
}

impl BmadInactiveBuilderDraft {
    /// Builds an inactive Builder draft: versioned data that cannot install,
    /// register, execute, or alter the capability catalog.
    ///
    /// # Errors
    ///
    /// Returns [`BmadCapabilityRunError::BoundsViolation`] on any exceeded
    /// bound and [`BmadCapabilityRunError::DuplicatePath`] on duplicate
    /// draft file paths.
    pub fn new(
        draft_kind: BmadBuilderDraftKind,
        title: String,
        revision_note: String,
        files: Vec<BmadBuilderDraftFile>,
    ) -> Result<Self, BmadCapabilityRunError> {
        if title.is_empty()
            || title.len() > TITLE_LIMIT
            || revision_note.is_empty()
            || revision_note.len() > REVISION_NOTE_LIMIT
            || files.is_empty()
            || files.len() > FILE_LIMIT
            || files.iter().any(|file| file.content.len() > CONTENT_LIMIT)
        {
            return Err(BmadCapabilityRunError::BoundsViolation);
        }
        let mut seen = std::collections::BTreeSet::new();
        for file in &files {
            if !seen.insert(file.path.as_str().to_owned()) {
                return Err(BmadCapabilityRunError::DuplicatePath);
            }
        }
        Ok(Self {
            schema_version: BMAD_INACTIVE_BUILDER_DRAFT_SCHEMA,
            draft_kind,
            title,
            revision_note,
            files,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BmadCapabilityOutput {
    DocumentArtifact(BmadDocumentArtifact),
    GovernedChangeSet(BmadGovernedChangeSet),
    InactiveBuilderDraft(BmadInactiveBuilderDraft),
}

impl serde::Serialize for BmadCapabilityOutput {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        // Mirrors bmad-capability-result.schema.json exactly: one tag plus
        // one archetype-named wrapper object.
        let mut map = serializer.serialize_map(Some(2))?;
        match self {
            Self::DocumentArtifact(artifact) => {
                map.serialize_entry("resultKind", "document_artifact")?;
                map.serialize_entry("documentArtifact", artifact)?;
            }
            Self::GovernedChangeSet(change_set) => {
                map.serialize_entry("resultKind", "governed_change_set")?;
                map.serialize_entry("governedChangeSet", change_set)?;
            }
            Self::InactiveBuilderDraft(draft) => {
                map.serialize_entry("resultKind", "inactive_builder_draft")?;
                map.serialize_entry("inactiveBuilderDraft", draft)?;
            }
        }
        map.end()
    }
}

impl BmadCapabilityOutput {
    #[must_use]
    pub const fn schema_id(&self) -> &'static str {
        match self {
            Self::DocumentArtifact(_) => BMAD_DOCUMENT_ARTIFACT_SCHEMA,
            Self::GovernedChangeSet(_) => BMAD_GOVERNED_CHANGE_SET_SCHEMA,
            Self::InactiveBuilderDraft(_) => BMAD_INACTIVE_BUILDER_DRAFT_SCHEMA,
        }
    }
}

/// Inputs for opening one capability run.
#[derive(Clone, Debug)]
pub struct BmadCapabilityRunParams {
    pub run_id: ContractId,
    pub capability_id: BmadClosureCapabilityId,
    pub workspace_id: ContractId,
    pub instruction_hash: Sha256Digest,
    pub context_manifest_hash: Sha256Digest,
    pub output_schema_id: String,
    pub consent_evidence_id: ContractId,
    pub created_at: UnixMillis,
}

/// One immutable capability run bound to its closure-ledger identity.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadCapabilityRun {
    schema_version: &'static str,
    pub run_id: ContractId,
    pub capability_id: BmadClosureCapabilityId,
    pub workspace_id: ContractId,
    pub instruction_hash: Sha256Digest,
    pub context_manifest_hash: Sha256Digest,
    pub output_schema_id: String,
    pub consent_evidence_id: ContractId,
    pub created_at: UnixMillis,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<BmadCapabilityOutput>,
}

impl BmadCapabilityRun {
    /// Opens a run for one capability with a declared output schema.
    ///
    /// # Errors
    ///
    /// Returns [`BmadCapabilityRunError::UnknownOutputSchema`] unless the
    /// schema is one of the three reviewed archetype schemas.
    pub fn open(params: BmadCapabilityRunParams) -> Result<Self, BmadCapabilityRunError> {
        if ![
            BMAD_DOCUMENT_ARTIFACT_SCHEMA,
            BMAD_GOVERNED_CHANGE_SET_SCHEMA,
            BMAD_INACTIVE_BUILDER_DRAFT_SCHEMA,
        ]
        .contains(&params.output_schema_id.as_str())
        {
            return Err(BmadCapabilityRunError::UnknownOutputSchema);
        }
        Ok(Self {
            schema_version: BMAD_CAPABILITY_RUN_SCHEMA,
            run_id: params.run_id,
            capability_id: params.capability_id,
            workspace_id: params.workspace_id,
            instruction_hash: params.instruction_hash,
            context_manifest_hash: params.context_manifest_hash,
            output_schema_id: params.output_schema_id,
            consent_evidence_id: params.consent_evidence_id,
            created_at: params.created_at,
            result: None,
        })
    }

    /// Records the single terminal result.
    ///
    /// # Errors
    ///
    /// Returns [`BmadCapabilityRunError::ResultArchetypeMismatch`] when the
    /// result archetype differs from the declared output schema, and
    /// [`BmadCapabilityRunError::ResultAlreadyRecorded`] on any second
    /// result.
    pub fn record_result(
        &mut self,
        output: BmadCapabilityOutput,
    ) -> Result<(), BmadCapabilityRunError> {
        if self.result.is_some() {
            return Err(BmadCapabilityRunError::ResultAlreadyRecorded);
        }
        if output.schema_id() != self.output_schema_id {
            return Err(BmadCapabilityRunError::ResultArchetypeMismatch);
        }
        self.result = Some(output);
        Ok(())
    }
}
