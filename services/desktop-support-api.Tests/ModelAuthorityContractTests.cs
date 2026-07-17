using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization;
using Sapphirus.DesktopSupportApi;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests;

public sealed class ModelAuthorityContractTests
{
    private const string Hash =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    private static readonly JsonSerializerOptions ContractJson = new(JsonSerializerDefaults.Web)
    {
        UnmappedMemberHandling = JsonUnmappedMemberHandling.Disallow,
    };

    [Theory]
    [InlineData("model-context-consent.json", typeof(ModelContextConsent))]
    [InlineData("model-access-receipt.json", typeof(ModelAccessReceipt))]
    public void Service_dtos_round_trip_the_cross_language_golden_fixture(
        string fixtureName,
        Type contractType)
    {
        string source = File.ReadAllText(
            Path.Combine(AppContext.BaseDirectory, "ContractFixtures", fixtureName));
        object? contract = JsonSerializer.Deserialize(source, contractType, ContractJson);
        JsonNode? expected = JsonNode.Parse(source);
        Assert.NotNull(contract);
        Assert.NotNull(expected);
        JsonNode? actual = JsonSerializer.SerializeToNode(contract, contractType, ContractJson);
        Assert.NotNull(actual);

        Assert.True(
            JsonNode.DeepEquals(expected, actual),
            $"Expected: {expected.ToJsonString()}\nActual: {actual.ToJsonString()}");
    }

    [Fact]
    public void Model_request_requires_consent_bound_to_the_exact_request_and_manifest()
    {
        ModelAccessRequest request = CreateRequest();

        Assert.True(
            RequestGuards.ValidateModelRequest(request, out string validErrorCode, out _),
            validErrorCode);

        ModelAccessRequest substituted = request with
        {
            Consent = request.Consent with
            {
                RequestId = "request_01J00000000000000000000001",
            },
        };
        Assert.False(RequestGuards.ValidateModelRequest(substituted, out string errorCode, out _));
        Assert.Equal("consent_binding_mismatch", errorCode);
    }

    [Fact]
    public void Model_result_requires_a_support_plane_proof_bound_to_the_receipt_hash()
    {
        ModelAccessRequest request = CreateRequest();
        const string payload = "{\"summary\":\"completed\"}";
        string payloadHash = Sha256(payload);
        DateTimeOffset now = DateTimeOffset.Parse("2026-07-16T10:00:01.000Z");
        ModelAccessReceipt receipt = new(
            "sapphirus.model-access-receipt.v1",
            "receipt_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            request.RequestId,
            RequestGuards.Fingerprint(request),
            payloadHash,
            "windows_local",
            request.Consent.TenantHash,
            request.Consent.SubjectHash,
            request.RegistrationId,
            request.LocalEgressManifestHash,
            request.Consent.InvocationBindingHash,
            request.Consent.ConsumptionHash,
            request.Consent.ConsentEnvelopeHash,
            request.Consent.ConsentDisclosureHash,
            request.Consent.ProviderProfileHash,
            request.Consent.ModelProfileHash,
            request.Consent.ModelCapabilityHash,
            request.Consent.DeploymentHash,
            request.CanonicalOutputSchemaId,
            request.CanonicalOutputSchemaHash,
            Hash,
            Hash,
            request.RetentionMode,
            request.Consent.Region,
            request.Items.Sum(item => item.ByteCount),
            Encoding.UTF8.GetByteCount(payload),
            new ModelAccessUsage(2100, 300, 7200, "EUR"),
            0,
            [],
            "provider-request-opaque",
            now,
            now.AddSeconds(1),
            "succeeded",
            Hash,
            new ModelAccessReceiptProof(
                "support_plane_signature",
                "ES256",
                "https://support.sapphirus.example/",
                "sapphirus-desktop",
                "model-receipt-key-2026-07",
                Hash,
                "ZXhhbXBsZS1zdXBwb3J0LXBsYW5lLXNpZ25hdHVyZQ"));
        ModelAccessResult result = new(
            "desktop-model-access-result.v1",
            request.RequestId,
            request.CanonicalOutputSchemaId,
            payload,
            payloadHash,
            receipt);
        RegisteredDevice device = new(
            "subject",
            request.RegistrationId,
            request.Consent.InstallationPublicKeyHash,
            "0.1.0-beta.1",
            "windows",
            "x64",
            "policy-v1",
            now,
            DeviceRegistrationState.Active,
            null);

        ModelResultGuards.ValidateOrThrow(device, request, result, request.Consent.Region);

        ModelAccessResult forged = result with
        {
            Receipt = receipt with
            {
                Proof = receipt.Proof with { SignedPayloadHash = Sha256("forged") },
            },
        };
        Assert.Throws<InvalidOperationException>(() =>
            ModelResultGuards.ValidateOrThrow(device, request, forged, request.Consent.Region));
    }

    [Fact]
    public async Task Receipt_signing_fails_closed_when_only_the_production_placeholder_is_configured()
    {
        DevelopmentModelReceiptSigner signer = new(new SupportPlaneOptions
        {
            Audience = "sapphirus-desktop",
            DevelopmentSigningEnabled = false,
        });
        ModelAccessRequest request = CreateRequest();
        UnsignedModelAccessResult unsigned = new(
            request.CanonicalOutputSchemaId,
            "{\"summary\":\"completed\"}",
            Hash,
            Hash,
            Hash,
            new ModelAccessUsage(1, 1, 0, "EUR"),
            0,
            [],
            null,
            DateTimeOffset.Parse("2026-07-16T10:00:01.000Z"),
            DateTimeOffset.Parse("2026-07-16T10:00:02.000Z"),
            "succeeded");

        await Assert.ThrowsAsync<InvalidOperationException>(() =>
            signer.SignAsync("subject", request, unsigned, TestContext.Current.CancellationToken));
    }

    [Fact]
    public async Task Consent_consumption_is_single_use_across_store_restart()
    {
        string root = Path.Combine(Path.GetTempPath(), "sapphirus-consent-" + Guid.NewGuid().ToString("N"));
        try
        {
            ContextConsentConsumptionRequest consumption = new(
                "subject-partition",
                "dreg_AAAAAAAAAAAAAAAAAAAAAAAAAA",
                "decision_01J00000000000000000000000",
                "request_01J00000000000000000000000",
                Hash,
                DateTimeOffset.Parse("2026-07-16T10:00:00.000Z"));
            DevelopmentFileContextConsentConsumptionStore first = new(root);
            DevelopmentFileContextConsentConsumptionStore restarted = new(root);

            Assert.Equal(
                ContextConsentConsumption.Consumed,
                await first.ConsumeAsync(consumption, TestContext.Current.CancellationToken));
            Assert.Equal(
                ContextConsentConsumption.AlreadyConsumed,
                await restarted.ConsumeAsync(consumption, TestContext.Current.CancellationToken));
        }
        finally
        {
            if (Directory.Exists(root))
            {
                Directory.Delete(root, recursive: true);
            }
        }
    }

    private static ModelAccessRequest CreateRequest()
    {
        const string content = "review this bounded context";
        ModelContextItem item = new(
            "context-item-1",
            "src/example.cs",
            "implementation",
            "csharp",
            Sha256(content),
            Encoding.UTF8.GetByteCount(content),
            "source",
            content);
        ModelContextConsent unboundConsent = new(
            "sapphirus.model-context-consent.v1",
            "decision_01J00000000000000000000000",
            "request_01J00000000000000000000000",
            "invoke_01J00000000000000000000000",
            "windows_local",
            Hash,
            Hash,
            "dreg_AAAAAAAAAAAAAAAAAAAAAAAAAA",
            Hash,
            "lease_01J00000000000000000000000",
            Hash,
            "policy_01J00000000000000000000000",
            7,
            Hash,
            "bmad_help",
            "planner",
            "sapphirus.bmad-method-help-proposal.v1",
            Hash,
            Hash,
            Hash,
            Hash,
            Hash,
            Hash,
            Hash,
            Hash,
            Hash,
            "westeurope",
            "transient_no_store",
            "interactive-standard",
            DateTimeOffset.Parse("2026-07-16T10:00:00.000Z"),
            DateTimeOffset.Parse("2026-07-16T10:00:00.000Z"),
            DateTimeOffset.Parse("2026-07-16T10:05:00.000Z"),
            Hash,
            Hash,
            new ModelContextConsentProof(
                "installation_signature",
                "ES256",
                "installation-key-2026-07",
                Hash,
                "ZXhhbXBsZS1kZXZpY2Utc2lnbmF0dXJl"));
        ModelAccessRequest request = new(
            "desktop-model-access-request.v1",
            unboundConsent.RequestId,
            "windows_local",
            unboundConsent.RegistrationId,
            unboundConsent.Purpose,
            unboundConsent.ModelRole,
            unboundConsent.CanonicalOutputSchemaId,
            unboundConsent.CanonicalOutputSchemaHash,
            Hash,
            unboundConsent,
            [item],
            "transient_no_store",
            "interactive-standard");
        string manifestHash = RequestGuards.ComputeContextManifestHash(request);
        return request with
        {
            LocalEgressManifestHash = manifestHash,
            Consent = request.Consent with { ManifestHash = manifestHash },
        };
    }

    private static string Sha256(string value) => "sha256:" + Convert.ToHexStringLower(
        System.Security.Cryptography.SHA256.HashData(Encoding.UTF8.GetBytes(value)));
}
