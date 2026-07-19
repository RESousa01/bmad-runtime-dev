using Sapphirus.DesktopSupportApi.Configuration;
using Sapphirus.DesktopSupportApi.Policy;

namespace Sapphirus.DesktopSupportApi.Model;

/// <summary>
/// The single immutable model profile the broker may use. Every field is
/// resolved from the verified policy snapshot and validated production
/// configuration — request data can never choose or alter any of them.
/// </summary>
public sealed record ModelAccessProfile(
    string Deployment,
    string ApiVersion,
    string Region,
    string RetentionMode,
    string ApprovedPurpose,
    string ApprovedModelRole,
    string CanonicalOutputSchemaId,
    int MaximumContextBytes,
    int MaximumContextItems,
    int MaximumOutputBytes,
    int MaximumAttempts,
    long InputPriceMicrounitsPerThousandTokens,
    long OutputPriceMicrounitsPerThousandTokens,
    string Currency,
    string PriceProfileVersion)
{
    public const string SupportedApiVersion = "2024-10-21";

    /// <summary>
    /// Resolves the fixed profile. Fails when the configured deployment is
    /// not approved by the verified policy or the policy region does not
    /// cover the service region.
    /// </summary>
    public static ModelAccessProfile Resolve(
        PolicySnapshot policy,
        ProductionOptions production,
        SupportPlaneOptions supportPlane)
    {
        ArgumentNullException.ThrowIfNull(policy);
        ArgumentNullException.ThrowIfNull(production);
        ArgumentNullException.ThrowIfNull(supportPlane);
        if (!policy.ApprovedModelDeployments.Contains(
            production.ModelDeployment,
            StringComparer.Ordinal))
        {
            throw new InvalidOperationException(
                "The configured model deployment is not approved by the tenant policy.");
        }
        if (!policy.AllowedRegions.Contains(supportPlane.Region, StringComparer.Ordinal))
        {
            throw new InvalidOperationException(
                "The service region is not allowed by the tenant policy.");
        }
        return new ModelAccessProfile(
            production.ModelDeployment,
            SupportedApiVersion,
            supportPlane.Region,
            policy.RetentionMode,
            "bmad_help",
            "planner",
            "sapphirus.bmad-method-help-proposal.v1",
            policy.MaximumContextBytes,
            policy.MaximumContextItems,
            256 * 1024,
            3,
            InputPriceMicrounitsPerThousandTokens: 2_750,
            OutputPriceMicrounitsPerThousandTokens: 11_000,
            "EUR",
            "desktop-price-profile.2026-07");
    }

    /// <summary>Server-side cost from the versioned price profile.</summary>
    public long ComputeCostMicrounits(long inputTokens, long outputTokens)
    {
        if (inputTokens < 0 || outputTokens < 0)
        {
            throw new ArgumentOutOfRangeException(nameof(inputTokens));
        }
        return checked(
            inputTokens * InputPriceMicrounitsPerThousandTokens / 1_000
            + outputTokens * OutputPriceMicrounitsPerThousandTokens / 1_000);
    }
}
