using System.Net;
using System.Text.Json;
using Azure.Core;
using Sapphirus.DesktopSupportApi.Foundry;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Foundry;

/// <summary>
/// FND-022. Every case here runs against <see cref="FakeFoundryServer"/> and
/// the canonical fixture bodies. There is no real project, deployment, or
/// credential anywhere in this file.
/// </summary>
public sealed class FoundryDeploymentDiscoveryTests
{
    private const string Marker = "forge-foundry-fake-marker-v1";
    private const string FixturePath = "ContractFixtures/foundry-fixtures.v1.json";
    private static readonly Uri ProjectEndpoint =
        new("https://forge-fake.test/api/projects/forge-test/");
    private static readonly DateTimeOffset Now =
        DateTimeOffset.Parse("2026-07-28T12:00:00Z");

    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    [Fact]
    public async Task Deployment_discovery_normalizes_one_page_into_the_canonical_contract()
    {
        using DiscoveryHarness harness = new();
        harness.EnqueueScenario("one_page");

        FoundryDiscoveryResult result = await harness.Client.DiscoverAsync(Ct);

        Assert.Equal(FoundryDiscoveryOutcome.Success, result.Outcome);
        FoundryDeploymentSnapshot snapshot = RequireSnapshot(result);
        Assert.Equal("sapphirus.foundry-deployment.v1", snapshot.SchemaVersion);
        Assert.Matches("^sha256:[0-9a-f]{64}$", snapshot.SnapshotHash);
        Assert.Equal(Now, snapshot.RetrievedAt);
        FoundryDeployment deployment = Assert.Single(snapshot.Deployments);
        Assert.Equal("desktop-planning", deployment.DeploymentName);
        Assert.Equal("ModelDeployment", deployment.DeploymentType);
        Assert.Equal("OpenAI", deployment.ModelPublisher);
        Assert.Equal("gpt-5-mini", deployment.ModelName);
        Assert.Equal("2026-06-01", deployment.ModelVersion);
        Assert.Equal("GlobalStandard", deployment.Sku.Name);
        Assert.Equal("Global", deployment.Sku.Tier);
        Assert.Equal("OpenAI", deployment.Sku.Family);
        Assert.Equal("S", deployment.Sku.Size);
        Assert.Equal(10, deployment.Sku.Capacity);
        Assert.Matches("^sha256:[0-9a-f]{64}$", deployment.DeploymentHash);
    }

    [Fact]
    public async Task Deployment_discovery_maps_provider_capability_keys_onto_canonical_names()
    {
        using DiscoveryHarness harness = new();
        harness.EnqueueScenario("one_page");

        FoundryDiscoveryResult result = await harness.Client.DiscoverAsync(Ct);

        FoundryDeploymentCapabilities capabilities =
            Assert.Single(RequireSnapshot(result).Deployments).Capabilities;
        // json_schema and tool_calling are the only advertised keys.
        Assert.True(capabilities.StructuredOutput);
        Assert.True(capabilities.ToolCalling);
        // Everything the provider did not advertise narrows to false; a
        // deployment may never broaden a profile.
        Assert.False(capabilities.TextInput);
        Assert.False(capabilities.ImageInput);
        Assert.False(capabilities.PdfInput);
        Assert.False(capabilities.Attachments);
        Assert.False(capabilities.Reasoning);
        Assert.False(capabilities.Streaming);
    }

    [Fact]
    public async Task Deployment_discovery_follows_a_same_project_next_link()
    {
        using DiscoveryHarness harness = new();
        harness.EnqueueScenario("multiple_pages");

        FoundryDiscoveryResult result = await harness.Client.DiscoverAsync(Ct);

        Assert.Equal(FoundryDiscoveryOutcome.Success, result.Outcome);
        Assert.Equal(
            ["desktop-coding", "desktop-planning"],
            RequireSnapshot(result).Deployments
                .Select(static deployment => deployment.DeploymentName));
        Assert.Equal(2, result.Evidence.PageCount);
        Assert.Equal(2, result.Evidence.ClientRequestIds.Count);
        Assert.Equal(
            result.Evidence.ClientRequestIds.Count,
            result.Evidence.ClientRequestIds.Distinct(StringComparer.Ordinal).Count());
    }

    [Fact]
    public async Task Deployment_discovery_rejects_a_cross_origin_next_link()
    {
        using DiscoveryHarness harness = new();
        harness.EnqueueScenario("wrong_next_link_origin");

        FoundryDiscoveryResult result = await harness.Client.DiscoverAsync(Ct);

        Assert.Equal(FoundryDiscoveryOutcome.PaginationIntegrity, result.Outcome);
        Assert.Equal("foundry_pagination_origin_invalid", result.ErrorCode);
        Assert.Null(result.Snapshot);
        // The forged origin was never contacted.
        Assert.Equal(1, result.Evidence.PageCount);
        Assert.Single(harness.Server.RequestReceipts);
    }

    [Theory]
    // Another origin entirely.
    [InlineData(
        "https://attacker.invalid/api/projects/forge-test/deployments?api-version=v1",
        "foundry_pagination_origin_invalid")]
    // Downgraded scheme, explicit port, and embedded credentials.
    [InlineData(
        "http://forge-fake.test/api/projects/forge-test/deployments?api-version=v1",
        "foundry_pagination_origin_invalid")]
    [InlineData(
        "https://forge-fake.test:8443/api/projects/forge-test/deployments?api-version=v1",
        "foundry_pagination_origin_invalid")]
    [InlineData(
        "https://user:pass@forge-fake.test/api/projects/forge-test/deployments?api-version=v1",
        "foundry_pagination_origin_invalid")]
    [InlineData(
        "https://forge-fake.test/api/projects/forge-test/deployments?api-version=v1#fragment",
        "foundry_pagination_origin_invalid")]
    // Same origin, another project or another route.
    [InlineData(
        "https://forge-fake.test/api/projects/other-project/deployments?api-version=v1",
        "foundry_pagination_path_invalid")]
    [InlineData(
        "https://forge-fake.test/api/projects/forge-test/deployments/extra?api-version=v1",
        "foundry_pagination_path_invalid")]
    [InlineData(
        "https://forge-fake.test/api/projects/forge-test/openai/v1/responses?api-version=v1",
        "foundry_pagination_path_invalid")]
    // The pinned API version and deployment type may not drift.
    [InlineData(
        "https://forge-fake.test/api/projects/forge-test/deployments",
        "foundry_pagination_query_invalid")]
    [InlineData(
        "https://forge-fake.test/api/projects/forge-test/deployments?api-version=2024-10-21",
        "foundry_pagination_query_invalid")]
    [InlineData(
        "https://forge-fake.test/api/projects/forge-test/deployments?api-version=v1&api-version=v1",
        "foundry_pagination_query_invalid")]
    [InlineData(
        "https://forge-fake.test/api/projects/forge-test/deployments?api-version=v1&deploymentType=Anything",
        "foundry_pagination_query_invalid")]
    [InlineData(
        "https://forge-fake.test/api/projects/forge-test/deployments?api-version=v1&redirect=https://attacker.invalid",
        "foundry_pagination_query_invalid")]
    [InlineData("not-a-uri", "foundry_pagination_uri_invalid")]
    [InlineData("/api/projects/forge-test/deployments?api-version=v1", "foundry_pagination_uri_invalid")]
    public void Deployment_discovery_refuses_every_next_link_that_leaves_the_project(
        string nextLink,
        string expectedErrorCode)
    {
        bool accepted = FoundryDeploymentDiscoveryClient.TryResolveNextLink(
            ProjectEndpoint,
            nextLink,
            out Uri? resolved,
            out string errorCode);

        Assert.False(accepted);
        Assert.Null(resolved);
        Assert.Equal(expectedErrorCode, errorCode);
    }

    [Fact]
    public void Deployment_discovery_accepts_only_a_same_project_continuation()
    {
        bool accepted = FoundryDeploymentDiscoveryClient.TryResolveNextLink(
            ProjectEndpoint,
            "https://forge-fake.test/api/projects/forge-test/deployments" +
            "?api-version=v1&deploymentType=ModelDeployment&continuationToken=page-2",
            out Uri? resolved,
            out string errorCode);

        Assert.True(accepted);
        Assert.Equal("", errorCode);
        Assert.NotNull(resolved);
        Assert.Equal(
            "https://forge-fake.test/api/projects/forge-test/deployments" +
            "?api-version=v1&deploymentType=ModelDeployment&continuationToken=page-2",
            resolved.AbsoluteUri);
    }

    [Fact]
    public async Task Deployment_discovery_stops_on_a_looping_next_link()
    {
        using DiscoveryHarness harness = new();
        string selfReferencingPage = JsonSerializer.Serialize(new
        {
            value = Array.Empty<object>(),
            nextLink =
                "https://forge-fake.test/api/projects/forge-test/deployments" +
                "?api-version=v1&deploymentType=ModelDeployment",
        });
        harness.Server.Enqueue(FakeFoundryReply.Json(HttpStatusCode.OK, selfReferencingPage));
        harness.Server.Enqueue(FakeFoundryReply.Json(HttpStatusCode.OK, selfReferencingPage));

        FoundryDiscoveryResult result = await harness.Client.DiscoverAsync(Ct);

        Assert.Equal(FoundryDiscoveryOutcome.PaginationIntegrity, result.Outcome);
        Assert.Equal("foundry_pagination_cycle", result.ErrorCode);
        Assert.Equal(1, result.Evidence.PageCount);
    }

    [Fact]
    public async Task Deployment_discovery_stops_at_the_fixed_page_bound()
    {
        using DiscoveryHarness harness = new();
        for (int page = 0; page <= FoundryDeploymentDiscoveryClient.MaximumPages; page++)
        {
            harness.Server.Enqueue(FakeFoundryReply.Json(
                HttpStatusCode.OK,
                JsonSerializer.Serialize(new
                {
                    value = Array.Empty<object>(),
                    nextLink =
                        "https://forge-fake.test/api/projects/forge-test/deployments" +
                        "?api-version=v1&deploymentType=ModelDeployment" +
                        $"&continuationToken=page-{page + 2}",
                })));
        }

        FoundryDiscoveryResult result = await harness.Client.DiscoverAsync(Ct);

        Assert.Equal(FoundryDiscoveryOutcome.PaginationIntegrity, result.Outcome);
        Assert.Equal("foundry_pagination_page_limit", result.ErrorCode);
        Assert.Equal(
            FoundryDeploymentDiscoveryClient.MaximumPages,
            result.Evidence.PageCount);
    }

    [Fact]
    public async Task Deployment_discovery_collapses_identical_duplicate_deployments()
    {
        using DiscoveryHarness harness = new();
        harness.EnqueueScenario("duplicate_deployments");

        FoundryDiscoveryResult result = await harness.Client.DiscoverAsync(Ct);

        Assert.Equal(FoundryDiscoveryOutcome.Success, result.Outcome);
        Assert.Equal(
            "desktop-planning",
            Assert.Single(RequireSnapshot(result).Deployments).DeploymentName);
    }

    [Fact]
    public async Task Deployment_discovery_rejects_duplicate_names_with_conflicting_identity()
    {
        using DiscoveryHarness harness = new();
        harness.Server.Enqueue(FakeFoundryReply.Json(
            HttpStatusCode.OK,
            """
            {"value":[
              {"name":"desktop-planning","type":"ModelDeployment","modelPublisher":"OpenAI",
               "modelName":"gpt-5-mini","modelVersion":"2026-06-01",
               "sku":{"name":"GlobalStandard","tier":"Global","size":"S","family":"OpenAI","capacity":10},
               "capabilities":{"json_schema":true}},
              {"name":"desktop-planning","type":"ModelDeployment","modelPublisher":"OpenAI",
               "modelName":"gpt-5-mini","modelVersion":"2026-06-02",
               "sku":{"name":"GlobalStandard","tier":"Global","size":"S","family":"OpenAI","capacity":10},
               "capabilities":{"json_schema":true}}
            ]}
            """));

        FoundryDiscoveryResult result = await harness.Client.DiscoverAsync(Ct);

        Assert.Equal(FoundryDiscoveryOutcome.InvalidBody, result.Outcome);
        Assert.Equal("foundry_deployment_duplicate_conflict", result.ErrorCode);
        Assert.Null(result.Snapshot);
    }

    [Fact]
    public async Task Deployment_discovery_never_projects_an_unknown_provider_field()
    {
        using DiscoveryHarness withUnknownFields = new();
        withUnknownFields.EnqueueScenario("unknown_fields");
        using DiscoveryHarness withoutUnknownFields = new();
        withoutUnknownFields.Server.Enqueue(FakeFoundryReply.Json(
            HttpStatusCode.OK,
            """
            {"value":[
              {"name":"desktop-planning","type":"ModelDeployment","modelPublisher":"OpenAI",
               "modelName":"gpt-5-mini","modelVersion":"2026-06-01",
               "sku":{"name":"GlobalStandard","tier":"Global","size":"S","family":"OpenAI","capacity":10},
               "capabilities":{"json_schema":true}}
            ]}
            """));

        FoundryDiscoveryResult noisy = await withUnknownFields.Client.DiscoverAsync(Ct);
        FoundryDiscoveryResult clean = await withoutUnknownFields.Client.DiscoverAsync(Ct);

        Assert.Equal(FoundryDiscoveryOutcome.Success, noisy.Outcome);
        // An added provider field changes neither the projection nor the hash.
        Assert.Equal(
            RequireSnapshot(clean).SnapshotHash,
            RequireSnapshot(noisy).SnapshotHash);
        string serialized = JsonSerializer.Serialize(noisy.Snapshot);
        Assert.DoesNotContain("connectionName", serialized, StringComparison.Ordinal);
        Assert.DoesNotContain("futureMetadata", serialized, StringComparison.Ordinal);
        Assert.DoesNotContain("futureSkuField", serialized, StringComparison.Ordinal);
        Assert.DoesNotContain("future_capability", serialized, StringComparison.Ordinal);
        Assert.DoesNotContain("unknown-field", serialized, StringComparison.Ordinal);
    }

    [Fact]
    public async Task Deployment_discovery_rejects_a_malformed_body()
    {
        using DiscoveryHarness harness = new();
        harness.EnqueueScenario("malformed_json");

        FoundryDiscoveryResult result = await harness.Client.DiscoverAsync(Ct);

        Assert.Equal(FoundryDiscoveryOutcome.InvalidBody, result.Outcome);
        Assert.Equal("foundry_discovery_body_malformed", result.ErrorCode);
    }

    [Fact]
    public async Task Deployment_discovery_rejects_a_body_over_the_aggregate_limit()
    {
        using DiscoveryHarness harness = new();
        harness.EnqueueScenario("oversized_body");

        FoundryDiscoveryResult result = await harness.Client.DiscoverAsync(Ct);

        Assert.Equal(FoundryDiscoveryOutcome.InvalidBody, result.Outcome);
        Assert.Equal("foundry_discovery_response_too_large", result.ErrorCode);
        Assert.True(
            result.Evidence.ResponseBytes
                <= FoundryDeploymentDiscoveryClient.MaximumAggregateResponseBytes);
    }

    [Fact]
    public async Task Deployment_discovery_rejects_a_duplicate_json_property()
    {
        using DiscoveryHarness harness = new();
        harness.Server.Enqueue(FakeFoundryReply.Json(
            HttpStatusCode.OK,
            """{"value":[],"value":[]}"""));

        FoundryDiscoveryResult result = await harness.Client.DiscoverAsync(Ct);

        Assert.Equal(FoundryDiscoveryOutcome.InvalidBody, result.Outcome);
        Assert.Equal("foundry_discovery_body_malformed", result.ErrorCode);
    }

    [Fact]
    public async Task Deployment_discovery_parses_provider_fields_case_sensitively()
    {
        using DiscoveryHarness harness = new();
        harness.Server.Enqueue(FakeFoundryReply.Json(
            HttpStatusCode.OK,
            """
            {"Value":[
              {"Name":"desktop-planning","Type":"ModelDeployment"}
            ]}
            """));

        FoundryDiscoveryResult result = await harness.Client.DiscoverAsync(Ct);

        // "Value" is not "value": the page carries no inventory at all.
        Assert.Equal(FoundryDiscoveryOutcome.InvalidBody, result.Outcome);
        Assert.Equal("foundry_discovery_body_malformed", result.ErrorCode);
    }

    [Fact]
    public async Task Deployment_discovery_returns_an_empty_inventory_without_failing()
    {
        using DiscoveryHarness harness = new();
        harness.EnqueueScenario("empty_inventory");

        FoundryDiscoveryResult result = await harness.Client.DiscoverAsync(Ct);

        Assert.Equal(FoundryDiscoveryOutcome.Success, result.Outcome);
        Assert.Empty(RequireSnapshot(result).Deployments);
        Assert.Matches("^sha256:[0-9a-f]{64}$", result.Snapshot!.SnapshotHash);
    }

    [Fact]
    public async Task Deployment_discovery_hash_changes_when_a_deployment_is_removed()
    {
        using DiscoveryHarness harness = new();
        harness.EnqueueScenario("deployment_removal");

        FoundryDiscoveryResult before = await harness.Client.DiscoverAsync(Ct);
        FoundryDiscoveryResult after = await harness.Client.DiscoverAsync(Ct);

        Assert.Single(RequireSnapshot(before).Deployments);
        Assert.Empty(RequireSnapshot(after).Deployments);
        Assert.NotEqual(before.Snapshot!.SnapshotHash, after.Snapshot!.SnapshotHash);
    }

    [Fact]
    public async Task Deployment_discovery_hash_is_stable_across_an_unchanged_inventory()
    {
        using DiscoveryHarness harness = new();
        harness.EnqueueScenario("one_page");
        harness.EnqueueScenario("one_page");

        FoundryDiscoveryResult first = await harness.Client.DiscoverAsync(Ct);
        harness.Advance(TimeSpan.FromHours(3));
        FoundryDiscoveryResult second = await harness.Client.DiscoverAsync(Ct);

        Assert.Equal(
            RequireSnapshot(first).SnapshotHash,
            RequireSnapshot(second).SnapshotHash);
        Assert.NotEqual(first.Snapshot!.RetrievedAt, second.Snapshot!.RetrievedAt);
    }

    [Theory]
    [InlineData(HttpStatusCode.Unauthorized, FoundryDiscoveryOutcome.Authorization)]
    [InlineData(HttpStatusCode.Forbidden, FoundryDiscoveryOutcome.Authorization)]
    [InlineData(HttpStatusCode.NotFound, FoundryDiscoveryOutcome.ProjectConfiguration)]
    [InlineData(HttpStatusCode.BadRequest, FoundryDiscoveryOutcome.ProjectConfiguration)]
    [InlineData(HttpStatusCode.RequestTimeout, FoundryDiscoveryOutcome.Transient)]
    [InlineData(HttpStatusCode.TooManyRequests, FoundryDiscoveryOutcome.Transient)]
    [InlineData(HttpStatusCode.InternalServerError, FoundryDiscoveryOutcome.Transient)]
    [InlineData(HttpStatusCode.ServiceUnavailable, FoundryDiscoveryOutcome.Transient)]
    [InlineData(HttpStatusCode.GatewayTimeout, FoundryDiscoveryOutcome.Transient)]
    public async Task Deployment_discovery_classifies_every_provider_status(
        HttpStatusCode statusCode,
        FoundryDiscoveryOutcome expected)
    {
        using DiscoveryHarness harness = new();
        harness.Server.Enqueue(FakeFoundryReply.Json(
            statusCode,
            """{"error":{"code":"fixture"}}"""));

        FoundryDiscoveryResult result = await harness.Client.DiscoverAsync(Ct);

        Assert.Equal(expected, result.Outcome);
        Assert.Null(result.Snapshot);
        Assert.NotEqual("", result.ErrorCode);
    }

    [Fact]
    public async Task Deployment_discovery_classifies_an_unavailable_token_as_authorization()
    {
        using DiscoveryHarness harness = new(tokenFails: true);

        FoundryDiscoveryResult result = await harness.Client.DiscoverAsync(Ct);

        Assert.Equal(FoundryDiscoveryOutcome.Authorization, result.Outcome);
        Assert.Equal("foundry_discovery_token_unavailable", result.ErrorCode);
        Assert.Empty(harness.Server.RequestReceipts);
    }

    [Fact]
    public async Task Deployment_discovery_requests_a_token_once_per_operation()
    {
        using DiscoveryHarness harness = new();
        harness.EnqueueScenario("multiple_pages");

        _ = await harness.Client.DiscoverAsync(Ct);

        Assert.Equal(1, harness.Credential.TokenRequests);
        Assert.Equal(
            [FoundryDeploymentDiscoveryClient.TokenScope],
            Assert.Single(harness.Credential.RequestedScopes));
    }

    [Fact]
    public async Task Deployment_discovery_sends_a_generated_client_request_id()
    {
        using DiscoveryHarness harness = new();
        harness.EnqueueScenario("one_page");

        FoundryDiscoveryResult result = await harness.Client.DiscoverAsync(Ct);

        FakeFoundryRequestReceipt receipt = Assert.Single(harness.Server.RequestReceipts);
        Assert.Contains("x-ms-client-request-id", receipt.HeaderNames);
        Assert.Contains("Authorization", receipt.HeaderNames);
        Assert.Equal("deployments", receipt.Route);
        Assert.True(Guid.TryParseExact(
            Assert.Single(result.Evidence.ClientRequestIds),
            "D",
            out _));
    }

    [Fact]
    public async Task Deployment_discovery_evidence_carries_no_deployment_name_or_endpoint()
    {
        using DiscoveryHarness harness = new();
        harness.EnqueueScenario("one_page");

        FoundryDiscoveryResult result = await harness.Client.DiscoverAsync(Ct);

        string evidence = JsonSerializer.Serialize(result.Evidence);
        Assert.DoesNotContain("desktop-planning", evidence, StringComparison.Ordinal);
        Assert.DoesNotContain("forge-fake.test", evidence, StringComparison.Ordinal);
        Assert.DoesNotContain("api/projects", evidence, StringComparison.Ordinal);
        Assert.DoesNotContain(Marker, evidence, StringComparison.Ordinal);
        Assert.Equal(Now, result.Evidence.RetrievedAt);
    }

    [Fact]
    public async Task Deployment_discovery_rejects_a_non_json_content_type()
    {
        using DiscoveryHarness harness = new();
        harness.Server.Enqueue(FakeFoundryReply.EventStream("data: {}\n\n"));

        FoundryDiscoveryResult result = await harness.Client.DiscoverAsync(Ct);

        Assert.Equal(FoundryDiscoveryOutcome.InvalidBody, result.Outcome);
        Assert.Equal("foundry_discovery_content_type_invalid", result.ErrorCode);
    }

    [Fact]
    public async Task Deployment_discovery_observes_caller_cancellation()
    {
        using DiscoveryHarness harness = new();
        harness.Server.Enqueue(FakeFoundryReply.WaitForCancellation());
        using CancellationTokenSource cancellation = new(TimeSpan.FromMilliseconds(50));

        await Assert.ThrowsAnyAsync<OperationCanceledException>(() =>
            harness.Client.DiscoverAsync(cancellation.Token));
    }

    [Theory]
    [InlineData("https://forge-fake.test/api/projects/forge-test")]
    [InlineData("https://forge-fake.test/api/projects/")]
    [InlineData("https://forge-fake.test/")]
    [InlineData("http://forge-fake.test/api/projects/forge-test/")]
    [InlineData("https://user@forge-fake.test/api/projects/forge-test/")]
    [InlineData("https://forge-fake.test/api/projects/forge-test/?api-version=v1")]
    [InlineData("https://forge-fake.test/api/projects/forge/test/")]
    public void Deployment_discovery_refuses_a_project_endpoint_of_the_wrong_shape(
        string endpoint)
    {
        FakeFoundryTestClock clock = new(Now);
        using FakeFoundryServer server = new("Test", clock);
        using HttpClient httpClient = new(server, disposeHandler: false);

        Assert.Throws<ArgumentException>(() => new FoundryDeploymentDiscoveryClient(
            httpClient,
            new ScriptedCredential(Marker, fails: false),
            new Uri(endpoint),
            new AdvanceableTimeProvider(Now)));
    }

    private static FoundryDeploymentSnapshot RequireSnapshot(FoundryDiscoveryResult result)
    {
        Assert.NotNull(result.Snapshot);
        return result.Snapshot;
    }

    private static IReadOnlyList<string> LoadDeploymentScenario(string name)
    {
        string path = Path.Combine(AppContext.BaseDirectory, FixturePath);
        using JsonDocument document = JsonDocument.Parse(File.ReadAllBytes(path));
        foreach (JsonElement scenario in document.RootElement
            .GetProperty("deploymentScenarios")
            .EnumerateArray())
        {
            if (!string.Equals(
                scenario.GetProperty("name").GetString(),
                name,
                StringComparison.Ordinal))
            {
                continue;
            }
            List<string> bodies = [];
            foreach (JsonElement response in scenario.GetProperty("responses").EnumerateArray())
            {
                if (response.TryGetProperty("jsonBody", out JsonElement jsonBody))
                {
                    bodies.Add(jsonBody.GetRawText());
                }
                else if (response.TryGetProperty("body", out JsonElement body))
                {
                    bodies.Add(body.GetString() ?? "");
                }
                else if (response.TryGetProperty("repeatBody", out JsonElement repeat))
                {
                    bodies.Add(new string(
                        repeat.GetProperty("value").GetString()![0],
                        repeat.GetProperty("count").GetInt32()));
                }
                else
                {
                    throw new InvalidOperationException(
                        "foundry_fixture_response_shape_unsupported");
                }
            }
            return bodies;
        }
        throw new InvalidOperationException("foundry_fixture_scenario_missing");
    }

    private sealed class DiscoveryHarness : IDisposable
    {
        public DiscoveryHarness(bool tokenFails = false)
        {
            Clock = new FakeFoundryTestClock(Now);
            Server = new FakeFoundryServer("Test", Clock);
            HttpClient = new HttpClient(Server, disposeHandler: false);
            Credential = new ScriptedCredential(Marker, tokenFails);
            TimeProvider = new AdvanceableTimeProvider(Now);
            Client = new FoundryDeploymentDiscoveryClient(
                HttpClient,
                Credential,
                ProjectEndpoint,
                TimeProvider);
        }

        public FakeFoundryTestClock Clock { get; }

        public FakeFoundryServer Server { get; }

        public HttpClient HttpClient { get; }

        public ScriptedCredential Credential { get; }

        public AdvanceableTimeProvider TimeProvider { get; }

        public FoundryDeploymentDiscoveryClient Client { get; }

        public void EnqueueScenario(string name)
        {
            foreach (string body in LoadDeploymentScenario(name))
            {
                Server.Enqueue(FakeFoundryReply.Json(HttpStatusCode.OK, body));
            }
        }

        /// <summary>Moves both the fake server clock and the client clock.</summary>
        public void Advance(TimeSpan duration)
        {
            Clock.Advance(duration);
            TimeProvider.Advance(duration);
        }

        public void Dispose()
        {
            HttpClient.Dispose();
            Server.Dispose();
        }
    }

    private sealed class ScriptedCredential(string token, bool fails) : TokenCredential
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
            return fails
                ? throw new InvalidOperationException(
                    "scripted credential failure with tenant 11111111 and endpoint detail")
                : new AccessToken(token, DateTimeOffset.MaxValue);
        }

        public override ValueTask<AccessToken> GetTokenAsync(
            TokenRequestContext requestContext,
            CancellationToken cancellationToken) =>
            new(GetToken(requestContext, cancellationToken));
    }

    private sealed class AdvanceableTimeProvider(DateTimeOffset start) : TimeProvider
    {
        private readonly Lock _gate = new();
        private DateTimeOffset _now = start;

        public override DateTimeOffset GetUtcNow()
        {
            lock (_gate)
            {
                return _now;
            }
        }

        public void Advance(TimeSpan duration)
        {
            lock (_gate)
            {
                _now = _now.Add(duration);
            }
        }
    }
}
