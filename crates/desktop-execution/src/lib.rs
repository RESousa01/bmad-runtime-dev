//! Journaled, governed UTF-8 patch application.
//!
//! This crate intentionally exposes no child-process API and never accepts an
//! absolute filesystem path. A workspace broker implements [`WorkspaceFileIo`]
//! for one already-selected capability; a store adapter implements the durable
//! checkpoint and journal ordering.

mod executor;
mod model;
mod rollback;

pub use executor::PatchExecutor;
pub use model::{
    CheckpointEntry, CheckpointFileState, EffectJournal, ExecutionError, ExecutionOutcome,
    ExecutionRequest, ExecutionStore, FileObservation, JournalOperation, JournalOperationState,
    JournalState, JournalStoreError, LocalCheckpoint, LocalExecutionResult, RecoveryDisposition,
    RecoveryOperation, RecoveryPlan, RecoveryReason, RecoveryRestoreResult, ResultFileOperation,
    WorkspaceFileIo, WorkspaceFileObservation, WorkspaceIoError,
};
pub use rollback::{
    plan_recovery, plan_rollback, restore_checkpoint, RollbackConflict, RollbackPlan,
};
