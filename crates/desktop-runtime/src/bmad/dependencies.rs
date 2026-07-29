//! Capability dependency closure (ENG-09)

/// Placeholder wiring; the implementation lands with its task.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BmadDependencyError {
    /// The input violated its bounded contract.
    BoundsViolation,
}
