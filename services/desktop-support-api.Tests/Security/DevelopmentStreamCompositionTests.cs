using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.FileProviders;
using Sapphirus.DesktopSupportApi.Configuration;
using Sapphirus.DesktopSupportApi.Foundry;
using Sapphirus.DesktopSupportApi.Model;
using Sapphirus.DesktopSupportApi.Observability;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Security;

/// <summary>
/// Development is the only composition that can ever reach the operator's real
/// Foundry project, because managed identity is unobtainable on that machine.
/// These tests pin that a development run actually composes the streaming
/// surface, and that composing it did not smuggle the production credential
/// posture in with it.
/// </summary>
public sealed class DevelopmentStreamCompositionTests
{
    private const string ProjectEndpoint =
        "https://account.services.ai.azure.com/api/projects/forge-project";

    /// <summary>
    /// A synthetic credential in the allowlisted canary spelling. It is not a
    /// key and must never be treated as one.
    /// </summary>
    private const string FakeCredential = "must-not-cross-development-credential";

    [Fact]
    public void A_development_composition_builds_a_streaming_coordinator()
    {
        ServiceCollection services = CreateDevelopmentServices();

        using ServiceProvider provider = services.BuildServiceProvider();
        ModelStreamingCoordinator coordinator =
            provider.GetRequiredService<ModelStreamingCoordinator>();

        // Without this the streaming route answered foundry_project_unavailable
        // no matter how the project was configured.
        Assert.NotNull(coordinator);
        Assert.Contains(
            services,
            descriptor => descriptor.ServiceType == typeof(IModelStreamRouteResolver));
    }

    /// <summary>
    /// The mechanism the coordinator reports is the mechanism the transport will
    /// use, and it is folded into every receipt's credential-binding hash. A
    /// development run therefore cannot mint a receipt that hashes like a
    /// managed-identity one.
    /// </summary>
    [Fact]
    public void The_composed_coordinator_reports_the_development_credential_mechanism()
    {
        using ServiceProvider provider = CreateDevelopmentServices().BuildServiceProvider();

        ModelStreamingCoordinator coordinator =
            provider.GetRequiredService<ModelStreamingCoordinator>();

        Assert.Equal(
            FoundryCredentialMechanisms.DevelopmentApiKey,
            coordinator.CredentialMechanismId);
        Assert.Equal(
            FoundryCredentialMechanisms.DevelopmentApiKey,
            provider.GetRequiredService<IFoundryRequestAuthenticator>()
                .CredentialMechanismId);
    }

    /// <summary>
    /// The development composition is still refused in a production graph, with
    /// the streaming registrations present. Adding a coordinator to it did not
    /// make it composable anywhere else.
    /// </summary>
    [Fact]
    public void A_production_graph_still_refuses_the_development_streaming_composition()
    {
        ServiceCollection services = CreateDevelopmentServices();

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
    public void The_streaming_composition_cannot_be_added_outside_development(
        string environmentName)
    {
        ServiceCollection services = new();

        InvalidOperationException error = Assert.Throws<InvalidOperationException>(
            () => services.AddDevelopmentFoundryCatalog(
                new StubHostEnvironment(environmentName),
                CreateConfiguration()));

        Assert.Equal(
            "development_foundry_composition_environment_forbidden",
            error.Message);
        Assert.DoesNotContain(
            services,
            descriptor => descriptor.ServiceType == typeof(ModelStreamingCoordinator));
    }

    /// <summary>
    /// The two dependencies the coordinator has that Program.cs supplies in the
    /// Development branch, plus the catalog composition itself. Nothing else
    /// from Program.cs is needed to prove the streaming surface is wired.
    /// </summary>
    private static ServiceCollection CreateDevelopmentServices()
    {
        ServiceCollection services = new();
        SupportPlaneOptions options = new()
        {
            Authority = "https://login.microsoftonline.com/"
                + "11111111-1111-1111-1111-111111111111/v2.0",
            Audience = "api://66666666-6666-6666-6666-666666666666",
            ApprovedDesktopClientId = "22222222-2222-2222-2222-222222222222",
            Region = "westeurope",
            DevelopmentSigningEnabled = true,
        };
        services.AddSingleton(options);
        services.AddSingleton<SupportPlaneTelemetry>();
        services.AddSingleton<IModelReceiptSignerV2, DevelopmentModelReceiptSignerV2>();
        services.AddDevelopmentFoundryCatalog(
            new StubHostEnvironment("Development"),
            CreateConfiguration());
        return services;
    }

    private static IConfiguration CreateConfiguration() =>
        new ConfigurationBuilder()
            .AddInMemoryCollection(new Dictionary<string, string?>
            {
                [DevelopmentFoundryComposition.ProjectEndpointConfigurationKey] =
                    ProjectEndpoint,
                [DevelopmentFoundryComposition.ApiKeyConfigurationKey] = FakeCredential,
            })
            .Build();

    private sealed class StubHostEnvironment(string environmentName) : IHostEnvironment
    {
        public string EnvironmentName { get; set; } = environmentName;
        public string ApplicationName { get; set; } = "Sapphirus.DesktopSupportApi.Tests";
        public string ContentRootPath { get; set; } = AppContext.BaseDirectory;
        public IFileProvider ContentRootFileProvider { get; set; } =
            new NullFileProvider();
    }
}
