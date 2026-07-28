using System.Text;
using System.Text.Json;
using Microsoft.AspNetCore.Http;
using Microsoft.Extensions.Diagnostics.HealthChecks;
using Sapphirus.DesktopSupportApi.Foundry;
using Sapphirus.DesktopSupportApi.Health;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Foundry;

/// <summary>
/// FND-023. Time is injected everywhere here: not one case waits on the wall
/// clock, and every staleness boundary is crossed by moving the provider.
/// </summary>
public sealed class FoundryDeploymentCatalogCacheTests
{
    private static readonly DateTimeOffset Start =
        DateTimeOffset.Parse("2026-07-28T12:00:00Z");
    private static readonly TimeSpan Refresh = TimeSpan.FromMinutes(5);
    private static readonly TimeSpan MaximumStale = TimeSpan.FromMinutes(30);

    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    [Fact]
    public void Catalog_defaults_are_five_minute_refresh_and_thirty_minute_maximum_stale()
    {
        Assert.Equal(
            TimeSpan.FromMinutes(5),
            FoundryDeploymentCatalogCache.DefaultRefreshInterval);
        Assert.Equal(
            TimeSpan.FromMinutes(30),
            FoundryDeploymentCatalogCache.DefaultMaximumStale);
    }

    [Fact]
    public async Task Catalog_returns_a_fresh_snapshot_without_calling_the_provider_again()
    {
        ScriptedDiscovery discovery = new();
        discovery.EnqueueSuccess(Snapshot("alpha"));
        ControlledTimeProvider time = new(Start);
        using FoundryDeploymentCatalogCache cache = new(discovery, time, Refresh, MaximumStale);

        FoundryCatalogRead first = await cache.GetAsync(Ct);
        time.Advance(TimeSpan.FromMinutes(4));
        FoundryCatalogRead second = await cache.GetAsync(Ct);

        Assert.Equal(FoundryCatalogState.Fresh, first.State);
        Assert.Equal(FoundryCatalogState.Fresh, second.State);
        Assert.Same(first.Snapshot, second.Snapshot);
        Assert.Equal(1, discovery.CallCount);
        Assert.Equal(1, cache.RefreshCount);
    }

    [Fact]
    public async Task Catalog_stores_generated_expiry_maximum_stale_hash_and_last_outcome()
    {
        ScriptedDiscovery discovery = new();
        discovery.EnqueueSuccess(Snapshot("alpha"));
        ControlledTimeProvider time = new(Start);
        using FoundryDeploymentCatalogCache cache = new(discovery, time, Refresh, MaximumStale);

        _ = await cache.GetAsync(Ct);
        FoundryCatalogStatus status = cache.Peek();

        Assert.Equal(FoundryCatalogState.Fresh, status.State);
        Assert.Equal(FoundryCatalogRefreshOutcome.Succeeded, status.LastRefreshOutcome);
        Assert.Equal(Start, status.GeneratedAt);
        Assert.Equal(Start + Refresh, status.ExpiresAt);
        Assert.Equal(Start + MaximumStale, status.MaximumStaleAt);
        Assert.Equal(HashFor("alpha"), status.SnapshotHash);
    }

    [Fact]
    public async Task Concurrent_readers_cause_exactly_one_refresh()
    {
        ScriptedDiscovery discovery = new();
        discovery.EnqueueSuccess(Snapshot("alpha"));
        discovery.BlockUntilReleased();
        ControlledTimeProvider time = new(Start);
        using FoundryDeploymentCatalogCache cache = new(discovery, time, Refresh, MaximumStale);

        Task<FoundryCatalogRead> leader = cache.GetAsync(Ct).AsTask();
        await discovery.WaitUntilInsideAsync(Ct);
        Task<FoundryCatalogRead>[] followers =
        [
            cache.GetAsync(Ct).AsTask(),
            cache.GetAsync(Ct).AsTask(),
            cache.GetAsync(Ct).AsTask(),
            cache.GetAsync(Ct).AsTask(),
        ];
        discovery.Release();
        FoundryCatalogRead[] reads = await Task.WhenAll([leader, .. followers]);

        Assert.Equal(1, discovery.CallCount);
        Assert.All(reads, read => Assert.Equal(FoundryCatalogState.Fresh, read.State));
        Assert.All(reads, read => Assert.Same(reads[0].Snapshot, read.Snapshot));
    }

    [Fact]
    public async Task Catalog_serves_the_last_known_valid_snapshot_through_a_transient_failure()
    {
        ScriptedDiscovery discovery = new();
        discovery.EnqueueSuccess(Snapshot("alpha"));
        discovery.EnqueueFailure(FoundryDiscoveryOutcome.Transient, "foundry_discovery_throttled");
        ControlledTimeProvider time = new(Start);
        using FoundryDeploymentCatalogCache cache = new(discovery, time, Refresh, MaximumStale);

        FoundryCatalogRead fresh = await cache.GetAsync(Ct);
        time.Advance(TimeSpan.FromMinutes(6));
        FoundryCatalogRead stale = await cache.GetAsync(Ct);

        Assert.Equal(FoundryCatalogState.Fresh, fresh.State);
        Assert.Equal(FoundryCatalogState.StaleUsable, stale.State);
        Assert.True(stale.IsUsable);
        Assert.Same(fresh.Snapshot, stale.Snapshot);
        Assert.Equal("foundry_discovery_throttled", stale.ErrorCode);
        Assert.Equal(FoundryCatalogStateNames.StaleUsable, cache.Peek().StateName);
    }

    [Fact]
    public async Task A_transient_failure_never_extends_the_maximum_stale_instant()
    {
        ScriptedDiscovery discovery = new();
        discovery.EnqueueSuccess(Snapshot("alpha"));
        discovery.EnqueueFailure(FoundryDiscoveryOutcome.Transient, "foundry_discovery_throttled");
        discovery.EnqueueFailure(FoundryDiscoveryOutcome.Transient, "foundry_discovery_throttled");
        ControlledTimeProvider time = new(Start);
        using FoundryDeploymentCatalogCache cache = new(discovery, time, Refresh, MaximumStale);

        _ = await cache.GetAsync(Ct);
        time.Advance(TimeSpan.FromMinutes(6));
        _ = await cache.GetAsync(Ct);
        DateTimeOffset? afterFirstFailure = cache.Peek().MaximumStaleAt;
        time.Advance(TimeSpan.FromMinutes(10));
        _ = await cache.GetAsync(Ct);

        Assert.Equal(Start + MaximumStale, afterFirstFailure);
        Assert.Equal(Start + MaximumStale, cache.Peek().MaximumStaleAt);
    }

    [Fact]
    public async Task Catalog_stops_serving_once_the_maximum_stale_instant_passes()
    {
        ScriptedDiscovery discovery = new();
        discovery.EnqueueSuccess(Snapshot("alpha"));
        discovery.EnqueueFailure(FoundryDiscoveryOutcome.Transient, "foundry_discovery_throttled");
        ControlledTimeProvider time = new(Start);
        using FoundryDeploymentCatalogCache cache = new(discovery, time, Refresh, MaximumStale);

        _ = await cache.GetAsync(Ct);
        time.Advance(TimeSpan.FromMinutes(31));
        FoundryCatalogRead expired = await cache.GetAsync(Ct);

        Assert.Equal(FoundryCatalogState.Unavailable, expired.State);
        Assert.Null(expired.Snapshot);
        Assert.False(expired.IsUsable);
        Assert.Equal(FoundryCatalogStateNames.Unavailable, cache.Peek().StateName);
    }

    [Theory]
    [InlineData(FoundryDiscoveryOutcome.Authorization, "foundry_discovery_unauthorized")]
    [InlineData(FoundryDiscoveryOutcome.ProjectConfiguration, "foundry_discovery_project_not_found")]
    [InlineData(FoundryDiscoveryOutcome.PaginationIntegrity, "foundry_pagination_origin_invalid")]
    public async Task A_hard_failure_stops_service_immediately_and_never_serves_a_stale_snapshot(
        FoundryDiscoveryOutcome outcome,
        string errorCode)
    {
        ScriptedDiscovery discovery = new();
        discovery.EnqueueSuccess(Snapshot("alpha"));
        discovery.EnqueueFailure(outcome, errorCode);
        ControlledTimeProvider time = new(Start);
        using FoundryDeploymentCatalogCache cache = new(discovery, time, Refresh, MaximumStale);

        _ = await cache.GetAsync(Ct);
        // Well inside the staleness budget: only the failure class matters.
        time.Advance(TimeSpan.FromMinutes(6));
        FoundryCatalogRead read = await cache.GetAsync(Ct);

        Assert.Equal(FoundryCatalogState.Unavailable, read.State);
        Assert.Null(read.Snapshot);
        Assert.Equal(errorCode, read.ErrorCode);
        Assert.Equal(Start + MaximumStale, cache.Peek().MaximumStaleAt);
    }

    [Fact]
    public async Task An_invalid_refreshed_snapshot_never_replaces_the_last_valid_one()
    {
        ScriptedDiscovery discovery = new();
        discovery.EnqueueSuccess(Snapshot("alpha"));
        discovery.EnqueueFailure(
            FoundryDiscoveryOutcome.InvalidBody,
            "foundry_deployment_duplicate_conflict");
        discovery.EnqueueSuccess(Snapshot("beta"));
        ControlledTimeProvider time = new(Start);
        using FoundryDeploymentCatalogCache cache = new(discovery, time, Refresh, MaximumStale);

        _ = await cache.GetAsync(Ct);
        time.Advance(TimeSpan.FromMinutes(6));
        FoundryCatalogRead invalid = await cache.GetAsync(Ct);
        FoundryCatalogStatus afterInvalid = cache.Peek();
        time.Advance(TimeSpan.FromMinutes(1));
        FoundryCatalogRead recovered = await cache.GetAsync(Ct);

        Assert.Equal(FoundryCatalogState.Invalid, invalid.State);
        Assert.Null(invalid.Snapshot);
        Assert.Equal(FoundryCatalogStateNames.Invalid, afterInvalid.StateName);
        // The stored snapshot is still the one that normalized.
        Assert.Equal(HashFor("alpha"), afterInvalid.SnapshotHash);
        Assert.Equal(FoundryCatalogState.Fresh, recovered.State);
        Assert.Equal(HashFor("beta"), cache.Peek().SnapshotHash);
    }

    [Fact]
    public async Task A_removed_deployment_produces_a_new_snapshot_and_a_new_hash()
    {
        ScriptedDiscovery discovery = new();
        discovery.EnqueueSuccess(Snapshot("alpha", "desktop-coding", "desktop-planning"));
        discovery.EnqueueSuccess(Snapshot("beta", "desktop-planning"));
        ControlledTimeProvider time = new(Start);
        using FoundryDeploymentCatalogCache cache = new(discovery, time, Refresh, MaximumStale);

        FoundryCatalogRead before = await cache.GetAsync(Ct);
        time.Advance(TimeSpan.FromMinutes(6));
        FoundryCatalogRead after = await cache.GetAsync(Ct);

        Assert.Equal(2, before.Snapshot?.Deployments.Count);
        Assert.Equal(1, after.Snapshot?.Deployments.Count);
        Assert.NotEqual(before.Snapshot?.SnapshotHash, after.Snapshot?.SnapshotHash);
        Assert.Equal(FoundryCatalogState.Fresh, after.State);
    }

    [Fact]
    public async Task A_cold_catalog_reports_unavailable_before_the_first_successful_refresh()
    {
        ScriptedDiscovery discovery = new();
        discovery.EnqueueFailure(FoundryDiscoveryOutcome.Transient, "foundry_discovery_unreachable");
        ControlledTimeProvider time = new(Start);
        using FoundryDeploymentCatalogCache cache = new(discovery, time, Refresh, MaximumStale);

        FoundryCatalogStatus cold = cache.Peek();
        FoundryCatalogRead read = await cache.GetAsync(Ct);

        Assert.Equal(FoundryCatalogState.Unavailable, cold.State);
        Assert.Equal(FoundryCatalogRefreshOutcome.NeverRefreshed, cold.LastRefreshOutcome);
        Assert.Null(cold.GeneratedAt);
        Assert.Equal("", cold.SnapshotHash);
        Assert.Equal(FoundryCatalogState.Unavailable, read.State);
    }

    [Fact]
    public async Task A_cold_catalog_reports_invalid_when_the_very_first_snapshot_is_unusable()
    {
        ScriptedDiscovery discovery = new();
        discovery.EnqueueFailure(
            FoundryDiscoveryOutcome.InvalidBody,
            "foundry_discovery_body_malformed");
        ControlledTimeProvider time = new(Start);
        using FoundryDeploymentCatalogCache cache = new(discovery, time, Refresh, MaximumStale);

        FoundryCatalogRead read = await cache.GetAsync(Ct);

        Assert.Equal(FoundryCatalogState.Invalid, read.State);
        Assert.Equal(FoundryCatalogStateNames.Invalid, cache.Peek().StateName);
    }

    [Fact]
    public async Task Catalog_reads_observe_cancellation()
    {
        ScriptedDiscovery discovery = new();
        discovery.EnqueueSuccess(Snapshot("alpha"));
        ControlledTimeProvider time = new(Start);
        using FoundryDeploymentCatalogCache cache = new(discovery, time, Refresh, MaximumStale);
        using CancellationTokenSource cancellation = new();
        await cancellation.CancelAsync();

        await Assert.ThrowsAnyAsync<OperationCanceledException>(() =>
            cache.GetAsync(cancellation.Token).AsTask());
        Assert.Equal(0, discovery.CallCount);
    }

    [Fact]
    public async Task Warming_never_throws_when_the_provider_is_cold()
    {
        ScriptedDiscovery discovery = new();
        discovery.EnqueueThrow(new InvalidOperationException("provider detail must not escape"));
        ControlledTimeProvider time = new(Start);
        using FoundryDeploymentCatalogCache cache = new(discovery, time, Refresh, MaximumStale);

        await cache.WarmAsync(Ct);

        Assert.Equal(FoundryCatalogState.Unavailable, cache.Peek().State);
    }

    [Fact]
    public async Task The_refresh_service_warms_at_startup_and_stops_cleanly_on_shutdown()
    {
        ScriptedDiscovery discovery = new();
        discovery.EnqueueSuccess(Snapshot("alpha"));
        ControlledTimeProvider time = new(Start);
        using FoundryDeploymentCatalogCache cache = new(discovery, time, Refresh, MaximumStale);
        using FoundryCatalogRefreshService service = new(cache, time, Refresh);

        await service.StartAsync(Ct);
        await discovery.WaitUntilCallCountAsync(1, Ct);
        await service.StopAsync(Ct);

        Assert.Equal(1, discovery.CallCount);
        Assert.Equal(FoundryCatalogState.Fresh, cache.Peek().State);
        Assert.NotNull(service.ExecuteTask);
        Assert.True(service.ExecuteTask.IsCompleted);
        Assert.False(service.ExecuteTask.IsFaulted);
    }

    [Theory]
    [InlineData(0.0, 300.0)]
    [InlineData(0.5, 330.0)]
    [InlineData(1.0, 360.0)]
    [InlineData(-1.0, 300.0)]
    [InlineData(7.5, 360.0)]
    public void Refresh_jitter_only_spreads_an_interval_and_never_shortens_it(
        double jitterSample,
        double expectedSeconds)
    {
        TimeSpan delay = FoundryCatalogRefreshService.ComputeRefreshDelay(
            TimeSpan.FromMinutes(5),
            jitterSample);

        Assert.Equal(expectedSeconds, delay.TotalSeconds, precision: 6);
        Assert.True(delay >= TimeSpan.FromMinutes(5));
        Assert.True(delay <= TimeSpan.FromMinutes(6));
    }

    [Theory]
    [InlineData(FoundryCatalogState.Fresh, HealthStatus.Healthy)]
    [InlineData(FoundryCatalogState.StaleUsable, HealthStatus.Degraded)]
    [InlineData(FoundryCatalogState.Unavailable, HealthStatus.Unhealthy)]
    [InlineData(FoundryCatalogState.Invalid, HealthStatus.Unhealthy)]
    public void The_catalog_health_check_maps_every_state_to_a_status(
        FoundryCatalogState state,
        HealthStatus expected)
    {
        Assert.Equal(expected, FoundryCatalogHealthCheck.ToHealthResult(state).Status);
    }

    [Fact]
    public void The_catalog_state_vocabulary_is_exactly_four_names()
    {
        Assert.Equal(
            ["fresh", "invalid", "stale_usable", "unavailable"],
            Enum.GetValues<FoundryCatalogState>()
                .Select(FoundryCatalogStateNames.ToWireName)
                .Order(StringComparer.Ordinal));
    }

    [Fact]
    public async Task The_catalog_health_check_never_reports_a_deployment_name_or_endpoint()
    {
        ScriptedDiscovery discovery = new();
        discovery.EnqueueSuccess(Snapshot("alpha", "desktop-planning"));
        ControlledTimeProvider time = new(Start);
        using FoundryDeploymentCatalogCache cache = new(discovery, time, Refresh, MaximumStale);
        _ = await cache.GetAsync(Ct);
        FoundryCatalogHealthCheck check = new(cache);

        HealthCheckResult result = await check.CheckHealthAsync(
            new HealthCheckContext(),
            Ct);
        string body = await RenderHealthResponseAsync(result);

        Assert.Equal(HealthStatus.Healthy, result.Status);
        Assert.Null(result.Description);
        Assert.Empty(result.Data);
        Assert.Null(result.Exception);
        Assert.Equal(
            """{"status":"healthy","dependencies":[{"dependency":"foundry_catalog","status":"healthy"}]}""",
            body);
        Assert.DoesNotContain("desktop-planning", body, StringComparison.Ordinal);
        Assert.DoesNotContain("sha256", body, StringComparison.Ordinal);
        Assert.DoesNotContain("services.ai.azure.com", body, StringComparison.Ordinal);
    }

    [Fact]
    public async Task The_catalog_health_check_probes_without_starting_a_refresh()
    {
        ScriptedDiscovery discovery = new();
        ControlledTimeProvider time = new(Start);
        using FoundryDeploymentCatalogCache cache = new(discovery, time, Refresh, MaximumStale);
        FoundryCatalogHealthCheck check = new(cache);

        HealthCheckResult result = await check.CheckHealthAsync(
            new HealthCheckContext(),
            Ct);

        Assert.Equal(HealthStatus.Unhealthy, result.Status);
        Assert.Equal(0, discovery.CallCount);
    }

    private static async Task<string> RenderHealthResponseAsync(HealthCheckResult result)
    {
        DefaultHttpContext context = new();
        using MemoryStream body = new();
        context.Response.Body = body;
        HealthReport report = new(
            new Dictionary<string, HealthReportEntry>(StringComparer.Ordinal)
            {
                [FoundryCatalogHealthCheck.FoundryCatalogDependency] = new(
                    result.Status,
                    result.Description,
                    TimeSpan.Zero,
                    result.Exception,
                    result.Data),
            },
            TimeSpan.Zero);

        await AzureDependencyHealthChecks.WriteSafeResponseAsync(context, report);

        return Encoding.UTF8.GetString(body.ToArray());
    }

    private static string HashFor(string label) =>
        "sha256:" + label.GetHashCode(StringComparison.Ordinal)
            .ToString("x8", System.Globalization.CultureInfo.InvariantCulture)
            .PadLeft(64, '0');

    private static FoundryDeploymentSnapshot Snapshot(
        string label,
        params string[] deploymentNames) =>
        new(
            FoundryDeploymentSnapshot.SchemaVersionValue,
            HashFor(label),
            Start,
            [.. deploymentNames.Select(static name => new FoundryDeployment(
                name,
                FoundryDeployment.ModelDeploymentType,
                "OpenAI",
                "gpt-5-mini",
                "2026-06-01",
                new FoundryDeploymentSku("GlobalStandard", "Global", "OpenAI", "S", 10),
                FoundryDeploymentCapabilities.None,
                HashFor(name)))]);

    private sealed class ControlledTimeProvider(DateTimeOffset start) : TimeProvider
    {
        private readonly Lock _gate = new();
        private DateTimeOffset _now = start;

        public override DateTimeOffset GetUtcNow()
        {
            lock (_gate)
            {
                return _now;
            }
        }

        public void Advance(TimeSpan duration)
        {
            lock (_gate)
            {
                _now = _now.Add(duration);
            }
        }
    }

    private sealed class ScriptedDiscovery : IFoundryDeploymentDiscovery
    {
        private readonly Lock _gate = new();
        private readonly Queue<Func<FoundryDiscoveryResult>> _script = new();
        private readonly TaskCompletionSource _entered =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource _released =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private bool _blocks;
        private int _callCount;

        public int CallCount => Volatile.Read(ref _callCount);

        public void EnqueueSuccess(FoundryDeploymentSnapshot snapshot)
        {
            lock (_gate)
            {
                _script.Enqueue(() => new FoundryDiscoveryResult(
                    FoundryDiscoveryOutcome.Success,
                    "",
                    snapshot,
                    Evidence()));
            }
        }

        public void EnqueueFailure(FoundryDiscoveryOutcome outcome, string errorCode)
        {
            lock (_gate)
            {
                _script.Enqueue(() => new FoundryDiscoveryResult(
                    outcome,
                    errorCode,
                    Snapshot: null,
                    Evidence()));
            }
        }

        public void EnqueueThrow(Exception error)
        {
            lock (_gate)
            {
                _script.Enqueue(() => throw error);
            }
        }

        public void BlockUntilReleased() => _blocks = true;

        public Task WaitUntilInsideAsync(CancellationToken cancellationToken) =>
            _entered.Task.WaitAsync(cancellationToken);

        public async Task WaitUntilCallCountAsync(int expected, CancellationToken cancellationToken)
        {
            while (CallCount < expected)
            {
                cancellationToken.ThrowIfCancellationRequested();
                await Task.Yield();
            }
        }

        public void Release() => _released.TrySetResult();

        public async Task<FoundryDiscoveryResult> DiscoverAsync(
            CancellationToken cancellationToken)
        {
            Interlocked.Increment(ref _callCount);
            _entered.TrySetResult();
            if (_blocks)
            {
                await _released.Task.WaitAsync(cancellationToken).ConfigureAwait(false);
            }
            Func<FoundryDiscoveryResult> next;
            lock (_gate)
            {
                if (!_script.TryDequeue(out next!))
                {
                    next = () => new FoundryDiscoveryResult(
                        FoundryDiscoveryOutcome.Transient,
                        "scripted_discovery_exhausted",
                        Snapshot: null,
                        Evidence());
                }
            }
            return next();
        }

        private static FoundryDiscoveryEvidence Evidence() =>
            new(Start, PageCount: 1, ResponseBytes: 0, [], []);
    }
}
