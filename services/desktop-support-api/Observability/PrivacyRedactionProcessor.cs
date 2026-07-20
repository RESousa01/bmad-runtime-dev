using System.Diagnostics;
using OpenTelemetry;

namespace Sapphirus.DesktopSupportApi.Observability;

/// <summary>
/// Last-line privacy defense for exported spans. Tags outside the allowlist
/// are removed; allowlisted tags whose values look like content hashes,
/// tokens, signatures, paths, or usernames are redacted; exception and
/// custom events are dropped entirely (the safe error code is already a
/// tag); raw URL paths are replaced by the route template.
/// </summary>
public sealed class PrivacyRedactionProcessor : BaseProcessor<Activity>
{
    private static readonly string[] AllowedTagKeys =
    [
        "http.route",
        "http.request.method",
        "http.response.status_code",
        "network.protocol.version",
        "server.address",
        "error.type",
        "otel.status_code",
        "sapphirus.error_code",
        "sapphirus.dependency",
        "sapphirus.outcome",
    ];

    public override void OnEnd(Activity activity)
    {
        foreach (KeyValuePair<string, string?> tag in activity.Tags.ToArray())
        {
            if (!AllowedTagKeys.Contains(tag.Key, StringComparer.Ordinal))
            {
                activity.SetTag(tag.Key, null);
                continue;
            }
            if (tag.Value is not null && !IsSafeValue(tag.Key, tag.Value))
            {
                activity.SetTag(tag.Key, "[redacted]");
            }
        }
        foreach (KeyValuePair<string, object?> tag in activity.TagObjects.ToArray())
        {
            if (!AllowedTagKeys.Contains(tag.Key, StringComparer.Ordinal))
            {
                activity.SetTag(tag.Key, null);
            }
        }
        // Exception events carry messages and stack traces; the safe error
        // code tag is the only failure detail that may leave the process.
        if (activity.Events.Any())
        {
            ClearEvents(activity);
            if (activity.Events.Any())
            {
                // The runtime no longer allows clearing events; failing open
                // is not an option, so the whole span is un-recorded.
                activity.ActivityTraceFlags &= ~ActivityTraceFlags.Recorded;
                activity.IsAllDataRequested = false;
            }
        }
        if (activity.TagObjects.Any(static tag => tag.Key == "http.route"))
        {
            activity.DisplayName = activity
                .TagObjects
                .First(static tag => tag.Key == "http.route")
                .Value?.ToString() ?? activity.DisplayName;
        }
        base.OnEnd(activity);
    }

    internal static bool IsSafeValue(string key, string value)
    {
        if (key is "http.response.status_code")
        {
            return value.Length <= 3;
        }
        return value.Length <= 256
            && !value.Contains("sha256:", StringComparison.OrdinalIgnoreCase)
            && !value.StartsWith("eyJ", StringComparison.Ordinal)
            && !value.Contains(":\\", StringComparison.Ordinal)
            && !value.Contains("\\\\", StringComparison.Ordinal)
            && !value.Contains("/Users/", StringComparison.OrdinalIgnoreCase)
            && !value.Contains("/home/", StringComparison.OrdinalIgnoreCase)
            && !(key != "http.route" && LooksLikeOpaqueBlob(value));
    }

    private static bool LooksLikeOpaqueBlob(string value) =>
        value.Length >= 40
        && value.All(static character =>
            char.IsAsciiLetterOrDigit(character) || character is '-' or '_' or '=' or '.');

    private static void ClearEvents(Activity activity)
    {
        // Activity has no public event-removal API; recreating the list via
        // reflection is brittle, so instead the span is marked and exporters
        // in this app are configured with this processor before export.
        // Dropping is achieved by replacing the activity's events source.
        System.Reflection.FieldInfo? eventsField = typeof(Activity).GetField(
            "_events",
            System.Reflection.BindingFlags.Instance
            | System.Reflection.BindingFlags.NonPublic);
        eventsField?.SetValue(activity, null);
    }
}
