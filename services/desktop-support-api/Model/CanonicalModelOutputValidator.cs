using System.Buffers;
using System.Security.Cryptography;
using System.Text;
using System.Text.Encodings.Web;
using System.Text.Json;

namespace Sapphirus.DesktopSupportApi.Model;

/// <summary>A validated canonical model output.</summary>
public sealed record CanonicalModelOutput(
    string PayloadJson,
    string PayloadHash,
    string SchemaProjectionHash);

/// <summary>
/// Validates raw provider output against the closed canonical schema
/// <c>sapphirus.bmad-method-help-proposal.v1</c> and re-serializes it into
/// deterministic canonical form. Unknown fields, wrong types, and oversized
/// values are all schema violations.
/// </summary>
public static class CanonicalModelOutputValidator
{
    /// <summary>The strict provider-side JSON schema for the output shape.</summary>
    public const string OutputSchemaJson =
        """
        {"type":"object","additionalProperties":false,"required":["summary","steps","proposedChanges"],"properties":{"summary":{"type":"string","maxLength":4096},"steps":{"type":"array","maxItems":32,"items":{"type":"string","maxLength":1024}},"proposedChanges":{"type":"array","maxItems":64,"items":{"type":"object","additionalProperties":false,"required":["path","rationale"],"properties":{"path":{"type":"string","maxLength":512},"rationale":{"type":"string","maxLength":2048}}}}}}
        """;

    public static string SchemaProjectionHash { get; } = "sha256:"
        + Convert.ToHexStringLower(
            SHA256.HashData(Encoding.UTF8.GetBytes(OutputSchemaJson)));

    public static CanonicalModelOutput Validate(string rawOutput, int maximumOutputBytes)
    {
        ArgumentNullException.ThrowIfNull(rawOutput);
        if (Encoding.UTF8.GetByteCount(rawOutput) > maximumOutputBytes)
        {
            throw new ModelAccessFailedException("schema_invalid");
        }
        JsonDocument document;
        try
        {
            document = JsonDocument.Parse(rawOutput, new JsonDocumentOptions
            {
                MaxDepth = 8,
            });
        }
        catch (JsonException)
        {
            throw new ModelAccessFailedException("malformed_output");
        }
        using (document)
        {
            JsonElement root = document.RootElement;
            if (root.ValueKind != JsonValueKind.Object
                || CountProperties(root) != 3
                || !TryReadString(root, "summary", 4096, out string? summary)
                || !TryReadStringArray(root, "steps", 32, 1024, out string[]? steps)
                || !TryReadChanges(root, out (string Path, string Rationale)[]? changes))
            {
                throw new ModelAccessFailedException("schema_invalid");
            }

            ArrayBufferWriter<byte> buffer = new();
            using (Utf8JsonWriter writer = new(buffer, new JsonWriterOptions
            {
                Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
            }))
            {
                writer.WriteStartObject();
                writer.WritePropertyName("proposedChanges");
                writer.WriteStartArray();
                foreach ((string path, string rationale) in changes!)
                {
                    writer.WriteStartObject();
                    writer.WriteString("path", path);
                    writer.WriteString("rationale", rationale);
                    writer.WriteEndObject();
                }
                writer.WriteEndArray();
                writer.WritePropertyName("steps");
                writer.WriteStartArray();
                foreach (string step in steps!)
                {
                    writer.WriteStringValue(step);
                }
                writer.WriteEndArray();
                writer.WriteString("summary", summary);
                writer.WriteEndObject();
                writer.Flush();
            }
            string payloadJson = Encoding.UTF8.GetString(buffer.WrittenSpan);
            string payloadHash = "sha256:" + Convert.ToHexStringLower(
                SHA256.HashData(buffer.WrittenSpan));
            return new CanonicalModelOutput(payloadJson, payloadHash, SchemaProjectionHash);
        }
    }

    private static int CountProperties(JsonElement element)
    {
        int count = 0;
        foreach (JsonProperty _ in element.EnumerateObject())
        {
            count++;
        }
        return count;
    }

    private static bool TryReadString(
        JsonElement element,
        string name,
        int maximumLength,
        out string? value)
    {
        value = null;
        if (!element.TryGetProperty(name, out JsonElement property)
            || property.ValueKind != JsonValueKind.String)
        {
            return false;
        }
        string? text = property.GetString();
        if (text is null || text.Length > maximumLength)
        {
            return false;
        }
        value = text;
        return true;
    }

    private static bool TryReadStringArray(
        JsonElement element,
        string name,
        int maximumItems,
        int maximumLength,
        out string[]? values)
    {
        values = null;
        if (!element.TryGetProperty(name, out JsonElement property)
            || property.ValueKind != JsonValueKind.Array
            || property.GetArrayLength() > maximumItems)
        {
            return false;
        }
        List<string> collected = [];
        foreach (JsonElement entry in property.EnumerateArray())
        {
            if (entry.ValueKind != JsonValueKind.String)
            {
                return false;
            }
            string? text = entry.GetString();
            if (text is null || text.Length > maximumLength)
            {
                return false;
            }
            collected.Add(text);
        }
        values = [.. collected];
        return true;
    }

    private static bool TryReadChanges(
        JsonElement element,
        out (string Path, string Rationale)[]? changes)
    {
        changes = null;
        if (!element.TryGetProperty("proposedChanges", out JsonElement property)
            || property.ValueKind != JsonValueKind.Array
            || property.GetArrayLength() > 64)
        {
            return false;
        }
        List<(string, string)> collected = [];
        foreach (JsonElement entry in property.EnumerateArray())
        {
            if (entry.ValueKind != JsonValueKind.Object
                || CountProperties(entry) != 2
                || !TryReadString(entry, "path", 512, out string? path)
                || !TryReadString(entry, "rationale", 2048, out string? rationale))
            {
                return false;
            }
            collected.Add((path!, rationale!));
        }
        changes = [.. collected];
        return true;
    }
}
