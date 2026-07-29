//! Renderer projections for the Foundry model catalog, selection, and stream.
//!
//! Three rules shape everything in this module.
//!
//! * **Connection authority is not representable.** A projection here carries a
//!   profile identifier, a variant identifier and display text. It has no field
//!   for a deployment, an endpoint, a credential, a capability set, a limit, or
//!   an Azure resource identifier, so no host mistake can leak one through this
//!   lane. Display text is additionally scanned, because a support plane that
//!   put an endpoint inside a profile name would otherwise route around the
//!   type.
//! * **Content is not a stream event.** The stream projection reports
//!   *progress* — which run advanced, by how many bytes, which tool opened, how
//!   it terminated. The answer text itself never travels here; it reaches the
//!   renderer through the reviewed completion projection after the host has
//!   verified the receipt. So "the model is answering" needs no authority over
//!   what the model said.
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
const MAX_SUMMARY_BYTES: usize = 512;
const MAX_REASON_BYTES: usize = 256;
const MAX_TOOL_NAME_BYTES: usize = 128;

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

/// One selectable profile, as the host holds it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelProfileInput {
    /// The stable profile identifier.
    pub profile_id: String,
    /// The operator-authored display name.
    pub display_name: String,
    /// Optional one-line description.
    pub summary: Option<String>,
    /// The selectable variants of this profile, in catalog order.
    pub variant_ids: Vec<String>,
}

/// One selectable profile, as the renderer sees it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProfileProjection {
    profile_id: String,
    display_name: String,
    summary: Option<String>,
    variant_ids: Vec<String>,
}

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
    /// The answer grew. The bytes themselves stay host-side.
    OutputProgress {
        /// Which output slot advanced.
        output_index: u8,
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
        #[serde(rename = "deltaByteCount")]
        delta_byte_count: u64,
    },
    ToolCallStarted {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "toolName")]
        tool_name: String,
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
            delta_byte_count,
        } => {
            if *delta_byte_count == 0 || *delta_byte_count > MAX_SAFE_JSON_INTEGER {
                return Err(ModelStreamProjectionError::Unavailable);
            }
            Ok(ModelStreamEventBodyProjection::OutputProgress {
                output_index: *output_index,
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
        check_catalog_display_text(&profile.display_name, MAX_DISPLAY_NAME_BYTES)?;
        if let Some(summary) = profile.summary.as_deref() {
            check_catalog_display_text(summary, MAX_SUMMARY_BYTES)?;
        }
        if profile.variant_ids.len() < MIN_VARIANTS_PER_PROFILE
            || profile.variant_ids.len() > MAX_VARIANTS_PER_PROFILE
        {
            return Err(ModelCatalogProjectionError::Unavailable);
        }
        let mut seen_variants: HashSet<&str> = HashSet::new();
        for variant_id in &profile.variant_ids {
            check_catalog_identifier(variant_id)?;
            if !seen_variants.insert(variant_id.as_str()) {
                return Err(ModelCatalogProjectionError::Unavailable);
            }
        }
        profiles.push(ModelProfileProjection {
            profile_id: profile.profile_id.clone(),
            display_name: profile.display_name.clone(),
            summary: profile.summary.clone(),
            variant_ids: profile.variant_ids.clone(),
        });
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
        project_model_catalog, project_model_selection, ModelCatalogInput,
        ModelCatalogProjectionError, ModelCatalogStateProjection, ModelProfileInput,
        ModelReceiptSummaryInput, ModelSelectionBlockedProjection, ModelSelectionInput,
        ModelBlockedAvailabilityProjection, ModelRecoveryActionProjection,
        ModelSelectionOutcomeInput, ModelSelectionSourceProjection,
        ModelStreamCancellationSourceProjection,
        ModelStreamEventEnvelope, ModelStreamEventInput, ModelStreamProjectionError,
        ModelStreamPublisher, ModelStreamSubscription, ModelStreamTerminalProjection,
        ModelUnavailableProfileInput,
    };

    fn id(value: &str) -> Result<ContractId, Box<dyn std::error::Error>> {
        Ok(ContractId::new(value)?)
    }

    fn profile(profile_id: &str) -> ModelProfileInput {
        ModelProfileInput {
            profile_id: profile_id.to_owned(),
            display_name: "Balanced reasoning".to_owned(),
            summary: Some("General drafting and review.".to_owned()),
            variant_ids: vec!["standard".to_owned()],
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
        assert_eq!(encoded["profiles"][0]["variantIds"][0], "standard");
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

        // The connection vocabulary has no field to arrive in. `capabilities`
        // and `limits` exist on the canonical profile and are dropped here:
        // neither is something the renderer is allowed to know or to set.
        let text = serde_json::to_string(&encoded)?.to_ascii_lowercase();
        for forbidden in [
            "endpoint",
            "apikey",
            "credential",
            "capabilit",
            "limits",
            "azure",
            "deploymentname",
        ] {
            assert!(
                !text.contains(forbidden),
                "catalog projection disclosed {forbidden}"
            );
        }
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
}
