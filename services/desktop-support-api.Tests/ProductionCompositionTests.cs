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

    [Theory]
    [InlineData("http://signing.vault.azure.net/")]
    [InlineData("https://user@signing.vault.azure.net/")]
    [InlineData("https://signing.vault.azure.net/keys")]
    [InlineData("https://signing.vault.azure.net/?sv=token")]
    [InlineData("https://signing.vault.example.com/")]
    public void Production_options_reject_untrusted_key_vault_uris(string keyVaultUri)
    {
        ProductionOptions options = CreateProductionOptions(keyVaultUri: keyVaultUri);

        Assert.Throws<InvalidOperationException>(() =>
            options.Validate(CreateSupportPlane()));
    }

    [Theory]
    [InlineData("http://models.openai.azure.com/")]
    [InlineData("https://user@models.openai.azure.com/")]
    [InlineData("https://models.openai.azure.com/v1")]
    [InlineData("https://models.openai.azure.com/?api-key=value")]
    [InlineData("https://models.openai.example.com/")]
    public void Production_options_reject_untrusted_model_endpoints(string modelEndpoint)
    {
        ProductionOptions options = CreateProductionOptions(modelEndpoint: modelEndpoint);

        Assert.Throws<InvalidOperationException>(() =>
            options.Validate(CreateSupportPlane()));
    }

    [Theory]
    [InlineData(true, false, "")]
    [InlineData(false, true, "")]
    [InlineData(false, false, "C:/store/consent")]
    public void Production_rejects_development_flags_and_file_stores(
        bool signingEnabled,
        bool modelEnabled,
        string consentStorePath)
    {
        SupportPlaneOptions supportPlane = new()
        {
            Authority = CreateSupportPlane().Authority,
            Audience = CreateSupportPlane().Audience,
            ApprovedDesktopClientId = CreateSupportPlane().ApprovedDesktopClientId,
            Region = CreateSupportPlane().Region,
            DevelopmentSigningEnabled = signingEnabled,
            DevelopmentModelEnabled = modelEnabled,
            DevelopmentConsentStorePath = consentStorePath,
        };

        Assert.Throws<InvalidOperationException>(() =>
            supportPlane.Validate(new ProductionHostEnvironment()));
    }

    [Fact]
    public void Each_azure_client_receives_its_own_managed_identity_credential()
    {
        SupportPlaneOptions supportPlane = CreateSupportPlane();
        ProductionOptions options = CreateProductionOptions();
        options.Validate(supportPlane);

        ProductionAzureCredentials credentials =
            AzureClientRegistration.CreateManagedIdentityCredentials(options);

        Assert.IsType<Azure.Identity.ManagedIdentityCredential>(credentials.AppConfiguration);
        Assert.IsType<Azure.Identity.ManagedIdentityCredential>(credentials.KeyVault);
        Assert.IsType<Azure.Identity.ManagedIdentityCredential>(credentials.Model);
        Assert.NotSame(credentials.AppConfiguration, credentials.KeyVault);
        Assert.NotSame(credentials.KeyVault, credentials.Model);
        Assert.NotSame(credentials.AppConfiguration, credentials.Model);
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
        string appConfigurationEndpoint = "https://configuration.azconfig.io/",
        string keyVaultUri = "https://signing.vault.azure.net/",
        string modelEndpoint = "https://models.openai.azure.com/") => new()
    {
        ManagedIdentityClientId = Guid.Parse("44444444-4444-4444-4444-444444444444"),
        AppConfigurationEndpoint = appConfigurationEndpoint,
        KeyVaultUri = keyVaultUri,
        ReceiptSigningKeyName = "model-receipt-signing",
        SqlServer = "authority.database.windows.net",
        SqlDatabase = "desktop-authority",
        ModelEndpoint = modelEndpoint,
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
