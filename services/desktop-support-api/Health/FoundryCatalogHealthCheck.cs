using Microsoft.Extensions.Diagnostics.HealthChecks;
using Sapphirus.DesktopSupportApi.Foundry;

namespace Sapphirus.DesktopSupportApi.Health;

/// <summary>
/// Reports Foundry deployment-catalog readiness.
///
/// The probe reads cached state only; it never triggers a refresh, so polling
/// health can never become its own provider stampede. The result carries a
/// status and nothing else: no deployment name, snapshot hash, project
/// endpoint, or provider message reaches a health response.
/// <see cref="AzureDependencyHealthChecks.WriteSafeResponseAsync"/> emits only
/// <c>{status, dependencies:[{dependency,status}]}</c>, and this check is
/// written to stay safe even if that renderer ever widened.
/// </summary>
public sealed class FoundryCatalogHealthCheck(FoundryDeploymentCatalogCache cache)
    : IHealthCheck
{
    /// <summary>
    /// The dependency class name. It lives here rather than in
    /// <see cref="AzureDependencyHealthChecks"/> because the Foundry catalog
    /// is not an Azure client dependency of the same shape.
    /// </summary>
    public const string FoundryCatalogDependency = "foundry_catalog";

    public Task<HealthCheckResult> CheckHealthAsync(
        HealthCheckContext context,
        CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        return Task.FromResult(ToHealthResult(cache.Peek().State));
    }

    /// <summary>
    /// A usable-but-aging catalog degrades rather than fails: the plane can
    /// still serve. An unavailable or unrepresentable catalog is unhealthy.
    /// </summary>
    internal static HealthCheckResult ToHealthResult(FoundryCatalogState state) =>
        state switch
        {
            FoundryCatalogState.Fresh => HealthCheckResult.Healthy(),
            FoundryCatalogState.StaleUsable => HealthCheckResult.Degraded(),
            _ => HealthCheckResult.Unhealthy(),
        };
}
