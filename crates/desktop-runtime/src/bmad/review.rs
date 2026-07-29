//! Canonical review and validation lenses (ENG-12)

/// Placeholder wiring; the implementation lands with its task.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BmadReviewError {
    /// The input violated its bounded contract.
    BoundsViolation,
}
