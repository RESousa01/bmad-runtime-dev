namespace Sapphirus.DesktopSupportApi.Model;

/// <summary>
/// Provenance of a profile registry snapshot.
///
/// Only <see cref="Signed"/> may ever be reached outside Development. The
/// fixture value exists so a developer can run the whole catalog architecture
/// without an App Configuration store, and it is labelled rather than hidden so
/// every consumer can refuse it structurally.
/// </summary>
public enum ModelProfileTrust
{
    /// <summary>Read from the signed organization configuration store.</summary>
    Signed,

    /// <summary>A Development-only fixture. Never selectable elsewhere.</summary>
    DevelopmentFixtureUntrusted,
}

/// <summary>Qualification state of a profile, mirroring the canonical enum.</summary>
public enum ModelQualificationStatus
{
    Qualified,
    Unqualified,
    Expired,
}

/// <summary>
/// The exact model identity the organization qualified behind a deployment
/// name. Discovery must confirm all three parts before a profile becomes
/// selectable; a deployment that now serves a different model fails closed.
/// </summary>
public sealed record ModelProfileExpectedModel(
    string Publisher,
    string Name,
    string Version);

/// <summary>
/// Capabilities Forge relies on for a profile. Discovery may narrow these by
/// refusing a deployment that lacks one; it may never broaden them.
/// </summary>
public sealed record ModelProfileCapabilities(
    bool TextInput,
    bool ImageInput,
    bool PdfInput,
    bool Attachments,
    bool ToolCalling,
    bool StructuredOutput,
    bool Reasoning,
    bool Streaming);

public sealed record ModelProfileLimits(
    int ContextTokens,
    int MaxInputTokens,
    int MaxOutputTokens,
    int MaxAttachmentBytes,
    int MaxOutputBytes);

/// <summary>
/// A named operating point of the same deployment. Variants change reasoning
/// effort, verbosity, and output budget; they never change the deployment or
/// the model identity.
/// </summary>
public sealed record ModelProfileVariant(
    string VariantId,
    string DisplayName,
    string ReasoningEffort,
    string Verbosity,
    int MaxOutputTokens);

public sealed record ModelProfileRetryPolicy(
    int MaximumAttempts,
    int InitialBackoffMilliseconds,
    int MaximumBackoffMilliseconds);

/// <summary>
/// Evidence that this exact profile was qualified. A profile without
/// qualification evidence is never selectable.
/// </summary>
public sealed record ModelProfileQualification(
    ModelQualificationStatus Status,
    string EvidenceHash,
    DateTimeOffset? QualifiedAt);

/// <summary>
/// Versioned server-side price profile. Cost is always computed from this,
/// never accepted from a provider or a client.
/// </summary>
public sealed record ModelPriceProfile(
    string Version,
    string Currency,
    long InputMicrounitsPerThousand,
    long OutputMicrounitsPerThousand,
    long ReasoningMicrounitsPerThousand,
    long CacheReadMicrounitsPerThousand);

/// <summary>
/// One versioned organization model profile, matching
/// <c>packages/contracts/schemas/model-profile.schema.json</c>
/// (<c>sapphirus.model-profile.v1</c>) field for field.
///
/// This is the only server-side type that carries a Foundry deployment name. It
/// is read exclusively from signed organization configuration and is never
/// projected to the desktop; <see cref="ModelCatalogProjection"/> is what the
/// desktop sees.
///
/// The generated Corvus types for this schema exist under
/// <c>packages/contracts/generated/dotnet</c> but are compiled only into the
/// contract-conformance project, so this service carries a hand-written record
/// exactly as <c>Contracts.cs</c> and <c>FoundryDeploymentDtos.cs</c> do.
/// </summary>
public sealed record ModelProfile(
    string SchemaVersion,
    string ProfileId,
    string DisplayName,
    string Description,
    string? Family,
    int Priority,
    bool Enabled,
    bool Visible,
    string DeploymentName,
    ModelProfileExpectedModel ExpectedModel,
    IReadOnlyList<string> PurposeAllowlist,
    IReadOnlyList<string> ModelRoleAllowlist,
    IReadOnlyList<string> RequestModeAllowlist,
    IReadOnlyList<string> CanonicalOutputSchemaAllowlist,
    IReadOnlyList<string> SupportedModalities,
    ModelProfileCapabilities RequiredCapabilities,
    ModelProfileLimits Limits,
    IReadOnlyList<ModelProfileVariant> Variants,
    ModelProfileRetryPolicy RetryPolicy,
    string RetentionMode,
    IReadOnlyList<string> AllowedRegions,
    ModelProfileQualification Qualification,
    ModelPriceProfile PriceProfile)
{
    public const string SchemaVersionValue = "sapphirus.model-profile.v1";
    public const string SupportedRetentionMode = "transient_no_store";

    /// <summary>Bound on the profile set, mirroring the catalog's array bound.</summary>
    public const int MaximumProfiles = 64;

    /// <summary>
    /// The deterministic default variant: the profile's declared order is the
    /// organization's order, so the first variant is the default operating
    /// point. Selection never reorders it.
    /// </summary>
    public ModelProfileVariant DefaultVariant => Variants[0];

    /// <summary>Server-side cost from the versioned price profile.</summary>
    public long ComputeCostMicrounits(
        long inputTokens,
        long outputTokens,
        long reasoningTokens,
        long cacheReadTokens)
    {
        if (inputTokens < 0
            || outputTokens < 0
            || reasoningTokens < 0
            || cacheReadTokens < 0)
        {
            throw new ArgumentOutOfRangeException(nameof(inputTokens));
        }
        return checked(
            (inputTokens * PriceProfile.InputMicrounitsPerThousand / 1_000)
            + (outputTokens * PriceProfile.OutputMicrounitsPerThousand / 1_000)
            + (reasoningTokens * PriceProfile.ReasoningMicrounitsPerThousand / 1_000)
            + (cacheReadTokens * PriceProfile.CacheReadMicrounitsPerThousand / 1_000));
    }
}

/// <summary>
/// An immutable, validated organization profile registry.
///
/// <see cref="RegistryHash"/> is recomputed here from the stored profile values
/// and compared against the publisher's recorded value, so the hash carried into
/// the catalog version and into consent and receipt binding is proof that the
/// server read exactly the set the publisher signed.
/// </summary>
public sealed record ModelProfileRegistrySnapshot(
    long RegistryVersion,
    string RegistryHash,
    string CapabilityHash,
    IReadOnlyList<ModelProfile> Profiles,
    ModelProfileTrust Trust,
    DateTimeOffset RefreshedAt)
{
    /// <summary>Wire discriminator of the registry-version record.</summary>
    public const string RecordSchemaVersion = "forge.model-profile-registry.v1";

    /// <summary>An empty registry never satisfies a selection; it fails closed.</summary>
    public bool IsUsable => Profiles.Count > 0;
}

/// <summary>Reads the current validated organization profile registry.</summary>
public interface IModelProfileRegistry
{
    ValueTask<ModelProfileRegistrySnapshot> GetSnapshotAsync(
        CancellationToken cancellationToken);
}
