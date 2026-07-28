using System.Net;
using System.Text.Json;

namespace Sapphirus.DesktopSupportApi.Model;

/// <summary>
/// One classified provider outcome. <see cref="Outcome"/> is empty exactly
/// when the call succeeded.
/// </summary>
public sealed record FoundryResponseClassification(string Outcome, bool Retryable)
{
    public static readonly FoundryResponseClassification Success = new("", false);

    public bool Succeeded => Outcome.Length == 0;
}

/// <summary>
/// Reads a non-streaming Foundry Responses body and classifies a failed one.
///
/// <para>
/// Nothing this type reads ever leaves it except a stable outcome string. The
/// provider's <c>error.code</c> is matched against a closed allowlist purely to
/// choose between two safe outcomes and is then discarded; the message, the
/// body, and any provider prose are never returned, logged, or attached to an
/// exception. That is what "never expose a response body" means in practice:
/// the body is a classification input, not an output.
/// </para>
/// <para>
/// The refusal text a model produces is treated the same way. The support
/// plane records that a refusal happened; it does not republish what the model
/// said while refusing.
/// </para>
/// </summary>
public static class FoundryResponseParser
{
    public const int MaximumOutputItems = 64;
    public const int MaximumContentParts = 64;
    private const int MaximumErrorCodeLength = 64;

    private static readonly JsonDocumentOptions BodyOptions = new()
    {
        AllowTrailingCommas = false,
        CommentHandling = JsonCommentHandling.Disallow,
        MaxDepth = 32,
    };

    /// <summary>
    /// Provider error codes the plane recognizes. Anything else falls through
    /// to the status-based default, so an unknown code widens nothing.
    /// </summary>
    private static readonly Dictionary<string, string> KnownErrorCodes =
        new(StringComparer.Ordinal)
        {
            ["content_filter"] = ModelProviderOutcomes.ContentFiltered,
            ["content_policy_violation"] = ModelProviderOutcomes.ContentFiltered,
            ["ResponsibleAIPolicyViolation"] = ModelProviderOutcomes.ContentFiltered,
            ["context_length_exceeded"] = ModelProviderOutcomes.ContextLimit,
            ["string_above_max_length"] = ModelProviderOutcomes.ContextLimit,
            ["insufficient_quota"] = ModelProviderOutcomes.QuotaExhausted,
            ["quota_exceeded"] = ModelProviderOutcomes.QuotaExhausted,
            ["DeploymentNotFound"] = ModelProviderOutcomes.DeploymentMissing,
            ["deployment_not_found"] = ModelProviderOutcomes.DeploymentMissing,
            ["model_not_found"] = ModelProviderOutcomes.DeploymentMissing,
            ["unsupported_parameter"] = ModelProviderOutcomes.CapabilityUnsupported,
            ["unsupported_value"] = ModelProviderOutcomes.CapabilityUnsupported,
            ["model_not_supported"] = ModelProviderOutcomes.CapabilityUnsupported,
            ["invalid_api_key"] = ModelProviderOutcomes.AuthenticationFailed,
            ["PermissionDenied"] = ModelProviderOutcomes.AuthenticationFailed,
        };

    /// <summary>
    /// Maps an HTTP status to a safe outcome and a retry class. Only timeout,
    /// throttling, and the four transient server statuses are retryable; every
    /// other failure is terminal, because retrying it spends budget and cannot
    /// change the answer.
    /// </summary>
    public static FoundryResponseClassification ClassifyStatus(
        HttpStatusCode statusCode,
        ReadOnlySpan<byte> body)
    {
        string? providerCode = TryReadErrorCode(body);
        string? mapped = providerCode is not null
            && KnownErrorCodes.TryGetValue(providerCode, out string? candidate)
            ? candidate
            : null;

        return (int)statusCode switch
        {
            400 => new FoundryResponseClassification(
                mapped ?? ModelProviderOutcomes.ProviderRefusal,
                false),
            401 => new FoundryResponseClassification(
                ModelProviderOutcomes.AuthenticationFailed,
                false),
            403 => new FoundryResponseClassification(
                // A 403 is an authorization answer unless the provider named a
                // quota. Reporting every 403 as exhausted quota would tell an
                // operator to buy capacity for a missing role assignment.
                mapped == ModelProviderOutcomes.QuotaExhausted
                    ? ModelProviderOutcomes.QuotaExhausted
                    : ModelProviderOutcomes.AuthenticationFailed,
                false),
            404 => new FoundryResponseClassification(
                ModelProviderOutcomes.DeploymentMissing,
                false),
            408 => new FoundryResponseClassification(ModelProviderOutcomes.Timeout, true),
            409 => new FoundryResponseClassification(
                ModelProviderOutcomes.ProviderRefusal,
                false),
            413 or 414 => new FoundryResponseClassification(
                ModelProviderOutcomes.ContextLimit,
                false),
            415 or 422 => new FoundryResponseClassification(
                ModelProviderOutcomes.CapabilityUnsupported,
                false),
            429 => new FoundryResponseClassification(
                mapped == ModelProviderOutcomes.QuotaExhausted
                    ? ModelProviderOutcomes.QuotaExhausted
                    : ModelProviderOutcomes.RateLimited,
                mapped != ModelProviderOutcomes.QuotaExhausted),
            501 => new FoundryResponseClassification(
                ModelProviderOutcomes.CapabilityUnsupported,
                false),
            500 or 502 or 503 or 504 => new FoundryResponseClassification(
                ModelProviderOutcomes.ProviderUnavailable,
                true),
            // Everything else, including any other 5xx, is unavailable but not
            // retried: the plane retries statuses it has a reason to believe
            // are transient, not statuses it does not recognize.
            _ => new FoundryResponseClassification(
                ModelProviderOutcomes.ProviderUnavailable,
                false),
        };
    }

    /// <summary>
    /// Parses a completed non-streaming response. A body that cannot be read
    /// canonically is a malformed output, never a partial success.
    /// </summary>
    public static ModelProviderResponse Parse(ReadOnlySpan<byte> body)
    {
        JsonDocument document;
        try
        {
            document = JsonDocument.Parse(body.ToArray(), BodyOptions);
        }
        catch (JsonException)
        {
            throw new ModelProviderException(
                ModelProviderOutcomes.MalformedOutput,
                retryable: false);
        }

        using (document)
        {
            JsonElement root = document.RootElement;
            if (root.ValueKind != JsonValueKind.Object)
            {
                throw new ModelProviderException(
                    ModelProviderOutcomes.MalformedOutput,
                    retryable: false);
            }

            string status = ReadBoundedString(root, "status", 64) ?? "";
            string? providerRequestId = ReadOpaqueRef(root, "id");
            string? reportedModel = ReadBoundedString(root, "model", 128);
            ModelProviderReportedUsage usage = ReadUsage(root);

            (string text, bool refused) = ReadOutput(root);
            string finishReason = ResolveFinishReason(root, status, refused);

            return new ModelProviderResponse(
                text,
                usage.InputTokens ?? 0,
                usage.OutputTokens ?? 0,
                providerRequestId,
                finishReason,
                usage,
                reportedModel);
        }
    }

    /// <summary>
    /// Collapses the provider's status and incomplete reason onto the finish
    /// vocabulary the broker already branches on.
    /// </summary>
    private static string ResolveFinishReason(
        JsonElement root,
        string status,
        bool refused)
    {
        if (refused)
        {
            return "refusal";
        }
        if (status is "failed")
        {
            return "failed";
        }
        if (status is "incomplete")
        {
            string reason = root.TryGetProperty("incomplete_details", out JsonElement details)
                && details.ValueKind == JsonValueKind.Object
                ? ReadBoundedString(details, "reason", 64) ?? ""
                : "";
            return reason is "content_filter" ? "content_filter" : "incomplete";
        }
        return status is "completed" ? "stop" : "incomplete";
    }

    /// <summary>
    /// Concatenates the assistant's visible output text. Reasoning items are
    /// skipped entirely rather than read and dropped later, so private
    /// reasoning text never enters a buffer the plane could serialize.
    /// </summary>
    private static (string Text, bool Refused) ReadOutput(JsonElement root)
    {
        if (!root.TryGetProperty("output", out JsonElement output)
            || output.ValueKind != JsonValueKind.Array)
        {
            return ("", false);
        }
        if (output.GetArrayLength() > MaximumOutputItems)
        {
            throw new ModelProviderException(
                ModelProviderOutcomes.ContextLimit,
                retryable: false);
        }

        System.Text.StringBuilder text = new();
        bool refused = false;
        foreach (JsonElement item in output.EnumerateArray())
        {
            if (item.ValueKind != JsonValueKind.Object)
            {
                throw new ModelProviderException(
                    ModelProviderOutcomes.MalformedOutput,
                    retryable: false);
            }
            string itemType = ReadBoundedString(item, "type", 64) ?? "";
            if (itemType is "reasoning")
            {
                continue;
            }
            if (itemType is not "message")
            {
                // A function call or a hosted-tool item in a structured
                // response means the provider answered a request the plane
                // never made.
                throw new ModelProviderException(
                    ModelProviderOutcomes.CapabilityUnsupported,
                    retryable: false);
            }
            if (!item.TryGetProperty("content", out JsonElement content)
                || content.ValueKind != JsonValueKind.Array
                || content.GetArrayLength() > MaximumContentParts)
            {
                throw new ModelProviderException(
                    ModelProviderOutcomes.MalformedOutput,
                    retryable: false);
            }
            foreach (JsonElement part in content.EnumerateArray())
            {
                if (part.ValueKind != JsonValueKind.Object)
                {
                    throw new ModelProviderException(
                        ModelProviderOutcomes.MalformedOutput,
                        retryable: false);
                }
                switch (ReadBoundedString(part, "type", 64))
                {
                    case "output_text":
                        text.Append(part.TryGetProperty("text", out JsonElement value)
                            && value.ValueKind == JsonValueKind.String
                            ? value.GetString()
                            : "");
                        break;
                    case "refusal":
                        // The refusal string itself is deliberately not read.
                        refused = true;
                        break;
                    default:
                        throw new ModelProviderException(
                            ModelProviderOutcomes.MalformedOutput,
                            retryable: false);
                }
            }
        }
        return (text.ToString(), refused);
    }

    private static ModelProviderReportedUsage ReadUsage(JsonElement root)
    {
        if (!root.TryGetProperty("usage", out JsonElement usage)
            || usage.ValueKind != JsonValueKind.Object)
        {
            // The provider said nothing. Every category stays null so a
            // receipt cannot read silence as a free call.
            return new ModelProviderReportedUsage(null, null, null, null, null);
        }
        return new ModelProviderReportedUsage(
            ReadTokenCount(usage, "input_tokens"),
            ReadTokenCount(usage, "output_tokens"),
            ReadNestedTokenCount(usage, "output_tokens_details", "reasoning_tokens"),
            ReadNestedTokenCount(usage, "input_tokens_details", "cached_tokens"),
            ReadNestedTokenCount(usage, "output_tokens_details", "cache_write_tokens"));
    }

    private static long? ReadNestedTokenCount(
        JsonElement parent,
        string objectName,
        string name) =>
        parent.TryGetProperty(objectName, out JsonElement nested)
            && nested.ValueKind == JsonValueKind.Object
            ? ReadTokenCount(nested, name)
            : null;

    private static long? ReadTokenCount(JsonElement parent, string name) =>
        parent.TryGetProperty(name, out JsonElement value)
            && value.ValueKind == JsonValueKind.Number
            && value.TryGetInt64(out long parsed)
            && parsed >= 0
            ? parsed
            : null;

    /// <summary>
    /// Reads a provider identifier only when it is short and opaque. A value
    /// carrying punctuation, whitespace, or unbounded length is dropped rather
    /// than recorded, because a receipt field is a durable surface.
    /// </summary>
    private static string? ReadOpaqueRef(JsonElement parent, string name)
    {
        string? value = ReadBoundedString(parent, name, 128);
        if (value is null || value.Length == 0)
        {
            return null;
        }
        foreach (char character in value)
        {
            if (!char.IsAsciiLetterOrDigit(character) && character is not ('-' or '_'))
            {
                return null;
            }
        }
        return value;
    }

    private static string? ReadBoundedString(JsonElement parent, string name, int maximumLength)
    {
        if (!parent.TryGetProperty(name, out JsonElement value)
            || value.ValueKind != JsonValueKind.String)
        {
            return null;
        }
        string? text = value.GetString();
        return text is not null && text.Length <= maximumLength ? text : null;
    }

    /// <summary>
    /// Extracts <c>error.code</c> for classification only. It is bounded,
    /// never returned to a caller, and never attached to an exception.
    /// </summary>
    private static string? TryReadErrorCode(ReadOnlySpan<byte> body)
    {
        if (body.Length is 0 or > 64 * 1024)
        {
            return null;
        }
        try
        {
            using JsonDocument document = JsonDocument.Parse(body.ToArray(), BodyOptions);
            if (document.RootElement.ValueKind != JsonValueKind.Object
                || !document.RootElement.TryGetProperty("error", out JsonElement error)
                || error.ValueKind != JsonValueKind.Object)
            {
                return null;
            }
            return ReadBoundedString(error, "code", MaximumErrorCodeLength);
        }
        catch (JsonException)
        {
            return null;
        }
    }
}
