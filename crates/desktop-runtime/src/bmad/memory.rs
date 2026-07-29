//! Typed memory classes and compaction (ENG-10).

/// Placeholder wiring; the memory service lands with ENG-10.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BmadMemoryError {
    /// Compaction attempted to drop an authoritative entry.
    AuthoritativeLoss,
}
