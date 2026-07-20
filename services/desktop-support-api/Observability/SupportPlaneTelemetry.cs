using System.Diagnostics;
using System.Diagnostics.Metrics;

namespace Sapphirus.DesktopSupportApi.Observability;

/// <summary>
/// The single telemetry surface for the support plane. Every metric
/// dimension passes through a hard allowlist: fixed keys, bounded safe
/// values, and never a subject, registration, request, receipt, or content
/// hash. Values that look like hashes, tokens, or paths are replaced with
/// <c>invalid_dimension</c> rather than exported.
/// </summary>
public sealed class SupportPlaneTelemetry : IDisposable
{
    public const string MeterName = "Sapphirus.DesktopSupport";

    private static readonly string[] AllowedTagKeys =
    [
        "outcome",
        "error_code",
        "dependency",
        "provider_status_class",
        "model_role",
        "budget_class",
        "region",
        "environment",
        "release",
        "http.route",
    ];

    private readonly Meter _meter;
    private readonly Counter<long> _authenticationOutcomes;
    private readonly Counter<long> _admissionDenials;
    private readonly Histogram<double> _dependencyLatency;
    private readonly Counter<long> _providerStatuses;
    private readonly Counter<long> _schemaOutcomes;
    private readonly Histogram<long> _retryCounts;
    private readonly Counter<long> _inputTokens;
    private readonly Counter<long> _outputTokens;
    private readonly Counter<long> _costMicrounits;
    private readonly Counter<long> _receiptsIssued;
    private readonly Counter<long> _replays;
    private readonly Counter<long> _revocations;

    public SupportPlaneTelemetry()
    {
        _meter = new Meter(MeterName);
        _authenticationOutcomes = _meter.CreateCounter<long>(
            "sapphirus.support.authentication.outcomes");
        _admissionDenials = _meter.CreateCounter<long>(
            "sapphirus.support.admission.denials");
        _dependencyLatency = _meter.CreateHistogram<double>(
            "sapphirus.support.dependency.latency",
            "ms");
        _providerStatuses = _meter.CreateCounter<long>(
            "sapphirus.support.provider.statuses");
        _schemaOutcomes = _meter.CreateCounter<long>(
            "sapphirus.support.schema.outcomes");
        _retryCounts = _meter.CreateHistogram<long>(
            "sapphirus.support.provider.retries");
        _inputTokens = _meter.CreateCounter<long>(
            "sapphirus.support.usage.input_tokens");
        _outputTokens = _meter.CreateCounter<long>(
            "sapphirus.support.usage.output_tokens");
        _costMicrounits = _meter.CreateCounter<long>(
            "sapphirus.support.usage.cost_microunits");
        _receiptsIssued = _meter.CreateCounter<long>(
            "sapphirus.support.receipts.issued");
        _replays = _meter.CreateCounter<long>(
            "sapphirus.support.replays.observed");
        _revocations = _meter.CreateCounter<long>(
            "sapphirus.support.revocations.observed");
    }

    public void RecordAuthenticationOutcome(bool succeeded) =>
        _authenticationOutcomes.Add(1, SafeTag("outcome", succeeded ? "ok" : "denied"));

    public void RecordAdmissionDenial(string errorCode) =>
        _admissionDenials.Add(1, SafeTag("error_code", errorCode));

    public void RecordDependencyLatency(
        string dependency,
        double milliseconds,
        bool succeeded) =>
        _dependencyLatency.Record(
            milliseconds,
            SafeTag("dependency", dependency),
            SafeTag("outcome", succeeded ? "ok" : "failed"));

    public void RecordProviderStatusClass(string statusClass) =>
        _providerStatuses.Add(1, SafeTag("provider_status_class", statusClass));

    public void RecordSchemaOutcome(bool valid) =>
        _schemaOutcomes.Add(1, SafeTag("outcome", valid ? "valid" : "invalid"));

    public void RecordUsage(
        long inputTokens,
        long outputTokens,
        long costMicrounits,
        int retryCount,
        string modelRole,
        string budgetClass)
    {
        KeyValuePair<string, object?> role = SafeTag("model_role", modelRole);
        KeyValuePair<string, object?> budget = SafeTag("budget_class", budgetClass);
        _inputTokens.Add(inputTokens, role, budget);
        _outputTokens.Add(outputTokens, role, budget);
        _costMicrounits.Add(costMicrounits, role, budget);
        _retryCounts.Record(retryCount, role);
    }

    public void RecordReceiptIssued() => _receiptsIssued.Add(1);

    public void RecordReplayObserved() => _replays.Add(1);

    public void RecordRevocationObserved() => _revocations.Add(1);

    /// <summary>
    /// Builds one allowlisted tag. Unknown keys throw (programming error);
    /// unsafe values are replaced with a fixed marker so high-cardinality or
    /// sensitive material can never become a metric dimension.
    /// </summary>
    internal static KeyValuePair<string, object?> SafeTag(string key, string value)
    {
        if (!AllowedTagKeys.Contains(key, StringComparer.Ordinal))
        {
            throw new InvalidOperationException(
                "Telemetry tag key is not on the allowlist.");
        }
        return new KeyValuePair<string, object?>(
            key,
            IsSafeDimensionValue(key, value) ? value : "invalid_dimension");
    }

    internal static bool IsSafeDimensionValue(string key, string value)
    {
        if (value.Length is < 1 or > 128)
        {
            return false;
        }
        if (value.Contains("sha256:", StringComparison.OrdinalIgnoreCase)
            || value.StartsWith("eyJ", StringComparison.Ordinal)
            || value.Contains('\\', StringComparison.Ordinal)
            || (key != "http.route" && value.Contains('/', StringComparison.Ordinal))
            || LooksLikeOpaqueBlob(value))
        {
            return false;
        }
        return true;
    }

    private static bool LooksLikeOpaqueBlob(string value) =>
        value.Length >= 40
        && value.All(static character =>
            char.IsAsciiLetterOrDigit(character) || character is '-' or '_' or '=');

    public void Dispose() => _meter.Dispose();

    /// <summary>Maps an activity/route candidate to a template-only value.</summary>
    public static bool IsRouteTemplate(string value) =>
        value.StartsWith("/desktop/v1/", StringComparison.Ordinal)
        && !value.Contains("dreg_", StringComparison.Ordinal)
        && !value.Contains("receipt_", StringComparison.Ordinal);
}
