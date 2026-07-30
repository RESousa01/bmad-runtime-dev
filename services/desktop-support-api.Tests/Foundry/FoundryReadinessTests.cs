using System.Net;
using Microsoft.Extensions.Diagnostics.HealthChecks;
using Sapphirus.DesktopSupportApi.Configuration;
using Sapphirus.DesktopSupportApi.Foundry;
using Sapphirus.DesktopSupportApi.Health;
using Sapphirus.DesktopSupportApi.Model;
using Sapphirus.DesktopSupportApi.Tests.Model;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Foundry;

/// <summary>
/// FND-031. Readiness reports each Foundry dependency class separately and
/// truthfully, probes the Responses surface without ever issuing a model
/// completion, and never discloses an endpoint, a credential, or a provider
/// message.
/// </summary>
public sealed class FoundryReadinessTests
{
    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    private static readonly HealthCheckContext Context = new();

    [Fact]
    public void The_readiness_dependency_classes_are_a_fixed_reviewed_set()
    {
        Assert.Equal(
            [
                "foundry_project_configuration",
                "foundry_credential",
                "foundry_catalog",
                "foundry_effective_profiles",
                "foundry_responses",
            ],
            FoundryHealthChecks.All);
    }

    [Fact]
    public async Task A_valid_project_endpoint_configuration_is_healthy()
    {
        ProductionOptions options = ValidProduction();

        HealthCheckResult result = await new FoundryProjectConfigurationHealthCheck(options)
            .CheckHealthAsync(Context, Ct);

        Assert.Equal(HealthStatus.Healthy, result.Status);
        Assert.Null(result.Description);
        Assert.Empty(result.Data);
    }

    [Fact]
    public async Task A_missing_project_endpoint_is_unhealthy_without_naming_it()
    {
        ProductionOptions options = new();

        HealthCheckResult result = await new FoundryProjectConfigurationHealthCheck(options)
            .CheckHealthAsync(Context, Ct);

        Assert.Equal(HealthStatus.Unhealthy, result.Status);
        Assert.Null(result.Description);
    }

    [Fact]
    public async Task An_acquirable_credential_is_healthy_and_a_failing_one_is_not()
    {
        FakeTimeProvider clock = new(DateTimeOffset.UnixEpoch);

        HealthCheckResult healthy = await new FoundryCredentialHealthCheck(
            new FoundryResponsesTestFixtures.FakeFoundryAuthenticator(),
            clock).CheckHealthAsync(Context, Ct);
        HealthCheckResult unhealthy = await new FoundryCredentialHealthCheck(
            new FoundryResponsesTestFixtures.FailingFoundryAuthenticator(),
            clock).CheckHealthAsync(Context, Ct);

        Assert.Equal(HealthStatus.Healthy, healthy.Status);
        Assert.Equal(HealthStatus.Unhealthy, unhealthy.Status);
        Assert.Null(unhealthy.Description);
        Assert.Null(unhealthy.Exception);
    }

    [Fact]
    public async Task The_credential_probe_is_cached_so_polling_is_not_a_token_storm()
    {
        FakeTimeProvider clock = new(DateTimeOffset.UnixEpoch);
        FoundryResponsesTestFixtures.FakeFoundryAuthenticator authenticator = new();
        FoundryCredentialHealthCheck check = new(
            authenticator,
            clock,
            TimeSpan.FromSeconds(30));

        _ = await check.CheckHealthAsync(Context, Ct);
        _ = await check.CheckHealthAsync(Context, Ct);
        clock.Advance(TimeSpan.FromSeconds(31));
        _ = await check.CheckHealthAsync(Context, Ct);

        Assert.Equal(2, authenticator.OperationCount);
    }

    [Fact]
    public async Task A_registry_with_no_profile_is_unhealthy()
    {
        FoundryEffectiveProfilesHealthCheck check = new(new FakeRegistry(profileCount: 0));

        HealthCheckResult result = await check.CheckHealthAsync(Context, Ct);

        Assert.Equal(HealthStatus.Unhealthy, result.Status);
    }

    [Fact]
    public async Task A_signed_registry_with_profiles_is_healthy_and_a_fixture_one_degrades()
    {
        HealthCheckResult signed =
            await new FoundryEffectiveProfilesHealthCheck(new FakeRegistry(profileCount: 2))
                .CheckHealthAsync(Context, Ct);
        HealthCheckResult fixture = await new FoundryEffectiveProfilesHealthCheck(
                new FakeRegistry(profileCount: 2, ModelProfileTrust.DevelopmentFixtureUntrusted))
            .CheckHealthAsync(Context, Ct);

        Assert.Equal(HealthStatus.Healthy, signed.Status);
        Assert.Equal(HealthStatus.Degraded, fixture.Status);
    }

    [Theory]
    [InlineData(HttpStatusCode.BadRequest, HealthStatus.Healthy)]
    [InlineData(HttpStatusCode.UnprocessableEntity, HealthStatus.Healthy)]
    [InlineData(HttpStatusCode.TooManyRequests, HealthStatus.Degraded)]
    [InlineData(HttpStatusCode.Unauthorized, HealthStatus.Unhealthy)]
    [InlineData(HttpStatusCode.Forbidden, HealthStatus.Unhealthy)]
    [InlineData(HttpStatusCode.InternalServerError, HealthStatus.Unhealthy)]
    public void A_shape_rejection_is_the_healthy_probe_answer(
        HttpStatusCode statusCode,
        HealthStatus expected)
    {
        // The probe body is deliberately incomplete, so a rejection for shape
        // proves the request authenticated and routed while spending nothing.
        Assert.Equal(
            expected,
            FoundryResponsesReachabilityHealthCheck.Classify(statusCode));
    }

    [Fact]
    public async Task The_reachability_probe_issues_no_model_completion()
    {
        FakeFoundryTestClock serverClock = new(DateTimeOffset.UnixEpoch);
        using FakeFoundryServer server = new("Test", serverClock);
        using HttpClient client = FoundryResponsesTestFixtures.CreateResponsesClient(server);
        server.Enqueue(FakeFoundryReply.Json(
            HttpStatusCode.BadRequest,
            """{"error":{"code":"missing_required_parameter"}}"""));
        FoundryResponsesReachabilityHealthCheck check = new(
            client,
            new FoundryResponsesTestFixtures.FakeFoundryAuthenticator(),
            new FakeTimeProvider(DateTimeOffset.UnixEpoch));

        HealthCheckResult result = await check.CheckHealthAsync(Context, Ct);

        Assert.Equal(HealthStatus.Healthy, result.Status);
        FakeFoundryRequestReceipt receipt = Assert.Single(server.RequestReceipts);
        Assert.Equal("responses", receipt.Route);
        Assert.Equal("POST", receipt.Method);
        // The probe body is the empty object and nothing else.
        Assert.Equal("{}", FoundryResponsesReachabilityHealthCheck.ProbeBody);
    }

    [Fact]
    public async Task The_reachability_probe_is_cached_so_polling_is_not_provider_load()
    {
        FakeFoundryTestClock serverClock = new(DateTimeOffset.UnixEpoch);
        using FakeFoundryServer server = new("Test", serverClock);
        using HttpClient client = FoundryResponsesTestFixtures.CreateResponsesClient(server);
        FakeTimeProvider clock = new(DateTimeOffset.UnixEpoch);
        server.Enqueue(FakeFoundryReply.Json(HttpStatusCode.BadRequest, """{"error":{}}"""));
        server.Enqueue(FakeFoundryReply.Json(HttpStatusCode.BadRequest, """{"error":{}}"""));
        FoundryResponsesReachabilityHealthCheck check = new(
            client,
            new FoundryResponsesTestFixtures.FakeFoundryAuthenticator(),
            clock,
            TimeSpan.FromMinutes(1));

        _ = await check.CheckHealthAsync(Context, Ct);
        _ = await check.CheckHealthAsync(Context, Ct);
        _ = await check.CheckHealthAsync(Context, Ct);

        Assert.Single(server.RequestReceipts);
    }

    [Fact]
    public async Task An_unreachable_project_never_reports_a_provider_message()
    {
        FakeFoundryTestClock serverClock = new(DateTimeOffset.UnixEpoch);
        using FakeFoundryServer server = new("Test", serverClock);
        using HttpClient client = FoundryResponsesTestFixtures.CreateResponsesClient(server);
        FoundryResponsesReachabilityHealthCheck check = new(
            client,
            new FoundryResponsesTestFixtures.FailingFoundryAuthenticator(),
            new FakeTimeProvider(DateTimeOffset.UnixEpoch));

        HealthCheckResult result = await check.CheckHealthAsync(Context, Ct);

        Assert.Equal(HealthStatus.Unhealthy, result.Status);
        Assert.Null(result.Description);
        Assert.Null(result.Exception);
        Assert.Empty(server.RequestReceipts);
    }

    private static ProductionOptions ValidProduction()
    {
        ProductionOptions options = new()
        {
            ManagedIdentityClientId = Guid.Parse("44444444-4444-4444-4444-444444444444"),
            SqlMigrationIdentityClientId = Guid.Parse("55555555-5555-5555-5555-555555555555"),
            AppConfigurationEndpoint = "https://configuration.azconfig.io/",
            KeyVaultUri = "https://signing.vault.azure.net/",
            ReceiptSigningKeyName = "model-receipt-signing",
            SqlServer = "authority.database.windows.net",
            SqlDatabase = "desktop-authority",
            FoundryProjectEndpoint =
                "https://account-example.services.ai.azure.com/api/projects/_project/",
            FoundryApiKey = "test-foundry-project-key-0001",
            FoundryPrerequisiteEvidenceHash = FoundryResponsesTestFixtures.Hash,
            ProviderProfileHash = FoundryResponsesTestFixtures.Hash,
            ModelProfileHash = FoundryResponsesTestFixtures.Hash,
            ModelCapabilityHash = FoundryResponsesTestFixtures.Hash,
        };
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

    private sealed class FakeRegistry(
        int profileCount,
        ModelProfileTrust trust = ModelProfileTrust.Signed) : IModelProfileRegistry
    {
        public ValueTask<ModelProfileRegistrySnapshot> GetSnapshotAsync(
            CancellationToken cancellationToken)
        {
            cancellationToken.ThrowIfCancellationRequested();
            List<ModelProfile> profiles = [];
            for (int index = 0; index < profileCount; index++)
            {
                profiles.Add(BuildProfile(index));
            }
            return ValueTask.FromResult(new ModelProfileRegistrySnapshot(
                1,
                FoundryResponsesTestFixtures.Hash,
                FoundryResponsesTestFixtures.OtherHash,
                profiles,
                trust,
                DateTimeOffset.UnixEpoch));
        }

        private static ModelProfile BuildProfile(int index) => new(
            ModelProfile.SchemaVersionValue,
            $"profile-{index}",
            "Profile",
            "A fixture profile.",
            null,
            index,
            Enabled: true,
            Visible: true,
            "desktop-planning",
            new ModelProfileExpectedModel("OpenAI", "gpt-5-mini", "2026-06-01"),
            ["bmad_help"],
            ["planner"],
            [ModelRequestModes.StreamingChat],
            [],
            ["text"],
            new ModelProfileCapabilities(
                TextInput: true,
                ImageInput: false,
                PdfInput: false,
                Attachments: false,
                ToolCalling: false,
                StructuredOutput: false,
                Reasoning: false,
                Streaming: true),
            new ModelProfileLimits(200_000, 100_000, 4_096, 0, 262_144),
            [new ModelProfileVariant("balanced", "Balanced", "medium", "low", 4_096)],
            new ModelProfileRetryPolicy(3, 100, 1_000),
            "transient_no_store",
            ["westeurope"],
            new ModelProfileQualification(
                ModelQualificationStatus.Qualified,
                FoundryResponsesTestFixtures.Hash,
                DateTimeOffset.UnixEpoch),
            FoundryResponsesTestFixtures.PriceProfile());
    }

    private sealed class FakeTimeProvider(DateTimeOffset initialValue) : TimeProvider
    {
        private DateTimeOffset _now = initialValue;

        public override DateTimeOffset GetUtcNow() => _now;

        public void Advance(TimeSpan duration) => _now = _now.Add(duration);
    }
}
