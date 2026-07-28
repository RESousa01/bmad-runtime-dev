using System.Text.Json.Serialization;

namespace Sapphirus.DesktopSupportApi.Model;

/// <summary>
/// Availability of a profile the user may invoke. Blocked states are
/// deliberately not representable here, so a selectable projection cannot carry
/// a reason it should not have been listed.
/// </summary>
public enum SelectableAvailability
{
    Available,
    Degraded,
}

/// <summary>
/// Why a visible profile is not currently invocable. Together with
/// <see cref="SelectableAvailability"/> this is the complete vocabulary.
/// </summary>
public enum BlockedAvailability
{
    PolicyBlocked,
    NotEntitled,
    Unqualified,
    DeploymentMissing,
    CatalogStale,
}

public enum ModelRecoveryAction
{
    RefreshCatalog,
    RequestEntitlement,
    AwaitingQualification,
    ContactAdministrator,
    SelectAnotherProfile,
}

/// <summary>
/// The exact wire spellings the canonical catalog schema pins. They are
/// rendered as strings rather than serialized enums so a future rename in this
/// service can never silently change the wire.
/// </summary>
public static class ModelCatalogWireNames
{
    public const string SchemaVersion = "sapphirus.model-catalog.v1";
    public const string Provider = "microsoft_foundry";
    public const string CatalogStateFresh = "fresh";
    public const string CatalogStateStaleUsable = "stale_usable";

    public static string ToWireName(SelectableAvailability availability) => availability switch
    {
        SelectableAvailability.Degraded => "degraded",
        _ => "available",
    };

    public static string ToWireName(BlockedAvailability availability) => availability switch
    {
        BlockedAvailability.PolicyBlocked => "policy_blocked",
        BlockedAvailability.NotEntitled => "not_entitled",
        BlockedAvailability.Unqualified => "unqualified",
        BlockedAvailability.DeploymentMissing => "deployment_missing",
        _ => "catalog_stale",
    };

    public static string ToWireName(ModelRecoveryAction action) => action switch
    {
        ModelRecoveryAction.RefreshCatalog => "refresh_catalog",
        ModelRecoveryAction.RequestEntitlement => "request_entitlement",
        ModelRecoveryAction.AwaitingQualification => "awaiting_qualification",
        ModelRecoveryAction.ContactAdministrator => "contact_administrator",
        _ => "select_another_profile",
    };

    /// <summary>
    /// The recovery action a blocked state implies. The desktop shows this
    /// instead of silently switching the user to a different model.
    /// </summary>
    public static ModelRecoveryAction RecoveryFor(BlockedAvailability availability) =>
        availability switch
        {
            BlockedAvailability.PolicyBlocked => ModelRecoveryAction.ContactAdministrator,
            BlockedAvailability.NotEntitled => ModelRecoveryAction.RequestEntitlement,
            BlockedAvailability.Unqualified => ModelRecoveryAction.AwaitingQualification,
            BlockedAvailability.DeploymentMissing => ModelRecoveryAction.SelectAnotherProfile,
            _ => ModelRecoveryAction.RefreshCatalog,
        };
}

/// <summary>
/// Capabilities Forge will actually honour. <c>streaming</c> is deliberately
/// absent: the desktop learns streaming availability from the stream surface,
/// not from the catalog.
/// </summary>
public sealed record ProjectedCapabilities(
    bool TextInput,
    bool ImageInput,
    bool PdfInput,
    bool Attachments,
    bool ToolCalling,
    bool StructuredOutput,
    bool Reasoning);

/// <summary>
/// Limits Forge will actually enforce. Null means the organization did not
/// publish a qualified value; the desktop must render that as unknown rather
/// than as zero or unlimited.
/// </summary>
public sealed record ProjectedLimits(
    int? ContextTokens,
    int? MaxInputTokens,
    int? MaxOutputTokens,
    int? MaxAttachmentBytes);

public sealed record ModelVariantProjection(
    string VariantId,
    string DisplayName,
    string ReasoningEffort,
    int MaxOutputTokens);

/// <summary>
/// One selectable profile as the desktop sees it. There is no member that can
/// carry a deployment name, a project endpoint, a subscription or resource
/// identifier, a role assignment, a managed-identity client ID, an API version,
/// or a credential.
/// </summary>
public sealed record ModelProfileProjection(
    string ProfileId,
    string DisplayName,
    string Description,
    string? Family,
    string Provider,
    string Availability,
    string? StatusDetail,
    ProjectedCapabilities Capabilities,
    ProjectedLimits Limits,
    IReadOnlyList<ModelVariantProjection> Variants,
    string DefaultVariantId,
    [property: JsonConverter(typeof(UtcInstantJsonConverter))] DateTimeOffset? QualifiedAt);

/// <summary>
/// A visible profile that exists for this organization but cannot be invoked
/// right now. Carrying it is what lets the desktop explain why a previously
/// selected profile disappeared instead of silently switching models. It states
/// the prerequisite and a recovery action, and nothing else — in particular no
/// group name, deployment name, endpoint, or role assignment.
/// </summary>
public sealed record UnavailableModelProfile(
    string ProfileId,
    string DisplayName,
    string Availability,
    string RecoveryAction);

/// <summary>
/// The sanitized effective model catalog, matching
/// <c>packages/contracts/schemas/model-catalog.schema.json</c>
/// (<c>sapphirus.model-catalog.v1</c>) field for field.
/// </summary>
public sealed record ModelCatalogProjection(
    string SchemaVersion,
    string CatalogVersion,
    string CatalogHash,
    string CatalogState,
    long PolicyVersion,
    string PolicyHash,
    string DeploymentSnapshotHash,
    [property: JsonConverter(typeof(UtcInstantJsonConverter))] DateTimeOffset GeneratedAt,
    [property: JsonConverter(typeof(UtcInstantJsonConverter))] DateTimeOffset ExpiresAt,
    string? DefaultProfileId,
    IReadOnlyList<ModelProfileProjection> Profiles,
    IReadOnlyList<UnavailableModelProfile> UnavailableProfiles);
