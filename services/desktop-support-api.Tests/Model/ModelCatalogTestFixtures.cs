using Sapphirus.DesktopSupportApi.Foundry;
using Sapphirus.DesktopSupportApi.Model;
using Sapphirus.DesktopSupportApi.Policy;

namespace Sapphirus.DesktopSupportApi.Tests.Model;

/// <summary>
/// Synthetic organization profile fixtures.
///
/// <para>
/// The canonical profile values below are byte-identical to what
/// <c>tools/foundry-profiles/validate-profiles.mjs</c> emits for the same input
/// documents, and the four pinned digests were computed by that canonicalizer
/// rather than by this code. That is what makes the parity assertions a real
/// cross-language check instead of a restatement of the implementation.
/// </para>
/// <para>
/// Every value is obviously synthetic. No endpoint, tenant, subscription,
/// credential, or real deployment name appears anywhere.
/// </para>
/// </summary>
internal static class ModelCatalogTestFixtures
{
    public const string Region = "testregion";
    public const string AlphaProfileId = "synthetic-alpha";
    public const string BetaProfileId = "synthetic-beta";
    public const string AlphaDeploymentName = "synthetic-deployment-alpha";
    public const string BetaDeploymentName = "synthetic-deployment-beta";
    public const long ParityRegistryVersion = 7;

    public const string ParityRegistryHash =
        "sha256:5f53fb027c35af3c519b56417c3fececd10f8f4306fd1c5a42685eecb6e370ef";

    public const string ParityCapabilityHash =
        "sha256:116bba8a39f92eb1fa49e0de7319d0cebed9c3a5c73b715eb9fad83bd0db0430";

    public const string ParityAlphaProfileHash =
        "sha256:078c0191f9518344decb658a5185a7ccd7d86effa5dbfcdc61705e24cc6a73e8";

    public const string ParityBetaProfileHash =
        "sha256:e738907566dbd99c73bb4c54d1bdb59aea8e38d453215b6366cd0b53c4bd25bd";

    public const string SyntheticEvidenceHash =
        "sha256:1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";

    public const string AlphaDeploymentHash =
        "sha256:aa11bb22cc33dd44ee55ff6677889900aa11bb22cc33dd44ee55ff6677889900";

    public const string BetaDeploymentHash =
        "sha256:bb11cc22dd33ee44ff5500667788990011223344556677889900aabbccddeeff";

    public const string SnapshotHash =
        "sha256:cc11dd22ee33ff445566778899001122334455667788990011223344556677aa";

    public const string PolicyId = "policy_01J00000000000000000000000";

    /// <summary>
    /// One canonical profile value, in the exact member order RFC 8785 fixes.
    /// The template exists once so the two fixtures cannot drift apart.
    /// </summary>
    public static string CanonicalProfileValue(
        string profileId,
        int priority,
        string deploymentName) =>
        $$"""
        {"allowedRegions":["testregion"],"canonicalOutputSchemaAllowlist":["sapphirus.bmad-method-help-proposal.v1"],"deploymentName":"{{deploymentName}}","description":"Synthetic parity fixture for {{profileId}}.","displayName":"Synthetic {{profileId}}","enabled":true,"expectedModel":{"name":"synthetic-model","publisher":"SyntheticPublisher","version":"0000-00-00"},"family":"synthetic-family","limits":{"contextTokens":256000,"maxAttachmentBytes":0,"maxInputTokens":220000,"maxOutputBytes":262144,"maxOutputTokens":32000},"modelRoleAllowlist":["planner","assistant"],"priceProfile":{"cacheReadMicrounitsPerThousand":275,"currency":"EUR","inputMicrounitsPerThousand":2750,"outputMicrounitsPerThousand":11000,"reasoningMicrounitsPerThousand":11000,"version":"synthetic-price-2026-07"},"priority":{{priority}},"profileId":"{{profileId}}","purposeAllowlist":["ordinary_chat","bmad_help"],"qualification":{"evidenceHash":"sha256:1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef","qualifiedAt":"2026-07-28T00:00:00.000Z","status":"qualified"},"requestModeAllowlist":["structured_response","streaming_chat"],"requiredCapabilities":{"attachments":false,"imageInput":false,"pdfInput":false,"reasoning":true,"streaming":true,"structuredOutput":true,"textInput":true,"toolCalling":true},"retentionMode":"transient_no_store","retryPolicy":{"initialBackoffMilliseconds":250,"maximumAttempts":3,"maximumBackoffMilliseconds":4000},"schemaVersion":"sapphirus.model-profile.v1","supportedModalities":["text"],"variants":[{"displayName":"Balanced","maxOutputTokens":16000,"reasoningEffort":"medium","variantId":"balanced","verbosity":"medium"},{"displayName":"Deep","maxOutputTokens":32000,"reasoningEffort":"high","variantId":"deep","verbosity":"high"}],"visible":true}
        """;

    public static string AlphaValue => CanonicalProfileValue(AlphaProfileId, 10, AlphaDeploymentName);

    public static string BetaValue => CanonicalProfileValue(BetaProfileId, 20, BetaDeploymentName);

    /// <summary>The registry-version record exactly as the publisher writes it.</summary>
    public static string RegistryRecordValue(
        long registryVersion,
        string registryHash,
        string capabilityHash,
        params string[] profileIds)
    {
        string ids = string.Join(",", profileIds.Select(static id => "\"" + id + "\""));
        return $$"""
        {"capabilityHash":"{{capabilityHash}}","profileIds":[{{ids}}],"registryHash":"{{registryHash}}","registryVersion":{{registryVersion}},"schemaVersion":"forge.model-profile-registry.v1"}
        """;
    }

    public static string ParityRegistryRecord => RegistryRecordValue(
        ParityRegistryVersion,
        ParityRegistryHash,
        ParityCapabilityHash,
        AlphaProfileId,
        BetaProfileId);

    public static ModelProfileSettings ParitySettings => new(
        new Dictionary<string, string>(StringComparer.Ordinal)
        {
            [AlphaProfileId] = AlphaValue,
            [BetaProfileId] = BetaValue,
        },
        ParityRegistryRecord);

    public static ModelProfile ParseAlpha() => Parse(AlphaValue);

    public static ModelProfile ParseBeta() => Parse(BetaValue);

    public static ModelProfile Parse(string canonicalValue)
    {
        if (!ModelProfileValidator.TryParseProfile(
            canonicalValue,
            out ModelProfile? profile,
            out string errorCode))
        {
            throw new InvalidOperationException(
                $"The fixture profile did not parse: {errorCode}");
        }
        return profile!;
    }

    public static FoundryDeploymentCapabilities FullCapabilities => new(
        TextInput: true,
        ImageInput: false,
        PdfInput: false,
        Attachments: false,
        ToolCalling: true,
        StructuredOutput: true,
        Reasoning: true,
        Streaming: true);

    public static FoundryDeployment Deployment(
        string deploymentName,
        string deploymentHash,
        FoundryDeploymentCapabilities? capabilities = null,
        string modelVersion = "0000-00-00") => new(
            deploymentName,
            FoundryDeployment.ModelDeploymentType,
            "SyntheticPublisher",
            "synthetic-model",
            modelVersion,
            new FoundryDeploymentSku("synthetic-sku", null, null, null, 1),
            capabilities ?? FullCapabilities,
            deploymentHash);

    public static FoundryDeploymentSnapshot Snapshot(params FoundryDeployment[] deployments) => new(
        FoundryDeploymentSnapshot.SchemaVersionValue,
        SnapshotHash,
        DateTimeOffset.UnixEpoch,
        deployments);

    public static PolicySnapshot Policy(
        long policyVersion = 1,
        string[]? allowedRegions = null,
        string[]? approvedDeployments = null) => new(
            PolicyId,
            policyVersion,
            SystemBrowserFallbackAllowed: false,
            MaximumContextBytes: 512 * 1024,
            MaximumContextItems: 64,
            allowedRegions ?? [Region],
            approvedDeployments ?? [],
            ModelProfile.SupportedRetentionMode,
            DateTimeOffset.UnixEpoch);

    /// <summary>
    /// A scripted clock, matching the one the Foundry cache tests already use.
    /// Caching and staleness are time-dependent behaviour, so they are proved
    /// against a clock the test moves rather than against a sleep.
    /// </summary>
    internal sealed class ControlledTimeProvider(DateTimeOffset start) : TimeProvider
    {
        private readonly Lock _gate = new();
        private DateTimeOffset _now = start;

        public override DateTimeOffset GetUtcNow()
        {
            lock (_gate)
            {
                return _now;
            }
        }

        public void Advance(TimeSpan duration)
        {
            lock (_gate)
            {
                _now = _now.Add(duration);
            }
        }
    }

    public static ModelEntitlementSnapshot Entitlement(
        IEnumerable<string> profileIds,
        string entitlementClass = "synthetic-class",
        string? organizationDefaultProfileId = null) => new(
            entitlementClass,
            new HashSet<string>(profileIds, StringComparer.Ordinal),
            organizationDefaultProfileId);
}
