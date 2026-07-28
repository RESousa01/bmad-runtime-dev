using System.Text.RegularExpressions;

namespace Sapphirus.DesktopSupportApi.Configuration;

public sealed partial class ProductionOptions
{
    /// <summary>Only this release channel may share one identity (FND-020).</summary>
    internal const string PilotReleaseChannel = "beta";

    private const string FoundryHostSuffix = ".services.ai.azure.com";
    private const string FoundryProjectPathPrefix = "/api/projects/";
    private const string FoundryProjectAliasName = "_project";
    private const string FoundryResponsesRelativePath = "openai/v1/";
    private const int MaximumEndpointLength = 512;
    private const int MinimumRefreshSeconds = 30;
    private const int MaximumRefreshSeconds = 3_600;
    private const int MaximumStaleSeconds = 86_400;

    public Guid ManagedIdentityClientId { get; init; }
    public Guid SqlMigrationIdentityClientId { get; init; }
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

    /// <summary>
    /// Complete Microsoft Foundry project endpoint. The account host, the
    /// project name, and nothing else: no deployment, no API version, no
    /// query. The Responses endpoint is derived from it and is never
    /// accepted separately from configuration.
    /// </summary>
    public string FoundryProjectEndpoint { get; init; } = "";

    /// <summary>
    /// Client ID of the user-assigned identity that reaches Foundry. It is
    /// distinct from the App Configuration/Key Vault/SQL runtime identity and
    /// from the SQL migration identity unless an explicit pilot flag says
    /// otherwise.
    /// </summary>
    public Guid FoundryModelIdentityClientId { get; init; }

    /// <summary>
    /// Opt-in, pilot-only escape hatch that lets the Foundry model identity be
    /// the same principal as the runtime identity. It defaults to false and is
    /// rejected outside the pilot release channel. The SQL migration identity
    /// is never shareable.
    /// </summary>
    public bool AllowSharedPilotIdentity { get; init; }

    public int FoundryDeploymentRefreshSeconds { get; init; } = 300;
    public int FoundryCatalogMaximumStaleSeconds { get; init; } = 1_800;

    /// <summary>
    /// App Configuration key prefix under which the server-owned model
    /// profiles live. Deployment names are never configured here.
    /// </summary>
    public string ModelProfileConfigurationPrefix { get; init; } =
        "desktop-support:model-profile:";

    /// <summary>
    /// True when the legacy fixed Chat Completions path is configured. It is
    /// all-or-nothing: supplying part of it is a configuration error, not a
    /// partial opt-in.
    /// </summary>
    /// <remarks>
    /// The legacy triple is optional so a Foundry-only revision validates and
    /// starts. It cannot simply be deleted yet, because the Chat Completions
    /// executor is deliberately kept alive until the Responses adapter
    /// replaces it. Leaving it required instead would mean the template that
    /// no longer injects these values could never deploy.
    /// </remarks>
    public bool HasLegacyModelPath =>
        ModelEndpoint.Length > 0
        || ModelDeployment.Length > 0
        || DeploymentHash.Length > 0;

    public Uri ValidatedAppConfigurationEndpoint { get; private set; } = null!;
    public Uri ValidatedKeyVaultUri { get; private set; } = null!;

    /// <summary>
    /// Null when the legacy Chat Completions path is not configured. Callers
    /// must consult <see cref="HasLegacyModelPath"/> first.
    /// </summary>
    public Uri? ValidatedModelEndpoint { get; private set; }

    /// <summary>Normalized project endpoint, always with a trailing slash.</summary>
    public Uri ValidatedFoundryProjectEndpoint { get; private set; } = null!;

    /// <summary>
    /// Derived Responses base address. It is computed from the validated
    /// project endpoint and has no configuration binding of its own.
    /// </summary>
    public Uri FoundryResponsesEndpoint { get; private set; } = null!;

    public TimeSpan FoundryDeploymentRefreshInterval =>
        TimeSpan.FromSeconds(FoundryDeploymentRefreshSeconds);

    public TimeSpan FoundryCatalogMaximumStaleDuration =>
        TimeSpan.FromSeconds(FoundryCatalogMaximumStaleSeconds);

    public void Validate(SupportPlaneOptions supportPlane)
    {
        ArgumentNullException.ThrowIfNull(supportPlane);
        if (ManagedIdentityClientId == Guid.Empty
            || !TryValidateEndpoint(
                AppConfigurationEndpoint,
                static host => host.EndsWith(".azconfig.io", StringComparison.OrdinalIgnoreCase),
                out Uri appConfigurationEndpoint)
            || !TryValidateEndpoint(
                KeyVaultUri,
                static host => host.EndsWith(".vault.azure.net", StringComparison.OrdinalIgnoreCase),
                out Uri keyVaultUri)
            || !TryValidateFoundryProjectEndpoint(
                FoundryProjectEndpoint,
                out Uri foundryProjectEndpoint)
            || !IsValidFoundryModelIdentity(supportPlane)
            || FoundryDeploymentRefreshSeconds
                is < MinimumRefreshSeconds or > MaximumRefreshSeconds
            || FoundryCatalogMaximumStaleSeconds < FoundryDeploymentRefreshSeconds
            || FoundryCatalogMaximumStaleSeconds > MaximumStaleSeconds
            || !ConfigurationKeyPrefix().IsMatch(ModelProfileConfigurationPrefix)
            || !SafeIdentifier().IsMatch(ReceiptSigningKeyName)
            || !SqlServer.EndsWith(".database.windows.net", StringComparison.OrdinalIgnoreCase)
            || !DnsName().IsMatch(SqlServer)
            || !SafeIdentifier().IsMatch(SqlDatabase)
            || !RequestGuards.IsSha256(ProviderProfileHash)
            || !RequestGuards.IsSha256(ModelProfileHash)
            || !RequestGuards.IsSha256(ModelCapabilityHash)
            || supportPlane.Region is "development" or "")
        {
            throw new InvalidOperationException(
                "Production configuration must provide validated private-service endpoints, managed identity, SQL, model, Foundry project, signing, region, and canonical profile hashes.");
        }

        if (HasLegacyModelPath)
        {
            if (!TryValidateEndpoint(
                    ModelEndpoint,
                    static host =>
                        host.EndsWith(".openai.azure.com", StringComparison.OrdinalIgnoreCase)
                        || host.EndsWith(
                            ".cognitiveservices.azure.com",
                            StringComparison.OrdinalIgnoreCase),
                    out Uri modelEndpoint)
                || !SafeIdentifier().IsMatch(ModelDeployment)
                || !RequestGuards.IsSha256(DeploymentHash))
            {
                throw new InvalidOperationException(
                    "The legacy fixed model path is all-or-nothing: supplying any of the model endpoint, deployment, or deployment hash requires all three to be valid.");
            }

            ValidatedModelEndpoint = modelEndpoint;
        }

        ValidatedAppConfigurationEndpoint = appConfigurationEndpoint;
        ValidatedKeyVaultUri = keyVaultUri;
        ValidatedFoundryProjectEndpoint = foundryProjectEndpoint;
        FoundryResponsesEndpoint = new Uri(
            foundryProjectEndpoint,
            FoundryResponsesRelativePath);
    }

    internal void ValidateSqlMigrationIdentity()
    {
        if (SqlMigrationIdentityClientId == Guid.Empty
            || SqlMigrationIdentityClientId == ManagedIdentityClientId)
        {
            throw new InvalidOperationException(
                "SQL migration requires a distinct, non-empty migration managed identity client ID.");
        }
    }

    /// <summary>
    /// The Foundry model identity must exist, must never be the SQL migration
    /// identity, and may equal the runtime identity only when an operator has
    /// explicitly opted in on the pilot channel.
    /// </summary>
    private bool IsValidFoundryModelIdentity(SupportPlaneOptions supportPlane)
    {
        if (FoundryModelIdentityClientId == Guid.Empty
            || FoundryModelIdentityClientId == SqlMigrationIdentityClientId)
        {
            return false;
        }
        if (FoundryModelIdentityClientId != ManagedIdentityClientId)
        {
            return true;
        }
        return AllowSharedPilotIdentity
            && string.Equals(
                supportPlane.ReleaseChannel,
                PilotReleaseChannel,
                StringComparison.Ordinal);
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

    /// <summary>
    /// Validates a Foundry project endpoint. <see cref="TryValidateEndpoint"/>
    /// cannot be reused: it requires a root path, and every project endpoint
    /// carries <c>/api/projects/&lt;project&gt;</c>.
    ///
    /// The configured value must be byte-identical to the canonical form
    /// (optionally without the trailing slash). That single comparison is what
    /// rejects mixed case, percent-encoding, dot-segment traversal, an
    /// explicit <c>:443</c>, and any other spelling that would parse to the
    /// same resource but hash or compare differently later.
    /// </summary>
    internal static bool TryValidateFoundryProjectEndpoint(
        string value,
        out Uri endpoint)
    {
        endpoint = null!;
        if (string.IsNullOrEmpty(value)
            || value.Length > MaximumEndpointLength
            || !Uri.TryCreate(value, UriKind.Absolute, out Uri? candidate)
            || candidate.Scheme != Uri.UriSchemeHttps
            || !candidate.IsDefaultPort
            || candidate.Port != 443
            || candidate.UserInfo.Length != 0
            || candidate.Query.Length != 0
            || candidate.Fragment.Length != 0)
        {
            return false;
        }

        string host = candidate.Host;
        if (!IsPlainAsciiDnsHost(host)
            || host.Length <= FoundryHostSuffix.Length
            || !host.EndsWith(FoundryHostSuffix, StringComparison.Ordinal))
        {
            return false;
        }

        string path = candidate.AbsolutePath;
        if (!path.StartsWith(FoundryProjectPathPrefix, StringComparison.Ordinal))
        {
            return false;
        }
        string project = path[FoundryProjectPathPrefix.Length..].TrimEnd('/');
        if (!string.Equals(project, FoundryProjectAliasName, StringComparison.Ordinal)
            && !SafeIdentifier().IsMatch(project))
        {
            return false;
        }

        string canonical =
            $"https://{host}{FoundryProjectPathPrefix}{project}/";
        if (!string.Equals(value, canonical, StringComparison.Ordinal)
            && !string.Equals(value, canonical[..^1], StringComparison.Ordinal))
        {
            return false;
        }

        endpoint = new Uri(canonical, UriKind.Absolute);
        return true;
    }

    /// <summary>
    /// Accepts only a lowercase ASCII DNS name. Unicode hosts and their
    /// punycode encodings are both rejected, so a homograph account name can
    /// never satisfy the trusted host suffix.
    /// </summary>
    private static bool IsPlainAsciiDnsHost(string host)
    {
        if (host.Length is < 1 or > 253)
        {
            return false;
        }
        foreach (string label in host.Split('.'))
        {
            if (label.Length is < 1 or > 63
                || label.StartsWith("xn--", StringComparison.OrdinalIgnoreCase)
                || label[0] == '-'
                || label[^1] == '-')
            {
                return false;
            }
            foreach (char character in label)
            {
                if (character is not ((>= 'a' and <= 'z') or (>= '0' and <= '9') or '-'))
                {
                    return false;
                }
            }
        }
        return true;
    }

    [GeneratedRegex("^[A-Za-z0-9][A-Za-z0-9_-]{0,126}[A-Za-z0-9]$")]
    private static partial Regex SafeIdentifier();

    [GeneratedRegex("^[A-Za-z0-9.-]{1,253}$")]
    private static partial Regex DnsName();

    [GeneratedRegex("^[a-z0-9][a-z0-9_-]{0,30}(:[a-z0-9][a-z0-9_-]{0,30}){0,3}:$")]
    private static partial Regex ConfigurationKeyPrefix();
}
