using System.Diagnostics.CodeAnalysis;
using System.Text.Json;

namespace Sapphirus.DesktopSupportApi.Model;

/// <summary>
/// Why a version one model access request was refused instead of adapted.
/// Mirrors <c>LegacyRejectionReason</c> in
/// <c>packages/contracts/schemas/model-access-compatibility.schema.json</c>
/// and <c>LegacyRejectionReason</c> in
/// <c>crates/desktop-cloud/src/model_access_compatibility.rs</c>.
/// </summary>
public enum LegacyRejectionReason
{
    /// <summary>The request named a model, profile, or deployment.</summary>
    ModelSelectionAttempted,

    /// <summary>The request asked for streaming.</summary>
    StreamingRequested,

    /// <summary>The request named a variant.</summary>
    VariantRequested,

    /// <summary>The request carried an attachment beyond its own contract.</summary>
    AttachmentRequested,

    /// <summary>The request supplied a catalog epoch it could not have read.</summary>
    CatalogVersionSupplied,

    /// <summary>The request declared tools.</summary>
    ToolDefinitionSupplied,

    /// <summary>The request carried a member outside the version one contract.</summary>
    UnknownMemberPresent,

    /// <summary>The sending installation is below the minimum contract epoch.</summary>
    ContractEpochTooLow,
}

/// <summary>
/// A version one request lifted into the shape version two resolution needs.
/// The catalog epoch and the re-bound consent are deliberately absent: a
/// legacy desktop never read a catalog, and its consent was signed over the
/// version one envelope, so both are stamped by the support plane after the
/// adapter has proved the request is adaptable at all.
/// </summary>
public sealed record AdaptedLegacyRequest(
    string RequestId,
    string RegistrationId,
    string Purpose,
    string ModelRole,
    string ModelProfileId,
    string RequestMode,
    string? RequestedVariantId,
    string? CatalogVersion,
    string CanonicalOutputSchemaId,
    string CanonicalOutputSchemaHash,
    string LocalEgressManifestHash,
    string BudgetClass);

/// <summary>
/// One-way version one to version two model access compatibility (FND-013).
/// <para>
/// A desktop that still speaks <c>desktop-model-access-request.v1</c> keeps
/// working after version two lands. Its request is lifted onto a single named
/// legacy profile whose deployment stays resolved server side, exactly as it
/// was before profiles existed.
/// </para>
/// <para>
/// The adapter is one way by construction. There is no method that lowers a
/// version two request to version one, because that would have to drop the
/// profile, catalog epoch, variant, and request mode that version two exists
/// to bind, and a dropped binding is how a user ends up talking to a different
/// model than the one the screen says.
/// </para>
/// <para>
/// A version one request that reaches for a version two capability is refused
/// with a named reason rather than partially honoured.
/// </para>
/// </summary>
public static class ModelAccessCompatibility
{
    /// <summary>Wire discriminator of the legacy request document.</summary>
    public const string LegacyRequestSchemaVersion = "desktop-model-access-request.v1";

    /// <summary>Wire discriminator of the current request document.</summary>
    public const string CurrentRequestSchemaVersion = "desktop-model-access-request.v2";

    /// <summary>Wire discriminator of the legacy receipt document.</summary>
    public const string LegacyReceiptSchemaVersion = "sapphirus.model-access-receipt.v1";

    /// <summary>Wire discriminator of the current receipt document.</summary>
    public const string CurrentReceiptSchemaVersion = "sapphirus.model-access-receipt.v2";

    /// <summary>
    /// The single profile identity every adapted legacy request lands on. Its
    /// deployment is resolved server side and never projected to the desktop,
    /// and it is not selectable from the catalog, so no user can pick it.
    /// </summary>
    public const string LegacyProfileId = "legacy-fixed-deployment";

    /// <summary>
    /// The request mode a legacy request is always adapted to. Version one had
    /// no mode; it was structured output or nothing.
    /// </summary>
    public const string LegacyRequestMode = "structured_response";

    /// <summary>
    /// The lowest desktop contract epoch still permitted to send version one.
    /// An installation below this is refused and told to update, because its
    /// receipt handling predates the fields the adapter emits.
    /// </summary>
    public const int MinimumDesktopContractEpoch = 1;

    /// <summary>
    /// The contract epoch at which version one stops being accepted. Reaching
    /// it is necessary but not sufficient: the removal gate in the canonical
    /// policy also requires the catalog to be published for every tenant, the
    /// installed base to be at the removal epoch, the legacy profile to be
    /// retired from the catalog, and a historical receipt replay check.
    /// </summary>
    public const int RemovalContractEpoch = 2;

    /// <summary>Members a legacy request document is allowed to carry.</summary>
    private static readonly HashSet<string> LegacyMembers = new(StringComparer.Ordinal)
    {
        "schemaVersion",
        "requestId",
        "deliveryModel",
        "registrationId",
        "purpose",
        "modelRole",
        "canonicalOutputSchemaId",
        "canonicalOutputSchemaHash",
        "localEgressManifestHash",
        "consent",
        "items",
        "retentionMode",
        "budgetClass",
    };

    /// <summary>
    /// Members that name a specific refusal, checked before the catch-all so a
    /// refusal always says which version two capability was reached for.
    /// </summary>
    private static readonly (string Member, LegacyRejectionReason Reason)[] NamedRefusals =
    [
        ("modelProfileId", LegacyRejectionReason.ModelSelectionAttempted),
        ("profileId", LegacyRejectionReason.ModelSelectionAttempted),
        ("deploymentName", LegacyRejectionReason.ModelSelectionAttempted),
        ("modelProfileHash", LegacyRejectionReason.ModelSelectionAttempted),
        ("requestedVariantId", LegacyRejectionReason.VariantRequested),
        ("variantId", LegacyRejectionReason.VariantRequested),
        ("catalogVersion", LegacyRejectionReason.CatalogVersionSupplied),
        ("tools", LegacyRejectionReason.ToolDefinitionSupplied),
        ("attachments", LegacyRejectionReason.AttachmentRequested),
        ("attachmentRefs", LegacyRejectionReason.AttachmentRequested),
        ("stream", LegacyRejectionReason.StreamingRequested),
    ];

    /// <summary>
    /// The stable snake case wire spelling of a refusal reason. It is the same
    /// spelling the canonical schema and the Rust adapter use.
    /// </summary>
    public static string ToWireString(LegacyRejectionReason reason) => reason switch
    {
        LegacyRejectionReason.ModelSelectionAttempted => "model_selection_attempted",
        LegacyRejectionReason.StreamingRequested => "streaming_requested",
        LegacyRejectionReason.VariantRequested => "variant_requested",
        LegacyRejectionReason.AttachmentRequested => "attachment_requested",
        LegacyRejectionReason.CatalogVersionSupplied => "catalog_version_supplied",
        LegacyRejectionReason.ToolDefinitionSupplied => "tool_definition_supplied",
        LegacyRejectionReason.UnknownMemberPresent => "unknown_member_present",
        LegacyRejectionReason.ContractEpochTooLow => "contract_epoch_too_low",
        _ => "unknown_member_present",
    };

    /// <summary>
    /// Reports whether a receipt of this schema version is still verifiable.
    /// Version two adds a second receipt family; it does not restate or
    /// re-sign the first, so a receipt signed before version two landed keeps
    /// verifying under the digest rule it was signed with.
    /// </summary>
    public static bool IsHistoricalReceiptVerifiable(string? receiptSchemaVersion) =>
        receiptSchemaVersion is LegacyReceiptSchemaVersion or CurrentReceiptSchemaVersion;

    /// <summary>
    /// Reports whether a request document version is still accepted. Dropping
    /// <see cref="LegacyRequestSchemaVersion"/> from this answer is the
    /// observable act that closes the legacy lane.
    /// </summary>
    public static bool IsRequestSchemaVersionAccepted(string? requestSchemaVersion) =>
        requestSchemaVersion is LegacyRequestSchemaVersion or CurrentRequestSchemaVersion;

    /// <summary>
    /// Lifts a version one request document onto the named legacy profile.
    /// <paramref name="document"/> is the loosely parsed legacy envelope as
    /// received, not a strictly validated one: the point of this adapter is to
    /// refuse the members strict validation would only report as unknown, with
    /// a reason that says which version two capability the caller reached for.
    /// </summary>
    /// <returns>
    /// <see langword="true"/> with <paramref name="adapted"/> set when the
    /// request is adaptable; otherwise <see langword="false"/> with
    /// <paramref name="reason"/> set.
    /// </returns>
    public static bool TryAdaptLegacyRequest(
        JsonElement document,
        int contractEpoch,
        [NotNullWhen(true)] out AdaptedLegacyRequest? adapted,
        out LegacyRejectionReason reason)
    {
        adapted = null;
        if (contractEpoch < MinimumDesktopContractEpoch)
        {
            reason = LegacyRejectionReason.ContractEpochTooLow;
            return false;
        }

        if (document.ValueKind is not JsonValueKind.Object
            || ReadString(document, "schemaVersion") != LegacyRequestSchemaVersion)
        {
            reason = LegacyRejectionReason.UnknownMemberPresent;
            return false;
        }

        if (TryClassifyRefusal(document, out reason))
        {
            return false;
        }

        string? requestId = ReadString(document, "requestId");
        string? registrationId = ReadString(document, "registrationId");
        string? purpose = ReadString(document, "purpose");
        string? modelRole = ReadString(document, "modelRole");
        string? outputSchemaId = ReadString(document, "canonicalOutputSchemaId");
        string? outputSchemaHash = ReadString(document, "canonicalOutputSchemaHash");
        string? manifestHash = ReadString(document, "localEgressManifestHash");
        string? budgetClass = ReadString(document, "budgetClass");
        if (requestId is null
            || registrationId is null
            || purpose is null
            || modelRole is null
            || outputSchemaId is null
            || outputSchemaHash is null
            || manifestHash is null
            || budgetClass is null)
        {
            reason = LegacyRejectionReason.UnknownMemberPresent;
            return false;
        }

        adapted = new AdaptedLegacyRequest(
            requestId,
            registrationId,
            purpose,
            modelRole,
            LegacyProfileId,
            LegacyRequestMode,
            RequestedVariantId: null,
            CatalogVersion: null,
            outputSchemaId,
            outputSchemaHash,
            manifestHash,
            budgetClass);
        reason = default;
        return true;
    }

    private static bool TryClassifyRefusal(JsonElement document, out LegacyRejectionReason reason)
    {
        if (document.TryGetProperty("requestMode", out JsonElement mode))
        {
            // A legacy document has no request mode at all. Naming the one
            // mode it could have had is still outside its contract, but it is
            // not an attempt to stream, so it is reported as what it is.
            reason = mode.ValueKind is JsonValueKind.String
                && mode.ValueEquals(LegacyRequestMode)
                    ? LegacyRejectionReason.UnknownMemberPresent
                    : LegacyRejectionReason.StreamingRequested;
            return true;
        }

        foreach ((string member, LegacyRejectionReason named) in NamedRefusals)
        {
            if (document.TryGetProperty(member, out _))
            {
                reason = named;
                return true;
            }
        }

        foreach (JsonProperty property in document.EnumerateObject())
        {
            if (!LegacyMembers.Contains(property.Name))
            {
                reason = LegacyRejectionReason.UnknownMemberPresent;
                return true;
            }
        }

        reason = default;
        return false;
    }

    private static string? ReadString(JsonElement document, string member) =>
        document.TryGetProperty(member, out JsonElement value)
            && value.ValueKind is JsonValueKind.String
                ? value.GetString()
                : null;
}
