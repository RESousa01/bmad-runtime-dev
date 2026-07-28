using System.Security.Cryptography;
using System.Text;
using Sapphirus.DesktopSupportApi.Security;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Security;

/// <summary>
/// The version two half of installation-key consent verification.
///
/// <para>
/// Two properties are proved separately because they fail for different
/// reasons. Editing the decision breaks the ES256 proof over the recomputed
/// envelope. Editing the request instead leaves a perfectly valid signature on
/// a decision that no longer describes the call, and the verifier must refuse
/// that too, on its own, without relying on the admission guard having run.
/// </para>
/// <para>
/// The version one verifier, its draft, and the cross-language golden vector are
/// deliberately untouched by all of this; <see cref="InstallationConsentVerifierTests"/>
/// still pins them.
/// </para>
/// </summary>
public sealed class InstallationModelStreamConsentVerifierTests
{
    private const string Hash =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    private const string OtherHash =
        "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    private const string RegistrationId = "dreg_AAAAAAAAAAAAAAAAAAAAAAAAAA";

    private static readonly DateTimeOffset Now =
        DateTimeOffset.Parse("2026-07-16T10:02:00.000Z");

    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    [Fact]
    public async Task A_correctly_signed_version_two_consent_verifies()
    {
        SignedFixture fixture = CreateSignedFixture();

        Assert.Equal(
            ContextConsentVerification.Verified,
            await CreateVerifier().VerifyAsync(
                "subject",
                fixture.Device,
                fixture.Request,
                Ct));
    }

    [Theory]
    [InlineData("key_id")]
    [InlineData("algorithm")]
    [InlineData("proof_type")]
    [InlineData("signature_encoding")]
    [InlineData("signature_bytes")]
    [InlineData("schema_version")]
    [InlineData("decision_id")]
    [InlineData("invocation_id")]
    [InlineData("tenant_hash")]
    [InlineData("subject_hash")]
    [InlineData("lease_hash")]
    [InlineData("policy_hash")]
    [InlineData("policy_version")]
    [InlineData("provider_profile_hash")]
    [InlineData("disclosure_hash")]
    [InlineData("consumption_hash")]
    [InlineData("invocation_binding_hash")]
    [InlineData("region")]
    [InlineData("issued_at")]
    [InlineData("expires_at")]
    [InlineData("nonce")]
    [InlineData("envelope_hash")]
    public async Task Any_tampered_decision_field_is_rejected(string mutation)
    {
        SignedFixture fixture = CreateSignedFixture();
        ModelAccessRequestV2 request = fixture.Request with
        {
            Consent = MutateConsent(fixture.Request.Consent, mutation),
        };

        Assert.Equal(
            ContextConsentVerification.Rejected,
            await CreateVerifier().VerifyAsync("subject", fixture.Device, request, Ct));
    }

    /// <summary>
    /// Every field the version two schema says the decision binds. The decision
    /// itself is left exactly as signed, so a verifier that only checked the
    /// signature would accept all of these.
    /// </summary>
    [Theory]
    [InlineData("model_profile_id")]
    [InlineData("catalog_version")]
    [InlineData("model_capability_hash")]
    [InlineData("model_profile_hash")]
    [InlineData("requested_variant_id")]
    [InlineData("canonical_output_schema_id")]
    [InlineData("canonical_output_schema_hash")]
    [InlineData("context_snapshot_id")]
    [InlineData("request_mode")]
    [InlineData("request_id")]
    [InlineData("purpose")]
    [InlineData("model_role")]
    [InlineData("manifest_hash")]
    [InlineData("retention_mode")]
    [InlineData("budget_class")]
    public async Task A_validly_signed_decision_cannot_authorize_a_different_request(
        string mutation)
    {
        SignedFixture fixture = CreateSignedFixture();
        ModelAccessRequestV2 request = MutateRequest(fixture.Request, mutation);

        Assert.Equal(
            ContextConsentVerification.Rejected,
            await CreateVerifier().VerifyAsync("subject", fixture.Device, request, Ct));
    }

    [Fact]
    public async Task A_consent_outside_its_time_window_is_rejected()
    {
        SignedFixture fixture = CreateSignedFixture();
        InstallationModelStreamConsentVerifier expired = new(
            new FixedTimeProvider(DateTimeOffset.Parse("2026-07-16T10:06:00.000Z")));
        InstallationModelStreamConsentVerifier early = new(
            new FixedTimeProvider(DateTimeOffset.Parse("2026-07-16T09:59:59.000Z")));

        Assert.Equal(
            ContextConsentVerification.Rejected,
            await expired.VerifyAsync("subject", fixture.Device, fixture.Request, Ct));
        Assert.Equal(
            ContextConsentVerification.Rejected,
            await early.VerifyAsync("subject", fixture.Device, fixture.Request, Ct));
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

        Assert.Equal(
            ContextConsentVerification.Rejected,
            await CreateVerifier().VerifyAsync("subject", hijacked, fixture.Request, Ct));
    }

    [Fact]
    public async Task A_decision_bound_to_another_registration_is_rejected()
    {
        SignedFixture fixture = CreateSignedFixture();
        RegisteredDevice other = fixture.Device with
        {
            RegistrationId = "dreg_BBBBBBBBBBBBBBBBBBBBBBBBBB",
        };

        Assert.Equal(
            ContextConsentVerification.Rejected,
            await CreateVerifier().VerifyAsync("subject", other, fixture.Request, Ct));
    }

    /// <summary>
    /// A decision that names no variant and one that names the literal string
    /// must not collapse into the same preimage. Writing JSON null rather than
    /// omitting the member is what keeps them apart.
    /// </summary>
    [Fact]
    public void An_absent_variant_and_the_literal_string_null_are_different_envelopes()
    {
        ModelContextConsentV2 absent = CreateConsent(Hash) with
        {
            RequestedVariantId = null,
        };
        ModelContextConsentV2 literal = absent with { RequestedVariantId = "null" };

        Assert.NotEqual(
            InstallationModelStreamConsentVerifier.ComputeEnvelopeHash(absent),
            InstallationModelStreamConsentVerifier.ComputeEnvelopeHash(literal));
    }

    /// <summary>
    /// The same decision content, expressed under each version, never produces
    /// the same envelope hash: the drafts are different documents and the two
    /// purpose preimages differ. A signature collected over a version one
    /// envelope therefore cannot be presented as a version two proof.
    /// </summary>
    [Fact]
    public void The_version_two_envelope_is_domain_separated_from_the_version_one_envelope()
    {
        ModelContextConsentV2 consent = CreateConsent(Hash);

        Assert.NotEqual(
            InstallationConsentVerifier.ComputeEnvelopeHash(ToVersionOne(consent)),
            InstallationModelStreamConsentVerifier.ComputeEnvelopeHash(consent));
    }

    private static InstallationModelStreamConsentVerifier CreateVerifier() =>
        new(new FixedTimeProvider(Now));

    private sealed record SignedFixture(
        ModelAccessRequestV2 Request,
        RegisteredDevice Device);

    private static SignedFixture CreateSignedFixture()
    {
        using ECDsa key = ECDsa.Create(ECCurve.NamedCurves.nistP256);
        byte[] spki = key.ExportSubjectPublicKeyInfo();
        string encodedKey = Base64Url(spki);
        string keyHash = "sha256:" + Convert.ToHexStringLower(SHA256.HashData(spki));

        ModelContextConsentV2 consent = CreateConsent(keyHash);
        string envelopeHash =
            InstallationModelStreamConsentVerifier.ComputeEnvelopeHash(consent);
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
            RegistrationId,
            encodedKey,
            keyHash,
            "0.1.0-beta.1",
            "windows",
            "x64",
            1,
            Now,
            DeviceRegistrationState.Active,
            null);
        return new SignedFixture(CreateRequest(consent), device);
    }

    private static ModelContextConsentV2 MutateConsent(
        ModelContextConsentV2 consent,
        string mutation) => mutation switch
        {
            "key_id" => consent with { Proof = consent.Proof with { KeyId = OtherHash } },
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
                Proof = consent.Proof with { Signature = Base64Url(new byte[64]) },
            },
            "schema_version" => consent with
            {
                SchemaVersion = "sapphirus.model-context-consent.v1",
            },
            "decision_id" => consent with
            {
                DecisionId = "decision_01J0000000000000000000000B",
            },
            "invocation_id" => consent with
            {
                InvocationId = "invoke_01J0000000000000000000000B",
            },
            "tenant_hash" => consent with { TenantHash = OtherHash },
            "subject_hash" => consent with { SubjectHash = Hash },
            "lease_hash" => consent with { EntitlementLeaseHash = OtherHash },
            "policy_hash" => consent with { TenantPolicyHash = OtherHash },
            "policy_version" => consent with { TenantPolicyVersion = 8 },
            "provider_profile_hash" => consent with { ProviderProfileHash = OtherHash },
            "disclosure_hash" => consent with { ConsentDisclosureHash = OtherHash },
            "consumption_hash" => consent with { ConsumptionHash = OtherHash },
            "invocation_binding_hash" => consent with { InvocationBindingHash = OtherHash },
            "region" => consent with { Region = "eastus" },
            "issued_at" => consent with
            {
                IssuedAt = DateTimeOffset.Parse("2026-07-16T09:59:00.000Z"),
            },
            "expires_at" => consent with
            {
                ExpiresAt = DateTimeOffset.Parse("2026-07-16T10:09:00.000Z"),
            },
            "nonce" => consent with { NonceHash = OtherHash },
            "envelope_hash" => consent with { ConsentEnvelopeHash = OtherHash },
            _ => throw new ArgumentOutOfRangeException(nameof(mutation)),
        };

    private static ModelAccessRequestV2 MutateRequest(
        ModelAccessRequestV2 request,
        string mutation) => mutation switch
        {
            "model_profile_id" => request with { ModelProfileId = "coding-fast" },
            "catalog_version" => request with { CatalogVersion = "v1.fedcba9876543210" },
            "model_capability_hash" => request with { ModelCapabilityHash = Hash },
            "model_profile_hash" => request with { ModelProfileHash = OtherHash },
            "requested_variant_id" => request with { RequestedVariantId = "fast" },
            "canonical_output_schema_id" => request with
            {
                CanonicalOutputSchemaId = "sapphirus.bmad-method-help-proposal.v1",
            },
            "canonical_output_schema_hash" => request with
            {
                CanonicalOutputSchemaHash = Hash,
            },
            "context_snapshot_id" => request with
            {
                ContextSnapshotId = "snap_BBBBBBBBBBBBBBBB",
            },
            "request_mode" => request with { RequestMode = "structured_response" },
            "request_id" => request with
            {
                RequestId = "request_01J0000000000000000000000B",
            },
            "purpose" => request with { Purpose = "other_purpose" },
            "model_role" => request with { ModelRole = "reviewer" },
            "manifest_hash" => request with { LocalEgressManifestHash = OtherHash },
            "retention_mode" => request with { RetentionMode = "persistent" },
            "budget_class" => request with { BudgetClass = "batch-cheap" },
            _ => throw new ArgumentOutOfRangeException(nameof(mutation)),
        };

    private static ModelAccessRequestV2 CreateRequest(ModelContextConsentV2 consent)
    {
        const string content = "review this bounded context";
        return new ModelAccessRequestV2(
            ModelAccessRequestV2.SchemaVersionValue,
            consent.RequestId,
            "windows_local",
            RegistrationId,
            consent.Purpose,
            consent.ModelRole,
            consent.RequestMode,
            consent.ModelProfileId,
            consent.RequestedVariantId,
            consent.CatalogVersion,
            consent.ModelProfileHash,
            consent.ModelCapabilityHash,
            consent.ContextSnapshotId,
            consent.CanonicalOutputSchemaId,
            consent.CanonicalOutputSchemaHash,
            consent.ManifestHash,
            consent,
            [
                new ModelContextItem(
                    "context-item-1",
                    "src/example.cs",
                    "implementation",
                    "csharp",
                    "sha256:" + Convert.ToHexStringLower(
                        SHA256.HashData(Encoding.UTF8.GetBytes(content))),
                    Encoding.UTF8.GetByteCount(content),
                    "internal",
                    content),
            ],
            [],
            consent.RetentionMode,
            consent.BudgetClass);
    }

    private static ModelContextConsentV2 CreateConsent(string installationPublicKeyHash) => new(
        ModelContextConsentV2.SchemaVersionValue,
        "decision_01J00000000000000000000000",
        "request_01J00000000000000000000000",
        "invoke_01J00000000000000000000000",
        "windows_local",
        Hash,
        OtherHash,
        RegistrationId,
        installationPublicKeyHash,
        "lease_01J00000000000000000000000",
        Hash,
        "policy_01J00000000000000000000000",
        7,
        Hash,
        "bmad_help",
        "planner",
        "streaming_chat",
        "coding-balanced",
        "balanced",
        "v1.0123456789abcdef",
        "snap_AAAAAAAAAAAAAAAA",
        null,
        null,
        Hash,
        Hash,
        Hash,
        Hash,
        Hash,
        Hash,
        OtherHash,
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
            installationPublicKeyHash,
            Hash,
            "ZXhhbXBsZS1kZXZpY2Utc2lnbmF0dXJl"));

    /// <summary>
    /// The same signed facts expressed as a version one decision, so the two
    /// preimages can be compared over identical content.
    /// </summary>
    private static ModelContextConsent ToVersionOne(ModelContextConsentV2 consent) => new(
        consent.SchemaVersion,
        consent.DecisionId,
        consent.RequestId,
        consent.InvocationId,
        consent.DeliveryModel,
        consent.TenantHash,
        consent.SubjectHash,
        consent.RegistrationId,
        consent.InstallationPublicKeyHash,
        consent.EntitlementLeaseId,
        consent.EntitlementLeaseHash,
        consent.TenantPolicyId,
        consent.TenantPolicyVersion,
        consent.TenantPolicyHash,
        consent.Purpose,
        consent.ModelRole,
        consent.CanonicalOutputSchemaId ?? "",
        consent.CanonicalOutputSchemaHash ?? "",
        consent.ManifestHash,
        consent.InvocationBindingHash,
        consent.ConsumptionHash,
        consent.ConsentDisclosureHash,
        consent.ProviderProfileHash,
        consent.ModelProfileHash,
        consent.ModelCapabilityHash,
        Hash,
        consent.Region,
        consent.RetentionMode,
        consent.BudgetClass,
        consent.IssuedAt,
        consent.NotBefore,
        consent.ExpiresAt,
        consent.NonceHash,
        consent.ConsentEnvelopeHash,
        consent.Proof);

    private static string Base64Url(byte[] bytes) =>
        Convert.ToBase64String(bytes).TrimEnd('=').Replace('+', '-').Replace('/', '_');

    private sealed class FixedTimeProvider(DateTimeOffset now) : TimeProvider
    {
        public override DateTimeOffset GetUtcNow() => now;
    }
}
