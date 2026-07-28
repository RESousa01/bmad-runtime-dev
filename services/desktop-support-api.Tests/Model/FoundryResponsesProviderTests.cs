using System.Net;
using System.Text;
using System.Text.Json;
using Sapphirus.DesktopSupportApi.Foundry;
using Sapphirus.DesktopSupportApi.Model;
using Sapphirus.DesktopSupportApi.Tests.Foundry;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Model;

/// <summary>
/// FND-028. The Responses adapter replaces Chat Completions: it speaks the raw
/// project surface, authenticates only through the credential seam, and turns
/// every provider failure into a stable safe outcome that carries no body.
/// </summary>
public sealed class FoundryResponsesProviderTests
{
    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    [Fact]
    public void The_structured_body_carries_only_allowlisted_top_level_keys()
    {
        byte[] body = FoundryResponseRequestBuilder.Build(
            FoundryResponsesTestFixtures.StructuredRequest());

        using JsonDocument document = JsonDocument.Parse(body);
        string[] keys = document.RootElement
            .EnumerateObject()
            .Select(property => property.Name)
            .ToArray();
        Assert.All(
            keys,
            key => Assert.Contains(
                key,
                FoundryResponseRequestBuilder.TopLevelKeys,
                StringComparer.Ordinal));
    }

    [Fact]
    public void Structured_mode_disables_storage_streaming_and_every_tool()
    {
        byte[] body = FoundryResponseRequestBuilder.Build(
            FoundryResponsesTestFixtures.StructuredRequest());

        using JsonDocument document = JsonDocument.Parse(body);
        JsonElement root = document.RootElement;
        Assert.False(root.GetProperty("store").GetBoolean());
        Assert.False(root.GetProperty("stream").GetBoolean());
        Assert.False(root.GetProperty("parallel_tool_calls").GetBoolean());
        Assert.Equal("none", root.GetProperty("tool_choice").GetString());
        Assert.Empty(root.GetProperty("tools").EnumerateArray());
        JsonElement format = root.GetProperty("text").GetProperty("format");
        Assert.Equal("json_schema", format.GetProperty("type").GetString());
        Assert.True(format.GetProperty("strict").GetBoolean());
        Assert.Equal(JsonValueKind.Object, format.GetProperty("schema").ValueKind);
    }

    [Fact]
    public void Streaming_mode_carries_the_resolved_variant_options_and_no_schema()
    {
        ModelProviderTool tool = new(
            "lookup",
            "Look one identifier up.",
            """{"type":"object","additionalProperties":false,"properties":{}}""");

        byte[] body = FoundryResponseRequestBuilder.Build(
            FoundryResponsesTestFixtures.StreamingRequest(tools: [tool]));

        using JsonDocument document = JsonDocument.Parse(body);
        JsonElement root = document.RootElement;
        Assert.True(root.GetProperty("stream").GetBoolean());
        Assert.False(root.GetProperty("store").GetBoolean());
        Assert.Equal("medium", root.GetProperty("reasoning").GetProperty("effort").GetString());
        Assert.Equal("low", root.GetProperty("text").GetProperty("verbosity").GetString());
        Assert.False(root.GetProperty("text").TryGetProperty("format", out _));
        Assert.Equal("auto", root.GetProperty("tool_choice").GetString());
        JsonElement declared = Assert.Single(root.GetProperty("tools").EnumerateArray());
        Assert.Equal("function", declared.GetProperty("type").GetString());
        Assert.Equal("lookup", declared.GetProperty("name").GetString());
        Assert.True(declared.GetProperty("strict").GetBoolean());
    }

    [Fact]
    public void The_body_never_names_a_project_endpoint_a_url_or_a_file_identifier()
    {
        byte[] body = FoundryResponseRequestBuilder.Build(
            FoundryResponsesTestFixtures.StreamingRequest());

        string rendered = Encoding.UTF8.GetString(body);
        Assert.DoesNotContain("https://", rendered, StringComparison.Ordinal);
        Assert.DoesNotContain("file_id", rendered, StringComparison.Ordinal);
        Assert.DoesNotContain("image_url", rendered, StringComparison.Ordinal);
        Assert.DoesNotContain("api-version", rendered, StringComparison.Ordinal);
        Assert.DoesNotContain(
            FoundryResponsesTestFixtures.CredentialCanary,
            rendered,
            StringComparison.Ordinal);
    }

    [Theory]
    [InlineData("../escape")]
    [InlineData("deployment/with/path")]
    [InlineData("deployment?query=1")]
    [InlineData("")]
    public void A_deployment_name_that_is_not_a_plain_identifier_is_refused(string deployment)
    {
        ModelProviderException failure = Assert.Throws<ModelProviderException>(() =>
            FoundryResponseRequestBuilder.Build(
                FoundryResponsesTestFixtures.StructuredRequest(deployment)));

        Assert.Equal(ModelProviderOutcomes.DeploymentMissing, failure.Outcome);
        Assert.False(failure.Retryable);
    }

    [Fact]
    public void A_structured_request_that_also_declares_tools_is_refused()
    {
        ModelProviderRequest request = FoundryResponsesTestFixtures.StructuredRequest() with
        {
            Tools = [new ModelProviderTool("lookup", "d", "{}")],
        };

        ModelProviderException failure = Assert.Throws<ModelProviderException>(() =>
            FoundryResponseRequestBuilder.Build(request));

        Assert.Equal(ModelProviderOutcomes.CapabilityUnsupported, failure.Outcome);
    }

    [Theory]
    [InlineData(HttpStatusCode.Unauthorized, ModelProviderOutcomes.AuthenticationFailed, false)]
    [InlineData(HttpStatusCode.Forbidden, ModelProviderOutcomes.AuthenticationFailed, false)]
    [InlineData(HttpStatusCode.NotFound, ModelProviderOutcomes.DeploymentMissing, false)]
    [InlineData(HttpStatusCode.RequestTimeout, ModelProviderOutcomes.Timeout, true)]
    [InlineData(HttpStatusCode.TooManyRequests, ModelProviderOutcomes.RateLimited, true)]
    [InlineData(HttpStatusCode.NotImplemented, ModelProviderOutcomes.CapabilityUnsupported, false)]
    [InlineData(
        HttpStatusCode.InternalServerError,
        ModelProviderOutcomes.ProviderUnavailable,
        true)]
    [InlineData(HttpStatusCode.BadGateway, ModelProviderOutcomes.ProviderUnavailable, true)]
    [InlineData(
        HttpStatusCode.ServiceUnavailable,
        ModelProviderOutcomes.ProviderUnavailable,
        true)]
    [InlineData(HttpStatusCode.GatewayTimeout, ModelProviderOutcomes.ProviderUnavailable, true)]
    [InlineData(HttpStatusCode.RequestEntityTooLarge, ModelProviderOutcomes.ContextLimit, false)]
    public void Provider_statuses_map_to_stable_outcomes_and_retry_classes(
        HttpStatusCode statusCode,
        string expectedOutcome,
        bool expectedRetryable)
    {
        FoundryResponseClassification classification =
            FoundryResponseParser.ClassifyStatus(statusCode, []);

        Assert.Equal(expectedOutcome, classification.Outcome);
        Assert.Equal(expectedRetryable, classification.Retryable);
    }

    [Fact]
    public void A_content_filter_four_hundred_is_a_filtered_outcome_and_never_retried()
    {
        byte[] body = Encoding.UTF8.GetBytes(
            FoundryResponsesTestFixtures.ResponseScenarioBody("content_filter"));

        FoundryResponseClassification classification =
            FoundryResponseParser.ClassifyStatus(HttpStatusCode.BadRequest, body);

        Assert.Equal(ModelProviderOutcomes.ContentFiltered, classification.Outcome);
        Assert.False(classification.Retryable);
    }

    [Fact]
    public void An_exhausted_quota_is_separated_from_ordinary_throttling()
    {
        byte[] body = Encoding.UTF8.GetBytes(
            """{"error":{"code":"insufficient_quota","message":"fixture"}}""");

        FoundryResponseClassification throttled =
            FoundryResponseParser.ClassifyStatus(HttpStatusCode.TooManyRequests, []);
        FoundryResponseClassification exhausted =
            FoundryResponseParser.ClassifyStatus(HttpStatusCode.TooManyRequests, body);

        Assert.Equal(ModelProviderOutcomes.RateLimited, throttled.Outcome);
        Assert.True(throttled.Retryable);
        Assert.Equal(ModelProviderOutcomes.QuotaExhausted, exhausted.Outcome);
        Assert.False(exhausted.Retryable);
    }

    [Fact]
    public async Task A_structured_call_reaches_the_responses_route_and_parses_the_answer()
    {
        FakeFoundryTestClock clock = new(DateTimeOffset.UnixEpoch);
        using FakeFoundryServer server = new("Test", clock);
        using HttpClient client = FoundryResponsesTestFixtures.CreateResponsesClient(server);
        FoundryResponsesTestFixtures.FakeFoundryAuthenticator authenticator = new();
        FoundryResponsesProviderExecutor executor = new(client, authenticator);
        server.Enqueue(FoundryResponsesTestFixtures.JsonReply(
            HttpStatusCode.OK,
            FoundryResponsesTestFixtures.ResponseScenarioBody("structured_output")));

        ModelProviderResponse response = await executor.ExecuteAsync(
            FoundryResponsesTestFixtures.StructuredRequest(),
            Ct);

        Assert.Equal("responses", Assert.Single(server.RequestReceipts).Route);
        Assert.Equal("POST", server.RequestReceipts[0].Method);
        Assert.Equal(1, authenticator.OperationCount);
        Assert.Equal("stop", response.FinishReason);
        Assert.Equal("""{"summary":"bounded plan"}""", response.OutputJson);
        Assert.Equal(12, response.ReportedUsage!.InputTokens);
        Assert.Equal(5, response.ReportedUsage.OutputTokens);
        Assert.Null(response.ReportedUsage.CacheReadTokens);
        Assert.Equal("resp_fixture_001", response.ProviderRequestId);
    }

    [Fact]
    public async Task A_refusal_is_reported_without_republishing_what_the_model_said()
    {
        FakeFoundryTestClock clock = new(DateTimeOffset.UnixEpoch);
        using FakeFoundryServer server = new("Test", clock);
        using HttpClient client = FoundryResponsesTestFixtures.CreateResponsesClient(server);
        FoundryResponsesProviderExecutor executor = new(
            client,
            new FoundryResponsesTestFixtures.FakeFoundryAuthenticator());
        server.Enqueue(FoundryResponsesTestFixtures.JsonReply(
            HttpStatusCode.OK,
            FoundryResponsesTestFixtures.ResponseScenarioBody("refusal")));

        ModelProviderResponse response = await executor.ExecuteAsync(
            FoundryResponsesTestFixtures.StructuredRequest(),
            Ct);

        Assert.Equal("refusal", response.FinishReason);
        Assert.Equal("", response.OutputJson);
        Assert.DoesNotContain(
            "fixture refusal",
            JsonSerializer.Serialize(response),
            StringComparison.Ordinal);
    }

    [Theory]
    [InlineData("rate_limited_429", HttpStatusCode.TooManyRequests, "rate_limited")]
    [InlineData("forbidden_403", HttpStatusCode.Forbidden, "authentication_failed")]
    [InlineData("deployment_missing_404", HttpStatusCode.NotFound, "deployment_missing")]
    [InlineData("server_error_500", HttpStatusCode.InternalServerError, "provider_unavailable")]
    public async Task Failure_fixtures_surface_only_a_safe_outcome(
        string scenario,
        HttpStatusCode statusCode,
        string expectedOutcome)
    {
        FakeFoundryTestClock clock = new(DateTimeOffset.UnixEpoch);
        using FakeFoundryServer server = new("Test", clock);
        using HttpClient client = FoundryResponsesTestFixtures.CreateResponsesClient(server);
        FoundryResponsesProviderExecutor executor = new(
            client,
            new FoundryResponsesTestFixtures.FakeFoundryAuthenticator());
        string body = FoundryResponsesTestFixtures.ResponseScenarioBody(scenario);
        server.Enqueue(FoundryResponsesTestFixtures.JsonReply(statusCode, body));

        ModelProviderException failure =
            await Assert.ThrowsAsync<ModelProviderException>(() =>
                executor.ExecuteAsync(FoundryResponsesTestFixtures.StructuredRequest(), Ct));

        Assert.Equal(expectedOutcome, failure.Outcome);
        Assert.DoesNotContain("fixture", failure.Message, StringComparison.Ordinal);
        Assert.DoesNotContain("fixture", failure.ToString(), StringComparison.Ordinal);
    }

    [Fact]
    public async Task A_credential_that_cannot_be_acquired_never_sends_an_unauthenticated_call()
    {
        FakeFoundryTestClock clock = new(DateTimeOffset.UnixEpoch);
        using FakeFoundryServer server = new("Test", clock);
        using HttpClient client = FoundryResponsesTestFixtures.CreateResponsesClient(server);
        FoundryResponsesProviderExecutor executor = new(
            client,
            new FoundryResponsesTestFixtures.FailingFoundryAuthenticator());

        ModelProviderException failure =
            await Assert.ThrowsAsync<ModelProviderException>(() =>
                executor.ExecuteAsync(FoundryResponsesTestFixtures.StructuredRequest(), Ct));

        Assert.Equal(ModelProviderOutcomes.AuthenticationFailed, failure.Outcome);
        Assert.Empty(server.RequestReceipts);
        Assert.DoesNotContain(
            FoundryResponsesTestFixtures.CredentialCanary,
            failure.ToString(),
            StringComparison.Ordinal);
    }

    [Fact]
    public async Task A_client_cancellation_propagates_rather_than_becoming_an_outcome()
    {
        FakeFoundryTestClock clock = new(DateTimeOffset.UnixEpoch);
        using FakeFoundryServer server = new("Test", clock);
        using HttpClient client = FoundryResponsesTestFixtures.CreateResponsesClient(server);
        FoundryResponsesProviderExecutor executor = new(
            client,
            new FoundryResponsesTestFixtures.FakeFoundryAuthenticator());
        server.Enqueue(FakeFoundryReply.WaitForCancellation());
        using CancellationTokenSource cancellation = new(TimeSpan.FromMilliseconds(50));

        await Assert.ThrowsAnyAsync<OperationCanceledException>(() =>
            executor.ExecuteAsync(
                FoundryResponsesTestFixtures.StructuredRequest(),
                cancellation.Token));
    }

    [Fact]
    public async Task A_streaming_request_on_the_structured_seam_is_refused()
    {
        FakeFoundryTestClock clock = new(DateTimeOffset.UnixEpoch);
        using FakeFoundryServer server = new("Test", clock);
        using HttpClient client = FoundryResponsesTestFixtures.CreateResponsesClient(server);
        FoundryResponsesProviderExecutor executor = new(
            client,
            new FoundryResponsesTestFixtures.FakeFoundryAuthenticator());

        ModelProviderException failure =
            await Assert.ThrowsAsync<ModelProviderException>(() =>
                executor.ExecuteAsync(FoundryResponsesTestFixtures.StreamingRequest(), Ct));

        Assert.Equal(ModelProviderOutcomes.CapabilityUnsupported, failure.Outcome);
        Assert.Empty(server.RequestReceipts);
    }

    [Fact]
    public async Task A_streaming_call_answered_with_a_buffered_document_is_an_invalid_stream()
    {
        FakeFoundryTestClock clock = new(DateTimeOffset.UnixEpoch);
        using FakeFoundryServer server = new("Test", clock);
        using HttpClient client = FoundryResponsesTestFixtures.CreateResponsesClient(server);
        FoundryResponsesProviderExecutor executor = new(
            client,
            new FoundryResponsesTestFixtures.FakeFoundryAuthenticator());
        server.Enqueue(FoundryResponsesTestFixtures.JsonReply(
            HttpStatusCode.OK,
            FoundryResponsesTestFixtures.ResponseScenarioBody("structured_output")));

        ModelProviderException failure =
            await Assert.ThrowsAsync<ModelProviderException>(() =>
                executor.OpenAsync(FoundryResponsesTestFixtures.StreamingRequest(), Ct));

        Assert.Equal(ModelProviderOutcomes.StreamInvalid, failure.Outcome);
    }

    [Fact]
    public async Task A_streaming_call_opens_the_event_stream_body_for_the_normalizer()
    {
        FakeFoundryTestClock clock = new(DateTimeOffset.UnixEpoch);
        using FakeFoundryServer server = new("Test", clock);
        using HttpClient client = FoundryResponsesTestFixtures.CreateResponsesClient(server);
        FoundryResponsesProviderExecutor executor = new(
            client,
            new FoundryResponsesTestFixtures.FakeFoundryAuthenticator());
        Assert.True(FoundryResponsesTestFixtures.IsEventStreamScenario("streaming_text_deltas"));
        server.Enqueue(FoundryResponsesTestFixtures.EventStreamReply(
            FoundryResponsesTestFixtures.ResponseScenarioBody("streaming_text_deltas")));

        await using FoundryStreamSession session = await executor.OpenAsync(
            FoundryResponsesTestFixtures.StreamingRequest(),
            Ct);

        using StreamReader reader = new(session.Body);
        string body = await reader.ReadToEndAsync(Ct);
        Assert.Contains("response.created", body, StringComparison.Ordinal);
        Assert.Equal("responses", Assert.Single(server.RequestReceipts).Route);
    }

    [Fact]
    public void The_executor_reports_its_credential_mechanism_and_never_the_credential()
    {
        FakeFoundryTestClock clock = new(DateTimeOffset.UnixEpoch);
        using FakeFoundryServer server = new("Test", clock);
        using HttpClient client = FoundryResponsesTestFixtures.CreateResponsesClient(server);
        FoundryResponsesProviderExecutor executor = new(
            client,
            new FoundryResponsesTestFixtures.FakeFoundryAuthenticator(
                FoundryCredentialMechanisms.DevelopmentApiKey));

        Assert.Equal(
            FoundryCredentialMechanisms.DevelopmentApiKey,
            executor.CredentialMechanismId);
    }

    [Fact]
    public void A_base_address_that_is_not_a_bounded_https_path_is_refused()
    {
        FakeFoundryTestClock clock = new(DateTimeOffset.UnixEpoch);
        using FakeFoundryServer server = new("Test", clock);
        using HttpClient client = new(server, disposeHandler: false)
        {
            BaseAddress = new Uri("http://forge-fake.test/api/projects/forge-test/openai/v1/"),
        };

        Assert.Throws<ArgumentException>(() => new FoundryResponsesProviderExecutor(
            client,
            new FoundryResponsesTestFixtures.FakeFoundryAuthenticator()));
    }
}
