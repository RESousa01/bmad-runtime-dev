using Azure.Data.AppConfiguration;

namespace Sapphirus.DesktopSupportApi.Policy;

/// <summary>An immutable, validated desktop policy snapshot.</summary>
public sealed record PolicySnapshot(
    string PolicyId,
    long PolicyVersion,
    bool SystemBrowserFallbackAllowed,
    int MaximumContextBytes,
    int MaximumContextItems,
    string[] AllowedRegions,
    string[] ApprovedModelDeployments,
    string RetentionMode,
    DateTimeOffset RefreshedAt);

/// <summary>Loads the raw allowlisted policy settings.</summary>
public interface IPolicySettingsSource
{
    Task<IReadOnlyDictionary<string, string>> LoadAsync(CancellationToken cancellationToken);
}

/// <summary>
/// Production source over Azure App Configuration. Only keys under the
/// policy prefix are read; labels are never merged into request behavior.
/// </summary>
public sealed class AppConfigurationPolicySettingsSource(ConfigurationClient client)
    : IPolicySettingsSource
{
    public const string Prefix = "desktop-support:policy:";

    public async Task<IReadOnlyDictionary<string, string>> LoadAsync(
        CancellationToken cancellationToken)
    {
        Dictionary<string, string> settings = new(StringComparer.Ordinal);
        SettingSelector selector = new() { KeyFilter = Prefix + "*" };
        await foreach (ConfigurationSetting setting in client
            .GetConfigurationSettingsAsync(selector, cancellationToken)
            .ConfigureAwait(false))
        {
            settings[setting.Key] = setting.Value;
        }
        return settings;
    }
}

/// <summary>
/// Validates allowlisted policy settings into an immutable snapshot with a
/// bounded refresh interval and a last-known-valid expiry. Downgrades, stale
/// refreshes, unknown fields, and out-of-policy values all fail closed.
/// </summary>
public sealed class AppConfigurationPolicyProvider(
    IPolicySettingsSource source,
    TimeProvider timeProvider,
    TimeSpan refreshInterval,
    TimeSpan lastKnownValidLifetime)
{
    private const int PolicyMaximumContextBytes = 512 * 1024;
    private const int PolicyMaximumContextItems = 64;

    private static readonly string[] RequiredKeys =
    [
        "policyId",
        "policyVersion",
        "systemBrowserFallbackAllowed",
        "maximumContextBytes",
        "maximumContextItems",
        "allowedRegions",
        "approvedModelDeployments",
        "retentionMode",
    ];

    private readonly SemaphoreSlim _gate = new(1, 1);
    private PolicySnapshot? _cached;

    public AppConfigurationPolicyProvider(
        IPolicySettingsSource source,
        TimeProvider timeProvider)
        : this(source, timeProvider, TimeSpan.FromMinutes(5), TimeSpan.FromHours(1))
    {
    }

    public async Task<PolicySnapshot> GetSnapshotAsync(CancellationToken cancellationToken)
    {
        DateTimeOffset now = timeProvider.GetUtcNow();
        await _gate.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            if (_cached is not null && now - _cached.RefreshedAt < refreshInterval)
            {
                return _cached;
            }
            IReadOnlyDictionary<string, string> settings;
            try
            {
                settings = await source
                    .LoadAsync(cancellationToken)
                    .ConfigureAwait(false);
            }
            catch (OperationCanceledException)
            {
                throw;
            }
            catch when (_cached is not null
                && now - _cached.RefreshedAt < lastKnownValidLifetime)
            {
                // Bounded last-known-valid: a transient *load* failure keeps
                // serving the verified snapshot until its lifetime lapses.
                // Validation failures below are always hard failures.
                return _cached;
            }
            PolicySnapshot refreshed = Validate(settings, now);
            if (_cached is not null && refreshed.PolicyVersion < _cached.PolicyVersion)
            {
                throw new InvalidOperationException(
                    "The refreshed policy is a downgrade of the cached policy version.");
            }
            _cached = refreshed;
            return refreshed;
        }
        finally
        {
            _gate.Release();
        }
    }

    internal static PolicySnapshot Validate(
        IReadOnlyDictionary<string, string> rawSettings,
        DateTimeOffset refreshedAt)
    {
        const string prefix = AppConfigurationPolicySettingsSource.Prefix;
        Dictionary<string, string> fields = new(StringComparer.Ordinal);
        foreach ((string key, string value) in rawSettings)
        {
            if (!key.StartsWith(prefix, StringComparison.Ordinal)
                || !RequiredKeys.Contains(key[prefix.Length..], StringComparer.Ordinal))
            {
                throw new InvalidOperationException(
                    "The policy snapshot contains an unknown configuration field.");
            }
            fields[key[prefix.Length..]] = value;
        }
        foreach (string required in RequiredKeys)
        {
            if (!fields.ContainsKey(required))
            {
                throw new InvalidOperationException(
                    "The policy snapshot is missing a required configuration field.");
            }
        }

        string[] allowedRegions = SplitList(fields["allowedRegions"]);
        string[] approvedDeployments = SplitList(fields["approvedModelDeployments"]);
        if (!ContractIds.Is(fields["policyId"], "policy")
            || !long.TryParse(fields["policyVersion"], out long policyVersion)
            || policyVersion < 1
            || !bool.TryParse(
                fields["systemBrowserFallbackAllowed"],
                out bool systemBrowserFallbackAllowed)
            || !int.TryParse(fields["maximumContextBytes"], out int maximumContextBytes)
            || maximumContextBytes is < 1 or > PolicyMaximumContextBytes
            || !int.TryParse(fields["maximumContextItems"], out int maximumContextItems)
            || maximumContextItems is < 1 or > PolicyMaximumContextItems
            || allowedRegions.Length == 0
            || allowedRegions.Any(static region => region.Length is < 1 or > 64)
            || approvedDeployments.Length == 0
            || approvedDeployments.Any(static deployment =>
                deployment.Length is < 1 or > 128)
            || fields["retentionMode"] != "transient_no_store")
        {
            throw new InvalidOperationException(
                "The policy snapshot contains out-of-policy values.");
        }

        return new PolicySnapshot(
            fields["policyId"],
            policyVersion,
            systemBrowserFallbackAllowed,
            maximumContextBytes,
            maximumContextItems,
            allowedRegions,
            approvedDeployments,
            "transient_no_store",
            refreshedAt);
    }

    private static string[] SplitList(string value) =>
        value.Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);
}
