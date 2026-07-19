using System.Security.Cryptography;
using System.Text;
using Sapphirus.DesktopSupportApi.Security;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Security;

public sealed class InstallationConsentVerifierTests
{
    private const string Hash =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    /// <summary>
    /// Pinned in crates/desktop-cloud/tests/consent_envelope_vector.rs; both
    /// languages must produce this digest for the same draft.
    /// </summary>
    private const string GoldenEnvelopeHash =
        "sha256:9789b78a496650d993bfd4d0595924117a9a7ba5beba6285a05608d37f298735";

    private static readonly DateTimeOffset Now =
        DateTimeOffset.Parse("2026-07-16T10:02:00.000Z");

    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    [Fact]
    public void Envelope_hash_matches_the_rust_golden_vector()
    {
        ModelContextConsent consent = CreateConsent(Hash, "dreg_AAAAAAAAAAAAAAAAAAAAAAAAAA");

        Assert.Equal(
            GoldenEnvelopeHash,
            InstallationConsentVerifier.ComputeEnvelopeHash(consent));
    }

    [Fact]
    public async Task A_correctly_signed_consent_verifies()
    {
        SignedFixture fixture = CreateSignedFixture();

        Assert.Equal(
            ContextConsentVerification.Verified,
            await CreateVerifier().VerifyAsync(fixture.Request, Ct));
    }

    [Theory]
    [InlineData("key_id")]
    [InlineData("algorithm")]
    [InlineData("proof_type")]
    [InlineData("signature_encoding")]
    [InlineData("signature_bytes")]
    [InlineData("registration")]
    [InlineData("manifest")]
    [InlineData("purpose")]
    [InlineData("lease_hash")]
    [InlineData("policy_hash")]
    [InlineData("profile_hash")]
    [InlineData("schema_hash")]
    [InlineData("region")]
    [InlineData("retention")]
    [InlineData("nonce")]
    [InlineData("envelope_hash")]
    public async Task Any_tampered_binding_is_rejected(string mutation)
    {
        SignedFixture fixture = CreateSignedFixture();
        ContextConsentVerificationRequest request = Mutate(fixture, mutation);

        Assert.Equal(
            ContextConsentVerification.Rejected,
            await CreateVerifier().VerifyAsync(request, Ct));
    }

    [Fact]
    public async Task A_consent_outside_its_time_window_is_rejected()
    {
        SignedFixture fixture = CreateSignedFixture();
        InstallationConsentVerifier expired = new(
            new FixedTimeProvider(DateTimeOffset.Parse("2026-07-16T10:06:00.000Z")));
        InstallationConsentVerifier early = new(
            new FixedTimeProvider(DateTimeOffset.Parse("2026-07-16T09:59:59.000Z")));

        Assert.Equal(
            ContextConsentVerification.Rejected,
            await expired.VerifyAsync(fixture.Request, Ct));
        Assert.Equal(
            ContextConsentVerification.Rejected,
            await early.VerifyAsync(fixture.Request, Ct));
    }

    [Fact]
    public async Task One_installation_key_cannot_authorize_another_registration()
    {
        SignedFixture fixture = CreateSignedFixture();
        using ECDsa otherKey = ECDsa.Create(ECCurve.NamedCurves.nistP256);
        string otherEncoded = Base64Url(otherKey.ExportSubjectPublicKeyInfo());
        Assert.True(InstallationPublicKey.TryParse(
            otherEncoded,
            out InstallationPublicKey? other));
        RegisteredDevice hijacked = fixture.Device with
        {
            InstallationPublicKey = otherEncoded,
            InstallationPublicKeyHash = other!.Hash,
        };
        ContextConsentVerificationRequest request = fixture.Request with
        {
            Device = hijacked,
        };

        Assert.Equal(
            ContextConsentVerification.Rejected,
            await CreateVerifier().VerifyAsync(request, Ct));
    }

    [Fact]
    public void Only_strict_p256_spki_is_accepted()
    {
        using ECDsa p256 = ECDsa.Create(ECCurve.NamedCurves.nistP256);
        byte[] valid = p256.ExportSubjectPublicKeyInfo();
        Assert.True(InstallationPublicKey.TryParse(Base64Url(valid), out _));

        using ECDsa p384 = ECDsa.Create(ECCurve.NamedCurves.nistP384);
        Assert.False(InstallationPublicKey.TryParse(
            Base64Url(p384.ExportSubjectPublicKeyInfo()),
            out _));

        byte[] trailing = [.. valid, 0x00];
        Assert.False(InstallationPublicKey.TryParse(Base64Url(trailing), out _));
        Assert.False(InstallationPublicKey.TryParse("not base64url!!", out _));
        Assert.False(InstallationPublicKey.TryParse(null, out _));
    }

    [Fact]
    public void Key_projections_redact_key_material()
    {
        SignedFixture fixture = CreateSignedFixture();
        Assert.True(InstallationPublicKey.TryParse(
            fixture.Device.InstallationPublicKey,
            out InstallationPublicKey? key));

        string projection = key!.ToString();
        Assert.Contains(key.Hash, projection);
        Assert.DoesNotContain(fixture.Device.InstallationPublicKey, projection);
    }

    private static InstallationConsentVerifier CreateVerifier() =>
        new(new FixedTimeProvider(Now));

    private sealed record SignedFixture(
        ContextConsentVerificationRequest Request,
        RegisteredDevice Device,
        ECDsa Key);

    private static SignedFixture CreateSignedFixture()
    {
        ECDsa key = ECDsa.Create(ECCurve.NamedCurves.nistP256);
        byte[] spki = key.ExportSubjectPublicKeyInfo();
        string encodedKey = Base64Url(spki);
        string keyHash = "sha256:" + Convert.ToHexStringLower(SHA256.HashData(spki));
        const string registrationId = "dreg_AAAAAAAAAAAAAAAAAAAAAAAAAA";

        ModelContextConsent consent = CreateConsent(keyHash, registrationId);
        string envelopeHash = InstallationConsentVerifier.ComputeEnvelopeHash(consent);
        string signature = Base64Url(key.SignData(
            Encoding.ASCII.GetBytes(envelopeHash),
            HashAlgorithmName.SHA256,
            DSASignatureFormat.IeeeP1363FixedFieldConcatenation));
        consent = consent with
        {
            ConsentEnvelopeHash = envelopeHash,
            Proof = new ModelContextConsentProof(
                "installation_signature",
                "ES256",
                keyHash,
                envelopeHash,
                signature),
        };

        RegisteredDevice device = new(
            "subject",
            registrationId,
            encodedKey,
            keyHash,
            "0.1.0-beta.1",
            "windows",
            "x64",
            1,
            Now,
            DeviceRegistrationState.Active,
            null);
        ModelAccessRequest request = new(
            "desktop-model-access-request.v1",
            consent.RequestId,
            "windows_local",
            registrationId,
            consent.Purpose,
            consent.ModelRole,
            consent.CanonicalOutputSchemaId,
            consent.CanonicalOutputSchemaHash,
            consent.ManifestHash,
            consent,
            [],
            "transient_no_store",
            "interactive-standard");
        return new SignedFixture(
            new ContextConsentVerificationRequest(
                "subject",
                device,
                request,
                consent.ManifestHash),
            device,
            key);
    }

    private static ContextConsentVerificationRequest Mutate(
        SignedFixture fixture,
        string mutation)
    {
        ContextConsentVerificationRequest request = fixture.Request;
        ModelContextConsent consent = request.Request.Consent;
        ModelContextConsent mutated = mutation switch
        {
            "key_id" => consent with
            {
                Proof = consent.Proof with { KeyId = Hash },
            },
            "algorithm" => consent with
            {
                Proof = consent.Proof with { Algorithm = "ES384" },
            },
            "proof_type" => consent with
            {
                Proof = consent.Proof with { ProofType = "operator_signature" },
            },
            "signature_encoding" => consent with
            {
                Proof = consent.Proof with { Signature = "AAECAw" },
            },
            "signature_bytes" => consent with
            {
                Proof = consent.Proof with
                {
                    Signature = Base64Url(new byte[64]),
                },
            },
            "registration" => consent with
            {
                RegistrationId = "dreg_BBBBBBBBBBBBBBBBBBBBBBBBBB",
            },
            "purpose" => consent with { Purpose = "other_purpose" },
            "lease_hash" => consent with { EntitlementLeaseHash = OtherHash() },
            "policy_hash" => consent with { TenantPolicyHash = OtherHash() },
            "profile_hash" => consent with { ModelProfileHash = OtherHash() },
            "schema_hash" => consent with { CanonicalOutputSchemaHash = OtherHash() },
            "region" => consent with { Region = "eastus" },
            "retention" => consent with { RetentionMode = "persistent" },
            "nonce" => consent with { NonceHash = OtherHash() },
            "envelope_hash" => consent with { ConsentEnvelopeHash = OtherHash() },
            "manifest" => consent,
            _ => throw new ArgumentOutOfRangeException(nameof(mutation)),
        };
        return request with
        {
            Request = request.Request with { Consent = mutated },
            RecomputedManifestHash = mutation == "manifest"
                ? OtherHash()
                : request.RecomputedManifestHash,
        };
    }

    private static ModelContextConsent CreateConsent(
        string installationPublicKeyHash,
        string registrationId) => new(
        "sapphirus.model-context-consent.v1",
        "decision_01J00000000000000000000000",
        "request_01J00000000000000000000000",
        "invoke_01J00000000000000000000000",
        "windows_local",
        Hash,
        Hash,
        registrationId,
        installationPublicKeyHash,
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

    private static string OtherHash() => "sha256:" + new string('b', 64);

    private static string Base64Url(byte[] bytes) =>
        Convert.ToBase64String(bytes).TrimEnd('=').Replace('+', '-').Replace('/', '_');

    private sealed class FixedTimeProvider(DateTimeOffset now) : TimeProvider
    {
        public override DateTimeOffset GetUtcNow() => now;
    }
}
