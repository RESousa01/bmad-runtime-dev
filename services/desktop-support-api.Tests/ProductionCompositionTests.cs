using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.FileProviders;
using Microsoft.Extensions.Hosting;
using Sapphirus.DesktopSupportApi;
using Sapphirus.DesktopSupportApi.Configuration;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests;

public sealed class ProductionCompositionTests
{
    private const string Hash =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    [Fact]
    public void Production_options_validate_private_service_endpoints_and_profile_bindings()
    {
        SupportPlaneOptions supportPlane = CreateSupportPlane();
        supportPlane.Validate(new ProductionHostEnvironment());
        ProductionOptions options = CreateProductionOptions();

        options.Validate(supportPlane);

        Assert.Equal("configuration.azconfig.io", options.ValidatedAppConfigurationEndpoint.Host);
        Assert.Equal("signing.vault.azure.net", options.ValidatedKeyVaultUri.Host);
        Assert.Equal("models.openai.azure.com", options.ValidatedModelEndpoint.Host);
    }

    [Theory]
    [InlineData("http://configuration.azconfig.io/")]
    [InlineData("https://user@configuration.azconfig.io/")]
    [InlineData("https://configuration.azconfig.io/path")]
    [InlineData("https://configuration.azconfig.io/?secret=value")]
    [InlineData("https://configuration.example.com/")]
    public void Production_options_reject_untrusted_app_configuration_endpoints(string endpoint)
    {
        ProductionOptions options = CreateProductionOptions(endpoint);

        Assert.Throws<InvalidOperationException>(() =>
            options.Validate(CreateSupportPlane()));
    }

    [Fact]
    public void Azure_clients_are_composed_with_managed_identity_without_network_access()
    {
        SupportPlaneOptions supportPlane = CreateSupportPlane();
        ProductionOptions options = CreateProductionOptions();
        options.Validate(supportPlane);
        ServiceCollection services = new();
        services.AddProductionAzureClients(options);

        using ServiceProvider provider = services.BuildServiceProvider();
        ProductionAzureClients clients =
            provider.GetRequiredService<ProductionAzureClients>();

        Assert.NotNull(clients.AppConfiguration);
        Assert.NotNull(clients.KeyVault);
        Assert.NotNull(clients.Model);
    }

    private static SupportPlaneOptions CreateSupportPlane() => new()
    {
        Authority =
            "https://login.microsoftonline.com/11111111-1111-1111-1111-111111111111/v2.0",
        Audience = "api://22222222-2222-2222-2222-222222222222",
        ApprovedDesktopClientId = "33333333-3333-3333-3333-333333333333",
        Region = "westeurope",
    };

    private static ProductionOptions CreateProductionOptions(
        string appConfigurationEndpoint = "https://configuration.azconfig.io/") => new()
    {
        ManagedIdentityClientId = Guid.Parse("44444444-4444-4444-4444-444444444444"),
        AppConfigurationEndpoint = appConfigurationEndpoint,
        KeyVaultUri = "https://signing.vault.azure.net/",
        ReceiptSigningKeyName = "model-receipt-signing",
        SqlServer = "authority.database.windows.net",
        SqlDatabase = "desktop-authority",
        ModelEndpoint = "https://models.openai.azure.com/",
        ModelDeployment = "desktop-planner",
        ProviderProfileHash = Hash,
        ModelProfileHash = Hash,
        ModelCapabilityHash = Hash,
        DeploymentHash = Hash,
    };

    private sealed class ProductionHostEnvironment : IHostEnvironment
    {
        public string EnvironmentName { get; set; } = Environments.Production;
        public string ApplicationName { get; set; } = "Tests";
        public string ContentRootPath { get; set; } = AppContext.BaseDirectory;
        public IFileProvider ContentRootFileProvider { get; set; } = new NullFileProvider();
    }
}
