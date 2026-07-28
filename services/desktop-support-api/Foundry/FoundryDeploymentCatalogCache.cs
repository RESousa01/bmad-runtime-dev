using Microsoft.Extensions.Hosting;

namespace Sapphirus.DesktopSupportApi.Foundry;

/// <summary>Public catalog states. These four strings are the whole vocabulary.</summary>
public enum FoundryCatalogState
{
    Fresh,
    StaleUsable,
    Unavailable,
    Invalid,
}

public enum FoundryCatalogRefreshOutcome
{
    NeverRefreshed,
    Succeeded,
    TransientFailure,
    HardFailure,
    InvalidSnapshot,
}

/// <summary>
/// Immutable catalog storage. One reference is swapped atomically, so a reader
/// never observes a half-updated catalog and a failed refresh can record its
/// outcome without disturbing the snapshot it could not replace.
/// </summary>
internal sealed record FoundryCatalogEntry(
    FoundryDeploymentSnapshot Snapshot,
    DateTimeOffset GeneratedAt,
    DateTimeOffset ExpiresAt,
    DateTimeOffset MaximumStaleAt,
    string SnapshotHash,
    FoundryCatalogRefreshOutcome LastRefreshOutcome);

/// <summary>
/// Public catalog status. It carries timings, a snapshot hash, and an outcome
/// — never a deployment name, an endpoint, or a provider message.
/// </summary>
public sealed record FoundryCatalogStatus(
    FoundryCatalogState State,
    FoundryCatalogRefreshOutcome LastRefreshOutcome,
    DateTimeOffset? GeneratedAt,
    DateTimeOffset? ExpiresAt,
    DateTimeOffset? MaximumStaleAt,
    string SnapshotHash)
{
    /// <summary>The exact wire vocabulary for public diagnostics.</summary>
    public string StateName => FoundryCatalogStateNames.ToWireName(State);
}

public static class FoundryCatalogStateNames
{
    public const string Fresh = "fresh";
    public const string StaleUsable = "stale_usable";
    public const string Unavailable = "unavailable";
    public const string Invalid = "invalid";

    public static string ToWireName(FoundryCatalogState state) => state switch
    {
        FoundryCatalogState.Fresh => Fresh,
        FoundryCatalogState.StaleUsable => StaleUsable,
        FoundryCatalogState.Invalid => Invalid,
        _ => Unavailable,
    };
}

/// <summary>
/// A catalog read. The snapshot is present only when it may actually be used:
/// a fresh catalog, or a last-known-valid one inside its staleness budget.
/// </summary>
public sealed record FoundryCatalogRead(
    FoundryCatalogState State,
    FoundryDeploymentSnapshot? Snapshot,
    string ErrorCode)
{
    public bool IsUsable => Snapshot is not null
        && State is FoundryCatalogState.Fresh or FoundryCatalogState.StaleUsable;
}

/// <summary>
/// Caches the Foundry deployment inventory.
///
/// A fresh snapshot is returned without touching the network. When it has
/// expired, exactly one caller refreshes: the rest wait on a single gate and
/// re-check freshness under it, so a burst of readers produces one provider
/// call rather than a stampede.
///
/// Only a transient discovery failure may keep serving the last known valid
/// snapshot, and only until its maximum-stale instant, which no failure ever
/// extends. An authorization, project-configuration, or pagination-integrity
/// failure stops service immediately. A refreshed snapshot that does not
/// normalize never replaces the one that did.
/// </summary>
public sealed class FoundryDeploymentCatalogCache : IDisposable
{
    public static readonly TimeSpan DefaultRefreshInterval = TimeSpan.FromMinutes(5);
    public static readonly TimeSpan DefaultMaximumStale = TimeSpan.FromMinutes(30);

    private readonly IFoundryDeploymentDiscovery _discovery;
    private readonly TimeProvider _timeProvider;
    private readonly TimeSpan _refreshInterval;
    private readonly TimeSpan _maximumStale;
    private readonly SemaphoreSlim _gate = new(1, 1);

    private FoundryCatalogEntry? _entry;
    private int _bootstrapOutcome = (int)FoundryCatalogRefreshOutcome.NeverRefreshed;
    private string _lastErrorCode = "";
    private int _refreshCount;

    public FoundryDeploymentCatalogCache(
        IFoundryDeploymentDiscovery discovery,
        TimeProvider timeProvider)
        : this(discovery, timeProvider, DefaultRefreshInterval, DefaultMaximumStale)
    {
    }

    public FoundryDeploymentCatalogCache(
        IFoundryDeploymentDiscovery discovery,
        TimeProvider timeProvider,
        TimeSpan refreshInterval,
        TimeSpan maximumStale)
    {
        ArgumentNullException.ThrowIfNull(discovery);
        ArgumentNullException.ThrowIfNull(timeProvider);
        if (refreshInterval <= TimeSpan.Zero || maximumStale < refreshInterval)
        {
            throw new ArgumentOutOfRangeException(
                nameof(maximumStale),
                "The maximum staleness budget must cover at least one refresh interval.");
        }
        _discovery = discovery;
        _timeProvider = timeProvider;
        _refreshInterval = refreshInterval;
        _maximumStale = maximumStale;
    }

    /// <summary>Provider calls made so far. Test and diagnostic use only.</summary>
    internal int RefreshCount => Volatile.Read(ref _refreshCount);

    public async ValueTask<FoundryCatalogRead> GetAsync(CancellationToken cancellationToken)
    {
        FoundryCatalogEntry? entry = Volatile.Read(ref _entry);
        if (IsServableWithoutRefresh(entry, _timeProvider.GetUtcNow()))
        {
            return new FoundryCatalogRead(
                FoundryCatalogState.Fresh,
                entry!.Snapshot,
                "");
        }

        await _gate.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            // Re-check under the gate: while this caller queued, another may
            // already have refreshed, and a second provider call would be pure
            // stampede.
            entry = Volatile.Read(ref _entry);
            if (IsServableWithoutRefresh(entry, _timeProvider.GetUtcNow()))
            {
                return new FoundryCatalogRead(
                    FoundryCatalogState.Fresh,
                    entry!.Snapshot,
                    "");
            }
            return await RefreshUnderGateAsync(entry, cancellationToken)
                .ConfigureAwait(false);
        }
        finally
        {
            _gate.Release();
        }
    }

    /// <summary>
    /// Warms the catalog at startup. It never throws on a discovery failure:
    /// a cold provider must not stop the host from coming up, it must show up
    /// as an unhealthy catalog.
    /// </summary>
    public async Task WarmAsync(CancellationToken cancellationToken)
    {
        try
        {
            _ = await GetAsync(cancellationToken).ConfigureAwait(false);
        }
        catch (OperationCanceledException)
        {
            throw;
        }
        catch (Exception)
        {
            // Already recorded as a failed refresh outcome.
        }
    }

    /// <summary>
    /// Current state without triggering a refresh. Health probes read this:
    /// a probe that could start a provider call would be its own stampede.
    /// </summary>
    public FoundryCatalogStatus Peek()
    {
        DateTimeOffset now = _timeProvider.GetUtcNow();
        FoundryCatalogEntry? entry = Volatile.Read(ref _entry);
        if (entry is null)
        {
            FoundryCatalogRefreshOutcome bootstrap =
                (FoundryCatalogRefreshOutcome)Volatile.Read(ref _bootstrapOutcome);
            return new FoundryCatalogStatus(
                bootstrap == FoundryCatalogRefreshOutcome.InvalidSnapshot
                    ? FoundryCatalogState.Invalid
                    : FoundryCatalogState.Unavailable,
                bootstrap,
                GeneratedAt: null,
                ExpiresAt: null,
                MaximumStaleAt: null,
                SnapshotHash: "");
        }
        return new FoundryCatalogStatus(
            DeriveState(entry, now),
            entry.LastRefreshOutcome,
            entry.GeneratedAt,
            entry.ExpiresAt,
            entry.MaximumStaleAt,
            entry.SnapshotHash);
    }

    private async ValueTask<FoundryCatalogRead> RefreshUnderGateAsync(
        FoundryCatalogEntry? previous,
        CancellationToken cancellationToken)
    {
        Interlocked.Increment(ref _refreshCount);
        FoundryDiscoveryResult result = await _discovery
            .DiscoverAsync(cancellationToken)
            .ConfigureAwait(false);
        DateTimeOffset completedAt = _timeProvider.GetUtcNow();
        Volatile.Write(ref _lastErrorCode, result.ErrorCode);

        if (result.IsSuccess)
        {
            FoundryCatalogEntry refreshed = new(
                result.Snapshot!,
                completedAt,
                completedAt + _refreshInterval,
                completedAt + _maximumStale,
                result.Snapshot!.SnapshotHash,
                FoundryCatalogRefreshOutcome.Succeeded);
            Volatile.Write(ref _entry, refreshed);
            Volatile.Write(ref _bootstrapOutcome, (int)FoundryCatalogRefreshOutcome.Succeeded);
            return new FoundryCatalogRead(FoundryCatalogState.Fresh, refreshed.Snapshot, "");
        }

        FoundryCatalogRefreshOutcome outcome = result.Outcome switch
        {
            FoundryDiscoveryOutcome.Transient => FoundryCatalogRefreshOutcome.TransientFailure,
            FoundryDiscoveryOutcome.InvalidBody => FoundryCatalogRefreshOutcome.InvalidSnapshot,
            _ => FoundryCatalogRefreshOutcome.HardFailure,
        };
        if (previous is null)
        {
            Volatile.Write(ref _bootstrapOutcome, (int)outcome);
            return new FoundryCatalogRead(
                outcome == FoundryCatalogRefreshOutcome.InvalidSnapshot
                    ? FoundryCatalogState.Invalid
                    : FoundryCatalogState.Unavailable,
                Snapshot: null,
                result.ErrorCode);
        }

        // The snapshot is kept and its expiry instants are copied verbatim:
        // a failed refresh must never buy the catalog more time.
        FoundryCatalogEntry retained = previous with { LastRefreshOutcome = outcome };
        Volatile.Write(ref _entry, retained);
        FoundryCatalogState state = DeriveState(retained, completedAt);
        return new FoundryCatalogRead(
            state,
            state == FoundryCatalogState.StaleUsable ? retained.Snapshot : null,
            result.ErrorCode);
    }

    private static bool IsServableWithoutRefresh(
        FoundryCatalogEntry? entry,
        DateTimeOffset now) =>
        entry is not null
        && entry.LastRefreshOutcome == FoundryCatalogRefreshOutcome.Succeeded
        && now < entry.ExpiresAt;

    private static FoundryCatalogState DeriveState(
        FoundryCatalogEntry entry,
        DateTimeOffset now) => entry.LastRefreshOutcome switch
        {
            FoundryCatalogRefreshOutcome.InvalidSnapshot => FoundryCatalogState.Invalid,
            FoundryCatalogRefreshOutcome.HardFailure => FoundryCatalogState.Unavailable,
            _ when now < entry.ExpiresAt => FoundryCatalogState.Fresh,
            _ when now < entry.MaximumStaleAt => FoundryCatalogState.StaleUsable,
            _ => FoundryCatalogState.Unavailable,
        };

    internal string LastErrorCode => Volatile.Read(ref _lastErrorCode);

    public void Dispose() => _gate.Dispose();
}

/// <summary>
/// Warms the catalog at startup and refreshes it on a jittered interval so a
/// fleet of replicas does not synchronize onto the provider. It stops on the
/// host's shutdown token and never lets a refresh failure escape.
/// </summary>
public sealed class FoundryCatalogRefreshService(
    FoundryDeploymentCatalogCache cache,
    TimeProvider timeProvider,
    TimeSpan refreshInterval) : BackgroundService
{
    /// <summary>Jitter never shortens an interval; it only spreads it out.</summary>
    public const double MaximumJitterFraction = 0.2;

    internal static TimeSpan ComputeRefreshDelay(
        TimeSpan refreshInterval,
        double jitterSample)
    {
        double clamped = Math.Clamp(jitterSample, 0.0, 1.0);
        return refreshInterval
            + (refreshInterval * MaximumJitterFraction * clamped);
    }

    protected override async Task ExecuteAsync(CancellationToken stoppingToken)
    {
        while (!stoppingToken.IsCancellationRequested)
        {
            try
            {
                await cache.WarmAsync(stoppingToken).ConfigureAwait(false);
                await Task.Delay(
                        ComputeRefreshDelay(refreshInterval, Random.Shared.NextDouble()),
                        timeProvider,
                        stoppingToken)
                    .ConfigureAwait(false);
            }
            catch (OperationCanceledException)
            {
                return;
            }
        }
    }
}
