using System.Net;
using System.Text;
using System.Text.Json;
using Sapphirus.DesktopSupportApi.Foundry;
using Sapphirus.DesktopSupportApi.Model;
using Sapphirus.DesktopSupportApi.Signing;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Model;

/// <summary>
/// FND-029 and FND-030 together: one run produces strictly sequenced events,
/// exactly one terminal event, and a signed receipt only when it actually
/// succeeded.
/// </summary>
public sealed class ModelStreamingCoordinatorTests
{
    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    [Fact]
    public async Task A_successful_run_ends_with_one_completed_event_and_a_receipt()
    {
        RecordingSink sink = new();
        ModelStreamingCoordinator coordinator = CreateCoordinator(
            new FakeStreamTransport([SuccessTranscript()]));

        ModelStreamRunResult result = await coordinator.RunAsync(
            FoundryResponsesTestFixtures.Call(),
            sink,
            Ct);

        Assert.True(result.Succeeded);
        Assert.NotNull(result.Receipt);
        Assert.Equal(ModelTerminalStatuses.Succeeded, result.Receipt!.TerminalStatus);
        Assert.Single(sink.Events, item => item.IsTerminal);
        Assert.IsType<ModelStreamCompletedEvent>(sink.Events[^1]);
        Assert.Equal(
            Enumerable.Range(1, sink.Events.Count).ToArray(),
            sink.Events.Select(item => item.Sequence).ToArray());
    }

    [Fact]
    public async Task The_receipt_lane_reports_the_stream_route_and_the_final_sequence()
    {
        RecordingSink sink = new();
        ModelStreamingCoordinator coordinator = CreateCoordinator(
            new FakeStreamTransport([SuccessTranscript()]));

        ModelStreamRunResult result = await coordinator.RunAsync(
            FoundryResponsesTestFixtures.Call(),
            sink,
            Ct);

        StreamingChatLane lane = Assert.IsType<StreamingChatLane>(result.Receipt!.ResponseLane);
        Assert.Equal(ModelSupportRoutes.ModelStream, lane.SupportRoute);
        Assert.Equal(ModelRequestModes.StreamingChat, lane.RequestMode);
        Assert.Equal(result.FinalSequence, lane.FinalSequence);
        Assert.Equal(sink.Events.Count, result.EventCount);
    }

    [Fact]
    public async Task A_run_under_the_development_key_binds_differently_from_managed_identity()
    {
        RecordingSink managedSink = new();
        RecordingSink developmentSink = new();
        ModelStreamRunResult managed = await CreateCoordinator(
                new FakeStreamTransport([SuccessTranscript()]),
                FoundryCredentialMechanisms.ManagedIdentity)
            .RunAsync(
                FoundryResponsesTestFixtures.Call(
                    credentialMechanismId: FoundryCredentialMechanisms.ManagedIdentity),
                managedSink,
                Ct);
        ModelStreamRunResult development = await CreateCoordinator(
                new FakeStreamTransport([SuccessTranscript()]),
                FoundryCredentialMechanisms.DevelopmentApiKey)
            .RunAsync(
                FoundryResponsesTestFixtures.Call(
                    credentialMechanismId: FoundryCredentialMechanisms.DevelopmentApiKey),
                developmentSink,
                Ct);

        Assert.NotEqual(
            managed.Receipt!.CredentialBindingHash,
            development.Receipt!.CredentialBindingHash);
        Assert.NotEqual(
            managed.Receipt.ProviderProfileHash,
            development.Receipt.ProviderProfileHash);
    }

    [Fact]
    public async Task A_refused_run_produces_evidence_but_no_receipt()
    {
        RecordingSink sink = new();
        ModelStreamingCoordinator coordinator = CreateCoordinator(
            new FakeStreamTransport([RefusedTranscript()]));

        ModelStreamRunResult result = await coordinator.RunAsync(
            FoundryResponsesTestFixtures.Call(),
            sink,
            Ct);

        ModelStreamRefusedEvent refused = Assert.IsType<ModelStreamRefusedEvent>(result.Terminal);
        Assert.Equal(ModelStreamRefusalCodes.ContentFiltered, refused.RefusalCode);
        Assert.Null(result.Receipt);
        Assert.Equal(SupportProblemCodes.ModelContentFiltered, result.ProblemCode);
        Assert.Single(sink.Events, item => item.IsTerminal);
    }

    [Fact]
    public async Task An_unnormalizable_transcript_fails_the_run_with_a_reviewed_detail()
    {
        RecordingSink sink = new();
        ModelStreamingCoordinator coordinator = CreateCoordinator(
            new FakeStreamTransport([
                FoundryResponsesTestFixtures.ResponseScenarioBody("malformed_sse"),
            ]));

        ModelStreamRunResult result = await coordinator.RunAsync(
            FoundryResponsesTestFixtures.Call(),
            sink,
            Ct);

        ModelStreamFailedEvent failed = Assert.IsType<ModelStreamFailedEvent>(result.Terminal);
        Assert.Equal(ModelStreamFailureCodes.OutputSchemaViolation, failed.FailureCode);
        Assert.False(failed.Retryable);
        Assert.Equal(
            ModelStreamingCoordinator.TranscriptRejectedDetail,
            failed.FailureDetail);
        // The desktop gets a code from the closed stream vocabulary; the
        // operator gets the truthful support-plane code.
        Assert.Equal(SupportProblemCodes.ModelStreamInvalid, result.ProblemCode);
        Assert.Null(result.Receipt);
    }

    [Fact]
    public async Task A_truncated_transcript_never_becomes_a_completed_run()
    {
        RecordingSink sink = new();
        ModelStreamingCoordinator coordinator = CreateCoordinator(
            new FakeStreamTransport([
                FoundryResponsesTestFixtures.ResponseScenarioBody("truncated_stream"),
            ]));

        ModelStreamRunResult result = await coordinator.RunAsync(
            FoundryResponsesTestFixtures.Call(),
            sink,
            Ct);

        Assert.IsType<ModelStreamFailedEvent>(result.Terminal);
        Assert.Null(result.Receipt);
    }

    [Fact]
    public async Task A_transport_failure_before_any_delta_is_retried_with_identical_bytes()
    {
        RecordingSink sink = new();
        FakeStreamTransport transport = new(
            [SuccessTranscript()],
            openFailures: [new ModelProviderException(ModelProviderOutcomes.Timeout, true)]);
        ModelStreamingCoordinator coordinator = CreateCoordinator(transport);

        ModelStreamRunResult result = await coordinator.RunAsync(
            FoundryResponsesTestFixtures.Call(),
            sink,
            Ct);

        Assert.True(result.Succeeded);
        Assert.Equal(2, transport.Requests.Count);
        Assert.Single(transport.Requests.Distinct());
        Assert.Equal(1, result.Receipt!.RetryCount);
    }

    [Fact]
    public async Task A_terminal_transport_failure_is_never_retried()
    {
        RecordingSink sink = new();
        FakeStreamTransport transport = new(
            [SuccessTranscript()],
            openFailures:
            [
                new ModelProviderException(ModelProviderOutcomes.DeploymentMissing, false),
            ]);
        ModelStreamingCoordinator coordinator = CreateCoordinator(transport);

        ModelStreamRunResult result = await coordinator.RunAsync(
            FoundryResponsesTestFixtures.Call(),
            sink,
            Ct);

        Assert.Single(transport.Requests);
        ModelStreamFailedEvent failed = Assert.IsType<ModelStreamFailedEvent>(result.Terminal);
        Assert.Equal(ModelStreamFailureCodes.ProfileUnavailable, failed.FailureCode);
        Assert.Equal(SupportProblemCodes.ModelDeploymentMissing, result.ProblemCode);
    }

    [Fact]
    public async Task A_desktop_disconnect_emits_exactly_one_cancelled_event_and_no_retry()
    {
        RecordingSink sink = new();
        using CancellationTokenSource cancellation = new();
        FakeStreamTransport transport = new([SuccessTranscript()], cancelOnOpen: cancellation);
        ModelStreamingCoordinator coordinator = CreateCoordinator(transport);

        ModelStreamRunResult result = await coordinator.RunAsync(
            FoundryResponsesTestFixtures.Call(),
            sink,
            cancellation.Token);

        ModelStreamCancelledEvent cancelled =
            Assert.IsType<ModelStreamCancelledEvent>(result.Terminal);
        Assert.Equal(ModelStreamCancellationSources.User, cancelled.CancellationSource);
        Assert.Single(transport.Requests);
        Assert.Null(result.Receipt);
        Assert.Single(sink.Events, item => item.IsTerminal);
        Assert.True(transport.Disposed);
    }

    [Fact]
    public async Task A_completed_run_with_no_visible_answer_is_not_a_success()
    {
        RecordingSink sink = new();
        ModelStreamingCoordinator coordinator = CreateCoordinator(
            new FakeStreamTransport([
                "event: response.completed\n"
                + "data: {\"type\":\"response.completed\",\"response\":"
                + "{\"id\":\"resp_empty\",\"status\":\"completed\"}}\n\n",
            ]));

        ModelStreamRunResult result = await coordinator.RunAsync(
            FoundryResponsesTestFixtures.Call(),
            sink,
            Ct);

        Assert.IsType<ModelStreamFailedEvent>(result.Terminal);
        Assert.Null(result.Receipt);
    }

    [Fact]
    public async Task No_forwarded_event_ever_carries_a_provider_body_or_reasoning_text()
    {
        RecordingSink sink = new();
        string transcript =
            "event: response.created\n"
            + "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_a\"}}\n\n"
            + "event: response.reasoning_summary_text.delta\n"
            + "data: {\"type\":\"response.reasoning_summary_text.delta\",\"delta\":\""
            + FoundryResponsesTestFixtures.ReasoningCanary + "\"}\n\n"
            + "event: response.output_text.delta\n"
            + "data: {\"type\":\"response.output_text.delta\",\"delta\":\"safe\"}\n\n"
            + "event: response.completed\n"
            + "data: {\"type\":\"response.completed\",\"response\":"
            + "{\"id\":\"resp_a\",\"status\":\"completed\"}}\n\n";
        ModelStreamingCoordinator coordinator = CreateCoordinator(
            new FakeStreamTransport([transcript]));

        ModelStreamRunResult result = await coordinator.RunAsync(
            FoundryResponsesTestFixtures.Call(),
            sink,
            Ct);

        string rendered = string.Join(
            "\n",
            sink.Events.Select(item => JsonSerializer.Serialize(item, item.GetType())));
        Assert.True(result.Succeeded);
        Assert.DoesNotContain(
            FoundryResponsesTestFixtures.ReasoningCanary,
            rendered,
            StringComparison.Ordinal);
        Assert.DoesNotContain(
            FoundryResponsesTestFixtures.ReasoningCanary,
            JsonSerializer.Serialize(result.Receipt),
            StringComparison.Ordinal);
    }

    [Fact]
    public void Every_rendered_frame_names_its_event_and_carries_only_closed_fields()
    {
        byte[] frame = ModelStreamEndpoint.RenderFrame(new ModelStreamOutputTextDeltaEvent(
            "request_01J00000000000000000000000",
            "run_01J00000000000000000000000",
            3,
            0,
            "safe"));

        string rendered = Encoding.UTF8.GetString(frame);
        Assert.StartsWith("event: output_text_delta\ndata: {", rendered, StringComparison.Ordinal);
        Assert.EndsWith("\n\n", rendered, StringComparison.Ordinal);
        Assert.Contains("\"type\":\"output_text_delta\"", rendered, StringComparison.Ordinal);
        Assert.Contains("\"sequence\":3", rendered, StringComparison.Ordinal);
        Assert.DoesNotContain("isTerminal", rendered, StringComparison.Ordinal);
    }

    [Fact]
    public void The_stream_route_is_a_fixed_reviewed_constant()
    {
        Assert.Equal("/desktop/v1/model-access/streams", ModelStreamEndpoint.FullRoute);
        Assert.Equal("/model-access/streams", ModelStreamEndpoint.RoutePattern);
        Assert.Equal("text/event-stream", ModelStreamEndpoint.EventStreamContentType);
    }

    private static string SuccessTranscript() =>
        FoundryResponsesTestFixtures.ResponseScenarioBody("streaming_text_deltas");

    private static string RefusedTranscript() =>
        "event: response.incomplete\n"
        + "data: {\"type\":\"response.incomplete\",\"response\":{\"id\":\"resp_f\","
        + "\"incomplete_details\":{\"reason\":\"content_filter\"}}}\n\n";

    private static ModelStreamingCoordinator CreateCoordinator(
        FakeStreamTransport transport,
        string credentialMechanismId = FoundryCredentialMechanisms.ManagedIdentity) => new(
        transport,
        new AzureModelReceiptSigner(
            new StubHashSigner(),
            new SupportPlaneOptions
            {
                Authority = "https://login.microsoftonline.com/"
                    + "11111111-1111-1111-1111-111111111111/v2.0",
                Audience = "api://22222222-2222-2222-2222-222222222222",
                ApprovedDesktopClientId = "33333333-3333-3333-3333-333333333333",
                Region = "westeurope",
            }),
        TimeProvider.System,
        ModelStreamLimits.Default,
        credentialMechanismId);

    private sealed class RecordingSink : IModelStreamSink
    {
        public List<IModelStreamEvent> Events { get; } = [];

        public ValueTask WriteAsync(
            IModelStreamEvent streamEvent,
            CancellationToken cancellationToken)
        {
            Events.Add(streamEvent);
            return ValueTask.CompletedTask;
        }
    }

    private sealed class FakeStreamTransport(
        IReadOnlyList<string> transcripts,
        IReadOnlyList<ModelProviderException>? openFailures = null,
        CancellationTokenSource? cancelOnOpen = null) : IModelStreamTransport
    {
        public List<ModelProviderRequest> Requests { get; } = [];

        public bool Disposed { get; private set; }

        public Task<FoundryStreamSession> OpenAsync(
            ModelProviderRequest request,
            CancellationToken cancellationToken)
        {
            Requests.Add(request);
            if (openFailures is not null && Requests.Count <= openFailures.Count)
            {
                return Task.FromException<FoundryStreamSession>(
                    openFailures[Requests.Count - 1]);
            }
            if (cancelOnOpen is not null)
            {
                Disposed = true;
                cancelOnOpen.Cancel();
                return Task.FromException<FoundryStreamSession>(
                    new OperationCanceledException(cancelOnOpen.Token));
            }
            string transcript = transcripts[
                Math.Min(Requests.Count - 1 - (openFailures?.Count ?? 0), transcripts.Count - 1)];
            HttpResponseMessage response = new(HttpStatusCode.OK);
            MemoryStream body = new(Encoding.UTF8.GetBytes(transcript));
            return Task.FromResult(new FoundryStreamSession(response, body));
        }
    }

    private sealed class StubHashSigner : IHashSigner
    {
        public string KeyId => "model-receipt-key-2026-07";

        public Task<string> SignAsync(byte[] sha256Digest, CancellationToken cancellationToken)
        {
            cancellationToken.ThrowIfCancellationRequested();
            return Task.FromResult(
                Convert.ToBase64String(sha256Digest)
                    .Replace('+', '-')
                    .Replace('/', '_')
                    .TrimEnd('='));
        }
    }
}
