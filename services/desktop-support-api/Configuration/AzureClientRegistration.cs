using System.Net;
using Azure.AI.OpenAI;
using Azure.Core;
using Azure.Data.AppConfiguration;
using Azure.Identity;
using Azure.Security.KeyVault.Keys;
using Microsoft.Extensions.DependencyInjection.Extensions;
using Sapphirus.DesktopSupportApi.Foundry;

namespace Sapphirus.DesktopSupportApi.Configuration;

/// <summary>
/// <paramref name="Model"/> is null when the legacy fixed Chat Completions
/// path is not configured. A Foundry-only revision composes and starts
/// without it; the legacy broker is simply not registered.
/// </summary>
public sealed record ProductionAzureClients(
    ConfigurationClient AppConfiguration,
    KeyClient KeyVault,
    AzureOpenAIClient? Model);

internal sealed record ProductionAzureCredentials(
    TokenCredential AppConfiguration,
    TokenCredential KeyVault,
    TokenCredential Model,
    TokenCredential FoundryModel);

public static class AzureClientRegistration
{
    /// <summary>
    /// The repository's first named <see cref="IHttpClientFactory"/> client.
    /// It exists so deployment discovery has a transport with its own
    /// redirect, decompression, timeout, and header policy instead of
    /// inheriting whatever a shared client happened to be configured with.
    /// </summary>
    public const string FoundryDiscoveryHttpClientName = "foundry-deployment-discovery";

    /// <summary>
    /// A fixed product marker. No OS build, machine name, or SDK version is
    /// disclosed to the provider.
    /// </summary>
    internal const string RedactedUserAgent = "sapphirus-support-plane/1";

    /// <summary>
    /// Asks Foundry to echo the per-operation client request ID so a support
    /// investigation can correlate without a log ever carrying a prompt,
    /// deployment name, or endpoint.
    /// </summary>
    internal const string CorrelationHeader = "x-ms-return-client-request-id";

    internal static readonly TimeSpan DiscoveryHeaderTimeout = TimeSpan.FromSeconds(10);

    /// <summary>
    /// Every dependency gets its own credential instance, and Foundry gets a
    /// separate user-assigned identity entirely. Nothing here ever falls back
    /// to an ambient or chained credential.
    /// </summary>
    internal static ProductionAzureCredentials CreateManagedIdentityCredentials(
        ProductionOptions options)
    {
        ArgumentNullException.ThrowIfNull(options);
        ManagedIdentityId runtimeIdentity = ManagedIdentityId.FromUserAssignedClientId(
            options.ManagedIdentityClientId.ToString("D"));
        ManagedIdentityId foundryIdentity = ManagedIdentityId.FromUserAssignedClientId(
            options.FoundryModelIdentityClientId.ToString("D"));
        return new ProductionAzureCredentials(
            new ManagedIdentityCredential(runtimeIdentity),
            new ManagedIdentityCredential(runtimeIdentity),
            new ManagedIdentityCredential(runtimeIdentity),
            new ManagedIdentityCredential(foundryIdentity));
    }

    public static IServiceCollection AddProductionAzureClients(
        this IServiceCollection services,
        ProductionOptions options)
    {
        ArgumentNullException.ThrowIfNull(services);
        ArgumentNullException.ThrowIfNull(options);
        services.TryAddSingleton(_ => CreateManagedIdentityCredentials(options));
        services.TryAddSingleton(provider =>
        {
            ProductionAzureCredentials credentials =
                provider.GetRequiredService<ProductionAzureCredentials>();
            return new ProductionAzureClients(
                new ConfigurationClient(
                    options.ValidatedAppConfigurationEndpoint,
                    credentials.AppConfiguration),
                new KeyClient(options.ValidatedKeyVaultUri, credentials.KeyVault),
                options.ValidatedModelEndpoint is null
                    ? null
                    : new AzureOpenAIClient(
                        options.ValidatedModelEndpoint,
                        credentials.Model));
        });
        services.TryAddSingleton(options);
        services.AddFoundryDiscoveryTransport(options);
        return services;
    }

    /// <summary>
    /// Registers the deployment-discovery transport, the production credential
    /// mechanism, and the client. The Responses executor is a separate task:
    /// the existing Azure OpenAI client stays exactly where it is and this seam
    /// is added beside it.
    ///
    /// The credential mechanism registered here is always managed identity. The
    /// development API-key mechanism is registered only by
    /// <see cref="DevelopmentFoundryComposition"/>, and its constructor refuses
    /// to build outside Development or Test.
    /// </summary>
    public static IServiceCollection AddFoundryDiscoveryTransport(
        this IServiceCollection services,
        ProductionOptions options)
    {
        ArgumentNullException.ThrowIfNull(services);
        ArgumentNullException.ThrowIfNull(options);
        services.AddFoundryDiscoveryHttpClient(options.ValidatedFoundryProjectEndpoint);
        services.TryAddSingleton<IFoundryRequestAuthenticator>(provider =>
            new ManagedIdentityFoundryAuthenticator(
                provider.GetRequiredService<ProductionAzureCredentials>().FoundryModel));
        services.TryAddSingleton(provider => new FoundryDeploymentDiscoveryClient(
            provider.GetRequiredService<IHttpClientFactory>()
                .CreateClient(FoundryDiscoveryHttpClientName),
            provider.GetRequiredService<IFoundryRequestAuthenticator>(),
            options.ValidatedFoundryProjectEndpoint,
            TimeProvider.System));
        services.TryAddSingleton<IFoundryDeploymentDiscovery>(provider =>
            new FoundryDeploymentDiscovery(
                provider.GetRequiredService<FoundryDeploymentDiscoveryClient>()));
        return services;
    }

    /// <summary>
    /// The bounded discovery transport, shared by every composition that talks
    /// to a Foundry project. Only the credential mechanism differs between
    /// Production and Development; the transport policy does not.
    /// </summary>
    internal static IServiceCollection AddFoundryDiscoveryHttpClient(
        this IServiceCollection services,
        Uri projectEndpoint)
    {
        ArgumentNullException.ThrowIfNull(services);
        ArgumentNullException.ThrowIfNull(projectEndpoint);
        services
            .AddHttpClient(FoundryDiscoveryHttpClientName, client =>
            {
                client.BaseAddress = projectEndpoint;
                client.Timeout = DiscoveryHeaderTimeout;
                client.MaxResponseContentBufferSize =
                    FoundryDeploymentDiscoveryClient.MaximumAggregateResponseBytes;
                client.DefaultRequestHeaders.UserAgent.Clear();
                client.DefaultRequestHeaders.UserAgent.ParseAdd(RedactedUserAgent);
                client.DefaultRequestHeaders.Accept.Clear();
                client.DefaultRequestHeaders.Accept.ParseAdd("application/json");
                client.DefaultRequestHeaders.Add(CorrelationHeader, "true");
                client.DefaultRequestHeaders.ExpectContinue = false;
            })
            .ConfigurePrimaryHttpMessageHandler(static () => new SocketsHttpHandler
            {
                // A redirect is the cheapest way to move a bearer token to
                // another origin, so the transport refuses to follow one.
                AllowAutoRedirect = false,
                AutomaticDecompression =
                    DecompressionMethods.GZip | DecompressionMethods.Deflate,
                MaxResponseHeadersLength = 16,
                ConnectTimeout = DiscoveryHeaderTimeout,
                UseCookies = false,
                UseProxy = false,
                PooledConnectionLifetime = TimeSpan.FromMinutes(5),
            });
        return services;
    }
}
