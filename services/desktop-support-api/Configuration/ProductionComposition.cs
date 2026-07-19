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

        // Tasks 6+ supply the model broker; production still cannot boot
        // until every authority adapter exists.
        throw new InvalidOperationException(
            "Production authority adapters are not configured.");
    }
}
