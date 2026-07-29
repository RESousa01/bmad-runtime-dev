//! Cancellation, failure, retry, and repair (ENG-13)

/// Placeholder wiring; the implementation lands with its task.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BmadFailureError {
    /// The input violated its bounded contract.
    BoundsViolation,
}
