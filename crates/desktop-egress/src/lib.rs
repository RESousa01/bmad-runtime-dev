//! Pure context-egress preparation and single-use consent authority.
//!
//! This crate deliberately contains no filesystem, network, process, database,
//! or renderer implementation.

#![deny(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

mod manifest;

pub use manifest::{
    ContextClassification, ContextEgressManifest, ContextEgressManifestDraft, ContextExclusion,
    ContextReviewItem, ContextReviewProjection, EgressError, EgressLimits, PreparedContextItem,
    RedactionRecord, RetentionMode, SecretFinding,
};
