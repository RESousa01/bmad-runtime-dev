using System.Buffers;
using System.Text;
using System.Text.Encodings.Web;
using System.Text.Json;

namespace Sapphirus.DesktopSupportApi.Model;

/// <summary>The in-memory minimum prompt for one authorized request.</summary>
public sealed record CanonicalPrompt(string SystemInstruction, string UserPayloadJson);

/// <summary>
/// Builds the minimum provider prompt from reviewed context items, enforcing
/// every pre-egress guard: no absolute/UNC/drive/traversal paths, no
/// username-bearing labels, no whole-repository patterns, bounded sizes, and
/// only supported classifications. Violations fail before any provider call.
/// </summary>
public static class CanonicalPromptProjector
{
    private static readonly string[] SupportedClassifications =
        ["public", "internal", "source"];

    private static readonly string[] UsernameMarkers =
        ["users", "home", "documents and settings"];

    public static CanonicalPrompt Project(
        ModelAccessRequest request,
        ModelAccessProfile profile)
    {
        ArgumentNullException.ThrowIfNull(request);
        ArgumentNullException.ThrowIfNull(profile);
        if (request.Items.Length > profile.MaximumContextItems)
        {
            throw new ModelAccessFailedException("context_rejected");
        }
        long totalBytes = 0;
        foreach (ModelContextItem item in request.Items)
        {
            RequireSafeLabel(item.RelativeLabel);
            if (!SupportedClassifications.Contains(item.Classification, StringComparer.Ordinal))
            {
                throw new ModelAccessFailedException("context_rejected");
            }
            totalBytes += item.ByteCount;
        }
        if (totalBytes > profile.MaximumContextBytes)
        {
            throw new ModelAccessFailedException("context_rejected");
        }

        ArrayBufferWriter<byte> buffer = new();
        using (Utf8JsonWriter writer = new(buffer, new JsonWriterOptions
        {
            Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
        }))
        {
            writer.WriteStartObject();
            writer.WriteString("purpose", profile.ApprovedPurpose);
            writer.WriteString("outputSchemaId", profile.CanonicalOutputSchemaId);
            writer.WritePropertyName("items");
            writer.WriteStartArray();
            foreach (ModelContextItem item in request.Items)
            {
                writer.WriteStartObject();
                writer.WriteString("relativeLabel", item.RelativeLabel);
                writer.WriteString("semanticRole", item.SemanticRole);
                writer.WriteString("language", item.Language);
                writer.WriteString("content", item.Content);
                writer.WriteEndObject();
            }
            writer.WriteEndArray();
            writer.WriteEndObject();
            writer.Flush();
        }
        return new CanonicalPrompt(
            "You are the Sapphirus desktop planning assistant. Use only the "
            + "provided context items. Respond with exactly one JSON object "
            + "matching the requested schema. Do not use tools, browse, or "
            + "reference external resources.",
            Encoding.UTF8.GetString(buffer.WrittenSpan));
    }

    private static void RequireSafeLabel(string relativeLabel)
    {
        string[] segments = relativeLabel.Split('/');
        bool unsafeLabel =
            relativeLabel.Length is < 1 or > 512
            || relativeLabel.StartsWith('/')
            || relativeLabel.StartsWith('\\')
            || relativeLabel.Contains('\\', StringComparison.Ordinal)
            || relativeLabel.Contains(':', StringComparison.Ordinal)
            || relativeLabel.Contains("**", StringComparison.Ordinal)
            || relativeLabel.Contains('*', StringComparison.Ordinal)
            || segments.Any(static segment => segment is "." or ".." or "")
            || segments.Any(static segment =>
                UsernameMarkers.Contains(segment, StringComparer.OrdinalIgnoreCase));
        if (unsafeLabel)
        {
            throw new ModelAccessFailedException("context_rejected");
        }
    }
}
