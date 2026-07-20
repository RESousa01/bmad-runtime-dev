using System.Text.Json;
using Microsoft.Data.SqlClient;
using Microsoft.Extensions.Diagnostics.HealthChecks;
using Sapphirus.DesktopSupportApi.Policy;
using Sapphirus.DesktopSupportApi.Sql;

namespace Sapphirus.DesktopSupportApi.Health;

/// <summary>
/// Dependency health checks that disclose only a status per safe dependency
/// class. No endpoint, tenant, database name, or exception text ever
/// appears in a health response.
/// </summary>
public static class AzureDependencyHealthChecks
{
    public const string SqlDependency = "sql";
    public const string ConfigurationDependency = "configuration";
    public const string SigningDependency = "signing";
    public const string ModelDependency = "model";

    /// <summary>
    /// Renders a health report as status plus safe dependency classes only.
    /// </summary>
    public static Task WriteSafeResponseAsync(HttpContext context, HealthReport report)
    {
        var body = new
        {
            status = report.Status.ToString().ToLowerInvariant(),
            dependencies = report.Entries
                .OrderBy(static entry => entry.Key, StringComparer.Ordinal)
                .Select(static entry => new
                {
                    dependency = entry.Key,
                    status = entry.Value.Status.ToString().ToLowerInvariant(),
                })
                .ToArray(),
        };
        context.Response.ContentType = "application/json";
        return context.Response.WriteAsync(JsonSerializer.Serialize(body));
    }
}

/// <summary>Round-trips one bounded query on the authority database.</summary>
public sealed class SqlAuthorityHealthCheck(SqlConnectionFactory connectionFactory)
    : IHealthCheck
{
    public async Task<HealthCheckResult> CheckHealthAsync(
        HealthCheckContext context,
        CancellationToken cancellationToken = default)
    {
        try
        {
            await using SqlConnection connection = await connectionFactory
                .OpenAsync(cancellationToken)
                .ConfigureAwait(false);
            await using SqlCommand probe = connection.CreateCommand();
            probe.CommandTimeout = 5;
            probe.CommandText = "SELECT 1;";
            _ = await probe.ExecuteScalarAsync(cancellationToken).ConfigureAwait(false);
            return HealthCheckResult.Healthy();
        }
        catch (OperationCanceledException)
        {
            throw;
        }
        catch
        {
            // Never propagate dependency exception details into responses.
            return HealthCheckResult.Unhealthy();
        }
    }
}

/// <summary>Reports policy snapshot availability and freshness.</summary>
public sealed class PolicyConfigurationHealthCheck(
    AppConfigurationPolicyProvider policyProvider) : IHealthCheck
{
    public async Task<HealthCheckResult> CheckHealthAsync(
        HealthCheckContext context,
        CancellationToken cancellationToken = default)
    {
        try
        {
            _ = await policyProvider
                .GetSnapshotAsync(cancellationToken)
                .ConfigureAwait(false);
            return HealthCheckResult.Healthy();
        }
        catch (OperationCanceledException)
        {
            throw;
        }
        catch
        {
            return HealthCheckResult.Unhealthy();
        }
    }
}

/// <summary>
/// Reports signing readiness from configuration state only. Probing Key
/// Vault with a real signature on every health poll would burn quota and
/// audit noise, so this check verifies the signer is composed; signing
/// failures surface through the receipt-signing failure alert instead.
/// </summary>
public sealed class SigningCompositionHealthCheck(IServiceProvider services) : IHealthCheck
{
    public Task<HealthCheckResult> CheckHealthAsync(
        HealthCheckContext context,
        CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        return Task.FromResult(
            services.GetService<Signing.IHashSigner>() is not null
                ? HealthCheckResult.Healthy()
                : HealthCheckResult.Unhealthy());
    }
}

/// <summary>
/// Reports model brokerage readiness from composition state only; live
/// provider probes are reserved for the deployed smoke gate.
/// </summary>
public sealed class ModelCompositionHealthCheck(IServiceProvider services) : IHealthCheck
{
    public Task<HealthCheckResult> CheckHealthAsync(
        HealthCheckContext context,
        CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        return Task.FromResult(
            services.GetService<IModelAccessBroker>() is not null
                ? HealthCheckResult.Healthy()
                : HealthCheckResult.Degraded());
    }
}
