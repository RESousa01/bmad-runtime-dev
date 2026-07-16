use std::{error::Error, fmt};

use crate::{AuthorityRef, ContractId};

use super::{
    BuilderAnalysisContextDecision, BuilderAnalysisRun, BuilderDraft, BuilderDraftRecord,
    BuilderDraftRevision, BuilderDraftScope, BuilderError, BuilderErrorCode,
    BuilderModelAnalysisDecisionInput, BuilderPersistenceEvent, BuilderRendererProjection,
};

/// Persistence boundary for the separate inactive Builder authoring lifecycle.
pub trait BuilderDraftRepository: Send + Sync {
    type Error;

    /// Creates the draft projection and its evidence/outbox records atomically.
    ///
    /// # Errors
    ///
    /// Returns the repository conflict, corruption, or storage error.
    fn create_builder_draft(&self, draft: &BuilderDraft) -> Result<(), Self::Error>;

    /// Loads one exact authority/owner/project/authoring-session-scoped draft.
    ///
    /// # Errors
    ///
    /// Returns the repository corruption or storage error.
    fn load_builder_draft(
        &self,
        scope: &BuilderDraftScope,
        draft_id: &ContractId,
    ) -> Result<Option<BuilderDraft>, Self::Error>;

    /// Persists one validated transition with immutable history and evidence.
    ///
    /// # Errors
    ///
    /// Returns the repository conflict, corruption, or storage error.
    fn persist_builder_transition(
        &self,
        draft: &BuilderDraft,
        expected_previous_version: u64,
        event: BuilderPersistenceEvent,
    ) -> Result<(), Self::Error>;
}

#[derive(Debug)]
pub enum BuilderServiceError<E> {
    Domain(BuilderError),
    Repository(E),
}

impl<E> From<BuilderError> for BuilderServiceError<E> {
    fn from(value: BuilderError) -> Self {
        Self::Domain(value)
    }
}

impl<E: fmt::Display> fmt::Display for BuilderServiceError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Domain(error) => error.fmt(formatter),
            Self::Repository(error) => error.fmt(formatter),
        }
    }
}

impl<E: Error + 'static> Error for BuilderServiceError<E> {}

/// Host coordinator for inactive Builder drafts. It intentionally has no
/// registration, execution, evaluation, publication, or activation methods.
pub struct BuilderAuthoringService<R> {
    repository: R,
}

impl<R: BuilderDraftRepository> BuilderAuthoringService<R> {
    #[must_use]
    pub const fn new(repository: R) -> Self {
        Self { repository }
    }

    #[must_use]
    pub const fn repository(&self) -> &R {
        &self.repository
    }

    /// Creates an authority-bound inactive draft.
    ///
    /// # Errors
    ///
    /// Returns a domain validation or repository error.
    pub fn create_draft(
        &self,
        record: BuilderDraftRecord,
        authority_ref: AuthorityRef,
    ) -> Result<BuilderDraft, BuilderServiceError<R::Error>> {
        let mut draft = BuilderDraft::create(record)?;
        draft.bind_authority(authority_ref)?;
        self.repository
            .create_builder_draft(&draft)
            .map_err(BuilderServiceError::Repository)?;
        Ok(draft)
    }

    /// Appends one immutable revision against the current optimistic version.
    ///
    /// # Errors
    ///
    /// Returns a domain validation, conflict, or repository error.
    pub fn append_revision(
        &self,
        scope: &BuilderDraftScope,
        draft_id: &ContractId,
        expected_version: u64,
        revision: BuilderDraftRevision,
    ) -> Result<BuilderDraft, BuilderServiceError<R::Error>> {
        self.transition(
            scope,
            draft_id,
            expected_version,
            |draft| draft.append_revision(expected_version, revision),
            BuilderPersistenceEvent::RevisionAppended,
        )
    }

    /// Records exact deterministic or separately consented model-lens evidence.
    ///
    /// # Errors
    ///
    /// Returns a domain binding, replay, conflict, or repository error.
    pub fn record_analysis(
        &self,
        scope: &BuilderDraftScope,
        draft_id: &ContractId,
        expected_version: u64,
        analysis: BuilderAnalysisRun,
    ) -> Result<BuilderDraft, BuilderServiceError<R::Error>> {
        self.transition(
            scope,
            draft_id,
            expected_version,
            |draft| draft.record_analysis(expected_version, analysis),
            BuilderPersistenceEvent::AnalysisRecorded,
        )
    }

    /// Persists one host-reviewed, exact-revision model-analysis decision.
    ///
    /// # Errors
    ///
    /// Returns a scope, revision, replay, optimistic conflict, or repository error.
    pub fn issue_model_analysis_decision(
        &self,
        scope: &BuilderDraftScope,
        draft_id: &ContractId,
        expected_version: u64,
        input: BuilderModelAnalysisDecisionInput,
    ) -> Result<BuilderDraft, BuilderServiceError<R::Error>> {
        self.transition(
            scope,
            draft_id,
            expected_version,
            |draft| {
                let _: BuilderAnalysisContextDecision =
                    draft.issue_model_analysis_decision(expected_version, input)?;
                Ok(())
            },
            BuilderPersistenceEvent::AnalysisDecisionIssued,
        )
    }

    /// Retains history while closing the current revision as superseded.
    ///
    /// # Errors
    ///
    /// Returns a stale-state, conflict, or repository error.
    pub fn supersede_revision(
        &self,
        scope: &BuilderDraftScope,
        draft_id: &ContractId,
        expected_version: u64,
    ) -> Result<BuilderDraft, BuilderServiceError<R::Error>> {
        self.transition(
            scope,
            draft_id,
            expected_version,
            |draft| draft.supersede_revision(expected_version),
            BuilderPersistenceEvent::RevisionSuperseded,
        )
    }

    /// Records user acceptance without registration or activation authority.
    ///
    /// # Errors
    ///
    /// Returns a stale-state, conflict, or repository error.
    pub fn accept_for_review(
        &self,
        scope: &BuilderDraftScope,
        draft_id: &ContractId,
        expected_version: u64,
    ) -> Result<BuilderDraft, BuilderServiceError<R::Error>> {
        self.transition(
            scope,
            draft_id,
            expected_version,
            |draft| draft.accept_for_review(expected_version),
            BuilderPersistenceEvent::AcceptedForReview,
        )
    }

    /// Records a present-tense authoring blocker.
    ///
    /// # Errors
    ///
    /// Returns a terminal-state, conflict, or repository error.
    pub fn block(
        &self,
        scope: &BuilderDraftScope,
        draft_id: &ContractId,
        expected_version: u64,
    ) -> Result<BuilderDraft, BuilderServiceError<R::Error>> {
        self.transition(
            scope,
            draft_id,
            expected_version,
            |draft| draft.block(expected_version),
            BuilderPersistenceEvent::Blocked,
        )
    }

    /// Abandons the inactive draft while retaining immutable history.
    ///
    /// # Errors
    ///
    /// Returns a terminal-state, conflict, or repository error.
    pub fn abandon(
        &self,
        scope: &BuilderDraftScope,
        draft_id: &ContractId,
        expected_version: u64,
    ) -> Result<BuilderDraft, BuilderServiceError<R::Error>> {
        self.transition(
            scope,
            draft_id,
            expected_version,
            |draft| draft.abandon(expected_version),
            BuilderPersistenceEvent::Abandoned,
        )
    }

    /// Returns a file-content-free renderer projection.
    ///
    /// # Errors
    ///
    /// Returns a repository or retained-state integrity error.
    pub fn projection(
        &self,
        scope: &BuilderDraftScope,
        draft_id: &ContractId,
    ) -> Result<BuilderRendererProjection, BuilderServiceError<R::Error>> {
        Ok(self.load_required(scope, draft_id)?.renderer_projection())
    }

    fn transition<F>(
        &self,
        scope: &BuilderDraftScope,
        draft_id: &ContractId,
        expected_version: u64,
        transition: F,
        event: BuilderPersistenceEvent,
    ) -> Result<BuilderDraft, BuilderServiceError<R::Error>>
    where
        F: FnOnce(&mut BuilderDraft) -> Result<(), BuilderError>,
    {
        let mut draft = self.load_required(scope, draft_id)?;
        transition(&mut draft)?;
        self.repository
            .persist_builder_transition(&draft, expected_version, event)
            .map_err(BuilderServiceError::Repository)?;
        Ok(draft)
    }

    fn load_required(
        &self,
        scope: &BuilderDraftScope,
        draft_id: &ContractId,
    ) -> Result<BuilderDraft, BuilderServiceError<R::Error>> {
        self.repository
            .load_builder_draft(scope, draft_id)
            .map_err(BuilderServiceError::Repository)?
            .ok_or_else(|| BuilderError::new(BuilderErrorCode::BuilderPayloadTampered).into())
    }
}
