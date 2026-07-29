using System.Collections.Concurrent;
using System.Diagnostics.Metrics;
using System.Text.Json;
using Sapphirus.DesktopSupportApi.Model;
using Sapphirus.DesktopSupportApi.Observability;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Model;

/// <summary>
/// FND-031. The model plane's metrics stay low cardinality, its problem codes
/// come from a reviewed closed vocabulary, and synthetic canaries never reach a
/// dimension, a problem document, or an outcome string.
/// </summary>
public sealed class SupportPlaneModelTelemetryTests
{
    [Theory]
    [InlineData("terminal_type", "completed")]
    [InlineData("cancellation_source", "user")]
    [InlineData("request_mode", "streaming_chat")]
    [InlineData("credential_mechanism", "managed_identity")]
    [InlineData("catalog_state", "fresh")]
    [InlineData("usage_provenance", "reported")]
    public void The_new_tag_keys_are_allowlisted_and_carry_closed_vocabularies(
        string key,
        string value)
    {
        KeyValuePair<string, object?> tag = SupportPlaneTelemetry.SafeTag(key, value);

        Assert.Equal(key, tag.Key);
        Assert.Equal(value, tag.Value);
    }

    [Theory]
    [InlineData("deployment_name")]
    [InlineData("project_endpoint")]
    [InlineData("prompt")]
    [InlineData("run_id")]
    public void A_tag_key_nobody_reviewed_still_throws(string key)
    {
        Assert.Throws<InvalidOperationException>(() =>
            SupportPlaneTelemetry.SafeTag(key, "anything"));
    }

    [Fact]
    public void A_canary_value_never_becomes_a_metric_dimension()
    {
        Assert.Equal(
            "invalid_dimension",
            SupportPlaneTelemetry.SafeTag(
                "error_code",
                FoundryResponsesTestFixtures.Hash).Value);
        Assert.Equal(
            "invalid_dimension",
            SupportPlaneTelemetry.SafeTag("error_code", "C:\\Users\\someone\\repo").Value);
        Assert.Equal(
            "invalid_dimension",
            SupportPlaneTelemetry.SafeTag(
                "terminal_type",
                "eyJhbGciOiJFUzI1NiJ9.payload").Value);
    }

    [Fact]
    public void Unavailable_usage_records_nothing_rather_than_a_free_zero()
    {
        using SupportPlaneTelemetry telemetry = new();
        ConcurrentQueue<(string Instrument, long Value)> measurements = new();
        using MeterListener listener = CreateListener(telemetry, measurements);
        listener.Start();

        telemetry.RecordUsageV2(
            ModelUsageV2.Unavailable("EUR", "foundry-price-2026-07"),
            retryCount: 0,
            "planner",
            "interactive-standard");
        listener.RecordObservableInstruments();

        Assert.DoesNotContain(
            measurements,
            entry => entry.Instrument.Contains("usage", StringComparison.Ordinal));
    }

    [Fact]
    public void Reported_usage_records_only_the_categories_the_provider_named()
    {
        using SupportPlaneTelemetry telemetry = new();
        ConcurrentQueue<(string Instrument, long Value)> measurements = new();
        using MeterListener listener = CreateListener(telemetry, measurements);
        listener.Start();

        telemetry.RecordUsageV2(
            new ModelUsageV2(
                ModelUsageProvenance.Reported,
                InputTokens: 100,
                OutputTokens: 50,
                ReasoningTokens: null,
                CacheReadTokens: null,
                CacheWriteTokens: null,
                CostMicrounits: 825,
                "EUR",
                "foundry-price-2026-07"),
            retryCount: 1,
            "planner",
            "interactive-standard");

        Assert.Contains(
            measurements,
            entry => entry.Instrument.EndsWith("input_tokens", StringComparison.Ordinal)
                && entry.Value == 100);
        Assert.DoesNotContain(
            measurements,
            entry => entry.Instrument.EndsWith("reasoning_tokens", StringComparison.Ordinal));
        Assert.DoesNotContain(
            measurements,
            entry => entry.Instrument.EndsWith("cache_read_tokens", StringComparison.Ordinal));
    }

    [Theory]
    [InlineData("authentication_failed", "foundry_authentication_failed")]
    [InlineData("provider_unavailable", "foundry_project_unavailable")]
    [InlineData("rate_limited", "model_rate_limited")]
    [InlineData("quota_exhausted", "model_quota_exhausted")]
    [InlineData("content_filtered", "model_content_filtered")]
    [InlineData("provider_refusal", "model_refused")]
    [InlineData("stream_invalid", "model_stream_invalid")]
    [InlineData("deployment_missing", "model_deployment_missing")]
    [InlineData("capability_unsupported", "model_capability_mismatch")]
    [InlineData("context_limit", "model_context_limit_exceeded")]
    public void Every_provider_outcome_maps_onto_a_reviewed_problem_code(
        string outcome,
        string expectedCode)
    {
        (string code, int status) = SupportProblemCodes.ForProviderOutcome(outcome);

        Assert.Equal(expectedCode, code);
        Assert.True(SupportProblemCodes.IsKnown(code));
        Assert.InRange(status, 400, 599);
    }

    [Fact]
    public void The_plan_vocabulary_is_present_in_full()
    {
        string[] planCodes =
        [
            "foundry_authentication_failed",
            "foundry_project_unavailable",
            "model_rate_limited",
            "model_quota_exhausted",
            "model_content_filtered",
            "model_refused",
            "model_stream_invalid",
            "model_call_uncertain",
            "model_deployment_missing",
            "model_capability_mismatch",
            "model_context_limit_exceeded",
        ];

        Assert.All(planCodes, code => Assert.True(SupportProblemCodes.IsKnown(code)));
    }

    [Fact]
    public void An_outcome_outside_the_vocabulary_never_leaks_through_as_a_problem_type()
    {
        (string code, _) = SupportProblemCodes.ForProviderOutcome(
            FoundryResponsesTestFixtures.BodyCanary);
        object problem = RequestGuards.SafeModelProblem(
            FoundryResponsesTestFixtures.BodyCanary,
            500);

        Assert.Equal(SupportProblemCodes.InternalError, code);
        string rendered = JsonSerializer.Serialize(problem);
        Assert.DoesNotContain(
            FoundryResponsesTestFixtures.BodyCanary,
            rendered,
            StringComparison.Ordinal);
        Assert.Contains("internal_error", rendered, StringComparison.Ordinal);
    }

    [Fact]
    public void A_problem_document_carries_only_a_code_a_title_and_a_status()
    {
        object problem = RequestGuards.SafeModelProblem(
            SupportProblemCodes.ModelStreamInvalid,
            502);

        using JsonDocument document = JsonDocument.Parse(JsonSerializer.Serialize(problem));
        Assert.Equal(
            ["type", "title", "status", "code"],
            document.RootElement.EnumerateObject().Select(item => item.Name).ToArray());
        Assert.Equal(
            "model_stream_invalid",
            document.RootElement.GetProperty("code").GetString());
    }

    /// <summary>
    /// A listener bound to exactly one <see cref="SupportPlaneTelemetry"/>.
    /// Every instance publishes under the same meter name, so matching on the
    /// name would also enable the instruments of instances built by test
    /// classes xUnit is running in parallel with this one, and their
    /// measurements would arrive here on another thread and be read as ours.
    /// Comparing the meter reference admits this instance alone; the queue then
    /// keeps the sink safe against any callback the runtime may deliver
    /// concurrently.
    /// </summary>
    private static MeterListener CreateListener(
        SupportPlaneTelemetry telemetry,
        ConcurrentQueue<(string Instrument, long Value)> measurements)
    {
        MeterListener listener = new()
        {
            InstrumentPublished = (instrument, activeListener) =>
            {
                if (ReferenceEquals(instrument.Meter, telemetry.MeterInstance))
                {
                    activeListener.EnableMeasurementEvents(instrument);
                }
            },
        };
        listener.SetMeasurementEventCallback<long>(
            (instrument, measurement, _, _) =>
                measurements.Enqueue((instrument.Name, measurement)));
        return listener;
    }
}
