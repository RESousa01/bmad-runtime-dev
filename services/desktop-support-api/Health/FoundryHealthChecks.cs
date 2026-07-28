using System.Net;
using System.Net.Http.Headers;
using Microsoft.Extensions.Diagnostics.HealthChecks;
using Sapphirus.DesktopSupportApi.Configuration;
using Sapphirus.DesktopSupportApi.Foundry;
using Sapphirus.DesktopSupportApi.Model;

namespace Sapphirus.DesktopSupportApi.Health;

/// <summary>
/// The Foundry readiness dependency classes.
///
/// <para>
/// They are separate checks rather than one aggregate because they fail for
/// different reasons and are fixed by different people: a malformed endpoint is
/// a template change, an unacquirable credential is a role assignment, an empty
/// catalog is a profile publication, and an unreachable Responses surface is a
/// provider incident.
/// </para>
/// <para>
/// Liveness stays process-only and is deliberately not listed here, so a
/// Foundry outage never restarts a healthy process. The non-model routes —
/// bootstrap, device registration, entitlements, policy — do not depend on any
/// of these checks either: readiness reports them, it does not gate them.
/// </para>
/// </summary>
public static class FoundryHealthChecks
{
    public const string ProjectConfigurationDependency = "foundry_project_configuration";
    public const string CredentialDependency = "foundry_credential";
    public const string EffectiveProfilesDependency = "foundry_effective_profiles";
    public const string ResponsesReachabilityDependency = "foundry_responses";

    /// <summary>
    /// The complete set of Foundry readiness dependency names, in the order a
    /// reader should work through them: configuration, then credential, then
    /// catalog, then the provider surface.
    /// </summary>
    public static readonly string[] All =
    [
        ProjectConfigurationDependency,
        CredentialDependency,
        FoundryCatalogHealthCheck.FoundryCatalogDependency,
        EffectiveProfilesDependency,
        ResponsesReachabilityDependency,
    ];
}

/// <summary>
/// Reports whether the project endpoint is configured in its exact canonical
/// form. It performs no I/O, so it stays truthful during a provider outage.
/// </summary>
public sealed class FoundryProjectConfigurationHealthCheck(ProductionOptions options)
    : IHealthCheck
{
    public Task<HealthCheckResult> CheckHealthAsync(
        HealthCheckContext context,
        CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        bool valid = ProductionOptions.TryValidateFoundryProjectEndpoint(
                options.FoundryProjectEndpoint,
                out Uri _)
            && options.FoundryResponsesEndpoint is not null;
        // The endpoint itself is never disclosed, only whether it is usable.
        return Task.FromResult(valid
            ? HealthCheckResult.Healthy()
            : HealthCheckResult.Unhealthy());
    }
}

/// <summary>
/// Reports whether the Foundry credential can be acquired.
///
/// The result is cached for a minimum interval so health polling cannot become
/// its own token-request storm, and no credential, tenant, or exception text
/// ever reaches the response.
/// </summary>
public sealed class FoundryCredentialHealthCheck(
    IFoundryRequestAuthenticator authenticator,
    TimeProvider timeProvider,
    TimeSpan? minimumInterval = null) : IHealthCheck
{
    public static readonly TimeSpan DefaultMinimumInterval = TimeSpan.FromSeconds(30);

    private readonly TimeSpan _minimumInterval = minimumInterval ?? DefaultMinimumInterval;
    private readonly SemaphoreSlim _gate = new(1, 1);
    private DateTimeOffset _checkedAt = DateTimeOffset.MinValue;
    private bool _lastResult;

    public async Task<HealthCheckResult> CheckHealthAsync(
        HealthCheckContext context,
        CancellationToken cancellationToken = default)
    {
        DateTimeOffset now = timeProvider.GetUtcNow();
        await _gate.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            if (_checkedAt != DateTimeOffset.MinValue && now - _checkedAt < _minimumInterval)
            {
                return _lastResult
                    ? HealthCheckResult.Healthy()
                    : HealthCheckResult.Unhealthy();
            }
            try
            {
                _ = await authenticator
                    .BeginOperationAsync(cancellationToken)
                    .ConfigureAwait(false);
                _lastResult = true;
            }
            catch (OperationCanceledException)
            {
                throw;
            }
            catch
            {
                // Credential library text routinely carries tenant, client, and
                // endpoint detail. None of it reaches a health response.
                _lastResult = false;
            }
            _checkedAt = now;
            return _lastResult
                ? HealthCheckResult.Healthy()
                : HealthCheckResult.Unhealthy();
        }
        finally
        {
            _gate.Release();
        }
    }
}

/// <summary>
/// Reports whether the organization published at least one usable profile.
///
/// A plane with a reachable project, a valid credential, and no profile can
/// serve nobody, and reporting that as healthy would send an operator hunting
/// through Azure instead of through their profile configuration.
/// </summary>
public sealed class FoundryEffectiveProfilesHealthCheck(
    IModelProfileRegistry registry) : IHealthCheck
{
    public async Task<HealthCheckResult> CheckHealthAsync(
        HealthCheckContext context,
        CancellationToken cancellationToken = default)
    {
        try
        {
            ModelProfileRegistrySnapshot snapshot = await registry
                .GetSnapshotAsync(cancellationToken)
                .ConfigureAwait(false);
            if (!snapshot.IsUsable)
            {
                return HealthCheckResult.Unhealthy();
            }
            // An untrusted fixture registry can never be production-ready. It
            // is degraded rather than unhealthy so a developer's machine still
            // reports a usable plane.
            return snapshot.Trust == ModelProfileTrust.Signed
                ? HealthCheckResult.Healthy()
                : HealthCheckResult.Degraded();
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
/// Reports whether the Responses surface is reachable and accepts the plane's
/// credential.
///
/// <para>
/// The probe is a <c>POST</c> with an empty JSON body. It is non-billable by
/// construction: the provider rejects it for shape before any model runs, so a
/// readiness poll can never spend tokens. That is also exactly the measurement
/// the operator took against the real project — an empty body returned
/// <c>400</c>, which is the desired answer, because it proves authentication
/// succeeded and only the body was refused.
/// </para>
/// <para>
/// A <c>401</c> or <c>403</c> is unhealthy: the surface is reachable but the
/// plane may not use it. A <c>429</c> is degraded rather than unhealthy,
/// because throttling is a capacity condition and not a broken dependency. The
/// result is cached for a minimum interval so polling cannot become its own
/// load.
/// </para>
/// </summary>
public sealed class FoundryResponsesReachabilityHealthCheck(
    HttpClient httpClient,
    IFoundryRequestAuthenticator authenticator,
    TimeProvider timeProvider,
    TimeSpan? minimumInterval = null) : IHealthCheck
{
    public static readonly TimeSpan DefaultMinimumInterval = TimeSpan.FromMinutes(1);

    /// <summary>
    /// The non-billable probe body. It is deliberately an empty object: valid
    /// JSON, and missing every required field, so it is refused for shape.
    /// </summary>
    internal const string ProbeBody = "{}";

    private readonly TimeSpan _minimumInterval = minimumInterval ?? DefaultMinimumInterval;
    private readonly SemaphoreSlim _gate = new(1, 1);
    private DateTimeOffset _checkedAt = DateTimeOffset.MinValue;
    private HealthStatus _lastStatus = HealthStatus.Unhealthy;

    public async Task<HealthCheckResult> CheckHealthAsync(
        HealthCheckContext context,
        CancellationToken cancellationToken = default)
    {
        DateTimeOffset now = timeProvider.GetUtcNow();
        await _gate.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            if (_checkedAt != DateTimeOffset.MinValue && now - _checkedAt < _minimumInterval)
            {
                return new HealthCheckResult(_lastStatus);
            }
            _lastStatus = await ProbeAsync(cancellationToken).ConfigureAwait(false);
            _checkedAt = now;
            return new HealthCheckResult(_lastStatus);
        }
        finally
        {
            _gate.Release();
        }
    }

    private async Task<HealthStatus> ProbeAsync(CancellationToken cancellationToken)
    {
        try
        {
            using HttpRequestMessage request = new(
                HttpMethod.Post,
                FoundryResponsesProviderExecutor.ResponsesPath)
            {
                Content = new StringContent(
                    ProbeBody,
                    System.Text.Encoding.UTF8,
                    new MediaTypeHeaderValue("application/json")),
            };
            IFoundryRequestAuthenticator operation = await authenticator
                .BeginOperationAsync(cancellationToken)
                .ConfigureAwait(false);
            await operation.ApplyAsync(request, cancellationToken).ConfigureAwait(false);

            using HttpResponseMessage response = await httpClient
                .SendAsync(request, HttpCompletionOption.ResponseHeadersRead, cancellationToken)
                .ConfigureAwait(false);
            return Classify(response.StatusCode);
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            throw;
        }
        catch
        {
            // Unreachable, timed out, or the credential could not be applied.
            // No provider text ever reaches a health response.
            return HealthStatus.Unhealthy;
        }
    }

    /// <summary>
    /// Maps the probe status. A shape rejection is the healthy answer: it means
    /// the request was authenticated and routed, and only the deliberately
    /// empty body was refused.
    /// </summary>
    internal static HealthStatus Classify(HttpStatusCode statusCode) => (int)statusCode switch
    {
        400 or 422 => HealthStatus.Healthy,
        429 => HealthStatus.Degraded,
        _ => HealthStatus.Unhealthy,
    };
}
