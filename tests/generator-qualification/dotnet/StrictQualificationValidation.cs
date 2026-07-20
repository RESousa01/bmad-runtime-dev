using System.Buffers;
using System.Globalization;
using System.Text;
using System.Text.Json;
using JsonSchemaResultsCollector = Corvus.Text.Json.JsonSchemaResultsCollector;
using JsonSchemaResultsLevel = Corvus.Text.Json.JsonSchemaResultsLevel;
using QualificationWire = Sapphirus.GeneratorQualification.Generated.GeneratorQualification;

namespace Sapphirus.GeneratorQualification.Tests;

public sealed class StrictJsonException(string code, string message, Exception? innerException = null)
    : JsonException(message, innerException)
{
    public string Code { get; } = code;
}

public static class StrictJson
{
    private const double MaximumInteroperableInteger = 9_007_199_254_740_991d;
    private static readonly UTF8Encoding StrictUtf8 = new(false, true);

    public static JsonDocument Parse(ReadOnlyMemory<byte> source, ParserLimits limits)
    {
        ArgumentNullException.ThrowIfNull(limits);
        if (limits.MaxBytes < 0 || limits.MaxContainerDepth < 1)
        {
            throw new ArgumentOutOfRangeException(nameof(limits));
        }

        if (source.Length > limits.MaxBytes)
        {
            throw new StrictJsonException(
                "MAX_BYTES_EXCEEDED",
                $"JSON input exceeds the {limits.MaxBytes}-byte limit.");
        }

        string text;
        try
        {
            text = StrictUtf8.GetString(source.Span);
        }
        catch (DecoderFallbackException exception)
        {
            throw new StrictJsonException("INVALID_UNICODE", "JSON input is not valid UTF-8.", exception);
        }

        ValidateUnicodeInStrings(text);

        try
        {
            InspectTokens(source.Span, limits.MaxContainerDepth);
            return JsonDocument.Parse(
                source,
                new JsonDocumentOptions
                {
                    AllowTrailingCommas = false,
                    CommentHandling = JsonCommentHandling.Disallow,
                    MaxDepth = 0,
                });
        }
        catch (StrictJsonException)
        {
            throw;
        }
        catch (JsonException exception)
        {
            throw new StrictJsonException("SCHEMA_INVALID", "JSON syntax is invalid.", exception);
        }
    }

    private static void InspectTokens(ReadOnlySpan<byte> source, int maximumContainerDepth)
    {
        var reader = new Utf8JsonReader(
            source,
            new JsonReaderOptions
            {
                AllowTrailingCommas = false,
                CommentHandling = JsonCommentHandling.Disallow,
                MaxDepth = 0,
            });
        var containers = new Stack<HashSet<string>?>();
        int containerDepth = 0;

        while (reader.Read())
        {
            switch (reader.TokenType)
            {
                case JsonTokenType.StartObject:
                    containerDepth++;
                    AssertDepth(containerDepth, maximumContainerDepth);
                    containers.Push(new HashSet<string>(StringComparer.Ordinal));
                    break;
                case JsonTokenType.StartArray:
                    containerDepth++;
                    AssertDepth(containerDepth, maximumContainerDepth);
                    containers.Push(null);
                    break;
                case JsonTokenType.EndObject:
                case JsonTokenType.EndArray:
                    containers.Pop();
                    containerDepth--;
                    break;
                case JsonTokenType.PropertyName:
                {
                    string propertyName = reader.GetString()
                        ?? throw new StrictJsonException("INVALID_UNICODE", "JSON property name is invalid.");
                    HashSet<string>? members = containers.Peek();
                    if (members is null || !members.Add(propertyName))
                    {
                        throw new StrictJsonException(
                            "DUPLICATE_MEMBER",
                            $"Duplicate JSON member {propertyName}.");
                    }

                    break;
                }
                case JsonTokenType.Number:
                    RejectNonInteroperableNumber(reader);
                    break;
            }
        }
    }

    private static void AssertDepth(int depth, int maximumContainerDepth)
    {
        if (depth > maximumContainerDepth)
        {
            throw new StrictJsonException(
                "MAX_DEPTH_EXCEEDED",
                $"JSON container depth exceeds the {maximumContainerDepth}-level limit.");
        }
    }

    private static void RejectNonInteroperableNumber(Utf8JsonReader reader)
    {
        string token = reader.HasValueSequence
            ? Encoding.UTF8.GetString(reader.ValueSequence.ToArray())
            : Encoding.UTF8.GetString(reader.ValueSpan);
        if (!double.TryParse(token, NumberStyles.Float, CultureInfo.InvariantCulture, out double value)
            || !double.IsFinite(value))
        {
            throw new StrictJsonException(
                "SCHEMA_INVALID",
                "JSON number cannot be represented finitely.");
        }

        if ((value == Math.Truncate(value) && Math.Abs(value) > MaximumInteroperableInteger)
            || NormalizeDecimalLexeme(token) is not NormalizedDecimal source
            || NormalizeDecimalLexeme(value.ToString("R", CultureInfo.InvariantCulture))
                is not NormalizedDecimal represented
            || source != represented)
        {
            throw new StrictJsonException(
                "INTEGER_OUT_OF_RANGE",
                "JSON number cannot be represented losslessly in the interoperable range.");
        }
    }

    private static NormalizedDecimal? NormalizeDecimalLexeme(string token)
    {
        bool negative = token.Length > 0 && token[0] == '-';
        int mantissaStart = negative ? 1 : 0;
        int exponentIndex = token.IndexOf('e', StringComparison.Ordinal);
        if (exponentIndex < 0)
        {
            exponentIndex = token.IndexOf('E', StringComparison.Ordinal);
        }

        int mantissaEnd = exponentIndex < 0 ? token.Length : exponentIndex;
        string mantissa = token[mantissaStart..mantissaEnd];
        string exponentToken = exponentIndex < 0 ? "0" : token[(exponentIndex + 1)..];
        int decimalIndex = mantissa.IndexOf('.', StringComparison.Ordinal);
        string whole = decimalIndex < 0 ? mantissa : mantissa[..decimalIndex];
        string fraction = decimalIndex < 0 ? string.Empty : mantissa[(decimalIndex + 1)..];
        string combined = string.Concat(whole, fraction);
        int firstSignificant = 0;
        while (firstSignificant < combined.Length && combined[firstSignificant] == '0')
        {
            firstSignificant++;
        }

        if (firstSignificant == combined.Length)
        {
            return new NormalizedDecimal(false, "0", 0);
        }

        if (!long.TryParse(
            exponentToken,
            NumberStyles.AllowLeadingSign,
            CultureInfo.InvariantCulture,
            out long exponent))
        {
            return null;
        }

        int significantEnd = combined.Length;
        while (significantEnd > firstSignificant && combined[significantEnd - 1] == '0')
        {
            significantEnd--;
        }

        try
        {
            long decimalExponent = checked(
                exponent - (long)fraction.Length + (combined.Length - significantEnd));
            return new NormalizedDecimal(
                negative,
                combined[firstSignificant..significantEnd],
                decimalExponent);
        }
        catch (OverflowException)
        {
            return null;
        }
    }

    private readonly record struct NormalizedDecimal(
        bool Negative,
        string Digits,
        long DecimalExponent);

    private static void ValidateUnicodeInStrings(string source)
    {
        bool insideString = false;
        for (int index = 0; index < source.Length; index++)
        {
            char current = source[index];
            if (!insideString)
            {
                insideString = current == '"';
                continue;
            }

            if (current == '"')
            {
                insideString = false;
                continue;
            }

            if (current == '\\')
            {
                if (index + 1 >= source.Length)
                {
                    continue;
                }

                if (source[index + 1] != 'u')
                {
                    index++;
                    continue;
                }

                if (!TryReadEscapedCodeUnit(source, index, out char codeUnit))
                {
                    continue;
                }

                if (char.IsLowSurrogate(codeUnit))
                {
                    ThrowInvalidUnicode();
                }

                if (char.IsHighSurrogate(codeUnit))
                {
                    int lowEscapeIndex = index + 6;
                    if (!TryReadEscapedCodeUnit(source, lowEscapeIndex, out char low)
                        || !char.IsLowSurrogate(low))
                    {
                        ThrowInvalidUnicode();
                    }

                    index += 11;
                    continue;
                }

                index += 5;
                continue;
            }

            if (char.IsLowSurrogate(current))
            {
                ThrowInvalidUnicode();
            }

            if (char.IsHighSurrogate(current))
            {
                if (index + 1 >= source.Length || !char.IsLowSurrogate(source[index + 1]))
                {
                    ThrowInvalidUnicode();
                }

                index++;
            }
        }
    }

    private static bool TryReadEscapedCodeUnit(string source, int slashIndex, out char value)
    {
        value = default;
        if (slashIndex < 0
            || slashIndex + 5 >= source.Length
            || source[slashIndex] != '\\'
            || source[slashIndex + 1] != 'u')
        {
            return false;
        }

        if (ushort.TryParse(
            source.AsSpan(slashIndex + 2, 4),
            NumberStyles.AllowHexSpecifier,
            CultureInfo.InvariantCulture,
            out ushort codeUnit))
        {
            value = (char)codeUnit;
            return true;
        }

        return false;
    }

    private static void ThrowInvalidUnicode() =>
        throw new StrictJsonException("INVALID_UNICODE", "JSON string contains an unpaired Unicode surrogate.");
}

public sealed record QualificationResult(
    bool Accepted,
    string? ReasonCategory,
    string RejectionStage,
    bool ValidatorInvoked);

public sealed class QualificationValidator(Action? onValidatorInvoked = null)
{
    private static readonly IReadOnlyDictionary<string, string> KeywordReasons =
        new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["additionalProperties"] = "UNKNOWN_PROPERTY",
            ["required"] = "REQUIRED_PROPERTY_MISSING",
            ["type"] = "TYPE_MISMATCH",
            ["minimum"] = "NUMBER_TOO_SMALL",
            ["maximum"] = "NUMBER_TOO_LARGE",
            ["pattern"] = "PATTERN_MISMATCH",
            ["oneOf"] = "ONE_OF_MISMATCH",
        };

    private readonly Action onValidatorInvoked = onValidatorInvoked ?? (() => { });

    public QualificationResult Validate(
        ReadOnlyMemory<byte> source,
        ParserLimits parserLimits,
        IReadOnlyList<string> reasonPriority)
    {
        JsonDocument document;
        try
        {
            document = StrictJson.Parse(source, parserLimits);
        }
        catch (StrictJsonException exception)
        {
            return new QualificationResult(false, exception.Code, "strict_parser", false);
        }

        using (document)
        {
            this.onValidatorInvoked();
            using Corvus.Text.Json.ParsedJsonDocument<QualificationWire> generatedDocument =
                Corvus.Text.Json.ParsedJsonDocument<QualificationWire>.Parse(source);
            QualificationWire generated = generatedDocument.RootElement;
            using JsonSchemaResultsCollector collector = JsonSchemaResultsCollector.CreateUnrented(
                JsonSchemaResultsLevel.Verbose,
                128);
            if (generated.EvaluateSchema(collector))
            {
                return new QualificationResult(true, null, "none", true);
            }

            string reason = DiscriminatorReason(document.RootElement)
                ?? MapCorvusResults(collector, reasonPriority, document.RootElement);
            return new QualificationResult(false, reason, "structural_validator", true);
        }
    }

    private static string? DiscriminatorReason(JsonElement root)
    {
        if (root.ValueKind != JsonValueKind.Object
            || !root.TryGetProperty("variant"u8, out JsonElement variant)
            || variant.ValueKind != JsonValueKind.Object
            || !variant.TryGetProperty("kind"u8, out JsonElement kindElement)
            || kindElement.ValueKind != JsonValueKind.String)
        {
            return null;
        }

        string? kind = kindElement.GetString();
        if (kind is not ("text" or "count"))
        {
            return "UNKNOWN_DISCRIMINATOR";
        }

        if ((kind == "text" && variant.TryGetProperty("count"u8, out _))
            || (kind == "count" && variant.TryGetProperty("text"u8, out _)))
        {
            return "ONE_OF_MISMATCH";
        }

        return null;
    }

    private static string MapCorvusResults(
        JsonSchemaResultsCollector collector,
        IReadOnlyList<string> reasonPriority,
        JsonElement root)
    {
        var priorities = reasonPriority
            .Select((reason, index) => KeyValuePair.Create(reason, index))
            .ToDictionary(static pair => pair.Key, static pair => pair.Value, StringComparer.Ordinal);
        string selectedReason = "SCHEMA_INVALID";
        string selectedLocation = string.Empty;
        int selectedPriority = priorities.GetValueOrDefault(selectedReason, int.MaxValue);

        foreach (JsonSchemaResultsCollector.Result result in collector.EnumerateResults())
        {
            if (result.IsMatch)
            {
                continue;
            }

            string evaluationLocation = result.GetEvaluationLocationText();
            string schemaLocation = result.GetSchemaEvaluationLocationText();
            string documentLocation = result.GetDocumentEvaluationLocationText();
            if (!IsRelevantBranch(root, evaluationLocation, documentLocation))
            {
                continue;
            }

            string? reason = ReasonForLocation(evaluationLocation) ?? ReasonForLocation(schemaLocation);
            if (reason == "UNKNOWN_PROPERTY" && documentLocation.Length == 0)
            {
                continue;
            }

            if (reason is null)
            {
                continue;
            }

            int priority = priorities.GetValueOrDefault(reason, int.MaxValue - 1);
            if (priority < selectedPriority
                || (priority == selectedPriority
                    && StringComparer.Ordinal.Compare(documentLocation, selectedLocation) < 0))
            {
                selectedReason = reason;
                selectedPriority = priority;
                selectedLocation = documentLocation;
            }
        }

        return selectedReason;
    }

    private static bool IsRelevantBranch(
        JsonElement root,
        string evaluationLocation,
        string documentLocation)
    {
        int expectedRootBranch = root.TryGetProperty("optionalValue"u8, out _) ? 1 : 0;
        if (TryGetBranch(evaluationLocation, "/oneOf/", 0, out int rootBranch)
            && rootBranch != expectedRootBranch)
        {
            return false;
        }

        const string variantMarker = "/properties/variant/oneOf/";
        int variantMarkerIndex = evaluationLocation.IndexOf(variantMarker, StringComparison.Ordinal);
        if (variantMarkerIndex >= 0
            && TryGetBranch(evaluationLocation, variantMarker, variantMarkerIndex, out int variantBranch)
            && root.TryGetProperty("variant"u8, out JsonElement variant)
            && variant.ValueKind == JsonValueKind.Object
            && variant.TryGetProperty("kind"u8, out JsonElement kindElement)
            && kindElement.ValueKind == JsonValueKind.String)
        {
            int expectedVariantBranch = kindElement.GetString() == "count" ? 1 : 0;
            if (variantBranch != expectedVariantBranch)
            {
                return false;
            }
        }

        const string nextMarker = "/properties/next/oneOf/";
        int nextMarkerIndex = evaluationLocation.LastIndexOf(nextMarker, StringComparison.Ordinal);
        if (nextMarkerIndex >= 0
            && TryGetBranch(evaluationLocation, nextMarker, nextMarkerIndex, out int nextBranch)
            && TryResolvePointer(root, documentLocation, out JsonElement nextValue))
        {
            int expectedNextBranch = nextValue.ValueKind == JsonValueKind.Null ? 0 : 1;
            if (nextBranch != expectedNextBranch)
            {
                return false;
            }
        }

        return true;
    }

    private static bool TryGetBranch(
        string location,
        string marker,
        int markerIndex,
        out int branch)
    {
        int branchIndex = markerIndex + marker.Length;
        if (branchIndex < location.Length && location[branchIndex] is '0' or '1')
        {
            branch = location[branchIndex] - '0';
            return true;
        }

        branch = default;
        return false;
    }

    private static bool TryResolvePointer(
        JsonElement root,
        string pointer,
        out JsonElement value)
    {
        value = root;
        if (pointer.Length == 0)
        {
            return true;
        }

        if (pointer[0] != '/')
        {
            return false;
        }

        foreach (string encodedSegment in pointer[1..].Split('/'))
        {
            string segment = encodedSegment.Replace("~1", "/", StringComparison.Ordinal)
                .Replace("~0", "~", StringComparison.Ordinal);
            if (value.ValueKind == JsonValueKind.Object)
            {
                if (!value.TryGetProperty(segment, out value))
                {
                    return false;
                }
            }
            else if (value.ValueKind == JsonValueKind.Array
                && int.TryParse(segment, NumberStyles.None, CultureInfo.InvariantCulture, out int index)
                && index >= 0
                && index < value.GetArrayLength())
            {
                value = value[index];
            }
            else
            {
                return false;
            }
        }

        return true;
    }

    private static string? ReasonForLocation(string location)
    {
        int separator = location.LastIndexOf('/');
        string keyword = separator >= 0 ? location[(separator + 1)..] : location;
        return KeywordReasons.GetValueOrDefault(keyword);
    }
}
