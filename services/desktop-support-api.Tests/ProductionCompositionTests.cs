using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.FileProviders;
using Microsoft.Extensions.Hosting;
using Sapphirus.DesktopSupportApi;
using Sapphirus.DesktopSupportApi.Configuration;
using Xunit;
using FoundryNamespace = Sapphirus.DesktopSupportApi.Foundry;

namespace Sapphirus.DesktopSupportApi.Tests;

public sealed class ProductionCompositionTests
{
    private const string Hash =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    private const string RuntimeIdentity = "44444444-4444-4444-4444-444444444444";
    private const string ProjectEndpoint =
        "https://account.services.ai.azure.com/api/projects/forge-project";
    private const string FoundryApiKey = "test-foundry-project-key-0001";

    [Fact]
    public void Production_options_validate_private_service_endpoints_and_profile_bindings()
    {
        SupportPlaneOptions supportPlane = CreateSupportPlane();
        supportPlane.Validate(new ProductionHostEnvironment());
        ProductionOptions options = CreateProductionOptions();

        options.Validate(supportPlane);

        Assert.Equal("configuration.azconfig.io", options.ValidatedAppConfigurationEndpoint.Host);
        Assert.Equal("signing.vault.azure.net", options.ValidatedKeyVaultUri.Host);
        Assert.True(options.HasLegacyModelPath);
        Assert.NotNull(options.ValidatedModelEndpoint);
        Assert.Equal("models.openai.azure.com", options.ValidatedModelEndpoint.Host);
        Assert.Equal(
            "account.services.ai.azure.com",
            options.ValidatedFoundryProjectEndpoint.Host);
    }

    [Fact]
    public void A_foundry_only_configuration_validates_without_the_legacy_model_path()
    {
        SupportPlaneOptions supportPlane = CreateSupportPlane();
        supportPlane.Validate(new ProductionHostEnvironment());
        ProductionOptions options = CreateProductionOptions(
            modelEndpoint: "",
            modelDeployment: "",
            deploymentHash: "");

        options.Validate(supportPlane);

        Assert.False(options.HasLegacyModelPath);
        Assert.Null(options.ValidatedModelEndpoint);
        Assert.Equal(
            "account.services.ai.azure.com",
            options.ValidatedFoundryProjectEndpoint.Host);
    }

    [Theory]
    [InlineData("", "fixture-deployment", Hash)]
    [InlineData("https://models.openai.azure.com/", "", Hash)]
    [InlineData("https://models.openai.azure.com/", "fixture-deployment", "")]
    public void A_partially_configured_legacy_model_path_is_rejected(
        string modelEndpoint,
        string modelDeployment,
        string deploymentHash)
    {
        SupportPlaneOptions supportPlane = CreateSupportPlane();
        supportPlane.Validate(new ProductionHostEnvironment());
        ProductionOptions options = CreateProductionOptions(
            modelEndpoint: modelEndpoint,
            modelDeployment: modelDeployment,
            deploymentHash: deploymentHash);

        Assert.Throws<InvalidOperationException>(() => options.Validate(supportPlane));
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
    // Account-only host with no project path.
    [InlineData("https://account.services.ai.azure.com/")]
    [InlineData("https://account.services.ai.azure.com")]
    // The retired Azure OpenAI account host is not a Foundry project host.
    [InlineData("https://account.openai.azure.com/api/projects/forge-project")]
    [InlineData("https://account.cognitiveservices.azure.com/api/projects/forge-project")]
    // Suffix lookalikes and bare suffix.
    [InlineData("https://account.services.ai.azure.com.evil.test/api/projects/forge-project")]
    [InlineData("https://services.ai.azure.com/api/projects/forge-project")]
    [InlineData("https://accountservices.ai.azure.com/api/projects/forge-project")]
    // Dot-segment traversal, even when it normalizes to a legal-looking path.
    [InlineData("https://account.services.ai.azure.com/api/projects/forge-project/../evil")]
    [InlineData("https://account.services.ai.azure.com/api/../api/projects/forge-project")]
    // Percent-encoding, whether or not it decodes to the canonical spelling.
    [InlineData("https://account.services.ai.azure.com/api/projects/for%67e-project")]
    [InlineData("https://account.services.ai.azure.com/api/projects/%2e%2e%2fadmin")]
    [InlineData("https://account.services.ai.azure.com/api/projects/forge%2Fdeployments")]
    // Mixed case in the host and in the fixed path segments.
    [InlineData("https://Account.services.ai.azure.com/api/projects/forge-project")]
    [InlineData("https://account.services.ai.azure.com/API/projects/forge-project")]
    [InlineData("https://account.services.ai.azure.com/api/Projects/forge-project")]
    // Extra trailing path beyond the project.
    [InlineData("https://account.services.ai.azure.com/api/projects/forge-project/deployments")]
    [InlineData("https://account.services.ai.azure.com/api/projects/forge-project/openai/v1/")]
    [InlineData("https://account.services.ai.azure.com/api/projects//forge-project")]
    [InlineData("https://account.services.ai.azure.com/api/projects/")]
    // Explicit port, query, fragment, user info, and plaintext scheme.
    [InlineData("https://account.services.ai.azure.com:443/api/projects/forge-project")]
    [InlineData("https://account.services.ai.azure.com:8443/api/projects/forge-project")]
    [InlineData("https://account.services.ai.azure.com/api/projects/forge-project?api-version=v1")]
    [InlineData("https://account.services.ai.azure.com/api/projects/forge-project#fragment")]
    [InlineData("https://user:pass@account.services.ai.azure.com/api/projects/forge-project")]
    [InlineData("http://account.services.ai.azure.com/api/projects/forge-project")]
    // Homograph hosts, in both their Unicode and punycode spellings.
    [InlineData("https://\u00e1ccount.services.ai.azure.com/api/projects/forge-project")]
    [InlineData("https://xn--ccount-2va.services.ai.azure.com/api/projects/forge-project")]
    [InlineData("https://account.services.ai.azure.\u0441om/api/projects/forge-project")]
    [InlineData("")]
    public void Production_options_reject_untrusted_foundry_project_endpoints(string endpoint)
    {
        ProductionOptions options = CreateProductionOptions(foundryProjectEndpoint: endpoint);

        Assert.Throws<InvalidOperationException>(() =>
            options.Validate(CreateSupportPlane()));
    }

    [Theory]
    [InlineData(
        "https://account.services.ai.azure.com/api/projects/forge-project",
        "https://account.services.ai.azure.com/api/projects/forge-project/")]
    [InlineData(
        "https://account.services.ai.azure.com/api/projects/forge-project/",
        "https://account.services.ai.azure.com/api/projects/forge-project/")]
    [InlineData(
        "https://account.services.ai.azure.com/api/projects/_project",
        "https://account.services.ai.azure.com/api/projects/_project/")]
    // The shape-valid placeholder the local production-configuration contract
    // and main.bicep inject. It is a project name, never a deployment name.
    [InlineData(
        "https://forge-contract.services.ai.azure.com/api/projects/forge-contract",
        "https://forge-contract.services.ai.azure.com/api/projects/forge-contract/")]
    public void Production_options_normalize_the_foundry_project_endpoint_to_one_trailing_slash(
        string configured,
        string expected)
    {
        ProductionOptions options = CreateProductionOptions(foundryProjectEndpoint: configured);

        options.Validate(CreateSupportPlane());

        Assert.Equal(expected, options.ValidatedFoundryProjectEndpoint.AbsoluteUri);
    }

    [Fact]
    public void Foundry_responses_endpoint_is_derived_and_never_configured_separately()
    {
        ProductionOptions options = CreateProductionOptions();

        options.Validate(CreateSupportPlane());

        Assert.Equal(
            "https://account.services.ai.azure.com/api/projects/forge-project/openai/v1/",
            options.FoundryResponsesEndpoint.AbsoluteUri);
        Assert.DoesNotContain(
            typeof(ProductionOptions).GetProperties(),
            property => property.Name == nameof(ProductionOptions.FoundryResponsesEndpoint)
                && property.SetMethod is { IsPublic: true });
    }

    [Theory]
    [InlineData("")]
    [InlineData("short")]
    [InlineData("has a space in it 12345")]
    [InlineData("has\ttab-and-more-characters")]
    public void Production_options_reject_a_missing_or_malformed_foundry_api_key(string apiKey)
    {
        ProductionOptions options = CreateProductionOptions(foundryApiKey: apiKey);

        Assert.Throws<InvalidOperationException>(() =>
            options.Validate(CreateSupportPlane()));
    }

    [Fact]
    public void Production_options_reject_an_undefined_foundry_inventory_mode()
    {
        ProductionOptions options = CreateProductionOptions(
            inventoryMode: (FoundryNamespace.FoundryInventoryMode)99);

        Assert.Throws<InvalidOperationException>(() =>
            options.Validate(CreateSupportPlane()));
    }

    [Fact]
    public void Production_options_reject_a_non_sha256_prerequisite_evidence_hash()
    {
        ProductionOptions options = CreateProductionOptions(
            prerequisiteEvidenceHash: "not-a-hash");

        Assert.Throws<InvalidOperationException>(() =>
            options.Validate(CreateSupportPlane()));
    }

    [Theory]
    [InlineData(FoundryNamespace.FoundryInventoryMode.ConfiguredAndProbed)]
    [InlineData(FoundryNamespace.FoundryInventoryMode.OpenAiModels)]
    [InlineData(FoundryNamespace.FoundryInventoryMode.ProjectDeploymentsApiKey)]
    public void Production_options_accept_each_explicit_foundry_inventory_mode(
        FoundryNamespace.FoundryInventoryMode mode)
    {
        ProductionOptions options = CreateProductionOptions(inventoryMode: mode);

        options.Validate(CreateSupportPlane());

        Assert.Equal(mode, options.FoundryInventoryMode);
    }

    [Theory]
    [InlineData(0, 1_800)]
    [InlineData(29, 1_800)]
    [InlineData(3_601, 7_200)]
    [InlineData(300, 299)]
    [InlineData(300, 86_401)]
    [InlineData(-1, 1_800)]
    public void Production_options_reject_unbounded_catalog_refresh_windows(
        int refreshSeconds,
        int maximumStaleSeconds)
    {
        ProductionOptions options = CreateProductionOptions(
            refreshSeconds: refreshSeconds,
            maximumStaleSeconds: maximumStaleSeconds);

        Assert.Throws<InvalidOperationException>(() =>
            options.Validate(CreateSupportPlane()));
    }

    [Theory]
    [InlineData("")]
    [InlineData("desktop-support:model-profiles")]
    [InlineData("Desktop-Support:model-profiles:")]
    [InlineData("desktop support:")]
    [InlineData("../secrets:")]
    public void Production_options_reject_unsafe_model_profile_configuration_prefixes(
        string prefix)
    {
        ProductionOptions options = CreateProductionOptions(modelProfilePrefix: prefix);

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

    [Fact]
    public void Foundry_discovery_transport_is_authenticated_with_the_backend_api_key_only()
    {
        SupportPlaneOptions supportPlane = CreateSupportPlane();
        ProductionOptions options = CreateProductionOptions();
        options.Validate(supportPlane);
        ServiceCollection services = new();
        services.AddProductionAzureClients(options);

        using ServiceProvider provider = services.BuildServiceProvider();
        FoundryNamespace.IFoundryRequestAuthenticator authenticator =
            provider.GetRequiredService<FoundryNamespace.IFoundryRequestAuthenticator>();

        Assert.IsType<FoundryNamespace.FoundryApiKeyAuthenticator>(authenticator);
        Assert.Equal(
            FoundryNamespace.FoundryCredentialMechanisms.ApiKey,
            authenticator.CredentialMechanismId);
    }

    private static SupportPlaneOptions CreateSupportPlane(string releaseChannel = "beta") => new()
    {
        Authority =
            "https://login.microsoftonline.com/11111111-1111-1111-1111-111111111111/v2.0",
        Audience = "api://22222222-2222-2222-2222-222222222222",
        ApprovedDesktopClientId = "33333333-3333-3333-3333-333333333333",
        Region = "westeurope",
        ReleaseChannel = releaseChannel,
    };

    internal static ProductionOptions CreateProductionOptions(
        string appConfigurationEndpoint = "https://configuration.azconfig.io/",
        string keyVaultUri = "https://signing.vault.azure.net/",
        string modelEndpoint = "https://models.openai.azure.com/",
        string modelDeployment = "desktop-planner",
        string deploymentHash = Hash,
        string foundryProjectEndpoint = ProjectEndpoint,
        Guid? sqlMigrationIdentity = null,
        string? foundryApiKey = null,
        FoundryNamespace.FoundryInventoryMode inventoryMode =
            FoundryNamespace.FoundryInventoryMode.ConfiguredAndProbed,
        string? prerequisiteEvidenceHash = null,
        int refreshSeconds = 300,
        int maximumStaleSeconds = 1_800,
        string modelProfilePrefix = "desktop-support:model-profiles:") => new()
    {
        ManagedIdentityClientId = Guid.Parse(RuntimeIdentity),
        SqlMigrationIdentityClientId = sqlMigrationIdentity ?? Guid.Empty,
        AppConfigurationEndpoint = appConfigurationEndpoint,
        KeyVaultUri = keyVaultUri,
        ReceiptSigningKeyName = "model-receipt-signing",
        SqlServer = "authority.database.windows.net",
        SqlDatabase = "desktop-authority",
        ModelEndpoint = modelEndpoint,
        ModelDeployment = modelDeployment,
        ProviderProfileHash = Hash,
        ModelProfileHash = Hash,
        ModelCapabilityHash = Hash,
        DeploymentHash = deploymentHash,
        FoundryProjectEndpoint = foundryProjectEndpoint,
        FoundryApiKey = foundryApiKey ?? FoundryApiKey,
        FoundryInventoryMode = inventoryMode,
        FoundryPrerequisiteEvidenceHash = prerequisiteEvidenceHash ?? Hash,
        FoundryDeploymentRefreshSeconds = refreshSeconds,
        FoundryCatalogMaximumStaleSeconds = maximumStaleSeconds,
        ModelProfileConfigurationPrefix = modelProfilePrefix,
    };

    private sealed class ProductionHostEnvironment : IHostEnvironment
    {
        public string EnvironmentName { get; set; } = Environments.Production;
        public string ApplicationName { get; set; } = "Tests";
        public string ContentRootPath { get; set; } = AppContext.BaseDirectory;
        public IFileProvider ContentRootFileProvider { get; set; } = new NullFileProvider();
    }
}
