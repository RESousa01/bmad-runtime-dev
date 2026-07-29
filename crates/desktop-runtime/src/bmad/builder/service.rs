//! The two Builder coordinators.
//!
//! Without this module the store is driven straight from command handlers, and
//! the load-mutate-persist ordering that makes an optimistic version guard mean
//! anything gets re-implemented at every call site.
//!
//! [`BuilderAuthoringService`] and [`BuilderLifecycleService`] are separate
//! types over separate repository traits on purpose: a caller holding the
//! authoring service must not be able to reach activation.

use crate::{AuthorityRef, ContractId, Sha256Digest};

use super::{
    BuilderAnalysisContextDecision, BuilderAnalysisRun, BuilderDraft, BuilderDraftRecord,
    BuilderDraftRepository, BuilderDraftRevision, BuilderDraftScope, BuilderError,
    BuilderErrorCode, BuilderLifecycle, BuilderLifecycleRepository, BuilderLifecycleState,
    BuilderModelAnalysisDecisionInput, BuilderPersistenceEvent, BuilderPromotion,
    BuilderRendererProjection, BuilderServiceError,
};

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

/// Host coordinator for the Builder package lifecycle.
///
/// Every method here is a promotion. There is deliberately no `set_state`: the
/// only way to reach a lifecycle state is to record the promotion that
/// justified it, which is what makes the event log an account rather than a
/// decoration.
pub struct BuilderLifecycleService<R> {
    repository: R,
}

impl<R: BuilderLifecycleRepository> BuilderLifecycleService<R> {
    #[must_use]
    pub const fn new(repository: R) -> Self {
        Self { repository }
    }

    #[must_use]
    pub const fn repository(&self) -> &R {
        &self.repository
    }

    /// Opens the lifecycle of one Builder package in `draft`.
    ///
    /// # Errors
    ///
    /// Returns a repository conflict or storage error.
    pub fn open_lifecycle(
        &self,
        lifecycle_id: ContractId,
        draft_id: ContractId,
        subject_hash: Sha256Digest,
        now_ms: u64,
    ) -> Result<BuilderLifecycle, BuilderServiceError<R::Error>> {
        let lifecycle = BuilderLifecycle::open(lifecycle_id, draft_id, subject_hash);
        self.repository
            .create_builder_lifecycle(&lifecycle, now_ms)
            .map_err(BuilderServiceError::Repository)?;
        Ok(lifecycle)
    }

    /// Records one promotion and the state it moves the package to.
    ///
    /// # Errors
    ///
    /// Returns the domain transition, approval, or tamper error, or the
    /// repository conflict or storage error.
    pub fn record_promotion(
        &self,
        lifecycle_id: &ContractId,
        expected_version: u64,
        promotion: &BuilderPromotion,
        now_ms: u64,
    ) -> Result<BuilderLifecycle, BuilderServiceError<R::Error>> {
        let mut lifecycle = self.load_required(lifecycle_id)?;
        let event = lifecycle.promote(expected_version, promotion.clone())?;
        self.repository
            .commit_builder_lifecycle_transition(&lifecycle, promotion, &event, now_ms)
            .map_err(BuilderServiceError::Repository)?;
        Ok(lifecycle)
    }

    /// Reports the state one package is durably in.
    ///
    /// # Errors
    ///
    /// Returns a repository or retained-state integrity error.
    pub fn lifecycle_state(
        &self,
        lifecycle_id: &ContractId,
    ) -> Result<BuilderLifecycleState, BuilderServiceError<R::Error>> {
        Ok(self.load_required(lifecycle_id)?.state())
    }

    fn load_required(
        &self,
        lifecycle_id: &ContractId,
    ) -> Result<BuilderLifecycle, BuilderServiceError<R::Error>> {
        self.repository
            .load_builder_lifecycle(lifecycle_id)
            .map_err(BuilderServiceError::Repository)?
            .ok_or_else(|| BuilderError::new(BuilderErrorCode::BuilderLifecycleStale).into())
    }
}
