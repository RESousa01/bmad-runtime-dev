using Microsoft.Extensions.Hosting;
using Sapphirus.DesktopSupportApi.Foundry;
using Sapphirus.DesktopSupportApi.Health;
using Sapphirus.DesktopSupportApi.Model;

namespace Sapphirus.DesktopSupportApi.Configuration;

/// <summary>
/// Development-only Foundry composition.
///
/// It wires the real deployment-discovery client, the real normalizer, and the
/// real catalog cache against a real Foundry project, and swaps exactly one
/// thing: the credential mechanism. The operator has neither Entra
/// app-registration rights nor Azure RBAC role-assignment rights, so managed
/// identity is unobtainable here; an API key in an <c>api-key</c> header is the
/// only credential that can reach the project from this machine.
///
/// This is not a production path and cannot become one. Every entry point
/// refuses to compose outside Development and Test, and the authenticator's own
/// constructor refuses a second time.
/// </summary>
public static class DevelopmentFoundryComposition
{
    /// <summary>
    /// Configuration key for the development API key, supplied through the
    /// <c>Sapphirus__FoundryDevelopmentApiKey</c> environment variable. It must
    /// never appear in a committed <c>appsettings*.json</c> with a real value.
    /// </summary>
    public const string ApiKeyConfigurationKey =
        DevelopmentFoundryApiKeyAuthenticator.ConfigurationKey;

    /// <summary>
    /// The same project endpoint key Production binds, validated by the same
    /// validator. Development does not get a looser endpoint rule.
    /// </summary>
    public const string ProjectEndpointConfigurationKey =
        "Sapphirus:FoundryProjectEndpoint";

    /// <summary>Environment-variable spelling of the two keys above.</summary>
    public const string ApiKeyEnvironmentVariableName =
        DevelopmentFoundryApiKeyAuthenticator.EnvironmentVariableName;

    public const string ProjectEndpointEnvironmentVariableName =
        "Sapphirus__FoundryProjectEndpoint";

    /// <summary>
    /// Parallel to the fake Foundry server's
    /// <c>fake_foundry_environment_forbidden</c>.
    /// </summary>
    public const string EnvironmentForbiddenCode =
        "development_foundry_composition_environment_forbidden";

    public const string ProjectEndpointInvalidCode =
        "development_foundry_project_endpoint_invalid";

    /// <summary>
    /// Half a configuration is a configuration error, never a partial opt-in.
    /// </summary>
    public const string PartialConfigurationCode =
        "development_foundry_configuration_incomplete";

    internal static readonly TimeSpan RefreshInterval = TimeSpan.FromMinutes(5);
    internal static readonly TimeSpan MaximumStale = TimeSpan.FromMinutes(30);

    /// <summary>
    /// Composes the development Foundry catalog when, and only when, the
    /// operator asked for it.
    ///
    /// Neither key set means the mode was not requested: nothing is registered
    /// and the caller is told so. Either key set means it was requested, and
    /// then both must be valid or composition fails. There is no path where a
    /// half-configured development mode silently produces unauthenticated
    /// requests.
    /// </summary>
    public static bool TryAddDevelopmentFoundryCatalog(
        this IServiceCollection services,
        IHostEnvironment environment,
        IConfiguration configuration)
    {
        ArgumentNullException.ThrowIfNull(services);
        ArgumentNullException.ThrowIfNull(environment);
        ArgumentNullException.ThrowIfNull(configuration);
        string endpoint = configuration[ProjectEndpointConfigurationKey] ?? "";
        string keyValue = configuration[ApiKeyConfigurationKey] ?? "";
        if (endpoint.Length == 0 && keyValue.Length == 0)
        {
            return false;
        }
        if (endpoint.Length == 0 || keyValue.Length == 0)
        {
            throw new InvalidOperationException(PartialConfigurationCode);
        }
        services.AddDevelopmentFoundryCatalog(environment, configuration);
        return true;
    }

    /// <summary>
    /// Composes the development Foundry catalog unconditionally. Both the
    /// project endpoint and the API key must already be configured.
    /// </summary>
    public static IServiceCollection AddDevelopmentFoundryCatalog(
        this IServiceCollection services,
        IHostEnvironment environment,
        IConfiguration configuration)
    {
        ArgumentNullException.ThrowIfNull(services);
        ArgumentNullException.ThrowIfNull(environment);
        ArgumentNullException.ThrowIfNull(configuration);

        // First fail-closed boundary. The authenticator's constructor is the
        // second, so an accidental call cannot activate this in Staging or
        // Production even if this one were somehow bypassed.
        if (!DevelopmentFoundryApiKeyAuthenticator.IsPermittedEnvironment(
            environment.EnvironmentName))
        {
            throw new InvalidOperationException(EnvironmentForbiddenCode);
        }
        if (!ProductionOptions.TryValidateFoundryProjectEndpoint(
            configuration[ProjectEndpointConfigurationKey] ?? "",
            out Uri projectEndpoint))
        {
            throw new InvalidOperationException(ProjectEndpointInvalidCode);
        }

        // Constructed here rather than in a factory so a missing or malformed
        // key fails at composition instead of at the first provider call.
        DevelopmentFoundryApiKeyAuthenticator authenticator = new(
            environment.EnvironmentName,
            configuration[ApiKeyConfigurationKey]);

        services.AddFoundryDiscoveryHttpClient(projectEndpoint);

        // Registered under its concrete type as well as the seam, so a
        // composition audit can see the mechanism in the descriptor list
        // instead of having to build the container.
        services.AddSingleton(authenticator);
        services.AddSingleton<IFoundryRequestAuthenticator>(provider =>
            provider.GetRequiredService<DevelopmentFoundryApiKeyAuthenticator>());

        services.AddSingleton(provider => new FoundryDeploymentDiscoveryClient(
            provider.GetRequiredService<IHttpClientFactory>()
                .CreateClient(AzureClientRegistration.FoundryDiscoveryHttpClientName),
            provider.GetRequiredService<IFoundryRequestAuthenticator>(),
            projectEndpoint,
            TimeProvider.System));
        services.AddSingleton<IFoundryDeploymentDiscovery>(provider =>
            new FoundryDeploymentDiscovery(
                provider.GetRequiredService<FoundryDeploymentDiscoveryClient>()));

        // The Responses adapter, composed exactly as Production composes it.
        // The endpoint is derived from the same validated project endpoint,
        // and the executor holds the same authenticator seam; only the
        // mechanism behind that seam differs, and the mechanism identifier is
        // folded into every receipt's credential-binding hash so a development
        // receipt can never be mistaken for a managed-identity one.
        services.AddFoundryResponsesHttpClient(
            new Uri(projectEndpoint, "openai/v1/"));
        services.AddSingleton(provider => new FoundryResponsesProviderExecutor(
            provider.GetRequiredService<IHttpClientFactory>()
                .CreateClient(FoundryResponsesProviderExecutor.HttpClientName),
            provider.GetRequiredService<IFoundryRequestAuthenticator>()));
        services.AddSingleton<IModelStreamTransport>(provider =>
            provider.GetRequiredService<FoundryResponsesProviderExecutor>());

        // The streaming surface, composed exactly as Production composes it.
        //
        // Without these two registrations the route answered
        // foundry_project_unavailable even with a real project endpoint and a
        // valid development key, because the coordinator and the route resolver
        // are what the endpoint checks for before it will stream at all.
        //
        // The credential mechanism handed to the coordinator is read from the
        // executor rather than hard-coded, so it is the mechanism the transport
        // will actually use. It is folded into every receipt's
        // credential-binding hash, which is what keeps a development receipt
        // distinguishable from a managed-identity one at the hash level, and the
        // development receipt signer composed in Program.cs makes it
        // unmistakable at the proof level as well.
        services.AddSingleton(provider => new ModelStreamingCoordinator(
            provider.GetRequiredService<IModelStreamTransport>(),
            provider.GetRequiredService<IModelReceiptSignerV2>(),
            TimeProvider.System,
            ModelStreamLimits.Default,
            provider.GetRequiredService<FoundryResponsesProviderExecutor>()
                .CredentialMechanismId,
            provider.GetRequiredService<Observability.SupportPlaneTelemetry>(),
            provider.GetRequiredService<FoundryResponsesProviderExecutor>()
                .CredentialVersionId,
            ProductionOptions.ComputeFoundryProjectEndpointHash(projectEndpoint),
            "development"));
        services.AddSingleton<IModelStreamRouteResolver>(provider =>
            new EffectiveCatalogStreamRouteResolver(
                provider.GetRequiredService<EffectiveModelCatalogService>(),
                provider.GetRequiredService<IModelEntitlementSource>(),
                provider.GetRequiredService<IModelCatalogPolicySource>(),
                provider.GetRequiredService<SupportPlaneOptions>().Region,
                provider.GetRequiredService<Observability.SupportPlaneTelemetry>()));

        // The same cache, the same background refresh, and the same health
        // check Production composes. Only the credential differs.
        services.AddSingleton(provider => new FoundryDeploymentCatalogCache(
            provider.GetRequiredService<IFoundryDeploymentDiscovery>(),
            TimeProvider.System,
            RefreshInterval,
            MaximumStale));
        services.AddHostedService(provider => new FoundryCatalogRefreshService(
            provider.GetRequiredService<FoundryDeploymentCatalogCache>(),
            TimeProvider.System,
            RefreshInterval));
        services.AddSingleton(provider => new FoundryCatalogHealthCheck(
            provider.GetRequiredService<FoundryDeploymentCatalogCache>()));
        services.AddHealthChecks()
            .AddCheck<FoundryCatalogHealthCheck>(
                FoundryCatalogHealthCheck.FoundryCatalogDependency);
        return services;
    }
}
