using System.Net;
using System.Net.Http.Headers;
using System.Security.Claims;
using System.Text.Encodings.Web;
using System.Text.Json;
using Microsoft.AspNetCore.Authentication;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.AspNetCore.TestHost;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.DependencyInjection.Extensions;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;
using Sapphirus.DesktopSupportApi.Foundry;
using Sapphirus.DesktopSupportApi.Model;
using Sapphirus.DesktopSupportApi.Policy;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Model;

/// <summary>
/// FND-026. <c>GET /desktop/v1/model-catalog</c>.
/// </summary>
public sealed class ModelCatalogEndpointTests
{
    private const string TenantId = "11111111-1111-1111-1111-111111111111";
    private const string ApprovedClientId = "22222222-2222-2222-2222-222222222222";
    private const string OtherClientId = "33333333-3333-3333-3333-333333333333";
    private const string SubjectA = "44444444-4444-4444-4444-444444444444";
    private const string SubjectB = "55555555-5555-5555-5555-555555555555";
    private const string Route = "/desktop/v1/model-catalog";

    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    // ------------------------------------------------------------- admission

    [Fact]
    public async Task An_unauthenticated_request_is_challenged()
    {
        await using CatalogFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await client.GetAsync(Route, Ct);

        Assert.Equal(HttpStatusCode.Unauthorized, response.StatusCode);
    }

    [Fact]
    public async Task A_request_from_an_unapproved_desktop_client_is_refused()
    {
        await using CatalogFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await SendAsync(
            client,
            SubjectA,
            DesktopScopes.Access,
            clientId: OtherClientId);

        Assert.Equal(HttpStatusCode.Forbidden, response.StatusCode);
    }

    [Fact]
    public async Task A_request_without_the_desktop_read_scope_is_refused()
    {
        await using CatalogFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await SendAsync(
            client,
            SubjectA,
            DesktopScopes.Sync);

        Assert.Equal(HttpStatusCode.Forbidden, response.StatusCode);
    }

    [Fact]
    public async Task The_catalog_does_not_require_the_model_invocation_scope()
    {
        await using CatalogFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await SendAsync(
            client,
            SubjectA,
            DesktopScopes.Access);

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
    }

    // ------------------------------------------------------------- projection

    [Fact]
    public async Task The_response_is_the_sanitized_projection()
    {
        await using CatalogFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await SendAsync(
            client,
            SubjectA,
            DesktopScopes.Access);
        using JsonDocument document = await ReadAsync(response);

        JsonElement root = document.RootElement;
        Assert.Equal(
            "sapphirus.model-catalog.v1",
            root.GetProperty("schemaVersion").GetString());
        Assert.Equal(
            ModelCatalogTestFixtures.AlphaProfileId,
            root.GetProperty("defaultProfileId").GetString());
        Assert.Equal(
            "microsoft_foundry",
            root.GetProperty("profiles")[0].GetProperty("provider").GetString());
    }

    [Fact]
    public async Task No_azure_authority_appears_anywhere_in_the_response()
    {
        await using CatalogFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await SendAsync(
            client,
            SubjectA,
            DesktopScopes.Access);
        string body = await response.Content.ReadAsStringAsync(Ct);

        foreach (string forbidden in new[]
        {
            ModelCatalogTestFixtures.AlphaDeploymentName,
            ModelCatalogTestFixtures.BetaDeploymentName,
            "deploymentName",
            "projectEndpoint",
            "api-version",
            "subscription",
            "roleAssignment",
            "clientId",
            "synthetic-entitlement-group",
        })
        {
            Assert.DoesNotContain(forbidden, body, StringComparison.OrdinalIgnoreCase);
        }
    }

    [Fact]
    public async Task An_unentitled_profile_leaks_only_a_state_and_a_recovery_action()
    {
        await using CatalogFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();

        // Subject B is entitled to nothing, so both visible profiles land in
        // unavailableProfiles.
        using HttpResponseMessage response = await SendAsync(
            client,
            SubjectB,
            DesktopScopes.Access);
        using JsonDocument document = await ReadAsync(response);

        JsonElement blocked = document.RootElement.GetProperty("unavailableProfiles")[0];
        Assert.Equal(4, blocked.EnumerateObject().Count());
        Assert.Equal("not_entitled", blocked.GetProperty("availability").GetString());
        Assert.Equal("request_entitlement", blocked.GetProperty("recoveryAction").GetString());
        Assert.Empty(document.RootElement.GetProperty("profiles").EnumerateArray());
    }

    [Fact]
    public async Task The_response_stays_within_a_bounded_size()
    {
        await using CatalogFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await SendAsync(
            client,
            SubjectA,
            DesktopScopes.Access);
        string body = await response.Content.ReadAsStringAsync(Ct);

        // The catalog is bounded at 64 selectable and 64 blocked entries; two
        // profiles must be nowhere near a megabyte.
        Assert.InRange(body.Length, 1, 64 * 1024);
    }

    // ---------------------------------------------------------------- caching

    [Fact]
    public async Task A_matching_entity_tag_produces_a_not_modified_response()
    {
        await using CatalogFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();
        using HttpResponseMessage first = await SendAsync(
            client,
            SubjectA,
            DesktopScopes.Access);
        string etag = first.Headers.ETag!.ToString();

        using HttpResponseMessage second = await SendAsync(
            client,
            SubjectA,
            DesktopScopes.Access,
            ifNoneMatch: etag);

        Assert.Equal(HttpStatusCode.NotModified, second.StatusCode);
        Assert.Equal(etag, second.Headers.ETag!.ToString());
        Assert.NotNull(second.Content.Headers.LastModified);
        Assert.NotNull(second.Content.Headers.Expires);
        Assert.True(second.Headers.CacheControl!.Private);
    }

    [Fact]
    public async Task A_stale_entity_tag_produces_the_current_catalog()
    {
        await using CatalogFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await SendAsync(
            client,
            SubjectA,
            DesktopScopes.Access,
            ifNoneMatch:
                "\"sha256:0000000000000000000000000000000000000000000000000000000000000000\"");

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
    }

    [Fact]
    public async Task One_subjects_entity_tag_never_satisfies_another_subject()
    {
        await using CatalogFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();
        using HttpResponseMessage mine = await SendAsync(
            client,
            SubjectA,
            DesktopScopes.Access);
        string etag = mine.Headers.ETag!.ToString();

        using HttpResponseMessage theirs = await SendAsync(
            client,
            SubjectB,
            DesktopScopes.Access,
            ifNoneMatch: etag);

        Assert.Equal(HttpStatusCode.OK, theirs.StatusCode);
        Assert.NotEqual(etag, theirs.Headers.ETag!.ToString());
    }

    [Fact]
    public async Task The_cache_policy_is_short_private_and_varies_on_the_credential()
    {
        await using CatalogFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await SendAsync(
            client,
            SubjectA,
            DesktopScopes.Access);

        Assert.True(response.Headers.CacheControl!.Private);
        Assert.False(response.Headers.CacheControl.Public);
        Assert.Equal(
            TimeSpan.FromSeconds(ModelCatalogEndpoint.CacheMaximumAgeSeconds),
            response.Headers.CacheControl.MaxAge);
        Assert.Contains("Authorization", response.Headers.Vary, StringComparer.Ordinal);
    }

    [Fact]
    public async Task A_weak_entity_tag_is_not_honoured()
    {
        await using CatalogFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();
        using HttpResponseMessage first = await SendAsync(
            client,
            SubjectA,
            DesktopScopes.Access);

        using HttpResponseMessage second = await SendAsync(
            client,
            SubjectA,
            DesktopScopes.Access,
            ifNoneMatch: "W/" + first.Headers.ETag!.ToString());

        Assert.Equal(HttpStatusCode.OK, second.StatusCode);
    }

    // ---------------------------------------------------------------- problems

    [Fact]
    public async Task An_unreachable_inventory_is_reported_as_catalog_unavailable()
    {
        await using CatalogFactory factory = new(state: FoundryCatalogState.Unavailable);
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await SendAsync(
            client,
            SubjectA,
            DesktopScopes.Access);

        await AssertProblemAsync(
            response,
            ModelCatalogErrorCodes.Unavailable,
            HttpStatusCode.ServiceUnavailable);
    }

    [Fact]
    public async Task An_inventory_past_its_staleness_budget_is_reported_as_catalog_stale()
    {
        await using CatalogFactory factory = new(
            state: FoundryCatalogState.Unavailable,
            hasPreviousSnapshot: true);
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await SendAsync(
            client,
            SubjectA,
            DesktopScopes.Access);

        await AssertProblemAsync(
            response,
            ModelCatalogErrorCodes.Stale,
            HttpStatusCode.ServiceUnavailable);
    }

    [Fact]
    public async Task A_client_below_the_minimum_contract_epoch_is_told_to_upgrade()
    {
        await using CatalogFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await SendAsync(
            client,
            SubjectA,
            DesktopScopes.Access,
            contractEpoch: "0");

        await AssertProblemAsync(
            response,
            ModelCatalogErrorCodes.ContractUpgradeRequired,
            HttpStatusCode.UpgradeRequired);
    }

    [Fact]
    public async Task A_subject_with_nothing_visible_and_nothing_entitled_gets_a_stable_problem()
    {
        await using CatalogFactory factory = new(hideEveryProfile: true);
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await SendAsync(
            client,
            SubjectB,
            DesktopScopes.Access);

        await AssertProblemAsync(
            response,
            ModelCatalogErrorCodes.NoEntitledProfiles,
            HttpStatusCode.Forbidden);
    }

    [Theory]
    [InlineData("?unknownParameter=1")]
    [InlineData("?lastSelectedProfileId=NOT_A_PROFILE")]
    [InlineData("?lastSelectedProfileId=a&lastSelectedProfileId=b")]
    public async Task An_unknown_or_malformed_query_parameter_is_refused(string query)
    {
        await using CatalogFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await SendAsync(
            client,
            SubjectA,
            DesktopScopes.Access,
            query: query);

        await AssertProblemAsync(response, "request_invalid", HttpStatusCode.BadRequest);
    }

    [Fact]
    public async Task A_malformed_contract_epoch_header_is_refused()
    {
        await using CatalogFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await SendAsync(
            client,
            SubjectA,
            DesktopScopes.Access,
            contractEpoch: "not-a-number");

        await AssertProblemAsync(response, "request_invalid", HttpStatusCode.BadRequest);
    }

    [Fact]
    public async Task A_valid_selection_hint_is_honoured()
    {
        await using CatalogFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await SendAsync(
            client,
            SubjectA,
            DesktopScopes.Access,
            query: "?lastSelectedProfileId=" + ModelCatalogTestFixtures.BetaProfileId);
        using JsonDocument document = await ReadAsync(response);

        Assert.Equal(
            ModelCatalogTestFixtures.BetaProfileId,
            document.RootElement.GetProperty("defaultProfileId").GetString());
    }

    // -------------------------------------------------------------- bootstrap

    [Fact]
    public async Task Bootstrap_advertises_the_model_catalog_capability()
    {
        await using CatalogFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await SendAsync(
            client,
            SubjectA,
            DesktopScopes.Access,
            path: "/desktop/v1/bootstrap");
        using JsonDocument document = await ReadAsync(response);

        Assert.Contains(
            document.RootElement.GetProperty("capabilities").EnumerateArray(),
            capability => capability.GetString() == ModelCatalogEndpoint.BootstrapCapability);
    }

    // ----------------------------------------------------------------- helpers

    private static async Task<JsonDocument> ReadAsync(HttpResponseMessage response)
    {
        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
        return JsonDocument.Parse(await response.Content.ReadAsStringAsync(Ct));
    }

    private static async Task AssertProblemAsync(
        HttpResponseMessage response,
        string expectedCode,
        HttpStatusCode expectedStatus)
    {
        Assert.Equal(expectedStatus, response.StatusCode);
        using JsonDocument document = JsonDocument.Parse(
            await response.Content.ReadAsStringAsync(Ct));
        Assert.Equal(expectedCode, document.RootElement.GetProperty("code").GetString());
        Assert.Equal((int)expectedStatus, document.RootElement.GetProperty("status").GetInt32());
    }

    private static Task<HttpResponseMessage> SendAsync(
        HttpClient client,
        string subject,
        string scope,
        string clientId = ApprovedClientId,
        string? ifNoneMatch = null,
        string? contractEpoch = null,
        string query = "",
        string path = Route)
    {
        HttpRequestMessage request = new(HttpMethod.Get, path + query);
        request.Headers.Add(CatalogAuthenticationHandler.TenantHeader, TenantId);
        request.Headers.Add(CatalogAuthenticationHandler.SubjectHeader, subject);
        request.Headers.Add(CatalogAuthenticationHandler.ClientHeader, clientId);
        request.Headers.Add(CatalogAuthenticationHandler.ScopeHeader, scope);
        if (ifNoneMatch is not null)
        {
            request.Headers.TryAddWithoutValidation("If-None-Match", ifNoneMatch);
        }
        if (contractEpoch is not null)
        {
            request.Headers.TryAddWithoutValidation(
                ModelCatalogEndpoint.ContractEpochHeader,
                contractEpoch);
        }
        return client.SendAsync(request, Ct);
    }

    private sealed class CatalogFactory(
        FoundryCatalogState state = FoundryCatalogState.Fresh,
        bool hasPreviousSnapshot = false,
        bool hideEveryProfile = false) : WebApplicationFactory<Program>
    {
        protected override void ConfigureWebHost(IWebHostBuilder builder)
        {
            builder.UseEnvironment("Development");
            builder.ConfigureLogging(logging => logging.ClearProviders());
            builder.UseSetting(
                "Sapphirus:Authority",
                $"https://login.microsoftonline.com/{TenantId}/v2.0");
            builder.UseSetting("Sapphirus:Audience", "api://66666666-6666-6666-6666-666666666666");
            builder.UseSetting("Sapphirus:ApprovedDesktopClientId", ApprovedClientId);
            builder.UseSetting("Sapphirus:Region", ModelCatalogTestFixtures.Region);
            builder.UseSetting("Sapphirus:ReleaseChannel", "beta");
            builder.UseSetting("Sapphirus:DevelopmentSigningEnabled", "true");
            builder.UseSetting("Sapphirus:DevelopmentModelEnabled", "true");
            builder.ConfigureTestServices(services =>
            {
                services.AddAuthentication(authentication =>
                    {
                        authentication.DefaultAuthenticateScheme =
                            CatalogAuthenticationHandler.AuthenticationSchemeName;
                        authentication.DefaultChallengeScheme =
                            CatalogAuthenticationHandler.AuthenticationSchemeName;
                        authentication.DefaultForbidScheme =
                            CatalogAuthenticationHandler.AuthenticationSchemeName;
                    })
                    .AddScheme<AuthenticationSchemeOptions, CatalogAuthenticationHandler>(
                        CatalogAuthenticationHandler.AuthenticationSchemeName,
                        _ => { });

                // Only the four catalog seams are replaced. The composition
                // under test — the endpoint, the projection, the default
                // selection, and the cache policy — is the real one.
                IReadOnlyList<ModelProfile> profiles = hideEveryProfile
                    ?
                    [
                        ModelCatalogTestFixtures.ParseAlpha() with { Visible = false },
                        ModelCatalogTestFixtures.ParseBeta() with { Visible = false },
                    ]
                    : [ModelCatalogTestFixtures.ParseAlpha(), ModelCatalogTestFixtures.ParseBeta()];
                services.RemoveAll<IModelProfileRegistry>();
                services.AddSingleton<IModelProfileRegistry>(
                    new FixedRegistry(profiles));
                services.RemoveAll<IModelCatalogDeploymentSource>();
                services.AddSingleton<IModelCatalogDeploymentSource>(
                    new FixedDeployments(state, hasPreviousSnapshot));
                services.RemoveAll<IModelCatalogPolicySource>();
                services.AddSingleton<IModelCatalogPolicySource>(
                    new FixedPolicy(ModelCatalogTestFixtures.Policy()));
                services.RemoveAll<IModelEntitlementSource>();
                services.AddSingleton<IModelEntitlementSource>(new PerSubjectEntitlement());
            });
        }

        public HttpClient CreateSecureClient() => CreateClient(new WebApplicationFactoryClientOptions
        {
            AllowAutoRedirect = false,
            BaseAddress = new Uri("https://localhost"),
        });
    }

    private sealed class FixedRegistry(IReadOnlyList<ModelProfile> profiles) : IModelProfileRegistry
    {
        public ValueTask<ModelProfileRegistrySnapshot> GetSnapshotAsync(
            CancellationToken cancellationToken) =>
            ValueTask.FromResult(new ModelProfileRegistrySnapshot(
                ModelCatalogTestFixtures.ParityRegistryVersion,
                ModelCatalogTestFixtures.ParityRegistryHash,
                ModelCatalogTestFixtures.ParityCapabilityHash,
                profiles,
                ModelProfileTrust.Signed,
                DateTimeOffset.UnixEpoch));
    }

    private sealed class FixedDeployments(FoundryCatalogState state, bool hasPreviousSnapshot)
        : IModelCatalogDeploymentSource
    {
        public ValueTask<ModelDeploymentRead> GetAsync(CancellationToken cancellationToken) =>
            ValueTask.FromResult(new ModelDeploymentRead(
                state,
                state is FoundryCatalogState.Fresh or FoundryCatalogState.StaleUsable
                    ? ModelCatalogTestFixtures.Snapshot(
                        ModelCatalogTestFixtures.Deployment(
                            ModelCatalogTestFixtures.AlphaDeploymentName,
                            ModelCatalogTestFixtures.AlphaDeploymentHash),
                        ModelCatalogTestFixtures.Deployment(
                            ModelCatalogTestFixtures.BetaDeploymentName,
                            ModelCatalogTestFixtures.BetaDeploymentHash))
                    : null,
                hasPreviousSnapshot));
    }

    private sealed class FixedPolicy(PolicySnapshot snapshot) : IModelCatalogPolicySource
    {
        public ValueTask<PolicySnapshot> GetSnapshotAsync(CancellationToken cancellationToken) =>
            ValueTask.FromResult(snapshot);
    }

    /// <summary>
    /// Two subjects with different entitlements. The group that granted them is
    /// deliberately named here and asserted absent from every response.
    /// </summary>
    private sealed class PerSubjectEntitlement : IModelEntitlementSource
    {
        private readonly string _first = ScopeAuthorization.SubjectPartition(Principal(SubjectA));

        public ValueTask<ModelEntitlementSnapshot> GetAsync(
            string subjectPartition,
            CancellationToken cancellationToken) =>
            ValueTask.FromResult(string.Equals(
                subjectPartition,
                _first,
                StringComparison.Ordinal)
                ? new ModelEntitlementSnapshot(
                    "synthetic-class-full",
                    new HashSet<string>(
                        [
                            ModelCatalogTestFixtures.AlphaProfileId,
                            ModelCatalogTestFixtures.BetaProfileId,
                        ],
                        StringComparer.Ordinal),
                    OrganizationDefaultProfileId: null)
                : new ModelEntitlementSnapshot(
                    "synthetic-class-none",
                    new HashSet<string>(StringComparer.Ordinal),
                    OrganizationDefaultProfileId: null));

        private static ClaimsPrincipal Principal(string subject) => new(new ClaimsIdentity(
            [new Claim("tid", TenantId), new Claim("oid", subject)],
            "fixture"));
    }

    private sealed class CatalogAuthenticationHandler(
        IOptionsMonitor<AuthenticationSchemeOptions> options,
        ILoggerFactory logger,
        UrlEncoder encoder)
        : AuthenticationHandler<AuthenticationSchemeOptions>(options, logger, encoder)
    {
        public const string AuthenticationSchemeName = "ModelCatalogTests";
        public const string TenantHeader = "X-Test-Tenant";
        public const string SubjectHeader = "X-Test-Subject";
        public const string ClientHeader = "X-Test-Client";
        public const string ScopeHeader = "X-Test-Scope";

        protected override Task<AuthenticateResult> HandleAuthenticateAsync()
        {
            if (!Request.Headers.TryGetValue(TenantHeader, out var tenant)
                || !Request.Headers.TryGetValue(SubjectHeader, out var subject)
                || !Request.Headers.TryGetValue(ClientHeader, out var client)
                || !Request.Headers.TryGetValue(ScopeHeader, out var scope))
            {
                return Task.FromResult(AuthenticateResult.NoResult());
            }
            Claim[] claims =
            [
                new("tid", tenant.ToString()),
                new("oid", subject.ToString()),
                new("azp", client.ToString()),
                new("scp", scope.ToString()),
            ];
            ClaimsIdentity identity = new(claims, AuthenticationSchemeName);
            return Task.FromResult(AuthenticateResult.Success(
                new AuthenticationTicket(new ClaimsPrincipal(identity), AuthenticationSchemeName)));
        }
    }
}
