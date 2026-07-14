using System.Text.Json;

namespace Sapphirus.Contracts.Conformance.Tests;

public sealed class BmadStrictJsonException(string code, string message, Exception? inner = null)
    : JsonException(message, inner)
{
    public string Code { get; } = code;
}

/// <summary>
/// Applies the BMAD model-output byte/depth limits and duplicate-member rejection
/// before any generated Corvus structural validator is invoked.
/// </summary>
public static class StrictBmadJson
{
    public const int MaximumBytes = 2_097_152;
    public const int MaximumContainerDepth = 16;

    public static JsonDocument Parse(ReadOnlyMemory<byte> source)
    {
        if (source.Length > MaximumBytes)
        {
            throw new BmadStrictJsonException(
                "MAX_BYTES_EXCEEDED",
                "BMAD JSON exceeds the reviewed serialized model-response limit.");
        }

        try
        {
            Inspect(source.Span);
            return JsonDocument.Parse(
                source,
                new JsonDocumentOptions
                {
                    AllowTrailingCommas = false,
                    CommentHandling = JsonCommentHandling.Disallow,
                    MaxDepth = 0,
                });
        }
        catch (BmadStrictJsonException)
        {
            throw;
        }
        catch (JsonException exception)
        {
            throw new BmadStrictJsonException(
                "BMAD_SCHEMA_INVALID",
                "BMAD JSON syntax is invalid.",
                exception);
        }
    }

    private static void Inspect(ReadOnlySpan<byte> source)
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
        int depth = 0;

        while (reader.Read())
        {
            switch (reader.TokenType)
            {
                case JsonTokenType.StartObject:
                    depth++;
                    AssertDepth(depth);
                    containers.Push(new HashSet<string>(StringComparer.Ordinal));
                    break;
                case JsonTokenType.StartArray:
                    depth++;
                    AssertDepth(depth);
                    containers.Push(null);
                    break;
                case JsonTokenType.EndObject:
                case JsonTokenType.EndArray:
                    containers.Pop();
                    depth--;
                    break;
                case JsonTokenType.PropertyName:
                {
                    string name = reader.GetString()
                        ?? throw new BmadStrictJsonException(
                            "INVALID_UNICODE",
                            "A BMAD JSON member name is not valid Unicode.");
                    HashSet<string>? members = containers.Peek();
                    if (members is null || !members.Add(name))
                    {
                        throw new BmadStrictJsonException(
                            "DUPLICATE_MEMBER",
                            $"Duplicate BMAD JSON member {name}.");
                    }

                    break;
                }
            }
        }
    }

    private static void AssertDepth(int depth)
    {
        if (depth > MaximumContainerDepth)
        {
            throw new BmadStrictJsonException(
                "MAX_DEPTH_EXCEEDED",
                "BMAD JSON exceeds the reviewed container-depth limit.");
        }
    }
}
