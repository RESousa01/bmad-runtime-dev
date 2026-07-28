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
    private const int MaximumTools = 32;

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

    public static bool IsRegistrationId(string? value) => ContractIds.Is(value, "dreg");

    public static bool ValidateDeviceRegistration(
        DeviceRegistrationRequest request,
        out string errorCode)
    {
        errorCode = "request_invalid";
        return request is not null
            && request.SchemaVersion == "desktop-device-registration.v1"
            && TryGetInstallationPublicKeyHash(
                request.InstallationPublicKey,
                out string installationPublicKeyHash)
            && IsSha256(request.InstallationPublicKeyHash)
            && string.Equals(
                installationPublicKeyHash,
                request.InstallationPublicKeyHash,
                StringComparison.Ordinal)
            && IsBounded(request.ClientRelease, 1, 64)
            && request.Platform == "windows"
            && request.Architecture is "x64" or "arm64"
            && request.TenantPolicyVersion is >= 1 and <= 9007199254740991;
    }

    public static bool TryGetInstallationPublicKeyHash(
        string? encodedPublicKey,
        out string installationPublicKeyHash)
    {
        installationPublicKeyHash = "";
        if (!Security.InstallationPublicKey.TryParse(
            encodedPublicKey,
            out Security.InstallationPublicKey? key)
            || key is null)
        {
            return false;
        }
        installationPublicKeyHash = key.Hash;
        return true;
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
            || request.Consent is null)
        {
            return false;
        }
        if (!ValidateConsentBinding(request))
        {
            errorCode = "consent_binding_mismatch";
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
                || item.RelativeLabel.StartsWith("/", StringComparison.Ordinal)
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

    /// <summary>
    /// Validates a version two streaming request.
    ///
    /// <para>
    /// Streaming mode is the only mode this route accepts, and a streaming
    /// request that also carries a canonical output schema is refused: asking
    /// for a token stream and a structured document at once is asking for two
    /// different result contracts from one call.
    /// </para>
    /// <para>
    /// Every consent field that duplicates a request field must agree with it,
    /// so changing the profile, epoch, variant, mode, or context snapshot after
    /// the decision was signed produces a decision that no longer matches the
    /// request it is presented with. Signature verification itself is the
    /// consent verifier's job, not this method's.
    /// </para>
    /// </summary>
    public static bool ValidateStreamRequest(
        ModelAccessRequestV2 request,
        out string errorCode)
    {
        errorCode = SupportProblemCodes.RequestInvalid;
        if (request is null
            || request.SchemaVersion != ModelAccessRequestV2.SchemaVersionValue
            || request.DeliveryModel != "windows_local"
            || request.RetentionMode != "transient_no_store"
            || request.RequestMode != ModelRequestModes.StreamingChat
            || !IsRegistrationId(request.RegistrationId)
            || !IsBounded(request.RequestId, 8, 128)
            || !IsBounded(request.Purpose, 1, 64)
            || !IsBounded(request.ModelRole, 1, 64)
            || !IsBounded(request.ModelProfileId, 1, 64)
            || !IsBounded(request.CatalogVersion, 3, 128)
            || !IsBounded(request.ContextSnapshotId, 8, 128)
            || !IsBounded(request.BudgetClass, 1, 64)
            || (request.RequestedVariantId is not null
                && !IsBounded(request.RequestedVariantId, 1, 64))
            || !IsSha256(request.ModelProfileHash)
            || !IsSha256(request.ModelCapabilityHash)
            || !IsSha256(request.LocalEgressManifestHash)
            || request.Items is null
            || request.Items.Length is < 1 or > MaximumContextItems
            || request.Tools is null
            || request.Tools.Length > MaximumTools
            || request.Consent is null)
        {
            return false;
        }
        if (request.CanonicalOutputSchemaId is not null
            || request.CanonicalOutputSchemaHash is not null)
        {
            errorCode = SupportProblemCodes.ModelCapabilityMismatch;
            return false;
        }
        if (!ValidateStreamItems(request, out errorCode))
        {
            return false;
        }
        if (!ValidateStreamTools(request.Tools))
        {
            errorCode = SupportProblemCodes.ModelCapabilityMismatch;
            return false;
        }
        if (!ValidateConsentBindingV2(request))
        {
            errorCode = "consent_binding_mismatch";
            return false;
        }
        errorCode = "";
        return true;
    }

    private static bool ValidateStreamItems(
        ModelAccessRequestV2 request,
        out string errorCode)
    {
        errorCode = SupportProblemCodes.RequestInvalid;
        long totalBytes = 0;
        HashSet<string> itemIds = new(StringComparer.Ordinal);
        foreach (ModelContextItem item in request.Items)
        {
            if (item is null
                || !IsBounded(item.ClientItemId, 1, 128)
                || !itemIds.Add(item.ClientItemId)
                || !IsBounded(item.RelativeLabel, 1, 512)
                || !IsBounded(item.SemanticRole, 1, 64)
                || !IsBounded(item.Language, 1, 64)
                || !IsBounded(item.Classification, 1, 128)
                || item.Content is null
                || item.ByteCount < 0
                || item.ByteCount != Encoding.UTF8.GetByteCount(item.Content)
                || item.RelativeLabel.StartsWith("/", StringComparison.Ordinal)
                || item.RelativeLabel.Contains('\\', StringComparison.Ordinal)
                || item.RelativeLabel.Contains(':', StringComparison.Ordinal)
                || item.RelativeLabel.Split('/').Any(segment => segment is "." or ".." or "")
                || !IsSha256(item.ContentHash))
            {
                errorCode = "context_item_invalid";
                return false;
            }
            if (!string.Equals(
                HashBytes(Encoding.UTF8.GetBytes(item.Content)),
                item.ContentHash,
                StringComparison.Ordinal))
            {
                errorCode = "context_item_hash_mismatch";
                return false;
            }
            totalBytes += item.ByteCount;
            if (totalBytes > MaximumContextBytes)
            {
                errorCode = SupportProblemCodes.ModelContextLimitExceeded;
                return false;
            }
        }
        errorCode = "";
        return true;
    }

    private static bool ValidateStreamTools(ModelToolDefinition[] tools)
    {
        HashSet<string> names = new(StringComparer.Ordinal);
        foreach (ModelToolDefinition tool in tools)
        {
            if (tool is null
                || !IsBounded(tool.ToolName, 1, 64)
                || !names.Add(tool.ToolName)
                || !tool.ToolName.All(character =>
                    char.IsAsciiLetterLower(character)
                    || char.IsAsciiDigit(character)
                    || character == '_')
                || !char.IsAsciiLetterLower(tool.ToolName[0])
                || !IsBounded(tool.Description, 1, 256)
                || !IsBounded(tool.ParametersSchemaId, 5, 128)
                || !IsSha256(tool.ParametersSchemaHash))
            {
                return false;
            }
        }
        return true;
    }

    /// <summary>
    /// The version two consent binding. It has no deployment hash to check,
    /// because a version two decision never names a deployment.
    /// </summary>
    private static bool ValidateConsentBindingV2(ModelAccessRequestV2 request)
    {
        ModelContextConsentV2 consent = request.Consent;
        ModelContextConsentProof? proof = consent.Proof;
        return consent.SchemaVersion == ModelContextConsentV2.SchemaVersionValue
            && consent.DeliveryModel == "windows_local"
            && string.Equals(consent.RequestId, request.RequestId, StringComparison.Ordinal)
            && string.Equals(
                consent.RegistrationId,
                request.RegistrationId,
                StringComparison.Ordinal)
            && string.Equals(consent.Purpose, request.Purpose, StringComparison.Ordinal)
            && string.Equals(consent.ModelRole, request.ModelRole, StringComparison.Ordinal)
            && string.Equals(consent.RequestMode, request.RequestMode, StringComparison.Ordinal)
            && string.Equals(
                consent.ModelProfileId,
                request.ModelProfileId,
                StringComparison.Ordinal)
            && string.Equals(
                consent.RequestedVariantId,
                request.RequestedVariantId,
                StringComparison.Ordinal)
            && string.Equals(
                consent.CatalogVersion,
                request.CatalogVersion,
                StringComparison.Ordinal)
            && string.Equals(
                consent.ContextSnapshotId,
                request.ContextSnapshotId,
                StringComparison.Ordinal)
            && consent.CanonicalOutputSchemaId is null
            && consent.CanonicalOutputSchemaHash is null
            && string.Equals(
                consent.ManifestHash,
                request.LocalEgressManifestHash,
                StringComparison.Ordinal)
            && string.Equals(
                consent.ModelProfileHash,
                request.ModelProfileHash,
                StringComparison.Ordinal)
            && string.Equals(
                consent.ModelCapabilityHash,
                request.ModelCapabilityHash,
                StringComparison.Ordinal)
            && string.Equals(
                consent.RetentionMode,
                request.RetentionMode,
                StringComparison.Ordinal)
            && string.Equals(consent.BudgetClass, request.BudgetClass, StringComparison.Ordinal)
            && IsBounded(consent.DecisionId, 8, 128)
            && IsBounded(consent.InvocationId, 8, 128)
            && IsBounded(consent.EntitlementLeaseId, 8, 128)
            && IsBounded(consent.TenantPolicyId, 8, 128)
            && consent.TenantPolicyVersion is >= 1 and <= 9007199254740991
            && IsBounded(consent.Region, 1, 64)
            && consent.IssuedAt != default
            && consent.NotBefore != default
            && consent.ExpiresAt != default
            && consent.IssuedAt <= consent.NotBefore
            && consent.NotBefore < consent.ExpiresAt
            && IsSha256(consent.TenantHash)
            && IsSha256(consent.SubjectHash)
            && IsSha256(consent.InstallationPublicKeyHash)
            && IsSha256(consent.EntitlementLeaseHash)
            && IsSha256(consent.TenantPolicyHash)
            && IsSha256(consent.InvocationBindingHash)
            && IsSha256(consent.ConsumptionHash)
            && IsSha256(consent.ConsentDisclosureHash)
            && IsSha256(consent.ProviderProfileHash)
            && IsSha256(consent.NonceHash)
            && IsSha256(consent.ConsentEnvelopeHash)
            && proof is not null
            && proof.ProofType == "installation_signature"
            && proof.Algorithm == "ES256"
            && IsBounded(proof.KeyId, 1, 128)
            && string.Equals(
                proof.SignedPayloadHash,
                consent.ConsentEnvelopeHash,
                StringComparison.Ordinal)
            && IsBounded(proof.Signature, 16, 2048)
            && proof.Signature.All(character =>
                char.IsAsciiLetterOrDigit(character) || character is '_' or '-');
    }

    private static bool ValidateConsentBinding(ModelAccessRequest request)
    {
        ModelContextConsent consent = request.Consent;
        ModelContextConsentProof? proof = consent.Proof;
        return consent.SchemaVersion == "sapphirus.model-context-consent.v1"
            && consent.DeliveryModel == "windows_local"
            && string.Equals(consent.RequestId, request.RequestId, StringComparison.Ordinal)
            && string.Equals(consent.RegistrationId, request.RegistrationId, StringComparison.Ordinal)
            && string.Equals(consent.Purpose, request.Purpose, StringComparison.Ordinal)
            && string.Equals(consent.ModelRole, request.ModelRole, StringComparison.Ordinal)
            && string.Equals(
                consent.CanonicalOutputSchemaId,
                request.CanonicalOutputSchemaId,
                StringComparison.Ordinal)
            && string.Equals(
                consent.CanonicalOutputSchemaHash,
                request.CanonicalOutputSchemaHash,
                StringComparison.Ordinal)
            && string.Equals(consent.ManifestHash, request.LocalEgressManifestHash, StringComparison.Ordinal)
            && string.Equals(consent.RetentionMode, request.RetentionMode, StringComparison.Ordinal)
            && string.Equals(consent.BudgetClass, request.BudgetClass, StringComparison.Ordinal)
            && IsBounded(consent.DecisionId, 8, 128)
            && IsBounded(consent.InvocationId, 8, 128)
            && IsBounded(consent.EntitlementLeaseId, 8, 128)
            && IsBounded(consent.TenantPolicyId, 8, 128)
            && consent.TenantPolicyVersion is >= 1 and <= 9007199254740991
            && IsBounded(consent.Region, 1, 64)
            && consent.IssuedAt != default
            && consent.NotBefore != default
            && consent.ExpiresAt != default
            && consent.IssuedAt <= consent.NotBefore
            && consent.NotBefore < consent.ExpiresAt
            && IsSha256(consent.TenantHash)
            && IsSha256(consent.SubjectHash)
            && IsSha256(consent.InstallationPublicKeyHash)
            && IsSha256(consent.EntitlementLeaseHash)
            && IsSha256(consent.TenantPolicyHash)
            && IsSha256(consent.InvocationBindingHash)
            && IsSha256(consent.ConsumptionHash)
            && IsSha256(consent.ConsentDisclosureHash)
            && IsSha256(consent.ProviderProfileHash)
            && IsSha256(consent.ModelProfileHash)
            && IsSha256(consent.ModelCapabilityHash)
            && IsSha256(consent.DeploymentHash)
            && IsSha256(consent.NonceHash)
            && IsSha256(consent.ConsentEnvelopeHash)
            && proof is not null
            && proof.ProofType == "installation_signature"
            && proof.Algorithm == "ES256"
            && IsBounded(proof.KeyId, 1, 128)
            && string.Equals(
                proof.SignedPayloadHash,
                consent.ConsentEnvelopeHash,
                StringComparison.Ordinal)
            && IsBounded(proof.Signature, 16, 2048)
            && proof.Signature.All(character =>
                char.IsAsciiLetterOrDigit(character) || character is '_' or '-');
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

    /// <summary>
    /// The problem document for a model-plane failure. The code is drawn from
    /// <see cref="SupportProblemCodes"/>, which is a closed vocabulary, so a
    /// provider string can never become a problem type.
    /// </summary>
    public static object SafeModelProblem(string code, int status) =>
        SupportProblemCodes.IsKnown(code)
            ? SafeProblem(code, status)
            : SafeProblem(SupportProblemCodes.InternalError, status);

    private static bool IsBounded(string? value, int minimum, int maximum) =>
        value is not null && value.Length >= minimum && value.Length <= maximum;

    private static string HashBytes(ReadOnlySpan<byte> value) =>
        "sha256:" + Convert.ToHexStringLower(SHA256.HashData(value));
}

/// <summary>
/// The closed RFC 7807 code vocabulary for model-plane failures.
///
/// <para>
/// Every code below is a distinct, actionable statement about what went wrong.
/// The point of separating them is truthfulness: an operator told
/// <c>model_rate_limited</c> waits, one told <c>model_quota_exhausted</c> buys
/// capacity, one told <c>foundry_authentication_failed</c> fixes a role
/// assignment, and one told <c>model_deployment_missing</c> fixes the catalog.
/// Collapsing them into a single "provider unavailable" would send all four to
/// the wrong place.
/// </para>
/// <para>
/// No code carries provider wording, and no problem document ever carries a
/// provider body, an endpoint, a deployment name, or a credential.
/// </para>
/// </summary>
public static class SupportProblemCodes
{
    public const string FoundryAuthenticationFailed = "foundry_authentication_failed";
    public const string FoundryProjectUnavailable = "foundry_project_unavailable";
    public const string ModelRateLimited = "model_rate_limited";
    public const string ModelQuotaExhausted = "model_quota_exhausted";
    public const string ModelContentFiltered = "model_content_filtered";
    public const string ModelRefused = "model_refused";
    public const string ModelStreamInvalid = "model_stream_invalid";
    public const string ModelCallUncertain = "model_call_uncertain";
    public const string ModelDeploymentMissing = "model_deployment_missing";
    public const string ModelCapabilityMismatch = "model_capability_mismatch";
    public const string ModelContextLimitExceeded = "model_context_limit_exceeded";

    // Pre-existing codes the model plane also emits. They stay in the same
    // vocabulary so one predicate answers "is this a reviewed code".
    public const string RequestInvalid = "request_invalid";
    public const string InternalError = "internal_error";
    public const string CatalogStale = "catalog_stale";
    public const string ProfileUnavailable = "profile_unavailable";
    public const string ProfileNotEntitled = "profile_not_entitled";
    public const string ConnectedOperationTimeout = "connected_operation_timeout";
    public const string ModelOutputSchemaViolation = "model_output_schema_violation";

    public static readonly string[] All =
    [
        FoundryAuthenticationFailed,
        FoundryProjectUnavailable,
        ModelRateLimited,
        ModelQuotaExhausted,
        ModelContentFiltered,
        ModelRefused,
        ModelStreamInvalid,
        ModelCallUncertain,
        ModelDeploymentMissing,
        ModelCapabilityMismatch,
        ModelContextLimitExceeded,
        RequestInvalid,
        InternalError,
        CatalogStale,
        ProfileUnavailable,
        ProfileNotEntitled,
        ConnectedOperationTimeout,
        ModelOutputSchemaViolation,
    ];

    public static bool IsKnown(string? code) =>
        code is not null && All.Contains(code, StringComparer.Ordinal);

    /// <summary>
    /// Maps one provider outcome onto a reviewed code and an HTTP status. The
    /// mapping is total: an outcome the vocabulary does not name becomes
    /// <see cref="InternalError"/> rather than leaking through.
    /// </summary>
    public static (string Code, int Status) ForProviderOutcome(string outcome) =>
        outcome switch
        {
            "authentication_failed" =>
                (FoundryAuthenticationFailed, StatusCodes.Status502BadGateway),
            "provider_unavailable" =>
                (FoundryProjectUnavailable, StatusCodes.Status503ServiceUnavailable),
            "rate_limited" => (ModelRateLimited, StatusCodes.Status429TooManyRequests),
            "quota_exhausted" =>
                (ModelQuotaExhausted, StatusCodes.Status503ServiceUnavailable),
            "content_filtered" => (ModelContentFiltered, StatusCodes.Status422UnprocessableEntity),
            "provider_refusal" => (ModelRefused, StatusCodes.Status422UnprocessableEntity),
            "stream_invalid" => (ModelStreamInvalid, StatusCodes.Status502BadGateway),
            "deployment_missing" =>
                (ModelDeploymentMissing, StatusCodes.Status503ServiceUnavailable),
            "capability_unsupported" =>
                (ModelCapabilityMismatch, StatusCodes.Status400BadRequest),
            "context_limit" =>
                (ModelContextLimitExceeded, StatusCodes.Status413PayloadTooLarge),
            "context_rejected" => (RequestInvalid, StatusCodes.Status400BadRequest),
            "profile_mismatch" =>
                (ModelCapabilityMismatch, StatusCodes.Status400BadRequest),
            "timeout" =>
                (ConnectedOperationTimeout, StatusCodes.Status504GatewayTimeout),
            "schema_invalid" or "malformed_output" or "incomplete_output" =>
                (ModelOutputSchemaViolation, StatusCodes.Status502BadGateway),
            _ => (InternalError, StatusCodes.Status502BadGateway),
        };
}

internal static class ModelResultGuards
{
    private const int MaximumPayloadBytes = 2 * 1024 * 1024;
    private static readonly UTF8Encoding StrictUtf8 = new(false, true);

    public static void ValidateOrThrow(
        RegisteredDevice device,
        ModelAccessRequest request,
        ModelAccessResult result,
        string expectedRegion)
    {
        ArgumentNullException.ThrowIfNull(device);
        ArgumentNullException.ThrowIfNull(request);
        ArgumentNullException.ThrowIfNull(result);
        if (!RequestGuards.ValidateModelRequest(request, out _, out _)
            || !string.Equals(
                request.RegistrationId,
                device.RegistrationId,
                StringComparison.Ordinal)
            || result.SchemaVersion != "desktop-model-access-result.v1"
            || !string.Equals(result.RequestId, request.RequestId, StringComparison.Ordinal)
            || !string.Equals(
                result.OutputSchemaId,
                request.CanonicalOutputSchemaId,
                StringComparison.Ordinal)
            || !TryValidatePayload(result.PayloadJson, out int payloadBytes)
            || !RequestGuards.IsSha256(result.PayloadHash)
            || !string.Equals(
                result.PayloadHash,
                HashBytes(StrictUtf8.GetBytes(result.PayloadJson)),
                StringComparison.Ordinal)
            || result.Receipt is null
            || !ValidateReceipt(
                request,
                result,
                result.Receipt,
                expectedRegion,
                payloadBytes))
        {
            throw new InvalidOperationException(
                "The model result was not bound to the authorized request.");
        }
    }

    private static bool ValidateReceipt(
        ModelAccessRequest request,
        ModelAccessResult result,
        ModelAccessReceipt receipt,
        string expectedRegion,
        int payloadBytes) =>
        receipt.SchemaVersion == "sapphirus.model-access-receipt.v1"
        && IsReceiptId(receipt.ReceiptId)
        && string.Equals(receipt.RequestId, request.RequestId, StringComparison.Ordinal)
        && string.Equals(
            receipt.RequestHash,
            RequestGuards.Fingerprint(request),
            StringComparison.Ordinal)
        && string.Equals(receipt.ResultHash, result.PayloadHash, StringComparison.Ordinal)
        && receipt.DeliveryModel == "windows_local"
        && string.Equals(receipt.TenantHash, request.Consent.TenantHash, StringComparison.Ordinal)
        && string.Equals(receipt.SubjectHash, request.Consent.SubjectHash, StringComparison.Ordinal)
        && string.Equals(receipt.RegistrationId, request.RegistrationId, StringComparison.Ordinal)
        && string.Equals(
            receipt.ManifestHash,
            request.LocalEgressManifestHash,
            StringComparison.Ordinal)
        && string.Equals(
            receipt.InvocationBindingHash,
            request.Consent.InvocationBindingHash,
            StringComparison.Ordinal)
        && string.Equals(
            receipt.ConsumptionHash,
            request.Consent.ConsumptionHash,
            StringComparison.Ordinal)
        && string.Equals(
            receipt.ConsentEnvelopeHash,
            request.Consent.ConsentEnvelopeHash,
            StringComparison.Ordinal)
        && string.Equals(
            receipt.ConsentDisclosureHash,
            request.Consent.ConsentDisclosureHash,
            StringComparison.Ordinal)
        && string.Equals(
            receipt.ProviderProfileHash,
            request.Consent.ProviderProfileHash,
            StringComparison.Ordinal)
        && string.Equals(
            receipt.ModelProfileHash,
            request.Consent.ModelProfileHash,
            StringComparison.Ordinal)
        && string.Equals(
            receipt.ModelCapabilityHash,
            request.Consent.ModelCapabilityHash,
            StringComparison.Ordinal)
        && string.Equals(
            receipt.DeploymentHash,
            request.Consent.DeploymentHash,
            StringComparison.Ordinal)
        && string.Equals(
            receipt.CanonicalOutputSchemaId,
            request.CanonicalOutputSchemaId,
            StringComparison.Ordinal)
        && string.Equals(
            receipt.CanonicalOutputSchemaHash,
            request.CanonicalOutputSchemaHash,
            StringComparison.Ordinal)
        && RequestGuards.IsSha256(receipt.ProviderSchemaProjectionHash)
        && RequestGuards.IsSha256(receipt.CredentialBindingHash)
        && string.Equals(receipt.RetentionMode, request.RetentionMode, StringComparison.Ordinal)
        && expectedRegion is { Length: >= 1 and <= 128 }
        && string.Equals(receipt.Region, expectedRegion, StringComparison.Ordinal)
        && receipt.InputBytes == request.Items.Sum(item => item.ByteCount)
        && receipt.OutputBytes == payloadBytes
        && receipt.Usage is not null
        && receipt.Usage.InputTokens >= 0
        && receipt.Usage.OutputTokens >= 0
        && receipt.Usage.CostMicrounits >= 0
        && receipt.Usage.Currency is { Length: 3 }
        && receipt.Usage.Currency.All(character => character is >= 'A' and <= 'Z')
        && receipt.RetryCount is >= 0 and <= 32
        && receipt.FallbackEvents is { Length: <= 16 }
        && receipt.FallbackEvents.All(eventItem =>
            eventItem is not null
            && eventItem.Sequence is >= 1 and <= 16
            && RequestGuards.IsSha256(eventItem.FromProfileHash)
            && RequestGuards.IsSha256(eventItem.ToProfileHash)
            && RequestGuards.IsSha256(eventItem.PolicyTransitionHash))
        && (receipt.ProviderRequestId is null
            || receipt.ProviderRequestId is { Length: >= 1 and <= 2048 })
        && receipt.StartedAt != default
        && receipt.CompletedAt != default
        && receipt.StartedAt <= receipt.CompletedAt
        && receipt.TerminalStatus == "succeeded"
        && RequestGuards.IsSha256(receipt.ReceiptHash)
        && receipt.Proof is not null
        && receipt.Proof.ProofType == "support_plane_signature"
        && receipt.Proof.Algorithm == "ES256"
        && receipt.Proof.Issuer is { Length: >= 8 and <= 2048 }
        && receipt.Proof.Audience is { Length: >= 1 and <= 256 }
        && receipt.Proof.KeyId is { Length: >= 1 and <= 128 }
        && string.Equals(
            receipt.Proof.SignedPayloadHash,
            receipt.ReceiptHash,
            StringComparison.Ordinal)
        && receipt.Proof.Signature is { Length: >= 16 and <= 2048 }
        && receipt.Proof.Signature.All(character =>
            char.IsAsciiLetterOrDigit(character) || character is '_' or '-');

    /// <summary>
    /// Validates a version two receipt against the request that authorized it
    /// and the binding the server actually resolved.
    ///
    /// <para>
    /// The four profile hashes are checked against
    /// <paramref name="binding"/> rather than against the consent envelope, so
    /// a receipt cannot assert a profile, capability set, catalog epoch, or
    /// deployment the resolver never chose. That is the whole point of making
    /// them server-authoritative: a check against the client's own claim would
    /// prove only that the client was self-consistent.
    /// </para>
    /// <para>
    /// A version two field added without a clause here would be unvalidated,
    /// so this method, the record, the digest draft, and the cross-language
    /// fixture move together.
    /// </para>
    /// </summary>
    internal static bool ValidateReceiptV2(
        ModelReceiptAuthorization authorization,
        ModelAccessReceiptV2 receipt,
        ResolvedReceiptBinding binding,
        string expectedRegion,
        string payloadHash,
        long payloadBytes)
    {
        ArgumentNullException.ThrowIfNull(authorization);
        ArgumentNullException.ThrowIfNull(binding);
        return receipt is not null
            && receipt.SchemaVersion == ModelAccessReceiptV2.SchemaVersionValue
            && IsReceiptId(receipt.ReceiptId)
            && ContractIds.Is(receipt.RunId, "run")
            && string.Equals(
                receipt.RequestId,
                authorization.RequestId,
                StringComparison.Ordinal)
            && string.Equals(
                receipt.RequestHash,
                authorization.RequestHash,
                StringComparison.Ordinal)
            && string.Equals(receipt.ResultHash, payloadHash, StringComparison.Ordinal)
            && receipt.DeliveryModel == "windows_local"
            && string.Equals(
                receipt.TenantHash,
                authorization.TenantHash,
                StringComparison.Ordinal)
            && string.Equals(
                receipt.SubjectHash,
                authorization.SubjectHash,
                StringComparison.Ordinal)
            && string.Equals(
                receipt.RegistrationId,
                authorization.RegistrationId,
                StringComparison.Ordinal)
            && string.Equals(
                receipt.ManifestHash,
                authorization.ManifestHash,
                StringComparison.Ordinal)
            && string.Equals(
                receipt.InvocationBindingHash,
                authorization.InvocationBindingHash,
                StringComparison.Ordinal)
            && string.Equals(
                receipt.ConsumptionHash,
                authorization.ConsumptionHash,
                StringComparison.Ordinal)
            && string.Equals(
                receipt.ConsentEnvelopeHash,
                authorization.ConsentEnvelopeHash,
                StringComparison.Ordinal)
            && string.Equals(
                receipt.ConsentDisclosureHash,
                authorization.ConsentDisclosureHash,
                StringComparison.Ordinal)
            && string.Equals(
                receipt.ProviderProfileHash,
                binding.ProviderProfileHash,
                StringComparison.Ordinal)
            && string.Equals(
                receipt.ModelProfileHash,
                binding.ModelProfileHash,
                StringComparison.Ordinal)
            && string.Equals(
                receipt.ModelCapabilityHash,
                binding.ModelCapabilityHash,
                StringComparison.Ordinal)
            && ValidateResponseLane(receipt.ResponseLane)
            && receipt.RequestedProfileId is { Length: >= 1 and <= 64 }
            && string.Equals(
                receipt.ResolvedProfileId,
                binding.ResolvedProfileId,
                StringComparison.Ordinal)
            && string.Equals(
                receipt.ResolvedProfileHash,
                binding.ResolvedProfileHash,
                StringComparison.Ordinal)
            && (receipt.RequestedVariantId is null
                || receipt.RequestedVariantId is { Length: >= 1 and <= 64 })
            && string.Equals(
                receipt.ResolvedVariantId,
                binding.ResolvedVariantId,
                StringComparison.Ordinal)
            && string.Equals(
                receipt.CatalogVersion,
                binding.CatalogVersion,
                StringComparison.Ordinal)
            && string.Equals(receipt.CatalogHash, binding.CatalogHash, StringComparison.Ordinal)
            && string.Equals(
                receipt.DeploymentInventoryHash,
                binding.DeploymentInventoryHash,
                StringComparison.Ordinal)
            && string.Equals(
                receipt.ResolvedDeploymentHash,
                binding.ResolvedDeploymentHash,
                StringComparison.Ordinal)
            && receipt.ResolvedModel == binding.ResolvedModel
            && (receipt.ReportedResponseModel is null
                || receipt.ReportedResponseModel is { Length: >= 1 and <= 128 })
            && string.Equals(
                receipt.CredentialBindingHash,
                binding.CredentialBindingHash,
                StringComparison.Ordinal)
            && RequestGuards.IsSha256(receipt.CredentialBindingHash)
            && string.Equals(
                receipt.RetentionMode,
                authorization.RetentionMode,
                StringComparison.Ordinal)
            && expectedRegion is { Length: >= 1 and <= 128 }
            && string.Equals(receipt.Region, expectedRegion, StringComparison.Ordinal)
            && receipt.InputBytes == authorization.InputBytes
            && receipt.OutputBytes == payloadBytes
            && ValidateUsage(receipt.Usage)
            && receipt.RetryCount is >= 0 and <= 32
            && receipt.FallbackEvents is { Length: <= 16 }
            && receipt.FallbackEvents.All(static eventItem =>
                eventItem is not null
                && eventItem.Sequence is >= 1 and <= 16
                && eventItem.FromProfileId is { Length: >= 1 and <= 64 }
                && eventItem.ToProfileId is { Length: >= 1 and <= 64 }
                && RequestGuards.IsSha256(eventItem.FromProfileHash)
                && RequestGuards.IsSha256(eventItem.ToProfileHash)
                && RequestGuards.IsSha256(eventItem.PolicyTransitionHash))
            && (receipt.ProviderRequestId is null
                || receipt.ProviderRequestId is { Length: >= 1 and <= 2048 })
            && receipt.StartedAt != default
            && receipt.CompletedAt != default
            && receipt.StartedAt <= receipt.CompletedAt
            // Only a succeeded run may carry a signature. Failed, refused, and
            // cancelled runs produce evidence, never a success receipt.
            && receipt.TerminalStatus == ModelTerminalStatuses.Succeeded
            && RequestGuards.IsSha256(receipt.ReceiptHash)
            && receipt.Proof is not null
            && receipt.Proof.ProofType == "support_plane_signature"
            && receipt.Proof.Algorithm == "ES256"
            && receipt.Proof.Issuer is { Length: >= 8 and <= 2048 }
            && receipt.Proof.Audience is { Length: >= 1 and <= 256 }
            && receipt.Proof.KeyId is { Length: >= 1 and <= 128 }
            && string.Equals(
                receipt.Proof.SignedPayloadHash,
                receipt.ReceiptHash,
                StringComparison.Ordinal)
            && receipt.Proof.Signature is { Length: >= 16 and <= 2048 }
            && receipt.Proof.Signature.All(static character =>
                char.IsAsciiLetterOrDigit(character) || character is '_' or '-');
    }

    /// <summary>
    /// The two lanes are closed variants. A structured lane must name its
    /// canonical schema and provider projection; a streaming lane must report
    /// a positive event count and final sequence, and cannot name a schema at
    /// all.
    /// </summary>
    private static bool ValidateResponseLane(IModelResponseLane? lane) => lane switch
    {
        StructuredResponseLane structured =>
            structured.CanonicalOutputSchemaId is { Length: >= 5 and <= 128 }
            && RequestGuards.IsSha256(structured.CanonicalOutputSchemaHash)
            && RequestGuards.IsSha256(structured.ProviderSchemaProjectionHash),
        StreamingChatLane streaming =>
            streaming.EventCount is >= 1 and <= 1_000_000
            && streaming.FinalSequence is >= 1 and <= 1_000_000
            && streaming.FinalSequence <= streaming.EventCount,
        _ => false,
    };

    /// <summary>
    /// An unavailable usage record must be all-null by construction, so it can
    /// never be read as a free call; a reported one must have no negative
    /// category.
    /// </summary>
    private static bool ValidateUsage(ModelUsageV2? usage)
    {
        if (usage is null
            || !ModelUsageProvenance.IsKnown(usage.UsageProvenance)
            || usage.Currency is not { Length: 3 }
            || !usage.Currency.All(static character => character is >= 'A' and <= 'Z')
            || usage.PriceProfileVersion is not { Length: >= 3 and <= 64 })
        {
            return false;
        }
        if (usage.UsageProvenance == ModelUsageProvenance.Unavailable)
        {
            return usage.IsFullyUnavailable;
        }
        return usage.InputTokens is null or >= 0
            && usage.OutputTokens is null or >= 0
            && usage.ReasoningTokens is null or >= 0
            && usage.CacheReadTokens is null or >= 0
            && usage.CacheWriteTokens is null or >= 0
            && usage.CostMicrounits is null or >= 0;
    }

    private static bool TryValidatePayload(string? payload, out int payloadBytes)
    {
        payloadBytes = 0;
        if (payload is null)
        {
            return false;
        }
        try
        {
            payloadBytes = StrictUtf8.GetByteCount(payload);
            if (payloadBytes is < 1 or > MaximumPayloadBytes)
            {
                return false;
            }
            using JsonDocument document = JsonDocument.Parse(
                payload,
                new JsonDocumentOptions
                {
                    AllowTrailingCommas = false,
                    CommentHandling = JsonCommentHandling.Disallow,
                    MaxDepth = 64,
                });
            return document.RootElement.ValueKind == JsonValueKind.Object
                && HasUniqueObjectMembers(document.RootElement);
        }
        catch (JsonException)
        {
            return false;
        }
        catch (EncoderFallbackException)
        {
            return false;
        }
    }

    private static bool HasUniqueObjectMembers(JsonElement value)
    {
        if (value.ValueKind == JsonValueKind.Object)
        {
            HashSet<string> names = new(StringComparer.Ordinal);
            foreach (JsonProperty property in value.EnumerateObject())
            {
                if (!names.Add(property.Name) || !HasUniqueObjectMembers(property.Value))
                {
                    return false;
                }
            }
        }
        else if (value.ValueKind == JsonValueKind.Array)
        {
            foreach (JsonElement item in value.EnumerateArray())
            {
                if (!HasUniqueObjectMembers(item))
                {
                    return false;
                }
            }
        }
        return true;
    }

    private static bool IsReceiptId(string? value) => ContractIds.Is(value, "receipt");

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
            BadHttpRequestException =>
                (StatusCodes.Status400BadRequest, "request_invalid"),
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
