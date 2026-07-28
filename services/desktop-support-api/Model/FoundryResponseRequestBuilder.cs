using System.Buffers;
using System.Text;
using System.Text.Encodings.Web;
using System.Text.Json;
using System.Text.RegularExpressions;

namespace Sapphirus.DesktopSupportApi.Model;

/// <summary>
/// Stable safe outcomes for one provider call. The broker, the coordinator,
/// and the problem-document mapper all branch on these strings, so they are
/// part of the support plane's contract with itself and never carry provider
/// wording.
/// </summary>
public static class ModelProviderOutcomes
{
    // Pre-existing vocabulary, unchanged so the legacy executor keeps mapping
    // onto exactly the outcomes the version one surfaces already know.
    public const string Timeout = "timeout";
    public const string RateLimited = "rate_limited";
    public const string QuotaExhausted = "quota_exhausted";
    public const string ProviderUnavailable = "provider_unavailable";
    public const string ProviderRefusal = "provider_refusal";
    public const string ContentFiltered = "content_filtered";
    public const string IncompleteOutput = "incomplete_output";
    public const string MalformedOutput = "malformed_output";
    public const string SchemaInvalid = "schema_invalid";
    public const string ContextRejected = "context_rejected";
    public const string ProfileMismatch = "profile_mismatch";

    // Added by FND-028. Each names a failure the Chat Completions path folded
    // into a weaker outcome, and each is separately actionable: one is an
    // operator fix, one is a catalog fix, one is a request-size fix, one is a
    // credential fix, and one means the transcript itself is untrustworthy.
    public const string DeploymentMissing = "deployment_missing";
    public const string CapabilityUnsupported = "capability_unsupported";
    public const string ContextLimit = "context_limit";
    public const string AuthenticationFailed = "authentication_failed";
    public const string StreamInvalid = "stream_invalid";

    /// <summary>
    /// The only outcomes a bounded retry may be attempted for. Everything else
    /// is terminal: retrying a refused, filtered, missing, or unauthenticated
    /// call spends money and changes nothing.
    /// </summary>
    public static readonly string[] Retryable =
        [Timeout, RateLimited, ProviderUnavailable];

    public static bool IsRetryable(string outcome) =>
        Retryable.Contains(outcome, StringComparer.Ordinal);
}

/// <summary>
/// Builds the request body for <c>POST &lt;projectEndpoint&gt;/openai/v1/responses</c>.
///
/// <para>
/// The body is assembled from a fixed key set — <see cref="TopLevelKeys"/> —
/// and nothing else. That is the structural argument that a local path, an
/// Azure resource identifier, a raw consent document, a credential, unrelated
/// memory, or unreviewed workspace content cannot reach the provider: there is
/// no key any of them could be written under, and the two text fields are the
/// canonical system instruction and the canonical projected user payload that
/// <see cref="CanonicalPromptProjector"/> already bounded.
/// </para>
/// <para>
/// Provider-side storage is off, hosted tools are absent, and no URL or file
/// identifier is representable. Structured mode additionally pins a strict
/// JSON Schema text format and refuses tools outright; streaming mode carries
/// the resolved variant's reasoning effort, verbosity, and output budget, plus
/// only allowlisted function tools.
/// </para>
/// </summary>
public static partial class FoundryResponseRequestBuilder
{
    /// <summary>
    /// The complete set of top-level keys this builder may emit. A test asserts
    /// the produced body's key set is a subset of this, so widening the wire
    /// shape requires widening this list first.
    /// </summary>
    public static readonly string[] TopLevelKeys =
    [
        "input",
        "max_output_tokens",
        "model",
        "parallel_tool_calls",
        "reasoning",
        "store",
        "stream",
        "text",
        "tool_choice",
        "tools",
    ];

    /// <summary>The strict JSON Schema format name for canonical output.</summary>
    public const string StructuredFormatName = "sapphirus_canonical_output_v1";

    public const int MaximumTools = 32;
    public const int MaximumInstructionBytes = 32 * 1024;
    public const int MaximumUserPayloadBytes = 1024 * 1024;
    public const int MaximumSchemaBytes = 128 * 1024;

    private static readonly string[] ReasoningEfforts =
        ["minimal", "low", "medium", "high"];

    private static readonly string[] Verbosities = ["low", "medium", "high"];

    private static readonly JsonWriterOptions WriterOptions = new()
    {
        Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
        Indented = false,
    };

    private static readonly JsonDocumentOptions SchemaOptions = new()
    {
        AllowTrailingCommas = false,
        CommentHandling = JsonCommentHandling.Disallow,
        MaxDepth = 32,
    };

    /// <summary>
    /// Renders the UTF-8 request body. A request that cannot be represented
    /// safely throws <see cref="ModelProviderException"/> with a terminal
    /// outcome rather than being sent in a weakened form.
    /// </summary>
    public static byte[] Build(ModelProviderRequest request)
    {
        ArgumentNullException.ThrowIfNull(request);
        bool streaming = request.IsStreaming;
        RequireSafeEnvelope(request, streaming);

        ArrayBufferWriter<byte> buffer = new();
        using (Utf8JsonWriter writer = new(buffer, WriterOptions))
        {
            writer.WriteStartObject();

            // "input": the canonical system and user turns, in that order.
            writer.WritePropertyName("input");
            writer.WriteStartArray();
            WriteInputMessage(writer, "system", request.SystemInstruction);
            WriteInputMessage(writer, "user", request.UserPayloadJson);
            writer.WriteEndArray();

            writer.WriteNumber("max_output_tokens", ResolveOutputTokens(request));
            writer.WriteString("model", request.Deployment);

            // Parallel tool calls stay off in both modes: a normalized stream
            // with interleaved argument deltas from concurrent calls is a
            // reordering hazard the desktop has no way to resolve.
            writer.WriteBoolean("parallel_tool_calls", false);

            if (streaming && request.ReasoningEffort.Length > 0)
            {
                writer.WritePropertyName("reasoning");
                writer.WriteStartObject();
                writer.WriteString("effort", request.ReasoningEffort);
                writer.WriteEndObject();
            }

            // Retention is transient_no_store everywhere in this plane, so the
            // provider is told not to keep the response.
            writer.WriteBoolean("store", false);
            writer.WriteBoolean("stream", streaming);

            WriteTextOptions(writer, request, streaming);

            // Structured mode declares no tools at all, so tool choice is
            // "none" rather than an empty allowlist that a later default could
            // reopen.
            writer.WriteString(
                "tool_choice",
                streaming && request.DeclaredTools.Count > 0 ? "auto" : "none");

            writer.WritePropertyName("tools");
            writer.WriteStartArray();
            if (streaming)
            {
                foreach (ModelProviderTool tool in request.DeclaredTools)
                {
                    WriteFunctionTool(writer, tool);
                }
            }
            writer.WriteEndArray();

            writer.WriteEndObject();
            writer.Flush();
        }
        return buffer.WrittenSpan.ToArray();
    }

    private static void WriteInputMessage(Utf8JsonWriter writer, string role, string text)
    {
        writer.WriteStartObject();
        writer.WritePropertyName("content");
        writer.WriteStartArray();
        writer.WriteStartObject();
        writer.WriteString("text", text);
        writer.WriteString("type", "input_text");
        writer.WriteEndObject();
        writer.WriteEndArray();
        writer.WriteString("role", role);
        writer.WriteEndObject();
    }

    private static void WriteTextOptions(
        Utf8JsonWriter writer,
        ModelProviderRequest request,
        bool streaming)
    {
        writer.WritePropertyName("text");
        writer.WriteStartObject();
        if (streaming)
        {
            if (request.Verbosity.Length > 0)
            {
                writer.WriteString("verbosity", request.Verbosity);
            }
        }
        else
        {
            writer.WritePropertyName("format");
            writer.WriteStartObject();
            writer.WriteString("name", StructuredFormatName);
            writer.WritePropertyName("schema");
            WriteRawJson(writer, request.OutputSchemaJson);
            writer.WriteBoolean("strict", true);
            writer.WriteString("type", "json_schema");
            writer.WriteEndObject();
        }
        writer.WriteEndObject();
    }

    private static void WriteFunctionTool(Utf8JsonWriter writer, ModelProviderTool tool)
    {
        writer.WriteStartObject();
        writer.WriteString("description", tool.Description);
        writer.WriteString("name", tool.ToolName);
        writer.WritePropertyName("parameters");
        WriteRawJson(writer, tool.ParametersSchemaJson);
        writer.WriteBoolean("strict", true);
        writer.WriteString("type", "function");
        writer.WriteEndObject();
    }

    /// <summary>
    /// Re-parses and re-emits a schema document rather than splicing it in as
    /// text. A schema that is not a JSON object never reaches the wire, and no
    /// caller can inject sibling keys through a crafted fragment.
    /// </summary>
    private static void WriteRawJson(Utf8JsonWriter writer, string json)
    {
        using JsonDocument document = ParseSchemaOrThrow(json);
        document.RootElement.WriteTo(writer);
    }

    private static JsonDocument ParseSchemaOrThrow(string json)
    {
        try
        {
            JsonDocument document = JsonDocument.Parse(json, SchemaOptions);
            if (document.RootElement.ValueKind != JsonValueKind.Object)
            {
                document.Dispose();
                throw new ModelProviderException(
                    ModelProviderOutcomes.CapabilityUnsupported,
                    retryable: false);
            }
            return document;
        }
        catch (JsonException)
        {
            throw new ModelProviderException(
                ModelProviderOutcomes.CapabilityUnsupported,
                retryable: false);
        }
    }

    private static int ResolveOutputTokens(ModelProviderRequest request) =>
        request.MaximumOutputTokens > 0
            ? request.MaximumOutputTokens
            : throw new ModelProviderException(
                ModelProviderOutcomes.CapabilityUnsupported,
                retryable: false);

    /// <summary>
    /// Everything the body may not represent, checked before a single byte is
    /// written. Each violation is a terminal outcome: none of them becomes
    /// safe on a second attempt.
    /// </summary>
    private static void RequireSafeEnvelope(ModelProviderRequest request, bool streaming)
    {
        if (!DeploymentName().IsMatch(request.Deployment))
        {
            // A deployment name that is not a plain identifier could carry a
            // path segment or a query, which is how a model field becomes a
            // request-target field.
            throw new ModelProviderException(
                ModelProviderOutcomes.DeploymentMissing,
                retryable: false);
        }
        if (request.RequestMode is not (ModelRequestModes.StructuredResponse
            or ModelRequestModes.StreamingChat))
        {
            throw new ModelProviderException(
                ModelProviderOutcomes.CapabilityUnsupported,
                retryable: false);
        }
        if (Encoding.UTF8.GetByteCount(request.SystemInstruction) is < 1
                or > MaximumInstructionBytes
            || Encoding.UTF8.GetByteCount(request.UserPayloadJson) is < 1
                or > MaximumUserPayloadBytes)
        {
            throw new ModelProviderException(
                ModelProviderOutcomes.ContextLimit,
                retryable: false);
        }
        if (streaming)
        {
            RequireSafeStreamingOptions(request);
            return;
        }
        if (Encoding.UTF8.GetByteCount(request.OutputSchemaJson) is < 2 or > MaximumSchemaBytes
            || request.DeclaredTools.Count != 0)
        {
            // A structured response with tools is asking for two different
            // result contracts at once.
            throw new ModelProviderException(
                ModelProviderOutcomes.CapabilityUnsupported,
                retryable: false);
        }
    }

    private static void RequireSafeStreamingOptions(ModelProviderRequest request)
    {
        if ((request.ReasoningEffort.Length > 0
                && !ReasoningEfforts.Contains(request.ReasoningEffort, StringComparer.Ordinal))
            || (request.Verbosity.Length > 0
                && !Verbosities.Contains(request.Verbosity, StringComparer.Ordinal))
            || request.DeclaredTools.Count > MaximumTools)
        {
            throw new ModelProviderException(
                ModelProviderOutcomes.CapabilityUnsupported,
                retryable: false);
        }
        HashSet<string> names = new(StringComparer.Ordinal);
        foreach (ModelProviderTool tool in request.DeclaredTools)
        {
            if (tool is null
                || !ToolName().IsMatch(tool.ToolName)
                || !names.Add(tool.ToolName)
                || tool.Description.Length is < 1 or > 256
                || Encoding.UTF8.GetByteCount(tool.ParametersSchemaJson)
                    is < 2 or > MaximumSchemaBytes)
            {
                throw new ModelProviderException(
                    ModelProviderOutcomes.CapabilityUnsupported,
                    retryable: false);
            }
        }
    }

    [GeneratedRegex("^[A-Za-z0-9][A-Za-z0-9_-]{0,126}[A-Za-z0-9]$")]
    private static partial Regex DeploymentName();

    [GeneratedRegex("^[a-z][a-z0-9_]{0,63}$")]
    private static partial Regex ToolName();
}
