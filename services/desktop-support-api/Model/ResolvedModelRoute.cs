namespace Sapphirus.DesktopSupportApi.Model;

/// <summary>
/// The immutable server-side resolution of one model request.
///
/// <para>
/// This type is SERVER-ONLY. It is the only place a Foundry deployment name
/// exists at request time, and nothing that serializes to a desktop client may
/// ever reference it. The desktop names a profile and a catalog epoch; the
/// support plane decides what that means.
/// </para>
/// <para>
/// It is also the retry unit. A retry re-dispatches this exact route with the
/// exact request bytes that produced it: there is no re-resolution between
/// attempts, and therefore no path by which attempt two reaches a different
/// profile, deployment, variant, or limit than attempt one. There is no
/// cross-profile fallback anywhere in this implementation.
/// </para>
/// </summary>
public sealed record ResolvedModelRoute(
    string RouteHash,
    string ProfileId,
    string ProfileBindingHash,
    string CapabilityBindingHash,
    string RegistryHash,
    string CatalogVersion,
    string CatalogHash,
    string DeploymentName,
    string DeploymentHash,
    string ModelPublisher,
    string ModelName,
    string ModelVersion,
    string VariantId,
    string ReasoningEffort,
    string Verbosity,
    string RequestMode,
    string? CanonicalOutputSchemaId,
    ProjectedCapabilities Capabilities,
    int ContextTokens,
    int MaxInputTokens,
    int MaxOutputTokens,
    int MaxOutputBytes,
    int MaxAttachmentBytes,
    string RetentionMode,
    string Region,
    ModelProfileRetryPolicy RetryPolicy,
    ModelPriceProfile PriceProfile)
{
    /// <summary>Hash purpose for the route digest.</summary>
    public const string RouteHashPurpose = "model-route";

    /// <summary>Hash purpose for the deployment binding digest.</summary>
    public const string DeploymentBindingHashPurpose = "model-route-deployment";

    /// <summary>Hash purpose for the profile binding the consent is bound to.</summary>
    public const string ProfileBindingHashPurpose = "model-profile-binding";

    /// <summary>Hash purpose for the capability binding the consent is bound to.</summary>
    public const string CapabilityBindingHashPurpose = "model-capability-binding";

    /// <summary>
    /// The deployment binding digest. It is computed from server-owned data
    /// only and is what a receipt records, so a receipt can prove which
    /// deployment served a request without the deployment name ever appearing
    /// in it.
    /// </summary>
    public string ComputeDeploymentBindingHash() =>
        ModelProfileValidator.Digest(
            DeploymentBindingHashPurpose,
            new SortedDictionary<string, object?>(StringComparer.Ordinal)
            {
                ["deploymentHash"] = DeploymentHash,
                ["modelName"] = ModelName,
                ["modelPublisher"] = ModelPublisher,
                ["modelVersion"] = ModelVersion,
                ["profileId"] = ProfileId,
                ["region"] = Region,
            });
}

/// <summary>Stable route resolution failure codes.</summary>
public static class ModelRouteErrorCodes
{
    public const string CatalogStale = "catalog_stale";
    public const string ProfileUnavailable = "profile_unavailable";
    public const string ProfileNotEntitled = "profile_not_entitled";
    public const string VariantInvalid = "variant_invalid";
    public const string CapabilityMismatch = "capability_mismatch";
    public const string DeploymentMissing = "deployment_missing";
    public const string ContextLimitExceeded = "context_limit_exceeded";
    public const string ProfileBindingMismatch = "profile_binding_mismatch";
}

public sealed record ModelRouteResolution(ResolvedModelRoute? Route, string ErrorCode)
{
    public bool IsSuccess => Route is not null;
}
