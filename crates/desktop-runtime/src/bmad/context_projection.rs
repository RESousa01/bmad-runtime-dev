//! BMAD Model Context projection (ENG-14)

/// Placeholder wiring; the implementation lands with its task.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BmadContextProjectionError {
    /// The input violated its bounded contract.
    BoundsViolation,
}
