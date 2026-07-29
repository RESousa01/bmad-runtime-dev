//! Governed installation and atomic catalog activation (ENG-16).

/// Placeholder wiring; the installation planner lands with ENG-16.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BmadInstallationError {
    /// A destination path escaped its governed root.
    UnsafeDestination,
}
