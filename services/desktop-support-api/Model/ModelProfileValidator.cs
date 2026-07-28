using System.Buffers;
using System.Globalization;
using System.Security.Cryptography;
using System.Text;
using System.Text.Encodings.Web;
using System.Text.Json;
using System.Text.RegularExpressions;

namespace Sapphirus.DesktopSupportApi.Model;

/// <summary>The registry-version record that makes a published profile set visible.</summary>
public sealed record ModelProfileRegistryRecord(
    long RegistryVersion,
    string RegistryHash,
    string CapabilityHash,
    IReadOnlyList<string> ProfileIds);

/// <summary>
/// Strict reader and validator for signed organization model profiles.
///
/// <para>
/// Parsing is deliberately hand-walked rather than bound by
/// <c>JsonSerializer</c>: the canonical contract is closed, so a member this
/// service does not know about is a signal that the store and the code have
/// diverged, and the only safe response is to refuse the whole registry. Member
/// names are compared with <see cref="StringComparison.Ordinal"/>, a repeated
/// member is an error rather than a last-one-wins overwrite, and every array is
/// bounded before it is read.
/// </para>
/// <para>
/// The three hashes reproduce
/// <c>packages/contracts/scripts/lib/canonical-json.mjs</c> exactly, which is
/// what lets this reader recompute the publisher's registry hash from the
/// stored values and refuse a set that was edited underneath it. The publisher
/// is <c>tools/foundry-profiles/publish-profiles.ps1</c>; the canonical
/// serializer is <c>tools/foundry-profiles/validate-profiles.mjs</c>.
/// </para>
/// </summary>
public static partial class ModelProfileValidator
{
    /// <summary>Hash purpose for one canonical profile value.</summary>
    public const string ProfileHashPurpose = "model-profile";

    /// <summary>Hash purpose for the ordered registry manifest.</summary>
    public const string RegistryHashPurpose = "model-profile-registry";

    /// <summary>Hash purpose for the capability surface of the whole registry.</summary>
    public const string CapabilityHashPurpose = "model-profile-capability";

    /// <summary>Largest stored profile value the reader will accept, in bytes.</summary>
    public const int MaximumProfileValueBytes = 10_000;

    /// <summary>Largest stored registry-version record, in bytes.</summary>
    public const int MaximumRegistryRecordBytes = 65_536;

    private const int MaximumJsonDepth = 16;
    private const int MaximumAllowlistItems = 32;
    private const int MaximumRequestModes = 2;
    private const int MaximumModalities = 3;
    private const int MaximumRegions = 16;
    private const int MaximumVariants = 8;

    private static readonly string[] ProfileMembers =
    [
        "schemaVersion",
        "profileId",
        "displayName",
        "description",
        "family",
        "priority",
        "enabled",
        "visible",
        "deploymentName",
        "expectedModel",
        "purposeAllowlist",
        "modelRoleAllowlist",
        "requestModeAllowlist",
        "canonicalOutputSchemaAllowlist",
        "supportedModalities",
        "requiredCapabilities",
        "limits",
        "variants",
        "retryPolicy",
        "retentionMode",
        "allowedRegions",
        "qualification",
        "priceProfile",
    ];

    private static readonly string[] CapabilityMembers =
    [
        "textInput",
        "imageInput",
        "pdfInput",
        "attachments",
        "toolCalling",
        "structuredOutput",
        "reasoning",
        "streaming",
    ];

    private static readonly string[] RequestModes =
    [
        "structured_response",
        "streaming_chat",
    ];

    private static readonly string[] Modalities = ["text", "image", "pdf"];

    private static readonly string[] ReasoningEfforts = ["none", "low", "medium", "high"];

    private static readonly string[] Verbosities = ["low", "medium", "high"];

    // ---------------------------------------------------------------- parsing

    /// <summary>
    /// Parses one stored profile value. <paramref name="rawValue"/> must be the
    /// publisher's canonical JSON byte for byte; the caller hashes the same
    /// bytes, so any drift shows up as a registry hash mismatch rather than as
    /// a silently accepted profile.
    /// </summary>
    public static bool TryParseProfile(
        string? rawValue,
        out ModelProfile? profile,
        out string errorCode)
    {
        profile = null;
        errorCode = "model_profile_invalid";
        if (rawValue is null
            || rawValue.Length == 0
            || Encoding.UTF8.GetByteCount(rawValue) > MaximumProfileValueBytes)
        {
            errorCode = "model_profile_value_out_of_bounds";
            return false;
        }

        JsonDocument document;
        try
        {
            document = JsonDocument.Parse(
                rawValue,
                new JsonDocumentOptions
                {
                    AllowTrailingCommas = false,
                    CommentHandling = JsonCommentHandling.Disallow,
                    MaxDepth = MaximumJsonDepth,
                });
        }
        catch (JsonException)
        {
            errorCode = "model_profile_not_json";
            return false;
        }

        using (document)
        {
            if (!TryReadObject(
                document.RootElement,
                ProfileMembers,
                out Dictionary<string, JsonElement>? members,
                out errorCode))
            {
                return false;
            }
            return TryProject(members!, out profile, out errorCode);
        }
    }

    private static bool TryProject(
        Dictionary<string, JsonElement> members,
        out ModelProfile? profile,
        out string errorCode)
    {
        profile = null;
        errorCode = "model_profile_invalid";

        if (!TryReadConst(
                members,
                "schemaVersion",
                ModelProfile.SchemaVersionValue,
                out errorCode)
            || !TryReadPattern(members, "profileId", ProfileId(), 3, 64, out string profileId, out errorCode)
            || !TryReadText(members, "displayName", 1, 64, out string displayName, out errorCode)
            || !TryReadText(members, "description", 1, 256, out string description, out errorCode)
            || !TryReadOptionalText(members, "family", 1, 64, out string? family, out errorCode)
            || !TryReadInt(members, "priority", 1, 1_000, out int priority, out errorCode)
            || !TryReadBool(members, "enabled", out bool enabled, out errorCode)
            || !TryReadBool(members, "visible", out bool visible, out errorCode))
        {
            return false;
        }

        if (!TryReadPattern(
            members,
            "deploymentName",
            DeploymentName(),
            2,
            128,
            out string deploymentName,
            out errorCode))
        {
            errorCode = "model_profile_deployment_name_unsafe";
            return false;
        }
        if (!IsSafeDeploymentName(deploymentName))
        {
            errorCode = "model_profile_deployment_name_unsafe";
            return false;
        }

        if (!TryReadExpectedModel(members, out ModelProfileExpectedModel? expectedModel, out errorCode)
            || !TryReadTokenList(
                members,
                "purposeAllowlist",
                AllowlistToken(),
                1,
                MaximumAllowlistItems,
                out string[] purposes,
                out errorCode)
            || !TryReadTokenList(
                members,
                "modelRoleAllowlist",
                AllowlistToken(),
                1,
                MaximumAllowlistItems,
                out string[] roles,
                out errorCode)
            || !TryReadEnumList(
                members,
                "requestModeAllowlist",
                RequestModes,
                1,
                MaximumRequestModes,
                out string[] requestModes,
                out errorCode)
            || !TryReadTokenList(
                members,
                "canonicalOutputSchemaAllowlist",
                CanonicalSchemaId(),
                0,
                MaximumAllowlistItems,
                out string[] outputSchemas,
                out errorCode)
            || !TryReadEnumList(
                members,
                "supportedModalities",
                Modalities,
                1,
                MaximumModalities,
                out string[] modalities,
                out errorCode)
            || !TryReadCapabilities(members, out ModelProfileCapabilities? capabilities, out errorCode)
            || !TryReadLimits(members, out ModelProfileLimits? limits, out errorCode)
            || !TryReadVariants(members, limits!, out ModelProfileVariant[]? variants, out errorCode)
            || !TryReadRetryPolicy(members, out ModelProfileRetryPolicy? retryPolicy, out errorCode))
        {
            return false;
        }

        if (!TryReadConst(
            members,
            "retentionMode",
            ModelProfile.SupportedRetentionMode,
            out errorCode))
        {
            errorCode = "model_profile_retention_unsupported";
            return false;
        }

        if (!TryReadTokenList(
                members,
                "allowedRegions",
                RegionName(),
                1,
                MaximumRegions,
                out string[] regions,
                out errorCode)
            || !TryReadQualification(members, out ModelProfileQualification? qualification, out errorCode)
            || !TryReadPriceProfile(members, out ModelPriceProfile? priceProfile, out errorCode))
        {
            return false;
        }

        // A modality the profile claims to support but whose matching input
        // capability is not required would let a request carry content the
        // deployment was never checked for.
        if ((modalities.Contains("text", StringComparer.Ordinal) && !capabilities!.TextInput)
            || (modalities.Contains("image", StringComparer.Ordinal) && !capabilities!.ImageInput)
            || (modalities.Contains("pdf", StringComparer.Ordinal) && !capabilities!.PdfInput))
        {
            errorCode = "model_profile_modality_contradicts_capability";
            return false;
        }

        profile = new ModelProfile(
            ModelProfile.SchemaVersionValue,
            profileId,
            displayName,
            description,
            family,
            priority,
            enabled,
            visible,
            deploymentName,
            expectedModel!,
            purposes,
            roles,
            requestModes,
            outputSchemas,
            modalities,
            capabilities!,
            limits!,
            variants!,
            retryPolicy!,
            ModelProfile.SupportedRetentionMode,
            regions,
            qualification!,
            priceProfile!);
        errorCode = "";
        return true;
    }

    private static bool TryReadExpectedModel(
        Dictionary<string, JsonElement> members,
        out ModelProfileExpectedModel? expectedModel,
        out string errorCode)
    {
        expectedModel = null;
        if (!TryReadObject(
            members["expectedModel"],
            ["publisher", "name", "version"],
            out Dictionary<string, JsonElement>? nested,
            out errorCode))
        {
            return false;
        }
        if (!TryReadPattern(nested!, "publisher", ModelIdentityPart(), 1, 128, out string publisher, out errorCode)
            || !TryReadPattern(nested!, "name", ModelIdentityPart(), 1, 128, out string name, out errorCode)
            || !TryReadPattern(nested!, "version", ModelIdentityPart(), 1, 128, out string version, out errorCode))
        {
            errorCode = "model_profile_expected_model_invalid";
            return false;
        }
        expectedModel = new ModelProfileExpectedModel(publisher, name, version);
        return true;
    }

    private static bool TryReadCapabilities(
        Dictionary<string, JsonElement> members,
        out ModelProfileCapabilities? capabilities,
        out string errorCode)
    {
        capabilities = null;
        if (!TryReadObject(
            members["requiredCapabilities"],
            CapabilityMembers,
            out Dictionary<string, JsonElement>? nested,
            out errorCode))
        {
            return false;
        }
        bool[] flags = new bool[CapabilityMembers.Length];
        for (int index = 0; index < CapabilityMembers.Length; index++)
        {
            if (!TryReadBool(nested!, CapabilityMembers[index], out flags[index], out errorCode))
            {
                errorCode = "model_profile_capabilities_invalid";
                return false;
            }
        }
        capabilities = new ModelProfileCapabilities(
            flags[0],
            flags[1],
            flags[2],
            flags[3],
            flags[4],
            flags[5],
            flags[6],
            flags[7]);
        return true;
    }

    private static bool TryReadLimits(
        Dictionary<string, JsonElement> members,
        out ModelProfileLimits? limits,
        out string errorCode)
    {
        limits = null;
        if (!TryReadObject(
            members["limits"],
            [
                "contextTokens",
                "maxInputTokens",
                "maxOutputTokens",
                "maxAttachmentBytes",
                "maxOutputBytes",
            ],
            out Dictionary<string, JsonElement>? nested,
            out errorCode))
        {
            return false;
        }
        // Every token and byte ceiling except the attachment budget must be
        // strictly positive: a zero or negative ceiling is not "unlimited", it
        // is a limit nobody can satisfy.
        if (!TryReadInt(nested!, "contextTokens", 1, 100_000_000, out int contextTokens, out errorCode)
            || !TryReadInt(nested!, "maxInputTokens", 1, 100_000_000, out int maxInputTokens, out errorCode)
            || !TryReadInt(nested!, "maxOutputTokens", 1, 10_000_000, out int maxOutputTokens, out errorCode)
            || !TryReadInt(nested!, "maxAttachmentBytes", 0, 134_217_728, out int maxAttachmentBytes, out errorCode)
            || !TryReadInt(nested!, "maxOutputBytes", 1, 16_777_216, out int maxOutputBytes, out errorCode))
        {
            errorCode = "model_profile_limit_not_positive";
            return false;
        }
        if (maxInputTokens > contextTokens)
        {
            errorCode = "model_profile_input_exceeds_context";
            return false;
        }
        limits = new ModelProfileLimits(
            contextTokens,
            maxInputTokens,
            maxOutputTokens,
            maxAttachmentBytes,
            maxOutputBytes);
        return true;
    }

    private static bool TryReadVariants(
        Dictionary<string, JsonElement> members,
        ModelProfileLimits limits,
        out ModelProfileVariant[]? variants,
        out string errorCode)
    {
        variants = null;
        errorCode = "model_profile_variants_invalid";
        JsonElement element = members["variants"];
        if (element.ValueKind != JsonValueKind.Array)
        {
            return false;
        }
        int count = element.GetArrayLength();
        if (count is < 1 or > MaximumVariants)
        {
            return false;
        }

        List<ModelProfileVariant> parsed = new(count);
        HashSet<string> variantIds = new(StringComparer.Ordinal);
        foreach (JsonElement item in element.EnumerateArray())
        {
            if (!TryReadObject(
                item,
                ["variantId", "displayName", "reasoningEffort", "verbosity", "maxOutputTokens"],
                out Dictionary<string, JsonElement>? nested,
                out errorCode))
            {
                return false;
            }
            if (!TryReadPattern(nested!, "variantId", VariantId(), 2, 32, out string variantId, out errorCode)
                || !TryReadText(nested!, "displayName", 1, 32, out string displayName, out errorCode)
                || !TryReadEnum(nested!, "reasoningEffort", ReasoningEfforts, out string effort, out errorCode)
                || !TryReadEnum(nested!, "verbosity", Verbosities, out string verbosity, out errorCode)
                || !TryReadInt(nested!, "maxOutputTokens", 1, 10_000_000, out int maxOutputTokens, out errorCode))
            {
                errorCode = "model_profile_variants_invalid";
                return false;
            }
            if (!variantIds.Add(variantId))
            {
                errorCode = "model_profile_variant_duplicate";
                return false;
            }
            // A variant that outruns the profile ceiling would silently raise
            // the organization's own output budget.
            if (maxOutputTokens > limits.MaxOutputTokens)
            {
                errorCode = "model_profile_variant_exceeds_output_ceiling";
                return false;
            }
            parsed.Add(new ModelProfileVariant(
                variantId,
                displayName,
                effort,
                verbosity,
                maxOutputTokens));
        }
        variants = [.. parsed];
        errorCode = "";
        return true;
    }

    private static bool TryReadRetryPolicy(
        Dictionary<string, JsonElement> members,
        out ModelProfileRetryPolicy? retryPolicy,
        out string errorCode)
    {
        retryPolicy = null;
        if (!TryReadObject(
            members["retryPolicy"],
            [
                "maximumAttempts",
                "initialBackoffMilliseconds",
                "maximumBackoffMilliseconds",
            ],
            out Dictionary<string, JsonElement>? nested,
            out errorCode))
        {
            return false;
        }
        if (!TryReadInt(nested!, "maximumAttempts", 1, 8, out int attempts, out errorCode)
            || !TryReadInt(nested!, "initialBackoffMilliseconds", 0, 60_000, out int initial, out errorCode)
            || !TryReadInt(nested!, "maximumBackoffMilliseconds", 0, 300_000, out int maximum, out errorCode)
            || initial > maximum)
        {
            errorCode = "model_profile_retry_policy_invalid";
            return false;
        }
        retryPolicy = new ModelProfileRetryPolicy(attempts, initial, maximum);
        return true;
    }

    private static bool TryReadQualification(
        Dictionary<string, JsonElement> members,
        out ModelProfileQualification? qualification,
        out string errorCode)
    {
        qualification = null;
        if (!TryReadObject(
            members["qualification"],
            ["status", "evidenceHash", "qualifiedAt"],
            out Dictionary<string, JsonElement>? nested,
            out errorCode))
        {
            return false;
        }
        if (!TryReadEnum(
                nested!,
                "status",
                ["qualified", "unqualified", "expired"],
                out string status,
                out errorCode)
            || !TryReadText(nested!, "evidenceHash", 71, 71, out string evidenceHash, out errorCode)
            || !RequestGuards.IsSha256(evidenceHash))
        {
            errorCode = "model_profile_qualification_missing";
            return false;
        }
        // A digest made of one repeated nibble is a placeholder constant, not
        // evidence. The publisher refuses to emit one; the reader refuses to
        // trust one that appeared some other way.
        if (IsDegenerateDigest(evidenceHash))
        {
            errorCode = "model_profile_qualification_placeholder";
            return false;
        }
        if (!TryReadOptionalInstant(nested!, "qualifiedAt", out DateTimeOffset? qualifiedAt, out errorCode))
        {
            errorCode = "model_profile_qualification_missing";
            return false;
        }
        ModelQualificationStatus parsedStatus = status switch
        {
            "qualified" => ModelQualificationStatus.Qualified,
            "expired" => ModelQualificationStatus.Expired,
            _ => ModelQualificationStatus.Unqualified,
        };
        // Qualified without an instant is evidence that does not say when, so
        // it can never expire and can never be audited.
        if (parsedStatus == ModelQualificationStatus.Qualified && qualifiedAt is null)
        {
            errorCode = "model_profile_qualification_missing";
            return false;
        }
        qualification = new ModelProfileQualification(parsedStatus, evidenceHash, qualifiedAt);
        return true;
    }

    private static bool TryReadPriceProfile(
        Dictionary<string, JsonElement> members,
        out ModelPriceProfile? priceProfile,
        out string errorCode)
    {
        priceProfile = null;
        if (!TryReadObject(
            members["priceProfile"],
            [
                "version",
                "currency",
                "inputMicrounitsPerThousand",
                "outputMicrounitsPerThousand",
                "reasoningMicrounitsPerThousand",
                "cacheReadMicrounitsPerThousand",
            ],
            out Dictionary<string, JsonElement>? nested,
            out errorCode))
        {
            return false;
        }
        if (!TryReadPattern(nested!, "version", PriceProfileVersion(), 3, 64, out string version, out errorCode)
            || !TryReadPattern(nested!, "currency", CurrencyCode(), 3, 3, out string currency, out errorCode)
            || !TryReadInt(nested!, "inputMicrounitsPerThousand", 0, 1_000_000_000, out int input, out errorCode)
            || !TryReadInt(nested!, "outputMicrounitsPerThousand", 0, 1_000_000_000, out int output, out errorCode)
            || !TryReadInt(nested!, "reasoningMicrounitsPerThousand", 0, 1_000_000_000, out int reasoning, out errorCode)
            || !TryReadInt(nested!, "cacheReadMicrounitsPerThousand", 0, 1_000_000_000, out int cacheRead, out errorCode))
        {
            errorCode = "model_profile_price_profile_invalid";
            return false;
        }
        priceProfile = new ModelPriceProfile(version, currency, input, output, reasoning, cacheRead);
        return true;
    }

    // ------------------------------------------------------- registry record

    /// <summary>
    /// Parses the registry-version record. It deliberately lives outside the
    /// profile key prefix, so a prefix key filter cannot mistake it for a
    /// profile and a partially published set is never visible.
    /// </summary>
    public static bool TryParseRegistryRecord(
        string? rawValue,
        out ModelProfileRegistryRecord? record,
        out string errorCode)
    {
        record = null;
        errorCode = "model_profile_registry_record_invalid";
        if (rawValue is null
            || rawValue.Length == 0
            || Encoding.UTF8.GetByteCount(rawValue) > MaximumRegistryRecordBytes)
        {
            return false;
        }
        JsonDocument document;
        try
        {
            document = JsonDocument.Parse(
                rawValue,
                new JsonDocumentOptions
                {
                    AllowTrailingCommas = false,
                    CommentHandling = JsonCommentHandling.Disallow,
                    MaxDepth = MaximumJsonDepth,
                });
        }
        catch (JsonException)
        {
            return false;
        }

        using (document)
        {
            if (!TryReadObject(
                document.RootElement,
                [
                    "schemaVersion",
                    "registryVersion",
                    "registryHash",
                    "capabilityHash",
                    "profileIds",
                ],
                out Dictionary<string, JsonElement>? members,
                out errorCode))
            {
                errorCode = "model_profile_registry_record_invalid";
                return false;
            }
            if (!TryReadConst(
                    members!,
                    "schemaVersion",
                    ModelProfileRegistrySnapshot.RecordSchemaVersion,
                    out errorCode)
                || !TryReadLong(members!, "registryVersion", 1, 9_007_199_254_740_991, out long version, out errorCode)
                || !TryReadText(members!, "registryHash", 71, 71, out string registryHash, out errorCode)
                || !RequestGuards.IsSha256(registryHash)
                || !TryReadText(members!, "capabilityHash", 71, 71, out string capabilityHash, out errorCode)
                || !RequestGuards.IsSha256(capabilityHash)
                || !TryReadTokenList(
                    members!,
                    "profileIds",
                    ProfileId(),
                    0,
                    ModelProfile.MaximumProfiles,
                    out string[] profileIds,
                    out errorCode))
            {
                errorCode = "model_profile_registry_record_invalid";
                return false;
            }
            if (profileIds.Distinct(StringComparer.Ordinal).Count() != profileIds.Length)
            {
                errorCode = "model_profile_registry_record_duplicate_id";
                return false;
            }
            record = new ModelProfileRegistryRecord(
                version,
                registryHash,
                capabilityHash,
                profileIds);
            errorCode = "";
            return true;
        }
    }

    // ------------------------------------------------------------ set checks

    /// <summary>
    /// Set-level rules a single profile cannot express. Duplicate visible
    /// display names are refused because two identically named entries in a
    /// picker are indistinguishable to the user, which is exactly the
    /// confusion the catalog exists to prevent.
    /// </summary>
    public static bool ValidateSet(IReadOnlyList<ModelProfile> profiles, out string errorCode)
    {
        ArgumentNullException.ThrowIfNull(profiles);
        errorCode = "";
        if (profiles.Count > ModelProfile.MaximumProfiles)
        {
            errorCode = "model_profile_registry_too_large";
            return false;
        }
        HashSet<string> profileIds = new(StringComparer.Ordinal);
        HashSet<string> visibleDisplayNames = new(StringComparer.Ordinal);
        foreach (ModelProfile profile in profiles)
        {
            if (!profileIds.Add(profile.ProfileId))
            {
                errorCode = "model_profile_id_duplicate";
                return false;
            }
            if (profile.Visible
                && !visibleDisplayNames.Add(NormalizeDisplayName(profile.DisplayName)))
            {
                errorCode = "model_profile_display_name_duplicate";
                return false;
            }
        }
        return true;
    }

    /// <summary>Publisher-equivalent display-name normalization.</summary>
    internal static string NormalizeDisplayName(string value) =>
        CollapseWhitespace().Replace(value.Trim(), " ").ToLowerInvariant();

    // ---------------------------------------------------------------- hashes

    /// <summary>
    /// Hashes one stored profile value exactly as the publisher does: the
    /// preimage is the purpose line followed by the canonical JSON bytes as
    /// stored. Hashing the stored bytes rather than a re-serialization is what
    /// makes the registry hash a true integrity check.
    /// </summary>
    public static string ComputeProfileValueHash(string rawCanonicalValue)
    {
        ArgumentNullException.ThrowIfNull(rawCanonicalValue);
        using IncrementalHash hash = IncrementalHash.CreateHash(HashAlgorithmName.SHA256);
        hash.AppendData(Encoding.UTF8.GetBytes($"sapphirus:{ProfileHashPurpose}:v1\n"));
        hash.AppendData(Encoding.UTF8.GetBytes(rawCanonicalValue));
        return "sha256:" + Convert.ToHexStringLower(hash.GetHashAndReset());
    }

    /// <summary>
    /// Hashes the ordered registry manifest. Profiles are canonicalized in
    /// profile-ID order, so the digest is a property of the set rather than of
    /// the order the store happened to enumerate keys in.
    /// </summary>
    public static string ComputeRegistryHash(
        long registryVersion,
        IReadOnlyList<(string ProfileId, string ProfileHash)> profiles)
    {
        ArgumentNullException.ThrowIfNull(profiles);
        List<object> ordered = [];
        foreach ((string profileId, string profileHash) in profiles
            .OrderBy(static entry => entry.ProfileId, StringComparer.Ordinal))
        {
            ordered.Add(new SortedDictionary<string, object?>(StringComparer.Ordinal)
            {
                ["profileHash"] = profileHash,
                ["profileId"] = profileId,
            });
        }
        return Digest(
            RegistryHashPurpose,
            new SortedDictionary<string, object?>(StringComparer.Ordinal)
            {
                ["profiles"] = ordered,
                ["registryVersion"] = registryVersion,
            });
    }

    /// <summary>
    /// Hashes the capability surface: the part of the registry that decides
    /// what the organization permits, as opposed to what it costs or how large
    /// a response may be. A price change therefore moves the registry hash but
    /// not the capability hash, and a consumer can tell the two apart without
    /// diffing.
    ///
    /// Every value in this surface is drawn from a closed ASCII alphabet the
    /// canonical schema pins, which is why re-serializing it here reproduces
    /// the publisher's bytes exactly. Free text such as the display name and
    /// the description is deliberately not part of it.
    /// </summary>
    public static string ComputeCapabilityHash(IReadOnlyList<ModelProfile> profiles)
    {
        ArgumentNullException.ThrowIfNull(profiles);
        List<object> entries = [];
        foreach (ModelProfile profile in profiles
            .OrderBy(static entry => entry.ProfileId, StringComparer.Ordinal))
        {
            entries.Add(new SortedDictionary<string, object?>(StringComparer.Ordinal)
            {
                ["allowedRegions"] = SortedStrings(profile.AllowedRegions),
                ["canonicalOutputSchemaAllowlist"] =
                    SortedStrings(profile.CanonicalOutputSchemaAllowlist),
                ["deploymentName"] = profile.DeploymentName,
                ["enabled"] = profile.Enabled,
                ["expectedModel"] = new SortedDictionary<string, object?>(StringComparer.Ordinal)
                {
                    ["name"] = profile.ExpectedModel.Name,
                    ["publisher"] = profile.ExpectedModel.Publisher,
                    ["version"] = profile.ExpectedModel.Version,
                },
                ["modelRoleAllowlist"] = SortedStrings(profile.ModelRoleAllowlist),
                ["profileId"] = profile.ProfileId,
                ["purposeAllowlist"] = SortedStrings(profile.PurposeAllowlist),
                ["qualificationStatus"] = ToWireName(profile.Qualification.Status),
                ["requestModeAllowlist"] = SortedStrings(profile.RequestModeAllowlist),
                ["requiredCapabilities"] = new SortedDictionary<string, object?>(StringComparer.Ordinal)
                {
                    ["attachments"] = profile.RequiredCapabilities.Attachments,
                    ["imageInput"] = profile.RequiredCapabilities.ImageInput,
                    ["pdfInput"] = profile.RequiredCapabilities.PdfInput,
                    ["reasoning"] = profile.RequiredCapabilities.Reasoning,
                    ["streaming"] = profile.RequiredCapabilities.Streaming,
                    ["structuredOutput"] = profile.RequiredCapabilities.StructuredOutput,
                    ["textInput"] = profile.RequiredCapabilities.TextInput,
                    ["toolCalling"] = profile.RequiredCapabilities.ToolCalling,
                },
                ["retentionMode"] = profile.RetentionMode,
                ["supportedModalities"] = SortedStrings(profile.SupportedModalities),
                ["visible"] = profile.Visible,
            });
        }
        return Digest(CapabilityHashPurpose, entries);
    }

    /// <summary>The stable wire spelling of a qualification status.</summary>
    public static string ToWireName(ModelQualificationStatus status) => status switch
    {
        ModelQualificationStatus.Qualified => "qualified",
        ModelQualificationStatus.Expired => "expired",
        _ => "unqualified",
    };

    private static List<object> SortedStrings(IReadOnlyList<string> values)
    {
        List<object> sorted = [];
        foreach (string value in values.OrderBy(static item => item, StringComparer.Ordinal))
        {
            sorted.Add(value);
        }
        return sorted;
    }

    /// <summary>
    /// RFC 8785 canonical serialization plus the Sapphirus purpose preimage,
    /// reproducing <c>canonicalHash</c> in
    /// <c>packages/contracts/scripts/lib/canonical-json.mjs</c>.
    /// <c>CanonicalProof</c> cannot be reused: it requires an object root and
    /// the capability surface is an array.
    /// </summary>
    internal static string Digest(string purpose, object? value)
    {
        ArrayBufferWriter<byte> buffer = new();
        using (Utf8JsonWriter writer = new(buffer, new JsonWriterOptions
        {
            Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
        }))
        {
            WriteCanonical(writer, value);
            writer.Flush();
        }
        using IncrementalHash hash = IncrementalHash.CreateHash(HashAlgorithmName.SHA256);
        hash.AppendData(Encoding.UTF8.GetBytes($"sapphirus:{purpose}:v1\n"));
        hash.AppendData(buffer.WrittenSpan);
        return "sha256:" + Convert.ToHexStringLower(hash.GetHashAndReset());
    }

    private static void WriteCanonical(Utf8JsonWriter writer, object? value)
    {
        switch (value)
        {
            case SortedDictionary<string, object?> nested:
                writer.WriteStartObject();
                foreach ((string name, object? entry) in nested)
                {
                    writer.WritePropertyName(name);
                    WriteCanonical(writer, entry);
                }
                writer.WriteEndObject();
                break;
            case IEnumerable<object> array:
                writer.WriteStartArray();
                foreach (object entry in array)
                {
                    WriteCanonical(writer, entry);
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
                    "Unsupported canonical model profile value type.");
        }
    }

    // ------------------------------------------------------------- primitives

    private static bool TryReadObject(
        JsonElement element,
        string[] allowedMembers,
        out Dictionary<string, JsonElement>? members,
        out string errorCode)
    {
        members = null;
        errorCode = "";
        if (element.ValueKind != JsonValueKind.Object)
        {
            errorCode = "model_profile_invalid";
            return false;
        }
        Dictionary<string, JsonElement> read = new(StringComparer.Ordinal);
        foreach (JsonProperty property in element.EnumerateObject())
        {
            if (!allowedMembers.Contains(property.Name, StringComparer.Ordinal))
            {
                errorCode = "model_profile_unknown_member";
                return false;
            }
            if (!read.TryAdd(property.Name, property.Value))
            {
                errorCode = "model_profile_duplicate_member";
                return false;
            }
        }
        foreach (string required in allowedMembers)
        {
            if (!read.ContainsKey(required))
            {
                errorCode = "model_profile_member_missing";
                return false;
            }
        }
        members = read;
        return true;
    }

    private static bool TryReadConst(
        Dictionary<string, JsonElement> members,
        string name,
        string expected,
        out string errorCode)
    {
        errorCode = "";
        JsonElement element = members[name];
        if (element.ValueKind != JsonValueKind.String
            || !string.Equals(element.GetString(), expected, StringComparison.Ordinal))
        {
            errorCode = "model_profile_schema_version_unsupported";
            return false;
        }
        return true;
    }

    private static bool TryReadText(
        Dictionary<string, JsonElement> members,
        string name,
        int minimum,
        int maximum,
        out string value,
        out string errorCode)
    {
        value = "";
        errorCode = "";
        JsonElement element = members[name];
        if (element.ValueKind != JsonValueKind.String)
        {
            errorCode = "model_profile_invalid";
            return false;
        }
        string? text = element.GetString();
        if (text is null || text.Length < minimum || text.Length > maximum)
        {
            errorCode = "model_profile_invalid";
            return false;
        }
        value = text;
        return true;
    }

    private static bool TryReadOptionalText(
        Dictionary<string, JsonElement> members,
        string name,
        int minimum,
        int maximum,
        out string? value,
        out string errorCode)
    {
        value = null;
        errorCode = "";
        if (members[name].ValueKind == JsonValueKind.Null)
        {
            return true;
        }
        bool ok = TryReadText(members, name, minimum, maximum, out string text, out errorCode);
        value = ok ? text : null;
        return ok;
    }

    private static bool TryReadPattern(
        Dictionary<string, JsonElement> members,
        string name,
        Regex pattern,
        int minimum,
        int maximum,
        out string value,
        out string errorCode)
    {
        if (!TryReadText(members, name, minimum, maximum, out value, out errorCode)
            || !pattern.IsMatch(value))
        {
            value = "";
            errorCode = "model_profile_invalid";
            return false;
        }
        return true;
    }

    private static bool TryReadBool(
        Dictionary<string, JsonElement> members,
        string name,
        out bool value,
        out string errorCode)
    {
        errorCode = "";
        value = false;
        JsonValueKind kind = members[name].ValueKind;
        if (kind is not (JsonValueKind.True or JsonValueKind.False))
        {
            errorCode = "model_profile_invalid";
            return false;
        }
        value = kind == JsonValueKind.True;
        return true;
    }

    private static bool TryReadInt(
        Dictionary<string, JsonElement> members,
        string name,
        int minimum,
        int maximum,
        out int value,
        out string errorCode)
    {
        errorCode = "";
        value = 0;
        JsonElement element = members[name];
        if (element.ValueKind != JsonValueKind.Number
            || !element.TryGetInt32(out int parsed)
            || parsed < minimum
            || parsed > maximum)
        {
            errorCode = "model_profile_invalid";
            return false;
        }
        value = parsed;
        return true;
    }

    private static bool TryReadLong(
        Dictionary<string, JsonElement> members,
        string name,
        long minimum,
        long maximum,
        out long value,
        out string errorCode)
    {
        errorCode = "";
        value = 0;
        JsonElement element = members[name];
        if (element.ValueKind != JsonValueKind.Number
            || !element.TryGetInt64(out long parsed)
            || parsed < minimum
            || parsed > maximum)
        {
            errorCode = "model_profile_invalid";
            return false;
        }
        value = parsed;
        return true;
    }

    private static bool TryReadEnum(
        Dictionary<string, JsonElement> members,
        string name,
        string[] allowed,
        out string value,
        out string errorCode)
    {
        if (!TryReadText(members, name, 1, 64, out value, out errorCode)
            || !allowed.Contains(value, StringComparer.Ordinal))
        {
            value = "";
            errorCode = "model_profile_invalid";
            return false;
        }
        return true;
    }

    private static bool TryReadTokenList(
        Dictionary<string, JsonElement> members,
        string name,
        Regex pattern,
        int minimumItems,
        int maximumItems,
        out string[] values,
        out string errorCode)
    {
        values = [];
        errorCode = "";
        JsonElement element = members[name];
        if (element.ValueKind != JsonValueKind.Array)
        {
            errorCode = "model_profile_invalid";
            return false;
        }
        int count = element.GetArrayLength();
        if (count < minimumItems || count > maximumItems)
        {
            errorCode = "model_profile_collection_out_of_bounds";
            return false;
        }
        List<string> parsed = new(count);
        foreach (JsonElement item in element.EnumerateArray())
        {
            if (item.ValueKind != JsonValueKind.String)
            {
                errorCode = "model_profile_invalid";
                return false;
            }
            string? text = item.GetString();
            if (text is null || text.Length is < 1 or > 128 || !pattern.IsMatch(text))
            {
                errorCode = "model_profile_invalid";
                return false;
            }
            parsed.Add(text);
        }
        values = [.. parsed];
        return true;
    }

    private static bool TryReadEnumList(
        Dictionary<string, JsonElement> members,
        string name,
        string[] allowed,
        int minimumItems,
        int maximumItems,
        out string[] values,
        out string errorCode)
    {
        values = [];
        errorCode = "";
        JsonElement element = members[name];
        if (element.ValueKind != JsonValueKind.Array)
        {
            errorCode = "model_profile_invalid";
            return false;
        }
        int count = element.GetArrayLength();
        if (count < minimumItems || count > maximumItems)
        {
            errorCode = "model_profile_collection_out_of_bounds";
            return false;
        }
        List<string> parsed = new(count);
        foreach (JsonElement item in element.EnumerateArray())
        {
            if (item.ValueKind != JsonValueKind.String)
            {
                errorCode = "model_profile_invalid";
                return false;
            }
            string? text = item.GetString();
            if (text is null
                || !allowed.Contains(text, StringComparer.Ordinal)
                || parsed.Contains(text, StringComparer.Ordinal))
            {
                errorCode = "model_profile_invalid";
                return false;
            }
            parsed.Add(text);
        }
        values = [.. parsed];
        return true;
    }

    private static bool TryReadOptionalInstant(
        Dictionary<string, JsonElement> members,
        string name,
        out DateTimeOffset? value,
        out string errorCode)
    {
        value = null;
        errorCode = "";
        JsonElement element = members[name];
        if (element.ValueKind == JsonValueKind.Null)
        {
            return true;
        }
        if (element.ValueKind != JsonValueKind.String
            || !DateTimeOffset.TryParseExact(
                element.GetString(),
                "yyyy-MM-dd'T'HH:mm:ss.fff'Z'",
                CultureInfo.InvariantCulture,
                DateTimeStyles.AssumeUniversal | DateTimeStyles.AdjustToUniversal,
                out DateTimeOffset instant))
        {
            errorCode = "model_profile_invalid";
            return false;
        }
        value = instant;
        return true;
    }

    /// <summary>
    /// Publisher-equivalent deployment-name safety. The regex is necessary but
    /// not sufficient: a name that is not already NFC, or that carries anything
    /// outside printable ASCII, is refused rather than escaped and forwarded.
    /// </summary>
    internal static bool IsSafeDeploymentName(string? name)
    {
        if (name is null || !DeploymentName().IsMatch(name))
        {
            return false;
        }
        if (!name.IsNormalized(NormalizationForm.FormC))
        {
            return false;
        }
        foreach (char character in name)
        {
            if (character is < ' ' or > '~')
            {
                return false;
            }
        }
        return true;
    }

    private static bool IsDegenerateDigest(string value)
    {
        ReadOnlySpan<char> hex = value.AsSpan("sha256:".Length);
        foreach (char character in hex)
        {
            if (character != hex[0])
            {
                return false;
            }
        }
        return true;
    }

    [GeneratedRegex("^[a-z][a-z0-9]*(-[a-z0-9]+){0,7}$")]
    private static partial Regex ProfileId();

    [GeneratedRegex("^[a-z][a-z0-9]*(-[a-z0-9]+){0,3}$")]
    private static partial Regex VariantId();

    [GeneratedRegex("^[a-z][a-z0-9_]{0,63}$")]
    private static partial Regex AllowlistToken();

    [GeneratedRegex("^[a-z][a-z0-9]{0,63}$")]
    private static partial Regex RegionName();

    [GeneratedRegex(@"^[a-z][a-z0-9.-]*\.v[1-9][0-9]*$")]
    private static partial Regex CanonicalSchemaId();

    [GeneratedRegex("^[A-Za-z0-9][A-Za-z0-9_-]{0,126}[A-Za-z0-9]$")]
    private static partial Regex DeploymentName();

    [GeneratedRegex("^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$")]
    private static partial Regex ModelIdentityPart();

    [GeneratedRegex("^[a-z][a-z0-9]*(-[a-z0-9]+)*$")]
    private static partial Regex PriceProfileVersion();

    [GeneratedRegex("^[A-Z]{3}$")]
    private static partial Regex CurrencyCode();

    [GeneratedRegex(@"\s+")]
    private static partial Regex CollapseWhitespace();
}
