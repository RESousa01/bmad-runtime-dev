use super::{
    MethodAdvanceDisposition, MethodAdvanceReceipt, MethodAdvanceRequest, MethodAdvanceResult,
    MethodArtifactProvenance, MethodExactBinding, MethodPersistenceEvent, MethodSession,
    MethodSessionScope,
};
use crate::ContractId;

/// Persistence port. Implementations must commit authority state, evidence,
/// outbox, and single-use decision consumption in one transaction.
pub trait MethodSessionRepository: Send + Sync {
    type Error;

    /// Creates the initial session/evidence/outbox transaction.
    ///
    /// # Errors
    ///
    /// Returns the repository error for conflicts, corruption, or storage failure.
    fn create_method_session(&self, session: &MethodSession) -> Result<(), Self::Error>;

    /// Loads one exact authority/owner/project/run-scoped session.
    ///
    /// # Errors
    ///
    /// Returns the repository error when retained state cannot be authenticated.
    fn load_method_session(
        &self,
        scope: &MethodSessionScope,
        session_id: &ContractId,
    ) -> Result<Option<MethodSession>, Self::Error>;

    /// Consumes one reviewed decision and advances state atomically.
    ///
    /// # Errors
    ///
    /// Returns the repository error for replay, conflicts, or storage failure.
    fn begin_method_advance(
        &self,
        scope: &MethodSessionScope,
        session_id: &ContractId,
        observed_binding: &MethodExactBinding,
        request: MethodAdvanceRequest,
    ) -> Result<MethodAdvanceReceipt, Self::Error>;

    /// Persists a validated aggregate transition with evidence and outbox.
    ///
    /// # Errors
    ///
    /// Returns the repository error for optimistic conflicts or storage failure.
    fn persist_method_transition(
        &self,
        session: &MethodSession,
        expected_previous_version: u64,
        event: MethodPersistenceEvent,
    ) -> Result<(), Self::Error>;

    /// Authenticates every model-proposed working-artifact reference in host CAS.
    ///
    /// # Errors
    ///
    /// Returns the repository error for missing, duplicated, or unauthenticated refs.
    fn validate_method_artifact_refs(
        &self,
        provenance: &MethodArtifactProvenance,
        binding: &MethodExactBinding,
        disposition: MethodAdvanceDisposition,
        refs: &[String],
    ) -> Result<(), Self::Error>;
}

/// Model port. A model can return content but cannot persist or transition a
/// Method session.
pub trait MethodModelPort: Send + Sync {
    type Error;

    /// Requests model content without granting persistence or transition authority.
    ///
    /// # Errors
    ///
    /// Returns the adapter error when the bounded model call fails.
    fn advance(&self, request: &MethodAdvanceRequest) -> Result<MethodAdvanceResult, Self::Error>;
}
