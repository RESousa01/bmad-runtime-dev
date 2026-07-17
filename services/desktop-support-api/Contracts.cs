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
    string InstallationPublicKeyHash,
    string ClientRelease,
    string Platform,
    string Architecture,
    string TenantPolicyVersion);

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
    string PolicyVersion,
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
