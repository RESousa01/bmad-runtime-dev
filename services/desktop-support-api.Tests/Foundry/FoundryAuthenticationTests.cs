using System.Net;
using System.Text;
using System.Text.Json;
using Azure.Core;
using Microsoft.AspNetCore.Http;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Diagnostics.HealthChecks;
using Microsoft.Extensions.FileProviders;
using Microsoft.Extensions.Hosting;
using Sapphirus.DesktopSupportApi.Configuration;
using Sapphirus.DesktopSupportApi.Foundry;
using Sapphirus.DesktopSupportApi.Health;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Foundry;

/// <summary>
/// The development Foundry credential extension.
///
/// The operator has neither Entra app-registration rights nor Azure RBAC
/// role-assignment rights, so managed identity is unobtainable rather than
/// merely unsupplied. Only the credential mechanism is swapped; the whole
/// architecture below it stays. These cases prove the swap is structurally
/// unreachable outside Development and Test, that the key never escapes, and
/// that the mechanism is visible in evidence.
/// </summary>
public sealed class FoundryAuthenticationTests
{
    private const string FakeApiKey = "forge-development-api-key-must-not-leak";
    private const string FakeToken = "forge-foundry-fake-marker-v1";
    private static readonly Uri ProjectEndpoint =
        new("https://forge-fake.test/api/projects/forge-test/");
    private const string ConfiguredProjectEndpoint =
        "https://account.services.ai.azure.com/api/projects/forge-project";

    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    [Theory]
    [InlineData("")]
    [InlineData("Staging")]
    [InlineData("Production")]
    public void Development_foundry_credential_rejects_every_non_development_environment(
        string environmentName)
    {
        InvalidOperationException error = Assert.Throws<InvalidOperationException>(
            () => new DevelopmentFoundryApiKeyAuthenticator(environmentName, FakeApiKey));

        Assert.Equal(
            "development_foundry_credential_environment_forbidden",
            error.Message);
    }

    [Fact]
    public void Development_foundry_credential_rejects_an_absent_environment_name()
    {
        InvalidOperationException error = Assert.Throws<InvalidOperationException>(
            () => new DevelopmentFoundryApiKeyAuthenticator(null, FakeApiKey));

        Assert.Equal(
            "development_foundry_credential_environment_forbidden",
            error.Message);
    }

    [Theory]
    [InlineData("Development")]
    [InlineData("Test")]
    public void Development_foundry_credential_allows_only_development_and_test(
        string environmentName)
    {
        DevelopmentFoundryApiKeyAuthenticator authenticator = new(
            environmentName,
            FakeApiKey);

        Assert.Equal("development_api_key", authenticator.CredentialMechanismId);
    }

    [Theory]
    [InlineData(null)]
    [InlineData("")]
    [InlineData("   ")]
    [InlineData("short")]
    // A blank-padded, newline-carrying, or non-ASCII key would either split the
    // request or make the header library raise an exception quoting the key.
    [InlineData("has a space in it")]
    [InlineData("has\ta\ttab\tin\tit")]
    [InlineData("injects\r\nX-Forwarded-For: 10.0.0.1")]
    [InlineData("não-ascii-key-value-here")]
    public void Development_foundry_credential_fails_closed_on_an_unusable_key(string? keyValue)
    {
        InvalidOperationException error = Assert.Throws<InvalidOperationException>(
            () => new DevelopmentFoundryApiKeyAuthenticator("Development", keyValue));

        Assert.Equal("development_foundry_api_key_invalid", error.Message);
    }

    [Fact]
    public void A_rejected_key_is_never_quoted_back_in_the_failure()
    {
        const string rejected = "rejected key value must not leak";

        InvalidOperationException error = Assert.Throws<InvalidOperationException>(
            () => new DevelopmentFoundryApiKeyAuthenticator("Development", rejected));

        Assert.DoesNotContain(rejected, error.ToString(), StringComparison.Ordinal);
    }

    [Fact]
    public void Development_foundry_credential_rejects_an_unbounded_key()
    {
        string oversized = new(
            'k',
            DevelopmentFoundryApiKeyAuthenticator.MaximumApiKeyLength + 1);

        InvalidOperationException error = Assert.Throws<InvalidOperationException>(
            () => new DevelopmentFoundryApiKeyAuthenticator("Development", oversized));

        Assert.Equal("development_foundry_api_key_invalid", error.Message);
    }

    [Fact]
    public async Task Development_foundry_credential_sends_an_api_key_header_and_never_a_bearer_token()
    {
        DevelopmentFoundryApiKeyAuthenticator authenticator = new("Test", FakeApiKey);
        using HttpRequestMessage request = new(HttpMethod.Get, ProjectEndpoint);

        await authenticator.ApplyAsync(request, Ct);

        Assert.Null(request.Headers.Authorization);
        Assert.True(request.Headers.TryGetValues("api-key", out IEnumerable<string>? values));
        Assert.Equal(FakeApiKey, Assert.Single(values!));
        Assert.DoesNotContain(
            request.Headers,
            header => string.Equals(
                header.Key,
                "Authorization",
                StringComparison.OrdinalIgnoreCase));
    }

    [Fact]
    public async Task Managed_identity_credential_sends_a_bearer_token_for_the_documented_scope()
    {
        ScriptedCredential credential = new(FakeToken);
        ManagedIdentityFoundryAuthenticator authenticator = new(credential);
        using HttpRequestMessage request = new(HttpMethod.Get, ProjectEndpoint);

        await authenticator.ApplyAsync(request, Ct);

        Assert.Equal("managed_identity", authenticator.CredentialMechanismId);
        Assert.Equal("Bearer", request.Headers.Authorization?.Scheme);
        Assert.Equal(FakeToken, request.Headers.Authorization?.Parameter);
        Assert.False(request.Headers.Contains("api-key"));
        Assert.Equal(
            ["https://ai.azure.com/.default"],
            Assert.Single(credential.RequestedScopes));
    }

    [Fact]
    public async Task Managed_identity_credential_acquires_one_token_for_a_whole_operation()
    {
        ScriptedCredential credential = new(FakeToken);
        ManagedIdentityFoundryAuthenticator authenticator = new(credential);

        IFoundryRequestAuthenticator operation = await authenticator.BeginOperationAsync(Ct);
        using HttpRequestMessage first = new(HttpMethod.Get, ProjectEndpoint);
        using HttpRequestMessage second = new(HttpMethod.Get, ProjectEndpoint);
        await operation.ApplyAsync(first, Ct);
        await operation.ApplyAsync(second, Ct);

        Assert.Equal(1, credential.TokenRequests);
        Assert.Equal(FakeToken, first.Headers.Authorization?.Parameter);
        Assert.Equal(FakeToken, second.Headers.Authorization?.Parameter);
        Assert.Equal("managed_identity", operation.CredentialMechanismId);
    }

    [Fact]
    public async Task Managed_identity_credential_fails_closed_on_an_empty_token()
    {
        ManagedIdentityFoundryAuthenticator authenticator = new(new ScriptedCredential(""));

        InvalidOperationException error =
            await Assert.ThrowsAsync<InvalidOperationException>(async () =>
                await authenticator.BeginOperationAsync(Ct));

        Assert.Equal("foundry_managed_identity_token_unavailable", error.Message);
    }

    [Fact]
    public void Credential_mechanism_identifiers_are_distinct_and_stable()
    {
        Assert.Equal("managed_identity", FoundryCredentialMechanisms.ManagedIdentity);
        Assert.Equal("development_api_key", FoundryCredentialMechanisms.DevelopmentApiKey);
        Assert.NotEqual(
            FoundryCredentialMechanisms.ManagedIdentity,
            FoundryCredentialMechanisms.DevelopmentApiKey);
    }

    [Fact]
    public void The_discovery_client_reports_the_mechanism_it_was_bound_to()
    {
        using FakeFoundryServer server = new("Test", new FakeFoundryTestClock(DateTimeOffset.UnixEpoch));
        using HttpClient httpClient = new(server, disposeHandler: false);

        FoundryDeploymentDiscoveryClient development = new(
            httpClient,
            new DevelopmentFoundryApiKeyAuthenticator("Test", FakeApiKey),
            ProjectEndpoint,
            TimeProvider.System);
        FoundryDeploymentDiscoveryClient managedIdentity = new(
            httpClient,
            new ScriptedCredential(FakeToken),
            ProjectEndpoint,
            TimeProvider.System);

        Assert.Equal("development_api_key", development.CredentialMechanismId);
        Assert.Equal("managed_identity", managedIdentity.CredentialMechanismId);
    }

    [Fact]
    public async Task Deployment_discovery_under_the_development_credential_sends_only_the_api_key_header()
    {
        FakeFoundryTestClock clock = new(DateTimeOffset.UnixEpoch);
        using FakeFoundryServer server = new("Test", clock);
        using HttpClient httpClient = new(server, disposeHandler: false);
        FoundryDeploymentDiscoveryClient client = new(
            httpClient,
            new DevelopmentFoundryApiKeyAuthenticator("Test", FakeApiKey),
            ProjectEndpoint,
            TimeProvider.System);
        server.Enqueue(FakeFoundryReply.Json(HttpStatusCode.OK, """{"value":[]}"""));

        FoundryDiscoveryResult result = await client.DiscoverAsync(Ct);

        FakeFoundryRequestReceipt receipt = Assert.Single(server.RequestReceipts);
        Assert.Contains("api-key", receipt.HeaderNames);
        Assert.DoesNotContain("Authorization", receipt.HeaderNames);
        Assert.Contains("x-ms-client-request-id", receipt.HeaderNames);
        // The fake server only honours the synthetic bearer marker, so this is
        // a clean 401. What matters is the header shape that reached it, and
        // that the evidence still carries no credential.
        Assert.Equal(FoundryDiscoveryOutcome.Authorization, result.Outcome);
        Assert.DoesNotContain(
            FakeApiKey,
            JsonSerializer.Serialize(result.Evidence),
            StringComparison.Ordinal);
    }

    [Fact]
    public void Production_composition_resolves_the_backend_api_key_authenticator()
    {
        ServiceCollection services = new();
        services.AddProductionAzureClients(CreateValidatedProductionOptions());

        using ServiceProvider provider = services.BuildServiceProvider();
        IFoundryRequestAuthenticator authenticator =
            provider.GetRequiredService<IFoundryRequestAuthenticator>();

        Assert.IsType<FoundryApiKeyAuthenticator>(authenticator);
        Assert.Equal(FoundryCredentialMechanisms.ApiKey, authenticator.CredentialMechanismId);
        Assert.Equal(
            FoundryCredentialMechanisms.ApiKey,
            provider.GetRequiredService<FoundryDeploymentDiscoveryClient>()
                .CredentialMechanismId);
    }

    [Fact]
    public void Production_composition_never_registers_the_development_credential()
    {
        ServiceCollection services = new();
        services.AddProductionAzureClients(CreateValidatedProductionOptions());

        ProductionComposition.AssertNoDevelopmentFoundryCredential(services);

        Assert.DoesNotContain(
            services,
            descriptor =>
                descriptor.ServiceType == typeof(DevelopmentFoundryApiKeyAuthenticator)
                || descriptor.ImplementationType
                    == typeof(DevelopmentFoundryApiKeyAuthenticator)
                || descriptor.ImplementationInstance
                    is DevelopmentFoundryApiKeyAuthenticator);
    }

    [Fact]
    public void The_production_guard_refuses_a_graph_carrying_the_development_credential()
    {
        ServiceCollection services = new();
        services.AddProductionAzureClients(CreateValidatedProductionOptions());
        services.AddSingleton(new DevelopmentFoundryApiKeyAuthenticator("Test", FakeApiKey));

        InvalidOperationException error = Assert.Throws<InvalidOperationException>(
            () => ProductionComposition.AssertNoDevelopmentFoundryCredential(services));

        Assert.Equal(
            "production_foundry_development_credential_forbidden",
            error.Message);
    }

    [Theory]
    [InlineData("")]
    [InlineData("Staging")]
    [InlineData("Production")]
    public void Development_foundry_composition_refuses_every_non_development_environment(
        string environmentName)
    {
        ServiceCollection services = new();

        InvalidOperationException error = Assert.Throws<InvalidOperationException>(
            () => services.AddDevelopmentFoundryCatalog(
                new StubHostEnvironment(environmentName),
                CreateDevelopmentConfiguration()));

        Assert.Equal(
            "development_foundry_composition_environment_forbidden",
            error.Message);
    }

    [Fact]
    public void Development_foundry_composition_wires_the_api_key_mechanism_and_the_catalog_path()
    {
        ServiceCollection services = new();

        services.AddDevelopmentFoundryCatalog(
            new StubHostEnvironment("Development"),
            CreateDevelopmentConfiguration());

        using ServiceProvider provider = services.BuildServiceProvider();
        IFoundryRequestAuthenticator authenticator =
            provider.GetRequiredService<IFoundryRequestAuthenticator>();
        Assert.IsType<DevelopmentFoundryApiKeyAuthenticator>(authenticator);
        Assert.Equal("development_api_key", authenticator.CredentialMechanismId);
        Assert.Equal(
            "development_api_key",
            provider.GetRequiredService<FoundryDeploymentDiscoveryClient>()
                .CredentialMechanismId);
        // The architecture below the credential is unchanged: the same
        // discovery seam, the same catalog cache, the same health check.
        Assert.NotNull(provider.GetRequiredService<IFoundryDeploymentDiscovery>());
        Assert.NotNull(provider.GetRequiredService<FoundryDeploymentCatalogCache>());
        Assert.NotNull(provider.GetRequiredService<FoundryCatalogHealthCheck>());
        using HttpClient client = provider
            .GetRequiredService<IHttpClientFactory>()
            .CreateClient(AzureClientRegistration.FoundryDiscoveryHttpClientName);
        Assert.Equal(
            "https://account.services.ai.azure.com/api/projects/forge-project/",
            client.BaseAddress?.AbsoluteUri);
        Assert.Null(client.DefaultRequestHeaders.Authorization);
        Assert.False(client.DefaultRequestHeaders.Contains("api-key"));
    }

    [Fact]
    public void Development_foundry_composition_rejects_an_untrusted_project_endpoint()
    {
        ServiceCollection services = new();

        InvalidOperationException error = Assert.Throws<InvalidOperationException>(
            () => services.AddDevelopmentFoundryCatalog(
                new StubHostEnvironment("Development"),
                CreateDevelopmentConfiguration(
                    projectEndpoint: "https://account.services.ai.azure.com/")));

        Assert.Equal("development_foundry_project_endpoint_invalid", error.Message);
    }

    [Fact]
    public void Development_foundry_mode_is_off_until_the_operator_configures_both_values()
    {
        ServiceCollection services = new();

        bool composed = services.TryAddDevelopmentFoundryCatalog(
            new StubHostEnvironment("Development"),
            CreateDevelopmentConfiguration(projectEndpoint: "", keyValue: ""));

        Assert.False(composed);
        Assert.Empty(services);
    }

    [Theory]
    [InlineData(ConfiguredProjectEndpoint, "")]
    [InlineData("", FakeApiKey)]
    public void A_half_configured_development_foundry_mode_is_a_configuration_error(
        string projectEndpoint,
        string keyValue)
    {
        ServiceCollection services = new();

        InvalidOperationException error = Assert.Throws<InvalidOperationException>(
            () => services.TryAddDevelopmentFoundryCatalog(
                new StubHostEnvironment("Development"),
                CreateDevelopmentConfiguration(projectEndpoint, keyValue)));

        Assert.Equal("development_foundry_configuration_incomplete", error.Message);
    }

    [Fact]
    public async Task The_development_api_key_never_reaches_serialized_state_or_a_health_response()
    {
        ServiceCollection services = new();
        services.TryAddDevelopmentFoundryCatalog(
            new StubHostEnvironment("Development"),
            CreateDevelopmentConfiguration());
        using ServiceProvider provider = services.BuildServiceProvider();
        DevelopmentFoundryApiKeyAuthenticator authenticator =
            provider.GetRequiredService<DevelopmentFoundryApiKeyAuthenticator>();
        FoundryDeploymentCatalogCache cache =
            provider.GetRequiredService<FoundryDeploymentCatalogCache>();

        HealthCheckResult health = await provider
            .GetRequiredService<FoundryCatalogHealthCheck>()
            .CheckHealthAsync(new HealthCheckContext(), Ct);
        string healthBody = await RenderHealthResponseAsync(health);

        Assert.DoesNotContain(
            FakeApiKey,
            JsonSerializer.Serialize(authenticator),
            StringComparison.Ordinal);
        Assert.DoesNotContain(FakeApiKey, authenticator.ToString(), StringComparison.Ordinal);
        Assert.DoesNotContain(
            FakeApiKey,
            JsonSerializer.Serialize(cache.Peek()),
            StringComparison.Ordinal);
        Assert.DoesNotContain(FakeApiKey, healthBody, StringComparison.Ordinal);
        Assert.Equal(
            """{"status":"unhealthy","dependencies":[{"dependency":"foundry_catalog","status":"unhealthy"}]}""",
            healthBody);
        foreach (ServiceDescriptor descriptor in services)
        {
            Assert.DoesNotContain(
                FakeApiKey,
                descriptor.ImplementationInstance?.ToString() ?? "",
                StringComparison.Ordinal);
            Assert.DoesNotContain(
                FakeApiKey,
                descriptor.ServiceType.FullName ?? "",
                StringComparison.Ordinal);
        }
    }

    [Fact]
    public void The_development_api_key_is_not_committed_in_any_appsettings_file()
    {
        foreach (string path in Directory.EnumerateFiles(
            RepositoryServicePath(),
            "appsettings*.json"))
        {
            using JsonDocument document = JsonDocument.Parse(File.ReadAllBytes(path));
            if (!document.RootElement.TryGetProperty("Sapphirus", out JsonElement section)
                || !section.TryGetProperty(
                    "FoundryDevelopmentApiKey",
                    out JsonElement configured))
            {
                continue;
            }
            Assert.Equal("", configured.GetString());
        }
    }

    private static ProductionOptions CreateValidatedProductionOptions()
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

    private static IConfiguration CreateDevelopmentConfiguration(
        string projectEndpoint = ConfiguredProjectEndpoint,
        string keyValue = FakeApiKey) =>
        new ConfigurationBuilder()
            .AddInMemoryCollection(new Dictionary<string, string?>(StringComparer.Ordinal)
            {
                ["Sapphirus:FoundryProjectEndpoint"] = projectEndpoint,
                ["Sapphirus:FoundryDevelopmentApiKey"] = keyValue,
            })
            .Build();

    private static async Task<string> RenderHealthResponseAsync(HealthCheckResult result)
    {
        DefaultHttpContext context = new();
        using MemoryStream body = new();
        context.Response.Body = body;
        HealthReport report = new(
            new Dictionary<string, HealthReportEntry>(StringComparer.Ordinal)
            {
                [FoundryCatalogHealthCheck.FoundryCatalogDependency] = new(
                    result.Status,
                    result.Description,
                    TimeSpan.Zero,
                    result.Exception,
                    result.Data),
            },
            TimeSpan.Zero);

        await AzureDependencyHealthChecks.WriteSafeResponseAsync(context, report);

        return Encoding.UTF8.GetString(body.ToArray());
    }

    /// <summary>
    /// Walks up from the test binaries to the support API project directory.
    /// </summary>
    private static string RepositoryServicePath()
    {
        DirectoryInfo? directory = new(AppContext.BaseDirectory);
        while (directory is not null)
        {
            string candidate = Path.Combine(
                directory.FullName,
                "services",
                "desktop-support-api");
            if (Directory.Exists(candidate))
            {
                return candidate;
            }
            directory = directory.Parent;
        }
        throw new InvalidOperationException("support_api_project_directory_missing");
    }

    private sealed class StubHostEnvironment(string environmentName) : IHostEnvironment
    {
        public string EnvironmentName { get; set; } = environmentName;

        public string ApplicationName { get; set; } = "Tests";

        public string ContentRootPath { get; set; } = AppContext.BaseDirectory;

        public IFileProvider ContentRootFileProvider { get; set; } = new NullFileProvider();
    }

    private sealed class ScriptedCredential(string token) : TokenCredential
    {
        private int _tokenRequests;
        private readonly List<string[]> _requestedScopes = [];

        public int TokenRequests => Volatile.Read(ref _tokenRequests);

        public IReadOnlyList<string[]> RequestedScopes
        {
            get
            {
                lock (_requestedScopes)
                {
                    return _requestedScopes.ToArray();
                }
            }
        }

        public override AccessToken GetToken(
            TokenRequestContext requestContext,
            CancellationToken cancellationToken)
        {
            Interlocked.Increment(ref _tokenRequests);
            lock (_requestedScopes)
            {
                _requestedScopes.Add(requestContext.Scopes);
            }
            return new AccessToken(token, DateTimeOffset.MaxValue);
        }

        public override ValueTask<AccessToken> GetTokenAsync(
            TokenRequestContext requestContext,
            CancellationToken cancellationToken) =>
            new(GetToken(requestContext, cancellationToken));
    }
}
