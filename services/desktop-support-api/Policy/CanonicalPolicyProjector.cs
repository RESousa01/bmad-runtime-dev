using System.Buffers;
using System.Security.Cryptography;
using System.Text;
using System.Text.Encodings.Web;
using System.Text.Json;

namespace Sapphirus.DesktopSupportApi.Policy;

/// <summary>
/// Deterministic canonical JSON hashing for signed support-plane artifacts,
/// matching the Rust `canonical_hash` (RFC 8785 UTF-16 lexical key order,
/// purpose preimage <c>sapphirus:&lt;purpose&gt;:v&lt;major&gt;\n</c>).
/// </summary>
internal static class CanonicalProof
{
    public static byte[] ComputeDigest(
        string purpose,
        SortedDictionary<string, object?> draft)
    {
        ArrayBufferWriter<byte> buffer = new();
        using (Utf8JsonWriter writer = new(buffer, new JsonWriterOptions
        {
            Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
        }))
        {
            WriteValue(writer, draft);
            writer.Flush();
        }
        using IncrementalHash hash = IncrementalHash.CreateHash(HashAlgorithmName.SHA256);
        hash.AppendData(Encoding.UTF8.GetBytes($"sapphirus:{purpose}:v1\n"));
        hash.AppendData(buffer.WrittenSpan);
        return hash.GetHashAndReset();
    }

    public static string Render(byte[] digest) =>
        "sha256:" + Convert.ToHexStringLower(digest);

    private static void WriteValue(Utf8JsonWriter writer, object? value)
    {
        switch (value)
        {
            case SortedDictionary<string, object?> nested:
                writer.WriteStartObject();
                foreach ((string name, object? entry) in nested)
                {
                    writer.WritePropertyName(name);
                    WriteValue(writer, entry);
                }
                writer.WriteEndObject();
                break;
            case IEnumerable<object> array:
                writer.WriteStartArray();
                foreach (object entry in array)
                {
                    WriteValue(writer, entry);
                }
                writer.WriteEndArray();
                break;
            case string text:
                writer.WriteStringValue(text);
                break;
            case bool flag:
                writer.WriteBooleanValue(flag);
                break;
            case int number:
                writer.WriteNumberValue(number);
                break;
            case long number:
                writer.WriteNumberValue(number);
                break;
            case null:
                writer.WriteNullValue();
                break;
            default:
                throw new InvalidOperationException(
                    "Unsupported canonical proof value type.");
        }
    }
}

/// <summary>
/// Projects a validated policy snapshot into the canonical signed policy
/// payload and its exact signing digest.
/// </summary>
public static class CanonicalPolicyProjector
{
    public static (SignedDesktopPolicy UnsignedPolicy, byte[] SigningDigest) Project(
        PolicySnapshot snapshot)
    {
        ArgumentNullException.ThrowIfNull(snapshot);
        SortedDictionary<string, object?> draft = new(StringComparer.Ordinal)
        {
            ["schemaVersion"] = "desktop-policy.v1",
            ["policyId"] = snapshot.PolicyId,
            ["policyVersion"] = snapshot.PolicyVersion,
            ["systemBrowserFallbackAllowed"] = snapshot.SystemBrowserFallbackAllowed,
            ["maximumContextBytes"] = snapshot.MaximumContextBytes,
            ["maximumContextItems"] = snapshot.MaximumContextItems,
            ["allowedRegions"] = snapshot.AllowedRegions.Cast<object>().ToArray(),
            ["retentionMode"] = snapshot.RetentionMode,
        };
        byte[] digest = CanonicalProof.ComputeDigest("desktop-policy", draft);
        SignedDesktopPolicy unsigned = new(
            "desktop-policy.v1",
            snapshot.PolicyId,
            snapshot.PolicyVersion,
            CanonicalProof.Render(digest),
            snapshot.SystemBrowserFallbackAllowed,
            snapshot.MaximumContextBytes,
            snapshot.MaximumContextItems,
            snapshot.AllowedRegions,
            "",
            "");
        return (unsigned, digest);
    }

    public static (SignedEntitlementLease UnsignedLease, byte[] SigningDigest) ProjectLease(
        SignedEntitlementLease unsignedLease)
    {
        ArgumentNullException.ThrowIfNull(unsignedLease);
        SortedDictionary<string, object?> draft = new(StringComparer.Ordinal)
        {
            ["schemaVersion"] = unsignedLease.SchemaVersion,
            ["leaseId"] = unsignedLease.LeaseId,
            ["registrationId"] = unsignedLease.RegistrationId,
            ["subjectHash"] = unsignedLease.SubjectHash,
            ["deliveryModel"] = unsignedLease.DeliveryModel,
            ["issuedAt"] = RenderInstant(unsignedLease.IssuedAt),
            ["notBefore"] = RenderInstant(unsignedLease.NotBefore),
            ["expiresAt"] = RenderInstant(unsignedLease.ExpiresAt),
            ["offlineGraceEndsAt"] = RenderInstant(unsignedLease.OfflineGraceEndsAt),
            ["features"] = unsignedLease.Features.Cast<object>().ToArray(),
            ["tenantPolicyHash"] = unsignedLease.TenantPolicyHash,
            ["minimumClientVersion"] = unsignedLease.MinimumClientVersion,
        };
        byte[] digest = CanonicalProof.ComputeDigest("entitlement-lease", draft);
        return (unsignedLease, digest);
    }

    internal static string RenderInstant(DateTimeOffset value) =>
        value.ToUniversalTime().ToString(
            "yyyy-MM-dd'T'HH:mm:ss.fff'Z'",
            System.Globalization.CultureInfo.InvariantCulture);
}
