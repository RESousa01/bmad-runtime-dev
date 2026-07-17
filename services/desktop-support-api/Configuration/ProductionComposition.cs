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

        throw new InvalidOperationException(
            "Production authority adapters are not configured.");
    }
}
