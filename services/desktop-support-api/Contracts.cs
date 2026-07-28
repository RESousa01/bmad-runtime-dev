using System.Text.Json.Serialization;

namespace Sapphirus.DesktopSupportApi;

public sealed record BootstrapResponse(
    string SchemaVersion,
    string Region,
    string ContractEpoch,
    string MinimumClientContractEpoch,
    string[] Capabilities,
    DateTimeOffset ServerTime);

public sealed record DeviceRegistrationRequest(
    string SchemaVersion,
    string InstallationPublicKey,
    string InstallationPublicKeyHash,
    string ClientRelease,
    string Platform,
    string Architecture,
    long TenantPolicyVersion);

public sealed record DeviceRegistrationResponse(
    string SchemaVersion,
    string RegistrationId,
    string Status,
    DateTimeOffset CreatedAt);

public sealed record EntitlementSummaryResponse(
    string DeliveryModel,
    string[] Features,
    bool RemoteJobsEnabled,
    string[] SyncCategories,
    string ReleaseChannel);

public sealed record LeaseRequest(string RegistrationId);

public sealed record SignedEntitlementLease(
    string SchemaVersion,
    string LeaseId,
    string RegistrationId,
    string SubjectHash,
    string DeliveryModel,
    DateTimeOffset IssuedAt,
    DateTimeOffset NotBefore,
    DateTimeOffset ExpiresAt,
    DateTimeOffset OfflineGraceEndsAt,
    string[] Features,
    string TenantPolicyHash,
    string MinimumClientVersion,
    string KeyId,
    string Signature);

public sealed record SignedDesktopPolicy(
    string SchemaVersion,
    string PolicyId,
    long PolicyVersion,
    string PolicyHash,
    bool SystemBrowserFallbackAllowed,
    int MaximumContextBytes,
    int MaximumContextItems,
    string[] AllowedRegions,
    string KeyId,
    string Signature);

public sealed record ModelContextItem(
    string ClientItemId,
    string RelativeLabel,
    string SemanticRole,
    string Language,
    string ContentHash,
    int ByteCount,
    string Classification,
    string Content);

public sealed record ModelContextConsentProof(
    string ProofType,
    string Algorithm,
    string KeyId,
    string SignedPayloadHash,
    string Signature);

public sealed record ModelContextConsent(
    string SchemaVersion,
    string DecisionId,
    string RequestId,
    string InvocationId,
    string DeliveryModel,
    string TenantHash,
    string SubjectHash,
    string RegistrationId,
    string InstallationPublicKeyHash,
    string EntitlementLeaseId,
    string EntitlementLeaseHash,
    string TenantPolicyId,
    long TenantPolicyVersion,
    string TenantPolicyHash,
    string Purpose,
    string ModelRole,
    string CanonicalOutputSchemaId,
    string CanonicalOutputSchemaHash,
    string ManifestHash,
    string InvocationBindingHash,
    string ConsumptionHash,
    string ConsentDisclosureHash,
    string ProviderProfileHash,
    string ModelProfileHash,
    string ModelCapabilityHash,
    string DeploymentHash,
    string Region,
    string RetentionMode,
    string BudgetClass,
    [property: JsonConverter(typeof(UtcInstantJsonConverter))] DateTimeOffset IssuedAt,
    [property: JsonConverter(typeof(UtcInstantJsonConverter))] DateTimeOffset NotBefore,
    [property: JsonConverter(typeof(UtcInstantJsonConverter))] DateTimeOffset ExpiresAt,
    string NonceHash,
    string ConsentEnvelopeHash,
    ModelContextConsentProof Proof);

public sealed record ModelAccessRequest(
    string SchemaVersion,
    string RequestId,
    string DeliveryModel,
    string RegistrationId,
    string Purpose,
    string ModelRole,
    string CanonicalOutputSchemaId,
    string CanonicalOutputSchemaHash,
    string LocalEgressManifestHash,
    ModelContextConsent Consent,
    ModelContextItem[] Items,
    string RetentionMode,
    string BudgetClass)
{
    [JsonIgnore]
    public string ConsentReceiptHash => Consent.ConsentEnvelopeHash;
}

public sealed record ModelAccessResult(
    string SchemaVersion,
    string RequestId,
    string OutputSchemaId,
    string PayloadJson,
    string PayloadHash,
    ModelAccessReceipt Receipt);

public sealed record ModelAccessUsage(
    long InputTokens,
    long OutputTokens,
    long CostMicrounits,
    string Currency);

public sealed record ModelFallbackEvent(
    int Sequence,
    string FromProfileHash,
    string ToProfileHash,
    string PolicyTransitionHash);

public sealed record ModelAccessReceiptProof(
    string ProofType,
    string Algorithm,
    string Issuer,
    string Audience,
    string KeyId,
    string SignedPayloadHash,
    string Signature);

public sealed record ModelAccessReceipt(
    string SchemaVersion,
    string ReceiptId,
    string RequestId,
    string RequestHash,
    string ResultHash,
    string DeliveryModel,
    string TenantHash,
    string SubjectHash,
    string RegistrationId,
    string ManifestHash,
    string InvocationBindingHash,
    string ConsumptionHash,
    string ConsentEnvelopeHash,
    string ConsentDisclosureHash,
    string ProviderProfileHash,
    string ModelProfileHash,
    string ModelCapabilityHash,
    string DeploymentHash,
    string CanonicalOutputSchemaId,
    string CanonicalOutputSchemaHash,
    string ProviderSchemaProjectionHash,
    string CredentialBindingHash,
    string RetentionMode,
    string Region,
    int InputBytes,
    int OutputBytes,
    ModelAccessUsage Usage,
    int RetryCount,
    ModelFallbackEvent[] FallbackEvents,
    string? ProviderRequestId,
    [property: JsonConverter(typeof(UtcInstantJsonConverter))] DateTimeOffset StartedAt,
    [property: JsonConverter(typeof(UtcInstantJsonConverter))] DateTimeOffset CompletedAt,
    string TerminalStatus,
    string ReceiptHash,
    ModelAccessReceiptProof Proof);

public sealed record ReleaseResponse(
    string SchemaVersion,
    string Channel,
    string Version,
    string Architecture,
    string ArtifactSha256,
    string MetadataSignature);

// ---------------------------------------------------------------------------
// Version two model access: usage, response lane, receipt, and stream events.
//
// These are hand-written wire records exactly as the version one records above
// are. The generated Corvus bindings for the same schemas live under
// packages/contracts/generated/dotnet and are compiled only into the
// contract-conformance project, so this service mirrors them by hand and the
// cross-language fixtures are what keep the two honest.
//
// Every record below is closed: there is no property a project endpoint, a
// deployment name, a bearer token, a raw provider body, or private reasoning
// text could arrive in.
// ---------------------------------------------------------------------------

/// <summary>
/// How the usage numbers on a version two receipt were obtained.
/// <c>unavailable</c> is not "free": every category is null by construction.
/// </summary>
public static class ModelUsageProvenance
{
    public const string Reported = "reported";
    public const string Estimated = "estimated";
    public const string Unavailable = "unavailable";

    public static bool IsKnown(string? value) =>
        value is Reported or Estimated or Unavailable;
}

/// <summary>
/// Usage as the provider reported it, mirroring
/// <c>model-access-receipt-v2.schema.json#/$defs/ModelUsageV2</c>.
///
/// A category the provider never reported stays null. Zero means the provider
/// said zero. The two are never conflated, because a receipt that reads a
/// silent provider as zero tokens is a receipt that under-bills.
/// </summary>
public sealed record ModelUsageV2(
    string UsageProvenance,
    long? InputTokens,
    long? OutputTokens,
    long? ReasoningTokens,
    long? CacheReadTokens,
    long? CacheWriteTokens,
    long? CostMicrounits,
    string Currency,
    string PriceProfileVersion)
{
    /// <summary>The all-null record. Nothing was obtainable.</summary>
    public static ModelUsageV2 Unavailable(string currency, string priceProfileVersion) =>
        new(
            ModelUsageProvenance.Unavailable,
            null,
            null,
            null,
            null,
            null,
            null,
            currency,
            priceProfileVersion);

    /// <summary>True when every token and cost category is null.</summary>
    public bool IsFullyUnavailable =>
        InputTokens is null
        && OutputTokens is null
        && ReasoningTokens is null
        && CacheReadTokens is null
        && CacheWriteTokens is null
        && CostMicrounits is null;
}

/// <summary>
/// The closed response lane. It carries the request mode and the support route
/// together, so a receipt can never claim a streaming chat answer arrived over
/// the structured response route.
/// </summary>
public interface IModelResponseLane
{
    string RequestMode { get; }
    string SupportRoute { get; }
}

public static class ModelRequestModes
{
    public const string StructuredResponse = "structured_response";
    public const string StreamingChat = "streaming_chat";
}

public static class ModelSupportRoutes
{
    public const string ModelAccess = "model_access";
    public const string ModelStream = "model_stream";
}

/// <summary>A structured response served over the model access route.</summary>
public sealed record StructuredResponseLane(
    string CanonicalOutputSchemaId,
    string CanonicalOutputSchemaHash,
    string ProviderSchemaProjectionHash) : IModelResponseLane
{
    [JsonPropertyOrder(-2)]
    public string RequestMode => ModelRequestModes.StructuredResponse;

    [JsonPropertyOrder(-1)]
    public string SupportRoute => ModelSupportRoutes.ModelAccess;
}

/// <summary>
/// A streaming chat answer served over the model stream route. No canonical
/// output schema is representable here; the observed event count and final
/// sequence are, so the receipt reconciles against what the desktop saw.
/// </summary>
public sealed record StreamingChatLane(
    int EventCount,
    int FinalSequence) : IModelResponseLane
{
    [JsonPropertyOrder(-2)]
    public string RequestMode => ModelRequestModes.StreamingChat;

    [JsonPropertyOrder(-1)]
    public string SupportRoute => ModelSupportRoutes.ModelStream;
}

/// <summary>
/// The model identity the support plane confirmed behind the resolved
/// deployment before the call was made. It is a model identity, never a
/// deployment name.
/// </summary>
public sealed record ModelResolvedIdentity(
    string Publisher,
    string Name,
    string Version);

public sealed record ModelFallbackEventV2(
    int Sequence,
    string FromProfileId,
    string ToProfileId,
    string FromProfileHash,
    string ToProfileHash,
    string PolicyTransitionHash);

public static class ModelTerminalStatuses
{
    public const string Succeeded = "succeeded";
    public const string Refused = "refused";
    public const string Incomplete = "incomplete";
    public const string Failed = "failed";
    public const string Cancelled = "cancelled";

    public static bool IsKnown(string? value) =>
        value is Succeeded or Refused or Incomplete or Failed or Cancelled;
}

/// <summary>
/// The version two signed model access receipt, mirroring
/// <c>model-access-receipt-v2.schema.json</c> field for field.
///
/// It exists beside <see cref="ModelAccessReceipt"/> rather than replacing it:
/// historical version one receipts must stay verifiable under their own digest
/// and their own guard.
/// </summary>
public sealed record ModelAccessReceiptV2(
    string SchemaVersion,
    string ReceiptId,
    string RequestId,
    string RunId,
    string RequestHash,
    string ResultHash,
    string DeliveryModel,
    string TenantHash,
    string SubjectHash,
    string RegistrationId,
    string ManifestHash,
    string InvocationBindingHash,
    string ConsumptionHash,
    string ConsentEnvelopeHash,
    string ConsentDisclosureHash,
    string ProviderProfileHash,
    string ModelProfileHash,
    string ModelCapabilityHash,
    IModelResponseLane ResponseLane,
    string RequestedProfileId,
    string ResolvedProfileId,
    string ResolvedProfileHash,
    string? RequestedVariantId,
    string ResolvedVariantId,
    string CatalogVersion,
    string CatalogHash,
    string DeploymentInventoryHash,
    string ResolvedDeploymentHash,
    ModelResolvedIdentity ResolvedModel,
    string? ReportedResponseModel,
    string CredentialBindingHash,
    string RetentionMode,
    string Region,
    long InputBytes,
    long OutputBytes,
    ModelUsageV2 Usage,
    int RetryCount,
    ModelFallbackEventV2[] FallbackEvents,
    string? ProviderRequestId,
    [property: JsonConverter(typeof(UtcInstantJsonConverter))] DateTimeOffset StartedAt,
    [property: JsonConverter(typeof(UtcInstantJsonConverter))] DateTimeOffset CompletedAt,
    string TerminalStatus,
    string ReceiptHash,
    ModelAccessReceiptProof Proof)
{
    public const string SchemaVersionValue = "sapphirus.model-access-receipt.v2";
}

// --- Normalized model stream events ----------------------------------------

/// <summary>Stable stream failure vocabulary. The desktop branches on these.</summary>
public static class ModelStreamFailureCodes
{
    public const string ProviderUnavailable = "provider_unavailable";
    public const string ProviderTimeout = "provider_timeout";
    public const string RateLimited = "rate_limited";
    public const string ProfileUnavailable = "profile_unavailable";
    public const string CatalogStale = "catalog_stale";
    public const string ConsentExpired = "consent_expired";
    public const string BudgetExhausted = "budget_exhausted";
    public const string OutputLimitExceeded = "output_limit_exceeded";
    public const string OutputSchemaViolation = "output_schema_violation";
    public const string InternalError = "internal_error";

    public static bool IsKnown(string? value) => value is ProviderUnavailable
        or ProviderTimeout or RateLimited or ProfileUnavailable or CatalogStale
        or ConsentExpired or BudgetExhausted or OutputLimitExceeded
        or OutputSchemaViolation or InternalError;
}

public static class ModelStreamRefusalCodes
{
    public const string PolicyBlocked = "policy_blocked";
    public const string ContentFiltered = "content_filtered";
    public const string NotEntitled = "not_entitled";
    public const string ToolNotPermitted = "tool_not_permitted";
    public const string ContextRejected = "context_rejected";

    public static bool IsKnown(string? value) => value is PolicyBlocked
        or ContentFiltered or NotEntitled or ToolNotPermitted or ContextRejected;
}

public static class ModelStreamCancellationSources
{
    public const string User = "user";
    public const string Host = "host";
    public const string SupportPlane = "support_plane";
    public const string DeadlineExceeded = "deadline_exceeded";

    public static bool IsKnown(string? value) =>
        value is User or Host or SupportPlane or DeadlineExceeded;
}

/// <summary>
/// One normalized stream event. The ten variants below are the whole set; the
/// desktop never parses a provider stream itself.
/// </summary>
public interface IModelStreamEvent
{
    string Type { get; }
    string RequestId { get; }
    string RunId { get; }
    int Sequence { get; }

    /// <summary>True for the four variants that end a run.</summary>
    bool IsTerminal { get; }
}

public sealed record ModelStreamResponseStartedEvent(
    string RequestId,
    string RunId,
    int Sequence,
    string ResolvedProfileId,
    string ResolvedVariantId,
    [property: JsonConverter(typeof(UtcInstantJsonConverter))] DateTimeOffset StartedAt)
    : IModelStreamEvent
{
    public const string TypeName = "response_started";

    [JsonPropertyOrder(-1)]
    public string Type => TypeName;

    [JsonIgnore]
    public bool IsTerminal => false;
}

public sealed record ModelStreamOutputTextDeltaEvent(
    string RequestId,
    string RunId,
    int Sequence,
    int OutputIndex,
    string TextDelta) : IModelStreamEvent
{
    public const string TypeName = "output_text_delta";

    [JsonPropertyOrder(-1)]
    public string Type => TypeName;

    [JsonIgnore]
    public bool IsTerminal => false;
}

public sealed record ModelStreamToolCallStartedEvent(
    string RequestId,
    string RunId,
    int Sequence,
    string ToolCallId,
    string ToolName) : IModelStreamEvent
{
    public const string TypeName = "tool_call_started";

    [JsonPropertyOrder(-1)]
    public string Type => TypeName;

    [JsonIgnore]
    public bool IsTerminal => false;
}

public sealed record ModelStreamToolCallArgumentsDeltaEvent(
    string RequestId,
    string RunId,
    int Sequence,
    string ToolCallId,
    string ArgumentsDelta) : IModelStreamEvent
{
    public const string TypeName = "tool_call_arguments_delta";

    [JsonPropertyOrder(-1)]
    public string Type => TypeName;

    [JsonIgnore]
    public bool IsTerminal => false;
}

public sealed record ModelStreamToolCallCompletedEvent(
    string RequestId,
    string RunId,
    int Sequence,
    string ToolCallId,
    string ArgumentsHash,
    int ArgumentsByteCount) : IModelStreamEvent
{
    public const string TypeName = "tool_call_completed";

    [JsonPropertyOrder(-1)]
    public string Type => TypeName;

    [JsonIgnore]
    public bool IsTerminal => false;
}

public sealed record ModelStreamUsageEvent(
    string RequestId,
    string RunId,
    int Sequence,
    ModelUsageV2 Usage) : IModelStreamEvent
{
    public const string TypeName = "usage";

    [JsonPropertyOrder(-1)]
    public string Type => TypeName;

    [JsonIgnore]
    public bool IsTerminal => false;
}

/// <summary>
/// The parts of the version two receipt that are safe to publish on the event
/// stream. The signed receipt itself is fetched over the reviewed receipt
/// route; this summary carries no signature.
/// </summary>
public sealed record ModelStreamReceiptSummary(
    string ReceiptId,
    string ReceiptHash,
    string TerminalStatus,
    string ResolvedProfileId,
    string ResolvedVariantId,
    long OutputBytes,
    [property: JsonConverter(typeof(UtcInstantJsonConverter))] DateTimeOffset CompletedAt);

public sealed record ModelStreamCompletedEvent(
    string RequestId,
    string RunId,
    int Sequence,
    ModelStreamReceiptSummary ReceiptSummary) : IModelStreamEvent
{
    public const string TypeName = "completed";

    [JsonPropertyOrder(-1)]
    public string Type => TypeName;

    [JsonIgnore]
    public bool IsTerminal => true;
}

public sealed record ModelStreamRefusedEvent(
    string RequestId,
    string RunId,
    int Sequence,
    string RefusalCode,
    string? RefusalDetail) : IModelStreamEvent
{
    public const string TypeName = "refused";

    [JsonPropertyOrder(-1)]
    public string Type => TypeName;

    [JsonIgnore]
    public bool IsTerminal => true;
}

public sealed record ModelStreamFailedEvent(
    string RequestId,
    string RunId,
    int Sequence,
    string FailureCode,
    bool Retryable,
    string? FailureDetail) : IModelStreamEvent
{
    public const string TypeName = "failed";

    [JsonPropertyOrder(-1)]
    public string Type => TypeName;

    [JsonIgnore]
    public bool IsTerminal => true;
}

public sealed record ModelStreamCancelledEvent(
    string RequestId,
    string RunId,
    int Sequence,
    string CancellationSource) : IModelStreamEvent
{
    public const string TypeName = "cancelled";

    [JsonPropertyOrder(-1)]
    public string Type => TypeName;

    [JsonIgnore]
    public bool IsTerminal => true;
}

// --- Version two streaming request ------------------------------------------

/// <summary>
/// One tool the model may call. It names a canonical parameter schema rather
/// than carrying an inline provider parameter document, so a request cannot
/// smuggle a provider-shaped payload through a tool definition.
/// </summary>
public sealed record ModelToolDefinition(
    string ToolName,
    string Description,
    string ParametersSchemaId,
    string ParametersSchemaHash);

/// <summary>
/// The version two signed context decision, mirroring
/// <c>model-access-request-v2.schema.json#/$defs/ModelContextConsentV2</c>.
///
/// It drops the deployment hash the version one decision carried — the desktop
/// no longer knows or chooses a deployment — and adds the profile, catalog
/// epoch, requested variant, request mode, and context snapshot.
/// </summary>
public sealed record ModelContextConsentV2(
    string SchemaVersion,
    string DecisionId,
    string RequestId,
    string InvocationId,
    string DeliveryModel,
    string TenantHash,
    string SubjectHash,
    string RegistrationId,
    string InstallationPublicKeyHash,
    string EntitlementLeaseId,
    string EntitlementLeaseHash,
    string TenantPolicyId,
    long TenantPolicyVersion,
    string TenantPolicyHash,
    string Purpose,
    string ModelRole,
    string RequestMode,
    string ModelProfileId,
    string? RequestedVariantId,
    string CatalogVersion,
    string ContextSnapshotId,
    string? CanonicalOutputSchemaId,
    string? CanonicalOutputSchemaHash,
    string ManifestHash,
    string InvocationBindingHash,
    string ConsumptionHash,
    string ConsentDisclosureHash,
    string ProviderProfileHash,
    string ModelProfileHash,
    string ModelCapabilityHash,
    string Region,
    string RetentionMode,
    string BudgetClass,
    [property: JsonConverter(typeof(UtcInstantJsonConverter))] DateTimeOffset IssuedAt,
    [property: JsonConverter(typeof(UtcInstantJsonConverter))] DateTimeOffset NotBefore,
    [property: JsonConverter(typeof(UtcInstantJsonConverter))] DateTimeOffset ExpiresAt,
    string NonceHash,
    string ConsentEnvelopeHash,
    ModelContextConsentProof Proof)
{
    public const string SchemaVersionValue = "sapphirus.model-context-consent.v2";
}

/// <summary>
/// The version two model access request, mirroring
/// <c>model-access-request-v2.schema.json</c>.
///
/// Foundry authority is unrepresentable: there is no property that can carry a
/// project endpoint, a deployment name, an API version, a provider SDK option,
/// a credential, or a bearer token. The desktop names a profile and a catalog
/// epoch; the support plane decides what that means.
/// </summary>
public sealed record ModelAccessRequestV2(
    string SchemaVersion,
    string RequestId,
    string DeliveryModel,
    string RegistrationId,
    string Purpose,
    string ModelRole,
    string RequestMode,
    string ModelProfileId,
    string? RequestedVariantId,
    string CatalogVersion,
    string ModelProfileHash,
    string ModelCapabilityHash,
    string ContextSnapshotId,
    string? CanonicalOutputSchemaId,
    string? CanonicalOutputSchemaHash,
    string LocalEgressManifestHash,
    ModelContextConsentV2 Consent,
    ModelContextItem[] Items,
    ModelToolDefinition[] Tools,
    string RetentionMode,
    string BudgetClass)
{
    public const string SchemaVersionValue = "desktop-model-access-request.v2";

    [JsonIgnore]
    public long TotalContextBytes => Items.Sum(item => (long)item.ByteCount);
}
