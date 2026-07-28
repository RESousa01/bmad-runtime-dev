using System.Security.Cryptography;
using System.Text;
using Sapphirus.DesktopSupportApi.Foundry;
using Sapphirus.DesktopSupportApi.Model;
using Sapphirus.DesktopSupportApi.Security;
using Sapphirus.DesktopSupportApi.Signing;
using Sapphirus.DesktopSupportApi.Tests.Model;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Security;

/// <summary>
/// The development version two receipt signer is what lets a local run against
/// the real project complete a streaming call. The whole point of these tests is
/// that letting it exist did not make a development receipt confusable with a
/// production one.
///
/// <para>
/// Production verification is modelled exactly as it happens for real: recompute
/// the canonical version two digest, decode the proof as a base64url raw
/// <c>r||s</c> pair, and verify it against the P-256 public key whose private
/// half lives in Key Vault. Nothing about that path is stubbed except the vault
/// itself.
/// </para>
/// </summary>
public sealed class DevelopmentModelReceiptSignerV2Tests
{
    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    [Fact]
    public async Task A_production_signed_receipt_verifies_under_the_production_key()
    {
        using ECDsa key = ECDsa.Create(ECCurve.NamedCurves.nistP256);
        AzureModelReceiptSigner signer = new(new EcdsaHashSigner(key), Options());

        ModelAccessReceiptV2 receipt = await signer.SignAsync(
            "subject-a",
            FoundryResponsesTestFixtures.Authorization(),
            Unsigned(FoundryCredentialMechanisms.ManagedIdentity),
            Ct);

        Assert.True(VerifiesUnderProductionKey(receipt, key));
    }

    /// <summary>
    /// The required proof. The same verification that accepts a vault-signed
    /// receipt rejects a development-signed one.
    /// </summary>
    [Fact]
    public async Task A_development_signed_receipt_fails_production_verification()
    {
        using ECDsa key = ECDsa.Create(ECCurve.NamedCurves.nistP256);
        DevelopmentModelReceiptSignerV2 development = new(Options(signingEnabled: true));

        ModelAccessReceiptV2 receipt = await development.SignAsync(
            "subject-a",
            FoundryResponsesTestFixtures.Authorization(),
            Unsigned(FoundryCredentialMechanisms.DevelopmentApiKey),
            Ct);

        Assert.False(VerifiesUnderProductionKey(receipt, key));
    }

    /// <summary>
    /// It fails for a structural reason rather than by luck: the proof cannot be
    /// read as an ES256 signature at all, and it names an issuer and a key that
    /// resolve to nothing.
    /// </summary>
    [Fact]
    public async Task A_development_proof_is_not_shaped_like_a_key_vault_signature()
    {
        DevelopmentModelReceiptSignerV2 development = new(Options(signingEnabled: true));

        ModelAccessReceiptV2 receipt = await development.SignAsync(
            "subject-a",
            FoundryResponsesTestFixtures.Authorization(),
            Unsigned(FoundryCredentialMechanisms.DevelopmentApiKey),
            Ct);

        byte[] decoded = InstallationPublicKey.DecodeBase64Url(receipt.Proof.Signature);
        Assert.NotEqual(64, decoded.Length);
        Assert.True(DevelopmentModelReceiptSignerV2.IsDevelopmentProof(receipt.Proof));
        Assert.Equal(DevelopmentModelReceiptSignerV2.ProofIssuer, receipt.Proof.Issuer);
        Assert.Equal(DevelopmentModelReceiptSignerV2.ProofKeyId, receipt.Proof.KeyId);
    }

    [Fact]
    public async Task A_vault_signed_proof_is_never_mistaken_for_a_development_proof()
    {
        using ECDsa key = ECDsa.Create(ECCurve.NamedCurves.nistP256);
        AzureModelReceiptSigner signer = new(new EcdsaHashSigner(key), Options());

        ModelAccessReceiptV2 receipt = await signer.SignAsync(
            "subject-a",
            FoundryResponsesTestFixtures.Authorization(),
            Unsigned(FoundryCredentialMechanisms.ManagedIdentity),
            Ct);

        Assert.False(DevelopmentModelReceiptSignerV2.IsDevelopmentProof(receipt.Proof));
    }

    /// <summary>
    /// The credential mechanism is already inside the server-computed binding,
    /// and the development signer copies that binding through rather than
    /// recomputing it. Two otherwise byte-identical calls therefore carry
    /// different credential bindings and different receipt hashes.
    /// </summary>
    [Fact]
    public async Task The_credential_mechanism_separates_the_two_receipts_at_the_hash_level()
    {
        DevelopmentModelReceiptSignerV2 development = new(Options(signingEnabled: true));
        UnsignedModelAccessResultV2 managed =
            Unsigned(FoundryCredentialMechanisms.ManagedIdentity);
        UnsignedModelAccessResultV2 apiKey =
            Unsigned(FoundryCredentialMechanisms.DevelopmentApiKey);

        Assert.NotEqual(
            managed.Binding.CredentialBindingHash,
            apiKey.Binding.CredentialBindingHash);
        Assert.NotEqual(
            managed.Binding.ProviderProfileHash,
            apiKey.Binding.ProviderProfileHash);

        ModelAccessReceiptV2 developmentReceipt = await development.SignAsync(
            "subject-a",
            FoundryResponsesTestFixtures.Authorization(),
            apiKey,
            Ct);

        Assert.Equal(
            apiKey.Binding.CredentialBindingHash,
            developmentReceipt.CredentialBindingHash);
        Assert.NotEqual(
            managed.Binding.CredentialBindingHash,
            developmentReceipt.CredentialBindingHash);
    }

    /// <summary>
    /// Distinguishable is not the same as malformed. A development receipt is
    /// still bound to the request that authorized it and to the route the server
    /// resolved, so the coordinator's post-signature guard accepts it and would
    /// still catch any edit.
    /// </summary>
    [Fact]
    public async Task A_development_receipt_is_still_bound_to_the_authorized_request()
    {
        DevelopmentModelReceiptSignerV2 development = new(Options(signingEnabled: true));
        UnsignedModelAccessResultV2 unsigned =
            Unsigned(FoundryCredentialMechanisms.DevelopmentApiKey);
        ModelReceiptAuthorization authorization =
            FoundryResponsesTestFixtures.Authorization();

        ModelAccessReceiptV2 receipt = await development.SignAsync(
            "subject-a",
            authorization,
            unsigned,
            Ct);

        Assert.True(ModelResultGuards.ValidateReceiptV2(
            authorization,
            receipt,
            unsigned.Binding,
            unsigned.Binding.Region,
            unsigned.PayloadHash,
            unsigned.OutputBytes));
        Assert.False(ModelResultGuards.ValidateReceiptV2(
            authorization,
            receipt with { ResultHash = FoundryResponsesTestFixtures.OtherHash },
            unsigned.Binding,
            unsigned.Binding.Region,
            unsigned.PayloadHash,
            unsigned.OutputBytes));
    }

    [Fact]
    public async Task The_development_signer_refuses_when_development_signing_is_disabled()
    {
        DevelopmentModelReceiptSignerV2 development = new(Options(signingEnabled: false));

        InvalidOperationException error =
            await Assert.ThrowsAsync<InvalidOperationException>(() =>
                development.SignAsync(
                    "subject-a",
                    FoundryResponsesTestFixtures.Authorization(),
                    Unsigned(FoundryCredentialMechanisms.DevelopmentApiKey),
                    Ct));

        Assert.Contains("production", error.Message, StringComparison.Ordinal);
    }

    [Theory]
    [InlineData("terminal_status")]
    [InlineData("run_id")]
    [InlineData("payload_hash")]
    public async Task The_development_signer_refuses_an_ineligible_result(string mutation)
    {
        DevelopmentModelReceiptSignerV2 development = new(Options(signingEnabled: true));
        UnsignedModelAccessResultV2 unsigned =
            Unsigned(FoundryCredentialMechanisms.DevelopmentApiKey);
        UnsignedModelAccessResultV2 ineligible = mutation switch
        {
            "terminal_status" => unsigned with
            {
                TerminalStatus = ModelTerminalStatuses.Cancelled,
            },
            "run_id" => unsigned with { RunId = "not-a-run-id" },
            "payload_hash" => unsigned with
            {
                PayloadHash = FoundryResponsesTestFixtures.OtherHash,
            },
            _ => throw new ArgumentOutOfRangeException(nameof(mutation)),
        };

        await Assert.ThrowsAsync<InvalidOperationException>(() =>
            development.SignAsync(
                "subject-a",
                FoundryResponsesTestFixtures.Authorization(),
                ineligible,
                Ct));
    }

    /// <summary>
    /// Production verification, modelled exactly: recompute the canonical
    /// version two digest and verify the decoded proof against the public half
    /// of the signing key.
    /// </summary>
    private static bool VerifiesUnderProductionKey(ModelAccessReceiptV2 receipt, ECDsa key)
    {
        byte[] signature;
        try
        {
            signature = InstallationPublicKey.DecodeBase64Url(receipt.Proof.Signature);
        }
        catch (FormatException)
        {
            return false;
        }
        if (signature.Length != 64)
        {
            return false;
        }
        return key.VerifyHash(
            AzureModelReceiptSigner.ComputeReceiptDigestV2(receipt),
            signature,
            DSASignatureFormat.IeeeP1363FixedFieldConcatenation);
    }

    private const string Transcript = "A bounded plain-text answer.";

    private static UnsignedModelAccessResultV2 Unsigned(string credentialMechanismId)
    {
        ResolvedModelRoute route = FoundryResponsesTestFixtures.Route();
        ResolvedReceiptBinding binding = ModelReceiptBinding.Create(
            route,
            FoundryResponsesTestFixtures.OtherHash,
            credentialMechanismId,
            "subject-a");
        return new UnsignedModelAccessResultV2(
            "run_01J00000000000000000000000",
            new StreamingChatLane(4, 4),
            Transcript,
            "sha256:" + Convert.ToHexStringLower(
                SHA256.HashData(Encoding.UTF8.GetBytes(Transcript))),
            binding,
            route.ProfileId,
            route.VariantId,
            "gpt-5-mini",
            ModelUsageProjector.Project(null, route.PriceProfile),
            Encoding.UTF8.GetByteCount(Transcript),
            0,
            [],
            null,
            DateTimeOffset.Parse("2026-07-16T10:00:00.000Z"),
            DateTimeOffset.Parse("2026-07-16T10:00:02.000Z"),
            ModelTerminalStatuses.Succeeded);
    }

    private static SupportPlaneOptions Options(bool signingEnabled = false) => new()
    {
        Authority = "https://login.microsoftonline.com/"
            + "11111111-1111-1111-1111-111111111111/v2.0",
        Audience = "api://66666666-6666-6666-6666-666666666666",
        ApprovedDesktopClientId = "22222222-2222-2222-2222-222222222222",
        Region = "westeurope",
        DevelopmentSigningEnabled = signingEnabled,
    };

    /// <summary>
    /// Stands in for the vault: a real P-256 key signing the same digest with
    /// the same raw r||s encoding Key Vault returns.
    /// </summary>
    private sealed class EcdsaHashSigner(ECDsa key) : IHashSigner
    {
        public string KeyId => "https://vault.invalid/keys/model-receipt/0123456789abcdef";

        public Task<string> SignAsync(byte[] sha256Digest, CancellationToken cancellationToken)
        {
            cancellationToken.ThrowIfCancellationRequested();
            byte[] signature = key.SignHash(
                sha256Digest,
                DSASignatureFormat.IeeeP1363FixedFieldConcatenation);
            return Task.FromResult(
                Convert.ToBase64String(signature)
                    .TrimEnd('=')
                    .Replace('+', '-')
                    .Replace('/', '_'));
        }
    }
}
