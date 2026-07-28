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

    /// <summary>
    /// The hard tag allowlist. Every key here is deliberately low cardinality:
    /// a closed vocabulary, a status class, or a configured constant. Nothing
    /// derived from a subject, a request, a receipt, a deployment name, a
    /// project endpoint, or model content may ever appear, which is why adding
    /// a key is a decision rather than a convenience.
    /// </summary>
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

        // Added by FND-031. Each is a closed enumeration:
        //   terminal_type        four stream terminal variants
        //   cancellation_source  four cancellation sources
        //   request_mode         two request modes
        //   credential_mechanism two credential mechanisms
        //   catalog_state        four catalog states
        //   usage_provenance     three usage provenances
        "terminal_type",
        "cancellation_source",
        "request_mode",
        "credential_mechanism",
        "catalog_state",
        "usage_provenance",
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
    private readonly Counter<long> _catalogRefreshes;
    private readonly Histogram<double> _catalogAge;
    private readonly Histogram<long> _effectiveProfiles;
    private readonly Counter<long> _routeResolutionFailures;
    private readonly Histogram<double> _streamDuration;
    private readonly Counter<long> _streamCancellations;
    private readonly Counter<long> _receiptOutcomes;
    private readonly Counter<long> _reasoningTokens;
    private readonly Counter<long> _cacheReadTokens;

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
        _catalogRefreshes = _meter.CreateCounter<long>(
            "sapphirus.support.catalog.refreshes");
        _catalogAge = _meter.CreateHistogram<double>(
            "sapphirus.support.catalog.age",
            "s");
        _effectiveProfiles = _meter.CreateHistogram<long>(
            "sapphirus.support.catalog.effective_profiles");
        _routeResolutionFailures = _meter.CreateCounter<long>(
            "sapphirus.support.route.resolution_failures");
        _streamDuration = _meter.CreateHistogram<double>(
            "sapphirus.support.stream.duration",
            "ms");
        _streamCancellations = _meter.CreateCounter<long>(
            "sapphirus.support.stream.cancellations");
        _receiptOutcomes = _meter.CreateCounter<long>(
            "sapphirus.support.receipts.outcomes");
        _reasoningTokens = _meter.CreateCounter<long>(
            "sapphirus.support.usage.reasoning_tokens");
        _cacheReadTokens = _meter.CreateCounter<long>(
            "sapphirus.support.usage.cache_read_tokens");
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

    /// <summary>
    /// Version two usage. A category the provider never reported contributes
    /// nothing rather than a zero, so a dashboard cannot read silence as a
    /// free call, and the provenance is carried as a dimension so reported and
    /// estimated totals are never summed together by accident.
    /// </summary>
    public void RecordUsageV2(
        ModelUsageV2 usage,
        int retryCount,
        string modelRole,
        string budgetClass)
    {
        ArgumentNullException.ThrowIfNull(usage);
        KeyValuePair<string, object?> role = SafeTag("model_role", modelRole);
        KeyValuePair<string, object?> budget = SafeTag("budget_class", budgetClass);
        KeyValuePair<string, object?> provenance =
            SafeTag("usage_provenance", usage.UsageProvenance);
        if (usage.InputTokens is long input)
        {
            _inputTokens.Add(input, role, budget, provenance);
        }
        if (usage.OutputTokens is long output)
        {
            _outputTokens.Add(output, role, budget, provenance);
        }
        if (usage.ReasoningTokens is long reasoning)
        {
            _reasoningTokens.Add(reasoning, role, budget, provenance);
        }
        if (usage.CacheReadTokens is long cacheRead)
        {
            _cacheReadTokens.Add(cacheRead, role, budget, provenance);
        }
        if (usage.CostMicrounits is long cost)
        {
            _costMicrounits.Add(cost, role, budget, provenance);
        }
        _retryCounts.Record(retryCount, role);
    }

    /// <summary>Catalog refresh outcome and how old the snapshot now is.</summary>
    public void RecordCatalogRefresh(string outcome, string catalogState, double ageSeconds)
    {
        _catalogRefreshes.Add(
            1,
            SafeTag("outcome", outcome),
            SafeTag("catalog_state", catalogState));
        _catalogAge.Record(ageSeconds, SafeTag("catalog_state", catalogState));
    }

    /// <summary>How many profiles the effective catalog actually offered.</summary>
    public void RecordEffectiveProfileCount(int count) =>
        _effectiveProfiles.Record(count);

    /// <summary>A request that named a profile the resolver would not serve.</summary>
    public void RecordRouteResolutionFailure(string errorCode) =>
        _routeResolutionFailures.Add(1, SafeTag("error_code", errorCode));

    /// <summary>How one stream ended and how long it ran.</summary>
    public void RecordStreamRun(string terminalType, double milliseconds) =>
        _streamDuration.Record(milliseconds, SafeTag("terminal_type", terminalType));

    public void RecordStreamCancellation(string source) =>
        _streamCancellations.Add(1, SafeTag("cancellation_source", source));

    /// <summary>
    /// Whether a run produced a signed receipt or deliberately withheld one.
    /// A withheld receipt is the expected outcome of a failed, refused, or
    /// cancelled run, not an error.
    /// </summary>
    public void RecordReceiptOutcome(string outcome) =>
        _receiptOutcomes.Add(1, SafeTag("outcome", outcome));

    /// <summary>Which credential mechanism a model call authenticated with.</summary>
    public void RecordCredentialMechanism(string mechanismId, string requestMode) =>
        _providerStatuses.Add(
            1,
            SafeTag("credential_mechanism", mechanismId),
            SafeTag("request_mode", requestMode),
            SafeTag("provider_status_class", "authenticated"));

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
