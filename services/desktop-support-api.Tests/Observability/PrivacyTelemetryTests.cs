using System.Diagnostics;
using Microsoft.AspNetCore.Http;
using Microsoft.Extensions.Diagnostics.HealthChecks;
using Sapphirus.DesktopSupportApi.Health;
using Sapphirus.DesktopSupportApi.Observability;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Observability;

public sealed class PrivacyTelemetryTests
{
    private const string SourceCanary = "CANARY_SOURCE_9f31d2";
    private const string TokenCanary =
        "eyJhbGciOiJFUzI1NiJ9.CANARY_TOKEN_PAYLOAD.CANARY_TOKEN_SIGNATURE";
    private const string HashCanary =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    private const string PathCanary = @"C:\Users\rodrigo\secret\project.cs";
    private const string SignatureCanary =
        "MEUCIQDCANARYSIGNATUREBYTESAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

    [Fact]
    public void Metric_dimensions_reject_hashes_tokens_paths_and_blobs()
    {
        Assert.Equal(
            "invalid_dimension",
            SupportPlaneTelemetry.SafeTag("error_code", HashCanary).Value);
        Assert.Equal(
            "invalid_dimension",
            SupportPlaneTelemetry.SafeTag("error_code", TokenCanary).Value);
        Assert.Equal(
            "invalid_dimension",
            SupportPlaneTelemetry.SafeTag("error_code", PathCanary).Value);
        Assert.Equal(
            "invalid_dimension",
            SupportPlaneTelemetry.SafeTag("error_code", SignatureCanary).Value);
        Assert.Equal(
            "invalid_dimension",
            SupportPlaneTelemetry.SafeTag("error_code", "subjects/subject-a").Value);

        Assert.Equal(
            "consent_binding_rejected",
            SupportPlaneTelemetry.SafeTag("error_code", "consent_binding_rejected").Value);
        Assert.Equal(
            "/desktop/v1/model-access/calls",
            SupportPlaneTelemetry.SafeTag("http.route", "/desktop/v1/model-access/calls").Value);
    }

    [Fact]
    public void Unknown_metric_dimension_keys_are_a_hard_error()
    {
        Assert.Throws<InvalidOperationException>(() =>
            SupportPlaneTelemetry.SafeTag("subject_hash", "anything"));
        Assert.Throws<InvalidOperationException>(() =>
            SupportPlaneTelemetry.SafeTag("registration_id", "anything"));
        Assert.Throws<InvalidOperationException>(() =>
            SupportPlaneTelemetry.SafeTag("receipt_id", "anything"));
    }

    [Fact]
    public void Exported_spans_drop_canaries_events_and_raw_paths()
    {
        using ActivitySource source = new("privacy-test");
        using ActivityListener listener = new()
        {
            ShouldListenTo = activitySource => activitySource.Name == "privacy-test",
            Sample = (ref ActivityCreationOptions<ActivityContext> _) =>
                ActivitySamplingResult.AllDataAndRecorded,
        };
        ActivitySource.AddActivityListener(listener);
        using PrivacyRedactionProcessor processor = new();

        using Activity activity = source.StartActivity("POST /raw")!;
        activity.SetTag("http.route", "/desktop/v1/model-access/calls");
        activity.SetTag("http.request.method", "POST");
        activity.SetTag("url.path", "/desktop/v1/model-access/receipts/receipt_ABC123");
        activity.SetTag("sapphirus.error_code", "consent_binding_rejected");
        activity.SetTag("request.body", SourceCanary);
        activity.SetTag("authorization", TokenCanary);
        activity.SetTag("content.hash", HashCanary);
        activity.SetTag("file.path", PathCanary);
        activity.SetTag("proof.signature", SignatureCanary);
        activity.AddEvent(new ActivityEvent(
            "exception",
            tags: new ActivityTagsCollection
            {
                ["exception.message"] = "boom at " + PathCanary,
                ["exception.stacktrace"] = SourceCanary,
            }));
        activity.Stop();
        processor.OnEnd(activity);

        string exported = string.Join(
            "\n",
            activity.TagObjects.Select(tag => $"{tag.Key}={tag.Value}"))
            + "\n" + string.Join(
                "\n",
                activity.Events.SelectMany(activityEvent => activityEvent.Tags)
                    .Select(tag => $"{tag.Key}={tag.Value}"))
            + "\n" + activity.DisplayName;
        Assert.DoesNotContain(SourceCanary, exported);
        Assert.DoesNotContain(TokenCanary, exported);
        Assert.DoesNotContain(HashCanary, exported);
        Assert.DoesNotContain("rodrigo", exported);
        Assert.DoesNotContain(SignatureCanary, exported);
        Assert.DoesNotContain("receipt_ABC123", exported);
        Assert.Contains("/desktop/v1/model-access/calls", exported);
        Assert.Equal("/desktop/v1/model-access/calls", activity.DisplayName);
        Assert.Empty(activity.Events);
    }

    [Fact]
    public async Task Health_responses_disclose_only_status_and_dependency_classes()
    {
        HealthReport report = new(
            new Dictionary<string, HealthReportEntry>
            {
                [AzureDependencyHealthChecks.SqlDependency] = new(
                    HealthStatus.Unhealthy,
                    "server " + PathCanary + " unreachable",
                    TimeSpan.FromMilliseconds(12),
                    new InvalidOperationException(
                        "Login failed for " + TokenCanary),
                    new Dictionary<string, object> { ["endpoint"] = PathCanary }),
                [AzureDependencyHealthChecks.SigningDependency] = new(
                    HealthStatus.Healthy,
                    null,
                    TimeSpan.FromMilliseconds(1),
                    null,
                    null),
            },
            TimeSpan.FromMilliseconds(13));
        DefaultHttpContext context = new();
        using MemoryStream body = new();
        context.Response.Body = body;

        await AzureDependencyHealthChecks.WriteSafeResponseAsync(context, report);

        string payload = System.Text.Encoding.UTF8.GetString(body.ToArray());
        Assert.Contains("\"status\":\"unhealthy\"", payload);
        Assert.Contains("\"dependency\":\"sql\"", payload);
        Assert.Contains("\"dependency\":\"signing\"", payload);
        Assert.DoesNotContain("rodrigo", payload);
        Assert.DoesNotContain("Login failed", payload);
        Assert.DoesNotContain("endpoint", payload);
        Assert.DoesNotContain(TokenCanary, payload);
    }

    [Fact]
    public void Usage_metrics_accept_only_coarse_role_and_budget_dimensions()
    {
        using SupportPlaneTelemetry telemetry = new();
        // High-cardinality or sensitive values degrade to the fixed marker
        // instead of throwing, so recording never fails a request.
        telemetry.RecordUsage(100, 50, 3000, 1, "planner", "interactive-standard");
        telemetry.RecordUsage(100, 50, 3000, 1, HashCanary, TokenCanary);
        telemetry.RecordAdmissionDenial("context_limit_exceeded");
        telemetry.RecordDependencyLatency("sql", 12.5, succeeded: true);
        telemetry.RecordProviderStatusClass("429");
        telemetry.RecordReceiptIssued();
        telemetry.RecordReplayObserved();
        telemetry.RecordRevocationObserved();
    }
}
