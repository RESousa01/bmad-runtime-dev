using System.Net;
using Azure.Core;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Http;
using Microsoft.Extensions.Options;
using Sapphirus.DesktopSupportApi.Configuration;
using Sapphirus.DesktopSupportApi.Foundry;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Foundry;

/// <summary>
/// FND-021. Discovery must reach Foundry on its own user-assigned identity and
/// its own bounded transport, with nothing credential-shaped observable from
/// the container.
/// </summary>
public sealed class FoundryTransportCompositionTests
{
    private static readonly string[] CredentialHeaderNames =
    [
        "api-key",
        "Api-Key",
        "Authorization",
        "Ocp-Apim-Subscription-Key",
        "x-api-key",
    ];

    [Fact]
    public void Deployment_discovery_uses_a_named_client_bound_to_the_project_endpoint()
    {
        using ServiceProvider provider = BuildProvider();

        using HttpClient client = provider
            .GetRequiredService<IHttpClientFactory>()
            .CreateClient(AzureClientRegistration.FoundryDiscoveryHttpClientName);

        Assert.Equal(
            "https://account.services.ai.azure.com/api/projects/forge-project/",
            client.BaseAddress?.AbsoluteUri);
        Assert.Equal(TimeSpan.FromSeconds(10), client.Timeout);
        Assert.Equal(
            FoundryDeploymentDiscoveryClient.MaximumAggregateResponseBytes,
            client.MaxResponseContentBufferSize);
    }

    [Fact]
    public void Deployment_discovery_sends_a_redacted_user_agent_and_a_correlation_header()
    {
        using ServiceProvider provider = BuildProvider();

        using HttpClient client = provider
            .GetRequiredService<IHttpClientFactory>()
            .CreateClient(AzureClientRegistration.FoundryDiscoveryHttpClientName);

        Assert.Equal(
            AzureClientRegistration.RedactedUserAgent,
            client.DefaultRequestHeaders.UserAgent.ToString());
        Assert.True(client.DefaultRequestHeaders.Contains(
            AzureClientRegistration.CorrelationHeader));
        string userAgent = client.DefaultRequestHeaders.UserAgent.ToString();
        Assert.DoesNotContain("azure", userAgent, StringComparison.OrdinalIgnoreCase);
        Assert.DoesNotContain(Environment.MachineName, userAgent, StringComparison.OrdinalIgnoreCase);
    }

    [Fact]
    public void Deployment_discovery_never_exposes_an_api_key_header_or_a_token()
    {
        using ServiceProvider provider = BuildProvider();

        using HttpClient client = provider
            .GetRequiredService<IHttpClientFactory>()
            .CreateClient(AzureClientRegistration.FoundryDiscoveryHttpClientName);

        Assert.Null(client.DefaultRequestHeaders.Authorization);
        foreach (string headerName in CredentialHeaderNames)
        {
            Assert.False(
                client.DefaultRequestHeaders.Contains(headerName),
                headerName);
        }
        foreach ((string name, IEnumerable<string> values) in client.DefaultRequestHeaders)
        {
            foreach (string value in values)
            {
                Assert.DoesNotContain("Bearer", value, StringComparison.OrdinalIgnoreCase);
                Assert.DoesNotContain("eyJ", value, StringComparison.Ordinal);
                Assert.NotEqual("Authorization", name, StringComparer.OrdinalIgnoreCase);
            }
        }
    }

    [Fact]
    public void Container_diagnostics_expose_no_token_and_no_ambient_credential()
    {
        ServiceCollection services = new();
        services.AddProductionAzureClients(CreateValidatedOptions());

        foreach (ServiceDescriptor descriptor in services)
        {
            Assert.NotEqual(
                "DefaultAzureCredential",
                descriptor.ServiceType.Name,
                StringComparer.Ordinal);
            Assert.NotEqual(
                "DefaultAzureCredential",
                descriptor.ImplementationType?.Name ?? "",
                StringComparer.Ordinal);
            if (descriptor.ImplementationInstance is string registeredString)
            {
                Assert.DoesNotContain(
                    "services.ai.azure.com",
                    registeredString,
                    StringComparison.OrdinalIgnoreCase);
            }
        }

        using ServiceProvider provider = services.BuildServiceProvider();
        IFoundryRequestAuthenticator foundryAuthenticator = provider
            .GetRequiredService<IFoundryRequestAuthenticator>();

        Assert.IsType<FoundryApiKeyAuthenticator>(foundryAuthenticator);
        Assert.Equal(
            FoundryCredentialMechanisms.ApiKey,
            foundryAuthenticator.CredentialMechanismId);
        Assert.DoesNotContain(
            "eyJ",
            foundryAuthenticator.ToString() ?? "",
            StringComparison.Ordinal);
    }

    [Fact]
    public void Foundry_credential_is_not_one_of_the_azure_managed_identity_credentials()
    {
        ProductionOptions options = CreateValidatedOptions();

        ProductionAzureCredentials credentials =
            AzureClientRegistration.CreateManagedIdentityCredentials(options);

        Assert.All(
            new object[] { credentials.AppConfiguration, credentials.KeyVault, credentials.Model },
            credential => Assert.IsType<Azure.Identity.ManagedIdentityCredential>(credential));
    }

    [Fact]
    public void Validated_options_expose_the_configured_credential_version_and_revision()
    {
        ProductionOptions options = CreateValidatedOptions();

        Assert.Equal("v1", options.FoundryCredentialVersion);
        Assert.False(string.IsNullOrWhiteSpace(options.ApplicationRevision));
        Assert.Matches("^sha256:[0-9a-f]{64}$", options.FoundryProjectEndpointHash);
    }

    [Theory]
    [InlineData("")]
    [InlineData("has a space")]
    public void Validate_rejects_an_unusable_credential_version_label(string versionLabel)
    {
        ProductionOptions options =
            CopyOptionsWithOverride(credentialVersion: versionLabel);

        Assert.Throws<InvalidOperationException>(() => options.Validate(ValidSupportPlaneOptions()));
    }

    [Theory]
    [InlineData("")]
    [InlineData("has a space")]
    public void Validate_rejects_an_unusable_application_revision(string revision)
    {
        ProductionOptions options =
            CopyOptionsWithOverride(applicationRevision: revision);

        Assert.Throws<InvalidOperationException>(() => options.Validate(ValidSupportPlaneOptions()));
    }

    private static SupportPlaneOptions ValidSupportPlaneOptions() => new()
    {
        Authority =
            "https://login.microsoftonline.com/11111111-1111-1111-1111-111111111111/v2.0",
        Audience = "api://22222222-2222-2222-2222-222222222222",
        ApprovedDesktopClientId = "33333333-3333-3333-3333-333333333333",
        Region = "westeurope",
    };

    /// <summary>
    /// Rebuilds an otherwise-valid options object with one field deliberately
    /// broken. <see cref="ProductionOptions"/> is not a record, so this
    /// copies every field <see cref="ProductionCompositionTests.CreateProductionOptions"/>
    /// sets rather than using a <c>with</c> expression.
    /// </summary>
    private static ProductionOptions CopyOptionsWithOverride(
        string? credentialVersion = null,
        string? applicationRevision = null)
    {
        ProductionOptions baseline = ProductionCompositionTests.CreateProductionOptions();
        return new ProductionOptions
        {
            ManagedIdentityClientId = baseline.ManagedIdentityClientId,
            SqlMigrationIdentityClientId = baseline.SqlMigrationIdentityClientId,
            AppConfigurationEndpoint = baseline.AppConfigurationEndpoint,
            KeyVaultUri = baseline.KeyVaultUri,
            ReceiptSigningKeyName = baseline.ReceiptSigningKeyName,
            SqlServer = baseline.SqlServer,
            SqlDatabase = baseline.SqlDatabase,
            ModelEndpoint = baseline.ModelEndpoint,
            ModelDeployment = baseline.ModelDeployment,
            ProviderProfileHash = baseline.ProviderProfileHash,
            ModelProfileHash = baseline.ModelProfileHash,
            ModelCapabilityHash = baseline.ModelCapabilityHash,
            DeploymentHash = baseline.DeploymentHash,
            FoundryProjectEndpoint = baseline.FoundryProjectEndpoint,
            FoundryApiKey = baseline.FoundryApiKey,
            FoundryCredentialVersion = credentialVersion ?? baseline.FoundryCredentialVersion,
            ApplicationRevision = applicationRevision ?? baseline.ApplicationRevision,
            FoundryInventoryMode = baseline.FoundryInventoryMode,
            FoundryPrerequisiteEvidenceHash = baseline.FoundryPrerequisiteEvidenceHash,
            FoundryDeploymentRefreshSeconds = baseline.FoundryDeploymentRefreshSeconds,
            FoundryCatalogMaximumStaleSeconds = baseline.FoundryCatalogMaximumStaleSeconds,
            ModelProfileConfigurationPrefix = baseline.ModelProfileConfigurationPrefix,
        };
    }

    [Fact]
    public void Deployment_discovery_requests_only_the_documented_foundry_scope()
    {
        Assert.Equal(
            "https://ai.azure.com/.default",
            FoundryDeploymentDiscoveryClient.TokenScope);
    }

    [Fact]
    public void Deployment_discovery_transport_disables_redirects_and_bounds_decompression()
    {
        using ServiceProvider provider = BuildProvider();
        HttpClientFactoryOptions options = provider
            .GetRequiredService<IOptionsMonitor<HttpClientFactoryOptions>>()
            .Get(AzureClientRegistration.FoundryDiscoveryHttpClientName);
        CapturingHandlerBuilder builder = new();

        foreach (Action<HttpMessageHandlerBuilder> action in
            options.HttpMessageHandlerBuilderActions)
        {
            action(builder);
        }

        SocketsHttpHandler primary = Assert.IsType<SocketsHttpHandler>(builder.PrimaryHandler);
        // A redirect is the cheapest way to move a bearer token to another
        // origin, so the transport must never chase one.
        Assert.False(primary.AllowAutoRedirect);
        Assert.Equal(
            DecompressionMethods.GZip | DecompressionMethods.Deflate,
            primary.AutomaticDecompression);
        Assert.False(primary.UseCookies);
        Assert.False(primary.UseProxy);
        Assert.Equal(TimeSpan.FromSeconds(10), primary.ConnectTimeout);
        Assert.Equal(16, primary.MaxResponseHeadersLength);
    }

    private static ServiceProvider BuildProvider()
    {
        ServiceCollection services = new();
        services.AddProductionAzureClients(CreateValidatedOptions());
        return services.BuildServiceProvider();
    }

    private static ProductionOptions CreateValidatedOptions()
    {
        ProductionOptions options = ProductionCompositionTests.CreateProductionOptions();
        options.Validate(new SupportPlaneOptions
        {
            Authority =
                "https://login.microsoftonline.com/11111111-1111-1111-1111-111111111111/v2.0",
            Audience = "api://22222222-2222-2222-2222-222222222222",
            ApprovedDesktopClientId = "33333333-3333-3333-3333-333333333333",
            Region = "westeurope",
        });
        return options;
    }

    private sealed class CapturingHandlerBuilder : HttpMessageHandlerBuilder
    {
        public override string? Name { get; set; }

        public override HttpMessageHandler PrimaryHandler { get; set; } =
            new HttpClientHandler();

        public override IList<DelegatingHandler> AdditionalHandlers { get; } = [];

        public override HttpMessageHandler Build() => PrimaryHandler;
    }
}
