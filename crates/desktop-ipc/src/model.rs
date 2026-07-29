//! Renderer projections for the Foundry model catalog, selection, and stream.
//!
//! Three rules shape everything in this module.
//!
//! * **Connection authority is not representable, but the organization's own
//!   published model metadata is not withheld either.** A profile projection
//!   carries its identity, its display text, its capabilities, its limits, its
//!   variants and when it was qualified -- FND-051 and FND-052 need every one
//!   of those to build the picker and the context-window rows. What it still
//!   has no field for, structurally, is a deployment name, a project endpoint,
//!   a subscription or resource identifier, a role assignment, a managed
//!   identity, an API version, or a credential: the canonical contract this
//!   projection mirrors was written so that none of those has anywhere to go,
//!   so no host mistake can leak one through this lane. Display text is
//!   additionally scanned, because a support plane that put an endpoint inside
//!   a profile name would otherwise route around the type.
//! * **Streamed text is display-only, not evidence.**
//!   [`ModelStreamEventInput::OutputProgress`] and
//!   [`ModelStreamEventInput::ToolCallArguments`] carry the actual delta text
//!   alongside their byte counts, because a chat timeline cannot render an
//!   answer from a byte counter and a tool call cannot surface as a reviewable
//!   typed proposal without its arguments. The byte counts stay too: they are
//!   genuinely useful for budget display on their own. But the authority for
//!   what a run produced is the verified receipt at the terminal event, not
//!   the accumulated deltas -- the host must keep accumulating its own
//!   canonical transcript for receipt verification regardless of what it
//!   forwards here, and nothing downstream may treat the displayed text as
//!   proof of what was said. Private reasoning text never crosses this
//!   boundary at all: the support-plane normalizer never buffers it, only a
//!   status flag, so there is nothing here for it to travel in.
//! * **Ownership is checked on every event, not once at subscribe.** A
//!   [`ModelStreamPublisher`] is bound to one workspace grant, one app epoch,
//!   one task and one run, and it refuses any envelope that disagrees with any
//!   of them, out-of-order sequences included. A stream that has terminated
//!   accepts nothing further.

use std::collections::HashSet;

use desktop_runtime::{ContractId, UnixMillis};
use serde::Serialize;
use thiserror::Error;

/// Ceiling for one encoded catalog projection.
pub const MAX_MODEL_CATALOG_PROJECTION_BYTES: usize = 64 * 1024;

/// Ceiling for one encoded selection projection.
pub const MAX_MODEL_SELECTION_PROJECTION_BYTES: usize = 4 * 1024;

/// Ceiling for one encoded stream-event projection.
pub const MAX_MODEL_STREAM_EVENT_PROJECTION_BYTES: usize = 8 * 1024;

const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_PROFILES: usize = 64;
const MAX_UNAVAILABLE_PROFILES: usize = 64;
const MAX_VARIANTS_PER_PROFILE: usize = 8;
const MIN_VARIANTS_PER_PROFILE: usize = 1;
const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_DISPLAY_NAME_BYTES: usize = 128;
const MAX_DESCRIPTION_BYTES: usize = 256;
const MAX_FAMILY_BYTES: usize = 64;
const MAX_STATUS_DETAIL_BYTES: usize = 160;
const MAX_VARIANT_DISPLAY_NAME_BYTES: usize = 32;
const MAX_REASON_BYTES: usize = 256;
const MAX_TOOL_NAME_BYTES: usize = 128;
/// Mirrors `ProjectedLimits.contextTokens` / `.maxInputTokens` in the canonical
/// schema.
const MAX_CONTEXT_OR_INPUT_TOKENS: u64 = 100_000_000;
/// Mirrors `ProjectedLimits.maxOutputTokens` and `ModelVariantProjection`'s
/// own `maxOutputTokens`.
const MAX_OUTPUT_TOKENS: u64 = 10_000_000;
/// Mirrors `ProjectedLimits.maxAttachmentBytes`.
const MAX_ATTACHMENT_BYTES: u64 = 134_217_728;
/// Ceiling for one forwarded output-text delta. Well under the whole event's
/// [`MAX_MODEL_STREAM_EVENT_PROJECTION_BYTES`] ceiling, which still applies on
/// top of this and is what actually stops an oversized delta.
const MAX_TEXT_DELTA_BYTES: usize = 4 * 1024;
/// Ceiling for one forwarded tool-call-arguments delta.
const MAX_ARGUMENTS_DELTA_BYTES: usize = 4 * 1024;

/// The catalog projection schema the renderer parses.
pub const MODEL_CATALOG_SCHEMA: &str = "sapphirus.model-catalog-projection.v1";

/// The selection projection schema the renderer parses.
pub const MODEL_SELECTION_SCHEMA: &str = "sapphirus.model-selection-projection.v1";

/// The stream-event projection schema the renderer parses.
pub const MODEL_STREAM_EVENT_SCHEMA: &str = "sapphirus.model-stream-event.v1";

/// Substrings that must never appear in catalog display text.
///
/// The type already has nowhere to put a deployment or an endpoint. This is the
/// second lock: it catches a support plane that smuggled one into a name or a
/// summary, which is the only remaining way such a value could reach the
/// renderer through this module.
const DISCLOSURE_MARKERS: [&str; 8] = [
    "http://",
    "https://",
    ".azure.com",
    ".openai.com",
    "api-key",
    "apikey",
    "bearer ",
    "deployment=",
];

/// Why a catalog or selection projection could not be produced.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ModelCatalogProjectionError {
    /// The projection is structurally invalid, inconsistent, or oversized.
    #[error("the model catalog projection is unavailable")]
    Unavailable,
    /// Display text carried control or bidirectional-formatting characters.
    #[error("the model catalog carries text that is unsafe to display")]
    UnsafeDisplayText,
    /// Display text carried a connection detail the renderer must never see.
    #[error("the model catalog would disclose connection authority")]
    DisclosesConnectionAuthority,
}

/// Why one stream event was refused.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ModelStreamProjectionError {
    /// The event named a different run than the subscription.
    #[error("the stream event does not belong to this run")]
    WrongRun,
    /// The event named a different task than the subscription.
    #[error("the stream event does not belong to this task")]
    WrongTask,
    /// The event named a different workspace or workspace grant epoch.
    #[error("the stream event does not belong to this workspace grant")]
    WrongWorkspace,
    /// The event was minted under a superseded model identity epoch.
    #[error("the stream event was issued under a superseded app epoch")]
    StaleAppEpoch,
    /// The sequence did not strictly increase.
    #[error("the stream sequence is not strictly increasing")]
    SequenceOutOfOrder,
    /// The run already terminated.
    #[error("the stream already reached a terminal event")]
    AlreadyTerminal,
    /// The event is structurally invalid or oversized.
    #[error("the stream event projection is unavailable")]
    Unavailable,
    /// The event carried text that is unsafe to display.
    #[error("the stream event carries text that is unsafe to display")]
    UnsafeDisplayText,
}

/// Freshness of the deployment snapshot behind a catalog.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelCatalogStateProjection {
    /// The snapshot is current.
    Fresh,
    /// The snapshot is past its refresh interval but still usable.
    Stale,
    /// The support plane reported a partial snapshot.
    Degraded,
}

/// Availability of a profile the user may invoke.
///
/// Deliberately a separate closed vocabulary from
/// [`ModelBlockedAvailabilityProjection`]: the canonical contract splits them
/// so a selectable projection cannot describe itself as blocked.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelSelectableAvailabilityProjection {
    /// Fully usable.
    Available,
    /// Usable, with a short qualifier in `statusDetail`.
    Degraded,
}

/// How much a variant is asked to reason before answering.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelReasoningEffortProjection {
    /// No deliberate reasoning step.
    None,
    /// Low effort.
    Low,
    /// Medium effort.
    Medium,
    /// High effort.
    High,
}

/// What a model profile can do. Every field is a fact the organization
/// published, not a connection detail: booleans cannot carry a deployment
/// name or an endpoint, so nothing here needed a separate input/output split.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(
    clippy::struct_excessive_bools,
    reason = "mirrors the canonical ProjectedCapabilities schema field for field; each \
              boolean is an independent published capability, not a state machine"
)]
pub struct ModelCapabilitiesProjection {
    /// Accepts text input.
    pub text_input: bool,
    /// Accepts image input.
    pub image_input: bool,
    /// Accepts PDF input.
    pub pdf_input: bool,
    /// Accepts file attachments.
    pub attachments: bool,
    /// Supports tool calling.
    pub tool_calling: bool,
    /// Supports structured output.
    pub structured_output: bool,
    /// Supports an explicit reasoning step.
    pub reasoning: bool,
}

/// Limits Forge will actually enforce for one profile.
///
/// Every field is nullable and that nullability is load-bearing: `None` means
/// the organization published no qualified value for it, and the renderer
/// must show that as unknown, never as zero or unlimited. Nothing here is
/// collapsed to a default in this module or any caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelLimitsProjection {
    /// The context window, in tokens.
    pub context_tokens: Option<u64>,
    /// The maximum input size, in tokens.
    pub max_input_tokens: Option<u64>,
    /// The maximum output size, in tokens.
    pub max_output_tokens: Option<u64>,
    /// The maximum total attachment size, in bytes.
    pub max_attachment_bytes: Option<u64>,
}

/// One named operating point of a profile, as the host holds it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelVariantInput {
    /// The stable variant identifier.
    pub variant_id: String,
    /// The operator-authored display name.
    pub display_name: String,
    /// How much this variant reasons before answering.
    pub reasoning_effort: ModelReasoningEffortProjection,
    /// The output budget for this variant.
    pub max_output_tokens: u64,
}

/// One named operating point of a profile, as the renderer sees it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelVariantProjection {
    variant_id: String,
    display_name: String,
    reasoning_effort: ModelReasoningEffortProjection,
    max_output_tokens: u64,
}

/// One selectable profile, as the host holds it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelProfileInput {
    /// The stable profile identifier.
    pub profile_id: String,
    /// The operator-authored display name.
    pub display_name: String,
    /// The operator-authored description.
    pub description: String,
    /// The model family, when the organization named one.
    pub family: Option<String>,
    /// Whether this profile is fully available or degraded right now.
    pub availability: ModelSelectableAvailabilityProjection,
    /// A short qualifier for a degraded profile.
    pub status_detail: Option<String>,
    /// What this profile can do.
    pub capabilities: ModelCapabilitiesProjection,
    /// The limits Forge will enforce for this profile.
    pub limits: ModelLimitsProjection,
    /// The selectable variants of this profile, in catalog order.
    pub variants: Vec<ModelVariantInput>,
    /// The variant selected when none is named explicitly.
    pub default_variant_id: String,
    /// When this profile's qualification evidence was recorded, if the
    /// organization published one. Carried through as the canonical
    /// `UtcInstant` string rather than converted to a numeric instant: the
    /// renderer only ever displays this value, and a string preserves the
    /// guarantee that what is shown is exactly what the catalog published,
    /// with no crate needed to parse the fixed-width shape.
    pub qualified_at: Option<String>,
}

/// One selectable profile, as the renderer sees it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProfileProjection {
    profile_id: String,
    display_name: String,
    description: String,
    family: Option<String>,
    /// Constant across every profile: the canonical contract fixes this as
    /// `microsoft_foundry` so the field can carry a value without ever
    /// becoming a place a different provider identity could leak through.
    provider: &'static str,
    availability: ModelSelectableAvailabilityProjection,
    status_detail: Option<String>,
    capabilities: ModelCapabilitiesProjection,
    limits: ModelLimitsProjection,
    variants: Vec<ModelVariantProjection>,
    default_variant_id: String,
    qualified_at: Option<String>,
}

/// The only provider a projected profile may name. A constant, not a lookup,
/// because Azure authority is not representable here at all.
const MODEL_PROFILE_PROVIDER: &str = "microsoft_foundry";

/// Why a visible profile is not currently invocable.
///
/// A closed vocabulary rather than the catalog's free text. `DeploymentMissing`
/// is the reason a profile is blocked; it is not, and cannot become, the name
/// of a deployment.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelBlockedAvailabilityProjection {
    /// Organization policy forbids it.
    PolicyBlocked,
    /// The organization is not entitled to it.
    NotEntitled,
    /// It has not completed qualification.
    Unqualified,
    /// Its backing deployment is absent.
    DeploymentMissing,
    /// The catalog snapshot behind it is too old to trust.
    CatalogStale,
}

/// What the catalog says would restore a blocked profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelRecoveryActionProjection {
    /// Refresh the catalog.
    RefreshCatalog,
    /// Ask the organization for an entitlement.
    RequestEntitlement,
    /// Wait for qualification to finish.
    AwaitingQualification,
    /// Escalate to an administrator.
    ContactAdministrator,
    /// Choose a different profile.
    SelectAnotherProfile,
}

/// One profile the catalog lists but cannot currently invoke.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelUnavailableProfileInput {
    /// The stable profile identifier.
    pub profile_id: String,
    /// The operator-authored display name.
    pub display_name: String,
    /// Why it cannot be invoked.
    pub availability: ModelBlockedAvailabilityProjection,
    /// What would restore it.
    pub recovery_action: ModelRecoveryActionProjection,
}

/// One unavailable profile, as the renderer sees it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUnavailableProfileProjection {
    profile_id: String,
    display_name: String,
    availability: ModelBlockedAvailabilityProjection,
    recovery_action: ModelRecoveryActionProjection,
}

/// Everything the host offers for one catalog projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelCatalogInput {
    /// The workspace the catalog was resolved for.
    pub workspace_id: ContractId,
    /// The grant epoch that authorized the resolution.
    pub workspace_grant_epoch: u64,
    /// The model identity epoch the resolution was performed under.
    pub app_epoch: u64,
    /// The monotonic catalog epoch.
    pub catalog_epoch: u64,
    /// The catalog version string.
    pub catalog_version: String,
    /// Freshness of the deployment snapshot.
    pub catalog_state: ModelCatalogStateProjection,
    /// The catalog's own default, when it named one that is selectable.
    pub default_profile_id: Option<String>,
    /// Selectable profiles, in catalog order.
    pub profiles: Vec<ModelProfileInput>,
    /// Listed-but-blocked profiles, in catalog order.
    pub unavailable_profiles: Vec<ModelUnavailableProfileInput>,
    /// When the support plane generated the catalog.
    pub generated_at: UnixMillis,
    /// When the catalog stops being usable.
    pub expires_at: UnixMillis,
    /// When the host last accepted a catalog from the support plane.
    pub refreshed_at: UnixMillis,
}

/// The renderer-visible catalog.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalogProjection {
    schema_version: &'static str,
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    app_epoch: u64,
    catalog_epoch: u64,
    catalog_version: String,
    catalog_state: ModelCatalogStateProjection,
    default_profile_id: Option<String>,
    profiles: Vec<ModelProfileProjection>,
    unavailable_profiles: Vec<ModelUnavailableProfileProjection>,
    generated_at: UnixMillis,
    expires_at: UnixMillis,
    refreshed_at: UnixMillis,
}

/// Where a resolved selection came from.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelSelectionSourceProjection {
    /// The user's current selection was still available.
    CurrentSelection,
    /// The most recent still-valid prior selection.
    RecentSelection,
    /// The catalog's own default.
    CatalogDefault,
    /// The deterministic first selectable profile.
    FirstSelectableProfile,
}

/// Why a persisted selection can no longer be honored.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelSelectionBlockedProjection {
    /// The profile is absent from the catalog entirely.
    ProfileRemoved,
    /// The profile is visible but not currently invocable.
    ProfileUnavailable,
    /// The profile is selectable but the chosen variant is gone.
    VariantRemoved,
}

/// The three terminal shapes of resolving a selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelSelectionStatusProjection {
    /// A model was resolved without asking the user anything.
    Selected,
    /// The user's own selection is gone or blocked and must be re-chosen.
    RequiresExplicitChoice,
    /// The catalog offers nothing selectable at all.
    NoneSelectable,
}

/// What resolving a selection produced, as the host holds it.
///
/// This mirrors `desktop_cloud::ModelSelectionOutcome` one variant for one
/// variant. It is a separate type because this crate is deliberately not
/// allowed to depend on the transport crate, and because collapsing
/// [`Self::RequiresExplicitChoice`] into a nullable selection is exactly the
/// mistake that would silently switch a model under a user mid-draft.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelSelectionOutcomeInput {
    /// A model was resolved.
    Selected {
        /// The resolved profile.
        profile_id: String,
        /// The resolved variant.
        variant_id: String,
        /// Which rule produced it.
        source: ModelSelectionSourceProjection,
    },
    /// The user's own selection can no longer be honored. Nothing here names a
    /// replacement, and nothing downstream may invent one.
    RequiresExplicitChoice {
        /// The profile that can no longer be honored.
        previous_profile_id: String,
        /// The variant that can no longer be honored.
        previous_variant_id: String,
        /// Why, in the catalog's own vocabulary.
        blocked: ModelSelectionBlockedProjection,
    },
    /// The catalog offers nothing selectable.
    NoneSelectable,
}

/// Everything the host offers for one selection projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelSelectionInput {
    /// The workspace the selection belongs to.
    pub workspace_id: ContractId,
    /// The grant epoch that authorized the resolution.
    pub workspace_grant_epoch: u64,
    /// The model identity epoch the resolution was performed under.
    pub app_epoch: u64,
    /// The monotonic catalog epoch the resolution ran against.
    pub catalog_epoch: u64,
    /// The catalog version the resolution ran against.
    pub catalog_version: String,
    /// The outcome.
    pub outcome: ModelSelectionOutcomeInput,
}

/// The renderer-visible selection.
///
/// `profileId` and `variantId` are populated only for
/// [`ModelSelectionStatusProjection::Selected`]. For
/// [`ModelSelectionStatusProjection::RequiresExplicitChoice`] they stay null
/// and `previousProfileId` names what was lost, so a renderer that reads only
/// `profileId` shows nothing rather than the wrong model.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSelectionProjection {
    schema_version: &'static str,
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    app_epoch: u64,
    catalog_epoch: u64,
    catalog_version: String,
    status: ModelSelectionStatusProjection,
    profile_id: Option<String>,
    variant_id: Option<String>,
    source: Option<ModelSelectionSourceProjection>,
    previous_profile_id: Option<String>,
    previous_variant_id: Option<String>,
    blocked: Option<ModelSelectionBlockedProjection>,
}

impl ModelSelectionProjection {
    /// Whether this selection requires the user to choose again.
    #[must_use]
    pub const fn requires_explicit_choice(&self) -> bool {
        matches!(
            self.status,
            ModelSelectionStatusProjection::RequiresExplicitChoice
        )
    }

    /// The resolved profile, which exists only for a resolved selection.
    #[must_use]
    pub fn profile_id(&self) -> Option<&str> {
        self.profile_id.as_deref()
    }
}

/// How a terminated run ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelStreamTerminalProjection {
    /// The run completed and left a receipt.
    Completed,
    /// The provider refused the request.
    Refused,
    /// The run failed.
    Failed,
    /// The run was cancelled.
    Cancelled,
}

/// Who cancelled a run.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelStreamCancellationSourceProjection {
    /// The user asked for it through `model.response.stop`.
    User,
    /// The host withdrew the run.
    Host,
    /// The provider ended it.
    Provider,
}

/// The receipt summary the renderer is allowed to see.
///
/// Counts and a status. No prompt, no response text, no token, no endpoint, no
/// deployment name, and no Azure identifier — none of which has a field here to
/// live in.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelReceiptSummaryInput {
    /// The receipt identifier.
    pub receipt_id: ContractId,
    /// How many stream events the run produced.
    pub event_count: u64,
    /// How many bytes of answer the run produced.
    pub output_byte_count: u64,
    /// Whether the host truncated the answer at its ceiling.
    pub truncated: bool,
    /// When the host recorded the receipt.
    pub recorded_at: UnixMillis,
}

/// The renderer-visible receipt summary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelReceiptSummaryProjection {
    receipt_id: ContractId,
    event_count: u64,
    output_byte_count: u64,
    truncated: bool,
    recorded_at: UnixMillis,
}

/// One stream event, as the host holds it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelStreamEventInput {
    /// The provider accepted the request and named the operating point.
    Started {
        /// The profile the provider actually resolved.
        resolved_profile_id: String,
        /// The variant the provider actually resolved.
        resolved_variant_id: String,
    },
    /// The answer grew. `delta_byte_count` is kept alongside the text for
    /// budget display; the text itself is display-only, never evidence — see
    /// the module doc.
    OutputProgress {
        /// Which output slot advanced.
        output_index: u8,
        /// The fragment of the visible answer that just arrived.
        text_delta: String,
        /// How many bytes it advanced by.
        delta_byte_count: u64,
    },
    /// A tool call opened.
    ToolCallStarted {
        /// The provider's identifier for the call.
        tool_call_id: String,
        /// The tool that was asked for.
        tool_name: String,
    },
    /// A fragment of the arguments a tool call is assembling. Display-only,
    /// same as [`Self::OutputProgress`]: the support plane validates the
    /// completed arguments against the declared schema before the call may
    /// run, so this is what lets the renderer show a reviewable typed
    /// proposal taking shape, not an authority to act on a partial document.
    ToolCallArguments {
        /// The provider's identifier for the call.
        tool_call_id: String,
        /// The fragment of arguments that just arrived.
        arguments_delta: String,
        /// How many bytes it advanced by.
        delta_byte_count: u64,
    },
    /// A tool call closed.
    ToolCallCompleted {
        /// The provider's identifier for the call.
        tool_call_id: String,
        /// How large the complete arguments were.
        arguments_byte_count: u64,
    },
    /// The run terminated.
    Terminal {
        /// How it ended.
        terminal: ModelStreamTerminalProjection,
        /// The receipt, present only for a completed run.
        receipt: Option<ModelReceiptSummaryInput>,
        /// Who cancelled, present only for a cancelled run.
        cancellation_source: Option<ModelStreamCancellationSourceProjection>,
        /// A short, secret-free reason for a refusal or a failure.
        reason_code: Option<String>,
        /// Whether a failure may be retried.
        retryable: bool,
    },
}

impl ModelStreamEventInput {
    const fn is_terminal(&self) -> bool {
        matches!(self, Self::Terminal { .. })
    }
}

/// One stream event together with the identity it claims.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelStreamEventEnvelope {
    /// The workspace the run belongs to.
    pub workspace_id: ContractId,
    /// The grant epoch the run was authorized under.
    pub workspace_grant_epoch: u64,
    /// The model identity epoch the run was minted under.
    pub app_epoch: u64,
    /// The task that owns the run.
    pub task_id: ContractId,
    /// The run itself.
    pub run_id: ContractId,
    /// The strictly increasing sequence within the run.
    pub sequence: u64,
    /// When the host observed the event.
    pub occurred_at: UnixMillis,
    /// The event.
    pub event: ModelStreamEventInput,
}

/// The renderer-visible stream event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStreamEventProjection {
    schema_version: &'static str,
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    app_epoch: u64,
    task_id: ContractId,
    run_id: ContractId,
    sequence: u64,
    occurred_at: UnixMillis,
    #[serde(flatten)]
    body: ModelStreamEventBodyProjection,
}

impl ModelStreamEventProjection {
    /// The run this event belongs to.
    #[must_use]
    pub const fn run_id(&self) -> &ContractId {
        &self.run_id
    }

    /// The task that owns the run this event belongs to.
    #[must_use]
    pub const fn task_id(&self) -> &ContractId {
        &self.task_id
    }

    /// The strictly increasing sequence within the run.
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
enum ModelStreamEventBodyProjection {
    Started {
        #[serde(rename = "resolvedProfileId")]
        resolved_profile_id: String,
        #[serde(rename = "resolvedVariantId")]
        resolved_variant_id: String,
    },
    OutputProgress {
        #[serde(rename = "outputIndex")]
        output_index: u8,
        #[serde(rename = "textDelta")]
        text_delta: String,
        #[serde(rename = "deltaByteCount")]
        delta_byte_count: u64,
    },
    ToolCallStarted {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "toolName")]
        tool_name: String,
    },
    ToolCallArguments {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "argumentsDelta")]
        arguments_delta: String,
        #[serde(rename = "deltaByteCount")]
        delta_byte_count: u64,
    },
    ToolCallCompleted {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "argumentsByteCount")]
        arguments_byte_count: u64,
    },
    Terminal {
        terminal: ModelStreamTerminalProjection,
        receipt: Option<ModelReceiptSummaryProjection>,
        #[serde(rename = "cancellationSource")]
        cancellation_source: Option<ModelStreamCancellationSourceProjection>,
        #[serde(rename = "reasonCode")]
        reason_code: Option<String>,
        retryable: bool,
    },
}

/// The identity one stream subscriber is bound to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelStreamSubscription {
    /// The workspace the subscriber holds a grant for.
    pub workspace_id: ContractId,
    /// The grant epoch it holds.
    pub workspace_grant_epoch: u64,
    /// The model identity epoch it subscribed under.
    pub app_epoch: u64,
    /// The task it owns.
    pub task_id: ContractId,
    /// The run it asked for.
    pub run_id: ContractId,
}

impl ModelStreamSubscription {
    /// Whether this subscriber may be told about `envelope`.
    ///
    /// Every identity field must agree. A subscriber that holds a stale grant
    /// epoch or a superseded app epoch is refused even when the run and the
    /// task match, because both of those epochs are how authority is withdrawn.
    ///
    /// # Errors
    ///
    /// Returns the specific [`ModelStreamProjectionError`] for the field that
    /// disagreed, so a refusal is attributable rather than generic.
    pub fn authorize(
        &self,
        envelope: &ModelStreamEventEnvelope,
    ) -> Result<(), ModelStreamProjectionError> {
        if self.workspace_id != envelope.workspace_id
            || self.workspace_grant_epoch != envelope.workspace_grant_epoch
        {
            return Err(ModelStreamProjectionError::WrongWorkspace);
        }
        if self.app_epoch != envelope.app_epoch {
            return Err(ModelStreamProjectionError::StaleAppEpoch);
        }
        if self.task_id != envelope.task_id {
            return Err(ModelStreamProjectionError::WrongTask);
        }
        if self.run_id != envelope.run_id {
            return Err(ModelStreamProjectionError::WrongRun);
        }
        Ok(())
    }
}

/// Publishes one run's events to one subscriber, in order, exactly once.
#[derive(Clone, Debug)]
pub struct ModelStreamPublisher {
    subscription: ModelStreamSubscription,
    last_sequence: Option<u64>,
    terminated: bool,
}

impl ModelStreamPublisher {
    /// Binds a publisher to one subscriber's identity.
    #[must_use]
    pub const fn new(subscription: ModelStreamSubscription) -> Self {
        Self {
            subscription,
            last_sequence: None,
            terminated: false,
        }
    }

    /// Whether this run has already reached a terminal event.
    #[must_use]
    pub const fn is_terminated(&self) -> bool {
        self.terminated
    }

    /// Projects one event after checking that it belongs to this subscriber.
    ///
    /// The order is deliberate: ownership first, then sequencing, then content.
    /// An envelope from another run must be refused as another run's rather
    /// than as a sequence error, because the two mean very different things
    /// when they show up in a diagnostic.
    ///
    /// # Errors
    ///
    /// Returns [`ModelStreamProjectionError::WrongWorkspace`],
    /// [`ModelStreamProjectionError::StaleAppEpoch`],
    /// [`ModelStreamProjectionError::WrongTask`] or
    /// [`ModelStreamProjectionError::WrongRun`] when the envelope does not
    /// belong to this subscriber; [`ModelStreamProjectionError::AlreadyTerminal`]
    /// after the run has ended; [`ModelStreamProjectionError::SequenceOutOfOrder`]
    /// when the sequence does not strictly increase; and
    /// [`ModelStreamProjectionError::Unavailable`] or
    /// [`ModelStreamProjectionError::UnsafeDisplayText`] when the body itself
    /// cannot be projected.
    pub fn publish(
        &mut self,
        envelope: &ModelStreamEventEnvelope,
    ) -> Result<ModelStreamEventProjection, ModelStreamProjectionError> {
        self.subscription.authorize(envelope)?;
        if self.terminated {
            return Err(ModelStreamProjectionError::AlreadyTerminal);
        }
        if envelope.sequence == 0 || envelope.sequence > MAX_SAFE_JSON_INTEGER {
            return Err(ModelStreamProjectionError::Unavailable);
        }
        if self
            .last_sequence
            .is_some_and(|previous| envelope.sequence <= previous)
        {
            return Err(ModelStreamProjectionError::SequenceOutOfOrder);
        }
        if envelope.occurred_at.0 > MAX_SAFE_JSON_INTEGER {
            return Err(ModelStreamProjectionError::Unavailable);
        }
        let body = project_stream_body(&envelope.event)?;
        let projection = ModelStreamEventProjection {
            schema_version: MODEL_STREAM_EVENT_SCHEMA,
            workspace_id: envelope.workspace_id.clone(),
            workspace_grant_epoch: envelope.workspace_grant_epoch,
            app_epoch: envelope.app_epoch,
            task_id: envelope.task_id.clone(),
            run_id: envelope.run_id.clone(),
            sequence: envelope.sequence,
            occurred_at: envelope.occurred_at,
            body,
        };
        let encoded = serde_json::to_vec(&projection)
            .map_err(|_| ModelStreamProjectionError::Unavailable)?;
        if encoded.len() > MAX_MODEL_STREAM_EVENT_PROJECTION_BYTES {
            return Err(ModelStreamProjectionError::Unavailable);
        }
        // State advances only once the event is fully accepted, so a refused
        // event never consumes a sequence number or closes the run.
        self.last_sequence = Some(envelope.sequence);
        self.terminated = envelope.event.is_terminal();
        Ok(projection)
    }
}

fn project_stream_body(
    event: &ModelStreamEventInput,
) -> Result<ModelStreamEventBodyProjection, ModelStreamProjectionError> {
    match event {
        ModelStreamEventInput::Started {
            resolved_profile_id,
            resolved_variant_id,
        } => {
            check_identifier(resolved_profile_id)?;
            check_identifier(resolved_variant_id)?;
            Ok(ModelStreamEventBodyProjection::Started {
                resolved_profile_id: resolved_profile_id.clone(),
                resolved_variant_id: resolved_variant_id.clone(),
            })
        }
        ModelStreamEventInput::OutputProgress {
            output_index,
            text_delta,
            delta_byte_count,
        } => {
            if *delta_byte_count == 0 || *delta_byte_count > MAX_SAFE_JSON_INTEGER {
                return Err(ModelStreamProjectionError::Unavailable);
            }
            check_stream_text(text_delta, MAX_TEXT_DELTA_BYTES)?;
            Ok(ModelStreamEventBodyProjection::OutputProgress {
                output_index: *output_index,
                text_delta: text_delta.clone(),
                delta_byte_count: *delta_byte_count,
            })
        }
        ModelStreamEventInput::ToolCallStarted {
            tool_call_id,
            tool_name,
        } => {
            check_identifier(tool_call_id)?;
            check_display_text(tool_name, MAX_TOOL_NAME_BYTES)?;
            Ok(ModelStreamEventBodyProjection::ToolCallStarted {
                tool_call_id: tool_call_id.clone(),
                tool_name: tool_name.clone(),
            })
        }
        ModelStreamEventInput::ToolCallArguments {
            tool_call_id,
            arguments_delta,
            delta_byte_count,
        } => {
            check_identifier(tool_call_id)?;
            if *delta_byte_count == 0 || *delta_byte_count > MAX_SAFE_JSON_INTEGER {
                return Err(ModelStreamProjectionError::Unavailable);
            }
            check_stream_text(arguments_delta, MAX_ARGUMENTS_DELTA_BYTES)?;
            Ok(ModelStreamEventBodyProjection::ToolCallArguments {
                tool_call_id: tool_call_id.clone(),
                arguments_delta: arguments_delta.clone(),
                delta_byte_count: *delta_byte_count,
            })
        }
        ModelStreamEventInput::ToolCallCompleted {
            tool_call_id,
            arguments_byte_count,
        } => {
            check_identifier(tool_call_id)?;
            if *arguments_byte_count > MAX_SAFE_JSON_INTEGER {
                return Err(ModelStreamProjectionError::Unavailable);
            }
            Ok(ModelStreamEventBodyProjection::ToolCallCompleted {
                tool_call_id: tool_call_id.clone(),
                arguments_byte_count: *arguments_byte_count,
            })
        }
        ModelStreamEventInput::Terminal {
            terminal,
            receipt,
            cancellation_source,
            reason_code,
            retryable,
        } => project_terminal(
            *terminal,
            receipt.as_ref(),
            *cancellation_source,
            reason_code.as_deref(),
            *retryable,
        ),
    }
}

fn project_terminal(
    terminal: ModelStreamTerminalProjection,
    receipt: Option<&ModelReceiptSummaryInput>,
    cancellation_source: Option<ModelStreamCancellationSourceProjection>,
    reason_code: Option<&str>,
    retryable: bool,
) -> Result<ModelStreamEventBodyProjection, ModelStreamProjectionError> {
    // Each terminal shape carries exactly its own evidence. A completed run
    // that also claimed a cancellation source, or a cancelled run that also
    // produced a receipt, is a host bug rather than something to render.
    let shape_is_consistent = match terminal {
        ModelStreamTerminalProjection::Completed => {
            receipt.is_some() && cancellation_source.is_none() && reason_code.is_none() && !retryable
        }
        ModelStreamTerminalProjection::Refused => {
            receipt.is_none() && cancellation_source.is_none() && !retryable
        }
        ModelStreamTerminalProjection::Failed => {
            receipt.is_none() && cancellation_source.is_none()
        }
        ModelStreamTerminalProjection::Cancelled => {
            receipt.is_none() && cancellation_source.is_some() && reason_code.is_none() && !retryable
        }
    };
    if !shape_is_consistent {
        return Err(ModelStreamProjectionError::Unavailable);
    }
    if let Some(reason_code) = reason_code {
        check_display_text(reason_code, MAX_REASON_BYTES)?;
    }
    let receipt = match receipt {
        Some(receipt) => Some(project_receipt_summary(receipt)?),
        None => None,
    };
    Ok(ModelStreamEventBodyProjection::Terminal {
        terminal,
        receipt,
        cancellation_source,
        reason_code: reason_code.map(str::to_owned),
        retryable,
    })
}

fn project_receipt_summary(
    input: &ModelReceiptSummaryInput,
) -> Result<ModelReceiptSummaryProjection, ModelStreamProjectionError> {
    if input.event_count > MAX_SAFE_JSON_INTEGER
        || input.output_byte_count > MAX_SAFE_JSON_INTEGER
        || input.recorded_at.0 > MAX_SAFE_JSON_INTEGER
    {
        return Err(ModelStreamProjectionError::Unavailable);
    }
    Ok(ModelReceiptSummaryProjection {
        receipt_id: input.receipt_id.clone(),
        event_count: input.event_count,
        output_byte_count: input.output_byte_count,
        truncated: input.truncated,
        recorded_at: input.recorded_at,
    })
}

/// Projects one selectable profile.
///
/// Split out of [`project_model_catalog`] to keep that function under the
/// repo's line ceiling; the two profile-uniqueness checks (identifier
/// uniqueness across selectable and unavailable profiles) stay in the caller
/// because they need the shared `seen_profiles` set.
fn project_profile(
    profile: &ModelProfileInput,
) -> Result<ModelProfileProjection, ModelCatalogProjectionError> {
    check_catalog_identifier(&profile.profile_id)?;
    check_catalog_display_text(&profile.display_name, MAX_DISPLAY_NAME_BYTES)?;
    check_catalog_display_text(&profile.description, MAX_DESCRIPTION_BYTES)?;
    if let Some(family) = profile.family.as_deref() {
        check_catalog_display_text(family, MAX_FAMILY_BYTES)?;
    }
    if let Some(status_detail) = profile.status_detail.as_deref() {
        check_catalog_display_text(status_detail, MAX_STATUS_DETAIL_BYTES)?;
    }
    check_limits(&profile.limits)?;
    if let Some(qualified_at) = profile.qualified_at.as_deref() {
        check_utc_instant(qualified_at)?;
    }
    if profile.variants.len() < MIN_VARIANTS_PER_PROFILE
        || profile.variants.len() > MAX_VARIANTS_PER_PROFILE
    {
        return Err(ModelCatalogProjectionError::Unavailable);
    }
    let mut seen_variants: HashSet<&str> = HashSet::new();
    let mut variants = Vec::with_capacity(profile.variants.len());
    for variant in &profile.variants {
        check_catalog_identifier(&variant.variant_id)?;
        if !seen_variants.insert(variant.variant_id.as_str()) {
            return Err(ModelCatalogProjectionError::Unavailable);
        }
        check_catalog_display_text(&variant.display_name, MAX_VARIANT_DISPLAY_NAME_BYTES)?;
        if variant.max_output_tokens == 0 || variant.max_output_tokens > MAX_OUTPUT_TOKENS {
            return Err(ModelCatalogProjectionError::Unavailable);
        }
        variants.push(ModelVariantProjection {
            variant_id: variant.variant_id.clone(),
            display_name: variant.display_name.clone(),
            reasoning_effort: variant.reasoning_effort,
            max_output_tokens: variant.max_output_tokens,
        });
    }
    check_catalog_identifier(&profile.default_variant_id)?;
    if !seen_variants.contains(profile.default_variant_id.as_str()) {
        return Err(ModelCatalogProjectionError::Unavailable);
    }
    Ok(ModelProfileProjection {
        profile_id: profile.profile_id.clone(),
        display_name: profile.display_name.clone(),
        description: profile.description.clone(),
        family: profile.family.clone(),
        provider: MODEL_PROFILE_PROVIDER,
        availability: profile.availability,
        status_detail: profile.status_detail.clone(),
        capabilities: profile.capabilities,
        limits: profile.limits,
        variants,
        default_variant_id: profile.default_variant_id.clone(),
        qualified_at: profile.qualified_at.clone(),
    })
}

/// Projects one verified catalog for the renderer.
///
/// Everything is checked here rather than trusted from the transport: profile
/// and variant identifiers are bounded and unique, every profile has at least
/// one variant, a stated default must actually be selectable, and no identifier
/// may appear in both the selectable and the unavailable list.
///
/// # Errors
///
/// Returns [`ModelCatalogProjectionError::UnsafeDisplayText`] for control or
/// bidirectional-formatting characters in display text,
/// [`ModelCatalogProjectionError::DisclosesConnectionAuthority`] when display
/// text carries a connection detail, and
/// [`ModelCatalogProjectionError::Unavailable`] for any structural
/// inconsistency, an out-of-range instant or epoch, or an oversized projection.
pub fn project_model_catalog(
    input: ModelCatalogInput,
) -> Result<ModelCatalogProjection, ModelCatalogProjectionError> {
    if input.workspace_grant_epoch == 0
        || input.workspace_grant_epoch > MAX_SAFE_JSON_INTEGER
        || input.app_epoch == 0
        || input.app_epoch > MAX_SAFE_JSON_INTEGER
        || input.catalog_epoch == 0
        || input.catalog_epoch > MAX_SAFE_JSON_INTEGER
    {
        return Err(ModelCatalogProjectionError::Unavailable);
    }
    if input.generated_at.0 > MAX_SAFE_JSON_INTEGER
        || input.expires_at.0 > MAX_SAFE_JSON_INTEGER
        || input.refreshed_at.0 > MAX_SAFE_JSON_INTEGER
        || input.expires_at.0 <= input.generated_at.0
    {
        return Err(ModelCatalogProjectionError::Unavailable);
    }
    check_catalog_identifier(&input.catalog_version)?;
    if input.profiles.len() > MAX_PROFILES
        || input.unavailable_profiles.len() > MAX_UNAVAILABLE_PROFILES
    {
        return Err(ModelCatalogProjectionError::Unavailable);
    }

    let mut seen_profiles: HashSet<&str> = HashSet::new();
    let mut profiles = Vec::with_capacity(input.profiles.len());
    for profile in &input.profiles {
        check_catalog_identifier(&profile.profile_id)?;
        if !seen_profiles.insert(profile.profile_id.as_str()) {
            return Err(ModelCatalogProjectionError::Unavailable);
        }
        profiles.push(project_profile(profile)?);
    }

    let mut unavailable_profiles = Vec::with_capacity(input.unavailable_profiles.len());
    for profile in &input.unavailable_profiles {
        check_catalog_identifier(&profile.profile_id)?;
        // A profile cannot be both selectable and blocked. Allowing it would
        // let the renderer offer a model the host would then refuse to invoke.
        if !seen_profiles.insert(profile.profile_id.as_str()) {
            return Err(ModelCatalogProjectionError::Unavailable);
        }
        check_catalog_display_text(&profile.display_name, MAX_DISPLAY_NAME_BYTES)?;
        unavailable_profiles.push(ModelUnavailableProfileProjection {
            profile_id: profile.profile_id.clone(),
            display_name: profile.display_name.clone(),
            availability: profile.availability,
            recovery_action: profile.recovery_action,
        });
    }

    if let Some(default_profile_id) = input.default_profile_id.as_deref() {
        check_catalog_identifier(default_profile_id)?;
        if !input
            .profiles
            .iter()
            .any(|profile| profile.profile_id == default_profile_id)
        {
            return Err(ModelCatalogProjectionError::Unavailable);
        }
    }

    let projection = ModelCatalogProjection {
        schema_version: MODEL_CATALOG_SCHEMA,
        workspace_id: input.workspace_id,
        workspace_grant_epoch: input.workspace_grant_epoch,
        app_epoch: input.app_epoch,
        catalog_epoch: input.catalog_epoch,
        catalog_version: input.catalog_version,
        catalog_state: input.catalog_state,
        default_profile_id: input.default_profile_id,
        profiles,
        unavailable_profiles,
        generated_at: input.generated_at,
        expires_at: input.expires_at,
        refreshed_at: input.refreshed_at,
    };
    let encoded =
        serde_json::to_vec(&projection).map_err(|_| ModelCatalogProjectionError::Unavailable)?;
    if encoded.len() > MAX_MODEL_CATALOG_PROJECTION_BYTES {
        return Err(ModelCatalogProjectionError::Unavailable);
    }
    Ok(projection)
}

/// Projects one resolved selection for the renderer.
///
/// [`ModelSelectionOutcomeInput::RequiresExplicitChoice`] is carried through
/// as itself. It is never resolved to a default, to the catalog's first
/// profile, or to anything from the user's history — the whole point of the
/// variant is that a user part-way through a draft must not discover afterwards
/// that a different model answered.
///
/// # Errors
///
/// Returns [`ModelCatalogProjectionError::Unavailable`] for an out-of-range
/// epoch, a malformed identifier, or an oversized projection, and
/// [`ModelCatalogProjectionError::UnsafeDisplayText`] or
/// [`ModelCatalogProjectionError::DisclosesConnectionAuthority`] when an
/// identifier is not safe to display.
pub fn project_model_selection(
    input: ModelSelectionInput,
) -> Result<ModelSelectionProjection, ModelCatalogProjectionError> {
    if input.workspace_grant_epoch == 0
        || input.workspace_grant_epoch > MAX_SAFE_JSON_INTEGER
        || input.app_epoch == 0
        || input.app_epoch > MAX_SAFE_JSON_INTEGER
        || input.catalog_epoch == 0
        || input.catalog_epoch > MAX_SAFE_JSON_INTEGER
    {
        return Err(ModelCatalogProjectionError::Unavailable);
    }
    check_catalog_identifier(&input.catalog_version)?;
    let (status, profile_id, variant_id, source, previous_profile_id, previous_variant_id, blocked) =
        match input.outcome {
            ModelSelectionOutcomeInput::Selected {
                profile_id,
                variant_id,
                source,
            } => {
                check_catalog_identifier(&profile_id)?;
                check_catalog_identifier(&variant_id)?;
                (
                    ModelSelectionStatusProjection::Selected,
                    Some(profile_id),
                    Some(variant_id),
                    Some(source),
                    None,
                    None,
                    None,
                )
            }
            ModelSelectionOutcomeInput::RequiresExplicitChoice {
                previous_profile_id,
                previous_variant_id,
                blocked,
            } => {
                check_catalog_identifier(&previous_profile_id)?;
                check_catalog_identifier(&previous_variant_id)?;
                (
                    ModelSelectionStatusProjection::RequiresExplicitChoice,
                    None,
                    None,
                    None,
                    Some(previous_profile_id),
                    Some(previous_variant_id),
                    Some(blocked),
                )
            }
            ModelSelectionOutcomeInput::NoneSelectable => (
                ModelSelectionStatusProjection::NoneSelectable,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
        };
    let projection = ModelSelectionProjection {
        schema_version: MODEL_SELECTION_SCHEMA,
        workspace_id: input.workspace_id,
        workspace_grant_epoch: input.workspace_grant_epoch,
        app_epoch: input.app_epoch,
        catalog_epoch: input.catalog_epoch,
        catalog_version: input.catalog_version,
        status,
        profile_id,
        variant_id,
        source,
        previous_profile_id,
        previous_variant_id,
        blocked,
    };
    let encoded =
        serde_json::to_vec(&projection).map_err(|_| ModelCatalogProjectionError::Unavailable)?;
    if encoded.len() > MAX_MODEL_SELECTION_PROJECTION_BYTES {
        return Err(ModelCatalogProjectionError::Unavailable);
    }
    Ok(projection)
}

fn check_catalog_identifier(value: &str) -> Result<(), ModelCatalogProjectionError> {
    if value.is_empty() || value.len() > MAX_IDENTIFIER_BYTES {
        return Err(ModelCatalogProjectionError::Unavailable);
    }
    if !value.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':')
    }) {
        return Err(ModelCatalogProjectionError::Unavailable);
    }
    check_disclosure(value).map_err(|()| ModelCatalogProjectionError::DisclosesConnectionAuthority)
}

fn check_catalog_display_text(
    value: &str,
    max_bytes: usize,
) -> Result<(), ModelCatalogProjectionError> {
    if value.is_empty() || value.len() > max_bytes {
        return Err(ModelCatalogProjectionError::Unavailable);
    }
    if contains_unsafe_character(value) {
        return Err(ModelCatalogProjectionError::UnsafeDisplayText);
    }
    check_disclosure(value).map_err(|()| ModelCatalogProjectionError::DisclosesConnectionAuthority)
}

/// The exact length of a canonical `UtcInstant`: `YYYY-MM-DDTHH:MM:SS.mmmZ`.
/// The schema's pattern admits no variation in width, so a length check plus
/// a per-position check is a complete validator -- no date/time crate needed.
const UTC_INSTANT_BYTES: usize = 24;

/// Validates the canonical `UtcInstant` shape byte for byte:
/// `^[0-9]{4}-(0[1-9]|1[0-2])-([0-2][0-9]|3[01])T([01][0-9]|2[0-3]):[0-5][0-9]:[0-5][0-9]\.[0-9]{3}Z$`.
///
/// This is a shape check, not a calendar check: it rejects anything that does
/// not fit the fixed positional grammar, but (like the schema itself) does not
/// reject a calendar-impossible date such as `2026-02-31`. Renderer display
/// text has never needed calendar validity, only a safe, bounded, correctly
/// shaped string to show.
fn check_utc_instant(value: &str) -> Result<(), ModelCatalogProjectionError> {
    let bytes = value.as_bytes();
    let digit = |index: usize| bytes.get(index).copied().filter(u8::is_ascii_digit);
    let matches = bytes.len() == UTC_INSTANT_BYTES
        && (0..4).all(|index| digit(index).is_some())
        && bytes[4] == b'-'
        && matches!(digit(5), Some(b'0'..=b'1')) && digit(6).is_some()
        && matches!((bytes[5], bytes[6]), (b'0', b'1'..=b'9') | (b'1', b'0'..=b'2'))
        && bytes[7] == b'-'
        && digit(8).is_some() && digit(9).is_some()
        && matches!((bytes[8], bytes[9]), (b'0'..=b'2', b'0'..=b'9') | (b'3', b'0'..=b'1'))
        && bytes[10] == b'T'
        && digit(11).is_some() && digit(12).is_some()
        && matches!((bytes[11], bytes[12]), (b'0'..=b'1', b'0'..=b'9') | (b'2', b'0'..=b'3'))
        && bytes[13] == b':'
        && matches!(bytes[14], b'0'..=b'5') && digit(15).is_some()
        && bytes[16] == b':'
        && matches!(bytes[17], b'0'..=b'5') && digit(18).is_some()
        && bytes[19] == b'.'
        && (20..23).all(|index| digit(index).is_some())
        && bytes[23] == b'Z';
    if matches {
        Ok(())
    } else {
        Err(ModelCatalogProjectionError::Unavailable)
    }
}

/// Bounds one profile's limits, leaving `None` exactly as `None`.
///
/// Nullable is load-bearing here: a limit the organization did not publish
/// stays absent, never becomes `0` or a synthesized ceiling. Only a *present*
/// value is range-checked, against the same ceilings the canonical schema
/// declares.
fn check_limits(limits: &ModelLimitsProjection) -> Result<(), ModelCatalogProjectionError> {
    if limits
        .context_tokens
        .is_some_and(|value| value == 0 || value > MAX_CONTEXT_OR_INPUT_TOKENS)
        || limits
            .max_input_tokens
            .is_some_and(|value| value == 0 || value > MAX_CONTEXT_OR_INPUT_TOKENS)
        || limits
            .max_output_tokens
            .is_some_and(|value| value == 0 || value > MAX_OUTPUT_TOKENS)
        || limits
            .max_attachment_bytes
            .is_some_and(|value| value > MAX_ATTACHMENT_BYTES)
    {
        return Err(ModelCatalogProjectionError::Unavailable);
    }
    Ok(())
}

fn check_identifier(value: &str) -> Result<(), ModelStreamProjectionError> {
    if value.is_empty() || value.len() > MAX_IDENTIFIER_BYTES {
        return Err(ModelStreamProjectionError::Unavailable);
    }
    if !value.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':')
    }) {
        return Err(ModelStreamProjectionError::Unavailable);
    }
    check_disclosure(value).map_err(|()| ModelStreamProjectionError::UnsafeDisplayText)
}

fn check_display_text(value: &str, max_bytes: usize) -> Result<(), ModelStreamProjectionError> {
    if value.is_empty() || value.len() > max_bytes {
        return Err(ModelStreamProjectionError::Unavailable);
    }
    if contains_unsafe_character(value) {
        return Err(ModelStreamProjectionError::UnsafeDisplayText);
    }
    check_disclosure(value).map_err(|()| ModelStreamProjectionError::UnsafeDisplayText)
}

/// Bounds and safety-checks one forwarded stream-text fragment.
///
/// Unlike [`check_display_text`] this does not run the disclosure-marker
/// check: `text_delta` and `arguments_delta` carry free-form model output and
/// tool-call arguments, which may legitimately contain a URL, the word
/// "bearer", or similar substrings without disclosing any connection
/// authority -- there is nowhere in this projection for a connection detail to
/// have come from in the first place. Only the same control/bidi-override
/// scan applies, because that is a rendering-safety concern regardless of what
/// the text says.
fn check_stream_text(value: &str, max_bytes: usize) -> Result<(), ModelStreamProjectionError> {
    if value.is_empty() || value.len() > max_bytes {
        return Err(ModelStreamProjectionError::Unavailable);
    }
    if contains_unsafe_character(value) {
        return Err(ModelStreamProjectionError::UnsafeDisplayText);
    }
    Ok(())
}

fn check_disclosure(value: &str) -> Result<(), ()> {
    let folded = value.to_ascii_lowercase();
    if DISCLOSURE_MARKERS
        .iter()
        .any(|marker| folded.contains(marker))
    {
        return Err(());
    }
    Ok(())
}

fn contains_unsafe_character(value: &str) -> bool {
    value.chars().any(|character| {
        character.is_control()
            || matches!(
                character,
                '\u{061c}'
                    | '\u{200e}'
                    | '\u{200f}'
                    | '\u{202a}'..='\u{202e}'
                    | '\u{2066}'..='\u{2069}'
            )
    })
}

#[cfg(test)]
mod tests {
    use desktop_runtime::{ContractId, UnixMillis};

    use super::{
        project_model_catalog, project_model_selection, ModelCapabilitiesProjection,
        ModelCatalogInput, ModelCatalogProjectionError, ModelCatalogStateProjection,
        ModelLimitsProjection, ModelProfileInput, ModelReasoningEffortProjection,
        ModelReceiptSummaryInput, ModelSelectableAvailabilityProjection,
        ModelSelectionBlockedProjection, ModelSelectionInput,
        ModelBlockedAvailabilityProjection, ModelRecoveryActionProjection,
        ModelSelectionOutcomeInput, ModelSelectionSourceProjection,
        ModelStreamCancellationSourceProjection,
        ModelStreamEventEnvelope, ModelStreamEventInput, ModelStreamProjectionError,
        ModelStreamPublisher, ModelStreamSubscription, ModelStreamTerminalProjection,
        ModelUnavailableProfileInput, ModelVariantInput,
    };

    fn id(value: &str) -> Result<ContractId, Box<dyn std::error::Error>> {
        Ok(ContractId::new(value)?)
    }

    fn full_capabilities() -> ModelCapabilitiesProjection {
        ModelCapabilitiesProjection {
            text_input: true,
            image_input: false,
            pdf_input: false,
            attachments: true,
            tool_calling: true,
            structured_output: true,
            reasoning: false,
        }
    }

    /// Every limit unset, on purpose: the null-limit property this projection
    /// is most likely to lose is exercised against this fixture.
    fn no_published_limits() -> ModelLimitsProjection {
        ModelLimitsProjection {
            context_tokens: None,
            max_input_tokens: None,
            max_output_tokens: None,
            max_attachment_bytes: None,
        }
    }

    fn profile(profile_id: &str) -> ModelProfileInput {
        ModelProfileInput {
            profile_id: profile_id.to_owned(),
            display_name: "Balanced reasoning".to_owned(),
            description: "General drafting and review.".to_owned(),
            family: Some("balanced-family".to_owned()),
            availability: ModelSelectableAvailabilityProjection::Available,
            status_detail: None,
            capabilities: full_capabilities(),
            limits: no_published_limits(),
            variants: vec![ModelVariantInput {
                variant_id: "standard".to_owned(),
                display_name: "Standard".to_owned(),
                reasoning_effort: ModelReasoningEffortProjection::Medium,
                max_output_tokens: 4_096,
            }],
            default_variant_id: "standard".to_owned(),
            qualified_at: Some("2026-01-01T00:00:00.000Z".to_owned()),
        }
    }

    fn catalog_input() -> Result<ModelCatalogInput, Box<dyn std::error::Error>> {
        Ok(ModelCatalogInput {
            workspace_id: id("workspace_1")?,
            workspace_grant_epoch: 3,
            app_epoch: 5,
            catalog_epoch: 12,
            catalog_version: "v12.abc1234567890a".to_owned(),
            catalog_state: ModelCatalogStateProjection::Fresh,
            default_profile_id: Some("balanced".to_owned()),
            profiles: vec![profile("balanced"), profile("precise")],
            unavailable_profiles: vec![ModelUnavailableProfileInput {
                profile_id: "vision".to_owned(),
                display_name: "Vision".to_owned(),
                availability: ModelBlockedAvailabilityProjection::DeploymentMissing,
                recovery_action: ModelRecoveryActionProjection::ContactAdministrator,
            }],
            generated_at: UnixMillis(1_000),
            expires_at: UnixMillis(61_000),
            refreshed_at: UnixMillis(1_200),
        })
    }

    fn subscription() -> Result<ModelStreamSubscription, Box<dyn std::error::Error>> {
        Ok(ModelStreamSubscription {
            workspace_id: id("workspace_1")?,
            workspace_grant_epoch: 3,
            app_epoch: 5,
            task_id: id("task_1")?,
            run_id: id("run_1")?,
        })
    }

    fn envelope(
        sequence: u64,
        event: ModelStreamEventInput,
    ) -> Result<ModelStreamEventEnvelope, Box<dyn std::error::Error>> {
        Ok(ModelStreamEventEnvelope {
            workspace_id: id("workspace_1")?,
            workspace_grant_epoch: 3,
            app_epoch: 5,
            task_id: id("task_1")?,
            run_id: id("run_1")?,
            sequence,
            occurred_at: UnixMillis(2_000 + sequence),
            event,
        })
    }

    fn started() -> ModelStreamEventInput {
        ModelStreamEventInput::Started {
            resolved_profile_id: "balanced".to_owned(),
            resolved_variant_id: "standard".to_owned(),
        }
    }

    #[test]
    fn a_catalog_projects_only_display_safe_profile_metadata(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let projection = project_model_catalog(catalog_input()?)?;
        let encoded = serde_json::to_value(&projection)?;
        assert_eq!(
            encoded["schemaVersion"],
            serde_json::json!("sapphirus.model-catalog-projection.v1")
        );
        assert_eq!(encoded["profiles"][0]["profileId"], "balanced");
        assert_eq!(encoded["profiles"][0]["variants"][0]["variantId"], "standard");
        assert_eq!(encoded["profiles"][0]["defaultVariantId"], "standard");
        assert_eq!(encoded["profiles"][0]["provider"], "microsoft_foundry");
        assert_eq!(encoded["profiles"][0]["availability"], "available");
        assert_eq!(encoded["profiles"][0]["capabilities"]["toolCalling"], true);
        // Nullable is load-bearing: the fixture publishes no limits, and that
        // must survive as null, never as 0.
        assert_eq!(encoded["profiles"][0]["limits"]["contextTokens"], serde_json::Value::Null);
        assert_eq!(encoded["unavailableProfiles"][0]["profileId"], "vision");
        // A closed reason vocabulary, not the catalog's free text. "the
        // deployment is missing" is a reason; it is not a deployment name.
        assert_eq!(
            encoded["unavailableProfiles"][0]["availability"],
            "deployment_missing"
        );
        assert_eq!(
            encoded["unavailableProfiles"][0]["recoveryAction"],
            "contact_administrator"
        );

        // The connection vocabulary still has no field to arrive in even
        // though capabilities and limits are now projected: none of these
        // strings should ever appear regardless of what the fixture carries.
        let text = serde_json::to_string(&encoded)?.to_ascii_lowercase();
        for forbidden in ["endpoint", "apikey", "credential", "azure", "deploymentname"] {
            assert!(
                !text.contains(forbidden),
                "catalog projection disclosed {forbidden}"
            );
        }
        Ok(())
    }

    /// The property most likely to be lost by a careless default: a limit the
    /// organization did not publish must stay `null` through the Rust
    /// projection, never collapse to `0` or to an invented ceiling.
    #[test]
    fn a_null_limit_survives_the_rust_projection() -> Result<(), Box<dyn std::error::Error>> {
        let mut input = catalog_input()?;
        input.profiles[0].limits = ModelLimitsProjection {
            context_tokens: Some(128_000),
            max_input_tokens: None,
            max_output_tokens: Some(8_192),
            max_attachment_bytes: None,
        };
        let projection = project_model_catalog(input)?;
        let encoded = serde_json::to_value(&projection)?;
        let limits = &encoded["profiles"][0]["limits"];
        assert_eq!(limits["contextTokens"], serde_json::json!(128_000));
        assert_eq!(limits["maxInputTokens"], serde_json::Value::Null);
        assert_eq!(limits["maxOutputTokens"], serde_json::json!(8_192));
        assert_eq!(limits["maxAttachmentBytes"], serde_json::Value::Null);
        Ok(())
    }

    /// `qualifiedAt` is carried through as the canonical `UtcInstant` string
    /// rather than converted to a numeric instant, so this is a shape check on
    /// that string, not a range check on a parsed value.
    #[test]
    fn a_malformed_qualified_at_is_refused() -> Result<(), Box<dyn std::error::Error>> {
        let mut input = catalog_input()?;
        input.profiles[0].qualified_at = Some("2026-01-01 00:00:00".to_owned());
        assert_eq!(
            project_model_catalog(input),
            Err(ModelCatalogProjectionError::Unavailable)
        );
        Ok(())
    }

    /// A null `qualifiedAt` -- the organization named no qualification
    /// instant -- must still project successfully.
    #[test]
    fn a_null_qualified_at_survives_the_projection() -> Result<(), Box<dyn std::error::Error>> {
        let mut input = catalog_input()?;
        input.profiles[0].qualified_at = None;
        let projection = project_model_catalog(input)?;
        let encoded = serde_json::to_value(&projection)?;
        assert_eq!(encoded["profiles"][0]["qualifiedAt"], serde_json::Value::Null);
        Ok(())
    }

    #[test]
    fn a_profile_name_carrying_an_endpoint_is_refused(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut input = catalog_input()?;
        input.profiles[0].display_name = "Balanced https://contoso.openai.azure.com".to_owned();
        assert_eq!(
            project_model_catalog(input),
            Err(ModelCatalogProjectionError::DisclosesConnectionAuthority)
        );
        Ok(())
    }

    #[test]
    fn a_profile_name_carrying_bidi_override_is_refused() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut input = catalog_input()?;
        input.profiles[1].display_name = "Precise\u{202e}esicerP".to_owned();
        assert_eq!(
            project_model_catalog(input),
            Err(ModelCatalogProjectionError::UnsafeDisplayText)
        );
        Ok(())
    }

    #[test]
    fn a_default_that_is_not_selectable_is_refused() -> Result<(), Box<dyn std::error::Error>> {
        let mut input = catalog_input()?;
        input.default_profile_id = Some("vision".to_owned());
        assert_eq!(
            project_model_catalog(input),
            Err(ModelCatalogProjectionError::Unavailable)
        );
        Ok(())
    }

    #[test]
    fn a_profile_listed_as_both_selectable_and_blocked_is_refused(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut input = catalog_input()?;
        input.unavailable_profiles[0].profile_id = "balanced".to_owned();
        assert_eq!(
            project_model_catalog(input),
            Err(ModelCatalogProjectionError::Unavailable)
        );
        Ok(())
    }

    /// The guard against silently switching a model under a user mid-draft.
    #[test]
    fn requires_explicit_choice_never_names_a_replacement(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let projection = project_model_selection(ModelSelectionInput {
            workspace_id: id("workspace_1")?,
            workspace_grant_epoch: 3,
            app_epoch: 5,
            catalog_epoch: 12,
            catalog_version: "v12.abc1234567890a".to_owned(),
            outcome: ModelSelectionOutcomeInput::RequiresExplicitChoice {
                previous_profile_id: "balanced".to_owned(),
                previous_variant_id: "standard".to_owned(),
                blocked: ModelSelectionBlockedProjection::ProfileRemoved,
            },
        })?;
        assert!(projection.requires_explicit_choice());
        assert_eq!(projection.profile_id(), None);
        let encoded = serde_json::to_value(&projection)?;
        assert_eq!(encoded["status"], "requires_explicit_choice");
        assert_eq!(encoded["profileId"], serde_json::Value::Null);
        assert_eq!(encoded["variantId"], serde_json::Value::Null);
        assert_eq!(encoded["source"], serde_json::Value::Null);
        assert_eq!(encoded["previousProfileId"], "balanced");
        assert_eq!(encoded["blocked"], "profile_removed");
        Ok(())
    }

    #[test]
    fn a_resolved_selection_names_its_rule() -> Result<(), Box<dyn std::error::Error>> {
        let projection = project_model_selection(ModelSelectionInput {
            workspace_id: id("workspace_1")?,
            workspace_grant_epoch: 3,
            app_epoch: 5,
            catalog_epoch: 12,
            catalog_version: "v12.abc1234567890a".to_owned(),
            outcome: ModelSelectionOutcomeInput::Selected {
                profile_id: "balanced".to_owned(),
                variant_id: "standard".to_owned(),
                source: ModelSelectionSourceProjection::CatalogDefault,
            },
        })?;
        let encoded = serde_json::to_value(&projection)?;
        assert_eq!(encoded["status"], "selected");
        assert_eq!(encoded["source"], "catalog_default");
        assert_eq!(encoded["profileId"], "balanced");
        Ok(())
    }

    #[test]
    fn a_stream_publishes_in_order_and_closes_at_its_terminal(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut publisher = ModelStreamPublisher::new(subscription()?);
        let first = publisher.publish(&envelope(1, started())?)?;
        assert_eq!(first.sequence(), 1);
        assert_eq!(first.run_id().as_str(), "run_1");
        assert_eq!(first.task_id().as_str(), "task_1");

        publisher.publish(&envelope(
            2,
            ModelStreamEventInput::OutputProgress {
                output_index: 0,
                text_delta: "Hello".to_owned(),
                delta_byte_count: 64,
            },
        )?)?;

        let terminal = publisher.publish(&envelope(
            3,
            ModelStreamEventInput::Terminal {
                terminal: ModelStreamTerminalProjection::Completed,
                receipt: Some(ModelReceiptSummaryInput {
                    receipt_id: id("receipt_1")?,
                    event_count: 3,
                    output_byte_count: 64,
                    truncated: false,
                    recorded_at: UnixMillis(2_100),
                }),
                cancellation_source: None,
                reason_code: None,
                retryable: false,
            },
        )?)?;
        assert_eq!(
            serde_json::to_value(&terminal)?["value"]["terminal"],
            "completed"
        );
        assert!(publisher.is_terminated());

        // Nothing arrives after the terminal event, not even a valid sequence.
        assert_eq!(
            publisher.publish(&envelope(4, started())?),
            Err(ModelStreamProjectionError::AlreadyTerminal)
        );
        Ok(())
    }

    #[test]
    fn a_stream_refuses_a_repeated_or_regressed_sequence(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut publisher = ModelStreamPublisher::new(subscription()?);
        publisher.publish(&envelope(4, started())?)?;
        assert_eq!(
            publisher.publish(&envelope(4, started())?),
            Err(ModelStreamProjectionError::SequenceOutOfOrder)
        );
        assert_eq!(
            publisher.publish(&envelope(3, started())?),
            Err(ModelStreamProjectionError::SequenceOutOfOrder)
        );
        // The refusals did not consume the sequence: the stream continues.
        publisher.publish(&envelope(5, started())?)?;
        Ok(())
    }

    #[test]
    fn a_stream_refuses_every_wrong_identity_field(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut publisher = ModelStreamPublisher::new(subscription()?);

        let mut wrong_run = envelope(1, started())?;
        wrong_run.run_id = id("run_2")?;
        assert_eq!(
            publisher.publish(&wrong_run),
            Err(ModelStreamProjectionError::WrongRun)
        );

        let mut wrong_task = envelope(1, started())?;
        wrong_task.task_id = id("task_2")?;
        assert_eq!(
            publisher.publish(&wrong_task),
            Err(ModelStreamProjectionError::WrongTask)
        );

        let mut stale_epoch = envelope(1, started())?;
        stale_epoch.app_epoch = 4;
        assert_eq!(
            publisher.publish(&stale_epoch),
            Err(ModelStreamProjectionError::StaleAppEpoch)
        );

        let mut wrong_workspace = envelope(1, started())?;
        wrong_workspace.workspace_id = id("workspace_2")?;
        assert_eq!(
            publisher.publish(&wrong_workspace),
            Err(ModelStreamProjectionError::WrongWorkspace)
        );

        let mut stale_grant = envelope(1, started())?;
        stale_grant.workspace_grant_epoch = 2;
        assert_eq!(
            publisher.publish(&stale_grant),
            Err(ModelStreamProjectionError::WrongWorkspace)
        );

        // None of the refusals advanced the publisher.
        publisher.publish(&envelope(1, started())?)?;
        Ok(())
    }

    #[test]
    fn an_inconsistent_terminal_shape_is_refused() -> Result<(), Box<dyn std::error::Error>> {
        let mut publisher = ModelStreamPublisher::new(subscription()?);
        // Cancelled, but claiming a receipt.
        assert_eq!(
            publisher.publish(&envelope(
                1,
                ModelStreamEventInput::Terminal {
                    terminal: ModelStreamTerminalProjection::Cancelled,
                    receipt: Some(ModelReceiptSummaryInput {
                        receipt_id: id("receipt_1")?,
                        event_count: 1,
                        output_byte_count: 0,
                        truncated: false,
                        recorded_at: UnixMillis(2_100),
                    }),
                    cancellation_source: Some(ModelStreamCancellationSourceProjection::User),
                    reason_code: None,
                    retryable: false,
                },
            )?),
            Err(ModelStreamProjectionError::Unavailable)
        );
        Ok(())
    }

    /// A receipt summary is counts and a status. Nothing it can hold is the
    /// answer, the prompt, or a way to reach the provider.
    #[test]
    fn a_receipt_summary_carries_no_content() -> Result<(), Box<dyn std::error::Error>> {
        let mut publisher = ModelStreamPublisher::new(subscription()?);
        let terminal = publisher.publish(&envelope(
            1,
            ModelStreamEventInput::Terminal {
                terminal: ModelStreamTerminalProjection::Completed,
                receipt: Some(ModelReceiptSummaryInput {
                    receipt_id: id("receipt_1")?,
                    event_count: 9,
                    output_byte_count: 4_096,
                    truncated: true,
                    recorded_at: UnixMillis(2_100),
                }),
                cancellation_source: None,
                reason_code: None,
                retryable: false,
            },
        )?)?;
        let text = serde_json::to_string(&terminal)?.to_ascii_lowercase();
        for forbidden in ["prompt", "response", "token", "endpoint", "deployment"] {
            assert!(
                !text.contains(forbidden),
                "receipt summary disclosed {forbidden}"
            );
        }
        Ok(())
    }

    /// The bug this module fixes: a chat timeline cannot render an answer from
    /// a byte counter. The delta text must reach the renderer, and the byte
    /// count must survive alongside it for budget display.
    #[test]
    fn output_progress_forwards_text_alongside_its_byte_count(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut publisher = ModelStreamPublisher::new(subscription()?);
        publisher.publish(&envelope(1, started())?)?;
        let projected = publisher.publish(&envelope(
            2,
            ModelStreamEventInput::OutputProgress {
                output_index: 0,
                text_delta: "The answer is".to_owned(),
                delta_byte_count: 13,
            },
        )?)?;
        let encoded = serde_json::to_value(&projected)?;
        assert_eq!(encoded["value"]["textDelta"], "The answer is");
        assert_eq!(encoded["value"]["deltaByteCount"], 13);
        Ok(())
    }

    /// Tool call arguments have to surface as a reviewable typed proposal,
    /// which needs the arguments themselves, not merely their size.
    #[test]
    fn tool_call_arguments_forward_text_alongside_their_byte_count(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut publisher = ModelStreamPublisher::new(subscription()?);
        publisher.publish(&envelope(1, started())?)?;
        let projected = publisher.publish(&envelope(
            2,
            ModelStreamEventInput::ToolCallArguments {
                tool_call_id: "call_1".to_owned(),
                arguments_delta: "{\"path\":".to_owned(),
                delta_byte_count: 8,
            },
        )?)?;
        let encoded = serde_json::to_value(&projected)?;
        assert_eq!(encoded["value"]["toolCallId"], "call_1");
        assert_eq!(encoded["value"]["argumentsDelta"], "{\"path\":");
        assert_eq!(encoded["value"]["deltaByteCount"], 8);
        Ok(())
    }

    /// A text delta must not become an unbounded channel: the per-fragment cap
    /// still applies now that the fragment carries real content.
    #[test]
    fn an_oversized_text_delta_is_refused() -> Result<(), Box<dyn std::error::Error>> {
        let mut publisher = ModelStreamPublisher::new(subscription()?);
        publisher.publish(&envelope(1, started())?)?;
        let oversized = "a".repeat(super::MAX_TEXT_DELTA_BYTES + 1);
        assert_eq!(
            publisher.publish(&envelope(
                2,
                ModelStreamEventInput::OutputProgress {
                    output_index: 0,
                    text_delta: oversized,
                    delta_byte_count: 1,
                },
            )?),
            Err(ModelStreamProjectionError::Unavailable)
        );
        Ok(())
    }

    /// Same bound, for the tool-call-arguments fragment.
    #[test]
    fn an_oversized_arguments_delta_is_refused() -> Result<(), Box<dyn std::error::Error>> {
        let mut publisher = ModelStreamPublisher::new(subscription()?);
        publisher.publish(&envelope(1, started())?)?;
        let oversized = "a".repeat(super::MAX_ARGUMENTS_DELTA_BYTES + 1);
        assert_eq!(
            publisher.publish(&envelope(
                2,
                ModelStreamEventInput::ToolCallArguments {
                    tool_call_id: "call_1".to_owned(),
                    arguments_delta: oversized,
                    delta_byte_count: 1,
                },
            )?),
            Err(ModelStreamProjectionError::Unavailable)
        );
        Ok(())
    }

    /// A bidi-override character in a text delta is refused the same way an
    /// unsafe catalog display name is: this is a rendering-safety scan, not a
    /// content-disclosure scan, so it applies to free-form model text too.
    #[test]
    fn a_text_delta_carrying_a_bidi_override_is_refused() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut publisher = ModelStreamPublisher::new(subscription()?);
        publisher.publish(&envelope(1, started())?)?;
        assert_eq!(
            publisher.publish(&envelope(
                2,
                ModelStreamEventInput::OutputProgress {
                    output_index: 0,
                    text_delta: "safe\u{202e}txet".to_owned(),
                    delta_byte_count: 5,
                },
            )?),
            Err(ModelStreamProjectionError::UnsafeDisplayText)
        );
        Ok(())
    }

    /// Free-form model output legitimately containing a URL or the word
    /// "bearer" must still forward: the disclosure-marker scan is for catalog
    /// display text sourced from operator-authored profile metadata, not for
    /// answer content this projection has no way to have received a
    /// connection detail into.
    #[test]
    fn a_text_delta_carrying_a_url_still_forwards() -> Result<(), Box<dyn std::error::Error>> {
        let mut publisher = ModelStreamPublisher::new(subscription()?);
        publisher.publish(&envelope(1, started())?)?;
        let projected = publisher.publish(&envelope(
            2,
            ModelStreamEventInput::OutputProgress {
                output_index: 0,
                text_delta: "See https://example.com for docs.".to_owned(),
                delta_byte_count: 34,
            },
        )?)?;
        let encoded = serde_json::to_value(&projected)?;
        assert_eq!(encoded["value"]["textDelta"], "See https://example.com for docs.");
        Ok(())
    }
}
