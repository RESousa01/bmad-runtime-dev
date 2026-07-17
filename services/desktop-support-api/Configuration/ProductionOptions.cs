using System.Text.RegularExpressions;

namespace Sapphirus.DesktopSupportApi.Configuration;

public sealed partial class ProductionOptions
{
    public Guid ManagedIdentityClientId { get; init; }
    public string AppConfigurationEndpoint { get; init; } = "";
    public string KeyVaultUri { get; init; } = "";
    public string ReceiptSigningKeyName { get; init; } = "";
    public string SqlServer { get; init; } = "";
    public string SqlDatabase { get; init; } = "";
    public string ModelEndpoint { get; init; } = "";
    public string ModelDeployment { get; init; } = "";
    public string ProviderProfileHash { get; init; } = "";
    public string ModelProfileHash { get; init; } = "";
    public string ModelCapabilityHash { get; init; } = "";
    public string DeploymentHash { get; init; } = "";

    public Uri ValidatedAppConfigurationEndpoint { get; private set; } = null!;
    public Uri ValidatedKeyVaultUri { get; private set; } = null!;
    public Uri ValidatedModelEndpoint { get; private set; } = null!;

    public void Validate(SupportPlaneOptions supportPlane)
    {
        if (ManagedIdentityClientId == Guid.Empty
            || !TryValidateEndpoint(
                AppConfigurationEndpoint,
                static host => host.EndsWith(".azconfig.io", StringComparison.OrdinalIgnoreCase),
                out Uri appConfigurationEndpoint)
            || !TryValidateEndpoint(
                KeyVaultUri,
                static host => host.EndsWith(".vault.azure.net", StringComparison.OrdinalIgnoreCase),
                out Uri keyVaultUri)
            || !TryValidateEndpoint(
                ModelEndpoint,
                static host => host.EndsWith(".openai.azure.com", StringComparison.OrdinalIgnoreCase)
                    || host.EndsWith(
                        ".cognitiveservices.azure.com",
                        StringComparison.OrdinalIgnoreCase),
                out Uri modelEndpoint)
            || !SafeIdentifier().IsMatch(ReceiptSigningKeyName)
            || !SqlServer.EndsWith(".database.windows.net", StringComparison.OrdinalIgnoreCase)
            || !DnsName().IsMatch(SqlServer)
            || !SafeIdentifier().IsMatch(SqlDatabase)
            || !SafeIdentifier().IsMatch(ModelDeployment)
            || !RequestGuards.IsSha256(ProviderProfileHash)
            || !RequestGuards.IsSha256(ModelProfileHash)
            || !RequestGuards.IsSha256(ModelCapabilityHash)
            || !RequestGuards.IsSha256(DeploymentHash)
            || supportPlane.Region is "development" or "")
        {
            throw new InvalidOperationException(
                "Production configuration must provide validated private-service endpoints, managed identity, SQL, model, signing, region, and canonical profile hashes.");
        }

        ValidatedAppConfigurationEndpoint = appConfigurationEndpoint;
        ValidatedKeyVaultUri = keyVaultUri;
        ValidatedModelEndpoint = modelEndpoint;
    }

    private static bool TryValidateEndpoint(
        string value,
        Func<string, bool> hostValidator,
        out Uri endpoint)
    {
        endpoint = null!;
        if (!Uri.TryCreate(value, UriKind.Absolute, out Uri? candidate)
            || candidate.Scheme != Uri.UriSchemeHttps
            || !candidate.IsDefaultPort
            || candidate.UserInfo.Length != 0
            || candidate.Query.Length != 0
            || candidate.Fragment.Length != 0
            || candidate.AbsolutePath != "/"
            || !hostValidator(candidate.Host))
        {
            return false;
        }
        endpoint = candidate;
        return true;
    }

    [GeneratedRegex("^[A-Za-z0-9][A-Za-z0-9_-]{0,126}[A-Za-z0-9]$")]
    private static partial Regex SafeIdentifier();

    [GeneratedRegex("^[A-Za-z0-9.-]{1,253}$")]
    private static partial Regex DnsName();
}
