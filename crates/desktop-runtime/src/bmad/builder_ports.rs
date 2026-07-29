//! Compatibility path for the Builder persistence boundary.
//!
//! The boundary itself now lives in `builder::ports`, next to the model and
//! service it constrains. This file stays because the crate facade re-exports
//! these names from `bmad::builder_ports`, and moving a public path is a
//! breaking change for every caller that already imports it.

pub use super::builder::{
    BuilderAuthoringService, BuilderDraftRepository, BuilderLifecycleRepository,
    BuilderLifecycleService, BuilderServiceError,
};
