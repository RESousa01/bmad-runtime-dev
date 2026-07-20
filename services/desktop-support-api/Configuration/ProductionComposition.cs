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
        ProductionOptions production = builder.Configuration
            .GetSection(SupportPlaneOptions.SectionName)
            .Get<ProductionOptions>() ?? new ProductionOptions();
        production.Validate(supportPlane);
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

        builder.Services.AddSingleton<IModelProviderExecutor>(provider =>
            new AzureOpenAiProviderExecutor(
                provider.GetRequiredService<ProductionAzureClients>().Model));
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
    }
}
