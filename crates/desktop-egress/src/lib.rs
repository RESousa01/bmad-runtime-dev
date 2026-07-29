//! Pure context-egress preparation and single-use consent authority.
//!
//! This crate deliberately contains no filesystem, network, process, database,
//! or renderer implementation.

#![deny(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

mod consent;
mod manifest;
mod preparation;
mod turn_grant;

pub use consent::{
    ApproveDecisionInput, CancelDecisionInput, ConsentService, ConsumeDecisionInput,
    ContextDecisionEvidence, DecisionConsumption, DecisionEvidenceInput, MemoryDecisionLedger,
    ModelInvocationBinding, ModelInvocationBindingDraft, ModelInvocationBindingV2,
    ModelSelectionBindingV2, PendingContextDecision,
};
pub use manifest::{
    ContextClassification, ContextEgressManifest, ContextEgressManifestDraft, ContextExclusion,
    ContextReviewItem, ContextReviewProjection, EgressError, EgressLimits, PreparedContextItem,
    RedactionRecord, RetentionMode, SecretFinding,
};
pub use preparation::{
    local_egress_manifest_hash_v2, reintroduced_redaction_marker, ContextCandidate,
    ContextPreparer, LocalEgressManifestV2Input, LocalEgressManifestV2Item,
    LocalEgressManifestV2Tool, ModelRequestMode, PatternSecretScanner, PrepareContextInput,
    SecretScanFinding, SecretScanResult, SecretScanner, LOCAL_EGRESS_MANIFEST_V2_SCHEMA,
    REDACTION_MARKERS,
};
pub use turn_grant::{CallRecord, TurnEgressEntry, TurnEgressGrant, MAX_TURN_MODEL_CALLS};
