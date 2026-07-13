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
    string ConsentReceiptHash,
    ModelContextItem[] Items,
    string RetentionMode,
    string BudgetClass);

public sealed record ModelAccessResult(
    string SchemaVersion,
    string RequestId,
    string OutputSchemaId,
    string PayloadJson,
    string PayloadHash,
    ModelAccessReceipt Receipt);

public sealed record ModelAccessReceipt(
    string SchemaVersion,
    string ReceiptId,
    string RequestHash,
    string ResultHash,
    string LocalEgressManifestHash,
    string ConsentReceiptHash,
    string ModelProfileHash,
    string RetentionMode,
    string Region,
    int InputBytes,
    int OutputBytes,
    DateTimeOffset StartedAt,
    DateTimeOffset CompletedAt,
    string Status);

public sealed record ReleaseResponse(
    string SchemaVersion,
    string Channel,
    string Version,
    string Architecture,
    string ArtifactSha256,
    string MetadataSignature);

