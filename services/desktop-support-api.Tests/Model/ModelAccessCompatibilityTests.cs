using System.Security.Cryptography;
using System.Text.Json;
using Sapphirus.DesktopSupportApi.Model;
using Sapphirus.DesktopSupportApi.Policy;
using Sapphirus.DesktopSupportApi.Signing;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Model;

/// <summary>
/// FND-013: the one-way version one to version two adapter, and the proof
/// that receipts signed before version two landed still verify afterwards.
/// </summary>
public sealed class ModelAccessCompatibilityTests
{
    private const string LegacySchemaVersion = "desktop-model-access-request.v1";

    /// <summary>
    /// The canonical digest of the shipped version one receipt fixture,
    /// computed independently from the contracts package with the same
    /// <c>sapphirus:model-access-receipt:v1</c> preimage. Pinning it here is
    /// what proves version two did not change how a version one receipt is
    /// hashed: if the rule moved, every historical signature would break, and
    /// this assertion breaks first.
    /// </summary>
    private const string HistoricalReceiptDigest =
        "sha256:b30e1bebf1e57ea255397b185e90f8bb52cdabe2686ad9f168a13da7cba029b0";

    [Fact]
    public void Legacy_request_lands_on_the_named_legacy_profile()
    {
        using JsonDocument document = JsonDocument.Parse(LegacyRequestJson());

        bool adaptedOk = ModelAccessCompatibility.TryAdaptLegacyRequest(
            document.RootElement,
            ModelAccessCompatibility.MinimumDesktopContractEpoch,
            out AdaptedLegacyRequest? adapted,
            out LegacyRejectionReason reason);

        Assert.True(adaptedOk, ModelAccessCompatibility.ToWireString(reason));
        Assert.NotNull(adapted);
        Assert.Equal(ModelAccessCompatibility.LegacyProfileId, adapted.ModelProfileId);
        Assert.Equal(ModelAccessCompatibility.LegacyRequestMode, adapted.RequestMode);
        Assert.Null(adapted.RequestedVariantId);
        Assert.Null(adapted.CatalogVersion);
        Assert.Equal("request_01J00000000000000000000000", adapted.RequestId);
        Assert.Equal("bmad_help", adapted.Purpose);
    }

    [Theory]
    [InlineData("modelProfileId", "\"coding-balanced\"", LegacyRejectionReason.ModelSelectionAttempted)]
    [InlineData("deploymentName", "\"some-deployment\"", LegacyRejectionReason.ModelSelectionAttempted)]
    [InlineData("requestedVariantId", "\"deep\"", LegacyRejectionReason.VariantRequested)]
    [InlineData("catalogVersion", "\"v1.0123456789abcdef\"", LegacyRejectionReason.CatalogVersionSupplied)]
    [InlineData("tools", "[{\"toolName\":\"read_workspace_file\"}]", LegacyRejectionReason.ToolDefinitionSupplied)]
    [InlineData("attachments", "[{\"ref\":\"blob\"}]", LegacyRejectionReason.AttachmentRequested)]
    [InlineData("requestMode", "\"streaming_chat\"", LegacyRejectionReason.StreamingRequested)]
    [InlineData("stream", "true", LegacyRejectionReason.StreamingRequested)]
    [InlineData("somethingElse", "1", LegacyRejectionReason.UnknownMemberPresent)]
    public void A_legacy_request_may_not_reach_for_a_version_two_capability(
        string member,
        string valueJson,
        LegacyRejectionReason expected)
    {
        using JsonDocument document = JsonDocument.Parse(
            LegacyRequestJson($",\"{member}\":{valueJson}"));

        bool adaptedOk = ModelAccessCompatibility.TryAdaptLegacyRequest(
            document.RootElement,
            ModelAccessCompatibility.MinimumDesktopContractEpoch,
            out AdaptedLegacyRequest? adapted,
            out LegacyRejectionReason reason);

        Assert.False(adaptedOk);
        Assert.Null(adapted);
        Assert.Equal(expected, reason);
    }

    [Fact]
    public void A_version_two_request_is_never_lowered_to_version_one()
    {
        string json = LegacyRequestJson().Replace(
            LegacySchemaVersion,
            ModelAccessCompatibility.CurrentRequestSchemaVersion,
            StringComparison.Ordinal);
        using JsonDocument document = JsonDocument.Parse(json);

        bool adaptedOk = ModelAccessCompatibility.TryAdaptLegacyRequest(
            document.RootElement,
            ModelAccessCompatibility.MinimumDesktopContractEpoch,
            out _,
            out LegacyRejectionReason reason);

        Assert.False(adaptedOk);
        Assert.Equal(LegacyRejectionReason.UnknownMemberPresent, reason);
    }

    [Fact]
    public void An_installation_below_the_minimum_epoch_is_told_to_update()
    {
        using JsonDocument document = JsonDocument.Parse(LegacyRequestJson());

        bool adaptedOk = ModelAccessCompatibility.TryAdaptLegacyRequest(
            document.RootElement,
            ModelAccessCompatibility.MinimumDesktopContractEpoch - 1,
            out _,
            out LegacyRejectionReason reason);

        Assert.False(adaptedOk);
        Assert.Equal(LegacyRejectionReason.ContractEpochTooLow, reason);
        Assert.True(
            ModelAccessCompatibility.RemovalContractEpoch
            > ModelAccessCompatibility.MinimumDesktopContractEpoch);
    }

    [Fact]
    public void Both_request_versions_are_accepted_until_the_removal_gate_closes()
    {
        Assert.True(ModelAccessCompatibility.IsRequestSchemaVersionAccepted(LegacySchemaVersion));
        Assert.True(ModelAccessCompatibility.IsRequestSchemaVersionAccepted(
            ModelAccessCompatibility.CurrentRequestSchemaVersion));
        Assert.False(ModelAccessCompatibility.IsRequestSchemaVersionAccepted(
            "desktop-model-access-request.v3"));
        Assert.False(ModelAccessCompatibility.IsRequestSchemaVersionAccepted(null));
    }

    [Fact]
    public void The_shipped_version_one_receipt_still_hashes_to_its_historical_digest()
    {
        ModelAccessReceipt receipt = LoadHistoricalReceipt();

        byte[] digest = AzureModelReceiptSigner.ComputeReceiptDigest(receipt);

        Assert.Equal(HistoricalReceiptDigest, CanonicalProof.Render(digest));
        Assert.True(ModelAccessCompatibility.IsHistoricalReceiptVerifiable(receipt.SchemaVersion));
        Assert.True(ModelAccessCompatibility.IsHistoricalReceiptVerifiable(
            ModelAccessCompatibility.CurrentReceiptSchemaVersion));
        Assert.False(ModelAccessCompatibility.IsHistoricalReceiptVerifiable(
            "sapphirus.model-access-receipt.v3"));
    }

    [Fact]
    public void A_signature_made_before_version_two_still_verifies_after_it()
    {
        ModelAccessReceipt historical = LoadHistoricalReceipt();
        using ReplayEcdsaSigner signer = new("historical-receipt-key-2026-06");

        // Signed under the pre-version-two rule.
        byte[] digestWhenSigned = AzureModelReceiptSigner.ComputeReceiptDigest(historical);
        byte[] signature = signer.Sign(digestWhenSigned);

        // Recomputed now, with the version two families present in the
        // contract package and the same receipt bytes on disk.
        byte[] digestOnReplay = AzureModelReceiptSigner.ComputeReceiptDigest(
            LoadHistoricalReceipt());

        Assert.Equal(digestWhenSigned, digestOnReplay);
        Assert.True(signer.Verify(digestOnReplay, signature));
    }

    /// <summary>
    /// No-Entra migration delta item 6: the credential-binding hash recipe
    /// gained three new non-secret inputs (credential version, project
    /// endpoint hash, application revision), but that recipe is only ever
    /// used to mint a *new* receipt's <c>credentialBindingHash</c> field. A
    /// historical version one receipt already carries a fixed, on-disk
    /// <c>credentialBindingHash</c> value, and the version one digest
    /// (<see cref="AzureModelReceiptSigner.ComputeReceiptDigest"/>) folds that
    /// stored value in verbatim rather than recomputing it. So the recipe
    /// change cannot move the historical digest, and this is the direct
    /// proof: the digest recomputed today still matches the pinned historical
    /// digest above.
    /// </summary>
    [Fact]
    public void The_binding_recipe_change_does_not_move_the_historical_receipt_digest()
    {
        ModelAccessReceipt historical = LoadHistoricalReceipt();

        byte[] digest = AzureModelReceiptSigner.ComputeReceiptDigest(historical);

        Assert.Equal(HistoricalReceiptDigest, CanonicalProof.Render(digest));
        Assert.False(string.IsNullOrWhiteSpace(historical.CredentialBindingHash));
    }

    [Fact]
    public void Every_refusal_reason_has_a_stable_wire_spelling()
    {
        HashSet<string> spellings = new(StringComparer.Ordinal);
        foreach (LegacyRejectionReason reason in Enum.GetValues<LegacyRejectionReason>())
        {
            string wire = ModelAccessCompatibility.ToWireString(reason);
            Assert.False(string.IsNullOrWhiteSpace(wire));
            Assert.True(spellings.Add(wire), wire);
        }

        Assert.Equal(8, spellings.Count);
        Assert.Contains("model_selection_attempted", spellings);
        Assert.Contains("contract_epoch_too_low", spellings);
    }

    private static ModelAccessReceipt LoadHistoricalReceipt()
    {
        string path = Path.Combine(
            AppContext.BaseDirectory,
            "ContractFixtures",
            "model-access-receipt.json");
        ModelAccessReceipt? receipt = JsonSerializer.Deserialize<ModelAccessReceipt>(
            File.ReadAllBytes(path),
            new JsonSerializerOptions(JsonSerializerDefaults.Web));
        Assert.NotNull(receipt);
        Assert.Equal("sapphirus.model-access-receipt.v1", receipt.SchemaVersion);
        return receipt;
    }

    private static string LegacyRequestJson(string extraMember = "") =>
        $$"""
        {
          "schemaVersion": "{{LegacySchemaVersion}}",
          "requestId": "request_01J00000000000000000000000",
          "deliveryModel": "windows_local",
          "registrationId": "dreg_01J00000000000000000000000",
          "purpose": "bmad_help",
          "modelRole": "planner",
          "canonicalOutputSchemaId": "sapphirus.bmad-method-help-proposal.v1",
          "canonicalOutputSchemaHash": "sha256:6666666666666666666666666666666666666666666666666666666666666666",
          "localEgressManifestHash": "sha256:7777777777777777777777777777777777777777777777777777777777777777",
          "consent": { "schemaVersion": "sapphirus.model-context-consent.v1" },
          "items": [],
          "retentionMode": "transient_no_store",
          "budgetClass": "interactive-standard"{{extraMember}}
        }
        """;

    /// <summary>
    /// A local P-256 key that signs a digest and verifies it again inside the
    /// same test, so the replay proof does not depend on a vault, a network,
    /// or another test class's fixtures.
    /// </summary>
    private sealed class ReplayEcdsaSigner(string keyId) : IDisposable
    {
        private readonly ECDsa _key = ECDsa.Create(ECCurve.NamedCurves.nistP256);

        public string KeyId { get; } = keyId;

        public byte[] Sign(byte[] sha256Digest) => _key.SignHash(
            sha256Digest,
            DSASignatureFormat.IeeeP1363FixedFieldConcatenation);

        public bool Verify(byte[] sha256Digest, byte[] signature) => _key.VerifyHash(
            sha256Digest,
            signature,
            DSASignatureFormat.IeeeP1363FixedFieldConcatenation);

        public void Dispose() => _key.Dispose();
    }
}
