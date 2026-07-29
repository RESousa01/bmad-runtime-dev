//! Deterministic step orchestration (ENG-08).

/// Placeholder wiring; the orchestrator lands with ENG-08.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BmadOrchestratorError {
    /// A source step requested an effect it has no authority to perform.
    UnauthorizedEffect,
}
