use thiserror::Error;

use super::{
    BmadCatalog, BmadHelpAdvisor, BmadHelpIntent, BmadHelpRecommendation, BmadKernelError,
    BmadKernelErrorCode, CreateMethodSession, MethodErrorCode, MethodServiceError, MethodSession,
    MethodSessionRepository, MethodSessionService,
};
use crate::{ContractId, DesktopLocalIdentity, DomainValidationError, UnixMillis};

/// Host-authored inputs for a local, persisted, deliberately unbound Help
/// session. Exact Method capability/model binding remains a later D2 step.
pub struct CreateInertBmadHelpSession {
    pub session_id: ContractId,
    pub project_id: ContractId,
    pub run_id: ContractId,
    pub local_identity: DesktopLocalIdentity,
    pub created_at: UnixMillis,
    pub intent: BmadHelpIntent,
}

/// A grounded catalog recommendation paired with a durable Method session
/// that has no execution authority and remains in `Created` state.
#[derive(Debug)]
pub struct InertBmadHelpSession {
    pub session: MethodSession,
    pub recommendation: BmadHelpRecommendation,
}

#[derive(Debug, Error)]
pub enum InertBmadHelpSessionError<E> {
    #[error(transparent)]
    Identity(#[from] DomainValidationError),
    #[error(transparent)]
    Advisor(#[from] BmadKernelError),
    #[error(transparent)]
    Session(#[from] MethodServiceError<E>),
}

impl<E> InertBmadHelpSessionError<E> {
    #[must_use]
    pub const fn advisor_code(&self) -> Option<BmadKernelErrorCode> {
        match self {
            Self::Advisor(error) => Some(error.code()),
            Self::Identity(_) | Self::Session(_) => None,
        }
    }

    #[must_use]
    pub const fn method_code(&self) -> Option<MethodErrorCode> {
        match self {
            Self::Session(MethodServiceError::Domain(error)) => Some(error.code()),
            Self::Identity(_)
            | Self::Advisor(_)
            | Self::Session(MethodServiceError::Repository(_)) => None,
        }
    }
}

/// Creates only the local precursor needed before D2 model binding exists.
/// It cannot bind, review, advance, call a model, or claim completion.
pub struct InertBmadHelpSessionCoordinator;

impl InertBmadHelpSessionCoordinator {
    /// Grounds the intent first, then persists one non-runnable Method session.
    ///
    /// # Errors
    ///
    /// Returns an advisor error without writing when the catalog cannot ground
    /// the intent, an identity error for an unauthentic local authority, or a
    /// Method/store error when session creation cannot commit.
    pub fn create<R>(
        repository: &R,
        catalog: &BmadCatalog,
        input: CreateInertBmadHelpSession,
    ) -> Result<InertBmadHelpSession, InertBmadHelpSessionError<R::Error>>
    where
        R: MethodSessionRepository,
    {
        let recommendation = BmadHelpAdvisor::recommend(catalog, &input.intent, &[])?;
        let authority_ref = input.local_identity.authority_ref()?;
        let session = MethodSessionService::new(repository).create(CreateMethodSession {
            session_id: input.session_id,
            owner_scope_ref: input.local_identity.owner_scope_ref().clone(),
            project_id: input.project_id,
            run_id: input.run_id,
            authority_ref,
            created_at: input.created_at,
        })?;
        Ok(InertBmadHelpSession {
            session,
            recommendation,
        })
    }
}
