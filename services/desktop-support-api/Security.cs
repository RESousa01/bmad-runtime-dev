using System.Buffers;
using System.Security.Claims;
using System.Security.Cryptography;
using System.Text;
using System.Text.Encodings.Web;
using System.Text.Json;
using Microsoft.AspNetCore.Diagnostics;

namespace Sapphirus.DesktopSupportApi;

public static class DesktopScopes
{
    public const string Access = "Desktop.Access";
    public const string ModelInvoke = "Desktop.Model.Invoke";
    public const string PackageRead = "Desktop.Package.Read";
    public const string Sync = "Desktop.Sync";
    public const string Collaboration = "Desktop.Collaboration";
    public const string RemoteJobSubmit = "Desktop.RemoteJob.Submit";
    public const string DiagnosticsUpload = "Desktop.Diagnostics.Upload";

    public static readonly string[] All =
    [
        Access,
        ModelInvoke,
        PackageRead,
        Sync,
        Collaboration,
        RemoteJobSubmit,
        DiagnosticsUpload,
    ];
}

public static class ScopeAuthorization
{
    public static bool HasScope(ClaimsPrincipal principal, string requiredScope) =>
        HasStableDesktopSubject(principal)
        && principal.FindAll("scp")
            .SelectMany(claim => claim.Value.Split(' ', StringSplitOptions.RemoveEmptyEntries))
            .Contains(requiredScope, StringComparer.Ordinal);

    public static bool IsApprovedDesktopPrincipal(
        ClaimsPrincipal principal,
        string requiredScope,
        SupportPlaneOptions options)
    {
        if (!HasScope(principal, requiredScope)
            || !Guid.TryParseExact(principal.FindFirstValue("tid"), "D", out Guid tenantId)
            || tenantId != options.TenantId)
        {
            return false;
        }

        string[] presentedClients =
        [
            principal.FindFirstValue("azp") ?? "",
            principal.FindFirstValue("appid") ?? "",
        ];
        Guid[] clients = presentedClients
            .Where(value => !string.IsNullOrWhiteSpace(value))
            .Select(value => Guid.TryParseExact(value, "D", out Guid client) ? client : Guid.Empty)
            .Distinct()
            .ToArray();
        return clients.Length == 1 && clients[0] == options.ApprovedDesktopClient;
    }

    private static bool HasStableDesktopSubject(ClaimsPrincipal principal) =>
        !string.IsNullOrWhiteSpace(principal.FindFirstValue("tid"))
        && !string.IsNullOrWhiteSpace(
            principal.FindFirstValue("oid") ?? principal.FindFirstValue("sub"));

    public static string SubjectPartition(ClaimsPrincipal principal)
    {
        if (!TrySubjectPartition(principal, out string partition))
        {
            throw new InvalidOperationException(
                "The authenticated identity has no stable tenant and subject claims.");
        }
        return partition;
    }

    public static string RateLimitPartition(ClaimsPrincipal principal) =>
        TrySubjectPartition(principal, out string partition)
            ? partition
            : "unauthenticated";

    private static bool TrySubjectPartition(
        ClaimsPrincipal principal,
        out string partition)
    {
        partition = "";
        if (!Guid.TryParseExact(principal.FindFirstValue("tid"), "D", out Guid tenantId))
        {
            return false;
        }
        string? subjectValue = principal.FindFirstValue("oid") ?? principal.FindFirstValue("sub");
        if (string.IsNullOrWhiteSpace(subjectValue))
        {
            return false;
        }
        string subject = Guid.TryParseExact(subjectValue, "D", out Guid subjectId)
            ? subjectId.ToString("D")
            : subjectValue;
        string tenant = tenantId.ToString("D");
        partition = Convert.ToHexStringLower(SHA256.HashData(
            Encoding.UTF8.GetBytes($"{tenant}:{subject}")));
        return true;
    }
}

public static class RequestGuards
{
    private const int MaximumContextItems = 64;
    private const int MaximumContextBytes = 512 * 1024;

    public static string? IdempotencyKey(HttpRequest request)
    {
        string value = request.Headers["Idempotency-Key"].ToString();
        return value.Length is >= 16 and <= 128
            && value.All(character => char.IsAsciiLetterOrDigit(character) || character is '-' or '_')
            ? value
            : null;
    }

    public static bool IsSha256(string? value)
    {
        if (value is null
            || value.Length != 71
            || !value.StartsWith("sha256:", StringComparison.Ordinal))
        {
            return false;
        }
        foreach (char character in value.AsSpan(7))
        {
            if (character is not (>= '0' and <= '9') and not (>= 'a' and <= 'f'))
            {
                return false;
            }
        }
        return true;
    }

    public static bool IsRegistrationId(string? value) =>
        value is not null
        && value.Length == 31
        && value.StartsWith("dreg_", StringComparison.Ordinal)
        && value.AsSpan(5).ToArray().All(character =>
            character is >= '0' and <= '9' or >= 'a' and <= 'f');

    public static bool ValidateDeviceRegistration(
        DeviceRegistrationRequest request,
        out string errorCode)
    {
        errorCode = "request_invalid";
        return request is not null
            && IsSha256(request.InstallationPublicKeyHash)
            && IsBounded(request.ClientRelease, 1, 64)
            && IsBounded(request.Platform, 1, 32)
            && IsBounded(request.Architecture, 1, 32)
            && IsBounded(request.TenantPolicyVersion, 1, 128);
    }

    public static bool ValidateModelRequest(
        ModelAccessRequest request,
        out string errorCode,
        out string recomputedManifestHash)
    {
        errorCode = "request_invalid";
        recomputedManifestHash = "";
        if (request is null
            || request.SchemaVersion != "desktop-model-access-request.v1"
            || request.DeliveryModel != "windows_local"
            || request.RetentionMode != "transient_no_store"
            || !IsRegistrationId(request.RegistrationId)
            || !IsBounded(request.RequestId, 8, 128)
            || !IsBounded(request.Purpose, 1, 128)
            || !IsBounded(request.ModelRole, 1, 128)
            || !IsBounded(request.CanonicalOutputSchemaId, 1, 512)
            || !IsBounded(request.BudgetClass, 1, 64)
            || request.Items is null
            || request.Items.Length is < 1 or > MaximumContextItems
            || !IsSha256(request.CanonicalOutputSchemaHash)
            || !IsSha256(request.LocalEgressManifestHash)
            || !IsSha256(request.ConsentReceiptHash))
        {
            return false;
        }
        int totalBytes = 0;
        HashSet<string> itemIds = new(StringComparer.Ordinal);
        foreach (ModelContextItem item in request.Items)
        {
            if (item is null
                || !IsBounded(item.ClientItemId, 1, 128)
                || !itemIds.Add(item.ClientItemId)
                || !IsBounded(item.RelativeLabel, 1, 512)
                || !IsBounded(item.SemanticRole, 1, 128)
                || !IsBounded(item.Language, 1, 64)
                || !IsBounded(item.Classification, 1, 128)
                || item.Content is null
                || item.ByteCount < 0
                || item.ByteCount != Encoding.UTF8.GetByteCount(item.Content)
                || item.RelativeLabel.StartsWith('/', StringComparison.Ordinal)
                || item.RelativeLabel.Contains('\\', StringComparison.Ordinal)
                || item.RelativeLabel.Contains(':', StringComparison.Ordinal)
                || item.RelativeLabel.Split('/').Any(segment => segment is "." or ".." or "")
                || !IsSha256(item.ContentHash))
            {
                errorCode = "context_item_invalid";
                return false;
            }
            string actualContentHash = HashBytes(Encoding.UTF8.GetBytes(item.Content));
            if (!string.Equals(actualContentHash, item.ContentHash, StringComparison.Ordinal))
            {
                errorCode = "context_item_hash_mismatch";
                return false;
            }
            if (item.ByteCount > MaximumContextBytes - totalBytes)
            {
                errorCode = "context_limit_exceeded";
                return false;
            }
            totalBytes += item.ByteCount;
        }
        recomputedManifestHash = ComputeContextManifestHash(request);
        if (!string.Equals(
            recomputedManifestHash,
            request.LocalEgressManifestHash,
            StringComparison.Ordinal))
        {
            errorCode = "context_manifest_mismatch";
            return false;
        }
        return true;
    }

    public static string ComputeContextManifestHash(ModelAccessRequest request)
    {
        ArgumentNullException.ThrowIfNull(request);
        ArrayBufferWriter<byte> buffer = new();
        using (Utf8JsonWriter writer = new(buffer, new JsonWriterOptions
        {
            Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
        }))
        {
            // Property order is RFC 8785/JCS UTF-16 lexical order for this closed manifest shape.
            writer.WriteStartObject();
            writer.WriteString("budgetClass", request.BudgetClass);
            writer.WriteString("canonicalOutputSchemaHash", request.CanonicalOutputSchemaHash);
            writer.WriteString("canonicalOutputSchemaId", request.CanonicalOutputSchemaId);
            writer.WriteString("deliveryModel", request.DeliveryModel);
            writer.WritePropertyName("items");
            writer.WriteStartArray();
            foreach (ModelContextItem item in request.Items)
            {
                writer.WriteStartObject();
                writer.WriteNumber("byteCount", item.ByteCount);
                writer.WriteString("classification", item.Classification);
                writer.WriteString("clientItemId", item.ClientItemId);
                writer.WriteString("contentHash", item.ContentHash);
                writer.WriteString("language", item.Language);
                writer.WriteString("relativeLabel", item.RelativeLabel);
                writer.WriteString("semanticRole", item.SemanticRole);
                writer.WriteEndObject();
            }
            writer.WriteEndArray();
            writer.WriteString("modelRole", request.ModelRole);
            writer.WriteString("purpose", request.Purpose);
            writer.WriteString("registrationId", request.RegistrationId);
            writer.WriteString("requestId", request.RequestId);
            writer.WriteString("retentionMode", request.RetentionMode);
            writer.WriteString("schemaVersion", request.SchemaVersion);
            writer.WriteEndObject();
            writer.Flush();
        }

        using IncrementalHash hash = IncrementalHash.CreateHash(HashAlgorithmName.SHA256);
        hash.AppendData(Encoding.UTF8.GetBytes("sapphirus:context-egress:1\n"));
        hash.AppendData(buffer.WrittenSpan);
        return "sha256:" + Convert.ToHexStringLower(hash.GetHashAndReset());
    }

    public static object SafeProblem(
        string code,
        int status = StatusCodes.Status400BadRequest) => new
    {
        type = $"https://errors.sapphirus.invalid/{code}",
        title = "The request could not be accepted.",
        status,
        code,
    };

    public static string Fingerprint<T>(T request) => "sha256:" + Convert.ToHexStringLower(
        SHA256.HashData(JsonSerializer.SerializeToUtf8Bytes(request)));

    private static bool IsBounded(string? value, int minimum, int maximum) =>
        value is not null && value.Length >= minimum && value.Length <= maximum;

    private static string HashBytes(ReadOnlySpan<byte> value) =>
        "sha256:" + Convert.ToHexStringLower(SHA256.HashData(value));
}

public sealed class SafeApiExceptionHandler(ILogger<SafeApiExceptionHandler> logger)
    : IExceptionHandler
{
    public async ValueTask<bool> TryHandleAsync(
        HttpContext httpContext,
        Exception exception,
        CancellationToken cancellationToken)
    {
        (int status, string code) = exception switch
        {
            IdempotencyConflictException =>
                (StatusCodes.Status409Conflict, "idempotency_conflict"),
            DeviceRegistrationRevokedException =>
                (StatusCodes.Status403Forbidden, "device_registration_revoked"),
            IdempotencyCapacityException =>
                (StatusCodes.Status503ServiceUnavailable, "idempotency_capacity_exhausted"),
            OperationCanceledException when httpContext.RequestAborted.IsCancellationRequested =>
                (StatusCodes.Status499ClientClosedRequest, "request_cancelled"),
            InvalidOperationException =>
                (StatusCodes.Status503ServiceUnavailable, "dependency_unavailable"),
            _ => (StatusCodes.Status500InternalServerError, "internal_error"),
        };
        logger.LogError(
            "Support API request failed with {ErrorCode}; exception type {ExceptionType}",
            code,
            exception.GetType().FullName);
        httpContext.Response.StatusCode = status;
        await httpContext.Response.WriteAsJsonAsync(
            new
            {
                type = $"https://errors.sapphirus.invalid/{code}",
                title = "The request could not be completed.",
                status,
                code,
            },
            cancellationToken);
        return true;
    }
}
