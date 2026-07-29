//! The two Builder persistence boundaries, kept deliberately separate.
//!
//! Without the split there is one repository trait and any holder of it can
//! reach both draft authoring and package activation. Authoring is app-local
//! and grants no effects; the lifecycle decides what runs on the user's
//! machine. They are different authorities, so they are different traits.

use std::{error::Error, fmt};

use crate::ContractId;

use super::{
    BuilderDraft, BuilderDraftScope, BuilderError, BuilderLifecycle, BuilderLifecycleEvent,
    BuilderPersistenceEvent, BuilderPromotion,
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

/// Persistence boundary for the Builder package lifecycle.
///
/// Every method takes `now_ms` from the caller. A store that read its own
/// clock would stamp a replayed transition with a time that never happened,
/// and the event log is the only account of when a package went live.
pub trait BuilderLifecycleRepository: Send + Sync {
    type Error;

    /// Opens one lifecycle at version 1 in `draft`.
    ///
    /// # Errors
    ///
    /// Returns the repository conflict, corruption, or storage error.
    fn create_builder_lifecycle(
        &self,
        lifecycle: &BuilderLifecycle,
        now_ms: u64,
    ) -> Result<(), Self::Error>;

    /// Recovers one lifecycle exactly as it was last accepted.
    ///
    /// # Errors
    ///
    /// Returns the repository corruption or storage error.
    fn load_builder_lifecycle(
        &self,
        lifecycle_id: &ContractId,
    ) -> Result<Option<BuilderLifecycle>, Self::Error>;

    /// Commits the new lifecycle state, its promotion, its event, and its
    /// outbox row in one transaction under an optimistic version guard.
    ///
    /// Implementations must treat an exact replay of an already-applied
    /// transition as a no-op success and any other version mismatch as a
    /// conflict, so a redelivered command converges instead of corrupting.
    ///
    /// # Errors
    ///
    /// Returns the repository conflict, corruption, or storage error.
    fn commit_builder_lifecycle_transition(
        &self,
        lifecycle: &BuilderLifecycle,
        promotion: &BuilderPromotion,
        event: &BuilderLifecycleEvent,
        now_ms: u64,
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
