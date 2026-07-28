using Azure.Monitor.OpenTelemetry.AspNetCore;
using Microsoft.Extensions.Hosting;
using OpenTelemetry.Trace;
using Sapphirus.DesktopSupportApi.Foundry;
using Sapphirus.DesktopSupportApi.Health;
using Sapphirus.DesktopSupportApi.Model;
using Sapphirus.DesktopSupportApi.Sql;
using Sapphirus.DesktopSupportApi.Policy;
using Sapphirus.DesktopSupportApi.Signing;

namespace Sapphirus.DesktopSupportApi.Configuration;

public static class ProductionComposition
{
    public static void AddProductionComposition(
        this WebApplicationBuilder builder,
        SupportPlaneOptions supportPlane)
    {
        ProductionOptions production =
            builder.GetValidatedProductionOptions(supportPlane);
        builder.Services.AddProductionAzureClients(production);
        builder.Services.AddSingleton<IPolicySettingsSource>(provider =>
            new AppConfigurationPolicySettingsSource(
                provider.GetRequiredService<ProductionAzureClients>().AppConfiguration));
        builder.Services.AddSingleton(provider => new AppConfigurationPolicyProvider(
            provider.GetRequiredService<IPolicySettingsSource>(),
            TimeProvider.System));
        builder.Services.AddSingleton<IHashSigner>(provider => new KeyVaultHashSigner(
            provider.GetRequiredService<ProductionAzureClients>().KeyVault
                .GetCryptographyClient(production.ReceiptSigningKeyName),
            production.ValidatedKeyVaultUri
                + "keys/" + production.ReceiptSigningKeyName));
        builder.Services.AddSingleton<ISignedPolicyService>(provider =>
            new AzureSignedPolicyService(
                provider.GetRequiredService<AppConfigurationPolicyProvider>(),
                provider.GetRequiredService<IHashSigner>(),
                TimeProvider.System));
        builder.Services.AddSingleton<IModelReceiptSigner>(provider =>
            new AzureModelReceiptSigner(
                provider.GetRequiredService<IHashSigner>(),
                supportPlane));

        // The legacy fixed Chat Completions broker is registered only when the
        // legacy model path is configured. A Foundry-only revision composes
        // and starts without it rather than failing at construction, which is
        // what lets the template stop injecting the legacy values before the
        // Responses adapter replaces the executor.
        if (production.HasLegacyModelPath)
        {
            builder.Services.AddSingleton<IModelProviderExecutor>(provider =>
                new AzureOpenAiProviderExecutor(
                    provider.GetRequiredService<ProductionAzureClients>().Model
                        ?? throw new InvalidOperationException(
                            "The legacy model client is unavailable despite a configured legacy model path.")));
            builder.Services.AddSingleton<IModelAccessBroker>(provider =>
                new AzureOpenAiModelAccessBroker(
                    async cancellationToken => ModelAccessProfile.Resolve(
                        await provider
                            .GetRequiredService<AppConfigurationPolicyProvider>()
                            .GetSnapshotAsync(cancellationToken)
                            .ConfigureAwait(false),
                        production,
                        supportPlane),
                    provider.GetRequiredService<IModelProviderExecutor>(),
                    provider.GetRequiredService<IModelReceiptSigner>(),
                    TimeProvider.System));
        }

        // Foundry deployment catalog (FND-021..FND-023): discovery runs on its
        // own user-assigned identity and its own bounded transport, and the
        // cache is the only thing the rest of the plane reads. Warming happens
        // in the background so a cold or unreachable provider shows up as an
        // unhealthy catalog rather than a host that will not start.
        builder.Services.AddSingleton(provider => new FoundryDeploymentCatalogCache(
            provider.GetRequiredService<IFoundryDeploymentDiscovery>(),
            TimeProvider.System,
            production.FoundryDeploymentRefreshInterval,
            production.FoundryCatalogMaximumStaleDuration));
        builder.Services.AddHostedService(provider => new FoundryCatalogRefreshService(
            provider.GetRequiredService<FoundryDeploymentCatalogCache>(),
            TimeProvider.System,
            production.FoundryDeploymentRefreshInterval));
        builder.Services.AddSingleton<FoundryCatalogHealthCheck>(provider =>
            new FoundryCatalogHealthCheck(
                provider.GetRequiredService<FoundryDeploymentCatalogCache>()));

        // Durable SQL authority stores (Task 7): the transactional
        // active/epoch checks in these adapters are the cross-replica
        // authority for revocation, consent single-use, and idempotency.
        builder.Services.AddSingleton(new SqlConnectionFactory(production));
        builder.Services.AddSingleton<IDeviceRegistry>(provider =>
            new SqlDeviceRegistry(provider.GetRequiredService<SqlConnectionFactory>()));
        builder.Services.AddSingleton<IIdempotencyStore>(provider =>
            new SqlIdempotencyStore(provider.GetRequiredService<SqlConnectionFactory>()));
        builder.Services.AddSingleton<IModelCallIdempotencyStore>(provider =>
            new SqlModelCallIdempotencyStore(
                provider.GetRequiredService<SqlConnectionFactory>()));
        builder.Services.AddSingleton<IContextConsentConsumptionStore>(provider =>
            new SqlConsentConsumptionStore(
                provider.GetRequiredService<SqlConnectionFactory>()));
        builder.Services.AddSingleton<IContextConsentVerifier>(
            new Security.InstallationConsentVerifier(TimeProvider.System));

        // Privacy-safe observability (Task 8): dependency health probes and
        // trace redaction; Azure Monitor export activates only when the
        // platform injects a connection string.
        builder.Services.AddHealthChecks()
            .AddCheck<SqlAuthorityHealthCheck>(
                AzureDependencyHealthChecks.SqlDependency)
            .AddCheck<PolicyConfigurationHealthCheck>(
                AzureDependencyHealthChecks.ConfigurationDependency)
            .AddCheck<FoundryCatalogHealthCheck>(
                FoundryCatalogHealthCheck.FoundryCatalogDependency);
        builder.Services.AddSingleton<SqlAuthorityHealthCheck>();
        builder.Services.AddSingleton<PolicyConfigurationHealthCheck>(provider =>
            new PolicyConfigurationHealthCheck(
                provider.GetRequiredService<AppConfigurationPolicyProvider>()));
        if (!string.IsNullOrWhiteSpace(
            builder.Configuration["APPLICATIONINSIGHTS_CONNECTION_STRING"]))
        {
            builder.Services.AddOpenTelemetry()
                .UseAzureMonitor()
                .WithTracing(tracing => tracing.AddProcessor(
                    new Observability.PrivacyRedactionProcessor()));
        }
    }

    internal static ProductionOptions GetValidatedProductionOptions(
        this WebApplicationBuilder builder,
        SupportPlaneOptions supportPlane)
    {
        ProductionOptions production = builder.Configuration
            .GetSection(SupportPlaneOptions.SectionName)
            .Get<ProductionOptions>() ?? new ProductionOptions();
        production.Validate(supportPlane);
        return production;
    }
}
