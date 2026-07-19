using Sapphirus.DesktopSupportApi.Model;
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

        // Task 7 binds the durable SQL authority stores; production still
        // cannot boot until every authority adapter exists.
        throw new InvalidOperationException(
            "Production authority adapters are not configured.");
    }
}
