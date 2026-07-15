use std::{error::Error, fmt};

use crate::{ContractId, UnixMillis};

use super::{
    MethodAdvanceReceipt, MethodAdvanceRequest, MethodContextDecision, MethodError,
    MethodErrorCode, MethodExactBinding, MethodPersistenceEvent, MethodSession,
    MethodSessionRepository, MethodSessionScope, MethodStepTable, MethodVerifiedAdvanceResult,
};

#[derive(Debug)]
pub enum MethodServiceError<E> {
    Domain(MethodError),
    Repository(E),
}

impl<E> From<MethodError> for MethodServiceError<E> {
    fn from(value: MethodError) -> Self {
        Self::Domain(value)
    }
}

impl<E> fmt::Display for MethodServiceError<E>
where
    E: fmt::Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Domain(error) => error.fmt(formatter),
            Self::Repository(error) => error.fmt(formatter),
        }
    }
}

impl<E> Error for MethodServiceError<E> where E: Error + 'static {}

/// Host-side coordinator that couples every accepted domain transition to its
/// repository transaction. The model and renderer never receive this type.
pub struct MethodSessionService<R> {
    repository: R,
}

impl<R> MethodSessionService<R>
where
    R: MethodSessionRepository,
{
    #[must_use]
    pub const fn new(repository: R) -> Self {
        Self { repository }
    }

    #[must_use]
    pub const fn repository(&self) -> &R {
        &self.repository
    }

    /// Creates and persists a new non-runnable session.
    ///
    /// # Errors
    ///
    /// Returns a domain validation or repository transaction error.
    pub fn create(
        &self,
        input: super::CreateMethodSession,
    ) -> Result<MethodSession, MethodServiceError<R::Error>> {
        let session = MethodSession::create(input)?;
        self.repository
            .create_method_session(&session)
            .map_err(MethodServiceError::Repository)?;
        Ok(session)
    }

    /// Binds exact capability inputs and a handwritten step table.
    ///
    /// # Errors
    ///
    /// Returns a domain conflict/binding error or repository transaction error.
    pub fn bind_invocation(
        &self,
        scope: &MethodSessionScope,
        session_id: &ContractId,
        expected_version: u64,
        binding: MethodExactBinding,
        step_table: MethodStepTable,
    ) -> Result<MethodSession, MethodServiceError<R::Error>> {
        let mut session = self.load_required(scope, session_id)?;
        session.bind_capability(expected_version, binding, step_table)?;
        self.persist(
            &session,
            expected_version,
            MethodPersistenceEvent::CapabilityBound,
        )?;
        Ok(session)
    }

    /// Rebinds drifted exact inputs and forces a fresh context review.
    ///
    /// # Errors
    ///
    /// Returns a domain conflict/binding error or repository transaction error.
    pub fn rebind_invocation(
        &self,
        scope: &MethodSessionScope,
        session_id: &ContractId,
        expected_version: u64,
        binding: MethodExactBinding,
        step_table: MethodStepTable,
    ) -> Result<MethodSession, MethodServiceError<R::Error>> {
        let mut session = self.load_required(scope, session_id)?;
        session.rebind_capability(expected_version, binding, step_table)?;
        self.persist(
            &session,
            expected_version,
            MethodPersistenceEvent::CapabilityRebound,
        )?;
        Ok(session)
    }

    /// Invalidates prior review and persists the review-required transition.
    ///
    /// # Errors
    ///
    /// Returns a domain conflict or repository transaction error.
    pub fn request_context_review(
        &self,
        scope: &MethodSessionScope,
        session_id: &ContractId,
        expected_version: u64,
    ) -> Result<MethodSession, MethodServiceError<R::Error>> {
        let mut session = self.load_required(scope, session_id)?;
        session.request_context_review(expected_version)?;
        self.persist(
            &session,
            expected_version,
            MethodPersistenceEvent::ContextReviewRequested,
        )?;
        Ok(session)
    }

    /// Persists a fresh exact context-review decision.
    ///
    /// # Errors
    ///
    /// Returns a domain binding/replay error or repository transaction error.
    pub fn record_context_review(
        &self,
        scope: &MethodSessionScope,
        session_id: &ContractId,
        expected_version: u64,
        decision: MethodContextDecision,
    ) -> Result<MethodSession, MethodServiceError<R::Error>> {
        let mut session = self.load_required(scope, session_id)?;
        session.record_context_review(expected_version, decision)?;
        self.persist(
            &session,
            expected_version,
            MethodPersistenceEvent::ContextReviewAccepted,
        )?;
        Ok(session)
    }

    /// Atomically consumes one reviewed decision and returns the authoritative receipt.
    ///
    /// # Errors
    ///
    /// Returns a single-use/conflict or repository transaction error.
    pub fn begin_advance(
        &self,
        scope: &MethodSessionScope,
        session_id: &ContractId,
        observed_binding: &MethodExactBinding,
        request: MethodAdvanceRequest,
    ) -> Result<(MethodSession, MethodAdvanceReceipt), MethodServiceError<R::Error>> {
        let receipt = self
            .repository
            .begin_method_advance(scope, session_id, observed_binding, request)
            .map_err(MethodServiceError::Repository)?;
        let session = self.load_required(scope, session_id)?;
        Ok((session, receipt))
    }

    /// Validates trusted-host result evidence and persists its immutable checkpoint.
    ///
    /// # Errors
    ///
    /// Returns a result/step validation or repository transaction error.
    pub fn accept_result(
        &self,
        scope: &MethodSessionScope,
        session_id: &ContractId,
        expected_version: u64,
        verified_result: MethodVerifiedAdvanceResult,
        recorded_at: UnixMillis,
    ) -> Result<MethodSession, MethodServiceError<R::Error>> {
        let mut session = self.load_required(scope, session_id)?;
        verified_result.verify()?;
        let provenance =
            session.artifact_provenance_for(&verified_result.binding().invocation_id)?;
        self.repository
            .validate_method_artifact_refs(
                &provenance,
                session.current_binding()?,
                verified_result.result().disposition,
                &verified_result.result().working_artifact_refs,
            )
            .map_err(MethodServiceError::Repository)?;
        let _ = session.accept_result(expected_version, verified_result, recorded_at)?;
        self.persist(
            &session,
            expected_version,
            MethodPersistenceEvent::ResultAccepted,
        )?;
        Ok(session)
    }

    /// Persists a user turn that requires a fresh review.
    ///
    /// # Errors
    ///
    /// Returns a domain conflict or repository transaction error.
    pub fn record_user_turn(
        &self,
        scope: &MethodSessionScope,
        session_id: &ContractId,
        expected_version: u64,
    ) -> Result<MethodSession, MethodServiceError<R::Error>> {
        self.transition(
            scope,
            session_id,
            expected_version,
            |session| session.record_user_turn(expected_version),
            MethodPersistenceEvent::UserTurnRecorded,
        )
    }

    /// Persists a terminal refusal.
    ///
    /// # Errors
    ///
    /// Returns a domain conflict or repository transaction error.
    pub fn record_refusal(
        &self,
        scope: &MethodSessionScope,
        session_id: &ContractId,
        expected_version: u64,
    ) -> Result<MethodSession, MethodServiceError<R::Error>> {
        self.transition(
            scope,
            session_id,
            expected_version,
            |session| session.record_refusal(expected_version),
            MethodPersistenceEvent::Refused,
        )
    }

    /// Persists a terminal incomplete result.
    ///
    /// # Errors
    ///
    /// Returns a domain conflict or repository transaction error.
    pub fn record_incomplete(
        &self,
        scope: &MethodSessionScope,
        session_id: &ContractId,
        expected_version: u64,
    ) -> Result<MethodSession, MethodServiceError<R::Error>> {
        self.transition(
            scope,
            session_id,
            expected_version,
            |session| session.record_incomplete(expected_version),
            MethodPersistenceEvent::Incomplete,
        )
    }

    /// Persists cancellation without reviving a consumed decision.
    ///
    /// # Errors
    ///
    /// Returns a domain conflict or repository transaction error.
    pub fn cancel(
        &self,
        scope: &MethodSessionScope,
        session_id: &ContractId,
        expected_version: u64,
    ) -> Result<MethodSession, MethodServiceError<R::Error>> {
        self.transition(
            scope,
            session_id,
            expected_version,
            |session| session.cancel(expected_version),
            MethodPersistenceEvent::Cancelled,
        )
    }

    /// Reloads the authoritative session without changing its version/state.
    ///
    /// # Errors
    ///
    /// Returns a repository error or stable recovery-required domain error.
    pub fn resume(
        &self,
        scope: &MethodSessionScope,
        session_id: &ContractId,
    ) -> Result<MethodSession, MethodServiceError<R::Error>> {
        self.load_required(scope, session_id)
    }

    fn transition<F>(
        &self,
        scope: &MethodSessionScope,
        session_id: &ContractId,
        expected_version: u64,
        transition: F,
        event: MethodPersistenceEvent,
    ) -> Result<MethodSession, MethodServiceError<R::Error>>
    where
        F: FnOnce(&mut MethodSession) -> Result<(), MethodError>,
    {
        let mut session = self.load_required(scope, session_id)?;
        transition(&mut session)?;
        self.persist(&session, expected_version, event)?;
        Ok(session)
    }

    fn persist(
        &self,
        session: &MethodSession,
        expected_previous_version: u64,
        event: MethodPersistenceEvent,
    ) -> Result<(), MethodServiceError<R::Error>> {
        self.repository
            .persist_method_transition(session, expected_previous_version, event)
            .map_err(MethodServiceError::Repository)
    }

    fn load_required(
        &self,
        scope: &MethodSessionScope,
        session_id: &ContractId,
    ) -> Result<MethodSession, MethodServiceError<R::Error>> {
        self.repository
            .load_method_session(scope, session_id)
            .map_err(MethodServiceError::Repository)?
            .ok_or_else(|| MethodError::new(MethodErrorCode::MethodStoreRecoveryRequired).into())
    }
}
