using System.Runtime.CompilerServices;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace Sapphirus.DesktopSupportApi.Model;

/// <summary>One dispatched server-sent event frame.</summary>
public sealed record FoundrySseFrame(string EventName, string Data);

/// <summary>
/// Raised when a provider transcript cannot be represented. It carries a
/// stable code and never a provider fragment.
/// </summary>
public sealed class ModelStreamInvalidException(string reason)
    : Exception("Model stream invalid: " + reason)
{
    public string Reason { get; } = reason;
}

/// <summary>Stable reasons a transcript was refused. Diagnostics only.</summary>
public static class ModelStreamInvalidReasons
{
    public const string FrameMalformed = "frame_malformed";
    public const string FrameTruncated = "frame_truncated";
    public const string FrameTooLarge = "frame_too_large";
    public const string DuplicateStart = "duplicate_start";
    public const string SequenceRegression = "sequence_regression";
    public const string DataAfterTerminal = "data_after_terminal";
    public const string MultipleTerminal = "multiple_terminal";
    public const string UnknownItemType = "unknown_item_type";
    public const string DeltaTooLarge = "delta_too_large";
    public const string TranscriptTooLarge = "transcript_too_large";
    public const string EventBudgetExhausted = "event_budget_exhausted";
    public const string MissingTerminal = "missing_terminal";
    public const string UnknownEvent = "unknown_event";
    public const string ToolCallUnknown = "tool_call_unknown";
}

/// <summary>What the provider transcript said, as opposed to what it sent.</summary>
public enum ModelStreamProviderOutcome
{
    /// <summary>Nothing terminal yet.</summary>
    None,

    /// <summary>The provider finished successfully.</summary>
    Completed,

    /// <summary>The model refused.</summary>
    Refused,

    /// <summary>The run ended without an answer.</summary>
    Failed,
}

/// <summary>
/// The result of feeding one frame: zero or more safe events to forward now,
/// plus whether the provider transcript has reached a terminal state.
/// </summary>
public sealed record ModelStreamStep(
    IReadOnlyList<IModelStreamEvent> Events,
    ModelStreamProviderOutcome Outcome,
    string Code,
    bool Retryable)
{
    public static readonly ModelStreamStep Ignored =
        new([], ModelStreamProviderOutcome.None, "", false);

    public static ModelStreamStep Forward(params IModelStreamEvent[] events) =>
        new(events, ModelStreamProviderOutcome.None, "", false);
}

/// <summary>Bounds for one normalized run.</summary>
public sealed record ModelStreamLimits(
    int MaximumFrameBytes = 256 * 1024,
    int MaximumDeltaBytes = 65_536,
    int MaximumTranscriptBytes = 1024 * 1024,
    int MaximumToolArgumentBytes = 256 * 1024,
    int MaximumToolCalls = 32,
    int MaximumEvents = 100_000)
{
    public static readonly ModelStreamLimits Default = new();
}

/// <summary>
/// Reads a Foundry server-sent event body into dispatched frames.
///
/// It is deliberately strict. A line without a colon, an unrecognized field
/// name, an oversized frame, or a block left unterminated at end of stream is
/// a refusal rather than a best-effort recovery: a partially understood
/// transcript is exactly the thing a receipt must not be computed from.
/// </summary>
public static class FoundrySseReader
{
    private static readonly string[] FieldNames = ["event", "data", "id", "retry"];

    /// <summary>The sentinel some providers send instead of closing.</summary>
    public const string DoneSentinel = "[DONE]";

    public static async IAsyncEnumerable<FoundrySseFrame> ReadAsync(
        Stream body,
        ModelStreamLimits limits,
        [EnumeratorCancellation] CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(body);
        ArgumentNullException.ThrowIfNull(limits);
        using StreamReader reader = new(
            body,
            new UTF8Encoding(false, false),
            detectEncodingFromByteOrderMarks: false,
            bufferSize: 8_192,
            leaveOpen: true);

        string eventName = "";
        StringBuilder data = new();
        bool pending = false;

        while (true)
        {
            string? line = await reader
                .ReadLineAsync(cancellationToken)
                .ConfigureAwait(false);
            if (line is null)
            {
                if (pending)
                {
                    // A block the provider never terminated. The bytes that
                    // would have completed it are exactly the bytes a
                    // truncated stream is missing.
                    throw new ModelStreamInvalidException(
                        ModelStreamInvalidReasons.FrameTruncated);
                }
                yield break;
            }
            if (line.Length > limits.MaximumFrameBytes)
            {
                throw new ModelStreamInvalidException(
                    ModelStreamInvalidReasons.FrameTooLarge);
            }
            if (line.Length == 0)
            {
                if (pending)
                {
                    // The dispatched frame keeps the event name captured with
                    // it, so a later "event:" line cannot relabel earlier data.
                    string payload = data.ToString();
                    string dispatchedName = eventName;
                    eventName = "";
                    data.Clear();
                    pending = false;
                    if (string.Equals(payload, DoneSentinel, StringComparison.Ordinal))
                    {
                        yield break;
                    }
                    yield return new FoundrySseFrame(dispatchedName, payload);
                }
                continue;
            }
            if (line[0] == ':')
            {
                continue;
            }

            int separator = line.IndexOf(':', StringComparison.Ordinal);
            if (separator <= 0)
            {
                throw new ModelStreamInvalidException(
                    ModelStreamInvalidReasons.FrameMalformed);
            }
            string field = line[..separator];
            if (!FieldNames.Contains(field, StringComparer.Ordinal))
            {
                throw new ModelStreamInvalidException(
                    ModelStreamInvalidReasons.FrameMalformed);
            }
            string value = line[(separator + 1)..];
            if (value.StartsWith(' '))
            {
                value = value[1..];
            }
            switch (field)
            {
                case "event":
                    eventName = value;
                    pending = true;
                    break;
                case "data":
                    if (data.Length > 0)
                    {
                        data.Append('\n');
                    }
                    if (data.Length + value.Length > limits.MaximumFrameBytes)
                    {
                        throw new ModelStreamInvalidException(
                            ModelStreamInvalidReasons.FrameTooLarge);
                    }
                    data.Append(value);
                    pending = true;
                    break;
                default:
                    // id and retry are accepted and ignored; they carry no
                    // model content and must not abort a valid stream.
                    break;
            }
        }
    }
}

/// <summary>
/// Turns a Foundry Responses transcript into strictly sequenced
/// <c>ModelStreamEvent</c> values.
///
/// <para>
/// Three properties are load bearing. Sequencing is the plane's own, monotonic
/// from one, so a consumer that sees a gap knows the run is unusable. The
/// forwarded events are a closed set of safe shapes, so private reasoning text
/// has no property to arrive in — reasoning is observed as a status and its
/// text is never read into a buffer at all. And the transcript accumulated for
/// hashing is bounded, so a provider cannot make the plane hold an unbounded
/// answer in memory while it forwards deltas.
/// </para>
/// <para>
/// Anything the normalizer cannot represent faithfully is a refusal:
/// a duplicate start, a provider sequence that goes backwards, data after a
/// terminal event, a second terminal event, an unknown output item type, or an
/// oversized delta.
/// </para>
/// </summary>
public sealed class FoundryModelStreamNormalizer
{
    private readonly string _requestId;
    private readonly string _runId;
    private readonly string _profileId;
    private readonly string _variantId;
    private readonly TimeProvider _timeProvider;
    private readonly ModelStreamLimits _limits;
    private readonly Func<ModelProviderReportedUsage?, ModelUsageV2> _usageProjector;
    private readonly StringBuilder _transcript = new();
    private readonly Dictionary<string, ToolCallState> _toolCalls = new(StringComparer.Ordinal);

    private int _sequence;
    private int _transcriptBytes;
    private long _providerSequence = -1;
    private bool _started;
    private bool _terminal;

    private static readonly JsonDocumentOptions FrameOptions = new()
    {
        AllowTrailingCommas = false,
        CommentHandling = JsonCommentHandling.Disallow,
        MaxDepth = 24,
    };

    public FoundryModelStreamNormalizer(
        string requestId,
        string runId,
        string profileId,
        string variantId,
        TimeProvider timeProvider,
        ModelStreamLimits limits,
        Func<ModelProviderReportedUsage?, ModelUsageV2> usageProjector)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(requestId);
        ArgumentException.ThrowIfNullOrWhiteSpace(runId);
        ArgumentException.ThrowIfNullOrWhiteSpace(profileId);
        ArgumentException.ThrowIfNullOrWhiteSpace(variantId);
        ArgumentNullException.ThrowIfNull(timeProvider);
        ArgumentNullException.ThrowIfNull(limits);
        ArgumentNullException.ThrowIfNull(usageProjector);
        _requestId = requestId;
        _runId = runId;
        _profileId = profileId;
        _variantId = variantId;
        _timeProvider = timeProvider;
        _limits = limits;
        _usageProjector = usageProjector;
    }

    /// <summary>The last sequence number assigned. Zero before the first event.</summary>
    public int LastSequence => _sequence;

    /// <summary>How many events have been minted, terminal included.</summary>
    public int EventCount => _sequence;

    /// <summary>True once a terminal event has been minted.</summary>
    public bool IsTerminal => _terminal;

    /// <summary>True once the provider's acceptance has been observed.</summary>
    public bool HasStarted => _started;

    /// <summary>
    /// True when the provider emitted reasoning activity. Only the fact is
    /// recorded; the reasoning text is never read.
    /// </summary>
    public bool ReasoningObserved { get; private set; }

    /// <summary>The bounded canonical transcript, for final validation.</summary>
    public string Transcript => _transcript.ToString();

    public int OutputByteCount => _transcriptBytes;

    /// <summary>Digest of the accumulated transcript, for receipt binding.</summary>
    public string TranscriptHash => "sha256:" + Convert.ToHexStringLower(
        SHA256.HashData(Encoding.UTF8.GetBytes(_transcript.ToString())));

    /// <summary>The provider identifiers, normalized only at authoritative points.</summary>
    public string? ProviderResponseId { get; private set; }

    public string? ReportedModel { get; private set; }

    /// <summary>Usage as last reported. Null when the provider never said.</summary>
    public ModelProviderReportedUsage? ReportedUsage { get; private set; }

    /// <summary>Accepts one dispatched frame.</summary>
    public ModelStreamStep Accept(FoundrySseFrame frame)
    {
        ArgumentNullException.ThrowIfNull(frame);
        if (_terminal)
        {
            throw new ModelStreamInvalidException(
                ModelStreamInvalidReasons.DataAfterTerminal);
        }
        if (_sequence >= _limits.MaximumEvents)
        {
            throw new ModelStreamInvalidException(
                ModelStreamInvalidReasons.EventBudgetExhausted);
        }

        using JsonDocument document = ParseFrame(frame.Data);
        JsonElement payload = document.RootElement;
        string type = ReadString(payload, "type", 128) ?? frame.EventName;
        RequireForwardSequence(payload);

        return type switch
        {
            "response.created" => AcceptCreated(payload),
            "response.queued" or "response.in_progress" => ModelStreamStep.Ignored,
            "response.output_item.added" => AcceptOutputItemAdded(payload),
            "response.output_item.done" or "response.content_part.added"
                or "response.content_part.done" or "response.output_text.done"
                or "response.output_text.annotation.added" => ModelStreamStep.Ignored,
            "response.output_text.delta" => AcceptTextDelta(payload),
            "response.function_call_arguments.delta" => AcceptArgumentsDelta(payload),
            "response.function_call_arguments.done" => AcceptArgumentsDone(payload),
            "response.reasoning_summary_part.added"
                or "response.reasoning_summary_part.done"
                or "response.reasoning_summary_text.delta"
                or "response.reasoning_summary_text.done"
                or "response.reasoning_text.delta"
                or "response.reasoning_text.done" => AcceptReasoningStatus(),
            "response.refusal.delta" => ModelStreamStep.Ignored,
            "response.refusal.done" => Refused(ModelStreamRefusalCodes.PolicyBlocked),
            "response.completed" => AcceptCompleted(payload),
            "response.incomplete" => AcceptIncomplete(payload),
            "response.failed" or "error" => AcceptFailed(payload),
            _ => throw new ModelStreamInvalidException(
                ModelStreamInvalidReasons.UnknownEvent),
        };
    }

    /// <summary>
    /// Mints the single completed terminal event. The receipt summary is
    /// supplied by the caller, because it exists only after the output has
    /// been validated and the receipt signed.
    /// </summary>
    public ModelStreamCompletedEvent Complete(ModelStreamReceiptSummary summary)
    {
        ArgumentNullException.ThrowIfNull(summary);
        RequireNoTerminal();
        _terminal = true;
        return new ModelStreamCompletedEvent(_requestId, _runId, ++_sequence, summary);
    }

    /// <summary>Mints the single refused terminal event.</summary>
    public ModelStreamRefusedEvent Refuse(string refusalCode, string? detail = null)
    {
        RequireNoTerminal();
        if (!ModelStreamRefusalCodes.IsKnown(refusalCode))
        {
            throw new ArgumentOutOfRangeException(nameof(refusalCode));
        }
        _terminal = true;
        return new ModelStreamRefusedEvent(
            _requestId,
            _runId,
            ++_sequence,
            refusalCode,
            detail);
    }

    /// <summary>Mints the single failed terminal event.</summary>
    public ModelStreamFailedEvent Fail(string failureCode, bool retryable, string? detail = null)
    {
        RequireNoTerminal();
        if (!ModelStreamFailureCodes.IsKnown(failureCode))
        {
            throw new ArgumentOutOfRangeException(nameof(failureCode));
        }
        _terminal = true;
        return new ModelStreamFailedEvent(
            _requestId,
            _runId,
            ++_sequence,
            failureCode,
            retryable,
            detail);
    }

    /// <summary>
    /// Mints the single cancelled terminal event. A cancelled run is never
    /// retried, so this is always the last thing the desktop sees.
    /// </summary>
    public ModelStreamCancelledEvent Cancel(string source)
    {
        RequireNoTerminal();
        if (!ModelStreamCancellationSources.IsKnown(source))
        {
            throw new ArgumentOutOfRangeException(nameof(source));
        }
        _terminal = true;
        return new ModelStreamCancelledEvent(_requestId, _runId, ++_sequence, source);
    }

    private void RequireNoTerminal()
    {
        if (_terminal)
        {
            throw new ModelStreamInvalidException(
                ModelStreamInvalidReasons.MultipleTerminal);
        }
    }

    private ModelStreamStep AcceptCreated(JsonElement payload)
    {
        if (_started)
        {
            throw new ModelStreamInvalidException(
                ModelStreamInvalidReasons.DuplicateStart);
        }
        _started = true;
        CaptureResponseFacts(payload, authoritative: false);
        return ModelStreamStep.Forward(new ModelStreamResponseStartedEvent(
            _requestId,
            _runId,
            ++_sequence,
            _profileId,
            _variantId,
            _timeProvider.GetUtcNow()));
    }

    private ModelStreamStep AcceptOutputItemAdded(JsonElement payload)
    {
        if (!payload.TryGetProperty("item", out JsonElement item)
            || item.ValueKind != JsonValueKind.Object)
        {
            throw new ModelStreamInvalidException(
                ModelStreamInvalidReasons.FrameMalformed);
        }
        string itemType = ReadString(item, "type", 64) ?? "";
        switch (itemType)
        {
            case "message":
            case "reasoning":
                return ModelStreamStep.Ignored;
            case "function_call":
                break;
            default:
                // A hosted tool, a web search, a computer call: item types the
                // plane never requested and cannot represent safely.
                throw new ModelStreamInvalidException(
                    ModelStreamInvalidReasons.UnknownItemType);
        }

        string toolCallId = ReadString(item, "id", 128) ?? "";
        string toolName = ReadString(item, "name", 64) ?? "";
        if (!IsToolCallId(toolCallId)
            || toolName.Length is < 1 or > 64
            || _toolCalls.Count >= _limits.MaximumToolCalls
            || !_toolCalls.TryAdd(toolCallId, new ToolCallState(toolName)))
        {
            throw new ModelStreamInvalidException(
                ModelStreamInvalidReasons.UnknownItemType);
        }
        return ModelStreamStep.Forward(new ModelStreamToolCallStartedEvent(
            _requestId,
            _runId,
            ++_sequence,
            toolCallId,
            toolName));
    }

    private ModelStreamStep AcceptTextDelta(JsonElement payload)
    {
        string delta = ReadString(payload, "delta", _limits.MaximumDeltaBytes)
            ?? throw new ModelStreamInvalidException(
                ModelStreamInvalidReasons.FrameMalformed);
        if (delta.Length == 0)
        {
            return ModelStreamStep.Ignored;
        }
        int deltaBytes = Encoding.UTF8.GetByteCount(delta);
        if (deltaBytes > _limits.MaximumDeltaBytes)
        {
            throw new ModelStreamInvalidException(
                ModelStreamInvalidReasons.DeltaTooLarge);
        }
        if (_transcriptBytes + deltaBytes > _limits.MaximumTranscriptBytes)
        {
            throw new ModelStreamInvalidException(
                ModelStreamInvalidReasons.TranscriptTooLarge);
        }
        _transcript.Append(delta);
        _transcriptBytes += deltaBytes;

        int outputIndex = ReadBoundedIndex(payload, "output_index");
        return ModelStreamStep.Forward(new ModelStreamOutputTextDeltaEvent(
            _requestId,
            _runId,
            ++_sequence,
            outputIndex,
            delta));
    }

    private ModelStreamStep AcceptArgumentsDelta(JsonElement payload)
    {
        (string toolCallId, ToolCallState state) = ResolveToolCall(payload);
        string delta = ReadString(payload, "delta", _limits.MaximumDeltaBytes)
            ?? throw new ModelStreamInvalidException(
                ModelStreamInvalidReasons.FrameMalformed);
        if (delta.Length == 0)
        {
            return ModelStreamStep.Ignored;
        }
        int deltaBytes = Encoding.UTF8.GetByteCount(delta);
        if (deltaBytes > _limits.MaximumDeltaBytes)
        {
            throw new ModelStreamInvalidException(
                ModelStreamInvalidReasons.DeltaTooLarge);
        }
        if (state.ByteCount + deltaBytes > _limits.MaximumToolArgumentBytes)
        {
            throw new ModelStreamInvalidException(
                ModelStreamInvalidReasons.TranscriptTooLarge);
        }
        state.Arguments.Append(delta);
        state.ByteCount += deltaBytes;
        return ModelStreamStep.Forward(new ModelStreamToolCallArgumentsDeltaEvent(
            _requestId,
            _runId,
            ++_sequence,
            toolCallId,
            delta));
    }

    private ModelStreamStep AcceptArgumentsDone(JsonElement payload)
    {
        (string toolCallId, ToolCallState state) = ResolveToolCall(payload);
        if (state.Completed)
        {
            throw new ModelStreamInvalidException(
                ModelStreamInvalidReasons.UnknownItemType);
        }
        state.Completed = true;
        string arguments = state.Arguments.ToString();
        byte[] argumentBytes = Encoding.UTF8.GetBytes(arguments);
        return ModelStreamStep.Forward(new ModelStreamToolCallCompletedEvent(
            _requestId,
            _runId,
            ++_sequence,
            toolCallId,
            "sha256:" + Convert.ToHexStringLower(SHA256.HashData(argumentBytes)),
            argumentBytes.Length));
    }

    /// <summary>
    /// Records that the model reasoned. The summary text is never read, so it
    /// cannot be forwarded, hashed, logged, or accidentally serialized later.
    /// </summary>
    private ModelStreamStep AcceptReasoningStatus()
    {
        ReasoningObserved = true;
        return ModelStreamStep.Ignored;
    }

    private ModelStreamStep AcceptCompleted(JsonElement payload)
    {
        CaptureResponseFacts(payload, authoritative: true);
        return ReportedUsage is null
            ? new ModelStreamStep([], ModelStreamProviderOutcome.Completed, "", false)
            : new ModelStreamStep(
                [
                    new ModelStreamUsageEvent(
                        _requestId,
                        _runId,
                        ++_sequence,
                        _usageProjector(ReportedUsage)),
                ],
                ModelStreamProviderOutcome.Completed,
                "",
                false);
    }

    private ModelStreamStep AcceptIncomplete(JsonElement payload)
    {
        CaptureResponseFacts(payload, authoritative: true);
        string reason = payload.TryGetProperty("response", out JsonElement response)
            && response.ValueKind == JsonValueKind.Object
            && response.TryGetProperty("incomplete_details", out JsonElement details)
            && details.ValueKind == JsonValueKind.Object
            ? ReadString(details, "reason", 64) ?? ""
            : "";
        return reason is "content_filter"
            ? Refused(ModelStreamRefusalCodes.ContentFiltered)
            : new ModelStreamStep(
                [],
                ModelStreamProviderOutcome.Failed,
                ModelStreamFailureCodes.OutputLimitExceeded,
                false);
    }

    private ModelStreamStep AcceptFailed(JsonElement payload)
    {
        CaptureResponseFacts(payload, authoritative: true);
        // The provider's error text is never read. Only a closed code set
        // chooses between a refusal and a failure.
        string code = ReadErrorCode(payload);
        return code switch
        {
            "content_filter" or "content_policy_violation" =>
                Refused(ModelStreamRefusalCodes.ContentFiltered),
            "rate_limit_exceeded" => new ModelStreamStep(
                [],
                ModelStreamProviderOutcome.Failed,
                ModelStreamFailureCodes.RateLimited,
                true),
            "insufficient_quota" or "quota_exceeded" => new ModelStreamStep(
                [],
                ModelStreamProviderOutcome.Failed,
                ModelStreamFailureCodes.BudgetExhausted,
                false),
            _ => new ModelStreamStep(
                [],
                ModelStreamProviderOutcome.Failed,
                ModelStreamFailureCodes.ProviderUnavailable,
                true),
        };
    }

    private static ModelStreamStep Refused(string refusalCode) =>
        new([], ModelStreamProviderOutcome.Refused, refusalCode, false);

    /// <summary>
    /// Reads response identity and usage. Only an authoritative frame — a
    /// terminal one — may set usage, so an early partial count never becomes
    /// the number a receipt bills.
    /// </summary>
    private void CaptureResponseFacts(JsonElement payload, bool authoritative)
    {
        if (!payload.TryGetProperty("response", out JsonElement response)
            || response.ValueKind != JsonValueKind.Object)
        {
            return;
        }
        ProviderResponseId = ReadOpaque(response, "id") ?? ProviderResponseId;
        ReportedModel = ReadString(response, "model", 128) ?? ReportedModel;
        if (!authoritative
            || !response.TryGetProperty("usage", out JsonElement usage)
            || usage.ValueKind != JsonValueKind.Object)
        {
            return;
        }
        ReportedUsage = new ModelProviderReportedUsage(
            ReadTokens(usage, "input_tokens"),
            ReadTokens(usage, "output_tokens"),
            ReadNestedTokens(usage, "output_tokens_details", "reasoning_tokens"),
            ReadNestedTokens(usage, "input_tokens_details", "cached_tokens"),
            ReadNestedTokens(usage, "output_tokens_details", "cache_write_tokens"));
    }

    /// <summary>
    /// The provider's own ordinal, when it sends one, must never go backwards.
    /// A regression means frames were reordered, and a reordered transcript
    /// cannot be replayed faithfully.
    /// </summary>
    private void RequireForwardSequence(JsonElement payload)
    {
        if (!payload.TryGetProperty("sequence_number", out JsonElement value)
            || value.ValueKind != JsonValueKind.Number
            || !value.TryGetInt64(out long provider))
        {
            return;
        }
        if (provider <= _providerSequence)
        {
            throw new ModelStreamInvalidException(
                ModelStreamInvalidReasons.SequenceRegression);
        }
        _providerSequence = provider;
    }

    private (string ToolCallId, ToolCallState State) ResolveToolCall(JsonElement payload)
    {
        string toolCallId = ReadString(payload, "item_id", 128) ?? "";
        if (!IsToolCallId(toolCallId)
            || !_toolCalls.TryGetValue(toolCallId, out ToolCallState? state))
        {
            // Arguments for a call the plane never saw start cannot be bound
            // to a tool name, and an unbound tool call is unreviewable.
            throw new ModelStreamInvalidException(
                ModelStreamInvalidReasons.ToolCallUnknown);
        }
        return (toolCallId, state);
    }

    private static JsonDocument ParseFrame(string data)
    {
        try
        {
            JsonDocument document = JsonDocument.Parse(data, FrameOptions);
            if (document.RootElement.ValueKind != JsonValueKind.Object)
            {
                document.Dispose();
                throw new ModelStreamInvalidException(
                    ModelStreamInvalidReasons.FrameMalformed);
            }
            return document;
        }
        catch (JsonException)
        {
            throw new ModelStreamInvalidException(
                ModelStreamInvalidReasons.FrameMalformed);
        }
    }

    private static string ReadErrorCode(JsonElement payload)
    {
        if (payload.TryGetProperty("error", out JsonElement error)
            && error.ValueKind == JsonValueKind.Object)
        {
            return ReadString(error, "code", 64) ?? "";
        }
        if (payload.TryGetProperty("response", out JsonElement response)
            && response.ValueKind == JsonValueKind.Object
            && response.TryGetProperty("error", out JsonElement nested)
            && nested.ValueKind == JsonValueKind.Object)
        {
            return ReadString(nested, "code", 64) ?? "";
        }
        return "";
    }

    private static bool IsToolCallId(string value) =>
        value.Length is > 0 and <= 128
        && char.IsAsciiLetterOrDigit(value[0])
        && value.All(static character =>
            char.IsAsciiLetterOrDigit(character) || character is '-' or '_');

    private static int ReadBoundedIndex(JsonElement payload, string name) =>
        payload.TryGetProperty(name, out JsonElement value)
            && value.ValueKind == JsonValueKind.Number
            && value.TryGetInt32(out int index)
            && index is >= 0 and <= 255
            ? index
            : 0;

    private static long? ReadTokens(JsonElement parent, string name) =>
        parent.TryGetProperty(name, out JsonElement value)
            && value.ValueKind == JsonValueKind.Number
            && value.TryGetInt64(out long parsed)
            && parsed >= 0
            ? parsed
            : null;

    private static long? ReadNestedTokens(JsonElement parent, string objectName, string name) =>
        parent.TryGetProperty(objectName, out JsonElement nested)
            && nested.ValueKind == JsonValueKind.Object
            ? ReadTokens(nested, name)
            : null;

    private static string? ReadString(JsonElement parent, string name, int maximumLength)
    {
        if (!parent.TryGetProperty(name, out JsonElement value)
            || value.ValueKind != JsonValueKind.String)
        {
            return null;
        }
        string? text = value.GetString();
        return text is not null && text.Length <= maximumLength ? text : null;
    }

    private static string? ReadOpaque(JsonElement parent, string name)
    {
        string? value = ReadString(parent, name, 128);
        if (value is null || value.Length == 0)
        {
            return null;
        }
        return value.All(static character =>
            char.IsAsciiLetterOrDigit(character) || character is '-' or '_')
            ? value
            : null;
    }

    private sealed class ToolCallState(string toolName)
    {
        public string ToolName { get; } = toolName;

        public StringBuilder Arguments { get; } = new();

        public int ByteCount { get; set; }

        public bool Completed { get; set; }
    }
}
