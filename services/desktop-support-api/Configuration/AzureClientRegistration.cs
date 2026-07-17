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

public static class AzureClientRegistration
{
    public static IServiceCollection AddProductionAzureClients(
        this IServiceCollection services,
        ProductionOptions options)
    {
        services.TryAddSingleton(_ =>
        {
            ManagedIdentityId managedIdentity = ManagedIdentityId.FromUserAssignedClientId(
                options.ManagedIdentityClientId.ToString("D"));
            TokenCredential appConfigurationCredential =
                new ManagedIdentityCredential(managedIdentity);
            TokenCredential keyVaultCredential =
                new ManagedIdentityCredential(managedIdentity);
            TokenCredential modelCredential =
                new ManagedIdentityCredential(managedIdentity);
            return new ProductionAzureClients(
                new ConfigurationClient(
                    options.ValidatedAppConfigurationEndpoint,
                    appConfigurationCredential),
                new KeyClient(options.ValidatedKeyVaultUri, keyVaultCredential),
                new AzureOpenAIClient(options.ValidatedModelEndpoint, modelCredential));
        });
        services.TryAddSingleton(options);
        return services;
    }
}
