using Azure.AI.OpenAI;
using Azure.Core;
using Azure.Data.AppConfiguration;
using Azure.Identity;
using Azure.Security.KeyVault.Keys;
using Microsoft.Extensions.DependencyInjection.Extensions;

namespace Sapphirus.DesktopSupportApi.Configuration;

public sealed record ProductionAzureClients(
    ConfigurationClient AppConfiguration,
    KeyClient KeyVault,
    AzureOpenAIClient Model);

internal sealed record ProductionAzureCredentials(
    TokenCredential AppConfiguration,
    TokenCredential KeyVault,
    TokenCredential Model);

public static class AzureClientRegistration
{
    internal static ProductionAzureCredentials CreateManagedIdentityCredentials(
        ProductionOptions options)
    {
        ManagedIdentityId managedIdentity = ManagedIdentityId.FromUserAssignedClientId(
            options.ManagedIdentityClientId.ToString("D"));
        return new ProductionAzureCredentials(
            new ManagedIdentityCredential(managedIdentity),
            new ManagedIdentityCredential(managedIdentity),
            new ManagedIdentityCredential(managedIdentity));
    }

    public static IServiceCollection AddProductionAzureClients(
        this IServiceCollection services,
        ProductionOptions options)
    {
        services.TryAddSingleton(_ =>
        {
            ProductionAzureCredentials credentials = CreateManagedIdentityCredentials(options);
            return new ProductionAzureClients(
                new ConfigurationClient(
                    options.ValidatedAppConfigurationEndpoint,
                    credentials.AppConfiguration),
                new KeyClient(options.ValidatedKeyVaultUri, credentials.KeyVault),
                new AzureOpenAIClient(options.ValidatedModelEndpoint, credentials.Model));
        });
        services.TryAddSingleton(options);
        return services;
    }
}
