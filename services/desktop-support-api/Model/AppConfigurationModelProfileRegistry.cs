using Azure.Data.AppConfiguration;
using Microsoft.Extensions.Hosting;

namespace Sapphirus.DesktopSupportApi.Model;

/// <summary>
/// The raw organization profile settings as stored: the canonical value of each
/// profile key, and the separate registry-version record.
/// </summary>
public sealed record ModelProfileSettings(
    IReadOnlyDictionary<string, string> ProfileValues,
    string? RegistryRecord);

/// <summary>Loads the raw allowlisted model profile settings.</summary>
public interface IModelProfileSettingsSource
{
    Task<ModelProfileSettings> LoadAsync(CancellationToken cancellationToken);
}

/// <summary>
/// Production source over Azure App Configuration.
///
/// <para>
/// Two reads, deliberately: a prefix key filter for the profile keys, and a
/// second exact-key read for the registry-version record. The record's key
/// sits outside the profile prefix on purpose, so the prefix filter cannot
/// pick it up and mistake it for a profile, and so a reader can never observe
/// a half-published set: the publisher writes the record last, and until it
/// names a profile that profile is inert.
/// </para>
/// <para>
/// The label is pinned. The publisher writes the live set under one label and
/// every superseded set under an immutable per-version archive label, so a
/// reader that did not pin the label could merge a live profile with an
/// archived one.
/// </para>
/// </summary>
public sealed class AppConfigurationModelProfileSettingsSource(
    ConfigurationClient client,
    string keyPrefix,
    string label)
    : IModelProfileSettingsSource
{
    /// <summary>
    /// The key prefix <c>tools/foundry-profiles/publish-profiles.ps1</c>
    /// actually writes.
    /// </summary>
    public const string PublisherKeyPrefix = "desktop-support:model-profile:";

    /// <summary>
    /// The registry-version key. It is not under
    /// <see cref="PublisherKeyPrefix"/>: after <c>model-profile</c> it
    /// continues with <c>-registry</c> rather than the prefix's terminating
    /// colon, so <c>KeyFilter = prefix + "*"</c> can never match it.
    /// </summary>
    public const string RegistryVersionKey = "desktop-support:model-profile-registry:version";

    /// <summary>The label the publisher writes the live set under.</summary>
    public const string DefaultLabel = "production";

    public async Task<ModelProfileSettings> LoadAsync(CancellationToken cancellationToken)
    {
        Dictionary<string, string> profiles = new(StringComparer.Ordinal);
        SettingSelector profileSelector = new()
        {
            KeyFilter = keyPrefix + "*",
            LabelFilter = label,
        };
        await foreach (ConfigurationSetting setting in client
            .GetConfigurationSettingsAsync(profileSelector, cancellationToken)
            .ConfigureAwait(false))
        {
            if (!setting.Key.StartsWith(keyPrefix, StringComparison.Ordinal))
            {
                // A store-side filter surprise is a reason to stop, not to
                // guess which key was meant.
                throw new InvalidOperationException(
                    "The profile key filter returned a key outside the configured prefix.");
            }
            profiles[setting.Key[keyPrefix.Length..]] = setting.Value;
        }

        string? record = null;
        SettingSelector recordSelector = new()
        {
            KeyFilter = RegistryVersionKey,
            LabelFilter = label,
        };
        await foreach (ConfigurationSetting setting in client
            .GetConfigurationSettingsAsync(recordSelector, cancellationToken)
            .ConfigureAwait(false))
        {
            if (!string.Equals(setting.Key, RegistryVersionKey, StringComparison.Ordinal))
            {
                continue;
            }
            record = setting.Value;
        }
        return new ModelProfileSettings(profiles, record);
    }
}

/// <summary>
/// Validates signed organization profile settings into an immutable snapshot,
/// with the caching discipline <c>AppConfigurationPolicyProvider</c> already
/// establishes: one gate, a bounded refresh interval, and a bounded
/// last-known-valid window that only a transient load failure may use.
///
/// <para>
/// Everything else fails closed. A registry-version rollback is refused, a
/// registry hash that does not match the values actually read is refused, a
/// capability hash that does not match the capability surface is refused, and
/// a profile named by the record but absent from the store is refused rather
/// than skipped.
/// </para>
/// </summary>
public sealed class AppConfigurationModelProfileRegistry : IModelProfileRegistry, IDisposable
{
    public static readonly TimeSpan DefaultRefreshInterval = TimeSpan.FromMinutes(5);
    public static readonly TimeSpan DefaultLastKnownValidLifetime = TimeSpan.FromHours(1);

    private readonly IModelProfileSettingsSource _source;
    private readonly TimeProvider _timeProvider;
    private readonly TimeSpan _refreshInterval;
    private readonly TimeSpan _lastKnownValidLifetime;
    private readonly SemaphoreSlim _gate = new(1, 1);

    private ModelProfileRegistrySnapshot? _cached;

    public AppConfigurationModelProfileRegistry(
        IModelProfileSettingsSource source,
        TimeProvider timeProvider)
        : this(source, timeProvider, DefaultRefreshInterval, DefaultLastKnownValidLifetime)
    {
    }

    public AppConfigurationModelProfileRegistry(
        IModelProfileSettingsSource source,
        TimeProvider timeProvider,
        TimeSpan refreshInterval,
        TimeSpan lastKnownValidLifetime)
    {
        ArgumentNullException.ThrowIfNull(source);
        ArgumentNullException.ThrowIfNull(timeProvider);
        if (refreshInterval <= TimeSpan.Zero || lastKnownValidLifetime < refreshInterval)
        {
            throw new ArgumentOutOfRangeException(
                nameof(lastKnownValidLifetime),
                "The last-known-valid window must cover at least one refresh interval.");
        }
        _source = source;
        _timeProvider = timeProvider;
        _refreshInterval = refreshInterval;
        _lastKnownValidLifetime = lastKnownValidLifetime;
    }

    public async ValueTask<ModelProfileRegistrySnapshot> GetSnapshotAsync(
        CancellationToken cancellationToken)
    {
        DateTimeOffset now = _timeProvider.GetUtcNow();
        await _gate.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            if (_cached is not null && now - _cached.RefreshedAt < _refreshInterval)
            {
                return _cached;
            }
            ModelProfileSettings settings;
            try
            {
                settings = await _source.LoadAsync(cancellationToken).ConfigureAwait(false);
            }
            catch (OperationCanceledException)
            {
                throw;
            }
            catch when (_cached is not null
                && now - _cached.RefreshedAt < _lastKnownValidLifetime)
            {
                // Bounded last-known-valid: only a transient *load* failure
                // keeps serving the verified snapshot. Every validation failure
                // below is a hard failure.
                return _cached;
            }
            ModelProfileRegistrySnapshot refreshed = Validate(settings, now);
            if (_cached is not null && refreshed.RegistryVersion < _cached.RegistryVersion)
            {
                throw new InvalidOperationException(
                    "model_profile_registry_version_rollback");
            }
            _cached = refreshed;
            return refreshed;
        }
        finally
        {
            _gate.Release();
        }
    }

    /// <summary>
    /// Validates a raw settings read into a snapshot. The registry hash is
    /// recomputed from the stored values rather than trusted, so this is the
    /// point at which "the server used exactly the set the publisher signed"
    /// stops being an assumption.
    /// </summary>
    internal static ModelProfileRegistrySnapshot Validate(
        ModelProfileSettings settings,
        DateTimeOffset refreshedAt)
    {
        ArgumentNullException.ThrowIfNull(settings);
        if (!ModelProfileValidator.TryParseRegistryRecord(
            settings.RegistryRecord,
            out ModelProfileRegistryRecord? record,
            out string recordError))
        {
            throw new InvalidOperationException(recordError);
        }

        if (settings.ProfileValues.Count > ModelProfile.MaximumProfiles)
        {
            throw new InvalidOperationException("model_profile_registry_too_large");
        }

        List<ModelProfile> profiles = new(record!.ProfileIds.Count);
        List<(string ProfileId, string ProfileHash)> hashes = new(record.ProfileIds.Count);
        foreach (string profileId in record.ProfileIds)
        {
            if (!settings.ProfileValues.TryGetValue(profileId, out string? rawValue))
            {
                // The record is the manifest. A profile it names but the store
                // does not carry means the set is incomplete, and an incomplete
                // set can never be reconciled with the signed registry hash.
                throw new InvalidOperationException("model_profile_registry_profile_missing");
            }
            if (!ModelProfileValidator.TryParseProfile(
                rawValue,
                out ModelProfile? profile,
                out string profileError))
            {
                throw new InvalidOperationException(profileError);
            }
            if (!string.Equals(profile!.ProfileId, profileId, StringComparison.Ordinal))
            {
                throw new InvalidOperationException("model_profile_key_identity_mismatch");
            }
            profiles.Add(profile);
            hashes.Add((profileId, ModelProfileValidator.ComputeProfileValueHash(rawValue)));
        }

        if (!ModelProfileValidator.ValidateSet(profiles, out string setError))
        {
            throw new InvalidOperationException(setError);
        }

        string registryHash = ModelProfileValidator.ComputeRegistryHash(
            record.RegistryVersion,
            hashes);
        if (!string.Equals(registryHash, record.RegistryHash, StringComparison.Ordinal))
        {
            throw new InvalidOperationException("model_profile_registry_hash_mismatch");
        }
        string capabilityHash = ModelProfileValidator.ComputeCapabilityHash(profiles);
        if (!string.Equals(capabilityHash, record.CapabilityHash, StringComparison.Ordinal))
        {
            throw new InvalidOperationException("model_profile_capability_hash_mismatch");
        }

        return new ModelProfileRegistrySnapshot(
            record.RegistryVersion,
            registryHash,
            capabilityHash,
            profiles,
            ModelProfileTrust.Signed,
            refreshedAt);
    }

    public void Dispose() => _gate.Dispose();
}

/// <summary>
/// A Development-only fixture registry.
///
/// <para>
/// It exists so the whole catalog architecture — discovery, projection,
/// default selection, and route resolution — can be exercised on a machine
/// with no App Configuration store. Its snapshots are labelled
/// <see cref="ModelProfileTrust.DevelopmentFixtureUntrusted"/>, and the
/// constructor refuses any environment but Development and Test, mirroring the
/// fake Foundry server's guard.
/// </para>
/// </summary>
public sealed class DevelopmentFixtureModelProfileRegistry : IModelProfileRegistry
{
    /// <summary>Parallel to <c>fake_foundry_environment_forbidden</c>.</summary>
    public const string EnvironmentForbiddenCode =
        "development_model_profile_registry_environment_forbidden";

    private readonly ModelProfileRegistrySnapshot _snapshot;

    public DevelopmentFixtureModelProfileRegistry(
        string environmentName,
        IReadOnlyList<ModelProfile> profiles,
        TimeProvider timeProvider)
    {
        ArgumentNullException.ThrowIfNull(profiles);
        ArgumentNullException.ThrowIfNull(timeProvider);
        if (!IsPermittedEnvironment(environmentName))
        {
            throw new InvalidOperationException(EnvironmentForbiddenCode);
        }
        if (!ModelProfileValidator.ValidateSet(profiles, out string setError))
        {
            throw new InvalidOperationException(setError);
        }
        _snapshot = new ModelProfileRegistrySnapshot(
            RegistryVersion: 1,
            ModelProfileValidator.ComputeRegistryHash(
                1,
                [.. profiles.Select(static profile => (
                    profile.ProfileId,
                    ModelProfileValidator.Digest("model-profile-development-fixture", profile.ProfileId)))]),
            ModelProfileValidator.ComputeCapabilityHash(profiles),
            profiles,
            ModelProfileTrust.DevelopmentFixtureUntrusted,
            timeProvider.GetUtcNow());
    }

    public static bool IsPermittedEnvironment(string? environmentName) =>
        string.Equals(environmentName, Environments.Development, StringComparison.Ordinal)
        || string.Equals(environmentName, "Test", StringComparison.Ordinal);

    public ValueTask<ModelProfileRegistrySnapshot> GetSnapshotAsync(
        CancellationToken cancellationToken) =>
        ValueTask.FromResult(_snapshot);
}

/// <summary>
/// The built-in Development fixture profile set.
///
/// Every value is obviously synthetic. In particular the deployment name is a
/// placeholder that no real project carries, so a developer who points this at
/// a real Foundry project sees an honest <c>deployment_missing</c> rather than
/// a profile that appears to work because a plausible name happened to match.
/// </summary>
public static class DevelopmentModelProfileFixtures
{
    /// <summary>Deliberately not a plausible real deployment name.</summary>
    public const string SyntheticDeploymentName = "synthetic-development-deployment";

    public const string SyntheticProfileId = "development-fixture";

    private const string FallbackRegion = "development";

    /// <summary>
    /// Builds the fixture set for one region. A configured region the canonical
    /// profile alphabet cannot express falls back to
    /// <c>development</c>, which makes the profile region-blocked rather than
    /// silently region-agnostic.
    /// </summary>
    public static IReadOnlyList<ModelProfile> Build(string region)
    {
        string allowed = IsCanonicalRegion(region) ? region : FallbackRegion;
        return
        [
            new ModelProfile(
                ModelProfile.SchemaVersionValue,
                SyntheticProfileId,
                "Development Fixture (untrusted)",
                "Synthetic development profile. Never published, never signed, never selectable outside Development.",
                "synthetic-fixture-family",
                Priority: 500,
                Enabled: true,
                Visible: true,
                SyntheticDeploymentName,
                new ModelProfileExpectedModel(
                    "SyntheticPublisher",
                    "synthetic-fixture-model",
                    "0000-00-00"),
                ["ordinary_chat"],
                ["assistant"],
                ["structured_response"],
                ["sapphirus.bmad-method-help-proposal.v1"],
                ["text"],
                new ModelProfileCapabilities(
                    TextInput: true,
                    ImageInput: false,
                    PdfInput: false,
                    Attachments: false,
                    ToolCalling: false,
                    StructuredOutput: true,
                    Reasoning: false,
                    Streaming: false),
                new ModelProfileLimits(
                    ContextTokens: 8_000,
                    MaxInputTokens: 4_000,
                    MaxOutputTokens: 2_000,
                    MaxAttachmentBytes: 0,
                    MaxOutputBytes: 65_536),
                [
                    new ModelProfileVariant(
                        "fixture",
                        "Fixture",
                        "none",
                        "low",
                        MaxOutputTokens: 2_000),
                ],
                new ModelProfileRetryPolicy(1, 0, 0),
                ModelProfile.SupportedRetentionMode,
                [allowed],
                new ModelProfileQualification(
                    ModelQualificationStatus.Qualified,
                    ModelProfileValidator.Digest(
                        "model-profile-development-fixture-evidence",
                        SyntheticProfileId),
                    DateTimeOffset.UnixEpoch),
                new ModelPriceProfile(
                    "development-fixture-price",
                    "EUR",
                    0,
                    0,
                    0,
                    0)),
        ];
    }

    private static bool IsCanonicalRegion(string? region)
    {
        if (region is null || region.Length is < 1 or > 64)
        {
            return false;
        }
        if (region[0] is not (>= 'a' and <= 'z'))
        {
            return false;
        }
        foreach (char character in region)
        {
            if (character is not ((>= 'a' and <= 'z') or (>= '0' and <= '9')))
            {
                return false;
            }
        }
        return true;
    }
}
