//! Builder authoring and package lifecycle.
//!
//! Without this module a Builder draft is only a pile of proposed files: there
//! is nothing that says which exact revision was analysed, which analysis
//! decision was already spent, or — once a draft leaves authoring — which
//! promotion authorised the package that is now live. The lifecycle half of
//! the module exists so that "active" is never a state a caller can simply
//! assert; it is reachable only through a recorded, approved promotion.
//!
//! The module is split by role rather than by contract so that the authoring
//! surface cannot quietly acquire lifecycle authority: [`model`] holds the
//! data and its self-verifying hashes, [`ports`] holds the persistence
//! boundaries, and [`service`] holds the two coordinators — one for drafts,
//! one for packages — that never share a repository trait.

mod model;
mod ports;
mod service;

pub use model::{
    BmadUtcInstant, BuilderActionName, BuilderAnalysisContextDecision,
    BuilderAnalysisDecisionConsumption, BuilderAnalysisDecisionInvalidation,
    BuilderAnalysisDecisionInvalidationReason, BuilderAnalysisKind, BuilderAnalysisModelBinding,
    BuilderAnalysisRun, BuilderApprovalDecision, BuilderAuthoringAction, BuilderCapabilityFact,
    BuilderDeterministicFinding, BuilderDraft, BuilderDraftRecord, BuilderDraftRevision,
    BuilderDraftScope, BuilderDraftState, BuilderError, BuilderErrorCode, BuilderFindingSeverity,
    BuilderKind, BuilderLensVerdict, BuilderLifecycle, BuilderLifecycleEvent,
    BuilderLifecycleReferences, BuilderLifecycleState, BuilderModelAnalysisDecisionInput,
    BuilderModelFinding, BuilderModelLens, BuilderModelLensResult,
    BuilderModelLensesNotPerformedReason, BuilderPersistenceEvent, BuilderPromotion,
    BuilderPromotionApproval, BuilderPromotionInstallation, BuilderPromotionStage,
    BuilderProposedFile, BuilderProposedFileSet, BuilderRendererProjection,
    BuilderValidationProfile,
};
pub use ports::{BuilderDraftRepository, BuilderLifecycleRepository, BuilderServiceError};
pub use service::{BuilderAuthoringService, BuilderLifecycleService};
