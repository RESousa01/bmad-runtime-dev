using Sapphirus.DesktopSupportApi.Model;

namespace Sapphirus.DesktopSupportApi;

public interface ISignedPolicyService
{
    Task<SignedDesktopPolicy> CurrentPolicyAsync(CancellationToken cancellationToken);
    Task<SignedEntitlementLease> CreateLeaseAsync(
        string subject,
        string registrationId,
        CancellationToken cancellationToken);
}

public interface IModelAccessBroker
{
    Task<ModelAccessResult> CompleteAsync(
        string subject,
        ModelAccessRequest request,
        CancellationToken cancellationToken);
}

public sealed record UnsignedModelAccessResult(
    string OutputSchemaId,
    string PayloadJson,
    string PayloadHash,
    string SchemaProjectionHash,
    string CredentialBindingHash,
    ModelAccessUsage Usage,
    int RetryCount,
    ModelFallbackEvent[] FallbackEvents,
    string? ProviderRequestId,
    DateTimeOffset StartedAt,
    DateTimeOffset CompletedAt,
    string TerminalStatus);

public interface IModelReceiptSigner
{
    Task<ModelAccessResult> SignAsync(
        string subject,
        ModelAccessRequest request,
        UnsignedModelAccessResult result,
        CancellationToken cancellationToken);
}

/// <summary>
/// Everything a version two receipt records that the server, not the request,
/// decides.
///
/// <para>
/// The version one receipt copied the four profile hashes straight out of the
/// client's consent envelope, which made them a statement about what the
/// client believed rather than about what the server did. Every hash here is
/// recomputed from <see cref="Model.ResolvedModelRoute"/>, so the receipt
/// proves which profile, catalog epoch, variant, and deployment actually
/// served the call.
/// </para>
/// </summary>
public sealed record ResolvedReceiptBinding(
    string ProviderProfileHash,
    string ModelProfileHash,
    string ModelCapabilityHash,
    string ResolvedProfileId,
    string ResolvedProfileHash,
    string ResolvedVariantId,
    string CatalogVersion,
    string CatalogHash,
    string DeploymentInventoryHash,
    string ResolvedDeploymentHash,
    ModelResolvedIdentity ResolvedModel,
    string CredentialBindingHash,
    string Region,
    string RetentionMode);

/// <summary>
/// The authorization facts a receipt binds itself to, lifted out of the wire
/// request.
///
/// <para>
/// The signer takes this rather than a request record so it is version
/// agnostic: a version one request adapted forward and a native version two
/// request produce the same binding surface, and the signer cannot accidentally
/// read a field only one of them has.
/// </para>
/// </summary>
public sealed record ModelReceiptAuthorization(
    string RequestId,
    string RequestHash,
    string RegistrationId,
    string TenantHash,
    string SubjectHash,
    string ManifestHash,
    string InvocationBindingHash,
    string ConsumptionHash,
    string ConsentEnvelopeHash,
    string ConsentDisclosureHash,
    string RetentionMode,
    long InputBytes);

/// <summary>
/// The unsigned version two result. It exists beside
/// <see cref="UnsignedModelAccessResult"/> rather than replacing it, because
/// historical version one receipts must stay verifiable under their own
/// digest.
/// </summary>
public sealed record UnsignedModelAccessResultV2(
    string RunId,
    IModelResponseLane ResponseLane,
    string PayloadJson,
    string PayloadHash,
    ResolvedReceiptBinding Binding,
    string RequestedProfileId,
    string? RequestedVariantId,
    string? ReportedResponseModel,
    ModelUsageV2 Usage,
    long OutputBytes,
    int RetryCount,
    ModelFallbackEventV2[] FallbackEvents,
    string? ProviderRequestId,
    DateTimeOffset StartedAt,
    DateTimeOffset CompletedAt,
    string TerminalStatus);

/// <summary>
/// Signs a version two receipt. Only a successful, validated result reaches
/// this seam: failed, refused, and cancelled runs produce evidence, never a
/// success receipt.
/// </summary>
public interface IModelReceiptSignerV2
{
    Task<ModelAccessReceiptV2> SignAsync(
        string subject,
        ModelReceiptAuthorization authorization,
        UnsignedModelAccessResultV2 result,
        CancellationToken cancellationToken);
}

public enum ContextConsentVerification
{
    Verified,
    Rejected,
    Unavailable,
}

public sealed record ContextConsentVerificationRequest(
    string Subject,
    RegisteredDevice Device,
    ModelAccessRequest Request,
    string RecomputedManifestHash);

public interface IContextConsentVerifier
{
    ValueTask<ContextConsentVerification> VerifyAsync(
        ContextConsentVerificationRequest request,
        CancellationToken cancellationToken);
}

/// <summary>
/// The canonical consent envelope is present and structurally bound to the exact request. The
/// default still fails closed because installation-key signature verification and durable
/// single-use consumption have no production provider until the external key resources exist.
/// </summary>
public sealed class UnavailableContextConsentVerifier : IContextConsentVerifier
{
    public ValueTask<ContextConsentVerification> VerifyAsync(
        ContextConsentVerificationRequest request,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        return ValueTask.FromResult(ContextConsentVerification.Unavailable);
    }
}

/// <summary>
/// Computes the server-authoritative parts of a version two receipt.
///
/// <para>
/// Everything here is derived from the resolved route and the credential
/// mechanism. Nothing is copied from the request or the consent envelope, so a
/// receipt records what the support plane did rather than what the desktop
/// asked for.
/// </para>
/// </summary>
public static class ModelReceiptBinding
{
    /// <summary>Hash purpose for the credential binding digest.</summary>
    public const string CredentialBindingPurpose = "model-credential-binding";

    /// <summary>Hash purpose for the provider profile digest.</summary>
    public const string ProviderProfilePurpose = "model-provider-profile";

    /// <summary>The one provider surface this plane calls.</summary>
    public const string ProviderApiSurface = "foundry_responses";

    /// <summary>
    /// The credential binding. The mechanism identifier is an input, so a
    /// receipt produced under the development API key can never be mistaken
    /// for one produced under managed identity even when the subject, route,
    /// and price profile are byte-identical.
    /// </summary>
    public static string ComputeCredentialBindingHash(
        ResolvedModelRoute route,
        string credentialMechanismId,
        string subject)
    {
        ArgumentNullException.ThrowIfNull(route);
        ArgumentException.ThrowIfNullOrWhiteSpace(credentialMechanismId);
        ArgumentException.ThrowIfNullOrWhiteSpace(subject);
        return ModelProfileValidator.Digest(
            CredentialBindingPurpose,
            new SortedDictionary<string, object?>(StringComparer.Ordinal)
            {
                ["apiSurface"] = ProviderApiSurface,
                ["credentialMechanismId"] = credentialMechanismId,
                ["deploymentBindingHash"] = route.ComputeDeploymentBindingHash(),
                ["priceProfileVersion"] = route.PriceProfile.Version,
                ["profileId"] = route.ProfileId,
                ["region"] = route.Region,
                ["subject"] = subject,
                ["variantId"] = route.VariantId,
            });
    }

    /// <summary>
    /// The provider profile the call actually used. The version one receipt
    /// copied this from the consent envelope; here it is recomputed, so it can
    /// no longer assert a provider surface the plane never called.
    /// </summary>
    public static string ComputeProviderProfileHash(
        ResolvedModelRoute route,
        string credentialMechanismId)
    {
        ArgumentNullException.ThrowIfNull(route);
        ArgumentException.ThrowIfNullOrWhiteSpace(credentialMechanismId);
        return ModelProfileValidator.Digest(
            ProviderProfilePurpose,
            new SortedDictionary<string, object?>(StringComparer.Ordinal)
            {
                ["apiSurface"] = ProviderApiSurface,
                ["credentialMechanismId"] = credentialMechanismId,
                ["deploymentBindingHash"] = route.ComputeDeploymentBindingHash(),
                ["region"] = route.Region,
                ["requestMode"] = route.RequestMode,
                ["retentionMode"] = route.RetentionMode,
            });
    }

    /// <summary>Assembles the full server-authoritative binding.</summary>
    public static ResolvedReceiptBinding Create(
        ResolvedModelRoute route,
        string deploymentInventoryHash,
        string credentialMechanismId,
        string subject)
    {
        ArgumentNullException.ThrowIfNull(route);
        ArgumentException.ThrowIfNullOrWhiteSpace(deploymentInventoryHash);
        return new ResolvedReceiptBinding(
            ComputeProviderProfileHash(route, credentialMechanismId),
            route.ProfileBindingHash,
            route.CapabilityBindingHash,
            route.ProfileId,
            route.ProfileBindingHash,
            route.VariantId,
            route.CatalogVersion,
            route.CatalogHash,
            deploymentInventoryHash,
            route.ComputeDeploymentBindingHash(),
            new ModelResolvedIdentity(
                route.ModelPublisher,
                route.ModelName,
                route.ModelVersion),
            ComputeCredentialBindingHash(route, credentialMechanismId, subject),
            route.Region,
            route.RetentionMode);
    }
}

/// <summary>
/// Normalizes provider usage into the version two usage record.
///
/// <para>
/// A category the provider never reported stays null; zero means the provider
/// reported zero. Cost is computed from the versioned server-side price
/// profile and never accepted from a provider or a client.
/// </para>
/// <para>
/// Cached input tokens and reasoning output tokens are treated as subsets of
/// the input and output totals respectively, which is how both providers
/// report them, so they are priced at their own rate and subtracted from the
/// base total rather than added on top of it.
/// </para>
/// </summary>
public static class ModelUsageProjector
{
    /// <summary>
    /// Projects reported usage. Null input, or a report with no category at
    /// all, produces the fully unavailable record rather than a zeroed one.
    /// </summary>
    public static ModelUsageV2 Project(
        ModelProviderReportedUsage? reported,
        ModelPriceProfile priceProfile)
    {
        ArgumentNullException.ThrowIfNull(priceProfile);
        if (reported is null
            || (reported.InputTokens is null
                && reported.OutputTokens is null
                && reported.ReasoningTokens is null
                && reported.CacheReadTokens is null
                && reported.CacheWriteTokens is null))
        {
            return ModelUsageV2.Unavailable(priceProfile.Currency, priceProfile.Version);
        }
        return new ModelUsageV2(
            ModelUsageProvenance.Reported,
            reported.InputTokens,
            reported.OutputTokens,
            reported.ReasoningTokens,
            reported.CacheReadTokens,
            reported.CacheWriteTokens,
            ComputeCostMicrounits(reported, priceProfile),
            priceProfile.Currency,
            priceProfile.Version);
    }

    /// <summary>
    /// Server-side cost. Null when no billable category was reported, because
    /// a zero cost and an unknown cost are different claims.
    /// </summary>
    public static long? ComputeCostMicrounits(
        ModelProviderReportedUsage reported,
        ModelPriceProfile priceProfile)
    {
        ArgumentNullException.ThrowIfNull(reported);
        ArgumentNullException.ThrowIfNull(priceProfile);
        if (reported.InputTokens is null && reported.OutputTokens is null)
        {
            return null;
        }
        long cacheRead = Clamp(reported.CacheReadTokens ?? 0);
        long reasoning = Clamp(reported.ReasoningTokens ?? 0);
        long billableInput = Math.Max(0, Clamp(reported.InputTokens ?? 0) - cacheRead);
        long billableOutput = Math.Max(0, Clamp(reported.OutputTokens ?? 0) - reasoning);
        return checked(
            (billableInput * priceProfile.InputMicrounitsPerThousand / 1_000)
            + (cacheRead * priceProfile.CacheReadMicrounitsPerThousand / 1_000)
            + (billableOutput * priceProfile.OutputMicrounitsPerThousand / 1_000)
            + (reasoning * priceProfile.ReasoningMicrounitsPerThousand / 1_000));
    }

    private static long Clamp(long value) => value < 0 ? 0 : value;
}
