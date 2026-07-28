using System.Text;
using System.Text.Json;
using Sapphirus.DesktopSupportApi.Model;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Model;

/// <summary>
/// FND-029. The normalizer turns a provider transcript into strictly sequenced
/// closed events, refuses anything it cannot represent faithfully, and never
/// lets private reasoning text out.
/// </summary>
public sealed class FoundryModelStreamNormalizerTests
{
    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    [Fact]
    public async Task A_text_stream_produces_a_start_then_ordered_deltas()
    {
        FoundryModelStreamNormalizer normalizer = CreateNormalizer();

        List<IModelStreamEvent> events = await NormalizeAsync(
            normalizer,
            FoundryResponsesTestFixtures.ResponseScenarioBody("streaming_text_deltas"));

        Assert.Collection(
            events,
            first =>
            {
                ModelStreamResponseStartedEvent started =
                    Assert.IsType<ModelStreamResponseStartedEvent>(first);
                Assert.Equal(1, started.Sequence);
                Assert.Equal("coding-balanced", started.ResolvedProfileId);
                Assert.Equal("balanced", started.ResolvedVariantId);
            },
            second =>
            {
                ModelStreamOutputTextDeltaEvent delta =
                    Assert.IsType<ModelStreamOutputTextDeltaEvent>(second);
                Assert.Equal(2, delta.Sequence);
                Assert.Equal("bounded", delta.TextDelta);
            });
        Assert.Equal("bounded", normalizer.Transcript);
        Assert.Equal("resp_stream_text", normalizer.ProviderResponseId);
    }

    [Fact]
    public async Task Tool_call_deltas_are_bound_to_a_started_call_and_hashed_at_the_end()
    {
        FoundryModelStreamNormalizer normalizer = CreateNormalizer();

        List<IModelStreamEvent> events = await NormalizeAsync(
            normalizer,
            FoundryResponsesTestFixtures.ResponseScenarioBody("streaming_tool_call_deltas"));

        ModelStreamToolCallStartedEvent started =
            Assert.IsType<ModelStreamToolCallStartedEvent>(events[0]);
        Assert.Equal("call_fixture", started.ToolCallId);
        Assert.Equal("lookup", started.ToolName);
        Assert.IsType<ModelStreamToolCallArgumentsDeltaEvent>(events[1]);
        Assert.IsType<ModelStreamToolCallArgumentsDeltaEvent>(events[2]);
        ModelStreamToolCallCompletedEvent completed =
            Assert.IsType<ModelStreamToolCallCompletedEvent>(events[3]);
        Assert.Equal("call_fixture", completed.ToolCallId);
        Assert.Equal(
            """{"query":"safe"}""".Length,
            completed.ArgumentsByteCount);
        Assert.Matches("^sha256:[0-9a-f]{64}$", completed.ArgumentsHash);
        Assert.Equal([1, 2, 3, 4], events.Select(item => item.Sequence).ToArray());
    }

    [Fact]
    public async Task Usage_is_read_only_at_an_authoritative_terminal_frame()
    {
        FoundryModelStreamNormalizer normalizer = CreateNormalizer();

        List<IModelStreamEvent> events = await NormalizeAsync(
            normalizer,
            FoundryResponsesTestFixtures.ResponseScenarioBody("streaming_usage"));

        ModelStreamUsageEvent usage = Assert.IsType<ModelStreamUsageEvent>(Assert.Single(events));
        Assert.Equal(ModelUsageProvenance.Reported, usage.Usage.UsageProvenance);
        Assert.Equal(9, usage.Usage.InputTokens);
        Assert.Equal(3, usage.Usage.OutputTokens);
        // The provider reported neither category, so both stay null rather
        // than being written as a free zero.
        Assert.Null(usage.Usage.ReasoningTokens);
        Assert.Null(usage.Usage.CacheReadTokens);
    }

    [Fact]
    public async Task A_malformed_frame_is_refused_rather_than_recovered()
    {
        FoundryModelStreamNormalizer normalizer = CreateNormalizer();

        ModelStreamInvalidException failure =
            await Assert.ThrowsAsync<ModelStreamInvalidException>(() => NormalizeAsync(
                normalizer,
                FoundryResponsesTestFixtures.ResponseScenarioBody("malformed_sse")));

        Assert.Equal(ModelStreamInvalidReasons.FrameMalformed, failure.Reason);
    }

    [Fact]
    public async Task A_truncated_stream_is_refused_at_end_of_body()
    {
        FoundryModelStreamNormalizer normalizer = CreateNormalizer();

        ModelStreamInvalidException failure =
            await Assert.ThrowsAsync<ModelStreamInvalidException>(() => NormalizeAsync(
                normalizer,
                FoundryResponsesTestFixtures.ResponseScenarioBody("truncated_stream")));

        Assert.Equal(ModelStreamInvalidReasons.FrameTruncated, failure.Reason);
    }

    [Fact]
    public void A_second_response_created_frame_is_a_duplicate_start()
    {
        FoundryModelStreamNormalizer normalizer = CreateNormalizer();
        FoundrySseFrame created = new(
            "response.created",
            """{"type":"response.created","response":{"id":"resp_a","status":"in_progress"}}""");
        _ = normalizer.Accept(created);

        ModelStreamInvalidException failure =
            Assert.Throws<ModelStreamInvalidException>(() => normalizer.Accept(created));

        Assert.Equal(ModelStreamInvalidReasons.DuplicateStart, failure.Reason);
    }

    [Fact]
    public void A_provider_sequence_that_goes_backwards_is_refused()
    {
        FoundryModelStreamNormalizer normalizer = CreateNormalizer();
        _ = normalizer.Accept(new FoundrySseFrame(
            "response.output_text.delta",
            """{"type":"response.output_text.delta","sequence_number":5,"delta":"a"}"""));

        ModelStreamInvalidException failure = Assert.Throws<ModelStreamInvalidException>(() =>
            normalizer.Accept(new FoundrySseFrame(
                "response.output_text.delta",
                """{"type":"response.output_text.delta","sequence_number":4,"delta":"b"}""")));

        Assert.Equal(ModelStreamInvalidReasons.SequenceRegression, failure.Reason);
    }

    [Fact]
    public void Data_after_a_terminal_event_is_refused()
    {
        FoundryModelStreamNormalizer normalizer = CreateNormalizer();
        _ = normalizer.Fail(ModelStreamFailureCodes.InternalError, retryable: false);

        ModelStreamInvalidException failure = Assert.Throws<ModelStreamInvalidException>(() =>
            normalizer.Accept(new FoundrySseFrame(
                "response.output_text.delta",
                """{"type":"response.output_text.delta","delta":"late"}""")));

        Assert.Equal(ModelStreamInvalidReasons.DataAfterTerminal, failure.Reason);
    }

    [Fact]
    public void Exactly_one_terminal_event_may_ever_be_minted()
    {
        FoundryModelStreamNormalizer normalizer = CreateNormalizer();
        _ = normalizer.Cancel(ModelStreamCancellationSources.User);

        ModelStreamInvalidException failure = Assert.Throws<ModelStreamInvalidException>(() =>
            normalizer.Fail(ModelStreamFailureCodes.InternalError, retryable: false));

        Assert.Equal(ModelStreamInvalidReasons.MultipleTerminal, failure.Reason);
    }

    [Fact]
    public void An_unknown_required_item_type_is_refused()
    {
        FoundryModelStreamNormalizer normalizer = CreateNormalizer();

        ModelStreamInvalidException failure = Assert.Throws<ModelStreamInvalidException>(() =>
            normalizer.Accept(new FoundrySseFrame(
                "response.output_item.added",
                """{"type":"response.output_item.added","item":{"id":"a1","type":"web_search_call"}}""")));

        Assert.Equal(ModelStreamInvalidReasons.UnknownItemType, failure.Reason);
    }

    [Fact]
    public void Arguments_for_a_call_the_plane_never_saw_start_are_refused()
    {
        FoundryModelStreamNormalizer normalizer = CreateNormalizer();

        ModelStreamInvalidException failure = Assert.Throws<ModelStreamInvalidException>(() =>
            normalizer.Accept(new FoundrySseFrame(
                "response.function_call_arguments.delta",
                """{"type":"response.function_call_arguments.delta","item_id":"ghost","delta":"{"}""")));

        Assert.Equal(ModelStreamInvalidReasons.ToolCallUnknown, failure.Reason);
    }

    [Fact]
    public void An_oversized_delta_is_refused_rather_than_forwarded()
    {
        FoundryModelStreamNormalizer normalizer = CreateNormalizer(
            new ModelStreamLimits(MaximumDeltaBytes: 8));

        ModelStreamInvalidException failure = Assert.Throws<ModelStreamInvalidException>(() =>
            normalizer.Accept(new FoundrySseFrame(
                "response.output_text.delta",
                """{"type":"response.output_text.delta","delta":"far too long for the bound"}""")));

        Assert.Equal(ModelStreamInvalidReasons.FrameMalformed, failure.Reason);
    }

    [Fact]
    public void The_accumulated_transcript_is_bounded()
    {
        FoundryModelStreamNormalizer normalizer = CreateNormalizer(
            new ModelStreamLimits(MaximumDeltaBytes: 16, MaximumTranscriptBytes: 8));

        ModelStreamInvalidException failure = Assert.Throws<ModelStreamInvalidException>(() =>
            normalizer.Accept(new FoundrySseFrame(
                "response.output_text.delta",
                """{"type":"response.output_text.delta","delta":"0123456789"}""")));

        Assert.Equal(ModelStreamInvalidReasons.TranscriptTooLarge, failure.Reason);
    }

    [Fact]
    public void Reasoning_is_observed_as_a_status_and_its_text_is_never_read()
    {
        FoundryModelStreamNormalizer normalizer = CreateNormalizer();
        string frame = JsonSerializer.Serialize(new
        {
            type = "response.reasoning_summary_text.delta",
            delta = FoundryResponsesTestFixtures.ReasoningCanary,
        });

        ModelStreamStep step = normalizer.Accept(
            new FoundrySseFrame("response.reasoning_summary_text.delta", frame));

        Assert.Empty(step.Events);
        Assert.True(normalizer.ReasoningObserved);
        Assert.Equal(0, normalizer.LastSequence);
        Assert.DoesNotContain(
            FoundryResponsesTestFixtures.ReasoningCanary,
            normalizer.Transcript,
            StringComparison.Ordinal);
        Assert.DoesNotContain(
            FoundryResponsesTestFixtures.ReasoningCanary,
            JsonSerializer.Serialize(step),
            StringComparison.Ordinal);
    }

    [Fact]
    public void An_unknown_event_name_is_refused_rather_than_ignored()
    {
        FoundryModelStreamNormalizer normalizer = CreateNormalizer();

        ModelStreamInvalidException failure = Assert.Throws<ModelStreamInvalidException>(() =>
            normalizer.Accept(new FoundrySseFrame(
                "response.invented",
                """{"type":"response.invented"}""")));

        Assert.Equal(ModelStreamInvalidReasons.UnknownEvent, failure.Reason);
    }

    [Fact]
    public void A_cancelled_run_carries_a_closed_cancellation_source()
    {
        FoundryModelStreamNormalizer normalizer = CreateNormalizer();

        ModelStreamCancelledEvent cancelled =
            normalizer.Cancel(ModelStreamCancellationSources.Host);

        Assert.Equal(ModelStreamCancellationSources.Host, cancelled.CancellationSource);
        Assert.Equal(1, cancelled.Sequence);
        Assert.True(cancelled.IsTerminal);
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            CreateNormalizer().Cancel("invented_source"));
    }

    [Fact]
    public void A_failure_code_outside_the_closed_vocabulary_is_refused()
    {
        FoundryModelStreamNormalizer normalizer = CreateNormalizer();

        Assert.Throws<ArgumentOutOfRangeException>(() =>
            normalizer.Fail("invented_failure", retryable: false));
    }

    private static FoundryModelStreamNormalizer CreateNormalizer(
        ModelStreamLimits? limits = null) => new(
        "request_01J00000000000000000000000",
        "run_01J00000000000000000000000",
        "coding-balanced",
        "balanced",
        TimeProvider.System,
        limits ?? ModelStreamLimits.Default,
        reported => ModelUsageProjector.Project(
            reported,
            FoundryResponsesTestFixtures.PriceProfile()));

    private static async Task<List<IModelStreamEvent>> NormalizeAsync(
        FoundryModelStreamNormalizer normalizer,
        string body)
    {
        List<IModelStreamEvent> events = [];
        using MemoryStream stream = new(Encoding.UTF8.GetBytes(body));
        await foreach (FoundrySseFrame frame in FoundrySseReader.ReadAsync(
            stream,
            ModelStreamLimits.Default,
            Ct))
        {
            events.AddRange(normalizer.Accept(frame).Events);
        }
        return events;
    }
}
